# Recovery state machine

RC8 separates release version from journal schema.

```text
journal_schema_version = 1
target_version = 0.2.0-rc8
```

Stable phases:

```text
01-backup-complete
02-staged-verified
03-prepared
04-apply-started
05-applied
06-published
```

Journal fields:

- `run_id`
- `source_version`
- `home_identity`
- `safety_backup_root`
- `recovery_backup_root`
- `source_manifest_sha256`
- `backup_manifest_sha256`
- `staged_binary_name`
- `staged_sha256`
- `planned_workspaces`
- `apply_attempts`
- `binary_replaced`
- `created_at`
- `updated_at`

Fresh runs are allowed for clean state, stale pre-apply backups, malformed
pre-apply journals, and pre-apply phase gaps. Apply-started and unknown
outcome evidence fails closed.

Read-only inspection:

```text
aopmem upgrade recovery inspect --json
```

Fresh recovery backup:

```text
aopmem upgrade backup --all-workspaces --json
```

Diagnostic adopt remains explicit only. The official installer never uses it.
