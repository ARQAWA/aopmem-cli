# Upgrade from AOPMem v0.1.0-rc3

This document defines the supported upgrade to `v0.2.0-rc3`.
It covers existing SQLite-backed AOPMem v0.1 workspaces only.
The old file MVP is not supported.

## Supported path

| Item | Value |
|---|---|
| Source | SQLite-backed `aopmem 0.1.0` |
| Canonical source release | `v0.1.0-rc3` |
| Target | `v0.2.0-rc3` |
| Scope | all user-level AOPMem workspaces |
| macOS | Apple Silicon |
| Windows | Windows 11 x64, PowerShell 5.1 |

The canonical `v0.1.0-rc3` assets report `aopmem 0.1.0`.
A different v0.1 binary hash produces `NONCANONICAL_V010_BINARY` warning.
It does not by itself block upgrade. Workspace schema, integrity, paths,
available disk space, and successful preparation determine compatibility.

Unknown, corrupt, unsupported, or newer schemas still block upgrade.

## Why `upgrade prepare` exists

SQLite read activity can leave `aopmem.sqlite-wal` or
`aopmem.sqlite-shm`, including a zero-byte WAL after all processes exit.
`upgrade plan` must remain read-only and therefore cannot checkpoint or remove
these files. Treating every sidecar as unsafe without a supported preparation
command creates an upgrade deadlock.

`v0.2.0-rc3` adds:

```text
aopmem upgrade prepare --all-workspaces --json
```

Preparation performs supported local SQLite maintenance. Users must not run
manual SQL and must not delete WAL, SHM, or journal files manually.

## Safe update order

Use this fixed sequence:

1. Close AOPMem UI and other AOPMem processes.
2. Record installed binary version/hash and workspace directory names.
3. Create a durable full backup of the current AOPMem home.
4. Download and verify the `v0.2.0-rc3` binary and `SHA256SUMS`.
5. Run the staged binary:

   ```text
   aopmem upgrade prepare --all-workspaces --json
   ```

6. Require successful preparation for every workspace.
7. Without running another AOPMem DB read, run:

   ```text
   aopmem upgrade plan --all-workspaces --json
   ```

8. Require `ok=true`, `ready=true`, and `writes_performed=false`.
9. Run once:

   ```text
   aopmem upgrade apply --all-workspaces --json --approved "+++"
   ```

10. Require successful apply and retained migration backups.
11. Atomically publish the staged binary.
12. Run adapter, doctor, verify, recall, and observability checks.

Do not run `recall`, `doctor`, `verify`, UI, or another database reader between
`prepare` and `plan`. Such reads may recreate SQLite coordination sidecars.

## Preparation contract

For each workspace, in stable order, `upgrade prepare`:

- validates workspace, database, and sidecar paths;
- rejects unsafe symlinks and reparse points;
- acquires the existing workspace mutation lock;
- creates a durable per-workspace SQLite backup before checkpoint;
- uses the canonical AOPMem SQLite connection path;
- runs `PRAGMA wal_checkpoint(TRUNCATE)`;
- requires a non-busy, complete checkpoint;
- closes the SQLite connection before sidecar cleanup;
- removes only verified empty direct-child WAL/SHM sidecars;
- never deletes a non-empty sidecar blindly;
- verifies integrity and schema after checkpoint;
- does not apply migrations or change schema version;
- does not change logical memory data.

The command is idempotent. A clean workspace returns success. Repeating a
successful preparation remains safe.

An active or busy database fails closed. Backups remain available.

## Plan remains no-write

`upgrade plan` remains strict inspection:

- `writes_performed=false`;
- no checkpoint;
- no sidecar deletion;
- no migration;
- no observability write;
- no adapter change.

If a blocking sidecar remains, plan returns `ready=false`. Its fix hint points
to:

```text
aopmem upgrade prepare --all-workspaces --json
```

Do not weaken plan by ignoring a WAL or SHM file.

## Backup layers

The update keeps three independent recovery layers:

1. Installer durable full backup before `prepare`.
2. Per-workspace SQLite Online Backup before checkpoint.
3. Existing migration backups created by `upgrade apply`.

Do not delete backups during RC dogfood.

If preparation fails, no migration has started. Fix the reported cause and
rerun preparation with the staged `v0.2.0-rc3` binary.

If plan fails, do not run apply.

If apply starts, never rerun apply blindly and never restore v0.1 over a
possibly migrated workspace. Preserve the staged/recovery v0.2 binary,
backups, JSON output, and exact stopped workspace/error.

## Data preserved

Upgrade preserves:

- nodes, links, aliases, tags, sources, and events;
- statuses, confidence, and trust;
- tool contracts, generated tool files, and artifacts;
- MCP profiles;
- adapter state;
- audit history;
- global skills and templates;
- existing workspaces and all upgrade backups.

Preparation itself preserves schema version and logical rows.

## Noncanonical v0.1 binary

For a SQLite-backed v0.1 installation with an unknown binary hash:

- record the exact version and SHA-256;
- report `NONCANONICAL_V010_BINARY`;
- require durable full backup;
- do not substitute a canonical hash;
- use staged `upgrade prepare` and `upgrade plan` as workspace compatibility
  checks;
- continue only when preparation succeeds and plan reports `ready=true`.

The warning does not allow unsupported schemas, corrupt databases, unsafe
paths, active writers, or insufficient disk space.

## Verification

After publication, run in the target repository:

```text
aopmem adapter status --file AGENTS.md --json
aopmem doctor --json
aopmem verify --json
aopmem recall --json
aopmem observe status --json
aopmem observe report --json
```

Expected:

- installed version is `aopmem 0.2.0-rc3`;
- adapter is in sync;
- doctor reports healthy;
- verify reports clean;
- recall and observability succeed;
- workspace keys remain unchanged;
- no repository-local `.aopmem` is created;
- all previous workspaces remain present.

## Proof status

Repository implementation, fixture results, release assets, hashes, and test
counts are recorded in:

- `.devplan/V020_RC3_WAL_REMEDIATION_REPORT.md`;
- `.devplan/V020_RC3_GLOBAL_AUDIT_REPORT.md`;
- `.devplan/RELEASE_CANDIDATE_v0.2.0-rc3.md`.

Native Windows retry remains required after macOS-hosted proof. macOS cannot
prove native Windows PowerShell or executable runtime behavior.
