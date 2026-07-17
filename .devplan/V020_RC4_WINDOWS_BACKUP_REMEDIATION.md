# AOPMem v0.2.0-rc4 Windows Backup Remediation

Status: `IMPLEMENTED`; native Windows runtime: `PENDING`.

## Thin fix

The shared prepare/apply backup flow now:

1. Creates a unique temporary database in the final backup directory.
2. Runs SQLite Online Backup under the existing workspace guard.
3. Explicitly drops `Backup`.
4. Explicitly closes destination and source SQLite connections.
5. Reopens the temporary database read-only.
6. Checks schema identity, `quick_check`, and representative tables.
7. Closes validation.
8. Reopens the temporary file with the existing anchored read-write handle.
9. Calls `sync_all` on that writable handle.
10. Publishes with the existing anchored Windows no-replace helper.
11. Reopens and validates the final database read-only.
12. Records final regular-file metadata.
13. Allows migration only after all prior steps succeed.

No second Windows filesystem framework was added. The code reuses
`AnchoredDir`, including its existing `CreateFileW` directory flags,
share-mode rules, and `SetFileInformationByHandle` publication path.

## Failure contract

The external code remains `WORKSPACE_BACKUP_FAILED`. Every allowed phase has
a typed fault boundary. Failures preserve direct OS error data and partial
evidence. Created temporary/final bytes are not deleted after failure.

The pending migration marker is now created only after final backup
validation. All backup failures report `migration_started=false`.

## Run-root and installer safety

- rc4 apply roots use prefix `upgrade-0.2.0-rc4-`.
- prepare roots use prefix `upgrade-prepare-rc4-`.
- roots use create-new semantics and regenerate ids after collision.
- failed rc3 roots are neither reused nor deleted.
- installers keep one `apply` attempt.
- installer failure text includes phase, raw OS error, partial path,
  validation state, and migration state.
- binary publication still occurs only after successful apply.

## Focused proof

- Upgrade module: `37/37` tests passed.
- Backup phase fault map: all allowed phases covered.
- Access-denied injection: phase and `raw_os_error=5` retained.
- Two workspaces: stable order, both backups reopen read-only, both schemas
  migrate `001 -> 003`.
- Failed rc3 root: retained and distinct from new rc4 root.
- Installer audit: `11/11` groups passed.

Native Windows-only tests are compiled into the Windows test target where
applicable. They are not claimed as executed on macOS.
