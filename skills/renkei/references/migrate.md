# Migration

```bash
rk migrate <path>
```

Scans an existing directory and generates a valid `renkei.json` manifest, reorganizing files into the conventional directory structure (`skills/`, `hooks/`, `agents/`).

The migrated package should pass `rk package` without errors.
