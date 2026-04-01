use std::path::PathBuf;

use thiserror::Error;

use crate::artifact::ArtifactKind;

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

    #[error("Package '{package}' is not installed in {scope} scope")]
    PackageNotFound { package: String, scope: String },

    #[error("No project root detected (not inside a git repository).\nUse the -g flag for global scope.")]
    NoProjectRoot,

    #[error("Git clone failed for {url}: {reason}")]
    GitCloneFailed { url: String, reason: String },

    #[error("No compatible backend detected. Package requires: {required}. Detected: {detected}.\nUse --force to override.")]
    BackendNotDetected { required: String, detected: String },

    #[error("Conflict: {kind} '{name}' is already deployed by package '{owner}'.\nUse --force to overwrite, or rename interactively in a TTY.")]
    ArtifactConflict {
        kind: ArtifactKind,
        name: String,
        owner: String,
    },

    #[error("No lockfile found at {path}.\n{hint}")]
    LockfileNotFound { path: String, hint: String },

    #[error("Integrity check failed for '{package}': expected {expected}, got {actual}")]
    IntegrityMismatch {
        package: String,
        expected: String,
        actual: String,
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
    fn test_package_not_found_project_message() {
        let err = RenkeiError::PackageNotFound {
            package: "@acme/review".to_string(),
            scope: "project".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("@acme/review"));
        assert!(msg.contains("project"));
        assert!(msg.contains("not installed"));
    }

    #[test]
    fn test_package_not_found_global_message() {
        let err = RenkeiError::PackageNotFound {
            package: "@acme/deploy".to_string(),
            scope: "global".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("@acme/deploy"));
        assert!(msg.contains("global"));
    }

    #[test]
    fn test_no_project_root_message_is_generic() {
        let err = RenkeiError::NoProjectRoot;
        let msg = err.to_string();
        assert!(msg.contains("-g flag"));
        assert!(!msg.contains("install"));
    }

    #[test]
    fn test_lockfile_not_found_message() {
        let err = RenkeiError::LockfileNotFound {
            path: "/projects/foo/rk.lock".to_string(),
            hint: "Use `rk install <source>` to install a package.".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("/projects/foo/rk.lock"));
        assert!(msg.contains("rk install <source>"));
    }

    #[test]
    fn test_integrity_mismatch_message() {
        let err = RenkeiError::IntegrityMismatch {
            package: "@test/pkg".to_string(),
            expected: "sha256-aaa".to_string(),
            actual: "sha256-bbb".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("@test/pkg"));
        assert!(msg.contains("sha256-aaa"));
        assert!(msg.contains("sha256-bbb"));
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
