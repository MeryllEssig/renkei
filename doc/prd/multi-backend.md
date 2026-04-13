# Multi-Backend Configuration

## Overview

Renkei supports deploying packages to multiple AI coding tool backends simultaneously. Rather than hardcoding a single backend, users declare which backends they use via `rk config`, and Renkei deploys to all of them in a single atomic `rk install`.

## Backend registry

Renkei recognizes five backends:

| Backend | Config directory (global) | Config directory (project) | Detection |
|---------|--------------------------|---------------------------|-----------|
| `claude` | `~/.claude/` | `.claude/` | Directory exists |
| `cursor` | `~/.cursor/` | `.cursor/` | Directory exists |
| `codex` | `~/.codex/` | `.codex/` | Directory exists |
| `gemini` | `~/.gemini/` | `.gemini/` | Directory exists |
| `agents` | `~/.agents/` | `.agents/` | Always available (no detection needed) |

The `agents` backend represents the emerging shared `.agents/` standard. It is not tied to any specific tool — it deploys to the `.agents/` directory, which is read by tools that support the convention (currently Codex and Gemini). It is always considered "detected" since it is a filesystem convention, not a runtime.

## Backend artifact support

Not all backends support all artifact types:

| Backend | Skills | Hooks | Agents | MCP |
|---------|--------|-------|--------|-----|
| `claude` | Yes | Yes | Yes | Yes |
| `cursor` | Yes | Yes | Yes | Yes |
| `codex` | Yes | Yes | Yes (TOML) | Yes (TOML) |
| `gemini` | Yes | Yes | Yes | Yes |
| `agents` | **Yes** | **No** | **No** | **No** |

When a package contains artifacts not supported by a backend (e.g., hooks for the `agents` backend), Renkei issues a **warning** and skips those artifacts for that backend. The install proceeds for supported artifacts.

## Reads-from matrix (deduplication)

Some backends read from the shared `.agents/` directory in addition to their own. When the `agents` backend has already deployed a skill, branded backends that read `.agents/` skip their own deployment for that skill to avoid duplication.

| Backend | Reads `.agents/skills/`? |
|---------|-------------------------|
| `claude` | No |
| `cursor` | No |
| `codex` | Yes |
| `gemini` | Yes |
| `agents` | Yes (it IS `.agents/`) |

This matrix is **hardcoded** in each `Backend` implementation. It reflects a property of the tool, not a user preference. When a tool adds `.agents/` support, the corresponding backend implementation is updated in a Renkei release.

**Example**: User configures `["agents", "codex", "claude"]`. A skill is deployed once to `.agents/skills/` (for `agents`) and once to `.claude/skills/` (for `claude`). Codex skips its own deployment because it reads `.agents/skills/`.

**Note on Codex**: Since Codex natively deploys skills to `.agents/skills/` (not `.codex/skills/`), the deduplication with the `agents` backend is always active. In practice, if a user configures both `agents` and `codex`, skill deployment for Codex is always a no-op. This is by design — the `agents` backend and Codex share the same skill directory.

## User configuration (`rk config`)

### Config file

- **Path**: `~/.renkei/config.json`
- **Created by**: `rk config` (interactive or `rk config set`) — never created implicitly
- **Format**:

```json
{
  "defaults": {
    "backends": ["claude", "agents"]
  }
}
```

### Interactive mode

`rk config` with no arguments launches an interactive prompt:

```
Configure Renkei defaults

Which backends do you want to deploy to?
(space to toggle, enter to confirm)

[x] claude    (detected)
[x] cursor    (detected)
[ ] codex     (not detected)
[ ] gemini    (not detected)
[x] agents    (shared .agents/ standard)
```

Detected backends are pre-checked. The `agents` backend is always listed and never shows a detection status.

### Programmatic mode

| Command | Effect |
|---------|--------|
| `rk config set defaults.backends claude,cursor` | Set backends |
| `rk config get defaults.backends` | Print current backends |
| `rk config list` | Print full config file |

### No config (fallback behavior)

When `~/.renkei/config.json` does not exist, `rk install` falls back to **auto-detection**: it scans for all backend config directories and deploys to every detected backend. This preserves the "it just works" experience for users who never run `rk config`.

Auto-detection results are **not persisted**. The user must explicitly run `rk config` to save their preferences.

## Backend resolution at install time

The effective set of target backends is determined by a three-step pipeline:

### Step 1: User backends

- If `~/.renkei/config.json` exists → `config.defaults.backends`
- If no config → auto-detect all backends with an existing config directory
- If `--backend <name>` flag is passed → override with `[<name>]` for this install only

### Step 2: Manifest intersection

Intersect user backends with `manifest.backends`:

```
effective = user_backends ∩ manifest.backends
```

If the intersection is empty → **error**: "Package supports [cursor] but your configured backends are [claude]. Add a compatible backend with `rk config` or use `--force`."

With `--force`: bypass the manifest restriction. Deploy to all user backends regardless of `manifest.backends`.

### Step 3: Detection filter

Intersect with actually detected backends:

```
final = effective ∩ detected_backends
```

- `agents` always passes detection (see [Backend registry](#backend-registry))
- If a configured backend is not detected → **warning**: "Backend 'cursor' is configured but not detected on this machine — skipping."
- If `final` is empty (all configured backends are undetected) → **error**: "None of your configured backends are detected. Run `rk config` to update your setup."
- `--force` does **not** bypass detection. Deploying to a non-existent backend directory serves no purpose.

## Deployment

### Multi-backend deploy

For each backend in the final set, the install pipeline runs the backend's deploy methods. Each backend deploys according to its own conventions (paths, formats, merge strategies).

### Atomicity

Multi-backend deployment is **fully atomic**. All writes across all backends are collected in a single `Vec<Write>`. On any error, all writes are rolled back in reverse order — including writes to backends that succeeded.

### Install-cache format (v2)

The install-cache groups deployed artifacts by backend:

```json
{
  "version": 2,
  "packages": {
    "@scope/name": {
      "version": "1.0.0",
      "source": "git",
      "source_path": "git@github.com:user/repo",
      "integrity": "sha256-...",
      "archive_path": "~/.renkei/archives/@scope/name/1.0.0.tar.gz",
      "deployed": {
        "claude": {
          "artifacts": [
            { "artifact_type": "skill", "name": "review", "deployed_path": "~/.claude/skills/review/SKILL.md" }
          ],
          "hooks": [
            { "event": "PreToolUse", "matcher": "Edit", "command": "..." }
          ],
          "mcp_servers": ["my-server"]
        },
        "agents": {
          "artifacts": [
            { "artifact_type": "skill", "name": "review", "deployed_path": ".agents/skills/review/SKILL.md" }
          ]
        }
      }
    }
  }
}
```

This grouped format is **always used**, even with a single backend. There is no flat format.

### Migration from v1

On load, if `version == 1`, the install-cache is automatically migrated to v2: all existing `deployed_artifacts`, `deployed_mcp_servers`, and hook data are wrapped under `"claude"` (the only backend in v1). The migrated cache is saved immediately.

## Impact on existing commands

| Command | Multi-backend impact |
|---------|---------------------|
| `rk install` | Deploys to N backends, atomic rollback across all |
| `rk uninstall` | Removes artifacts from all backends listed in the package's install-cache entry |
| `rk list` | Displays target backends per package |
| `rk doctor` | Checks artifacts per backend, reports per-backend health |
| `rk config` | **New command** — manage user preferences |
| `rk package` | No impact (creates an archive, no deployment) |
| `rk migrate` | No impact |

The `--backend` flag is available **only on `rk install`**. All other commands operate on all backends recorded in the install-cache entry.

## Relationship with other PRD sections

- **[Backends](./backends.md)**: defines the `Backend` trait and per-tool deployment conventions. Multi-backend configuration adds the orchestration layer on top.
- **[Scope](./scope.md)**: scope (global/project) is orthogonal to backend selection. A package can be installed in project scope to multiple backends simultaneously.
- **[Installation](./installation.md)**: the install pipeline gains a backend resolution step before artifact deployment.
- **[Storage](./storage.md)**: install-cache format changes to v2 with per-backend grouping.
