# Official upgrade to AOPMem v0.2.0-rc6

> **Superseded:** RC6 is historical. For current installations and updates,
> use [AOPMem v0.2.0-rc7](UPGRADE_TO_RC7.md). RC7 fixes the native Windows
> PowerShell 5.1 proxy transport and source classification. The RC6 content
> below is preserved for audit and recovery of the immutable RC6 release.

`v0.2.0-rc6` supports SQLite-backed `v0.1.0-rc3`, compatible local v0.1,
`v0.2.0-rc1` through `rc5`, the active side-by-side `rc4` layout, and mixed
workspace schemas `001`, `003`, and `004`. The target remains
`004_task_protocol_and_tool_aliases`; RC6 adds no migration `005`.

An unknown old binary SHA-256 is a warning, not the only compatibility gate.
The staged platform check, preparation, and read-only plan decide whether an
update may continue.

## Audited RC6 asset digests

These are the exact pre-publication RC6 release assets. Verify the selected
asset against the downloaded `SHA256SUMS` manifest before execution.

| Asset | SHA-256 |
| --- | --- |
| `aopmem-darwin-arm64` | `b933d921ae6ec68ce7e0f118de27fd7eabe9d1c42d715a0a6df8f2ec731cb949` |
| `aopmem-windows-x86_64.exe` | `8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7` |
| `SHA256SUMS` | `e4e7142e30cb6ef4cac2c7402b8ace8b87fc37df87add59ccb8d79d15d0f3dba` |

## Required order

1. Close all AOPMem UI and CLI processes. Do not use an administrator shell,
   WSL, a source build, Codex, manual SQLite, or manual WAL/SHM cleanup.
2. The official installer first creates a durable sibling full-home backup and
   deterministic `MANIFEST.sha256`. It does this before downloading RC6.
   Keep the old binary and every backup.
3. Download the RC6 asset and verify its SHA-256 and exact version from
   `SHA256SUMS`.
4. The verified RC6 binary adopts the unchanged pre-download backup:

   ```text
   <verified-rc6-binary> upgrade backup --adopt <sibling-backup-dir> --manifest-sha256 <backup-manifest-sha256> --json
   ```

5. Retain that exact verified artifact:

   ```text
   <verified-rc6-binary> upgrade stage --artifact <verified-rc6-binary> --sha256 <sha256-from-SHA256SUMS> --json
   ```

6. Run the staged platform check first:

   ```text
   <verified-rc6-binary> platform check --json
   ```

   If it fails, stop. Do not run audit repair, prepare, plan, apply, or binary
   publication. User data remains unchanged.
7. Run staged `audit repair --all-workspaces --json` when a snapshot is
   pending. An `already_clean` result is valid.
8. Run staged preparation, then immediately run the staged plan:

   ```text
   <verified-rc6-binary> upgrade prepare --all-workspaces --json
   <verified-rc6-binary> upgrade plan --all-workspaces --json
   ```

   Require `ok=true`, `ready=true`, and `writes_performed=false` from plan.
   Do not run another AOPMem database-read command between prepare and plan.
9. Run core apply exactly once:

   ```text
   <verified-rc6-binary> upgrade apply --all-workspaces --json --approved "+++"
   ```

   Do not retry apply automatically.
10. Only after an `applied` result, publish the binary with the native command:

    ```text
    <verified-rc6-binary> upgrade publish --json
    ```

    Do not copy, move, or replace the installed binary manually.
11. Sync the one explicitly selected adapter. Then run post-publish audit
    repair, `doctor`, `verify`, task-protocol smoke, observability, and debug
    capsule export.

The installer owns the sequence: process gate, full backup, verification,
platform check, audit repair, prepare, plan, apply once, binary publication,
adapter sync, post-publish audit repair, health checks, and export.

## RC6 publish boundary

RC6 fixes the Windows `ERROR_SHARING_VIOLATION` (`32`) seen during staged
platform check. The temporary writer now closes before source validation. The
source and optional destination validation readers close before publication.
No-replace and replace-existing paths still use the shared guarded publisher,
which reopens and validates the final destination after commit.

## Recovery and failure

RC6 creates versioned recovery journals, staged binaries, and full-home
backups with `v0.2.0-rc6` names. They remain outside or beside
`AOPMEM_HOME`, as appropriate, and must be retained.

Before apply starts, the installed binary remains unchanged. After apply
starts, never restore or republish the old binary over migrated data. Keep the
journal, staged binary, full-home backup, workspace backups, JSON output, and
the exact stopped workspace/error. Continue only with a separately reviewed
native recovery operation.

`upgrade publish` may be repeated only after an `applied` checkpoint when its
retained staged binary and digest are valid. It never reruns core apply.

## Prohibitions

Do not delete full-home backups, recovery journals, migration backups,
rollback homes, WAL, SHM, or journal files. Do not require administrator
rights, WSL, a source build, or onboarding questions during update.

Native Windows runtime acceptance remains required. See
`../.devplan/RC6_WINDOWS_ACCEPTANCE_PROMPT.md` after the RC6 prerelease is
available.
