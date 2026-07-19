# RC8 Proof Log

## Evidence

- External evidence root exists outside the Git repository.
- 53 files read recursively.
- `MANIFEST.json` verified 49 listed files by size and SHA-256.
- Mutation flags were false for live, repo, and external state.
- Sanitized conclusions are in `.devplan/RC8_FIELD_EVIDENCE.md`.

## Source-Level Confirmations

- Official installers do not contain normal `upgrade backup --adopt`.
- Update flow runs recovery inspect and fresh `upgrade backup --all-workspaces`.
- Safety Backup is preserved as emergency evidence.
- Legacy RC7 and RC8 Safety Backup names are rejected by explicit adopt.
- Upgrade Recovery Backup is created by Rust recovery code.
- Journal schema is v1 and target version is `0.2.0-rc8`.
- Inventory includes `.venv`, tools, runtimes, pending markers.
- Inventory excludes `.mutation.lock`, WAL, SHM, and product temp publish files.
- Windows path helper adds verbatim paths for Windows filesystem traversal.

## Rejected Or Modified Forensic Recommendations

- Did not trust empty `workspace_dirs`.
- Did not use installer Safety Backup as normal adopt source.
- Did not copy raw forensic inventory into the repository.
- Did not log proxy values, credentials, or secret contents.

## Checks Run

| Check | Result |
| --- | --- |
| `cargo fmt --all` | PASS |
| focused CLI parse test | PASS |
| recovery inspect tests | PASS |
| inventory policy test | PASS |
| Safety Backup adopt rejection test | PASS |
| installer audit | PASS, 14 groups |
| `cargo build --locked` | PASS |
| `cargo test --locked` | PASS, 775 tests |
| `cargo test --tests --locked` | PASS, 775 tests |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS |
| `scripts/dev_verify.sh` | PASS |
| macOS release asset build | PASS |
| macOS release asset version | PASS, `aopmem 0.2.0-rc8` |
| Windows x64 PE build/import check | PASS |
| `(cd dist && shasum -a 256 -c SHA256SUMS)` | PASS |
| `git diff --check` | PASS |

## Final Assets

| Asset | Size | SHA-256 |
| --- | ---: | --- |
| `dist/aopmem-darwin-arm64` | 9825376 | `84eb321603b0bb2dd8dc961946abebe56ccaa79cb1c170f6bd1fdcf63a8d58ca` |
| `dist/aopmem-windows-x86_64.exe` | 10740224 | `b27fe37afbb33c91a906a40f6667599ef6d33f40b179fb6e7e5300d578ad6839` |
| `dist/SHA256SUMS` | 178 | `2d2042c066699da4373dc5a8ca796a144cf4274e2e220d71f8f4ff6a4efd2421` |

External GitHub release mutation is still gated on standalone `+++`.
