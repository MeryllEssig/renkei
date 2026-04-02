use std::fs;

use owo_colors::OwoColorize;

use crate::artifact::{Artifact, ArtifactKind};
use crate::config::{BackendId, Config};
use crate::error::{RenkeiError, Result};
use crate::hook;
use crate::mcp::DeployedMcpEntry;

use super::{Backend, DeployedArtifact};

pub struct CodexBackend;

impl Backend for CodexBackend {
    fn name(&self) -> &str {
        "codex"
    }

    fn backend_id(&self) -> BackendId {
        BackendId::Codex
    }

    fn detect_installed(&self, config: &Config) -> bool {
        config.backend(BackendId::Codex).root_dir.is_dir()
    }

    fn reads_agents_skills(&self) -> bool {
        true
    }

    fn deploy_skill(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let skill_dir = config
            .backend(BackendId::Agents)
            .skills_dir
            .unwrap()
            .join(format!("renkei-{}", artifact.name));
        super::deploy_file(artifact, skill_dir, "SKILL.md")
    }

    /// Deploy agent as TOML file to `.codex/agents/{name}.toml`.
    fn deploy_agent(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let dirs = config.backend(BackendId::Codex);
        let dest_dir = dirs.agents_dir.unwrap();
        fs::create_dir_all(&dest_dir)?;

        let md_content = fs::read_to_string(&artifact.source_path).map_err(|e| {
            RenkeiError::DeploymentFailed(format!(
                "Failed to read {}: {}",
                artifact.source_path.display(),
                e
            ))
        })?;

        let mut table = toml::map::Map::new();
        table.insert("name".to_string(), toml::Value::String(artifact.name.clone()));
        table.insert(
            "developer_instructions".to_string(),
            toml::Value::String(md_content),
        );

        let toml_content = toml::to_string_pretty(&toml::Value::Table(table)).map_err(|e| {
            RenkeiError::DeploymentFailed(format!("Failed to serialize agent TOML: {}", e))
        })?;

        let dest = dest_dir.join(format!("{}.toml", artifact.name));
        fs::write(&dest, toml_content).map_err(|e| {
            RenkeiError::DeploymentFailed(format!("Failed to write {}: {}", dest.display(), e))
        })?;

        Ok(DeployedArtifact {
            artifact_kind: ArtifactKind::Agent,
            artifact_name: artifact.name.clone(),
            deployed_path: dest,
            deployed_hooks: vec![],
        })
    }

    fn deploy_hook(&self, artifact: &Artifact, config: &Config) -> Result<DeployedArtifact> {
        let dirs = config.backend(BackendId::Codex);
        let renkei_hooks = hook::parse_hook_file(&artifact.source_path)?;
        let translated = hook::translate_hooks_with(&renkei_hooks, hook::translate_event_codex)?;
        let hooks_path = dirs.hooks_path.unwrap();
        let deployed_entries = hook::write_standalone_hooks(&hooks_path, &translated)?;

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
        let servers = mcp_config.as_object().ok_or_else(|| {
            RenkeiError::InvalidManifest("mcp field must be a JSON object".into())
        })?;

        let dirs = config.backend(BackendId::Codex);
        let config_path = dirs.config_path.unwrap();

        let mut toml_value: toml::Value = match fs::read_to_string(&config_path) {
            Ok(content) => toml::from_str(&content).map_err(|e| {
                RenkeiError::DeploymentFailed(format!(
                    "Failed to parse {}: {}",
                    config_path.display(),
                    e
                ))
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                toml::Value::Table(toml::map::Map::new())
            }
            Err(e) => return Err(e.into()),
        };

        let root = toml_value.as_table_mut().ok_or_else(|| {
            RenkeiError::DeploymentFailed("codex config.toml is not a TOML table".into())
        })?;

        let mcp_servers = root
            .entry("mcp_servers".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

        let servers_table = mcp_servers.as_table_mut().ok_or_else(|| {
            RenkeiError::DeploymentFailed(
                "codex config.toml mcp_servers is not a TOML table".into(),
            )
        })?;

        let mut deployed = Vec::new();

        for (name, server_config) in servers {
            if servers_table.contains_key(name) {
                eprintln!(
                    "  {} MCP server '{}' already exists in codex config, skipping",
                    "Warning:".yellow().bold(),
                    name
                );
                continue;
            }
            let toml_config = json_to_toml(server_config)?;
            servers_table.insert(name.clone(), toml_config);
            deployed.push(DeployedMcpEntry {
                server_name: name.clone(),
            });
        }

        if !deployed.is_empty() {
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(&toml_value).map_err(|e| {
                RenkeiError::DeploymentFailed(format!("Failed to serialize config.toml: {}", e))
            })?;
            fs::write(&config_path, content)?;
        }

        Ok(deployed)
    }
}

fn json_to_toml(json: &serde_json::Value) -> Result<toml::Value> {
    match json {
        serde_json::Value::String(s) => Ok(toml::Value::String(s.clone())),
        serde_json::Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                Err(RenkeiError::DeploymentFailed(format!(
                    "Unrepresentable number in TOML: {}",
                    n
                )))
            }
        }
        serde_json::Value::Array(arr) => {
            let items: Result<Vec<_>> = arr.iter().map(json_to_toml).collect();
            Ok(toml::Value::Array(items?))
        }
        serde_json::Value::Object(obj) => {
            let mut table = toml::map::Map::new();
            for (k, v) in obj {
                table.insert(k.clone(), json_to_toml(v)?);
            }
            Ok(toml::Value::Table(table))
        }
        serde_json::Value::Null => Err(RenkeiError::DeploymentFailed(
            "Null values are not representable in TOML".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactKind;
    use crate::backend::test_helpers::{make_agent_artifact, make_hook_artifact, make_skill_artifact};
    use tempfile::tempdir;

    #[test]
    fn test_codex_detect_with_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".codex")).unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(CodexBackend.detect_installed(&config));
    }

    #[test]
    fn test_codex_detect_without_dir() {
        let dir = tempdir().unwrap();
        let config = Config::with_home_dir(dir.path().to_path_buf());
        assert!(!CodexBackend.detect_installed(&config));
    }

    #[test]
    fn test_codex_reads_agents_skills_true() {
        assert!(CodexBackend.reads_agents_skills());
    }

    #[test]
    fn test_deploy_skill_creates_agents_path() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_skill_artifact(pkg.path(), "review", "# Review\nDo a review.");
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = CodexBackend.deploy_skill(&artifact, &config).unwrap();

        let expected = home.path().join(".agents/skills/renkei-review/SKILL.md");
        assert_eq!(result.deployed_path, expected);
        assert!(expected.exists());
        assert_eq!(
            fs::read_to_string(&expected).unwrap(),
            "# Review\nDo a review."
        );
    }

    #[test]
    fn test_deploy_agent_creates_toml() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_agent_artifact(pkg.path(), "researcher", "# Researcher\nDoes research.");
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = CodexBackend.deploy_agent(&artifact, &config).unwrap();

        let expected = home.path().join(".codex/agents/researcher.toml");
        assert_eq!(result.deployed_path, expected);
        assert_eq!(result.artifact_kind, ArtifactKind::Agent);
        assert!(expected.exists());

        let content = fs::read_to_string(&expected).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(parsed["name"].as_str(), Some("researcher"));
        assert!(parsed["developer_instructions"]
            .as_str()
            .unwrap()
            .contains("# Researcher"));
    }

    #[test]
    fn test_deploy_hook_writes_standalone_json() {
        let home = tempdir().unwrap();
        let pkg = tempdir().unwrap();

        let artifact = make_hook_artifact(
            pkg.path(),
            "lint",
            r#"[{"event":"before_tool","matcher":"bash","command":"lint.sh","timeout":5}]"#,
        );
        let config = Config::with_home_dir(home.path().to_path_buf());

        let result = CodexBackend.deploy_hook(&artifact, &config).unwrap();

        let hooks_path = home.path().join(".codex/hooks.json");
        assert_eq!(result.deployed_path, hooks_path);
        assert_eq!(result.deployed_hooks.len(), 1);
        assert_eq!(result.deployed_hooks[0].event, "PreToolUse");
        assert_eq!(result.deployed_hooks[0].command, "lint.sh");
        assert!(hooks_path.exists());

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&hooks_path).unwrap()).unwrap();
        assert!(content["hooks"]["PreToolUse"].is_array());
        assert_eq!(
            content["hooks"]["PreToolUse"][0]["matcher"],
            "bash"
        );
    }

    #[test]
    fn test_register_mcp_writes_config_toml() {
        let home = tempdir().unwrap();
        let config = Config::with_home_dir(home.path().to_path_buf());
        fs::create_dir_all(home.path().join(".codex")).unwrap();

        let mcp = serde_json::json!({
            "my-server": {
                "command": "node",
                "args": ["server.js"],
                "env": { "PORT": "3000" }
            }
        });

        let entries = CodexBackend.register_mcp(&mcp, &config).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server_name, "my-server");

        let config_path = home.path().join(".codex/config.toml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["my-server"]["command"].as_str(),
            Some("node")
        );
    }

    #[test]
    fn test_register_mcp_merges_into_existing_toml() {
        let home = tempdir().unwrap();
        let codex_dir = home.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(
            codex_dir.join("config.toml"),
            "[mcp_servers.existing]\ncommand = \"keep\"\n",
        )
        .unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let mcp = serde_json::json!({
            "new-server": { "command": "python", "args": ["serve.py"] }
        });

        let entries = CodexBackend.register_mcp(&mcp, &config).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server_name, "new-server");

        let content = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["existing"]["command"].as_str(),
            Some("keep")
        );
        assert_eq!(
            parsed["mcp_servers"]["new-server"]["command"].as_str(),
            Some("python")
        );
    }

    #[test]
    fn test_register_mcp_skips_existing_server() {
        let home = tempdir().unwrap();
        let codex_dir = home.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(
            codex_dir.join("config.toml"),
            "[mcp_servers.existing]\ncommand = \"keep\"\n",
        )
        .unwrap();

        let config = Config::with_home_dir(home.path().to_path_buf());
        let mcp = serde_json::json!({
            "existing": { "command": "override" }
        });

        let entries = CodexBackend.register_mcp(&mcp, &config).unwrap();
        assert!(entries.is_empty());

        let content = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["existing"]["command"].as_str(),
            Some("keep")
        );
    }
}
