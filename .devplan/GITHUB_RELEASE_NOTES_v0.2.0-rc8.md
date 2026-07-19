# AOPMem v0.2.0-rc8

RC8 fixes the Windows updater recovery flow.

## Changes

- Separates installer Safety Backup from Upgrade Recovery Backup.
- Removes normal installer use of `upgrade backup --adopt`.
- Adds `upgrade recovery inspect --json`.
- Adds fresh recovery backup via `upgrade backup --all-workspaces --json`.
- Adds recovery journal schema v1.
- Preserves tools, runtimes, `.venv`, secrets containers, audit evidence, and
  pending markers.
- Excludes `.mutation.lock`, WAL, SHM, and product temp files.
- Adds long-path-safe recovery filesystem handling for Windows.

## Assets

| Asset | SHA-256 |
| --- | --- |
| `aopmem-darwin-arm64` | `84eb321603b0bb2dd8dc961946abebe56ccaa79cb1c170f6bd1fdcf63a8d58ca` |
| `aopmem-windows-x86_64.exe` | `b27fe37afbb33c91a906a40f6667599ef6d33f40b179fb6e7e5300d578ad6839` |
| `SHA256SUMS` | `2d2042c066699da4373dc5a8ca796a144cf4274e2e220d71f8f4ff6a4efd2421` |

Native Windows acceptance must be run after publication.
