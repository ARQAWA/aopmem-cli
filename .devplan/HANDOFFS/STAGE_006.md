# HANDOFF — STAGE_006

Status: `DONE`

Objective:

- Add user-level path resolver.

Requirements covered:

- `REQ-PROD-002`
- `REQ-STORAGE-001`

Files changed:

- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_006.md`

Implementation:

- Added `AopmemPaths`.
- Added `resolve_paths()` with `AOPMEM_HOME` override.
- Added default `HOME/.aopmem`.
- Added global path accessors for `bin`, `skills`, `templates`, and
  `workspaces`.
- Added unit tests with temporary `HOME` and `AOPMEM_HOME`.
- Resolver does not create files or directories.

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 005 state
PASS rtk cargo test: 10 passed
PASS removed generated Cargo.lock and target because they are outside Stage 006 scope
PASS json valid
```

Known limitations:

- Workspace key generation is not implemented. It belongs to Stage 007.
- Directory creation is not implemented. It belongs to Stage 008.
- SQLite connection and pragmas are not implemented. They belong to Stage 009.
- Stage 006 still requires Codex high audit before it can become `VERIFIED`.

Next stage:

- `STAGE_007`
