---
name: renkei
description: Explain what Renkei is and how to use it. Use when the user asks about Renkei, wants to install a renkei package, create a renkei package, or manage agentic workflows — "what is renkei", "how do I install", "create a package", "renkei help".
---

Renkei (`rk`) is a CLI package manager written in Rust that installs, versions, and shares agentic workflows (skills, hooks, agents, MCP configs) across AI coding tools.

## Reference index

Dig into these files only when you need details on a specific action:

- [Installing a package](references/install.md) — install from local path or git, global vs project scope, lockfile replay
- [Creating a package](references/creating-a-package.md) — manifest format, directory conventions, artifact types
- [Listing installed packages](references/list.md) — `rk list` command
- [Uninstalling a package](references/uninstall.md) — `rk uninstall` command
- [Diagnosing issues](references/doctor.md) — `rk doctor` health checks
- [Lockfile](references/lockfile.md) — reproducible installs with `rk.lock`
- [Packaging for distribution](references/package.md) — `rk package` archive creation
- [Workspace support](references/workspace.md) — multi-package monorepos
- [Migration](references/migrate.md) — `rk migrate` from old format
