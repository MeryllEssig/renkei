# PRD — Renkei: Package Manager for Agentic Workflows

## Problem Statement

Developers and teams using AI tools (Claude Code, Cursor, Codex, etc.) produce complex agentic workflows: skills, hooks, specialized agents, MCP configurations, scripts. These artifacts are currently:

- **unversioned**: no version tracking, impossible to know which version is running in production;
- **not easily shareable**: manual copy-paste between machines, Slack, email, drive;
- **not portable**: each developer manually reconfigures for each AI tool;
- **invisible**: no list of installed workflows, no health diagnostics;
- **fragile**: a local modification to a skill silently breaks the workflow.

There is no standard primitive for distributing a "complete workflow" (skill + hook + agent + MCP config) the way you distribute an npm package or a homebrew plugin.

---

## Solution

**Renkei** is a CLI package manager (`rk`) written in Rust that lets you install, version, and share agentic workflows. A workflow is a **package**: a folder with a `renkei.json` manifest describing its artifacts. The CLI deploys each artifact to the correct location based on the detected backend (Claude Code, Cursor...), with no manual configuration.

```
rk install git@github.com:meryll/mr-review
rk install ./my-workflow/
rk list
rk doctor
```

Renkei is **content-agnostic** — it doesn't execute packages, it distributes them.

---

## User Stories

### Installation and deployment

1. As a developer, I want to install a workflow from a Git repo (SSH) so I can use it immediately in my AI tool without manual configuration.
2. As a developer, I want to install a workflow from a Git repo (HTTPS) so I can do it from an environment without SSH keys.
3. As a developer, I want to install a specific version of a workflow via a Git tag (`--tag v1.2.0`) to guarantee reproducibility of my environment.
4. As a developer, I want to install a workflow from a local folder (relative or absolute path) to test a package in development without publishing it.
5. As a developer, I want `rk install` to validate the `renkei.json` manifest before any deployment so it fails early on invalid configuration.
6. As a developer, I want `rk install` to automatically detect the installed backend (Claude Code, Cursor) so it only deploys where relevant.
7. As a developer, I want to be warned if the package doesn't support my backend before installation to avoid partial or inconsistent deployment.
8. As a developer, I want to force-install an incompatible package via `--force` to install it despite the declared incompatibility, at my own risk.
9. As a developer, I want skills to be deployed under `~/.claude/skills/renkei-<name>/` so they are isolated from native skills and easily identifiable.
10. As a developer, I want hooks to be merged into `~/.claude/settings.json` so they activate automatically in Claude Code.
11. As a developer, I want agents to be deployed in `~/.claude/agents/` so they are available directly from Claude Code.
12. As a developer, I want MCP configurations declared in `renkei.json` to be registered in `~/.claude.json` to automatically activate the required MCP servers.
13. As a developer, I want to see the list of missing required environment variables after installation so I can configure them without digging through documentation.
14. As a developer, I want to re-run `rk install` on an already-installed package to update the deployed artifacts to the new version.

### Conflict management

15. As a developer, I want to be alerted if two packages deploy a skill with the same name to avoid silent overwrites.
16. As a developer, I want to rename a conflicting skill via an interactive prompt to keep both packages side by side.
17. As a developer, I want the rename to update the `name` field in the skill's frontmatter so the reference stays consistent.
18. As a developer, I want the original-name → deployed-name mapping to be persisted in the local cache so `doctor` and `list` commands remain accurate after renaming.

### Listing and visibility

19. As a developer, I want to list all installed packages with their versions and sources (`rk list`) to get an overview of my environment.
20. As a developer, I want to distinguish Git-installed packages from locally-installed ones in `rk list` to know which can be updated automatically.

### Diagnostics

21. As a developer, I want to diagnose the state of my installed packages (`rk doctor`) to detect problems without manually inspecting files.
22. As a developer, I want `rk doctor` to flag locally modified skills so I know which ones have diverged from the original.
23. As a developer, I want `rk doctor` to list missing required environment variables per package so I can quickly fix configuration gaps.
24. As a developer, I want `rk doctor` to check for the presence of backends (Claude Code, Cursor) to confirm that artifacts have a runtime.
25. As a developer, I want a non-zero exit code when `rk doctor` detects problems so I can integrate it into CI scripts.

### Package creation

26. As a package creator, I want to validate that all files declared in `renkei.json` exist (`rk package`) to avoid distributing a broken package.
27. As a package creator, I want to generate a `<name>-<version>.tar.gz` archive of my package for easy distribution.
28. As a package creator, I want to auto-bump the version (patch / minor / major) via `--bump` to follow semver without manually editing the manifest.
29. As a package creator, I want to see a summary of included files and archive size after `rk package` to verify the contents before distribution.

### Lockfile

30. As a developer, I want a `rk.lock` lockfile to be automatically generated at the project root after each installation to pin the exact installed versions in this project context.
31. As a team member, I want to commit `rk.lock` to the project repo so the rest of the team works with the same workflow versions.
32. As a new team member, I want to clone the project and run `rk install` (no arguments) to immediately get the same workflows as the rest of the team, with no additional configuration.
33. As a developer, I want `rk install` without arguments to read `rk.lock` and install the exact declared versions to reproduce the environment identically.
34. As a developer, I want the lockfile to include integrity (SHA-256 hash) of each package to detect any corruption or tampering.

### Phase 1 — Delivery and migration

35. As a project maintainer, I want the CLI to be compiled into native binaries for Linux / macOS / Windows and automatically published via GitHub Actions on each release so users can install it without dependencies.
36. As a package creator, I want to migrate existing workflows (renkei-old) into valid Renkei packages to validate the `renkei.json` format on real cases from v1.

### Phase 2 — Registry and advanced commands

37. As a package creator, I want to publish my package to a centralized registry (`rk publish`) to make it discoverable by other teams.
38. As a developer, I want to search for packages in the registry (`rk search <query>`) to find existing workflows without manually browsing repos.
39. As a developer, I want to install a package by its scoped name (`rk install @scope/name`) without needing to know the Git URL.
40. As a developer, I want to update a package to its latest compatible version (`rk update`) to benefit from improvements without reinstalling manually.
41. As a developer, I want to uninstall a package and clean up all its deployed artifacts (`rk uninstall`) to leave no residuals.
42. As a developer, I want to get package details (description, author, versions, dependencies) via `rk info` to evaluate it before installation.
43. As a package creator, I want to interactively scaffold a new package (`rk init`) to start with a valid structure without writing it from scratch.
44. As a developer, I want to see the diff between deployed artifacts and the original archive (`rk diff`) to audit my local modifications.
45. As a developer, I want to restore a package's artifacts from the original archive (`rk reset`) to undo my local modifications.
46. As a package creator, I want to fork an existing package under my scope (`rk fork --scope <s>`) to create an independent variant without modifying the original.
47. As a user, I want to authenticate with the registry (`rk login` / `rk logout`) to publish under my scope.
48. As a developer, I want Cursor packages to be deployed in `.cursor/skills/<name>/` to use my workflows in Cursor without configuration.
49. As a package creator, I want to declare an organizational scope (`@acme-corp/`) to avoid name collisions between teams.

### Phase 3 — Ecosystem

50. As a developer, I want to browse available packages on a public website to discover workflows without the CLI.
51. As a package creator, I want a public profile displaying my published packages to build my reputation in the ecosystem.
52. As an organization, I want a private registry under my scope to distribute internal workflows without exposing them publicly.
53. As a developer, I want to auto-update the CLI (`rk self-update`) to always have the latest fixes.
54. As an admin, I want access to installation statistics for my packages to measure their adoption.

---

## Implementation Decisions

### Language and distribution
- CLI written in **Rust**: native binary, zero runtime dependencies.
- Cross-compilation for Linux / macOS / Windows via GitHub Actions.
- Distribution via **GitHub Releases** — a single executable file.
- Open source license for the CLI; the registry website will be closed source.

### Workspace

A Git repo can contain multiple packages (workspace). Each sub-package lives in a folder at the root (`./mr-review/`, `./auto-test/`). A root `renkei.json` declares members via a `workspace` field:

```json
{
  "workspace": ["mr-review", "auto-test"]
}
```

Each subfolder contains its own complete `renkei.json` and its conventional directories.

For a repo without a workspace (single package), the conventional directories (`skills/`, `hooks/`, `agents/`) are directly at the root.

### Manifest `renkei.json`
- Required fields: `name` (scoped `@scope/name`, **required from v1**), `version` (semver), `description`, `author`, `license`, `backends`.
- Optional fields: `keywords`, `mcp`, `requiredEnv`, `workspace`.
- **No `artifacts` field**: pure convention. The `skills/`, `hooks/`, `agents/` directories are the source of truth. Any file present in these directories is a deployed artifact.
- `mcp` declares MCP configurations in the native `command`/`args`/`env` format (standard between Claude and Cursor, no extra abstraction).
- `requiredEnv` lists environment variables with their descriptions.

```json
{
  "name": "@meryll/mr-review",
  "version": "1.2.0",
  "description": "Automated code review",
  "author": "meryll",
  "license": "MIT",
  "backends": ["claude"],
  "mcp": {
    "my-server": {
      "command": "node",
      "args": ["server.js"],
      "env": { "API_KEY": "${API_KEY}" }
    }
  },
  "requiredEnv": {
    "GITHUB_TOKEN": "Required for GitHub API access"
  }
}
```

### Neutral artifact format

All artifacts are written in a neutral Renkei format that each backend translates:

- **Skills and agents**: markdown + frontmatter format (Claude Code style). This format is the Renkei neutral format — other backends translate from it.
- **Hooks**: abstract Renkei format with normalized events (see Hooks section below).
- **MCP**: native `command`/`args`/`env` format directly in the manifest (already portable across backends).

```markdown
---
name: review
description: Review code changes
---
Review the code...
```

### Hooks: format and events

Files in `hooks/*.json` use an abstract Renkei format with normalized events. Each backend maps these events to its own native events.

```json
[
  {
    "event": "before_tool",
    "matcher": "bash",
    "command": "bash scripts/lint.sh",
    "timeout": 5
  }
]
```

**Renkei → Claude Code event mapping:**

| Renkei Event | Claude Code |
|-------------|-------------|
| `before_tool` | `PreToolUse` |
| `after_tool` | `PostToolUse` |
| `after_tool_failure` | `PostToolUseFailure` |
| `on_notification` | `Notification` |
| `on_session_start` | `SessionStart` |
| `on_session_end` | `SessionEnd` |
| `on_stop` | `Stop` |
| `on_stop_failure` | `StopFailure` |
| `on_subagent_start` | `SubagentStart` |
| `on_subagent_stop` | `SubagentStop` |
| `on_elicitation` | `Elicitation` |

This mapping is maintained in `ClaudeBackend`. Other backends will define their own mapping.

**Tracking**: deployed hooks are tracked in `~/.renkei/install-cache.json`, not in the backend's JSON. The backend JSON (`settings.json`, etc.) stays 100% native with no custom fields. On uninstall, Renkei compares with its cache to remove the right entries.

### Backend interface
- A `Backend` trait defines operations: `name`, `detect_installed`, `deploy_skill`, `deploy_hook`, `deploy_agent`, `register_mcp`.
- **Everything backend-specific must be abstracted** behind this interface.
- `ClaudeBackend` is the only implementation in v1. `CursorBackend` will be added in v2 without refactoring.
- **Detection**: a backend is considered installed if its config directory exists (`~/.claude/` for Claude, `.cursor/` for Cursor). No binary check in PATH.

### Multi-backend support matrix

| Artifact   | Claude Code              | Cursor               | Codex      | Gemini |
|------------|--------------------------|----------------------|------------|--------|
| Skills     | `SKILL.md`               | Skills               | `AGENTS.md`| ?      |
| Hooks      | `settings.json` events   | N/A                  | N/A        | N/A    |
| Agents     | `agents/*.md`            | N/A                  | N/A        | N/A    |
| MCP config | `~/.claude.json`         | `.cursor/mcp.json`   | ?          | ?      |

Codex and Gemini are on the radar but not planned. Artifact format varies by backend (`AGENTS.md` for Codex vs `agents/*.md` for Claude).

### Deployment conventions (hardcoded, not configurable)

| Artifact  | Claude Code                              | Cursor                       |
|-----------|------------------------------------------|------------------------------|
| Skills    | `~/.claude/skills/renkei-<name>/SKILL.md` | `.cursor/skills/<name>/`     |
| Hooks     | Merge into `~/.claude/settings.json`     | N/A                          |
| Agents    | `~/.claude/agents/<name>.md`             | N/A                          |
| MCP config| Merge into `~/.claude.json`              | Merge into `.cursor/mcp.json` |

The `renkei-` prefix on skills creates a clear namespace and avoids collisions with native skills.

### Installation: Git

1. `git clone --depth 1` into a temp directory (`/tmp/rk-xxx/`)
2. Validate the `renkei.json` manifest
3. Create the `.tar.gz` archive in `~/.renkei/cache/@scope/name/<version>.tar.gz`
4. Deploy artifacts from the archive
5. Delete the temp clone

Without `--tag` or `--branch`, HEAD of the default branch is used. The commit SHA is recorded in the lockfile for reproducibility. The version in `renkei.json` is authoritative (trust the manifest) — no consistency check against Git tags.

### Installation: local

- `rk install ./my-workflow/` creates a **copy** (snapshot archive in cache), same as Git.
- `rk install --link ./my-workflow/` creates **symlinks** for development (`npm link` / `pip install -e` model). Changes in source files are immediately reflected.

### Installation: no arguments

- If `rk.lock` exists in the current directory → installs the exact versions from the lockfile.
- If no lockfile but workspace detected → explicit error: "workspace detected, use `rk install --link .` for dev".

### Error handling: fail-fast + rollback

On the first error during installation, immediate stop and rollback of all already-deployed artifacts. Guaranteed atomicity: either everything succeeds, or nothing changes.

### Conflict management
- Detection via `install-cache.json` before any deployment.
- **TTY (interactive)**: prompt to rename the conflicting artifact. Renaming updates the `name` field in the skill's frontmatter.
- **Non-TTY (CI)**: error with exit code 1.
- **`--force`**: last installed silently overwrites.
- The original-name → deployed-name mapping is persisted in `install-cache.json`.

### Environment variables

Missing required environment variables trigger a **warning** after installation, not a blocker. `rk doctor` re-checks them. The user configures after installation.

### Local storage
- `~/.renkei/cache/@scope/name/<version>.tar.gz` — immutable archives per version.
- `~/.renkei/install-cache.json` — mapping of packages → deployed artifacts + tracked hooks + renames.
- `~/.renkei/config.json` — local configuration (registries, preferences).
- `rk.lock` at the project root — committable lockfile per project.

### Lockfile
- Versioned JSON format (`lockfileVersion: 1`).
- Each entry: `version`, `source`, `tag` (optional), `resolved` (commit SHA), `integrity` (sha256).
- Automatically generated by `rk install`, committable to the repo.

```json
{
  "lockfileVersion": 1,
  "packages": {
    "@meryll/mr-review": {
      "version": "1.2.0",
      "source": "git@github.com:meryll/mr-review",
      "tag": "v1.2.0",
      "resolved": "abc123def",
      "integrity": "sha256-..."
    }
  }
}
```

### Diagnostics (`rk doctor`)

v1 checks:
- Installed backends (config directory exists)
- Deployed files still exist
- Required environment variables present
- Locally modified skills (SHA-256 hash diff against cached archive)
- Hooks still present in the backend's config file
- MCP configs still registered

No remote version check (registry v2). Exit code 0 if everything passes, non-0 otherwise.

### Archive (`rk package`)

The `.tar.gz` archive contains only:
- `renkei.json`
- `skills/`
- `hooks/`
- `agents/`
- `scripts/`

Everything else (tests, docs, README, etc.) is excluded.

### Registry v2
- HTTP service: index `@scope/name` → source URL + metadata.
- `rk publish` sends the archive + updates the index.
- Scopes: `@renkei/` reserved for official packages, others are registered on first publish.
- Auth via API token.

---

## Testing Decisions

**Principle**: only test externally observable behavior, not internal implementation details. A good test verifies what the CLI does (files created, correct content, exit code, displayed messages) — not how it does it.

**Modules to test:**

- **Manifest parsing**: validate that a valid `renkei.json` is accepted, that missing required fields produce a descriptive error, that incorrect types are rejected, that the `@scope/name` scope is required.
- **Convention-based artifact discovery**: verify that files in `skills/`, `hooks/`, `agents/` are correctly detected as artifacts.
- **Artifact deployment** (`ClaudeBackend`): verify that files are copied to the correct paths after `rk install`, that the merge into `settings.json` and `~/.claude.json` is correct, that the `renkei-` prefix is applied.
- **Hook translation**: verify that the abstract Renkei format (`before_tool`, etc.) is correctly translated into native Claude Code events (`PreToolUse`, etc.).
- **Hook tracking**: verify that deployed hooks are recorded in `install-cache.json` and that rollback removes them correctly.
- **Lockfile**: verify that `rk.lock` is created with the correct versions and hashes, that `rk install` without arguments installs the exact lockfile versions.
- **Backend detection**: verify that `ClaudeBackend::detect_installed` returns true when Claude Code is present.
- **Conflict management**: verify collision detection and renaming in `install-cache.json`.
- **`rk doctor`**: verify exit codes (0 = healthy, non-0 = problems), detection of modified skills, missing env vars.
- **`rk package`**: verify archive creation, version bump in `renkei.json`, rejection when declared artifacts are missing.

---

## Out of Scope

- **Workflow runtime / executor**: Renkei distributes workflows, it doesn't execute them.
- **MCP orchestrator**: MCP server lifecycle management is left to the AI tool.
- **Pattern library / agentic pattern framework**: Renkei is content-agnostic.
- **Workflow → skill compiler**: no transformation of package contents.
- **Inter-workflow dependency system**: a package cannot declare dependencies on other packages (v1).
- **Observability / execution metrics**: out of scope.
- **Local GUI**: the CLI is the only entry point.

---

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| **Over-engineering** | High | Minimal scope in v1. Every feature justified by a concrete need. |
| **Zero adoption** | High | Validate with early users (team members) before investing in the registry. |
| **Rapid evolution of AI tools** | Medium | The `Backend` interface isolates from change. A single adaptation point per tool. |
| **Unstable skill format** | Medium | Monitor Claude Code / Cursor changelogs. Adapt quickly. |
| **Rust learning curve** | Low | The CLI scope is well-defined. No concurrency, no complex async. |
| **Native competition** | Low | Renkei is multi-tool and workflow-oriented, not component-oriented. Complementary to native stores. |

---

## Licensing

| Component | License |
|-----------|---------|
| CLI `rk` | Open source |
| Registry website | Closed source |
| Individual packages | Creator's choice |
| `@renkei/` scope | Reserved for official packages |

---

## Further Notes

- **Clean break**: the existing codebase (`renkei-old`) serves as reference but the new Renkei starts from scratch in Rust. Existing workflows will be packaged as Renkei packages once the CLI v1 is functional — this is a Phase 1 deliverable.
- **Convention over config**: deployment destinations are hardcoded. Adding a `destination` field to the manifest is explicitly rejected — less error surface, fewer decisions for the package creator.
- **Claude-first**: in v1, only `ClaudeBackend` is implemented. The `Backend` interface is the only concession to future flexibility.
- **Early user validation**: before investing in the registry (v2), validate adoption with users. If nobody installs packages, the registry is premature.
- **The website (v3) should only be built if the ecosystem justifies it** — no speculative builds.
- **Scripts in packages**: the package structure can include a `scripts/` directory with arbitrary scripts. These scripts are not a named artifact type in `artifacts` — they are included in the archive but their deployment is not natively managed by `rk`. This behavior will need to be clarified during `rk package` implementation.
