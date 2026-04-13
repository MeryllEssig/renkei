use std::path::Path;

use crate::artifact::ArtifactKind;
use crate::backend::DeployedArtifact;
use crate::config::{BackendId, Config};
use crate::hook;
use crate::install_cache::{InstallCache, McpLocalRef};
use crate::mcp;

fn remove_artifact_file(path: &Path) {
    if path.is_dir() {
        let _ = std::fs::remove_dir_all(path);
    } else {
        let _ = std::fs::remove_file(path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

pub(crate) fn undo_artifact(
    kind: &ArtifactKind,
    path: &Path,
    hooks: &[hook::DeployedHookEntry],
    config: &Config,
) {
    match kind {
        ArtifactKind::Hook => {
            let claude_dirs = config.backend(BackendId::Claude);
            let _ = hook::remove(&hook::CLAUDE, &claude_dirs.settings_path.unwrap(), hooks);
        }
        _ => remove_artifact_file(path),
    }
}

pub(crate) fn cleanup_previous_installation(
    full_name: &str,
    install_cache: &InstallCache,
    config: &Config,
) {
    if let Some(entry) = install_cache.packages.get(full_name) {
        for artifact in entry.all_artifacts() {
            undo_artifact(
                &artifact.artifact_type,
                Path::new(&artifact.deployed_path),
                &artifact.deployed_hooks,
                config,
            );
        }
        let mcp_servers = entry.all_mcp_servers();
        if !mcp_servers.is_empty() {
            // Local MCPs are shared across refs; their removal from the backend
            // config is owned by `cleanup_local_mcp_refs` (only when the last ref
            // disappears). External MCPs go straight out of the config.
            let mcp_entries: Vec<mcp::DeployedMcpEntry> = mcp_servers
                .iter()
                .filter(|name| !install_cache.mcp_local.contains_key(**name))
                .map(|name| mcp::DeployedMcpEntry {
                    server_name: name.to_string(),
                })
                .collect();
            if !mcp_entries.is_empty() {
                let claude_dirs = config.backend(BackendId::Claude);
                let _ =
                    mcp::remove_mcp_from_config(&claude_dirs.config_path.unwrap(), &mcp_entries);
            }
        }
    }
}

/// Decrement local-MCP refs owned by `full_name` at the current install scope.
/// When a ref count reaches zero, the on-disk folder (or symlink) is removed
/// and the server entry is stripped from the backend config.
pub(crate) fn cleanup_local_mcp_refs(
    full_name: &str,
    cache: &mut InstallCache,
    config: &Config,
) {
    let scope = config.scope_label().to_string();
    let project_root = config
        .project_root
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    let candidate_names: Vec<String> = cache
        .mcp_local
        .iter()
        .filter(|(_, entry)| {
            entry.referenced_by.iter().any(|r| {
                r.package == full_name && r.scope == scope && r.project_root == project_root
            })
        })
        .map(|(name, _)| name.clone())
        .collect();

    for name in candidate_names {
        // same_install() matches on (package, scope, project_root); version is ignored.
        let match_ref = McpLocalRef {
            package: full_name.to_string(),
            version: String::new(),
            scope: scope.clone(),
            project_root: project_root.clone(),
        };
        if let Some(name_to_gc) = cache.remove_mcp_local_ref(&name, &match_ref) {
            let target = config.global_mcp_dir().join(&name_to_gc);
            if let Ok(meta) = std::fs::symlink_metadata(&target) {
                if meta.file_type().is_symlink() {
                    let _ = std::fs::remove_file(&target);
                } else {
                    let _ = std::fs::remove_dir_all(&target);
                }
            }
            let claude_cfg = config.backend(BackendId::Claude).config_path.unwrap();
            let _ = mcp::remove_mcp_from_config(
                &claude_cfg,
                &[mcp::DeployedMcpEntry {
                    server_name: name_to_gc,
                }],
            );
        }
    }
}

pub(super) fn rollback(deployed: &[DeployedArtifact], config: &Config) {
    for artifact in deployed.iter().rev() {
        undo_artifact(
            &artifact.artifact_kind,
            &artifact.deployed_path,
            &artifact.deployed_hooks,
            config,
        );
    }
}
