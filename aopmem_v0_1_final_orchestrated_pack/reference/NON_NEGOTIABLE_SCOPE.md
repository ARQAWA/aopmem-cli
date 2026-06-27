# NON-NEGOTIABLE SCOPE — AOPMem v0.1

This file is enforced by every stage prompt. If a stage requires violating this file, the agent must mark the stage BLOCKED.

## Must build

- Separate Rust CLI product repo.
- macOS ARM target.
- One Rust crate.
- User-level global storage in `~/.aopmem`.
- Per-workspace SQLite DB.
- SQLite FTS5/BM25.
- Typed nodes/links/events/registries.
- Memory Keeper contract and CLI operations.
- Tool runner through `aopmem tool run`.
- Generated tool registry and `tool.json` contract.
- Optional `.understand.docs` setup.
- Optional Codebase Memory MCP profile/setup.
- Empty Corporate MCP registry.
- Hunches in recall bundle.
- Full reflection workflow support through CLI storage/proposal/apply, not built-in LLM.
- Local artifact retention: 7 days or 1 GB per workspace.
- Local audit git with SQL dump/audit snapshots.
- `doctor` and local proof commands.
- DERC development protocol.
- 45–60 granular stages.

## Must not build

- Mem0.
- Hindsight.
- Semantic search.
- Vector search.
- Embeddings.
- Qdrant.
- Custom MCP server.
- GitHub Actions / CI.
- Current state/task history memory.
- Migration/import from existing MVP.
- QA domain pack.
- PR/handoff contract.
- Markdown exports/views/imports for AOPMem memory.
- Background enrichment daemon.
- Direct SQL access for agent.
- Generated tool tests.
- Shell fallback inside main chat for Memory Keeper.
- Windows/Linux support in v0.1.

## Must not decide later

The implementation agent must not invent architecture, dependencies, storage layout, command naming, risk categories, or adapters outside this specification.

If unclear, mark BLOCKED.
