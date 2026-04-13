# Workspace support

A workspace is a root `renkei.json` with a `workspace` field listing member subdirectories. Each member has its own `renkei.json`.

## Manifest

```json
{
  "name": "@scope/my-workspace",
  "version": "1.0.0",
  "description": "My workspace",
  "author": "Author",
  "license": "MIT",
  "backends": ["claude"],
  "workspace": ["packages/skill-a", "packages/skill-b"]
}
```

## Installation

```bash
rk install ./workspace/                    # installs all members
rk install ./workspace/ -m skill-a         # only skill-a
rk install git@host:repo -m a -m b         # selective, git source
rk install git@host:repo -m a,b            # same, CSV form
```

Each member is cached, deployed, and tracked independently. Members appear separately in `rk list` and each has its own lockfile entry (with a `member` field when installed via `-m`).

Requesting a member not declared in `workspace` fails fast with the list of available members. `-m` on a non-workspace source is rejected.

`rk install` without arguments in a workspace context without `rk.lock` returns an error with guidance toward `rk install --link .`.
