# AOPMem v0.2.0-rc9 clean install prompt

Use this prompt for AOPMem v0.2.0-rc9.
RC9 has no in-place updater. Install only into an absent or empty
`AOPMEM_HOME`.

You are installing AOPMem v0.2.0-rc9 for the current project.

Do not use WSL, Docker, Cargo, Rustup, source builds, Codex CLI, or another
terminal. Do not use administrator rights. Do not create a repo-local
`.aopmem`.

If `%USERPROFILE%\.aopmem` already exists and contains data, stop.
Quarantine it first. Do not rename, delete, or modify it automatically.

Supported hosts:

- macOS Apple Silicon: `Darwin arm64`
- Windows 11 x64: native Windows PowerShell 5.1

Expected binary version:

`aopmem 0.2.0-rc9`

Release assets:

| Asset | Size | SHA-256 |
| --- | ---: | --- |
| `aopmem-darwin-arm64` | 9329872 | `16ed13b6b5df96a345a93a406cebb5394704a63b665bac2c836d8adca1f15999` |
| `aopmem-windows-x86_64.exe` | 10093056 | `de3e58ac70d00636532536fc815152c9efa952f0571292abfb496db245e9fd9e` |
| `SHA256SUMS` | 178 | `5c9a25bbff2b2b015d98de5604e85ca92d03693c12e8247e0c6aa802249f73f2` |
| `install.ps1` | 11330 | `0e60b6b7322ce157b27b88bc3e5123342a2bea34eb94c994e871a7da173ff63f` |
| `install.sh` | 6035 | `9cb47d956aff8e37c46a3c22d097e18d9dfdf03d400b56e68f2ebf7bd3342f33` |

Windows direct bootstrap:

```powershell
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) `
    ("aopmem-rc9-bootstrap-" + [Guid]::NewGuid().ToString("N"))
$null = New-Item -ItemType Directory -Path $tempRoot -ErrorAction Stop
$installer = Join-Path $tempRoot "install.ps1"
Invoke-WebRequest `
    -Uri "https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc9/install/v0.2/install.ps1" `
    -OutFile $installer `
    -UseBasicParsing `
    -TimeoutSec 900 `
    -ErrorAction Stop
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($installerHash -ine "0e60b6b7322ce157b27b88bc3e5123342a2bea34eb94c994e871a7da173ff63f") {
    throw "INSTALLER_SHA256_MISMATCH"
}
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $installer `
    -AssetBaseUri "https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc9"
```

Windows bootstrap with proxy:

```powershell
$proxyUri = [Uri]"<PROXY_URI>"
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) `
    ("aopmem-rc9-bootstrap-" + [Guid]::NewGuid().ToString("N"))
$null = New-Item -ItemType Directory -Path $tempRoot -ErrorAction Stop
$installer = Join-Path $tempRoot "install.ps1"
Invoke-WebRequest `
    -Uri "https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc9/install/v0.2/install.ps1" `
    -OutFile $installer `
    -UseBasicParsing `
    -Proxy $proxyUri `
    -ProxyUseDefaultCredentials `
    -TimeoutSec 900 `
    -ErrorAction Stop
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($installerHash -ine "0e60b6b7322ce157b27b88bc3e5123342a2bea34eb94c994e871a7da173ff63f") {
    throw "INSTALLER_SHA256_MISMATCH"
}
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $installer `
    -AssetBaseUri "https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc9" `
    -ProxyUri $proxyUri `
    -ProxyUseDefaultCredentials
```

Clean flow:

1. Check environment.
2. Download immutable asset.
3. Verify `SHA256SUMS` and selected binary.
4. Run `platform check`.
5. Create fresh `AOPMEM_HOME`.
6. Install binary.
7. Run normal initialization for the current repository.
8. Sync adapter.
9. Run `doctor` and `verify`.

Existing-home behavior:

- absent home: continue clean install;
- existing empty home: continue clean install;
- existing populated home: fail with `CLEAN_INSTALL_REQUIRES_EMPTY_HOME`;
- populated home remains untouched.

Never put proxy credentials in the URI, logs, files, or final report.
