# RC8 Global Audit Report

## Scope

RC8 scope is limited to Windows updater recovery, backup inventory, long-path
recovery IO, docs, tests, and release assets.

## Verified

- Version moved to `0.2.0-rc8`.
- Normal installer flow no longer adopts Safety Backup.
- Recovery inspect command exists.
- Fresh backup command requires `--all-workspaces`.
- Safety Backup names are rejected as normal adopt sources.
- `cargo test --locked` passed with 775 tests.
- `cargo test --tests --locked` passed with 775 tests.
- `cargo clippy --locked --all-targets -- -D warnings` passed.
- `scripts/dev_verify.sh` passed.
- Installer audit passed with 14 groups.
- macOS and Windows assets were rebuilt locally.

Open P1/P2: `0/0`.

## Risk

Native Windows runtime acceptance is still required after GitHub prerelease
publication. macOS proof and PE inspection do not claim native Windows PASS.
