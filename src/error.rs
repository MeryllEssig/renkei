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

    #[error("No project root detected (not inside a git repository).\nUse `rk install -g <source>` to install globally.")]
    NoProjectRoot,

    #[error("Git clone failed for {url}: {reason}")]
    GitCloneFailed { url: String, reason: String },

    #[error("No compatible backend detected. Package requires: {required}. Detected: {detected}.\nUse --force to override.")]
    BackendNotDetected { required: String, detected: String },

    #[error("Conflict: {kind} '{name}' is already deployed by package '{owner}'.\nUse --force to overwrite, or rename interactively in a TTY.")]
    ArtifactConflict {
        kind: String,
        name: String,
        owner: String,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, RenkeiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_clone_failed_message() {
        let err = RenkeiError::GitCloneFailed {
            url: "git@github.com:user/repo".to_string(),
            reason: "repository not found".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("git@github.com:user/repo"));
        assert!(msg.contains("repository not found"));
    }

    #[test]
    fn test_backend_not_detected_message() {
        let err = RenkeiError::BackendNotDetected {
            required: "cursor".to_string(),
            detected: "claude".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("cursor"));
        assert!(msg.contains("claude"));
        assert!(msg.contains("--force"));
    }

    #[test]
    fn test_backend_not_detected_none() {
        let err = RenkeiError::BackendNotDetected {
            required: "cursor".to_string(),
            detected: "none".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("none"));
    }
}
