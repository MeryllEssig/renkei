# Manifest and Artifact Format

## Workspace

A Git repo can contain multiple packages (workspace). Each sub-package lives in a folder at the root (`./mr-review/`, `./auto-test/`). A root `renkei.json` declares members via a `workspace` field:

```json
{
  "workspace": ["mr-review", "auto-test"]
}
```

Each subfolder contains its own complete `renkei.json` and its conventional directories.

For a repo without a workspace (single package), the conventional directories (`skills/`, `hooks/`, `agents/`) are directly at the root.

By default, `rk install` deploys all declared members. Use `-m <member>` (repeatable, CSV-aware) to install a subset — e.g. `rk install <giturl> -m mr-review`. The selected member name is persisted in the lockfile so `rk install` (no-arg) replays the same subset. See [Installation](./installation.md#selective-workspace-install--m--member) for the full validation rules.

## Manifest `renkei.json`

- Required fields: `name` (scoped `@scope/name`, **required from v1**), `version` (semver), `description`, `author`, `license`, `backends`.
- Optional fields: `keywords`, `mcp`, `requiredEnv`, `workspace`, `scope`, `messages`.
- `backends` declares which tools the package supports. Valid values: `"claude"`, `"cursor"`, `"codex"`, `"gemini"`, `"agents"`. At install time, the effective target is `manifest.backends ∩ user configured backends ∩ detected backends`. See [Multi-Backend Configuration](./multi-backend.md) for the full resolution pipeline.
- **No `artifacts` field**: pure convention. The `skills/`, `hooks/`, `agents/` directories are the source of truth. Any file present in these directories is a deployed artifact.
- `mcp` declares MCP configurations. By default, `mcp.<name>` is the native `command`/`args`/`env` block for an externally-installed server. Adding `entrypoint` (relative path inside `mcp/<name>/`) and `build` (array of argv steps) turns the entry into a **local MCP**: Renkei copies the source from `mcp/<name>/`, runs the build, and registers the absolute entrypoint with the backend. See [MCP — External and Local Servers](./mcp.md).
- `requiredEnv` lists environment variables with their descriptions.
- `scope` controls where the package can be installed: `"any"` (default, both global and project), `"global"` (only with `-g`), or `"project"` (only without `-g`). See [Scope](./scope.md).
- `messages` declares optional install-time notices for the user:
  - `messages.preinstall`: shown before any deployment work, gated by a `[y/N]` prompt. Use it to communicate prerequisites (env vars to set, MCP servers to configure separately, breaking changes). Required to be confirmed every time; non-TTY callers must pass `--yes`. See [Installation > Preinstall confirmation](./installation.md#preinstall-confirmation).
  - `messages.postinstall`: passive notice rendered after a successful install, after the `requiredEnv` warnings. Use it for follow-up steps (run `rk doctor`, restart Claude Code, etc.).
  - Both are plain strings; `\n` allowed for multi-line. Hard cap of 2000 characters per field, enforced at manifest validation.

```json
{
  "name": "@meryll/mr-review",
  "version": "1.2.0",
  "description": "Automated code review",
  "author": "meryll",
  "license": "MIT",
  "backends": ["claude"],
  "scope": "any",
  "mcp": {
    "my-server": {
      "command": "node",
      "args": ["server.js"],
      "env": { "API_KEY": "${API_KEY}" }
    }
  },
  "requiredEnv": {
    "GITHUB_TOKEN": "Required for GitHub API access"
  },
  "messages": {
    "preinstall": "This workflow expects the GitLab MCP server to already be configured.",
    "postinstall": "Run `rk doctor` to verify the install, then restart Claude Code."
  }
}
```

## Neutral artifact format

All artifacts are written in a neutral Renkei format that each backend translates:

- **Skills and agents**: markdown + frontmatter format (Claude Code style). This format is the Renkei neutral format — other backends translate from it.
- **Hooks**: abstract Renkei format with normalized events (see [Hooks](./hooks.md)).
- **MCP**: native `command`/`args`/`env` format directly in the manifest (portable across backends). Local MCPs additionally declare `entrypoint` + `build` and ship sources under `mcp/<name>/` — see [MCP — External and Local Servers](./mcp.md).

```markdown
---
name: review
description: Review code changes
---
Review the code...
```
