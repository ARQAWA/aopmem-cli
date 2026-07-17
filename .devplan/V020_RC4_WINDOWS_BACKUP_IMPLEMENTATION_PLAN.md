# AOPMem v0.2.0-rc4 Windows Backup Implementation Plan

Status: `IMPLEMENTED_AND_VERIFIED`

Execution authorization note:

- the original planning stop rule remains historical evidence;
- the later explicit operator request authorizes commit, push, tag, and
  GitHub prerelease for this completed rc4 change;
- real workspace installation and backup deletion remain forbidden.

Purpose: handoff plan for implementing the native Windows
`WORKSPACE_BACKUP_FAILED: Access is denied. (os error 5)` remediation.

Repository:

```text
/Users/arkadijcukavin/PycharmProjects/aopmem-cli
```

Target release:

```text
v0.2.0-rc4
```

## 1. Outcome

Make native Windows upgrade work without admin rights:

```text
prepare
-> plan ready=true
-> apply creates valid per-workspace backups
-> migrations 001->003
-> binary publish
-> health checks
```

Preserve:

- memory model;
- schema semantics;
- migration contents;
- recall;
- observability;
- UI;
- approval policy;
- installer product scope;
- plan no-write contract;
- no-cross-workspace-rollback contract;
- one apply attempt per installer run.

Never:

- continue update with current rc3 binary;
- add manual SQLite copy workaround;
- delete old or failed-run backups;
- weaken backup failure to warning;
- add automatic apply retry;
- add a second Windows filesystem framework;
- install into a real workspace;
- push, tag, or create GitHub Release.

## 2. Real Failure

Environment:

- Windows 11 VDI;
- native PowerShell 5.1;
- no admin;
- repository: `C:\SRC\P-SIT-Warranty`;
- AOPMem home: `C:\Users\chukaa\.aopmem`.

Installed binary:

```text
aopmem 0.1.0
SHA-256 429225C28F36958092D2FBCD1563A37C31AB2345E499EA223635BDB5DC661E5A
```

Staged rc3 binary:

```text
aopmem 0.2.0-rc3
SHA-256 ED59BE73D99EFD2C1A4FE99E50B85E8B6CE8E8A73B7FF0C96B5327E1C2D39477
```

Workspaces:

```text
p-sit-cat-rental-8ef3bf83
p-sit-warranty-5708363a
```

Prepare:

- exit `0`;
- valid JSON;
- `ok=true`;
- `success=true`;
- both workspaces `already_clean`;
- `ready_for_plan=true`;
- `writes_performed=false`.

Plan:

- exit `0`;
- valid JSON;
- `ok=true`;
- `ready=true`;
- `writes_performed=false`;
- both workspaces schema `001`;
- target schema `003`;
- disk sufficient;
- no sidecar, corruption, or schema blocker.

Apply:

```text
upgrade apply --all-workspaces --json --approved "+++"
```

- attempts: `1`;
- exit: `1`;
- valid JSON;
- `ok=false`;
- `success=false`;
- `binary_replaced=false`;
- code: `WORKSPACE_BACKUP_FAILED`;
- message: `Access is denied. (os error 5)`;
- stopped workspace: `p-sit-cat-rental-8ef3bf83`;
- migrations: none;
- second workspace: not started;
- installed binary: unchanged v0.1.

Failed rc3 run root:

```text
C:\Users\chukaa\.aopmem\backups\upgrade-0.2.0-rc3-2ea0b5a589d8f2422bc2e4cbb60a3495
```

Observed partial file:

```text
size = 184,320 bytes
```

Durable external full backup:

```text
C:\Users\chukaa\AppData\Local\AOPMemBackups\pre-v020-rc3-20260717-045820
```

Preserve both roots.

## 3. Code-Level Root Cause

Strongest exact failing operation:

```text
src/upgrade/backup.rs:54
```

Current code:

```rust
drop(destination);
File::open(destination_path)?.sync_all()?;
destination_root.sync()
```

Facts:

1. `File::open` opens a read-only file handle.
2. Rust Windows `File::sync_all` uses `FlushFileBuffers`.
3. Win32 `FlushFileBuffers` requires `GENERIC_WRITE`.
4. A read-only handle can return `ERROR_ACCESS_DENIED = 5`.
5. SQLite Online Backup and `quick_check` run before this operation.
6. This explains a populated `184,320`-byte file followed by failure.
7. Migration starts only after the helper returns success.

Primary API proof:

- Rust `File::open`:
  <https://doc.rust-lang.org/stable/std/fs/struct.File.html#method.open>
- Win32 `FlushFileBuffers`:
  <https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers>

The next directory sync is not the likely external error source:

```text
src/audit/anchored.rs:1043-1058
```

The existing Windows helper treats directory `raw_os_error=1|5` as
unsupported directory flush after the file itself was flushed.

Current native report has no `backup_phase`. Therefore:

- record this as the code-level root cause;
- add phase diagnostics before claiming new native runtime confirmation;
- do not claim antivirus or sharing conflict without new evidence.

## 4. Current Call Chain

```text
src/cli/mod.rs
  upgrade apply dispatch
    -> src/upgrade/apply.rs
       apply_all_workspaces
         -> create_backup_run
         -> stable workspace loop
         -> backup_and_migrate_workspace
            -> validate workspace paths
            -> acquire mutation and snapshot locks
            -> open writable operational DB
            -> BEGIN IMMEDIATE
            -> inspect schema
            -> open read-only backup source
            -> src/upgrade/backup.rs
               online_backup_to_path
               -> open AnchoredDir
               -> create file under final name
               -> open destination SQLite connection
               -> SQLite Online Backup
               -> quick_check while destination is open
               -> drop destination connection
               -> File::open(final).sync_all()
               -> directory sync
            -> mark backup complete
            -> apply migrations
```

Important current gaps:

- writes directly under final name;
- no unique temporary database;
- no temporary-to-final atomic publish;
- no final read-only reopen;
- no final schema/metadata validation;
- backup source closes only after helper returns;
- `Backup` closes by RAII, not explicit phase boundary;
- errors collapse into generic `io::Error`;
- all backup failures share one context-free message.

## 5. Existing Reusable Windows Filesystem Code

Reuse:

```text
src/audit/anchored.rs
```

Relevant APIs:

- `AnchoredDir::open_workspace`;
- `AnchoredDir::create_new_regular_os`;
- `AnchoredDir::publish_regular_no_replace_committed_os`;
- existing handle-relative Windows rename;
- existing `CreateFileW` directory open with
  `FILE_FLAG_BACKUP_SEMANTICS`;
- existing Windows directory durability behavior.

Windows publish implementation:

```text
SetFileInformationByHandle(FileRenameInfo)
```

Do not add:

- a new `MoveFileExW` path;
- generic path-based rename for workspace backups;
- `File::open(directory).sync_all()` on Windows;
- Unix parent-directory fsync on Windows.

Minimal expected audit change:

- expose an existing-file read-write/publish handle opener if required;
- keep Windows syscalls and share-mode logic centralized in
  `src/audit/anchored.rs`.

## 6. Implementation Stages

### Stage A: Proof First and Diagnostics

Create:

```text
.devplan/V020_RC4_WINDOWS_BACKUP_ROOT_CAUSE.md
```

Include:

- real Windows failure;
- exact call chain;
- code-level failing operation;
- why `184,320` bytes do not mean accepted backup;
- distinction between code proof and pending native phase confirmation;
- explicit rejection of antivirus speculation.

Add a typed phase model:

```rust
enum BackupPhase {
    CreateBackupRoot,
    CreateTemporaryDatabase,
    OpenSourceDatabase,
    OpenDestinationDatabase,
    SqliteOnlineBackup,
    CloseSqliteHandles,
    ValidateTemporaryDatabase,
    FlushTemporaryFile,
    PublishBackup,
    ValidatePublishedDatabase,
    FinalizeBackupMetadata,
}
```

Serialize with `snake_case`.

Add typed backup error. Required structured fields:

```text
workspace_key
backup_phase
source_path
temporary_path
final_path
raw_os_error
io_kind
partial_file_exists
partial_file_size
partial_file_validated
migration_started
fix_hint
```

Rules:

- preserve external code `WORKSPACE_BACKUP_FAILED`;
- include `backup_phase` in message and structured details;
- preserve direct `io::Error::raw_os_error`;
- use stable normalized `io_kind`;
- never include SQL contents, credentials, or secret values;
- set `migration_started=false` for every backup failure;
- describe retained partial evidence;
- never recommend copying SQLite or deleting backups.

Add backup-phase fault injection before changing semantics.

DoR:

- current behavior mapped;
- JSON envelope location mapped;
- apply and prepare callers mapped.

DoD:

- every allowed phase serializes exactly;
- raw error `5` survives to CLI JSON;
- context-free `Access is denied` no longer possible;
- no migration or backup flow change yet.

### Stage B: Thin SQLite Backup Flow

Replace direct-final flow with:

```text
1. Create unique temporary name in final backup directory.
2. Open source under existing workspace write guard.
3. Create destination SQLite DB at temporary path.
4. Run SQLite Online Backup.
5. Reach StepResult::Done.
6. Explicitly drop Backup object.
7. Explicitly close destination Connection.
8. Explicitly close read-only source Connection.
9. Reopen temporary DB read-only.
10. Validate temporary DB.
11. Close temporary validation Connection.
12. Open temporary file with writable anchored handle.
13. Flush temporary file.
14. Publish no-replace through AnchoredDir.
15. Reopen final DB read-only.
16. Validate final DB.
17. Record path, size, validation facts.
18. Return backup success.
```

Prefer consuming the read-only source `Connection` in the helper. This makes
source close order explicit and keeps publish after every SQLite handle.

Temporary validation:

- `PRAGMA quick_check(1) = ok`;
- expected `schema_migrations` table;
- supported applied migration identity;
- representative required tables;
- no new sidecar;
- expected regular-file metadata;
- source logical digest only if an existing contract already requires it.

Final validation:

- reopen final path read-only;
- repeat quick/schema/table validation;
- verify final metadata and size;
- ensure temporary path is no longer visible after successful publish.

Durability:

- flush temporary file through a handle with write access;
- publish through existing anchored Windows-safe helper;
- require confirmed file durability contract;
- do not convert real publish/final-validation failure to warning.

Failure evidence:

- retain temporary file when bytes exist and success is not established;
- retain final file when publish completed but final validation failed;
- do not delete previous run roots;
- report evidence path, exists, size, validation state;
- do not mark database backup completed.

DoR:

- Stage A tests pass;
- exact helper contract defined;
- existing AnchoredDir publish contract reviewed.

DoD:

- no SQLite handle open during publish;
- no read-only file flush;
- temporary and final validation pass;
- backup completion returned only after final validation.

### Stage C: Apply and Prepare Integration

Apply:

```text
src/upgrade/apply.rs
```

Required order:

```text
path guards
-> workspace locks
-> BEGIN IMMEDIATE
-> schema inspect
-> complete and validate final backup
-> create pending migration marker if needed
-> migrations
-> schema-after validation
-> commit
```

Move pending migration marker creation after successful final backup
validation. It remains before migration.

Preserve:

- one workspace transaction at a time;
- stable workspace order;
- no rollback of an earlier committed workspace;
- no migration before valid final backup;
- exact stopped workspace;
- remaining workspaces `not_started`.

Prepare:

```text
src/upgrade/prepare.rs
```

Use the same backup helper. Preserve:

- clean workspace remains no-write and creates no backup;
- backup precedes checkpoint;
- busy/incomplete checkpoint fails closed;
- no schema migration;
- WAL/SHM cleanup only after connection close;
- only verified empty direct-child sidecars removed;
- committed WAL data preserved.

Unique run roots:

- change prefix to `upgrade-0.2.0-rc4-`;
- create root with create-new semantics;
- regenerate random ID on collision;
- never reuse an existing directory;
- never inspect rc3 failed root as migration backup;
- never delete rc3 failed root.

DoD:

- backup path appears in apply report before migration begins;
- every backup failure says `migration_started=false`;
- both apply and prepare share one backup algorithm;
- plan remains byte-for-byte no-write in behavior.

### Stage D: Focused Tests

#### Backup module

1. Online Backup creates valid final DB.
2. Backup object closes before destination close.
3. Destination connection closes before publish.
4. Source read connection closes before publish.
5. Temporary DB opens read-only and validates.
6. Final DB opens read-only after publish.
7. Source logical data remains unchanged.
8. Temporary create failure reports
   `create_temporary_database`.
9. Source open failure reports `open_source_database`.
10. Destination open failure reports `open_destination_database`.
11. Online Backup failure reports `sqlite_online_backup`.
12. Handle close failure reports `close_sqlite_handles`.
13. Temporary validation failure reports
    `validate_temporary_database`.
14. File flush failure reports `flush_temporary_file`.
15. Publish failure reports `publish_backup`.
16. Final validation failure reports
    `validate_published_database`.
17. Metadata failure reports `finalize_backup_metadata`.
18. Access denied injection preserves `raw_os_error=5`.
19. Partial existence, size, and validation facts are exact.
20. Successful publish leaves no temporary path.

#### Apply module

1. Migration does not begin before final backup validation.
2. First and second workspaces both backup and migrate `001->003`.
3. Workspace order remains stable.
4. Failure in first workspace leaves second `not_started`.
5. Failure in second workspace keeps first committed.
6. Source unchanged for every pre-migration failure.
7. Existing rc3 root is not reused.
8. Existing rc3 root is not deleted.
9. Rerun with new run ID remains safe.
10. Apply attempt contract remains one attempt per run.
11. Full v0.1 payload remains exact:
    nodes, links, aliases, tags, sources, events, tool contracts/files,
    MCP profiles, artifacts, audit state.

#### Prepare and plan

1. `prepare -> plan -> apply` fixture passes.
2. Clean prepare remains `writes_performed=false`.
3. Zero-byte WAL remediation passes.
4. Committed WAL remediation passes.
5. Plan remains `writes_performed=false`.
6. Noncanonical v0.1 fixture remains supported.

#### Windows-only

Use `#[cfg(windows)]` where native behavior is required:

1. Writable flush handle succeeds without admin.
2. Retained SQLite destination handle blocks publish regression test.
3. Production flow proves destination handle closed before publish.
4. No generic `File::open(directory).sync_all()` path is used.
5. Anchored directory open uses existing Windows flags.
6. Final DB read-only reopen succeeds.

Do not claim these tests passed from macOS execution.

### Stage E: Installer Contract

Preserve order:

```text
process gate
-> durable full backup
-> download and verify
-> prepare
-> plan
-> apply once
-> atomic binary publish
-> health checks
```

Update:

```text
install/v0.2/install_prompt.md
install/v0.2/install.ps1
install/v0.2/install.sh
scripts/audit_v020_installers.sh
```

At `WORKSPACE_BACKUP_FAILED`:

- print full JSON report;
- include `workspace_key`;
- include `backup_phase`;
- include raw OS error and evidence facts;
- preserve durable and upgrade backups;
- leave installed binary unchanged;
- stop immediately;
- do not call apply again;
- do not publish binary.

Installer audit must count exact apply attempts.

### Stage F: Version, Docs, Reports, Assets

Set version:

```text
0.2.0-rc4
```

Update at minimum:

```text
Cargo.toml
Cargo.lock
src/upgrade/apply.rs
src/cli/mod.rs version assertions
install/v0.2/install_prompt.md
install/v0.2/install.ps1
install/v0.2/install.sh
docs/WINDOWS_NATIVE_UPDATE.md
docs/UPGRADE_FROM_V010_RC3.md
.devplan/V020_FINAL_DECISION_LOG.md
.devplan/V020_PROOF_LOG.md
scripts/audit_v020_installers.sh
```

Create:

```text
.devplan/V020_RC4_WINDOWS_BACKUP_ROOT_CAUSE.md
.devplan/V020_RC4_WINDOWS_BACKUP_REMEDIATION.md
.devplan/V020_RC4_GLOBAL_AUDIT_REPORT.md
.devplan/RELEASE_CANDIDATE_v0.2.0-rc4.md
```

Build:

```text
dist/aopmem-darwin-arm64
dist/aopmem-windows-x86_64.exe
dist/SHA256SUMS
```

RC report must include:

- exact Windows failure;
- exact code-level root cause;
- native phase confirmation status;
- why `184,320` bytes were insufficient;
- fixed handle-close order;
- publish proof;
- final validation proof;
- no-migration-before-backup proof;
- two-workspace proof;
- macOS isolated proof;
- Windows PE/hash proof;
- native Windows retry `PENDING`;
- open P1/P2 counts.

## 7. Real macOS Proof

Use isolated temporary home and install directory only.

Create:

- v0.1 cat-rental analogue;
- v0.1 warranty analogue;
- two-workspace upgrade;
- WAL clean;
- zero-byte WAL;
- committed WAL;
- noncanonical v0.1 binary warning.

Run:

```text
prepare
-> plan
-> apply
-> publish in isolated temp install
-> doctor
-> verify
-> recall
-> observe status
-> observe report
-> export
```

Prove:

- both final backup DBs valid;
- both schemas migrate `001->003`;
- old logical data exact;
- tools and artifacts exact;
- no onboarding;
- no repository-local `.aopmem`;
- no apply retry;
- all old backup roots retained.

Do not label this native Windows proof.

## 8. Verification Order

Use narrow checks first:

```bash
cargo fmt --check
cargo test --locked upgrade::backup::tests
cargo test --locked upgrade::apply::tests
cargo test --locked upgrade::prepare::tests
cargo test --locked upgrade::
```

Then full gates:

```bash
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
cargo test --tests --locked
scripts/dev_verify.sh
scripts/audit_v020_installers.sh
git diff --check
```

Installer syntax:

```bash
sh -n install/v0.2/install.sh scripts/audit_v020_installers.sh
```

Assets:

```bash
scripts/build_macos_arm.sh
scripts/build_windows_x64_from_macos.sh
file dist/aopmem-darwin-arm64 dist/aopmem-windows-x86_64.exe
(cd dist && shasum -a 256 -c SHA256SUMS)
```

Windows cross-build:

```bash
cargo xwin build --locked --release --target x86_64-pc-windows-msvc
```

Prove:

- Windows artifact is `PE32+` x86-64;
- no dynamic MSVC/UCRT import;
- `SHA256SUMS` verifies;
- forbidden drift scan clean;
- no new dependency without explicit need;
- no generated Python cache;
- no unrelated UI, memory, schema, or observability drift.

## 9. Complexity Review

Current upgrade complexity:

```text
O(W log W + total SQLite pages)
```

Expected after fix:

```text
O(W log W + total SQLite pages)
```

Asymptotic complexity unchanged.

Added constant-factor I/O:

- temporary validation scan;
- final validation scan.

Acceptable because:

- upgrade is a cold safety-critical path;
- final validation is an explicit durability requirement;
- no extra retry, cache, background worker, or parallel backup.

Complexity scanner found only pre-existing benchmark/UI leads outside rc4
scope. Do not modify them.

## 10. Risk Controls

### P1 risks

- migration begins before final backup validation;
- backup reported successful before final reopen;
- old backup root reused or deleted;
- installer retries apply;
- Windows publish uses open SQLite handle;
- failed backup converted to warning.

### P2 risks

- `raw_os_error` lost through error conversion;
- `backup_phase` differs between report locations;
- temporary evidence removed after failure;
- prepare and apply diverge into two backup implementations;
- plan starts writing;
- stable workspace order changes;
- rc3 version remains in executable or installer tests.

Close all P1 and P2 before asset approval.

## 11. Expected File Scope

Core:

```text
src/upgrade/backup.rs
src/upgrade/apply.rs
src/upgrade/prepare.rs
src/upgrade/mod.rs
src/audit/anchored.rs
src/cli/mod.rs
```

Possible minimal path helper change:

```text
src/storage/mod.rs
```

Release:

```text
Cargo.toml
Cargo.lock
install/v0.2/install_prompt.md
install/v0.2/install.ps1
install/v0.2/install.sh
scripts/audit_v020_installers.sh
docs/WINDOWS_NATIVE_UPDATE.md
docs/UPGRADE_FROM_V010_RC3.md
.devplan/V020_FINAL_DECISION_LOG.md
.devplan/V020_PROOF_LOG.md
.devplan/V020_RC4_WINDOWS_BACKUP_ROOT_CAUSE.md
.devplan/V020_RC4_WINDOWS_BACKUP_REMEDIATION.md
.devplan/V020_RC4_GLOBAL_AUDIT_REPORT.md
.devplan/RELEASE_CANDIDATE_v0.2.0-rc4.md
dist/aopmem-darwin-arm64
dist/aopmem-windows-x86_64.exe
dist/SHA256SUMS
```

Treat changes outside this set as drift requiring justification.

## 12. Definition of Done

Ready only when:

1. Exact code-level failing operation recorded.
2. New diagnostics identify exact native phase.
3. Generic context-free Access Denied removed.
4. SQLite handles close before publish.
5. Windows avoids Unix directory fsync.
6. Final backup reopens and validates.
7. Migration begins only after valid final backup.
8. Failed rc3 root remains untouched.
9. Two-workspace success fixture passes.
10. Prepare and plan contracts remain intact.
11. Installer performs one apply attempt.
12. Full tests and checks pass.
13. Windows PE/hash proof passes.
14. Open P1=`0`.
15. Open P2=`0`.
16. Native Windows dogfood status reported honestly.

## 13. Final Response Format

```text
Result:
Exact root cause:
Failing operation:
Files changed:
Tests:
Windows-specific proof:
Two-workspace proof:
Migration-order proof:
Backup validation proof:
macOS proof:
Windows asset/hash:
P1/P2:
Ready for Windows dogfood:
Next:
```

## 14. Stop Conditions

Stop without:

- push;
- tag;
- GitHub Release;
- real workspace installation;
- old backup deletion;
- rc3 apply continuation;
- automatic retry.

Leave native Windows runtime result `PENDING` until rc4 runs on Windows 11
PowerShell 5.1 without admin rights.
