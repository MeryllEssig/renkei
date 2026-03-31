use std::path::Path;

use owo_colors::OwoColorize;

use crate::artifact::{self, ArtifactKind};
use crate::backend::claude::ClaudeBackend;
use crate::backend::Backend;
use crate::cache;
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install_cache::{DeployedArtifactEntry, InstallCache, PackageEntry};
use crate::manifest::Manifest;

pub fn install_local(package_dir: &Path, config: &Config) -> Result<()> {
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

    let backend = ClaudeBackend;
    let mut deployed = Vec::new();

    for art in &artifacts {
        match art.kind {
            ArtifactKind::Skill => {
                let result = backend.deploy_skill(art, config)?;
                deployed.push(result);
            }
        }
    }

    let mut install_cache = InstallCache::load(config)?;
    let deployed_entries: Vec<DeployedArtifactEntry> = deployed
        .iter()
        .map(|d| DeployedArtifactEntry {
            artifact_type: "skill".to_string(),
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
