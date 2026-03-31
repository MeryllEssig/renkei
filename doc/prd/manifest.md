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

## Manifest `renkei.json`

- Required fields: `name` (scoped `@scope/name`, **required from v1**), `version` (semver), `description`, `author`, `license`, `backends`.
- Optional fields: `keywords`, `mcp`, `requiredEnv`, `workspace`.
- **No `artifacts` field**: pure convention. The `skills/`, `hooks/`, `agents/` directories are the source of truth. Any file present in these directories is a deployed artifact.
- `mcp` declares MCP configurations in the native `command`/`args`/`env` format (standard between Claude and Cursor, no extra abstraction).
- `requiredEnv` lists environment variables with their descriptions.

```json
{
  "name": "@meryll/mr-review",
  "version": "1.2.0",
  "description": "Automated code review",
  "author": "meryll",
  "license": "MIT",
  "backends": ["claude"],
  "mcp": {
    "my-server": {
      "command": "node",
      "args": ["server.js"],
      "env": { "API_KEY": "${API_KEY}" }
    }
  },
  "requiredEnv": {
    "GITHUB_TOKEN": "Required for GitHub API access"
  }
}
```

## Neutral artifact format

All artifacts are written in a neutral Renkei format that each backend translates:

- **Skills and agents**: markdown + frontmatter format (Claude Code style). This format is the Renkei neutral format — other backends translate from it.
- **Hooks**: abstract Renkei format with normalized events (see [Hooks](./hooks.md)).
- **MCP**: native `command`/`args`/`env` format directly in the manifest (already portable across backends).

```markdown
---
name: review
description: Review code changes
---
Review the code...
```
