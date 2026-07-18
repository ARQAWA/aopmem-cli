# RC5 Stage 020 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_021`

Verified through: `STAGE_020`

Next cumulative audit: `STAGES_021_025`

Native Windows runtime: `PENDING_DOGFOOD`

P1: `0`

P2: `0`

## Result

`aopmem observe export` keeps the frozen exact 12-entry deterministic Stored
ZIP64 contract and now completes the RC5 Windows integration.

- Capsule publication owns the source handle and calls only shared
  `platform_publish` in `NoReplace` mode.
- Windows parent, temporary, source, and final opens now share one absolute
  verbatim drive/UNC conversion with `ReplaceFileW`/`MoveFileExW`.
- Ordinary paths beyond legacy `MAX_PATH`, Unicode paths, OS temp paths,
  existing destinations, unsafe parents, and error `87` are covered.
- Publish errors preserve path-private phase, strategy, raw OS code, commit,
  validation, durability, existence, size, and cleanup state.
- A committed and final-validated directory-sync failure remains successful
  with `EXPORT_PUBLISHED_WITH_WARNING`; an unvalidated committed state remains
  a truthful error.
- Error `87` leaves no destination and no temporary file, and does not change
  the audit pending marker.
- Operational SQLite opens in live-WAL read-only URI mode with
  `query_only=ON` and `temp_store=MEMORY`.
- Live committed WAL rows are included while operational DB/WAL bytes remain
  unchanged. Export creates no observability store and records no self-event.
- Tagged-value redaction removes both raw exact values and stored canonical
  JSON-string copies before ZIP serialization.
- The exact 12 names, order, timestamps, modes, compression, entry data, and
  unchanged-input bytes remain deterministic.

Audit repair and capsule export now demonstrably share the same Atomic Publish
V2 boundary while retaining separate marker and no-overwrite policies.

## Complexity

The complexity-optimizer and Rust rules kept the patch local and streaming.
Windows path conversion is one `O(path bytes)` pass. Publication stays `O(1)`
in directory entries and OS calls. Capsule generation remains `O(exported
rows + exported text)` with stable SQL ordering, no second database scan, no
second ZIP buffer, no fallback copy, and no recursive cleanup.

## Proof

```text
focused debug capsule
PASS 22/22

focused structured CLI publish error
PASS 1/1

focused shared Windows path conversion
PASS 2/2

focused platform publisher
PASS 7/7

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 733/733

rtk cargo test --tests --locked
PASS 733/733

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

The first full test run exposed one stale source-audit assertion that expected
verbatim constants inside `platform_publish.rs`. It was updated to prove that
both the anchored open boundary and publisher call the one shared
`windows_path` helper. Both complete test runs then passed.

Native Windows execution is not available in this environment. No native
Windows PASS is claimed. Windows runtime remains `PENDING_DOGFOOD`.

Independent Stage 020 review: `PASS`; P1 `0`; P2 `0`.

## Cumulative audit remediation

The first cumulative audit for Stages 016–020 returned `FAIL`, P1 `1`, P2
`0`. It found that the normal mutation snapshot path removed
`.pending-snapshot` directly after its Git commit. On Unix, removal can succeed
and the following parent-directory sync can fail, leaving a failed operation
without its recovery marker.

The normal path now uses the same `finish_repair_locked` restore-on-clear-error
boundary as official audit repair. A deterministic hook test removes the
marker, injects a durability error, and proves:

- the command returns failure;
- `memory.sql` is already published;
- the Git snapshot commit is already present and replay is `Unchanged`;
- `.pending-snapshot` is restored before the error returns.

```text
focused post-remove marker restoration
PASS 1/1

focused existing repair clear restoration
PASS 1/1

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 733/733

rtk cargo test --tests --locked
PASS 733/733

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

The independent cumulative re-audit returned `PASS`; P1 `0`; P2 `0`.
See `.devplan/RC5_HANDOFFS/AUDIT_016_020.md`.
