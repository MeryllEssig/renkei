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
rk install ./workspace/      # installs all members
```

Each member is cached, deployed, and tracked independently. Members appear separately in `rk list` and each has its own lockfile entry.

`rk install` without arguments in a workspace context without `rk.lock` returns an error with guidance toward `rk install --link .`.
