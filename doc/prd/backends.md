# Backends

## Backend interface

- A `Backend` trait defines operations: `name`, `detect_installed`, `deploy_skill`, `deploy_hook`, `deploy_agent`, `register_mcp`.
- **Everything backend-specific must be abstracted** behind this interface.
- `ClaudeBackend` is the only implementation in v1. `CursorBackend` will be added in v2 without refactoring.
- **Detection**: a backend is considered installed if its config directory exists (`~/.claude/` for Claude, `.cursor/` for Cursor). No binary check in PATH.

## Multi-backend support matrix

| Artifact   | Claude Code                | Cursor                   | Codex                    | Gemini CLI               | Agents (shared) |
|------------|----------------------------|--------------------------|--------------------------|--------------------------|-----------------|
| Skills     | `SKILL.md` (Markdown)      | `.mdc` (MD + frontmatter)| `SKILL.md` (MD + frontmatter) | `SKILL.md` (MD + frontmatter) | `SKILL.md`      |
| Hooks      | In `settings.json`         | `hooks.json` (standalone)| `hooks.json` (standalone)| In `settings.json`       | Not supported   |
| Agents     | `agents/*.md` (Markdown)   | `agents/*.md` (MD + frontmatter) | `agents/*.toml` (TOML) | `agents/*.md` (MD + frontmatter) | Not supported |
| MCP config | `~/.claude.json` (JSON)    | `mcp.json` (JSON)        | In `config.toml` (TOML)  | In `settings.json` (JSON)| Not supported   |

See [Multi-Backend Configuration](./multi-backend.md) for user-facing backend selection, the `agents` shared standard, and the deployment deduplication strategy.

## Deployment conventions (hardcoded, not configurable)

| Artifact   | Claude Code (global)                      | Claude Code (project)                              | Cursor (global)                        | Cursor (project)                      |
|------------|-------------------------------------------|-----------------------------------------------------|----------------------------------------|---------------------------------------|
| Skills     | `~/.claude/skills/renkei-<name>/SKILL.md` | `<project>/.claude/skills/renkei-<name>/SKILL.md`  | `~/.cursor/rules/renkei-<name>.mdc`    | `.cursor/rules/renkei-<name>.mdc`     |
| Hooks      | Merge into `~/.claude/settings.json`      | Merge into `~/.claude/settings.json` (always global)| `~/.cursor/hooks.json`                 | `.cursor/hooks.json`                  |
| Agents     | `~/.claude/agents/<name>.md`              | `<project>/.claude/agents/<name>.md`               | `~/.cursor/agents/<name>.md`           | `.cursor/agents/<name>.md`            |
| MCP config | Merge into `~/.claude.json`               | Merge into `~/.claude.json` (always global)        | `~/.cursor/mcp.json`                   | `.cursor/mcp.json`                    |

| Artifact   | Codex (global)                             | Codex (project)                              | Gemini (global)                         | Gemini (project)                       |
|------------|--------------------------------------------|----------------------------------------------|-----------------------------------------|----------------------------------------|
| Skills     | `~/.agents/skills/renkei-<name>/SKILL.md`  | `.agents/skills/renkei-<name>/SKILL.md`      | `~/.gemini/skills/renkei-<name>/SKILL.md` | `.gemini/skills/renkei-<name>/SKILL.md` |
| Hooks      | `~/.codex/hooks.json`                      | `.codex/hooks.json`                          | Merge into `~/.gemini/settings.json`    | Merge into `.gemini/settings.json`     |
| Agents     | `~/.codex/agents/<name>.toml`              | `.codex/agents/<name>.toml`                  | `~/.gemini/agents/<name>.md`            | `.gemini/agents/<name>.md`             |
| MCP config | In `~/.codex/config.toml`                  | In `.codex/config.toml`                      | In `~/.gemini/settings.json`            | In `.gemini/settings.json`             |

| Artifact   | Agents (global)                             | Agents (project)                            |
|------------|---------------------------------------------|---------------------------------------------|
| Skills     | `~/.agents/skills/renkei-<name>/SKILL.md`   | `.agents/skills/renkei-<name>/SKILL.md`     |
| Hooks      | Not supported                               | Not supported                               |
| Agents     | Not supported                               | Not supported                               |
| MCP config | Not supported                               | Not supported                               |

In project scope, skills and agents deploy to the equivalent paths under `<project_root>/` instead of `~/`. The `renkei-` prefix on skills creates a clear namespace and avoids collisions with native skills.

**Scope behavior per backend**: Claude Code hooks and MCP always deploy globally regardless of scope. Cursor, Codex, and Gemini support project-level hooks and MCP.
