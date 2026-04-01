# Creating a renkei package

A package is a folder with a `renkei.json` manifest and conventional directories.

## Directory structure

```
my-package/
├── renkei.json        # required
├── skills/            # markdown files (*.md)
├── hooks/             # JSON hook definitions (*.json)
├── agents/            # markdown files (*.md)
└── scripts/           # supporting scripts
```

Artifacts are discovered automatically from `skills/`, `hooks/`, `agents/` — no listing needed in the manifest.

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
| Skills | `.claude/skills/renkei-<name>/SKILL.md` | `~/.claude/skills/renkei-<name>/SKILL.md` |
| Agents | `.claude/agents/<name>.md` | `~/.claude/agents/<name>.md` |
| Hooks | `~/.claude/settings.json` (always global) | `~/.claude/settings.json` |
| MCP | `~/.claude.json` (always global) | `~/.claude.json` |
