# HANDOFF — STAGE_020

Status: `VERIFIED`

Objective:

- Implement hunch selection.

Requirements covered:

- `REQ-MEM-003`

Dependency scope:

- Not used.
- Product changes stayed in `src/recall/**`.
- Did not edit `src/cli/**`, `src/storage/**`, or `src/schema/**`.

Files changed:

- `src/recall/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_020.md`

Implementation:

- Added additive recall JSON field `hunches`.
- Added `RecallHunch` with required `source_node_id`.
- Hunch output includes source type, title, optional summary, reason, and
  source `updated_at`.
- Hunch output omits full node body so it is not the source of truth.
- Hunches are selected from FTS fallback candidates.
- Selection returns at most 3 hunches.
- Selection is deterministic by:
  - linked `failure_mode`, `tool_contract`, `workflow` signal;
  - FTS rank;
  - `updated_at` hotness;
  - source node id.
- Deprecated and superseded candidates remain excluded by existing fallback
  filtering.
- Added focused unit test for hunch selection, ordering, source node id, and
  max count.
- No semantic/vector search was added.
- No Mem0, Hindsight, custom MCP, CI, or markdown export work was added.
- Did not start Stage 021.

Commands run:

```text
git status --short
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
PASS cargo fmt
PASS rtk cargo test: 59 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage
     scope
```

Known limitations:

- Recall bundle shaping and limits belong to Stage 021.

Audit:

- Stage 020 audit passed.

Next step:

- Start Stage 021.
