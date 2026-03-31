use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{RenkeiError, Result};
use crate::json_file;

const HOOK_TYPE_COMMAND: &str = "command";

#[derive(Debug, Clone, Deserialize)]
pub struct RenkeiHook {
    pub event: String,
    #[serde(default)]
    pub matcher: Option<String>,
    pub command: String,
    #[serde(default)]
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeployedHookEntry {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub command: String,
}

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

pub fn parse_hook_file(path: &Path) -> Result<Vec<RenkeiHook>> {
    let content = std::fs::read_to_string(path)?;
    let hooks: Vec<RenkeiHook> = serde_json::from_str(&content).map_err(|e| {
        RenkeiError::InvalidManifest(format!("Invalid hook file {}: {}", path.display(), e))
    })?;
    Ok(hooks)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeHookEntry {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeHookGroup {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hooks: Vec<ClaudeHookEntry>,
}

pub fn translate_hooks(
    renkei_hooks: &[RenkeiHook],
) -> Result<BTreeMap<String, Vec<ClaudeHookGroup>>> {
    let mut result: BTreeMap<String, Vec<ClaudeHookGroup>> = BTreeMap::new();

    for hook in renkei_hooks {
        let claude_event = translate_event(&hook.event)
            .ok_or_else(|| {
                RenkeiError::InvalidManifest(format!("Unknown hook event '{}'", hook.event))
            })?
            .to_string();

        let entry = ClaudeHookEntry {
            hook_type: HOOK_TYPE_COMMAND.to_string(),
            command: hook.command.clone(),
            timeout: hook.timeout,
        };

        let groups = result.entry(claude_event).or_default();

        let existing = groups.iter_mut().find(|g| g.matcher == hook.matcher);
        match existing {
            Some(group) => group.hooks.push(entry),
            None => groups.push(ClaudeHookGroup {
                matcher: hook.matcher.clone(),
                hooks: vec![entry],
            }),
        }
    }

    Ok(result)
}

fn read_settings(path: &Path) -> Result<serde_json::Value> {
    json_file::read_json_or_empty(path)
}

fn write_settings(path: &Path, value: &serde_json::Value) -> Result<()> {
    json_file::write_json_pretty(path, value)
}

pub fn merge_hooks_into_settings(
    settings_path: &Path,
    translated: &BTreeMap<String, Vec<ClaudeHookGroup>>,
) -> Result<Vec<DeployedHookEntry>> {
    let mut settings = read_settings(settings_path)?;

    let settings_obj = settings.as_object_mut().ok_or_else(|| {
        RenkeiError::DeploymentFailed("settings.json is not a JSON object".into())
    })?;

    let hooks_obj = settings_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_map = hooks_obj.as_object_mut().ok_or_else(|| {
        RenkeiError::DeploymentFailed("settings.json 'hooks' is not a JSON object".into())
    })?;

    let mut deployed_entries = Vec::new();

    for (event, groups) in translated {
        let event_array = hooks_map
            .entry(event)
            .or_insert_with(|| serde_json::json!([]));
        let arr = event_array.as_array_mut().ok_or_else(|| {
            RenkeiError::DeploymentFailed(format!("settings.json hooks.{event} is not an array"))
        })?;

        for group in groups {
            arr.push(serde_json::to_value(group)?);

            for hook_entry in &group.hooks {
                deployed_entries.push(DeployedHookEntry {
                    event: event.clone(),
                    matcher: group.matcher.clone(),
                    command: hook_entry.command.clone(),
                });
            }
        }
    }

    write_settings(settings_path, &settings)?;
    Ok(deployed_entries)
}

pub fn remove_hooks_from_settings(
    settings_path: &Path,
    entries_to_remove: &[DeployedHookEntry],
) -> Result<()> {
    let mut settings: serde_json::Value = match std::fs::read_to_string(settings_path) {
        Ok(content) => serde_json::from_str(&content)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    let hooks_map = match settings
        .as_object_mut()
        .and_then(|s| s.get_mut("hooks"))
        .and_then(|h| h.as_object_mut())
    {
        Some(h) => h,
        None => return Ok(()),
    };

    for entry in entries_to_remove {
        if let Some(event_array) = hooks_map
            .get_mut(&entry.event)
            .and_then(|v| v.as_array_mut())
        {
            event_array.retain(|group| {
                let group_matcher = group
                    .get("matcher")
                    .and_then(|m| m.as_str())
                    .map(String::from);
                if group_matcher != entry.matcher {
                    return true;
                }
                let has_match =
                    group
                        .get("hooks")
                        .and_then(|h| h.as_array())
                        .is_some_and(|hooks| {
                            hooks.iter().any(|h| {
                                h.get("command").and_then(|c| c.as_str()) == Some(&entry.command)
                            })
                        });
                !has_match
            });
        }
    }

    hooks_map.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));

    if hooks_map.is_empty() {
        settings.as_object_mut().unwrap().remove("hooks");
    }

    write_settings(settings_path, &settings)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

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
            assert_eq!(translate_event(renkei), Some(claude), "Failed for {renkei}");
        }
    }

    #[test]
    fn test_translate_unknown_event_returns_none() {
        assert_eq!(translate_event("unknown_event"), None);
        assert_eq!(translate_event(""), None);
        assert_eq!(translate_event("PreToolUse"), None);
    }

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
        fs::write(&path, r#"[{"event":"on_stop","command":"cleanup.sh"}]"#).unwrap();

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
    fn test_parse_hook_file_not_found() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.json");
        assert!(parse_hook_file(&path).is_err());
    }

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

    fn make_renkei_hook(
        event: &str,
        matcher: Option<&str>,
        cmd: &str,
        timeout: Option<u64>,
    ) -> RenkeiHook {
        RenkeiHook {
            event: event.to_string(),
            matcher: matcher.map(String::from),
            command: cmd.to_string(),
            timeout,
        }
    }

    #[test]
    fn test_translate_hooks_single() {
        let hooks = vec![make_renkei_hook(
            "before_tool",
            Some("bash"),
            "lint.sh",
            Some(5),
        )];
        let result = translate_hooks(&hooks).unwrap();

        assert_eq!(result.len(), 1);
        let groups = &result["PreToolUse"];
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].matcher, Some("bash".to_string()));
        assert_eq!(groups[0].hooks.len(), 1);
        assert_eq!(groups[0].hooks[0].hook_type, "command");
        assert_eq!(groups[0].hooks[0].command, "lint.sh");
        assert_eq!(groups[0].hooks[0].timeout, Some(5));
    }

    #[test]
    fn test_translate_hooks_same_event_different_matcher() {
        let hooks = vec![
            make_renkei_hook("before_tool", Some("bash"), "lint.sh", None),
            make_renkei_hook("before_tool", Some("Write"), "check.sh", None),
        ];
        let result = translate_hooks(&hooks).unwrap();

        let groups = &result["PreToolUse"];
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].matcher, Some("bash".to_string()));
        assert_eq!(groups[1].matcher, Some("Write".to_string()));
    }

    #[test]
    fn test_translate_hooks_same_event_same_matcher_grouped() {
        let hooks = vec![
            make_renkei_hook("before_tool", Some("bash"), "lint.sh", Some(5)),
            make_renkei_hook("before_tool", Some("bash"), "format.sh", Some(3)),
        ];
        let result = translate_hooks(&hooks).unwrap();

        let groups = &result["PreToolUse"];
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].hooks.len(), 2);
        assert_eq!(groups[0].hooks[0].command, "lint.sh");
        assert_eq!(groups[0].hooks[1].command, "format.sh");
    }

    #[test]
    fn test_translate_hooks_multiple_events() {
        let hooks = vec![
            make_renkei_hook("before_tool", Some("bash"), "lint.sh", None),
            make_renkei_hook("on_stop", None, "cleanup.sh", None),
        ];
        let result = translate_hooks(&hooks).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains_key("PreToolUse"));
        assert!(result.contains_key("Stop"));
    }

    #[test]
    fn test_translate_hooks_no_matcher() {
        let hooks = vec![make_renkei_hook("on_stop", None, "cleanup.sh", None)];
        let result = translate_hooks(&hooks).unwrap();

        let groups = &result["Stop"];
        assert_eq!(groups[0].matcher, None);
    }

    #[test]
    fn test_translate_hooks_unknown_event_fails() {
        let hooks = vec![make_renkei_hook("on_magic", None, "magic.sh", None)];
        let err = translate_hooks(&hooks).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Unknown hook event 'on_magic'"), "Got: {msg}");
    }

    #[test]
    fn test_merge_into_empty_settings() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        let mut translated = BTreeMap::new();
        translated.insert(
            "PreToolUse".to_string(),
            vec![ClaudeHookGroup {
                matcher: Some("bash".to_string()),
                hooks: vec![ClaudeHookEntry {
                    hook_type: "command".to_string(),
                    command: "lint.sh".to_string(),
                    timeout: Some(5),
                }],
            }],
        );

        let entries = merge_hooks_into_settings(&settings_path, &translated).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event, "PreToolUse");
        assert_eq!(entries[0].command, "lint.sh");

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(settings["hooks"]["PreToolUse"].is_array());
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "bash");
    }

    #[test]
    fn test_merge_into_nonexistent_creates_file() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join(".claude").join("settings.json");

        let mut translated = BTreeMap::new();
        translated.insert(
            "Stop".to_string(),
            vec![ClaudeHookGroup {
                matcher: None,
                hooks: vec![ClaudeHookEntry {
                    hook_type: "command".to_string(),
                    command: "cleanup.sh".to_string(),
                    timeout: None,
                }],
            }],
        );

        merge_hooks_into_settings(&settings_path, &translated).unwrap();
        assert!(settings_path.exists());

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        let stop = &settings["hooks"]["Stop"][0];
        assert!(stop.get("matcher").is_none());
        assert_eq!(stop["hooks"][0]["command"], "cleanup.sh");
    }

    #[test]
    fn test_merge_preserves_existing_settings_keys() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{"permissions":{"allow":["Bash"]},"language":"French"}"#,
        )
        .unwrap();

        let mut translated = BTreeMap::new();
        translated.insert(
            "PreToolUse".to_string(),
            vec![ClaudeHookGroup {
                matcher: Some("bash".to_string()),
                hooks: vec![ClaudeHookEntry {
                    hook_type: "command".to_string(),
                    command: "lint.sh".to_string(),
                    timeout: None,
                }],
            }],
        );

        merge_hooks_into_settings(&settings_path, &translated).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(settings["language"], "French");
        assert!(settings["permissions"]["allow"].is_array());
        assert!(settings["hooks"]["PreToolUse"].is_array());
    }

    #[test]
    fn test_merge_appends_to_existing_event_array() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{"hooks":{"PreToolUse":[{"matcher":"Write","hooks":[{"type":"command","command":"existing.sh"}]}]}}"#,
        )
        .unwrap();

        let mut translated = BTreeMap::new();
        translated.insert(
            "PreToolUse".to_string(),
            vec![ClaudeHookGroup {
                matcher: Some("bash".to_string()),
                hooks: vec![ClaudeHookEntry {
                    hook_type: "command".to_string(),
                    command: "lint.sh".to_string(),
                    timeout: None,
                }],
            }],
        );

        merge_hooks_into_settings(&settings_path, &translated).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        let pre_tool = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool.len(), 2);
        assert_eq!(pre_tool[0]["matcher"], "Write");
        assert_eq!(pre_tool[1]["matcher"], "bash");
    }

    #[test]
    fn test_remove_specific_hooks() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{"hooks":{"PreToolUse":[{"matcher":"bash","hooks":[{"type":"command","command":"lint.sh"}]}]}}"#,
        )
        .unwrap();

        let entries = vec![DeployedHookEntry {
            event: "PreToolUse".to_string(),
            matcher: Some("bash".to_string()),
            command: "lint.sh".to_string(),
        }];

        remove_hooks_from_settings(&settings_path, &entries).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(settings.get("hooks").is_none());
    }

    #[test]
    fn test_remove_leaves_other_events() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{"hooks":{"PreToolUse":[{"matcher":"bash","hooks":[{"type":"command","command":"lint.sh"}]}],"Stop":[{"hooks":[{"type":"command","command":"cleanup.sh"}]}]}}"#,
        )
        .unwrap();

        let entries = vec![DeployedHookEntry {
            event: "PreToolUse".to_string(),
            matcher: Some("bash".to_string()),
            command: "lint.sh".to_string(),
        }];

        remove_hooks_from_settings(&settings_path, &entries).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(settings["hooks"].get("PreToolUse").is_none());
        assert!(settings["hooks"]["Stop"].is_array());
    }

    #[test]
    fn test_remove_leaves_other_groups_in_same_event() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{"hooks":{"PreToolUse":[{"matcher":"Write","hooks":[{"type":"command","command":"existing.sh"}]},{"matcher":"bash","hooks":[{"type":"command","command":"lint.sh"}]}]}}"#,
        )
        .unwrap();

        let entries = vec![DeployedHookEntry {
            event: "PreToolUse".to_string(),
            matcher: Some("bash".to_string()),
            command: "lint.sh".to_string(),
        }];

        remove_hooks_from_settings(&settings_path, &entries).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        let pre_tool = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool.len(), 1);
        assert_eq!(pre_tool[0]["matcher"], "Write");
    }

    #[test]
    fn test_remove_nonexistent_file_is_noop() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("missing.json");
        let entries = vec![DeployedHookEntry {
            event: "Stop".to_string(),
            matcher: None,
            command: "cleanup.sh".to_string(),
        }];
        remove_hooks_from_settings(&settings_path, &entries).unwrap();
    }

    #[test]
    fn test_merge_then_remove_roundtrip() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(&settings_path, r#"{"language":"French"}"#).unwrap();

        let mut translated = BTreeMap::new();
        translated.insert(
            "PreToolUse".to_string(),
            vec![ClaudeHookGroup {
                matcher: Some("bash".to_string()),
                hooks: vec![ClaudeHookEntry {
                    hook_type: "command".to_string(),
                    command: "lint.sh".to_string(),
                    timeout: Some(5),
                }],
            }],
        );

        let entries = merge_hooks_into_settings(&settings_path, &translated).unwrap();
        remove_hooks_from_settings(&settings_path, &entries).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(settings["language"], "French");
        assert!(settings.get("hooks").is_none());
    }

    #[test]
    fn test_remove_hook_without_matcher() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"cleanup.sh"}]}]}}"#,
        )
        .unwrap();

        let entries = vec![DeployedHookEntry {
            event: "Stop".to_string(),
            matcher: None,
            command: "cleanup.sh".to_string(),
        }];

        remove_hooks_from_settings(&settings_path, &entries).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(settings.get("hooks").is_none());
    }
}
