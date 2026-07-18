# AOPMem v0.2.0-rc5 Field Findings

Status: `DONE_LOCAL_CHECKS_PASSED`

This document records the Stage 01 baseline. It separates user-supplied field
facts, repository evidence, scanner leads, and verified implementation gaps.
It does not claim a new native Windows run.

## Baseline identity

| Field | Value |
|---|---|
| Goal | `AOPMem v0.2.0-rc5` |
| Baseline branch | `main` |
| Upstream state | `main...origin/main` |
| Baseline commit | `0af9b22c2e4a8217cbf6b1de558eb2181ce79a84` |
| Baseline tag | `v0.2.0-rc4` |
| Package version | `0.2.0-rc4` |
| Edition / MSRV | Rust 2021 / `1.89` |
| Local compiler | Homebrew `rustc 1.95.0` |
| Local Cargo | Homebrew `cargo 1.95.0` |
| Worktree at Stage 01 start | clean |
| User changes to preserve | none present |

The repository remains one Rust crate. SQLite operational memory, FTS5/BM25,
workspace-local storage, separate Local Observability, and the read-only local
UI already exist and remain the architectural base.

## Field facts

The facts below come from the supplied RC5 field audit. They are input facts,
not results reproduced on this macOS host.

| ID | Field fact | RC5 requirement |
|---|---|---|
| F-01 | Inspected sessions: `10` | `RC5-FLD-001` |
| F-02 | Recall before first action: `0/10` | `RC5-TSK-001`, `RC5-KPR-001`, `RC5-BLK-002` |
| F-03 | Bundle reuse: `0/10` | `RC5-TSK-007`, `RC5-KPR-003` |
| F-04 | User memory reminders: `10/10` | `RC5-BLK-002`, `RC5-DOG-002` |
| F-05 | Recall/reminder apparently ignored: `8` sequences | `RC5-TSK-007`, `RC5-DOG-001` |
| F-06 | Tools: `6`; duplicate/overlap pairs: `3` | `RC5-TOL-001`, `RC5-DUP-001`, `RC5-CGD-001` |
| F-07 | Confluence pair: `SAME_IMPLEMENTATION_DIFFERENT_NAME` | `RC5-DUP-003` |
| F-08 | Recommended canonical id: `confluence_reader` | `RC5-DUP-003` |
| F-09 | Blanket `Do not store secrets` blocks safe test credentials | `RC5-SEC-001`, `RC5-SEC-002` |
| F-10 | Global instructions lack a hard task-start gate | `RC5-BLK-001`, `RC5-BLK-002` |
| F-11 | Managed block describes AOPMem but does not block work | `RC5-BLK-002` |
| F-12 | Official Windows updater fails | `RC5-WIN-001`, `RC5-UPG-001` |
| F-13 | Windows audit snapshot has a pending marker | `RC5-AUD-001`, `RC5-AUD-002` |
| F-14 | Windows debug capsule export fails | `RC5-DBG-001` |
| F-15 | Core SQLite memory, recall, status, and report work | `RC5-ARC-001`, `RC5-TST-002` |
| F-16 | Corrected shell/helper failures were audit harness noise | `RC5-FLD-002` |

`F-16` is excluded from product defects. Shell and helper mistakes from field
collection must not be copied into the RC5 bug list.

## Current implementation inventory

| Area | Current evidence | RC5 gap |
|---|---|---|
| CLI | `src/cli/mod.rs:620-684` | No `task`, `platform`, or `audit repair` commands |
| Recall model | `src/recall/mod.rs:20-62` | Budgets and response base exist; no task lifecycle package |
| Recall paging | `src/cli/mod.rs:5303-5449` | Caller-visible continuation; task start must finish internally |
| Operational schema | `src/schema/mod.rs:5-171` | Only migrations `001..003`; no `004` |
| Node aliases | `src/schema/mod.rs:55-64` | Existing `aliases` are node aliases, not tool aliases |
| Observability | `src/observability/mod.rs:23-27` | Schema v1; no task, dedupe, repair, or platform facts |
| Observability open | `src/observability/mod.rs:2616-2716` | Strict v1 validation; explicit v1 to v2 migration required |
| Managed block | `templates/managed-block/AGENTS.managed-block.md:1-36` | V1, manual recall, blanket secret ban |
| Adapter default | `src/adapter/mod.rs:120-122` | Defaults to `AGENTS.md`; no active-adapter detection |
| Memory Keeper | `templates/skills/memory-keeper/SKILL.md:14-33` | Manual cursor flow; no start/apply/complete receipt |
| Redaction | `src/observability/mod.rs:247-270` | Uses `[REDACTED]`, not `<TEST_SECRET_REDACTED>` |
| Tool CLI | `src/cli/mod.rs:1211-1219` | No tool alias, resolve, or dedupe commands |
| Tool lookup | `src/tools/mod.rs:640-665` | Direct `tool_id` lookup only |
| Tool run/validate | `src/tools/mod.rs:994-1119` | No canonical alias resolution before path lookup |
| Audit snapshot | `src/audit/mod.rs:437-531` | Streaming base and pending marker exist; no repair command |
| Debug export | `src/observability/export.rs:463-492` | Uses current shared no-replace publish boundary |
| Upgrade backup | `src/upgrade/backup.rs:350-386` | Uses current shared no-replace publish boundary |
| Windows backend | `src/audit/anchored.rs:962-1020` | Uses `SetFileInformationByHandle(FileRenameInfo)` |
| Windows sharing | `src/audit/anchored.rs:901-940` | File handles omit `FILE_SHARE_DELETE` |
| Upgrader | `src/upgrade/apply.rs:34-38` | Target and recovery names hardcoded to rc4 |
| Installers | `install/v0.2/install.sh:653-708`; `install/v0.2/install.ps1:782-859` | v0.1-only source flow; no platform check or repair |
| UI | `src/ui/data.rs:208-432` | No task compliance, aliases, or duplicate facts |

## Reusable foundations

- Mandatory context already loads active `kernel_contract`, `gate`,
  `project_profile`, `source`, and `rule` nodes.
- Recall already has a 1 MiB mandatory hard budget and 256 KiB task soft
  budget.
- Recall already has deterministic revision binding, deduplication, source
  priority, typed roots, FTS, direct links, and depth-two graph expansion.
- Audit snapshot generation already streams rows.
- Pending snapshot removal already occurs only after publish and Git commit.
- Audit snapshot, debug export, and upgrade backup already converge on one
  anchored publish boundary.
- Schema planning already validates an applied migration prefix.
- Local Observability already has 30-day or 100 MB retention.

These foundations should be extended. Parallel replacement frameworks are out
of scope.

## Complexity evidence

The repository scanner reported 62 heuristic leads:

- 10 in `scripts/benchmark_v020.py`;
- 52 in `src/ui/assets/app.js`;
- none from Rust because the scanner does not parse Rust.

Manual review classifies the benchmark loops as harness work. UI work is
bounded by the existing 200-node and 500-edge limits. Neither group justifies
an RC5 rewrite.

Verified RC5 hot path:

- `operational_recall_revision` scans eight operational tables for every task
  recall: `src/storage/mod.rs:1676-1746`;
- current cost is `O(all operational rows)` per invocation.

Measured-before-change risk:

- continuation uses `LIMIT/OFFSET`;
- graph pages rerun a recursive CTE and ordered result construction;
- relevant code is `src/storage/mod.rs:1822-2163` and
  `src/cli/mod.rs:5460-5591`.

Stage 05 must enforce a scan bound and preserve deterministic results.
Keyset paging, one-invocation materialization, or a durable revision counter
remain implementation options only after focused measurement. Stage 26 owns
the reproducible performance proof.

Tool fingerprints must not run on every normal tool execution. Fingerprint
hashing is limited to creation/dedupe preflight, with each shortlisted
implementation file hashed once per operation.

Audit repair and debug export must remain streaming and linear in emitted data.

## Historical proof baseline

The rc4 global audit records:

| Check | Historical result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo build --locked` | PASS |
| `cargo test --locked` | `616/616` PASS |
| `cargo test --tests --locked` | `616/616` PASS |
| `scripts/dev_verify.sh` | PASS |
| Installer audit | `11/11` PASS |
| Native Windows runtime | `PENDING` |

This is historical evidence from
`.devplan/V020_RC4_GLOBAL_AUDIT_REPORT.md`. Stage 01 did not rerun Cargo
checks because its scope is repository inspection and planning artifacts.

Current source contains 618 `#[test]` attributes. This count is inventory, not
a current test result.

## Release asset baseline

| Asset | Type | SHA-256 |
|---|---|---|
| `dist/aopmem-darwin-arm64` | Mach-O 64-bit arm64 | `4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e` |
| `dist/aopmem-windows-x86_64.exe` | PE32+ console x86-64 | `e4442fd06622a6b94f997e23b67a55753f1d841f6570ef20ac72b99083a6cc1c` |
| `dist/SHA256SUMS` | manifest | `bd456530a2e716575cc97d7306c155f39e583dc36d9ea387b7769ae89bcf4da8` |

Both manifest entries verify. These are rc4 assets, not rc5 assets.

## Worktree classification

Stage 01 start state was clean. No recovery ref, hunk classification, or user
change preservation action is required.

Stages 01 and 02 may add only RC5 planning and DERC bookkeeping under
`.devplan/`. Product code, tests, installers, templates, docs, and assets stay
unchanged.

## Stage 01 conclusion

- Field facts are recorded without harness noise.
- Every field fact maps to at least one RC5 requirement.
- Current reusable foundations and gaps are identified.
- Complexity scanner leads are separated from verified hot paths.
- Native Windows remains explicitly pending.
- Stage 02 Definition of Ready: `PASS`.
