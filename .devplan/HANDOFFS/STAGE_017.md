# HANDOFF — STAGE_017

Status: `DONE`

Objective:

- Implement structured recall base.

Requirements covered:

- `REQ-MEM-002`

Files changed:

- `src/recall/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_017.md`

Implementation:

- Added `StructuredRecallBundle` in `src/recall/mod.rs`.
- Grouped `project_profile`, `gate`, and `workflow` nodes by status.
- Wired `aopmem recall` to `storage::list_nodes`.
- Returned the bundle through the existing CLI JSON envelope.
- Added focused unit tests for recall grouping and CLI routing.
- No FTS/BM25, graph traversal, semantic search, vectors, or embeddings were
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
PASS recovery git status did not contradict Stage 017; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 52 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 017 scope
PASS json valid
```

Known limitations:

- Graph traversal belongs to Stage 018.
- FTS/BM25 fallback belongs to Stage 019.
- Hunch selection and bundle shaping/limits belong to Stage 020+.
- Deprecated/superseded exclusion belongs to Stage 022.

Next stage:

- `STAGE_018`
