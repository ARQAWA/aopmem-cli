# AOPMem v0.2.0-rc8 install prompt

Use this prompt to install or update AOPMem v0.2.0-rc8.
Run only the official installer and release assets.

````text
You are installing AOPMem v0.2.0-rc8 for the current project.

Do not use WSL, Docker, Cargo, Rustup, source builds, Codex CLI, or another
terminal. Do not use administrator rights. Do not create a repo-local
`.aopmem`.

Set exactly one active adapter pair before running the installer:

- `codex` / `AGENTS.md`
- `claude` / `CLAUDE.md`
- `cursor` / `.cursor/rules/aopmem.mdc`
- `copilot` / `.github/copilot-instructions.md`

Supported hosts:

- macOS Apple Silicon: `Darwin arm64`
- Windows 11 x64: native Windows PowerShell 5.1

Trusted RC8 release assets:

| Asset | Size | SHA-256 |
| --- | ---: | --- |
| `aopmem-darwin-arm64` | 9825376 | `84eb321603b0bb2dd8dc961946abebe56ccaa79cb1c170f6bd1fdcf63a8d58ca` |
| `aopmem-windows-x86_64.exe` | 10740224 | `b27fe37afbb33c91a906a40f6667599ef6d33f40b179fb6e7e5300d578ad6839` |
| `SHA256SUMS` | 178 | `2d2042c066699da4373dc5a8ca796a144cf4274e2e220d71f8f4ff6a4efd2421` |
| `install.ps1` | 69643 | `346162c857febaffd8384549f475a9175145e250b0e63f423c0158aef11c5938` |
| `install.sh` | 40627 | `139d26d278c0c10f5e213deefc8ed5e799f7eb619845e4b8374aea9ec475a4db` |

Windows bootstrap with proxy:

```powershell
$proxyUri = [Uri]"<PROXY_URI>"
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) `
    ("aopmem-rc8-bootstrap-" + [Guid]::NewGuid().ToString("N"))
$null = New-Item -ItemType Directory -Path $tempRoot -ErrorAction Stop
$installer = Join-Path $tempRoot "install.ps1"
Invoke-WebRequest `
    -Uri "https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc8/install/v0.2/install.ps1" `
    -OutFile $installer `
    -UseBasicParsing `
    -Proxy $proxyUri `
    -ProxyUseDefaultCredentials `
    -TimeoutSec 900 `
    -ErrorAction Stop
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($installerHash -ine "346162c857febaffd8384549f475a9175145e250b0e63f423c0158aef11c5938") {
    throw "INSTALLER_SHA256_MISMATCH"
}
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $installer `
    -AssetBaseUri "https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc8" `
    -ProxyUri $proxyUri `
    -ProxyUseDefaultCredentials
```

Direct Windows bootstrap:

```powershell
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) `
    ("aopmem-rc8-bootstrap-" + [Guid]::NewGuid().ToString("N"))
$null = New-Item -ItemType Directory -Path $tempRoot -ErrorAction Stop
$installer = Join-Path $tempRoot "install.ps1"
Invoke-WebRequest `
    -Uri "https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc8/install/v0.2/install.ps1" `
    -OutFile $installer `
    -UseBasicParsing `
    -TimeoutSec 900 `
    -ErrorAction Stop
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($installerHash -ine "346162c857febaffd8384549f475a9175145e250b0e63f423c0158aef11c5938") {
    throw "INSTALLER_SHA256_MISMATCH"
}
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $installer `
    -AssetBaseUri "https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc8"
```

Never put proxy credentials in the URI, logs, files, or final report.

Fresh flow:

1. Verify `SHA256SUMS` and the selected binary.
2. Require the binary to report `aopmem 0.2.0-rc8`.
3. Publish the binary atomically.
4. Run normal `aopmem init`.
5. Seed exactly the selected adapter file.
6. Run `doctor`, `verify`, `task start`, and observability checks.

Update flow:

1. Close active AOPMem processes.
2. Create installer Safety Backup of the current home.
3. Download and verify RC8.
4. Run `platform check --json`.
5. Run `upgrade recovery inspect --json`.
6. Run `upgrade backup --all-workspaces --json`.
7. Run `upgrade stage --artifact <verified-binary> --sha256 <hash> --json`.
8. Run staged `audit repair --all-workspaces --json` if needed.
9. Run `upgrade prepare --all-workspaces --json`.
10. Run `upgrade plan --all-workspaces --json`.
11. Require `ready=true` and `writes_performed=false`.
12. Run `upgrade apply --all-workspaces --json --approved "+++"` exactly once.
13. Run `upgrade publish --json`.
14. Sync the selected adapter.
15. Run post-publish audit repair, doctor, verify, task smoke, observability,
    and debug capsule.

The installer Safety Backup is emergency evidence only. It is never passed to
normal `upgrade backup --adopt`. Normal RC8 uses a fresh Upgrade Recovery
Backup made by the verified RC8 binary.

Failure rules:

- Before apply starts, the old binary must remain unchanged.
- After apply starts, never auto-retry apply.
- Keep every Safety Backup, Upgrade Recovery Backup, journal, JSON report,
  retained staged binary, and debug capsule.
- If `upgrade recovery inspect` reports apply-started evidence, stop and
  preserve evidence.
- If long path failure occurs, report `RECOVERY_LONG_PATH_FAILURE`.
````
