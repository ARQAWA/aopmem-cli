# AOPMem v0.2.0-rc3 WAL Remediation Report

Status: `COMPLETE`

## Real blocker

Native Windows dogfood for `v0.1 -> v0.2.0-rc2` stopped safely during
authoritative planning:

- plan exit: `0`;
- `ok=true`;
- `writes_performed=false`;
- `ready=false`;
- code: `UNSAFE_DATABASE_SIDECAR`;
- sidecar: `aopmem.sqlite-wal`;
- observed size: `0` bytes;
- apply attempts: `0`;
- migration/publish: not started.

The installation and both workspaces remained unchanged. Durable full backup
was retained.

## Root cause

Normal SQLite reads can leave WAL/SHM coordination files. Upgrade plan
correctly refuses sidecars but must remain no-write. The product had no
supported command to backup, checkpoint, and verify a workspace before plan.

## Implemented remediation

Added:

```text
aopmem upgrade prepare --all-workspaces --json
```

Behavior:

- stable all-workspace order;
- existing mutation locks and path/reparse guards;
- durable per-workspace SQLite Online Backup before checkpoint;
- `PRAGMA wal_checkpoint(TRUNCATE)`;
- busy/incomplete checkpoint fails closed;
- SQLite connection closes before cleanup;
- only verified empty direct-child WAL/SHM removed;
- no manual or blind non-empty sidecar deletion;
- no schema migration;
- schema version and logical memory unchanged;
- idempotent clean/repeated execution;
- exact structured workspace error.

`upgrade plan` remains strict no-write inspection. Sidecar fix hint points to
the prepare command.

Installer order:

```text
process gate
-> binary/full-home backups
-> asset download and verification
-> verified stage
-> prepare
-> plan
-> apply once
-> atomic publish
-> health checks
```

No AOPMem database read runs between prepare and plan.

## Noncanonical v0.1

Unknown v0.1 binary SHA-256 produces:

```text
NONCANONICAL_V010_BINARY
```

Hash remains visible evidence, not sole blocker. Durable backup plus staged
prepare and plan decide workspace compatibility. Unknown/corrupt/newer schema
and unsafe paths remain blockers.

## Implementation proof

| Item | Result | Evidence |
|---|---|---|
| CLI prepare command | PASS | CLI parser/JSON tests |
| Backup before checkpoint | PASS | focused test and real backup trees |
| Zero-byte WAL remediation | PASS | focused + real macOS update |
| Committed WAL preservation | PASS | focused + real row/recall proof |
| Busy/incomplete checkpoint safety | PASS | focused negative tests |
| Path/reparse safety | PASS | focused negative tests |
| Idempotence | PASS | repeated clean preparation test |
| Schema/logical preservation | PASS | schema markers and representative rows |
| Plan strict no-write | PASS | tree fingerprint and plan contract tests |
| Installer ordering | PASS | 11/11 audit and real traces |
| Noncanonical warning | PASS | installer compatibility fixture |

## Checks

| Check | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo build --locked` | PASS |
| `cargo test --locked` | 609/609 PASS |
| `cargo test --tests --locked` | 609/609 PASS |
| `scripts/dev_verify.sh` | PASS, 609 tests |
| Focused prepare tests | 9/9 PASS |
| Upgrade tests | 32/32 PASS |
| Independent broad upgrade filter | 33/33 PASS |
| `scripts/audit_v020_installers.sh` | 11/11 PASS |
| Installer shell syntax | PASS |
| `git diff --check` | PASS |
| Release-scope version drift | clean |

## Real macOS proof

Proof root:

```text
/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T/aopmem-rc3-real-proof.tPGZ5n
```

- fresh install: PASS;
- v0.1 zero-byte WAL update: PASS;
- v0.1 committed non-empty WAL update: PASS;
- committed rule preserved and returned by recall;
- adapter in sync, doctor healthy, verify clean;
- recall and observability successful;
- prepare and migration backups retained;
- traces contain no DB read between prepare and plan.

## Release assets

| Asset | Type | SHA-256 |
|---|---|---|
| `dist/aopmem-darwin-arm64` | Mach-O arm64, macOS 11 minimum | `8bc4d3a7ae38253c1a6e4c653292cf954fb2c8eee916c69a03c6dc5e2484261c` |
| `dist/aopmem-windows-x86_64.exe` | PE32+ console x86-64 | `ed59be73d99efd2c1a4fe99e50b85e8b6ce8e8a73b7ff0c96b5327e1c2d39477` |
| `dist/SHA256SUMS` | checksum manifest | `e871a6dcd53909e80b0cd7e1ab794e300fd4faeb961e3bbb83a770c4a8fcb871` |

Windows PE imports only system DLLs. Static CRT boundary remains satisfied.
`shasum -a 256 -c dist/SHA256SUMS` passes.

## Result

- Current changed paths: 22.
- Tests: 609/609.
- Open P1: 0.
- Open P2: 0.
- Open P3: 0.
- Ready for prerelease: yes.
- Ready for repeat native Windows dogfood: yes.
- Native Windows runtime retry: pending after release.
