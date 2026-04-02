use std::path::Path;

use crate::artifact::ArtifactKind;
use crate::cache;
use crate::env_check;
use crate::install_cache::PackageEntry;

use super::types::{ArchiveState, DiagnosticKind};

pub fn check_deployed_files(entry: &PackageEntry) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    for artifact in entry.all_artifacts() {
        match artifact.artifact_type {
            ArtifactKind::Skill | ArtifactKind::Agent => {
                if !Path::new(&artifact.deployed_path).exists() {
                    issues.push(DiagnosticKind::FileMissing {
                        artifact_name: artifact.name.clone(),
                    });
                }
            }
            ArtifactKind::Hook => {}
        }
    }
    issues
}

pub fn check_archive(entry: &PackageEntry) -> ArchiveState {
    let archive_path = Path::new(&entry.archive_path);
    if archive_path.exists() {
        ArchiveState::Available
    } else {
        ArchiveState::Missing(entry.archive_path.clone())
    }
}

pub fn check_skill_modifications(entry: &PackageEntry) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    let archive_path = Path::new(&entry.archive_path);

    for artifact in entry.all_artifacts() {
        if artifact.artifact_type != ArtifactKind::Skill {
            continue;
        }
        let deployed_path = Path::new(&artifact.deployed_path);
        if !deployed_path.exists() {
            continue;
        }

        let archive_name = artifact.original_name.as_deref().unwrap_or(&artifact.name);
        let inner_path = format!("skills/{}.md", archive_name);

        let original_bytes = match cache::extract_file_from_archive(archive_path, &inner_path) {
            Ok(bytes) => bytes,
            Err(_) => break,
        };

        let original_hash = cache::compute_sha256_bytes(&original_bytes);
        let deployed_hash = match cache::compute_sha256(deployed_path) {
            Ok(h) => h,
            Err(_) => continue,
        };

        if original_hash != deployed_hash {
            issues.push(DiagnosticKind::SkillModified {
                artifact_name: artifact.name.clone(),
            });
        }
    }
    issues
}

pub fn check_env_vars(entry: &PackageEntry) -> Vec<DiagnosticKind> {
    let archive_path = Path::new(&entry.archive_path);

    let manifest_bytes = match cache::extract_file_from_archive(archive_path, "renkei.json") {
        Ok(bytes) => bytes,
        Err(_) => return vec![],
    };

    let manifest: serde_json::Value = match serde_json::from_slice(&manifest_bytes) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let required_env = match manifest.get("requiredEnv") {
        Some(v) => v,
        None => return vec![],
    };

    env_check::check_required_env(required_env)
        .into_iter()
        .map(|m| DiagnosticKind::EnvVarMissing {
            var_name: m.name,
            description: m.description,
        })
        .collect()
}

pub fn check_hooks(entry: &PackageEntry, settings: &serde_json::Value) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    for artifact in entry.all_artifacts() {
        for hook in &artifact.deployed_hooks {
            if !hook_exists_in_settings(settings, &hook.event, &hook.matcher, &hook.command) {
                issues.push(DiagnosticKind::HookMissing {
                    event: hook.event.clone(),
                    command: hook.command.clone(),
                });
            }
        }
    }
    issues
}

fn hook_exists_in_settings(
    settings: &serde_json::Value,
    event: &str,
    matcher: &Option<String>,
    command: &str,
) -> bool {
    let groups = match settings
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(|e| e.as_array())
    {
        Some(arr) => arr,
        None => return false,
    };

    groups.iter().any(|group| {
        let group_matcher = group
            .get("matcher")
            .and_then(|m| m.as_str())
            .map(String::from);
        if group_matcher != *matcher {
            return false;
        }
        group
            .get("hooks")
            .and_then(|h| h.as_array())
            .is_some_and(|hooks| {
                hooks
                    .iter()
                    .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(command))
            })
    })
}

pub fn check_mcp(entry: &PackageEntry, claude_config: &serde_json::Value) -> Vec<DiagnosticKind> {
    let mut issues = Vec::new();
    for server_name in entry.all_mcp_servers() {
        let exists = claude_config
            .get("mcpServers")
            .and_then(|m| m.get(server_name))
            .is_some();
        if !exists {
            issues.push(DiagnosticKind::McpMissing {
                server_name: server_name.to_string(),
            });
        }
    }
    issues
}
