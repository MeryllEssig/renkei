# Testing Decisions

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
