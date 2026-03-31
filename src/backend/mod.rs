pub mod claude;

use std::path::PathBuf;

use crate::artifact::Artifact;
use crate::config::Config;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct DeployedArtifact {
    pub artifact_name: String,
    pub deployed_path: PathBuf,
}

#[allow(dead_code)]
pub trait Backend {
    fn name(&self) -> &str;
    fn detect_installed(&self, config: &Config) -> bool;
    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact>;
}
