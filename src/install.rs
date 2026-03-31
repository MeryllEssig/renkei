use std::path::Path;

use owo_colors::OwoColorize;

use crate::artifact::{self, ArtifactKind};
use crate::backend::{Backend, DeployedArtifact};
use crate::cache;
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install_cache::{DeployedArtifactEntry, InstallCache, PackageEntry};
use crate::manifest::Manifest;

fn cleanup_previous_installation(full_name: &str, install_cache: &InstallCache) {
    if let Some(entry) = install_cache.packages.get(full_name) {
        for artifact in &entry.deployed_artifacts {
            let path = std::path::PathBuf::from(&artifact.deployed_path);
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
            if let Some(parent) = path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}

fn rollback(deployed: &[DeployedArtifact]) {
    for artifact in deployed.iter().rev() {
        let path = &artifact.deployed_path;
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        if let Some(parent) = path.parent() {
            // Only removes if empty — safe no-op otherwise
            let _ = std::fs::remove_dir(parent);
        }
    }
}

pub fn install_local(package_dir: &Path, config: &Config, backend: &dyn Backend) -> Result<()> {
    let package_dir = package_dir
        .canonicalize()
        .map_err(|_| RenkeiError::ManifestNotFound(package_dir.to_path_buf()))?;

    let raw_manifest = Manifest::from_path(&package_dir)?;
    let manifest = raw_manifest.validate()?;

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
    cleanup_previous_installation(&manifest.full_name, &install_cache);

    let (archive_path, integrity) = cache::create_archive(&package_dir, &manifest, config)?;

    let mut deployed = Vec::new();

    for art in &artifacts {
        let result = match art.kind {
            ArtifactKind::Skill => backend.deploy_skill(art, config),
            ArtifactKind::Agent => backend.deploy_agent(art, config),
        };
        match result {
            Ok(d) => deployed.push(d),
            Err(e) => {
                rollback(&deployed);
                return Err(e);
            }
        }
    }
    let deployed_entries: Vec<DeployedArtifactEntry> = deployed
        .iter()
        .map(|d| DeployedArtifactEntry {
            artifact_type: d.artifact_type.clone(),
            name: d.artifact_name.clone(),
            deployed_path: d.deployed_path.to_string_lossy().to_string(),
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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    use crate::install_cache::{DeployedArtifactEntry, InstallCache, PackageEntry};
    use std::collections::HashMap;

    fn make_cache_with_artifacts(artifacts: Vec<(&str, &str, &str)>) -> InstallCache {
        let deployed: Vec<DeployedArtifactEntry> = artifacts
            .into_iter()
            .map(|(atype, name, path)| DeployedArtifactEntry {
                artifact_type: atype.to_string(),
                name: name.to_string(),
                deployed_path: path.to_string(),
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
            },
        );
        InstallCache {
            version: 1,
            packages,
        }
    }

    #[test]
    fn test_cleanup_removes_old_artifacts() {
        let dir = tempdir().unwrap();
        let skill_dir = dir.path().join("renkei-review");
        fs::create_dir_all(&skill_dir).unwrap();
        let file1 = skill_dir.join("SKILL.md");
        let file2 = dir.path().join("agent.md");
        fs::write(&file1, "old skill").unwrap();
        fs::write(&file2, "old agent").unwrap();

        let cache = make_cache_with_artifacts(vec![
            ("skill", "review", file1.to_str().unwrap()),
            ("agent", "deploy", file2.to_str().unwrap()),
        ]);

        cleanup_previous_installation("@test/pkg", &cache);
        assert!(!file1.exists());
        assert!(!file2.exists());
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_cleanup_noop_on_missing_package() {
        let cache = InstallCache {
            version: 1,
            packages: HashMap::new(),
        };
        // Should not panic
        cleanup_previous_installation("@test/nonexistent", &cache);
    }

    #[test]
    fn test_cleanup_tolerates_already_missing_file() {
        let cache = make_cache_with_artifacts(vec![("skill", "gone", "/tmp/nonexistent/SKILL.md")]);
        // Should not panic
        cleanup_previous_installation("@test/pkg", &cache);
    }

    #[test]
    fn test_rollback_removes_deployed_files() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("file1.md");
        let file2 = dir.path().join("file2.md");
        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        let deployed = vec![
            DeployedArtifact {
                artifact_type: "skill".to_string(),
                artifact_name: "s1".to_string(),
                deployed_path: file1.clone(),
            },
            DeployedArtifact {
                artifact_type: "skill".to_string(),
                artifact_name: "s2".to_string(),
                deployed_path: file2.clone(),
            },
        ];

        rollback(&deployed);
        assert!(!file1.exists());
        assert!(!file2.exists());
    }

    #[test]
    fn test_rollback_removes_empty_parent_dir() {
        let dir = tempdir().unwrap();
        let skill_dir = dir.path().join("renkei-review");
        fs::create_dir_all(&skill_dir).unwrap();
        let file = skill_dir.join("SKILL.md");
        fs::write(&file, "content").unwrap();

        let deployed = vec![DeployedArtifact {
            artifact_type: "skill".to_string(),
            artifact_name: "review".to_string(),
            deployed_path: file.clone(),
        }];

        rollback(&deployed);
        assert!(!file.exists());
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_rollback_skips_missing_files() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nonexistent.md");

        let deployed = vec![DeployedArtifact {
            artifact_type: "skill".to_string(),
            artifact_name: "ghost".to_string(),
            deployed_path: missing,
        }];

        // Should not panic
        rollback(&deployed);
    }

    use crate::artifact::{Artifact, ArtifactKind};
    use crate::backend::claude::ClaudeBackend;
    use std::cell::Cell;

    struct FailingBackend {
        fail_on: usize,
        call_count: Cell<usize>,
    }

    impl Backend for FailingBackend {
        fn name(&self) -> &str {
            "failing"
        }

        fn detect_installed(&self, _config: &Config) -> bool {
            true
        }

        fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            let count = self.call_count.get();
            self.call_count.set(count + 1);
            if count >= self.fail_on {
                return Err(RenkeiError::DeploymentFailed("simulated failure".into()));
            }
            ClaudeBackend.deploy_skill(artifact, config)
        }

        fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
            let count = self.call_count.get();
            self.call_count.set(count + 1);
            if count >= self.fail_on {
                return Err(RenkeiError::DeploymentFailed("simulated failure".into()));
            }
            ClaudeBackend.deploy_agent(artifact, config)
        }
    }

    #[test]
    fn test_rollback_cleans_partial_deploy() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        // Create a package with 2 skills + 1 agent
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
        // Fail on the 3rd artifact (sorted: deploy(agent), lint(skill), review(skill))
        // deploy is first (alphabetical), lint second, review third
        let backend = FailingBackend {
            fail_on: 2,
            call_count: Cell::new(0),
        };

        let result = install_local(pkg.path(), &config, &backend);
        assert!(result.is_err());

        // First two deployed files should have been rolled back
        assert!(!home.path().join(".claude/agents/deploy.md").exists());
        assert!(!home
            .path()
            .join(".claude/skills/renkei-lint/SKILL.md")
            .exists());
        // The parent dir for lint should also be cleaned
        assert!(!home.path().join(".claude/skills/renkei-lint").exists());
    }
}
