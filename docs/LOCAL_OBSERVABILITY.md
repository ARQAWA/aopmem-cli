# Local Observability

Local Observability stores local product facts for one AOPMem workspace. It is
not remote telemetry. AOPMem does not send these facts to a cloud service.

## Storage and lifetime

The store is separate from operational memory:

```text
<workspace>/observability/observability.sqlite
```

It uses schema version 1. It is not included in `memory.sql` audit snapshots.
The collector runs only inside normal CLI invocations. There is no daemon or
background worker.

Retention is 30 days or 100,000,000 bytes per workspace, whichever limit is
reached first. Oldest facts are removed first. Export files are not removed by
collector retention.

Collector failure never changes the core command result. The command keeps its
exit status and adds `OBSERVABILITY_WRITE_FAILED`.

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

- recall count, failure, empty, and mandatory overflow;
- continuation and `more_results` use;
- FTS fallback and graph traversal use;
- selected node types and the most selected workflows, tools, and failure
  modes;
- useful, partial, and wrong feedback;
- tool success, failure, timeout, and repeated errors;
- repeated correction and failure-mode titles;
- reflection proposed, applied, and drafted counts;
- adapter drift missing, drifted, and failed events, pending audit snapshots,
  and doctor/verify failures;
- artifact cleanup deletion counts;
- MCP missing and configured-unverified observations.

Top lists contain at most 20 rows and always include `limit` and
`more_results`. The report does not invent a product score or advice.
This read boundary passed independent Stage 26 re-audit with `P1=0`, `P2=0`,
and `P3=0`. Debug capsule export reuses it without collecting or writing new
events.

## Privacy boundary

Local Observability may store bounded ids, types, titles, summaries, source
references, selection reasons, scores, counts, durations, error codes, and
artifact metadata.

It does not store raw chat, hidden reasoning, full task text, raw node bodies,
raw tool stdout/stderr, environment values, authorization headers, cookies,
tokens, or secrets. Text is redacted before collection. Report output redacts
emitted titles, tool ids, and repeated error codes again.

`bundle_id` links one recall to later AOPMem operations. Feedback stays only in
this observability store and never changes operational memory.
