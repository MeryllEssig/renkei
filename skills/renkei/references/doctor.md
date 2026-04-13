# Diagnosing issues

```bash
rk doctor        # check project-scoped packages (default)
rk doctor -g     # check globally installed packages
```

Runs health checks on all installed packages:

1. **Backend detection** — installed backends (config directory exists)
2. **Deployed files** — all deployed artifacts still exist on disk
3. **Environment variables** — required env vars are present
4. **Integrity** — skills match their cached archive (SHA-256 hash comparison)
5. **Hooks** — hooks still present in `settings.json`
6. **MCP configs** — MCP servers still in `~/.claude.json`
7. **Local MCP folders** — for each entry under `install_cache.mcp_local`:
   - `exists` (error): `~/.renkei/mcp/<name>/` (or symlink) is present.
   - `entrypoint` (error): the file at `<folder>/<entrypoint>` exists.
   - `integrity` (warning): SHA-256 of the deployed source matches the recorded `source_sha256`. Drift is expected for `--link` installs and post-build user tampering.

Output shows checkmark/cross per check, grouped by package, plus a separate "Local MCPs" section. Exit code 0 if healthy (warnings allowed), 1 if any error-level check fails.
