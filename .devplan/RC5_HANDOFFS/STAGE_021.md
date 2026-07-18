# RC5 Stage 021 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_022`

Native Windows runtime: `PENDING_DOGFOOD`

## Result

RC5 now has four separate native recovery steps:
`upgrade backup`, `upgrade stage --artifact --sha256`,
`upgrade apply --all-workspaces`, and `upgrade publish`. Apply never creates a
backup, downloads an artifact, or publishes the installed binary. Publish
never invokes core apply.

The path-private journal is an immutable ordered checkpoint chain outside
`AOPMEM_HOME`. Every transition uses Atomic Publish V2 `NoReplace` and requires
commit, final validation, cleanup, and confirmed durability. Missing first or
middle checkpoints are rejected. Structured publish phase, strategy, I/O kind,
OS code, commit, and durability details are preserved.

The full-home backup uses anchored, no-follow traversal. Its deterministic
SHA-256/size/mode manifest is written while files stream, with explicit entry,
per-directory, depth, and manifest-size bounds. The journal binds the exact
home identity, backup manifest, staged hash, and frozen workspace root,
database, Observability, and source-schema identities.

After a crash in `apply_started`, recovery advances only when every frozen
workspace proves operational schema `004` and Observability v2. Otherwise it
fails closed and never invokes core apply again. Integrated fault tests cover
every effect/checkpoint window, exact-once core invocation, mixed schema
`001`/`003`, Observability v1-to-v2, and publish-before-checkpoint replay.

`apply_core_all_workspaces` stops after durable workspace migration. Managed
asset refresh, adapter sync, doctor, and verify remain Stage 022 installer
work.

## Review remediation

The first independent Stage 021 review result was `FAIL`: P1 `7`, P2 `5`.

- P1-A fixed retained binary replay, executable mode, stale temporaries,
  full-home no-follow copy, and journal/home/manifest binding.
- P1-B fixed exact frozen workspace identity and drift gates, mandatory
  Observability v2, reconciliation, and core-only post-publish separation.
- P1-C fixed separate native commands, immutable durable checkpoints,
  bounded streaming manifest work, transition fault proof, CLI guards, and
  exact operator flow.

The final independent re-audit returned `PASS`; P1 `0`; P2 `0`.

The remaining re-audit then found five more local gaps:

- old `rc1`–`rc4` binaries could not invoke a pre-download RC5 command;
- committed/validated Windows `ReplaceFileW` durability uncertainty was
  treated as a hard binary-publish failure;
- serialized checkpoints had no pre-write 1 MiB rejection;
- recovery publish errors lacked the complete typed CLI detail contract;
- stale retain/publish temporaries survived an idempotent existing-file return.

They are now remediated locally. RC5 can adopt an exact installer-created
pre-download sibling backup without making a second backup. Only committed,
final-validated, cleaned installed-binary replacement may complete with a
top-level durability warning; checkpoint and backup transitions remain
strict. Published replay always revalidates the installed digest. Typed JSON
uses only path-private roles. Explicit tests prove tampered/foreign/changed
home rejection, checkpoint size, warning/checkpoint ordering, and temporary
cleanup.

The final remediation closes two more findings. Explicit `upgrade publish`
now repairs a missing or mismatched installed binary from the verified
retained artifact even after phase `published`. It does not rewrite the
immutable checkpoint and cannot invoke core apply. Recovery entry points also
run bounded cleanup for only the two UUID temporary-name families; overflow
and matching unsafe/reparse entries fail closed. Tests prove repaired digest,
retained preservation, unchanged core invocation count, idempotent checkpoint
cleanup, overflow-before-removal, and symlink target preservation.

## Proof

```text
cargo test upgrade:: --locked                         PASS 63/63
focused CLI native/adopt/error/warning contract       PASS 3/3
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 759/759
cargo test --tests --locked                           PASS 759/759
./scripts/dev_verify.sh                               PASS
./scripts/audit_v020_installers.sh                    PASS 11 groups
git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
```

Native Windows execution is not available here. Do not claim native Windows
PASS.

Final independent Stage 021 re-audit: `PASS`; P1 `0`; P2 `0`.
