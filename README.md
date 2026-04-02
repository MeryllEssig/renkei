<div align="center">

<img src="./doc/renkei.png" alt="Renkei" height="128" />

# Renkei

*A package manager for AI agentic workflows*

[![Build Status](https://img.shields.io/github/actions/workflow/status/meryll/renkei/ci.yml?style=flat-square&label=Build)](https://github.com/meryll/renkei/actions)
[![License](https://img.shields.io/badge/License-MIT-yellow?style=flat-square)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.94+-orange?style=flat-square&logo=rust&logoColor=white)

[Features](#features) | [Installation](#installation) | [Quick Start](#quick-start) | [Usage](#usage) | [Supported Backends](#supported-backends)

</div>

AI tool configurations — skills, hooks, agents, MCP servers — are scattered across different tools, unversioned, and impossible to share. **Renkei** (`rk`) packages them into a single portable format and deploys them across your AI tools simultaneously.

```bash
rk install git@github.com:team/our-workflows   # Install from git
rk install ./local-package                      # Install from local path
rk list                                          # See what's installed
rk doctor                                        # Run health checks
```

## Features

- **Multi-backend deployment** — One package installs to Claude Code, Cursor, Codex, Gemini CLI, and the shared `.agents/` standard, all at once
- **Portable format** — A `renkei.json` manifest plus convention directories (`skills/`, `agents/`, `hooks/`) that any team member can install
- **Lockfile & integrity** — `rk.lock` records exact git SHAs and SHA-256 hashes for reproducible installs
- **Scoped installs** — Project-level by default, global with `-g`
- **Conflict detection** — Interactive resolution in TTY, hard errors in CI, `--force` to override
- **Atomic rollback** — If any backend fails, all changes are rolled back
- **Workspaces** — Monorepo support for multi-package repositories
- **Health diagnostics** — `rk doctor` checks deployed files, integrity hashes, env vars, and backend availability

## Installation

### From releases

```bash
# macOS (Apple Silicon)
curl -fsSL https://github.com/meryll/renkei/releases/latest/download/rk-darwin-aarch64.tar.gz | tar xz
sudo mv rk /usr/local/bin/

# Linux (x86_64)
curl -fsSL https://github.com/meryll/renkei/releases/latest/download/rk-linux-x86_64.tar.gz | tar xz
sudo mv rk /usr/local/bin/
```

### From source

```bash
git clone git@github.com:meryll/renkei.git
cd renkei
cargo install --path .
```

## Quick Start

### Create a package

```
my-workflow/
├── renkei.json
├── skills/
│   └── code-review.md
├── agents/
│   └── researcher.md
└── hooks/
    └── pre-commit.json
```

```json
{
  "name": "@team/my-workflow",
  "version": "1.0.0",
  "description": "Shared AI workflow for our team",
  "author": "Team",
  "license": "MIT",
  "backends": ["claude", "cursor"]
}
```

### Install it

```bash
rk install ./my-workflow
```

Skills, agents, and hooks are deployed to `.claude/` and `.cursor/` in your project. A `rk.lock` is created to track what was installed.

### Distribute it

```bash
rk package --bump minor   # Creates a versioned .tar.gz archive
```

Share via git or distribute the archive directly.

## Usage

| Command | Description |
|:---|:---|
| `rk install <source>` | Install from git URL or local path |
| `rk install` | Restore from `rk.lock` |
| `rk install -g <source>` | Install globally (`~/`) |
| `rk install --tag v1.0` | Pin to a git tag |
| `rk uninstall @scope/name` | Remove a package |
| `rk list` | Show installed packages |
| `rk doctor` | Run health diagnostics |
| `rk package` | Create distributable archive |
| `rk package --bump <level>` | Bump version and package |
| `rk config` | Interactive backend configuration |
| `rk config set defaults.backends claude,cursor` | Set backends |
| `rk migrate ./path` | Convert a directory to a Renkei package |

> [!TIP]
> Run `rk install` with no arguments to restore all packages from an existing `rk.lock` — useful for onboarding or CI.

## Supported Backends

| Backend | Skills | Agents | Hooks | MCP |
|:---|:---:|:---:|:---:|:---:|
| Claude Code | `.claude/skills/` | `.claude/agents/` | `settings.json` | `~/.claude.json` |
| Cursor | `.cursor/rules/` | `.cursor/agents/` | `hooks.json` | `mcp.json` |
| Codex | `.agents/skills/` | `.toml` files | `hooks.json` | `config.toml` |
| Gemini CLI | `.gemini/skills/` | `.md` files | `settings.json` | `settings.json` |
| Agents (shared) | `.agents/skills/` | — | — | — |

Renkei auto-detects which backends are available and deploys to all of them. Override with `--backend` or `rk config`.

## Manifest Reference

The `renkei.json` manifest supports these fields:

```json
{
  "name": "@scope/name",
  "version": "1.0.0",
  "description": "...",
  "author": "...",
  "license": "MIT",
  "backends": ["claude", "cursor", "codex", "gemini"],
  "scope": "any",
  "keywords": [],
  "mcp": { },
  "requiredEnv": { },
  "workspace": ["packages/a", "packages/b"]
}
```

| Field | Required | Description |
|:---|:---:|:---|
| `name` | Yes | Scoped package name (`@scope/name`) |
| `version` | Yes | Semver version |
| `description` | Yes | Short description |
| `author` | Yes | Author name |
| `license` | Yes | License identifier |
| `backends` | Yes | Target backends |
| `scope` | No | `"any"` (default), `"global"`, or `"project"` |
| `mcp` | No | MCP server configurations |
| `requiredEnv` | No | Required environment variables |
| `workspace` | No | Monorepo member paths |

## Artifact Format

Artifacts use a neutral Markdown format with optional YAML frontmatter. Renkei translates them to each backend's native format during deployment.

```markdown
---
description: Reviews code for best practices
---

Review the code changes and provide feedback on:
- Code quality and readability
- Potential bugs or edge cases
- Performance considerations
```

> [!NOTE]
> Hooks use a JSON format with abstract events (`pre-commit`, `post-save`, etc.) that Renkei maps to each backend's hook system.

## Development

```bash
cargo test              # Run all tests
cargo test -- --nocapture  # With output
cargo clippy            # Lint
cargo fmt               # Format
```
