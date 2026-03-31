use std::path::Path;

use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

use crate::error::{RenkeiError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeployedMcpEntry {
    pub server_name: String,
}

fn read_claude_config(path: &Path) -> Result<serde_json::Value> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(serde_json::from_str(&content)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(e) => Err(e.into()),
    }
}

fn write_claude_config(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(value)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn merge_mcp_into_config(
    config_path: &Path,
    mcp_config: &serde_json::Value,
) -> Result<Vec<DeployedMcpEntry>> {
    let servers = mcp_config.as_object().ok_or_else(|| {
        RenkeiError::InvalidManifest("mcp field must be a JSON object".into())
    })?;

    let mut config = read_claude_config(config_path)?;

    let config_obj = config.as_object_mut().ok_or_else(|| {
        RenkeiError::DeploymentFailed("~/.claude.json is not a JSON object".into())
    })?;

    let mcp_servers = config_obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    let mcp_map = mcp_servers.as_object_mut().ok_or_else(|| {
        RenkeiError::DeploymentFailed("mcpServers in ~/.claude.json is not a JSON object".into())
    })?;

    let mut deployed = Vec::new();

    for (name, server_config) in servers {
        if mcp_map.contains_key(name) {
            eprintln!(
                "  {} MCP server '{}' already exists, skipping",
                "Warning:".yellow().bold(),
                name
            );
            continue;
        }
        mcp_map.insert(name.clone(), server_config.clone());
        deployed.push(DeployedMcpEntry {
            server_name: name.clone(),
        });
    }

    write_claude_config(config_path, &config)?;
    Ok(deployed)
}

pub fn remove_mcp_from_config(
    config_path: &Path,
    entries_to_remove: &[DeployedMcpEntry],
) -> Result<()> {
    let mut config: serde_json::Value = match std::fs::read_to_string(config_path) {
        Ok(content) => serde_json::from_str(&content)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    let mcp_map = match config
        .as_object_mut()
        .and_then(|c| c.get_mut("mcpServers"))
        .and_then(|m| m.as_object_mut())
    {
        Some(m) => m,
        None => return Ok(()),
    };

    for entry in entries_to_remove {
        mcp_map.remove(&entry.server_name);
    }

    if mcp_map.is_empty() {
        config.as_object_mut().unwrap().remove("mcpServers");
    }

    write_claude_config(config_path, &config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_merge_into_empty_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");

        let mcp = serde_json::json!({
            "test-server": {
                "command": "node",
                "args": ["server.js"],
                "env": { "PORT": "3000" }
            }
        });

        let entries = merge_mcp_into_config(&config_path, &mcp).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server_name, "test-server");

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["test-server"]["command"], "node");
        assert_eq!(config["mcpServers"]["test-server"]["args"][0], "server.js");
    }

    #[test]
    fn test_merge_into_nonexistent_creates_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("sub").join(".claude.json");

        let mcp = serde_json::json!({
            "my-server": { "command": "python", "args": ["serve.py"] }
        });

        merge_mcp_into_config(&config_path, &mcp).unwrap();
        assert!(config_path.exists());
    }

    #[test]
    fn test_merge_preserves_existing_keys() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");
        fs::write(
            &config_path,
            r#"{"projects":{"/my/project":{"allowedTools":["Bash"]}}}"#,
        )
        .unwrap();

        let mcp = serde_json::json!({
            "test-server": { "command": "node", "args": ["server.js"] }
        });

        merge_mcp_into_config(&config_path, &mcp).unwrap();

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert!(config["projects"]["/my/project"]["allowedTools"].is_array());
        assert_eq!(config["mcpServers"]["test-server"]["command"], "node");
    }

    #[test]
    fn test_merge_does_not_overwrite_existing_server() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");
        fs::write(
            &config_path,
            r#"{"mcpServers":{"test-server":{"command":"original","args":[]}}}"#,
        )
        .unwrap();

        let mcp = serde_json::json!({
            "test-server": { "command": "new", "args": ["new.js"] }
        });

        let entries = merge_mcp_into_config(&config_path, &mcp).unwrap();
        assert!(entries.is_empty());

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["test-server"]["command"], "original");
    }

    #[test]
    fn test_merge_multiple_servers() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");

        let mcp = serde_json::json!({
            "server-a": { "command": "node", "args": ["a.js"] },
            "server-b": { "command": "python", "args": ["b.py"] }
        });

        let entries = merge_mcp_into_config(&config_path, &mcp).unwrap();
        assert_eq!(entries.len(), 2);

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert!(config["mcpServers"]["server-a"].is_object());
        assert!(config["mcpServers"]["server-b"].is_object());
    }

    #[test]
    fn test_merge_partial_collision() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");
        fs::write(
            &config_path,
            r#"{"mcpServers":{"existing":{"command":"keep","args":[]}}}"#,
        )
        .unwrap();

        let mcp = serde_json::json!({
            "existing": { "command": "skip", "args": [] },
            "new-server": { "command": "add", "args": [] }
        });

        let entries = merge_mcp_into_config(&config_path, &mcp).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server_name, "new-server");

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["existing"]["command"], "keep");
        assert_eq!(config["mcpServers"]["new-server"]["command"], "add");
    }

    #[test]
    fn test_merge_invalid_mcp_not_object() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");

        let mcp = serde_json::json!("not an object");
        let err = merge_mcp_into_config(&config_path, &mcp).unwrap_err();
        assert!(err.to_string().contains("mcp field must be a JSON object"));
    }

    #[test]
    fn test_remove_specific_servers() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");
        fs::write(
            &config_path,
            r#"{"mcpServers":{"server-a":{"command":"a"},"server-b":{"command":"b"}}}"#,
        )
        .unwrap();

        let entries = vec![DeployedMcpEntry {
            server_name: "server-a".to_string(),
        }];

        remove_mcp_from_config(&config_path, &entries).unwrap();

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert!(config["mcpServers"].get("server-a").is_none());
        assert_eq!(config["mcpServers"]["server-b"]["command"], "b");
    }

    #[test]
    fn test_remove_cleans_empty_mcp_servers_key() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");
        fs::write(
            &config_path,
            r#"{"mcpServers":{"only-server":{"command":"x"}},"other":"keep"}"#,
        )
        .unwrap();

        let entries = vec![DeployedMcpEntry {
            server_name: "only-server".to_string(),
        }];

        remove_mcp_from_config(&config_path, &entries).unwrap();

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert!(config.get("mcpServers").is_none());
        assert_eq!(config["other"], "keep");
    }

    #[test]
    fn test_remove_nonexistent_file_is_noop() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("missing.json");

        let entries = vec![DeployedMcpEntry {
            server_name: "ghost".to_string(),
        }];

        remove_mcp_from_config(&config_path, &entries).unwrap();
    }

    #[test]
    fn test_merge_then_remove_roundtrip() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".claude.json");
        fs::write(&config_path, r#"{"projects":{}}"#).unwrap();

        let mcp = serde_json::json!({
            "test-server": { "command": "node", "args": ["server.js"] }
        });

        let entries = merge_mcp_into_config(&config_path, &mcp).unwrap();
        remove_mcp_from_config(&config_path, &entries).unwrap();

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert!(config.get("mcpServers").is_none());
        assert!(config["projects"].is_object());
    }
}
