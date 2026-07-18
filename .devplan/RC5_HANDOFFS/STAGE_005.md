# RC5 Stage 005 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

`aopmem task start` now resolves the workspace, reads one revision-bound
operational snapshot, builds the complete mandatory section, retrieves all
four task layers internally, persists authoritative start state, and returns
one final JSON package without a continuation cursor.

Raw query text is absent from task state, events, operational memory, and the
protocol fields. Missing memory, mandatory overflow, invalid state facts, RNG
failure, and authoritative write failure fail closed. Best-effort event
failure cannot undo a durable start.

P1: `0`.

P2: `0`.

## Files

Stage 005 implementation:

- `src/cli/mod.rs`;
- `src/storage/mod.rs`;
- `src/task/mod.rs`;
- `docs/TASK_START_PROTOCOL.md`.

Two approved adjacent invariant exceptions:

- `src/observability/mod.rs`: one schema CHECK plus its fresh-schema test;
- `src/observability/task_state.rs`: fail-closed load validation plus a
  corruption test.

The exceptions were required because constructor-only validation would not
protect direct SQLite rows or corrupted existing state.

## Contract proof

- inline and stdin query transports are exclusive and bounded to 65,536 bytes;
- task and bundle UUID allocation is fallible and maps RNG failure to
  `TASK_RANDOM_UNAVAILABLE`;
- operational memory is opened read-only and one WAL snapshot binds revision,
  mandatory context, and all retrieval layers;
- mandatory context is exact and atomic at the 1 MiB hard limit;
- typed, FTS/BM25, direct-link, and depth-two graph candidates are globally
  deduplicated with merged selection reasons;
- more than 128 small candidates in every layer complete in one invocation;
- correction and failure-mode graph expansion is retained;
- lessons and incident scars remain original response nodes but normalize to
  the authoritative `correction` apply category;
- candidate scan and canonical JSON byte exhaustion are distinct;
- all incomplete packages have `budget_exhausted=true`; complete packages do
  not;
- the response has all 17 required fields, no cursor, and no raw-query field;
- mandatory overflow and missing memory create no task/event state;
- an event projection failure leaves the authoritative started task intact.

## Complexity proof

Scope: task-start candidate retrieval in `src/storage/mod.rs` and its caller in
`src/cli/mod.rs`.

Before the Stage 005 patch, four layers could collect `4 * 2,048` complete
rows before 256 KiB packing. With 1 MiB bodies this admitted roughly 8 GiB of
logical candidate payload.

After the patch:

- at most four SQL statements stream candidate rows;
- there is no OFFSET loop and no query inside a candidate loop;
- one shared 16 MiB logical resident payload budget covers all layers;
- one current SQLite row, whose body is independently capped at 1 MiB, may
  exist temporarily beyond the shared budget;
- count remains capped at 2,048 rows per layer;
- overflow stops scanning and sets candidate-scan exhaustion exactly.

Row scanning is `O(n)` after SQLite ordering. Existing bounded root dedupe and
final stable ordering remain `O(n log n)`. No cache or broader retrieval
rewrite was added.

## Checks

```text
cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

task start focused tests
PASS 6/6

complete task recall focused tests
PASS 4/4

WAL snapshot focused test
PASS 1/1

fresh schema retrieval/budget CHECK
PASS 1/1

corrupt retrieval/budget load rejection
PASS 1/1

storage task-recall regression
PASS 7/7

rtk cargo test observability:: --locked
PASS 91/91

rtk cargo test --locked
PASS 633/633

git diff --check
PASS
```

## Audit state

Stages 001–005 are not `VERIFIED`. The required cumulative audit through
Stage 005 is next.

After that audit passes, continue with `STAGE_006`: task apply and task
complete.
