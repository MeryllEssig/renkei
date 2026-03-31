# Hooks

## Format and events

Files in `hooks/*.json` use an abstract Renkei format with normalized events. Each backend maps these events to its own native events.

```json
[
  {
    "event": "before_tool",
    "matcher": "bash",
    "command": "bash scripts/lint.sh",
    "timeout": 5
  }
]
```

## Renkei → Claude Code event mapping

| Renkei Event | Claude Code |
|-------------|-------------|
| `before_tool` | `PreToolUse` |
| `after_tool` | `PostToolUse` |
| `after_tool_failure` | `PostToolUseFailure` |
| `on_notification` | `Notification` |
| `on_session_start` | `SessionStart` |
| `on_session_end` | `SessionEnd` |
| `on_stop` | `Stop` |
| `on_stop_failure` | `StopFailure` |
| `on_subagent_start` | `SubagentStart` |
| `on_subagent_stop` | `SubagentStop` |
| `on_elicitation` | `Elicitation` |

This mapping is maintained in `ClaudeBackend`. Other backends will define their own mapping.

## Tracking

Deployed hooks are tracked in the install-cache for the active scope — `~/.renkei/install-cache.json` (global) or `~/.renkei/projects/<slug>/install-cache.json` (project). Even though hooks always deploy globally to `~/.claude/settings.json` regardless of scope, they are tracked in the *project* install-cache when installed in project scope. The backend JSON (`settings.json`, etc.) stays 100% native with no custom fields. On uninstall, Renkei reads the appropriate install-cache to remove the right entries.
