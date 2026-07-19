# Debug Capsule

`aopmem observe export` creates a local, read-only ZIP for external analysis.
The command does not upload the capsule or contact any external service.

## Command

```text
aopmem observe export --output <existing-parent>/<new-file>.zip
aopmem --json observe export --output <existing-parent>/<new-file>.zip
```

The current project workspace must already contain a valid AOPMem operational
database. The output parent must be an existing real directory. The output
file must not exist. Export never overwrites a file.

## Read-only contract

Export:

- opens operational memory and Local Observability read-only;
- uses SQLite URI `mode=ro`, `query_only=ON`, and in-memory temp storage for
  the live operational database, including committed WAL-only state;
- does not run operational or observability migrations;
- does not initialize a collector or record its own invocation;
- does not run retention, tools, adapter actions, or memory mutations;
- validates schema, integrity, workspace binding, and row types before publish;
- reads each database from one stable read transaction.

The operational and observability databases are separate stores. Export does
not claim a cross-database atomic snapshot. It starts the operational snapshot
first, then reads one stable observability snapshot.

A missing Local Observability store is valid. The capsule reports
`collection_status=not_collected`, emits empty JSONL datasets, and does not
create the store. An existing unsafe, corrupt, or incompatible store fails
closed and leaves no final capsule.

SQLite read access may create or maintain empty WAL locking sidecars when the
database journal mode requires them. Export does not change main database
bytes, schema, rows, or modification time, and creates no rollback journal.

## Exact contents

The ZIP contains these 12 entries in this exact order:

| Entry | Content |
|---|---|
| `manifest.json` | Format, versions, workspace, reference time, entry list |
| `product.json` | Product and schema versions, local-only flag |
| `workspace_summary.json` | Collection state and fact-only effectiveness report |
| `memory_summary.json` | Counts and privacy-bounded summaries for all nodes |
| `health.json` | Latest validated doctor and verify observations |
| `events.jsonl` | Bounded event metadata without raw payloads |
| `recall_bundles.jsonl` | Recall outcomes, timings, and continuation facts |
| `bundle_nodes.jsonl` | Selected nodes, scores, and selection reasons |
| `feedback.jsonl` | Useful, partial, or wrong feedback and bounded reason |
| `tools_summary.json` | Tool status, side effects, and approval requirement |
| `mcp_summary.json` | MCP status and safe operation metadata |
| `README.md` | Capsule scope and privacy note |

`memory_summary.json` includes counts by node type and status, broken,
orphaned, deprecated, and draft counts, link count, and a streamed node list.
Each node may include its id, type, status, title, bounded summary, source,
trust, confidence, and incoming/outgoing link counts. It never includes the
full node body.

`health.json` reports `not_collected`, `success`, `warning`, or `failure` for
doctor and verify. It uses validated persisted observations. It does not
invent a healthy default when no observation exists.

## Determinism

Unchanged database snapshots produce byte-identical ZIP files. Entries use a
fixed order, stored ZIP64 encoding, fixed ZIP metadata, LF line endings, and
stable row ordering.

The reference time is the latest persisted Local Observability timestamp. A
missing or initialized-empty store uses
`1970-01-01T00:00:00.000Z`. Export does not use the wall clock.

## Privacy boundary

All free text is passed through the shared deterministic tagged-value and
Local Observability redactors. Exact tagged values and their canonical
JSON-string copies are removed before ZIP serialization. The capsule may
include node ids, types, titles, bounded summaries, source references,
trust/confidence values, selection reasons, counts, durations, error codes,
tool ids, MCP ids, approval facts, and recall scores.

The capsule does not include:

- the operational or observability SQLite database;
- full node bodies;
- raw artifacts, chats, task text, or tool stdout/stderr;
- raw event `payload_json`;
- full tool contracts;
- MCP credential sources or notes;
- environment variables, secrets, tokens, cookies, or authorization headers.

Installer and transplant reports may be referenced only as bounded facts:
stable error code, phase, rollback status, counts, and sanitized evidence
roles. Capsules must not embed raw inventories, proxy URI values, secret file
contents, quarantined old-home payloads, or failed RC9 payloads.

Review the capsule before sharing it. The export is designed for manual upload
to ChatGPT or another external analysis tool, but sharing remains a user
action.

## Safe publication and errors

Export writes a private same-directory temporary file, syncs it, captures its
identity, closes the writable handle, then validates a fresh read handle in a
short scope. All validation handles close before publication. It transfers the
owned file to shared Atomic Publish V2 in `NoReplace` mode. Windows uses
`MoveFileExW(MOVEFILE_WRITE_THROUGH)` and one shared absolute verbatim
drive/UNC path conversion for normal long and non-ASCII paths. Export never
uses replace mode. Temporary files are removed and their absence is checked
after pre-publication failure.

The command does not create, clear, or otherwise change the audit
`.pending-snapshot` marker. Audit repair and capsule export share the same
publisher but keep separate lifecycle rules.

Common error codes are:

| Code | Meaning |
|---|---|
| `OUTPUT_EXISTS` | The requested final file already exists |
| `EXPORT_UNSAFE_PATH` | The output parent or leaf is unsafe |
| `WORKSPACE_NOT_FOUND` | Operational memory does not exist |
| `WORKSPACE_UNSAFE_PATH` | The managed workspace path is unsafe |
| `DB_SCHEMA_ERROR` | Operational memory is invalid or incompatible |
| `OBSERVABILITY_UNSAFE_PATH` | The observability path is unsafe |
| `OBSERVABILITY_INVALID_STORE` | The store is corrupt or incompatible |
| `OBSERVABILITY_READ_FAILED` | The read-only observability read failed |
| `PLATFORM_PUBLISH_FAILED` | Atomic publish failed; inspect private structured state and `raw_os_error` |
| `EXPORT_FAILED` | Export failed before final publication |

If the final file is already visible but directory durability or temporary
cleanup could not be confirmed, the core command remains successful and emits
`EXPORT_PUBLISHED_WITH_WARNING`. JSON then reports
`publication_status=published_with_warning` and an honest
`temporary_cleanup_confirmed` value. Keep the published file and inspect the
warning; do not retry to the same path.

Structured publish errors expose only stable roles and state: mode, strategy,
phase, raw OS code, I/O kind, source/destination existence, source size,
commit, validation, durability, and cleanup. They never expose either path or
file content. A committed but not validated failure is reported as committed;
it is never mislabeled as a pre-publication failure.

The v0.2 local/no-sandbox threat boundary excludes active tampering by another
process running as the same user. On Unix this includes the narrow leaf-name
race between source identity verification and `linkat`. Export still rejects
ordinary links, unsafe managed paths, replacement of the verified temporary
file, and all tested no-clobber violations.
