# AOPMem v0.2.0-rc4 Release Candidate

## Summary

rc4 fixes native Windows `WORKSPACE_BACKUP_FAILED: Access is denied.
(os error 5)` during `upgrade apply`.

The rc3 code flushed the completed SQLite backup through a read-only file
handle. rc4 closes all SQLite handles, validates a temporary database,
flushes it with a writable anchored handle, publishes through the existing
Windows-safe no-replace helper, and validates the final database before any
migration starts.

## Compatibility

- External failure code remains `WORKSPACE_BACKUP_FAILED`.
- Failures now include exact phase, OS error, paths, retained evidence, and
  migration state.
- Schema and migration contents are unchanged.
- Plan remains no-write.
- Prepare and apply use one shared backup algorithm.
- Installer still invokes apply once and publishes only after apply succeeds.
- Failed rc3 and older backup roots are retained.

## Verification

- `cargo fmt --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo build --locked`: PASS.
- `cargo test --locked`: 616/616 PASS.
- `cargo test --tests --locked`: 616/616 PASS.
- focused upgrade tests: 38/38 PASS.
- installer audit: 11/11 PASS.
- isolated macOS two-workspace upgrade and health proof: PASS.
- open P1: 0.
- open P2: 0.

Native Windows runtime confirmation is still `PENDING`; macOS-hosted PE and
fault-injection proof do not replace the Windows 11 PowerShell 5.1 retry.

## Assets

```text
4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e  aopmem-darwin-arm64
e4442fd06622a6b94f997e23b67a55753f1d841f6570ef20ac72b99083a6cc1c  aopmem-windows-x86_64.exe
```

`SHA256SUMS` SHA-256:

```text
bd456530a2e716575cc97d7306c155f39e583dc36d9ea387b7769ae89bcf4da8
```

Windows asset proof:

- `PE32+ executable (console) x86-64`;
- only system DLL imports;
- no dynamic MSVC/UCRT runtime;
- identical hash across two unchanged-source builds.

## Operator boundary

Use the rc4 binary for the next native Windows dogfood run. Do not continue
with rc3, copy SQLite manually, delete old backups, or retry apply
automatically.
