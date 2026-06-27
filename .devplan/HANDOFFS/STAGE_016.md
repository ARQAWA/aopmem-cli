# HANDOFF — STAGE_016

Status: `DONE`

Objective:

- Add FTS5 table and indexing hooks.

Requirements covered:

- `REQ-STORAGE-003`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_016.md`

Implementation:

- Added SQLite FTS5 virtual table `fts_nodes`.
- Indexed `title`, `summary`, `body`, and `aliases`.
- Added `refresh_fts_node` storage helper.
- `create_node` refreshes `fts_nodes` after successful node insert.
- `create_alias` refreshes `fts_nodes` after successful alias insert.
- Added focused schema test for `fts_nodes`.
- Added storage tests for node create FTS indexing and alias FTS indexing.
- No vector, semantic, embedding, custom MCP server, BM25 CLI, or recall code
  was added.

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
PASS recovery git status did not contradict Stage 016; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 50 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 016 scope
PASS json valid
```

Known limitations:

- Node update is not implemented yet, so only node create and alias create
  hooks are implemented and tested.
- BM25 search CLI belongs to Stage 019.
- Recall logic belongs to Stage 017+.

Next stage:

- `STAGE_017`
