# Audit prompt — Orchestrator state

You are gpt-5.4 high auditing the AOPMem orchestrator state.

Read:

- `reference/FINAL_DECISION_LOG.md`
- `reference/DERC_PROTOCOL.md`
- `reference/ORCHESTRATOR_EXECUTION_MODEL.md`
- `reference/STAGE_GRAPH.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/CURRENT_STAGE.md`
- latest `.devplan/HANDOFFS/*.md`

Check:

1. No stage is marked VERIFIED without audit evidence.
2. No stage advanced while previous stage was not VERIFIED.
3. Patch stages fix only audit findings.
4. Ledger, handoff, proof log, and git status are consistent.
5. No out-of-scope features were introduced.
6. The orchestrator is using subagent flow, not manual self-implementation.

Output exactly:

- `ORCHESTRATOR_STATE_OK`, or
- `ORCHESTRATOR_STATE_FAILED_WITH_FINDINGS`, with numbered findings.
