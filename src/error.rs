use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenkeiError {
    #[error("Manifest not found at {0}")]
    ManifestNotFound(PathBuf),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("Invalid package name '{name}': must be scoped as @scope/name")]
    InvalidScope { name: String },

    #[error("Invalid version '{version}': {reason}")]
    InvalidVersion { version: String, reason: String },

    #[error("No artifacts found in {0}")]
    NoArtifactsFound(PathBuf),

    #[error("Deployment failed: {0}")]
    DeploymentFailed(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Scope conflict: {message}")]
    ScopeConflict { message: String },

    #[allow(dead_code)]
    #[error("No project root detected (not inside a git repository).\nUse `rk install -g <source>` to install globally.")]
    NoProjectRoot,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, RenkeiError>;
