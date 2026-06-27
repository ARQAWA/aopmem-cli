# MASTER SPEC — AOPMem v0.1

This file summarizes the reference sources for deterministic recovery.

Sources:

- `reference/FINAL_DECISION_LOG.md`
- `reference/NON_NEGOTIABLE_SCOPE.md`
- `reference/DERC_PROTOCOL.md`
- `reference/REQUIREMENTS_MATRIX.md`
- `reference/STAGE_GRAPH.md`

## Product

AOPMem v0.1 is a separate Rust CLI product repo.

It is not developed inside the target workspace repo.

The CLI binary name is `aopmem`.

v0.1 supports only macOS ARM / Apple Silicon.

## Runtime Model

AOPMem has three contexts:

- dev repo;
- global host installation under `~/.aopmem`;
- workspace init for a concrete repo.

Runtime data is user-level only.

Per-workspace data is stored under:

```text
~/.aopmem/workspaces/<workspace-key>/
```

The workspace key is deterministic:

```text
<sanitized-repo-folder-name>-<8-char-path-hash>
```

## Storage

AOPMem canonical memory is SQLite only.

Allowed retrieval:

- structured lookup;
- typed link traversal;
- SQLite FTS5/BM25.

Forbidden in v0.1:

- Mem0;
- Hindsight;
- semantic search;
- vector search;
- embeddings;
- Qdrant;
- custom MCP server;
- markdown memory exports/views/imports.

## Rust Layout

v0.1 uses one Rust crate: `aopmem`.

Expected internal modules:

- `cli`
- `storage`
- `schema`
- `recall`
- `install`
- `tools`
- `reflection`
- `verify`
- `audit`
- `artifacts`
- `adapter`

Multiple workspace crates are out of scope.

## Install And Adapter

Installer updates only the current agent instruction file.

The managed block markers are:

```md
<!-- AOPMEM:BEGIN managed block -->
...
<!-- AOPMEM:END managed block -->
```

If the block exists, update only the block.

If the block is damaged, stop with an error.

## Dependencies

Dependencies are allowed only with a short reason in
`DEPS_JUSTIFICATION.md`.

## Verification

CI is out of scope.

All checks are local:

- `cargo build`;
- `cargo test`;
- CLI proof scenarios;
- negative checks;
- drift checks;
- reproducible proof files.

## DERC

Every stage must:

- read recovery files;
- follow only its scope;
- change only allowed files;
- avoid out-of-scope features;
- run local proof;
- update the ledger;
- write a handoff;
- stop at the stage stop condition.

If a stage cannot be completed without an adjacent layer, the orchestrator
opens `AUTO_PATCH_WINDOW` from `reference/DERC_PROTOCOL.md` instead of waiting
for user approval. The patch subagent may touch only dependency-matrix files,
must record that use in `.devplan/PROOF_LOG.md`, and must list touched files in
the handoff. Milestone audit must verify that scope stayed minimal, no extra
feature was added, and the next stage was not started inside the patch.

`BLOCKED` is only for real blockers: out-of-scope, forbidden feature,
forbidden technology, FINAL_DECISION_LOG change, architecture change,
high-risk contract, missing dependency justification, file outside the
dependency matrix, external-system side effect, or required checks with no
deterministic patch.

Subagent model policy:

- implementation subagent: `gpt-5.4`, `reasoning_effort=medium`;
- audit subagent: `gpt-5.4`, `reasoning_effort=high`;
- patch subagent: `gpt-5.4`, `reasoning_effort=medium`.

The orchestrator must pass model and effort explicitly and must not rely on
default subagent model selection.

Rust subagent skill policy:

- if a stage or patch touches Rust files, attach `rust-skills`;
- Rust files mean `Cargo.toml`, `src/**/*.rs`, `tests/**/*.rs`;
- do not attach `rust-skills` for bookkeeping-only `.devplan/**` patches.

Milestone audit policy:

- ordinary stages finish as `DONE_LOCAL_CHECKS_PASSED`;
- milestone audit runs every 5 stages: `005`, `010`, `015`, `020`, `025`,
  `030`, `035`, ...;
- milestone audit is cumulative through the milestone stage;
- `VERIFIED` means covered by cumulative milestone audit;
- record `verified_through_stage` in ledger state.

Subagent control policy:

- keep only one active subagent at a time;
- close any completed or stale subagent before spawning the next one;
- if subagent spawn or wait stalls for 10 minutes, close it, reread
  ledger/current-stage/handoff state, then continue the current thin slice
  locally or with one fresh replacement subagent;
- do not keep parallel retries open for the same stage.

Dependency matrix note:

- install stage may open `AUTO_PATCH_WINDOW` only for minimal
  `src/workspace/**` or `src/cli/**` wiring.
- reflection stage may open `AUTO_PATCH_WINDOW` for minimal `src/storage/**`
  or `src/cli/**` wiring.
