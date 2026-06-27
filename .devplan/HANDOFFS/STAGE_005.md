# HANDOFF — STAGE_005

Status: `DONE`

Objective:

- Add JSON envelope and exit code model.

Requirements covered:

- `REQ-CLI-002`
- `REQ-CLI-003`
- `REQ-CLI-004`

Files changed:

- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_005.md`

Implementation:

- Added global `--json` flag.
- Added fixed exit code constants from `CLI_CONTRACT.md`.
- Added stable JSON envelope fields:
  - `ok`
  - `command`
  - `data`
  - `warnings`
  - `errors`
  - `meta.version`
- Kept all real command handlers as stubs.
- Stub commands emit `NOT_IMPLEMENTED` and exit code `7`.
- In JSON mode, stub errors print JSON to stdout.
- In human mode, stub errors print readable text to stderr.
- Patched audit finding: parse errors with `--json` now print a stable
  JSON envelope to stdout and exit `2`.

Commands run:

```text
git status --short
rtk cargo test
rtk cargo run -- --json tool run
rm -rf target Cargo.lock
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 004 state
PASS rtk cargo test: 6 passed
PASS JSON command proof: exit 7, stdout is valid JSON envelope
PASS removed generated Cargo.lock and target because they are outside Stage 005 scope
PASS json valid
```

Known limitations:

- CLI command business logic remains stubbed.
- Clap help and version remain clap human output.
- No storage/path logic was added.
- Stage 005 still requires Codex high audit before it can become `VERIFIED`.

Audit patch proof:

```text
PASS rtk cargo test: 7 passed
PASS --json invalid args proof: exit 2, stdout is valid JSON envelope
PASS direct binary proof: exit 2, stdout JSON only, stderr empty
PASS python3 -m json.tool .devplan/EXECUTION_LEDGER.json
PASS removed generated Cargo.lock and target because they are outside Stage 005 scope
```

Next stage:

- `STAGE_006`
