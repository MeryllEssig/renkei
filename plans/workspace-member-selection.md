# Plan: Install selected workspace members (`-m` flag)

Allow installing a subset of members from a workspace repository (Git or local) via a repeatable `-m` / `--member` flag.

## Context

Today, `rk install <giturl>` on a workspace repo always installs **all** declared members. There is no way to install only `mr-review` from a repo that also ships `auto-test`. The local workaround (`rk install ./repo/mr-review`) does not exist for Git sources, since the clone happens before any subpath resolution.

## Design summary

- **CLI**: `rk install <source> [-m <member>]...` — repeatable, also accepts CSV (`-m a,b`).
- **Symmetry**: works on local AND Git workspace sources.
- **No `-m`**: install all members (current behavior preserved).
- **Validation (fail-fast, before clone/deploy)**:
  - `-m` on a non-workspace source → error.
  - `-m foo` where `foo` not in `workspace` declaration → error listing available members.
  - `-m` combined with no-arg lockfile install → error.
- **Independence**: each selected member is an independent package in `rk.lock` and install-cache. No persisted "selection state". Updates handled by future `rk update`.
- **Lockfile**: new optional field `member: Option<String>` on `LockfileEntry`. Written for both Git and local workspace sources. No `lockfileVersion` bump (additive optional field).
- **Reinstall (no-arg)**: naïve — each lockfile entry processed independently, even when several share `(source, resolved)`. Group-by-source clone optimization deferred.
- **`rk list`**: append `#<member>` suffix to the source line when the field is present.
- **Unchanged**: `rk uninstall` (by `@scope/name`), install-cache layout, archive paths, `--tag` / `--branch` / `--force` / `--backend` (propagated uniformly to every selected member of a single invocation).

## Phase 1: CLI surface and validation

- [x] 1.1 Add `members: Vec<String>` field to `Commands::Install` in `src/cli.rs` (clap `short = 'm'`, `long = "member"`, `value_delimiter = ','`, `action = Append`).
- [x] 1.2 Add error variants in `src/error.rs`:
  - `MemberNotInWorkspace { requested: String, available: Vec<String> }`
  - `MemberFlagOnNonWorkspace`
  - `MemberFlagWithLockfileInstall`
- [x] 1.3 Reject `-m` + no-arg install in `main.rs` dispatch (before any work).

## Phase 2: Workspace install filtering

- [x] 2.1 Extend `install_workspace` in `src/workspace.rs` to accept a `selected: Option<&[String]>` parameter; when `Some`, install only those members; when `None`, current behavior (install all).
- [x] 2.2 Validate every requested member exists in `members` before any install (fail-fast); error lists available members.
- [x] 2.3 Update `install_or_workspace` in `main.rs`: route to `install_workspace(selected)` when manifest declares `workspace`; if `-m` given but manifest has no `workspace`, raise `MemberFlagOnNonWorkspace`.
- [x] 2.4 TDD: tests in `src/workspace.rs` for selected subset, single member, unknown member error, empty selection (= install all), `-m` propagated through `force`.

## Phase 3: Lockfile member field

- [x] 3.1 Add `pub member: Option<String>` to `LockfileEntry` in `src/lockfile.rs` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- [x] 3.2 Plumb `member` through `PackageEntry` → `LockfileEntry::from_package_entry` (or via a new constructor) so the member name reaches the lockfile.
- [x] 3.3 Set `member` when installing a workspace member (Git or local). Leave `None` for non-workspace installs.
- [x] 3.4 TDD: lockfile round-trip with and without `member`; serialization omits the field when `None`.

## Phase 4: Reinstall from lockfile honoring `member`

- [x] 4.1 In the no-arg lockfile install path, when an entry has `member: Some(name)`:
  - Git source: clone the repo, then call `install_local(clone_dir.join(name), ...)`.
  - Local source: install from `<source>/<member>` directly.
- [x] 4.2 TDD: integration test — write a lockfile with two entries from the same Git source (`file://` local bare repo) each with a different `member`, run no-arg install, assert both deployed.

## Phase 5: CLI dispatch wiring

- [x] 5.1 Pass `members` from `Commands::Install` through `run_install` to `install_or_workspace`.
- [x] 5.2 Source-side resolution: when `members` non-empty, force the workspace path (error if manifest is not a workspace).
- [x] 5.3 Propagate `--tag` / `--branch` / `--force` / `--backend` once (single clone, single tag) — already the case via current `InstallOptions`, just verify.

## Phase 6: `rk list` display

- [x] 6.1 In `src/list.rs`, append `#<member>` to the source column when `entry.member.is_some()`.
- [x] 6.2 TDD: snapshot/string assertion on listing output for an entry with a member.

## Phase 7: Integration tests

- [x] 7.1 Add `tests/integration_workspace_member_selection.rs`:
  - Git workspace via `file://` bare repo, `-m mr-review` deploys only `mr-review`, lockfile has 1 entry with `member`.
  - Same with two members `-m a,b` and `-m a -m b`.
  - Member not in `workspace` → exit 1, error message lists available members, no deployment.
  - `-m` on a non-workspace Git repo → exit 1.
  - `-m` combined with no-arg install → exit 1.
  - Reinstall from lockfile honoring `member`.
  - Local workspace symmetry: same scenarios with a local path source.

## Phase 8: Documentation

- [ ] 8.1 Update `doc/prd/installation.md` with the `-m` flag, validation rules, and lockfile `member` field.
- [ ] 8.2 Update `doc/prd/manifest.md` workspace section: mention selective install via `-m`.
- [ ] 8.3 Add a user story under "Installation and deployment" in `doc/prd/user-stories.md` (e.g. US 14h: selective workspace member install).
- [ ] 8.4 Update `README.md` install examples with a `-m` example.
