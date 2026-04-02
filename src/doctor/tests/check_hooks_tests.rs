use crate::doctor::checks;
use crate::doctor::types::DiagnosticKind;

use super::{make_entry, make_hook_entry};

#[test]
fn test_hooks_present() {
    let settings = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "bash",
                "hooks": [{"type": "command", "command": "lint.sh"}]
            }]
        }
    });
    let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
    assert!(checks::check_hooks(&entry, &settings).is_empty());
}

#[test]
fn test_hooks_missing() {
    let settings = serde_json::json!({});
    let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
    let issues = checks::check_hooks(&entry, &settings);
    assert_eq!(issues.len(), 1);
    assert!(
        matches!(&issues[0], DiagnosticKind::HookMissing { event, command } if event == "PreToolUse" && command == "lint.sh")
    );
}

#[test]
fn test_hooks_without_matcher() {
    let settings = serde_json::json!({
        "hooks": {
            "Stop": [{
                "hooks": [{"type": "command", "command": "cleanup.sh"}]
            }]
        }
    });
    let entry = make_entry(vec![make_hook_entry("Stop", None, "cleanup.sh")]);
    assert!(checks::check_hooks(&entry, &settings).is_empty());
}

#[test]
fn test_hooks_wrong_command() {
    let settings = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "bash",
                "hooks": [{"type": "command", "command": "other.sh"}]
            }]
        }
    });
    let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
    let issues = checks::check_hooks(&entry, &settings);
    assert_eq!(issues.len(), 1);
}

#[test]
fn test_hooks_wrong_matcher() {
    let settings = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "Write",
                "hooks": [{"type": "command", "command": "lint.sh"}]
            }]
        }
    });
    let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
    let issues = checks::check_hooks(&entry, &settings);
    assert_eq!(issues.len(), 1);
}

#[test]
fn test_hooks_no_hooks_key() {
    let settings = serde_json::json!({"language": "French"});
    let entry = make_entry(vec![make_hook_entry("PreToolUse", Some("bash"), "lint.sh")]);
    let issues = checks::check_hooks(&entry, &settings);
    assert_eq!(issues.len(), 1);
}
