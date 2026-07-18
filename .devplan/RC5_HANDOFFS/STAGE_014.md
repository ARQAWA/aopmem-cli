# RC5 Stage 014 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Implemented `aopmem tool dedupe apply --exact-only --json`.

The operation rebuilds authoritative bounded dedupe state after lock and
`BEGIN IMMEDIATE`. It hashes each shortlisted implementation once, groups
equal fingerprints without pair expansion, preserves non-exact candidates as
review-only results, and never performs an unsafe apply mode.

Duplicates are marked `superseded` in SQLite and `tool.json`; aliases point
directly to an active canonical. Existing aliases targeting a duplicate are
retargeted before its status changes. Tool directories and executables remain.

Manifest writes and rollback restoration use anchored no-follow handles.
Snapshot failure follows the shared committed-mutation pending-marker contract.

## Focused checks

`rtk cargo test --locked stage_014 -- --nocapture --test-threads=1`: PASS 10/10.

`rtk cargo clippy --all-targets --locked -- -D warnings`: PASS.

`cargo fmt --check`: PASS.

`rtk cargo test --locked`: PASS 701/701.

`rtk cargo test --tests --locked`: PASS 701/701.

`rtk ./scripts/dev_verify.sh`: PASS.

`git diff --check`: PASS.

P1: `0`. P2: `0`.

## Audit remediation

The authoritative rescan inherits Stage 012's post-hash anchored file identity
check. It rejects a same-path regular implementation replacement before any
canonicalization write; identity data is not exported.

It also inherits the streamed descendant-entry cap and manifest identity
recheck. Drift fails before canonicalization and CLI errors expose no paths.
Focused public plan/preflight/apply replacement tests prove the shared scanner
fails closed; apply leaves manifests and aliases unchanged.

Verified through remains `STAGE_010`; next cumulative audit is `STAGE_015`.

## Cumulative-audit apply privacy and zero-write remediation

The compiled CLI is now exercised for real apply filesystem failures in both
text and JSON modes. The complete captured stdout plus stderr contains
`TOOL_DEDUPE_FILESYSTEM_UNSAFE` and contains no unique symlink-name,
`AOPMEM_HOME`, isolated root, or absolute-target canary. JSON is parseable and
uses command `tool_dedupe_apply`, code `TOOL_DEDUPE_PLAN_FAILED`, null data,
one error, and no warnings. Text mode writes only the exact stable error to
stderr.

The otherwise hard-to-force direct `ToolDedupeApplyError::Io` mapper branch
also has a raw absolute-path canary test. Both its text rendering and JSON
envelope retain only the stable safe reason.

The targeted same-path replacement apply proof now uses a same-byte
replacement and snapshots both complete SQLite `ToolContractRecord` values,
both manifest byte sequences, both runner byte sequences, both tool-directory
existence states, and aliases. Identity drift fails before canonicalization;
all snapshots and aliases remain unchanged.

```text
rtk cargo test --locked \
  stage_014_dedupe_apply_io_error_never_exposes_raw_path \
  -- --nocapture --test-threads=1
PASS 1/1; 708 filtered

rtk cargo test --locked \
  stage_012_013_014_public_operations_fail_closed_on_targeted_file_swap \
  -- --nocapture --test-threads=1
PASS 1/1; 708 filtered

rtk cargo test --locked
PASS 709/709

rtk cargo test --tests --locked
PASS 709/709

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

The cumulative audit deliberately remains `FAIL`; this implementation handoff
does not self-promote Stages 011–015 to `VERIFIED`.
