use crate::artifact::{Artifact, ArtifactKind};
use crate::config::{BackendId, Config};
use crate::error::Result;
use crate::hook;
use crate::mcp::{self, DeployedMcpEntry};

use super::{Backend, DeployedArtifact};

pub struct GeminiBackend;

impl Backend for GeminiBackend {
    fn name(&self) -> &str {
        "gemini"
    }

    fn backend_id(&self) -> BackendId {
        BackendId::Gemini
    }

    fn detect_installed(&self, config: &Config) -> bool {
        config.backend(BackendId::Gemini).root_dir.is_dir()
    }

    fn reads_agents_skills(&self) -> bool {
        true
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let dirs = config.backend(BackendId::Gemini);
        let skill_dir = dirs
            .skills_dir
            .unwrap()
            .join(format!("renkei-{}", artifact.name));
        super::deploy_skill_dir(artifact, skill_dir)
    }

    fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let dirs = config.backend(BackendId::Gemini);
        let dest_filename = format!("{}.md", artifact.name);
        super::deploy_file(artifact, dirs.agents_dir.unwrap(), &dest_filename)
    }

    fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let dirs = config.backend(BackendId::Gemini);
        let renkei_hooks = hook::parse_hook_file(&artifact.source_path)?;
        let settings_path = dirs.settings_path.unwrap();
        let deployed_entries = hook::deploy(&hook::GEMINI, &renkei_hooks, &settings_path)?;

        Ok(DeployedArtifact {
            artifact_kind: ArtifactKind::Hook,
            artifact_name: artifact.name.clone(),
            deployed_path: settings_path,
            deployed_hooks: deployed_entries,
        })
    }

    /// Register MCP servers by merging into `.gemini/settings.json` under `mcpServers`.
    fn register_mcp(
        &self,
        mcp_config: &serde_json::Value,
        config: &Config,
    ) -> Result<Vec<DeployedMcpEntry>> {
        let dirs = config.backend(BackendId::Gemini);
        mcp::merge_mcp_into_config(&dirs.settings_path.unwrap(), mcp_config)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::artifact::ArtifactKind;
    use crate::backend::test_helpers::{
        make_agent_artifact, make_hook_artifact, make_skill_artifact,
    };
    use tempfile::tempdir;

    #[test]
    fn test_gemini_detect_with_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".gemini")).unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(GeminiBackend.detect_installed(&config));
    }

    #[test]
    fn test_gemini_detect_without_dir() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(!GeminiBackend.detect_installed(&config));
    }

    #[test]
    fn test_gemini_reads_agents_skills_true() {
        assert!(GeminiBackend.reads_agents_skills());
    }

    #[test]
    fn test_deploy_skill_creates_correct_path() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_skill_artifact(pkg.path(), "review", "# Review\nDo a review.");
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = GeminiBackend.deploy_skill(&artifact, &config).unwrap();

        let expected_dir = home.path().join(".gemini/skills/renkei-review");
        assert_eq!(result.deployed_path, expected_dir);
        assert_eq!(result.artifact_kind, ArtifactKind::Skill);
        assert!(expected_dir.join("SKILL.md").exists());
        assert_eq!(
            fs::read_to_string(expected_dir.join("SKILL.md")).unwrap(),
            "# Review\nDo a review."
        );
    }

    #[test]
    fn test_deploy_skill_project_scope() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_skill_artifact(pkg.path(), "lint", "# Lint");
        let config = Config::for_project(home.path().to_path_buf(), project.path().to_path_buf());

        GeminiBackend.deploy_skill(&artifact, &config).unwrap();

        let expected_dir = project.path().join(".gemini/skills/renkei-lint");
        assert!(expected_dir.join("SKILL.md").exists());
    }

    #[test]
    fn test_deploy_agent_creates_md() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_agent_artifact(pkg.path(), "researcher", "# Researcher");
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = GeminiBackend.deploy_agent(&artifact, &config).unwrap();

        let expected = home.path().join(".gemini/agents/researcher.md");
        assert_eq!(result.deployed_path, expected);
        assert_eq!(result.artifact_kind, ArtifactKind::Agent);
        assert!(expected.exists());
    }

    #[test]
    fn test_deploy_hook_merges_into_settings() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_hook_artifact(
            pkg.path(),
            "security",
            r#"[{"event":"before_tool","matcher":"write_file","command":"check.sh","timeout":5}]"#,
        );
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = GeminiBackend.deploy_hook(&artifact, &config).unwrap();

        let settings_path = home.path().join(".gemini/settings.json");
        assert_eq!(result.deployed_path, settings_path);
        assert_eq!(result.deployed_hooks.len(), 1);
        assert_eq!(result.deployed_hooks[0].event, "BeforeTool");
        assert_eq!(result.deployed_hooks[0].command, "check.sh");
        assert!(settings_path.exists());

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(content["hooks"]["BeforeTool"].is_array());
        assert_eq!(content["hooks"]["BeforeTool"][0]["matcher"], "write_file");
    }

    #[test]
    fn test_deploy_hook_merges_with_existing_settings() {
        let home = tempdir().unwrap();
        let gemini_dir = home.path().join(".gemini");
        fs::create_dir_all(&gemini_dir).unwrap();
        fs::write(
            gemini_dir.join("settings.json"),
            r#"{"theme":"dark","hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"init.sh"}]}]}}"#,
        )
        .unwrap();

        let pkg = tempdir().unwrap();
        let artifact = make_hook_artifact(
            pkg.path(),
            "tool-check",
            r#"[{"event":"before_tool","command":"check.sh"}]"#,
        );
        let config = Config::with_home_dir(home.path().to_path_buf());

        GeminiBackend.deploy_hook(&artifact, &config).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(gemini_dir.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(content["theme"], "dark");
        assert!(content["hooks"]["SessionStart"].is_array());
        assert!(content["hooks"]["BeforeTool"].is_array());
    }

    #[test]
    fn test_register_mcp_merges_into_settings() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());

        let mcp = serde_json::json!({
            "my-server": { "command": "node", "args": ["server.js"] }
        });

        let entries = GeminiBackend.register_mcp(&mcp, &config).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server_name, "my-server");

        let settings_path = home.path().join(".gemini/settings.json");
        assert!(settings_path.exists());
        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(content["mcpServers"]["my-server"]["command"], "node");
    }

    #[test]
    fn test_register_mcp_preserves_other_settings() {
        let home = tempdir().unwrap();
        let gemini_dir = home.path().join(".gemini");
        fs::create_dir_all(&gemini_dir).unwrap();
        fs::write(
            gemini_dir.join("settings.json"),
            r#"{"theme":"dark","mcpServers":{"existing":{"command":"keep"}}}"#,
        )
        .unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let mcp = serde_json::json!({
            "new-server": { "command": "python" }
        });

        GeminiBackend.register_mcp(&mcp, &config).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(gemini_dir.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(content["theme"], "dark");
        assert_eq!(content["mcpServers"]["existing"]["command"], "keep");
        assert_eq!(content["mcpServers"]["new-server"]["command"], "python");
    }
}
