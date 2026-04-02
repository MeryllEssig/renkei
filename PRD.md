# PRD — Renkei: Package Manager for Agentic Workflows

> This document is the index of the Renkei PRD. Each section links to a dedicated thematic file in [`doc/prd/`](./doc/prd/) for progressive discovery.

## [Overview](./doc/prd/overview.md)

Problem statement and solution summary. Renkei is a CLI package manager (`rk`) written in Rust that lets you install, version, and share agentic workflows across AI tools (Claude Code, Cursor, etc.).

## [User Stories](./doc/prd/user-stories.md)

All user stories organized by theme and phase:
- Installation and deployment (US 1–14)
- Installation scope (US 14b–14g)
- Conflict management (US 15–18)
- Listing and visibility (US 19–20)
- Diagnostics (US 21–25)
- Package creation (US 26–29)
- Lockfile (US 30–34)
- Phase 1 — Delivery and migration (US 35–36)
- Multi-backend configuration (US 55–65)
- Phase 2 — Registry and advanced commands (US 37–49)
- Phase 3 — Ecosystem (US 50–54)

## [Manifest and Artifact Format](./doc/prd/manifest.md)

`renkei.json` manifest specification, workspace support, and neutral artifact format (markdown + frontmatter for skills/agents, abstract JSON for hooks, native format for MCP).

## [Backends](./doc/prd/backends.md)

`Backend` trait interface, multi-backend support matrix (Claude Code, Cursor, Codex, Gemini), and hardcoded deployment conventions (paths, prefixes, merge targets).

## [Multi-Backend Configuration](./doc/prd/multi-backend.md)

User-facing backend selection via `rk config`, auto-detection fallback, backend resolution pipeline (user config ∩ manifest ∩ detection), the `agents` shared standard backend, install-cache v2 format with per-backend grouping, atomic multi-backend deployment and rollback.

## [Installation Scope](./doc/prd/scope.md)

Global vs project installation scope. Project scope (default) deploys skills/agents locally, global scope (`-g`) deploys everything to `~/`. Scope field in manifest (`any`, `global`, `project`), project root detection, Config adaptation, uninstall mirroring.

## [Installation](./doc/prd/installation.md)

Git and local installation flows, no-argument lockfile install, scope validation, fail-fast + rollback strategy, conflict management (TTY/non-TTY/force), and environment variable handling.

## [Hooks](./doc/prd/hooks.md)

Abstract hook format with normalized events, Renkei → Claude Code event mapping table, and hook tracking strategy via `install-cache.json`.

## [Local Storage and Lockfile](./doc/prd/storage.md)

`~/.renkei/` directory layout (archives, install-cache, projects, config) and `rk.lock` lockfile format with versioning, source tracking, and SHA-256 integrity.

## [Commands](./doc/prd/commands.md)

`rk doctor` diagnostic checks (backends, files, env vars, hashes, hooks, MCP) and `rk package` archive creation (included directories, version bump).

## [Testing Decisions](./doc/prd/testing.md)

Testing philosophy (external behavior only) and modules to test: manifest parsing, artifact discovery, deployment, hook translation, lockfile, backend detection, conflicts, doctor, package.

## [Roadmap, Scope and Risks](./doc/prd/roadmap.md)

Out of scope items, risk matrix, licensing model, language/distribution choices, registry (Phase 2) design, and further implementation notes.
