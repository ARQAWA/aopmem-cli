# HANDOFF — STAGE_023

Status: `DONE`

Objective:

- Implement adapter managed block seed.

Requirements covered:

- `REQ-INSTALL-005`

Dependency scope:

- Not used.
- Product changes stayed in `src/adapter/**` and `src/cli/**`.
- Did not edit files outside stage scope.

Files changed:

- `src/adapter/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_023.md`

Implementation:

- Added adapter seed logic that defaults to the Codex/OpenAI instruction file
  `AGENTS.md`.
- Added optional `--file` to `aopmem adapter seed` for a provided instruction
  file path.
- Seed now inserts the managed block when missing.
- Seed now replaces only the content inside the managed block markers when the
  block already exists.
- Seed now returns an error when markers are damaged or duplicated.
- Seed creates the instruction file when it does not exist yet.
- Added focused tests for create, append, replace, damaged block, and CLI arg
  parsing.
- Did not start Stage 024 work.

Commands run:

```text
pwd
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_023.md
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/PROOF_LOG.md
rg --files aopmem_v0_1_final_orchestrated_pack/reference .devplan/HANDOFFS | rg 'FINAL_DECISION_LOG\.md$|NON_NEGOTIABLE_SCOPE\.md$|DERC_PROTOCOL\.md$|STAGE_022\.md$'
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_022.md
git status --short
rg -n "REQ-INSTALL-005|managed block|adapter seed|AGENTS.md|AOPMEM:BEGIN" .devplan aopmem_v0_1_final_orchestrated_pack/reference aopmem_v0_1_final_orchestrated_pack/stage_prompts
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md
sed -n '1,180p' aopmem_v0_1_final_orchestrated_pack/reference/CLI_CONTRACT.md
sed -n '80,120p' .devplan/MASTER_SPEC.md
cargo test
cargo fmt
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is currently untracked in git
PASS cargo test: 68 passed
PASS cargo fmt
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Limitations:

- Default adapter detection in this stage seeds only Codex/OpenAI `AGENTS.md`.
- Managed block body is a minimal inline seed; shared template files belong to
  a later stage.
- `adapter sync` and `adapter status` are still not implemented.

Next stage:

- `STAGE_024` after Stage 023 audit.
