# Overview

## Problem Statement

Developers and teams using AI tools (Claude Code, Cursor, Codex, etc.) produce complex agentic workflows: skills, hooks, specialized agents, MCP configurations, scripts. These artifacts are currently:

- **unversioned**: no version tracking, impossible to know which version is running in production;
- **not easily shareable**: manual copy-paste between machines, Slack, email, drive;
- **not portable**: each developer manually reconfigures for each AI tool;
- **invisible**: no list of installed workflows, no health diagnostics;
- **fragile**: a local modification to a skill silently breaks the workflow.

There is no standard primitive for distributing a "complete workflow" (skill + hook + agent + MCP config) the way you distribute an npm package or a homebrew plugin.

## Solution

**Renkei** is a CLI package manager (`rk`) written in Rust that lets you install, version, and share agentic workflows. A workflow is a **package**: a folder with a `renkei.json` manifest describing its artifacts. The CLI deploys each artifact to the correct location based on the detected backend (Claude Code, Cursor...), with no manual configuration.

```
rk install git@github.com:meryll/mr-review
rk install ./my-workflow/
rk list
rk doctor
```

Renkei is **content-agnostic** — it doesn't execute packages, it distributes them.
