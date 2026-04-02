use std::fs;

use crate::artifact::{Artifact, ArtifactKind};
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::mcp::DeployedMcpEntry;

use super::{Backend, DeployedArtifact};

pub struct AgentsBackend;

impl Backend for AgentsBackend {
    fn name(&self) -> &str {
        "agents"
    }

    fn detect_installed(&self, _config: &Config) -> bool {
        true
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let skill_dir = config
            .agents_skills_dir()
            .join(format!("renkei-{}", artifact.name));
        fs::create_dir_all(&skill_dir)?;

        let dest = skill_dir.join("SKILL.md");
        fs::copy(&artifact.source_path, &dest).map_err(|e| {
            RenkeiError::DeploymentFailed(format!(
                "Failed to copy {} to {}: {}",
                artifact.source_path.display(),
                dest.display(),
                e
            ))
        })?;

        Ok(DeployedArtifact {
            artifact_kind: ArtifactKind::Skill,
            artifact_name: artifact.name.clone(),
            deployed_path: dest,
            deployed_hooks: vec![],
        })
    }

    fn deploy_agent(&self, _artifact: &Artifact, _config: &Config) -> Result<DeployedArtifact> {
        Err(RenkeiError::DeploymentFailed(
            "Agents backend does not support deploying agents".into(),
        ))
    }

    fn deploy_hook(&self, _artifact: &Artifact, _config: &Config) -> Result<DeployedArtifact> {
        Err(RenkeiError::DeploymentFailed(
            "Agents backend does not support deploying hooks".into(),
        ))
    }

    fn register_mcp(
        &self,
        _mcp_config: &serde_json::Value,
        _config: &Config,
    ) -> Result<Vec<DeployedMcpEntry>> {
        Err(RenkeiError::DeploymentFailed(
            "Agents backend does not support MCP registration".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Artifact;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_agents_name() {
        assert_eq!(AgentsBackend.name(), "agents");
    }

    #[test]
    fn test_agents_always_detected() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(AgentsBackend.detect_installed(&config));
    }

    #[test]
    fn test_deploy_skill_creates_correct_path_global() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let skills_dir = pkg.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        let source = skills_dir.join("review.md");
        fs::write(&source, "# Review\nReview the code.").unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let artifact = Artifact {
            kind: ArtifactKind::Skill,
            name: "review".to_string(),
            source_path: source,
        };

        let result = AgentsBackend.deploy_skill(&artifact, &config).unwrap();

        let expected = home
            .path()
            .join(".agents/skills/renkei-review/SKILL.md");
        assert_eq!(result.deployed_path, expected);
        assert_eq!(result.artifact_kind, ArtifactKind::Skill);
        assert!(expected.exists());
        assert_eq!(
            fs::read_to_string(&expected).unwrap(),
            "# Review\nReview the code."
        );
    }

    #[test]
    fn test_deploy_skill_creates_correct_path_project() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let skills_dir = pkg.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        let source = skills_dir.join("lint.md");
        fs::write(&source, "# Lint\nLint the code.").unwrap();

        let config = Config::for_project(
            home.path().to_path_buf(),
            project.path().to_path_buf(),
        );
        let artifact = Artifact {
            kind: ArtifactKind::Skill,
            name: "lint".to_string(),
            source_path: source,
        };

        let result = AgentsBackend.deploy_skill(&artifact, &config).unwrap();

        let expected = project
            .path()
            .join(".agents/skills/renkei-lint/SKILL.md");
        assert_eq!(result.deployed_path, expected);
        assert!(expected.exists());
    }

    #[test]
    fn test_deploy_agent_returns_unsupported() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let artifact = Artifact {
            kind: ArtifactKind::Agent,
            name: "deploy".to_string(),
            source_path: PathBuf::from("/tmp/fake.md"),
        };
        let result = AgentsBackend.deploy_agent(&artifact, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not support"));
    }

    #[test]
    fn test_deploy_hook_returns_unsupported() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let artifact = Artifact {
            kind: ArtifactKind::Hook,
            name: "lint".to_string(),
            source_path: PathBuf::from("/tmp/fake.json"),
        };
        let result = AgentsBackend.deploy_hook(&artifact, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not support"));
    }

    #[test]
    fn test_register_mcp_returns_unsupported() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let mcp = serde_json::json!({"server": {"command": "node"}});
        let result = AgentsBackend.register_mcp(&mcp, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not support"));
    }
}
