# RC5 Stage 027 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next action: `STAGE_028`

Verified through: `STAGE_025`

Native Windows runtime: `PENDING_DOGFOOD`

## Result

The full §24 task, managed-block, memory-keeper, secret, tool, Windows,
upgrade, observability, and UI negative/security catalog is mapped to exact
passing tests in `.devplan/RC5_REGRESSION_REPORT.md`.

Focused testing exposed a reproducible macOS tool-run `SIGKILL`. Instrumented
failures proved that the current process cleanup had no live identity and made
no signal call. A standalone reproducer then proved macOS endpoint-security
rejection of newly executed hardlink anchors. The stable macOS executable
snapshot now uses fd-bound `fclonefileat`; only `ENOTSUP`/`EXDEV` use a bounded
fd-copy fallback. Source identity and metadata are rechecked, snapshots are
cleaned, fast same-group orphans remain terminated, and escaped descendants
remain covered.

No test was weakened. Three focused tests were added for 100 repeated short
runs, forced fallback, and in-place mutation; one negative test covers a fast
same-pgid orphan.

## Checks

```text
two isolated 100-run regression loops                 PASS
forced fallback / mutation / orphan / ancestor swap   PASS
original CLI regression                               PASS
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 768/768
cargo test --tests --locked                           PASS 768/768
scripts/dev_verify.sh                                 PASS
scripts/audit_v020_installers.sh                      PASS 14 groups
git diff --check                                      PASS
jq empty RC5_EXECUTION_LEDGER.json                    PASS
native Windows runtime                               PENDING_DOGFOOD
```

Stage 026 benchmark artifacts were not changed.

Self-review: `PASS`; P1 `0`; P2 `0`.
