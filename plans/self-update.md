# Plan: `rk self-update` + version notification

## Phase 1: Version alignment
- [x] 1.1 Bump `Cargo.toml` version to `1.0.0`
- [x] 1.2 Add CI guard in `release.yml` that fails if tag doesn't match `Cargo.toml` version

## Phase 2: Core self-update module
- [x] 2.1 Add `ureq` dependency to `Cargo.toml`
- [x] 2.2 Create `src/self_update.rs` — GitHub API client, version comparison, binary download+replace
- [x] 2.3 Add error variants for self-update in `error.rs`
- [x] 2.4 Add `SelfUpdate` command to `cli.rs`
- [x] 2.5 Wire `self-update` in `main.rs`

## Phase 3: Background version check + notification
- [x] 3.1 Create `src/update_notifier.rs` — cache file, background thread check, notification display
- [x] 3.2 Integrate notifier in `main.rs` (spawn on start, display after command, skip for self-update, skip if not TTY)

## Phase 4: Tests
- [x] 4.1 Unit tests for version comparison, cache staleness, artifact name resolution
- [x] 4.2 Run full test suite to validate no regressions (511 tests pass)
