# AOPMem v0.2.0-rc5 Requirements Matrix

Status: `VERIFIED`

Source: supplied RC5 specification, sections 1–34.

Status values:

- `DONE_LOCAL_CHECKS_PASSED`: implemented as Stage 01/02 planning evidence;
- `FROZEN`: contract fixed, implementation pending;
- `TODO`: implementation and proof pending;
- `VERIFIED`: available only after a cumulative milestone audit.
- `COMPLETE_LOCAL_RELEASE_READY`: local release boundary is complete after the
  Stage026–030 cumulative audit; external release actions remain separate.

Final cumulative status: `COMPLETE_LOCAL_RELEASE_READY`. Requirements owned
by Stages 026–030 below are `VERIFIED`; `RC5-STOP-002` remains an external
release-action contract and is intentionally not executed by local audit.

## Requirement matrix

| ID | Contract | Source | Owner | Verification | Proof / product doc | Status |
|---|---|---:|---:|---|---|---|
| RC5-FLD-001 | Record all field statements in 16 finding rows and map each to RC5 closure work | 1 | 01 | field table completeness | `RC5_FIELD_FINDINGS.md` | VERIFIED |
| RC5-FLD-002 | Exclude corrected shell/helper harness failures from product defects | 1 | 01, 29 | drift sweep | field findings, global audit | DONE_LOCAL_CHECKS_PASSED |
| RC5-GOL-001 | Enforce explicit memory writes, task retrieval before action, actual context use, no repeated learned failure/tool, no reminder, no manual SQLite | 2 | 05–08, 13–15, 25, 30 | task protocol and 10 dogfood scenarios | task protocol, compliance report, RC report | COMPLETE_LOCAL_PENDING_CUMULATIVE_AUDIT |
| RC5-GOV-001 | Apply BDUF-light, KISS, YAGNI, DoR, Thin Slice, Proof First, least surprise, fail fast, least privilege, risk tests, regression, drift, reproducible proof | 3 | 01–30 | every handoff and Stage 29 sweep | proof log, global audit | COMPLETE_LOCAL_PENDING_CUMULATIVE_AUDIT |
| RC5-ARC-001 | Preserve one Rust crate, SQLite/FTS5, typed graph, local workspaces/home, optional retrieval helpers, native Keeper, explicit writes, separate observability, read-only UI | 4 | 01–30 | architecture drift scan | global audit | COMPLETE_LOCAL_PENDING_CUMULATIVE_AUDIT |
| RC5-TSK-001 | Add `task start`, `task apply`, and `task complete` JSON commands | 5 | 04–06 | TP-01, TP-09 | `docs/TASK_START_PROTOCOL.md` | DONE_LOCAL_CHECKS_PASSED |
| RC5-TSK-002 | Start resolves workspace, opens operational DB read-only, fully loads active kernel contract, gates, project profile, source hierarchy, and rules | 5.1 | 05 | TP-01, TP-10 | task protocol, Stage 05 proof | VERIFIED |
| RC5-TSK-003 | Retrieve typed roots, FTS/BM25, direct links, depth graph, workflows, tools, corrections, failure modes; order by source/trust/confidence; dedupe | 5.1 | 05 | TP-01, TP-02 | task protocol, Stage 05 proof | VERIFIED |
| RC5-TSK-004 | Consume continuation inside one invocation until complete or bounded budget; no ordinary manual cursor loop | 5.1 | 05 | TP-02, TP-03 | task protocol, performance proof | VERIFIED |
| RC5-TSK-005 | Return all required IDs, revision/completeness flags, mandatory/task nodes, selected categories, hunches, and reasons in one final package | 5.1 | 05 | JSON golden contract | task protocol | VERIFIED |
| RC5-TSK-006 | Never persist raw query; allow only fingerprint/redacted summary and bounded factual IDs/counts/durations/codes; overflow and unavailable memory fail closed | 5.1 | 04–05, 10 | TP-03, TP-10, TP-12, SEC-04 | task protocol, secret doc | COMPLETE_LOCAL_PENDING_CUMULATIVE_AUDIT |
| RC5-TSK-007 | Apply accepts selected IDs/none-relevant and validates workspace, bundle, revision, membership, active status; writes observability only | 5.2 | 06 | TP-04..TP-08, operational DB fingerprint | task protocol | DONE_LOCAL_CHECKS_PASSED |
| RC5-TSK-008 | Complete records only result, duration, factual code, accepted workflow/tool IDs, bounded redacted reason; keeps feedback separate | 5.3 | 06 | TP-09, privacy scan | task protocol | DONE_LOCAL_CHECKS_PASSED |
| RC5-KPR-001 | Keeper is native subagent, receives exact request/root/shell/instruction file, reads no project code first, and fails `MEMORY_KEEPER_UNAVAILABLE` without shell fallback | 6 | 07 | MK-01, MK-02, dogfood ordering | `docs/MEMORY_KEEPER_V2.md` | DONE_LOCAL_CHECKS_PASSED |
| RC5-KPR-002 | Keeper runs start, checks completeness/budget/bundle/revision, selects context, then runs apply; never uses normal `recall --full` | 6 | 07 | MK-03..MK-06 | Memory Keeper doc and skill | DONE_LOCAL_CHECKS_PASSED |
| RC5-KPR-003 | Keeper returns compact receipt with IDs, constraints, workflow/tools/corrections/source order; reuses task only within same goal | 6–7.2 | 07–08, 25 | MK-07, DOG-09, DOG-10 | Memory Keeper doc, compliance report | VERIFIED |
| RC5-BLK-001 | Install contract version 2, 18 required sections, useful 100–180-line target, hard maximum 24 KiB | 7 | 03, 08 | MB-01..MB-04 | managed-block template | DONE_LOCAL_CHECKS_PASSED |
| RC5-BLK-002 | Hard gate covers new chat, compaction, pause, new task/goal and every listed substantive action; before receipt allow only instructions/root/shell/Keeper; work silently | 7.1 | 03, 08, 25 | MB-05, dogfood 10/10 | managed-block template, compliance report | VERIFIED |
| RC5-BLK-003 | Reuse receipt for clarification/continuation/same-goal correction; start new task for independent goal/project/work type or unreliable receipt | 7.2 | 03, 07–08, 25 | DOG-09, DOG-10 | managed block, Memory Keeper doc | VERIFIED |
| RC5-RET-001 | Preserve exact nine-step source order; Codebase Memory is retrieval aid; current files are final technical truth; read only relevant files | 8 | 03, 07–08 | textual contract and dogfood order | managed block, Memory Keeper doc | DONE_LOCAL_CHECKS_PASSED |
| RC5-SEC-001 | Remove blanket ban; permit user-authorized/test/VDI/test-contour credentials without refusal, placeholder coercion, lecture, or value removal | 9.1 | 09 | SEC-01 | `docs/SECRET_HANDLING.md` | DONE_LOCAL_CHECKS_PASSED |
| RC5-SEC-002 | Never auto-persist exact secret; permit exact local persistence only by explicit remember/teach/save trigger, without extra `+++`; no secrets platform | 9.2 | 09–10 | SEC-02, SEC-03 | secret doc | DONE_LOCAL_CHECKS_PASSED |
| RC5-SEC-003 | Redact exact secrets in observability, capsule, evidence, reports, errors, audit metadata/snapshot, and task summaries with `<TEST_SECRET_REDACTED>` | 9.3 | 10, 19–20 | SEC-04..SEC-06 and deterministic canary sweep | secret doc, capsule doc | VERIFIED |
| RC5-SEC-004 | Permit designated test-system transmission/authentication; require `+++` by action class, not secret presence; use no real credentials in tests | 9.4 | 09–10 | SEC-07, SEC-08 | secret doc | DONE_LOCAL_CHECKS_PASSED |
| RC5-TOL-001 | Model one agent-only capability as one canonical tool ID plus display name, aliases, and in-contract platform launchers; forbid user/internal/platform/wrapper duplicates | 10 | 13–15 | tool contract and drift tests | aliases/dedupe doc | VERIFIED |
| RC5-ALS-001 | Migration `004_task_protocol_and_tool_aliases` adds direct `tool_aliases` with required fields, uniqueness, canonical existence, no chain/cycle/directory/contract/copy | 11 | 11 | TOOL-01..TOOL-03, migration matrix | `docs/TOOL_ALIASES_AND_DEDUPLICATION.md` | VERIFIED |
| RC5-ALS-002 | Add alias CRUD/resolve; get/run/validate resolve canonical contract and path; list canonical by default and optionally alias rows; emit alias facts | 11 | 13 | TOOL-01, TOOL-04, TOOL-05 | aliases doc | VERIFIED |
| RC5-DUP-001 | Compute canonical fingerprint from behavioral/runtime/layout/hash/launcher fields, excluding identity/status/time/cosmetic description; classify five duplicate types | 12 | 12 | TOOL-06..TOOL-08 | dedupe report and doc | VERIFIED |
| RC5-DUP-002 | `dedupe plan` is deterministic/read-only; exact-only apply aliases old IDs, supersedes duplicates, preserves directories/executables, and reports overlaps | 12 | 12, 14 | TOOL-08..TOOL-10, FS/DB unchanged plan proof | dedupe report | VERIFIED |
| RC5-DUP-003 | Select canonical by exact five-step order; generic Confluence fixture yields `confluence_reader`, old ID alias, one active implementation, no hardcoded branch | 12 | 14–15 | TOOL-11 | dedupe report | VERIFIED |
| RC5-CGD-001 | Before create-draft, search registry/aliases/fingerprint/implementation/descriptions; block exact duplicate without writes; require review for overlap; create only after user request/consent | 13 | 13, 15 | create preflight negative suite | aliases doc, managed block | VERIFIED |
| RC5-OBS-001 | Migrate Local Observability to v2 and add exact task, duplicate, alias, repair, and platform factual events | 14 | 04, 11–20 | fresh/v1 migration and event transition tests | `docs/LOCAL_OBSERVABILITY.md` | VERIFIED |
| RC5-OBS-002 | Report task/application/completion/context/tool/audit facts, unresolved overlaps, pending state, last repair; no score; retain 30 days or 100 MB | 14 | 23 | observability report/retention suite | local observability doc | VERIFIED |
| RC5-UI-001 | Add only effectiveness and tools facts: lifecycle/context, missing apply, duplicates, aliases, superseded tools, overlaps, pending audit | 15 | 23 | UI API/view fixtures | local observability doc | VERIFIED |
| RC5-UI-002 | Keep UI local, read-only, desktop-only, exact `127.0.0.1`, token protected, no CDN or write endpoints | 15 | 23, 29 | HTTP security and no-write fingerprint tests | local observability doc | DONE_LOCAL_CHECKS_PASSED |
| RC5-WIN-001 | Audit error 87 root cause and route backup, audit snapshot, and capsule through one shared Windows publish boundary | 16 | 16–17 | WIN-04, call-path/source audit | Windows publish report | VERIFIED |
| RC5-WIN-002 | Same-parent/reparse/handle/flush guards; ReplaceFileW replace and MoveFileExW no-replace; reopen/caller validation; all structured error fields; no workaround/framework/admin | 17 | 17 | WIN-02..WIN-06 | Windows publish report | VERIFIED |
| RC5-PLT-001 | `platform check --json` tests create/flush/no-replace/replace/reopen/cleanup/reparse only in private temp; no memory/workspace/admin; failure blocks update unchanged | 18 | 18, 22 | WIN-01, failed-check home fingerprint | `docs/PLATFORM_CHECK.md` | VERIFIED |
| RC5-AUD-001 | Repair current/all workspaces with lock, read-only DB, streaming redacted SQL, shared publish, validation, Git commit, observability, idempotency | 19 | 19 | WIN-07, WIN-08, DB fingerprint | `docs/WINDOWS_AUDIT_REPAIR.md` | VERIFIED |
| RC5-AUD-002 | Clear pending marker only after full per-workspace success; support already-clean no-op; never manual delete; doctor/verify emit exact repair hint | 19 | 19 | failure injection and hint tests | audit repair doc | VERIFIED |
| RC5-DBG-001 | Export uses shared publish and preserves deterministic no-overwrite redacted read-only no-self-write ZIP; cover TEMP/non-ASCII/existing/long/error-87 paths | 20 | 20 | WIN-09, WIN-11 | `docs/DEBUG_CAPSULE.md` | VERIFIED |
| RC5-UPG-001 | Support v0.1.0-rc3, compatible noncanonical v0.1, rc1–rc4, side-by-side rc4, mixed `001/003`; target operational `004` and obs v2 | 21 | 21, 24 | UPG-01..UPG-08 plus native mixed proof | `docs/UPGRADE_TO_RC5.md`, RC5 macOS proof report | VERIFIED |
| RC5-UPG-002 | Enforce exact process/backup/download/check/repair/prepare/plan/one-apply/publish/adapter/repair/doctor/verify/task/observe/export order | 21 | 21–22, 24 | command-order spies, failure matrix, native trace | upgrade doc, installers, RC5 macOS proof report | VERIFIED |
| RC5-UPG-003 | No onboarding, auto-retry, backup deletion, manual SQLite/WAL/SHM, admin, WSL, source build, or Codex launch; unknown hash warning only; preserve rollback homes and data | 21 | 21–22, 24 | UPG-09..UPG-11, isolated hashes, native failure proof | upgrade/native docs, RC5 macOS proof report | VERIFIED |
| RC5-ADP-001 | Detect v1/v2, replace markers only, preserve user/custom approval text, remove old secret ban, install gate, remain idempotent and nonduplicating | 22 | 08, 22 | MB-06..MB-08 | template and installer tests | VERIFIED |
| RC5-ADP-002 | Support Codex, Claude, Cursor, Copilot; change exactly one explicitly active instruction file | 22 | 08, 22 | MB-09 | adapter/installer proof | VERIFIED |
| RC5-DOG-001 | Run all 10 clean native-subagent scenarios in isolated temporary workspace; prove ordering, apply, context use, secret use, tool reuse, and task boundary | 23 | 25 | DOG-01..DOG-10 | agent compliance report | VERIFIED |
| RC5-DOG-002 | Reach 10/10 start-before-action, 0 reminders, 0 duplicates, 0 blanket refusals; retain privacy-safe action/receipt transcripts only | 23 | 25 | DOG-01..DOG-10 checksum and aggregate scan | agent compliance report | VERIFIED |
| RC5-TST-001 | Implement every focused task, block, secret, tool, Windows, repair, export, upgrade, observability, and UI test group | 24 | 05–27 | TP/MB/SEC/TOOL/WIN/UPG catalogs | proof log, RC5 regression report | VERIFIED |
| RC5-TST-002 | Preserve prior behavior and pass fmt, clippy, build, both locked test runs, dev verify, installer audit, and diff check | 24, 29 | 27–30 | required command log | proof log, global audit | VERIFIED |
| RC5-PERF-001 | Measure only RC5 overhead for start sizes/apply/complete/dedupe/alias/repair/export/check; no unsupported percentage; bounded scans; no normal-run N+1 hashes | 25 | 26 | reproducible raw median/p95 benchmark | performance report, raw evidence, proof log | VERIFIED |
| RC5-DERC-001 | Execute exact finite 30-stage graph with a new medium implementation agent per stage and high cumulative audit after each fifth stage | 26–27 | 01–30 | ledger/handoff consistency | execution ledger and proof log | VERIFIED |
| RC5-DERC-002 | Continue between normal stages; AUTO_PATCH only adjacent wiring, at most 3 production files, proof and re-audit; BLOCKED only for listed real blockers | 26 | 01–30 | audit patch drift checks | ledger, proof, global audit | VERIFIED |
| RC5-SWP-001 | Stage 29 runs 15 separate PASS/FAIL sweeps with evidence/files/tests/findings and ends P1=0, P2=0 | 28 | 29 | sweep completeness validator | global audit report | VERIFIED |
| RC5-CMD-001 | Run all required Cargo/scripts/diff commands and focused suites; build/check both assets; keep native Windows runtime pending | 29 | 27–30 | command and asset logs | proof/global audit | VERIFIED |
| RC5-DOC-001 | Create all ten required `.devplan` RC5 artifacts | 30 | 01–30 | path and content audit | `.devplan/RC5_*` | VERIFIED |
| RC5-DOC-002 | Create six required product docs and update managed block, Keeper, installers, Windows, observability, and capsule docs | 30 | 03–28 | path/content/drift audit | `docs/**`, `templates/**`, `install/**` | VERIFIED |
| RC5-REL-001 | Set `0.2.0-rc5`; build Darwin arm64, Windows PE32+ x86-64, and verified SHA256SUMS with dependency proof | 31 | 28 | asset commands and two-build hash proof | RC5 Stage 028 handoff | VERIFIED |
| RC5-DOD-001 | Prove every one of the 32 Definition of Done items | 32 | 30 | final matrix and RC report | final matrix and RC report | VERIFIED |
| RC5-OOS-001 | Add none of 21 excluded platforms, services, backends, automation, managers, histories, dashboards, or domain packs | 33 | 29 | forbidden-feature drift scan | global audit | VERIFIED |
| RC5-STOP-001 | Stop local implementation only after checks, audit, assets, RC report, P1=0, P2=0; still forbid real Windows install, backup deletion, and older-tag mutation | 34 | 30 | stop checklist | RC report | VERIFIED |
| RC5-STOP-002 | After 100% local proof, create one commit, push current branch, create new `v0.2.0-rc5` tag, push it, and create GitHub Release as explicitly authorized | user amendment | release | remote Git/tag/release verification | RC report and release URL | FROZEN |

## Decision-to-requirement map

| Decisions | Requirements |
|---|---|
| `RC5-D-001..003` | `RC5-FLD-*`, `RC5-ARC-*`, `RC5-DERC-*` |
| `RC5-D-004..018` | `RC5-TSK-*`, `RC5-KPR-*`, `RC5-OBS-*` |
| `RC5-D-019..021` | `RC5-BLK-*`, `RC5-ADP-*` |
| `RC5-D-022..025` | `RC5-SEC-*`, `RC5-AUD-*`, `RC5-DBG-*`, `RC5-UPG-*` |
| `RC5-D-026..031` | `RC5-TOL-*`, `RC5-ALS-*`, `RC5-DUP-*`, `RC5-CGD-*` |
| `RC5-D-032` | `RC5-OBS-*`, `RC5-ALS-*`, `RC5-UPG-*` |
| `RC5-D-033..036` | `RC5-WIN-*`, `RC5-PLT-*`, `RC5-AUD-*`, `RC5-DBG-*` |
| `RC5-D-037..040` | `RC5-UPG-*`, `RC5-ADP-*` |
| `RC5-D-041` | `RC5-DOG-*`, `RC5-SEC-*` |
| `RC5-D-042` | `RC5-PERF-*` |
| `RC5-D-043` | `RC5-CMD-*`, `RC5-REL-*`, `RC5-STOP-*` |
| `RC5-D-044` | `RC5-DERC-*` |
| `RC5-D-045` | `RC5-SWP-*`, `RC5-DOD-*` |
| `RC5-D-046..047` | `RC5-STOP-*` |

## Focused test catalog

| ID | Required check |
|---|---|
| TP-01 | start returns complete mandatory context |
| TP-02 | continuation completes internally |
| TP-03 | budget exhaustion explicit |
| TP-04 | stale revision rejected |
| TP-05 | wrong workspace rejected |
| TP-06 | unknown applied node rejected |
| TP-07 | node outside bundle rejected |
| TP-08 | deprecated/superseded node rejected |
| TP-09 | complete lifecycle and idempotency |
| TP-10 | missing memory fails closed |
| TP-11 | observability projection failure isolation |
| TP-12 | raw query absent from persistence |
| MB-01 | canonical template parity and contract v2 |
| MB-02 | 18 sections, hard gate, source order |
| MB-03 | no blanket secret ban or user-facing tool model |
| MB-04 | line/byte limits |
| MB-05 | exact substantive/pre-receipt/task-boundary wording |
| MB-06 | exact adapter sync |
| MB-07 | user/custom approval text preserved |
| MB-08 | repeated sync idempotent |
| MB-09 | only active adapter changed |
| MK-01 | native subagent invoked before substantive action |
| MK-02 | unavailable native subagent fails exact |
| MK-03 | start called with secure query input |
| MK-04 | completeness/budget/revision validated |
| MK-05 | selected context applied |
| MK-06 | no full recall or shell fallback |
| MK-07 | compact receipt complete |
| SEC-01 | test password/token usable |
| SEC-02 | explicit remember may persist exact value |
| SEC-03 | automatic memory write does not persist exact value |
| SEC-04 | observability redacts |
| SEC-05 | export/audit snapshot redact |
| SEC-06 | error output redacts |
| SEC-07 | authentication needs no `+++` |
| SEC-08 | external write still needs `+++` |
| TOOL-01 | alias add/list/remove/resolve |
| TOOL-02 | cycle blocked |
| TOOL-03 | alias-to-alias blocked |
| TOOL-04 | runner/get/validate resolve canonical ID and path |
| TOOL-05 | list canonical by default |
| TOOL-06 | exact duplicate blocked |
| TOOL-07 | same implementation detected |
| TOOL-08 | possible overlap not auto-merged |
| TOOL-09 | exact dedupe preserves old ID as alias |
| TOOL-10 | no directory or executable deletion |
| TOOL-11 | Confluence fixture selects `confluence_reader` generically |
| WIN-01 | private-temp platform check |
| WIN-02 | replace-existing publish |
| WIN-03 | no-replace publish |
| WIN-04 | error 87 regression |
| WIN-05 | non-ASCII/long normal paths |
| WIN-06 | handles closed before publish |
| WIN-07 | marker clears only after success |
| WIN-08 | repair idempotent |
| WIN-09 | export publish succeeds |
| WIN-10 | backup publish succeeds |
| WIN-11 | no manual copy or apply retry |
| UPG-01 | schema `001` source |
| UPG-02 | schema `003` source |
| UPG-03 | mixed two-workspace source |
| UPG-04 | operational migration `004` |
| UPG-05 | exact logical/byte data preservation |
| UPG-06 | tool aliases preserved |
| UPG-07 | observability v1 to v2 migration |
| UPG-08 | pending audit marker repaired |
| UPG-09 | failed platform check changes nothing |
| UPG-10 | failed apply retains backups |
| UPG-11 | binary publishes only after apply |
| DOG-01..DOG-10 | exact ten clean-agent scenarios from section 23 |

## Required artifact ownership

| Artifact | Producing stage |
|---|---:|
| `.devplan/RC5_FIELD_FINDINGS.md` | 01 |
| `.devplan/RC5_FINAL_DECISION_LOG.md` | 02 |
| `.devplan/RC5_REQUIREMENTS_MATRIX.md` | 02, finalized 30 |
| `.devplan/RC5_EXECUTION_LEDGER.json` | 01–30 |
| `.devplan/RC5_PROOF_LOG.md` | 01–30 |
| `.devplan/RC5_GLOBAL_AUDIT_REPORT.md` | 29 |
| `.devplan/RC5_AGENT_COMPLIANCE_REPORT.md` | 25 |
| `.devplan/RC5_WINDOWS_PUBLISH_REPORT.md` | 16–20 |
| `.devplan/RC5_TOOL_DEDUPE_REPORT.md` | 12–15 |
| `.devplan/RELEASE_CANDIDATE_v0.2.0-rc5.md` | 30 |
| `docs/TASK_START_PROTOCOL.md` | 03–06 |
| `docs/MEMORY_KEEPER_V2.md` | 07 |
| `docs/SECRET_HANDLING.md` | 09–10 |
| `docs/TOOL_ALIASES_AND_DEDUPLICATION.md` | 11–15 |
| `docs/WINDOWS_AUDIT_REPAIR.md` | 16–20 |
| `docs/UPGRADE_TO_RC5.md` | 21–22 |

## Definition of Done reverse map

| DoD | Requirement |
|---:|---|
| 1 | `RC5-TSK-001` |
| 2 | `RC5-TSK-001`, `RC5-TSK-007` |
| 3 | `RC5-TSK-001`, `RC5-TSK-008` |
| 4 | `RC5-TSK-004` |
| 5 | `RC5-BLK-001` |
| 6 | `RC5-KPR-001`, `RC5-BLK-002` |
| 7 | `RC5-DOG-002` |
| 8 | `RC5-DOG-002` |
| 9 | `RC5-KPR-002`, `RC5-GOL-001` |
| 10 | `RC5-SEC-001` |
| 11 | `RC5-SEC-001` |
| 12 | `RC5-SEC-002` |
| 13 | `RC5-SEC-003` |
| 14 | `RC5-TOL-001` |
| 15 | `RC5-ALS-001`, `RC5-ALS-002` |
| 16 | `RC5-DUP-001`, `RC5-CGD-001` |
| 17 | `RC5-DUP-002`, `RC5-ALS-002` |
| 18 | `RC5-DUP-003` |
| 19 | `RC5-WIN-001`, `RC5-WIN-002` |
| 20 | `RC5-AUD-001` |
| 21 | `RC5-AUD-002` |
| 22 | `RC5-DBG-001` |
| 23 | `RC5-PLT-001` |
| 24 | `RC5-UPG-001`, `RC5-UPG-002` |
| 25 | `RC5-UPG-001` |
| 26 | `RC5-UPG-003` |
| 27 | `RC5-UPG-003`, Stage 24 proof |
| 28 | `RC5-REL-001`, `RC5-CMD-001` |
| 29 | `RC5-TST-002` |
| 30 | `RC5-SWP-001` |
| 31 | `RC5-SWP-001` |
| 32 | `RC5-STOP-001`, `RC5-CMD-001` |

## Source-section coverage

| Sections | Requirement IDs |
|---:|---|
| 1 | `RC5-FLD-*` |
| 2 | `RC5-GOL-*` |
| 3 | `RC5-GOV-*` |
| 4 | `RC5-ARC-*` |
| 5 | `RC5-TSK-*` |
| 6 | `RC5-KPR-*` |
| 7 | `RC5-BLK-*` |
| 8 | `RC5-RET-*` |
| 9 | `RC5-SEC-*` |
| 10 | `RC5-TOL-*` |
| 11 | `RC5-ALS-*` |
| 12 | `RC5-DUP-*` |
| 13 | `RC5-CGD-*` |
| 14 | `RC5-OBS-*` |
| 15 | `RC5-UI-*` |
| 16 | `RC5-WIN-001` |
| 17 | `RC5-WIN-002` |
| 18 | `RC5-PLT-*` |
| 19 | `RC5-AUD-*` |
| 20 | `RC5-DBG-*` |
| 21 | `RC5-UPG-*` |
| 22 | `RC5-ADP-*` |
| 23 | `RC5-DOG-*` |
| 24 | `RC5-TST-*` and focused test catalog |
| 25 | `RC5-PERF-*` |
| 26 | `RC5-DERC-002` |
| 27 | `RC5-DERC-001` |
| 28 | `RC5-SWP-*` |
| 29 | `RC5-CMD-*` |
| 30 | `RC5-DOC-*` |
| 31 | `RC5-REL-*` |
| 32 | `RC5-DOD-*` and reverse map |
| 33 | `RC5-OOS-*` |
| 34 | `RC5-STOP-*` |

Coverage result:

- source sections covered: `34/34`;
- source field statements mapped: `17/17` in 16 finding rows;
- DoD items reverse-mapped: `32/32`;
- required documents assigned: `16/16`;
- unresolved requirement owner: `0`;
- unresolved product/security decision: `0`.
