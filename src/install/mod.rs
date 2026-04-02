mod cleanup;
mod deploy;
mod resolve;
mod types;

pub use types::{ConflictResolver, InstallOptions, SourceKind};
pub(crate) use cleanup::cleanup_previous_installation;

use std::io::IsTerminal;
use std::path::Path;

use owo_colors::OwoColorize;

use crate::artifact;
use crate::backend::Backend;
use crate::cache;
use crate::config::Config;
use crate::conflict::Conflict;
use crate::env_check;
use crate::error::{RenkeiError, Result};
use crate::install_cache::{InstallCache, PackageEntry};
use crate::lockfile::{Lockfile, LockfileEntry};
use crate::manifest::{self, Manifest, RequestedScope};

fn prompt_rename(conflict: &Conflict) -> Result<String> {
    let prompt = format!(
        "{} '{}' conflicts with package '{}'. Enter a new name:",
        conflict.artifact_kind, conflict.artifact_name, conflict.owner_package,
    );
    inquire::Text::new(&prompt)
        .with_help_message("The artifact will be deployed under this name")
        .prompt()
        .map_err(|e| RenkeiError::DeploymentFailed(format!("Prompt failed: {e}")))
}

/// Build the conflict resolver based on --force and TTY detection.
fn default_resolver(force: bool) -> Box<ConflictResolver> {
    if force {
        Box::new(|_: &Conflict| Ok(None))
    } else if std::io::stdin().is_terminal() {
        Box::new(|c: &Conflict| prompt_rename(c).map(Some))
    } else {
        Box::new(|c: &Conflict| {
            Err(RenkeiError::ArtifactConflict {
                kind: c.artifact_kind.clone(),
                name: c.artifact_name.clone(),
                owner: c.owner_package.clone(),
            })
        })
    }
}

pub fn install_local(
    package_dir: &Path,
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    options: &InstallOptions,
) -> Result<()> {
    let resolver = default_resolver(options.force);
    install_local_with_resolver(
        package_dir,
        config,
        backends,
        requested_scope,
        options,
        &*resolver,
    )
}

/// Testable core of `install_local` with an injectable conflict resolver.
pub(crate) fn install_local_with_resolver(
    package_dir: &Path,
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    options: &InstallOptions,
    conflict_resolver: &ConflictResolver,
) -> Result<()> {
    let package_dir = package_dir
        .canonicalize()
        .map_err(|_| RenkeiError::ManifestNotFound(package_dir.to_path_buf()))?;

    let raw_manifest = Manifest::from_path(&package_dir)?;
    let manifest = raw_manifest.validate()?;
    manifest::validate_scope(&manifest.install_scope, requested_scope)?;

    // Resolve backends: intersect manifest requirements with detected backends
    let active_backends: Vec<&dyn Backend> = if options.force {
        backends.to_vec()
    } else {
        backends
            .iter()
            .filter(|b| manifest.backends.iter().any(|mb| mb == b.name()))
            .copied()
            .collect()
    };

    if active_backends.is_empty() {
        let detected_names: Vec<&str> = backends.iter().map(|b| b.name()).collect();
        return Err(RenkeiError::BackendNotDetected {
            required: manifest.backends.join(", "),
            detected: if detected_names.is_empty() {
                "none".to_string()
            } else {
                detected_names.join(", ")
            },
        });
    }

    println!(
        "{} {} v{}",
        "Installing".green().bold(),
        manifest.full_name,
        manifest.version
    );

    let artifacts = artifact::discover_artifacts(&package_dir)?;
    if artifacts.is_empty() {
        return Err(RenkeiError::NoArtifactsFound(package_dir));
    }

    let mut install_cache = InstallCache::load(config)?;
    cleanup_previous_installation(&manifest.full_name, &install_cache, config);

    // --- Conflict resolution + rename ---
    let resolved = resolve::resolve_conflicts_and_rename(
        artifacts,
        &mut install_cache,
        &manifest.full_name,
        conflict_resolver,
    )?;

    let (archive_path, integrity) = if options.from_lockfile {
        let path = cache::archive_path(
            config,
            &manifest.scope,
            &manifest.short_name,
            &manifest.version,
        );
        let hash = if path.exists() {
            cache::compute_sha256(&path)?
        } else {
            String::new()
        };
        (path, hash)
    } else {
        cache::create_archive(&package_dir, &manifest, config)?
    };

    // --- Deploy to all backends ---
    let deployment = deploy::deploy_to_backends(
        &resolved.effective,
        &active_backends,
        &raw_manifest,
        config,
    )?;

    install_cache.upsert_package(
        &manifest.full_name,
        PackageEntry {
            version: manifest.version.to_string(),
            source: options.source_kind.as_str().to_string(),
            source_path: match options.source_kind {
                SourceKind::Git => options.source_url.clone(),
                SourceKind::Local => package_dir.to_string_lossy().to_string(),
            },
            integrity,
            archive_path: archive_path.to_string_lossy().to_string(),
            deployed: deployment.deployed_map,
            resolved: options.resolved.clone(),
            tag: options.tag.clone(),
        },
    );
    install_cache.save(config)?;

    if !options.from_lockfile {
        let lockfile_path = config.lockfile_path();
        let mut lockfile = Lockfile::load(&lockfile_path)?;
        lockfile.upsert(
            &manifest.full_name,
            LockfileEntry::from_package_entry(
                install_cache.packages.get(&manifest.full_name).unwrap(),
            ),
        );
        lockfile.save(&lockfile_path)?;
    }

    println!(
        "{} Deployed {} artifact(s) for {}",
        "Done.".green().bold(),
        deployment.all_deployed.len(),
        manifest.full_name
    );
    for d in &deployment.all_deployed {
        println!("  {} {}", "→".dimmed(), d.deployed_path.display());
    }

    if let Some(ref env) = raw_manifest.required_env {
        let missing = env_check::check_required_env(env);
        if !missing.is_empty() {
            env_check::print_env_warnings(&missing);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    use crate::artifact::{Artifact, ArtifactKind};
    use crate::backend::{Backend, DeployedArtifact};
    use crate::conflict;
    use crate::hook::DeployedHookEntry;
    use crate::install_cache::{
        BackendDeployment, DeployedArtifactEntry, InstallCache, PackageEntry,
    };
    use std::collections::HashMap;

    fn make_cache_with_artifacts(artifacts: Vec<(ArtifactKind, &str, &str)>) -> InstallCache {
        let deployed_entries: Vec<DeployedArtifactEntry> = artifacts
            .into_iter()
            .map(|(kind, name, path)| DeployedArtifactEntry {
                artifact_type: kind,
                name: name.to_string(),
                deployed_path: path.to_string(),
                deployed_hooks: vec![],
                original_name: None,
            })
            .collect();
        let mut deployed = HashMap::new();
        deployed.insert(
            "claude".to_string(),
            BackendDeployment {
                artifacts: deployed_entries,
                mcp_servers: vec![],
            },
        );
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg".to_string(),
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/pkg".to_string(),
                integrity: "abc".to_string(),
                archive_path: "/tmp/a.tar.gz".to_string(),
                deployed,
                resolved: None,
                tag: None,
            },
        );
        InstallCache {
            version: 2,
            packages,
        }
    }

    #[test]
    fn test_cleanup_removes_old_artifacts() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let dir = tempdir().unwrap();
        let skill_dir = dir.path().join("renkei-review");
        fs::create_dir_all(&skill_dir).unwrap();
        let file1 = skill_dir.join("SKILL.md");
        let file2 = dir.path().join("agent.md");
        fs::write(&file1, "old skill").unwrap();
        fs::write(&file2, "old agent").unwrap();

        let cache = make_cache_with_artifacts(vec![
            (ArtifactKind::Skill, "review", file1.to_str().unwrap()),
            (ArtifactKind::Agent, "deploy", file2.to_str().unwrap()),
        ]);

        cleanup_previous_installation("@test/pkg", &cache, &config);
        assert!(!file1.exists());
        assert!(!file2.exists());
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_cleanup_noop_on_missing_package() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let cache = InstallCache {
            version: 1,
            packages: HashMap::new(),
        };
        cleanup_previous_installation("@test/nonexistent", &cache, &config);
    }

    #[test]
    fn test_cleanup_tolerates_already_missing_file() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let cache = make_cache_with_artifacts(vec![(
            ArtifactKind::Skill,
            "gone",
            "/tmp/nonexistent/SKILL.md",
        )]);
        cleanup_previous_installation("@test/pkg", &cache, &config);
    }

    #[test]
    fn test_rollback_removes_deployed_files() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("file1.md");
        let file2 = dir.path().join("file2.md");
        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        let deployed = vec![
            DeployedArtifact {
                artifact_kind: ArtifactKind::Skill,
                artifact_name: "s1".to_string(),
                deployed_path: file1.clone(),
                deployed_hooks: vec![],
            },
            DeployedArtifact {
                artifact_kind: ArtifactKind::Skill,
                artifact_name: "s2".to_string(),
                deployed_path: file2.clone(),
                deployed_hooks: vec![],
            },
        ];

        cleanup::rollback(&deployed, &config);
        assert!(!file1.exists());
        assert!(!file2.exists());
    }

    #[test]
    fn test_rollback_removes_empty_parent_dir() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let dir = tempdir().unwrap();
        let skill_dir = dir.path().join("renkei-review");
        fs::create_dir_all(&skill_dir).unwrap();
        let file = skill_dir.join("SKILL.md");
        fs::write(&file, "content").unwrap();

        let deployed = vec![DeployedArtifact {
            artifact_kind: ArtifactKind::Skill,
            artifact_name: "review".to_string(),
            deployed_path: file.clone(),
            deployed_hooks: vec![],
        }];

        cleanup::rollback(&deployed, &config);
        assert!(!file.exists());
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_rollback_skips_missing_files() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nonexistent.md");

        let deployed = vec![DeployedArtifact {
            artifact_kind: ArtifactKind::Skill,
            artifact_name: "ghost".to_string(),
            deployed_path: missing,
            deployed_hooks: vec![],
        }];

        cleanup::rollback(&deployed, &config);
    }

    use crate::backend::claude::ClaudeBackend;
    use std::cell::Cell;

    struct FailingBackend {
        fail_on: usize,
        call_count: Cell<usize>,
    }

    impl FailingBackend {
        fn try_call<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
            let count = self.call_count.get();
            self.call_count.set(count + 1);
            if count >= self.fail_on {
                return Err(RenkeiError::DeploymentFailed("simulated failure".into()));
            }
            f()
        }
    }

    impl Backend for FailingBackend {
        fn name(&self) -> &str {
            "failing"
        }

        fn detect_installed(&self, _config: &Config) -> bool {
            true
        }

        fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            self.try_call(|| ClaudeBackend.deploy_skill(artifact, config))
        }

        fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            self.try_call(|| ClaudeBackend.deploy_agent(artifact, config))
        }

        fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            self.try_call(|| ClaudeBackend.deploy_hook(artifact, config))
        }

        fn register_mcp(
            &self,
            mcp_config: &serde_json::Value,
            config: &Config,
        ) -> Result<Vec<crate::mcp::DeployedMcpEntry>> {
            self.try_call(|| ClaudeBackend.register_mcp(mcp_config, config))
        }
    }

    #[test]
    fn test_rollback_cleans_partial_deploy() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        fs::write(
            pkg.path().join("renkei.json"),
            r#"{"name":"@test/rollback","version":"1.0.0","description":"test","author":"t","license":"MIT","backends":["claude"]}"#,
        )
        .unwrap();

        let skills_dir = pkg.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("lint.md"), "# Lint").unwrap();
        fs::write(skills_dir.join("review.md"), "# Review").unwrap();

        let agents_dir = pkg.path().join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("deploy.md"), "# Deploy").unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let backend = FailingBackend {
            fail_on: 2,
            call_count: Cell::new(0),
        };

        let options = InstallOptions {
            force: true,
            ..InstallOptions::local("/tmp".to_string())
        };
        let result = install_local(
            pkg.path(),
            &config,
            &[&backend as &dyn Backend],
            RequestedScope::Global,
            &options,
        );
        assert!(result.is_err());

        assert!(!home.path().join(".claude/agents/deploy.md").exists());
        assert!(!home
            .path()
            .join(".claude/skills/renkei-lint/SKILL.md")
            .exists());
        assert!(!home.path().join(".claude/skills/renkei-lint").exists());
    }

    fn make_backend_test_pkg(backends_json: &str) -> tempfile::TempDir {
        let pkg = tempdir().unwrap();
        fs::write(
            pkg.path().join("renkei.json"),
            format!(
                r#"{{"name":"@test/pkg","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":{backends_json}}}"#
            ),
        ).unwrap();
        let skills = pkg.path().join("skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("check.md"), "# Check").unwrap();
        pkg
    }

    #[test]
    fn test_backend_detected_succeeds() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let pkg = make_backend_test_pkg(r#"["claude"]"#);

        let config = Config::with_home_dir(home.path().to_path_buf());
        let options = InstallOptions::local("/tmp".to_string());
        let result = install_local(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_backend_not_detected_fails() {
        let home = tempdir().unwrap();
        let pkg = make_backend_test_pkg(r#"["claude"]"#);

        let config = Config::with_home_dir(home.path().to_path_buf());
        let options = InstallOptions::local("/tmp".to_string());
        let empty_backends: Vec<&dyn Backend> = vec![];
        let result = install_local(
            pkg.path(),
            &config,
            &empty_backends,
            RequestedScope::Global,
            &options,
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No compatible backend"));
        assert!(err.contains("--force"));
    }

    #[test]
    fn test_backend_force_bypasses_check() {
        let home = tempdir().unwrap();
        let pkg = make_backend_test_pkg(r#"["claude"]"#);

        let config = Config::with_home_dir(home.path().to_path_buf());
        let options = InstallOptions {
            force: true,
            ..InstallOptions::local("/tmp".to_string())
        };
        let result = install_local(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_backend_multi_with_partial_match() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let pkg = make_backend_test_pkg(r#"["claude","cursor"]"#);

        let config = Config::with_home_dir(home.path().to_path_buf());
        let options = InstallOptions::local("/tmp".to_string());
        let result = install_local(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &options,
        );
        assert!(
            result.is_ok(),
            "Should succeed with at least one matching backend"
        );
    }

    // --- Conflict management tests ---

    fn make_pkg_with_skill(name: &str, skill_name: &str) -> tempfile::TempDir {
        let pkg = tempdir().unwrap();
        fs::write(
            pkg.path().join("renkei.json"),
            format!(
                r#"{{"name":"{name}","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":["claude"]}}"#
            ),
        )
        .unwrap();
        let skills = pkg.path().join("skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(
            skills.join(format!("{skill_name}.md")),
            format!("---\nname: {skill_name}\ndescription: test\n---\nContent of {skill_name}"),
        )
        .unwrap();
        pkg
    }

    fn force_resolver(_: &conflict::Conflict) -> Result<Option<String>> {
        Ok(None)
    }

    fn error_resolver(c: &conflict::Conflict) -> Result<Option<String>> {
        Err(RenkeiError::ArtifactConflict {
            kind: c.artifact_kind.clone(),
            name: c.artifact_name.clone(),
            owner: c.owner_package.clone(),
        })
    }

    fn rename_resolver(
        new_name: &str,
    ) -> impl Fn(&conflict::Conflict) -> Result<Option<String>> + '_ {
        move |_| Ok(Some(new_name.to_string()))
    }

    #[test]
    fn test_conflict_force_overwrites() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Install package A with skill "review"
        let pkg_a = make_pkg_with_skill("@test/conflict-a", "review");
        let opts_a = InstallOptions::local("/tmp/a".to_string());
        install_local_with_resolver(
            pkg_a.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_a,
            &force_resolver,
        )
        .unwrap();

        // Install package B with skill "review" using force resolver (overwrite)
        let pkg_b = make_pkg_with_skill("@test/conflict-b", "review");
        let opts_b = InstallOptions::local("/tmp/b".to_string());
        install_local_with_resolver(
            pkg_b.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_b,
            &force_resolver,
        )
        .unwrap();

        // B's skill should be deployed
        let skill_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
        assert!(skill_path.exists());
        let content = fs::read_to_string(&skill_path).unwrap();
        assert!(content.contains("Content of review"));

        // A's cache should no longer list "review"
        let cache = InstallCache::load(&config).unwrap();
        let a_entry = &cache.packages["@test/conflict-a"];
        assert_eq!(
            a_entry.all_artifacts().count(),
            0,
            "Package A should have no deployed artifacts after force overwrite"
        );

        // B's cache should list "review"
        let b_entry = &cache.packages["@test/conflict-b"];
        let b_arts: Vec<_> = b_entry.all_artifacts().collect();
        assert_eq!(b_arts.len(), 1);
        assert_eq!(b_arts[0].name, "review");
    }

    #[test]
    fn test_conflict_error_resolver_aborts() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Install package A
        let pkg_a = make_pkg_with_skill("@test/conflict-a", "review");
        let opts = InstallOptions::local("/tmp/a".to_string());
        install_local_with_resolver(
            pkg_a.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
            &force_resolver,
        )
        .unwrap();

        // Install package B with error resolver
        let pkg_b = make_pkg_with_skill("@test/conflict-b", "review");
        let opts_b = InstallOptions::local("/tmp/b".to_string());
        let result = install_local_with_resolver(
            pkg_b.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_b,
            &error_resolver,
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("review"));
        assert!(err.contains("@test/conflict-a"));
    }

    #[test]
    fn test_conflict_rename_deploys_under_new_name() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Install package A with skill "review"
        let pkg_a = make_pkg_with_skill("@test/conflict-a", "review");
        let opts = InstallOptions::local("/tmp/a".to_string());
        install_local_with_resolver(
            pkg_a.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
            &force_resolver,
        )
        .unwrap();

        // Install package B with skill "review", rename to "review-v2"
        let pkg_b = make_pkg_with_skill("@test/conflict-b", "review");
        let opts_b = InstallOptions::local("/tmp/b".to_string());
        let resolver = rename_resolver("review-v2");
        install_local_with_resolver(
            pkg_b.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_b,
            &resolver,
        )
        .unwrap();

        // Original skill (pkg A) should still exist
        let a_path = home.path().join(".claude/skills/renkei-review/SKILL.md");
        assert!(a_path.exists());

        // Renamed skill (pkg B) should exist under new name
        let b_path = home.path().join(".claude/skills/renkei-review-v2/SKILL.md");
        assert!(b_path.exists());

        // Verify frontmatter was rewritten
        let content = fs::read_to_string(&b_path).unwrap();
        assert!(content.contains("name: review-v2"));
        assert!(content.contains("Content of review"));
    }

    #[test]
    fn test_conflict_rename_tracks_original_name_in_cache() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Install package A
        let pkg_a = make_pkg_with_skill("@test/conflict-a", "review");
        let opts = InstallOptions::local("/tmp/a".to_string());
        install_local_with_resolver(
            pkg_a.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
            &force_resolver,
        )
        .unwrap();

        // Install package B with rename
        let pkg_b = make_pkg_with_skill("@test/conflict-b", "review");
        let opts_b = InstallOptions::local("/tmp/b".to_string());
        let resolver = rename_resolver("review-v2");
        install_local_with_resolver(
            pkg_b.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_b,
            &resolver,
        )
        .unwrap();

        // Check install-cache
        let cache = InstallCache::load(&config).unwrap();
        let b_entry = &cache.packages["@test/conflict-b"];
        let b_arts: Vec<_> = b_entry.all_artifacts().collect();
        assert_eq!(b_arts[0].name, "review-v2");
        assert_eq!(
            b_arts[0].original_name.as_deref(),
            Some("review")
        );
    }

    #[test]
    fn test_no_conflict_on_reinstall() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let pkg = make_pkg_with_skill("@test/pkg", "review");
        let opts = InstallOptions::local("/tmp".to_string());

        // Install, then reinstall — should succeed with error resolver
        // (which would fail if any conflict was detected)
        install_local_with_resolver(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
            &error_resolver,
        )
        .unwrap();
        install_local_with_resolver(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
            &error_resolver,
        )
        .unwrap();
    }

    #[test]
    fn test_no_conflict_different_skill_names() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let pkg_a = make_pkg_with_skill("@test/pkg-a", "review");
        let pkg_b = make_pkg_with_skill("@test/pkg-b", "lint");
        let opts_a = InstallOptions::local("/tmp/a".to_string());
        let opts_b = InstallOptions::local("/tmp/b".to_string());

        // Both should succeed with error resolver (no conflicts)
        install_local_with_resolver(
            pkg_a.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_a,
            &error_resolver,
        )
        .unwrap();
        install_local_with_resolver(
            pkg_b.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_b,
            &error_resolver,
        )
        .unwrap();

        assert!(home
            .path()
            .join(".claude/skills/renkei-review/SKILL.md")
            .exists());
        assert!(home
            .path()
            .join(".claude/skills/renkei-lint/SKILL.md")
            .exists());
    }

    // --- Deduplication tests ---

    struct ReadsAgentsSkillsBackend;

    impl Backend for ReadsAgentsSkillsBackend {
        fn name(&self) -> &str {
            "reads-agents"
        }

        fn detect_installed(&self, _config: &Config) -> bool {
            true
        }

        fn reads_agents_skills(&self) -> bool {
            true
        }

        fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            // Deploy to .agents/skills/ path (same as agents backend) — should not be called
            // when agents backend is also active (dedup).
            ClaudeBackend.deploy_skill(artifact, config)
        }

        fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            ClaudeBackend.deploy_agent(artifact, config)
        }

        fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            ClaudeBackend.deploy_hook(artifact, config)
        }

        fn register_mcp(
            &self,
            mcp_config: &serde_json::Value,
            config: &Config,
        ) -> Result<Vec<crate::mcp::DeployedMcpEntry>> {
            ClaudeBackend.register_mcp(mcp_config, config)
        }
    }

    struct AgentsFakeBackend;

    impl Backend for AgentsFakeBackend {
        fn name(&self) -> &str {
            "agents"
        }

        fn detect_installed(&self, _: &Config) -> bool {
            true
        }

        fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            ClaudeBackend.deploy_skill(artifact, config)
        }

        fn deploy_agent(&self, _artifact: &Artifact, _config: &Config) -> Result<DeployedArtifact> {
            Err(RenkeiError::DeploymentFailed("agents: unsupported".into()))
        }

        fn deploy_hook(&self, _artifact: &Artifact, _config: &Config) -> Result<DeployedArtifact> {
            Err(RenkeiError::DeploymentFailed("agents: unsupported".into()))
        }

        fn register_mcp(
            &self,
            _mcp_config: &serde_json::Value,
            _config: &Config,
        ) -> Result<Vec<crate::mcp::DeployedMcpEntry>> {
            Err(RenkeiError::DeploymentFailed("agents: unsupported".into()))
        }
    }

    fn make_multi_backend_pkg(backends_json: &str) -> tempfile::TempDir {
        let pkg = tempdir().unwrap();
        fs::write(
            pkg.path().join("renkei.json"),
            format!(
                r#"{{"name":"@test/pkg","version":"1.0.0","description":"t","author":"t","license":"MIT","backends":{backends_json}}}"#
            ),
        )
        .unwrap();
        let skills = pkg.path().join("skills");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("check.md"), "# Check").unwrap();
        let agents = pkg.path().join("agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(agents.join("helper.md"), "# Helper").unwrap();
        pkg
    }

    #[test]
    fn test_dedup_skips_skill_when_agents_and_reads_agents_backend() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let pkg = make_multi_backend_pkg(r#"["agents","reads-agents"]"#);
        let opts = InstallOptions {
            force: true,
            ..InstallOptions::local("/tmp".to_string())
        };

        // Use a flag to detect if deploy_skill was called on reads-agents backend
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        struct TrackingBackend {
            skill_deploy_count: Arc<AtomicUsize>,
        }

        impl Backend for TrackingBackend {
            fn name(&self) -> &str {
                "agents"
            }

            fn detect_installed(&self, _: &Config) -> bool {
                true
            }

            fn deploy_skill(
                &self,
                artifact: &Artifact,
                config: &Config,
            ) -> Result<DeployedArtifact> {
                self.skill_deploy_count.fetch_add(1, Ordering::SeqCst);
                ClaudeBackend.deploy_skill(artifact, config)
            }

            fn deploy_agent(
                &self,
                _: &Artifact,
                _: &Config,
            ) -> Result<DeployedArtifact> {
                Err(RenkeiError::DeploymentFailed("unsupported".into()))
            }

            fn deploy_hook(
                &self,
                _: &Artifact,
                _: &Config,
            ) -> Result<DeployedArtifact> {
                Err(RenkeiError::DeploymentFailed("unsupported".into()))
            }

            fn register_mcp(
                &self,
                _: &serde_json::Value,
                _: &Config,
            ) -> Result<Vec<crate::mcp::DeployedMcpEntry>> {
                Err(RenkeiError::DeploymentFailed("unsupported".into()))
            }
        }

        let agents_skill_count = Arc::new(AtomicUsize::new(0));
        let agents_backend = TrackingBackend {
            skill_deploy_count: agents_skill_count.clone(),
        };

        let result = install_local(
            pkg.path(),
            &config,
            &[&agents_backend as &dyn Backend, &ReadsAgentsSkillsBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
        );
        assert!(result.is_ok(), "{:?}", result);

        // agents backend deploys the skill once; reads-agents backend skips it
        assert_eq!(
            agents_skill_count.load(Ordering::SeqCst),
            1,
            "Agents backend should deploy skill once"
        );
    }

    #[test]
    fn test_no_dedup_when_agents_not_in_active_set() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Only reads-agents backend, no agents backend — should deploy skill normally
        let pkg = make_multi_backend_pkg(r#"["reads-agents"]"#);
        let opts = InstallOptions {
            force: true,
            ..InstallOptions::local("/tmp".to_string())
        };

        let result = install_local(
            pkg.path(),
            &config,
            &[&ReadsAgentsSkillsBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
        );
        // ReadsAgentsSkillsBackend.deploy_skill calls ClaudeBackend.deploy_skill which
        // writes to .claude/skills/ — skill file should exist
        assert!(result.is_ok(), "{:?}", result);
        assert!(home
            .path()
            .join(".claude/skills/renkei-check/SKILL.md")
            .exists());
    }

    #[test]
    fn test_no_dedup_for_agent_artifacts() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        // Even with agents + reads-agents, agents should still be deployed to reads-agents
        let pkg = make_multi_backend_pkg(r#"["agents","reads-agents"]"#);
        let opts = InstallOptions {
            force: true,
            ..InstallOptions::local("/tmp".to_string())
        };

        let result = install_local(
            pkg.path(),
            &config,
            &[
                &AgentsFakeBackend as &dyn Backend,
                &ReadsAgentsSkillsBackend as &dyn Backend,
            ],
            RequestedScope::Global,
            &opts,
        );
        assert!(result.is_ok(), "{:?}", result);

        // reads-agents deployed agent via ClaudeBackend pattern
        assert!(home.path().join(".claude/agents/helper.md").exists());
    }

    // --- Lockfile tests ---

    #[test]
    fn test_install_writes_lockfile_global() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let pkg = make_pkg_with_skill("@test/lockpkg", "review");
        let opts = InstallOptions::local("/tmp/lockpkg".to_string());
        install_local_with_resolver(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
            &force_resolver,
        )
        .unwrap();

        let lockfile_path = home.path().join(".renkei/rk.lock");
        assert!(lockfile_path.exists(), "Lockfile should be created");

        let lockfile = Lockfile::load(&lockfile_path).unwrap();
        assert_eq!(lockfile.lockfile_version, 1);
        let entry = &lockfile.packages["@test/lockpkg"];
        assert_eq!(entry.version, "1.0.0");
        assert!(entry.integrity.starts_with("sha256-"));
    }

    #[test]
    fn test_install_writes_lockfile_project() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::for_project(home.path().to_path_buf(), project.path().to_path_buf());

        let pkg = make_pkg_with_skill("@test/lockpkg", "review");
        let opts = InstallOptions::local("/tmp/lockpkg".to_string());
        install_local_with_resolver(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Project,
            &opts,
            &force_resolver,
        )
        .unwrap();

        let lockfile_path = project.path().join("rk.lock");
        assert!(lockfile_path.exists(), "Project lockfile should be created");

        let lockfile = Lockfile::load(&lockfile_path).unwrap();
        assert!(lockfile.packages.contains_key("@test/lockpkg"));
    }

    #[test]
    fn test_install_two_packages_lockfile_has_both() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let pkg_a = make_pkg_with_skill("@test/pkg-a", "review");
        let pkg_b = make_pkg_with_skill("@test/pkg-b", "lint");
        let opts_a = InstallOptions::local("/tmp/a".to_string());
        let opts_b = InstallOptions::local("/tmp/b".to_string());

        install_local_with_resolver(
            pkg_a.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_a,
            &force_resolver,
        )
        .unwrap();
        install_local_with_resolver(
            pkg_b.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts_b,
            &force_resolver,
        )
        .unwrap();

        let lockfile = Lockfile::load(&config.lockfile_path()).unwrap();
        assert_eq!(lockfile.packages.len(), 2);
        assert!(lockfile.packages.contains_key("@test/pkg-a"));
        assert!(lockfile.packages.contains_key("@test/pkg-b"));
    }

    #[test]
    fn test_reinstall_updates_lockfile_entry() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let pkg = make_pkg_with_skill("@test/pkg", "review");
        let opts = InstallOptions::local("/tmp".to_string());

        // Install twice (reinstall)
        install_local_with_resolver(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
            &force_resolver,
        )
        .unwrap();
        install_local_with_resolver(
            pkg.path(),
            &config,
            &[&ClaudeBackend as &dyn Backend],
            RequestedScope::Global,
            &opts,
            &force_resolver,
        )
        .unwrap();

        let lockfile = Lockfile::load(&config.lockfile_path()).unwrap();
        assert_eq!(lockfile.packages.len(), 1);
        assert_eq!(lockfile.packages["@test/pkg"].version, "1.0.0");
    }
}
