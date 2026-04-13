# MCP — External and Local Servers

Renkei lets a package declare MCP (Model Context Protocol) servers in its `renkei.json`. There are two flavours, distinguished by the presence of `entrypoint` / `build`:

- **External MCP**: the server is installed out-of-band (npm global, system binary, …). The manifest holds a ready-to-merge `command`/`args`/`env` block that Renkei copies into the backend's MCP config.
- **Local MCP**: the package ships the server's source code under `mcp/<name>/`. Renkei copies the sources to `~/.renkei/mcp/<name>/`, runs the declared `build` argv steps, and registers the absolute entrypoint path with the backend.

Both flavours use the same backend registration plumbing (`mcpServers` in `~/.claude.json`, `~/.cursor/mcp.json`, …). The only difference is that local MCPs prepend an absolute path to the manifest's `args` so the backend can launch the freshly built binary.

## Convention

A package that wants to ship a local MCP places its source under `mcp/<name>/` at the package root (or workspace member root). The folder name **must** match the key in `mcp.<name>` (strict match, validated). Workspace members cannot collide on the same `<name>` — validation rejects the manifest.

```
my-pkg/
  renkei.json
  mcp/
    my-server/
      package.json
      src/index.ts
      dist/index.js     # optional: prebuilt entrypoint
```

## Manifest fields

`mcp.<name>` accepts the existing native fields (`command`, `args`, `env`, …) plus two local-MCP fields, both optional:

| Field        | Type           | Required for local? | Description |
|--------------|----------------|---------------------|-------------|
| `entrypoint` | string         | yes                 | Path inside `mcp/<name>/` to the runtime file Renkei prepends to `args`. |
| `build`      | array of argv  | conditional         | Sequential build steps. Required unless the resolved entrypoint already exists on disk (prebuilt / vendored). |

`build` is `[[cmd, ...args], ...]` — never a shell string. Each inner array is at least one non-empty token. Example: `[["bun","install"],["bun","run","build"]]`.

```json
{
  "mcp": {
    "my-server": {
      "command": "node",
      "entrypoint": "dist/index.js",
      "args": ["--verbose"],
      "build": [["bun","install"],["bun","run","build"]]
    }
  }
}
```

The merged backend entry becomes:

```json
{
  "command": "node",
  "args": ["/home/u/.renkei/mcp/my-server/dist/index.js", "--verbose"]
}
```

## Scope

Local MCP sources are **always deployed to `~/.renkei/mcp/<name>/`**, regardless of the package's `scope` (`any` | `project` | `global`). Backend MCP registration is global by nature (`~/.claude.json` is shared across projects), so the source folder follows the same constraint. See [scope.md](./scope.md#exception-local-mcps-are-always-global) for the broader rule.

## Install flow

1. Gather `messages.preinstall` notices (existing `[y/N]` prompt, gated by `--yes`).
2. Gather local-MCP build commands. A dedicated yellow/bold block lists `@scope/name → <mcp-name>: <argv>` per package; `--allow-build` bypasses the prompt. Non-TTY without the flag → hard error pointing at `--allow-build`.
3. Deploy skills/agents/hooks as today.
4. For each local MCP:
   - Copy `mcp/<name>/` into staging at `~/.renkei/mcp/<name>.new/` (rkignore-filtered — see [Packaging exclusions](#packaging-exclusions-rkignore)).
   - Run `build` steps sequentially with `cwd = <staging>`, streaming stdout/stderr live, with a filtered env (see [Build environment](#build-environment)).
   - On build success: atomically swap staging → `~/.renkei/mcp/<name>/` (rename existing aside, swap in new, drop the old).
   - On build failure: remove staging, leave any previous version intact, abort with rollback of other deployed artifacts.
5. Merge into the backend MCP config with `entrypoint` resolved to the absolute path.
6. Render `requiredEnv` warnings + `messages.postinstall`.

## Build environment

Each build step runs as argv via `std::process::Command::env_clear().envs(filtered)`:

- **Plain whitelist**: `PATH`, `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_*`, `TMPDIR`, `SHELL`, `TERM`.
- **Proxies (case-insensitive)**: `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`.
- **Certs**: `NODE_EXTRA_CA_CERTS`, `SSL_CERT_FILE`, `SSL_CERT_DIR`, `REQUESTS_CA_BUNDLE`.
- **Prefixes inherited in bulk**: `npm_config_*`, `PIP_*`, `BUN_*`, `CARGO_*`, `UV_*`, `POETRY_*`.
- **Always excluded**: any name matching `*_TOKEN`, `*_KEY`, `*_SECRET`, `*PASSWORD*`, `AWS_*`, `GITHUB_*`, `GITLAB_*`, `ANTHROPIC_*`, `OPENAI_*`.

`requiredEnv` entries are **not** propagated into the build env. The build runtime is isolated from runtime secrets by design. Windows support of the build runner is out of scope for v1; linux/macOS only.

## `--allow-build` and lockfile replay

- `--allow-build` skips the build confirmation prompt and accepts every declared build step in the batch. Required in non-interactive environments (CI, scripted installs).
- Lockfile replay (`rk install` with no source) re-materializes sources, recomputes the source SHA, and compares with the lockfile. A mismatch triggers an explicit error: `Lockfile drift for '@scope/name': local MCP '<name>' source hash changed (expected …, got …).` Trust does not carry across machines: `--allow-build` must be re-supplied on every replay.

## Reference counting

Local MCPs are tracked in `install_cache.mcp_local` (install-cache v3) separately from per-package `mcp_servers`. Each entry holds:

```text
McpLocalEntry { owner_package, version, source_sha256, referenced_by: [McpLocalRef] }
McpLocalRef   { package, version, scope, project_root }
```

Install outcomes:

| Existing entry | New ref's `package` | Result |
|---|---|---|
| absent | — | `FreshInstall` — create entry, build, write ref. |
| same `owner_package`, same version | — | `AddedRef` — skip build, append ref. |
| same `owner_package`, higher version | — | `UpgradeRequired` — rebuild, swap, single ref bumps version (other refs share the new folder transparently). |
| different `owner_package` | — | `ConflictDifferentOwner` — hard error unless `--force`, which transfers ownership and rebuilds. |

Uninstall mirrors this: `rk uninstall @scope/name` decrements its ref. When the last ref disappears, the folder (or symlink in `--link` mode) is removed and the server entry is stripped from the backend config. Other refs remain untouched.

## `--link` mode

`rk install --link <workspace>` symlinks `~/.renkei/mcp/<name>/` to `<workspace>/mcp/<name>/`. Renkei does **not** run the build; the developer manages the build lifecycle in the workspace. The backend MCP config is still registered with the resolved entrypoint (followed through the symlink). Uninstall removes the symlink only and never touches the workspace source.

A re-link from a different workspace is treated as an owner conflict — uninstall first, or use `--force`.

## Doctor checks

`rk doctor` audits each entry under `install_cache.mcp_local`:

| Check       | Severity | Test |
|-------------|----------|------|
| `exists`    | error    | `~/.renkei/mcp/<name>/` (or symlink) is present. |
| `entrypoint`| error    | The file at `<folder>/<entrypoint>` exists. |
| `integrity` | warning  | `rkignore::hash_directory(folder)` matches the recorded `source_sha256`. |

Integrity is a warning (not an error) because `--link` installs and post-install user tampering legitimately drift. The folder presence and entrypoint checks remain authoritative.

## Packaging exclusions (`.rkignore`)

`rk package` and `cache::create_archive` walk every directory through an rkignore-filtered iterator:

- **Default exclusions** (always applied): `node_modules/`, `dist/`, `build/`, `target/`, `.venv/`, `venv/`, `__pycache__/`, `.pytest_cache/`, `*.pyc`, `.DS_Store`, `.git/`.
- **`.rkignore`** at the package root (gitignore syntax) extends the defaults.
- For the `mcp/<name>/` subtree, a trimmed default list applies — `dist/`, `build/`, `target/` are kept so prebuilt entrypoints survive the archive.

The same patterns drive `source_sha256` computation at install time, so the lockfile's recorded hash is consistent with the published archive contents.
