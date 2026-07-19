# Windows native update

This path updates compatible AOPMem homes to `v0.2.0-rc8` on Windows 11 x64
with native Windows PowerShell 5.1.

## Boundaries

- No administrator rights.
- No WSL, Docker, Cargo, Rustup, source build, Node.js, or Codex CLI.
- Use `%USERPROFILE%\.aopmem`.
- Do not create repository-local `.aopmem`.
- Do not delete `.venv`, tools, runtimes, secrets, WAL, SHM, or journals
  manually.
- Do not put proxy credentials in commands, files, logs, or reports.

## Assets

| Asset | SHA-256 |
| --- | --- |
| `aopmem-windows-x86_64.exe` | `b27fe37afbb33c91a906a40f6667599ef6d33f40b179fb6e7e5300d578ad6839` |
| `SHA256SUMS` | `2d2042c066699da4373dc5a8ca796a144cf4274e2e220d71f8f4ff6a4efd2421` |
| `install.ps1` | `346162c857febaffd8384549f475a9175145e250b0e63f423c0158aef11c5938` |

Verify `SHA256SUMS`, then verify the selected binary before execution.
The binary must report exactly `aopmem 0.2.0-rc8`.

## Update Order

```text
process gate
→ installer Safety Backup
→ download and SHA-256/version verification
→ platform check --json
→ upgrade recovery inspect --json
→ upgrade backup --all-workspaces --json
→ upgrade stage --artifact ... --sha256 ... --json
→ staged audit repair --all-workspaces --json
→ upgrade prepare --all-workspaces --json
→ upgrade plan --all-workspaces --json
→ upgrade apply --all-workspaces --json --approved "+++" exactly once
→ upgrade publish --json
→ adapter sync
→ post-publish audit repair
→ doctor, verify, task smoke, observability, debug capsule
```

The installer Safety Backup is whole-home emergency evidence. It is not a
normal adopt source. The normal updater must never call
`upgrade backup --adopt`.

The Upgrade Recovery Backup is created by the verified RC8 binary. It owns the
recovery journal and the transactional apply boundary.

## Field Fixes

RC8 fixes the RC7 failure where a PowerShell-created Safety Backup was passed
to `upgrade backup --adopt`, then rejected because live home contained extra
operational files. RC8 classifies stale pre-apply evidence, keeps old backups,
and creates a fresh recovery backup.

RC8 also uses long-path-safe Rust filesystem operations for the recovery
backup boundary. It must work when `LongPathsEnabled=0`.

## Failure Rules

- Before apply starts, the installed binary stays unchanged.
- After apply starts, never auto-retry apply.
- Preserve every backup, journal, JSON report, and retained staged binary.
- Apply-started or unknown outcome evidence blocks a fresh run.
- `RECOVERY_LONG_PATH_FAILURE` is a product blocker, not a request to shorten
  paths or delete `.venv`.

Native Windows acceptance remains required for the exact published RC8 assets.
