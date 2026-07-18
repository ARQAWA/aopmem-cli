# AOPMem v0.2.0-rc5 Windows Publish Report

Status: `STAGE_020_DONE_LOCAL_CHECKS_PASSED`

Native Windows runtime: `PENDING_DOGFOOD`

## Stage progress

| Stage | Result | Evidence |
|---:|---|---|
| 16 | `DONE_LOCAL_CHECKS_PASSED` | `RC5_WINDOWS_PUBLISH_ROOT_CAUSE.md` |
| 17 | `DONE_LOCAL_CHECKS_PASSED` | unified Atomic Publish V2; 716/716 tests |
| 18 | `DONE_LOCAL_CHECKS_PASSED` | private-temp platform self-check; 720/720 tests |
| 19 | `DONE_LOCAL_CHECKS_PASSED` | official current/all audit repair; read-only DB; marker-last |
| 20 | `DONE_LOCAL_CHECKS_PASSED` | exact 12-entry capsule; shared long-path publish; typed error 87 |

Stage 016 confirmed one shared root cause:
`SetFileInformationByHandle(FileRenameInfo)` with a handle-relative
`RootDirectory` request reaches audit snapshot, audit Git, SQLite backup,
debug export, and upgrade managed-file publication. The supplied corporate
Windows VDI returned `ERROR_INVALID_PARAMETER / os error 87`.

The frozen remediation is one `src/platform_publish.rs` module:

- `ReplaceFileW` for an existing destination;
- `MoveFileExW(MOVEFILE_WRITE_THROUGH)` for no-replace or absent destination;
- source ownership, flush, handle closure, same-parent/reparse guards;
- final reopen and typed committed/durability/failure state;
- no shell, PowerShell, manual SQLite, admin, or per-caller workaround.

## Stage 017 implementation

One production module now owns every regular-file publication policy:
`src/platform_publish.rs`.

The ownership-taking API accepts one anchored parent, one `File`, two direct
child names, and `ReplaceOrCreate` or `NoReplace`. Its typed result separates
commit, final identity validation, durability, and temporary cleanup. Failure
details contain stable endpoint roles, OS code, I/O kind, existence, and
source size, never raw paths, contents, or secrets.

Windows:

- immediately revalidates and flushes the exact supplied source;
- closes conflicting validation and writable handles;
- uses `ReplaceFileW(destination, source, NULL, 0, NULL, NULL)` only for a
  validated existing regular destination;
- uses `MoveFileExW(MOVEFILE_WRITE_THROUGH)` for absent and no-replace;
- never uses `MOVEFILE_REPLACE_EXISTING`;
- retries at most once, only after a proven safe existence race;
- reports actual partial state for error 87 and errors 1175–1177;
- uses absolute verbatim drive and UNC paths;
- contains no `FileRenameInfo` regular-file publication.

Unix keeps anchored `renameat` and `linkat + unlinkat`, including exact
committed-but-not-clean state.

Migrated boundaries:

1. audit `memory.sql`;
2. audit Git loose objects;
3. audit Git `HEAD` and refs;
4. online SQLite backup;
5. debug capsule export;
6. managed adapter and assets.

Caller policy stays explicit. Audit, Git refs, and managed files require a
committed, final-validated, clean result. Backup additionally requires
durability and revalidates SQLite schema. Capsule preserves its typed warning
result. Git object replay verifies an already-existing object.

Complexity is `O(1)`: one parent, one source, at most one destination, and at
most two Windows OS calls. No traversal, buffering, recursive cleanup, shell,
PowerShell, manual SQLite, admin, or second filesystem framework was added.

## Stage 017 proof

```text
focused platform/audit/Git/backup/export/apply/mutation
PASS 93/93

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 716/716 (715 unit + 1 compiled CLI integration)

rtk cargo test --tests --locked
PASS 716/716 (715 unit + 1 compiled CLI integration)

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

No local Windows target exists (`rustup` is absent). Native Windows runtime
remains `PENDING_DOGFOOD`; no false Windows PASS is claimed.

P1: `0`.

P2: `0`.

Later stages must append repair, final export integration, cross-build, and
dogfood evidence.

## Stage 018 implementation

`aopmem platform check --json` now returns before workspace resolution and
before `CommandObservation` construction.

It uses one exclusive UUID-named private OS temp directory and the shared
Atomic Publish V2 helper. It proves create, writable flush, no-replace,
unchanged existing destination, replace existing, reopen byte validation,
direct-child rejection, anchored root identity, the shared helper's reparse
guard contract, and bounded non-recursive cleanup.

The JSON contract reports `observability_recorded:false`,
`admin_required:false`, strategy, durability, and cleanup state. Structured
failures preserve exact OS errors, including error `87`, without paths,
contents, workspace access, database access, or event writes.

```text
focused platform, reparse, and compiled CLI isolation tests
PASS 5/5

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 720/720

rtk cargo test --tests --locked
PASS 720/720

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Native Windows remains `PENDING_DOGFOOD`. P1: `0`. P2: `0`.

## Stage 020 implementation

Debug capsule export now completes the shared publish integration without
changing its exact ordered 12-entry deterministic Stored ZIP64 contract.
Capsule and audit snapshot use the same ownership-taking
`platform_publish` boundary with separate `NoReplace` and marker policies.

One shared `windows_path` helper converts normal absolute drive paths, UNC
paths, existing verbatim paths, Unicode, and paths beyond legacy `MAX_PATH`
before every direct Win32 open or publish call. Anchored parent, temporary,
and source opens therefore use the same path boundary as
`ReplaceFileW`/`MoveFileExW`.

Export reports path-private typed state and exact raw OS errors. Injected
error `87` reports no commit, no destination, confirmed cleanup, and leaves
the audit pending marker unchanged. A committed and final-validated
parent-sync failure returns `EXPORT_PUBLISHED_WITH_WARNING`.

Operational memory uses live-WAL URI read-only mode with `query_only=ON` and
in-memory temp storage. Committed WAL-only data is exported while DB/WAL bytes
stay unchanged. No observability store or self-event is created. Tagged-value
redaction covers raw values and canonical JSON-string copies.

```text
focused capsule/path/CLI/publisher
PASS 32/32

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 732/732

rtk cargo test --tests --locked
PASS 732/732

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Independent review: `PASS`; P1 `0`; P2 `0`.

Native Windows runtime remains `PENDING_DOGFOOD`. Verified through remains
`STAGE_015`. Run the cumulative audit for Stages 016–020 before Stage 021.
