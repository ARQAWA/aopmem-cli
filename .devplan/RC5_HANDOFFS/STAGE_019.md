# RC5 Stage 019 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_020`

Verified through: `STAGE_015`

Next cumulative audit: `STAGE_020`

Native Windows runtime: `PENDING_DOGFOOD`

P1: `0`

P2: `0`

## Result

Implemented official commands:

```text
aopmem audit repair --current-workspace --json
aopmem audit repair --all-workspaces --json
```

Clap requires exactly one selector. Current-workspace resolution creates
nothing. All-workspaces discovery is bounded by 10,000 direct entries and
4 MiB of entry-name bytes, sorted by exact platform bytes, reparse-aware, and
continues through isolated unsafe entries. Any per-workspace failure yields a
non-zero aggregate exit while later valid workspaces still run.

## Core repair boundary

Per workspace:

1. acquire only the permanent snapshot lock;
2. inspect the anchored pending marker;
3. return `already_clean` without opening SQLite, publishing, or Git when the
   marker is absent;
4. open live operational SQLite through URI `mode=ro`, `NOFOLLOW`,
   `query_only=ON`, and `temp_store=MEMORY`;
5. establish one deferred read transaction;
6. reuse the Stage 010 tagged-value redactor and canonical streaming SQL
   writer;
7. publish through shared Atomic Publish V2;
8. reopen `memory.sql` through the anchor and compare SHA-256 with the digest
   accumulated during streaming;
9. update local audit Git with truthful
   `GitCommitOutcome::{Created, Unchanged}`;
10. sync prior state, then clear `.pending-snapshot` as the final core action.

No workspace mutation lock is taken. Operational memory is never written.
DB and WAL bytes remain unchanged, including when the latest committed row is
present only in live WAL.

Publish error `87`, digest mismatch, Git failure, and marker-clear durability
failure retain or restore the pending marker. The command never deletes it
manually.

## Observability and privacy

After core success/failure, best-effort Local Observability records exact
`audit.repair.completed` or `audit.repair.failed` events with empty payloads.
Observability failure cannot change core status or marker state.

JSON/text errors contain only safe workspace key, stable code, I/O kind,
optional raw platform error, and marker/operational-write facts. They contain
no paths, SQL, memory content, or secret values.

Doctor and verify now emit the exact fix hint:

```text
aopmem audit repair --current-workspace --json
```

## Complexity

Discovery is one bounded pass plus `O(n log n)` stable sort and `O(n)` repair
dispatch. Each workspace opens one operational connection and one read
transaction. SQL rows stream through the existing writer. Digest validation
adds two linear byte passes, one while writing and one anchored reopen.
No pair expansion, N+1 database/event query, unbounded traversal, or second
publish/dump framework was added.

## Proof

```text
focused Stage 019
PASS 5/5

doctor/verify exact repair hint
PASS 1/1

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 725/725

rtk cargo test --tests --locked
PASS 725/725

rtk ./scripts/dev_verify.sh
PASS, 725 tests plus CLI, negative, hunch, and drift proof

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Independent final read-only review: `PASS`.

P1: `0`.

P2: `0`.

Native Windows execution remains `PENDING_DOGFOOD`. No Windows runtime PASS is
claimed.
