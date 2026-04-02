# Refactoring Plan: install.rs Module Deepening

**Status**: ✅ Implemented (steps 1-6), Step 7 (test reorg) deferred

## Problem

`install.rs` was a 1,463-LOC mega-orchestrator that handled 11 sequential concerns in a single function (`install_local_with_resolver`, 250 LOC): manifest validation, backend filtering, artifact discovery, cache management, conflict resolution with rename/frontmatter rewrite, archive creation, multi-backend deployment with dedup, MCP registration, cache persistence, lockfile write, and output.

This made it:
- Hard to test individual phases in isolation
- Hard to navigate (understanding one concept required reading 250+ lines)
- Coupled: `uninstall.rs` reached into `install.rs::cleanup_previous_installation`
- Test-heavy: ~1000 LOC of tests with 4 inline mock backends

## Implemented Solution

Converted `src/install.rs` into `src/install/` directory module with 5 files:

| File | LOC | Responsibility |
|------|-----|----------------|
| `mod.rs` | ~210 (code) + ~1000 (tests) | Public API, orchestrator, conflict resolver factory |
| `types.rs` | 54 | `InstallOptions`, `SourceKind`, `ConflictResolver` type alias |
| `cleanup.rs` | 67 | `cleanup_previous_installation`, `rollback`, `undo_artifact` |
| `resolve.rs` | 89 | Conflict detection + rename, `ResolvedArtifacts` struct (owns temp files) |
| `deploy.rs` | 116 | Multi-backend deploy loop, dedup, MCP, rollback-on-error, `DeploymentResult` |

### Key Design Decisions

1. **`ResolvedArtifacts` owns `NamedTempFile`**: Structural lifetime guarantee
2. **Rollback encapsulated in `deploy_to_backends`**: Only the deploy phase needs undo
3. **`cleanup_previous_installation` re-exported**: `uninstall.rs` import unchanged
4. **Zero public API change**: All callers (`main.rs`, `workspace.rs`, `lockfile.rs`) untouched

### Orchestrator (~75 LOC, was 250)

```
1. Validate manifest + scope
2. Filter active backends
3. Discover artifacts
4. Load cache + cleanup previous
5. resolve::resolve_conflicts_and_rename()
6. Create/fetch archive
7. deploy::deploy_to_backends()
8. Save state (cache + lockfile)
9. Output + env warnings
```

## Deferred: Step 7 — Test Reorganization

- [ ] Move mock backends (`FailingBackend`, `TrackingBackend`, `ReadsAgentsSkillsBackend`, `AgentsFakeBackend`) to shared test helpers
- [ ] Distribute test functions by topic into separate files
- [ ] Add per-submodule boundary tests for `resolve.rs` and `deploy.rs`

## Dependency Category

**Local-substitutable** (category 2): filesystem I/O tested with tempdir, Backend trait tested with mock implementations.
