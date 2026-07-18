# RC5 Stage 024 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_025`

Verified through stage: `STAGE_020`

Native Windows runtime: `PENDING_DOGFOOD`

## Result

`scripts/prove_rc5_macos.sh` is a reproducible native Darwin arm64 harness.
It uses the exact tagged v0.1.0-rc3 binary, the exact published rc4 binary,
and a prebuilt rc5 candidate through the installer's isolated local-asset test
boundary.

One clean run passed fresh install, shared-home mixed schema `001/003` update,
real pending-audit repair, migration to operational `004` and observability
v2, exact source/backup database bytes, CLI-visible node, node-alias,
node-tag, and node-source preservation, exact tool bytes, selected-adapter
isolation, current health, both-workspace verify/task/observe/export, and
pre-download failure safety.

The official update trace contains exactly one apply and no init or adapter
seed. The durable full backup precedes download. Every backup remains present.

The complete evidence and limitations are in
`.devplan/RC5_MACOS_PROOF_REPORT.md`.

## Proof

```text
cargo build --locked                              PASS
sh -n scripts/prove_rc5_macos.sh                  PASS
scripts/prove_rc5_macos.sh                        PASS
cargo fmt --all -- --check                        PASS
cargo test --locked upgrade::                     PASS 63/63
scripts/audit_v020_installers.sh                  PASS 14 groups
git diff --check                                  PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json       PASS

fresh exact five questions                        PASS 5/5
fresh doctor/verify/task/observe/export            PASS
published v0.1 source SHA-256                     PASS
published rc4 source SHA-256                      PASS
shared home source schemas                        PASS 001 + 003
official update command order                     PASS
core apply invocation count                       PASS 1
update onboarding/init count                      PASS 0
pending audit marker repair                       PASS
target operational schemas                        PASS 004 + 004
target observability schemas                      PASS v2 + v2
CLI nodes/aliases/tags/sources preservation       PASS
tool filesystem byte preservation                 PASS
source/full-backup DB and sidecar bytes            PASS exact
selected adapter only + user text                 PASS
current rc4 workspace doctor/verify               PASS
both workspace task/observe/export                PASS
pre-download failure safety                       PASS
manual SQLite/WAL/SHM/admin/WSL workaround        NOT USED
native Windows runtime                            PENDING_DOGFOOD
```

Retained clean proof root:

```text
/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T//aopmem-rc5-stage24.q62Phs
```

Summary SHA-256:

```text
0c0f36b0e862f738242df0add5161243381492d588f96404c19db9a70f7dcb33
```

## Complexity and Rust review

The complexity skill was applied conservatively. The harness performs bounded
linear passes over two workspaces and two small tool trees. It introduces no
runtime query, N+1 path, cache, or production algorithm.

No Rust production code changed. Rust ownership/error/API behavior therefore
did not drift. Existing Stage 21–23 Rust checks remain authoritative.

## Findings

No product finding was opened.

The non-selected schema001 repository intentionally has no adapter file.
Its DB/schema/audit/tools health is ready and verify is clean; its aggregate
doctor result is not healthy only because adapter status is missing. Syncing it
would violate the selected-adapter-only contract.

Source schemas `001/003` predate `tool_aliases`; Stage 24 preserves their node
aliases. Tool-alias migration preservation stays proven by Stage 21 tests.

Independent review found one P2 evidence gap: the seeded schema001 tag and
secondary source were not compared in the first harness version. The harness
now uses bounded CLI `tag list` and `source list` projections, stable
normalization, and exact pre/post comparison. A full clean rerun passed.
Schema003 has no separately seeded node-source row; its node `source_ref` is
covered by the exact node projection.

Self-review: `PASS`; P1 `0`; P2 `0`.
