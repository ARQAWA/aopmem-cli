# Audit repair

Official commands:

```text
aopmem audit repair --current-workspace --json
aopmem audit repair --all-workspaces --json
```

Exactly one selector is required.

Audit repair keeps the same Atomic Publish V2 boundary. It opens operational
SQLite read-only, writes a redacted snapshot to a temp file, validates the
digest after close, then publishes. `.pending-snapshot` is persistent evidence
until the repair completes.

The RC8 updater may run staged audit repair after recovery backup and before
prepare. It may also run post-publish repair. Failure before apply keeps the
old binary unchanged.
