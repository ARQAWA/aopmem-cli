# RC5 Stage 016 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_017`

Verified through: `STAGE_015`

Next cumulative audit: `STAGE_020`

Native Windows runtime: `PENDING_DOGFOOD`

P1: `0`

P2: `0`

## Result

Completed exact source audit for the Windows publish failure.

Confirmed root cause:

- `src/audit/anchored.rs:993-1051` uses
  `SetFileInformationByHandle(FileRenameInfo)`;
- `FILE_RENAME_INFO.RootDirectory` receives a held parent handle;
- the destination is handle-relative;
- source and parent remain open;
- `CreateFileW` share mode omits `FILE_SHARE_DELETE`;
- the supplied corporate VDI rejects this request with
  `ERROR_INVALID_PARAMETER / os error 87`.

This is source/static evidence. No native Windows run occurred.

## Production call sites

| Boundary | Location | Current mode |
|---|---|---|
| audit `memory.sql` | `src/audit/mod.rs:474-475` | replace |
| Git loose object | `src/audit/anchored_git.rs:339` | no-replace |
| Git ref / `HEAD` | `src/audit/anchored_git.rs:408` | replace |
| SQLite backup | `src/upgrade/backup.rs:374-375` | no-replace |
| debug capsule | `src/observability/export.rs:503-507` | no-replace |
| adapter/assets wrapper | `src/upgrade/apply.rs:2056-2079` | replace/create |

Current binary publication remains outside Rust:

- `install/v0.2/install.sh:474-498`: `mv -f`;
- `install/v0.2/install.ps1:615-648`: `[IO.File]::Replace/Move`.

Stages 021–022 must route it through the recovery binary and the same Rust
helper. Stage 016 changed no installer.

## Frozen Stage 017 boundary

Create one `src/platform_publish.rs` module with:

- `PublishMode::{ReplaceOrCreate, NoReplace}`;
- `PublishStrategy`;
- `PublishPhase`;
- `PublishOutcome`;
- `PublishFailureDetails`;
- one ownership-taking `publish_regular` operation.

Windows:

- existing destination: `ReplaceFileW`;
- absent/no-replace: `MoveFileExW(MOVEFILE_WRITE_THROUGH)`;
- never replace in `NoReplace`;
- flush exact source, close conflicting handles, publish, reopen, validate.

Unix:

- replace: anchored `renameat`;
- no-replace: anchored `linkat + unlinkat`.

Forbidden:

- separate fixes per caller;
- shell or PowerShell filesystem workaround;
- manual SQLite/WAL/SHM copy;
- admin elevation;
- second filesystem framework.

Full API, ownership, caller table, state semantics, privacy rules, complexity,
and 24 Stage 017 acceptance checks are in
`.devplan/RC5_WINDOWS_PUBLISH_ROOT_CAUSE.md`.

## State semantics preserved

- operational commit remains committed if later audit publish fails;
- `AUDIT_SNAPSHOT_PENDING` remains a structured warning;
- pending marker survives every snapshot/Git failure;
- marker clears only after snapshot validation and Git commit;
- backup keeps its validated temporary/final evidence;
- export stays no-overwrite and read-only;
- native Windows remains `PENDING_DOGFOOD`.

## Checks

Focused source/current-regression checks:

```text
audit snapshot and Git tests
PASS 26/26 audit snapshot tests; 5/5 anchored Git tests

upgrade backup tests
PASS 5/5 on macOS; Windows-only runtime tests not executed

debug capsule export tests
PASS 18/18

upgrade apply tests
PASS 16/16

mutation marker tests
PASS 16/16

cargo fmt --check
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

No product code or tests changed.

Open Stage 016 blockers: `0`.
