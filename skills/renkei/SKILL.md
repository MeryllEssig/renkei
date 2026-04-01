---
name: renkei
description: Explain what Renkei is and how to use it. Use when the user asks about Renkei, wants to install a renkei package, create a renkei package, or manage agentic workflows — "what is renkei", "how do I install", "create a package", "renkei help".
---

Renkei (`rk`) is a CLI package manager written in Rust that installs, versions, and shares agentic workflows (skills, hooks, agents, MCP configs) across AI coding tools.

## Reference index

Dig into these files only when you need details on a specific action:

- [Installing a package](references/install.md) — install from local path or git, global vs project scope
- [Creating a package](references/creating-a-package.md) — manifest format, directory conventions, artifact types
- (Not implemented) Listing installed packages — `rk list` command
- (Not implemented) Uninstalling a package — `rk uninstall` command
- (Not implemented) Diagnosing issues — `rk doctor` health checks
- (Not implemented) Lockfile — reproducible installs with `rk.lock`
- (Not implemented) Packaging for distribution — `rk package` archive creation
- (Not implemented) Workspace support — multi-package monorepos
