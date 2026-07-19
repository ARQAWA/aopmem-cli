# RC6 Windows Publish Remediation

Status: `DONE_LOCAL_CHECKS_PASSED`

## Changed boundary

Only `src/platform_publish.rs` changed production behavior.

New lifecycle:

```text
owned source writer
→ sync_all
→ capture FileIdentity
→ drop(writer)
→ reopen source and compare FileIdentity
→ drop(validation reader)
→ validate/drop optional destination reader
→ verify parent identity
→ os_publish
→ reopen and validate final destination
```

The source and validation reader are scoped `File` values. Rust ownership
ends each handle before the next incompatible operation. No clone, retained
borrow, closure, or result owns either file across `os_publish`.

## Preserved contracts

- one `platform_publish` implementation;
- existing `ReplaceFileW` / `MoveFileExW` selection;
- no-replace never overwrites;
- replace-existing remains explicit;
- same-parent, direct-child, reparse, and parent identity guards remain;
- directory `FILE_SHARE_READ | FILE_SHARE_WRITE` contract remains unchanged;
- no shell, PowerShell, Python, manual SQLite, admin, or retry workaround;
- final destination is reopened and identity-validated before success;
- commit, cleanup, and raw I/O failure semantics remain typed.

The remediation does not add a schema migration. Target schema remains `004`.

## Local proof

```text
cargo fmt --all -- --check                  PASS
rtk cargo test platform_publish --locked    PASS 7 passed
rtk cargo test platform_check --locked      PASS 4 passed
git diff --check                            PASS
```

Next: Stage 04 adds lifecycle and Windows sharing-violation regression proof.
