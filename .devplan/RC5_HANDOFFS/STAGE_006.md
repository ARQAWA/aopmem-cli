# RC5 Stage 006 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

`aopmem task apply` and `aopmem task complete` now implement the
authoritative task lifecycle contract.

Apply requires the exact global bundle id and factual selected ids or a
proved `--none-relevant` result. First apply validates one current
revision-bound operational snapshot and one bulk node query before the
authoritative `started -> applied` transition. Exact replay returns stored
state without an operational reread or duplicate event.

Complete records only result, derived duration, stable error code, bounded
redacted failure reason, and workflow/tool ids derived from authoritative
applied state. Terminal state is immutable. Failed completion is allowed from
started or applied.

P1: `0`.

P2: `0`.

## Files

Production scope stayed at the Stage 06 limit of three files:

- `src/cli/mod.rs`;
- `src/task/mod.rs`;
- `src/observability/task_state.rs`.

Documentation and bookkeeping:

- `docs/TASK_START_PROTOCOL.md`;
- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_006.md`.

No schema, operational-storage, installer, adapter, alias, or release file was
changed by Stage 06.

## Contract proof

- apply requires canonical task and global bundle ids;
- empty apply without `--none-relevant` fails closed;
- none-relevant accepts mandatory gate/rule facts only and requires complete,
  non-exhausted retrieval with zero stored task nodes;
- exact replay fingerprint is derived inside typed inputs after stable sort
  and normalization;
- different apply/complete arguments return `TASK_CONFLICTING_REPLAY`;
- exact apply replay remains valid after operational revision changes and
  creates no duplicate event;
- first apply checks workspace, bundle, current revision, stored membership,
  operational existence, application kind, and current status;
- `draft` and `active` are accepted; `deprecated`, `superseded`, and `broken`
  return `TASK_NODE_INACTIVE`;
- correction, lesson, and incident-scar nodes use one correction kind;
- operational memory remains read-only by revision and table-row proof;
- complete derives duration from stored start time;
- accepted workflow/tool ids come only from stored applied nodes;
- success and partial require applied state;
- failed requires an error code and is allowed from started or applied;
- raw failure reason is absent from database, WAL/SHM, output data, and event
  payload; persistence stores only redacted reason and replay fingerprint;
- different raw secret reasons produce different replay fingerprints even
  when their persisted redaction is the same;
- best-effort apply/complete projection failure does not undo authoritative
  state;
- exact replay emits no second context-applied, completed, or failed event.

## Complexity proof

First apply uses one bounded JSON-backed bulk SQL query for all requested node
ids. There is no SQL query inside the applied-node loop.

Typed input normalization is `O(n log n)` for stable sort and duplicate
detection. Operational membership validation is `O(n log m)` by binary search
over the stored sorted bundle. Authoritative bundle validation is `O(n + m)`
with a monotonic two-pointer scan. Complete reads the stored applied ids once.

The existing 8,192-node defensive bound applies before persistence and bulk
validation. No cache, schema change, or broader retrieval rewrite was added.

## Checks

```text
cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked task_ -- --nocapture
PASS 45/45

rtk cargo test --locked
PASS 639/639
```

Focused proof includes TP-04 through TP-09, exact replay, current-status
validation, correction-family normalization, none-relevant proof, logical
operational no-write, privacy scan, event facts, and projection isolation.

## Audit state

Stages 001–005 remain `VERIFIED`.

Stage 006 is `DONE_LOCAL_CHECKS_PASSED`, not `VERIFIED`. The next cumulative
audit is due through Stage 010.

Continue with `STAGE_007`: Memory Keeper V2 skill.
