# RC5 Stage 007 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Memory Keeper V2 now defines the fail-closed native-subagent boundary for
task start, context application, and the compact Task Context Receipt.

The obsolete normal-recall cursor flow is removed from the skill. Managed
requests now use a fixed process runner, the exact repo root as `cwd`, and a
separate stdin channel. Request-derived bytes never enter command text, argv,
temporary files, environment values, logs, errors, or receipts.

P1: `0`.

P2: `0`.

## Files

Product and test scope:

- `templates/skills/memory-keeper/SKILL.md`;
- `docs/MEMORY_KEEPER_V2.md`;
- `src/adapter/mod.rs`.

Bookkeeping:

- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_007.md`.

No managed-block, runtime task implementation, schema, installer, upgrade,
secret, tool, observability, or release file was changed.

## Contract proof

- valid skill frontmatter contains only `name` and `description`;
- instructions are imperative, bounded, and low-freedom;
- Keeper receives exact request/root/shell/instruction inputs;
- no project-code or external read is allowed before task start;
- exact request bytes go only to `task start --query-stdin --json` stdin;
- start and apply use the supplied repo root as `cwd`;
- current shell/PATH is validated for fixed executable resolution;
- no request interpolation, request-bearing pipeline, temp query file, argv
  query, shell recursion, parent continuation, or normal `recall --full`;
- native Keeper or safe-stdin failure returns exact
  `MEMORY_KEEPER_UNAVAILABLE`;
- all 17 core fields and the complete-XOR-exhausted state are validated;
- no cursor is accepted and every category/selected ID belongs to the bundle;
- apply uses the exact Stage 006 global bundle and repeatable flags;
- none-relevant is restricted to complete, non-exhausted, empty task context;
- the receipt contains identity, revision, applied IDs, bounded constraints,
  selected context, source hierarchy, remaining retrieval order, and apply
  facts;
- raw request, full nodes, database dump, transcript, output, secret,
  environment capture, and hidden reasoning are excluded;
- same-goal reuse and every frozen new-task boundary are explicit;
- exact nine-step retrieval order is preserved.

The process-runner review found one P2: an unspecified `cwd` could resolve the
wrong workspace. The final contract fixes both start and apply to the exact
supplied repo root. No open P2 remains.

## Scope choices

No `agents/openai.yaml`, script, or auxiliary skill asset was added. The
current product installer embeds only `SKILL.md`, and Stage 007 needs no
additional runtime resource.

No forward subagent was launched. Native-subagent dogfood and ten clean-agent
scenarios remain explicitly owned by Stage 025.

## Checks

```text
quick_validate.py templates/skills/memory-keeper
PASS Skill is valid!

rtk cargo test --locked \
  memory_keeper_v2_contract_is_fail_closed_and_privacy_safe -- --nocapture
PASS 1/1

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked
PASS 639/639

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

## Audit state

Stages 001–005 remain `VERIFIED`.

Stages 006–007 are `DONE_LOCAL_CHECKS_PASSED`, not `VERIFIED`. The next
cumulative audit remains due through Stage 010.

Continue with `STAGE_008`: Managed block implementation and adapters.
