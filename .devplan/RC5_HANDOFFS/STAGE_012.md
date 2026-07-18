# RC5 Stage 012 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Canonical tool fingerprinting and deterministic, strictly read-only
`aopmem tool dedupe plan --json` are implemented.

The typed plan API is reusable by Stages 013, 014, and 023. It performs no
apply, alias write, status change, contract rewrite, directory deletion,
executable deletion, creation guard, or Confluence-specific behavior.

P1: `0`.

P2: `0`.

## Files

Production:

- `Cargo.toml`;
- `Cargo.lock`;
- `src/audit/anchored.rs`;
- `src/tools/mod.rs`;
- `src/storage/mod.rs`;
- `src/cli/mod.rs`.

Documentation and bookkeeping:

- `docs/TOOL_ALIASES_AND_DEDUPLICATION.md`;
- `.devplan/RC5_TOOL_DEDUPE_REPORT.md`;
- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_012.md`.

## Contract

The SHA-256 full fingerprint uses explicit domain and length separation.
Canonical object-key order, normalized safe relative paths, deterministic
platform launcher order, and implementation path/hash order make it stable.

It includes side effects, approval, schemas, runtime/output/dry-run limits,
relative layout, platform launchers, and implementation hashes. It excludes
tool ID, display name, status, timestamps, owner identity, examples, and
cosmetic descriptions.

`platform_launchers` is a `serde(default)` `BTreeMap` inside `ToolContract`.
Empty maps are omitted during serialization, so existing manifests remain
compatible and stable.

All five duplicate classes are public and tested. Displayed class and
`exact_only_eligible` are separate. Eligibility is true only for equal full
fingerprints.

Public JSON exposes safe IDs, class, eligibility, reason codes, and counts.
It exposes no canonical, capability, or implementation fingerprint values.

## Read-only and filesystem safety

The CLI bypasses Local Observability. It uses a clean immutable SQLite view
with query-only and in-memory temp state. Existing WAL/SHM sidecars block the
operation, preventing stale immutable reads and preventing a nominal reader
from creating shared-memory state.

Workspace, tools root, tool directories, descendant directories, manifests,
and implementation files use the existing OS-native anchored no-follow
handle layer.

The complete descendant tree is validated before `tool.json` is read.
Manifest reads are capped at 1 MiB. File hashing reads the already-opened
regular-file handle. Pre/post metadata and final anchored-directory identity
checks reject in-place drift and same-path replacement.

The exact zero-write test compares the complete AOPMem home tree before and
after two plans: paths, types, sizes, nanosecond mtimes, and SHA-256 bytes are
identical. No observability database is created.

## Complexity

The planner indexes capability signatures, normalized labels, and tokens
before hashing. It checks theoretical bucket-pair size before pair
generation, deduplicates the bounded union, and hashes only shortlisted
tools.

Bounds:

- tools: 1,000;
- pairs: 10,000;
- implementation files/tool: 256;
- implementation bytes/tool: 64 MiB;
- depth: 16;
- manifest bytes: 1 MiB.

Index work is `O(T log T)`. Pair work is capped at 10,000. Filesystem work is
`O(F + B)` for shortlisted tools. Each implementation file is hashed exactly
once per operation and reused across comparisons. Normal tool execution does
not fingerprint.

## Checks

```text
rtk cargo test --locked stage_012 -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked tools::tests -- --nocapture --test-threads=1
PASS 75/75

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 679/679

rtk ./scripts/dev_verify.sh
PASS, including build, 679 tests, CLI proof, negative checks, and drift check
```

Final bookkeeping checks:

- `jq` ledger syntax and Stage 012/013 boundaries: PASS;
- `git diff --check`: PASS.

## Requirement state

`RC5-DUP-001` is `DONE_LOCAL_CHECKS_PASSED`.

The Stage 012 read-only-plan portion of `RC5-DUP-002` and focused testing is
complete. Exact-only apply remains `TODO` until Stage 014.

## Next boundary

## Audit remediation after Stage 014

The implementation scanner now reopens each anchored relative implementation
path after hashing and compares its private filesystem identity to the held
file handle. A same-path regular-file replacement, including one with matching
size and mtime, fails closed as `ImplementationDrift`.

It also streams a total 1,024 descendant-entry cap before sorting and
rechecks the anchored `tool.json` identity after each read.

Stages 001–010 remain `VERIFIED`. Stages 011–012 are
`DONE_LOCAL_CHECKS_PASSED`.

Verified through remains Stage 010. The cumulative audit remains due after
Stage 015. Continue with Stage 013: alias-aware tool list/get/run/validate.

## Cumulative-audit CLI privacy remediation

A real compiled-binary integration test now creates one isolated repository,
`HOME`, and `AOPMEM_HOME`, registers two overlapping tools through the public
CLI, and adds an unsafe child symlink whose name and absolute target contain a
unique raw-path canary.

The test invokes all four public failure surfaces:

- `tool dedupe plan` in text mode;
- `tool dedupe plan --json`;
- `tool dedupe apply --exact-only` in text mode;
- `tool dedupe apply --exact-only --json`.

Every invocation exits with code `5`, contains only the stable
`TOOL_DEDUPE_FILESYSTEM_UNSAFE` reason, and exposes neither the canary nor the
isolated root/absolute target in the complete captured stdout plus stderr.
JSON outputs parse as the standard error envelope with the correct plan/apply
command ID, `TOOL_DEDUPE_PLAN_FAILED`, null data, one error, and no warnings.
Text outputs use empty stdout and the exact stable stderr line.

```text
rtk cargo test --locked \
  stage_012_014_cli_dedupe_filesystem_errors_are_private_in_text_and_json \
  -- --nocapture --test-threads=1
PASS 1/1; 708 filtered

rtk cargo test --locked
PASS 709/709

rtk cargo test --tests --locked
PASS 709/709

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk ./scripts/dev_verify.sh
PASS, including 708 unit tests, 1 compiled-CLI integration test, CLI proof,
negative checks, and drift check

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

This closes the Stage 012 raw-path CLI proof gap only. The cumulative audit
status remains `FAIL`, and verified through remains `STAGE_010`, until a fresh
independent audit accepts all remediations.
