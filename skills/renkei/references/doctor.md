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

Output shows checkmark/cross per check, grouped by package. Exit code 0 if healthy, 1 if problems found.
