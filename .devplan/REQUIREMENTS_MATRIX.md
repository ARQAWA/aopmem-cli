# REQUIREMENTS MATRIX

Every stage must reference requirement IDs. Final verification fails if required IDs are uncovered.

## Product boundary

| ID | Requirement |
|---|---|
| REQ-PROD-001 | AOPMem is a separate Rust CLI product repo. |
| REQ-PROD-002 | Runtime install is global under `~/.aopmem`. |
| REQ-PROD-003 | Workspace init connects AOPMem to a repo via managed adapter block. |
| REQ-PROD-004 | macOS ARM only v0.1. |
| REQ-PROD-005 | No Mem0/Hindsight/semantic/vector/MCP-server/CI/migration. |

## Storage

| ID | Requirement |
|---|---|
| REQ-STORAGE-001 | Per-workspace SQLite DB under user-level workspace folder. |
| REQ-STORAGE-002 | SQLite-only canonical AOPMem memory. |
| REQ-STORAGE-003 | SQLite FTS5/BM25 only. |
| REQ-STORAGE-004 | Nodes/links/events/registries schema. |
| REQ-STORAGE-005 | Audit SQL dump/snapshots, not binary SQLite DB. |

## CLI

| ID | Requirement |
|---|---|
| REQ-CLI-001 | CLI binary name `aopmem`. |
| REQ-CLI-002 | Stable JSON envelope for `--json`. |
| REQ-CLI-003 | Stable exit codes. |
| REQ-CLI-004 | Fail-fast structured errors. |
| REQ-CLI-005 | No direct SQL for agents. |

## Install

| ID | Requirement |
|---|---|
| REQ-INSTALL-001 | Silent technical detection. |
| REQ-INSTALL-002 | Ask only semantic/user questions. |
| REQ-INSTALL-003 | Optional Understand Anything first. |
| REQ-INSTALL-004 | Optional Codebase Memory MCP second. |
| REQ-INSTALL-005 | Managed block insertion without overwriting full instruction file. |

## Memory

| ID | Requirement |
|---|---|
| REQ-MEM-001 | Memory Keeper required by contract. |
| REQ-MEM-002 | Recall via structured lookup + graph traversal + FTS fallback. |
| REQ-MEM-003 | Hunches: 1–3 source-backed memory hints. |
| REQ-MEM-004 | Deprecated/superseded excluded from normal recall. |
| REQ-MEM-005 | User-triggered memory writes only. |

## Reflection

| ID | Requirement |
|---|---|
| REQ-REFLECT-001 | Reflection user-triggered only. |
| REQ-REFLECT-002 | CLI does not call LLM API. |
| REQ-REFLECT-003 | CLI tracks reflected sessions. |
| REQ-REFLECT-004 | Low-risk auto-apply, high-risk draft. |
| REQ-REFLECT-005 | No raw hidden chain-of-thought storage. |

## Tools/MCP

| ID | Requirement |
|---|---|
| REQ-TOOLS-001 | Generated tools live under workspace tools. |
| REQ-TOOLS-002 | Canonical registry in SQLite. |
| REQ-TOOLS-003 | `tool.json` near implementation. |
| REQ-TOOLS-004 | Agents invoke tools via `aopmem tool run`. |
| REQ-TOOLS-005 | Corporate MCP registry exists and may be empty. |

## Artifacts

| ID | Requirement |
|---|---|
| REQ-ART-001 | Artifacts are files only. |
| REQ-ART-002 | Artifacts under workspace `artifacts/YYYY-MM-DD`. |
| REQ-ART-003 | Cleanup: 7 days or 1 GB per workspace. |
| REQ-ART-004 | Cleanup never deletes DB/tools/logs/audit. |

## DERC/verification

| ID | Requirement |
|---|---|
| REQ-DERC-001 | Stage ledger required. |
| REQ-DERC-002 | Stage handoff required. |
| REQ-DERC-003 | Proof log required. |
| REQ-DERC-004 | Every stage has allowed files and DoD. |
| REQ-DERC-005 | Cumulative milestone audit every 5 stages by gpt-5.4 high. |
| REQ-VERIFY-001 | Local checks only. |
| REQ-VERIFY-002 | `cargo build` and `cargo test` required. |
| REQ-VERIFY-003 | CLI proof scenarios required. |
| REQ-VERIFY-004 | Negative tests required. |
| REQ-VERIFY-005 | Drift check required. |

## Stage coverage

Direct requirement tags from stage handoffs through `STAGE_054`.

| Stage | Requirements |
|---|---|
| `STAGE_001` | `REQ-DERC-001`, `REQ-DERC-002`, `REQ-DERC-003`, `REQ-DERC-004` |
| `STAGE_002` | `REQ-PROD-001` |
| `STAGE_003` | `REQ-PROD-001` |
| `STAGE_004` | `REQ-CLI-001` |
| `STAGE_005` | `REQ-CLI-002`, `REQ-CLI-003`, `REQ-CLI-004` |
| `STAGE_006` | `REQ-PROD-002`, `REQ-STORAGE-001` |
| `STAGE_007` | `REQ-STORAGE-001` |
| `STAGE_008` | `REQ-PROD-002`, `REQ-STORAGE-001` |
| `STAGE_009` | `REQ-STORAGE-001` |
| `STAGE_010` | `REQ-STORAGE-004` |
| `STAGE_011` | `REQ-STORAGE-004`, `REQ-CLI-005` |
| `STAGE_012` | `REQ-STORAGE-004` |
| `STAGE_013` | `REQ-STORAGE-004` |
| `STAGE_014` | `REQ-STORAGE-005` |
| `STAGE_015` | `REQ-TOOLS-002`, `REQ-TOOLS-005` |
| `STAGE_016` | `REQ-STORAGE-003` |
| `STAGE_017` | `REQ-MEM-002` |
| `STAGE_018` | `REQ-MEM-002` |
| `STAGE_019` | `REQ-STORAGE-003`, `REQ-MEM-002` |
| `STAGE_020` | `REQ-MEM-003` |
| `STAGE_021` | `REQ-MEM-002`, `REQ-MEM-003` |
| `STAGE_022` | `REQ-MEM-004` |
| `STAGE_023` | `REQ-INSTALL-005` |
| `STAGE_024` | `REQ-INSTALL-005`, `REQ-VERIFY-005` |
| `STAGE_025` | `REQ-PROD-002`, `REQ-INSTALL-001` |
| `STAGE_026` | `REQ-INSTALL-001`, `REQ-STORAGE-001` |
| `STAGE_027` | `REQ-INSTALL-001`, `REQ-INSTALL-002`, `REQ-INSTALL-003`, `REQ-INSTALL-004` |
| `STAGE_028` | `REQ-INSTALL-003` |
| `STAGE_029` | `REQ-INSTALL-003` |
| `STAGE_030` | `REQ-INSTALL-004` |
| `STAGE_031` | `REQ-TOOLS-005` |
| `STAGE_032` | `REQ-TOOLS-001`, `REQ-TOOLS-002`, `REQ-TOOLS-003` |
| `STAGE_033` | `REQ-TOOLS-001`, `REQ-TOOLS-003` |
| `STAGE_034` | `REQ-TOOLS-003` |
| `STAGE_035` | `REQ-TOOLS-004` |
| `STAGE_036` | `REQ-ART-001`, `REQ-ART-002`, `REQ-ART-003`, `REQ-ART-004` |
| `STAGE_037` | `REQ-STORAGE-005` |
| `STAGE_038` | `REQ-VERIFY-003` |
| `STAGE_039` | `REQ-MEM-005` |
| `STAGE_040` | `REQ-MEM-005` |
| `STAGE_041` | `REQ-REFLECT-001`, `REQ-REFLECT-003` |
| `STAGE_042` | `REQ-REFLECT-002`, `REQ-REFLECT-003` |
| `STAGE_043` | `REQ-REFLECT-004`, `REQ-MEM-005` |
| `STAGE_044` | `REQ-CLI-004` |
| `STAGE_045` | `REQ-MEM-002`, `REQ-TOOLS-005` |
| `STAGE_046` | `REQ-VERIFY-005` |
| `STAGE_047` | `REQ-VERIFY-004` |
| `STAGE_048` | `REQ-VERIFY-005` |
| `STAGE_049` | `REQ-VERIFY-001`, `REQ-VERIFY-002`, `REQ-VERIFY-003`, `REQ-VERIFY-004`, `REQ-VERIFY-005` |
| `STAGE_050` | `REQ-VERIFY-003` |
| `STAGE_051` | `REQ-INSTALL-001`, `REQ-INSTALL-002` |
| `STAGE_052` | `REQ-INSTALL-005` |
| `STAGE_053` | `REQ-PROD-004` |
| `STAGE_054` | `REQ-DERC-001`, `REQ-DERC-002`, `REQ-DERC-003`, `REQ-DERC-004`, `REQ-DERC-005` |
| `STAGE_055` | `REQ-VERIFY-003` |

## Requirement coverage

`Traceability-only stages` close coverage where implementation evidence existed
in earlier stage outputs but the earlier handoff did not tag that requirement
directly.

| Requirement | Direct stages | Traceability-only stages | Status |
|---|---|---|---|
| `REQ-ART-001` | `STAGE_036` | - | `covered` |
| `REQ-ART-002` | `STAGE_036` | - | `covered` |
| `REQ-ART-003` | `STAGE_036` | - | `covered` |
| `REQ-ART-004` | `STAGE_036` | - | `covered` |
| `REQ-CLI-001` | `STAGE_004` | - | `covered` |
| `REQ-CLI-002` | `STAGE_005` | - | `covered` |
| `REQ-CLI-003` | `STAGE_005` | - | `covered` |
| `REQ-CLI-004` | `STAGE_005`, `STAGE_044` | - | `covered` |
| `REQ-CLI-005` | `STAGE_011` | - | `covered` |
| `REQ-DERC-001` | `STAGE_001`, `STAGE_054` | - | `covered` |
| `REQ-DERC-002` | `STAGE_001`, `STAGE_054` | - | `covered` |
| `REQ-DERC-003` | `STAGE_001`, `STAGE_054` | - | `covered` |
| `REQ-DERC-004` | `STAGE_001`, `STAGE_054` | - | `covered` |
| `REQ-DERC-005` | `STAGE_054` | `STAGE_005`, `STAGE_010`, `STAGE_015`, `STAGE_020`, `STAGE_025`, `STAGE_030`, `STAGE_035`, `STAGE_040`, `STAGE_045`, `STAGE_050` | `covered` |
| `REQ-INSTALL-001` | `STAGE_025`, `STAGE_026`, `STAGE_027`, `STAGE_051` | - | `covered` |
| `REQ-INSTALL-002` | `STAGE_027`, `STAGE_051` | - | `covered` |
| `REQ-INSTALL-003` | `STAGE_027`, `STAGE_028`, `STAGE_029` | - | `covered` |
| `REQ-INSTALL-004` | `STAGE_027`, `STAGE_030` | - | `covered` |
| `REQ-INSTALL-005` | `STAGE_023`, `STAGE_024`, `STAGE_052` | - | `covered` |
| `REQ-MEM-001` | - | `STAGE_052` | `covered` |
| `REQ-MEM-002` | `STAGE_017`, `STAGE_018`, `STAGE_019`, `STAGE_021`, `STAGE_045` | - | `covered` |
| `REQ-MEM-003` | `STAGE_020`, `STAGE_021` | - | `covered` |
| `REQ-MEM-004` | `STAGE_022` | - | `covered` |
| `REQ-MEM-005` | `STAGE_039`, `STAGE_040`, `STAGE_043` | - | `covered` |
| `REQ-PROD-001` | `STAGE_002`, `STAGE_003` | - | `covered` |
| `REQ-PROD-002` | `STAGE_006`, `STAGE_008`, `STAGE_025` | - | `covered` |
| `REQ-PROD-003` | - | `STAGE_023`, `STAGE_024`, `STAGE_026` | `covered` |
| `REQ-PROD-004` | `STAGE_053` | - | `covered` |
| `REQ-PROD-005` | - | `STAGE_001`, `STAGE_048`, `STAGE_054` | `covered` |
| `REQ-REFLECT-001` | `STAGE_041` | - | `covered` |
| `REQ-REFLECT-002` | `STAGE_042` | - | `covered` |
| `REQ-REFLECT-003` | `STAGE_041`, `STAGE_042` | - | `covered` |
| `REQ-REFLECT-004` | `STAGE_043` | - | `covered` |
| `REQ-REFLECT-005` | - | `STAGE_042`, `STAGE_043` | `covered` |
| `REQ-STORAGE-001` | `STAGE_006`, `STAGE_007`, `STAGE_008`, `STAGE_009`, `STAGE_026` | - | `covered` |
| `REQ-STORAGE-002` | - | `STAGE_009`, `STAGE_035`, `STAGE_037` | `covered` |
| `REQ-STORAGE-003` | `STAGE_016`, `STAGE_019` | - | `covered` |
| `REQ-STORAGE-004` | `STAGE_010`, `STAGE_011`, `STAGE_012`, `STAGE_013` | - | `covered` |
| `REQ-STORAGE-005` | `STAGE_014`, `STAGE_037` | - | `covered` |
| `REQ-TOOLS-001` | `STAGE_032`, `STAGE_033` | - | `covered` |
| `REQ-TOOLS-002` | `STAGE_015`, `STAGE_032` | - | `covered` |
| `REQ-TOOLS-003` | `STAGE_032`, `STAGE_033`, `STAGE_034` | - | `covered` |
| `REQ-TOOLS-004` | `STAGE_035` | - | `covered` |
| `REQ-TOOLS-005` | `STAGE_015`, `STAGE_031`, `STAGE_045` | - | `covered` |
| `REQ-VERIFY-001` | `STAGE_049` | - | `covered` |
| `REQ-VERIFY-002` | `STAGE_049` | - | `covered` |
| `REQ-VERIFY-003` | `STAGE_038`, `STAGE_049`, `STAGE_050` | - | `covered` |
| `REQ-VERIFY-004` | `STAGE_047`, `STAGE_049` | - | `covered` |
| `REQ-VERIFY-005` | `STAGE_024`, `STAGE_046`, `STAGE_048`, `STAGE_049` | - | `covered` |

## Traceability-only notes

| Requirement | Evidence |
|---|---|
| `REQ-PROD-003` | `STAGE_023` seeded the managed adapter block, `STAGE_024` added sync/status for that block, and `STAGE_026` added workspace init around the same repo connection flow. |
| `REQ-PROD-005` | `STAGE_001` fixed the product boundary in DERC files, `STAGE_048` added forbidden-feature drift checks, and `STAGE_054` rechecked final traceability. |
| `REQ-STORAGE-002` | `STAGE_009` opened the workspace SQLite DB, `STAGE_035` enforced canonical SQLite tool-contract data, and `STAGE_037` wrote SQL snapshots from SQLite without making another canonical store. |
| `REQ-MEM-001` | `STAGE_052` added the Memory Keeper skill contract template, which is the explicit contract artifact for this requirement. |
| `REQ-REFLECT-005` | `STAGE_042` and `STAGE_043` stored strict structured reflection proposal/apply records and did not add raw hidden chain-of-thought storage. |
| `REQ-DERC-005` | Milestone cadence is visible in `CURRENT_STAGE`, `EXECUTION_LEDGER`, and completed milestone audits through `STAGE_050`; `STAGE_054` finalized the traceability pass before `STAGE_055`. |
