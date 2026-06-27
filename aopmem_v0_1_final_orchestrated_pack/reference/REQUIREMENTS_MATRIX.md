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
