# Commands

## Install (`rk install`)

Flags relevant to local MCPs (full reference in [MCP](./mcp.md) and [Installation](./installation.md)):

- `--allow-build` — accept every declared local-MCP `build` step in the batch without prompting. Required in non-interactive environments when at least one local MCP needs building.
- `--link` — symlink the source instead of copying it; for local MCPs, also symlinks `~/.renkei/mcp/<name>/` to the workspace folder and skips the build entirely.
- `--force` — among other things, transfers ownership of a local MCP from another package to the one being installed.

## Diagnostics (`rk doctor`)

v1 checks:
- Installed backends (config directory exists)
- Deployed files still exist
- Required environment variables present
- Locally modified skills (SHA-256 hash diff against cached archive)
- Hooks still present in the backend's config file
- MCP configs still registered
- **Local MCP folders** — for each entry in `install_cache.mcp_local`:
  - `exists`: `~/.renkei/mcp/<name>/` (or symlink) is present (error if missing).
  - `entrypoint`: the file at `<folder>/<entrypoint>` exists (error if missing).
  - `integrity`: SHA-256 of the deployed content matches the recorded `source_sha256` (warning on drift; expected for `--link` installs).

No remote version check (registry v2). Exit code 0 if everything passes (warnings allowed), non-0 if any error-level check fails.

## Archive (`rk package`)

The `.tar.gz` archive contains:
- `renkei.json`
- `skills/`
- `hooks/`
- `agents/`
- `scripts/`
- `mcp/<name>/` — local-MCP source trees, filtered through `.rkignore`.

Default exclusions (always applied): `node_modules/`, `dist/`, `build/`, `target/`, `.venv/`, `venv/`, `__pycache__/`, `.pytest_cache/`, `*.pyc`, `.DS_Store`, `.git/`. Inside `mcp/<name>/` the `dist/build/target` defaults are relaxed so prebuilt entrypoints survive. A `.rkignore` at the package root extends the default list.

Everything else (tests, docs, README, etc.) is excluded.
