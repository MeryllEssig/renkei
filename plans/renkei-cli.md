# Plan: Renkei CLI (`rk`)

> Source PRD: `./PRD.md` — Package Manager for Agentic Workflows

## Architectural decisions

Durable decisions that apply across all phases:

- **Language**: Rust, single binary `rk`
- **Backend trait**: `Backend` with methods `name()`, `detect_installed()`, `deploy_skill()`, `deploy_hook()`, `deploy_agent()`, `register_mcp()`. Only `ClaudeBackend` in v1.
- **Manifest**: `renkei.json` — required fields: `name` (scoped `@scope/name`), `version` (semver), `description`, `author`, `license`, `backends`. Optional: `keywords`, `mcp`, `requiredEnv`, `workspace`, `scope`.
- **Scope field**: `scope` in `renkei.json` — `"any"` (default), `"global"` (only `-g`), `"project"` (only without `-g`). Controls where a package can be installed.
- **Installation scope**: project scope by default (`rk install`), global scope with `-g` (`rk install -g`). In project scope, skills/agents deploy to `.claude/` at the project root; hooks/MCP always deploy globally to `~/.claude/`. The `Config` struct absorbs the scope — the backend is agnostic.
- **Convention over config**: artifacts discovered from `skills/`, `hooks/`, `agents/`. No `artifacts` field in the manifest.
- **Deployment paths (hardcoded)**:
  - Skills → `~/.claude/skills/renkei-<name>/SKILL.md` (global) or `.claude/skills/renkei-<name>/SKILL.md` (project)
  - Hooks → merge into `~/.claude/settings.json` (always global)
  - Agents → `~/.claude/agents/<name>.md` (global) or `.claude/agents/<name>.md` (project)
  - MCP → merge into `~/.claude.json` (always global)
- **Local storage**:
  - `~/.renkei/archives/@scope/name/<version>.tar.gz` (immutable archives)
  - `~/.renkei/install-cache.json` (global install-cache: packages installed with `-g`)
  - `~/.renkei/projects/<slug>/install-cache.json` (per-project install-cache, slug = slugified absolute path)
  - `~/.renkei/rk.lock` (global lockfile)
  - `rk.lock` at the project root (committable project lockfile)
- **Injectable home directory**: every function reading/writing `~/.claude/` or `~/.renkei/` accepts a configurable base path (`Config` struct with `home_dir: PathBuf`) to enable testing in a tempdir. `Config::for_project(project_root)` redirects skill/agent paths to the project's `.claude/`.
- **Hook tracking**: in `install-cache.json`, never in the backend's JSON. The backend JSON stays 100% native.
- **Fail-fast + rollback**: every installation is atomic. Writes are collected in a `Vec`, rolled back in reverse order on error.
- **Main crates**: `clap` (derive), `serde` + `serde_json`, `semver`, `tar` + `flate2`, `sha2`, `tempfile`, `thiserror`, `inquire`, `owo-colors`, `etcetera`. Dev: `assert_cmd`, `predicates`. If you need to install another crate, please make sure it's well supported. Don't add too much crates if you can do otherwise.

---

## Phase 1: CLI skeleton + Manifest + Local skill deployment

**User stories**: 4, 5, 9

### What to build

The thinnest possible tracer bullet: `rk install ./local-path/` deploys a single skill from a local folder to Claude Code.

Covers end-to-end: CLI parsing (clap) → reading and validating the `renkei.json` manifest → discovering skills by convention (`skills/`) → creating the `.tar.gz` archive in cache → deploying the skill to `~/.claude/skills/renkei-<name>/SKILL.md` → writing `install-cache.json`.

Rust project structure created from scratch: `Cargo.toml`, `src/main.rs`, modules for manifest, artifact, backend, cache, install, error.

### Acceptance criteria

- [x] `cargo build` produces an `rk` binary
- [x] `rk install ./fixture/` with a valid `renkei.json` and `skills/review.md` deploys the file to `~/.claude/skills/renkei-review/SKILL.md`
- [x] `rk install ./fixture/` with an invalid manifest (missing field, incorrect scope, invalid semver) fails with a descriptive error message
- [x] The archive `~/.renkei/cache/@scope/name/<version>.tar.gz` is created
- [x] `install-cache.json` contains the package entry with deployed paths
- [x] Unit tests: manifest parsing (valid, missing fields, bad scope, bad semver), artifact discovery, skill deployment
- [x] Integration test: `rk install ./fixture/` end-to-end in a tempdir

---

## Phase 2: Rollback + Agents + Reinstall

**User stories**: 11, 14

### What to build

Add atomic rollback mechanism: during installation, every filesystem write is recorded. On error, all writes are undone in reverse order.

Agent support: discovery from `agents/`, deployment to `~/.claude/agents/<name>.md`.

Reinstall support: if a package is already in `install-cache.json`, its old artifacts are removed before redeploying the new version.

### Acceptance criteria

- [x] Installing a package with 2 skills and 1 agent deploys all 3 files to the correct paths
- [x] If an artifact fails during deployment, all already-deployed artifacts are removed (rollback)
- [x] `rk install` on an already-installed package removes old artifacts and deploys new ones
- [x] `install-cache.json` is updated correctly after reinstall
- [x] Tests: rollback (deploy 2/3, error on 3rd, assert first 2 removed), multi-skill, agent deploy, reinstall

---

## Phase 3: Hooks — deployment + event translation

**User stories**: 10

### What to build

Hook deployment: discovery from `hooks/*.json`, parsing the abstract Renkei format (`event`, `matcher`, `command`, `timeout`), translation to native Claude Code events (`before_tool` → `PreToolUse`, etc.), merge into `~/.claude/settings.json`.

The merge into `settings.json` must respect the actual structure: each event key maps to an array of objects `[{ "matcher": "...", "hooks": [{ "type": "command", "command": "...", "timeout": N }] }]`. The merge appends without overwriting existing hooks.

Hook tracking in `install-cache.json`. Extended rollback to remove hooks from settings.json on error.

### Acceptance criteria

- [x] All 11 Renkei events are correctly translated (`before_tool` → `PreToolUse`, `after_tool` → `PostToolUse`, etc.)
- [x] `rk install` of a package with hooks merges entries into `settings.json`
- [x] Existing hooks in `settings.json` are not overwritten
- [x] Rollback removes only the failing package's hooks
- [x] `install-cache.json` tracks which hooks belong to which package
- [x] Tests: translation of all 11 events, hook JSON parsing, settings.json merge (empty, existing, append), hook rollback

---

## Phase 4: MCP + Environment variable warnings

**User stories**: 12, 13

### What to build

MCP registration: read the `mcp` field from the manifest, merge into `~/.claude.json` (`mcpServers` section). Track in `install-cache.json`. MCP rollback.

Environment variable checking: after successful installation, each variable from `requiredEnv` is checked. Warning displayed (not blocking) for missing variables.

### Acceptance criteria

- [x] `rk install` of a package with `mcp` registers the servers in `~/.claude.json`
- [x] Existing MCP servers are not overwritten
- [x] Missing environment variables trigger a warning (not an error)
- [x] Present variables do not generate a warning
- [x] Rollback removes MCP servers from the failing package
- [x] Tests: claude.json merge, MCP tracking, env var checking

---

## Phase 5: Installation scope (global vs project)

**User stories**: 14b, 14c, 14d, 14e, 14f, 14g

### What to build

Add support for two installation scopes: **project** (default) and **global** (`-g` / `--global`).

**Scope in the manifest**: parse the optional `scope` field from `renkei.json` (`"any"` default, `"global"`, `"project"`). Validate scope compatibility at install time — error if the manifest's scope conflicts with the requested install scope (see validation matrix in PRD [scope.md](../doc/prd/scope.md)).

**Project root detection**: detect the git root via `git rev-parse --show-toplevel`. If not inside a git repo and `-g` not specified → error with guidance.

**Config adaptation**: add `Config::for_project(project_root: PathBuf)` that redirects skill/agent deployment paths to `<project_root>/.claude/` while keeping hook/MCP paths pointing to `~/.claude/`. The existing `Config::with_home_dir()` continues to serve global scope. The backend trait methods remain unchanged — they follow the paths from `Config`.

**Dual install-cache**: the global install-cache stays at `~/.renkei/install-cache.json`. Per-project install-caches are stored at `~/.renkei/projects/<slug>/install-cache.json` where `<slug>` is the slugified absolute path of the project root (e.g., `/Users/meryll/Projects/foo` → `Users-meryll-Projects-foo`).

**Storage migration**: rename `~/.renkei/cache/` to `~/.renkei/archives/`. Update all code referencing the old path.

**CLI flag**: add `-g` / `--global` flag to `rk install`. Default is project scope.

### Acceptance criteria

- [ ] `rk install ./fixture/` deploys skills/agents to `.claude/` at the project root
- [ ] `rk install ./fixture/` deploys hooks/MCP to `~/.claude/` (global) even in project scope
- [ ] `rk install -g ./fixture/` deploys everything to `~/.claude/`
- [ ] Project install-cache is written to `~/.renkei/projects/<slug>/install-cache.json`
- [ ] Global install-cache is written to `~/.renkei/install-cache.json`
- [ ] `rk install` outside a git repo (without `-g`) → error with guidance message
- [ ] Manifest with `scope: "global"` + `rk install` (no `-g`) → error
- [ ] Manifest with `scope: "project"` + `rk install -g` → error
- [ ] Manifest with `scope: "any"` works with both `-g` and without
- [ ] `~/.renkei/cache/` renamed to `~/.renkei/archives/` — all archive operations use the new path
- [ ] Reinstall in project scope correctly cleans up old project-scoped artifacts
- [ ] Tests: Config::for_project paths, scope validation matrix, project root detection (git / no git), slug generation, dual install-cache (project + global), storage path migration, end-to-end project-scope install, end-to-end global install

---

## Phase 6: Git installation (SSH, HTTPS, tags) + Backend detection

**User stories**: 1, 2, 3, 6, 7, 8

### What to build

Source parsing: distinguish local path vs Git SSH (`git@...`) vs Git HTTPS (`https://...`). Run `git clone --depth 1` in a tempdir, with `--tag` / `--branch` support. After cloning, delegate to the existing installation pipeline. Clean up the tempdir in all cases.

Backend detection: `ClaudeBackend::detect_installed()` checks for the existence of `~/.claude/`. Before installation, verify that the package's `backends` match the detected backends. Error if incompatible, unless `--force` is used.

Extract the commit SHA from the clone for future use (lockfile).

### Acceptance criteria

- [ ] `rk install git@github.com:user/repo` clones and installs
- [ ] `rk install https://github.com/user/repo` clones and installs
- [ ] `rk install git@... --tag v1.0.0` clones the specific tag
- [ ] The tempdir is cleaned up after installation (success or failure)
- [ ] A package with `backends: ["cursor"]` on a machine without Cursor fails with a clear message
- [ ] `--force` allows installation despite backend incompatibility
- [ ] The commit SHA is extracted and stored
- [ ] Tests: source parsing (SSH, HTTPS, local), backend detection, compatibility, force override

---

## Phase 7: Conflict management + Interactive renaming

**User stories**: 15, 16, 17, 18

### What to build

Before each skill deployment, check `install-cache.json` for whether another package already owns a skill with the same name. In project scope, check the project install-cache; in global scope, check the global install-cache.

Behavior by context:
- **TTY**: interactive prompt (`dialoguer`) to choose a new name
- **Non-TTY**: error with exit code 1
- **`--force`**: silent overwrite

On rename: deploy under the new name (`renkei-<new>/SKILL.md`), update the `name` field in the skill's frontmatter, persist the `original-name → deployed-name` mapping in `install-cache.json`.

### Acceptance criteria

- [ ] Installing 2 packages with a skill of the same name triggers conflict detection
- [ ] In TTY mode, the prompt offers renaming and deploys under the new name
- [ ] In non-TTY mode, error with exit code 1
- [ ] With `--force`, last installed overwrites first
- [ ] The renamed skill's frontmatter contains the new name
- [ ] `install-cache.json` contains the rename mapping
- [ ] Tests: conflict detection, frontmatter rename, mapping persistence

---

## Phase 8: `rk list`

**User stories**: 19, 20

### What to build

`rk list` command: read the project install-cache (default) or global install-cache (`-g`), display a table of all installed packages with name, version, source, scope, and artifact types.

Visual distinction between Git sources (`[git]`) and local sources (`[local]`). Handle the empty case ("No packages installed").

### Acceptance criteria

- [ ] `rk list` displays project-scoped installed packages
- [ ] `rk list -g` displays globally installed packages
- [ ] Git and local packages are visually distinguished
- [ ] With no installed packages, explicit message
- [ ] Exit code 0 in all cases
- [ ] Tests: output formatting, empty case, mixed sources, scope filtering

---

## Phase 9: `rk doctor`

**User stories**: 21, 22, 23, 24, 25

### What to build

`rk doctor` command running a series of health checks:

1. Installed backends (config directory exists)
2. Deployed files still exist (check both project and global paths)
3. Required environment variables present
4. Locally modified skills (SHA-256 hash vs cached archive)
5. Hooks still present in `settings.json`
6. MCP configs still in `~/.claude.json`

Output: checkmark/cross per check, grouped by package. Exit code 0 if healthy, 1 if problems found. By default checks project scope; `-g` checks global scope.

### Acceptance criteria

- [ ] `rk doctor` on a healthy environment returns exit code 0
- [ ] Deleted deployed file → flagged, exit code 1
- [ ] Locally modified skill → flags the modification
- [ ] Missing environment variable → flagged
- [ ] Missing hook in settings.json → flagged
- [ ] Missing MCP in claude.json → flagged
- [ ] Tests: each check individually, exit codes, formatting

---

## Phase 10: Lockfile

**User stories**: 30, 31, 32, 33, 34

### What to build

After each `rk install <source>`, generate/update the lockfile. The lockfile location depends on the scope:
- **Project scope** (`rk install <source>`): write/update `rk.lock` at the project root (detected via git root). This lockfile is committable to the repo for team reproducibility.
- **Global scope** (`rk install -g <source>`): write/update `~/.renkei/rk.lock`.

Both lockfiles use the same JSON format: `lockfileVersion: 1`, packages with `version`, `source`, `tag` (optional), `resolved` (commit SHA), `integrity` (SHA-256 of the archive).

**No-argument install from lockfile**:
- `rk install` (no args): detect `rk.lock` at the project root, read it, reinstall each package in project scope (skills/agents to `.claude/`, hooks/MCP to `~/.claude/`). Use cached archives when available, re-clone at the exact commit otherwise.
- `rk install -g` (no args): detect `~/.renkei/rk.lock`, read it, reinstall each package in global scope.
- No lockfile found → explicit error: "No rk.lock found. Use `rk install <source>` to install a package."

Integrity check: hash of the cached archive vs lockfile hash.

**Important**: a package installed in project scope that includes hooks/MCP (which deploy globally) is still tracked in the project `rk.lock`. The lockfile records *what was installed in this scope*, not *where each artifact physically lives*. When replaying from lockfile, the install pipeline handles routing hooks/MCP to global paths automatically via `Config::for_project`.

### Acceptance criteria

- [ ] `rk install <source>` generates/updates `rk.lock` at the project root
- [ ] `rk install -g <source>` generates/updates `~/.renkei/rk.lock`
- [ ] The lockfile contains version, source, tag, resolved (SHA), integrity (SHA-256)
- [ ] `rk install` (no args) with project `rk.lock` installs in project scope
- [ ] `rk install -g` (no args) with global `rk.lock` installs in global scope
- [ ] Corrupted archive in cache → integrity error
- [ ] `rk install` without args and without `rk.lock` → explicit error
- [ ] Tests: lockfile serialization/deserialization, SHA-256 computation, round-trip install → lockfile → clean → install-from-lockfile (both scopes), integrity check

---

## Phase 11: `rk package`

**User stories**: 26, 27, 28, 29

### What to build

`rk package` command run from a package directory: validate the manifest, scan conventional directories, create a `<name>-<version>.tar.gz` archive containing only `renkei.json`, `skills/`, `hooks/`, `agents/`, `scripts/`.

`--bump patch|minor|major` flag: increment the version in `renkei.json` before archiving, rewrite the manifest.

Display summary: list of included files, count, archive size.

### Acceptance criteria

- [ ] `rk package` creates `<name>-<version>.tar.gz` with the correct contents
- [ ] The archive excludes everything except `renkei.json`, `skills/`, `hooks/`, `agents/`, `scripts/`
- [ ] `rk package --bump minor` increments the minor version in `renkei.json`
- [ ] `rk package` in a directory without `renkei.json` → error
- [ ] Summary displayed with file list and size
- [ ] Tests: archive contents, version bump (patch/minor/major), validation, summary

---

## Phase 12: Workspace

**User stories**: workspace support (PRD "Workspace" section)

### What to build

Workspace detection: a root `renkei.json` with a `workspace` field listing member subdirectories. Each member has its own `renkei.json` and conventional directories.

`rk install ./workspace/` installs each member independently. Each member is cached, deployed, and tracked separately.

`rk install` without arguments in a workspace context without `rk.lock` → error with a message guiding toward `rk install --link .`.

### Acceptance criteria

- [ ] `rk install ./workspace/` installs all members listed in the `workspace` field
- [ ] Each member appears independently in `rk list`
- [ ] Each member has its own lockfile entry
- [ ] `rk install` without args in a workspace without lockfile → error with guidance
- [ ] Tests: workspace detection, member enumeration, independent installation, error message

---

## Phase 13: CI/CD + Migration

**User stories**: 35, 36

### What to build

GitHub Actions: release workflow on tag push — cross-compilation matrix (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64). Publish binaries as GitHub Releases.

CI workflow: tests + clippy + fmt on each PR.

`rk migrate <path>` command: scan an existing renkei-old structure, generate a valid `renkei.json`, reorganize files into conventional directories.

### Acceptance criteria

- [ ] Release workflow produces binaries for all 5 targets
- [ ] CI workflow runs tests, clippy, fmt
- [ ] `rk migrate` generates a valid `renkei.json` from the old format
- [ ] The migrated package passes `rk package` without errors
- [ ] Tests: old format → valid new format migration
