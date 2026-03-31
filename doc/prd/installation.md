# Installation

## Git installation

1. `git clone --depth 1` into a temp directory (`/tmp/rk-xxx/`)
2. Validate the `renkei.json` manifest
3. Create the `.tar.gz` archive in `~/.renkei/cache/@scope/name/<version>.tar.gz`
4. Deploy artifacts from the archive
5. Delete the temp clone

Without `--tag` or `--branch`, HEAD of the default branch is used. The commit SHA is recorded in the lockfile for reproducibility. The version in `renkei.json` is authoritative (trust the manifest) — no consistency check against Git tags.

## Local installation

- `rk install ./my-workflow/` creates a **copy** (snapshot archive in cache), same as Git.
- `rk install --link ./my-workflow/` creates **symlinks** for development (`npm link` / `pip install -e` model). Changes in source files are immediately reflected.

## No-argument installation

- If `rk.lock` exists in the current directory → installs the exact versions from the lockfile.
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
