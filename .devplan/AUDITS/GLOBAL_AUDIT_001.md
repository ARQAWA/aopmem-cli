# GLOBAL AUDIT 001 — AOPMem v0.1

## Verdict

FAILED_NEEDS_PATCH

## Summary

Read-only audit checked DERC state, final specs, stage ledger, handoffs, proof
log, Rust source, install prompt, templates, and local verification commands.

The implementation is close, and core Rust checks pass, but it is not a
release candidate yet. Three P1 issues block RC status:

- recall bundle does not reliably include kernel/tool/MCP contract data;
- CLI advertises required commands that are still `NOT_IMPLEMENTED`;
- generated tool approval gate blocks `external_read` despite the decision log.

## Commands Run

- `sed -n ... pasted-text.txt`: read audit task.
- `sed -n ... /Users/arkadijcukavin/.codex/RTK.md`: read RTK rules.
- `mcp__codebase_memory_mcp.list_projects`: found indexed project.
- `mcp__codebase_memory_mcp.search_graph ...`: inspected indexed code areas.
- `find . -maxdepth 4 ...`: listed required repo files.
- `sed -n ... .devplan/*`: read required DERC docs.
- `python3 -m json/tooling snippets`: inspected `EXECUTION_LEDGER.json`.
- `rg ... forbidden terms`: checked out-of-scope drift.
- `CARGO_TARGET_DIR=/tmp/aopmem-audit-target rtk cargo build`: PASS, 2 warnings.
- `CARGO_TARGET_DIR=/tmp/aopmem-audit-target-test rtk cargo test`: PASS, 158 tests.
- `rtk bash scripts/dev_verify.sh`: PASS, 158 tests and CLI proof passed.
- `/tmp/aopmem-audit-target/debug/aopmem --help`: PASS, help exists.
- `/tmp/aopmem-audit-target/debug/aopmem tool --help`: PASS, help exists.
- `/tmp/aopmem-audit-target/debug/aopmem node --help`: PASS, help exists.
- `/tmp/aopmem-audit-target/debug/aopmem --json node update`: FAIL, exit 7.
- `/tmp/aopmem-audit-target/debug/aopmem --json tool list`: FAIL, exit 7.
- `/tmp/aopmem-audit-target/debug/aopmem --json tool get`: FAIL, exit 7.
- temp `init + tool create-draft + recall`: recall missed tool/MCP/kernel data.
- temp `init + external_read tool run`: FAIL, exit 6 without approval.
- `find . -name .aopmem`: PASS, no repo-local `.aopmem`.
- `find .github ...`: PASS, no CI/GitHub Actions found.

## Findings

* ID: `GA001-P1-001`
* Severity: P1
* Area: Recall / Memory Keeper bundle
* Evidence:
  - `.devplan/FINAL_DECISION_LOG.md:414-425` requires hunch-backed recall and
    Memory Keeper context.
  - `templates/skills/memory-keeper/SKILL.md` requires active project profile,
    kernel, gates, workflows, tool contracts, MCP profiles, links, FTS, hunches.
  - `src/cli/mod.rs:1798-1807` loads only `nodes` and `links` for recall.
  - `src/recall/mod.rs:149-159` keeps only `project_profile`, `gate`,
    and `workflow` as structured root groups.
  - `src/recall/mod.rs:217-224` has compact sections for `tool_contracts`
    and `mcp_profiles`, but they are populated only from node bundle data.
  - Temp proof after init with Codebase Memory enabled and a draft tool:
    `compact_tool_contracts = 0`, `compact_mcp_profiles = 0`,
    `has_kernel_key = False`.
* Expected:
  - Normal recall bundle includes kernel/contracts, tool contracts, and MCP
    profile context without relying on accidental FTS/link hits.
* Actual:
  - Kernel contract node is seeded, but not surfaced in recall output.
  - SQLite `tool_contracts` and `mcp_profiles` registries are not read by recall.
* Required fix:
  - Add structured recall inputs/sections for active `kernel_contract` nodes,
    canonical tool contracts, and MCP profiles.
  - Keep bundle compact and continue excluding deprecated/superseded nodes.
* Suggested patch stage: `PATCH_STAGE_GA001_RECALL_CONTRACTS`

* ID: `GA001-P1-002`
* Severity: P1
* Area: CLI contract
* Evidence:
  - Audit task requires `node create/get/list/update` and
    `tool create-draft/list/get/run`.
  - `src/cli/mod.rs:81-85` declares `NodeCommand::Update`.
  - `src/cli/mod.rs:320-328` declares `ToolCommand::List` and `ToolCommand::Get`.
  - `src/cli/mod.rs:539-550` routes only `create-draft`, `run`, `validate`,
    and `artifacts cleanup`; missing routes fall through to `NOT_IMPLEMENTED`
    at `src/cli/mod.rs:551-557`.
  - CLI proof:
    - `aopmem --json node update` -> exit 7, `NOT_IMPLEMENTED`.
    - `aopmem --json tool list` -> exit 7, `NOT_IMPLEMENTED`.
    - `aopmem --json tool get` -> exit 7, `NOT_IMPLEMENTED`.
* Expected:
  - Required commands advertised in help are implemented or an explicit
    equivalent exists.
* Actual:
  - Required commands exist in help but fail at runtime.
* Required fix:
  - Implement `node update`, `tool list`, and `tool get <tool-id>` with stable
    JSON envelopes and exit codes, or remove/replace with documented
    equivalents that satisfy the contract.
* Suggested patch stage: `PATCH_STAGE_GA001_CLI_REQUIRED_COMMANDS`

* ID: `GA001-P1-003`
* Severity: P1
* Area: Security / generated tool gates
* Evidence:
  - `.devplan/FINAL_DECISION_LOG.md:402-408` says external write/high-risk
    external action requires `+++`, and external read does not require approval.
  - `src/tools/mod.rs:621-627` treats `external_read` as approval-required.
  - Temp proof: a tool with `side_effects=external_read` and
    `approval_requirement=none` failed with exit 6 `UNSAFE_ACTION_BLOCKED`.
* Expected:
  - `external_read` with `approval_requirement=none` can run without `+++`.
  - `external_write` and `destructive` stay gated.
* Actual:
  - `external_read` is blocked without approval.
* Required fix:
  - Remove `external_read` from default approval-required side effects.
  - Keep explicit `approval_requirement != "none"` as an override.
* Suggested patch stage: `PATCH_STAGE_GA001_EXTERNAL_READ_GATE`

* ID: `GA001-P2-001`
* Severity: P2
* Area: Generated tools contract
* Evidence:
  - `.devplan/FINAL_DECISION_LOG.md:444-451` requires `--dry-run` if side
    effects.
  - `src/tools/mod.rs:528-532` creates draft tools with
    `supports_dry_run: false`.
  - `aopmem tool run --help` has no AOPMem-level `--dry-run` option.
* Expected:
  - Side-effectful generated tools have a clear dry-run contract, or
    AOPMem validates/enforces that dry-run exists when required.
* Actual:
  - The contract field exists, but the current draft path defaults it to false,
    and CLI run does not expose or enforce dry-run.
* Required fix:
  - Enforce `supports_dry_run` for side-effectful tools, or add an explicit
    documented exception for draft tools before validation/run.
* Suggested patch stage: `PATCH_STAGE_GA001_TOOL_DRY_RUN_CONTRACT`

* ID: `GA001-P3-001`
* Severity: P3
* Area: DERC integrity
* Evidence:
  - Audit task asks to verify `blocked_count = 0`.
  - `.devplan/EXECUTION_LEDGER.json` has `blocked: false`, but no
    `blocked_count` field.
  - JSON proof found 55 stages, all `VERIFIED`, no missing stage, no active
    blocked status.
* Expected:
  - Ledger state has explicit `blocked_count: 0`, or the omission is documented.
* Actual:
  - `blocked_count` is absent, but equivalent state is derivable.
* Required fix:
  - Add `blocked_count: 0` to ledger state, or document `blocked: false` as the
    canonical field.
* Suggested patch stage: `PATCH_STAGE_GA001_DERC_LEDGER_FIELD`

## Requirements Coverage

| Requirement | Status | Notes |
|---|---:|---|
| SQLite canonical memory | ✅ | SQLite under user workspace is implemented. |
| FTS/BM25 | ✅ | FTS5 table and BM25 search are implemented. |
| no semantic/vector | ✅ | No forbidden search backend implementation found. |
| Memory Keeper/subagent contract | ⚠️ | Template exists, but recall misses kernel/tool/MCP data. |
| DERC | ⚠️ | 55 verified stages; `blocked_count` field absent. |
| Rust CLI | ⚠️ | Build/test pass; required commands missing. |
| user-level storage | ✅ | Uses `~/.aopmem` / `AOPMEM_HOME`; no repo `.aopmem`. |
| install flow | ✅ | 5 questions, Understand first, Codebase Memory second. |
| workspace init | ✅ | Creates workspace DB and user-level dirs. |
| hunch | ✅ | Max 3, source-backed, deterministic from FTS/link signals. |
| reflection | ✅ | User-triggered; no LLM API calls found. |
| generated tools | ⚠️ | Registry/run exist; list/get and dry-run contract incomplete. |
| artifacts cleanup | ✅ | 7 days / 1 GB; deletes only artifacts dirs. |
| audit/proof | ✅ | SQL snapshot, not binary DB; proof log exists. |
| no migration | ✅ | No old MVP import/migration found; schema migrations are DB versioning. |
| no external memory backend | ✅ | No Mem0/Hindsight/Qdrant/vector backend found. |

## Out-of-scope Drift

No forbidden implementation drift found for Mem0, Hindsight, semantic search,
vector search, embeddings, Qdrant, custom MCP server, markdown AOPMem memory
exports/views, old MVP migration/import, background enrichment daemon, QA
domain pack, PR/handoff pack, current_state/task-history memory, or CI/GitHub
Actions.

Notes:

- Forbidden terms appear in specs, prompts, proof logs, or drift tests as
  out-of-scope text.
- Source references to `semantic_nodes` are install onboarding answer nodes,
  not semantic/vector search.

## Release Candidate Decision

Do not continue to release. Create patch stages only for findings above.
