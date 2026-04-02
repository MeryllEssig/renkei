use std::fs;

use crate::artifact::{Artifact, ArtifactKind};
use crate::config::Config;
use crate::error::{RenkeiError, Result};
use crate::hook;
use crate::mcp::{self, DeployedMcpEntry};

use super::{Backend, DeployedArtifact};

pub struct CursorBackend;

impl CursorBackend {
    fn deploy_file(
        &self,
        artifact: &Artifact,
        dest_dir: std::path::PathBuf,
        dest_filename: &str,
    ) -> Result<DeployedArtifact> {
        fs::create_dir_all(&dest_dir)?;
        let dest = dest_dir.join(dest_filename);
        fs::copy(&artifact.source_path, &dest).map_err(|e| {
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
}

impl Backend for CursorBackend {
    fn name(&self) -> &str {
        "cursor"
    }

    fn detect_installed(&self, config: &Config) -> bool {
        config.cursor_dir().is_dir()
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let dest_dir = config.cursor_rules_dir();
        fs::create_dir_all(&dest_dir)?;

        let source_content = fs::read_to_string(&artifact.source_path).map_err(|e| {
            RenkeiError::DeploymentFailed(format!(
                "Failed to read {}: {}",
                artifact.source_path.display(),
                e
            ))
        })?;

        let frontmatter = "---\ndescription: \"\"\nalwaysApply: false\n---\n";
        let content = format!("{frontmatter}{source_content}");

        let dest = dest_dir.join(format!("renkei-{}.mdc", artifact.name));
        fs::write(&dest, content).map_err(|e| {
            RenkeiError::DeploymentFailed(format!(
                "Failed to write {}: {}",
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

    fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let dest_filename = format!("{}.md", artifact.name);
        self.deploy_file(artifact, config.cursor_agents_dir(), &dest_filename)
    }

    fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let renkei_hooks = hook::parse_hook_file(&artifact.source_path)?;
        let translated = hook::translate_hooks_cursor(&renkei_hooks)?;
        let hooks_path = config.cursor_hooks_path();
        let deployed_entries = hook::write_cursor_hooks(&hooks_path, &translated)?;

        Ok(DeployedArtifact {
            artifact_kind: ArtifactKind::Hook,
            artifact_name: artifact.name.clone(),
            deployed_path: hooks_path,
            deployed_hooks: deployed_entries,
        })
    }

    fn register_mcp(
        &self,
        mcp_config: &serde_json::Value,
        config: &Config,
    ) -> Result<Vec<DeployedMcpEntry>> {
        mcp::merge_mcp_into_config(&config.cursor_mcp_path(), mcp_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactKind;
    use tempfile::tempdir;

    fn make_skill_artifact(pkg_dir: &std::path::Path, name: &str, content: &str) -> Artifact {
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

    fn make_agent_artifact(pkg_dir: &std::path::Path, name: &str, content: &str) -> Artifact {
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

    fn make_hook_artifact(pkg_dir: &std::path::Path, name: &str, content: &str) -> Artifact {
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

    #[test]
    fn test_cursor_detect_with_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".cursor")).unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(CursorBackend.detect_installed(&config));
    }

    #[test]
    fn test_cursor_detect_without_dir() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(!CursorBackend.detect_installed(&config));
    }

    #[test]
    fn test_deploy_skill_creates_mdc_with_frontmatter() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_skill_artifact(pkg.path(), "review", "# Review\nDo a code review.");
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = CursorBackend.deploy_skill(&artifact, &config).unwrap();

        let expected_path = home.path().join(".cursor/rules/renkei-review.mdc");
        assert_eq!(result.deployed_path, expected_path);
        assert_eq!(result.artifact_kind, ArtifactKind::Skill);
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        assert!(content.starts_with("---\ndescription: \"\"\nalwaysApply: false\n---\n"));
        assert!(content.contains("# Review\nDo a code review."));
    }

    #[test]
    fn test_deploy_skill_project_scope() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_skill_artifact(pkg.path(), "lint", "# Lint\nLint the code.");
        let config = Config::for_project(
            home.path().to_path_buf(),
            project.path().to_path_buf(),
        );

        CursorBackend.deploy_skill(&artifact, &config).unwrap();

        let expected = project.path().join(".cursor/rules/renkei-lint.mdc");
        assert!(expected.exists());
    }

    #[test]
    fn test_deploy_agent_creates_md() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_agent_artifact(pkg.path(), "deploy", "# Deploy\nDeploy the app.");
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = CursorBackend.deploy_agent(&artifact, &config).unwrap();

        let expected = home.path().join(".cursor/agents/deploy.md");
        assert_eq!(result.deployed_path, expected);
        assert_eq!(result.artifact_kind, ArtifactKind::Agent);
        assert!(expected.exists());
        assert_eq!(
            fs::read_to_string(&expected).unwrap(),
            "# Deploy\nDeploy the app."
        );
    }

    #[test]
    fn test_deploy_hook_writes_cursor_format() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_hook_artifact(
            pkg.path(),
            "lint",
            r#"[{"event":"before_tool","matcher":"bash","command":"lint.sh","timeout":5}]"#,
        );
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = CursorBackend.deploy_hook(&artifact, &config).unwrap();

        let hooks_path = home.path().join(".cursor/hooks.json");
        assert_eq!(result.deployed_path, hooks_path);
        assert_eq!(result.deployed_hooks.len(), 1);
        assert_eq!(result.deployed_hooks[0].event, "preToolUse");
        assert_eq!(result.deployed_hooks[0].command, "lint.sh");
        assert!(hooks_path.exists());

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&hooks_path).unwrap()).unwrap();
        assert_eq!(content["version"], 1);
        assert!(content["hooks"]["preToolUse"].is_array());
        assert_eq!(content["hooks"]["preToolUse"][0]["command"], "lint.sh");
        assert_eq!(content["hooks"]["preToolUse"][0]["type"], "command");
        assert_eq!(content["hooks"]["preToolUse"][0]["matcher"], "bash");
        assert_eq!(content["hooks"]["preToolUse"][0]["timeout"], 5);
    }

    #[test]
    fn test_deploy_hook_merges_with_existing() {
        let home = tempdir().unwrap();
        let cursor_dir = home.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        fs::write(
            cursor_dir.join("hooks.json"),
            r#"{"version":1,"hooks":{"preToolUse":[{"command":"existing.sh","type":"command"}]}}"#,
        )
        .unwrap();

        let pkg = tempdir().unwrap();
        let artifact = make_hook_artifact(
            pkg.path(),
            "new",
            r#"[{"event":"before_tool","command":"new.sh"}]"#,
        );
        let config = Config::with_home_dir(home.path().to_path_buf());

        CursorBackend.deploy_hook(&artifact, &config).unwrap();

        let content: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(cursor_dir.join("hooks.json")).unwrap(),
        )
        .unwrap();
        let arr = content["hooks"]["preToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_register_mcp_merges_into_mcp_json() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let mcp = serde_json::json!({
            "my-server": { "command": "node", "args": ["server.js"] }
        });

        let entries = CursorBackend.register_mcp(&mcp, &config).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server_name, "my-server");

        let mcp_path = home.path().join(".cursor/mcp.json");
        assert!(mcp_path.exists());
        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
        assert_eq!(content["mcpServers"]["my-server"]["command"], "node");
    }
}
