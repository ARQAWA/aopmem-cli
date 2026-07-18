# RC5 Stage 003 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

The Managed Block V2 normative specification is frozen for Stage 008.
It defines contract version 2, exactly 18 numbered sections, the hard
Task-Start Gate, receipt boundary, task reuse boundary, source order, and
approval, secret, tool, error, completion, and observability rules.

## Evidence

- `.devplan/RC5_MANAGED_BLOCK_V2_SPEC.md`;
- `.devplan/RC5_PROOF_LOG.md#stage-003`;
- `.devplan/RC5_EXECUTION_LEDGER.json`.

## Implementation constraints

- Keep one canonical body in
  `templates/managed-block/AGENTS.managed-block.md`.
- Do not maintain a second handwritten Rust body.
- Keep exactly 18 numbered sections, target 100–180 useful lines, and stay
  below the 24 KiB UTF-8 hard limit.
- Preserve text outside managed markers, including custom approval rules.
- Replace v1 idempotently and update only the explicitly active instruction
  file.
- Remove the blanket secret ban; keep action-based approval.

## Audit state

Stages 001–003 are not `VERIFIED`. Cumulative audit occurs after Stage 005.

## Next

Implement only `STAGE_004`: task lifecycle and Local Observability schema v2.
