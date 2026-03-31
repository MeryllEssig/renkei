use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{RenkeiError, Result};

// ---------------------------------------------------------------------------
// Renkei hook format (hooks/*.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct RenkeiHook {
    pub event: String,
    #[serde(default)]
    pub matcher: Option<String>,
    pub command: String,
    #[serde(default)]
    pub timeout: Option<u64>,
}

// ---------------------------------------------------------------------------
// Tracking struct (stored in install-cache.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeployedHookEntry {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub command: String,
}

// ---------------------------------------------------------------------------
// Event translation
// ---------------------------------------------------------------------------

pub fn translate_event(renkei_event: &str) -> Option<&'static str> {
    match renkei_event {
        "before_tool" => Some("PreToolUse"),
        "after_tool" => Some("PostToolUse"),
        "after_tool_failure" => Some("PostToolUseFailure"),
        "on_notification" => Some("Notification"),
        "on_session_start" => Some("SessionStart"),
        "on_session_end" => Some("SessionEnd"),
        "on_stop" => Some("Stop"),
        "on_stop_failure" => Some("StopFailure"),
        "on_subagent_start" => Some("SubagentStart"),
        "on_subagent_stop" => Some("SubagentStop"),
        "on_elicitation" => Some("Elicitation"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Hook file parsing
// ---------------------------------------------------------------------------

pub fn parse_hook_file(path: &Path) -> Result<Vec<RenkeiHook>> {
    let content = std::fs::read_to_string(path)?;
    let hooks: Vec<RenkeiHook> = serde_json::from_str(&content).map_err(|e| {
        RenkeiError::InvalidManifest(format!("Invalid hook file {}: {}", path.display(), e))
    })?;

    for hook in &hooks {
        if translate_event(&hook.event).is_none() {
            return Err(RenkeiError::InvalidManifest(format!(
                "Unknown hook event '{}' in {}",
                hook.event,
                path.display()
            )));
        }
    }

    Ok(hooks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // -- translate_event ---------------------------------------------------

    #[test]
    fn test_translate_all_11_events() {
        let mappings = [
            ("before_tool", "PreToolUse"),
            ("after_tool", "PostToolUse"),
            ("after_tool_failure", "PostToolUseFailure"),
            ("on_notification", "Notification"),
            ("on_session_start", "SessionStart"),
            ("on_session_end", "SessionEnd"),
            ("on_stop", "Stop"),
            ("on_stop_failure", "StopFailure"),
            ("on_subagent_start", "SubagentStart"),
            ("on_subagent_stop", "SubagentStop"),
            ("on_elicitation", "Elicitation"),
        ];
        for (renkei, claude) in mappings {
            assert_eq!(
                translate_event(renkei),
                Some(claude),
                "Failed for {renkei}"
            );
        }
    }

    #[test]
    fn test_translate_unknown_event_returns_none() {
        assert_eq!(translate_event("unknown_event"), None);
        assert_eq!(translate_event(""), None);
        assert_eq!(translate_event("PreToolUse"), None);
    }

    // -- parse_hook_file ---------------------------------------------------

    #[test]
    fn test_parse_valid_hook_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lint.json");
        fs::write(
            &path,
            r#"[{"event":"before_tool","matcher":"bash","command":"bash lint.sh","timeout":5}]"#,
        )
        .unwrap();

        let hooks = parse_hook_file(&path).unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, "before_tool");
        assert_eq!(hooks[0].matcher, Some("bash".to_string()));
        assert_eq!(hooks[0].command, "bash lint.sh");
        assert_eq!(hooks[0].timeout, Some(5));
    }

    #[test]
    fn test_parse_hook_file_optional_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("stop.json");
        fs::write(
            &path,
            r#"[{"event":"on_stop","command":"cleanup.sh"}]"#,
        )
        .unwrap();

        let hooks = parse_hook_file(&path).unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].matcher, None);
        assert_eq!(hooks[0].timeout, None);
    }

    #[test]
    fn test_parse_hook_file_multiple_hooks() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("multi.json");
        fs::write(
            &path,
            r#"[
                {"event":"before_tool","matcher":"bash","command":"lint.sh","timeout":5},
                {"event":"on_stop","command":"cleanup.sh"}
            ]"#,
        )
        .unwrap();

        let hooks = parse_hook_file(&path).unwrap();
        assert_eq!(hooks.len(), 2);
    }

    #[test]
    fn test_parse_hook_file_invalid_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "not json at all").unwrap();

        let err = parse_hook_file(&path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Invalid hook file"), "Got: {msg}");
    }

    #[test]
    fn test_parse_hook_file_unknown_event() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad_event.json");
        fs::write(
            &path,
            r#"[{"event":"on_magic","command":"magic.sh"}]"#,
        )
        .unwrap();

        let err = parse_hook_file(&path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Unknown hook event 'on_magic'"), "Got: {msg}");
    }

    #[test]
    fn test_parse_hook_file_not_found() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.json");
        assert!(parse_hook_file(&path).is_err());
    }

    // -- DeployedHookEntry serialization -----------------------------------

    #[test]
    fn test_deployed_hook_entry_serde_with_matcher() {
        let entry = DeployedHookEntry {
            event: "PreToolUse".to_string(),
            matcher: Some("bash".to_string()),
            command: "lint.sh".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"matcher\":\"bash\""));
        let roundtrip: DeployedHookEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, entry);
    }

    #[test]
    fn test_deployed_hook_entry_serde_without_matcher() {
        let entry = DeployedHookEntry {
            event: "Stop".to_string(),
            matcher: None,
            command: "cleanup.sh".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("matcher"));
        let roundtrip: DeployedHookEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, entry);
    }
}
