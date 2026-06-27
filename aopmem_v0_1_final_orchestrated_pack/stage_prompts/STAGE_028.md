# STAGE 028 — Implement .understand.docs creation

## Actor

Implementation actor: gpt-5.4, reasoning_effort medium.
Milestone audit actor: gpt-5.4, reasoning_effort high. Run cumulative audit only at stages 005, 010, 015, 020, 025, 030, 035, ... .

## Mandatory recovery protocol

Before doing anything, read:

1. `reference/FINAL_DECISION_LOG.md`
2. `reference/NON_NEGOTIABLE_SCOPE.md`
3. `reference/DERC_PROTOCOL.md`
4. `.devplan/CURRENT_STAGE.md` if it exists
5. `.devplan/EXECUTION_LEDGER.md` if it exists
6. Latest `.devplan/HANDOFFS/*` if any
7. This stage prompt

Run `git status`. If repository state contradicts the ledger or this stage, mark BLOCKED.

## Objective

Implement .understand.docs creation.

## Requirements covered

- `REQ-INSTALL-003`

## Allowed files

- `src/install/**`
- `tests/cli/**`

## Forbidden

- Do not add out-of-scope features.
- Do not modify files outside allowed scope unless absolutely required; if required, mark BLOCKED first.
- Do not implement Mem0, Hindsight, semantic/vector search, custom MCP server, migration, CI, or markdown memory exports.
- Do not continue to the next stage.

## Implementation tasks

1. Create structure only if Understand enabled
2. Default local-only/exclude support
3. Create SCHEMA/index/log/raw/concepts/entities/architecture/domain/adr/module-notes/testing-model/maps

## Required checks

Run these checks and record results in `.devplan/PROOF_LOG.md`:

- `cargo test`

If a check cannot run yet because the project stage is too early, write exactly why and what replaces it as proof.

## DERC update

At the end:

1. Update `.devplan/EXECUTION_LEDGER.md`.
2. Update `.devplan/EXECUTION_LEDGER.json` if it exists.
3. Update `.devplan/CURRENT_STAGE.md` to point to the next stage.
4. Append proof to `.devplan/PROOF_LOG.md`.
5. Write `.devplan/HANDOFFS/STAGE_028.md`.

## Stop condition

Stop after writing the handoff. Do not start the next stage.
