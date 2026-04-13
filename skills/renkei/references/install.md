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
| `-m` / `--member <name>` | Workspace only: install a subset of members (repeatable or CSV) |
| `-y` / `--yes` | Auto-accept preinstall confirmation prompts |

## Selective workspace install

For a workspace source (local or git), install only specific members:

```bash
rk install ./workspace/ -m mr-review
rk install git@github.com:user/repo -m a -m b
rk install git@github.com:user/repo -m a,b
```

Validation (fail-fast, before deploy):
- `-m` on a non-workspace source → error.
- `-m foo` where `foo` is not declared in `workspace` → error listing available members.
- `-m` with no-arg lockfile install → error.

Each selected member is an independent entry in `rk.lock` (with a `member` field) and install-cache.

## Preinstall / postinstall messages

Packages may declare `messages.preinstall` and/or `messages.postinstall` in their manifest.

- **Preinstall**: all collected messages from the current install batch (single, workspace, lockfile replay) are rendered in one framed block, followed by a single `Install all? [y/N]` prompt. Refusal exits 0 with `Installation cancelled.` — nothing is deployed.
- **`--yes` / `-y`**: bypasses the prompt. Required in non-TTY contexts (CI); otherwise install errors out with a message pointing to `--yes`.
- **Postinstall**: shown at the end of each successful install, after `Done.` and after `requiredEnv` warnings. In workspace mode, one labeled block per member (`@scope/member:`).

## What happens during install

1. Manifest (`renkei.json`) is read and validated
2. Preinstall messages collected across the batch → single confirmation prompt (skipped if none declared or `--yes`)
3. Artifacts are discovered from `skills/`, `hooks/`, `agents/`
4. Archive is created in `~/.renkei/archives/`
5. Artifacts are deployed to the correct paths
6. Install-cache is updated
7. Postinstall messages rendered after `Done.` + `requiredEnv` warnings
8. If any step fails, all deployed artifacts are rolled back
