use std::collections::HashMap;

use crate::artifact::{Artifact, ArtifactKind};
use crate::backend::{Backend, DeployedArtifact};
use crate::config::Config;
use crate::error::Result;
use crate::install_cache::{BackendDeployment, DeployedArtifactEntry};
use crate::manifest::Manifest;

use super::cleanup;

pub(crate) struct DeploymentResult {
    pub all_deployed: Vec<DeployedArtifact>,
    pub deployed_map: HashMap<String, BackendDeployment>,
}

/// Check if an artifact kind is known to be unsupported for a backend.
fn is_unsupported_for_backend(backend_name: &str, kind: &ArtifactKind) -> bool {
    match backend_name {
        "agents" => matches!(kind, ArtifactKind::Agent | ArtifactKind::Hook),
        _ => false,
    }
}

/// Check if MCP registration is known to be unsupported for a backend.
fn is_mcp_unsupported(backend_name: &str) -> bool {
    matches!(backend_name, "agents")
}

/// Deploy artifacts + MCP to all active backends, with dedup and rollback on failure.
pub(crate) fn deploy_to_backends(
    effective_artifacts: &[(Artifact, Option<String>)],
    active_backends: &[&dyn Backend],
    raw_manifest: &Manifest,
    config: &Config,
) -> Result<DeploymentResult> {
    let mut all_deployed: Vec<DeployedArtifact> = Vec::new();
    let mut deployed_map: HashMap<String, BackendDeployment> = HashMap::new();

    let has_agents = active_backends.iter().any(|b| b.name() == "agents");

    // Pre-build lookup for original names (avoids O(n²) scan in entry building)
    let original_names: HashMap<(&ArtifactKind, &str), Option<&String>> = effective_artifacts
        .iter()
        .map(|(art, orig)| ((&art.kind, art.name.as_str()), orig.as_ref()))
        .collect();

    for backend in active_backends {
        let mut backend_deployed = Vec::new();

        for (art, _) in effective_artifacts {
            if art.kind == ArtifactKind::Skill && backend.reads_agents_skills() && has_agents {
                continue;
            }

            let result = match art.kind {
                ArtifactKind::Skill => backend.deploy_skill(art, config),
                ArtifactKind::Agent => backend.deploy_agent(art, config),
                ArtifactKind::Hook => backend.deploy_hook(art, config),
            };
            match result {
                Ok(d) => backend_deployed.push(d),
                Err(_) if is_unsupported_for_backend(backend.name(), &art.kind) => {
                    continue;
                }
                Err(e) => {
                    cleanup::rollback(&all_deployed, config);
                    cleanup::rollback(&backend_deployed, config);
                    return Err(e);
                }
            }
        }

        let mcp_servers = if let Some(ref mcp) = raw_manifest.mcp {
            match backend.register_mcp(mcp, config) {
                Ok(entries) => entries.into_iter().map(|e| e.server_name).collect(),
                Err(_) if is_mcp_unsupported(backend.name()) => vec![],
                Err(e) => {
                    cleanup::rollback(&all_deployed, config);
                    cleanup::rollback(&backend_deployed, config);
                    return Err(e);
                }
            }
        } else {
            vec![]
        };

        let deployed_entries: Vec<DeployedArtifactEntry> = backend_deployed
            .iter()
            .map(|d| {
                let original = original_names
                    .get(&(&d.artifact_kind, d.artifact_name.as_str()))
                    .and_then(|o| o.cloned());
                DeployedArtifactEntry {
                    artifact_type: d.artifact_kind.clone(),
                    name: d.artifact_name.clone(),
                    deployed_path: d.deployed_path.to_string_lossy().to_string(),
                    deployed_hooks: d.deployed_hooks.clone(),
                    original_name: original,
                }
            })
            .collect();

        deployed_map.insert(
            backend.name().to_string(),
            BackendDeployment {
                artifacts: deployed_entries,
                mcp_servers,
            },
        );

        all_deployed.extend(backend_deployed);
    }

    Ok(DeploymentResult {
        all_deployed,
        deployed_map,
    })
}
