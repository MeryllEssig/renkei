# Installing a renkei package

## From a local path

```bash
rk install ./path/to/package/
```

## From git

```bash
rk install git@github.com:user/repo
rk install https://github.com/user/repo
rk install git@github.com:user/repo --tag v1.0.0
```

## Scope

By default, skills and agents deploy to the **project** (`.claude/` at the git root). Hooks and MCP always deploy globally (`~/.claude/`).

```bash
rk install ./pkg/          # project scope (default)
rk install -g ./pkg/       # global scope (~/.claude/)
```

The manifest's `scope` field (`"any"`, `"global"`, `"project"`) controls where a package is allowed to be installed. Mismatches produce an error.

## Flags

| Flag | Effect |
|------|--------|
| `-g` / `--global` | Install in global scope |
| `--tag <tag>` | Clone a specific git tag/branch |
| `--force` | Install even if backend is incompatible |

## What happens during install

1. Manifest (`renkei.json`) is read and validated
2. Artifacts are discovered from `skills/`, `hooks/`, `agents/`
3. Archive is created in `~/.renkei/archives/`
4. Artifacts are deployed to the correct paths
5. Install-cache is updated
6. If any step fails, all deployed artifacts are rolled back
