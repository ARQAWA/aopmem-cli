# RC6 global audit report

## Result

Local RC6 audit: `PASS`.

| Severity | Open findings |
| --- | ---: |
| P1 | 0 |
| P2 | 0 |

The requested fresh audit subagent was not started. The user explicitly
closed all subagents and required continuation without them. This is therefore
a fresh local independent-style audit, not a claimed subagent review.

## Scope reviewed

Reviewed the RC6 goal, RC5 frozen decisions and requirements matrix, current
source, RC5/RC6 release evidence, the native Windows failure transcription,
all RC6 diff paths, installer/update contracts, and final assets.

| Required sweep | Result |
| --- | --- |
| Error 32 root cause | Writer had `DELETE`; reader shared only read/write, so live writer blocked validation with error `32`. |
| Handle lifecycle | Writer syncs, identity is captured, then writer drops before source validation; source and destination readers drop before OS publish. |
| Windows shares | Existing `FILE_SHARE_READ | FILE_SHARE_WRITE` contract is retained; no unsupported share-mode change. |
| No-replace / replace | Guarded no-replace and `ReplaceFileW`/`MoveFileExW` strategy tests pass. |
| Reparse and containment | Direct-child, reparse, same-parent, and parent-identity guards remain in the shared publisher. |
| Platform check | Error-32 structured/private cleanup regression and repeatable no-workspace checks pass. |
| Upgrade and preservation | Apply-once, durable backup, staged artifact, journal, and recovery tests pass. |
| Audit and debug consumers | Backup, audit repair, export, and binary publication retain the shared publisher contract. |
| RC5 regression | Full test, integration test, dev verification, installer audit, task, adapter, observability, and UI coverage pass. |
| Installer/update contract | RC6 accepts v0.1 through RC5, rejects RC6 source, preserves exact updater order and no manual replacement. |
| Assets/docs | macOS RC6 version, Windows PE type/imports, reproducible Windows hash, manifest, and final release hashes pass. |
| Forbidden drift | No `src/schema` diff, no production migration `005`, no forbidden changed paths, no candidate secret markers, and no unexpected executable outside existing fixtures or `dist`. |
| Release stop conditions | Native Windows PowerShell runtime remains `PENDING_DOGFOOD`; publication needs the explicit external-action gate. |

## Exact remediation review

The failing RC5 call chain was:

```text
create source writer (GENERIC_READ | GENERIC_WRITE | DELETE,
FILE_SHARE_READ | FILE_SHARE_WRITE)
→ validation reader (GENERIC_READ | FILE_READ_ATTRIBUTES,
FILE_SHARE_READ | FILE_SHARE_WRITE)
→ ERROR_SHARING_VIOLATION (32)
```

RC6 flushes and identity-captures the writer, drops it, reopens and compares
the source identity, scopes destination validation, then publishes. Final
destination reopen and identity validation remain after commit. The change is
inside the unified publisher, so backup, audit repair, debug export, recovery,
and binary publication retain one repaired boundary.

## Commands

```text
cargo fmt --all -- --check                       PASS
rtk cargo clippy --all-targets --locked -- -D warnings  PASS
rtk cargo build --locked                         PASS
rtk cargo test --locked                          PASS 771
rtk cargo test --tests --locked                  PASS 771
rtk scripts/dev_verify.sh                        PASS 769 unit + 2 integration
rtk scripts/audit_v020_installers.sh             PASS 14 groups
git diff --check                                 PASS
jq empty .devplan/RC6_EXECUTION_LEDGER.json      PASS
shasum -a 256 -c dist/SHA256SUMS                 PASS
```

Static audit also passed: `src/schema` has no RC6 diff; only pre-existing
forced-failure test fixtures mention `005`; no candidate secret marker or
forbidden changed path was found.

## Asset boundary

| Asset | SHA-256 |
| --- | --- |
| `aopmem-darwin-arm64` | `b933d921ae6ec68ce7e0f118de27fd7eabe9d1c42d715a0a6df8f2ec731cb949` |
| `aopmem-windows-x86_64.exe` | `8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7` |
| `SHA256SUMS` | `e4e7142e30cb6ef4cac2c7402b8ace8b87fc37df87add59ccb8d79d15d0f3dba` |

The two unchanged-source Windows cross-builds match exactly. macOS isolated
install/update proof passed. Native Windows 11 / PowerShell 5.1 runtime is
not claimed and remains required after prerelease publication.
