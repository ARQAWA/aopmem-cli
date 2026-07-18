# Official upgrade to AOPMem v0.2.0-rc5

`v0.2.0-rc5` supports SQLite-backed `v0.1.0-rc3`, compatible local v0.1,
`v0.2.0-rc1` through `rc4`, an active side-by-side `rc4`, and mixed workspace
schemas `001` and `003`. The target is operational schema
`004_task_protocol_and_tool_aliases` and Observability v2.

The old binary hash may be unknown. That is a warning, not the only reason to
block an upgrade. The staged platform check and plan decide compatibility.

## Required order

1. Close AOPMem processes. Do not use an administrator shell, WSL, a source
   build, Codex, manual SQLite, or manual WAL/SHM cleanup.
2. For an installed `rc1` through `rc4`, the Stage 22 installer first creates
   a durable sibling full-home backup and its deterministic
   `MANIFEST.sha256`. This happens before any RC5 binary download. The old
   binary is not expected to know the new recovery commands.

   For an already-RC5/direct recovery run, use:

   ```text
   aopmem upgrade backup --json
   ```

3. Download the RC5 asset and verify its SHA-256 from `SHA256SUMS`.
4. For the old-version installer path, the downloaded RC5 binary adopts the
   unchanged pre-download backup. Adoption validates the anchored backup,
   caller manifest hash, current-home content, and sibling association. It
   does not create another backup:

   ```text
   <verified-rc5-binary> upgrade backup --adopt <sibling-backup-dir> --manifest-sha256 <backup-manifest-sha256> --json
   ```

5. Retain that exact verified artifact:

   ```text
   <verified-rc5-binary> upgrade stage --artifact <verified-rc5-binary> --sha256 <sha256-from-SHA256SUMS> --json
   ```

6. Run `<verified-rc5-binary> platform check --json`.
7. If the report says an audit snapshot is pending, repair it with the staged
   binary. Keep every existing backup and rollback home.
8. Run `<verified-rc5-binary> upgrade prepare --all-workspaces --json`, then
   `<verified-rc5-binary> upgrade plan --all-workspaces --json`.
9. Require `ok=true`, `ready=true`, and `writes_performed=false` from plan.
10. Run core apply exactly once:

   ```text
   <verified-rc5-binary> upgrade apply --all-workspaces --json
   ```

11. Only after phase `applied`, publish the binary:

    ```text
    <verified-rc5-binary> upgrade publish --json
    ```

`backup`, `stage`, `apply`, and `publish` are separate native state-machine
steps. `apply` never creates a home backup, downloads an artifact, or publishes
the installed binary. `publish` never calls core apply.

The recovery journal uses immutable, ordered phase checkpoints outside
`AOPMEM_HOME`: `backup_complete`, `staged_verified`, `prepared`,
`apply_started`, `applied`, and `published`. Each checkpoint is accepted only
after Atomic Publish V2 confirms commit, final validation, cleanup, and
durability.

One narrow Windows exception applies only to installed-binary replacement.
When `ReplaceFileW` has committed the binary, reopened and validated its exact
digest, and cleaned the temporary file, but cannot independently confirm
directory durability, `upgrade publish` completes the durable `published`
checkpoint and returns warning
`UPGRADE_BINARY_DURABILITY_UNCONFIRMED`. Backup and journal transitions never
use this exception.

If binary publication completed but its checkpoint did not, repeat only
`upgrade publish`. Publication is idempotent and the retained staged binary is
preserved. Even when the immutable phase is already `published`, this explicit
command validates the installed digest. A missing or mismatched installed
binary is safely republished from the verified retained binary without
rewriting the checkpoint or invoking core apply.

Before each recovery operation, RC5 performs a bounded parent scan. It removes
only direct regular UUID-named journal and current-home-manifest temporary
files. Overflow, unsafe entries, symlinks, and reparse points fail closed.

If the process stopped at `apply_started`, RC5 checks every frozen workspace
for schema `004` and Observability v2. It advances only when all are proven
applied. Otherwise it fails closed. Preserve the journal and all backups. Do
not retry apply.

Stage 21 deliberately stops after binary publication. Adapter sync, post-
publish audit repair, doctor, verify, task-start smoke, observability and debug
capsule export are official installer responsibilities in Stage 22.

## Prohibitions

Do not delete the full-home backup, recovery journal, migration backups,
rollback homes, WAL, SHM, or journal files. Do not copy the binary manually.
The native command owns binary publication and reports path-private errors.

Native Windows runtime proof remains `PENDING_DOGFOOD` in this macOS-hosted
development environment.
