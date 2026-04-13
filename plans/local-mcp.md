# Plan: Local MCP servers

Allow package authors to ship MCP server source code inside their package, with a declared build procedure and an `entrypoint`. Renkei copies the sources to `~/.renkei/mcp/<name>/`, runs the build there under a minimal env, and registers the backend's MCP config with the absolute path to the built entrypoint. Deployment is always global, regardless of the package's install scope.

## Context

Today, the `mcp` field in `renkei.json` declares a ready-to-run server config (`command`, `args`, `env`) that renkei merges into `~/.claude.json` (see `src/mcp.rs:14`). This requires the server to already be installed out-of-band (npm global, binary in `PATH`, etc.). There is no story for a workflow that wants to ship its own MCP server alongside its skills/agents.

This plan adds a **local MCP** convention: an `mcp/<name>/` directory at the package root (or workspace member root) containing the server's sources. Renkei copies, builds, and registers the server. Because MCP registration is a global-level concern for Claude Code (single `~/.claude.json`), the MCP sources are also always stored globally — even when the package itself is installed in project scope.

## Design summary

### Convention and manifest

- Source directory `mcp/<name>/` at the package root (workspace: at the member root). Folder name **must** match the key in `mcp.<name>` of `renkei.json` (strict match, validated).
- Manifest extension (`mcp.<name>` gains two fields — both optional, presence implies local MCP):
  - `entrypoint: string` — relative path inside `mcp/<name>/` pointing to the built runtime file.
  - `build: string[][]` — array of argv arrays (no shell), each ≥ 1 token. Example: `[["bun","install"],["bun","run","build"]]`.
- When a local MCP is declared, renkei writes into the backend's MCP config with `args` absolute-resolved to `~/.renkei/mcp/<name>/<entrypoint>` (prepended to the user-declared `args`, or fully replacing the entrypoint slot — see Phase 1 for exact semantics).

### Scope

- The package's `scope` (`any` | `project` | `global`) remains free. Deployment of local MCP sources is **always global** (`~/.renkei/mcp/<name>/`), decoupled from the package's install scope. Rationale: Claude Code's MCP registration is global (`~/.claude.json`) and cannot be scoped to a project; the source directory follows that constraint.

### Install flow

1. Gather `messages.preinstall` notices → existing `[y/N]` prompt.
2. Gather local-MCP build commands → **new dedicated prompt** listing `@scope/name → [cmd1, cmd2, ...]` per MCP. `--allow-build` bypasses it. Non-TTY without the flag → hard error.
3. Deploy skills/agents as today (respecting the active scope).
4. For each local MCP to deploy: copy `mcp/<name>/` into staging `~/.renkei/mcp/<name>.new/`.
5. Execute `build` commands sequentially inside the staging dir, `cwd = <staging>`, streaming stdout/stderr live.
6. On build success: atomically swap staging → `~/.renkei/mcp/<name>/` (rename; if target exists, rename it to `.old` first, then swap, then rm `.old`).
7. On build failure: `rm -rf` staging, previous version (if any) remains untouched, install aborts with rollback of other deployed artifacts.
8. Merge into backend MCP config with `entrypoint` resolved to absolute path.
9. `requiredEnv` warnings + `messages.postinstall`.

### Build environment

- No shell: each build step is invoked as argv via `std::process::Command`.
- Minimal env: filter from the inherited env, keeping only the whitelist below.
  - Plain: `PATH`, `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_*`, `TMPDIR`, `SHELL`, `TERM`.
  - Proxies (case-insensitive): `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`.
  - Certs: `NODE_EXTRA_CA_CERTS`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `REQUESTS_CA_BUNDLE`.
  - Prefixes (inherited in bulk): `npm_config_*`, `PIP_*`, `BUN_*`, `CARGO_*`, `UV_*`, `POETRY_*`.
  - Explicitly excluded: any name matching `*_TOKEN`, `*_KEY`, `*_SECRET`, `*PASSWORD*`, `AWS_*`, `GITHUB_*`, `GITLAB_*`, `ANTHROPIC_*`, `OPENAI_*`.
- `requiredEnv` entries are **not** propagated into the build env. The build runtime is isolated from runtime secrets by design.

### Reference counting + conflicts

- `install-cache.json` bumps to v3: new top-level section `mcp_local: HashMap<name, McpLocalEntry>` separate from per-package `mcp_servers`.
- `McpLocalEntry { owner_package, version, source_sha256, referenced_by: Vec<McpLocalRef> }`. `McpLocalRef { package, version, scope, project_root }`.
- Install: if `mcp_local.<name>` absent → create entry, `owner_package = <installing>`, single ref. If present and `owner_package == <installing>` → add ref, upgrade sources + rebuild if version > stored. If present and `owner_package != <installing>` → **hard error** unless `--force` (which transfers ownership and rebuilds).
- "One version at a time" rule: upgrading a local MCP forces all `referenced_by` to the new version (transparent to callers; they share the same folder). Documented as intentional.
- Uninstall: remove matching `McpLocalRef`. If `referenced_by` becomes empty → `rm -rf` the folder + remove from backend MCP config. Otherwise leave untouched.

### `--link` mode

- Symlink `~/.renkei/mcp/<name>/` → `<workspace>/mcp/<name>/`.
- Renkei does **not** run the build. The developer manages their own build lifecycle in the workspace.
- The backend MCP config is still registered with the resolved entrypoint (the symlink target).
- Uninstall removes the symlink only (never touches the workspace source).

### Lockfile

- `rk.lock` records `source_sha256` of the MCP source directory (computed over the package's rkignore-filtered content). Build outputs are **not** locked.
- Lockfile replay rebuilds from sources and still requires `--allow-build` at each invocation (no trust carryover from the original install — a new machine re-consents explicitly).

### `rk package` exclusions

- Hardcoded default exclusions: `node_modules/`, `dist/`, `build/`, `target/`, `.venv/`, `venv/`, `__pycache__/`, `.pytest_cache/`, `*.pyc`, `.DS_Store`, `.git/`.
- Optional `.rkignore` at package root (or workspace member root) overrides/extends the defaults. Gitignore syntax.
- Same exclusion logic is reused when computing `source_sha256` at install time.

### `rk doctor`

- Per local MCP entry in `mcp_local`:
  - `exists` → `~/.renkei/mcp/<name>/` on disk. Severity: **error**.
  - `integrity` → SHA-256 over rkignore-filtered source content matches stored `source_sha256`. Severity: **warning**.
  - `entrypoint` → file pointed by `<~/.renkei/mcp/<name>/<entrypoint>>` exists. Severity: **error**.

### Manifest validation rules

- If `mcp/<name>/` exists on disk → `mcp.<name>` must be declared in the manifest (and vice versa). Mismatch → `InvalidManifest`.
- If `mcp.<name>.build` declared → `mcp/<name>/` must exist in the package.
- If `mcp/<name>/` exists → `mcp.<name>.entrypoint` must be set (the `command`/`args` alone cannot point at the deployed dir without it).
- `build`: each entry is an array of strings, first element non-empty. Invalid → `InvalidManifest`.
- Workspace: if two members declare local MCPs with the same `<name>` → validation error at the root manifest load, regardless of `-m` selection.

---

## Phase 1: Manifest schema, `entrypoint`, `build`, validation

- [x] 1.1 Extend `mcp` parsing in `src/manifest.rs`. Today `pub mcp: Option<serde_json::Value>` is opaque — introduce a typed `McpServer` struct with `#[serde(flatten)] extra: serde_json::Value` to preserve arbitrary native fields (`command`, `args`, `env`, ...). Add typed `entrypoint: Option<String>` and `build: Option<Vec<Vec<String>>>`. Store as `Option<HashMap<String, McpServer>>`.
- [x] 1.2 Back-compat: packages with only external MCPs (no `entrypoint`/`build`) must continue to parse and behave exactly as today. Add a helper `McpServer::is_local() -> bool` = `entrypoint.is_some() || build.is_some()`.
- [x] 1.3 Validation in `Manifest::validate` (new helper `validate_local_mcp(package_root: &Path)`):
  - For each `mcp.<name>` with `entrypoint` or `build` → require `package_root/mcp/<name>/` to exist and be a directory.
  - For each `package_root/mcp/<X>/` directory → require `mcp.<X>` to be declared.
  - For each declared `build`: non-empty outer array, every inner array non-empty, every string non-empty.
  - If `entrypoint` declared, require `build.is_some()` **unless** the resolved entrypoint file already exists on disk (supports prebuilt / vendored entrypoints).
- [x] 1.4 Workspace-level validation in `src/workspace.rs` (or wherever members are loaded): collect each member's local MCP names; if any collision across members → error `InvalidManifest("workspace members share MCP name '<X>' in '<memberA>' and '<memberB>'")`.
- [x] 1.5 TDD — unit tests in `src/manifest.rs`:
  - External-only MCP still parses (no regression).
  - `entrypoint` without `build` + existing `dist/` file → OK.
  - `entrypoint` without `build` + missing file → error.
  - `build` without `entrypoint` → error ("local MCP requires `entrypoint`").
  - `mcp/foo/` exists on disk but `mcp.foo` absent → error (requires a fixture dir).
  - `mcp.foo` declares `build` but no `mcp/foo/` dir → error.
  - `build: [[]]` → error. `build: [["bun"]]` → OK.
  - Workspace collision → error.

## Phase 2: Install-cache v3 — `mcp_local` reference counting

- [x] 2.1 Bump `CURRENT_VERSION` to 3 in `src/install_cache.rs`. Add `V2Cache` struct for the v2→v3 migration (trivial: no `mcp_local` yet, empty map).
- [x] 2.2 Add types:
  ```rust
  pub struct McpLocalEntry {
      pub owner_package: String,
      pub version: String,
      pub source_sha256: String,
      pub referenced_by: Vec<McpLocalRef>,
  }
  pub struct McpLocalRef {
      pub package: String,
      pub version: String,
      pub scope: String,        // "global" | "project"
      pub project_root: Option<String>,  // None for global installs
  }
  ```
- [x] 2.3 Extend `InstallCache` with `pub mcp_local: HashMap<String, McpLocalEntry>` (`#[serde(default)]`).
- [x] 2.4 Helpers on `InstallCache`:
  - `add_mcp_local_ref(name, entry_ctor, new_ref) -> McpLocalOutcome` where the outcome is one of `{ FreshInstall, AddedRef, UpgradeRequired, ConflictDifferentOwner }`.
  - `remove_mcp_local_ref(name, match: McpLocalRef) -> Vec<String>` returning names to GC (empty-refs) for caller-driven cleanup.
- [x] 2.5 TDD in `src/install_cache.rs`:
  - v2 cache migrates to v3 with empty `mcp_local`.
  - Fresh install → `FreshInstall`.
  - Second install of same `owner_package`, same version, new project → `AddedRef`, refs length == 2.
  - Same `owner_package` with higher version → `UpgradeRequired` (caller rebuilds).
  - Different owner → `ConflictDifferentOwner`.
  - `remove_mcp_local_ref` removes by `(package, scope, project_root)` tuple; when last ref gone, name returned for GC.

## Phase 3: Rkignore + source hashing

- [ ] 3.1 New module `src/rkignore.rs`:
  - `DEFAULT_IGNORES: &[&str]` (hardcoded list from the design summary).
  - `pub fn load_rkignore(root: &Path) -> Vec<String>` — reads `<root>/.rkignore` if present, appends to defaults.
  - `pub fn is_ignored(relative_path: &Path, patterns: &[String]) -> bool` — implement via the `ignore` crate (already transitive via `globset`?) or fall back to a minimal gitignore-like matcher if adding a dep is undesirable. **Decision to record:** prefer the `ignore` crate unless Cargo.toml review shows it's not present and a tiny in-house matcher is enough.
- [ ] 3.2 `pub fn hash_directory(root: &Path, patterns: &[String]) -> Result<String>`:
  - Walk `root` deterministically (sorted), skip ignored paths, hash `(relative_path_bytes, file_mode, file_contents)` into a `Sha256`, return hex.
  - Reused by packaging (Phase 7) and install-time hashing (Phase 4/5).
- [ ] 3.3 TDD:
  - Default ignores exclude `node_modules/`, `.DS_Store`, etc.
  - `.rkignore` additions merge with defaults.
  - Hash is stable across platforms (normalize path separators).
  - Two directories with identical content + different ignored garbage produce the same hash.

## Phase 4: Build execution (env filter, argv runner, streaming)

- [ ] 4.1 New module `src/install/build.rs`:
  - `pub fn build_env() -> HashMap<String, String>` — apply the whitelist + prefixes + exclude list described above. Log filtered-out variables at debug level only.
  - `pub struct BuildStep { pub argv: Vec<String> }`.
  - `pub fn run_build(steps: &[BuildStep], cwd: &Path) -> Result<()>` — executes sequentially, `Command::new(argv[0]).args(&argv[1..]).current_dir(cwd).env_clear().envs(build_env())`, stdout/stderr inherited (streams live). On non-zero exit: stop, return `RenkeiError::BuildFailed { step: argv.join(" "), exit_code }`.
- [ ] 4.2 Error variant `RenkeiError::BuildFailed { step, exit_code }` in `src/error.rs`. Message includes the failed argv string and the exit code; no stdout capture (user already saw it live).
- [ ] 4.3 TDD — integration-style tests with real processes (gated to Unix for simplicity; `sh` / `echo` / `false` present):
  - Single `["true"]` step → Ok.
  - `["false"]` → `BuildFailed` with exit code 1.
  - Multi-step, second fails → first ran, error carries second step's argv.
  - Env filter: run `["env"]`, capture output via a variant helper that swaps stdio → assert `HOME`, `PATH` present, `AWS_SECRET_ACCESS_KEY` absent, `npm_config_foo` preserved.
  - cwd respected: `["pwd"]` matches the passed path.
- [ ] 4.4 Non-Unix fallback: guard the process-based tests with `#[cfg(unix)]`; document that Windows support of `run_build` is out of scope for v1.

## Phase 5: CLI flag, build prompt, deploy orchestration

- [ ] 5.1 Add `#[arg(long = "allow-build")] allow_build: bool` to `Commands::Install` in `src/cli.rs`. Plumb through `run_install` / `install_or_workspace` / batch coordinator alongside `yes`/`force`.
- [ ] 5.2 New type and collector in `src/install/build.rs` (or `messages.rs`):
  ```rust
  pub struct BuildNotice { pub full_name: String, pub mcp_name: String, pub steps: Vec<Vec<String>> }
  pub fn collect_build_notices(manifests: &[(&Manifest, &Path)]) -> Vec<BuildNotice>
  pub fn confirm_builds(notices: &[BuildNotice], allow_build: bool) -> Result<bool>
  ```
  `confirm_builds` returns `Ok(true)` to proceed, `Err(RenkeiError::BuildRequiresConfirmation)` in non-TTY without `--allow-build`, never prompts if the notices list is empty.
- [ ] 5.3 Rendering: yellow/bold framed block titled `Build notice: the following commands will execute with a minimal environment:`. Per line: `  @scope/name → <mcp-name>: bun install && bun run build` (steps joined with ` && ` visually, clarifying it's still argv, not a shell).
- [ ] 5.4 Integrate into the batch coordinator (`src/install/batch.rs`):
  - After `confirm_preinstall`, call `collect_build_notices` + `confirm_builds`.
  - Build notice collection is cheap (reads the manifest already loaded) and must run before any artifact copy.
- [ ] 5.5 Deploy pipeline `src/install/deploy.rs` (per package):
  1. Compute `source_sha256` over `mcp/<name>/` using `rkignore::hash_directory`.
  2. Consult `install_cache.add_mcp_local_ref(...)` → branch on outcome:
     - `FreshInstall` → continue to staging+build.
     - `AddedRef` → skip staging/build, but ensure the backend MCP config still has the entry (idempotent merge).
     - `UpgradeRequired` → continue to staging+build; the existing `~/.renkei/mcp/<name>/` is **not** overwritten until the atomic swap.
     - `ConflictDifferentOwner` → return `RenkeiError::McpOwnerConflict { name, owner, attempted_by }` unless `force` was passed (in which case treat like `UpgradeRequired` + overwrite owner).
  3. For branches needing build: `cp -r mcp/<name>/ ~/.renkei/mcp/<name>.new/`, `run_build(...)`, then swap:
     - If `~/.renkei/mcp/<name>/` exists: rename it to `.old`; rename `.new` → `<name>`; `rm -rf .old`.
     - If anything fails between steps, best-effort rollback: if `.old` exists and `<name>` missing, rename `.old` back.
  4. On build failure: `rm -rf ~/.renkei/mcp/<name>.new/`, leave the previous version intact, propagate error up → batch rollback runs as today.
  5. Resolve `entrypoint` to absolute path: `~/.renkei/mcp/<name>/<entrypoint>`. Merge into backend MCP config using the existing `merge_mcp_into_config` with a rewritten `args` array (prepend the absolute path, or replace the slot — see 5.6).
- [ ] 5.6 `args` resolution rule to document and test explicitly: renkei builds the final backend `args` as `[abs_entrypoint, ...manifest_args]`. The manifest's `args` carry pass-through flags only; the entrypoint always comes from `entrypoint`. Example manifest:
  ```json
  "my-server": {
      "command": "node",
      "entrypoint": "dist/index.js",
      "args": ["--verbose"],
      "build": [["bun","install"],["bun","run","build"]]
  }
  ```
  → `.claude.json`: `{"command":"node","args":["/home/u/.renkei/mcp/my-server/dist/index.js","--verbose"]}`.
- [ ] 5.7 Batch coordinator records the `McpLocalRef` in `install_cache` only **after** successful swap + successful backend merge.
- [ ] 5.8 TDD — mix of unit and integration:
  - Happy path: fresh install with a trivial MCP (build = `["true"]`, entrypoint = a pre-existing stub file). Assert folder exists, `install_cache.json` entry present, backend config has absolute path.
  - Build failure: staging dir is removed, previous folder (if any) intact, no install_cache entry, no backend config entry, install returns non-zero.
  - Same-owner re-install on another project → `AddedRef`, no rebuild, `referenced_by.len() == 2`.
  - Upgrade (same owner, higher version) → rebuild, staging swap, single ref bumps version.
  - Different-owner conflict without `--force` → error. With `--force` → overwrite, owner updated, single ref.
  - Non-TTY without `--allow-build` + build needed → error with explicit hint.
  - `--allow-build` bypasses prompt.

## Phase 6: Workspace, lockfile, scope interactions

- [ ] 6.1 Workspace install: each selected member's local MCPs participate in the global batch prompt (one `Build notice:` block for the invocation, lines from all members listed). Already naturally covered if Phase 5 operates on the flat list of `(manifest, member_root)` pairs.
- [ ] 6.2 Lockfile (`src/lockfile.rs`): add optional `mcp_local_sources: HashMap<String, String>` (MCP name → `source_sha256`) per package entry. Written alongside `integrity` at install time.
- [ ] 6.3 Lockfile replay (no-arg `rk install`): re-materialize sources, recompute hash, compare with lockfile — if mismatch, fail with a clear message ("lockfile drift: `@scope/name` MCP `my-server` source hash changed"). On match, proceed with normal install (including build prompt + `--allow-build`).
- [ ] 6.4 Scope behaviour: `deploy` always writes MCP sources to `~/.renkei/mcp/` irrespective of `RequestedScope`. `McpLocalRef.scope` and `McpLocalRef.project_root` record where the owning package was installed, purely for reference accounting (so uninstall in project scope X only decrements X's ref).
- [ ] 6.5 TDD:
  - Workspace with two members each declaring a local MCP → one prompt, both build, both refs recorded.
  - Workspace collision (same MCP name in two members) fails at validation (covered in Phase 1, double-check integration).
  - Lockfile replay after source modification in source-of-truth → drift error.
  - Install project scope on two different projects → same folder, two refs with distinct `project_root`.

## Phase 7: `--link` mode

- [ ] 7.1 In `src/install/deploy.rs`, branch on the source kind (already tracked — `Source::LocalLink` vs `Source::LocalCopy`/Git):
  - For linked installs: skip staging+build entirely.
  - `symlink(<workspace>/mcp/<name>, ~/.renkei/mcp/<name>)` if target absent. If target exists and is a symlink pointing elsewhere → `ConflictDifferentOwner` semantics. If target exists and is a real directory → error ("cannot link: `~/.renkei/mcp/<name>` is a real directory from a previous copy install; uninstall it first").
  - Compute `source_sha256` at link time (snapshot) for lockfile/doctor parity. Accept that the hash is only a snapshot — the user modifying the source will drift immediately. Doctor's integrity check is a warning anyway.
  - Still merge into backend config with absolute `entrypoint` resolved through the symlink.
- [ ] 7.2 Uninstall: if `~/.renkei/mcp/<name>/` is a symlink → `remove_file` (never `remove_dir_all`). Test both branches.
- [ ] 7.3 TDD:
  - `rk install --link <workspace>` on a package with local MCP → symlink created, no build executed, backend config registered.
  - Build commands declared but never run in `--link` mode — no `--allow-build` prompt triggered for links.
  - Uninstall removes symlink only; workspace source intact.
  - Re-link same MCP from different workspace → conflict error.

## Phase 8: Uninstall GC

- [ ] 8.1 In `src/uninstall.rs`: after removing artifacts, for each local MCP referenced by the uninstalled package, call `install_cache.remove_mcp_local_ref(...)`.
- [ ] 8.2 For each returned name (empty refs): `rm -rf ~/.renkei/mcp/<name>/` (or `remove_file` if symlink) + remove the server from the backend MCP config via `remove_mcp_from_config`.
- [ ] 8.3 TDD:
  - Install on 2 projects → uninstall from 1 → folder + backend config unchanged, cache shows 1 ref.
  - Uninstall the last ref → folder removed, backend config entry gone, `mcp_local.<name>` removed from cache.
  - Uninstall a linked install → symlink removed, no error, no `remove_dir_all`.

## Phase 9: `rk doctor` checks for local MCPs

- [ ] 9.1 Extend `src/doctor/checks.rs`: new `check_mcp_local` iterating `install_cache.mcp_local`:
  - `exists`: `~/.renkei/mcp/<name>/` present. Error if missing.
  - `integrity`: `rkignore::hash_directory(...)` == stored `source_sha256`. Warning if differs.
  - `entrypoint`: file at `<folder>/<entrypoint>` exists. Error if missing.
  - Entrypoint is read from the owning package's manifest, which is reachable via `package_store` using `owner_package` + `version`.
- [ ] 9.2 Report integration: add a `McpLocal` variant in `src/doctor/report.rs` / types if needed, or reuse a generic `Check` row.
- [ ] 9.3 TDD in `src/doctor/tests/`:
  - All three checks OK → all green.
  - Folder deleted manually → `exists` error.
  - A source file tampered with → `integrity` warning, others OK.
  - `dist/index.js` removed (build artifact) → `entrypoint` error.
  - Linked install with identical source → integrity OK.
  - Linked install with user-modified source → integrity warning (expected).

## Phase 10: `rk package` exclusions + `.rkignore`

- [ ] 10.1 Refactor `src/package.rs` archive builder to delegate to `rkignore::is_ignored` for each candidate file.
- [ ] 10.2 If `.rkignore` exists at the package root, read and apply.
- [ ] 10.3 Default exclusions always apply, even without `.rkignore`.
- [ ] 10.4 TDD:
  - Package a fixture with `mcp/foo/node_modules/` → archive excludes it.
  - Package with `.rkignore` that adds `generated/` → `generated/` excluded; defaults still applied.
  - Package with **no** local MCP → behavior unchanged (no regressions on existing fixtures).

## Phase 11: Integration tests

- [ ] 11.1 New file `tests/integration_local_mcp.rs` with fixtures under `tests/fixtures/local-mcp-pkg/` (a minimal package whose `build` is `[["sh","-c","mkdir -p dist && printf '#!/usr/bin/env node\\nconsole.log(1)' > dist/index.js"]]` — wait, no shell. Use `[["cp","entry-src.js","dist/index.js"]]` with a pre-mkdir'd `dist/`, or a vendored `dist/index.js` with `build: [["true"]]`). Keep it portable and fast.
- [ ] 11.2 Scenarios:
  - Fresh install: folder + backend config + cache entry.
  - Re-install on second "project" (simulated via different `CWD` + git init): same folder, two refs.
  - Uninstall from one of two projects: folder stays, ref count decrements.
  - Uninstall the last: folder gone, backend config cleaned.
  - Conflict: two packages with different `@scope/name` both shipping `mcp/my-server/` → second install fails with owner conflict; `--force` overrides and transfers ownership.
  - `--allow-build` required in non-TTY: assert stderr mentions the flag.
  - `--link` path: symlink present, no build, uninstall removes symlink only.
  - Lockfile drift detection: modify `mcp/<name>/` between installs, rerun `rk install` from the lockfile → drift error.

## Phase 12: Documentation / PRD

- [ ] 12.1 New thematic file `doc/prd/mcp.md`:
  - Convention `mcp/<name>/`, manifest fields `entrypoint`/`build`.
  - Scope decoupling: MCP sources always global; package scope independent.
  - Install flow (preinstall prompt → build prompt → copy → build → swap → merge → postinstall).
  - Env whitelist (exact list).
  - `--allow-build` semantics, lockfile replay behaviour.
  - Reference counting model + same-owner / different-owner rules + `--force`.
  - `--link` semantics.
  - Doctor checks.
  - Packaging exclusions + `.rkignore`.
- [ ] 12.2 Update `PRD.md` index: add a line under the existing entries pointing to `doc/prd/mcp.md` (with the one-sentence summary style used by neighbours).
- [ ] 12.3 Update `doc/prd/manifest.md`: extend the `mcp.<name>` description with `entrypoint` and `build`; cross-link to `mcp.md`. Replace the current "native `command`/`args`/`env` format (already portable across backends)" paragraph to mention local MCPs.
- [ ] 12.4 Update `doc/prd/installation.md`: add a "Local MCP builds" section under the existing "Environment variables" / "Preinstall confirmation" neighbourhood, covering the build prompt, `--allow-build`, and the atomic swap + rollback guarantee.
- [ ] 12.5 Update `doc/prd/scope.md`: document the MCP scope exception (always global, regardless of package scope).
- [ ] 12.6 Update `doc/prd/storage.md`: add `~/.renkei/mcp/<name>/` to the directory layout; document `install-cache` v3 `mcp_local` section.
- [ ] 12.7 Update `doc/prd/commands.md` if a flag list exists there (fall back to `README.md` otherwise): add `--allow-build`. Add the new `rk doctor` checks to the diagnostics list.
- [ ] 12.8 Update `doc/prd/user-stories.md`: add stories for shipping a local MCP (author), installing a workflow with a bundled MCP (user), and the build-consent UX.
- [ ] 12.9 Update `README.md`: a `renkei.json` example with `entrypoint` + `build`, a one-line note about `--allow-build`, and a short paragraph explaining that MCP sources always deploy globally.
- [ ] 12.10 Update `BACKENDS.md` if it discusses MCP registration: clarify that absolute paths in `args` come from local MCPs.
