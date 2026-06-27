# HANDOFF — STAGE_012

Status: `DONE`

Objective:

- Implement links table and link add/list.

Requirements covered:

- `REQ-STORAGE-004`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_012.md`

Implementation:

- Added `links` table in migration `001_init`.
- Added indexes for `source_node_id`, `target_node_id`, and `link_type`.
- Added foreign keys from links to nodes with `ON DELETE RESTRICT`.
- Added storage `Link` and `NewLink` models.
- Added `create_link` and `list_links`.
- `create_link` validates source and target node IDs exist.
- `create_link` rejects empty link type.
- CLI now implements `link add` and `link list`.
- CLI resolves the current workspace key from the current absolute directory.
- CLI opens the user-level workspace DB through storage APIs.
- CLI keeps Stage 005 JSON envelope behavior for `--json`.
- CLI does not expose direct SQL.
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
PASS recovery git status did not contradict Stage 012; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 32 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 012 scope
PASS json valid
```

Known limitations:

- Link type is validated as non-empty, but no allowed link type enum exists yet
  in the v0.1 specs.
- Stage 013 must implement aliases, tags, and sources.
- Stage 014 must implement events audit table.
- Stage 016 must implement FTS.
- `node update` remains not implemented.

Next stage:

- `STAGE_013`
