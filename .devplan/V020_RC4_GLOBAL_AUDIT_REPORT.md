# AOPMem v0.2.0-rc4 Global Audit Report

Status: `PASS`; native Windows runtime confirmation: `PENDING`.

## Result

The rc3 native Windows backup failure is fixed in code and covered by
cross-platform negative proof. rc4 is ready for native Windows dogfood.

Exact rc3 failing operation:

```rust
File::open(destination_path)?.sync_all()?;
```

On Windows this used a read-only handle for `FlushFileBuffers`, which can
return `ERROR_ACCESS_DENIED` (`5`). In rc4 this boundary is
`backup_phase=flush_temporary_file`; native phase confirmation awaits the rc4
Windows run.

## Safety audit

| Contract | Result |
|---|---|
| SQLite backup object closes before publish | PASS |
| Destination/source SQLite connections close before publish | PASS |
| Temporary DB read-only validation | PASS |
| Writable anchored file flush | PASS |
| Existing Windows anchored no-replace publish | PASS |
| Final DB read-only reopen and validation | PASS |
| Migration starts only after valid final backup | PASS |
| Partial/final evidence retained on failure | PASS |
| Exact phase and raw OS error in JSON | PASS |
| Failed rc3 root retained and not reused | PASS |
| Plan remains no-write | PASS |
| Prepare/apply share one backup helper | PASS |
| Installer applies once and publishes only after success | PASS |

No second Windows filesystem framework, manual SQLite copy, backup deletion,
automatic apply retry, or failure-to-warning conversion was added.

## Test proof

| Check | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo build --locked` | PASS |
| `cargo test --locked` | 616/616 PASS |
| `cargo test --tests --locked` | 616/616 PASS |
| `scripts/dev_verify.sh` | PASS, 616 tests |
| Focused upgrade tests | 38/38 PASS |
| Windows test target `cargo xwin test --no-run` | PASS |
| Installer audit | 11/11 PASS |
| `git diff --check` | PASS |
| Release version drift scan | clean |
| Forbidden Windows/installer drift scan | clean |

The test count increased from 615 to 616 after adding the complete exact
11-value `backup_phase` serialization contract.

## Negative proof

- Every backup phase can fail without returning backup success.
- Injected access denied preserves `raw_os_error=5`.
- Flush failure retains a validated non-empty temporary database.
- Every backup failure reports `migration_started=false`.
- Final-validation and metadata failures retain published evidence.
- First-workspace backup failure leaves the second workspace not started.
- Migration failure keeps its validated backup and follows existing rollback.
- Clean prepare remains `writes_performed=false`.
- WAL backup failure prevents checkpoint.

## Two-workspace and macOS proof

Proof root:

```text
/tmp/aopmem-rc4-real-proof.LmFqjz
```

Workspaces:

```text
cat-rental-d2cb053f
warranty-5f87c76e
```

Proof:

- repeated clean prepare: both `already_clean`, no writes, no backup root;
- zero-byte WAL and committed non-empty WAL: prepared and removed safely;
- committed WAL row survived checkpoint and migration;
- plan reported both schemas `001`, target `003`, and made no file change;
- apply was invoked once and migrated both in stable order;
- both final backups reopened with `quick_check=ok` and schema `001`;
- both live databases reopened with `quick_check=ok` and schema
  `001,002,003`;
- logical rows, tool contracts, MCP profiles, tool files, and artifacts match;
- the exact failed rc3 sentinel root remains;
- installed isolated binary reports `aopmem 0.2.0-rc4`;
- adapter status, doctor, verify, recall, observe status/report/export pass for
  both repositories;
- no repository-local `.aopmem` exists.

This is macOS proof. It is not native Windows runtime proof.

## Complexity and drift

The remediation keeps one stable workspace pass. SQLite backup cost remains
linear in database pages. Schema/table validation uses a fixed 10-table set.
Run-root collision attempts are bounded at 8. No unbounded retry, nested
workspace scan, N+1 external query, or new asymptotic hotspot was added.

All changed production paths are inside the approved rc4 scope. Two
out-of-scope files have test-only portability changes:
`src/tools/mod.rs` and `src/audit/anchored_git.rs`. They remove Unix-only test
compile drift so the Windows test target can link; production behavior is
unchanged.

## Release assets

| Asset | Proof | SHA-256 |
|---|---|---|
| `aopmem-darwin-arm64` | Mach-O arm64, macOS 11.0 minimum | `4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e` |
| `aopmem-windows-x86_64.exe` | PE32+ console x86-64 | `e4442fd06622a6b94f997e23b67a55753f1d841f6570ef20ac72b99083a6cc1c` |
| `SHA256SUMS` | both entries verify | `bd456530a2e716575cc97d7306c155f39e583dc36d9ea387b7769ae89bcf4da8` |

The Windows asset was rebuilt twice from unchanged final source and produced
the same SHA-256. Imports contain system DLLs only; no dynamic
`VCRUNTIME`, `MSVCP`, `UCRTBASE`, or `api-ms-win-crt` dependency was found.

## Open findings

- Open P1: `0`.
- Open P2: `0`.
- Native Windows 11 / PowerShell 5.1 / no-admin rc4 retry: `PENDING`.

## Decision

Approved for `v0.2.0-rc4` prerelease and native Windows dogfood. Do not
continue with the failed rc3 binary. Preserve all existing backup roots.
