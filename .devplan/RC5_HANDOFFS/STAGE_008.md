# RC5 Stage 008 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Managed Block V2 now comes from one canonical template. Adapter seed and sync
install the exact contract, preserve all outside bytes, and reject ambiguous
marker layouts without writing.

Legacy default behavior remains `AGENTS.md`. Explicit `--file` targets cover
`AGENTS.md`, `CLAUDE.md`, `.cursor/rules/aopmem.mdc`, and
`.github/copilot-instructions.md`.

P1: `0`.

P2: `0`.

## Files

Product and test scope:

- `templates/managed-block/AGENTS.managed-block.md`;
- `src/adapter/mod.rs`;
- `src/cli/mod.rs` test coverage only.

Bookkeeping:

- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_008.md`.

No installer, secret, redaction, release, schema, storage, task-runtime, or
observability implementation file was changed.

## Contract proof

- `src/adapter/mod.rs` uses one `include_str!` source for the canonical block;
- the exact V2 content has 18 numbered sections and no duplicate handwritten
  body;
- the template has 124 useful lines and is 10835 bytes, below 24 KiB;
- hard-gate, task-boundary, receipt-reuse, action approval, secret, tool, and
  nine-step retrieval contracts match the frozen Stage 003 specification;
- obsolete normal recall, cursor, and blanket secret-ban text is absent;
- legacy V1 upgrades to exact V2 while custom approval and outside bytes stay
  byte-exact;
- repeated sync is byte-identical;
- duplicate and damaged markers fail closed without a write;
- explicit target tests cover all four adapter path types;
- only the selected target changes;
- no-`--file` behavior still selects `AGENTS.md`.

The first full-suite run exposed one stale V1 drift fixture in CLI tests. The
fixture now mutates a V2 phrase. The final full suite passes.

## Checks

```text
canonical Managed Block V2 parity check
PASS

template structure check
PASS sections=18 useful_lines=124 bytes=10835

rtk cargo test --locked adapter::tests -- --nocapture
PASS 15/15

rtk cargo test --locked \
  adapter_seed_parses_legacy_default_and_all_explicit_stage_008_targets \
  -- --nocapture
PASS 1/1

rtk cargo test --locked \
  adapter_commands_record_seed_sync_and_real_drift_only -- --nocapture
PASS 1/1

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked
PASS 642/642

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

## Matrix state

`RC5-BLK-001` and `RC5-RET-001` are
`DONE_LOCAL_CHECKS_PASSED`.

Stage 008 portions of `RC5-KPR-003`, `RC5-BLK-002`, `RC5-BLK-003`,
`RC5-ADP-001`, `RC5-ADP-002`, `RC5-GOL-001`, and `RC5-TST-001` are complete.
Their later dogfood, installer, or cross-stage proof remains with the owning
stages.

## Audit state

Stages 001–005 remain `VERIFIED`.

Stages 006–008 are `DONE_LOCAL_CHECKS_PASSED`, not `VERIFIED`. The next
cumulative audit remains due through Stage 010.

Continue with `STAGE_009`: Secret contract implementation.
