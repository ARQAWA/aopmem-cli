# HANDOFF — STAGE_004

Status: `DONE`

Objective:

- Add CLI shell and command routing skeleton.

Requirements covered:

- `REQ-CLI-001`

Files changed:

- `src/cli/mod.rs`
- `src/main.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_004.md`

Implementation:

- Added clap parser for every command group from `CLI_CONTRACT.md`.
- Added nested routing enums for command families.
- Added stub handler that prints `NOT_IMPLEMENTED: <command>` to stderr.
- Stub commands return exit code `7`.
- Wired `main` to `cli::run`.
- Added focused unit tests for clap command validity and routability.

Commands run:

```text
git status --short
rtk cargo build
rtk cargo test
./target/debug/aopmem --help
./target/debug/aopmem --version
./target/debug/aopmem tool run
rm -rf target Cargo.lock
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 003 state
PASS rtk cargo build
PASS rtk cargo test: 3 passed
PASS --help exits 0 and is served by clap
PASS --version exits 0 and prints aopmem 0.1.0
PASS stub command exits 7 and prints NOT_IMPLEMENTED
PASS removed generated Cargo.lock and target because they are outside Stage 004 scope
PASS json valid
```

Known limitations:

- CLI commands are routing stubs only.
- JSON envelope is not implemented. It is Stage 005 scope.
- No business logic is implemented in `cli`.
- Stage 004 still requires Codex high audit before it can become `VERIFIED`.

Next stage:

- `STAGE_005`
