# RC8 Recovery State Machine

Schema:

```text
journal_schema_version = 1
target_version = 0.2.0-rc8
```

Phases:

```text
01-backup-complete
02-staged-verified
03-prepared
04-apply-started
05-applied
06-published
```

Fresh run is safe for clean, stale pre-apply, malformed pre-apply, and
pre-apply gap evidence. Apply-started and unknown outcome evidence fails
closed.
