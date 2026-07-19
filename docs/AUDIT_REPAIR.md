# Audit repair

Official RC6 commands:

```text
aopmem audit repair --current-workspace --json
aopmem audit repair --all-workspaces --json
```

Exactly one selector is required.

Repair takes only the audit snapshot lock. It opens operational SQLite with
URI `mode=ro`, enables `query_only`, and keeps temporary SQLite state in
memory. One deferred read transaction streams canonical `memory.sql` through
the tagged-value redactor and shared Atomic Publish V2 helper. Its writer
closes before source validation, and validation closes before publication. The reopened
published file must match the streamed SHA-256 digest before local audit Git
is updated.

`.pending-snapshot` is removed last, after publish, digest validation, and Git
success. Any core failure retains or restores it. Never delete the marker
manually. An absent marker returns `already_clean` without snapshot or Git
changes. Repeated unchanged repair reports `git_commit: unchanged`.

`--all-workspaces` uses bounded, stable discovery. Unsafe entries are isolated,
valid workspaces continue, and any failure produces a non-zero aggregate exit.
Errors expose safe status, I/O kind, raw platform error, digest/commit/marker
facts only. They do not expose paths, SQL, memory content, or secrets.

Observability is best-effort and runs after each core result. Its failure never
changes repair status or marker state. Operational DB and WAL bytes are not
changed.

Native Windows runtime proof remains `PENDING_DOGFOOD`.
