# Installation

## Installation scope

By default, `rk install` operates in **project scope**: skills and agents are deployed to the project's local backend directory (`.claude/`), while hooks and MCP servers go to the global backend directory (`~/.claude/`) since they are inherently global resources. The package is tracked in the project's `rk.lock` and install-cache.

Use `-g` / `--global` to install in **global scope**: all artifacts deploy to `~/.claude/`, tracked in `~/.renkei/rk.lock` and `~/.renkei/install-cache.json`.

See [Scope](./scope.md) for the full scope specification.

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
