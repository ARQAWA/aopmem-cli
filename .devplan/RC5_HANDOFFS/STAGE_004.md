# RC5 Stage 004 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Local Observability schema v2 now owns authoritative task lifecycle state.
The model uses typed UUID v4 ids, fingerprints, states, results, node kinds,
and stable errors. Exact v1 stores migrate transactionally and retain rows.

Authoritative state is stored in `tasks`, `task_bundle_nodes`, and
`task_applied_nodes`. Best-effort lifecycle events remain a separate API.
Raw query, chat, output, and reasoning are not persisted.

## Evidence

- `src/task/mod.rs`;
- `src/observability/task_state.rs`;
- `docs/TASK_START_PROTOCOL.md`;
- `.devplan/RC5_PROOF_LOG.md#stage-004`;
- `cargo test --locked`: `624/624` PASS.

## Implementation constraints

- Persist `TaskStartInput` successfully before returning a start receipt.
- Pass only a normalized query fingerprint to task state. Never pass raw query.
- Keep task/bundle ids canonical lowercase UUID v4.
- Keep workspace, bundle, memory revision, membership, and transition checks
  fail closed.
- Use replay fingerprints for exact idempotency and typed conflicts.
- Do not replace authoritative state with `LocalCollector` events.
- Do not write task history into operational memory.
- Keep task-state retention at 30 days or 100,000,000 bytes.

## Audit state

Stages 001–004 are not `VERIFIED`. The cumulative audit occurs after Stage 005.

## Next

Implement only `STAGE_005`: complete internal retrieval and `task start`.
