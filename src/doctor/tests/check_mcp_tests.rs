use crate::doctor::checks;
use crate::doctor::types::DiagnosticKind;

use super::{make_entry, set_mcp_servers};

#[test]
fn test_mcp_present() {
    let config = serde_json::json!({
        "mcpServers": {
            "test-server": {"command": "node", "args": ["server.js"]}
        }
    });
    let mut entry = make_entry(vec![]);
    set_mcp_servers(&mut entry, vec!["test-server".to_string()]);
    assert!(checks::check_mcp(&entry, &config).is_empty());
}

#[test]
fn test_mcp_missing() {
    let config = serde_json::json!({});
    let mut entry = make_entry(vec![]);
    set_mcp_servers(&mut entry, vec!["test-server".to_string()]);
    let issues = checks::check_mcp(&entry, &config);
    assert_eq!(issues.len(), 1);
    assert!(
        matches!(&issues[0], DiagnosticKind::McpMissing { server_name } if server_name == "test-server")
    );
}

#[test]
fn test_mcp_no_mcp_servers_key() {
    let config = serde_json::json!({"projects": {}});
    let mut entry = make_entry(vec![]);
    set_mcp_servers(&mut entry, vec!["srv".to_string()]);
    let issues = checks::check_mcp(&entry, &config);
    assert_eq!(issues.len(), 1);
}

#[test]
fn test_mcp_partial_match() {
    let config = serde_json::json!({
        "mcpServers": {
            "server-a": {"command": "a"}
        }
    });
    let mut entry = make_entry(vec![]);
    set_mcp_servers(
        &mut entry,
        vec!["server-a".to_string(), "server-b".to_string()],
    );
    let issues = checks::check_mcp(&entry, &config);
    assert_eq!(issues.len(), 1);
    assert!(
        matches!(&issues[0], DiagnosticKind::McpMissing { server_name } if server_name == "server-b")
    );
}

#[test]
fn test_mcp_no_servers_deployed() {
    let config = serde_json::json!({});
    let entry = make_entry(vec![]);
    assert!(checks::check_mcp(&entry, &config).is_empty());
}
