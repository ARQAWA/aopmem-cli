# AOPMem v0.2.0-rc4 Windows Backup Root Cause

Status: `CODE_PROVEN`; native Windows rc4 confirmation: `PENDING`.

## Observed dogfood failure

- Host: Windows 11 x64 VDI, native PowerShell 5.1, no admin.
- Staged binary: `aopmem 0.2.0-rc3`.
- Command:
  `upgrade apply --all-workspaces --json --approved "+++"`.
- Result: exit `1`, `WORKSPACE_BACKUP_FAILED`,
  `Access is denied. (os error 5)`.
- Stopped workspace: `p-sit-cat-rental-8ef3bf83`.
- Partial backup size: `184,320` bytes.
- Migrations: none. Installed v0.1 binary: unchanged.

The failed rc3 root and the durable external full backup must be retained.

## Exact code-level cause

The rc3 backup helper finished SQLite Online Backup and validation, dropped
the destination connection, then called:

```rust
File::open(destination_path)?.sync_all()?;
```

`File::open` creates a read-only Windows handle. `sync_all` uses
`FlushFileBuffers`, which requires write access. Windows can therefore return
`ERROR_ACCESS_DENIED` (`raw_os_error=5`) for this exact operation.

The populated 184,320-byte file fits this sequence: SQLite wrote the backup,
but the later durability flush failed. Bytes alone were never proof of an
accepted backup, so migration correctly did not start.

## Call chain

```text
CLI upgrade apply
-> apply_all_workspaces
-> backup_and_migrate_workspace
-> SQLite source read connection
-> online backup destination connection
-> SQLite Backup::step until Done
-> destination validation
-> destination connection close
-> File::open(destination_path).sync_all()
-> ERROR_ACCESS_DENIED 5
-> WORKSPACE_BACKUP_FAILED
-> no migration
```

## Excluded hypotheses

- The SQLite backup handle was already dropped before the failing flush.
- The destination SQLite connection was dropped before the failing flush.
- Existing Windows anchored directory durability treats unsupported directory
  flush errors `1` and `5` as the documented platform outcome.
- There is no evidence for antivirus or a transient sharing conflict.

The root cause is the read-only file handle used for `FlushFileBuffers`, not a
speculative external process.

## rc4 diagnostic boundary

rc4 reports:

- `workspace_key`;
- exact `backup_phase`;
- source, temporary, and final paths;
- `raw_os_error` and normalized `io_kind`;
- partial existence, size, and validation state;
- `migration_started`;
- safe `fix_hint`.

The macOS fault-injection proof confirms that `raw_os_error=5` at the fixed
boundary becomes `backup_phase=flush_temporary_file` and
`migration_started=false`. Native Windows rc4 runtime confirmation remains
`PENDING` until the new asset runs on Windows 11 without admin rights.
