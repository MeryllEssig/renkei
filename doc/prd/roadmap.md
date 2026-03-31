# Roadmap, Scope and Risks

## Out of Scope

- **Workflow runtime / executor**: Renkei distributes workflows, it doesn't execute them.
- **MCP orchestrator**: MCP server lifecycle management is left to the AI tool.
- **Pattern library / agentic pattern framework**: Renkei is content-agnostic.
- **Workflow → skill compiler**: no transformation of package contents.
- **Inter-workflow dependency system**: a package cannot declare dependencies on other packages (v1).
- **Observability / execution metrics**: out of scope.
- **Local GUI**: the CLI is the only entry point.

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| **Over-engineering** | High | Minimal scope in v1. Every feature justified by a concrete need. |
| **Zero adoption** | High | Validate with early users (team members) before investing in the registry. |
| **Rapid evolution of AI tools** | Medium | The `Backend` interface isolates from change. A single adaptation point per tool. |
| **Unstable skill format** | Medium | Monitor Claude Code / Cursor changelogs. Adapt quickly. |
| **Rust learning curve** | Low | The CLI scope is well-defined. No concurrency, no complex async. |
| **Native competition** | Low | Renkei is multi-tool and workflow-oriented, not component-oriented. Complementary to native stores. |

## Licensing

| Component | License |
|-----------|---------|
| CLI `rk` | Open source |
| Registry website | Closed source |
| Individual packages | Creator's choice |
| `@renkei/` scope | Reserved for official packages |

## Language and distribution

- CLI written in **Rust**: native binary, zero runtime dependencies.
- Cross-compilation for Linux / macOS / Windows via GitHub Actions.
- Distribution via **GitHub Releases** — a single executable file.
- Open source license for the CLI; the registry website will be closed source.

## Registry (Phase 2)

- HTTP service: index `@scope/name` → source URL + metadata.
- `rk publish` sends the archive + updates the index.
- Scopes: `@renkei/` reserved for official packages, others are registered on first publish.
- Auth via API token.

## Further Notes

- **Clean break**: the existing codebase (`renkei-old`) serves as reference but the new Renkei starts from scratch in Rust. Existing workflows will be packaged as Renkei packages once the CLI v1 is functional — this is a Phase 1 deliverable.
- **Convention over config**: deployment destinations are hardcoded. Adding a `destination` field to the manifest is explicitly rejected — less error surface, fewer decisions for the package creator.
- **Claude-first**: in v1, only `ClaudeBackend` is implemented. The `Backend` interface is the only concession to future flexibility.
- **Early user validation**: before investing in the registry (v2), validate adoption with users. If nobody installs packages, the registry is premature.
- **The website (v3) should only be built if the ecosystem justifies it** — no speculative builds.
- **Scripts in packages**: the package structure can include a `scripts/` directory with arbitrary scripts. These scripts are not a named artifact type in `artifacts` — they are included in the archive but their deployment is not natively managed by `rk`. This behavior will need to be clarified during `rk package` implementation.
