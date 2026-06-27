# AUDIT MILESTONE TEMPLATE — gpt-5.4 high

Use this prompt after a group of stages or before final release candidate.

## Goal

Verify cumulative consistency, not just local stage completion.

## Read

- All reference docs
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/REQUIREMENTS_MATRIX.md` if generated
- `.devplan/PROOF_LOG.md`
- All handoffs in the milestone
- Git diff since previous milestone

## Checks

1. Cumulative requirements coverage.
2. No forbidden features.
3. No missing proof.
4. No stage skipped without `SKIPPED_BY_SCOPE`.
5. No drift between final decisions and implementation.
6. No hidden migration/import logic.
7. No Mem0/Hindsight/semantic/vector/Qdrant/MCP server/CI.
8. AOPMem remains separate dev repo product.
9. Runtime data remains under `~/.aopmem`.
10. Workspace init only seeds adapter block and optional `.understand.docs`.

## Output

Same PASS/FAIL format. If FAIL, create one patch-stage prompt.
