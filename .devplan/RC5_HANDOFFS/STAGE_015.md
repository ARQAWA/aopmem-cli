# RC5 Stage 015 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Added a real, bounded, secret-free fixture at
`fixtures/stage_015/confluence_tools/`. Its two active contracts are
`confluence_reader` and `confluence_reader_internal`. Their behavior and
runner bytes match; only identity/display name differ.

The fixture makes `_internal` older in SQLite. The generic five-rule selector
still chooses `confluence_reader` because its neutral suffix rule precedes the
final created-at tie-break. There is no Confluence branch in production code.

Focused tests prove plan class `SAME_IMPLEMENTATION_DIFFERENT_NAME`, exact
eligibility, one canonicalization, manifest and SQLite supersession, old-ID
direct alias, default canonical list, old-ID get/validate/run canonical path,
unchanged `external_read` approval `none`, retained directories and runner
bytes, idempotent replay, safe `tool.canonicalized`, and a non-Confluence
control pair. The adapter parity test also asserts Managed Block V2 tool
reuse/creation and approval rules.

P1: `0`. P2: `0`.

## Checks

```text
rtk cargo test --locked stage_015 -- --nocapture --test-threads=1
PASS 4/4

rtk cargo test --locked managed_block -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked stage_01 -- --nocapture --test-threads=1
PASS 58/58

cargo fmt --check; rtk cargo clippy --all-targets -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 704 passed

rtk cargo test --tests --locked
PASS 704 passed

rtk ./scripts/dev_verify.sh
PASS `dev verify passed` with 704 tests

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

## Audit state

Stages 011–015 are `DONE_LOCAL_CHECKS_PASSED`. Verified through remains
`STAGE_010`. The next required work is `CUMULATIVE_AUDIT_011_015`; this stage
does not independently mark 011–015 as verified.
