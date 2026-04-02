# Plan: Multi-Backend Support

> Source PRD: `PRD.md` + `doc/prd/backends.md` + `doc/prd/multi-backend.md`

## Architectural decisions

- **Backend trait**: Extend existing `Backend` trait in `src/backend/mod.rs` with `reads_agents_skills() -> bool` (default `false`)
- **Backend registry**: New `BackendRegistry` struct holding all 5 backends, with `detect()` and `resolve()` methods
- **Resolution pipeline**: `user_config.backends` (or auto-detect) → intersect `manifest.backends` → filter by `detect_installed()` → final set
- **Install cache v2**: `deployed_artifacts` becomes `deployed: HashMap<String, BackendDeployment>` keyed by backend name; version bumped to 2
- **Config file**: `~/.renkei/config.json` with `{ "defaults": { "backends": [...] } }`
- **Atomic deploy**: Collect all `DeployedArtifact` across all backends, rollback all on any error
- **Deduplication**: If `agents` backend deploys a skill AND another backend has `reads_agents_skills() == true`, skip that backend's skill deploy
- **Detection**: `AgentsBackend::detect_installed()` always returns `true`

---

## Phase 1: Multi-Backend Infrastructure + Agents Backend + Cache v2

**User stories**: US 56, 57, 59, 60, 61, 64, 65

### What to build

Transform the single-backend system into a multi-backend architecture. This is the foundational phase that makes everything else possible.

**1a. Backend registry + resolution pipeline** (`src/backend/mod.rs`)
- Add `BackendRegistry` with `all()`, `detect()`, `resolve()` methods
- Resolution: auto-detect all installed backends → intersect with manifest's `backends` field → warn per non-detected backend (US 59) → error if final set empty
- `--force` bypasses manifest intersection but NOT detection filter (US 60)

**1b. Agents backend** (`src/backend/agents.rs` — new)
- `name()` → `"agents"`
- `detect_installed()` → always `true` (US 61)
- `deploy_skill()` → writes to `.agents/skills/renkei-{name}/SKILL.md` (global: `~/.agents/`, project: `{root}/.agents/`)
- `deploy_agent/hook/register_mcp` → return unsupported error (skills-only backend)

**1c. Multi-backend deploy loop** (`src/install.rs`)
- Change `install_local` signature: `backend: &dyn Backend` → `backends: &[&dyn Backend]`
- Loop over resolved backends, deploy all artifacts to each
- Collect all `DeployedArtifact` with backend name for rollback + cache
- Rollback spans all backends (US 57 atomicity)
- Update `cleanup_previous_installation` to iterate per-backend in cache

**1d. Install cache v2 + migration** (`src/install_cache.rs`)
- New struct: `PackageEntry.deployed: HashMap<String, BackendDeployment>` replacing flat `deployed_artifacts`
- `BackendDeployment { artifacts: Vec<DeployedArtifactEntry>, mcp_servers: Vec<String> }`
- `InstallCache::load()`: if `version == 1`, migrate all entries under `"claude"` key, save as v2 (US 65)
- Version field → 2

**1e. Config paths for all backends** (`src/config.rs`)
- Add: `agents_dir()`, `cursor_dir()`, `codex_dir()`, `gemini_dir()` path methods
- Agents: `.agents/` relative to scope (home or project root)

**1f. Update all consumers** (`src/main.rs`, `src/uninstall.rs`, `src/doctor.rs`, `src/list.rs`, `src/lockfile.rs`, `src/workspace.rs`)
- `main.rs`: Replace `let backend = ClaudeBackend;` with `BackendRegistry::all()`, wire through resolution
- `uninstall.rs`: Iterate all backend keys in cache entry to cleanup
- `doctor.rs`: Per-backend health checks
- `list.rs`: Show backend grouping in output
- `lockfile.rs`: Pass registry instead of single backend
- `workspace.rs`: Accept backends slice

### Tests

**Unit tests — `src/backend/mod.rs`**
- `test_registry_all_contains_five_backends`
- `test_detect_returns_only_installed` (mock config dirs with tempdir)
- `test_resolve_intersects_manifest_and_detected`
- `test_resolve_empty_intersection_errors`
- `test_resolve_force_bypasses_manifest`
- `test_resolve_force_does_not_bypass_detection`
- `test_resolve_warns_per_undetected_backend`

**Unit tests — `src/backend/agents.rs`**
- `test_agents_name`
- `test_agents_always_detected`
- `test_deploy_skill_creates_correct_path` (global + project scope)
- `test_deploy_agent_returns_unsupported`
- `test_deploy_hook_returns_unsupported`
- `test_register_mcp_returns_unsupported`

**Unit tests — `src/install_cache.rs`**
- `test_v2_save_and_load_roundtrip`
- `test_v2_per_backend_grouping`
- `test_v1_to_v2_migration_wraps_under_claude`
- `test_v1_migration_preserves_all_artifacts`
- `test_v1_migration_preserves_mcp_servers`
- `test_load_empty_creates_v2`

**Integration tests — `tests/integration_install.rs`**
- `test_install_multi_backend_claude_and_agents` — verify files exist in both locations
- `test_install_rollback_spans_all_backends` — simulate failure mid-deploy, check all cleaned
- `test_install_claude_only_still_works` — regression, iso-functionality

**Integration tests — `tests/integration_uninstall.rs`**
- `test_uninstall_removes_from_all_backends`

**Integration tests — `tests/integration_list.rs`**
- `test_list_shows_per_backend_breakdown`

### Acceptance criteria

- [x] `BackendRegistry` resolves backends from manifest + detection
- [x] `AgentsBackend` deploys skills to `.agents/skills/renkei-{name}/SKILL.md`
- [x] `rk install ./pkg` with manifest `["claude", "agents"]` deploys to both locations
- [x] Install cache v2 groups artifacts per backend
- [x] Loading a v1 cache auto-migrates to v2 under `"claude"` key
- [x] Warning printed when configured backend not detected
- [x] Error when no backend in final set
- [x] `--force` skips manifest check but still filters by detection
- [x] Rollback removes artifacts from ALL backends on error
- [x] `rk uninstall` cleans up artifacts across all backends
- [x] `rk list` shows per-backend breakdown
- [x] `rk doctor` checks per-backend health
- [x] All existing tests pass (iso-functionality for Claude-only installs)

---

## Phase 2: Remaining Backends + Deduplication + Config + CLI Override

**User stories**: US 55, 58, 62, 63

### What to build

Add the 3 remaining backends (Cursor, Codex, Gemini), implement deduplication logic, add `--backend` CLI override, and build the `rk config` command (interactive + programmatic).

**2a. Cursor backend** (`src/backend/cursor.rs` — new)
- `detect_installed()`: check `.cursor/` existence
- `deploy_skill()`: write `.cursor/rules/renkei-{name}.mdc` (Cursor rule format with `alwaysApply`/`globs` frontmatter)
- `deploy_agent()`: write `.cursor/agents/{name}.md`
- `deploy_hook()`: merge into `.cursor/hooks.json`
- `register_mcp()`: merge into `.cursor/mcp.json`

**2b. Codex backend** (`src/backend/codex.rs` — new)
- `detect_installed()`: check `.codex/` existence
- `reads_agents_skills()` → `true`
- `deploy_skill()`: Codex reads `.agents/skills/` — skip if agents backend handles it
- `deploy_agent()`: write `.codex/agents/{name}.toml` (TOML format)
- `deploy_hook()`: write `.codex/hooks.json`
- `register_mcp()`: write to `config.toml` embedded MCP section

**2c. Gemini backend** (`src/backend/gemini.rs` — new)
- `detect_installed()`: check `.gemini/` existence
- `reads_agents_skills()` → `true`
- `deploy_skill()`: Gemini reads `.agents/skills/` — skip if agents backend handles it
- `deploy_agent()`: write `.gemini/agents/{name}.md`
- `deploy_hook()`: merge into `.gemini/settings.json`
- `register_mcp()`: merge into `.gemini/settings.json`

**2d. Deduplication logic** (`src/install.rs`)
- Before deploying skills: if `agents` backend is in resolved set AND current backend has `reads_agents_skills() == true`, skip skill deploy for that backend (US 62)
- Still record in cache for accurate tracking

**2e. `--backend` CLI override** (`src/cli.rs`, `src/main.rs`)
- Add `--backend <name>` flag to `Install` command
- Overrides both config and auto-detect: use ONLY specified backend
- Bypass manifest intersection for override (US 58)

**2f. User config system** (`src/user_config.rs` — new)
- `UserConfig { defaults: { backends: Option<Vec<String>> } }`
- Load from `~/.renkei/config.json`, save JSON
- Integrate into resolution pipeline: if config exists, use `config.backends` instead of auto-detect

**2g. `rk config` command** (`src/config_cmd.rs` — new, `src/cli.rs`)
- `rk config` (no args): interactive TUI with `inquire` multi-select — list all 5 backends, pre-check detected ones, save to config (US 55)
- `rk config set defaults.backends claude,cursor` (US 63)
- `rk config get defaults.backends` (US 63)
- `rk config list` (US 63)
- Add `Config` command variant to CLI with subcommands

### Tests

**Unit tests — `src/backend/cursor.rs`**
- `test_cursor_detect_with_dir` / `test_cursor_detect_without_dir`
- `test_deploy_skill_creates_mdc_file` (verify `.mdc` extension + frontmatter format)
- `test_deploy_agent_creates_md`
- `test_deploy_hook_merges_into_hooks_json`
- `test_register_mcp_merges_into_mcp_json`

**Unit tests — `src/backend/codex.rs`**
- `test_codex_detect_with_dir` / `test_codex_detect_without_dir`
- `test_codex_reads_agents_skills_true`
- `test_deploy_agent_creates_toml`
- `test_deploy_hook_writes_hooks_json`
- `test_register_mcp_writes_config_toml`

**Unit tests — `src/backend/gemini.rs`**
- `test_gemini_detect_with_dir` / `test_gemini_detect_without_dir`
- `test_gemini_reads_agents_skills_true`
- `test_deploy_agent_creates_md`
- `test_deploy_hook_merges_into_settings`
- `test_register_mcp_merges_into_settings`

**Unit tests — deduplication (`src/install.rs`)**
- `test_dedup_skips_skill_when_agents_and_codex`
- `test_dedup_skips_skill_when_agents_and_gemini`
- `test_no_dedup_when_agents_not_in_set`
- `test_no_dedup_for_non_skill_artifacts`

**Unit tests — `src/user_config.rs`**
- `test_load_missing_returns_defaults`
- `test_save_and_load_roundtrip`
- `test_load_with_backends`

**Unit tests — `src/config_cmd.rs`**
- `test_config_set_backends`
- `test_config_get_backends`
- `test_config_list`
- `test_config_set_invalid_backend_errors`

**Integration tests — `tests/integration_install.rs`**
- `test_install_with_backend_override_cursor`
- `test_install_dedup_agents_codex` — skills only in `.agents/`, not in `.codex/`
- `test_install_dedup_agents_gemini`

**Integration tests — `tests/integration_config.rs` (new)**
- `test_config_set_get_roundtrip`
- `test_config_list_output`
- `test_install_uses_config_backends`
- `test_install_falls_back_to_autodetect_without_config`

### Acceptance criteria

- [x] Cursor backend deploys `.mdc` rules, agents, hooks, MCP to correct paths
- [x] Codex backend deploys TOML agents and embedded MCP config
- [x] Gemini backend deploys to `.gemini/` with embedded settings format
- [x] When `agents` + `codex` both resolved, skills only deployed once to `.agents/`
- [x] When `agents` + `gemini` both resolved, same dedup applies
- [x] `--backend cursor` deploys only to Cursor regardless of config/manifest
- [x] `rk config` launches TUI multi-select with detection status
- [x] `rk config set/get/list` work programmatically
- [x] Config persists to `~/.renkei/config.json`
- [x] Resolution pipeline uses config when present, falls back to auto-detect
- [x] All Phase 1 tests still pass

---

## Verification

- Run full test suite after each phase
- Test v1→v2 migration with fixture JSON files
- Test multi-backend install with a fixture package declaring `["claude", "agents"]`
- Test deduplication scenario: package with `["claude", "codex", "agents"]`
- Test `--backend` override + `--force` combinations
- Test `rk config` roundtrip (set → get → install uses config)
- Run validator agent post-migration to confirm iso-functionality

## Key files to modify

| File | Changes |
|------|---------|
| `src/backend/mod.rs` | Registry, trait extension (`reads_agents_skills`) |
| `src/backend/claude.rs` | No change (already conforms) |
| `src/backend/agents.rs` | **New** — Agents backend |
| `src/backend/cursor.rs` | **New** — Cursor backend |
| `src/backend/codex.rs` | **New** — Codex backend |
| `src/backend/gemini.rs` | **New** — Gemini backend |
| `src/install.rs` | Multi-backend loop, dedup, rollback |
| `src/install_cache.rs` | v2 format, migration |
| `src/config.rs` | Path methods for all backends |
| `src/user_config.rs` | **New** — User config file |
| `src/config_cmd.rs` | **New** — `rk config` command |
| `src/cli.rs` | `--backend` flag, `Config` command |
| `src/main.rs` | Registry wiring, command dispatch |
| `src/uninstall.rs` | Per-backend cleanup |
| `src/doctor.rs` | Per-backend checks |
| `src/list.rs` | Per-backend display |
| `src/lockfile.rs` | Multi-backend replay |
| `src/workspace.rs` | Accept backends slice |
