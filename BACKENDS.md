# Backend Research: Claude Code, Cursor, Codex, Gemini CLI

> Last updated: 2026-04-02
>
> This document compares the configuration systems of the four AI coding tools Renkei targets as backends. It covers skills, hooks, MCP, agents, and scope conventions for each tool, focusing on what Renkei needs to know to deploy artifacts correctly.

---

## Table of Contents

1. [Quick Comparison Matrix](#quick-comparison-matrix)
2. [Claude Code](#claude-code)
3. [Cursor](#cursor)
4. [Codex (OpenAI)](#codex-openai)
5. [Gemini CLI (Google)](#gemini-cli-google)
6. [Cross-Backend Analysis](#cross-backend-analysis)
7. [Implications for Renkei](#implications-for-renkei)

---

## Quick Comparison Matrix

### Artifact Support

| Artifact    | Claude Code         | Cursor                | Codex                 | Gemini CLI            |
|-------------|---------------------|-----------------------|-----------------------|-----------------------|
| Skills      | Yes (`SKILL.md`)    | Yes (`.mdc`/`.md`)   | Yes (`SKILL.md`)      | Yes (`SKILL.md`)      |
| Hooks       | Yes (JSON)          | Yes (JSON)            | Yes (JSON)            | Yes (JSON)            |
| Agents      | Yes (`.md`)         | Yes (`.md`)           | Yes (`.toml`)         | Yes (`.md`)           |
| MCP         | Yes (JSON)          | Yes (JSON)            | Yes (TOML)            | Yes (JSON)            |

### Config Formats

| Tool        | Settings format | Skills format             | Hooks format      | Agents format     | MCP format |
|-------------|-----------------|---------------------------|--------------------|-------------------|------------|
| Claude Code | JSON            | Markdown                  | In settings JSON   | Markdown           | JSON       |
| Cursor      | JSON            | Markdown + YAML frontmatter (`.mdc`) | Standalone JSON | Markdown + YAML frontmatter | JSON |
| Codex       | TOML            | Markdown + YAML frontmatter | Standalone JSON  | TOML               | In config TOML |
| Gemini CLI  | JSON            | Markdown + YAML frontmatter | In settings JSON | Markdown + YAML frontmatter | In settings JSON |

### Config Directory Names

| Tool        | Global config dir | Project config dir | Context file   |
|-------------|-------------------|--------------------|----------------|
| Claude Code | `~/.claude/`      | `.claude/`         | `CLAUDE.md`    |
| Cursor      | `~/.cursor/`      | `.cursor/`         | `.cursorrules` (legacy) / `.cursor/rules/` |
| Codex       | `~/.codex/`       | `.codex/`          | `AGENTS.md`    |
| Gemini CLI  | `~/.gemini/`      | `.gemini/`         | `GEMINI.md`    |

---

## Claude Code

### Skills

- **Format**: Plain Markdown (`SKILL.md`), no required frontmatter
- **Global path**: `~/.claude/skills/<name>/SKILL.md`
- **Project path**: `<project>/.claude/skills/<name>/SKILL.md`
- **Activation**: Description-based auto-matching or explicit invocation
- **Subdirectories**: Can contain additional files (scripts, references)

### Hooks

- **Format**: JSON, embedded in `settings.json` under event keys
- **Location**: `~/.claude/settings.json` (global only, no project-level hooks file)
- **Events** (6):
  | Event | Key in settings.json |
  |-------|---------------------|
  | Before tool use | `PreToolUse` |
  | After tool use | `PostToolUse` |
  | User prompt submit | `UserPromptSubmit` |
  | Session stop | `Stop` |
  | Notification | `Notification` |
  | MCP tool use | `McpToolUse` |
- **Structure**:
  ```json
  {
    "hooks": {
      "PreToolUse": [{
        "matcher": "Bash",
        "hooks": [{
          "type": "command",
          "command": "./scripts/check.sh",
          "timeout": 300
        }]
      }]
    }
  }
  ```
- **Scope**: Always global. Hooks live in `~/.claude/settings.json` regardless of install scope.

### Agents

- **Format**: Markdown (`.md`), description in content
- **Global path**: `~/.claude/agents/<name>.md`
- **Project path**: `<project>/.claude/agents/<name>.md`
- **No frontmatter required** (name derived from filename)

### MCP Servers

- **Format**: JSON
- **Location**: `~/.claude.json` (global, separate file from settings.json)
- **Structure**:
  ```json
  {
    "mcpServers": {
      "server-name": {
        "command": "node",
        "args": ["server.js"],
        "env": { "KEY": "value" }
      }
    }
  }
  ```
- **Scope**: Always global.

### Scope Summary

| Artifact | Global | Project |
|----------|--------|---------|
| Skills   | `~/.claude/skills/` | `<project>/.claude/skills/` |
| Hooks    | `~/.claude/settings.json` | `~/.claude/settings.json` (always global) |
| Agents   | `~/.claude/agents/` | `<project>/.claude/agents/` |
| MCP      | `~/.claude.json` | `~/.claude.json` (always global) |

---

## Cursor

### Skills (Rules)

- **Format**: Markdown with YAML frontmatter (`.mdc` or `.md` extension)
- **Global path**: Configured via Cursor Settings UI ("User Rules"), not filesystem files
- **Project path**: `.cursor/rules/<name>.mdc`
- **Legacy**: `.cursorrules` at project root (deprecated, still supported)
- **AGENTS.md**: Also read from project root (cross-tool compatibility)
- **Frontmatter fields**:
  ```markdown
  ---
  description: "When to apply this rule"
  alwaysApply: false
  globs: "**/*.ts"
  ---
  
  Rule content here...
  ```
- **Application modes**:
  | Mode | Trigger |
  |------|---------|
  | Always | `alwaysApply: true` |
  | Auto (intelligent) | `description` present, agent decides |
  | Glob-matched | `globs` pattern matches files in context |
  | Manual | Explicit `@rule-name` invocation |

**Key difference from Claude**: Cursor rules have `globs` for file-pattern matching and `alwaysApply` for unconditional injection. Claude skills rely purely on description matching.

### Hooks

- **Format**: Standalone JSON file (`hooks.json`)
- **Global path**: `~/.cursor/hooks.json`
- **Project path**: `<project>/.cursor/hooks.json`
- **Enterprise**: `/Library/Application Support/Cursor/hooks.json` (macOS MDM)
- **Events** (16 -- much richer than Claude):
  | Category | Events |
  |----------|--------|
  | Agent lifecycle | `sessionStart`, `sessionEnd`, `stop` |
  | Tool execution | `preToolUse`, `postToolUse`, `postToolUseFailure` |
  | Shell | `beforeShellExecution`, `afterShellExecution` |
  | MCP | `beforeMCPExecution`, `afterMCPExecution` |
  | File operations | `beforeReadFile`, `afterFileEdit`, `beforeTabFileRead`, `afterTabFileEdit` |
  | Prompt/context | `beforeSubmitPrompt`, `preCompact` |
  | Response | `afterAgentResponse`, `afterAgentThought` |
  | Subagents | `subagentStart`, `subagentStop` |
- **Structure**:
  ```json
  {
    "version": 1,
    "hooks": {
      "preToolUse": [{
        "command": "./scripts/audit.sh",
        "type": "command",
        "timeout": 30,
        "matcher": "Shell",
        "failClosed": false
      }]
    }
  }
  ```
- **Hook types**: `"command"` (shell script) or `"prompt"` (LLM-evaluated natural language)
- **Exit codes**: `0` = success, `2` = block action, other = fail-open
- **Env vars injected**: `CURSOR_PROJECT_DIR`, `CURSOR_VERSION`, `CURSOR_USER_EMAIL`, `CURSOR_TRANSCRIPT_PATH`

**Key difference from Claude**: Cursor hooks are standalone files (not embedded in settings), support project-level hooks, have 16 events (vs 6), and support a `"prompt"` hook type.

### Agents (Subagents)

- **Format**: Markdown with YAML frontmatter
- **Global path**: `~/.cursor/agents/<name>.md`
- **Project path**: `.cursor/agents/<name>.md`
- **Compatibility aliases**: Also reads `.claude/agents/`, `.codex/agents/`
- **Frontmatter fields**:
  ```yaml
  ---
  name: verifier
  description: "Validates completed work"
  model: fast
  readonly: true
  is_background: false
  ---
  ```
- **Built-in subagents**: Explore, Bash, Browser
- **Invocation**: Automatic delegation or explicit `/subagent-name [task]`
- **No nesting**: Subagents cannot spawn other subagents

**Key difference from Claude**: Cursor agents have richer frontmatter (`model`, `readonly`, `is_background`). Claude agents are simpler Markdown files with no frontmatter.

### MCP Servers

- **Format**: JSON
- **Global path**: `~/.cursor/mcp.json`
- **Project path**: `.cursor/mcp.json`
- **Structure**:
  ```json
  {
    "mcpServers": {
      "server-name": {
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "my-mcp-server"],
        "env": { "API_KEY": "${env:MY_API_KEY}" },
        "envFile": ".env"
      }
    }
  }
  ```
- **Variable interpolation**: `${env:NAME}`, `${userHome}`, `${workspaceFolder}`, `${pathSeparator}`
- **Transports**: stdio, SSE, Streamable HTTP
- **OAuth support**: Native `auth` field for remote servers

**Key difference from Claude**: Cursor MCP uses a dedicated `mcp.json` file (not mixed into `settings.json`), supports project-level MCP, and has variable interpolation.

### Scope Summary

| Artifact | Global | Project |
|----------|--------|---------|
| Rules    | Cursor Settings UI only | `.cursor/rules/*.mdc` |
| Hooks    | `~/.cursor/hooks.json` | `.cursor/hooks.json` |
| Agents   | `~/.cursor/agents/*.md` | `.cursor/agents/*.md` |
| MCP      | `~/.cursor/mcp.json` | `.cursor/mcp.json` |

---

## Codex (OpenAI)

### Skills

- **Format**: Markdown with YAML frontmatter (`SKILL.md`)
- **Discovery locations** (in order):
  | Scope | Path |
  |-------|------|
  | Repo (cwd) | `.agents/skills/<name>/SKILL.md` |
  | Repo (root) | `$REPO_ROOT/.agents/skills/<name>/SKILL.md` |
  | User | `~/.agents/skills/<name>/SKILL.md` |
  | Admin | `/etc/codex/skills/<name>/SKILL.md` |
  | System | Bundled with Codex binary |
- **Frontmatter fields**: `name`, `description`
- **Subdirectories**: `scripts/`, `references/`, `assets/`, `agents/openai.yaml`
- **System skills**: Extracted from binary to `~/.codex/skills/.system/` at startup
- **Config-level control**:
  ```toml
  [[skills.config]]
  path = "/path/to/skill"
  enabled = true
  ```
- **Activation**: Explicit (`/skills`, `$skill-name`) or auto-selected by the agent

**Key difference from Claude**: Skills live under `.agents/skills/` (not `.codex/skills/`). There's an admin-level `/etc/codex/skills/` path. Skills can be toggled via TOML config.

### Hooks

- **Format**: Standalone JSON file (`hooks.json`)
- **Global path**: `~/.codex/hooks.json`
- **Project path**: `<project>/.codex/hooks.json`
- **Feature flag required**: `codex_hooks = true` in `config.toml`
- **Events** (5):
  | Event | Can block? |
  |-------|-----------|
  | `SessionStart` | No |
  | `PreToolUse` | Yes (approve/block/deny) |
  | `PostToolUse` | Yes (block) |
  | `UserPromptSubmit` | Yes (block) |
  | `Stop` | Yes (causes continuation) |
- **Structure**:
  ```json
  {
    "hooks": {
      "PreToolUse": [{
        "matcher": "Bash",
        "hooks": [{
          "type": "command",
          "command": "/usr/bin/python3 policy.py",
          "timeout": 600,
          "statusMessage": "Checking Bash command"
        }]
      }]
    }
  }
  ```
- **Handler types**: `command` (shell), `prompt` (not yet implemented), `agent` (not yet implemented)
- **Runtime**: Multiple matching hooks execute concurrently. Default timeout: 600s.

**Key difference from Claude**: Codex hooks are standalone files, feature-flagged, and have `PreToolUse` decisions (approve/block/deny with permission semantics).

### Agents (Roles)

- **Format**: TOML files (not Markdown!)
- **Global path**: `~/.codex/agents/<name>.toml`
- **Project path**: `<project>/.codex/agents/<name>.toml`
- **Also declarable** inline in `config.toml` under `[agents.<name>]`
- **Structure**:
  ```toml
  name = "researcher"
  description = "Research-focused agent"
  nickname_candidates = ["Herodotus"]
  model = "gpt-5-pro"
  model_reasoning_effort = "high"
  sandbox_mode = "read-only"
  developer_instructions = "Focus on thorough research..."
  
  [mcp_servers.docs]
  command = "..."
  
  [[skills.config]]
  path = "/path/to/skill"
  enabled = true
  ```
- **Built-in roles**: `default`, `worker`, `explorer`
- **Features**: Per-agent MCP servers, skill configs, model override, sandbox mode

**Key difference from Claude**: Codex agents are TOML (not Markdown), with rich configuration options (model, sandbox, per-agent MCP). This is the most different format from all other tools.

### MCP Servers

- **Format**: TOML (in `config.toml`)
- **Structure**:
  ```toml
  [mcp_servers.my-server]
  command = "node"
  args = ["/path/to/server.js"]
  env = { "API_KEY" = "value" }
  enabled = true
  startup_timeout_sec = 10
  tool_timeout_sec = 60
  ```
- **Transports**: stdio (`command`), HTTP/SSE (`url`), OAuth-capable
- **Per-tool approvals**: `[mcp_servers.name.tools.search] approval_mode = "approve"`
- **HTTP transport extras**: `http_headers`, `bearer_token`, `bearer_token_env_var`

**Key difference from Claude**: MCP is in TOML config (not standalone JSON), with per-tool approval modes and OAuth.

### Other Notable Features

- **Profiles**: Switch entire config sets via `--profile name`
  ```toml
  [profiles.deep-review]
  model = "gpt-5-pro"
  model_reasoning_effort = "high"
  ```
- **Plugins**: Full plugin system bundling skills + MCP + connectors via `plugin.json`
- **Apps/Connectors**: External service integrations (GitHub, Slack, Google Drive)
- **Trust system**: Project configs only load when the project is trusted

### Scope Summary

| Artifact | Global | Project |
|----------|--------|---------|
| Skills   | `~/.agents/skills/` | `.agents/skills/` |
| Hooks    | `~/.codex/hooks.json` | `.codex/hooks.json` |
| Agents   | `~/.codex/agents/*.toml` | `.codex/agents/*.toml` |
| MCP      | In `~/.codex/config.toml` | In `.codex/config.toml` |
| Config   | `~/.codex/config.toml` | `.codex/config.toml` |
| Context  | `~/.codex/AGENTS.md` | `AGENTS.md` (root + subdirs) |

---

## Gemini CLI (Google)

### Skills

- **Format**: Markdown with YAML frontmatter (`SKILL.md`)
- **Discovery locations**:
  | Scope | Path | Precedence |
  |-------|------|------------|
  | Project | `.gemini/skills/` or `.agents/skills/` | Highest |
  | User | `~/.gemini/skills/` or `~/.agents/skills/` | Medium |
  | Extension | Bundled in extensions | Lowest |
- **Frontmatter fields**: `name`, `description`
- **Subdirectories**: `scripts/`, `references/`, `assets/`
- **Activation**: Progressive disclosure -- only metadata loaded at start, full content injected via `activate_skill` tool
- **Management**: `/skills list|link|disable|enable|reload` or `gemini skills install|uninstall|list|link`
- **Install sources**: Git repos, local dirs, `.skill` zip files

**Key difference from Claude**: Gemini supports `.agents/skills/` as an alias (cross-tool compat), has extension-bundled skills, and a built-in skill install/management CLI.

### Hooks

- **Format**: JSON, embedded in `settings.json` under `hooks` key
- **Global**: In `~/.gemini/settings.json`
- **Project**: In `.gemini/settings.json`
- **System/Admin**: In `/etc/gemini-cli/settings.json`
- **Events** (11):
  | Event | Can block? | Purpose |
  |-------|-----------|---------|
  | `SessionStart` | No | Inject context |
  | `SessionEnd` | No | Advisory |
  | `BeforeAgent` | Yes | Block turn, inject context |
  | `AfterAgent` | Yes | Retry / halt |
  | `BeforeModel` | Yes | Block turn, mock response |
  | `AfterModel` | Yes | Block turn, redact |
  | `BeforeToolSelection` | Yes | Filter tools |
  | `BeforeTool` | Yes | Block tool, rewrite args |
  | `AfterTool` | Yes | Block result, add context |
  | `PreCompress` | No | Advisory |
  | `Notification` | No | Advisory |
- **Structure**:
  ```json
  {
    "hooks": {
      "BeforeTool": [{
        "matcher": "write_file|replace",
        "hooks": [{
          "name": "security-check",
          "type": "command",
          "command": ".gemini/hooks/security.sh",
          "timeout": 5000
        }]
      }]
    }
  }
  ```
- **Env vars injected**: `GEMINI_PROJECT_DIR`, `GEMINI_SESSION_ID`, `GEMINI_CWD`, `CLAUDE_PROJECT_DIR` (compat alias)
- **Security**: Project-level hooks are fingerprinted. Changed hooks trigger trust warnings.

**Key difference from Claude**: Gemini has 11 events (including model-level hooks like `BeforeModel`/`AfterModel`), supports project-level hooks (in settings.json), and has a hook fingerprinting/trust system.

### Agents (Subagents)

- **Format**: Markdown with YAML frontmatter
- **Global path**: `~/.gemini/agents/<name>.md`
- **Project path**: `.gemini/agents/<name>.md`
- **Frontmatter fields**:
  ```yaml
  ---
  name: security-auditor
  description: Specialized in finding security vulnerabilities
  kind: local           # or "remote" (A2A protocol)
  tools:
    - read_file
    - grep_search
  mcpServers:
    my-server:
      command: 'node'
      args: ['server.js']
  model: gemini-3-flash-preview
  temperature: 0.2
  max_turns: 10
  timeout_mins: 10
  ---
  ```
- **Tool scoping**: Restrict which tools a subagent can use (wildcards: `*`, `mcp_*`, `mcp_server_*`)
- **Inline MCP**: Subagents can declare their own MCP servers
- **Remote agents**: A2A (Agent-to-Agent) protocol support with `kind: remote`
- **Built-in subagents**: `codebase_investigator`, `cli_help`, `generalist_agent`, `browser_agent`

**Key difference from Claude**: Gemini agents have the richest frontmatter (tool scoping, inline MCP, temperature, max_turns, timeout, remote A2A). Claude agents are plain Markdown with no frontmatter.

### MCP Servers

- **Format**: JSON, embedded in `settings.json` under `mcpServers` key
- **Global**: In `~/.gemini/settings.json`
- **Project**: In `.gemini/settings.json`
- **Structure**:
  ```json
  {
    "mcpServers": {
      "server-name": {
        "command": "node",
        "args": ["server.js"],
        "env": { "API_KEY": "$MY_API_TOKEN" },
        "cwd": "./server-directory",
        "timeout": 30000,
        "trust": false
      }
    }
  }
  ```
- **Transports**: stdio, SSE (`url`), Streamable HTTP (`httpUrl`)
- **Per-server options**: `includeTools`, `excludeTools`, `trust`, `cwd`
- **Global MCP settings**: `mcp.allowed` (whitelist), `mcp.excluded` (blacklist)
- **Resources**: Auto-discovered, referenceable via `@server://resource/path`

**Key difference from Claude**: MCP is in settings.json (not a separate file), supports project-level config, has per-server tool filtering and a trust flag.

### Other Notable Features

- **Extensions**: Full plugin system via `gemini-extension.json` manifest (bundles MCP, skills, hooks, agents, commands)
- **Custom commands**: TOML files in `.gemini/commands/` or `~/.gemini/commands/` (namespaced, with arg/shell/file injection)
- **Policy engine**: TOML files in `.gemini/policies/` for per-tool allow/deny/ask rules
- **System prompt override**: `GEMINI_SYSTEM_MD=true` reads `.gemini/system.md`, fully replacing the default prompt
- **`.geminiignore`**: Exclude files from context discovery
- **Admin lockdown**: System-level settings at `/etc/gemini-cli/settings.json` override everything

### Scope Summary

| Artifact | Global | Project |
|----------|--------|---------|
| Skills   | `~/.gemini/skills/` or `~/.agents/skills/` | `.gemini/skills/` or `.agents/skills/` |
| Hooks    | In `~/.gemini/settings.json` | In `.gemini/settings.json` |
| Agents   | `~/.gemini/agents/*.md` | `.gemini/agents/*.md` |
| MCP      | In `~/.gemini/settings.json` | In `.gemini/settings.json` |
| Config   | `~/.gemini/settings.json` | `.gemini/settings.json` |
| Context  | `~/.gemini/GEMINI.md` | `GEMINI.md` (root + subdirs) |

---

## Cross-Backend Analysis

### Skills Format Convergence

All four tools converge on **Markdown with YAML frontmatter** for skills. The frontmatter fields vary:

| Field | Claude | Cursor | Codex | Gemini |
|-------|--------|--------|-------|--------|
| `name` | No | No | Yes | Yes |
| `description` | Yes (in content) | Yes | Yes | Yes |
| `alwaysApply` | No | Yes | No | No |
| `globs` | No | Yes | No | No |

**Renkei implication**: The neutral skill format (Markdown + frontmatter) can target all backends. Cursor-specific fields (`alwaysApply`, `globs`) need backend-specific frontmatter injection or a manifest-level config.

### Hook Event Mapping

| Renkei abstract event | Claude Code | Cursor | Codex | Gemini |
|----------------------|-------------|--------|-------|--------|
| `before_tool` | `PreToolUse` | `preToolUse` | `PreToolUse` | `BeforeTool` |
| `after_tool` | `PostToolUse` | `postToolUse` | `PostToolUse` | `AfterTool` |
| `session_start` | -- | `sessionStart` | `SessionStart` | `SessionStart` |
| `session_end` | -- | `sessionEnd` | -- | `SessionEnd` |
| `stop` | `Stop` | `stop` | `Stop` | -- |
| `user_prompt` | `UserPromptSubmit` | `beforeSubmitPrompt` | `UserPromptSubmit` | -- |
| `notification` | `Notification` | -- | -- | `Notification` |
| `before_shell` | -- | `beforeShellExecution` | -- | -- |
| `after_shell` | -- | `afterShellExecution` | -- | -- |
| `before_mcp` | `McpToolUse` | `beforeMCPExecution` | -- | -- |
| `after_mcp` | -- | `afterMCPExecution` | -- | -- |
| `before_file_read` | -- | `beforeReadFile` | -- | -- |
| `after_file_edit` | -- | `afterFileEdit` | -- | -- |
| `before_model` | -- | -- | -- | `BeforeModel` |
| `after_model` | -- | -- | -- | `AfterModel` |
| `before_agent` | -- | -- | -- | `BeforeAgent` |
| `after_agent` | -- | `afterAgentResponse` | -- | `AfterAgent` |
| `pre_compact` | -- | `preCompact` | -- | `PreCompress` |
| `subagent_start` | -- | `subagentStart` | -- | -- |
| `subagent_stop` | -- | `subagentStop` | -- | -- |

**Renkei implication**: Only `before_tool` and `after_tool` are universal. A hook using a Cursor-only event should warn when installed on Claude/Codex/Gemini.

### Hook File Location

| Tool | Location | Standalone file? | Project-level? |
|------|----------|-----------------|----------------|
| Claude Code | `~/.claude/settings.json` (merged) | No | No (always global) |
| Cursor | `.cursor/hooks.json` | Yes | Yes |
| Codex | `.codex/hooks.json` | Yes | Yes |
| Gemini CLI | `.gemini/settings.json` (merged) | No | Yes |

**Renkei implication**: Two deployment strategies needed -- merge-into-settings (Claude, Gemini) vs write-standalone-file (Cursor, Codex). Project-level hooks are possible everywhere except Claude.

### MCP Configuration

| Tool | File | Format | Project-level? | Standalone file? |
|------|------|--------|---------------|-----------------|
| Claude Code | `~/.claude.json` | JSON | No | Yes |
| Cursor | `.cursor/mcp.json` | JSON | Yes | Yes |
| Codex | `config.toml` | TOML | Yes | No (embedded) |
| Gemini CLI | `settings.json` | JSON | Yes | No (embedded) |

**Renkei implication**: Three MCP deployment strategies: standalone JSON (Claude, Cursor), embedded in TOML config (Codex), embedded in JSON settings (Gemini).

### Agent Format

| Tool | Format | Location | Frontmatter |
|------|--------|----------|-------------|
| Claude Code | Markdown | `.claude/agents/*.md` | None |
| Cursor | Markdown + YAML | `.cursor/agents/*.md` | `name`, `description`, `model`, `readonly`, `is_background` |
| Codex | **TOML** | `.codex/agents/*.toml` | N/A (TOML is the format) |
| Gemini CLI | Markdown + YAML | `.gemini/agents/*.md` | `name`, `description`, `kind`, `tools`, `mcpServers`, `model`, `temperature`, `max_turns` |

**Renkei implication**: Agents need format conversion. Renkei's neutral format (Markdown) works for Claude/Cursor/Gemini but needs TOML conversion for Codex. Rich frontmatter fields are backend-specific.

### Scope: What Can Be Project-Level?

| Artifact | Claude | Cursor | Codex | Gemini |
|----------|--------|--------|-------|--------|
| Skills | Yes | Yes | Yes | Yes |
| Hooks | **No** | Yes | Yes | Yes |
| Agents | Yes | Yes | Yes | Yes |
| MCP | **No** | Yes | Yes | Yes |

**Renkei implication**: Claude is the only tool where hooks and MCP are always global. For other backends, Renkei should support project-level hook/MCP deployment.

---

## Implications for Renkei

### 1. Skill Deployment is Mostly Universal

All tools use `SKILL.md` in a directory. The main differences are:
- **Path prefix**: `.claude/skills/`, `.cursor/rules/`, `.agents/skills/` (Codex), `.gemini/skills/`
- **Frontmatter**: Cursor expects `.mdc` extension and `alwaysApply`/`globs` fields
- **Action**: Renkei can deploy the same skill content with backend-specific path and optional frontmatter enrichment

### 2. Hooks Need a Strategy Matrix

Two patterns exist:
- **Merge-into-settings**: Claude (`settings.json`), Gemini (`settings.json`)
- **Standalone file**: Cursor (`hooks.json`), Codex (`hooks.json`)

Event names differ per tool. Renkei's abstract event mapping is correct but needs expansion for the 20+ events across tools.

### 3. MCP Has Three Patterns

- **Standalone JSON**: Claude (`~/.claude.json`), Cursor (`.cursor/mcp.json`)
- **Embedded in JSON settings**: Gemini (`settings.json`)
- **Embedded in TOML config**: Codex (`config.toml`)

### 4. Agents Need Format Conversion for Codex

Claude/Cursor/Gemini all use Markdown. Codex uses TOML. A Renkei neutral agent format (Markdown) would need a Codex-specific serializer.

### 5. Scope Model Varies

Claude is unique in forcing hooks/MCP to global scope. All other tools support project-level hooks and MCP. When implementing new backends, `Backend::deploy_hook()` and `Backend::register_mcp()` should accept a scope parameter rather than hardcoding global-only.

### 6. Cross-Tool Compatibility Features

Some tools already read each other's directories:
- Cursor reads `.claude/agents/` and `.codex/agents/`
- Gemini reads `.agents/skills/` (Codex convention)
- Codex uses `.agents/skills/` as a shared namespace

This means deploying to `.agents/skills/` could serve both Codex and Gemini simultaneously.
