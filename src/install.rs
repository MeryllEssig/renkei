use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::Path;

use owo_colors::OwoColorize;

use crate::artifact::{self, Artifact, ArtifactKind};
use crate::backend::{Backend, DeployedArtifact};
use crate::cache;
use crate::config::Config;
use crate::conflict::{self, Conflict};
use crate::env_check;
use crate::error::{RenkeiError, Result};
use crate::frontmatter;
use crate::hook;
use crate::install_cache::{DeployedArtifactEntry, InstallCache, PackageEntry};
use crate::manifest::{self, Manifest, RequestedScope};
use crate::mcp;

#[derive(Debug, Clone, PartialEq)]
pub enum SourceKind {
    Local,
    Git,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceKind::Local => "local",
            SourceKind::Git => "git",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub force: bool,
    pub source_kind: SourceKind,
    pub source_url: String,
    pub resolved: Option<String>,
    pub tag: Option<String>,
}

impl InstallOptions {
    pub fn local(source_path: String) -> Self {
        Self {
            force: false,
            source_kind: SourceKind::Local,
            source_url: source_path,
            resolved: None,
            tag: None,
        }
    }

    pub fn git(url: String, resolved: String, tag: Option<String>) -> Self {
        Self {
            force: false,
            source_kind: SourceKind::Git,
            source_url: url,
            resolved: Some(resolved),
            tag,
        }
    }
}

fn remove_artifact_file(path: &Path) {
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

fn undo_artifact(
    kind: &ArtifactKind,
    path: &Path,
    hooks: &[hook::DeployedHookEntry],
    config: &Config,
) {
    match kind {
        ArtifactKind::Hook => {
            let _ = hook::remove_hooks_from_settings(&config.claude_settings_path(), hooks);
        }
        _ => remove_artifact_file(path),
    }
}

pub(crate) fn cleanup_previous_installation(full_name: &str, install_cache: &InstallCache, config: &Config) {
    if let Some(entry) = install_cache.packages.get(full_name) {
        for artifact in &entry.deployed_artifacts {
            undo_artifact(
                &artifact.artifact_type,
                Path::new(&artifact.deployed_path),
                &artifact.deployed_hooks,
                config,
            );
        }
        if !entry.deployed_mcp_servers.is_empty() {
            let mcp_entries: Vec<mcp::DeployedMcpEntry> = entry
                .deployed_mcp_servers
                .iter()
                .map(|name| mcp::DeployedMcpEntry {
                    server_name: name.clone(),
                })
                .collect();
            let _ = mcp::remove_mcp_from_config(&config.claude_config_path(), &mcp_entries);
        }
    }
}

fn rollback(deployed: &[DeployedArtifact], config: &Config) {
    for artifact in deployed.iter().rev() {
        undo_artifact(
            &artifact.artifact_kind,
            &artifact.deployed_path,
            &artifact.deployed_hooks,
            config,
        );
    }
}

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

type ConflictResolver = dyn Fn(&Conflict) -> Result<Option<String>>;

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
    backend: &dyn Backend,
    requested_scope: RequestedScope,
    options: &InstallOptions,
) -> Result<()> {
    let resolver = default_resolver(options.force);
    install_local_with_resolver(
        package_dir,
        config,
        backend,
        requested_scope,
        options,
        &*resolver,
    )
}

/// Testable core of `install_local` with an injectable conflict resolver.
pub(crate) fn install_local_with_resolver(
    package_dir: &Path,
    config: &Config,
    backend: &dyn Backend,
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

    // Backend detection
    if !options.force {
        let is_installed = backend.detect_installed(config);
        let has_match = is_installed && manifest.backends.iter().any(|b| b == backend.name());
        if !has_match {
            return Err(RenkeiError::BackendNotDetected {
                required: manifest.backends.join(", "),
                detected: if is_installed {
                    backend.name().to_string()
                } else {
                    "none".to_string()
                },
            });
        }
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

    // --- Conflict detection ---
    let conflicts = conflict::detect_conflicts(&artifacts, &install_cache, &manifest.full_name);

    // Resolve conflicts: build a rename map (original_name -> new_name)
    // and clean up previous owners when force-overwriting
    let mut renames: HashMap<(ArtifactKind, String), String> = HashMap::new();
    for c in &conflicts {
        match conflict_resolver(c)? {
            None => {
                // Force overwrite: remove the artifact from the previous owner's cache
                if let Some(owner_entry) = install_cache.packages.get_mut(&c.owner_package) {
                    owner_entry.deployed_artifacts.retain(|a| {
                        !(a.artifact_type == c.artifact_kind && a.name == c.artifact_name)
                    });
                }
            }
            Some(new_name) => {
                renames.insert((c.artifact_kind.clone(), c.artifact_name.clone()), new_name);
            }
        }
    }

    let (archive_path, integrity) = cache::create_archive(&package_dir, &manifest, config)?;

    // Build effective artifacts (apply renames).
    // Hold temp files alive until deployment completes (they are deleted on Drop).
    let mut temp_files: Vec<tempfile::NamedTempFile> = Vec::new();
    let effective_artifacts: Vec<(Artifact, Option<String>)> = artifacts
        .into_iter()
        .map(|art| {
            let key = (art.kind.clone(), art.name.clone());
            if let Some(new_name) = renames.get(&key) {
                // Read source, rewrite frontmatter, write to temp file
                let content = std::fs::read_to_string(&art.source_path).map_err(|e| {
                    RenkeiError::DeploymentFailed(format!(
                        "Cannot read {}: {e}",
                        art.source_path.display()
                    ))
                })?;
                let rewritten = frontmatter::replace_frontmatter_name(&content, new_name)?;

                let mut tmp = tempfile::NamedTempFile::new().map_err(|e| {
                    RenkeiError::DeploymentFailed(format!("Cannot create temp file: {e}"))
                })?;
                std::io::Write::write_all(&mut tmp, rewritten.as_bytes()).map_err(|e| {
                    RenkeiError::DeploymentFailed(format!("Cannot write temp file: {e}"))
                })?;

                let original_name = art.name;
                let renamed_artifact = Artifact {
                    kind: art.kind,
                    name: new_name.to_string(),
                    source_path: tmp.path().to_path_buf(),
                };
                temp_files.push(tmp);
                Ok((renamed_artifact, Some(original_name)))
            } else {
                Ok((art, None))
            }
        })
        .collect::<Result<Vec<_>>>()?;

    let mut deployed = Vec::new();

    for (art, _) in &effective_artifacts {
        let result = match art.kind {
            ArtifactKind::Skill => backend.deploy_skill(art, config),
            ArtifactKind::Agent => backend.deploy_agent(art, config),
            ArtifactKind::Hook => backend.deploy_hook(art, config),
        };
        match result {
            Ok(d) => deployed.push(d),
            Err(e) => {
                rollback(&deployed, config);
                return Err(e);
            }
        }
    }

    let deployed_mcp_servers = if let Some(ref mcp) = raw_manifest.mcp {
        match backend.register_mcp(mcp, config) {
            Ok(entries) => entries.into_iter().map(|e| e.server_name).collect(),
            Err(e) => {
                rollback(&deployed, config);
                return Err(e);
            }
        }
    } else {
        vec![]
    };

    let deployed_entries: Vec<DeployedArtifactEntry> = deployed
        .iter()
        .zip(effective_artifacts.iter())
        .map(|(d, (_, original))| DeployedArtifactEntry {
            artifact_type: d.artifact_kind.clone(),
            name: d.artifact_name.clone(),
            deployed_path: d.deployed_path.to_string_lossy().to_string(),
            deployed_hooks: d.deployed_hooks.clone(),
            original_name: original.clone(),
        })
        .collect();

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
            deployed_artifacts: deployed_entries,
            deployed_mcp_servers,
            resolved: options.resolved.clone(),
            tag: options.tag.clone(),
        },
    );
    install_cache.save(config)?;

    println!(
        "{} Deployed {} artifact(s) for {}",
        "Done.".green().bold(),
        deployed.len(),
        manifest.full_name
    );
    for d in &deployed {
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

    use crate::artifact::ArtifactKind;
    use crate::install_cache::{DeployedArtifactEntry, InstallCache, PackageEntry};
    use std::collections::HashMap;

    fn make_cache_with_artifacts(artifacts: Vec<(ArtifactKind, &str, &str)>) -> InstallCache {
        let deployed: Vec<DeployedArtifactEntry> = artifacts
            .into_iter()
            .map(|(kind, name, path)| DeployedArtifactEntry {
                artifact_type: kind,
                name: name.to_string(),
                deployed_path: path.to_string(),
                deployed_hooks: vec![],
                original_name: None,
            })
            .collect();
        let mut packages = HashMap::new();
        packages.insert(
            "@test/pkg".to_string(),
            PackageEntry {
                version: "1.0.0".to_string(),
                source: "local".to_string(),
                source_path: "/tmp/pkg".to_string(),
                integrity: "abc".to_string(),
                archive_path: "/tmp/a.tar.gz".to_string(),
                deployed_artifacts: deployed,
                deployed_mcp_servers: vec![],
                resolved: None,
                tag: None,
            },
        );
        InstallCache {
            version: 1,
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

        rollback(&deployed, &config);
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

        rollback(&deployed, &config);
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

        rollback(&deployed, &config);
    }

    use crate::artifact::Artifact;
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
            &backend,
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
            &ClaudeBackend,
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
        let result = install_local(
            pkg.path(),
            &config,
            &ClaudeBackend,
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
            &ClaudeBackend,
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
            &ClaudeBackend,
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
            &ClaudeBackend,
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
            &ClaudeBackend,
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
        assert!(
            a_entry.deployed_artifacts.is_empty(),
            "Package A should have no deployed artifacts after force overwrite"
        );

        // B's cache should list "review"
        let b_entry = &cache.packages["@test/conflict-b"];
        assert_eq!(b_entry.deployed_artifacts.len(), 1);
        assert_eq!(b_entry.deployed_artifacts[0].name, "review");
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
            &ClaudeBackend,
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
            &ClaudeBackend,
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
            &ClaudeBackend,
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
            &ClaudeBackend,
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
            &ClaudeBackend,
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
            &ClaudeBackend,
            RequestedScope::Global,
            &opts_b,
            &resolver,
        )
        .unwrap();

        // Check install-cache
        let cache = InstallCache::load(&config).unwrap();
        let b_entry = &cache.packages["@test/conflict-b"];
        assert_eq!(b_entry.deployed_artifacts[0].name, "review-v2");
        assert_eq!(
            b_entry.deployed_artifacts[0].original_name.as_deref(),
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
            &ClaudeBackend,
            RequestedScope::Global,
            &opts,
            &error_resolver,
        )
        .unwrap();
        install_local_with_resolver(
            pkg.path(),
            &config,
            &ClaudeBackend,
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
            &ClaudeBackend,
            RequestedScope::Global,
            &opts_a,
            &error_resolver,
        )
        .unwrap();
        install_local_with_resolver(
            pkg_b.path(),
            &config,
            &ClaudeBackend,
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
}
