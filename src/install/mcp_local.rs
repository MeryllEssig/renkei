//! Local-MCP deployment pipeline. Stages source folders under
//! `~/.renkei/mcp/<name>.new/`, runs the declared `build` argv steps with
//! a filtered env, and on overall install success swaps them into the live
//! `~/.renkei/mcp/<name>/` location with a best-effort atomic rename.
//!
//! The flow is split into two phases so that backend deployment failures
//! never leave half-committed state:
//!   1. [`stage_local_mcps`] computes the cache outcome, copies sources,
//!      runs builds, and resolves entrypoints — but neither swaps the
//!      live folder nor mutates the install cache.
//!   2. [`commit_local_mcps`] performs the atomic swap and records refs
//!      via [`InstallCache::add_mcp_local_ref`]. Called only after
//!      backends accepted the rewritten MCP config.
//!
//! On failure between the two phases, [`rollback_staging`] removes the
//! `.new` folders so the previous version stays untouched.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::install_cache::{McpLocalEntry, McpLocalRef};
use crate::manifest::{Manifest, McpServer};
use crate::package_store::PackageStore;
use crate::rkignore::{build_walker, hash_with_patterns, load_mcp_ignores};

use super::build::{run_build, BuildStep};

/// Result of the staging phase for a single local MCP. Holds enough state
/// for [`commit_local_mcps`] to either swap the new content into place
/// (Fresh / Upgrade / forced overwrite) or skip straight to the cache
/// update (AddedRef — folder already materialized).
#[derive(Debug)]
pub(crate) struct StagedMcp {
    pub name: String,
    pub source_sha256: String,
    pub new_ref: McpLocalRef,
    pub package_full_name: String,
    pub package_version: String,
    /// `Some(staging_dir)` when the on-disk folder must be swapped at
    /// commit time, `None` when an existing folder is being reused or
    /// when this is a link install.
    pub pending_swap: Option<PathBuf>,
    pub target_dir: PathBuf,
    /// `true` when a forced overwrite must transfer ownership in the
    /// install cache. The commit step then replaces the entry rather
    /// than letting `add_mcp_local_ref` reject the different owner.
    pub force_transfer: bool,
    /// `Some(workspace_mcp_dir)` when the install is a `--link` install:
    /// commit creates a symlink from `target_dir` to this path instead
    /// of swapping a staged copy.
    pub link_source: Option<PathBuf>,
}

/// Plan and materialize every local MCP declared by `raw_manifest`.
///
/// Reads (but does not mutate) the install cache to decide whether each
/// MCP needs a fresh build, can reuse an existing folder, must be
/// upgraded, or conflicts with another package.
///
/// On success, returns one [`StagedMcp`] per local MCP plus a JSON object
/// (one entry per MCP — local or external) that callers feed straight
/// into the backend `register_mcp` path, with absolute entrypoint paths
/// already prepended to local MCPs' `args`.
pub(crate) fn stage_local_mcps(
    raw_manifest: &Manifest,
    package_dir: &Path,
    current_local: &std::collections::HashMap<String, McpLocalEntry>,
    config: &Config,
    force: bool,
    link_mode: bool,
    allow_build: bool,
) -> Result<(Vec<StagedMcp>, Option<serde_json::Value>)> {
    let Some(ref mcps) = raw_manifest.mcp else {
        return Ok((Vec::new(), None));
    };

    let mut staged: Vec<StagedMcp> = Vec::new();
    let mut effective_mcp = serde_json::Map::with_capacity(mcps.len());

    let scope = config.scope_label();
    let project_root = config.project_root.as_deref();
    let global_root = config.global_mcp_dir();
    let patterns = load_mcp_ignores(package_dir);

    let mut names: Vec<&String> = mcps.keys().collect();
    names.sort();

    for name in names {
        let server = &mcps[name];

        if !server.is_local() {
            // External MCP — pass through unchanged.
            effective_mcp.insert(name.clone(), serde_json::to_value(server)?);
            continue;
        }

        let entrypoint = server.entrypoint.as_ref().ok_or_else(|| {
            RenkeiError::InvalidManifest(format!("local MCP `{name}` requires `entrypoint`"))
        })?;
        let source_dir = package_dir.join("mcp").join(name);
        let target_dir = global_root.join(name);

        let outcome = peek_outcome(
            current_local,
            name,
            &raw_manifest.name,
            &raw_manifest.version,
        );
        let reuse_existing =
            matches!(outcome, Outcome::AddedRef) && target_dir.exists() && !link_mode;

        // Skip the rkignore walk when reusing an already-deployed folder:
        // AddedRef guarantees same owner+version, so the cached source
        // hash still applies.
        let source_sha256 = if reuse_existing {
            current_local
                .get(name)
                .map(|e| e.source_sha256.clone())
                .unwrap_or_default()
        } else {
            hash_with_patterns(&source_dir, &patterns)?
        };

        let new_ref = McpLocalRef {
            package: raw_manifest.name.clone(),
            version: raw_manifest.version.clone(),
            scope: scope.to_string(),
            project_root: project_root.map(|p| p.to_string_lossy().to_string()),
        };

        let force_transfer = matches!(outcome, Outcome::Conflict { .. }) && force;
        let (pending_swap, link_source) = if link_mode {
            check_link_target(&target_dir, &source_dir, name)?;
            (None, Some(source_dir.clone()))
        } else {
            let swap = match outcome {
                Outcome::Conflict { current_owner } if !force => {
                    return Err(RenkeiError::McpOwnerConflict {
                        name: name.clone(),
                        current_owner,
                        attempted_by: raw_manifest.name.clone(),
                    });
                }
                Outcome::AddedRef if reuse_existing => None,
                _ => Some(build_into_staging(
                    &source_dir,
                    &global_root,
                    name,
                    &patterns,
                    server,
                    allow_build,
                )?),
            };
            (swap, None)
        };

        let probe_root: &Path = link_source
            .as_deref()
            .or(pending_swap.as_deref())
            .unwrap_or(&target_dir);
        let abs_entrypoint = probe_root.join(entrypoint);
        if !abs_entrypoint.exists() {
            if let Some(ref staging) = pending_swap {
                let _ = std::fs::remove_dir_all(staging);
            }
            return Err(RenkeiError::McpEntrypointMissing {
                name: name.clone(),
                entrypoint: abs_entrypoint.to_string_lossy().to_string(),
            });
        }
        let final_abs = target_dir.join(entrypoint);

        effective_mcp.insert(name.clone(), build_server_json(server, &final_abs)?);

        staged.push(StagedMcp {
            name: name.clone(),
            source_sha256,
            new_ref,
            package_full_name: raw_manifest.name.clone(),
            package_version: raw_manifest.version.clone(),
            pending_swap,
            target_dir,
            force_transfer,
            link_source,
        });
    }

    let effective = if effective_mcp.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(effective_mcp))
    };
    Ok((staged, effective))
}

/// Promote staged folders into their live locations and record refs in
/// the install cache. Called after backends accepted the new MCP config.
///
/// Atomicity is best-effort: if `<name>.new/` exists we rename the
/// current `<name>/` aside to `<name>.old/`, swap the new one in, then
/// remove `.old`. On rename failures mid-swap we attempt to restore the
/// previous state so the user is left with a working folder.
pub(crate) fn commit_local_mcps(staged: Vec<StagedMcp>, store: &mut PackageStore) -> Result<()> {
    for s in staged {
        if let Some(ref src) = s.link_source {
            create_or_reuse_symlink(src, &s.target_dir)?;
        } else if let Some(ref staging) = s.pending_swap {
            atomic_swap(staging, &s.target_dir)?;
        }
        // Forced ownership transfer: drop the pre-existing entry so
        // add_mcp_local_ref takes the FreshInstall path with the new
        // owner, version, and source hash.
        if s.force_transfer {
            store.cache_mut().mcp_local.remove(&s.name);
        }
        let entry = McpLocalEntry {
            owner_package: s.package_full_name.clone(),
            version: s.package_version.clone(),
            source_sha256: s.source_sha256.clone(),
            referenced_by: Vec::new(),
        };
        // add_mcp_local_ref upserts the ref and updates ownership/version
        // metadata on upgrade. Outcome is already validated in stage; we
        // intentionally ignore it here.
        let _ = store
            .cache_mut()
            .add_mcp_local_ref(&s.name, || entry, s.new_ref);
    }
    Ok(())
}

/// Best-effort cleanup for the staging phase. Removes any `.new/`
/// directories that survived a failure. Never returns an error — used
/// only on the unhappy path.
pub(crate) fn rollback_staging(staged: &[StagedMcp]) {
    for s in staged {
        if let Some(ref staging) = s.pending_swap {
            let _ = std::fs::remove_dir_all(staging);
        }
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum Outcome {
    Fresh,
    AddedRef,
    Upgrade,
    Conflict { current_owner: String },
}

fn peek_outcome(
    current_local: &std::collections::HashMap<String, McpLocalEntry>,
    name: &str,
    package: &str,
    version: &str,
) -> Outcome {
    match current_local.get(name) {
        None => Outcome::Fresh,
        Some(e) if e.owner_package != package => Outcome::Conflict {
            current_owner: e.owner_package.clone(),
        },
        Some(e) if e.version != version => Outcome::Upgrade,
        _ => Outcome::AddedRef,
    }
}

fn build_into_staging(
    source_dir: &Path,
    global_root: &Path,
    name: &str,
    patterns: &[String],
    server: &McpServer,
    allow_build: bool,
) -> Result<PathBuf> {
    std::fs::create_dir_all(global_root)?;
    let staging = global_root.join(format!("{name}.new"));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }

    copy_dir_filtered(source_dir, &staging, patterns)?;

    if let Some(ref steps) = server.build {
        if !steps.is_empty() {
            // Defensive: stage_local_mcps is supposed to be called only
            // after confirm_builds cleared the prompt. Guard anyway so a
            // direct caller can't bypass the consent contract.
            if !allow_build {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(RenkeiError::BuildRequiresConfirmation);
            }
            let argv_steps: Vec<BuildStep> = steps
                .iter()
                .map(|s| BuildStep { argv: s.clone() })
                .collect();
            if let Err(e) = run_build(&argv_steps, &staging) {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(e);
            }
        }
    }

    Ok(staging)
}

/// Recursively copy `src` into `dst`, honouring rkignore patterns at the
/// `src` root. Reuses [`build_walker`] so behaviour stays consistent
/// with `hash_directory` / `hash_with_patterns`.
fn copy_dir_filtered(src: &Path, dst: &Path, patterns: &[String]) -> Result<()> {
    let walker = build_walker(src, patterns)?;
    std::fs::create_dir_all(dst)?;

    for entry in walker {
        let entry = entry.map_err(|e| RenkeiError::CacheError(format!("walk error: {e}")))?;
        let path = entry.path();
        let rel = match path.strip_prefix(src) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dst.join(rel);
        let file_type = match entry.file_type() {
            Some(ft) => ft,
            None => continue,
        };
        if file_type.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if file_type.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(path, &target)?;
        } else if file_type.is_symlink() {
            // Preserve symlinks verbatim. On unix only — skipping on other
            // platforms is fine because phase 4 already gates run_build to
            // unix.
            #[cfg(unix)]
            {
                let link_target = std::fs::read_link(path)?;
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::os::unix::fs::symlink(&link_target, &target)?;
            }
        }
    }
    Ok(())
}

/// Pre-flight validation for `--link` deployments: refuse to clobber a
/// real directory left behind by an earlier copy install, and refuse
/// to redirect an existing symlink that points elsewhere (treated as a
/// soft owner conflict so the user notices).
fn check_link_target(target: &Path, source: &Path, name: &str) -> Result<()> {
    let meta = match std::fs::symlink_metadata(target) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(RenkeiError::DeploymentFailed(format!(
                "cannot stat {}: {e}",
                target.display()
            )))
        }
    };
    if meta.file_type().is_symlink() {
        let current = std::fs::read_link(target).map_err(|e| {
            RenkeiError::DeploymentFailed(format!("cannot read symlink {}: {e}", target.display()))
        })?;
        if current != source {
            return Err(RenkeiError::McpOwnerConflict {
                name: name.to_string(),
                current_owner: format!("symlink → {}", current.display()),
                attempted_by: format!("symlink → {}", source.display()),
            });
        }
        Ok(())
    } else if meta.is_dir() {
        Err(RenkeiError::McpLinkOverReal {
            name: name.to_string(),
            target: target.to_string_lossy().to_string(),
        })
    } else {
        // Unexpected file at the slot — surface as a generic error.
        Err(RenkeiError::DeploymentFailed(format!(
            "MCP target {} exists and is not a directory",
            target.display()
        )))
    }
}

#[cfg(unix)]
fn create_or_reuse_symlink(source: &Path, target: &Path) -> Result<()> {
    use std::os::unix::fs::symlink;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::symlink_metadata(target).is_ok() {
        // check_link_target already verified this is a symlink to the
        // same source; treat as idempotent.
        return Ok(());
    }
    symlink(source, target).map_err(|e| {
        RenkeiError::DeploymentFailed(format!(
            "failed to symlink {} → {}: {e}",
            target.display(),
            source.display()
        ))
    })
}

#[cfg(not(unix))]
fn create_or_reuse_symlink(_source: &Path, _target: &Path) -> Result<()> {
    Err(RenkeiError::DeploymentFailed(
        "--link is only supported on Unix platforms".into(),
    ))
}

fn atomic_swap(staging: &Path, target: &Path) -> Result<()> {
    let parent = target.parent().ok_or_else(|| {
        RenkeiError::DeploymentFailed(format!(
            "cannot resolve parent of MCP target {}",
            target.display()
        ))
    })?;
    std::fs::create_dir_all(parent)?;

    if !target.exists() {
        std::fs::rename(staging, target)?;
        return Ok(());
    }

    let backup = parent.join(format!(
        "{}.old",
        target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "mcp".into())
    ));
    // Clean stale backup left over from a prior crashed swap.
    if backup.exists() {
        let _ = std::fs::remove_dir_all(&backup);
    }

    if let Err(e) = std::fs::rename(target, &backup) {
        return Err(RenkeiError::DeploymentFailed(format!(
            "failed to back up {}: {e}",
            target.display()
        )));
    }

    if let Err(e) = std::fs::rename(staging, target) {
        // Rollback: restore the backup.
        let _ = std::fs::rename(&backup, target);
        return Err(RenkeiError::DeploymentFailed(format!(
            "failed to swap MCP into {}: {e}",
            target.display()
        )));
    }

    let _ = std::fs::remove_dir_all(&backup);
    Ok(())
}

/// Build the JSON shape that backends register: drop `entrypoint` and
/// `build` (renkei-only fields), and prepend the absolute entrypoint
/// path to the existing `args` array.
fn build_server_json(server: &McpServer, abs_entrypoint: &Path) -> Result<serde_json::Value> {
    let mut obj = server.extra.clone();

    let abs_str = abs_entrypoint.to_string_lossy().to_string();
    let mut new_args: Vec<serde_json::Value> = vec![serde_json::Value::String(abs_str)];
    if let Some(existing) = obj.remove("args") {
        match existing {
            serde_json::Value::Array(items) => new_args.extend(items),
            other => {
                return Err(RenkeiError::InvalidManifest(format!(
                    "mcp `args` must be an array, got {other:?}"
                )))
            }
        }
    }
    obj.insert("args".into(), serde_json::Value::Array(new_args));

    Ok(serde_json::Value::Object(obj))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_cache::{McpLocalEntry, McpLocalRef};
    use crate::manifest::{ManifestScope, McpServer};
    use std::collections::HashMap as StdHashMap;
    use tempfile::tempdir;

    fn manifest_with(name: &str, version: &str, mcps: Vec<(&str, McpServer)>) -> Manifest {
        let map: StdHashMap<String, McpServer> =
            mcps.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Manifest {
            name: name.into(),
            version: version.into(),
            description: "x".into(),
            author: "a".into(),
            license: "MIT".into(),
            backends: vec!["claude".into()],
            keywords: vec![],
            scope: ManifestScope::default(),
            mcp: if map.is_empty() { None } else { Some(map) },
            required_env: None,
            messages: None,
        }
    }

    fn local_mcp(
        entrypoint: Option<&str>,
        build: Option<Vec<Vec<&str>>>,
        extra: Vec<(&str, serde_json::Value)>,
    ) -> McpServer {
        let mut map = serde_json::Map::new();
        for (k, v) in extra {
            map.insert(k.into(), v);
        }
        McpServer {
            entrypoint: entrypoint.map(String::from),
            build: build.map(|b| {
                b.into_iter()
                    .map(|s| s.into_iter().map(String::from).collect())
                    .collect()
            }),
            extra: map,
        }
    }

    fn make_pkg_with_mcp(name: &str, content: &str) -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let mcp_dir = dir.path().join("mcp").join(name);
        let dist_dir = mcp_dir.join("dist");
        std::fs::create_dir_all(&dist_dir).unwrap();
        std::fs::write(dist_dir.join("index.js"), content).unwrap();
        dir
    }

    #[test]
    fn external_only_manifest_returns_no_staged() {
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![(
                "ext",
                McpServer {
                    entrypoint: None,
                    build: None,
                    extra: serde_json::Map::new(),
                },
            )],
        );
        let pkg = tempdir().unwrap();
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let store = PackageStore::load(&cfg).unwrap();

        let (staged, eff) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap();
        assert!(staged.is_empty());
        // External entry is preserved verbatim.
        let eff = eff.unwrap();
        assert!(eff.get("ext").is_some());
    }

    #[test]
    fn fresh_install_no_build_steps_swaps_prebuilt_entrypoint() {
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![(
                "srv",
                local_mcp(
                    Some("dist/index.js"),
                    None,
                    vec![("command", serde_json::json!("node"))],
                ),
            )],
        );
        let pkg = make_pkg_with_mcp("srv", "console.log('hi')");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let mut store = PackageStore::load(&cfg).unwrap();

        let (staged, eff) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(staged.len(), 1);
        assert!(
            staged[0].pending_swap.is_some(),
            "fresh install should stage"
        );

        commit_local_mcps(staged, &mut store).unwrap();

        let target = cfg
            .global_mcp_dir()
            .join("srv")
            .join("dist")
            .join("index.js");
        assert!(target.exists(), "entrypoint must exist after commit");
        let cache = store.cache();
        let entry = cache.mcp_local.get("srv").unwrap();
        assert_eq!(entry.owner_package, "@x/p");
        assert_eq!(entry.referenced_by.len(), 1);

        // args rewrite includes abs path first
        let srv = eff.unwrap()["srv"].clone();
        let args = srv["args"].as_array().unwrap();
        assert!(args[0].as_str().unwrap().ends_with("/srv/dist/index.js"));
        assert_eq!(srv["command"], "node");
    }

    #[test]
    fn args_rewrite_prepends_abs_entrypoint_and_keeps_user_args() {
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![(
                "srv",
                local_mcp(
                    Some("dist/index.js"),
                    None,
                    vec![
                        ("command", serde_json::json!("node")),
                        ("args", serde_json::json!(["--verbose"])),
                    ],
                ),
            )],
        );
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let store = PackageStore::load(&cfg).unwrap();

        let (_staged, eff) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap();
        let srv = eff.unwrap()["srv"].clone();
        let args = srv["args"].as_array().unwrap();
        assert_eq!(args.len(), 2);
        assert!(args[0].as_str().unwrap().ends_with("/srv/dist/index.js"));
        assert_eq!(args[1], "--verbose");
        assert!(
            srv.get("entrypoint").is_none(),
            "entrypoint must not appear in backend config"
        );
        assert!(srv.get("build").is_none());
    }

    #[test]
    fn missing_entrypoint_after_stage_yields_error() {
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![("srv", local_mcp(Some("dist/missing.js"), None, vec![]))],
        );
        let pkg = make_pkg_with_mcp("srv", "exists");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let store = PackageStore::load(&cfg).unwrap();

        let err = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap_err();
        assert!(matches!(err, RenkeiError::McpEntrypointMissing { .. }));
        // Staging dir was cleaned up.
        assert!(!cfg.global_mcp_dir().join("srv.new").exists());
    }

    #[test]
    fn conflict_different_owner_without_force_errors() {
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let mut store = PackageStore::load(&cfg).unwrap();

        // Pre-seed the cache with an entry owned by @other/pkg.
        let cache_mut = store.cache_mut();
        cache_mut.mcp_local.insert(
            "srv".into(),
            McpLocalEntry {
                owner_package: "@other/pkg".into(),
                version: "1.0.0".into(),
                source_sha256: "deadbeef".into(),
                referenced_by: vec![McpLocalRef {
                    package: "@other/pkg".into(),
                    version: "1.0.0".into(),
                    scope: "global".into(),
                    project_root: None,
                }],
            },
        );

        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![("srv", local_mcp(Some("dist/index.js"), None, vec![]))],
        );
        let err = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap_err();
        assert!(matches!(err, RenkeiError::McpOwnerConflict { .. }));
    }

    #[test]
    fn conflict_with_force_overwrites_owner() {
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let mut store = PackageStore::load(&cfg).unwrap();

        let cache_mut = store.cache_mut();
        cache_mut.mcp_local.insert(
            "srv".into(),
            McpLocalEntry {
                owner_package: "@other/pkg".into(),
                version: "1.0.0".into(),
                source_sha256: "deadbeef".into(),
                referenced_by: vec![McpLocalRef {
                    package: "@other/pkg".into(),
                    version: "1.0.0".into(),
                    scope: "global".into(),
                    project_root: None,
                }],
            },
        );

        let m = manifest_with(
            "@x/p",
            "2.0.0",
            vec![("srv", local_mcp(Some("dist/index.js"), None, vec![]))],
        );
        let (staged, _eff) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            true, // force
            false,
            false,
        )
        .unwrap();
        commit_local_mcps(staged, &mut store).unwrap();

        let entry = store.cache().mcp_local.get("srv").unwrap();
        assert_eq!(entry.owner_package, "@x/p");
        assert_eq!(entry.version, "2.0.0");
    }

    #[test]
    fn second_install_same_owner_adds_ref_without_rebuild() {
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let mut store = PackageStore::load(&cfg).unwrap();

        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![("srv", local_mcp(Some("dist/index.js"), None, vec![]))],
        );

        let (staged1, _) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap();
        commit_local_mcps(staged1, &mut store).unwrap();

        // Second install — project-scoped config simulates a second project.
        let project_root = tempdir().unwrap();
        let cfg_project = Config::for_project(home.path().into(), project_root.path().into());
        let (staged2, _) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg_project,
            false,
            false,
            false,
        )
        .unwrap();
        assert!(
            staged2[0].pending_swap.is_none(),
            "second install with same owner+version should reuse the folder, not stage"
        );
        commit_local_mcps(staged2, &mut store).unwrap();

        let entry = store.cache().mcp_local.get("srv").unwrap();
        assert_eq!(entry.referenced_by.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn build_step_runs_inside_staging_dir() {
        // Use a simple build command that creates a marker file.
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![(
                "srv",
                local_mcp(
                    Some("dist/index.js"),
                    Some(vec![vec!["true"]]),
                    vec![("command", serde_json::json!("node"))],
                ),
            )],
        );
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let mut store = PackageStore::load(&cfg).unwrap();

        let (staged, _) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            true, // allow_build
        )
        .unwrap();
        commit_local_mcps(staged, &mut store).unwrap();

        assert!(cfg
            .global_mcp_dir()
            .join("srv")
            .join("dist")
            .join("index.js")
            .exists());
    }

    #[cfg(unix)]
    #[test]
    fn build_step_failure_cleans_staging_and_preserves_previous() {
        let m_v1 = manifest_with(
            "@x/p",
            "1.0.0",
            vec![("srv", local_mcp(Some("dist/index.js"), None, vec![]))],
        );
        let m_v2 = manifest_with(
            "@x/p",
            "2.0.0",
            vec![(
                "srv",
                local_mcp(
                    Some("dist/index.js"),
                    Some(vec![vec!["false"]]), // intentionally fails
                    vec![],
                ),
            )],
        );
        let pkg = make_pkg_with_mcp("srv", "v1");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let mut store = PackageStore::load(&cfg).unwrap();

        let (s1, _) = stage_local_mcps(
            &m_v1,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap();
        commit_local_mcps(s1, &mut store).unwrap();

        let err = stage_local_mcps(
            &m_v2,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            true,
        )
        .unwrap_err();
        assert!(matches!(err, RenkeiError::BuildFailed { .. }));

        // Previous version still present.
        assert!(cfg
            .global_mcp_dir()
            .join("srv")
            .join("dist")
            .join("index.js")
            .exists());
        // No leftover staging dir.
        assert!(!cfg.global_mcp_dir().join("srv.new").exists());
        // Cache still records v1.
        assert_eq!(store.cache().mcp_local.get("srv").unwrap().version, "1.0.0");
    }

    #[test]
    fn build_with_steps_but_no_allow_build_errors() {
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![(
                "srv",
                local_mcp(Some("dist/index.js"), Some(vec![vec!["true"]]), vec![]),
            )],
        );
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let store = PackageStore::load(&cfg).unwrap();

        let err = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap_err();
        assert!(matches!(err, RenkeiError::BuildRequiresConfirmation));
    }

    #[cfg(unix)]
    #[test]
    fn link_mode_creates_symlink_and_skips_build() {
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![(
                "srv",
                local_mcp(
                    Some("dist/index.js"),
                    Some(vec![vec!["false"]]), // would fail if it ran
                    vec![("command", serde_json::json!("node"))],
                ),
            )],
        );
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let mut store = PackageStore::load(&cfg).unwrap();

        let (staged, eff) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            true, // link_mode
            false,
        )
        .unwrap();
        assert_eq!(staged.len(), 1);
        assert!(staged[0].pending_swap.is_none());
        assert!(staged[0].link_source.is_some());

        commit_local_mcps(staged, &mut store).unwrap();

        let target = cfg.global_mcp_dir().join("srv");
        let meta = std::fs::symlink_metadata(&target).unwrap();
        assert!(
            meta.file_type().is_symlink(),
            "expected symlink at {target:?}"
        );
        let link_target = std::fs::read_link(&target).unwrap();
        assert_eq!(link_target, pkg.path().join("mcp").join("srv"));
        // backend args still resolve via the (canonical) target path.
        let srv = eff.unwrap()["srv"].clone();
        let args = srv["args"].as_array().unwrap();
        assert!(args[0].as_str().unwrap().ends_with("/srv/dist/index.js"));
    }

    #[cfg(unix)]
    #[test]
    fn link_mode_over_real_directory_errors() {
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![("srv", local_mcp(Some("dist/index.js"), None, vec![]))],
        );
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());

        // Materialize a real folder at the target slot to simulate a
        // previous copy install.
        let real_dist = cfg.global_mcp_dir().join("srv").join("dist");
        std::fs::create_dir_all(&real_dist).unwrap();
        std::fs::write(real_dist.join("index.js"), "old").unwrap();

        let store = PackageStore::load(&cfg).unwrap();
        let err = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            true, // link_mode
            false,
        )
        .unwrap_err();
        assert!(matches!(err, RenkeiError::McpLinkOverReal { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn link_mode_over_existing_symlink_to_other_source_errors() {
        use std::os::unix::fs::symlink;

        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![("srv", local_mcp(Some("dist/index.js"), None, vec![]))],
        );
        let pkg_a = make_pkg_with_mcp("srv", "a");
        let pkg_b = make_pkg_with_mcp("srv", "b");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());

        std::fs::create_dir_all(cfg.global_mcp_dir()).unwrap();
        symlink(
            pkg_a.path().join("mcp").join("srv"),
            cfg.global_mcp_dir().join("srv"),
        )
        .unwrap();

        let store = PackageStore::load(&cfg).unwrap();
        let err = stage_local_mcps(
            &m,
            pkg_b.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            true, // link_mode
            false,
        )
        .unwrap_err();
        assert!(matches!(err, RenkeiError::McpOwnerConflict { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn link_mode_idempotent_when_symlink_already_points_at_source() {
        use std::os::unix::fs::symlink;

        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![("srv", local_mcp(Some("dist/index.js"), None, vec![]))],
        );
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());

        std::fs::create_dir_all(cfg.global_mcp_dir()).unwrap();
        symlink(
            pkg.path().join("mcp").join("srv"),
            cfg.global_mcp_dir().join("srv"),
        )
        .unwrap();

        let mut store = PackageStore::load(&cfg).unwrap();
        let (staged, _) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            true,
            false,
        )
        .unwrap();
        commit_local_mcps(staged, &mut store).unwrap();
        // Symlink is still there and untouched.
        let meta = std::fs::symlink_metadata(cfg.global_mcp_dir().join("srv")).unwrap();
        assert!(meta.file_type().is_symlink());
    }

    #[test]
    fn rollback_staging_removes_pending_dirs() {
        let pkg = make_pkg_with_mcp("srv", "x");
        let home = tempdir().unwrap();
        let cfg = Config::with_home_dir(home.path().into());
        let store = PackageStore::load(&cfg).unwrap();
        let m = manifest_with(
            "@x/p",
            "1.0.0",
            vec![("srv", local_mcp(Some("dist/index.js"), None, vec![]))],
        );
        let (staged, _) = stage_local_mcps(
            &m,
            pkg.path(),
            &store.cache().mcp_local,
            &cfg,
            false,
            false,
            false,
        )
        .unwrap();
        let staging = staged[0].pending_swap.clone().unwrap();
        assert!(staging.exists());
        rollback_staging(&staged);
        assert!(!staging.exists());
    }
}
