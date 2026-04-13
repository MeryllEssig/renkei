pub(crate) mod batch;
mod cleanup;
mod deploy;
pub(crate) mod messages;
pub(crate) mod pipeline;
mod resolve;
mod types;

pub(crate) use cleanup::cleanup_previous_installation;
pub use types::{ConflictResolver, InstallOptions, SourceInfo, SourceKind};

use std::io::IsTerminal;
use std::path::Path;

use owo_colors::OwoColorize;

use crate::backend::Backend;
use crate::cache;
use crate::config::Config;
use crate::conflict::Conflict;
use crate::env_check;
use crate::error::{RenkeiError, Result};
use crate::install_cache::PackageEntry;
use crate::manifest::{self, RequestedScope};
use crate::package_store::PackageStore;

use pipeline::CorePipeline;

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
pub(crate) fn default_resolver(force: bool) -> Box<ConflictResolver> {
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
    let postinstall = install_local_with_resolver(
        package_dir,
        config,
        backends,
        requested_scope,
        options,
        &*resolver,
    )?;
    if let Some(msg) = postinstall {
        print_postinstall_block(&msg, None);
    }
    Ok(())
}

/// Testable core of `install_local` with an injectable conflict resolver.
///
/// Returns the package's optional `messages.postinstall` string so callers
/// (single-package wrapper, workspace coordinator, lockfile coordinator)
/// can decide *when* to render the notice — inline for single packages,
/// at the end of the batch for workspace/lockfile.
pub(crate) fn install_local_with_resolver(
    package_dir: &Path,
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    options: &InstallOptions,
    conflict_resolver: &ConflictResolver,
) -> Result<Option<String>> {
    let pipeline = CorePipeline::discover(package_dir, backends, options.force)?;
    manifest::validate_scope(&pipeline.manifest.install_scope, requested_scope)?;

    println!(
        "{} {} v{}",
        "Installing".green().bold(),
        pipeline.manifest.full_name,
        pipeline.manifest.version
    );

    let mut store = PackageStore::load(config)?;
    let resolved = pipeline.cleanup_and_resolve(&mut store, conflict_resolver, config)?;

    let (archive_path, integrity) =
        cache::create_archive(&resolved.package_dir, &resolved.manifest, config)?;

    let deployment = resolved.deploy(config)?;

    store.record_install(
        &resolved.manifest.full_name,
        PackageEntry {
            version: resolved.manifest.version.to_string(),
            source: options.source_kind.as_str().to_string(),
            source_path: match options.source_kind {
                SourceKind::Git => options.source_url.clone(),
                SourceKind::Local => resolved.package_dir.to_string_lossy().to_string(),
            },
            integrity,
            archive_path: archive_path.to_string_lossy().to_string(),
            deployed: deployment.deployed_map,
            resolved: options.resolved.clone(),
            tag: options.tag.clone(),
            member: options.member.clone(),
        },
    );
    store.save(config)?;

    print_post_deploy(
        &resolved.manifest.full_name,
        &deployment.all_deployed,
        &resolved.raw_manifest,
    );
    Ok(resolved
        .raw_manifest
        .messages
        .as_ref()
        .and_then(|m| m.postinstall.clone())
        .filter(|s| !s.is_empty()))
}

pub(crate) fn print_post_deploy(
    full_name: &str,
    deployed: &[crate::backend::DeployedArtifact],
    raw_manifest: &crate::manifest::Manifest,
) {
    println!(
        "{} Deployed {} artifact(s) for {}",
        "Done.".green().bold(),
        deployed.len(),
        full_name
    );
    for d in deployed {
        println!("  {} {}", "→".dimmed(), d.deployed_path.display());
    }

    if let Some(ref env) = raw_manifest.required_env {
        let missing = env_check::check_required_env(env);
        if !missing.is_empty() {
            env_check::print_env_warnings(&missing);
        }
    }
}

/// Render a single postinstall block, optionally prefixed with a package label
/// (used by the workspace/batch coordinator to attribute each block to a member).
pub(crate) fn print_postinstall_block(message: &str, package_label: Option<&str>) {
    println!("{}", "Postinstall notice:".yellow().bold());
    let prefix = package_label.map(|p| format!("{p}: ")).unwrap_or_default();
    let mut lines = message.lines();
    if let Some(first) = lines.next() {
        if prefix.is_empty() {
            println!("  {}", first);
        } else {
            println!("  {}{}", prefix.bold(), first);
        }
    }
    for line in lines {
        println!("  {}", line);
    }
}

/// Install a package from a lockfile entry.
///
/// Unlike `install_local`, this function:
/// - Always force-overwrites conflicts (no interactive prompt)
/// - Reuses the cached archive instead of creating a new one
/// - Does not update the lockfile (avoids cycles during lockfile replay)
pub fn install_from_lock_entry(
    package_dir: &Path,
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    source: &SourceInfo,
) -> Result<Option<String>> {
    let force_resolver: Box<ConflictResolver> = Box::new(|_: &Conflict| Ok(None));

    let pipeline = CorePipeline::discover(package_dir, backends, false)?;
    manifest::validate_scope(&pipeline.manifest.install_scope, requested_scope)?;

    println!(
        "{} {} v{}",
        "Installing".green().bold(),
        pipeline.manifest.full_name,
        pipeline.manifest.version
    );

    let mut store = PackageStore::load(config)?;
    let resolved = pipeline.cleanup_and_resolve(&mut store, &*force_resolver, config)?;

    // Reuse cached archive (no new archive creation for lockfile installs)
    let archive_path = cache::archive_path(
        config,
        &resolved.manifest.scope,
        &resolved.manifest.short_name,
        &resolved.manifest.version,
    );
    let integrity = cache::compute_sha256(&archive_path).unwrap_or_default();

    let deployment = resolved.deploy(config)?;

    store.record_install_from_lockfile(
        &resolved.manifest.full_name,
        PackageEntry {
            version: resolved.manifest.version.to_string(),
            source: source.source_kind.as_str().to_string(),
            source_path: source.source_url.clone(),
            integrity,
            archive_path: archive_path.to_string_lossy().to_string(),
            deployed: deployment.deployed_map,
            resolved: source.resolved.clone(),
            tag: source.tag.clone(),
            member: source.member.clone(),
        },
    );
    store.save(config)?;

    print_post_deploy(
        &resolved.manifest.full_name,
        &deployment.all_deployed,
        &resolved.raw_manifest,
    );
    Ok(resolved
        .raw_manifest
        .messages
        .as_ref()
        .and_then(|m| m.postinstall.clone())
        .filter(|s| !s.is_empty()))
}

#[cfg(test)]
mod tests;
