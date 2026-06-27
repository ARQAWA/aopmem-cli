# HANDOFF — STAGE_022

Status: `DONE`

Objective:

- Implement deprecated/superseded exclusion for normal recall only.

Requirements covered:

- `REQ-MEM-004`

Dependency scope:

- Not used.
- Product changes stayed in `src/recall/**`.
- Did not edit `src/cli/**` or `src/storage/**`.

Files changed:

- `src/recall/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_022.md`

Implementation:

- Excluded `deprecated` and `superseded` nodes from normal structured recall
  sections.
- Kept exclusion for `deprecated` and `superseded` nodes in traversal roots,
  traversal targets, and FTS fallback results.
- Renamed the exclusion helper to reflect normal recall behavior.
- Kept grouped recall status structure intact for future lint/audit mode work.
- Added focused tests for normal section exclusion.
- Did not start Stage 023.

Commands run:

```text
git status --short
rg --files | rg '(^|/)FINAL_DECISION_LOG\.md$|(^|/)NON_NEGOTIABLE_SCOPE\.md$|(^|/)DERC_PROTOCOL\.md$'
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_022.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_021.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery used reference files from aopmem_v0_1_final_orchestrated_pack/reference/
PASS recovery git status matched prior handoff note that repo content is currently untracked in git
PASS cargo test: 62 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- This stage changes normal recall only.
- No lint/audit mode behavior was added yet.

Next step:

- Re-audit `STAGE_022`.
- Do not start `STAGE_023` until audit passes.
