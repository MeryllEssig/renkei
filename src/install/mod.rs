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
mod tests;
