# Uninstalling a package

```bash
rk uninstall @scope/name       # project scope (default)
rk uninstall -g @scope/name    # global scope
```

Removes all deployed artifacts for the package:
- Skills and agents from `.claude/` (project) or `~/.claude/` (global)
- Hooks from `~/.claude/settings.json`
- MCP servers from `~/.claude.json`
- Package entry from the install-cache
- Package entry from the lockfile (if present)

If the package is not found in the requested scope, an error is returned. There is no cross-scope fallback.
