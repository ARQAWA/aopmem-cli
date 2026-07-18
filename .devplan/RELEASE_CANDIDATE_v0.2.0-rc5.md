# AOPMem v0.2.0-rc5 Release Candidate

Status: `COMPLETE_LOCAL_RELEASE_READY`
Date: `2026-07-18`
Baseline: `v0.2.0-rc4` / `0af9b22c2e4a8217cbf6b1de558eb2181ce79a84`
Local severity: P1 `0`; P2 `0`
Native Windows runtime: `PENDING_DOGFOOD`

## Executive summary

RC5 is locally complete. It adds the V2 task-start protocol, a single
canonical Managed Block and native Memory Keeper, private lifecycle state,
explicit test-secret rules and redaction, canonical tools and aliases,
Windows-safe publish/repair/export primitives, a read-only local UI, and the
documented upgrade path from supported older layouts.

This report is a local release-candidate decision record, not a publication.
The full RC5 diff is relative to the clean rc4 baseline above. The final
independent global audit (`RC5_GLOBAL_AUDIT_REPORT.md`) passed fifteen sweeps;
the independent Stage026–030 cumulative audit verified the final release,
evidence, matrix, and stop claims.

No commit, push, tag, GitHub Release, real Windows installation, or backup
deletion has occurred. The one remaining platform activity is the documented
native Windows 11 x64 PowerShell 5.1 dogfood. It is deliberately not promoted
to PASS by cross-build, PE inspection, or unit-test evidence.

## Contract and architecture outcome

| Area | Delivered contract | Primary evidence |
|---|---|---|
| Task protocol | `task start/apply/complete`; durable private state; revision/bundle validation; idempotent replay | `docs/TASK_START_PROTOCOL.md`, TP-01..12 |
| Managed operation | One 18-section canonical template; native Keeper obtains and applies context before work | `RC5_MANAGED_BLOCK_V2_SPEC.md`, `docs/MEMORY_KEEPER_V2.md`, MB/MK tests |
| Privacy | No raw query/chat/output persistence; exact tagged test values redact on every export surface | `docs/SECRET_HANDLING.md`, SEC-01..08 |
| Tools | One canonical ID with direct aliases; duplicate preflight and exact-only canonicalisation | `docs/TOOL_ALIASES_AND_DEDUPLICATION.md`, TOOL-01..11 |
| Observability/UI | Local factual store is separate from operational memory; UI is read-only, token protected, loopback-only | `docs/LOCAL_OBSERVABILITY.md`, `docs/DESKTOP_UI.md` |
| Windows boundary | One guarded publish primitive for backup, audit snapshot, and capsule; platform check and repair semantics | `RC5_WINDOWS_PUBLISH_REPORT.md`, WIN-01..11 |
| Upgrade | Prepared recovery journal, verified staged binary, apply-once and no backup deletion | `docs/UPGRADE_TO_RC5.md`, UPG-01..11 |

The architecture remains one Rust crate with SQLite/FTS5, typed graph data,
local workspace storage, separate factual observability, and no new hosted
service, backend, history product, or domain pack. Stage029's excluded-scope
scan found no drift.

## Changes, by implementation group

| Stages | Change | Local proof |
|---:|---|---|
| 04–08 | Lifecycle schema V2, bounded complete retrieval, apply/complete rules, Keeper and adapter/block installation | TP-01..12, MK-01..07, MB-01..09; Stage005 and 006 audits |
| 09–10 | Explicit test-secret persistence, action-based approval, tagged redaction in events, reports, snapshots, and capsules | SEC-01..08; protected-sink and deterministic canary checks |
| 11–15 | `004_task_protocol_and_tool_aliases`, direct alias resolution, fingerprint preflight, exact-only dedupe, Confluence fixture | TOOL-01..11; `RC5_TOOL_DEDUPE_REPORT.md` |
| 16–20 | Windows error-87 root-cause closure, guarded atomic publish, platform check, repair, pending marker, debug capsule | WIN-01..11; Windows publish/repair/capsule docs |
| 21–24 | Recovery journal and staged binary, installer order, explicit active adapter, native macOS fresh/mixed upgrades | UPG-01..11; `RC5_MACOS_PROOF_REPORT.md` |
| 23 | V2 observability reports and local read-only desktop UI | observability/UI source and security regressions |
| 25 | Ten isolated clean-agent protocol scenarios with redacted receipts/facts only | `RC5_AGENT_COMPLIANCE_REPORT.md`, DOG-01..10 |
| 26 | Measured task/alias structural bounds; raw median/p95 retained without an invented SLA | `RC5_PERFORMANCE_REPORT.md`, checksumed raw samples |
| 27 | Full negative/security regression, including executable snapshot fallback, mutation, orphan, and repeated-short-run cases | `RC5_REGRESSION_REPORT.md` |
| 28–29 | Flat release assets, two Windows cross-build hash proof, documentation, and independent 15-sweep audit | Stage028/029 handoffs and global audit |

### Stage25 agent proof

The isolated native-subagent evidence contains ten authoritative scenarios.
All ten start before substantive action, apply context before substantive
action, select an applicable gate/workflow/tool, and complete. Reminders,
duplicate tool creation, test-secret blanket refusals, external writes, and
started-but-not-applied tasks are each `0`. DOG-09 reuses a continuing receipt;
DOG-10 creates a distinct task/bundle for a material new goal. Evidence is
privacy-safe and its ten checksum entries validate.

### Stage26 performance proof

The reproducible benchmark uses three warmups and fifteen samples per task
start corpus. Measured medians/p95 are: small `20.039/22.061 ms`, medium
`25.526/27.968 ms`, and large `46.891/65.797 ms`. The alias proof exercises
real add-and-`kind=Alias` resolution in three corpora. Structural checks prove
bounded scans and no normal-run N+1 implementation hashes. These values are
host-local observations, not a latency SLA or Windows-runtime result.

### Stage27 regression and negative proof

The final locked suites passed `768/768` for both `cargo test --locked` and
`cargo test --tests --locked`. Negative coverage includes two isolated
100-invocation short-tool loops, forced clone fallback, in-place source
mutation fail-closed behavior, same-process-group orphan cleanup, escaped
descendant cleanup, and path/ancestor swap protection. The regression report
also maps the task, block, Keeper, secret, tool, Windows, upgrade,
observability, and UI catalogs. The error-87 root cause was resolved by one
publish boundary; no second filesystem workaround was introduced.

### Exact 30-stage delivery record

Stages 001–025 have already passed their five-stage cumulative audits.
Stages026–030 passed their independent cumulative audit. This table is the
compact execution record; exact artifacts remain in the stage handoffs and
proof log.

| Stage | Result | State |
|---:|---|---|
| 001 | Field findings, baseline, and worktree classification | VERIFIED |
| 002 | Frozen decisions and requirements matrix | VERIFIED |
| 003 | Managed Block V2 normative specification | VERIFIED |
| 004 | Task lifecycle and observability schema V2 | VERIFIED |
| 005 | Bounded complete `task start` retrieval | VERIFIED |
| 006 | `task apply` and `task complete` | VERIFIED |
| 007 | Native Memory Keeper V2 skill | VERIFIED |
| 008 | Canonical managed block and adapters | VERIFIED |
| 009 | Secret contract implementation | VERIFIED |
| 010 | Redaction and explicit persistence tests | VERIFIED |
| 011 | Alias migration and storage API | VERIFIED |
| 012 | Canonical fingerprint and dedupe plan | VERIFIED |
| 013 | Alias-aware list/get/run/validate | VERIFIED |
| 014 | Exact-only duplicate canonicalisation | VERIFIED |
| 015 | Confluence fixture and tool rules | VERIFIED |
| 016 | Windows publish root-cause audit | VERIFIED |
| 017 | Unified Windows Atomic Publish V2 | VERIFIED |
| 018 | Private platform self-check | VERIFIED |
| 019 | Audit repair and durable marker rules | VERIFIED |
| 020 | Debug capsule and snapshot integration | VERIFIED |
| 021 | Upgrade/recovery support through rc4 | VERIFIED |
| 022 | Prompt and shell/PowerShell installer order | VERIFIED |
| 023 | Effectiveness reports and minimal UI | VERIFIED |
| 024 | Native macOS fresh/mixed update proof | VERIFIED |
| 025 | Ten clean-agent dogfood scenarios | VERIFIED |
| 026 | Focused measured performance proof | VERIFIED |
| 027 | Full negative/security regression | VERIFIED |
| 028 | Assets, checksums, and release docs | VERIFIED |
| 029 | Independent global audit, fifteen sweeps | VERIFIED |
| 030 | RC report, matrix, DoD, stop proof | VERIFIED |

### Stage25 scenario summary

Only bounded, privacy-safe facts are reproduced here; persistent session data
and full receipts are not included.

| Scenario | Contract exercised | Result |
|---|---|---|
| DOG-01 | Simple discussion after receipt | start/apply/complete before answer |
| DOG-02 | Clarifying question after receipt | no pre-receipt question |
| DOG-03 | Code investigation | file read only after receipt |
| DOG-04 | Modification planning | bounded plan; no implementation drift |
| DOG-05 | External Confluence read | canonical read-only tool; zero writes |
| DOG-06 | SMTP/API discussion | sandbox design; zero calls and writes |
| DOG-07 | Authorized synthetic credential | stdin auth succeeds; durable value redacted |
| DOG-08 | Existing equivalent tool | canonical reuse; zero new paths/aliases/tools |
| DOG-09 | Same-goal continuation | no extra start/apply; one completion |
| DOG-10 | Material new goal | fresh task and bundle boundary |

The aggregate remains: starts `10/10`, applies `10/10`, completions `10/10`,
mandatory applicable context `10/10`, reminders `0`, duplicate tools `0`,
blanket credential refusals `0`, external writes `0`, and unfinished starts
`0`. The checksum manifest validates all ten normalized evidence files.

### Stage26 complete metric table and method limits

| Corpus | Operation | Median ms | P95 ms |
|---|---|---:|---:|
| small 16 | task start | 20.039 | 22.061 |
| medium 64 | task start | 25.526 | 27.968 |
| large 256 | task start | 46.891 | 65.797 |
| small | apply / complete | 19.215 / 18.782 | 24.078 / 21.873 |
| medium | apply / complete | 21.971 / 21.161 | 23.712 / 23.341 |
| large | apply / complete | 35.062 / 32.183 | 40.392 / 81.662 |
| small | dedupe / canonical resolver | 11.979 / 5.577 | 13.564 / 6.442 |
| medium | dedupe / canonical resolver | 13.162 / 5.644 | 14.896 / 6.487 |
| large | dedupe / canonical resolver | 19.464 / 6.163 | 35.033 / 9.453 |
| small | repair / export / check | 10.579 / 25.162 / 37.465 | 11.301 / 28.652 / 40.886 |
| medium | repair / export / check | 11.612 / 33.928 / 39.594 | 14.002 / 42.792 / 45.852 |
| large | repair / export / check | 16.522 / 75.128 / 41.639 | 28.213 / 103.092 / 49.531 |

P95 is nearest-rank `ceil(0.95 * 15)`, not an interpolation. Every corpus
uses a new temporary `AOPMEM_HOME` and Git repository and production CLI flows.
The separate in-process alias proof verifies real alias insertion and resolution
at 16/64/256 active tools. There is no baseline comparison, no percentage
claim, no manual SQLite/WAL/SHM manipulation, and no Windows-runtime claim.

### Regression chronology and root-cause closure

During focused Stage027 proof, a reproducible P1 occurred: the second short
macOS tool process sometimes ended with `SIGKILL`, while the first succeeded.
Instrumentation showed an empty live process tree and no signal call, excluding
process-tree cleanup as the cause. A native reproducer identified macOS endpoint
security rejecting repeated execution through freshly created hardlinks.

The fix anchors execution with `fclonefileat` from an identity-checked source
fd into an opened tool-root fd. Only `ENOTSUP`/`EXDEV` permits a bounded fd copy;
the copy has a byte cap, preserves mode, fsyncs, resets source offset, validates
device/inode/size/mode/mtime/ctime, and removes every temporary snapshot on all
outcomes. Cleanup remains bounded to the Darwin process group, tracks escaped
descendants, and never signals the already-finished root. Repeated loops and
the full locked suite then passed; final P1/P2 is `0/0`.

## Upgrade, data, and privacy boundaries

Native macOS proof covers fresh and mixed supported upgrade forms, operational
`004` and observability V2 migration detection, recovery staging, apply-once
behavior, adapter installation, doctor/verify, task flow, and export. It uses
isolated fixtures and preserves the rollback/home data boundary. RC5 does not
auto-retry an apply, delete backups, manually manipulate SQLite/WAL/SHM,
require admin/WSL/source builds, or launch Codex.

Privacy negative tests establish that raw task query, raw chat, raw command
output, hidden reasoning, and raw tagged test values are not durable lifecycle
or exported evidence. Redaction uses the exact marker
`<TEST_SECRET_REDACTED>`. The report intentionally contains no raw dogfood
text, secret, credential, or hidden reasoning.

## Release assets and supply-chain boundary

The bundle is flat and contains exactly the two binaries and checksum manifest.

| File | SHA-256 | Verification |
|---|---|---|
| `dist/aopmem-darwin-arm64` | `594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e` | Mach-O 64-bit arm64 |
| `dist/aopmem-windows-x86_64.exe` | `150db4699c2f41c6e529f9606ac099c9ac6b4771b5084952f2cb5df3226d1b58` | PE32+ console x86-64; two unchanged-source build hashes match |
| `dist/SHA256SUMS` | `6236d2cf502df5036609f202f541e38a12173321a0a85fbc83e388ed4548213a` | verifies both assets |

Windows import inspection is limited to system DLLs: `KERNEL32.dll`,
`shell32.dll`, `api-ms-win-core-synch-l1-2-0.dll`, `bcryptprimitives.dll`,
`WS2_32.dll`, `userenv.dll`, `ntdll.dll`, and `advapi32.dll`. It has no
`VCRUNTIME`, `MSVCP`, `UCRTBASE`, or `api-ms-win-crt` import. This proves the
cross-built artifact boundary only; it does not replace native dogfood.

## Stage29 global audit

The independent audit passed all fifteen sweeps: governance; task/private
lifecycle; block/Keeper/adapter; secrets; tools; observability/UI; Windows
publish; repair/marker durability; capsule privacy; upgrade/installers; agent
dogfood; performance; full regression commands; docs/DoD ownership; and
assets/excluded-scope drift. Each sweep records its files/tests and reports no
open P1 or P2 in `RC5_GLOBAL_AUDIT_REPORT.md`.

## Final required local command evidence

| Command or proof | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo build --locked` | PASS |
| `cargo test --locked` | PASS `768/768` |
| `cargo test --tests --locked` | PASS `768/768` |
| `scripts/dev_verify.sh` | PASS |
| `scripts/audit_v020_installers.sh` | PASS `14` groups |
| `scripts/prove_rc5_macos.sh` | PASS |
| RC5 benchmark checksum manifest | PASS `4/4` |
| agent-evidence checksum manifest | PASS `10/10` |
| `dist` checksum manifest | PASS `2/2` |
| `git diff --check` and ledger JSON parse | PASS |

## Definition of Done: 32/32 local closure

| DoD | Requirement(s) | Local evidence | Result |
|---:|---|---|---|
| 1 | TSK-001 | TP-01 | PASS |
| 2 | TSK-001, TSK-007 | TP-04..08 | PASS |
| 3 | TSK-001, TSK-008 | TP-09 | PASS |
| 4 | TSK-004 | TP-02/03 continuation proof | PASS |
| 5 | BLK-001 | MB-01..04 | PASS |
| 6 | KPR-001, BLK-002 | MK-01/02, MB-05 | PASS |
| 7 | DOG-002 | DOG-01..10 starts | PASS |
| 8 | DOG-002 | DOG-01..10 applies | PASS |
| 9 | KPR-002, GOL-001 | MK-03..07, DOG-01..10 | PASS |
| 10 | SEC-001 | SEC-01 | PASS |
| 11 | SEC-001 | SEC-01 negative refusal proof | PASS |
| 12 | SEC-002 | SEC-02/03 | PASS |
| 13 | SEC-003 | SEC-04..06, canary scan | PASS |
| 14 | TOL-001 | TOOL contract/drift tests | PASS |
| 15 | ALS-001, ALS-002 | TOOL-01..05 | PASS |
| 16 | DUP-001, CGD-001 | TOOL-06..08 | PASS |
| 17 | DUP-002, ALS-002 | TOOL-08..10 | PASS |
| 18 | DUP-003 | TOOL-11 | PASS |
| 19 | WIN-001, WIN-002 | WIN-02..06 | PASS |
| 20 | AUD-001 | WIN-07 | PASS |
| 21 | AUD-002 | WIN-08 | PASS |
| 22 | DBG-001 | WIN-09/11 | PASS |
| 23 | PLT-001 | WIN-01 | PASS |
| 24 | UPG-001, UPG-002 | UPG-01..08 | PASS |
| 25 | UPG-001 | supported-source matrix | PASS |
| 26 | UPG-003 | UPG-09..11 | PASS |
| 27 | UPG-003 | Stage24 macOS proof | PASS |
| 28 | REL-001, CMD-001 | assets, hash, import proof | PASS |
| 29 | TST-002 | final command suite | PASS |
| 30 | SWP-001 | Stage29 sweeps 1–8 | PASS |
| 31 | SWP-001 | Stage29 sweeps 9–15 | PASS |
| 32 | STOP-001, CMD-001 | this report and stop proof | PASS |

The canonical reverse map remains in `RC5_REQUIREMENTS_MATRIX.md`; no DoD item
is skipped. The Stage026–030 cumulative audit verified the local matrix
closure.

## Stop condition, risks, and operator boundary

Local implementation stop condition is met: required local checks and
artifacts pass, the RC report and matrix closure exist, Stage029 found P1 `0`
and P2 `0`, and the native Windows limitation is explicit. The local tree must
now stop changing product scope.

The only real remaining risk is native Windows runtime behavior. Operator work
is the documented Windows 11 x64 PowerShell 5.1 dogfood, with a controlled
fixture and retained backups. Do not delete backups or perform a real Windows
install as part of this release-candidate conclusion. Do not mutate an older
tag. Commit/push/tag/GitHub Release remain separate authorized release actions
after the Stage026–030 cumulative audit passes; none is claimed here.
