use crate::artifact::{Artifact, ArtifactKind};
use crate::config::{BackendId, Config};
use crate::error::Result;
use crate::mcp::{self, DeployedMcpEntry};

use super::{Backend, DeployedArtifact};

pub struct ClaudeBackend;


impl Backend for ClaudeBackend {
    fn name(&self) -> &str {
        "claude"
    }

    fn backend_id(&self) -> BackendId {
        BackendId::Claude
    }

    fn detect_installed(&self, config: &Config) -> bool {
        config.claude_dir().is_dir()
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let skill_dir = config
            .claude_skills_dir()
            .join(format!("renkei-{}", artifact.name));
        super::deploy_file(artifact, skill_dir, "SKILL.md")
    }

    fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let dest_filename = format!("{}.md", artifact.name);
        super::deploy_file(artifact, config.claude_agents_dir(), &dest_filename)
    }

    fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        use crate::hook;

        let renkei_hooks = hook::parse_hook_file(&artifact.source_path)?;
        let translated = hook::translate_hooks(&renkei_hooks)?;
        let settings_path = config.claude_settings_path();
        let deployed_entries = hook::merge_hooks_into_settings(&settings_path, &translated)?;

        Ok(DeployedArtifact {
            artifact_kind: ArtifactKind::Hook,
            artifact_name: artifact.name.clone(),
            deployed_path: settings_path,
            deployed_hooks: deployed_entries,
        })
    }

    fn register_mcp(
        &self,
        mcp_config: &serde_json::Value,
        config: &Config,
    ) -> Result<Vec<DeployedMcpEntry>> {
        let config_path = config.claude_config_path();
        mcp::merge_mcp_into_config(&config_path, mcp_config)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

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
    fn test_deploy_agent() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let agents_dir = pkg.path().join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        let source = agents_dir.join("deploy.md");
        fs::write(&source, "# Deploy\nDeploy the application.").unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let artifact = Artifact {
            kind: ArtifactKind::Agent,
            name: "deploy".to_string(),
            source_path: source,
        };

        let backend = ClaudeBackend;
        let result = backend.deploy_agent(&artifact, &config).unwrap();

        let expected = home.path().join(".claude/agents/deploy.md");
        assert_eq!(result.deployed_path, expected);
        assert_eq!(result.artifact_kind, ArtifactKind::Agent);
        assert!(expected.exists());
        assert_eq!(
            fs::read_to_string(&expected).unwrap(),
            "# Deploy\nDeploy the application."
        );
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
        assert_eq!(result.artifact_kind, ArtifactKind::Skill);
        assert!(expected.exists());
        assert_eq!(
            fs::read_to_string(&expected).unwrap(),
            "# Review\nReview the code."
        );
    }

    #[test]
    fn test_deploy_hook_creates_settings() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let hooks_dir = pkg.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let source = hooks_dir.join("lint.json");
        fs::write(
            &source,
            r#"[{"event":"before_tool","matcher":"bash","command":"lint.sh","timeout":5}]"#,
        )
        .unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let artifact = Artifact {
            kind: ArtifactKind::Hook,
            name: "lint".to_string(),
            source_path: source,
        };

        let backend = ClaudeBackend;
        let result = backend.deploy_hook(&artifact, &config).unwrap();

        assert_eq!(result.artifact_kind, ArtifactKind::Hook);
        assert_eq!(result.deployed_hooks.len(), 1);
        assert_eq!(result.deployed_hooks[0].event, "PreToolUse");
        assert_eq!(result.deployed_hooks[0].command, "lint.sh");

        let settings_path = home.path().join(".claude/settings.json");
        assert!(settings_path.exists());
        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "bash");
        assert_eq!(settings["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"], 5);
    }

    #[test]
    fn test_deploy_hook_merges_into_existing() {
        let home = tempdir().unwrap();
        let claude_dir = home.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("settings.json"),
            r#"{"language":"French","hooks":{"Stop":[{"hooks":[{"type":"command","command":"existing.sh"}]}]}}"#,
        )
        .unwrap();

        let pkg = tempdir().unwrap();
        let hooks_dir = pkg.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::write(
            hooks_dir.join("safety.json"),
            r#"[{"event":"before_tool","matcher":"bash","command":"check.sh"}]"#,
        )
        .unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let artifact = Artifact {
            kind: ArtifactKind::Hook,
            name: "safety".to_string(),
            source_path: hooks_dir.join("safety.json"),
        };

        let backend = ClaudeBackend;
        backend.deploy_hook(&artifact, &config).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(claude_dir.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(settings["language"], "French");
        assert!(settings["hooks"]["Stop"].is_array());
        assert!(settings["hooks"]["PreToolUse"].is_array());
    }

    #[test]
    fn test_deploy_hook_invalid_file() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();
        let hooks_dir = pkg.path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::write(hooks_dir.join("bad.json"), "not json").unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let artifact = Artifact {
            kind: ArtifactKind::Hook,
            name: "bad".to_string(),
            source_path: hooks_dir.join("bad.json"),
        };

        let backend = ClaudeBackend;
        assert!(backend.deploy_hook(&artifact, &config).is_err());
    }

    #[test]
    fn test_register_mcp_creates_claude_json() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        let backend = ClaudeBackend;

        let mcp = serde_json::json!({
            "test-server": {
                "command": "node",
                "args": ["server.js"],
                "env": { "PORT": "3000" }
            }
        });

        let entries = backend.register_mcp(&mcp, &config).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server_name, "test-server");

        let config_path = home.path().join(".claude.json");
        assert!(config_path.exists());
        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(content["mcpServers"]["test-server"]["command"], "node");
    }

    #[test]
    fn test_register_mcp_preserves_existing() {
        let home = tempdir().unwrap();
        let config_path = home.path().join(".claude.json");
        fs::write(
            &config_path,
            r#"{"mcpServers":{"existing":{"command":"keep","args":[]}}}"#,
        )
        .unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let backend = ClaudeBackend;

        let mcp = serde_json::json!({
            "new-server": { "command": "python", "args": ["serve.py"] }
        });

        let entries = backend.register_mcp(&mcp, &config).unwrap();
        assert_eq!(entries.len(), 1);

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(content["mcpServers"]["existing"]["command"], "keep");
        assert_eq!(content["mcpServers"]["new-server"]["command"], "python");
    }
}
