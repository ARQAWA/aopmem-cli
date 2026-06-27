# HANDOFF — STAGE_025

Status: `DONE`

Objective:

- Implement global install check.

Requirements covered:

- `REQ-PROD-002`
- `REQ-INSTALL-001`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/install/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_025.md`

Implementation:

- Added minimal global install status inspection in `src/install/mod.rs`.
- Status checks only the required global pieces for this stage:
  global dirs, binary, and templates dir.
- Wired `aopmem status` in `src/cli/mod.rs` to return the global install check.
- Non-JSON output stays short and does not print paths during normal status.
- Added focused tests for missing and ready global install states.
- Added a CLI test to ensure `status` is no longer the Stage 004 stub.
- Did not start `STAGE_026`.

Commands run:

```text
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_025.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/PROOF_LOG.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_024.md
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo state and Stage 024 handoff
PASS cargo test: 77 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Limitations:

- Global install check is presence-only for this stage.
- It does not install, repair, or initialize workspace state.
- Template content is not validated yet.

Next stage:

- `STAGE_026`
