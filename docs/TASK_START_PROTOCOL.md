# Task lifecycle protocol

This document defines the authoritative task-state boundary and the
`aopmem task start`, `task apply`, and `task complete` contracts for AOPMem
v0.2.0-rc5.

## Start command

`task start` accepts exactly one query transport:

```text
aopmem task start --query "<exact request>" --json
printf '%s' "<exact request>" | aopmem task start --query-stdin --json
```

The transports are mutually exclusive. Input must be non-blank UTF-8, contain
no NUL byte, and be at most 65,536 UTF-8 bytes. The stdin form is the managed
agent transport because it avoids shell interpolation.

A global `--bundle-id` is invalid for start. Start allocates one task id and
one bundle id through the fallible task-state UUID allocator. Random-source
failure is a typed, fail-closed error.

## Authority

Local Observability schema v2 is the only authority for task and bundle
lifecycle state. Operational memory is read for validation, but task history
is never written there.

The authoritative tables are:

- `tasks`: identity, workspace, memory revision, privacy-safe fingerprints,
  lifecycle timestamps, status, retrieval facts, replay fingerprints, and
  terminal result;
- `task_bundle_nodes`: the exact selected mandatory and task node ids;
- `task_applied_nodes`: the exact applied gate, rule, workflow, tool,
  correction, and failure-mode ids.

The lifecycle is:

```text
started -> applied -> completed
        \-> failed
started ------------> failed
```

Terminal state is immutable. An exact replay fingerprint is idempotent. A
different replay is a typed conflict.

## Start durability

A successful start response is allowed only after one transaction stores:

- canonical lowercase UUID v4 `task_id` and `bundle_id`;
- managed `workspace_key`;
- workspace-bound `memory_revision`;
- normalized-query fingerprint;
- mandatory/retrieval/budget facts;
- the complete selected node-id set;
- the `started` status and durable start timestamp.

The persistence API does not accept raw query text. Failure to open, migrate,
validate, or write required state fails closed.

There is no fallible core step after the authoritative start transaction.
Only the best-effort event projection runs after it. Therefore event failure
may add `OBSERVABILITY_WRITE_FAILED`, but cannot turn a durable start into a
failed start or roll it back.

## Retrieval snapshot and bounds

Start resolves the current workspace and opens operational memory read-only.
Revision binding, mandatory loading, and every retrieval layer run inside one
`BEGIN DEFERRED` read transaction. WAL writers may continue, but the start
package sees one revision-bound snapshot. Start never changes the operational
revision.

Active mandatory nodes are loaded completely for these types, in order:

1. `kernel_contract`;
2. `gate`;
3. `project_profile`;
4. `source`;
5. `rule`.

The mandatory section has a 1 MiB canonical JSON UTF-8 hard limit. Any
overflow returns `MANDATORY_CONTEXT_OVERFLOW`. No task row, partial package,
or observability event is created.

Task candidates are read in four SQL layers: typed roots, FTS5/BM25, direct
links, and depth-two graph traversal. Ordering uses source hierarchy, trust,
confidence, layer priority, BM25 where applicable, and stable IDs. Selection
deduplicates nodes globally and merges every reason.

The invocation has two independent retrieval bounds:

- at most 2,048 candidates per layer;
- at most 16 MiB of complete resident candidate payload across all layers.

Rows are streamed through four SQL statements. There is no OFFSET loop or
per-node query. One current SQLite row may temporarily exceed the 16 MiB
logical resident cap; every stored body is independently capped at 1 MiB.
Small packages with more than 128 candidates in every layer remain complete.

Selected task context has a 256 KiB canonical JSON UTF-8 soft limit. Nodes are
never split or shortened. Candidate-scan exhaustion and canonical-byte
exhaustion are reported separately. Both produce a valid bounded package with
`retrieval_complete=false` and `budget_exhausted=true`. A complete package
always has `retrieval_complete=true` and `budget_exhausted=false`.

Ordinary start never returns a continuation cursor and never requires a
manual continuation loop.

## Start response

The JSON `data` object contains these required fields:

1. `task_id`;
2. `bundle_id`;
3. `workspace_key`;
4. `memory_revision`;
5. `mandatory_context_complete`;
6. `retrieval_complete`;
7. `budget_exhausted`;
8. `mandatory_nodes`;
9. `task_nodes`;
10. `applicable_gates`;
11. `applicable_rules`;
12. `candidate_workflows`;
13. `candidate_tools`;
14. `relevant_corrections`;
15. `relevant_failure_modes`;
16. `hunches`;
17. `selection_reasons`.

It also reports exact byte-budget metadata, separate
`canonical_json_bytes`/`candidate_scan` exhaustion flags, the per-layer count
limit, and the candidate-scan byte limit. It has no `continuation_cursor`.

`relevant_corrections` includes `correction`, `lesson`, and `incident_scar`
nodes. Their authoritative apply category is normalized to `correction` while
their returned full nodes keep the original operational type.

The raw query is not returned as a protocol field and is never written to the
operational database, Local Observability database, WAL, task state, or event
payload. Only its normalized fingerprint enters authoritative task state.

## Apply command

Apply requires the task id, the exact global bundle id returned by start, and
either factual context ids or `--none-relevant`:

```text
aopmem task apply \
  --task-id "<task UUID>" \
  --bundle-id "<bundle UUID>" \
  --applied-gate-id 11 \
  --applied-rule-id 12 \
  --selected-workflow-id 21 \
  --selected-tool-id 22 \
  --selected-correction-id 23 \
  --selected-failure-mode-id 24 \
  --json
```

Every id flag is repeatable. An empty application without
`--none-relevant` fails with `TASK_EMPTY_APPLICATION`. The none-relevant proof
may carry mandatory gate/rule ids, but conflicts with workflow, tool,
correction, or failure-mode ids.

The first apply:

- loads authoritative task state and checks workspace and bundle identity;
- opens operational memory read-only in one bounded snapshot;
- checks the current workspace-bound revision before node facts;
- reads all requested ids in one bounded bulk query;
- checks stored bundle membership and application-kind compatibility;
- accepts current `draft` and `active` nodes;
- rejects unknown, `deprecated`, `superseded`, and `broken` nodes;
- treats `correction`, `lesson`, and `incident_scar` as one correction kind;
- writes only authoritative Local Observability task state.

`--none-relevant` also requires a complete, non-exhausted retrieval with zero
stored task nodes. It cannot be inferred from an exhausted candidate scan.

The response reports authoritative applied ids, state, revision,
`none_relevant`, and `replayed`. An exact replay returns the stored state
before operational revision or node revalidation and creates no duplicate
event. Different arguments return `TASK_CONFLICTING_REPLAY`.

Stable validation errors include:

- `TASK_BUNDLE_REQUIRED`;
- `TASK_WRONG_WORKSPACE`;
- `TASK_FOREIGN_BUNDLE`;
- `TASK_STALE_REVISION`;
- `TASK_UNKNOWN_NODE`;
- `TASK_NODE_OUTSIDE_BUNDLE`;
- `TASK_NODE_INACTIVE`;
- `TASK_NODE_KIND_MISMATCH`;
- `TASK_NONE_RELEVANT_CONFLICT`.

## Complete command

Complete accepts only terminal facts:

```text
aopmem task complete \
  --task-id "<task UUID>" \
  --result success \
  --json

aopmem task complete \
  --task-id "<task UUID>" \
  --result failed \
  --error-code TOOL_FAILED \
  --reason "bounded factual reason" \
  --json
```

`success` and `partial` require the applied state and reject error details.
`failed` requires a stable error code and is valid from started or applied.
Its optional reason is trimmed, bounded to 1,024 bytes, and redacted before
persistence. Complete never accepts raw output, chat, reasoning, or new
workflow/tool ids.

Duration is derived from the stored start timestamp. Accepted workflow and
tool ids are derived from authoritative `task_applied_nodes` and returned in
the response. A supplied global bundle id is optional for complete, but must
match the stored task bundle.

Terminal state is immutable. Exact completion replay returns the same
timestamp and duration without a duplicate event. A different result, code,
or normalized reason returns `TASK_CONFLICTING_REPLAY`.

## Events, privacy, and retention

`task.started`, `task.context_applied`, `task.completed`, and `task.failed` are
best-effort factual projections. They never replace authoritative state and
their failure never rolls back a valid task-state transition.

Raw query, chat, output, and reasoning are never persisted. Task state follows
the Local Observability limit: 30 days or 100,000,000 bytes per workspace.
Expired state and unknown task ids return the same stable
`TASK_NOT_FOUND_OR_EXPIRED` error.

Missing or unsafe operational memory fails closed before task-state or event
creation. Mandatory overflow behaves the same. Required task-state write
failure also fails closed. Only factual event projection is best effort.
