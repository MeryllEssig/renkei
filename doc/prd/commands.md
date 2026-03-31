# Commands

## Diagnostics (`rk doctor`)

v1 checks:
- Installed backends (config directory exists)
- Deployed files still exist
- Required environment variables present
- Locally modified skills (SHA-256 hash diff against cached archive)
- Hooks still present in the backend's config file
- MCP configs still registered

No remote version check (registry v2). Exit code 0 if everything passes, non-0 otherwise.

## Archive (`rk package`)

The `.tar.gz` archive contains only:
- `renkei.json`
- `skills/`
- `hooks/`
- `agents/`
- `scripts/`

Everything else (tests, docs, README, etc.) is excluded.
