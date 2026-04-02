use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{RenkeiError, Result};
use crate::json_file;

const HOOK_TYPE_COMMAND: &str = "command";

// ---------------------------------------------------------------------------
// Data-driven hook profiles
// ---------------------------------------------------------------------------

/// Static mapping from renkei event name → backend-specific event name.
pub type EventTable = &'static [(&'static str, &'static str)];

/// JSON serialization layout for hooks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HookLayout {
    /// Grouped by matcher: `{ "EventName": [{ "matcher": "...", "hooks": [...] }] }`
    Nested,
    /// Flat entries: `{ "eventName": [{ "command": "...", "type": "command", "matcher": "..." }] }`
    Flat,
}

/// Where translated hooks are written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HookTarget {
    /// Merge into an existing settings.json (e.g. Claude, Gemini).
    MergeIntoSettings,
    /// Write to a standalone hooks.json (e.g. Cursor, Codex).
    StandaloneFile,
}

/// Declarative profile for a backend's hook system.
pub struct HookProfile {
    pub events: EventTable,
    pub layout: HookLayout,
    pub target: HookTarget,
}

pub const CLAUDE: HookProfile = HookProfile {
    events: &[
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
    ],
    layout: HookLayout::Nested,
    target: HookTarget::MergeIntoSettings,
};

pub const CURSOR: HookProfile = HookProfile {
    events: &[
        ("before_tool", "preToolUse"),
        ("after_tool", "postToolUse"),
        ("after_tool_failure", "postToolUseFailure"),
        ("on_session_start", "sessionStart"),
        ("on_session_end", "sessionEnd"),
        ("on_stop", "stop"),
        ("user_prompt", "beforeSubmitPrompt"),
    ],
    layout: HookLayout::Flat,
    target: HookTarget::StandaloneFile,
};

pub const CODEX: HookProfile = HookProfile {
    events: &[
        ("before_tool", "PreToolUse"),
        ("after_tool", "PostToolUse"),
        ("on_session_start", "SessionStart"),
        ("on_stop", "Stop"),
        ("user_prompt", "UserPromptSubmit"),
    ],
    layout: HookLayout::Nested,
    target: HookTarget::StandaloneFile,
};

pub const GEMINI: HookProfile = HookProfile {
    events: &[
        ("before_tool", "BeforeTool"),
        ("after_tool", "AfterTool"),
        ("on_session_start", "SessionStart"),
        ("on_session_end", "SessionEnd"),
        ("on_notification", "Notification"),
    ],
    layout: HookLayout::Nested,
    target: HookTarget::MergeIntoSettings,
};

impl HookProfile {
    /// Translate a renkei event name using this profile's event table.
    fn translate_event(&self, renkei_event: &str) -> Option<&'static str> {
        self.events
            .iter()
            .find(|(from, _)| *from == renkei_event)
            .map(|(_, to)| *to)
    }
}

/// Translate renkei hooks to backend-specific JSON using the given profile.
pub fn translate(
    profile: &HookProfile,
    renkei_hooks: &[RenkeiHook],
) -> Result<serde_json::Value> {
    match profile.layout {
        HookLayout::Nested => {
            let map = translate_hooks_with(renkei_hooks, |e| profile.translate_event(e))?;
            Ok(serde_json::to_value(map)?)
        }
        HookLayout::Flat => {
            let map = translate_hooks_cursor_with(renkei_hooks, |e| profile.translate_event(e))?;
            Ok(serde_json::to_value(map)?)
        }
    }
}

/// Deploy hooks using the given profile. Returns deployed entries for tracking.
pub fn deploy(
    profile: &HookProfile,
    hooks: &[RenkeiHook],
    path: &Path,
) -> Result<Vec<DeployedHookEntry>> {
    match (profile.layout, profile.target) {
        (HookLayout::Nested, HookTarget::MergeIntoSettings) => {
            let translated = translate_hooks_with(hooks, |e| profile.translate_event(e))?;
            merge_hooks_into_settings(path, &translated)
        }
        (HookLayout::Nested, HookTarget::StandaloneFile) => {
            let translated = translate_hooks_with(hooks, |e| profile.translate_event(e))?;
            write_standalone_hooks(path, &translated)
        }
        (HookLayout::Flat, HookTarget::StandaloneFile) => {
            let translated =
                translate_hooks_cursor_with(hooks, |e| profile.translate_event(e))?;
            write_cursor_hooks(path, &translated)
        }
        (HookLayout::Flat, HookTarget::MergeIntoSettings) => {
            unreachable!("Flat layout with MergeIntoSettings is not supported")
        }
    }
}

/// Remove previously deployed hooks using the given profile.
pub fn remove(
    profile: &HookProfile,
    path: &Path,
    entries: &[DeployedHookEntry],
) -> Result<()> {
    match profile.layout {
        HookLayout::Nested => remove_hooks_from_settings(path, entries),
        HookLayout::Flat => remove_cursor_hooks(path, entries),
    }
}

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

fn translate_event(renkei_event: &str) -> Option<&'static str> {
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

fn translate_event_cursor(renkei_event: &str) -> Option<&'static str> {
    match renkei_event {
        "before_tool" => Some("preToolUse"),
        "after_tool" => Some("postToolUse"),
        "after_tool_failure" => Some("postToolUseFailure"),
        "on_session_start" => Some("sessionStart"),
        "on_session_end" => Some("sessionEnd"),
        "on_stop" => Some("stop"),
        "user_prompt" => Some("beforeSubmitPrompt"),
        _ => None,
    }
}

fn translate_event_codex(renkei_event: &str) -> Option<&'static str> {
    match renkei_event {
        "before_tool" => Some("PreToolUse"),
        "after_tool" => Some("PostToolUse"),
        "on_session_start" => Some("SessionStart"),
        "on_stop" => Some("Stop"),
        "user_prompt" => Some("UserPromptSubmit"),
        _ => None,
    }
}

fn translate_event_gemini(renkei_event: &str) -> Option<&'static str> {
    match renkei_event {
        "before_tool" => Some("BeforeTool"),
        "after_tool" => Some("AfterTool"),
        "on_session_start" => Some("SessionStart"),
        "on_session_end" => Some("SessionEnd"),
        "on_notification" => Some("Notification"),
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
struct ClaudeHookEntry {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ClaudeHookGroup {
    #[serde(skip_serializing_if = "Option::is_none")]
    matcher: Option<String>,
    hooks: Vec<ClaudeHookEntry>,
}

/// Translate Renkei hooks to the nested group format (Claude/Codex/Gemini), using the
/// provided event translation function.
fn translate_hooks_with<F>(
    renkei_hooks: &[RenkeiHook],
    translate_fn: F,
) -> Result<BTreeMap<String, Vec<ClaudeHookGroup>>>
where
    F: Fn(&str) -> Option<&'static str>,
{
    let mut result: BTreeMap<String, Vec<ClaudeHookGroup>> = BTreeMap::new();

    for hook in renkei_hooks {
        let translated_event = translate_fn(&hook.event)
            .ok_or_else(|| {
                RenkeiError::InvalidManifest(format!("Unknown hook event '{}'", hook.event))
            })?
            .to_string();

        let entry = ClaudeHookEntry {
            hook_type: HOOK_TYPE_COMMAND.to_string(),
            command: hook.command.clone(),
            timeout: hook.timeout,
        };

        let groups = result.entry(translated_event).or_default();

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

fn translate_hooks(
    renkei_hooks: &[RenkeiHook],
) -> Result<BTreeMap<String, Vec<ClaudeHookGroup>>> {
    translate_hooks_with(renkei_hooks, translate_event)
}

/// Cursor hook entry — flat format (no nested `hooks` array).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CursorHookEntry {
    command: String,
    #[serde(rename = "type")]
    hook_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    matcher: Option<String>,
}

/// Translate Renkei hooks to flat-entry format using the provided translation function.
fn translate_hooks_cursor_with<F>(
    renkei_hooks: &[RenkeiHook],
    translate_fn: F,
) -> Result<BTreeMap<String, Vec<CursorHookEntry>>>
where
    F: Fn(&str) -> Option<&'static str>,
{
    let mut result: BTreeMap<String, Vec<CursorHookEntry>> = BTreeMap::new();

    for hook in renkei_hooks {
        let event = translate_fn(&hook.event)
            .ok_or_else(|| {
                RenkeiError::InvalidManifest(format!("Unknown hook event '{}'", hook.event))
            })?
            .to_string();

        result.entry(event).or_default().push(CursorHookEntry {
            command: hook.command.clone(),
            hook_type: HOOK_TYPE_COMMAND.to_string(),
            timeout: hook.timeout,
            matcher: hook.matcher.clone(),
        });
    }

    Ok(result)
}

/// Translate Renkei hooks to Cursor's flat-entry format.
fn translate_hooks_cursor(
    renkei_hooks: &[RenkeiHook],
) -> Result<BTreeMap<String, Vec<CursorHookEntry>>> {
    translate_hooks_cursor_with(renkei_hooks, translate_event_cursor)
}

/// Write/merge translated cursor hooks into a standalone `hooks.json` file.
/// Returns deployed entries for tracking.
fn write_cursor_hooks(
    hooks_path: &Path,
    translated: &BTreeMap<String, Vec<CursorHookEntry>>,
) -> Result<Vec<DeployedHookEntry>> {
    let mut file: serde_json::Value = json_file::read_json_or_empty(hooks_path)?;

    let obj = file.as_object_mut().ok_or_else(|| {
        RenkeiError::DeploymentFailed("cursor hooks.json is not a JSON object".into())
    })?;

    // Ensure version field
    obj.entry("version").or_insert(serde_json::json!(1));

    let hooks_obj = obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_map = hooks_obj.as_object_mut().ok_or_else(|| {
        RenkeiError::DeploymentFailed("cursor hooks.json 'hooks' is not a JSON object".into())
    })?;

    let mut deployed = Vec::new();

    for (event, entries) in translated {
        let event_array = hooks_map
            .entry(event)
            .or_insert_with(|| serde_json::json!([]));
        let arr = event_array.as_array_mut().ok_or_else(|| {
            RenkeiError::DeploymentFailed(format!("cursor hooks.json hooks.{event} is not an array"))
        })?;

        for entry in entries {
            arr.push(serde_json::to_value(entry)?);
            deployed.push(DeployedHookEntry {
                event: event.clone(),
                matcher: entry.matcher.clone(),
                command: entry.command.clone(),
            });
        }
    }

    json_file::write_json_pretty(hooks_path, &file)?;
    Ok(deployed)
}

/// Remove cursor hook entries from a standalone `hooks.json` file.
fn remove_cursor_hooks(hooks_path: &Path, entries_to_remove: &[DeployedHookEntry]) -> Result<()> {
    let mut file: serde_json::Value = match std::fs::read_to_string(hooks_path) {
        Ok(content) => serde_json::from_str(&content)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    let hooks_map = match file
        .as_object_mut()
        .and_then(|o| o.get_mut("hooks"))
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
            event_array.retain(|item| {
                let cmd_match =
                    item.get("command").and_then(|c| c.as_str()) != Some(&entry.command);
                let matcher_match = item
                    .get("matcher")
                    .and_then(|m| m.as_str())
                    .map(String::from)
                    != entry.matcher;
                // Keep if either command or matcher differs
                cmd_match || matcher_match
            });
        }
    }

    hooks_map.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));

    json_file::write_json_pretty(hooks_path, &file)?;
    Ok(())
}

/// Write/merge translated hooks (nested format) to a standalone `hooks.json` file.
/// Used by Codex backend. Returns deployed entries for tracking.
fn write_standalone_hooks(
    hooks_path: &Path,
    translated: &BTreeMap<String, Vec<ClaudeHookGroup>>,
) -> Result<Vec<DeployedHookEntry>> {
    let mut file: serde_json::Value = json_file::read_json_or_empty(hooks_path)?;

    let obj = file.as_object_mut().ok_or_else(|| {
        RenkeiError::DeploymentFailed("hooks.json is not a JSON object".into())
    })?;

    let hooks_obj = obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_map = hooks_obj.as_object_mut().ok_or_else(|| {
        RenkeiError::DeploymentFailed("hooks.json 'hooks' is not a JSON object".into())
    })?;

    let mut deployed = Vec::new();

    for (event, groups) in translated {
        let event_array = hooks_map
            .entry(event)
            .or_insert_with(|| serde_json::json!([]));
        let arr = event_array.as_array_mut().ok_or_else(|| {
            RenkeiError::DeploymentFailed(format!("hooks.json hooks.{event} is not an array"))
        })?;

        for group in groups {
            arr.push(serde_json::to_value(group)?);
            for hook_entry in &group.hooks {
                deployed.push(DeployedHookEntry {
                    event: event.clone(),
                    matcher: group.matcher.clone(),
                    command: hook_entry.command.clone(),
                });
            }
        }
    }

    json_file::write_json_pretty(hooks_path, &file)?;
    Ok(deployed)
}

/// Remove hook entries from a standalone `hooks.json` file (Codex format).
fn remove_standalone_hooks(
    hooks_path: &Path,
    entries_to_remove: &[DeployedHookEntry],
) -> Result<()> {
    remove_hooks_from_settings(hooks_path, entries_to_remove)
}

fn read_settings(path: &Path) -> Result<serde_json::Value> {
    json_file::read_json_or_empty(path)
}

fn write_settings(path: &Path, value: &serde_json::Value) -> Result<()> {
    json_file::write_json_pretty(path, value)
}

fn merge_hooks_into_settings(
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

fn remove_hooks_from_settings(
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

    // -- Profile-based translate() tests --

    #[test]
    fn test_profile_translate_claude() {
        let hooks = vec![
            make_renkei_hook("before_tool", Some("bash"), "lint.sh", Some(5)),
            make_renkei_hook("on_stop", None, "cleanup.sh", None),
        ];
        let result = translate(&CLAUDE, &hooks).unwrap();
        // Nested format: { "EventName": [{ "matcher": ..., "hooks": [...] }] }
        let pre = &result["PreToolUse"];
        assert!(pre.is_array());
        assert_eq!(pre[0]["matcher"], "bash");
        assert_eq!(pre[0]["hooks"][0]["command"], "lint.sh");
        assert_eq!(pre[0]["hooks"][0]["timeout"], 5);

        let stop = &result["Stop"];
        assert!(stop.is_array());
        assert!(stop[0].get("matcher").is_none());
        assert_eq!(stop[0]["hooks"][0]["command"], "cleanup.sh");
    }

    #[test]
    fn test_profile_translate_cursor() {
        let hooks = vec![make_renkei_hook(
            "before_tool",
            Some("bash"),
            "lint.sh",
            Some(5),
        )];
        let result = translate(&CURSOR, &hooks).unwrap();
        // Flat format: { "eventName": [{ "command": ..., "type": ..., "matcher": ... }] }
        let pre = &result["preToolUse"];
        assert!(pre.is_array());
        assert_eq!(pre[0]["command"], "lint.sh");
        assert_eq!(pre[0]["type"], "command");
        assert_eq!(pre[0]["matcher"], "bash");
        assert_eq!(pre[0]["timeout"], 5);
    }

    #[test]
    fn test_profile_translate_codex() {
        let hooks = vec![make_renkei_hook("user_prompt", None, "check.sh", None)];
        let result = translate(&CODEX, &hooks).unwrap();
        // Codex uses nested format with its own event names
        assert!(result["UserPromptSubmit"].is_array());
        assert_eq!(result["UserPromptSubmit"][0]["hooks"][0]["command"], "check.sh");
    }

    #[test]
    fn test_profile_translate_gemini() {
        let hooks = vec![make_renkei_hook(
            "before_tool",
            Some("write_file"),
            "check.sh",
            None,
        )];
        let result = translate(&GEMINI, &hooks).unwrap();
        assert!(result["BeforeTool"].is_array());
        assert_eq!(result["BeforeTool"][0]["matcher"], "write_file");
    }

    #[test]
    fn test_profile_translate_unknown_event_fails() {
        let hooks = vec![make_renkei_hook("on_magic", None, "x.sh", None)];
        assert!(translate(&CLAUDE, &hooks).is_err());
        assert!(translate(&CURSOR, &hooks).is_err());
    }

    // -- Profile-based deploy()+remove() roundtrip tests --

    #[test]
    fn test_profile_deploy_remove_claude() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"language":"French"}"#).unwrap();

        let hooks = vec![make_renkei_hook("before_tool", Some("bash"), "lint.sh", Some(5))];
        let entries = deploy(&CLAUDE, &hooks, &path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event, "PreToolUse");

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(settings["language"], "French");
        assert!(settings["hooks"]["PreToolUse"].is_array());

        remove(&CLAUDE, &path, &entries).unwrap();
        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(settings["language"], "French");
        assert!(settings.get("hooks").is_none());
    }

    #[test]
    fn test_profile_deploy_remove_cursor() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hooks.json");

        let hooks = vec![make_renkei_hook("before_tool", Some("bash"), "lint.sh", Some(5))];
        let entries = deploy(&CURSOR, &hooks, &path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event, "preToolUse");

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["version"], 1);
        assert_eq!(content["hooks"]["preToolUse"][0]["command"], "lint.sh");

        remove(&CURSOR, &path, &entries).unwrap();
        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(content["hooks"]
            .as_object()
            .map_or(true, |m| m.is_empty()));
    }

    #[test]
    fn test_profile_deploy_remove_codex() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hooks.json");

        let hooks = vec![make_renkei_hook("before_tool", Some("bash"), "lint.sh", None)];
        let entries = deploy(&CODEX, &hooks, &path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event, "PreToolUse");

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(content["hooks"]["PreToolUse"].is_array());

        remove(&CODEX, &path, &entries).unwrap();
        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(content.get("hooks").is_none());
    }

    #[test]
    fn test_profile_deploy_remove_gemini() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"theme":"dark"}"#).unwrap();

        let hooks = vec![make_renkei_hook("before_tool", Some("write_file"), "check.sh", None)];
        let entries = deploy(&GEMINI, &hooks, &path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event, "BeforeTool");

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(settings["theme"], "dark");
        assert!(settings["hooks"]["BeforeTool"].is_array());

        remove(&GEMINI, &path, &entries).unwrap();
        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(settings["theme"], "dark");
        assert!(settings.get("hooks").is_none());
    }
}
