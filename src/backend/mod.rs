pub mod claude;

use std::path::PathBuf;

use crate::artifact::{Artifact, ArtifactKind};
use crate::config::Config;
use crate::error::Result;
use crate::hook::DeployedHookEntry;
use crate::mcp::DeployedMcpEntry;

#[derive(Debug, Clone)]
pub struct DeployedArtifact {
    pub artifact_kind: ArtifactKind,
    pub artifact_name: String,
    pub deployed_path: PathBuf,
    pub deployed_hooks: Vec<DeployedHookEntry>,
}

#[allow(dead_code)]
pub trait Backend {
    fn name(&self) -> &str;
    fn detect_installed(&self, config: &Config) -> bool;
    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact>;
    fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact>;
    fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact>;
    fn register_mcp(
        &self,
        mcp_config: &serde_json::Value,
        config: &Config,
    ) -> Result<Vec<DeployedMcpEntry>>;
}
