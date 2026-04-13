pub(crate) mod batch;
pub(crate) mod build;
mod cleanup;
mod deploy;
pub(crate) mod mcp_local;
pub(crate) mod messages;
pub(crate) mod pipeline;
mod resolve;
mod types;

pub(crate) use cleanup::cleanup_previous_installation;
pub use types::{ConflictResolver, InstallOptions, SourceInfo, SourceKind};

use std::path::Path;

use owo_colors::OwoColorize;

use crate::backend::Backend;
use crate::cache;
use crate::config::Config;
use crate::conflict::Conflict;
use crate::env_check;
use crate::error::Result;
use crate::install_cache::PackageEntry;
use crate::manifest::{self, Manifest, RequestedScope};
use crate::package_store::PackageStore;

use pipeline::CorePipeline;

/// Build the conflict resolver. When `force` is set, conflicts are overwritten.
/// Otherwise, colliding artifacts are automatically renamed to `{scope}-{name}`
/// where `scope` is the incoming package's scope. Residual conflicts on the
/// renamed target are caught later by `resolve_conflicts_and_rename`.
pub(crate) fn default_resolver(force: bool, scope: &str) -> Box<ConflictResolver> {
    if force {
        Box::new(|_: &Conflict| Ok(None))
    } else {
        let scope = scope.to_string();
        Box::new(move |c: &Conflict| Ok(Some(format!("{scope}-{}", c.artifact_name))))
    }
}

pub fn install_local(
    package_dir: &Path,
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    options: &InstallOptions,
    allow_build: bool,
) -> Result<()> {
    let raw_manifest = Manifest::from_path(package_dir)?;
    let validated = raw_manifest.validate()?;
    let resolver = default_resolver(options.force, &validated.scope);
    let postinstall = install_local_with_resolver(
        package_dir,
        config,
        backends,
        requested_scope,
        options,
        &*resolver,
        allow_build,
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
    allow_build: bool,
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

    let link_mode = options.source_kind == SourceKind::LocalLink;

    // Linked installs skip archiving entirely — sources are live, the
    // workspace owns their lifecycle.
    let (archive_path, integrity) = if link_mode {
        (std::path::PathBuf::new(), String::new())
    } else {
        cache::create_archive(&resolved.package_dir, &resolved.manifest, config)?
    };

    let scope_label = if config.is_project() { "project" } else { "global" };
    let project_root = config.project_root.as_deref();
    let (staged, mcp_json) = mcp_local::stage_local_mcps(
        &resolved.raw_manifest,
        &resolved.package_dir,
        &store,
        config,
        scope_label,
        project_root,
        options.force,
        link_mode,
        allow_build,
    )?;

    let deployment = match resolved.deploy(config, mcp_json.as_ref()) {
        Ok(d) => d,
        Err(e) => {
            mcp_local::rollback_staging(&staged);
            return Err(e);
        }
    };

    let mcp_local_sources: std::collections::HashMap<String, String> = staged
        .iter()
        .map(|s| (s.name.clone(), s.source_sha256.clone()))
        .collect();
    mcp_local::commit_local_mcps(staged, &mut store)?;

    let entry = PackageEntry {
        version: resolved.manifest.version.to_string(),
        source: options.source_kind.as_str().to_string(),
        source_path: match options.source_kind {
            SourceKind::Git => options.source_url.clone(),
            SourceKind::Local | SourceKind::LocalLink => {
                resolved.package_dir.to_string_lossy().to_string()
            }
        },
        integrity,
        archive_path: archive_path.to_string_lossy().to_string(),
        deployed: deployment.deployed_map,
        resolved: options.resolved.clone(),
        tag: options.tag.clone(),
        member: options.member.clone(),
        mcp_local_sources,
    };
    if link_mode {
        // record only in the install cache; lockfile stays untouched so
        // a teammate can't `rk install` from a lockfile that points at
        // a personal workspace path.
        store.record_install_from_lockfile(&resolved.manifest.full_name, entry);
    } else {
        store.record_install(&resolved.manifest.full_name, entry);
    }
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
    allow_build: bool,
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

    let scope_label = if config.is_project() { "project" } else { "global" };
    let project_root = config.project_root.as_deref();
    let (staged, mcp_json) = mcp_local::stage_local_mcps(
        &resolved.raw_manifest,
        &resolved.package_dir,
        &store,
        config,
        scope_label,
        project_root,
        true, // lockfile replay always force-overwrites
        false,
        allow_build,
    )?;

    let deployment = match resolved.deploy(config, mcp_json.as_ref()) {
        Ok(d) => d,
        Err(e) => {
            mcp_local::rollback_staging(&staged);
            return Err(e);
        }
    };

    let mcp_local_sources: std::collections::HashMap<String, String> = staged
        .iter()
        .map(|s| (s.name.clone(), s.source_sha256.clone()))
        .collect();
    mcp_local::commit_local_mcps(staged, &mut store)?;

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
            mcp_local_sources,
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
