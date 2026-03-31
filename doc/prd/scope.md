# Installation Scope: Global vs Project

## Overview

Renkei supports two installation scopes:

- **Project scope** (default): artifacts are deployed relative to the current project. Skills and agents go into the project's local backend directory (e.g., `.claude/` for Claude Code). Hooks and MCP servers are always deployed globally (they are inherently global resources), but the package is tracked in the project's install-cache and lockfile.
- **Global scope** (`-g` / `--global`): all artifacts are deployed to the user's home backend directory (e.g., `~/.claude/`). The package is tracked in the global install-cache and lockfile.

## Default behavior

| Command | Scope | Artifacts | Tracking |
|---------|-------|-----------|----------|
| `rk install <source>` | Project | Skills/agents → `.claude/`, hooks/MCP → `~/.claude/` | `./rk.lock` + `~/.renkei/projects/<slug>/install-cache.json` |
| `rk install -g <source>` | Global | Everything → `~/.claude/` | `~/.renkei/rk.lock` + `~/.renkei/install-cache.json` |
| `rk install` (no args) | Project | Reads `./rk.lock` | Same as project |
| `rk install -g` (no args) | Global | Reads `~/.renkei/rk.lock` | Same as global |

## Project root detection

The project root is detected via `git rev-parse --show-toplevel`. If no git repository is found and `-g` is not specified, `rk install` fails with:

```
Error: No project root detected (not inside a git repository).
Use `rk install -g <source>` to install globally.
```

## Scope field in the manifest

The `renkei.json` manifest supports an optional `scope` field:

```json
{
  "scope": "any"
}
```

| Value | Meaning |
|-------|---------|
| `any` (default) | Installable in both project and global scope |
| `global` | Only installable with `-g`. Error otherwise: "This package is global-only, use -g" |
| `project` | Only installable without `-g`. Error with `-g`: "This package is project-only, remove -g" |

## Scope coexistence

A package can be installed both globally and in a project simultaneously. Renkei does not manage priority between the two — the backend (Claude Code, Cursor, etc.) determines which takes precedence based on its own resolution rules.

For hooks and MCP servers (always deployed globally), if the same entry exists from both a global and a project installation, both coexist in the backend's configuration files.

## Config adaptation

The `Config` struct absorbs the scope. A `Config::for_project(project_root)` constructor redirects skill and agent deployment paths to the project's `.claude/` directory while keeping hooks and MCP paths pointing to `~/.claude/`. The backend remains agnostic to the scope — it follows the paths from `Config`.

## Project install-cache location

Project install-caches are stored centrally in `~/.renkei/projects/` to avoid polluting project directories with files that would need to be gitignored. The subdirectory name is a slugified version of the project's absolute path:

```
~/.renkei/projects/Users-meryll-Projects-foo/install-cache.json
~/.renkei/projects/Users-meryll-Projects-bar/install-cache.json
```

## Uninstall behavior

- `rk uninstall @scope/pkg` — looks up the project install-cache, removes artifacts (skills/agents from `.claude/`, hooks/MCP from `~/.claude/`), updates `./rk.lock`.
- `rk uninstall -g @scope/pkg` — looks up the global install-cache, removes all artifacts from `~/.claude/`, updates `~/.renkei/rk.lock`.
- If the package is not found in the requested scope, error — no fallback to the other scope.
