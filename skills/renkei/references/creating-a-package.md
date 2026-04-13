# Creating a renkei package

A package is a folder with a `renkei.json` manifest and conventional directories.

## Directory structure

```
my-package/
├── renkei.json        # required
├── skills/            # skill directories (each with SKILL.md)
├── hooks/             # JSON hook definitions (*.json)
├── agents/            # markdown files (*.md)
├── scripts/           # supporting scripts
└── mcp/               # local MCP server sources (one folder per server)
    └── my-server/     # name MUST match mcp.<name> in the manifest
        └── …
```

Artifacts are discovered automatically from `skills/`, `hooks/`, `agents/` — no listing needed in the manifest. Local MCP servers under `mcp/<name>/` are picked up by matching the folder name to a `mcp.<name>` entry in the manifest — see [Local MCP servers](local-mcp.md).

A `.rkignore` file at the package root (gitignore syntax) extends the always-on exclusion list (`node_modules/`, `dist/`, `build/`, `target/`, `.venv/`, `venv/`, `__pycache__/`, `.pytest_cache/`, `*.pyc`, `.DS_Store`, `.git/`). Inside `mcp/<name>/`, the `dist/build/target` defaults are relaxed so prebuilt entrypoints survive the archive.

## Manifest (`renkei.json`)

```json
{
  "name": "@scope/package-name",
  "version": "1.0.0",
  "description": "What this package does",
  "author": "Your Name",
  "license": "MIT",
  "backends": ["claude"],
  "scope": "any",
  "keywords": ["review", "testing"],
  "requiredEnv": ["GITHUB_TOKEN"],
  "messages": {
    "preinstall": "This workflow requires a configured Redmine MCP server.",
    "postinstall": "Run `rk doctor` to verify the setup."
  },
  "mcp": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "some-mcp-server"]
    }
  }
}
```

### Required fields

| Field | Format |
|-------|--------|
| `name` | Scoped: `@scope/name` |
| `version` | Semver: `1.0.0` |
| `description` | Free text |
| `author` | Free text |
| `license` | SPDX identifier |
| `backends` | Array: `["claude"]` |

### Optional fields

| Field | Default | Purpose |
|-------|---------|---------|
| `scope` | `"any"` | `"any"`, `"global"`, or `"project"` |
| `keywords` | `[]` | Search keywords |
| `requiredEnv` | `[]` | Env vars checked post-install (warning only) |
| `messages` | `{}` | `preinstall` (requires user confirmation) and `postinstall` (passive notice). Each capped at 2000 chars. |
| `mcp` | `{}` | MCP servers to register in `~/.claude.json` |

## Hooks format

Each hook file in `hooks/` is a JSON object:

```json
{
  "event": "before_tool",
  "matcher": "Edit",
  "command": "python scripts/check.py $FILE",
  "timeout": 5000
}
```

Supported events: `before_tool`, `after_tool`, `before_command`, `after_command`, `before_model`, `after_model`, `notification`, `before_submit`, `after_submit`, `before_stop`, `after_stop`.

## Deployment paths

| Artifact | Project scope | Global scope |
|----------|--------------|--------------|
| Skills | `.claude/skills/<name>/SKILL.md` | `~/.claude/skills/<name>/SKILL.md` |
| Agents | `.claude/agents/<name>.md` | `~/.claude/agents/<name>.md` |
| Hooks | `~/.claude/settings.json` (always global) | `~/.claude/settings.json` |
| MCP | `~/.claude.json` (always global) | `~/.claude.json` |
