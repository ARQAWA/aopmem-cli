# RC6 Proof Log

## Stage 01 — baseline, failure transcription, worktree classification

Status: `VERIFIED`

### Baseline commands

```text
git status --short                     PASS; clean worktree
git branch --show-current              main
git remote -v                          origin https://github.com/ARQAWA/aopmem-cli.git
git log -5 --oneline                   HEAD d2eb26b release: v0.2.0-rc5
git show-ref --tags                    v0.2.0-rc5 preserved
git rev-parse HEAD                     d2eb26bc36b20349061bf89d23a152c8c0b161bf
git rev-parse origin/main              d2eb26bc36b20349061bf89d23a152c8c0b161bf
```

Classification: no user, generated, or untracked changes existed before RC6
work. RC6 may be isolated in one later local release commit. No reset,
checkout, stash, amend, tag movement, or RC5 asset modification occurred.

### Source and contract review

Reviewed `Cargo.toml`, `Cargo.lock`, unified publication and platform-check
code, all known publication consumers, RC5 Windows report, RC5 global audit,
RC5 release candidate, frozen decision log, requirements matrix, proof-log
stage index, current installer/docs, and exact publish/search symbols.

Recorded commands: `rg --files`, exact `rg -n` symbol sweeps, source and
evidence reads. Relevant production boundary is `src/platform_publish.rs`;
its Windows file opens live in `src/audit/anchored.rs`.

Next: complete Stage 01 evidence review, then perform an independent Stage 02
handle/share-mode audit before any product-code change.

### Completion

Files: `.devplan/RC6_CURRENT_STAGE.md`,
`.devplan/RC6_EXECUTION_LEDGER.json`, `.devplan/RC6_PROOF_LOG.md`, and
`.devplan/RC6_WINDOWS_ACCEPTANCE_FAILURE.md`.

Commands: `jq empty .devplan/RC6_EXECUTION_LEDGER.json`; `git diff --check`.

Result: `PASS`. Ledger has exactly 10 stages. Native Windows evidence is
transcribed without a cause claim. Worktree began clean and RC5 refs remain
untouched.

Next stage: `02 — exact Windows handle/share-mode root-cause audit`.

## Stage 02 — exact Windows handle/share-mode root-cause audit

Status: `VERIFIED`

Files: `.devplan/RC6_WINDOWS_PUBLISH_ROOT_CAUSE.md` and the source evidence
in `src/platform_check.rs`, `src/platform_publish.rs`, and
`src/audit/anchored.rs`.

Commands: exact symbol/source inspection; `rtk cargo test platform_publish
--locked` (`7 passed`); independent fresh audit command
`rtk cargo test --locked platform_check` (`4 passed`).

Result: `PASS`. Exact conflict is live source writer `DELETE` access against
the validation reader's `FILE_SHARE_READ | FILE_SHARE_WRITE` share mode. Error
32 occurs during `validate_source`, before any OS publication or user-data
mutation. No antivirus claim is made.

Next stage: `03 — minimal unified publish remediation`.

## Stage 03 — minimal unified publish remediation

Status: `DONE`

Files: `src/platform_publish.rs`,
`.devplan/RC6_WINDOWS_PUBLISH_REMEDIATION.md`.

Commands: `cargo fmt --all -- --check`; `rtk cargo test platform_publish
--locked` (`7 passed`); `rtk cargo test platform_check --locked` (`4 passed`);
`git diff --check`.

Result: `PASS`. Source writer flushes and captures identity, then drops before
source validation. Validation and optional destination readers leave their
scopes before `os_publish`; final destination validation remains after commit.
The expected handle/share contract and syscalls are unchanged.

Next stage: `04 — platform-check and sharing-violation regression tests`.

## Stage 04 — platform-check and sharing-violation regression tests

Status: `DONE`

Files: `src/platform_publish.rs`, `src/platform_check.rs`.

Added proof:

- test-only lifecycle trace proves writer drop before source validation,
  source/destination validation close before OS publish, and final validation
  after commit;
- deterministic source-validation error `32` proves exact structured state and
  platform-check cleanup with no user-data or observability write;
- native Windows-only legacy `CreateFileW` reader uses
  `FILE_SHARE_READ | FILE_SHARE_WRITE` and proves the live writer's `DELETE`
  access produces raw error `32`; corrected no-replace publication succeeds;
- safe Windows-only source-validation diagnostics are serialized only when
  present: role, access, share mode, disposition, flags, and prior-writer
  closed expectation. They contain no path, secret, or numeric handle.

Commands: `cargo fmt --all -- --check`; `rtk cargo test platform_publish
--locked` (`9 passed`); `rtk cargo test platform_check --locked` (`5 passed`);
`rtk cargo test --tests --locked` (`771 passed`); `git diff --check`.

Result: `PASS`. No-replace, replace, Unicode/long name, reparse/path,
structured-error, private cleanup, and workspace-independent platform-check
contracts remain covered.

Next stage: `05 — backup/audit/export consumer integration proof`.

## Stage 05 — backup/audit/export consumer integration proof

Status: `VERIFIED`

Source sweep: every regular-file call site uses `publish_regular`:

- platform check: `src/platform_check.rs`;
- online SQLite backup: `src/upgrade/backup.rs`;
- audit `memory.sql` and Git objects/refs: `src/audit/mod.rs` and
  `src/audit/anchored_git.rs`;
- debug capsule ZIP: `src/observability/export.rs`;
- recovery journal, backup manifest/home files, staged and installed binaries:
  `src/upgrade/recovery.rs`;
- managed update files: `src/upgrade/apply.rs`.

Audit/apply/Git require `require_committed_validated_clean`. Backup additionally
requires durability and reopens the final SQLite database. Recovery uses its
stricter `require_recovery_publish`; the pre-existing installed-binary
durability-warning path requires committed, validated, clean publication and
never reruns apply. Capsule intentionally reports a committed+validated
durability/cleanup uncertainty as its established typed warning.

Commands:

```text
rtk cargo test upgrade::backup --locked          PASS 5 passed
rtk cargo test audit_repair --locked             PASS 1 passed
rtk cargo test audit:: --locked                  PASS 35 passed
rtk cargo test audit::anchored_git --locked      PASS 5 passed
rtk cargo test observability::export --locked    PASS 22 passed
rtk cargo test upgrade::recovery --locked        PASS 23 passed
rtk cargo test upgrade::apply --locked           PASS 17 passed
git diff --check                                 PASS
```

Cumulative Stage 01–05 local audit: `PASS`; P1 `0`; P2 `0`. Per direct user
instruction, all subagents were closed before this audit; this record does not
claim an additional subagent review.

The only `005` strings remain pre-existing forced-failure test migrations in
`src/schema/mod.rs`; `MIGRATIONS` remains unchanged and production target
schema is `004`.

Next stage: `06 — version, installer, updater, docs, recovery support`.

## Stage 06 — version, installer, updater, docs, recovery support

Status: `VERIFIED`

Files: `Cargo.toml`, `Cargo.lock`, `src/upgrade/apply.rs`,
`src/upgrade/recovery.rs`, `src/cli/mod.rs`, `src/tools/mod.rs`, both
official installers, installer prompt/audit, current Windows/update/export
docs, `docs/UPGRADE_TO_RC6.md`, and the RC6 native acceptance prompt.

RC6 now emits and validates only RC6 recovery names: journal, staged binary,
full-home backup, backup-run, and temporary publication names. The updater
accepts existing v0.1, rc1, rc2, rc3, rc4, and rc5 binaries; RC6 itself stays
an unsupported update source. Schema target remains `004`; no production
migration `005` was added.

The installer order remains process gate, durable backup, download/verify,
staged platform check, pending audit repair, prepare, plan, exactly one apply,
binary publish, adapter sync, post-publish audit repair, health, and export.
The acceptance prompt contains the final RC6 release URLs and the Stage 08
actual Windows SHA-256.

Commands:

```text
cargo fmt --all -- --check                 PASS
rtk cargo test upgrade::recovery --locked  PASS 23 passed
rtk cargo test upgrade::apply --locked     PASS 17 passed
rtk scripts/audit_v020_installers.sh       PASS
git diff --check                           PASS
```

Schema sweep: `MIGRATIONS` remains at `004`; the only `005` identifiers are
pre-existing forced-failure test fixtures in `src/schema/mod.rs`.

Next stage: `07 — full local regression and isolated macOS proof`.

## Stage 07 — full local regression and isolated macOS proof

Status: `VERIFIED`

Full local commands:

```text
cargo fmt --all -- --check                       PASS
rtk cargo clippy --all-targets --locked -- -D warnings  PASS
rtk cargo build --locked                         PASS
rtk cargo test --locked                          PASS
rtk cargo test --tests --locked                  PASS
rtk scripts/dev_verify.sh                        PASS
rtk scripts/audit_v020_installers.sh             PASS
git diff --check                                 PASS
```

Focused proof:

```text
platform_publish      PASS 9
platform_check        PASS 5
upgrade::             PASS 63
audit_repair          PASS 1
observability::export PASS 22
task_                 PASS 50
adapter::             PASS 16
stage_009_            PASS 4
stage_014_            PASS 11
stage_015_            PASS 4
observability::       PASS 103
observability::ui     PASS 3
```

`scripts/prove_rc6_macos.sh` passed on Darwin arm64. It fresh-installs RC6,
updates published RC4/schema-003 and RC5/schema-004 fixtures, preserves exact
backup source bytes, proves one apply/no onboarding, and covers health, task
lifecycle, dedupe plan, and debug export. The separate loopback UI command
served its local read-only HTML shell and was shut down. Full evidence is in
`.devplan/RC6_MACOS_PROOF_REPORT.md`.

Native Windows runtime is not claimed. The RC6 acceptance prompt now contains
the final Windows asset SHA-256 and remains `PENDING_DOGFOOD`.

Next stage: `08 — release assets and supply-chain proof`.

## Stage 08 — release assets and supply-chain proof

Status: `VERIFIED`

Commands:

```text
scripts/build_macos_arm.sh                         PASS
scripts/build_windows_x64_from_macos.sh            PASS twice, unchanged source
file dist/aopmem-darwin-arm64                      PASS Mach-O arm64
./dist/aopmem-darwin-arm64 --version               PASS aopmem 0.2.0-rc6
file dist/aopmem-windows-x86_64.exe                PASS PE32+ x86-64
llvm-readobj --coff-imports ...                    PASS no dynamic MSVC/UCRT
shasum -a 256 -c dist/SHA256SUMS                   PASS both binary assets
```

The two independent unchanged-source Windows cross-builds both produced
`8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7`.
Final hashes, exact sizes, types, imports, and macOS version are recorded in
`.devplan/RC6_ASSET_REPORT.md`. Installer and RC6 release documents now carry
the audited actual asset digests.

Native Windows runtime is not claimed. Its required PowerShell 5.1 dogfood
acceptance remains `PENDING_DOGFOOD`.

Next stage: `09 — independent global audit, P1=0, P2=0`.

## Stage 09 — independent global audit

Status: `VERIFIED`; P1 `0`, P2 `0`.

The user explicitly closed all subagents. A fresh local independent-style
audit reviewed the full RC6 diff, goal, RC5 frozen evidence, current source,
native failure record, installer/update contracts, and assets without claiming
a subagent review.

Required commands all passed: formatting, Clippy, build, 771 Rust/integration
tests, `scripts/dev_verify.sh`, installer audit, diff check, SHA manifest, and
ledger JSON validation. Static sweeps found no `src/schema` diff, no production
`005`, no forbidden changed path, and no candidate secret marker. Full details
are in `.devplan/RC6_GLOBAL_AUDIT_REPORT.md`.

Files created: global audit report, RC6 release candidate, and GitHub release
notes. Native Windows remains `PENDING_DOGFOOD`; external publication still
requires the exact gate approval.

Next stage: `10 — local commit and externally gated publication`.
