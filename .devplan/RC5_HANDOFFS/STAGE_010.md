# RC5 Stage 010 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Exact values anchored by the binary tag `sensitivity:test_secret` are now
replaced with `<TEST_SECRET_REDACTED>` before every protected publication.

Operational SQLite and durable SQLite/full-home backup keep the exact
authorized body and tag. Audit Git and debug capsules are redacted exports.

P1: `0`.

P2: `0`.

## Files

Product and test scope:

- `src/redaction.rs`;
- `src/main.rs`;
- `src/audit/mod.rs`;
- `src/mutation/mod.rs`;
- `src/observability/mod.rs`;
- `src/observability/task_state.rs`;
- `src/observability/report.rs`;
- `src/observability/export.rs`;
- `src/observability/ui.rs`;
- `src/ui/data.rs`;
- `src/ui/http.rs`;
- `src/upgrade/backup.rs`;
- `src/cli/mod.rs`;
- `docs/SECRET_HANDLING.md`.

Verification integration:

- `scripts/dev_verify.sh` now mutates one unique Managed Block V2 contract
  marker and fails if the drift fixture does not change the file.

Bookkeeping:

- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_010.md`.

No schema, detector, secrets manager, side-effect class, approval rule,
installer, release, or automatic-classification feature was added.

## Redactor contract

- one caller-owned operational read snapshot supplies one
  `tags JOIN nodes` query;
- the tag comparison is exact binary
  `sensitivity:test_secret`;
- only tagged node bodies become anchors;
- anchors are non-empty valid UTF-8 without NUL and are bounded by existing
  node-body limits;
- there are at most 1024 distinct anchors and 16 MiB raw anchor bytes;
- JSON-escaped copies have a separate 16 MiB bound;
- output expansion is limited to 16 MiB;
- invalid type, invalid value, unsafe database, query failure, or a breached
  bound fails closed;
- duplicate anchors are removed deterministically;
- candidates are ordered longest-first, then by raw bytes;
- matching uses one left-to-right pass over original input;
- the earliest match wins and the longest candidate wins at one offset;
- an existing marker is copied atomically, so repeated redaction is
  idempotent;
- there is no minimum anchor length.

Proposal JSON can contain escaped copies of quotes, backslashes, and control
characters. Audit rows match raw and canonical JSON-escaped candidates in
the same pass. Structured JSON redacts values and keys before serialization;
a key collision after redaction fails closed.

## Protected surfaces

- Local Observability events, errors, recall metadata, and feedback;
- authoritative task completion failure code and reason;
- effectiveness reports and Local UI API responses;
- audit `memory.sql`, including teach proposal copies;
- debug capsule JSON and JSONL entries.

An existing invalid or unreadable anchor source disables best-effort
observability with `OBSERVABILITY_WRITE_FAILED`. It blocks other protected
outputs. Task completion still persists a safe terminal transition with
`TASK_REDACTION_UNAVAILABLE` and no reason.

A genuinely absent operational database is treated as an empty anchor set
only by pre-initialization best-effort observability/report paths. No tagged
anchor can exist before the database exists.

## Regression and canary proof

- overlap cases `TEST` and `SECRET`, marker substrings, duplicate values,
  Unicode, quotes, backslashes, control characters, one-byte anchors, and
  repeated redaction are deterministic;
- source count, body size, total source bytes, JSON-copy bytes, and output
  expansion fail closed;
- fake canaries are absent from observability SQLite/WAL, task state,
  reports, UI JSON, audit SQL, and debug ZIP entries;
- `<TEST_SECRET_REDACTED>` is present where a protected copy existed;
- audit SQL restores without recreating an exact tagged secret;
- operational reads and online SQLite backup preserve the exact body and
  exact tag;
- Stage 009 atomic persistence and approval tests remain green;
- all existing tests pass after the intentional fail-closed observability
  expectation for corrupt operational databases.

## Complexity review

Source load uses one bounded query, deterministic dedupe, `O(k log k)` sort,
and first-byte candidate buckets. Redaction scans input offsets once and does
not rescan generated output. Candidate comparisons are bounded by fixed
anchor count and body-size limits.

Memory is bounded by raw anchors, separately bounded escaped anchors, their
indexes, the input, and at most 16 MiB output expansion. No N+1 query or
unbounded nested scan was introduced.

## Checks

```text
rtk cargo test --locked stage_010 -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked stage_009 -- --nocapture --test-threads=1
PASS 4/4

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked
PASS 661/661

rtk ./scripts/dev_verify.sh
PASS, including build, 661 tests, CLI proof, negative checks, and drift check

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

## Requirement state

`RC5-SEC-002`, `RC5-SEC-003`, and `RC5-SEC-004` are
`DONE_LOCAL_CHECKS_PASSED`.

Stage 010 portions of `RC5-TST-001` are complete.

## Next boundary

Stages 001–005 remain `VERIFIED`. Stages 006–010 remain
`DONE_LOCAL_CHECKS_PASSED`.

The cumulative audit through Stage 010 is now due. Until it passes, verified
through remains Stage 005. The current implementation stage is Stage 011:
tool aliases schema and storage API.
