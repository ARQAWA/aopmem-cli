# HANDOFF — STAGE_011

Status: `DONE`

Objective:

- Implement nodes table and node create/get/list.

Requirements covered:

- `REQ-STORAGE-004`
- `REQ-CLI-005`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_011.md`

Implementation:

- Added `nodes` table in migration `001_init`.
- Added indexes for `node_type` and `status`.
- Added storage `Node` and `NewNode` models.
- Added `create_node`, `get_node`, and `list_nodes`.
- Added allowed node type and status validation from storage spec.
- Active nodes require `source_ref`, `confidence`, and `trust_level`.
- Added CLI `node create`, `node get`, and `node list`.
- CLI resolves the current workspace key from the current absolute directory.
- CLI opens the user-level workspace DB through storage APIs.
- CLI keeps Stage 005 JSON envelope behavior for `--json`.
- CLI does not expose direct SQL.
- No links table was added.
- No aliases, tags, sources, or events tables were added.
- No FTS, semantic/vector search, MCP server, or markdown import/export was
  added.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 011; repo content is
     currently untracked in git
PASS cargo fmt
FAIL first rtk cargo test found one Rust lifetime error in list_nodes
PASS final rtk cargo test: 28 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 011 scope
PASS json valid
```

Known limitations:

- Stage 012 must implement links table and link add/list.
- Stage 013 must implement aliases, tags, and sources.
- Stage 014 must implement events audit table.
- Stage 016 must implement FTS.
- `node update` remains not implemented.

Next stage:

- `STAGE_012`
