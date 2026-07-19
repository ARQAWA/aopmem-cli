# RC6 Windows Publish Root Cause

Status: `VERIFIED`

## Exact call chain

```text
platform_check::create_source
  → AnchoredDir::create_new_regular_os
  → windows_open(CreateNew, regular)
  → write_all + sync_all
  → platform_publish::publish_regular_inner
  → AnchoredDir::regular_child_matches_open_file
  → AnchoredDir::open_regular_os
  → windows_open(ReadOnly, regular)
  → CreateFileW returns ERROR_SHARING_VIOLATION (32)
  → PLATFORM_PUBLISH_FAILED / validate_source
  → source unwinds and CleanupGuard removes private files/root
```

`os_publish`, `MoveFileExW`, and `ReplaceFileW` are not reached.

## Windows handles at the failure boundary

| Handle | Desired access | Share mode | Creation / flags | Lifetime |
|---|---|---|---|---|
| Anchored ancestors and parent directory | `GENERIC_READ | FILE_READ_ATTRIBUTES` | `FILE_SHARE_READ | FILE_SHARE_WRITE` | `OPEN_EXISTING`; `FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS` | `AnchoredDir` through publish/cleanup |
| Source writer | `GENERIC_READ | GENERIC_WRITE | DELETE` | `FILE_SHARE_READ | FILE_SHARE_WRITE` | `CREATE_NEW`; `FILE_FLAG_OPEN_REPARSE_POINT` | create/write/flush through the original source validation |
| Source validation reopen | `GENERIC_READ | FILE_READ_ATTRIBUTES` | `FILE_SHARE_READ | FILE_SHARE_WRITE` | `OPEN_EXISTING`; `FILE_FLAG_OPEN_REPARSE_POINT` | intended: only `same_file`; actual: `CreateFileW` fails |
| Optional destination validation | readonly when present | `FILE_SHARE_READ | FILE_SHARE_WRITE` | `OPEN_EXISTING`; `FILE_FLAG_OPEN_REPARSE_POINT` | dropped before publish |
| Final destination reopen | readonly | `FILE_SHARE_READ | FILE_SHARE_WRITE` | `OPEN_EXISTING`; `FILE_FLAG_OPEN_REPARSE_POINT` | short scope after publish |
| Cleanup regular file | `DELETE | FILE_READ_ATTRIBUTES` | `FILE_SHARE_READ | FILE_SHARE_WRITE` | `OPEN_EXISTING`; `FILE_FLAG_OPEN_REPARSE_POINT` | after source unwind/drop |
| Cleanup private directory | `DELETE | FILE_READ_ATTRIBUTES` | `FILE_SHARE_READ | FILE_SHARE_WRITE` | `OPEN_EXISTING`; `FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS` | after `drop(anchor)` |

No raw numeric OS handle is recorded.

## Exact conflict

The source writer remains live during `validate_source`. It has requested
`DELETE`. The new validation open shares only read/write, not delete. Windows
requires the new handle's share mode to permit every desired access held by
the existing handle. The missing `FILE_SHARE_DELETE` therefore causes
`ERROR_SHARING_VIOLATION (32)` before publication.

This is a proven handle/share mismatch. There is no evidence that antivirus
participated.

## Why prior proof missed it

macOS/Unix does not enforce this Windows `CreateFileW` share-mode contract.
RC5 local tests and the macOS cross-build proved compilation and Unix runtime
behavior, not native Windows runtime. Existing source inspection explicitly
preserved non-delete-sharing anchored handles, but did not exercise this live
writer/reopen sequence on Windows.

## Commit and data state

The error occurs during `validate_source`, before `os_publish`; therefore
`committed=false`, destination is absent, and the source is still in the
private platform-check directory. Unwind drops the writer; bounded cleanup
then removes its known private files and root. No workspace, database,
upgrade, audit repair, migration, or binary publication starts.

## Smallest safe remediation

Keep directory handles non-delete-sharing, preserving their identity and
reparse protections. In the one unified publisher:

1. flush and capture source identity from the owned writer;
2. drop the writer;
3. reopen and validate the source against the captured identity;
4. drop that validation handle;
5. publish and reopen the final destination.

This directly satisfies the close-before-validation contract, preserves the
existing stricter `FILE_SHARE_READ | FILE_SHARE_WRITE` boundary, changes no
Windows syscall, and avoids a second filesystem implementation.

Required diagnostics for the failing validation remain path- and secret-free:
`handle_role=source_validation`, `desired_access`, `share_mode`,
`creation_disposition`, `flags`, `handle_expected_closed`, strategy,
operation, phase, raw OS error, I/O kind, existence, commit, and cleanup.
