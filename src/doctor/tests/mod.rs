mod check_deployed_tests;
mod check_env_tests;
mod check_hooks_tests;
mod check_mcp_tests;
mod check_skill_tests;
mod format_tests;
mod report_tests;

use std::collections::HashMap;

use crate::artifact::ArtifactKind;
use crate::hook::DeployedHookEntry;
use crate::install_cache::{BackendDeployment, DeployedArtifactEntry, PackageEntry};

pub fn make_entry(artifacts: Vec<DeployedArtifactEntry>) -> PackageEntry {
    let mut deployed = HashMap::new();
    deployed.insert(
        "claude".to_string(),
        BackendDeployment {
            artifacts,
            mcp_servers: vec![],
        },
    );
    PackageEntry {
        version: "1.0.0".to_string(),
        source: "local".to_string(),
        source_path: "/tmp/pkg".to_string(),
        integrity: "abc".to_string(),
        archive_path: "/tmp/a.tar.gz".to_string(),
        deployed,
        resolved: None,
        tag: None,
    }
}

pub fn make_artifact(kind: ArtifactKind, name: &str, path: &str) -> DeployedArtifactEntry {
    DeployedArtifactEntry {
        artifact_type: kind,
        name: name.to_string(),
        deployed_path: path.to_string(),
        deployed_hooks: vec![],
        original_name: None,
    }
}

pub fn make_hook_entry(event: &str, matcher: Option<&str>, command: &str) -> DeployedArtifactEntry {
    DeployedArtifactEntry {
        artifact_type: ArtifactKind::Hook,
        name: "hook".to_string(),
        deployed_path: "/settings.json".to_string(),
        deployed_hooks: vec![DeployedHookEntry {
            event: event.to_string(),
            matcher: matcher.map(String::from),
            command: command.to_string(),
        }],
        original_name: None,
    }
}

pub fn set_mcp_servers(entry: &mut PackageEntry, servers: Vec<String>) {
    if let Some(claude) = entry.deployed.get_mut("claude") {
        claude.mcp_servers = servers;
    }
}
