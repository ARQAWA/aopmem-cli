# HANDOFF — STAGE_019

Status: `VERIFIED`

Objective:

- Implement FTS/BM25 fallback.

Requirements covered:

- `REQ-STORAGE-003`
- `REQ-MEM-002`

Dependency scope:

- Used DERC Recall stage dependency scope.
- Product scope included `src/recall/**`.
- Minimal adjacent files used:
  `src/storage/mod.rs` and `src/cli/mod.rs`.
- `src/schema/**` was not edited because `fts_nodes` already exists.

Files changed:

- `src/storage/mod.rs`
- `src/recall/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_019.md`
- `.devplan/BLOCKERS.md`

Implementation:

- Added public `storage::search_nodes_fts(connection, query, limit)`.
- Search uses SQLite FTS5 `MATCH`.
- Results order by `bm25(fts_nodes)` ascending and `nodes.id` ascending.
- Search excludes `deprecated` and `superseded`.
- Added additive recall output field `fts_fallback`.
- CLI recall fills fallback only when structured recall is insufficient.
- Fallback query is deterministic and derived from structured recall titles.
- Fallback de-duplicates nodes already present in structured recall output.
- No semantic/vector search was added.
- No Mem0, Hindsight, custom MCP, CI, or markdown export work was added.

Commands run:

```text
git status --short
cargo fmt && rtk cargo test
cargo fmt && rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is
     currently untracked in git
FAIL first rtk cargo test: Rust lifetime error in new FTS search method
PASS cargo fmt
PASS rtk cargo test: 57 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage
     scope
```

Known limitations:

- Hunch selection belongs to Stage 020.

Audit:

- Stage 019 audit passed.

Next step:

- Start Stage 020.
