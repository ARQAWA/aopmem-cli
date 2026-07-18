# RC5 Stage 013 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

## Result

Alias CRUD/resolve, alias-aware tool commands, deterministic canonical list
semantics, safe alias observability, and the bounded tool creation guard are
implemented.

P1: `0`.

P2: `0`.

## Files

Production:

- `src/tools/mod.rs`;
- `src/cli/mod.rs`.

Documentation and bookkeeping:

- `docs/TOOL_ALIASES_AND_DEDUPLICATION.md`;
- `.devplan/RC5_TOOL_DEDUPE_REPORT.md`;
- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_013.md`.

No exact-only dedupe apply, status canonicalization, contract rewrite,
directory deletion, executable deletion, Confluence fixture, or UI work was
added.

## Alias CLI and resolution

Implemented:

```text
aopmem tool alias add <alias> --to <tool-id>
aopmem tool alias list
aopmem tool alias remove <alias>
aopmem tool resolve <id-or-alias>
```

Storage invariants from Stage 011 remain authoritative. Active canonical IDs
cannot be shadowed. Alias targets are direct active canonical IDs. Chains,
cycles, duplicate directories, copied contracts, and copied executables are
not created.

`tool get`, `tool validate`, and `tool run` resolve before forming a
filesystem path. Alias requests use only the canonical:

- tool directory;
- `tool.json`;
- executable;
- process cwd;
- runtime resources;
- artifact behavior.

Approval behavior is unchanged. The focused external-write fixture remains
blocked without approval and runs with the same exact `+++` approval.

`tool get` returns requested ID, matched alias, canonical ID, and canonical
contract. Alias run/validation output adds resolution facts while direct
calls keep their prior record shape.

## List contract

Default `tool list` returns non-superseded canonical records. Each canonical
row contains its alias IDs.

`--include-aliases` adds explicit alias rows after their canonical row.
Canonical keyset pagination remains authoritative:

- aliases do not consume the limit;
- aliases do not change the cursor;
- aliases do not hide a canonical row;
- canonical and alias ordering is deterministic.

All aliases for one canonical page are loaded with one indexed batch query.
There is no alias N+1 query.

## Observability and privacy

An alias run records the factual v2 event `tool.alias_resolved` before
validation/run facts. The payload contains only the bounded alias ID and
approval-presence boolean.

Creation guard blocks record `tool.duplicate_detected` and
`tool.duplicate_blocked`. A reviewed overlap that is explicitly bypassed
records only safe candidate facts.

Raw `--technical-distinction` text is never:

- persisted in operational memory;
- persisted in Local Observability;
- returned in success JSON;
- copied into an error;
- sent through clap value-parser diagnostics;
- logged or added to proof evidence.

Only `technical_distinction_provided: true|false` may be returned.

## Creation guard

Before draft registry/filesystem writes, the guard checks:

1. direct registry ID collision;
2. direct alias collision;
3. normalized capability label;
4. behavioral capability signature where semantic evidence exists;
5. deterministic bounded BM25 over registry ID/name terms;
6. anchored implementation safety and drift for shortlisted existing tools.

Direct ID or alias collisions return `TOOL_DUPLICATE`,
`writes_performed: false`, canonical ID, alias suggestion, class, and safe
reason codes. A possible semantic overlap returns
`TOOL_OVERLAP_REVIEW_REQUIRED`.

`--technical-distinction` accepts non-blank UTF-8 without NUL, from 1 through
1024 bytes. It bypasses only a current possible-overlap review. It cannot
bypass:

- direct ID or alias collision;
- invalid input/contract;
- unsafe path or file type;
- symlink/reparse point;
- manifest or implementation drift;
- resource/plan failure.

A new draft has no implementation bytes. The guard therefore never invents a
proposed full fingerprint and never labels metadata equality as an exact
fingerprint duplicate. Existing shortlisted candidates use the same Stage
012 anchored scanner and each file is hashed once.

The registry decision is repeated as the first action inside
`mutation::mutate_workspace`. A race cannot insert a conflicting tool between
preflight and the first write.

Clean databases use an immutable SQLite view. If an existing WAL is active,
preflight uses the existing production read-only WAL-aware view rather than
reading stale immutable state or blocking normal sequential create commands.

## Complexity

Hard bounds:

- registry tools: 1,000;
- creation candidates: 64;
- BM25 terms/document: 512;
- BM25 terms/operation: 262,144;
- Stage 012 implementation files/tool: 256;
- Stage 012 implementation bytes/tool: 64 MiB;
- Stage 012 directory depth: 16.

Creation search builds one bounded document-frequency model and scores each
document once. Work is `O(T * K)` for bounded terms plus `O(F + B)` for each
shortlisted implementation. There is no broad `O(T²)` comparison.

Alias resolution is one indexed bounded query. Canonical listing is one
keyset query plus one indexed alias batch query.

## Focused proof

Covered cases include:

- exact CLI syntax for add/list/remove/resolve;
- chain/cycle/shadow storage regression;
- requested/matched/canonical get facts;
- canonical validate/run paths and no alias directory;
- unchanged approval;
- canonical page semantics and explicit alias rows;
- one batch alias query;
- ordered safe `tool.alias_resolved`;
- ID/alias collision and overlap zero-write decisions;
- in-mutation race recheck before write;
- real BM25 ranking, not token-intersection naming;
- BM25 total-term fail-closed bound;
- distinction blank/NUL/oversize/privacy behavior;
- distinction cannot bypass collision or unsafe/drifting candidate;
- symlink and manifest drift rejection.

## Checks

```text
rtk cargo test --locked stage_013 -- --nocapture --test-threads=1
PASS 13/13

rtk cargo test --locked cli::tests::tool_ -- --nocapture --test-threads=1
PASS 18/18

rtk cargo test --locked tools::tests -- --nocapture --test-threads=1
PASS 83/83

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 691/691

rtk ./scripts/dev_verify.sh
PASS, including build, 691 tests, CLI proof, negative checks, and drift check
```

Final bookkeeping checks:

- `jq` ledger syntax and Stage 013/014 boundary: PASS;
- `git diff --check`: PASS.

## Requirement state

`RC5-ALS-002` is `DONE_LOCAL_CHECKS_PASSED`.

The Stage 013 guard portion of `RC5-CGD-001`, the Stage 013 tool-model portion
of `RC5-TOL-001`, the alias-event portion of `RC5-OBS-001`, and the focused
Stage 013 portion of `RC5-TST-001` are complete. Their later-stage portions
remain `TODO`.

## Next boundary

## Audit remediation

Creation preflight reuses the remediated Stage 012 scanner: manifest and
implementation identity are rechecked, descendant traversal is bounded, and
filesystem failures are returned through safe CLI reason codes.

Stages 001–010 remain `VERIFIED`. Stages 011–013 are
`DONE_LOCAL_CHECKS_PASSED`.

Verified through remains Stage 010. The cumulative audit remains due after
Stage 015. Continue with Stage 014: authoritative exact-only canonicalization.
