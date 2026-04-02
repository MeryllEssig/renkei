pub mod agents;
pub mod claude;
pub mod codex;
pub mod cursor;
pub mod gemini;

use std::path::PathBuf;

use crate::artifact::{Artifact, ArtifactKind};
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::hook::DeployedHookEntry;
use crate::mcp::DeployedMcpEntry;

#[derive(Debug, Clone)]
pub struct DeployedArtifact {
    pub artifact_kind: ArtifactKind,
    pub artifact_name: String,
    pub deployed_path: PathBuf,
    pub deployed_hooks: Vec<DeployedHookEntry>,
}

/// Copy an artifact source file to `dest_dir/dest_filename`, creating dirs as needed.
/// Shared by all backends that do simple file copies (skills, agents).
pub(super) fn deploy_file(
    artifact: &Artifact,
    dest_dir: PathBuf,
    dest_filename: &str,
) -> Result<DeployedArtifact> {
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(dest_filename);
    std::fs::copy(&artifact.source_path, &dest).map_err(|e| {
        RenkeiError::DeploymentFailed(format!(
            "Failed to copy {} to {}: {}",
            artifact.source_path.display(),
            dest.display(),
            e
        ))
    })?;
    Ok(DeployedArtifact {
        artifact_kind: artifact.kind.clone(),
        artifact_name: artifact.name.clone(),
        deployed_path: dest,
        deployed_hooks: vec![],
    })
}

#[allow(dead_code)]
pub trait Backend {
    fn name(&self) -> &str;
    fn detect_installed(&self, config: &Config) -> bool;
    /// Returns true if this backend reads skills from the `.agents/skills/` directory
    /// (e.g. Codex, Gemini). Used for deduplication: when `agents` backend is also
    /// in the active set, skills for this backend are skipped to avoid double-deploy.
    fn reads_agents_skills(&self) -> bool {
        false
    }
    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact>;
    fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact>;
    fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact>;
    fn register_mcp(
        &self,
        mcp_config: &serde_json::Value,
        config: &Config,
    ) -> Result<Vec<DeployedMcpEntry>>;
}

/// All known backend names.
#[allow(dead_code)]
pub const ALL_BACKEND_NAMES: &[&str] = &["claude", "agents", "cursor", "codex", "gemini"];

pub struct BackendRegistry {
    backends: Vec<Box<dyn Backend>>,
}

impl BackendRegistry {
    /// Create a registry with all known backends.
    pub fn all() -> Self {
        Self {
            backends: vec![
                Box::new(claude::ClaudeBackend),
                Box::new(agents::AgentsBackend),
                Box::new(cursor::CursorBackend),
                Box::new(codex::CodexBackend),
                Box::new(gemini::GeminiBackend),
            ],
        }
    }

    /// Return only backends that are detected as installed.
    pub fn detect(&self, config: &Config) -> Vec<&dyn Backend> {
        self.backends
            .iter()
            .filter(|b| b.detect_installed(config))
            .map(|b| b.as_ref())
            .collect()
    }

    /// Resolve which backends to use for a given install.
    ///
    /// - `manifest_backends`: the `backends` field from renkei.json
    /// - `force`: if true, skip manifest intersection (use all detected)
    /// - `warnings`: collects warning messages for backends in manifest but not detected
    #[allow(dead_code)]
    pub fn resolve(
        &self,
        config: &Config,
        manifest_backends: &[String],
        force: bool,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<&dyn Backend>> {
        let detected = self.detect(config);
        let detected_names: Vec<&str> = detected.iter().map(|b| b.name()).collect();

        if force {
            // Force bypasses manifest intersection but NOT detection filter
            return if detected.is_empty() {
                Err(RenkeiError::BackendNotDetected {
                    required: manifest_backends.join(", "),
                    detected: "none".to_string(),
                })
            } else {
                Ok(detected)
            };
        }

        // Warn for each manifest backend that is not detected
        for mb in manifest_backends {
            if !detected_names.contains(&mb.as_str()) {
                warnings.push(format!(
                    "Backend '{}' listed in manifest but not detected",
                    mb
                ));
            }
        }

        // Intersect manifest with detected
        let resolved: Vec<&dyn Backend> = detected
            .into_iter()
            .filter(|b| manifest_backends.iter().any(|mb| mb == b.name()))
            .collect();

        if resolved.is_empty() {
            return Err(RenkeiError::BackendNotDetected {
                required: manifest_backends.join(", "),
                detected: if detected_names.is_empty() {
                    "none".to_string()
                } else {
                    detected_names.join(", ")
                },
            });
        }

        Ok(resolved)
    }

    /// Return (name, detected) pairs for all known backends.
    pub fn status(&self, config: &Config) -> Vec<(String, bool)> {
        self.backends
            .iter()
            .map(|b| (b.name().to_string(), b.detect_installed(config)))
            .collect()
    }

    /// Get a backend by name (for uninstall/doctor lookups).
    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<&dyn Backend> {
        self.backends
            .iter()
            .find(|b| b.name() == name)
            .map(|b| b.as_ref())
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use std::fs;

    use crate::artifact::{Artifact, ArtifactKind};

    pub fn make_skill_artifact(pkg_dir: &std::path::Path, name: &str, content: &str) -> Artifact {
        let skills_dir = pkg_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        let source = skills_dir.join(format!("{name}.md"));
        fs::write(&source, content).unwrap();
        Artifact {
            kind: ArtifactKind::Skill,
            name: name.to_string(),
            source_path: source,
        }
    }

    pub fn make_agent_artifact(pkg_dir: &std::path::Path, name: &str, content: &str) -> Artifact {
        let agents_dir = pkg_dir.join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        let source = agents_dir.join(format!("{name}.md"));
        fs::write(&source, content).unwrap();
        Artifact {
            kind: ArtifactKind::Agent,
            name: name.to_string(),
            source_path: source,
        }
    }

    pub fn make_hook_artifact(pkg_dir: &std::path::Path, name: &str, content: &str) -> Artifact {
        let hooks_dir = pkg_dir.join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let source = hooks_dir.join(format!("{name}.json"));
        fs::write(&source, content).unwrap();
        Artifact {
            kind: ArtifactKind::Hook,
            name: name.to_string(),
            source_path: source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_registry_all_contains_known_backends() {
        let registry = BackendRegistry::all();
        let names: Vec<&str> = registry.backends.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"agents"));
    }

    #[test]
    fn test_registry_all_contains_five_backends() {
        let registry = BackendRegistry::all();
        assert_eq!(registry.backends.len(), 5);
        let names: Vec<&str> = registry.backends.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"gemini"));
    }

    #[test]
    fn test_reads_agents_skills_defaults_false() {
        let registry = BackendRegistry::all();
        let claude = registry.get("claude").unwrap();
        let agents = registry.get("agents").unwrap();
        let cursor = registry.get("cursor").unwrap();
        assert!(!claude.reads_agents_skills());
        assert!(!agents.reads_agents_skills());
        assert!(!cursor.reads_agents_skills());
    }

    #[test]
    fn test_detect_returns_only_installed() {
        let dir = tempdir().unwrap();
        // No .claude dir → claude not detected, but agents always detected
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let registry = BackendRegistry::all();
        let detected = registry.detect(&config);
        let names: Vec<&str> = detected.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"agents"));
        assert!(!names.contains(&"claude"));
    }

    #[test]
    fn test_detect_with_claude_installed() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let registry = BackendRegistry::all();
        let detected = registry.detect(&config);
        let names: Vec<&str> = detected.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"agents"));
    }

    #[test]
    fn test_resolve_intersects_manifest_and_detected() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let registry = BackendRegistry::all();
        let mut warnings = Vec::new();

        let resolved = registry
            .resolve(
                &config,
                &["claude".to_string(), "agents".to_string()],
                false,
                &mut warnings,
            )
            .unwrap();

        let names: Vec<&str> = resolved.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"agents"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_resolve_empty_intersection_errors() {
        let dir = tempdir().unwrap();
        // No .claude dir, manifest only asks for claude
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let registry = BackendRegistry::all();
        let mut warnings = Vec::new();

        let result = registry.resolve(&config, &["claude".to_string()], false, &mut warnings);

        let err = result.err().expect("should be an error").to_string();
        assert!(err.contains("claude"));
    }

    #[test]
    fn test_resolve_force_bypasses_manifest() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let registry = BackendRegistry::all();
        let mut warnings = Vec::new();

        // Manifest says only "agents", but force=true → returns all detected
        let resolved = registry
            .resolve(&config, &["agents".to_string()], true, &mut warnings)
            .unwrap();

        let names: Vec<&str> = resolved.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"agents"));
    }

    #[test]
    fn test_resolve_force_does_not_bypass_detection() {
        let dir = tempdir().unwrap();
        // No .claude dir → claude not detected even with force
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let registry = BackendRegistry::all();
        let mut warnings = Vec::new();

        let resolved = registry
            .resolve(&config, &["claude".to_string()], true, &mut warnings)
            .unwrap();

        let names: Vec<&str> = resolved.iter().map(|b| b.name()).collect();
        assert!(!names.contains(&"claude"));
        assert!(names.contains(&"agents")); // agents always detected
    }

    #[test]
    fn test_resolve_warns_per_undetected_backend() {
        let dir = tempdir().unwrap();
        // No .claude dir
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let registry = BackendRegistry::all();
        let mut warnings = Vec::new();

        // Ask for claude+agents, only agents detected
        let resolved = registry
            .resolve(
                &config,
                &["claude".to_string(), "agents".to_string()],
                false,
                &mut warnings,
            )
            .unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name(), "agents");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("claude"));
        assert!(warnings[0].contains("not detected"));
    }

    #[test]
    fn test_get_backend_by_name() {
        let registry = BackendRegistry::all();
        assert_eq!(registry.get("claude").unwrap().name(), "claude");
        assert_eq!(registry.get("agents").unwrap().name(), "agents");
        assert!(registry.get("nonexistent").is_none());
    }
}
