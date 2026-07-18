# RC5 Stage 009 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Authorized test credentials are usable without blanket refusal or extra
approval. Exact test-secret persistence now has one safe agent path:
`teach propose --apply --payload-stdin`.

The proposal, exact node, `sensitivity:test_secret` tag, and apply receipt
commit in one workspace mutation before one audit snapshot attempt.

P1: `0`.

P2: `0`.

## Files

Product and test scope:

- `src/cli/mod.rs`;
- `templates/skills/memory-keeper/SKILL.md`;
- `docs/MEMORY_KEEPER_V2.md`;
- `docs/SECRET_HANDLING.md`.

Bookkeeping:

- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_009.md`.

No managed-block, schema, storage, installer, redaction, export, audit,
observability, release, or tool-policy implementation file was changed.

## Atomic persistence proof

- `--payload-stdin` and inline `--payload` are mutually exclusive;
- `--payload-stdin` requires `--apply`;
- stdin is bounded to 2 MiB, requires UTF-8, and rejects NUL bytes;
- secret-bearing JSON never enters argv, shell text, a temporary file, an
  environment value, a log, an error, or a receipt;
- `store_teach_proposal` and `apply_teach_proposal` run inside one outer
  `BEGIN IMMEDIATE` mutation transaction;
- the canonical target uses generic title `Authorized test credential`,
  generic provenance, exact value only in bodies, and exact tag
  `sensitivity:test_secret`;
- the successful fixture publishes one snapshot only after target and tag
  coexist;
- a late duplicate-tag failure leaves proposal, node, tag, revision, and
  snapshot count unchanged;
- the atomic response and observability payload contain no fake canary.

The old two-command `teach propose` then `teach apply` flow remains available
for nonsecret compatibility. It is forbidden by the agent contract for exact
test-secret storage. `remember` then `tag add` is also forbidden for this use.

## Use and approval proof

- documentation permits user-provided and authorized test, VDI, and
  closed-contour credentials;
- it forbids refusal, placeholder coercion, lecture, and value removal solely
  because input looks secret;
- an external-read authentication fixture receives the exact fake credential
  without `+++`;
- the read changes no operational node, tag, or revision;
- external write remains blocked without approval and runs with standalone
  exact `+++`;
- no real credential is present in code, tests, docs, or proof.

No detector, secrets manager, schema, side-effect class, or automatic
classification was added.

## Checks

```text
rtk cargo test --locked stage_009 -- --nocapture --test-threads=1
PASS 4/4

rtk cargo test --locked \
  teach_propose_stdin_is_exclusive_bounded_utf8_and_canary_safe_on_error \
  -- --nocapture
PASS 1/1

rtk cargo test --locked teach_ -- --nocapture --test-threads=1
PASS 11/11

Memory Keeper quick validation
PASS Skill is valid!

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked
PASS 647/647

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

## Requirement state

`RC5-SEC-001` is `DONE_LOCAL_CHECKS_PASSED`.

Stage 009 portions of `RC5-SEC-002`, `RC5-SEC-004`, and `RC5-TST-001`
are complete. Their redaction and cumulative proof remains with Stage 010.

## Stage 010 boundary

Stage 010 owns all redaction implementation and canary sweeps. It must load
tagged exact values from operational memory and replace every export copy,
including the teach proposal record, with exactly
`<TEST_SECRET_REDACTED>`.

Stage 010 must re-run the Stage 009 tests. Operational memory and durable
full-home backup continue preserving exact explicitly authorized values.

Stages 001–005 remain `VERIFIED`. Stages 006–009 remain
`DONE_LOCAL_CHECKS_PASSED` until the cumulative audit through Stage 010.
