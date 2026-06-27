# EXECUTION LEDGER — AOPMem v0.1

Status values:

- `TODO`
- `IN_PROGRESS`
- `DONE_LOCAL_CHECKS_PASSED`
- `DONE`
- `NEEDS_AUTO_PATCH`
- `AUTO_PATCHED`
- `VERIFIED`
- `BLOCKED`
- `SKIPPED_BY_SCOPE`

Historical note:

- `DONE` and existing per-stage `VERIFIED` entries before the cutover are
  historical and remain valid.
- Starting at `STAGE_029`, normal implementation stages finish as
  `DONE_LOCAL_CHECKS_PASSED` until the next cumulative milestone audit.

## Current State

| Field | Value |
|---|---|
| Current stage | `STAGE_055_COMPLETE` |
| Last completed implementation stage | `STAGE_055` |
| Verified through stage | `STAGE_055` |
| Last audit status | `PASS` |
| Audit cadence | `cumulative milestone every 5 stages` |
| Next cumulative audit | `none` |
| DERC model cutover | `starts at STAGE_029` |
| Blocked | `false` |

## Post-Global-Audit GA-001 Resolution

Status: `VERIFIED_BY_DECISION_PATCH`

- `GA-001` resolved by final decision/spec update.
- No product code changed.
- `configured_unverified` is accepted for enabled optional MCP capabilities
  when the CLI cannot reliably verify agent-local, host-global,
  shell-managed, or otherwise non-deterministic capabilities.
- AOPMem CLI must not fake `installed` without deterministic evidence.
- Optional MCP `missing` or `configured_unverified` must not fail install.

## Stage Ledger

| Stage | Status | Notes |
|---|---|---|
| 001 | VERIFIED | Initialized dev repo DERC files. Audit passed. |
| 002 | VERIFIED | Created Rust crate skeleton. Audit passed. |
| 003 | VERIFIED | Added near-term CLI/storage dependencies with crate justifications. Audit passed. |
| 004 | VERIFIED | Added CLI shell and command routing skeleton. Audit passed. |
| 005 | VERIFIED | Added JSON envelope and fixed exit code model. Audit passed after patch. |
| 006 | VERIFIED | Added pure user-level path resolver. Audit passed. |
| 007 | VERIFIED | Added deterministic workspace key generation. Audit passed. |
| 008 | VERIFIED | Added global/workspace directory creation. Audit passed. |
| 009 | VERIFIED | Added SQLite connection and pragmas. Audit passed. |
| 010 | VERIFIED | Added migration system and schema v1 skeleton. Audit passed. |
| 011 | VERIFIED | Implemented nodes table and node create/get/list. Audit passed. |
| 012 | VERIFIED | Implemented links table and link add/list. Audit passed. |
| 013 | VERIFIED | Implemented aliases/tags/sources tables. Audit passed. |
| 014 | VERIFIED | Implemented events audit table and audit recording. Audit passed after patch. |
| 015 | VERIFIED | Implemented registries base and minimal MCP profile add/get/list. Audit passed. |
| 016 | VERIFIED | Added FTS5 table and node create/alias indexing hooks. Audit passed. |
| 017 | VERIFIED | Implemented structured recall base. Audit passed. |
| 018 | VERIFIED | Implemented recall graph traversal through existing links. Audit passed after user-approved scope patch. |
| 019 | VERIFIED | Implemented FTS/BM25 fallback with DERC dependency scope for minimal storage API and CLI recall wiring. Audit passed. |
| 020 | VERIFIED | Implemented deterministic hunch selection. Audit passed. |
| 021 | VERIFIED | Implemented additive compact recall bundle shaping and limits. Audit passed. |
| 022 | VERIFIED | Implemented normal recall exclusion for deprecated/superseded nodes. Audit passed after bookkeeping patch. |
| 023 | VERIFIED | Implemented adapter managed block seed. Audit passed. |
| 024 | VERIFIED | Implemented adapter sync/status/drift detection with minimal `src/cli/**` AUTO_PATCH_WINDOW wiring. Audit passed. |
| 025 | VERIFIED | Implemented global install check. Audit passed. |
| 026 | VERIFIED | Implemented workspace init. Audit passed after bookkeeping/test-lock patch. |
| 027 | VERIFIED | Implemented interactive install flow with 5 semantic questions, silent technical detection, and semantic answer seeding. Audit passed. |
| 028 | VERIFIED | Implemented `.understand.docs` creation only when Understand Anything is enabled, with local-only exclude support and required structure. Audit passed. |
| 029 | VERIFIED | Implemented Understand registry/profile with best-effort install behavior. Covered by cumulative audit at STAGE_030. |
| 030 | VERIFIED | Implemented Codebase Memory MCP registry/profile with best-effort install behavior. Covered by cumulative audit at STAGE_030. |
| 031 | VERIFIED | Implemented corporate MCP registry CRUD with empty-registry coverage and corporate policy-field persistence checks. Covered by cumulative audit at STAGE_035. |
| 032 | VERIFIED | Implemented tool contract registry CRUD and `tool.json` read/write helpers with round-trip tests. Covered by cumulative audit at STAGE_035. |
| 033 | VERIFIED | Implemented tool create-draft with draft layout, tool.json template, SQLite registration, and focused CLI/tests coverage. Covered by cumulative audit at STAGE_035. |
| 034 | VERIFIED | Implemented tool validate with tool.json contract checks, executable existence validation, and focused CLI/tests coverage. Covered by cumulative audit at STAGE_035. |
| 035 | VERIFIED | Implemented `aopmem tool run` with safe-only execution, runtime metadata launch, unsafe-action blocking coverage, and canonical SQLite drift enforcement. Covered by cumulative audit at STAGE_035 rerun. |
| 036 | VERIFIED | Implemented artifact day paths and local cleanup with 7-day/1 GB retention plus CLI cleanup wiring. Covered by cumulative audit at STAGE_040 rerun. |
| 037 | VERIFIED | Implemented deterministic audit-git SQL snapshot dump and wired it after successful memory writes. Covered by cumulative audit at STAGE_040 rerun. |
| 038 | VERIFIED | Implemented doctor health checks and JSON health output for prepared and missing workspace states. Covered by cumulative audit at STAGE_040 rerun. |
| 039 | VERIFIED | Implemented remember helper workflow with default raw_note writes and explicit structured node writes through existing node creation logic. Covered by cumulative audit at STAGE_040 rerun. |
| 040 | VERIFIED | Implemented teach session storage with deterministic start/add/propose/apply flows on top of existing node/link/metadata storage. Covered by cumulative audit at STAGE_040 rerun. |
| 041 | VERIFIED | Implemented reflection session inventory with tracked session ids from strict reflection records and recorded inventory status via `aopmem reflect inventory`. Covered by cumulative audit at STAGE_045. |
| 042 | VERIFIED | Implemented reflection proposal schema with minimal `src/cli/**` AUTO_PATCH_WINDOW wiring for proposal file intake and validation. Covered by cumulative audit at STAGE_045. |
| 043 | VERIFIED | Implemented reflection apply risk policy with minimal `src/cli/**` AUTO_PATCH_WINDOW wiring. Covered by cumulative audit at STAGE_045. |
| 044 | VERIFIED | Implemented approval flag handling. Covered by cumulative audit at STAGE_045. |
| 045 | VERIFIED | Implemented source hierarchy helpers, least-privilege metadata parsing, and recall source-priority ordering. Covered by cumulative audit at STAGE_045. |
| 046 | VERIFIED | Implemented lint command with verify report checks for duplicate ids, broken links, deprecated active links, missing source, missing summary, and missing gates. Covered by cumulative audit at STAGE_050. |
| 047 | VERIFIED | Implemented negative CLI scenarios coverage for missing workspace, bad node type, bad status, duplicate id, broken link, unsafe tool run, and deprecated recall exclusion. Covered by cumulative audit at STAGE_050. |
| 048 | VERIFIED | Implemented drift checks for adapter block drift, schema drift, and forbidden feature terms. Covered by cumulative audit at STAGE_050. |
| 049 | VERIFIED | Implemented local dev verify script with build/test/CLI proof, negative checks, and drift check. Covered by cumulative audit at STAGE_050. |
| 050 | VERIFIED | Implemented runtime proof scenario in local verification script. Covered by cumulative audit at STAGE_050. |
| 051 | VERIFIED | Added final install prompt file with silent detection and 5 semantic questions only. Covered by cumulative audit at STAGE_055. |
| 052 | VERIFIED | Added docs and templates for managed block, .understand.docs schema, and Memory Keeper skill contract. Covered by cumulative audit at STAGE_055. |
| 053 | VERIFIED | Added local macOS ARM build script and produced dist/aopmem-darwin-arm64/aopmem. Covered by cumulative audit at STAGE_055. |
| 054 | VERIFIED | Updated requirements matrix with full stage coverage, ran the 15x checklist, recorded clean drift status, and passed `bash scripts/dev_verify.sh`. Covered by cumulative audit at STAGE_055. |
| 055 | VERIFIED | Ran final release-candidate proof, kept scope to proof/.devplan bookkeeping, and wrote the release handoff. Covered by cumulative audit at STAGE_055. |

## Stage 020 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-MEM-003` |
| Files changed | `src/recall/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_020.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_020.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_021` |

Implementation:

- Added additive recall JSON field `hunches`.
- Added `RecallHunch` with `source_node_id`.
- Hunch payload carries source metadata and compact text only, not node body.
- Selected up to 3 hunches from FTS fallback candidates with linked signal
  metadata.
- Selection is deterministic by linked `failure_mode`, `tool_contract`, or
  `workflow` signal, FTS rank, `updated_at` hotness, then node id.
- Hunches keep both FTS source node id and optional linked signal node id.
- No semantic, vector, Mem0, Hindsight, MCP, CI, or markdown export work.
- Did not edit `src/cli/**`, `src/storage/**`, or `src/schema/**`.

Commands run:

```text
git status --short
cargo fmt && rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 59 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage
     scope
```

Known limitations:

- Recall bundle shaping and limits belong to Stage 021.

Next stage:

- `STAGE_021`

## Stage 021 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-MEM-002`, `REQ-MEM-003` |
| Files changed | `src/recall/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_021.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_021.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_022` |

Implementation:

- Added additive recall JSON field `compact`.
- Added compact sections for workflows, active gates, tool contracts,
  MCP profiles, project profile facts, corrections/lessons, hunches, and
  source refs.
- Added source ref, confidence, and trust level markers to compact node and
  hunch outputs.
- Added deterministic compact section caps.
- Kept existing recall JSON fields intact.
- Added focused unit tests for compact limits, source/trust markers, and max
  hunch count.
- No semantic, vector, Mem0, Hindsight, MCP, CI, or markdown export work.
- Did not edit `src/cli/**`, `src/storage/**`, or `src/schema/**`.

Commands run:

```text
git status --short
cargo fmt && rtk cargo test
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 61 passed
```

Known limitations:

- Deprecated/superseded exclusion hardening belongs to Stage 022.

Next stage:

- `STAGE_022`

## Stage 041 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-REFLECT-001`, `REQ-REFLECT-003` |
| Files changed | `src/reflection/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_041.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_041.md` |
| Audit result | `PASS at STAGE_045` |
| Next stage | `STAGE_042` |

Implementation:

- Implemented `aopmem reflect inventory` through `src/reflection/**` and
  `src/cli/**`.
- Added strict reflection inventory record format with `inventory_status` and
  deterministic `reflected_session_ids`.
- Tracked reflected sessions only from owned reflection record summaries; no
  universal parser was added.
- Recorded an inventory raw-note snapshot on each inventory run and refreshed
  the SQL audit snapshot after success.
- Added focused reflection unit tests and CLI end-to-end coverage.
- Did not start Stage 042.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 130 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Known limitations:

- Reflection inventory reads only strict AOPMem reflection records.
- Proposal schema and apply policy remain for Stages 042 and 043.

Next stage:

- `STAGE_042`

## Stage 042 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-REFLECT-002`, `REQ-REFLECT-003` |
| Files changed | `src/reflection/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_042.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_042.md` |
| Audit result | `PASS at STAGE_045` |
| Next stage | `STAGE_043` |

Implementation:

- Implemented structured reflection proposal JSON schema in `src/reflection/**`.
- Added deterministic low/high risk validation for proposal item types.
- Stored accepted reflection proposals as strict `reflection_proposal_v1`
  raw-note records with tracked `session_id`.
- Opened a minimal AUTO_PATCH_WINDOW in `src/cli/**` to accept
  `--proposal-file` JSON input for `aopmem reflect proposal create`.
- Added focused reflection and CLI tests for schema parsing, file-backed
  proposal creation, and risk mismatch rejection.
- Did not start Stage 043 apply policy work.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 134 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Known limitations:

- `aopmem reflect proposal apply` remains for Stage 043.
- High-risk items are validated and stored, but not applied in this stage.

Next stage:

- `STAGE_043`

## Stage 043 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-REFLECT-004`, `REQ-MEM-005` |
| Files changed | `src/reflection/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_043.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_043.md` |
| Audit result | `PASS at STAGE_045` |
| Next stage | `STAGE_044` |

Implementation:

- Implemented `reflect proposal apply` in `src/reflection/**`.
- Auto-applied only low-risk proposal items for node creation, aliases, tags,
  sources, and links.
- Kept high-risk items as draft and also kept dependent low-risk items draft
  when their proposal-local refs could not resolve.
- Stored strict `reflection_apply_v1` raw-note receipts with applied indexes,
  draft reasons, created ids, and tracked `session_id`.
- Opened a minimal AUTO_PATCH_WINDOW in `src/cli/**` to route
  `aopmem reflect proposal apply` and persist audit snapshots.
- Added focused reflection and CLI tests for low-risk apply, high-risk draft,
  and unresolved dependency draft behavior.
- Did not start Stage 044 approval handling work.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 137 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Known limitations:

- High-risk items remain in proposal draft state until Stage 044 adds approval
  handling.
- Apply resolution is deterministic and order-dependent for proposal-local
  `node_ref` usage.

Next stage:

- `STAGE_044`

## Stage 044 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-CLI-004` |
| Files changed | `src/cli/mod.rs`, `src/tools/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_044.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_044.md` |
| Audit result | `PASS at STAGE_045` |
| Next stage | `STAGE_045` |

Implementation:

- Added global CLI flag `--approved` and accepted any approval text that
  contains `+++`.
- Updated `aopmem tool run` to allow approved external/high-risk runs and to
  keep blocking those runs when approval is missing.
- Kept safe tool runs working without approval and kept drift/executable checks
  unchanged.
- Added focused CLI and tool tests for blocked and approved run paths.
- Did not start Stage 045 source hierarchy work.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 139 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Known limitations:

- Approval handling is wired for current CLI/tool execution paths only.
- Stage 045 source hierarchy and least-privilege metadata remain unchanged.

Next stage:

- `STAGE_045`

## Stage 045 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-MEM-002`, `REQ-TOOLS-005` |
| Files changed | `src/storage/mod.rs`, `src/recall/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_045.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_045.md` |
| Audit result | `PASS at STAGE_045` |
| Next stage | `STAGE_046` |

Implementation:

- Added derived source hierarchy parsing in `src/storage/**` without changing
  SQLite schema, including stable root/path/leaf/priority fields from
  `source_ref`.
- Added least-privilege metadata helpers for MCP profiles and node-backed
  tool/MCP records, reusing existing side-effect and approval fields.
- Updated structured recall compact output to include additive source hierarchy
  and least-privilege metadata.
- Recall ordering now prefers stronger source priority before trust/confidence
  tie-breaks for compact sections, FTS fallback ordering, and hunch selection.
- Added focused storage and recall tests for hierarchy parsing, metadata
  extraction, and priority ordering.
- Did not start Stage 046.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 144 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Known limitations:

- This stage adds derived source hierarchy and least-privilege metadata only.
- No schema migration, milestone audit, or Stage 046 lint work was started.

Next stage:

- `STAGE_047`

## Stage 047 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-VERIFY-004` |
| Files changed | `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_047.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_047.md` |
| Audit result | `PASS at STAGE_050` |
| Next stage | `STAGE_048` |

Implementation:

- Added negative CLI coverage in the `src/cli/mod.rs` test module.
- Covered missing workspace, bad node type, bad status, duplicate id, broken
  link, unsafe tool run, and deprecated recall exclusion scenarios.
- Kept product code unchanged in this stage.
- Did not start Stage 048.

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS cargo test: 155 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS repo matched the expected untracked baseline
```

Known limitations:

- Stage 047 adds negative CLI coverage only.
- Drift check remains for Stage 048.

Next stage:

- `STAGE_048`

## Stage 048 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-VERIFY-005` |
| Files changed | `src/verify/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_048.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_048.md` |
| Audit result | `PASS at STAGE_050` |
| Next stage | `STAGE_049` |

Implementation:

- Added drift checks in `src/verify/mod.rs`.
- Detects adapter managed block drift in `AGENTS.md`.
- Detects schema drift for migration marker, required tables, and FTS.
- Detects forbidden feature terms in code paths under `src` and `tests/cli`
  when found.
- Added focused unit tests for all three drift cases.
- Did not start Stage 049.

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS cargo test: 158 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS repo matched the expected untracked baseline
```

Known limitations:

- Stage 048 provides local drift checks only.
- Dev verify script remains for Stage 049.

Next stage:

- `STAGE_049`

## Stage 049 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-VERIFY-001`, `REQ-VERIFY-002`, `REQ-VERIFY-003`, `REQ-VERIFY-004`, `REQ-VERIFY-005` |
| Files changed | `scripts/dev_verify.sh`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_049.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_049.md` |
| Audit result | `PASS at STAGE_050` |
| Next stage | `STAGE_050` |

Implementation:

- Added `scripts/dev_verify.sh`.
- Script runs deterministic local `cargo build`, `cargo test`, clean CLI proof,
  negative checks, and a drift scenario in temp environment.
- No CI files were added.
- Did not start Stage 050.

Commands run:

```text
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS bash scripts/dev_verify.sh
PASS cargo build inside script
PASS cargo test inside script: 158 passed
PASS clean CLI proof inside script
PASS negative checks inside script
PASS drift check inside script
PASS repo matched the expected untracked baseline
```

Known limitations:

- Stage 049 provides a local verification script only.
- Runtime proof scenario remains for Stage 050.

Next stage:

- `STAGE_050`

## Stage 050 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-VERIFY-003` |
| Files changed | `scripts/dev_verify.sh`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_050.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_050.md` |
| Audit result | `PASS at STAGE_050` |
| Next stage | `STAGE_051` |

Implementation:

- Extended `scripts/dev_verify.sh` with runtime proof scenario coverage.
- Runtime proof now covers init workspace, node create, recall, isolated hunch
  fixture, tool create-draft, artifacts cleanup, doctor, negative checks, and
  drift check.
- Did not start Stage 051.

Commands run:

```text
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS bash scripts/dev_verify.sh
PASS cargo build inside script
PASS cargo test inside script: 158 passed
PASS runtime proof scenario inside script
PASS repo matched the expected untracked baseline
```

Known limitations:

- Stage 050 uses local proof script coverage only.
- Install prompt file remains for Stage 051.

Next stage:

- `STAGE_051`

## Stage 051 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-INSTALL-001`, `REQ-INSTALL-002` |
| Files changed | `install/v0.1/install_prompt.md`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_051.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_051.md` |
| Audit result | `PASS at STAGE_055` |
| Next stage | `STAGE_052` |

Implementation:

- Added `install/v0.1/install_prompt.md`.
- Prompt uses silent technical detection and asks only 5 semantic questions.
- No irrelevant technical questionnaire was added.
- Did not start Stage 052.

Commands run:

```text
test -f install/v0.1/install_prompt.md
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS install/v0.1/install_prompt.md exists
PASS json valid
PASS repo matched the expected untracked baseline
```

Known limitations:

- Stage 051 adds the install prompt file only.
- Docs and templates remain for Stage 052.

Next stage:

- `STAGE_052`

## Stage 052 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-INSTALL-005` |
| Files changed | `docs/stage_052_templates.md`, `templates/managed-block/AGENTS.managed-block.md`, `templates/understand-docs/SCHEMA.md`, `templates/skills/memory-keeper/SKILL.md`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_052.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_052.md` |
| Audit result | `PASS at STAGE_055` |
| Next stage | `STAGE_053` |

Implementation:

- Added managed block template.
- Added `.understand.docs` `SCHEMA.md` template.
- Added Memory Keeper skill contract template.
- Added short docs index for the templates.
- Did not start Stage 053.

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS cargo test: 158 passed
PASS json valid
PASS repo matched the expected untracked baseline
```

Known limitations:

- Stage 052 adds docs and templates only.
- macOS ARM build script remains for Stage 053.

Next stage:

- `STAGE_053`

## Stage 053 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-PROD-004` |
| Files changed | `scripts/build_macos_arm.sh`, `dist/aopmem-darwin-arm64/aopmem`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_053.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_053.md` |
| Audit result | `PASS at STAGE_055` |
| Next stage | `STAGE_054` |

Implementation:

- Added `scripts/build_macos_arm.sh`.
- Script produces `dist/aopmem-darwin-arm64/aopmem`.
- Verified output file exists and is a Mach-O 64-bit arm64 executable.
- Did not start Stage 054.

Commands run:

```text
bash scripts/build_macos_arm.sh || true
ls -l dist/aopmem-darwin-arm64/aopmem
file dist/aopmem-darwin-arm64/aopmem
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS build script ran
PASS dist/aopmem-darwin-arm64/aopmem exists
PASS file reports Mach-O 64-bit executable arm64
PASS json valid
PASS repo matched the expected untracked baseline
```

Known limitations:

- Stage 053 adds local build script and artifact only.
- Final traceability pass remains for Stage 054.

## Stage 046 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-VERIFY-005` |
| Files changed | `src/verify/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_046.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_046.md` |
| Audit result | `PASS at STAGE_050` |
| Next stage | `STAGE_047` |

Implementation:

- Added `aopmem verify` lint command in `src/verify/**` and CLI wiring in
  `src/cli/**`.
- Lint report checks duplicate ids, broken links, deprecated active links,
  missing source on active nodes, missing summary on active nodes, and missing
  gate inventory.
- Added focused tests for clean and dirty workspace lint behavior.
- Did not start Stage 047.

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS cargo test: 149 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS repo matched the expected untracked baseline
```

Known limitations:

- Stage 046 provides local verify lint checks only.
- Negative CLI scenarios remain for Stage 047.

Next stage:

- `STAGE_046`

## Stage 022 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-MEM-004` |
| Files changed | `src/recall/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_022.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_022.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_023` |

Implementation:

- Normal recall now excludes `deprecated` and `superseded` nodes from primary
  structured sections.
- Normal recall keeps excluding `deprecated` and `superseded` nodes from link
  traversal and FTS fallback.
- Internal grouped status shape stays intact so later lint/audit-specific
  behavior can be added without redesigning recall data structures.
- Added focused recall tests for normal section exclusion.
- No semantic, vector, Mem0, Hindsight, MCP, CI, or markdown export work.
- Did not edit `src/cli/**` or `src/storage/**`.

Commands run:

```text
git status --short
rg --files | rg '(^|/)FINAL_DECISION_LOG\.md$|(^|/)NON_NEGOTIABLE_SCOPE\.md$|(^|/)DERC_PROTOCOL\.md$'
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_022.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_021.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery used reference files from aopmem_v0_1_final_orchestrated_pack/reference/
PASS recovery git status matched prior handoff note that repo content is currently untracked in git
PASS cargo test: 62 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- This stage changes normal recall only.
- No lint/audit mode behavior was added yet.

Next stage:

- `STAGE_023`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_022
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None found.
Recommended next action: Mark STAGE_022 as VERIFIED, then move CURRENT_STAGE to STAGE_023.
```

## Stage 023 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-INSTALL-005` |
| Files changed | `src/adapter/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_023.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_023.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_024` |

Implementation:

- Added adapter seed logic in `src/adapter/mod.rs` for Codex/OpenAI
  `AGENTS.md`.
- `aopmem adapter seed` now accepts optional `--file` and otherwise detects the
  default instruction file.
- Seed inserts the managed block when absent, replaces only the managed block
  when present, and errors when markers are damaged or duplicated.
- Seed creates the instruction file when it is missing.
- Added focused tests for create, append, replace, damaged block, and CLI arg
  parsing.
- Did not implement `adapter sync` or `adapter status`.
- Did not start `STAGE_024`.

Commands run:

```text
pwd
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_023.md
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/PROOF_LOG.md
rg --files aopmem_v0_1_final_orchestrated_pack/reference .devplan/HANDOFFS | rg 'FINAL_DECISION_LOG\.md$|NON_NEGOTIABLE_SCOPE\.md$|DERC_PROTOCOL\.md$|STAGE_022\.md$'
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_022.md
git status --short
rg -n "REQ-INSTALL-005|managed block|adapter seed|AGENTS.md|AOPMEM:BEGIN" .devplan aopmem_v0_1_final_orchestrated_pack/reference aopmem_v0_1_final_orchestrated_pack/stage_prompts
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md
sed -n '1,180p' aopmem_v0_1_final_orchestrated_pack/reference/CLI_CONTRACT.md
sed -n '80,120p' .devplan/MASTER_SPEC.md
cargo test
cargo fmt
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is currently untracked in git
PASS cargo test: 68 passed
PASS cargo fmt
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- Default adapter detection in this stage seeds only Codex/OpenAI `AGENTS.md`.
- Managed block body is a minimal inline seed; template files belong to a later
  stage.
- `adapter sync` and `adapter status` remain unimplemented.

Next stage:

- `STAGE_024`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_023
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None found.
Recommended next action: Mark STAGE_023 as VERIFIED, then continue with STAGE_024.
```

## Stage 024 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-INSTALL-005`, `REQ-VERIFY-005` |
| Files changed | `src/adapter/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_024.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_024.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_025` |

Implementation:

- Added adapter status inspection with deterministic states:
  `missing`, `in_sync`, and `drifted`.
- Added adapter sync logic that appends a missing managed block, leaves an
  in-sync block unchanged, and replaces only the drifted managed block body.
- Damaged or duplicated managed block markers still fail fast and now map to
  drift/conflict handling for `adapter sync` and `adapter status`.
- Used the allowed Adapter -> CLI `AUTO_PATCH_WINDOW` for minimal
  `src/cli/**` wiring of `aopmem adapter sync` and `aopmem adapter status`.
- Added focused adapter and CLI tests for Stage 024 parsing and sync/status
  behavior.
- Did not start `STAGE_025`.

Commands run:

```text
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_024.md
sed -n '1,320p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_023.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
git status --short
cargo test
rm -rf Cargo.lock target
test ! -e Cargo.lock && echo NO_CARGO_LOCK
test ! -e target && echo NO_TARGET
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery used prompt-pack reference files and matched prior untracked repo state
PASS cargo test: 74 passed
PASS AUTO_PATCH_WINDOW used only for minimal src/cli adapter sync/status wiring
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- Managed block content is still the minimal inline seed from Stage 023.
- This stage covers adapter block sync/status only, not the later full drift
  check work from `STAGE_048`.

Next stage:

- `STAGE_025`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_024
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None found.
Recommended next action: Mark STAGE_024 as VERIFIED, then allow STAGE_025 to start.
```

## Stage 025 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-PROD-002`, `REQ-INSTALL-001` |
| Files changed | `src/install/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_025.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_025.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_026` |

Implementation:

- Added minimal global install status inspection in `src/install/mod.rs`.
- Status now checks global directory readiness, global binary presence, and
  templates directory presence under `~/.aopmem`.
- Wired `aopmem status` in `src/cli/mod.rs` to return this check instead of
  the stage stub.
- Non-JSON output stays short and avoids path details unless an error happens.
- Added focused tests for missing and ready global install states and for
  Stage 025 CLI routing.
- Did not start `STAGE_026`.

Commands run:

```text
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_025.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/PROOF_LOG.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_024.md
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo state and Stage 024 handoff
PASS cargo test: 77 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- Global install check is presence-only for this stage.
- It does not install, repair, or initialize workspace state.
- Template content is not validated yet.

Next stage:

- `STAGE_026`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_025
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark STAGE_025 as VERIFIED, then allow STAGE_026 to start.
```

## Stage 026 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-INSTALL-001`, `REQ-STORAGE-001` |
| Files changed | `src/install/mod.rs`, `src/cli/mod.rs`, `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_026.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_026.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_027` |

Implementation:

- Added minimal idempotent workspace init in `src/install/mod.rs`.
- `init` now creates global dirs, workspace dirs, and per-workspace SQLite DB.
- Seeded default active base nodes for kernel contract, gates, and
  communication-style preference.
- Wired `aopmem init` in `src/cli/mod.rs` with stable JSON and short text
  output.
- Added focused tests for workspace init creation, idempotency, and CLI
  routing.
- Patched the Stage 026 audit finding by reusing one shared process-wide test
  lock across install, cli, and storage env/current-dir mutation tests.
- Kept Stage 027 semantic install questions out of scope.

Commands run:

```text
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_026.md
sed -n '1,240p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
git status --short --branch
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_025.md
sed -n '1,260p' .devplan/MASTER_SPEC.md
sed -n '1,260p' .devplan/REQUIREMENTS_MATRIX.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/CLI_CONTRACT.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md
sed -n '60,140p' aopmem_v0_1_final_orchestrated_pack/reference/PRODUCT_SPEC.md
sed -n '60,140p' aopmem_v0_1_final_orchestrated_pack/reference/STORAGE_AND_SQLITE_SPEC.md
cargo test
RUST_TEST_THREADS=1 cargo test
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery used prompt-pack reference files and matched the prior untracked repo state
PASS implemented workspace init inside allowed stage scope only
PASS cargo test: 80 passed
PASS RUST_TEST_THREADS=1 cargo test: 80 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- Stage 026 does not add semantic onboarding questions or adapter block seeding.

Patch note:

- Stage 026 audit finding was fixed under the opened minimal
  Install -> Storage AUTO_PATCH_WINDOW.
- The code change stayed inside `src/storage/mod.rs`.
- Both `cargo test` and `RUST_TEST_THREADS=1 cargo test` passed after the fix.

Next stage:

- `STAGE_027`

## Stage 001 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-DERC-001`, `REQ-DERC-002`, `REQ-DERC-003`, `REQ-DERC-004` |
| Files changed | `.devplan/*`, `DEPS_JUSTIFICATION.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_001.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_002` |

## Stage 002 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-PROD-001` |
| Files changed | `Cargo.toml`, `src/main.rs`, `src/**`, `.devplan/*` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_002.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_003` |

## Stage 003 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-PROD-001` |
| Files changed | `Cargo.toml`, `DEPS_JUSTIFICATION.md`, `.devplan/*` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_003.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_004` |

## Stage 004 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-CLI-001` |
| Files changed | `src/cli/mod.rs`, `src/main.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_004.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_004.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_005` |

## Stage 005 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-CLI-002`, `REQ-CLI-003`, `REQ-CLI-004` |
| Files changed | `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_005.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_005.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_006` |

Audit patch note:

- Fixed `--json` parse errors to emit the stable JSON envelope to stdout.
- `aopmem --json nope` exits `2`.
- Stage 005 was re-audited and marked `VERIFIED`.

## Stage 006 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-PROD-002`, `REQ-STORAGE-001` |
| Files changed | `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_006.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_006.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_007` |

Implementation:

- Added `AopmemPaths`.
- Added `resolve_paths()` with `AOPMEM_HOME` override.
- Added default `HOME/.aopmem`.
- Added global path accessors for `bin`, `skills`, `templates`, and
  `workspaces`.
- Added unit tests with temporary `HOME` and `AOPMEM_HOME`.
- Resolver does not create files or directories.

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
rtk cargo test
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 005 state
PASS rtk cargo test: 10 passed
PASS removed generated Cargo.lock and target because they are outside Stage 006 scope
PASS json valid
```

Known limitations:

- Workspace key generation is not implemented. It belongs to Stage 007.
- Directory creation is not implemented. It belongs to Stage 008.
- SQLite connection and pragmas are not implemented. They belong to Stage 009.

Next stage:

- `STAGE_007`

## Stage 007 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-001` |
| Files changed | `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_007.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_007.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_008` |

Implementation:

- Added `workspace_key(repo_root)`.
- Added absolute path validation.
- Added sanitized repo folder slug.
- Added deterministic 8-character path hash.
- Added deterministic unit tests.
- Workspace key generation does not create files or directories.
- User-provided project id is not accepted by the API.

Commands run:

```text
git status --short
rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 006 state
PASS rtk cargo test: 15 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside Stage 007 scope
```

Known limitations:

- Directory creation is not implemented. It belongs to Stage 008.
- SQLite connection and pragmas are not implemented. They belong to Stage 009.

Next stage:

- `STAGE_008`

## Stage 008 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-PROD-002`, `REQ-STORAGE-001` |
| Files changed | `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_008.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_008.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_009` |

Implementation:

- Added `WorkspacePaths`.
- Added explicit `ensure_global_dirs(paths)`.
- Added explicit `ensure_workspace_dirs(paths, workspace_key)`.
- Created global dirs: `bin`, `skills`, `templates`, `workspaces`.
- Created workspace dirs: `tools`, `artifacts`, `audit-git`, `runtimes`, `logs`.
- Kept creation idempotent with `create_dir_all`.
- Did not create repo-local `.aopmem`.
- Did not create `aopmem.sqlite`.
- Did not add CLI wiring.

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 007 state
PASS rtk cargo test: 18 passed
PASS removed generated Cargo.lock and target because they are outside Stage 008 scope
PASS json valid
PASS final rtk cargo test after test robustness patch: 18 passed
PASS final cleanup removed generated Cargo.lock and target
```

Known limitations:

- SQLite connection and pragmas are not implemented. They belong to Stage 009.
- Directory creation is exposed only as direct storage module functions.

Next stage:

- `STAGE_009`

## Stage 009 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-001` |
| Files changed | `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_009.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_009.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_010` |

Implementation:

- Added per-workspace DB path `aopmem.sqlite`.
- Added `open_workspace_db(workspace_paths)`.
- Applied SQLite pragmas on each opened connection:
  - `PRAGMA foreign_keys = ON;`
  - `PRAGMA journal_mode = WAL;`
  - `PRAGMA busy_timeout = 5000;`
- Added unit test that opens the DB and verifies all three pragmas.
- Did not add migrations, schema tables, CRUD, CLI wiring, or repo-local
  `.aopmem`.

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status did not contradict Stage 009; repo content is
     currently untracked in git
PASS rtk cargo test: 19 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 009 scope
PASS json valid
```

Known limitations:

- No migrations or schema tables. They belong to Stage 010.
- No node/link CRUD.
- No CLI wiring.

Next stage:

- `STAGE_010`

## Notes

Initial `git status --short` showed:

```text
?? .DS_Store
?? aopmem_v0_1_final_orchestrated_pack/
```

No existing `.devplan` ledger or handoff existed.

## Stage 010 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-004` |
| Files changed | `src/schema/mod.rs`, `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_010.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_010.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_011` |

Implementation:

- Added `schema_migrations` table creation.
- Added `001_init` migration marker.
- Added idempotent migration application through `INSERT OR IGNORE`.
- `open_workspace_db` now applies pragmas, then migrations.
- Added tests for migration marker creation and idempotent re-run.
- Did not add nodes, links, aliases, events, registries, CRUD, CLI wiring,
  semantic/vector search, MCP server, or markdown memory exports.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status did not contradict Stage 010; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 22 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 010 scope
PASS json valid
```

Known limitations:

- Concrete nodes/links/aliases/events/registries tables are not implemented.
  They belong to Stage 011+.
- No CRUD.
- No CLI wiring.

Next stage:

- `STAGE_011`

## Stage 011 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-004`, `REQ-CLI-005` |
| Files changed | `src/schema/mod.rs`, `src/storage/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_011.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_011.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_012` |

Implementation:

- Added `nodes` table in migration `001_init`.
- Added indexes for node type and status.
- Added storage `Node` and `NewNode` models.
- Added `create_node`, `get_node`, and `list_nodes` storage functions.
- Added validation for allowed node types and statuses from storage spec.
- Required `source_ref`, `confidence`, and `trust_level` for active nodes.
- Added CLI `node create`, `node get`, and `node list`.
- CLI commands use storage APIs and do not expose direct SQL.
- JSON envelope behavior is preserved for `--json`.
- Did not add links, aliases, tags, sources, events, FTS, semantic/vector
  search, MCP server, or markdown import/export.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 011; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 28 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 011 scope
PASS json valid
```

Known limitations:

- Links table and link commands are not implemented. They belong to Stage 012.
- Aliases, tags, and sources tables are not implemented. They belong to
  Stage 013.
- Events audit table is not implemented. It belongs to Stage 014.
- FTS is not implemented. It belongs to Stage 016.

Next stage:

- `STAGE_012`

## Stage 012 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-004` |
| Files changed | `src/schema/mod.rs`, `src/storage/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_012.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_012.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_013` |

Implementation:

- Added `links` table in migration `001_init`.
- Added indexes for source node, target node, and link type.
- Added foreign keys from links to nodes with `ON DELETE RESTRICT`.
- Added storage `Link` and `NewLink` models.
- Added `create_link` and `list_links` storage functions.
- Validated non-empty link type.
- Validated source and target node IDs exist before insert.
- Added CLI `link add` and `link list`.
- CLI commands use storage APIs and do not expose direct SQL.
- JSON envelope behavior is preserved for `--json`.
- Did not add aliases, tags, sources, events, FTS, semantic/vector search,
  MCP server, or markdown import/export.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 012; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 32 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 012 scope
PASS json valid
```

Known limitations:

- Link type is validated as non-empty, but no allowed link type enum exists yet
  in the v0.1 specs.
- Aliases, tags, and sources tables are not implemented. They belong to
  Stage 013.
- Events audit table is not implemented. It belongs to Stage 014.
- FTS is not implemented. It belongs to Stage 016.

Next stage:

- `STAGE_013`

## Stage 013 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-004` |
| Files changed | `src/schema/mod.rs`, `src/storage/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_013.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_013.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_014` |

Implementation:

- Added `aliases`, `tags`, and `sources` tables in migration `001_init`.
- Added indexes for node lookup and value lookup.
- Added unique `(node_id, value)` constraints for each table.
- Added foreign keys from metadata rows to `nodes` with `ON DELETE RESTRICT`.
- Added storage models and add/list functions.
- Added validation for existing node IDs and non-empty values.
- Added CLI commands `alias add/list`, `tag add/list`, and `source add/list`.
- Kept JSON envelope behavior for `--json`.
- Stored aliases in a dedicated `aliases.alias` text column for later FTS use.
- Did not add events, registries, FTS, semantic/vector search, MCP server, or
  markdown import/export.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
cargo fmt && rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 013; repo content is
     currently untracked in git
PASS cargo fmt
FAIL first rtk cargo test found three Rust lifetime errors in metadata list
     functions
PASS final rtk cargo test: 36 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 013 scope
PASS json valid
```

Known limitations:

- Metadata commands are intentionally minimal add/list commands.
- `node update` remains not implemented.
- Events audit table belongs to Stage 014.
- Registries belong to Stage 015.
- FTS table/indexing belongs to Stage 016.

Next stage:

- `STAGE_014`

## Stage 014 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-005` |
| Files changed | `src/schema/mod.rs`, `src/audit/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_014.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_014.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_015` |

Implementation:

- Added SQLite `events` table to migration `001_init`.
- Added required `type`, `timestamp`, and `source` event columns.
- Added `subject_kind` and `subject_id` to identify node/link subjects.
- Added indexes for event type, timestamp, and subject lookup.
- Added `src/audit` API to record `node.created` and `link.created` events.
- Patched `src/storage` so successful `create_node` records `node.created`.
- Patched `src/storage` so successful `create_link` records `link.created`.
- Storage audit source is deterministic: `aopmem_cli`.
- Added event validation for non-empty type/source and positive subject IDs.
- Added audit and schema unit tests.
- Added focused storage tests for automatic node/link creation events.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
cargo fmt && rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 014; repo content is
     currently untracked in git
PASS cargo fmt
FAIL first rtk cargo test found one Rust lifetime error in audit list_events
PASS final rtk cargo test: 42 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 014 scope
PASS json valid
```

Known limitations:

- Real automatic event recording from `create_node` and `create_link` requires
  editing `src/storage/**`, which Stage 014 explicitly forbids in this run.
- Stage 014 therefore provides the audit write API and tests only.
- Registries belong to Stage 015.
- FTS table/indexing belongs to Stage 016.

Next stage:

- `STAGE_015`

## Stage 015 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-TOOLS-002`, `REQ-TOOLS-005` |
| Files changed | `src/schema/mod.rs`, `src/storage/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_015.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_015.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_016` |

Implementation:

- Added SQLite registry base tables in migration `001_init`.
- Added `registries`, `tool_contracts`, and `mcp_profiles` tables.
- Added required MCP profile fields from the registry spec.
- Added minimal storage API for MCP profile create/get/list.
- Added `aopmem mcp list`, `aopmem mcp add`, and `aopmem mcp get`.
- Preserved the existing JSON envelope for successful and error output.
- Added focused schema, storage, and CLI parsing tests.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 015; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 47 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 015 scope
PASS json valid
```

Known limitations:

- Generated tool create/run/validate are not implemented. They belong to
  Stage 032+.
- MCP installation is not implemented.
- Corporate MCP registry starts empty.
- FTS is not implemented. It belongs to Stage 016.

Next stage:

- `STAGE_016`

## Stage 016 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-003` |
| Files changed | `src/schema/mod.rs`, `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_016.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_016.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_017` |

Implementation:

- Added SQLite FTS5 virtual table `fts_nodes`.
- Indexed `title`, `summary`, `body`, and `aliases`.
- Added storage refresh hook after successful node create.
- Added storage refresh hook after successful alias create.
- Added focused schema and storage tests for FTS table creation, node create
  indexing, and alias indexing.
- Did not add BM25 search CLI, recall logic, semantic search, vectors, or
  embeddings.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 016; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 50 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 016 scope
PASS json valid
```

Known limitations:

- Node update is not implemented yet, so only node create and alias create
  hooks are implemented and tested.
- BM25 search CLI belongs to Stage 019.
- Recall logic belongs to Stage 017+.
- No semantic/vector/embedding code was added.

Next stage:

- `STAGE_017`

## Stage 017 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-MEM-002` |
| Files changed | `src/recall/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_017.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_017.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_018` |

Implementation:

- Added structured recall bundle builder in `src/recall/mod.rs`.
- Grouped `project_profile`, `gate`, and `workflow` nodes by status.
- Wired `aopmem recall` to read nodes with `storage::list_nodes`.
- Returned the structured bundle through the existing JSON envelope.
- Did not use FTS, BM25, graph traversal, semantic search, vectors, or
  embeddings.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 017; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 52 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 017 scope
PASS json valid
```

Known limitations:

- Recall graph traversal belongs to Stage 018.
- FTS/BM25 fallback belongs to Stage 019.
- Hunch selection and bundle shaping/limits belong to Stage 020+.
- Deprecated/superseded exclusion belongs to Stage 022.

Next stage:

- `STAGE_018`

## Stage 018 Record

| Field | Value |
|---|---|
| Status | `BLOCKED` |
| Requirements | `REQ-MEM-002` |
| Files changed | `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_018.md`, `.devplan/BLOCKERS.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_018.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_018` remains current until unblocked |

Blocker:

- Stage 018 requires recall graph traversal through links from selected
  workflow/tool/rules.
- The existing CLI `run_recall` calls `storage::list_nodes` and then
  `recall::build_structured_bundle(nodes)`.
- `storage::list_links` exists, but no links are passed to `src/recall/**`.
- Implementing real traversal would require changing `src/cli/**` wiring and
  possibly storage-facing API usage.
- Stage 018 explicitly allows only `src/recall/**` and `tests/cli/**`.
- User scope also forbids changing `src/cli/**`, `src/storage/**`, and
  `src/schema/**`.
- A recall-only implementation would be unused by the product path and would
  be a product hack.

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status matches prior handoff note that repo content is
     currently untracked in git
PASS rtk cargo test: 52 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 018 scope
PASS json valid
```

Known limitations:

- Graph traversal is not implemented.
- Deprecated/superseded exclusion is not implemented.
- Stage 018 needs scope change or a patch stage that allows CLI recall wiring
  to pass links into recall.

Next stage:

- Do not start `STAGE_019`.
- Resolve the `STAGE_018` blocker before starting `STAGE_019`.

## Stage 018 Patch Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-MEM-002` |
| Files changed | `src/recall/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_018.md`, `.devplan/BLOCKERS.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_018.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_019` |

Implementation:

- Added bounded recall graph traversal in `src/recall/mod.rs`.
- Traversal starts from non-deprecated/non-superseded `workflow`, `rule`, and
  `tool_contract` nodes.
- Traversal follows existing directed links from `storage::list_links`.
- Traversal depth is limited to 2.
- Deprecated and superseded nodes are excluded from normal traversal.
- Existing Stage 017 grouped output remains compatible; `linked_nodes` is an
  additive field.
- Wired `aopmem recall` in `src/cli/mod.rs` to pass both nodes and links into
  recall.
- Did not edit `src/storage/**` or `src/schema/**`.
- Did not add FTS/BM25, semantic/vector search, Mem0, Hindsight, custom MCP,
  CI, or markdown exports.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
```

Results:

```text
PASS recovery git status matched prior note that repo content is currently
     untracked in git
PASS cargo fmt
PASS rtk cargo test: 54 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside patch
     scope
```

Known limitations:

- Stage 018 is implementation `DONE`, not `VERIFIED`.
- Stage 018 still needs audit before starting Stage 019 work.

Next stage:

- `STAGE_019` after audit verifies Stage 018.

## Stage 019 Record

| Field | Value |
|---|---|
| Status | `BLOCKED` |
| Requirements | `REQ-STORAGE-003`, `REQ-MEM-002` |
| Files changed | `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_019.md`, `.devplan/BLOCKERS.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_019.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_019` remains current until scope is expanded or a patch stage allows FTS search wiring |

Blocker:

- Stage 019 requires FTS5/BM25 fallback results ordered by `bm25`
  ascending.
- `src/cli/mod.rs::run_recall` currently loads only nodes and links, then
  calls `recall::build_structured_bundle_with_links(nodes, links)`.
- `src/recall/**` has no SQLite connection, search query, FTS result rows, or
  public storage search API available to it.
- `src/storage/**` contains private FTS helpers and tests, but no public FTS
  search function.
- The stage allows product edits only in `src/recall/**` and `tests/cli/**`.
- The user explicitly forbids editing `src/cli/**`, `src/storage/**`, and
  `src/schema/**` unless Stage 019 is impossible in allowed scope.
- Implementing a fake lexical fallback inside `src/recall/**` would not use
  SQLite FTS5/BM25 and would violate the stage objective.

Decision:

- Marked `STAGE_019` as `BLOCKED`.
- No product files were changed.
- Did not implement semantic/vector search.

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 54 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage
     scope
```

## Stage 027 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-INSTALL-001`, `REQ-INSTALL-002`, `REQ-INSTALL-003`, `REQ-INSTALL-004` |
| Files changed | `src/install/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_027.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_027.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_028` |

Implementation:

- Added interactive install flow that asks only 5 semantic blocks:
  Understand Anything, Codebase Memory MCP, project meaning, roles, and
  scope boundaries.
- Kept technical detection silent by reusing current dir, path resolution,
  workspace key resolution, and workspace/db setup without extra user chatter.
- Seeded semantic install answers into SQLite as active `preference` and
  `project_profile` nodes.
- Kept JSON stdout stable by sending prompts and style note to prompt output,
  while `--json` still returns one success envelope.
- Added focused install-flow tests for semantic prompt collection and
  idempotent semantic node seeding.
- Did not start `.understand.docs`, MCP profile setup, or Stage 028 work.

Commands run:

```text
git status --short
cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery git status matched prior untracked repo state
PASS cargo test: 82 passed
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS json valid
```

Known limitations:

- Semantic answer nodes are idempotent by fixed title and are not updated on
  later reruns yet.
- Understand Anything setup and Codebase Memory MCP registration remain for
  later stages.

Next stage:

- `STAGE_028`

## Stage 028 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-INSTALL-003` |
| Files changed | `src/install/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_028.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_028.md` |
| Audit result | `PASS` |
| Next stage | `STAGE_029` |

Implementation:

- Added `.understand.docs` creation only when the install answer enables
  Understand Anything.
- Created the required root `SCHEMA.md` file plus these directories:
  `index`, `log`, `raw`, `concepts`, `entities`, `architecture`, `domain`,
  `adr`, `module-notes`, `testing-model`, and `maps`.
- Added default local-only git exclude support through repo-local
  `.git/info/exclude` without touching tracked project files.
- Kept disabled Understand Anything flow unchanged and skipped docs creation in
  that path.
- Added focused tests for enabled creation, disabled skip behavior, and
  exclude-entry idempotency.
- Did not start Understand registry/profile or Codebase Memory MCP setup.

Commands run:

```text
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_028.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_027.md
sed -n '1,260p' .devplan/FINAL_DECISION_LOG.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
git status --short
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md
sed -n '260,330p' .devplan/FINAL_DECISION_LOG.md
cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo state and Stage 027 handoff
PASS cargo test: 83 passed
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS json valid
```

Known limitations:

- `SCHEMA.md` is a minimal runtime scaffold for the required structure only.
- Understand registry/profile and Codebase Memory MCP setup remain for later
  stages.

Next stage:

- `STAGE_029`

## Stage 029 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-INSTALL-003` |
| Files changed | `src/install/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_029.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_029.md` |
| Audit result | `PASS at STAGE_030 cumulative audit` |
| Next stage | `STAGE_030` |

Implementation:

- Registered the `Understand Anything` MCP profile during install with
  final optional MCP status contract:
  `disabled`, `installed`, `missing`, or `configured_unverified`.
- Used existing storage `upsert_mcp_profile` wiring and kept the change inside
  install scope.
- Made profile registration best-effort so a storage failure does not fail the
  overall AOPMem install flow.
- Added focused install tests for enabled/disabled optional MCP profile
  registration and swallowed best-effort storage failure behavior.
- Did not start Codebase Memory MCP work.

Commands run:

```text
git status --short
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_029.md
sed -n '1,240p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_028.md
cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo state and Stage 028 handoff
PASS cargo test: 85 passed
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS json valid
```

Known limitations:

- Understand profile registration is best-effort and intentionally ignores
  storage write failure during install.
- Corporate MCP registry CRUD remains for Stage 031.

Next stage:

- `STAGE_030`

## Stage 030 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-INSTALL-004` |
| Files changed | `src/install/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_030.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_030.md` |
| Audit result | `PASS at STAGE_030 cumulative audit` |
| Next stage | `STAGE_031` |

Implementation:

- Added best-effort registration of the `Codebase Memory MCP` profile in the
  install flow.
- Stored profile status according to the final optional MCP status contract.
- Kept AOPMem install successful even if Codebase Memory MCP profile storage
  write fails.
- Added focused install tests for enabled, disabled, and best-effort failure
  paths.
- Did not start corporate MCP registry CRUD.

Commands run:

```text
git status --short
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_029.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_030.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline and Stage 029 handoff
PASS cargo test: 86 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Known limitations:

- This stage registers only the Codebase Memory MCP profile.
- The cumulative milestone audit for `STAGE_026`–`STAGE_030` is still pending.

Next stage:

- `STAGE_031`

## Stage 031 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-TOOLS-005` |
| Files changed | `src/cli/mod.rs`, `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_031.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_031.md` |
| Audit result | `PASS at STAGE_035 cumulative audit` |
| Next stage | `STAGE_032` |

Implementation:

- Confirmed existing MCP storage/CLI wiring already covers create/get/list/add.
- Added focused CLI tests proving an empty corporate MCP registry succeeds.
- Added focused CLI tests proving a corporate MCP profile persists
  `kind=corporate`, `side_effects`, and `approval_requirement`.
- Tightened storage assertions so MCP profile round-trip checks verify the
  stored policy fields explicitly.
- Did not start tool registry or any Stage 032 work.

Commands run:

```text
git status --short
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_030.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_031.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline and Stage 030 handoff
PASS cargo test: 88 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Known limitations:

- Corporate MCP registry remains intentionally allowed to be empty.
- No tool registry, `tool.json`, or Stage 032 work was started.

Next stage:

- `STAGE_032`

## Stage 032 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-TOOLS-001`, `REQ-TOOLS-002`, `REQ-TOOLS-003` |
| Files changed | `src/tools/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_032.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_032.md` |
| Audit result | `PASS at STAGE_035 cumulative audit` |
| Next stage | `STAGE_033` |

Implementation:

- Added a minimal `src/tools` contract model for `tool.json` with stable tool
  id, command entrypoint, status, owner workflow, args/output schema,
  side effects, approval requirement, examples, and runtime info.
- Added SQLite create/get/list helpers over the canonical `tool_contracts`
  table using direct `rusqlite` queries.
- Added workspace `tool.json` path, write, and read helpers under
  `tools/<tool-id>/tool.json`.
- Added focused unit tests for SQLite round-trip and `tool.json` round-trip.
- Did not edit `src/storage/**` or `src/cli/**`.
- Did not start Stage 033.

Commands run:

```text
git status --short
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,220p' .devplan/EXECUTION_LEDGER.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_031.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_032.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md
cargo test tools:: -- --nocapture
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the existing untracked repo baseline
PASS cargo test tools::: 3 passed
PASS cargo test: 91 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Known limitations:

- No create-draft, validate, or run behavior is implemented in this stage.

Next stage:

- `STAGE_033`

## Stage 033 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-TOOLS-001`, `REQ-TOOLS-003` |
| Files changed | `src/cli/mod.rs`, `src/tools/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_033.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_033.md` |
| Audit result | `PASS at STAGE_035 cumulative audit` |
| Next stage | `STAGE_034` |

Implementation:

- Implemented `aopmem tool create-draft` with a conservative arg set:
  `--id`, `--name`, optional `--entrypoint`, optional `--owner-workflow`,
  and safe defaults for `--side-effects` / `--approval-requirement`.
- Added draft creation helper in `src/tools/mod.rs` that creates
  `tools/<tool-id>/`, `bin/`, `runtime/`, writes a draft `tool.json`,
  and registers the contract in canonical SQLite.
- Kept tool status fixed to `draft` in the generated contract template.
- Added focused CLI parse and end-to-end tests for draft creation.
- Did not edit `src/storage/**`.
- Did not start Stage 034.

Commands run:

```text
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
ls -1t .devplan/HANDOFFS | head -n 5
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_033.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md
git status --short
sed -n '1,260p' .devplan/HANDOFFS/STAGE_032.md
sed -n '1,260p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 94 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Known limitations:

- This stage creates draft contracts only.
- Validate and run behavior remain for later stages.

Next stage:

- `STAGE_034`

## Stage 034 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-TOOLS-003` |
| Files changed | `src/cli/mod.rs`, `src/tools/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_034.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_034.md` |
| Audit result | `PASS at STAGE_035 cumulative audit` |
| Next stage | `STAGE_035` |

Implementation:

- Implemented `aopmem tool validate <tool-id>` with a positional tool id.
- Added tool validation flow in `src/tools/mod.rs` that checks registry
  presence, validates `tool.json`, and resolves the runtime executable path
  under `tools/<tool-id>/`.
- Reused existing contract validation for required fields, `side_effects`, and
  example presence, then added a focused executable existence check.
- Used the allowed Tool -> CLI `AUTO_PATCH_WINDOW` for minimal `src/cli/**`
  wiring of `tool validate`.
- Added focused Rust unit tests for success and missing executable cases, plus
  CLI parse and end-to-end validate coverage.
- Did not edit `src/storage/**`.
- Did not start Stage 035.

Commands run:

```text
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_033.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_034.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md
git status --short
sed -n '1,260p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 98 passed
PASS AUTO_PATCH_WINDOW used only for minimal src/cli tool validate wiring
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Known limitations:

- This stage validates manifest shape and executable presence only.
- `aopmem tool run` remains for Stage 035.

Next stage:

- `STAGE_035`

## Stage 035 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-TOOLS-004` |
| Files changed | `src/cli/mod.rs`, `src/tools/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_035.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_035.md` |
| Audit result | `PASS at STAGE_035 cumulative audit rerun` |
| Next stage | `STAGE_036` |

Implementation:

- Implemented `aopmem tool run <tool-id> --json -- <args...>` CLI parsing with
  trailing argument forwarding.
- Added runtime execution through registered `tool.json` metadata and reused
  tool registry + manifest validation helpers.
- Added safe-first policy: only `side_effects` `none` or `local_read` with
  `approval_requirement` `none` may run now.
- Blocked side-effectful or approval-required tools with a structured
  unsafe-action error and `EXIT_UNSAFE_ACTION_BLOCKED`.
- Captured child stdout/stderr deterministically in the tool-run result.
- Added focused Rust unit tests and CLI end-to-end tests for parse, safe run,
  and blocked unsafe run.
- Did not edit `src/storage/**`.
- Did not start Stage 036.

Commands run:

```text
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,220p' .devplan/HANDOFFS/STAGE_034.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_035.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md
git status --short
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 103 passed
PASS safe tool run executes via registry/tool.json runtime metadata
PASS unsafe tool run blocks with EXIT_UNSAFE_ACTION_BLOCKED
PASS AUTO_PATCH_WINDOW not used
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Known limitations:

- Approval flag UX is still out of scope until Stage 044.
- `tool list` and `tool get` remain unimplemented by stage plan.

Next stage:

- `STAGE_036`

## Stage 054 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-DERC-001`, `REQ-DERC-002`, `REQ-DERC-003`, `REQ-DERC-004`, `REQ-DERC-005` |
| Files changed | `.devplan/REQUIREMENTS_MATRIX.md`, `proof/stage_054_requirements_traceability.md`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_054.md` |
| Proof | `.devplan/PROOF_LOG.md`, `proof/stage_054_requirements_traceability.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_054.md` |
| Audit result | `PASS at STAGE_055` |
| Next stage | `STAGE_055` |

Implementation:

- Expanded `.devplan/REQUIREMENTS_MATRIX.md` with direct stage coverage
  through `STAGE_054`, per-requirement coverage, and traceability-only notes
  for previously uncovered requirement ids.
- Added `proof/stage_054_requirements_traceability.md` with a final `49/49`
  requirement coverage summary, a `54/54` stage row summary, the 15x
  checklist, and clean drift status.
- Kept the work inside `.devplan/**` and `proof/**` only.
- Did not start Stage 055.

Commands run:

```text
git status --short
python3 - <<'PY'
from pathlib import Path
import json
import re

matrix_text = Path('.devplan/REQUIREMENTS_MATRIX.md').read_text()
all_reqs = sorted(set(re.findall(r'REQ-[A-Z-]+-\\d{3}', matrix_text)))

handoff_reqs = {}
for path in sorted(Path('.devplan/HANDOFFS').glob('STAGE_*.md')):
    text = path.read_text()
    match = re.search(r'Requirements(?: covered)?:\\n\\n((?:- `[^`]+`\\n)+)', text)
    reqs = re.findall(r'`([^`]+)`', match.group(1)) if match else []
    handoff_reqs[path.stem] = reqs

traceability_only = {
    'REQ-PROD-003': ['STAGE_023', 'STAGE_024', 'STAGE_026'],
    'REQ-PROD-005': ['STAGE_001', 'STAGE_048', 'STAGE_054'],
    'REQ-STORAGE-002': ['STAGE_009', 'STAGE_035', 'STAGE_037'],
    'REQ-MEM-001': ['STAGE_052'],
    'REQ-REFLECT-005': ['STAGE_042', 'STAGE_043'],
    'REQ-DERC-005': ['STAGE_005', 'STAGE_010', 'STAGE_015', 'STAGE_020', 'STAGE_025', 'STAGE_030', 'STAGE_035', 'STAGE_040', 'STAGE_045', 'STAGE_050', 'STAGE_054'],
}

covered = set()
for reqs in handoff_reqs.values():
    covered.update(reqs)
covered.update(traceability_only)

missing = [req for req in all_reqs if req not in covered]
assert not missing, missing
assert len([stage for stage in handoff_reqs if stage <= 'STAGE_054']) >= 54
print('TRACEABILITY_OK', len(all_reqs), len(handoff_reqs))
PY
python3 - <<'PY'
from pathlib import Path
import re

text = Path('.devplan/REQUIREMENTS_MATRIX.md').read_text()
stage_block = text.split('## Requirement coverage', 1)[0]
stages = sorted(set(re.findall(r'\\| `STAGE_(\\d{3})` \\|', stage_block)))
assert len(stages) == 54, len(stages)
assert stages[0] == '001'
assert stages[-1] == '054'
print('STAGE_ROWS_OK', len(stages), stages[0], stages[-1])
PY
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS traceability audit confirmed 49/49 requirement ids are covered
PASS traceability audit confirmed stage coverage rows through STAGE_054
PASS 15x checklist is fully PASS with clean drift status
PASS bash scripts/dev_verify.sh
PASS json valid
PASS final git status kept the expected untracked baseline plus Stage 054 bookkeeping files
```

Known limitations:

- `STAGE_054` is a bookkeeping and proof pass only.
- Final release candidate proof remains for `STAGE_055`.

Next stage:

- `STAGE_055`

## Stage 036 Record

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Requirements | `REQ-ART-001`, `REQ-ART-002`, `REQ-ART-003`, `REQ-ART-004` |
| Files changed | `src/artifacts/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_036.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_036.md` |
| Audit result | `PASS at STAGE_040 rerun` |
| Next stage | `STAGE_037` |

Implementation:

- Added deterministic artifact day parsing and day-folder creation under
  `artifacts/YYYY-MM-DD`.
- Added local-only cleanup that deletes only inside `artifacts/`.
- Cleanup first removes folders older than the retained 7 calendar days, then
  prunes oldest remaining artifact day folders until total artifact size is at
  or below 1 GB.
- Cleanup ignores non-date entries under `artifacts/` and never touches
  sibling workspace paths like DB, `tools`, `logs`, or `audit-git`.
- Wired `aopmem artifacts cleanup` in CLI with stable JSON success output.
- Added unit and CLI tests for day-path creation, age cleanup, size cleanup,
  sibling-dir safety, parse routing, and command execution.
- Did not edit `src/storage/**`.
- Did not start Stage 037.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 111 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Known limitations:

- Stage 036 only provides artifact path helpers and cleanup.
- Audit-git SQL dump snapshots remain for Stage 037.

Next stage:

- `STAGE_037`

## Stage 037 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-STORAGE-005` |
| Files changed | `src/audit/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_037.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_037.md` |
| Audit result | `PASS at STAGE_040 rerun` |
| Next stage | `STAGE_038` |

Implementation:

- Added deterministic SQL dump generation in `src/audit/mod.rs` that writes a
  text snapshot to workspace `audit-git/memory.sql`.
- Snapshot content includes schema objects and ordered table row inserts, and
  keeps binary SQLite DB out of `audit-git`.
- Wired SQL snapshot refresh after successful CLI memory writes:
  node create, link add, alias add, tag add, source add, MCP add, and tool
  create-draft.
- Added focused audit tests for SQL dump content and text snapshot file
  writing.
- Added focused CLI proof that node create writes the SQL snapshot under
  workspace `audit-git`.
- Did not edit `src/storage/**`.
- Did not start Stage 038.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 114 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Known limitations:

- Stage 037 writes a local text SQL snapshot only; no git commit workflow was
  added yet.
- Stage 038 doctor command remains for the next stage.

Next stage:

- `STAGE_038`

## Stage 038 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-VERIFY-003` |
| Files changed | `src/verify/mod.rs`, `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_038.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_038.md` |
| Audit result | `PASS at STAGE_040 rerun` |
| Next stage | `STAGE_039` |

Implementation:

- Implemented `aopmem doctor` health checks in `src/verify/mod.rs`.
- Added checks for global dirs, workspace, DB, schema, FTS, adapter block,
  artifacts dirs, and tools dirs.
- Added stable JSON health output and plain-text summary wiring in
  `src/cli/mod.rs`.
- Added focused verify and CLI tests for prepared and missing workspace
  states.
- Did not start Stage 039.

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
```

Results:

```text
PASS cargo test: 118 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Known limitations:

- Stage 038 covers doctor health only.
- Remember helper workflow remains for Stage 039.

Next stage:

- `STAGE_039`

## Stage 039 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-MEM-005` |
| Files changed | `src/cli/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_039.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_039.md` |
| Audit result | `PASS at STAGE_040 rerun` |
| Next stage | `STAGE_040` |

Implementation:

- Implemented `aopmem remember` as a user-triggered helper on top of existing
  node creation flow in `src/cli/mod.rs`.
- Default `remember <note>` now writes a `raw_note` with `draft` status and no
  semantic classification.
- Explicit remember fields now allow direct structured node creation through
  existing node fields such as `--type`, `--status`, `--title`, `--summary`,
  `--body`, `--source-ref`, `--confidence`, and `--trust-level`.
- Added focused CLI parse coverage, end-to-end raw_note coverage,
  end-to-end structured node coverage, and a negative proof that no hidden
  classification path upgrades note content into another node type.
- Did not start Stage 040.

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 122 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Known limitations:

- Stage 039 adds only the deterministic remember helper workflow.
- Teach session storage remains for Stage 040.

Next stage:

- `STAGE_040`

## Stage 040 Record

| Field | Value |
|---|---|
| Status | `VERIFIED` |
| Requirements | `REQ-MEM-005` |
| Files changed | `src/cli/mod.rs`, `src/storage/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/CURRENT_STAGE.md`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_040.md` |
| Proof | `.devplan/PROOF_LOG.md` |
| Handoff | `.devplan/HANDOFFS/STAGE_040.md` |
| Audit result | `PASS at STAGE_040 rerun` |
| Next stage | `STAGE_041` |

Implementation:

- Implemented `teach start`, `teach add`, `teach propose`, and `teach apply`
  with explicit deterministic CLI args and JSON payloads.
- Reused existing node/link/alias/tag/source storage instead of adding hidden
  CLI inference or new schema work in this stage.
- Teach sessions, materials, proposals, and apply receipts are stored as
  deterministic draft `raw_note` records with stable summary markers and link
  wiring.
- Teach apply accepts structured proposal items and deterministically creates
  nodes, aliases, tags, sources, and links from explicit payload data only.
- Added focused CLI parse coverage and end-to-end teach flow coverage proving
  apply stores deterministic structured data without hidden classification.
- Did not start Stage 041.

Commands run:

```text
git status --short
cargo test
cargo fmt
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 124 passed
PASS cargo fmt
PASS cargo test after fmt: 124 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Known limitations:

- This stage stores and applies only deterministic teach data.
- Reflection inventory/proposal/apply stages remain out of scope for Stage 040.

Next stage:

- `STAGE_041`

## STAGE_035 Audit Patch 001

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Scope | `STAGE_035 audit finding only` |
| Requirements | `REQ-TOOLS-004` |
| Files changed | `src/cli/mod.rs`, `src/tools/mod.rs`, `.devplan/EXECUTION_LEDGER.md`, `.devplan/EXECUTION_LEDGER.json`, `.devplan/PROOF_LOG.md`, `.devplan/HANDOFFS/STAGE_035.md` |
| Audit result | `PASS at STAGE_035 cumulative audit rerun` |
| Next stage | `STAGE_036` |

Implementation:

- Fixed the STAGE_035 audit gap where validate/run only checked SQLite row
  presence and then trusted local `tool.json`.
- Added fail-fast exact contract drift detection between canonical SQLite and
  local `tool.json` before validate/run continue.
- Switched validate/run to use canonical SQLite contract data for runtime
  executable path and safety policy after drift check passes.
- Added focused negative tests for SQLite-vs-`tool.json` mismatch in validate
  and run paths.
- Added minimal CLI drift error mapping with `EXIT_DRIFT_DETECTED`.
- Did not start Stage 036.

Commands run:

```text
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/PROOF_LOG.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_035.md
git status --short
cargo test validate_tool_rejects_sqlite_and_tool_json_drift
cargo test run_tool_rejects_sqlite_and_tool_json_drift_before_local_policy_override
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS validate drift test passed
PASS run drift test passed
PASS cargo test: 105 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside patch scope
PASS STAGE_035 blocker is cleared for cumulative re-audit
```

## PATCH_GLOBAL_AUDIT_P2

| Field | Value |
|---|---|
| Status | `DONE_LOCAL_CHECKS_PASSED` |
| Scope | `GA-001`, `GA-002`, `GA-003`, `GA-004`, `GA-008` |
| Files changed | `src/audit/mod.rs`, `src/storage/mod.rs`, `src/tools/mod.rs`, `src/install/mod.rs`, `src/cli/mod.rs`, `src/adapter/mod.rs`, storage spec, DERC files |
| Checks | `cargo build`, `cargo test`, `cargo test --tests`, CLI probes, drift scan |

Implementation:

- Implemented `node update` with validation, `updated_at`, FTS refresh, and
  `node.updated` audit event.
- Implemented SQLite-backed `tool list` and `tool get`.
- Implemented runner-level `tool run --dry-run` with planned invocation,
  side effects, and approval requirement.
- Updated approval policy so `external_read` with
  `approval_requirement=none` runs without `+++`.
- Updated optional MCP profile statuses:
  `disabled`, `installed`, `missing`, `configured_unverified`.
- Reconciled storage spec: reflection sessions/proposals and workspace
  settings are node-backed in v0.1.
- Removed dead-code build warnings without changing runtime behavior.

Results:

```text
PASS cargo build
PASS cargo test: 164 passed
PASS cargo test --tests: 164 passed
PASS required CLI probes
PASS drift scan classified as docs/spec/scanner-test hits only
```

Known limitations:

- No new global audit was run.
- No new features outside the listed findings were added.
