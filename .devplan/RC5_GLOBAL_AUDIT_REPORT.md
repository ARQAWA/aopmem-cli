# AOPMem v0.2.0-rc5 Global Audit Report

Date: `2026-07-18`

Stage: `STAGE_029`

Verdict: `PASS`

Open findings: P1 `0`; P2 `0`.

Native Windows runtime: `PENDING_DOGFOOD`.

## Scope and boundary

Independent audit of the complete RC5 worktree relative to baseline
`0af9b22c2e4a8217cbf6b1de558eb2181ce79a84` (`v0.2.0-rc4`). This audit did
not modify product code, release assets, installers, tests, or published Git
state. The only new files are this audit report and Stage 029 bookkeeping.

The Stage 028 retained proof supplies two sequential unchanged-source Windows
cross-build hashes. This audit independently verified the resulting flat asset
bytes, file types, checksum manifest, and PE imports. It does not claim a
native Windows runtime result.

## Fifteen cumulative requirement sweeps

| # | Sweep | Result | Evidence, files, tests | Open findings |
|---:|---|---|---|---|
| 1 | RC5 governance, decisions, and stage graph | PASS | `RC5_FINAL_DECISION_LOG.md` remains frozen; ledger JSON parses; stages 001–025 are cumulatively verified; Stage 026–028 handoffs are present. | None |
| 2 | Task protocol and private lifecycle state | PASS | `src/task/mod.rs`, `src/observability/task_state.rs`, `src/cli/mod.rs`; TP-01..TP-12 mapping in `RC5_REGRESSION_REPORT.md`; fresh locked suites pass. | None |
| 3 | Managed Block V2, Keeper, and adapter boundary | PASS | Canonical template has 18 sections, 124 useful lines, 10,835 UTF-8 bytes; `templates/skills/memory-keeper/SKILL.md`; MB-01..09 and MK-01..07 mapping. | None |
| 4 | Secrets, explicit persistence, and redaction | PASS | `src/redaction.rs`, `docs/SECRET_HANDLING.md`; canary scan found no raw durable canary; SEC-01..08 mapping and Stage 010 protected-sink tests. | None |
| 5 | Canonical tools, aliases, and deduplication | PASS | `src/tools/mod.rs`, `src/schema/mod.rs`, `RC5_TOOL_DEDUPE_REPORT.md`; TOOL-01..11 mapping; Stage 011–015 cumulative audit remains PASS. | None |
| 6 | Observability v2 and local read-only UI | PASS | `src/observability/**`, `src/ui/**`, `docs/LOCAL_OBSERVABILITY.md`, `docs/DESKTOP_UI.md`; exact loopback/token/read-only source and regression checks. | None |
| 7 | Windows publish and platform-check contract | PASS | `src/platform_publish.rs`, `src/platform_check.rs`, `RC5_WINDOWS_PUBLISH_REPORT.md`; WIN-01..06 mappings, PE cross-compile proof, no dynamic MSVC/UCRT imports. | Native runtime remains `PENDING_DOGFOOD`. |
| 8 | Audit repair and pending-marker durability | PASS | `src/audit_repair.rs`, `src/audit/**`, `docs/WINDOWS_AUDIT_REPAIR.md`; WIN-07..08 mapping, including marker restoration after post-remove sync failure. | None |
| 9 | Debug capsule and export privacy | PASS | `src/observability/export.rs`, `docs/DEBUG_CAPSULE.md`; WIN-09, redaction, deterministic ZIP, no-overwrite and no-self-write proofs. | None |
| 10 | Upgrade, recovery, and installer order | PASS | `src/upgrade/**`, `install/v0.2/**`, `docs/UPGRADE_TO_RC5.md`; UPG-01..11 mapping, fresh/mixed macOS proof, installer audit. | Native Windows dogfood remains pending. |
| 11 | Native Keeper dogfood and privacy-safe evidence | PASS | `RC5_AGENT_COMPLIANCE_REPORT.md`; DOG-01..10 evidence checksums pass 10/10; starts/applies/completions 10/10, reminders/duplicates/refusals 0. | None |
| 12 | Performance and bounded complexity | PASS | `RC5_PERFORMANCE_REPORT.md`, `benchmarks/rc5_stage26`; 4/4 benchmark checksums pass; raw median/p95 retained without unsupported percentage claim. | None |
| 13 | Full regression and required local commands | PASS | Fresh `fmt`, `clippy`, build, both locked suites, `dev_verify`, installer audit, macOS proof, and diff check pass; `RC5_REGRESSION_REPORT.md` maps TP/MB/MK/SEC/TOOL/WIN/UPG catalog. | None |
| 14 | Documentation, artifact ownership, and DoD traceability | PASS | 16 required ownership paths are non-empty; 62 requirement rows; reverse map has exact DoD 1–32; RC5 product docs, block, Keeper, installers, Windows and capsule docs exist. | Stage 030 still owns final RC report/matrix closure. |
| 15 | Release assets, checksums, and excluded-scope drift | PASS | `dist` has exactly three flat files; SHA-256 values match RC5 candidate; Darwin is arm64, Windows is PE32+ console x86-64; forbidden implementation scan is clean outside the verifier/test fixtures. | Native runtime remains `PENDING_DOGFOOD`; no release action performed. |

## Fresh command evidence

```text
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 768/768
cargo test --tests --locked                           PASS 768/768
scripts/dev_verify.sh                                 PASS
scripts/audit_v020_installers.sh                      PASS 14 groups
scripts/prove_rc5_macos.sh                            PASS
git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
(cd benchmark && shasum -a 256 -c SHA256SUMS)         PASS 4/4
(cd dogfood evidence && shasum -a 256 -c SHA256SUMS)  PASS 10/10
(cd dist && shasum -a 256 -c SHA256SUMS)              PASS 2/2
```

Release asset evidence:

```text
aopmem-darwin-arm64       594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e
aopmem-windows-x86_64.exe 150db4699c2f41c6e529f9606ac099c9ac6b4771b5084952f2cb5df3226d1b58
SHA256SUMS                6236d2cf502df5036609f202f541e38a12173321a0a85fbc83e388ed4548213a
```

`file` reports Mach-O arm64 and PE32+ console x86-64. `llvm-readobj`
reports only system DLL imports: `KERNEL32.dll`, `shell32.dll`,
`api-ms-win-core-synch-l1-2-0.dll`, `bcryptprimitives.dll`, `WS2_32.dll`,
`userenv.dll`, `ntdll.dll`, and `advapi32.dll`. No `VCRUNTIME`, `MSVCP`,
`UCRTBASE`, or `api-ms-win-crt` import is present.

## Finding and release boundary

There are no open P1 or P2 findings. Native Windows execution was not run and
is deliberately not promoted from `PENDING_DOGFOOD`. No commit, push, tag,
GitHub Release, real Windows installation, or backup deletion occurred.

Stage 029 is complete. Continue with `STAGE_030` for the final RC report,
final requirement-matrix closure, Definition-of-Done proof, and stop-condition
check.
