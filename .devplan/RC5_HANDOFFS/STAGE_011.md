# RC5 Stage 011 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Operational migration `004_task_protocol_and_tool_aliases` and the typed
direct-alias storage/domain API are implemented.

An alias resolves only to an existing active canonical tool. It cannot target
another alias, form a cycle, or shadow a non-superseded tool. A superseded old
tool ID may become an alias as required by RC5-D-026.

P1: `0`.

P2: `0`.

## Files

Product and storage scope:

- `src/schema/mod.rs`;
- `src/tools/mod.rs`;
- `src/storage/mod.rs`;
- `src/verify/mod.rs`;
- `src/observability/export.rs`;
- `src/audit/mod.rs`;
- `src/mutation/mod.rs`;
- `src/upgrade/mod.rs`;
- `src/upgrade/apply.rs`;
- `src/upgrade/prepare.rs`;
- `src/upgrade/backup.rs`;
- `docs/TOOL_ALIASES_AND_DEDUPLICATION.md`.

Bookkeeping:

- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_011.md`.

No alias CLI, alias-aware runner, fingerprint, dedupe planner, canonicalizer,
directory copy, contract copy, executable copy, or observability emission was
added in this stage.

## Migration contract

Migration `004_task_protocol_and_tool_aliases` is an immutable, atomic
operational migration after `003_task_recall_exact_indexes`.

`tool_aliases` has:

- `alias` as the workspace-unique primary key;
- `canonical_tool_id` as a restricted foreign key to `tool_contracts`;
- `created_at`;
- bounded non-empty `source`;
- exact status `active`.

Checks reject blank, NUL-containing, oversized, or unsupported persisted
values. Target/status and status/alias indexes support direct lookup,
canonical grouping, and keyset listing.

SQLite triggers enforce:

- target exists and is active;
- target is not another alias;
- alias cannot shadow a non-superseded direct tool;
- a new non-superseded tool cannot shadow an alias;
- an aliased canonical target cannot be renamed or made non-active.

The migration is proven from valid `001` and `003` sources, is idempotent
after its marker is committed, and rolls back its table, indexes, triggers,
and marker if a later migration fails.

## Storage and resolution contract

Typed APIs cover:

- add;
- get;
- deterministic full list;
- bounded keyset page;
- bounded atomic bulk add;
- remove;
- resolve.

All inputs are validated before a write. Bulk input is capped at 1000 rows,
deduplicated with a sorted set, and applied under one nested savepoint. A late
failure leaves zero rows from that batch.

Resolver precedence is exactly:

1. direct non-superseded canonical tool;
2. active alias to an active canonical tool;
3. direct superseded fallback;
4. not found.

This lets a superseded old ID resolve through its alias while an active or
draft/deprecated/broken direct ID cannot be shadowed.

## State, filesystem, and compatibility proof

`tool_aliases` is streamed into `operational_recall_revision`. Alias add and
remove change the revision, while an exact add/remove round trip restores the
same logical revision.

Alias APIs touch SQLite only. A filesystem-count fixture proves no tool
directory, `tool.json`, executable, runtime directory, or artifact namespace
is created.

Doctor/verify now require migration `004` and `tool_aliases`. Debug capsule
schema validation accepts exactly migrations `001` through `004`. Audit SQL
round-trip includes alias rows. Existing upgrade fixtures now model a real
schema-`001` source by removing both the `004` marker and its schema objects;
successful upgrade tests expect the new `004` target.

## Complexity review

- exact add/get/remove/resolve use indexed point queries;
- resolution is one bounded SQL statement with three indexed candidates, not
  a scan loop;
- keyset pagination uses the alias primary-key index and reads at most
  `limit + 1`, with a hard limit of 1000;
- batch duplicate detection is `O(n log n)` for bounded `n <= 1000`;
- each batch row performs a constant number of indexed invariant queries;
- there is no pairwise alias scan, alias-chain traversal, recursive resolver,
  directory scan, executable hash, or N+1 filesystem work.

## Checks

```text
rtk cargo test --locked stage_011 -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked schema::tests -- --nocapture --test-threads=1
PASS 19/19

rtk cargo test --locked observability::export::tests -- --nocapture --test-threads=1
PASS 18/18

rtk cargo test --locked audit::tests -- --nocapture --test-threads=1
PASS 26/26

rtk cargo test --locked verify::tests -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked upgrade::backup::tests -- --nocapture --test-threads=1
PASS 5/5

rtk cargo test --locked upgrade::prepare::tests -- --nocapture --test-threads=1
PASS 9/9

rtk cargo test --locked upgrade::apply::tests -- --nocapture --test-threads=1
PASS 16/16

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 670/670

rtk ./scripts/dev_verify.sh
PASS, including build, 670 tests, CLI proof, negative checks, and drift check

jq ledger syntax and Stage 011 boundary checks
PASS

git diff --check
PASS
```

## Requirement state

`RC5-ALS-001` is `DONE_LOCAL_CHECKS_PASSED`.

Stage 011 storage portions of `RC5-ALS-002` and `RC5-TST-001` are complete.
Their later CLI/integration requirements remain `TODO`.

## Next boundary

Stages 001–010 remain `VERIFIED`. Stage 011 is
`DONE_LOCAL_CHECKS_PASSED`.

Verified through remains Stage 010. The next cumulative audit remains due
after Stage 015. Continue with Stage 012: tool fingerprint and read-only
dedupe plan.
