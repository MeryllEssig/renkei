# Installation

## Installation scope

By default, `rk install` operates in **project scope**: skills and agents are deployed to the project's local backend directories, while hooks and MCP servers follow each backend's conventions (global-only for Claude Code, project-level for Cursor/Codex/Gemini). The package is tracked in the project's `rk.lock` and install-cache.

Use `-g` / `--global` to install in **global scope**: all artifacts deploy to the user's home backend directories, tracked in `~/.renkei/rk.lock` and `~/.renkei/install-cache.json`.

See [Scope](./scope.md) for the full scope specification and [Multi-Backend Configuration](./multi-backend.md) for backend selection and resolution.

### Scope validation

Before installation, the manifest's `scope` field is checked against the requested scope:

| Manifest `scope` | `rk install` (project) | `rk install -g` (global) |
|---|---|---|
| `any` (default) | OK | OK |
| `global` | Error: "This package is global-only, use -g" | OK |
| `project` | OK | Error: "This package is project-only, remove -g" |

### Project root detection

Without `-g`, the project root is detected via `git rev-parse --show-toplevel`. If not inside a git repository, `rk install` fails with an explicit error suggesting `-g`.

## Git installation

1. `git clone --depth 1` into a temp directory (`/tmp/rk-xxx/`)
2. Validate the `renkei.json` manifest
3. Create the `.tar.gz` archive in `~/.renkei/archives/@scope/name/<version>.tar.gz`
4. Deploy artifacts from the archive (respecting the installation scope)
5. Delete the temp clone

Without `--tag` or `--branch`, HEAD of the default branch is used. The commit SHA is recorded in the lockfile for reproducibility. The version in `renkei.json` is authoritative (trust the manifest) — no consistency check against Git tags.

## Local installation

- `rk install ./my-workflow/` creates a **copy** (snapshot archive in `~/.renkei/archives/`), same as Git, respecting the active scope.
- `rk install --link ./my-workflow/` creates **symlinks** for development (`npm link` / `pip install -e` model), respecting the active scope. Changes in source files are immediately reflected.

## Selective workspace install (`-m` / `--member`)

When the source is a workspace package (manifest declares `workspace`), `rk install` deploys **all** members by default. Pass `-m <member>` (repeatable, also CSV) to install only the named members:

```bash
rk install git@github.com:team/our-workflows -m mr-review
rk install ./monorepo -m mr-review -m auto-test
rk install ./monorepo -m mr-review,auto-test       # equivalent
```

Validation (fail-fast, before any deploy):

- `-m foo` where `foo` is not in the manifest's `workspace` array → error listing the available members.
- `-m` on a non-workspace source (no `workspace` field) → error.
- `-m` combined with no-argument `rk install` (lockfile restore) → error.

Each selected member is recorded as an independent entry in `rk.lock`, with an additional optional `member` field naming the workspace subdirectory it was installed from. `rk install` (no-arg) replays each lockfile entry independently: for entries with `member` set, the install resolves to `<clone>/<member>` (Git) or `<source>/<member>` (Local) before deploying. Cached archives are member-scoped, so the cache hit path is unchanged.

`--tag` / `--force` / `--backend` apply once per invocation and are propagated uniformly to every selected member.

## No-argument installation

- `rk install` → if `rk.lock` exists in the project root → installs the exact versions from the lockfile in project scope.
- `rk install -g` → if `~/.renkei/rk.lock` exists → installs the exact versions from the global lockfile in global scope.
- If no lockfile found in the expected location → scope-specific error:
  - Project scope: "No rk.lock found at project root. Use `rk install <source>` to install a package."
  - Global scope: "No global rk.lock found at ~/.renkei/rk.lock. Use `rk install -g <source>` to install a package."
- If no lockfile but workspace detected → explicit error: "workspace detected, use `rk install --link .` for dev".

## Error handling: fail-fast + rollback

On the first error during installation, immediate stop and rollback of all already-deployed artifacts. Guaranteed atomicity: either everything succeeds, or nothing changes.

## Conflict management

- Detection via `install-cache.json` before any deployment.
- **TTY (interactive)**: prompt to rename the conflicting artifact. Renaming updates the `name` field in the skill's frontmatter.
- **Non-TTY (CI)**: error with exit code 1.
- **`--force`**: last installed silently overwrites.
- The original-name → deployed-name mapping is persisted in `install-cache.json`.

## Environment variables

Missing required environment variables trigger a **warning** after installation, not a blocker. `rk doctor` re-checks them. The user configures after installation.
