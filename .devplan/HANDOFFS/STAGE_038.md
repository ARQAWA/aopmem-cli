# HANDOFF — STAGE_038

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- Implement doctor command.

Requirements covered:

- `REQ-VERIFY-003`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/verify/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_038.md`

Implementation:

- Implemented `aopmem doctor` health checks in `src/verify/mod.rs`.
- Added checks for global dirs, workspace, DB, schema, FTS, adapter block,
  artifacts dirs, and tools dirs.
- Added stable JSON health output and plain-text summary in `src/cli/mod.rs`.
- Added focused verify and CLI tests for prepared and missing workspace
  states.
- Did not start Stage 039.

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
```

Results:

```text
PASS cargo test: 118 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Limitations:

- Stage 038 covers doctor health only.
- Remember helper workflow remains for Stage 039.

Next stage:

- `STAGE_039`
