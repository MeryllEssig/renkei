use std::fs;

use crate::artifact::Artifact;
use crate::config::Config;
use crate::error::{RenkeiError, Result};

use super::{Backend, DeployedArtifact};

pub struct ClaudeBackend;

impl Backend for ClaudeBackend {
    fn name(&self) -> &str {
        "claude"
    }

    fn detect_installed(&self, config: &Config) -> bool {
        config.claude_dir().is_dir()
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let skill_dir = config
            .claude_skills_dir()
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
            artifact_name: artifact.name.clone(),
            deployed_path: dest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactKind;
    use tempfile::tempdir;

    #[test]
    fn test_detect_installed_true() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let backend = ClaudeBackend;
        assert!(backend.detect_installed(&config));
    }

    #[test]
    fn test_detect_installed_false() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        let backend = ClaudeBackend;
        assert!(!backend.detect_installed(&config));
    }

    #[test]
    fn test_deploy_skill() {
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

        let backend = ClaudeBackend;
        let result = backend.deploy_skill(&artifact, &config).unwrap();

        let expected = home.path().join(".claude/skills/renkei-review/SKILL.md");
        assert_eq!(result.deployed_path, expected);
        assert!(expected.exists());
        assert_eq!(
            fs::read_to_string(&expected).unwrap(),
            "# Review\nReview the code."
        );
    }
}
