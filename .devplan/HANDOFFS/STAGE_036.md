# HANDOFF — STAGE_036

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- Implement artifact paths and cleanup.

Requirements covered:

- `REQ-ART-001`
- `REQ-ART-002`
- `REQ-ART-003`
- `REQ-ART-004`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/artifacts/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_036.md`

Implementation:

- Added strict artifact day parsing and folder creation under
  `artifacts/YYYY-MM-DD`.
- Added local-only cleanup that deletes only dated directories inside
  `artifacts/`.
- Cleanup first removes folders older than the retained 7 calendar days, then
  removes oldest remaining artifact folders until total artifact usage is at or
  below 1 GB.
- Kept cleanup scoped away from workspace DB, `tools`, `logs`, and `audit-git`.
- Wired `aopmem artifacts cleanup` into the CLI with stable JSON output.
- Added focused tests for day-path creation, retention cleanup, size cleanup,
  sibling-dir safety, command parse, and CLI execution.
- Did not edit `src/storage/**`.
- Did not start Stage 037.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 111 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Limitations:

- Stage 036 covers only artifact path helpers and cleanup behavior.
- Audit-git SQL dump snapshots remain for Stage 037.

Next stage:

- `STAGE_037`
