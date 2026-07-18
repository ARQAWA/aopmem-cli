# Local Observability

Local Observability stores local product facts for one AOPMem workspace. It is
not remote telemetry. AOPMem does not send these facts to a cloud service.

## Storage and lifetime

The store is separate from operational memory:

```text
<workspace>/observability/observability.sqlite
```

It uses schema version 2. Exact version-1 stores migrate transactionally on
the next writable open. It is not included in `memory.sql` audit snapshots.
The collector runs only inside normal CLI invocations. There is no daemon or
background worker.

Retention is 30 days or 100,000,000 bytes per workspace, whichever limit is
reached first. Oldest facts are removed first. Export files are not removed by
collector retention.

Best-effort collector failure never changes the core command result. The
command keeps its exit status and adds `OBSERVABILITY_WRITE_FAILED`.
Authoritative task-state writes are different: they fail closed before a task
transition can be reported as successful.

## Read-only commands

```sh
aopmem observe status
aopmem observe status --json
aopmem observe report
aopmem observe report --json
```

These commands are strictly read-only. They do not:

- create a missing workspace or observability store;
- write a self-observation;
- open or migrate `aopmem.sqlite`;
- run retention;
- change memory, tools, artifacts, or audit history.

A missing store is a successful `not_collected` result. An unsafe,
incompatible, or malformed existing store fails closed. Preserve that store
and a backup, then run `aopmem doctor` for diagnosis.

## Report contract

For an initialized store, one report uses one SQLite read snapshot and one
captured RFC3339 millisecond time. The period is the inclusive 30 days ending
at that time. A concurrent recall continuation cannot add newer facts to that
report.

Recall facts use each lifecycle event timestamp in that window. `recall.count`
counts `recall.started`; failures, empty results, mandatory overflows, and
continuations count their matching events. Bundle-level `more_results`, FTS,
and graph facts use distinct bundle ids from in-window `recall.completed`
events. Terminal `more_results` uses the last completion for each bundle in
stable `(timestamp, id)` order. Parent bundle timestamps, latest outcomes, and
lifetime continuation counters do not change period facts. Selected nodes use
their in-window `first_seen_at`, including continuations of older bundles.

The report contains verifiable facts:

- task starts, context applications, starts without an apply by report end,
  completions, and failures;
- applied gate, rule, workflow, tool, correction, and failure-mode counts,
  grouped again by mandatory or task context;
- recall count, failure, empty, and mandatory overflow;
- continuation and `more_results` use;
- FTS fallback and graph traversal use;
- selected node types and the most selected workflows, tools, and failure
  modes;
- useful, partial, and wrong feedback;
- tool success, failure, timeout, and repeated errors;
- repeated correction and failure-mode titles;
- reflection proposed, applied, and drafted counts;
- blocked tool duplicates, resolved aliases, unresolved overlap blocks, and
  the last successful audit repair time;
- adapter drift missing, drifted, and failed events, pending audit snapshots,
  and doctor/verify failures;
- artifact cleanup deletion counts;
- MCP missing and configured-unverified observations.

Top lists contain at most 20 rows and always include `limit` and
`more_results`. The report does not invent a product score or advice.
Task lifecycle rows live only in this retention-bounded observability store.
They are not operational-memory nodes and are not included in `memory.sql`.
Debug capsule export reuses the same read boundary without collecting or
writing new events.

## Privacy boundary

Local Observability may store bounded ids, types, titles, summaries, source
references, selection reasons, scores, counts, durations, error codes, and
artifact metadata.

It does not store raw query or chat, hidden reasoning, full task text, raw node
bodies, raw tool stdout/stderr, environment values, authorization headers,
cookies, tokens, or secrets. Text is redacted before collection. Report output
redacts emitted titles, tool ids, and repeated error codes again.

`bundle_id` links one recall to later AOPMem operations. Feedback stays only in
this observability store and never changes operational memory.
