# RC5 Stage 022 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_023`

Native Windows runtime: `PENDING_DOGFOOD`

## Result

The official RC5 installers now use the native recovery state machine for an
update: process gate, pre-download direct-sibling full-home backup with a
deterministic manifest, verified download, backup adoption, stage, platform
check, audit repair, prepare, plan, one apply, native publish, one explicit
adapter sync, post-publish repair, health checks, task-start smoke,
observability, and capsule export.

The adapter is fail-fast before filesystem changes. Both
`AOPMEM_ACTIVE_ADAPTER` and `AOPMEM_ACTIVE_INSTRUCTION_FILE` are required and
only these pairs are accepted: Codex/`AGENTS.md`, Claude/`CLAUDE.md`,
Cursor/`.cursor/rules/aopmem.mdc`, and Copilot/`.github/copilot-instructions.md`.

Shell and PowerShell backup producers use bounded no-follow DFS manifests.
They reject links/reparse points and unsafe/non-regular entries. Windows
runtime proof is not available on this macOS host.

Re-opened P1 remediation: POSIX recursion now uses scoped positional inputs so
a child cannot corrupt a later sibling path. Both producers enforce 10,000
entries per directory, 100,000 total entries, depth 128, and a 32 MiB manifest
limit. The macOS audit creates `..x`, `.a`, `a/child`, `a.txt`, and a Unicode
file through the installer-owned producer, then proves that the real RC5 debug
binary accepts the produced manifest with `upgrade backup --adopt`.

Final high-review remediation replaces the PowerShell bulk copier with bounded
no-follow `Copy-DurableFile` traversal: every regular backup file is created
through an exclusive handle and `Flush(true)`. `MANIFEST.sha256` is likewise
created and flushed through a FileStream. PowerShell selector/file pairs are
case-sensitive exact lowercase values and run before TLS, console, environment,
or filesystem changes.

## Proof

```text
scripts/audit_v020_installers.sh                     PASS 14 groups
cargo test --locked upgrade::recovery::tests::installer_backup_adoption
                                                    PASS 2/2
cargo fmt --all -- --check                           PASS
cargo clippy --all-targets --locked -- -D warnings   PASS
cargo build --locked                                 PASS
git diff --check                                     PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json          PASS
```

The installer audit covers exact command order, failures, task-start smoke,
version allowlist (`0.1.0`, rc1–rc4), rejected rc5/unknown versions, selected
adapter pair, and retained recovery evidence.

The final review also proved the source tree is bounded and no-follow before
any bulk copy. Nested `MANIFEST.sha256` files are backed up normally. A root
file with that reserved name fails before full-home copy or download and is
never replaced. The manifest itself is created no-replace.

Final independent Stage 022 re-audit: `PASS`; P1 `0`; P2 `0`.
