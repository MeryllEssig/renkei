use std::path::Path;

use owo_colors::OwoColorize;

use crate::artifact::{self, ArtifactKind};
use crate::backend::{Backend, DeployedArtifact};
use crate::cache;
use crate::config::Config;
use crate::env_check;
use crate::error::{RenkeiError, Result};
use crate::hook;
use crate::install_cache::{DeployedArtifactEntry, InstallCache, PackageEntry};
use crate::manifest::{self, Manifest, RequestedScope};
use crate::mcp;

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

fn cleanup_previous_installation(full_name: &str, install_cache: &InstallCache, config: &Config) {
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

pub fn install_local(
    package_dir: &Path,
    config: &Config,
    backend: &dyn Backend,
    requested_scope: RequestedScope,
) -> Result<()> {
    let package_dir = package_dir
        .canonicalize()
        .map_err(|_| RenkeiError::ManifestNotFound(package_dir.to_path_buf()))?;

    let raw_manifest = Manifest::from_path(&package_dir)?;
    let manifest = raw_manifest.validate()?;
    manifest::validate_scope(&manifest.install_scope, requested_scope)?;

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

    let (archive_path, integrity) = cache::create_archive(&package_dir, &manifest, config)?;

    let mut deployed = Vec::new();

    for art in &artifacts {
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
        .map(|d| DeployedArtifactEntry {
            artifact_type: d.artifact_kind.clone(),
            name: d.artifact_name.clone(),
            deployed_path: d.deployed_path.to_string_lossy().to_string(),
            deployed_hooks: d.deployed_hooks.clone(),
        })
        .collect();

    install_cache.upsert_package(
        &manifest.full_name,
        PackageEntry {
            version: manifest.version.to_string(),
            source: "local".to_string(),
            source_path: package_dir.to_string_lossy().to_string(),
            integrity,
            archive_path: archive_path.to_string_lossy().to_string(),
            deployed_artifacts: deployed_entries,
            deployed_mcp_servers,
            resolved: None,
            tag: None,
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

        let result = install_local(pkg.path(), &config, &backend, RequestedScope::Global);
        assert!(result.is_err());

        assert!(!home.path().join(".claude/agents/deploy.md").exists());
        assert!(!home
            .path()
            .join(".claude/skills/renkei-lint/SKILL.md")
            .exists());
        assert!(!home.path().join(".claude/skills/renkei-lint").exists());
    }
}
