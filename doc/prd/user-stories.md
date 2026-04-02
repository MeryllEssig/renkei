# User Stories

## Installation and deployment

1. As a developer, I want to install a workflow from a Git repo (SSH) so I can use it immediately in my AI tool without manual configuration.
2. As a developer, I want to install a workflow from a Git repo (HTTPS) so I can do it from an environment without SSH keys.
3. As a developer, I want to install a specific version of a workflow via a Git tag (`--tag v1.2.0`) to guarantee reproducibility of my environment.
4. As a developer, I want to install a workflow from a local folder (relative or absolute path) to test a package in development without publishing it.
5. As a developer, I want `rk install` to validate the `renkei.json` manifest before any deployment so it fails early on invalid configuration.
6. As a developer, I want `rk install` to automatically detect the installed backend (Claude Code, Cursor) so it only deploys where relevant.
7. As a developer, I want to be warned if the package doesn't support my backend before installation to avoid partial or inconsistent deployment.
8. As a developer, I want to force-install an incompatible package via `--force` to install it despite the declared incompatibility, at my own risk.
9. As a developer, I want skills to be deployed under `~/.claude/skills/renkei-<name>/` so they are isolated from native skills and easily identifiable.
10. As a developer, I want hooks to be merged into `~/.claude/settings.json` so they activate automatically in Claude Code.
11. As a developer, I want agents to be deployed in `~/.claude/agents/` so they are available directly from Claude Code.
12. As a developer, I want MCP configurations declared in `renkei.json` to be registered in `~/.claude.json` to automatically activate the required MCP servers.
13. As a developer, I want to see the list of missing required environment variables after installation so I can configure them without digging through documentation.
14. As a developer, I want to re-run `rk install` on an already-installed package to update the deployed artifacts to the new version.

## Installation scope

14b. As a developer, I want `rk install` to deploy skills and agents at the project level by default (`.claude/`) so that different projects can have different workflows without interference.
14c. As a developer, I want to install a package globally with `-g` so that the workflow is available across all my projects.
14d. As a developer, I want hooks and MCP servers to always be deployed globally (even in project scope) since they are inherently global resources.
14e. As a package creator, I want to tag my package as `global`-only or `project`-only via the `scope` field so that it cannot be installed in an inappropriate context.
14f. As a developer, I want `rk install` outside a git repository (without `-g`) to fail with a clear error so I don't accidentally deploy to the wrong location.
14g. As a developer, I want `rk uninstall` to mirror `rk install` scoping so that project packages are removed from the project and global packages from the global scope, with no cross-scope fallback.

## Conflict management

15. As a developer, I want to be alerted if two packages deploy a skill with the same name to avoid silent overwrites.
16. As a developer, I want to rename a conflicting skill via an interactive prompt to keep both packages side by side.
17. As a developer, I want the rename to update the `name` field in the skill's frontmatter so the reference stays consistent.
18. As a developer, I want the original-name → deployed-name mapping to be persisted in the local cache so `doctor` and `list` commands remain accurate after renaming.

## Listing and visibility

19. As a developer, I want to list all installed packages with their versions and sources (`rk list`) to get an overview of my project's environment, and `rk list -g` for the global environment.
20. As a developer, I want to distinguish Git-installed packages from locally-installed ones in `rk list` to know which can be updated automatically.

## Diagnostics

21. As a developer, I want to diagnose the state of my installed packages (`rk doctor`) to detect problems without manually inspecting files.
22. As a developer, I want `rk doctor` to flag locally modified skills so I know which ones have diverged from the original.
23. As a developer, I want `rk doctor` to list missing required environment variables per package so I can quickly fix configuration gaps.
24. As a developer, I want `rk doctor` to check for the presence of backends (Claude Code, Cursor) to confirm that artifacts have a runtime.
25. As a developer, I want a non-zero exit code when `rk doctor` detects problems so I can integrate it into CI scripts.

## Package creation

26. As a package creator, I want to validate that all files declared in `renkei.json` exist (`rk package`) to avoid distributing a broken package.
27. As a package creator, I want to generate a `<name>-<version>.tar.gz` archive of my package for easy distribution.
28. As a package creator, I want to auto-bump the version (patch / minor / major) via `--bump` to follow semver without manually editing the manifest.
29. As a package creator, I want to see a summary of included files and archive size after `rk package` to verify the contents before distribution.

## Lockfile

30. As a developer, I want a `rk.lock` lockfile to be automatically generated at the project root after each installation to pin the exact installed versions in this project context.
31. As a team member, I want to commit `rk.lock` to the project repo so the rest of the team works with the same workflow versions.
32. As a new team member, I want to clone the project and run `rk install` (no arguments) to immediately get the same workflows as the rest of the team, with no additional configuration.
33. As a developer, I want `rk install` without arguments to read `rk.lock` and install the exact declared versions to reproduce the environment identically.
34. As a developer, I want the lockfile to include integrity (SHA-256 hash) of each package to detect any corruption or tampering.

## Phase 1 — Delivery and migration

35. As a project maintainer, I want the CLI to be compiled into native binaries for Linux / macOS / Windows and automatically published via GitHub Actions on each release so users can install it without dependencies.
36. As a package creator, I want to migrate existing workflows (renkei-old) into valid Renkei packages to validate the `renkei.json` format on real cases from v1.

## Multi-backend configuration

55. As a developer, I want to configure which backends I use (`rk config`) so that Renkei deploys to the right tools without me specifying them each time.
56. As a developer, I want Renkei to auto-detect installed backends when no config exists so that `rk install` works out of the box without running `rk config` first.
57. As a developer, I want `rk install` to deploy to all my configured backends in a single atomic operation so that my workflows are available everywhere at once.
58. As a developer, I want to override my configured backends for a single install (`--backend cursor`) so I can target a specific tool without changing my global config.
59. As a developer, I want a warning when a configured backend is not detected on my machine so I know it was skipped, and an error when none of my backends are detected.
60. As a developer, I want `rk install --force` to bypass manifest backend restrictions but not detection, so I can override the package author's intent without deploying into the void.
61. As a developer, I want the `agents` backend to deploy skills to the shared `.agents/` directory so that tools supporting the `.agents/` standard (Codex, Gemini) pick them up automatically.
62. As a developer, I want Renkei to avoid deploying duplicate skills when a branded backend already reads from `.agents/`, so I don't end up with two copies of the same skill.
63. As a developer, I want `rk config set/get/list` subcommands so I can manage my configuration programmatically and in CI scripts.
64. As a developer, I want the install-cache to group deployed artifacts by backend so that uninstall, doctor, and list can operate per-backend accurately.
65. As a developer, I want my existing v1 install-cache to be automatically migrated to the v2 format so I don't have to reinstall all my packages.

## Phase 2 — Registry and advanced commands

37. As a package creator, I want to publish my package to a centralized registry (`rk publish`) to make it discoverable by other teams.
38. As a developer, I want to search for packages in the registry (`rk search <query>`) to find existing workflows without manually browsing repos.
39. As a developer, I want to install a package by its scoped name (`rk install @scope/name`) without needing to know the Git URL.
40. As a developer, I want to update a package to its latest compatible version (`rk update`) to benefit from improvements without reinstalling manually.
41. As a developer, I want to uninstall a package and clean up all its deployed artifacts (`rk uninstall`) to leave no residuals.
42. As a developer, I want to get package details (description, author, versions, dependencies) via `rk info` to evaluate it before installation.
43. As a package creator, I want to interactively scaffold a new package (`rk init`) to start with a valid structure without writing it from scratch.
44. As a developer, I want to see the diff between deployed artifacts and the original archive (`rk diff`) to audit my local modifications.
45. As a developer, I want to restore a package's artifacts from the original archive (`rk reset`) to undo my local modifications.
46. As a package creator, I want to fork an existing package under my scope (`rk fork --scope <s>`) to create an independent variant without modifying the original.
47. As a user, I want to authenticate with the registry (`rk login` / `rk logout`) to publish under my scope.
48. As a developer, I want Cursor packages to be deployed in `.cursor/skills/<name>/` to use my workflows in Cursor without configuration.
49. As a package creator, I want to declare an organizational scope (`@acme-corp/`) to avoid name collisions between teams.

## Phase 3 — Ecosystem

50. As a developer, I want to browse available packages on a public website to discover workflows without the CLI.
51. As a package creator, I want a public profile displaying my published packages to build my reputation in the ecosystem.
52. As an organization, I want a private registry under my scope to distribute internal workflows without exposing them publicly.
53. As a developer, I want to auto-update the CLI (`rk self-update`) to always have the latest fixes.
54. As an admin, I want access to installation statistics for my packages to measure their adoption.
