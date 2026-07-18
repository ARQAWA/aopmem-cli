# RC5 Stage 018 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_019`

Verified through: `STAGE_015`

Next cumulative audit: `STAGE_020`

Native Windows runtime: `PENDING_DOGFOOD`

P1: `0`

P2: `0`

## Result

Implemented `aopmem platform check --json`.

The command returns before `CommandObservation` construction. It never
resolves `AOPMEM_HOME`, current workspace, operational memory, or
observability. A poisoned regular file supplied as `AOPMEM_HOME` remains
byte-for-byte and metadata unchanged.

The check uses one exclusive UUID-named private OS temp directory, Unix mode
`0700`, and the shared `src/platform_publish.rs` helper. It proves create,
writable flush, no-replace, unchanged existing destination, replace existing,
reopen byte validation, direct-child rejection, root identity, Unix reparse
rejection, the shared helper's reparse guard contract, and bounded cleanup.

Cleanup is `O(1)`. It removes only known children through the anchor, then
uses non-recursive `remove_dir`. There is no traversal, `remove_dir_all`,
workspace access, database access, shell, admin API, or second publish
framework.

Failure details are path-private and preserve `raw_os_error`, including an
injected error `87`, I/O kind, strategy, phase, existence, size, commit,
validation, durability, and cleanup state. Cleanup runs after every injected
failure point.

## Proof

```text
focused platform, reparse, and compiled CLI isolation tests
PASS 5/5

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 720/720

rtk cargo test --tests --locked
PASS 720/720

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Native Windows execution remains `PENDING_DOGFOOD`. No Windows runtime PASS
is claimed.

## Independent privacy review

Result: `PASS`.

The independent reviewer verified early CLI dispatch, poisoned home/temp
isolation, anchored create-new and cleanup, direct-child/reparse guards,
structured error `87`, stable JSON, and path privacy.

P1: `0`.

P2: `0`.
