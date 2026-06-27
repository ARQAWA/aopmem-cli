# AOPMem v0.1 Global Audit Report

## Verdict

`PASS`

Build, tests, local CLI contract probes, SQLite schema, temp E2E proof,
adapter block, reflection, tool runner approval, artifacts cleanup, and audit
SQL snapshot proof passed.

Post-audit update: `GA-001` is resolved by final decision update.
`configured_unverified` is now accepted for enabled optional MCP capabilities
that are agent-local, host-global, shell-managed, or otherwise outside
deterministic CLI detection. No product code changed.

## Executive Summary

AOPMem is a working single-crate Rust CLI. Runtime data is under user-level
`~/.aopmem` or `AOPMEM_HOME`, not inside the target repo. Memory is SQLite-only
with FTS5/BM25. No Mem0, Hindsight, Qdrant, vector/semantic backend, custom MCP
server, CI workflow, old-MVP import, background enrichment, or markdown memory
export/import implementation drift was found.

The original release risk was optional MCP status. That risk is now resolved by
contract clarification: `configured_unverified` is a valid, non-blocking
status when the CLI has no reliable detector. The CLI must still not fake
`installed` without deterministic evidence.

## Commands Run

See `.devplan/GLOBAL_AUDIT_COMMANDS.log`.

## DERC State

| Check | Status | Evidence |
|---|---:|---|
| Current stage final | PASS | `.devplan/CURRENT_STAGE.md`: `STAGE_055_COMPLETE` |
| Ledger JSON valid | PASS | `python3 -m json.tool` passed |
| Stages 001-055 present | PASS | `55`, range `001..055` |
| All stages verified | PASS | unique status: `VERIFIED` |
| blocked_count | PASS | JSON has `blocked=false`; derived count `0` |
| verified_through_stage | PASS | `STAGE_055` |
| next audit | PASS | `none` |
| STAGE_055 proof | PASS | `STAGE_055_MILESTONE_AUDIT` exists and passed |
| Open blockers | PASS | ledger has no active blocker; blockers file is resolved history |
| Final handoff | PARTIAL | Stage 055 handoff still says verified-through remains `STAGE_050`; see `GA-003` |

## Requirement Compliance Table

| Requirement | Status | Evidence | Finding ID |
|---|---:|---|---|
| Separate Rust CLI repo | PASS | `Cargo.toml`, `src/main.rs` |  |
| macOS ARM v0.1 | PASS | `scripts/build_macos_arm.sh`; `dist/.../aopmem` is Mach-O arm64 |  |
| One Rust crate | PASS | single package `aopmem` |  |
| Dependencies justified | PASS | `DEPS_JUSTIFICATION.md` |  |
| No CI/GitHub Actions | PASS | no `.github` or YAML workflow files |  |
| Local checks only | PASS | `cargo build`, `cargo test`, CLI proof |  |
| Global install user-level backend | PASS | install prompt targets `~/.aopmem/bin`; runtime resolver uses `~/.aopmem`/`AOPMEM_HOME` |  |
| Runtime storage under `~/.aopmem` | PASS | temp workspace under `AOPMEM_HOME/workspaces/repo-main-4c46e55f` |  |
| No heavy runtime in target repo | PASS | temp repo had no `.aopmem` |  |
| Only managed adapter block in repo | PASS | adapter seed/sync changed only marked block |  |
| `.understand.docs` optional | PASS | absent when disabled, present when enabled |  |
| Memory only in SQLite | PASS | memory tables in `aopmem.sqlite` |  |
| No markdown memory import/export | PASS | drift scan docs-only |  |
| No semantic/vector search | PASS | no deps/tables/backend; only FTS5/BM25 |  |
| SQLite FTS/BM25 | PASS | `fts_nodes`; `bm25(fts_nodes)` in storage |  |
| Typed nodes/links/events/registries | PASS | schema and CLI probes |  |
| Retrieval structure + links + FTS | PASS | E2E recall returned linked node, FTS fallback, hunch |  |
| Memory Keeper contract | PASS | `templates/skills/memory-keeper/SKILL.md` |  |
| No Mem0/Hindsight | PASS | drift scan |  |
| No custom MCP server | PASS | registry/profile only |  |
| No old MVP migration/import | PASS | only schema migrations; no old-MVP import path |  |
| No background enrichment | PASS | no daemon/background loop found |  |
| No current_state/task history | PASS | docs-only out-of-scope mentions |  |
| Hunch 1-3 with source node | PASS | E2E `hunches=1`; code cap `MAX_HUNCHES=3` |  |
| Tool runner via `aopmem tool run` | PASS | E2E used runner only |  |
| Generated tools not direct agent calls | PASS | contract/docs say runner; E2E followed it |  |
| Tool contract fields | PASS | `tool.json`, SQLite registry, validate/list/get/run |  |
| Side-effect dry-run | PASS | `aopmem tool run <id> --dry-run` plans without execution |  |
| External write requires `+++` | PASS | blocked exit 6 without approval; passed with `+++` |  |
| External read no approval | PASS | E2E `external_read` ran with `approval_requirement=none` |  |
| Reflection user-triggered | PASS | only explicit `reflect ...` CLI commands |  |
| Reflection low auto/high draft | PASS | E2E low applied, high drafted |  |
| CLI does not call LLM API | PASS | no LLM/API deps or reflection client code |  |
| Hidden chain-of-thought not stored | PASS | structured proposal/apply records only |  |
| Artifacts files in `YYYY-MM-DD` | PASS | artifacts module and cleanup proof |  |
| Artifacts cleanup 7 days or 1 GB | PASS | constants and cleanup tests/proof |  |
| Audit snapshot `.sql` not DB | PASS | `memory.sql: ASCII text` |  |
| No secret scanner implementation required | PASS | contract-level policy only |  |
| DERC ledger/proof/handoff/recovery | PARTIAL | ledger/proof pass; recovery paths/handoff stale | `GA-002`, `GA-003` |
| Cumulative audit cadence | PASS | milestone audits through `STAGE_055` |  |
| STAGE_001-STAGE_055 VERIFIED | PASS | ledger JSON all 55 `VERIFIED` |  |
| Optional MCP enabled unverifiable status | PASS | final contract accepts `configured_unverified` when no reliable detector exists |  |

## Findings

### GA-001

- Severity: P2
- Area: Install / MCP registry
- Status: RESOLVED BY FINAL DECISION UPDATE
- Description: enabled optional MCP profiles may be recorded as
  `configured_unverified` when the CLI cannot reliably verify agent-local,
  host-global, shell-managed, or otherwise non-deterministic capabilities.
- Evidence:
  - `src/install/mod.rs` calls `optional_mcp_status(enabled, None)`.
  - `optional_mcp_status(true, None)` returns `configured_unverified`.
  - Temp E2E output:
    `codebase-memory-mcp=configured_unverified`,
    `understand-anything=configured_unverified`.
  - Final decision/spec now lists `configured_unverified` as an allowed status.
- Expected: enabled + detector pass -> `installed`; enabled + detector fail ->
  `missing`; enabled + no reliable detector -> `configured_unverified`;
  disabled -> `disabled`.
- Actual: existing runtime behavior matches the clarified contract.
- Recommended patch scope: none for product code.
- Must fix before release: no

### GA-002

- Severity: P3
- Area: Recovery / repo layout
- Description: mandatory recovery files are not at the root paths named by the
  audit request.
- Evidence:
  - root `RUN_FIRST.md` absent;
  - root `reference/` absent;
  - root README is stored as lowercase `readme.md`;
  - full recovery docs exist under
    `aopmem_v0_1_final_orchestrated_pack/`.
- Expected: canonical recovery paths are stable and match audit prompts, or the
  fallback location is explicitly canonical.
- Actual: audit had to use the orchestrated pack fallback.
- Recommended patch scope: documentation/bookkeeping only. Either copy
  canonical docs to root or state that the pack path is canonical.
- Must fix before release: no

### GA-003

- Severity: P3
- Area: DERC handoff
- Description: final Stage 055 handoff does not match the final verified
  state.
- Evidence:
  - `.devplan/HANDOFFS/STAGE_055.md` says verified-through remains
    `STAGE_050`.
  - `.devplan/EXECUTION_LEDGER.json` says `verified_through_stage=STAGE_055`.
  - `.devplan/PROOF_LOG.md` has `STAGE_055_MILESTONE_AUDIT` PASS.
- Expected: latest handoff reflects the final audited state.
- Actual: ledger/proof are final, but the latest handoff text is stale.
- Recommended patch scope: `.devplan/**` bookkeeping only.
- Must fix before release: no

### GA-004

- Severity: INFO
- Area: Git state
- Description: the repository baseline is almost entirely untracked.
- Evidence: `git status --short` shows `.devplan/`, `src/`, `Cargo.toml`,
  `Cargo.lock`, `install/`, `templates/`, `scripts/`, `dist/`, and others as
  untracked.
- Expected: not specified by v0.1 audit scope.
- Actual: final proof treats this as expected baseline.
- Recommended patch scope: none for product behavior. Release packaging may
  want a clean tracked state later.
- Must fix before release: no

## Out-of-scope Drift

No forbidden implementation drift found.

| Term group | Result | Classification |
|---|---:|---|
| Mem0 / Hindsight / Qdrant | docs/scanner hits only | allowed |
| semantic/vector/embedding search | docs, install wording, scanner tests only | allowed |
| custom MCP server | docs only | allowed |
| old MVP import/migration | docs only; DB schema migrations are allowed | allowed |
| background enrichment | docs only | allowed |
| current_state/task history | docs only | allowed |
| markdown memory export/import | docs only | allowed |
| GitHub Actions / CI | no workflow files | pass |

## CLI Proof

Passing:

- `cargo run --quiet -- --help`
- `cargo run --quiet -- doctor --help`
- `cargo run --quiet -- init --help`
- `cargo run --quiet -- recall --help`
- `cargo run --quiet -- tool --help`
- temp JSON probes for `status`, `doctor`, `node get/list/update`,
  `link list`, `recall`, `mcp list/get`, `adapter status`,
  `reflect inventory`, `artifacts cleanup`
- temp E2E for tool create/list/get/validate/run/dry-run and approval policy

JSON mode uses a stable envelope:
`ok`, `command`, `data`, `warnings`, `errors`, `meta`.

Errors are structured. Parse errors under `--json` are also JSON. Human stdout
does not contaminate JSON command stdout; init prompts go to stderr in JSON
mode.

## SQLite Proof

Confirmed tables:

- `schema_migrations`
- `nodes`
- `links`
- `aliases`
- `tags`
- `sources`
- `events`
- `registries`
- `tool_contracts`
- `mcp_profiles`
- `fts_nodes`

Confirmed fields include:

- `nodes.source_ref`
- `nodes.confidence`
- `nodes.trust_level`
- tool side-effect and approval fields
- MCP read/write/side-effect/approval fields

Confirmed behavior:

- migration marker `001|001_init`;
- FTS5 virtual table indexes title, summary, body, aliases;
- BM25 ordering is used in storage search;
- deprecated/superseded nodes are excluded from normal recall and FTS fallback;
- no vector or embedding tables.

Connection pragmas are set in product code:

- `PRAGMA foreign_keys = ON`
- `PRAGMA journal_mode = WAL`
- `PRAGMA busy_timeout = 5000`

## Install/Workspace Proof

Temp proof confirmed:

- workspace path under `AOPMEM_HOME/workspaces/<repo>-<hash>`;
- `aopmem.sqlite`, `tools/`, `artifacts/`, `logs/`, `runtimes/`,
  `audit-git/`;
- no `.aopmem` in target repo;
- `.understand.docs` absent when disabled;
- `.understand.docs/SCHEMA.md` present when enabled;
- installer asks only the 5 semantic/user questions via stdin;
- base gates and semantic project profile nodes are seeded.

Install/MCP note:

- optional enabled MCP profiles may use `configured_unverified` when the CLI has
  no reliable detector. This is valid and non-blocking for v0.1.

## Reflection Proof

Temp proof created a proposal with:

- low-risk `create_node`;
- high-risk `delete_node`.

Apply result:

- low-risk item applied;
- high-risk item returned as draft with `high_risk_item`.

Source/code review confirmed:

- no LLM API calls;
- no background reflection loop;
- no hidden chain-of-thought storage path;
- reflection records are node-backed per updated storage spec.

## Tool Runner Proof

Temp proof confirmed:

- generated tools live under user-level workspace `tools/<tool-id>/`;
- `tool.json` exists beside the tool;
- SQLite registry is canonical and drift is checked;
- `tool validate`, `tool list`, `tool get`, and `tool run` pass;
- runner-level `--dry-run` does not execute implementation code;
- `external_read` with no approval requirement runs;
- `external_write` blocks without `+++`;
- `external_write` runs with `--approved '+++'`.

## Artifacts Proof

Temp proof confirmed:

- artifacts root is under workspace `artifacts/`;
- cleanup creates today dir;
- old dated dir `2026-05-31` was removed;
- DB, tools, logs, runtimes, and audit snapshot dirs were not cleanup targets.

Code/tests confirm:

- 7-day retention;
- 1 GB per workspace cap;
- deterministic dated-dir cleanup.

## Final Recommendation

- Release candidate: **yes**, after the GA-001 final decision patch.
- Required patch stages:
  - none for product code.
- Optional follow-ups:
  - fix root recovery docs layout or document the pack path as canonical;
  - update Stage 055 handoff text;
  - decide whether the untracked repo baseline is acceptable for release
    packaging.
