# Backends

## Backend interface

- A `Backend` trait defines operations: `name`, `detect_installed`, `deploy_skill`, `deploy_hook`, `deploy_agent`, `register_mcp`.
- **Everything backend-specific must be abstracted** behind this interface.
- `ClaudeBackend` is the only implementation in v1. `CursorBackend` will be added in v2 without refactoring.
- **Detection**: a backend is considered installed if its config directory exists (`~/.claude/` for Claude, `.cursor/` for Cursor). No binary check in PATH.

## Multi-backend support matrix

| Artifact   | Claude Code              | Cursor               | Codex      | Gemini |
|------------|--------------------------|----------------------|------------|--------|
| Skills     | `SKILL.md`               | Skills               | `AGENTS.md`| ?      |
| Hooks      | `settings.json` events   | N/A                  | N/A        | N/A    |
| Agents     | `agents/*.md`            | N/A                  | N/A        | N/A    |
| MCP config | `~/.claude.json`         | `.cursor/mcp.json`   | ?          | ?      |

Codex and Gemini are on the radar but not planned. Artifact format varies by backend (`AGENTS.md` for Codex vs `agents/*.md` for Claude).

## Deployment conventions (hardcoded, not configurable)

| Artifact  | Claude Code (global)                      | Claude Code (project)                              | Cursor                       |
|-----------|-------------------------------------------|-----------------------------------------------------|------------------------------|
| Skills    | `~/.claude/skills/renkei-<name>/SKILL.md` | `<project>/.claude/skills/renkei-<name>/SKILL.md`  | `.cursor/skills/<name>/`     |
| Hooks     | Merge into `~/.claude/settings.json`      | Merge into `~/.claude/settings.json` (always global)| N/A                          |
| Agents    | `~/.claude/agents/<name>.md`              | `<project>/.claude/agents/<name>.md`               | N/A                          |
| MCP config| Merge into `~/.claude.json`               | Merge into `~/.claude.json` (always global)        | Merge into `.cursor/mcp.json` |

In project scope, skills and agents deploy to the equivalent paths under `<project_root>/.claude/` instead of `~/.claude/`. Hooks and MCP always deploy to the global paths regardless of scope. The `renkei-` prefix on skills creates a clear namespace and avoids collisions with native skills.
