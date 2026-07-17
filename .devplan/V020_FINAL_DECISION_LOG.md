# AOPMem v0.2.0-rc1 Final Decision Log

## Frozen decisions

| ID | Decision | Status |
|---|---|---|
| D-001 | `7c6bf85e...` is authoritative mixed checkpoint | accepted |
| D-002 | `cdad5a9b...` is archive-only; never restore wholesale | accepted |
| D-003 | Recall budget unit is canonical JSON UTF-8 bytes | accepted |
| D-004 | Task soft budget is 256 KiB | accepted |
| D-005 | Mandatory hard budget is 1 MiB | accepted |
| D-006 | Mandatory active types: kernel_contract, gate, project_profile, source, rule | accepted |
| D-007 | Mandatory overflow returns `MANDATORY_CONTEXT_OVERFLOW` and node ids | accepted |
| D-008 | Bundle and correlation ids are lowercase UUID v4 | accepted |
| D-009 | Legacy tool output mode defaults to `inline` | accepted |
| D-010 | `recall --full` is debug/audit/export/migration mode only | accepted |
| D-011 | `--all-workspaces` migrates every DB; adapter/doctor/verify target current repo | accepted |
| D-012 | `upgrade plan`, observe read commands, and UI do not write observability | accepted |
| D-013 | Normal list page is 100; maximum is 500; JSON always exposes completeness | accepted |
| D-014 | Tool ceilings: 15 minutes and 10 MiB per output stream | accepted |
| D-015 | No daemon, cloud, telemetry, Node.js, WSL, CI, or extra release targets | accepted |
| D-016 | Stop after RC proof; no push, tag, GitHub Release, real workspace install, backup deletion | accepted |
| D-017 | Active same-UID DB/path tampering, including the Unix leaf-name race between inode verification and `linkat`, is outside the local/no-sandbox v0.2 boundary; do not add a custom SQLite VFS or platform-specific rename syscall expansion | accepted |
| D-018 | Effectiveness facts use one inclusive 30-day SQLite read snapshot, bounded top-20 lists with explicit `more_results`, and no synthetic score or advice | accepted |
| D-019 | Recall report facts use lifecycle-event timestamps, terminal `more_results` uses the last in-window completion, selected nodes use `first_seen_at`, and adapter drift exposes missing/drifted/failed counts | accepted; clean re-audit P1=0 P2=0 P3=0 |
| D-020 | Debug capsules use the exact ordered 12-entry deterministic Stored ZIP64 contract, derive reference time from persisted observability or the fixed epoch when missing/empty, never overwrite output, and represent missing observability as `not_collected` | accepted; independent audit P1=0 P2=0 P3=1 under D-017 |
| D-021 | Stage 28 UI is invocation-scoped, binds only exact IPv4 `127.0.0.1`, uses a random 32-byte URL token and an embedded exact GET allowlist, and never writes memory or observability. Accept the rare nonfatal `tiny_http` worker panic seen once under aggressive local valid-request `SO_LINGER` RST stress for rc1: the process stayed alive, the token did not leak, and a normal GET still returned 200. Revisit dependency hardening after RC; do not expand Stage 28 scope. | accepted; independent audit P1=0 P2=0 P3=1 |
| D-022 | Stage 29 UI exposes exactly 11 authenticated GET-only read APIs. Lists use scoped keyset cursors; graph responses cap the page plus fixed center context at 200 unique nodes and 500 edges. `center_node` is fixed context and may duplicate the center on the first page; Memory reports `body_omitted=true` even for an empty page because body is absent from the endpoint schema. | accepted; independent audit P1=0 P2=0 P3=2 semantics-only |
| D-023 | Stage 30 ships six embedded, read-only desktop views with safe text-only DOM insertion, bounded deterministic graph rendering, and real temporary-workspace API screenshot proof. Normal SQLite read-only WAL coordination may touch the exact `-wal`/`-shm` sidecars; proof requires unchanged main DB bytes, schema, rows, size, and mtime. `immutable=1` is rejected because the invocation may coexist with legitimate writers. Browser-returned JPEG bytes are mechanically converted and verified as true PNG. | accepted; cumulative audit P1=0 P2=0 after remediation |
| D-024 | `upgrade plan --all-workspaces --json` is a strict no-write, no-self-observation inspection of only v0.1 AOPMem workspaces. It rejects sidecars, corrupt or unsupported schemas, and insufficient disk space per workspace while returning a stable complete plan. Upgrade writes, backups, and rollback belong only to `upgrade apply`. | accepted; Stage 31 P1=0 P2=0 |
| D-025 | Release label `v0.1.0-rc3` contains binaries that report package version `0.1.0`. Update installers therefore accept only that exact reported semver together with the platform-specific SHA-256 from the peeled tag; the benchmark preserves the reported `0.1.0` label and never relabels it. | accepted; tagged macOS binary executed and both tagged assets hash-bound in Stage 33 |
| D-026 | `upgrade apply` holds a write guard across each SQLite Online Backup and migration, never rolls back a previously committed workspace when a later workspace fails, and leaves durable backups plus an exact resumable report. Unsafe recovery after a concurrent commit fails closed and retains the pending marker. | accepted; Stage 32 P1=0 P2=0 after remediation |
| D-027 | The v0.2 installer verifies and stages the new binary before apply, keeps the old installed binary untouched until apply succeeds, and publishes v0.2 atomically only after successful apply. Once apply begins it never restores v0.1; apply or publish failure retains a verified v0.2 recovery binary and all upgrade backups. | accepted; Stage 33 static/fixture audit 10 groups PASS |
| D-028 | Stage 34 timings cover the exact final source tree, Cargo.lock, and release profile. Its release candidate binary is reproducible but not byte-identical to the explicit-target/minimum-macOS-11 flat asset, so no asset-specific speed claim is made. | accepted; source digest and candidate hash reproduced in Stage 35; final asset has separate real install/update/health proof |
| D-029 | Fresh install completes `init`, then creates the managed adapter block with `adapter seed`, and accepts health only when doctor reports `healthy=true` and verify reports `clean=true`. Update asks no onboarding questions and leaves adapter sync to `upgrade apply`. | accepted; Stage 35 real fresh proof and 11-group installer audit PASS |
| D-030 | Windows RC readiness is based on the native PowerShell 5.1 static contract plus a cargo-xwin PE x64 build with no dynamic MSVC/UCRT imports. Native execution remains Windows dogfood work and is not claimed as a macOS-hosted runtime proof. | accepted; no unsupported cross-host runtime claim |
| D-031 | `upgrade plan` remains strictly read-only. Sidecar remediation belongs to explicit `upgrade prepare --all-workspaces --json`, never plan. | accepted; plan no-write and post-prepare ready proof PASS |
| D-032 | `upgrade prepare` creates a durable per-workspace SQLite backup before `PRAGMA wal_checkpoint(TRUNCATE)`, fails closed on busy/incomplete checkpoint, and removes only verified empty direct-child WAL/SHM files after closing SQLite. | accepted; zero-byte and committed-WAL fixtures PASS |
| D-033 | Preparation is idempotent, applies no migration, changes no schema version or logical memory, and uses existing locks, path guards, and backup primitives. | accepted; focused preservation/negative tests PASS |
| D-034 | Installer order is process gate, binary/full-home backups, verified staged binary, prepare, no intervening DB read, read-only plan, one apply, atomic publish, then health checks. | accepted; 11/11 installer audit and real macOS traces PASS |
| D-035 | A SQLite-backed v0.1 binary with unknown hash produces `NONCANONICAL_V010_BINARY` warning. Hash mismatch alone does not block compatible workspaces; corrupt/unsupported/newer schema and unsafe paths still block. | accepted; installer compatibility fixture PASS |
| D-036 | Native Windows retry is required after rc3 release proof. macOS-hosted PE/static checks never count as native Windows runtime proof. | accepted; native Windows proof PENDING |

## Change rule

New product decisions require a real blocker. Implementation choices may change only
when contracts, data preservation, platform support, and proof remain unchanged.
