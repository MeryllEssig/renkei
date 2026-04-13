# Local MCP servers

A package can ship its own MCP server source instead of asking users to install it separately. Renkei copies the source to `~/.renkei/mcp/<name>/`, runs the declared `build` steps, and registers the absolute entrypoint with the backend.

## Convention (author)

```
my-pkg/
├── renkei.json
└── mcp/
    └── my-server/         # folder name MUST match mcp.<name> in the manifest
        ├── package.json
        ├── src/index.ts
        └── dist/index.js  # optional: prebuilt entrypoint
```

In `renkei.json`:

```json
{
  "mcp": {
    "my-server": {
      "command": "node",
      "entrypoint": "dist/index.js",
      "args": ["--verbose"],
      "build": [["bun", "install"], ["bun", "run", "build"]]
    }
  }
}
```

Rules:

- `entrypoint` (relative to `mcp/<name>/`) is required for a local MCP.
- `build` is an array of argv arrays — **never a shell string**. Each step is invoked as `Command::new(argv[0]).args(&argv[1..])`. Required unless the resolved entrypoint already exists on disk (prebuilt or vendored).
- Renkei prepends the absolute path (`~/.renkei/mcp/<name>/<entrypoint>`) to the manifest's `args`. Author-declared `args` are passed through untouched as flags.

## Install (consumer)

```bash
rk install --allow-build ./my-pkg/
```

- Local MCP sources always deploy to `~/.renkei/mcp/<name>/`, regardless of the package scope.
- Without `--allow-build`, `rk install` shows a yellow framed block listing every build command and asks for `[y/N]`. In CI / non-TTY, `--allow-build` is mandatory or the install errors out.
- Build runs with a filtered env (whitelist of `PATH`, `HOME`, `LANG`, `LC_*`, proxies, certs, plus `npm_config_*`, `BUN_*`, `CARGO_*`, etc.). Tokens / secrets / `*_KEY` / `*_SECRET` / `AWS_*` / `GITHUB_*` / `ANTHROPIC_*` are stripped — `requiredEnv` is **not** propagated.
- On build success: atomic swap from `~/.renkei/mcp/<name>.new/` into `~/.renkei/mcp/<name>/`. On failure: staging removed, previous version untouched, the rest of the install rolls back.

## `--link` mode

```bash
rk install --link ./my-pkg/
```

Symlinks `~/.renkei/mcp/<name>/` to `<workspace>/mcp/<name>/`. Renkei does **not** run the build — manage that lifecycle yourself in the workspace. Uninstall removes the symlink only and never touches the workspace source.

## Reference counting & uninstall

Local MCPs are tracked in `install_cache.mcp_local` (separate from per-package `mcp_servers`). Each install is recorded as a ref `(package, scope, project_root)`.

- Re-install of the same `owner_package` from a second project → adds a ref, no rebuild.
- Same owner, higher version → rebuild, all refs share the new folder.
- Different owner trying to claim the same `<name>` → hard error pointing to `--force` (which transfers ownership and rebuilds).

`rk uninstall @scope/name` decrements the ref for the current scope. The folder + backend entry are GC'd only when the **last** ref disappears.

## Lockfile replay

`rk install` with no source replays the lockfile, re-fetches sources, and re-hashes them. A mismatch with the recorded `source_sha256` aborts with `Lockfile drift for '@scope/name': local MCP '<name>' source hash changed`. `--allow-build` is required again on every replay — there is no machine-to-machine trust carryover.

## Diagnostics

`rk doctor` adds three checks per local MCP:

| Check | Severity | Test |
|-------|----------|------|
| `exists` | error | `~/.renkei/mcp/<name>/` (or symlink) is present. |
| `entrypoint` | error | The file at `<folder>/<entrypoint>` exists. |
| `integrity` | warning | Source content hash matches the recorded `source_sha256`. |

Integrity drift is a **warning**, not an error: `--link` installs and post-build user tampering legitimately diverge.

## Packaging exclusions

`rk package` filters every directory through `.rkignore` defaults: `node_modules/`, `dist/`, `build/`, `target/`, `.venv/`, `venv/`, `__pycache__/`, `.pytest_cache/`, `*.pyc`, `.DS_Store`, `.git/`. Inside `mcp/<name>/` the `dist/build/target` defaults are relaxed so prebuilt entrypoints survive. A `.rkignore` at the package root extends the list.
