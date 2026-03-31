use std::path::Path;

use owo_colors::OwoColorize;

use crate::artifact::{self, ArtifactKind};
use crate::backend::{Backend, DeployedArtifact};
use crate::cache;
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install_cache::{DeployedArtifactEntry, InstallCache, PackageEntry};
use crate::manifest::Manifest;

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

    let mut install_cache = InstallCache::load(config)?;
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
}
