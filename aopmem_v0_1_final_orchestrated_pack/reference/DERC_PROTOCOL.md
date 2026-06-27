# DERC — Deterministic Execution & Recovery Contract

DERC is mandatory for AOPMem development.

It ensures that a new chat, dirty context, compaction, or weak agent can recover from files and continue deterministically.


## Orchestrated execution update

Development is not driven by manual user launching each stage. The user
launches one orchestrator session. The orchestrator launches implementation
subagents, milestone audit subagents, and patch subagents.

Starting with the new model cutover, ordinary stages do not stop for a
separate audit after every stage. A non-milestone stage may finish as
`DONE_LOCAL_CHECKS_PASSED` and the orchestrator continues forward.

`VERIFIED` now means that the stage is covered by a cumulative milestone audit.
Historical `VERIFIED` / `PASS` results produced by the older per-stage audit
model remain valid.

See `reference/ORCHESTRATOR_EXECUTION_MODEL.md`.

## Devplan directory

```text
.devplan/
  MASTER_SPEC.md
  FINAL_DECISION_LOG.md
  REQUIREMENTS_MATRIX.md
  EXECUTION_LEDGER.md
  EXECUTION_LEDGER.json
  STAGE_GRAPH.md
  CURRENT_STAGE.md
  BLOCKERS.md
  PROOF_LOG.md
  HANDOFFS/
```

## Stage recovery protocol

At the start of every stage, the implementation agent must read:

1. `.devplan/MASTER_SPEC.md`
2. `.devplan/FINAL_DECISION_LOG.md`
3. `.devplan/CURRENT_STAGE.md`
4. `.devplan/EXECUTION_LEDGER.md`
5. Latest handoff in `.devplan/HANDOFFS/`
6. Current stage prompt

Then run `git status` and inspect expected files.

If state contradicts ledger, mark BLOCKED.

## Stage execution rule

Each stage prompt must specify:

- objective;
- allowed files;
- forbidden files;
- requirements covered;
- exact implementation tasks;
- exact local verification;
- ledger update;
- handoff output;
- stop condition.

## Subagent model policy

The orchestrator must launch subagents with explicit model and effort:

```text
Implementation subagent:
  model: gpt-5.4
  reasoning_effort: medium

Audit subagent:
  model: gpt-5.4
  reasoning_effort: high

Patch subagent:
  model: gpt-5.4
  reasoning_effort: medium
```

The orchestrator must not rely on default subagent model selection.

## Rust subagent skill policy

If a stage or patch edits Rust files, the orchestrator must attach the
`rust-skills` skill to the implementation, audit, or patch subagent.

Rust-touching scope means:

```text
Cargo.toml
src/**/*.rs
tests/**/*.rs
```

Do not attach `rust-skills` for bookkeeping-only edits in `.devplan/**`.

This rule is KISS/YAGNI:

- use the skill only when Rust files are touched;
- do not require it for non-Rust stages;
- do not expand stage scope because of the skill.

## Single-subagent control

The orchestrator uses single-subagent mode by default:

- keep only one active implementation, audit, or patch subagent at a time;
- close any completed or stale subagent before spawning the next one;
- never keep parallel retries open for the same stage.

Watchdog rule:

- if subagent spawn fails, do not fan out retries;
- if subagent spawn or wait stalls for 10 minutes, close the stale subagent,
  reread DERC state from files, and continue the same thin slice with either
  one fresh replacement subagent or a local emergency completion;
- record any watchdog-driven patch window or recovery step in proof/handoff if
  it changed stage state.

This rule is KISS/YAGNI/Thin Slice:

- one stage step, one active subagent;
- one recovery attempt at a time;
- continue the current slice instead of opening parallel branches.

## AUTO_PATCH_WINDOW

If a stage cannot be completed because its original allowed files miss a normal
adjacent layer, the orchestrator must not stop for user approval. It opens an
`AUTO_PATCH_WINDOW` and launches a patch subagent automatically.

The window is allowed only when the needed files are listed in the dependency
scope matrix below. The patch subagent may change only the minimum required
files for the current stage objective.

AUTO_PATCH_WINDOW flow:

```text
implementation subagent
-> local checks
-> if missing scope is allowed by dependency matrix:
     mark NEEDS_AUTO_PATCH
     launch patch subagent
     rerun local checks
     mark DONE_LOCAL_CHECKS_PASSED when the current stage is healthy
     continue stage flow
-> if real blocker, mark BLOCKED
```

Dependency scope matrix:

```text
CLI stage:
  primary: src/cli/**
  auto_patch_allowed: src/storage/**, src/recall/**

Storage stage:
  primary: src/storage/**
  auto_patch_allowed: src/schema/**, src/types/**

Recall stage:
  primary: src/recall/**
  auto_patch_allowed: src/storage/**, src/cli/**

Tool stage:
  primary: src/tools/**
  auto_patch_allowed: src/cli/**, src/storage/**

Install stage:
  primary: src/install/**
  auto_patch_allowed: src/workspace/**, src/cli/**

Reflection stage:
  primary: src/reflection/**
  auto_patch_allowed: src/storage/**, src/cli/**

Verification stage:
  primary: tests/**, scripts/**
  auto_patch_allowed: src/cli/** only for testability wiring
```

The patch subagent must record AUTO_PATCH_WINDOW use in
`.devplan/PROOF_LOG.md` and list all touched dependency files in the handoff.

At milestone audit time, the audit subagent must verify:

- patch touched only dependency matrix files;
- patch did not start the next stage;
- patch did not add features outside the current stage;
- patch only fixed the scope blocker;
- all required checks pass.

`BLOCKED` is allowed only for real blockers:

```text
need to violate out-of-scope
need to add a forbidden feature
need to change FINAL_DECISION_LOG
need to change an architectural decision
need to touch a high-risk contract
need a new dependency without justification
need a file outside the dependency matrix
need an external-system action or side effect
required checks fail and there is no deterministic patch
```

## Milestone cumulative audit

Milestone audit cadence is every 5 stages:

```text
005
010
015
020
025
030
035
...
```

Milestone audit is cumulative:

- audit after `STAGE_030` checks the state of `STAGE_001`–`STAGE_030`;
- audit focuses especially on the most recent 5 stages.

Between milestones, a stage is acceptable as complete when it has:

- `DONE_LOCAL_CHECKS_PASSED`;
- proof written;
- handoff written;
- ledger updated;
- required local checks passed;
- no scope drift.

After milestone audit PASS:

- set `verified_through_stage = STAGE_XXX`;
- mark covered stages `VERIFIED`;
- continue to the next stage.

If milestone audit FAILS:

- launch a patch subagent only for audit findings;
- rerun the cumulative milestone audit;
- continue only after PASS or real `BLOCKED`.

## Status values

```text
TODO
IN_PROGRESS
DONE_LOCAL_CHECKS_PASSED
DONE
NEEDS_AUTO_PATCH
AUTO_PATCHED
VERIFIED
BLOCKED
SKIPPED_BY_SCOPE
```

Historical note:

- `DONE` may still appear in old records from the previous per-stage audit
  model;
- new stages use `DONE_LOCAL_CHECKS_PASSED` before milestone audit.

## Proof rule

Each stage must write proof:

```text
.devplan/PROOF_LOG.md
.devplan/HANDOFFS/STAGE_XXX.md
```

Proof contains:

- commands run;
- results;
- files changed;
- requirements covered;
- known limitations;
- next stage.

## No invention rule

If requirement unclear, mark BLOCKED. Do not invent architecture.

## Milestone audit rule

`gpt-5.4` with `reasoning_effort=high` checks milestone stages only, every 5
stages, using a cumulative audit.

Non-milestone stages do not wait for a separate audit before the next stage.

If milestone audit finds issues, run a dedicated patch stage. The patch
subagent uses `gpt-5.4` with `reasoning_effort=medium` and must fix only audit
findings.
