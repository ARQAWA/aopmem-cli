# AOPMem v0.2.0-rc6 release candidate

Status: `LOCAL_RC_READY_FOR_EXTERNAL_GATE`

## Fix

RC5 native Windows staged `platform check --json` failed at
`validate_source` with `ERROR_SHARING_VIOLATION` (`32`). The source writer
still held `DELETE` access while the validation reader shared only read/write.

RC6 closes the writer before validation and scopes both validation readers
before OS publication. It retains no-replace, replace-existing, reparse,
same-parent, final-reopen, and final-identity protections.

## Proof

- Platform-check error-32 regression is structured, private, and cleanup-safe.
- All shared publisher consumers passed: upgrade backup, audit repair, debug
  export, recovery, and binary publication.
- Full Rust, integration, dev, and installer checks passed.
- Isolated macOS fresh/install/update proof passed for RC4/schema-003,
  RC5/schema-004, and mixed workspaces.
- Windows cross-build created a PE32+ console x86-64 artifact twice with the
  same SHA-256 and no dynamic MSVC/UCRT imports.

## Compatibility

- All RC5 features are retained.
- Target schema remains `004_task_protocol_and_tool_aliases`.
- No migration `005` was added.
- Native Windows RC6 acceptance is still `PENDING_DOGFOOD`.

## Assets

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `aopmem-darwin-arm64` | 9747560 | `b933d921ae6ec68ce7e0f118de27fd7eabe9d1c42d715a0a6df8f2ec731cb949` |
| `aopmem-windows-x86_64.exe` | 10572288 | `8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7` |
| `SHA256SUMS` | 178 | `e4e7142e30cb6ef4cac2c7402b8ace8b87fc37df87add59ccb8d79d15d0f3dba` |

Open P1/P2: `0/0`.

Next: create the local RC6 commit, then wait for the explicit external-action
approval before any push, tag, or GitHub prerelease.
