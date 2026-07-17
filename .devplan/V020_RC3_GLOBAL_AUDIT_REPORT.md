# AOPMem v0.2.0-rc3 Global Audit Report

Verdict: `PASS`

Open findings: `P1=0`, `P2=0`, `P3=0`.

## Scope

Audited cumulative rc3 diff:

- upgrade prepare CLI and JSON contracts;
- SQLite backup/checkpoint safety;
- sidecar path and cleanup guards;
- logical/schema preservation;
- strict no-write upgrade plan;
- apply backup refactor parity;
- macOS and Windows installer ordering/recovery;
- canonical/noncanonical v0.1 behavior;
- version/dependency/document drift;
- release binaries and checksums;
- focused, full, installer, and real fixture evidence.

## Findings

### Closed P3 — installer lifecycle order

Initial audit found both installers downloaded assets before process gate and
durable full-home backup. Checkpoint safety was intact, but task lifecycle and
documentation required backup first.

Remediation moved update flow to:

```text
detect -> process gate -> backups -> download -> stage
-> prepare -> plan -> apply -> publish
```

Repeated installer audit: 11/11 PASS. Repeated real fresh, zero-WAL, and
committed-WAL macOS fixtures: PASS. Finding closed.

No other P1/P2/P3 finding remains.

## Contract matrix

| Contract | Status | Evidence |
|---|---|---|
| Backup precedes checkpoint | PASS | focused + real backup trees |
| Active/busy database fails closed | PASS | focused negative |
| Incomplete checkpoint retains sidecars | PASS | validator/negative proof |
| No manual/non-empty WAL deletion | PASS | anchored empty-only cleanup |
| Prepare is idempotent | PASS | repeated clean prepare |
| Schema version unchanged | PASS | schema marker comparison |
| Logical data preserved | PASS | representative rows + real recall |
| Plan performs no writes | PASS | immutable/tree fingerprint proof |
| No DB read between prepare/plan | PASS | real traces |
| Apply starts once after ready plan | PASS | installer audit/traces |
| Noncanonical hash remains warning | PASS | installer compatibility fixture |
| Native Windows claim remains honest | PASS | runtime still marked pending |

## Rust and complexity review

- Backup implementation moved from `apply.rs` into one shared private module;
  no second backup algorithm was introduced.
- Prepare processes stable workspace list sequentially. Complexity remains
  `O(W log W + total SQLite pages)`, appropriate for a cold upgrade path.
- No new dependency, daemon, async worker, retry engine, cache, or schema was
  added.
- Expected errors use structured results; no production panic path found.
- Complexity scanner reported only pre-existing benchmark/UI leads outside
  rc3 WAL-remediation scope.

## Command evidence

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
| `sh -n` installers/audit | PASS |
| `git diff --check` | PASS |
| Release drift scan | clean |
| macOS fresh/update fixtures | PASS |
| Windows PE/static audit | PASS |
| asset checksum verification | PASS |

## Asset audit

- macOS: Mach-O arm64, minimum macOS 11.0,
  `8bc4d3a7ae38253c1a6e4c653292cf954fb2c8eee916c69a03c6dc5e2484261c`.
- Windows: PE32+ console x86-64,
  `ed59be73d99efd2c1a4fe99e50b85e8b6ce8e8a73b7ff0c96b5327e1c2d39477`.
- Windows imports only system DLLs; no dynamic MSVC/UCRT dependency found.
- `dist/SHA256SUMS` verification: PASS.

## Real fixture audit

Canonical proof root:

```text
/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T/aopmem-rc3-real-proof.tPGZ5n
```

Fresh, zero-byte WAL, and committed-WAL flows pass. Committed WAL row remains
available after prepare and migration. Update traces prove required order and
absence of health/recall/observability DB reads between prepare and plan.

## Known limitation

Native Windows execution remains pending after prerelease publication.
macOS-hosted PE and PowerShell static checks do not replace Windows runtime
proof. This is a dogfood condition, not a blocker for prerelease publication.

## Decision

Approved for `v0.2.0-rc3` prerelease and repeat native Windows dogfood.
