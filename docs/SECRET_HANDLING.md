# Secret handling

AOPMem separates secret use, operational persistence, external action
approval, and export redaction. It does not add a secrets manager, automatic
secret detector, or parallel storage system.

## Authorized use

The agent may use passwords, tokens, cookies, API keys, and other credentials
provided or authorized by the user. This includes test credentials, protected
VDI credentials, and credentials limited to a closed test contour.

The agent must not refuse, lecture, force a placeholder, remove the value from
a required command, or change the task solely because input looks secret.

## Explicit operational persistence

A secret-bearing request alone never writes operational memory. Exact local
storage requires an explicit `remember`, `teach`, save, or equivalent direct
user instruction. That instruction is local-storage permission and needs no
extra `+++`.

An exact test secret must be stored with this canonical shape:

- node type: `raw_note`;
- status: `active`;
- title: `Authorized test credential`;
- summary: absent;
- source reference: `source=user_instruction`;
- confidence: `1.0`;
- trust level: `high`;
- exact value: only in `body`;
- tag: exactly `sensitivity:test_secret`.

Use a nonsecret teach session. Then use a fixed direct-process runner for the
one secret-bearing invocation:

```text
executable: aopmem
argv: ["teach", "propose", "--session-id", "<session-id>",
       "--apply", "--payload-stdin"]
stdin:
{"items":[
  {"op":"create_node","node_ref":"test_secret","node_type":"raw_note",
   "status":"active","title":"Authorized test credential",
   "body":"TEST_ONLY_TOKEN_STAGE009_CANARY",
   "source_ref":"source=user_instruction","confidence":1.0,
   "trust_level":"high"},
  {"op":"add_tag","node_ref":"test_secret",
   "tag":"sensitivity:test_secret"}
]}
```

The canary above is fake test data. Never put a real credential in a test,
fixture, document, proof log, or source file.

Put secret-bearing proposal JSON only on direct process stdin. Never put it in
argv, shell text or a pipeline, a temporary file, an environment value, a log,
an error, or a receipt. Inline `--payload` remains available only for
nonsecret compatibility flows.

`teach propose --apply --payload-stdin` stores the proposal and applies all
items inside one workspace mutation transaction. The command publishes one
audit snapshot only after the exact node and tag coexist. A failure rolls back
the proposal, node, tag, and apply receipt together.

Do not use `remember` followed by `tag add`. Do not split a secret-bearing
proposal into separate `teach propose` and `teach apply` commands. Both
patterns can create a committed untagged state before an audit snapshot.

## Approval follows the action

| Action | `+++` |
|---|---|
| Contract-safe external read with `approval_requirement=none` | Not required |
| Authentication to an authorized designated test system | Not required solely for credentials |
| External write, destructive action, or explicit high risk | Standalone exact `+++` required |

Secret presence never changes the action class. A non-explicit external read
or authentication may use the credential but must not create nodes, tags, or
another operational-memory revision.

## Redaction and backup

Operational memory may retain an exact value only through the atomic explicit
flow above. Durable full-home backup preserves that authorized value.

Observability, reports, errors, task summaries, session evidence, audit
snapshots, debug capsules, and other exports replace every copy of a tagged
exact value with:

```text
<TEST_SECRET_REDACTED>
```

The tag is the redaction anchor. Export code must also scrub copies of the
same exact value from proposal records and other export rows. Operational
memory remains the only exact local working copy.

## Exact tagged-value redactor

Every protected read or export loads one `TaggedValueRedactor` from the
caller-owned operational read snapshot. It uses one `tags JOIN nodes` query
for the exact binary tag `sensitivity:test_secret`. Only the tagged node
`body` is an anchor. Titles, summaries, arguments, environment values, and
heuristic matches never become anchors.

The source is bounded and fail-closed:

- at most 1024 distinct values;
- at most 16 MiB of distinct source bytes;
- each body is non-empty valid UTF-8, contains no NUL, and is at most 1 MiB;
- canonical JSON-string representations have a separate 16 MiB bound;
- redaction output may expand by at most 16 MiB.

Duplicate values are removed deterministically. Candidates are ordered by
longest value first, then raw byte order. Matching scans the original input
once from left to right. The earliest match wins. At one offset, the longest
match wins. An existing `<TEST_SECRET_REDACTED>` marker is copied as one
atomic token and is never scanned again. This keeps repeated redaction
idempotent even when tagged values are `TEST`, `SECRET`, or a marker
substring. There is no minimum secret length.

Audit rows can contain a proposal JSON copy where quotes, backslashes, and
control characters are escaped. The audit path uses the same one-pass rule
over both raw values and their canonical JSON-string representations.
Structured JSON outputs redact every string value and object key before JSON
serialization. A key collision after redaction fails closed.

## Protected surfaces and failures

The exact marker is applied before publication to:

- Local Observability payloads, error codes, recall metadata, and feedback;
- authoritative task failure codes and bounded reasons;
- effectiveness reports and Local UI responses;
- audit `memory.sql`, including teach proposal copies;
- every debug capsule JSON and JSONL entry.

Existing heuristic `[REDACTED]` handling remains active for untagged
secret-like text. It does not replace the exact tagged-value marker.

An invalid or unreadable tagged source stops the protected output. The
best-effort collector disables itself and emits only
`OBSERVABILITY_WRITE_FAILED`. A failed task redaction lookup still persists
the authoritative terminal transition with
`TASK_REDACTION_UNAVAILABLE` and no reason. It never reports ordinary success
without a safe persisted state.

A genuinely absent operational database is an empty anchor set only for
pre-initialization best-effort observability and report paths. No tagged value
can exist before that database exists. If the database exists but is invalid,
unreadable, linked, or otherwise unsafe, the protected path fails closed.

The operational SQLite database keeps the exact authorized value. Durable
SQLite/full-home backup also keeps the exact body and tag. Audit Git and debug
capsules are redacted exports and are not secret recovery backups.
