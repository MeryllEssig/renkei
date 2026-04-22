# Plan: `.agents/` as the universal default, vendors declarative

> Source PRD: `PRD.md` + `doc/prd/backends.md` + `doc/prd/multi-backend.md` + `doc/prd/scope.md`
>
> Follow-up to `plans/renkei-cli-multi-backend.md`. Aligns renkei's deployment model with `skills.sh`: `.agents/skills/` is the always-included "universal" destination for skills; specific vendors (`.claude/`, `.cursor/`, `.codex/`, `.gemini/`) become pure declarative opt-ins.

## Context

Today `rk install` deploys a skill to every backend whose directory exists on disk (filesystem detection) and that is listed in the manifest's `backends` field. This has two UX problems:

1. **Filesystem detection is implicit and surprising**: a user who tried Claude Code once and kept `~/.claude/` around will get renkei skills leaking into it even if they've moved on to Codex.
2. **No universal fallback**: a skill-only package that doesn't declare a vendor is rejected (`backends` is required, non-empty). Codex/Gemini users have no path to "just give me the skill, I don't care about the vendor".

`skills.sh` solves both by enforcing `.agents/skills/` as always-included (universal) and making every vendor an explicit, additive opt-in the user chooses. This plan brings renkei to the same model.

## Design summary

- **Universal `.agents/skills/`**: every skill install always deploys a copy to `.agents/skills/<name>/`. The `agents` backend is no longer declarable — it's implicit and always active. Attempting `backends: ["agents"]` in a manifest becomes a validation error.
- **Vendors are opt-in, declarative**: a user targets `.claude/`, `.cursor/`, `.codex/`, or `.gemini/` only by:
  - `rk config set defaults.backends claude,cursor` (persistent), **or**
  - `rk install --backend claude ./pkg` (one-shot; prints a note suggesting `rk config set` for persistence).
  - **CLI flag merges with config, not overrides**: `--backend codex` + `defaults.backends=[claude,cursor]` → effective `[claude,cursor,codex]`. The tip message clarifies the addition is temporary.
  - **Unknown backend names are rejected** at both entry points (`rk config set` and `--backend`) with a hard error listing valid names.
  - **`"agents"` is rejected symmetrically**: `rk config set defaults.backends agents,claude` errors the same way the manifest validation does.
  - **Filesystem detection is abandoned**: having `~/.claude/` on disk no longer implies anything. The `Backend::detect_installed()` helper and the "detected" filter in `BackendRegistry::resolve` are removed from the install path. (Kept alive only for `rk doctor` as an informational signal.)
- **`manifest.backends` becomes optional**: defaults to `[]`. When empty, the package is "skill-only universal" — it deploys only to `.agents/skills/`. When non-empty, it declares a **hard compatibility requirement**: "this package ships at least one vendor-specific artifact (hook, agent, MCP) that requires one of these vendors to be a selected target".
- **Strict intersection for non-skill-only packages**: if `manifest.backends` is non-empty, `(user_selected_backends) ∩ (manifest.backends)` **must** be non-empty, or `rk install` hard-errors with a message pointing at `rk config set defaults.backends <x>`. **`--force` does NOT bypass this rule** (unlike the pre-rework `--force` which bypassed filesystem-detection intersection). The scope of `--force` is now limited to file-level conflict resolution (overwriting existing artifacts).
- **Workspace atomic validation**: when installing a workspace, ALL member manifests are validated (including the intersection check) before any deploy runs. If one member fails, the whole batch aborts — cohérent with the existing preinstall-prompt flow.
- **Copies, not symlinks**: all deployments are independent file copies. No shared canonical source. Simple, uniform, no vendor-specific edge cases (e.g. Cursor's injected frontmatter + `.mdc` extension doesn't break symlinks because there are none).
- **Dedup preserved**: Codex and Gemini read `.agents/skills/` natively. `reads_agents_skills()` stays, and when the vendor is selected, its own `deploy_skill` is skipped (single copy under `.agents/skills/`). Claude and Cursor get their own copies because they don't read `.agents/`.
- **No auto-migration**: existing installs whose skills live under `.claude/skills/` (or elsewhere) stay put until the user does `rk uninstall && rk install`. A warning at `rk doctor` flags packages installed under the legacy resolution model, detected via an explicit `PackageEntry.schema_version` field in the install cache (new installs write `2`, entries without the field are assumed `1`).
- **One-shot educational notice**: the very first `rk install` on a machine where `defaults.backends` is unset prints a framed message explaining the opt-in model. State persisted in `~/.renkei/state.json`.
- **Lockfile replay uses local config**: `rk install` with no args reads `rk.lock` for package versions but uses the **local** `defaults.backends` for destinations. The recorded `deployed_map` in the cache is informational only during replay.
- **Collateral: Cursor frontmatter fix**: while the deploy pipeline is being reworked, fix `src/backend/cursor.rs` to extract the skill's `description` (from the neutral SKILL.md frontmatter) and emit it in the `.mdc` frontmatter, so Cursor actually auto-discovers the rule instead of falling back to manual-only mode.

## Breaking changes

1. `manifest.backends = ["agents"]` rejected at validation (previously accepted).
2. `manifest.backends = []` or field omitted is now accepted (previously rejected).
3. Filesystem detection (`~/.claude/` exists → claude is targeted) is removed from the install path. Users who relied on implicit detection will see their skills land only in `.agents/skills/` until they run `rk config set defaults.backends`.
4. `--force` no longer bypasses backend selection. It is strictly a file-conflict overwrite flag now.
5. `"agents"` is rejected in `defaults.backends` (user config) the same way it is in `manifest.backends`.
6. Install cache schema: **new field** `PackageEntry.schema_version: u32` (default `1` when absent, always `2` for new installs). `rk.lock` schema: no change required.

Acceptable because renkei v1.x is not yet widely adopted (confirmed per conversation).

---

## Phase 1: Manifest schema changes

- [x] 1.1 In `src/manifest.rs`, change `pub backends: Vec<String>` to accept `#[serde(default)]` so the field is optional and defaults to `[]`.
- [x] 1.2 In `Manifest::validate`, remove the "backends must contain at least one entry" check. Add a new check: reject `"agents"` as a declared backend with `RenkeiError::InvalidManifest("\"agents\" cannot be declared in backends — it is always active implicitly")`.
- [x] 1.3 Add a helper `Manifest::is_skill_only(&self) -> bool` that returns `true` iff `backends.is_empty()`.
- [x] 1.4 TDD: (a) manifest omitting `backends` parses with empty vec; (b) manifest with `backends: []` parses; (c) manifest with `backends: ["agents"]` fails with the explicit error; (d) manifest with `backends: ["claude", "agents"]` also fails; (e) manifest with `backends: ["claude"]` parses as non-skill-only.
- [x] 1.5 Update `renkei.json` fixtures in `tests/fixtures/` that currently declare `["agents"]` — replace with `[]` or relevant vendor. **Deviation (implementation note):** updating these fixtures made seven integration tests fail because the current resolver still uses strict intersection and the assertions depended on `"agents"` being declared. Those tests were marked `#[ignore = "Phase 2 rework: assertions assume pre-opt-in resolver"]` to keep the tree green until Phase 2 reintroduces the `.agents/` deployment via `resolve_for_install` + always-on `AgentsBackend`. Tests tagged: `test_install_multi_backend_claude_and_agents`, `test_install_dedup_agents_codex`, `test_install_dedup_agents_gemini`, `test_install_uses_config_backends`, `test_install_falls_back_to_autodetect_without_config`, `test_list_shows_per_backend_breakdown`, `test_uninstall_removes_from_all_backends`. Phase 2 must un-ignore and rewrite their assertions.

## Phase 2: Backend resolution — abandon filesystem detection

- [ ] 2.1 In `src/backend/mod.rs`, rename the current `BackendRegistry::resolve()` to `resolve_for_doctor()` (kept for diagnostic use) and introduce a new `resolve_for_install()` that does NOT call `detect_installed()` on any backend.
- [ ] 2.2 New `resolve_for_install` signature: `fn resolve_for_install(&self, manifest_backends: &[String], user_selected: &[String]) -> Result<Vec<&dyn Backend>>`. Always includes `AgentsBackend`. Intersects `user_selected` with the registry's vendor list (claude/cursor/codex/gemini). If `manifest_backends` is non-empty, enforces the strict intersection rule (Phase 3).
- [ ] 2.3 New helper `fn validate_backend_names(names: &[String]) -> Result<()>` — returns `RenkeiError::UnknownBackend { name, valid: &["claude","cursor","codex","gemini"] }` if any name is unknown, and `RenkeiError::AgentsNotDeclarable` if `"agents"` is in the list. Call from both `rk config set defaults.backends` and `--backend` flag parsing.
- [ ] 2.4 Remove `ClaudeBackend::detect_installed`, `CursorBackend::detect_installed`, etc. being called from the install path. The methods stay on the trait but are consumed only by `rk doctor`.
- [ ] 2.5 TDD: (a) empty `manifest_backends` + empty `user_selected` → resolves to `[agents]` only; (b) empty `manifest_backends` + `user_selected = ["claude"]` → `[agents, claude]`; (c) non-empty manifest `["claude"]` + `user_selected = ["cursor"]` → hard error (no intersection); (d) manifest `["claude"]` + `user_selected = ["claude", "cursor"]` → `[agents, claude, cursor]`; (e) `validate_backend_names(["foo"])` → `UnknownBackend`; (f) `validate_backend_names(["agents"])` → `AgentsNotDeclarable`.

## Phase 3: Strict intersection rule for non-skill-only packages

- [ ] 3.1 New `RenkeiError::BackendRequirementUnmet { required: Vec<String>, selected: Vec<String> }` with a message pointing at `rk config set defaults.backends <one of required>`.
- [ ] 3.2 In `resolve_for_install`, when `manifest_backends` is non-empty, compute `required = manifest_backends` and check that `user_selected ∩ required` is non-empty. If empty → return `BackendRequirementUnmet`. **`--force` does NOT bypass this check** (sever the old `force: true` → skip-intersection path; `--force` only affects conflict resolution now).
- [ ] 3.3 TDD: (a) skill-only (`manifest_backends = []`) + no user selection → no error, resolves to `[agents]`; (b) `manifest_backends = ["claude"]` + no user selection → `BackendRequirementUnmet`; (c) `manifest_backends = ["claude", "cursor"]` + `user_selected = ["cursor"]` → OK; (d) `manifest_backends = ["claude"]` + `user_selected = ["cursor"]` → error; (e) case (d) + `force = true` → STILL error (regression test for the `--force` scope change).
- [ ] 3.4 Error message smoke test: the human-readable message must include all three of: the package's required backends, the user's currently selected backends, and the exact `rk config set defaults.backends <backend>` command.
- [ ] 3.5 Workspace atomic validation: extend the existing preinstall-prompt two-phase pipeline (`install-messages.md` Phase 4 coordinator) so that the `resolve_for_install` check runs for **every** member's manifest BEFORE any deploy. Any member failing intersection aborts the whole batch with a single consolidated error listing each offending `(member, required, selected)` tuple. TDD: two-member workspace where one member fails → neither member is deployed.

## Phase 4: User config + CLI flag

- [ ] 4.1 Confirm `~/.renkei/config.json` already stores `{ "defaults": { "backends": [...] } }` (via `rk config set defaults.backends ...`). If not, wire it up. `src/user_config.rs` is the existing home for this.
- [ ] 4.2 In `src/cli.rs`, ensure the existing `--backend <name>` flag on `rk install` is plumbed through to the resolver. Flag is repeatable and also CSV-tolerant (`--backend claude,cursor`).
- [ ] 4.3 **Merge semantics**: the effective `user_selected` is `config.defaults.backends ∪ cli_flags_backends` (union, deduplicated). Example: config `[claude,cursor]` + `--backend codex` → effective `[claude,cursor,codex]`.
- [ ] 4.4 In `rk config set defaults.backends <csv>`, call `validate_backend_names` (Phase 2.3). Reject unknown names with the full valid-list, reject `"agents"` with the not-declarable message. Persist only after validation.
- [ ] 4.5 When `--backend` is used on the CLI, after a successful install, print a dimmed note: `Tip: \`--backend\` additions are temporary. Persist with \`rk config set defaults.backends <csv>\`` (where `<csv>` is the effective merged set). Suppressed if the CLI flag added nothing new beyond what's already in config.
- [ ] 4.6 TDD: (a) `--backend claude` + empty config → effective `["claude"]`, tip printed; (b) `--backend codex` + config `["claude","cursor"]` → effective `["claude","cursor","codex"]`, tip printed mentioning the codex addition; (c) `--backend claude` + config `["claude"]` → effective `["claude"]`, tip suppressed (no addition); (d) no flag + config empty → effective `[]`; (e) no flag + config `["claude","cursor"]` → effective `["claude","cursor"]`; (f) `--backend foo` → hard error before any install; (g) `rk config set defaults.backends agents,claude` → hard error, config unchanged.

## Phase 5: One-shot educational notice

- [ ] 5.1 New `src/state.rs` (or reuse existing `~/.renkei/state.json` machinery if present) exposing `fn has_shown_backend_notice() -> bool` and `fn mark_backend_notice_shown() -> Result<()>`.
- [ ] 5.2 On every `rk install` entrypoint (single, workspace, lockfile replay), BEFORE the preinstall prompt but AFTER source resolution, if `user_config.defaults.backends.is_empty()` AND `!has_shown_backend_notice()` → print a framed yellow block:
  ```
  Heads up:
    Renkei is deploying skills to .agents/skills/ only — the universal destination
    read by Codex and Gemini natively. Claude Code and Cursor do NOT read .agents/,
    so your skills will not appear there until you opt in:

      rk config set defaults.backends claude,cursor

    This notice shows once per machine.
  ```
  Then call `mark_backend_notice_shown()`.
- [ ] 5.3 TDD: (a) first install with no config → notice printed, state file written; (b) second install → no notice; (c) first install with non-empty config → no notice, state file still written (so an empty-config user won't suddenly get the notice if they unset their config later).

## Phase 6: Deploy pipeline — `.agents/` always active

- [ ] 6.1 In `src/install/deploy.rs`, verify that the order of iteration in `deploy_to_backends` puts `agents` first (currently it follows registry insertion order, which is `[claude, agents, cursor, codex, gemini]` — reorder so `agents` is always first, so that a failure in a vendor rollbacks the agents-side copy deterministically).
- [ ] 6.2 The existing dedup logic (`reads_agents_skills() && has_agents` → skip) stays unchanged. Verify Codex and Gemini still return `true` and Claude/Cursor still return `false`.
- [ ] 6.3 Remove `is_unsupported_for_backend("agents", Agent|Hook)` hack from `deploy.rs:17-22`. Now that `.agents/` is always active and skill-only, the loop should naturally skip non-skill artifacts for the agents backend via a clean `if backend.name() == "agents" && !matches!(art.kind, Skill) { continue; }`. (Or, better, add a `fn supports(&self, kind: &ArtifactKind) -> bool` to the `Backend` trait and drive all "skip unsupported" decisions from there.)
- [ ] 6.4 TDD: integration test — a package with one skill + one hook + `backends: ["claude"]` + `user_selected = ["claude"]` deploys the skill under both `.agents/skills/foo/` AND `.claude/skills/foo/`, and the hook only under `~/.claude/settings.json`.
- [ ] 6.5 TDD: integration test — a skill-only package (`backends: []`) + `user_selected = []` deploys the skill ONLY under `.agents/skills/foo/`, nothing under `.claude/`, `.cursor/`, etc.

## Phase 7: Cursor frontmatter fix (collateral)

- [ ] 7.1 In `src/frontmatter.rs`, expose `fn parse_fields(source: &str) -> Option<serde_yaml::Mapping>` that parses the YAML frontmatter of a neutral SKILL.md and returns its fields. Also expose `fn strip_frontmatter(source: &str) -> &str` returning the body without the leading `---`-delimited block.
- [ ] 7.2 In `src/backend/cursor.rs::deploy_skill`, replace the hard-coded injection `---\ndescription: ""\nalwaysApply: false\n---\n` with a composed frontmatter:
  - `description`: from the source frontmatter's `description`, or fallback to `"Skill: <name>"`.
  - `alwaysApply`: **passthrough** from the source if present, else `false`.
  - `globs`: **passthrough** from the source if present, else omitted.
  - Strip the source's frontmatter from the body before concatenating, so the `.mdc` contains exactly one frontmatter block followed by the body.
- [ ] 7.3 Document in `doc/prd/backends.md` (Phase 11) that Cursor-specific frontmatter fields (`alwaysApply`, `globs`) placed in the neutral SKILL.md are propagated to Cursor and ignored by other backends (Claude/Codex/Gemini's parsers skip unknown fields).
- [ ] 7.4 TDD: (a) source with `description: "Review code"` → `.mdc` frontmatter has `description: "Review code"`; (b) source with no frontmatter → `.mdc` has `description: "Skill: <name>"`, `alwaysApply: false`; (c) source with frontmatter but no `description` → same fallback on description, other fields preserved; (d) source frontmatter is stripped from the body, body is not duplicated; (e) source with `alwaysApply: true` → `.mdc` has `alwaysApply: true` (passthrough); (f) source with `globs: "**/*.ts"` → `.mdc` has `globs: "**/*.ts"` (passthrough); (g) other backends (Claude/Codex/Gemini) deploying the same source with `alwaysApply: true` do NOT break or emit the field into their output.

## Phase 8: Lockfile replay — local config wins

- [ ] 8.1 In `src/install/mod.rs::install_from_lock_entry`, verify that the `backends` argument (selected by the caller from local user config) is what's used for deploy, NOT the `deployed_map` keys recorded in the entry.
- [ ] 8.2 The only use of the recorded `deployed_map` in `rk.lock` during replay is to know which paths to clean up if the package is being re-installed over a previous version. (Already handled by `cleanup_previous_installation`.)
- [ ] 8.3 TDD: scenario test — record an install with `user_selected = ["claude","cursor"]`, commit `rk.lock`, change local config to `["claude"]` only, run `rk install` (no args, replay) — assert that the skill is deployed under `.agents/` + `.claude/` only, NOT `.cursor/`.

## Phase 9: Doctor updates + schema_version

- [ ] 9.1 In `src/install_cache.rs`, add `#[serde(default = "default_schema_version")] pub schema_version: u32` to `PackageEntry`. `default_schema_version()` returns `1` (for existing entries). New installs set `schema_version = 2`. Bump the `INSTALL_CACHE_SCHEMA` module-level constant if one exists.
- [ ] 9.2 In `src/doctor/report.rs`, add a new `DiagnosticKind::LegacyInstall` check: for every package in the install cache with `schema_version < 2`, emit a diagnostic "Package `@x/y` was installed under the legacy resolution (pre-universal-agents). Run `rk uninstall @x/y && rk install <source>` to rebalance.".
- [ ] 9.3 In `rk doctor`'s backend status section, keep the filesystem-detection signal (`.claude/` exists on disk) but label it "detected on filesystem" — purely informational, no action taken. If a user has a vendor detected on disk but NOT in `defaults.backends`, print an informational line: "Backend `claude` is installed on this machine but not in your `defaults.backends`. Run `rk config set defaults.backends ...,claude` to publish skills there."
- [ ] 9.4 TDD: (a) `PackageEntry` with `schema_version: 1` → `LegacyInstall` diagnostic; (b) `PackageEntry` with `schema_version: 2` → no diagnostic; (c) deserializing a cache from v1 without the field → `schema_version` defaults to `1`; (d) user has `~/.claude/` but empty config → informational line printed in doctor output.

## Phase 10: Uninstall verification

- [ ] 10.1 Read `src/uninstall.rs` and verify: uninstall iterates every entry of `deployed_map` for every backend the package has entries for, removes each `deployed_path`, and deletes the install-cache entry. Given copies-everywhere, the canonical `.agents/skills/foo/` and every vendor copy are independent `deployed_path`s — no special ordering needed.
- [ ] 10.2 TDD: install a skill with `user_selected = ["claude"]`, assert copies exist at both `.agents/skills/foo/` and `.claude/skills/foo/`, run uninstall, assert both are gone.
- [ ] 10.3 TDD: install a skill-only package (`.agents/` only), uninstall, assert `.agents/skills/foo/` is gone.

## Phase 11: Documentation updates

- [ ] 11.1 Update `PRD.md`: reword the backend section to introduce `.agents/` as the universal default and the declarative-opt-in model for vendors.
- [ ] 11.2 Update `doc/prd/backends.md`: document the two classes (universal `agents`, vendors) and the new opt-in mechanics.
- [ ] 11.3 Update `doc/prd/multi-backend.md`: rewrite the resolution pipeline (filesystem detection → explicit opt-in). Document the strict intersection rule for non-skill-only packages.
- [ ] 11.4 Update `doc/prd/manifest.md`: mark `backends` as optional, default `[]`; document that `"agents"` is forbidden.
- [ ] 11.5 Update `doc/prd/scope.md` if it references filesystem detection.
- [ ] 11.6 Update `BACKENDS.md`: add a "How Renkei targets each backend" section summarizing the universal-plus-opt-in model. Remove any remaining mention of filesystem-based detection as the activation trigger.
- [ ] 11.7 Update `README.md`:
  - In "Quick Start", remove the example manifest's `"backends": ["claude", "cursor"]` (show the skill-only default form and the explicit-vendor form side by side).
  - In "Supported Backends", adjust the table heading to clarify `.agents/skills/` is always-on.
  - In "Usage", add a note about `rk config set defaults.backends <csv>` being the recommended first-run step.
  - In "Manifest Reference", change the `backends` row from "Required" to "Optional" with a sentence on the skill-only vs vendor-requiring semantics.
  - Add a "Breaking changes from v1.x" section (or a CHANGELOG entry) listing the three breaks from the Breaking Changes list above.

## Phase 12: Skill `renkei` update (`skills/renkei/`)

- [ ] 12.1 `skills/renkei/SKILL.md`: revise the top-level description if needed (no major change expected since the SKILL.md just points at references).
- [ ] 12.2 `skills/renkei/references/install.md`: rewrite to describe the new opt-in model — `.agents/` universal, vendors opt-in via `rk config set`/`--backend`, one-shot notice, strict intersection error example.
- [ ] 12.3 `skills/renkei/references/creating-a-package.md`: document that `backends` is optional; show a skill-only example (`backends` omitted) and a vendor-requiring example side by side.
- [ ] 12.4 `skills/renkei/references/doctor.md`: mention the new `LegacyInstall` diagnostic and the informational "backend detected on filesystem but not in your config" line.
- [ ] 12.5 `skills/renkei/references/lockfile.md`: document that lockfile replay uses the local `defaults.backends`, not the recorded deployments.

## Phase 13: Migration cues (no auto-migration)

- [ ] 13.1 On any `rk install` when the install cache contains entries with `schema_version < 2`, print a one-time warning at the end of the install: "N package(s) in your install cache were installed under the legacy resolution. Run `rk doctor` for details and `rk uninstall <pkg> && rk install <source>` to rebalance each one." Suppressed on subsequent installs in the same session via `state.json` equivalent, OR printed once per `rk install` invocation regardless — pick the less-annoying variant during implementation, default to once-per-invocation.
- [ ] 13.2 TDD: synthesize an install cache with mixed `schema_version` values (1 for one entry, 2 for another), run `rk install ./some-other-pkg`, assert the legacy warning appears and mentions the count correctly.

## Post-implementation report

At the end, summarize for the user:
- Packages that declare `backends: ["agents"]` now fail manifest validation (breaking).
- `rk install` on a machine with no `defaults.backends` + no `--backend` flag will only deploy to `.agents/skills/`. Users who had `~/.claude/` and relied on implicit detection must now `rk config set defaults.backends claude`.
- The one-shot notice shows once per machine and is idempotent.
- `rk doctor` surfaces legacy installs and filesystem-vs-config mismatches.
- `rk.lock` replay ignores the recorded backends and uses the local user config.

## Decision log

- Copies everywhere, no symlinks: consistency, no vendor-specific edge cases, no symlink chain in `--link` mode, no broken-symlink diagnostics needed.
- Filesystem detection abandoned for installs: explicit opt-in is worth the one-time "why isn't my skill in `.claude/` anymore" surprise — softened by the one-shot notice + `rk doctor` hints.
- `"agents"` forbidden in both `manifest.backends` and `defaults.backends` (user config): implicit-always-on makes declaring it noise or wrong-signal. Symmetric rejection is cleaner than silent drop.
- `--backend` merges with `defaults.backends` (union), not overrides: the CLI flag is additive, matching the "explicit opt-in accumulates" philosophy.
- `--force` no longer bypasses backend selection: the old behavior dates from the detection-based model where the intersection was an auto-discovery check, not a real requirement. Under the new model, the intersection encodes a hard compatibility requirement that must not be silenced.
- Workspace validates atomically: consistent with the preinstall-prompt two-phase pipeline. No partial installs.
- `PackageEntry.schema_version` (explicit) rather than inferring legacy from missing `agents` key: advisor-recommended, more robust to future schema evolution.
- Cursor frontmatter fix folded into this plan (Phase 7): the skill pipeline is being touched anyway; fixing the `description: ""` → manual-only-mode bug here is cheaper than deferring. Plus: `alwaysApply`/`globs` passthrough so skill authors can target Cursor auto-matching when they want to, without breaking other backends.
