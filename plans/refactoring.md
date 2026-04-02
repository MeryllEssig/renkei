# Refactoring Plan: Module Deepening

> Architecture improvement plan for renkei, organized in 3 phases.
> Each phase builds on the previous one. Follow dependency order strictly.

---

## Phase 1 — Foundations (Config + Package State)

These are the data structures consumed by every other module. Stabilize them first.

### 1.1 Config Path Consolidation

**Problem**: `config.rs` has 15+ per-backend path methods. Each backend calls them directly (`config.claude_skills_dir()`, `config.codex_config_path()`...). Adding a backend = adding 4+ methods to config.rs. Violates Open/Closed.

**Chosen design**: Backend Handle (Design C) — `config.backend(Backend::Claude)` returns a pre-resolved `BackendDirs` struct.

**Interface**:

```rust
pub enum BackendId { Claude, Cursor, Codex, Gemini, Agents }

pub struct BackendDirs {
    pub skills_dir: PathBuf,
    pub agents_dir: PathBuf,
    pub settings_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub hooks_path: Option<PathBuf>,
    pub mcp_path: Option<PathBuf>,
}

impl Config {
    /// Single entry point for all backend path resolution.
    pub fn backend(&self, id: BackendId) -> BackendDirs;

    // Renkei-internal paths stay as direct methods:
    pub fn renkei_dir(&self) -> PathBuf;
    pub fn archives_dir(&self) -> PathBuf;
    pub fn install_cache_path(&self) -> PathBuf;
    pub fn lockfile_path(&self) -> PathBuf;
}
```

**Dependency strategy**: In-process (pure path computation, no I/O).

**Tasks**:

- [x] Create `BackendId` enum and `BackendDirs` struct in `config.rs`
- [x] Implement `BackendDirs::resolve(config, id)` with one match arm per backend, encoding current scope logic (global-only vs scoped paths)
- [x] Add `Config::backend(&self, id: BackendId) -> BackendDirs` method
- [x] Write boundary tests: verify all paths for each backend in both global and project scope
- [x] Migrate `claude.rs` to use `config.backend(BackendId::Claude).skills_dir` etc.
- [x] Migrate `cursor.rs`, `codex.rs`, `gemini.rs`, `agents.rs` the same way
- [x] Migrate `doctor.rs` path lookups (settings, config paths)
- [x] Remove the 15+ old per-backend methods from `Config` (keep renkei-internal ones)
- [x] Delete old per-backend path tests, replaced by new boundary tests
- [x] Run full test suite, verify iso-functionality

### 1.2 Unified Package Store

**Problem**: `install_cache.rs` (573 LOC) and `lockfile.rs` (648 LOC) both track installed packages. Install and uninstall must write to both. No validation of consistency. `LockfileEntry` is a strict projection of `PackageEntry`.

**Chosen design**: Unified `PackageStore` (Design A) — single type that owns both, lockfile derived on save.

**Interface**:

```rust
pub struct PackageStore {
    cache: InstallCache,
    lockfile: Lockfile,
}

impl PackageStore {
    pub fn load(config: &Config) -> Result<Self>;
    pub fn save(&self, config: &Config) -> Result<()>; // writes both files

    pub fn record_install(&mut self, name: &str, entry: PackageEntry);
    pub fn remove(&mut self, name: &str);
    pub fn get(&self, name: &str) -> Option<&PackageEntry>;
    pub fn packages(&self) -> impl Iterator<Item = (&str, &PackageEntry)>;

    /// For install_from_lockfile: read-only access to lockfile entries.
    pub fn lockfile_entries(&self) -> &HashMap<String, LockfileEntry>;

    /// For conflict resolution: mutable access to underlying cache.
    pub fn cache_mut(&mut self) -> &mut InstallCache;
}
```

**Dependency strategy**: Local-substitutable (file I/O, tested with tempdir).

**Tasks**:

- [x] Create `src/package_store.rs` with `PackageStore` struct
- [x] Implement `load()`: loads both InstallCache and Lockfile
- [x] Implement `save()`: saves InstallCache, then derives and saves Lockfile from cache
- [x] Implement `record_install()`: upserts into cache (lockfile derived on save)
- [x] Implement `remove()`: removes from both
- [x] Write boundary tests: load/save roundtrip, record_install writes both files, remove cleans both
- [x] Migrate `install/mod.rs`: replace separate `InstallCache::load` + `Lockfile::load/save` with `PackageStore`
- [x] Migrate `uninstall.rs`: use `PackageStore::remove()` + `save()`
- [ ] Migrate `lockfile.rs::install_from_lockfile()` to use `PackageStore::lockfile_entries()` *(deferred to Phase 3 — install_from_lockfile has complex logic and its own test suite, keeping in lockfile.rs with internal access)*
- [x] Migrate `doctor.rs::run_doctor()` to use `PackageStore::packages()`
- [ ] Move `install_from_lockfile()` from `lockfile.rs` into `package_store.rs` (or keep in lockfile, importing PackageStore) *(deferred to Phase 3)*
- [x] Remove direct `InstallCache` and `Lockfile` usage from callers (keep types `pub(crate)` for internal use)
- [ ] Delete old dual-write tests, replaced by PackageStore boundary tests *(not needed: existing tests use InstallCache directly for setup, they don't duplicate PackageStore behavior)*
- [x] Run full test suite, verify iso-functionality

---

## Phase 2 — Domain Systems (Hooks + Doctor)

With stable Config and PackageStore from Phase 1, refactor the domain systems.

### 2.1 Data-Driven Hook System

**Problem**: `hook.rs` (913 LOC) has 4 separate `translate_event_*` functions, 2 JSON format families (nested/flat), 2 write strategies (merge into settings / standalone file). Adding an event = editing 4 match blocks. Adding a backend = new structs + functions.

**Chosen design**: Data-Driven Deployer (Design C) — `HookProfile` const per backend with event table + layout + target.

**Interface**:

```rust
pub type EventTable = &'static [(&'static str, &'static str)];

pub enum HookLayout { Nested, Flat }
pub enum HookTarget { MergeIntoSettings, StandaloneFile }

pub struct HookProfile {
    pub events: EventTable,
    pub layout: HookLayout,
    pub target: HookTarget,
}

// Backend declarations — pure data
pub const CLAUDE: HookProfile = HookProfile {
    events: &[("before_tool", "PreToolUse"), ("after_tool", "PostToolUse"), /* ... 11 total */],
    layout: HookLayout::Nested,
    target: HookTarget::MergeIntoSettings,
};
pub const CURSOR: HookProfile = HookProfile {
    events: &[("before_tool", "preToolUse"), /* ... 7 total */],
    layout: HookLayout::Flat,
    target: HookTarget::StandaloneFile,
};
pub const CODEX: HookProfile = HookProfile { /* ... */ };
pub const GEMINI: HookProfile = HookProfile { /* ... */ };

// Two functions handle everything
pub fn deploy(profile: &HookProfile, hooks: &[RenkeiHook], path: &Path) -> Result<Vec<DeployedHookEntry>>;
pub fn remove(profile: &HookProfile, path: &Path, entries: &[DeployedHookEntry]) -> Result<()>;

// Pure translation for dry-run/preview
pub fn translate(profile: &HookProfile, hooks: &[RenkeiHook]) -> Result<serde_json::Value>;
```

**Dependency strategy**: In-process (event translation is pure data transformation; I/O only in deploy/remove).

**Tasks**:

- [x] Define `HookProfile`, `HookLayout`, `HookTarget`, `EventTable` types
- [x] Create `CLAUDE`, `CURSOR`, `CODEX`, `GEMINI` const profiles with full event tables
- [x] Implement `translate()`: lookup events from profile's table, serialize according to layout
- [x] Implement `deploy()`: call `translate()`, then write according to `target` (merge or standalone)
- [x] Implement `remove()`: read file, remove matching entries according to layout, write back
- [x] Write boundary tests for `translate()` with each profile (pure, no I/O)
- [x] Write boundary tests for `deploy()` + `remove()` roundtrip per profile (tempdir)
- [x] Migrate `claude.rs` to use `hook::deploy(&hook::CLAUDE, &hooks, &settings_path)`
- [x] Migrate `cursor.rs` to use `hook::deploy(&hook::CURSOR, ...)`
- [x] Migrate `codex.rs` and `gemini.rs` similarly
- [x] Remove old functions: `translate_event`, `translate_event_cursor`, `translate_event_codex`, `translate_event_gemini`, `translate_hooks`, `translate_hooks_with`, `translate_hooks_cursor`, `write_cursor_hooks`, `write_standalone_hooks`, `merge_hooks_into_settings`
- [x] Remove old types that become private: `ClaudeHookGroup`, `ClaudeHookEntry`, `CursorHookEntry` (or make `pub(crate)`)
- [x] Delete old per-backend translation tests, replaced by profile-based boundary tests
- [x] Run full test suite, verify iso-functionality

### 2.2 Doctor Module Deepening

**Problem**: `doctor.rs` (1053 LOC) is a god-object. 6 check functions + formatting + orchestration + 36 tests all in one file. Checks know internal schemas of cache, settings.json, archives.

**Chosen design**: `DoctorReport::build()` (Design C) + file split from Design A.

**Interface**:

```rust
// doctor/mod.rs — thin orchestrator
pub fn run_doctor(config: &Config, global: bool, registry: &BackendRegistry) -> Result<bool>;

// doctor/types.rs — unchanged
pub enum DiagnosticKind { FileMissing { .. }, SkillModified { .. }, ... }
pub struct PackageDiagnostic { pub package_name: String, pub version: String, pub issues: Vec<DiagnosticKind> }
pub struct DoctorReport { pub backend_ok: bool, pub package_diagnostics: Vec<PackageDiagnostic> }

// doctor/report.rs — report knows how to build and format itself
impl DoctorReport {
    pub fn build(
        cache: &InstallCache,
        settings: &serde_json::Value,
        claude_config: &serde_json::Value,
        registry: &BackendRegistry,
        config: &Config,
    ) -> Self;

    pub fn format(&self, scope_label: &str, backend_statuses: &[(String, bool)]) -> String;
}

// doctor/checks.rs — free functions, unchanged signatures
pub fn check_deployed_files(entry: &PackageEntry) -> Vec<DiagnosticKind>;
pub fn check_skill_modifications(entry: &PackageEntry) -> Vec<DiagnosticKind>;
pub fn check_env_vars(entry: &PackageEntry) -> Vec<DiagnosticKind>;
pub fn check_hooks(entry: &PackageEntry, settings: &Value) -> Vec<DiagnosticKind>;
pub fn check_mcp(entry: &PackageEntry, claude_config: &Value) -> Vec<DiagnosticKind>;
```

**File structure**:

```
src/doctor/
  mod.rs          — run_doctor (~20 lines), re-exports
  types.rs        — DiagnosticKind, PackageDiagnostic, ArchiveState (~55 lines)
  report.rs       — DoctorReport::build(), format(), format_check_section (~180 lines)
  checks.rs       — 5 check fns + hook_exists_in_settings helper (~160 lines)
  tests/
    mod.rs
    check_deployed_tests.rs
    check_skill_tests.rs
    check_env_tests.rs
    check_hooks_tests.rs
    check_mcp_tests.rs
    format_tests.rs
```

**Dependency strategy**: Local-substitutable (archive extraction, tempdir in tests).

**Tasks**:

- [x] Create `src/doctor/` directory structure
- [x] Extract `DiagnosticKind`, `PackageDiagnostic`, `DoctorReport`, `ArchiveState` to `doctor/types.rs`
- [x] Extract 5 check functions + `hook_exists_in_settings` helper to `doctor/checks.rs`
- [x] Implement `DoctorReport::build()` in `doctor/report.rs` (move orchestration loop from `run_doctor`)
- [x] Implement `DoctorReport::format()` in `doctor/report.rs` (move `format_report` + `format_check_section`)
- [x] Simplify `run_doctor` in `doctor/mod.rs` to: load data → `DoctorReport::build()` → `print!("{}", report.format(...))` → `Ok(report.is_healthy())`
- [x] Write boundary tests for `DoctorReport::build()` (construct inputs, assert on returned struct without printing)
- [x] Split existing 36 tests into topic-based files under `doctor/tests/`
- [x] Extract shared test helpers (`make_entry`, `make_artifact`, `make_hook_entry`) to `doctor/tests/mod.rs`
- [x] Note: if Phase 1.2 is done, `run_doctor` uses `PackageStore::packages()` instead of `InstallCache::load()`
- [x] Run full test suite, verify iso-functionality

---

## Phase 3 — Orchestration (Install Pipeline)

With Config, PackageStore, and Hook system stable, refactor the main pipeline.

### 3.1 Install Pipeline Decomposition

**Problem**: `install/mod.rs` (210 LOC) orchestrates 8 sequential steps touching 7+ modules. Two callers (`install_local` and `install_from_lockfile`) share most steps but skip different ones, currently handled via `from_lockfile` boolean flag.

**Chosen design**: Caller-Optimized (Design C) — two explicit public functions sharing an internal `CorePipeline`.

**Interface**:

```rust
// Public: two functions, zero options
pub fn install_local(
    package_dir: &Path,
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
    options: &InstallOptions,  // source_kind, source_url, tag, resolved, force
) -> Result<()>;

pub fn install_from_lock_entry(
    package_dir: &Path,
    config: &Config,
    backends: &[&dyn Backend],
    requested_scope: RequestedScope,
) -> Result<()>;

// Internal shared core
struct CorePipeline {
    manifest: ValidatedManifest,
    raw_manifest: Manifest,
    active_backends: Vec<BackendRef>,
    artifacts: Vec<Artifact>,
}

impl CorePipeline {
    fn discover(package_dir: &Path, config: &Config, backends: &[&dyn Backend], force: bool) -> Result<Self>;
    fn cleanup_and_resolve(&mut self, store: &mut PackageStore, resolver: &ConflictResolver) -> Result<ResolvedArtifacts>;
    fn deploy(&self, resolved: &ResolvedArtifacts, config: &Config) -> Result<DeploymentResult>;
}
```

**Implementation sketch**:

```rust
pub fn install_local(...) -> Result<()> {
    let mut core = CorePipeline::discover(package_dir, config, backends, options.force)?;
    manifest::validate_scope(&core.manifest.install_scope, requested_scope)?;

    let mut store = PackageStore::load(config)?;
    let resolved = core.cleanup_and_resolve(&mut store, &default_resolver(options.force))?;

    let (archive_path, integrity) = cache::create_archive(package_dir, &core.manifest, config)?;
    let deployment = core.deploy(&resolved, config)
        .map_err(|e| { let _ = fs::remove_file(&archive_path); e })?;

    store.record_install(&core.manifest.full_name, PackageEntry { /* ... */ });
    store.save(config)?;
    Ok(())
}

pub fn install_from_lock_entry(...) -> Result<()> {
    let mut core = CorePipeline::discover(package_dir, config, backends, false)?;

    let mut store = PackageStore::load(config)?;
    let resolved = core.cleanup_and_resolve(&mut store, &force_resolver())?;
    core.deploy(&resolved, config)?;

    store.record_install(&core.manifest.full_name, PackageEntry { /* ... */ });
    store.save(config)?; // save() skips lockfile derivation for entries already in lockfile
    Ok(())
}
```

**Dependency strategy**: Local-substitutable (file I/O, tempdir).

**Tasks**:

- [ ] Create `CorePipeline` struct in `install/pipeline.rs` (or inline in `install/mod.rs`)
- [ ] Implement `CorePipeline::discover()`: manifest load + validate, backend filtering, artifact discovery
- [ ] Implement `CorePipeline::cleanup_and_resolve()`: cleanup previous install + conflict resolution
- [ ] Implement `CorePipeline::deploy()`: multi-backend deployment with rollback on failure
- [ ] Refactor `install_local` to use `CorePipeline` + `PackageStore`
- [ ] Create `install_from_lock_entry` using `CorePipeline` without archive creation
- [ ] Refactor `lockfile.rs::install_from_lockfile()` to iterate lockfile entries and call `install_from_lock_entry`
- [ ] Remove `from_lockfile` boolean from `InstallOptions`
- [ ] Remove `SourceKind` from `InstallOptions` if no longer needed (source info passed directly)
- [ ] Update integration tests to cover both paths
- [ ] Write boundary tests for `CorePipeline::discover()`, `cleanup_and_resolve()`, `deploy()` individually
- [ ] Run full test suite, verify iso-functionality

---

## Cleanup Tasks (After All Phases)

- [ ] Delete `json_file.rs` if confirmed dead code (20 LOC)
- [ ] Remove any `#[allow(dead_code)]` that are no longer needed
- [ ] Run `cargo clippy` and fix any new warnings
- [ ] Run full integration test suite one final time

---

## Dependency Graph

```
Phase 1.1 (Config) ──┬──> Phase 2.1 (Hooks)   ──┐
                      │                           │
Phase 1.2 (Store)  ──┼──> Phase 2.2 (Doctor)  ──┼──> Phase 3.1 (Install Pipeline)
                      │                           │
                      └───────────────────────────┘
```

Phase 1.1 and 1.2 can run in parallel.
Phase 2.1 and 2.2 can run in parallel (after Phase 1 complete).
Phase 3.1 requires all of Phase 1 + Phase 2.

---

## Principles

1. **TDD**: Write boundary tests first, then refactor. Tests assert on observable outcomes through the public interface.
2. **Replace, don't layer**: Old shallow-module tests are deleted once boundary tests exist.
3. **Atomic commits**: One commit per task checkbox. Conventional commit format.
4. **Iso-functionality**: Each task must pass the full test suite before and after. No behavior changes.
5. **No new features**: This plan is purely structural. No new CLI commands, no new capabilities.
