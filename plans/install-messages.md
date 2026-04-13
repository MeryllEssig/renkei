# Plan: Preinstall and postinstall messages

Allow package authors to declare informational `preinstall` and `postinstall` messages in `renkei.json`. Preinstall messages require user confirmation before installation proceeds; postinstall messages are passive notices shown after a successful install.

## Context

Today, package authors have no way to communicate prerequisites or follow-up instructions at install time. Typical needs:
- Warn users that a workflow depends on an external MCP server (e.g. Redmine, GitLab) that they must configure separately.
- Show post-install steps (run `rk doctor`, set an env var, restart Claude Code).

Without this, authors stuff prerequisites in README files that nobody reads before `rk install`.

## Design summary

- **Schema**: new optional top-level object `messages: { preinstall?: string, postinstall?: string }` in `renkei.json`. Plain string, `\n` allowed for multi-line, hard cap 2000 chars. Validation error in `Manifest::validate` if exceeded.
- **Preinstall flow (collect → bloc → 1 prompt)**:
  - Two-phase install: a new "discover all manifests" phase runs *before* any deploy work.
  - Applies to every install variant: single local, single git, lockfile replay, workspace (with or without `-m`).
  - For git sources, the clone happens before the prompt (manifest only readable after clone). Refused clones stay in cache.
  - All collected `preinstall` messages are rendered in a single yellow/bold framed block listing `@scope/name: <message>` per package, followed by one `Install all? [y/N]` prompt.
  - If no package in the batch declares a `preinstall`, the prompt is skipped entirely (silent fast-path).
  - Refusal → exit 0 with `Installation cancelled.`
  - Non-TTY without `--yes` → hard error pointing to `--yes`.
  - New CLI flag `--yes` / `-y` on `rk install`, distinct from `--force`. Only `--yes` bypasses the confirmation; `--force` keeps its current "bypass backend detection" semantics.
  - Always re-prompted on every (re)install, no state tracking.
- **Postinstall flow (info passive)**:
  - Displayed at the end of each successful install, no prompt, no bypass.
  - Order: `Done. Deployed…` → `requiredEnv` warnings → postinstall block(s) (yellow/bold framed).
  - Workspace: one block per member with prefix `@scope/member:`, sequential.
- **Unchanged**: `Manifest` struct fields beyond the new `messages` addition, archive/cache layout, lockfile schema, all other CLI flags.

## Phase 1: Manifest schema and validation

- [x] 1.1 Add `Messages { preinstall: Option<String>, postinstall: Option<String> }` struct in `src/manifest.rs` with `serde(rename_all = "lowercase")` (snake_case keys: `preinstall`, `postinstall`).
- [x] 1.2 Add `pub messages: Option<Messages>` field to `Manifest` with `#[serde(default)]`.
- [x] 1.3 In `Manifest::validate`, fail with `RenkeiError::InvalidManifest` if either message exceeds 2000 chars; include the field name in the error.
- [x] 1.4 Carry `messages: Option<Messages>` onto `ValidatedManifest` (or read from `raw_manifest` downstream — pick whichever keeps changes localized). _→ Decision: read from `raw_manifest` downstream; `ValidatedManifest` left untouched (per humble-drifting-hennessy plan note 1)._
- [x] 1.5 TDD: parse manifest with both messages, with only one, with neither; reject manifest where a message > 2000 chars; reject unknown nested key inside `messages` only if cheap, otherwise allow (lean on serde defaults). _→ 6 new tests, all green._

## Phase 2: CLI flag

- [x] 2.1 Add `#[arg(short = 'y', long = "yes")] yes: bool` to `Commands::Install` in `src/cli.rs`.
- [x] 2.2 Plumb `yes` from `run_install` down to the install dispatch (alongside `force`, `members`, etc.). _→ Plumbed through `run_install`, `run_install_from_lockfile`, and `install_or_workspace`; consumed by Phase 4 coordinator (currently parked as `_yes`)._
- [x] 2.3 TDD: clap parsing test — `rk install ./pkg --yes` and `-y` both set the flag; default is `false`.

## Phase 3: Confirmation prompt module

- [x] 3.1 New module `src/install/messages.rs` (or extend `install/mod.rs`) exposing:
  - `struct PreinstallNotice { full_name: String, message: String }`
  - `fn collect_preinstall(manifests: &[&Manifest]) -> Vec<PreinstallNotice>`
  - `fn confirm_preinstall(notices: &[PreinstallNotice], yes: bool) -> Result<bool>` — returns `Ok(true)` to proceed, `Ok(false)` if the user declined, `Err` on non-TTY without `--yes`.
- [x] 3.2 Rendering: yellow/bold framed block via `owo_colors`. Title `"Preinstall notice:"`, then one line per package: `  @scope/name: <message>` (multi-line messages indented).
- [x] 3.3 Prompt via `inquire::Confirm` (already a dep — see `install/mod.rs::prompt_rename`); default `false`; final question `Install all? [y/N]`.
- [x] 3.4 Non-TTY detection via `std::io::stdin().is_terminal()` (same pattern as `default_resolver`). Error message: `"Refusing to prompt in non-interactive mode. Re-run with --yes to accept all preinstall notices."`
- [x] 3.5 Refusal path: print `"Installation cancelled."` and return `Ok(false)`. Caller exits 0.
- [x] 3.6 TDD: empty notices → no prompt, returns `Ok(true)`. With `yes = true` → no prompt, `Ok(true)`. Non-TTY + `yes = false` → `Err`. Render snapshot test for the framed block (string assertion). _→ 9 tests, all green._

## Phase 4: Install pipeline refactor (two-phase)

- [ ] 4.1 Extract a "discover only" entrypoint that loads `Manifest` for a single source (local path or post-clone git dir) without running `cleanup_and_resolve`/`deploy`. Likely `CorePipeline::discover` is already that — verify and reuse.
- [ ] 4.2 Introduce a coordinator (likely in `src/install/mod.rs` or a new `src/install/batch.rs`) that:
  1. Resolves all sources for the current invocation (single, workspace expansion, lockfile entries) into local paths (cloning git as needed).
  2. Calls `CorePipeline::discover` for each and collects manifests.
  3. Calls `confirm_preinstall(notices, yes)`.
  4. On `Ok(true)` → for each pipeline, run `cleanup_and_resolve` + `deploy` + lockfile/store recording (existing logic, factored).
  5. On `Ok(false)` → exit 0, leave clones in cache.
- [ ] 4.3 Single-package non-workspace install (`install_local`): wrap in the coordinator with a single-element batch; preserve current behavior when no preinstall is declared.
- [ ] 4.4 Workspace install (`install_workspace`): batch all selected members; one prompt for the lot.
- [ ] 4.5 Lockfile replay (no-arg `rk install`): batch all entries; one prompt; non-TTY behaviour same as everything else.
- [ ] 4.6 TDD: per-path tests asserting collection across multiple manifests, refusal short-circuits before any deploy, `--yes` bypasses prompt across all paths, single package without `messages` keeps current zero-prompt UX.

## Phase 5: Postinstall display

- [ ] 5.1 Extend `print_post_deploy` in `src/install/mod.rs` to take an optional `postinstall: Option<&str>` and a `package_label: Option<&str>` (used in workspace mode for the `@scope/member:` prefix).
- [ ] 5.2 Render order: existing `Done.` line → existing `requiredEnv` warnings → postinstall block (yellow/bold framed).
- [ ] 5.3 In the workspace coordinator, after each member deploy, accumulate postinstalls; print them in order at the end of the batch (one block per member with prefix).
- [ ] 5.4 TDD: single package without postinstall → unchanged output. With postinstall → block appears after env warnings. Workspace with N postinstalls → N blocks in order, each labeled. Workspace where some members have no postinstall → only the ones with messages render.

## Phase 6: Error variant and exit codes

- [ ] 6.1 Add `RenkeiError::PreinstallDeclined` (or reuse a generic cancellation path) — but per design, refusal is `exit 0`, not an error. So this is just a control-flow `bool`/early-return, not an error variant.
- [x] 6.2 Add `RenkeiError::PreinstallRequiresConfirmation` for the non-TTY case, with the suggested `--yes` message. _→ Landed alongside Phase 3 because `confirm_preinstall`'s signature depends on it._
- [ ] 6.3 Wire the early-return through `main.rs` so refusal does not produce an error stack trace.

## Phase 7: Integration tests

- [ ] 7.1 Add `tests/integration_install_messages.rs`:
  - Single local install with `messages.preinstall`: TTY simulated → prompt accepted → installs; refused → exits 0, no artifacts deployed, no lockfile entry.
  - Same with `--yes`: no prompt, installs.
  - Same in non-TTY (stdin redirected from `/dev/null`) without `--yes` → exit non-zero, error mentions `--yes`.
  - Single local install with `messages.postinstall`: postinstall block appears in stdout after `Done.` and after `requiredEnv` warnings (test with both env warnings present and absent).
  - Workspace install with two members each declaring `preinstall`: a single prompt lists both `@scope/name:` lines. Refusal → neither member installed.
  - Workspace install with two members each declaring `postinstall`: two labeled blocks in order at the end.
  - Lockfile replay where one entry declares `preinstall`: prompt appears; `--yes` bypasses; refusal exits 0 and leaves no half-state.
  - Manifest with `messages.preinstall` exceeding 2000 chars → install fails with manifest validation error pre-clone (or pre-deploy for git).

## Phase 8: Documentation

- [ ] 8.1 Update `doc/prd/manifest.md`: document the `messages` object, both fields, the 2000-char cap, and what happens at install time.
- [ ] 8.2 Update `doc/prd/installation.md`: describe the preinstall confirmation flow, the `--yes` flag, the non-TTY error, and the postinstall display order.
- [ ] 8.3 Update `doc/prd/commands.md`: add `-y` / `--yes` to the `rk install` flag list.
- [ ] 8.4 Add a user story under "Installation and deployment" in `doc/prd/user-stories.md` (preinstall consent + postinstall notice).
- [ ] 8.5 Update `README.md` with a `messages` example in the `renkei.json` snippet and an `--yes` example in the install snippets.
- [ ] 8.6 Update `PRD.md` index if the new content lives in a new sub-file (it should not — extend existing files).
