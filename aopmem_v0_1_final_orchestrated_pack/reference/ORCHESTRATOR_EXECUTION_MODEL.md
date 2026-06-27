# Orchestrator Execution Model

This file defines the actual development flow for AOPMem v0.1.

The user starts **one orchestrator session**. The orchestrator does not
manually implement the whole product. It delegates each stage to subagents and
controls verification.

## Roles

| Role | Model | Responsibility |
|---|---|---|
| Orchestrator | Current Codex session | Coordinates stages, ledger, subagent launches, audit loop. |
| Implementation subagent | gpt-5.4, reasoning_effort medium | Implements one stage only. |
| Audit subagent | gpt-5.4, reasoning_effort high | Reviews cumulative milestone state every 5 stages. |
| Patch subagent | gpt-5.4, reasoning_effort medium | Fixes audit findings only. |

The orchestrator must pass the subagent model and effort explicitly. It must
not rely on default model selection.

For Rust-touching work, the orchestrator must also attach the `rust-skills`
skill to the subagent. This applies only when the stage or patch edits Rust
sources, Rust tests, or `Cargo.toml`. It does not apply to bookkeeping-only
`.devplan/**` patches.

Subagent control is single-threaded by default:

- keep only one active subagent at a time;
- close any completed or stale subagent before spawning the next one;
- if subagent spawn or wait stalls for 10 minutes, run a watchdog:
  close the stale subagent, reread ledger/current-stage/handoff state, then
  either finish the current thin slice locally or spawn one fresh replacement;
- never keep parallel retries open for the same stage.

## Stage lifecycle

```text
TODO
  -> orchestrator launches implementation subagent
IN_PROGRESS
  -> implementation subagent writes code/proof/handoff
DONE_LOCAL_CHECKS_PASSED
  -> continue to next stage or, if milestone, launch cumulative audit
```

Milestone audit flow:

```text
DONE_LOCAL_CHECKS_PASSED at milestone stage
  -> orchestrator launches cumulative audit subagent
PASS
  -> set verified_through_stage
  -> mark covered stages VERIFIED
  -> continue
FAIL
NEEDS_AUTO_PATCH
  -> orchestrator launches patch subagent
AUTO_PATCHED
  -> cumulative audit again
VERIFIED or BLOCKED
```

If an adjacent-layer scope miss is allowed by
`AUTO_PATCH_WINDOW`:

```text
IN_PROGRESS or DONE_LOCAL_CHECKS_PASSED
  -> orchestrator detects dependency-matrix scope miss
NEEDS_AUTO_PATCH
  -> orchestrator launches patch subagent
AUTO_PATCHED
  -> rerun local checks
DONE_LOCAL_CHECKS_PASSED or BLOCKED
```

`BLOCKED` is reserved for real blockers only: out-of-scope, forbidden feature,
forbidden technology, FINAL_DECISION_LOG change, architecture change,
high-risk contract, missing dependency justification, file outside the
dependency matrix, external-system side effect, or required checks with no
deterministic patch.

## Non-negotiable rule

A stage can become `VERIFIED` only after a cumulative milestone audit covers it.

The implementation subagent cannot self-verify to VERIFIED. It may complete a
stage only up to `DONE_LOCAL_CHECKS_PASSED`.

## Orchestrator responsibilities

The orchestrator must:

1. Read the DERC protocol and decision log.
2. Maintain `.devplan/EXECUTION_LEDGER.md` and `.devplan/EXECUTION_LEDGER.json`.
3. Maintain `.devplan/CURRENT_STAGE.md`.
4. Ensure every implementation subagent receives the exact stage prompt.
   If the stage touches Rust files, attach `rust-skills`.
5. Ensure every audit subagent receives:
   - milestone target stage;
   - the stage prompts and handoffs inside the cumulative range;
   - changed files and bookkeeping for the latest 5 stages;
   - relevant decisions;
   - proof output.
   If the audited change touches Rust files, attach `rust-skills`.
6. Ensure patch subagents fix only audit findings.
   If the patch touches Rust files, attach `rust-skills`.
7. Open `AUTO_PATCH_WINDOW` automatically when a normal adjacent-layer wiring
   need is allowed by the dependency matrix.
8. Keep subagent execution single-threaded unless the user explicitly approves
   parallel work.
9. Launch cumulative milestone audit only after stages 005, 010, 015, 020,
   025, 030, 035, and so on.
10. Maintain `verified_through_stage` in ledger state.
11. Stop and mark `BLOCKED` only for real blockers or state contradictions.

## Implementation subagent contract

Each implementation subagent must:

1. Work on exactly one stage.
2. Modify only allowed files, plus `AUTO_PATCH_WINDOW` dependency-matrix files
   when the orchestrator explicitly opens that window.
   If those files include Rust code, use `rust-skills`.
3. Run required checks.
4. Write proof.
5. Write handoff.
6. Stop at `DONE_LOCAL_CHECKS_PASSED`.

It must not:

- continue to next stage;
- change architecture;
- add out-of-scope features;
- skip DERC files;
- mark itself VERIFIED.

## Audit subagent contract

Each audit subagent must check:

- cumulative state through the milestone stage;
- latest 5 stages with special focus;
- DoD satisfied;
- allowed files only;
- requirements covered;
- no out-of-scope features;
- no forbidden dependencies/features;
- if AUTO_PATCH_WINDOW was used, patch touched only dependency matrix files;
- if AUTO_PATCH_WINDOW was used, patch fixed only the scope blocker;
- if AUTO_PATCH_WINDOW was used, patch did not start the next stage;
- proof exists;
- ledger/handoff updated;
- regression/negative/drift checks pass where applicable.

If the audited change touches Rust files, the audit subagent must use
`rust-skills` while reviewing the Rust code and tests.

The audit summary must start with:

```text
AUDIT RESULT: PASS
AUDIT RESULT: FAIL
```

## Patch subagent contract

Patch subagent receives only audit findings or an orchestrator-opened
`AUTO_PATCH_WINDOW`. It must not improve beyond the finding/window.

If the patch touches Rust files, the patch subagent must use `rust-skills`.

Allowed actions:

- fix listed defect;
- fix only the adjacent-layer wiring allowed by AUTO_PATCH_WINDOW;
- rerun listed checks;
- update proof/handoff for patch.

Forbidden actions:

- new features;
- refactor outside finding;
- change unrelated files;
- skip audit.

## Recovery protocol

At the start of any orchestrator session:

1. Read `RUN_FIRST.md`.
2. Read `reference/FINAL_DECISION_LOG.md`.
3. Read `reference/DERC_PROTOCOL.md`.
4. Read `reference/STAGE_GRAPH.md`.
5. Read `.devplan/EXECUTION_LEDGER.md` if exists.
6. Read `.devplan/CURRENT_STAGE.md` if exists.
7. Read latest `.devplan/HANDOFFS/*.md` if exists.
8. Run `git status`.
9. Decide next action from ledger, not from memory.

If files disagree, mark `BLOCKED` and explain mismatch.
