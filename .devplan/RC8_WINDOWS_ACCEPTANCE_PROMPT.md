# RC8 Windows acceptance prompt

Use this only after the GitHub prerelease and assets exist.

Validate `AOPMem v0.2.0-rc8` on native Windows 11 x64 with Windows
PowerShell 5.1, non-admin, no WSL, no Docker, no Cargo, no source build.

Inputs:

```text
release: https://github.com/ARQAWA/aopmem-cli/releases/tag/v0.2.0-rc8
asset base: https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc8
installer: https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc8/install/v0.2/install.ps1
```

Required hashes:

| Asset | SHA-256 |
| --- | --- |
| `install.ps1` | `346162c857febaffd8384549f475a9175145e250b0e63f423c0158aef11c5938` |
| `aopmem-windows-x86_64.exe` | `b27fe37afbb33c91a906a40f6667599ef6d33f40b179fb6e7e5300d578ad6839` |
| `SHA256SUMS` | `2d2042c066699da4373dc5a8ca796a144cf4274e2e220d71f8f4ff6a4efd2421` |

Acceptance checks:

- host is Windows 11 x64 build 22631 or later;
- PowerShell is 5.1 Desktop;
- user is non-admin;
- `LongPathsEnabled=0` is recorded;
- proxy path uses redacted URI only;
- current installed binary and SHA-256 are recorded;
- exact recovery parent has no apply-started blocker;
- installer creates Safety Backup and keeps it;
- installer does not call `upgrade backup --adopt`;
- `upgrade recovery inspect --json` runs;
- `upgrade backup --all-workspaces --json` creates Upgrade Recovery Backup;
- journal schema is v1 and target is `0.2.0-rc8`;
- apply attempts exactly one;
- installed binary reports `aopmem 0.2.0-rc8`;
- `.venv`, tools, runtimes, secrets containers, audit evidence, and pending
  marker are preserved;
- `.mutation.lock`, WAL, and SHM are not copied into recovery backup;
- doctor healthy, verify clean, task smoke ok, observability ok;
- no repository-local `.aopmem`;
- debug capsule exported.

Return exactly one result:

- `RC8_ACCEPTED`
- `RC8_REJECTED`
- `RELEASE_NOT_READY`
