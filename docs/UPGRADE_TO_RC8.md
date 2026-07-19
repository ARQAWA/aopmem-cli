# Official upgrade to AOPMem v0.2.0-rc8

`v0.2.0-rc8` fixes the Windows updater recovery boundary from RC7.

## Audited Assets

| Asset | Size | SHA-256 |
| --- | ---: | --- |
| `aopmem-darwin-arm64` | 9825376 | `84eb321603b0bb2dd8dc961946abebe56ccaa79cb1c170f6bd1fdcf63a8d58ca` |
| `aopmem-windows-x86_64.exe` | 10740224 | `b27fe37afbb33c91a906a40f6667599ef6d33f40b179fb6e7e5300d578ad6839` |
| `SHA256SUMS` | 178 | `2d2042c066699da4373dc5a8ca796a144cf4274e2e220d71f8f4ff6a4efd2421` |

## Windows Flow

```text
Safety Backup
→ verified RC8 binary platform check
→ recovery inspect
→ fresh Upgrade Recovery Backup
→ recovery journal phase 01
→ stage
→ prepare
→ plan
→ apply once
→ publish
```

Normal installer flow never uses `upgrade backup --adopt`. Explicit adopt is
diagnostic only and rejects installer Safety Backup names.

## Field Fix

RC7 created a full-home Safety Backup and tried to adopt it as recovery state.
The live home had at least 250 additional operational entries compared with
that manifest. The first proven extra entry was a workspace `.mutation.lock`.
There were no RC7 recovery journals and no apply attempts.

RC8 creates recovery state with one Rust inventory engine. It includes tools,
runtimes, `.venv`, secrets containers, audit evidence, observability, and
pending markers. It excludes only documented product ephemeral files.

## Long Paths

Windows field evidence has `LongPathsEnabled=0`. RC8 uses long-path-safe
Windows filesystem paths internally and keeps logical relative paths in
manifests.
