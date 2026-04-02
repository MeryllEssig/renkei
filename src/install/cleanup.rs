use std::path::Path;

use crate::artifact::ArtifactKind;
use crate::backend::DeployedArtifact;
use crate::config::Config;
use crate::hook;
use crate::install_cache::InstallCache;
use crate::mcp;

fn remove_artifact_file(path: &Path) {
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
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
            let _ = hook::remove_hooks_from_settings(&config.claude_settings_path(), hooks);
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
            let mcp_entries: Vec<mcp::DeployedMcpEntry> = mcp_servers
                .iter()
                .map(|name| mcp::DeployedMcpEntry {
                    server_name: name.to_string(),
                })
                .collect();
            let _ = mcp::remove_mcp_from_config(&config.claude_config_path(), &mcp_entries);
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
