# AOPMem v0.2.0-rc3 Release Candidate

Status: `READY_FOR_PRERELEASE`

## Purpose

Fix SQLite WAL upgrade deadlock found during native Windows dogfood while
preserving strict upgrade safety.

## Changes

- added `upgrade prepare --all-workspaces --json`;
- backup each affected workspace before checkpoint;
- checkpoint committed WAL through SQLite;
- remove only verified empty direct-child WAL/SHM after connection close;
- keep prepare idempotent and schema/logical-data preserving;
- keep `upgrade plan` strictly read-only;
- point sidecar blocker to supported prepare command;
- run update order `gate -> backups -> download -> prepare -> plan -> apply
  -> publish`;
- run no DB read between prepare and plan;
- report noncanonical v0.1 binary hash as warning, not sole blocker;
- retain recovery evidence and backups.

## Compatibility

- Source: SQLite-backed AOPMem v0.1 workspaces.
- Canonical source release: `v0.1.0-rc3`.
- Target: `v0.2.0-rc3`.
- macOS: Apple Silicon.
- Windows: Windows 11 x64, native PowerShell 5.1.
- Old file MVP: unsupported.
- Linux, Windows ARM, Intel Mac: unsupported.

## Release gates

| Gate | Status |
|---|---|
| WAL remediation implementation | PASS |
| Focused positive/negative tests | PASS |
| Logical preservation proof | PASS |
| Plan no-write proof | PASS |
| Installer audit | 11/11 PASS |
| Full Rust checks | 609/609 PASS |
| macOS real fresh/update proof | PASS |
| macOS arm64 asset | PASS |
| Windows x64 PE asset | PASS |
| `SHA256SUMS` | PASS |
| Global audit P1=0, P2=0 | PASS |

## Assets

| Asset | SHA-256 |
|---|---|
| `dist/aopmem-darwin-arm64` | `8bc4d3a7ae38253c1a6e4c653292cf954fb2c8eee916c69a03c6dc5e2484261c` |
| `dist/aopmem-windows-x86_64.exe` | `ed59be73d99efd2c1a4fe99e50b85e8b6ce8e8a73b7ff0c96b5327e1c2d39477` |
| `dist/SHA256SUMS` | `e871a6dcd53909e80b0cd7e1ab794e300fd4faeb961e3bbb83a770c4a8fcb871` |

## Proof summary

- 609/609 tests pass.
- Focused prepare: 9/9 PASS.
- Installer audit: 11/11 PASS.
- Zero-byte WAL: PASS.
- Committed WAL: PASS; committed row preserved.
- Busy DB and incomplete checkpoint: fail closed.
- Backup-before-checkpoint: PASS.
- Plan no-write: PASS.
- macOS fresh, zero-WAL update, committed-WAL update: PASS.
- Windows PE/static/import proof: PASS.
- Open findings: P1=0, P2=0, P3=0.

Canonical proof root:

```text
/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T/aopmem-rc3-real-proof.tPGZ5n
```

## Known limitation

Native Windows runtime retry is pending after prerelease publication. Static
PE and PowerShell checks passed. Native retry remains required dogfood, not a
prerelease publication blocker.

## Decision

Ready to commit, push, tag `v0.2.0-rc3`, and publish GitHub prerelease with:

- `dist/aopmem-darwin-arm64`;
- `dist/aopmem-windows-x86_64.exe`;
- `dist/SHA256SUMS`.
