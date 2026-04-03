# Lockfile

The lockfile (`rk.lock`) enables reproducible installs across machines and team members.

## Location

| Scope | Path | Purpose |
|-------|------|---------|
| Project | `rk.lock` at project root | Committable, shared with team |
| Global | `~/.renkei/rk.lock` | Machine-local |

## Format

JSON with `lockfileVersion: 1` and a `packages` object. Each entry contains:
- `version` — semver
- `source` — original install source (path or git URL)
- `tag` — git tag/branch (optional)
- `resolved` — commit SHA (for git sources)
- `integrity` — SHA-256 hash of the cached archive

## Usage

```bash
rk install <source>     # installs and updates rk.lock
rk install              # replays project rk.lock (no args)
rk install -g           # replays global rk.lock (no args)
```

When replaying from lockfile, cached archives are used when available. If missing, the package is re-cloned at the exact commit SHA.

Without a lockfile, `rk install` (no args) returns an error with guidance.
