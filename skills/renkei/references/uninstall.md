# Uninstalling a package

```bash
rk uninstall @scope/name       # project scope (default)
rk uninstall -g @scope/name    # global scope
```

Removes all deployed artifacts for the package:
- Skills and agents from `.claude/` (project) or `~/.claude/` (global)
- Hooks from `~/.claude/settings.json`
- External MCP servers from `~/.claude.json`
- For **local MCPs** (servers shipped under `mcp/<name>/`): decrements the install-cache `mcp_local` ref for the current scope. The `~/.renkei/mcp/<name>/` folder and the backend MCP entry are GC'd only when the **last** ref disappears. A `--link` install removes the symlink only and leaves the workspace source untouched. See [Local MCP servers](local-mcp.md).
- Package entry from the install-cache
- Package entry from the lockfile (if present)

If the package is not found in the requested scope, an error is returned. There is no cross-scope fallback.
