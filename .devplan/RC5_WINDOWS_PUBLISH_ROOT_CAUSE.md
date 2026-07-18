# AOPMem v0.2.0-rc5 Windows Publish Root-Cause Audit

Status: `DONE_LOCAL_CHECKS_PASSED`

Stage: `STAGE_016`

Native Windows runtime: `PENDING_DOGFOOD`

Source/static root-cause evidence: `PASS`

P1: `0`

P2: `0`

## 1. Scope and proof boundary

This stage audited the current file-publication implementation and every
required caller. It did not change product code, tests, installers, or user
data.

Reviewed boundaries:

- canonical audit `memory.sql`;
- audit Git loose objects;
- audit Git `HEAD` or branch ref;
- pending snapshot marker lifecycle;
- operational SQLite upgrade backups;
- debug capsule export;
- upgrade-owned adapter and asset replacement;
- current installer binary replacement.

The audit uses source evidence and the supplied field result
`ERROR_INVALID_PARAMETER / os error 87`. No native Windows run was performed.
Therefore it does not claim that Stage 017 is implemented or that Windows
runtime proof passed.

Skills applied:

- `rust-skills`: ownership, typed errors, platform API, fail-fast behavior,
  tests, and unsafe FFI review;
- `complexity-optimizer`: call-graph consolidation, bounded work, and rejection
  of per-caller fixes.

## 2. Exact root cause

The only shared regular-file publication backend is
`src/audit/anchored.rs`.

On Windows:

1. `windows_open` opens every file and directory with `CreateFileW`.
2. Share mode is exactly `FILE_SHARE_READ | FILE_SHARE_WRITE`.
3. `FILE_SHARE_DELETE` is absent.
4. writable temporary files request `DELETE` access.
5. `rename_child_regular` builds variable-length `FILE_RENAME_INFO`.
6. `ReplaceIfExists` receives the caller's replace boolean.
7. `RootDirectory` receives the held parent-directory handle.
8. `FileName` contains only the destination child name.
9. `SetFileInformationByHandle(source, FileRenameInfo, ...)` performs the
   publish while the source handle and parent handle remain open.

Source evidence:

- open modes and share flags:
  `src/audit/anchored.rs:932-983`;
- shared replace/no-replace entry points:
  `src/audit/anchored.rs:298-387`;
- Windows `FILE_RENAME_INFO` construction:
  `src/audit/anchored.rs:992-1035`;
- `SetFileInformationByHandle(FileRenameInfo)`:
  `src/audit/anchored.rs:1036-1051`;
- Windows directory flush behavior:
  `src/audit/anchored.rs:1077-1093`.

This is a handle-relative rename design. It is not merely one audit-snapshot
implementation detail. Audit Git, backup, export, and upgrade managed-file
replacement all reach it.

### Why error 87 is compatible with this implementation

The field result proves that a real corporate Windows VDI rejected the current
publish request with `ERROR_INVALID_PARAMETER` (`87`). Source proves that the
rejected request shape can only be the current `FileRenameInfo` path for these
callers.

The exact VDI filter-driver policy is not observable from this macOS source
audit. The safe conclusion is narrower:

- the request depends on `FILE_RENAME_INFO.RootDirectory`;
- it passes a relative destination name;
- it executes against an open source handle;
- all retained handles omit delete sharing;
- this request shape is rejected in the supplied real environment;
- every affected caller repeats the same request shape through one helper.

Stage 017 must remove that request shape from the Windows publication path.
It must not guess at a VDI-specific workaround or retain
`SetFileInformationByHandle(FileRenameInfo)` as the only path.

## 3. Current lifecycle

| Step | Current behavior | Result |
|---|---|---|
| parent validation | opens real directory chain; rejects reparse points | keep |
| source creation | creates same-parent regular temporary | keep |
| source write | caller writes complete bytes | keep |
| source flush | caller calls `sync_all()` | keep, centralize |
| source identity | open `File` is supplied to shared helper | keep proof |
| source close | source stays open through OS publish | change |
| destination validation | caller/helper checks optional regular child | strengthen |
| Windows publish | handle-relative `FileRenameInfo` | replace |
| directory durability | directory `FlushFileBuffers`; errors `1` and `5` ignored | preserve typed outcome |
| final reopen | backup and some callers validate; not a single helper invariant | centralize |
| caller validation | schema/hash/ZIP/Git semantics vary by caller | preserve |

The source handle cannot simply remain open when switching to path-based
`ReplaceFileW` or `MoveFileExW`, because it was opened without
`FILE_SHARE_DELETE`. Stage 017 therefore needs an ownership-taking API:
validate and flush the exact open source, then close all source/destination
validation handles before the Windows publish call.

## 4. Exact call graph

### Required publication callers

| Boundary | Source call site | Mode | Validation after publish | Stage 017 action |
|---|---|---|---|---|
| audit `memory.sql` | `src/audit/mod.rs:474-475` | replace-or-create | Git reads snapshot and commits it | migrate |
| Git loose object | `src/audit/anchored_git.rs:339` | no-replace | existing-object stream or written object identity | migrate |
| Git `HEAD` / branch ref | `src/audit/anchored_git.rs:408` | replace-or-create | Git commit path continues | migrate |
| SQLite backup | `src/upgrade/backup.rs:374-428` | no-replace | reopen SQLite, compare schema, non-empty metadata | migrate |
| debug capsule | `src/observability/export.rs:503-527` | no-replace | publication outcome; ZIP contract already written | migrate |
| adapter/assets managed file | `src/upgrade/apply.rs:2056-2079` | replace-or-create | caller-level expected bytes/health | migrate or remove with Stage 021 flow |

There are six production publication boundaries above. The first five call
the current anchored helper directly. The sixth is one local wrapper with two
branches over the same helper.

### Audit state callers

Audit mutation flow:

```text
mutate_workspace
  -> ensure_pending_snapshot_marker_locked
  -> commit operational SQLite mutation
  -> write_sql_snapshot_locked
       -> create/write/flush temporary memory.sql
       -> replace_regular
       -> anchored_git::commit_snapshot
            -> publish Git loose objects
            -> replace HEAD or branch ref
       -> clear_pending_snapshot_marker_locked
```

Evidence:

- mutation marker and post-commit warning:
  `src/mutation/mod.rs:193-290`;
- snapshot write, publish, Git commit, then marker clear:
  `src/audit/mod.rs:500-547`;
- marker create/clear:
  `src/audit/mod.rs:578-620`;
- Git object and ref publication:
  `src/audit/anchored_git.rs:253-418`.

If snapshot or Git publication fails after operational commit,
`mutate_workspace` returns `AUDIT_SNAPSHOT_PENDING` and retains the marker.
That behavior is correct and must not be weakened. Stage 019 may clear the
marker only after published snapshot validation and successful Git commit.

### Upgrade backup callers

Both `upgrade prepare` and `upgrade apply` call
`online_backup_to_path_with_faults`:

- prepare call: `src/upgrade/prepare.rs:356`;
- apply call: `src/upgrade/apply.rs:1528`;
- common implementation: `src/upgrade/backup.rs:168-469`.

The implementation closes SQLite source and destination connections before
opening a writable temporary file, flushing it, and publishing. It then
reopens the final database, compares the schema identity, and checks non-empty
regular-file metadata. These semantics must remain.

### Current staged binary and file assets

The current Rust `upgrade apply` does not replace the installed binary.
Current installers publish it separately:

- macOS shell: `install/v0.2/install.sh:474-498`, using `mv -f`;
- Windows PowerShell: `install/v0.2/install.ps1:615-648`, using
  `[IO.File]::Replace` or `[IO.File]::Move`.

This is outside the present Rust helper. Stage 016 does not patch it. The
Stage 021 recovery command and Stage 022 installers must route staged binary
publication through the same `platform_publish` implementation. Shell move,
PowerShell move, manual SQLite copy, admin elevation, and per-installer
filesystem fixes are forbidden.

The current upgrade asset and adapter wrapper `durable_replace` does use the
affected anchored backend. Stage 021 removes these writes from core apply;
until then, Stage 017 must migrate the wrapper or its callers so no active
regular-file publication retains the old Windows primitive.

Direct `create_new` backup receipts and already-unique immutable copies do not
rename an existing file and are not error-87 call sites. Tool artifact
directory publication is also a different directory-only boundary and already
uses `MoveFileExW(WRITE_THROUGH)` without replace on Windows. Neither justifies
a second regular-file publish framework.

## 5. Frozen Stage 017 module boundary

Create exactly one product module:

```text
src/platform_publish.rs
```

Required type surface:

```rust
pub(crate) enum PublishMode {
    ReplaceOrCreate,
    NoReplace,
}

pub(crate) enum PublishStrategy {
    WindowsReplaceFileW,
    WindowsMoveFileExW,
    UnixRenameAt,
    UnixLinkAtUnlinkAt,
}

pub(crate) enum PublishPhase {
    ValidateParent,
    ValidateSource,
    ValidateDestination,
    FlushSource,
    CloseHandles,
    OsPublish,
    ReopenDestination,
    ValidatePublishedIdentity,
    SyncParent,
}

pub(crate) struct PublishOutcome {
    pub strategy: PublishStrategy,
    pub destination_existed: bool,
    pub committed: bool,
    pub durability_confirmed: bool,
    pub temporary_cleanup_confirmed: bool,
}

pub(crate) struct PublishFailureDetails {
    pub code: &'static str,
    pub mode: PublishMode,
    pub strategy: PublishStrategy,
    pub phase: PublishPhase,
    pub raw_os_error: Option<i32>,
    pub io_kind: &'static str,
    pub source_exists: bool,
    pub destination_exists: bool,
    pub committed: bool,
    pub durability_confirmed: bool,
    pub temporary_cleanup_confirmed: bool,
}
```

The public crate-level operation must:

```rust
pub(crate) fn publish_regular(
    parent: &AnchoredDir,
    source: File,
    source_name: &OsStr,
    destination_name: &OsStr,
    mode: PublishMode,
) -> Result<PublishOutcome, PublishError>;
```

Exact implementation naming may change only if the same type and ownership
guarantees remain. No API may accept arbitrary source and destination parent
paths.

### Ownership rules

1. Take the source `File` by value.
2. Require source and destination as direct child components.
3. Require one already validated anchored parent.
4. Reject reparse source, destination, and parent.
5. Confirm the source path still identifies the supplied open file.
6. Flush the writable source.
7. Close the source and every destination validation handle before Windows
   publication.
8. Do not reopen the source between close and OS publication.
9. Reopen the destination through the same anchored parent.
10. Validate regular-file, no-reparse, and expected source identity/metadata.
11. Return typed committed/durability state.
12. Never include file contents, secrets, arbitrary user text, or raw absolute
    paths in generic error details.

## 6. Platform strategies

### Windows replace-or-create

- if destination exists: use `ReplaceFileW`;
- if destination is absent: use `MoveFileExW(MOVEFILE_WRITE_THROUGH)`;
- reject cross-parent input before the API call;
- close source and destination handles first;
- reopen and validate final file;
- report a destination race as a typed failure or safely retry inside the one
  helper without weakening replace semantics.

### Windows no-replace

- require destination absent at validation;
- use `MoveFileExW(MOVEFILE_WRITE_THROUGH)`;
- never pass `MOVEFILE_REPLACE_EXISTING`;
- if destination appears, return typed already-exists;
- reopen and validate only the committed destination.

### Unix

- replace-or-create: retain anchored same-parent `renameat`;
- no-replace: retain anchored `linkat` then `unlinkat`;
- preserve source identity and directory sync checks;
- expose the same typed outcome as Windows.

No fallback may use:

- `SetFileInformationByHandle(FileRenameInfo)` as the only Windows path;
- shell `move`, `mv`, or `copy`;
- PowerShell filesystem workaround;
- manual SQLite/WAL/SHM copy;
- admin elevation;
- a second filesystem framework.

## 7. Caller migration contract

| Caller | Helper mode | Caller-owned validation |
|---|---|---|
| `memory.sql` | `ReplaceOrCreate` | snapshot bytes then Git commit |
| Git object | `NoReplace` | object hash/stream; existing object idempotency |
| Git ref / `HEAD` | `ReplaceOrCreate` | compare-and-swap lock state |
| operational DB backup | `NoReplace` | SQLite integrity/schema and non-empty metadata |
| debug capsule | `NoReplace` | deterministic 12-entry ZIP and no overwrite |
| managed adapter/assets | `ReplaceOrCreate` | exact expected bytes and rollback |
| staged rc5 binary | `ReplaceOrCreate` | SHA-256 and `aopmem --version` in recovery flow |

Caller-specific validation stays outside the helper. Filesystem safety,
publication strategy, source ownership, final reopen, and structured OS
failure stay inside it.

## 8. Existing tests and Stage 017 acceptance

Existing focused proof to preserve:

- audit writer/publish/Git/marker tests in `src/audit/mod.rs`;
- Git object/ref swap tests in `src/audit/anchored_git.rs`;
- backup phase, schema, handle-close, and publish tests in
  `src/upgrade/backup.rs`;
- export determinism, privacy, no-write, no-overwrite, and anchored swap tests
  in `src/observability/export.rs`;
- upgrade backup, rollback, pending marker, and post-commit warning tests in
  `src/upgrade/apply.rs`;
- mutation marker lifecycle tests in `src/mutation/mod.rs`.

The source-contract test
`windows_anchor_source_has_no_delete_share_and_uses_handle_relative_rename`
currently asserts the obsolete implementation. Stage 017 must replace it with
positive checks for the new API and absence of the old sole-path primitive.

Required Stage 017 acceptance tests:

1. replace existing regular file;
2. create when destination absent under `ReplaceOrCreate`;
3. no-replace success;
4. no-replace existing destination failure with unchanged bytes;
5. same-parent enforcement;
6. source reparse rejection;
7. destination reparse rejection;
8. parent reparse/identity-swap rejection;
9. source identity swap before close fails closed;
10. all conflicting source/destination handles close before Windows API;
11. source flush failure blocks publication;
12. `ERROR_INVALID_PARAMETER` fault injection returns phase, strategy, mode,
    `raw_os_error=87`, and `committed=false`;
13. non-ASCII child names;
14. long normal paths;
15. reopen/identity validation failure is typed as committed but unvalidated;
16. replace race and no-replace race preserve mode semantics;
17. audit snapshot still retains marker on publish or Git failure;
18. Git object no-replace idempotency remains;
19. backup closes SQLite handles, publishes, reopens, and validates schema;
20. export never overwrites an existing destination;
21. adapter/assets rollback stays byte-exact if still present;
22. source scan shows backup, snapshot, Git, export, managed files, and future
    binary command use only `platform_publish`;
23. Windows cross-build compiles the FFI path without running it;
24. native Windows runtime remains separately `PENDING_DOGFOOD`.

## 9. Complexity and privacy

The current and proposed publication operation is `O(1)` filesystem work per
file, plus caller-owned streaming/hash/schema validation. Consolidation removes
six copies of platform policy without adding scans, retries over unbounded
inputs, N+1 I/O, or file-content buffering.

Required bounds:

- direct child names only;
- one parent;
- one source and at most one destination;
- bounded retry count for destination race, if used;
- no directory traversal in the publish helper;
- no recursive cleanup.

Generic publish diagnostics must contain only stable enums, booleans, I/O kind,
and raw OS code. Existing caller reports may expose already-approved display
paths, but the shared error must not add new raw-path or secret surfaces.

## 10. Result

Root cause: confirmed by source and supplied field evidence.

Single migration boundary: frozen as `src/platform_publish.rs`.

Stage 017 implementation: pending.

Native Windows runtime: `PENDING_DOGFOOD`.

Open Stage 016 blockers: `0`.

P1: `0`.

P2: `0`.
