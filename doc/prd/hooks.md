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

Deployed hooks are tracked in the install-cache for the active scope — `~/.renkei/install-cache.json` (global) or `~/.renkei/projects/<slug>/install-cache.json` (project), grouped by backend (see install-cache v2 format in [Multi-Backend Configuration](./multi-backend.md)). The backend config files (`settings.json`, `hooks.json`, etc.) stay 100% native with no custom fields. On uninstall, Renkei reads the appropriate install-cache to remove the right entries.

## Scope behavior per backend

Hook deployment scope varies by backend:

- **Claude Code**: hooks always deploy globally to `~/.claude/settings.json`, even in project scope.
- **Cursor**: hooks deploy to `~/.cursor/hooks.json` (global) or `.cursor/hooks.json` (project).
- **Codex**: hooks deploy to `~/.codex/hooks.json` (global) or `.codex/hooks.json` (project).
- **Gemini CLI**: hooks merge into `~/.gemini/settings.json` (global) or `.gemini/settings.json` (project).

See [Backends](./backends.md) for the full deployment conventions table.
