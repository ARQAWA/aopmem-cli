# Windows proxy installation

Use this path for AOPMem `v0.2.0-rc8` on native Windows PowerShell 5.1 when
GitHub needs a corporate proxy.

Proxy URI rules:

- absolute `http` or `https`;
- no username;
- no password;
- no query or fragment;
- no logging of the real value.

## Proxy Bootstrap

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
```

Run the installer in the same console:

```powershell
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $installer `
    -AssetBaseUri "https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc8" `
    -ProxyUri $proxyUri `
    -ProxyUseDefaultCredentials
```

## Direct Bootstrap

Omit every proxy flag when no proxy is configured.

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

## Transport Guarantees

- Proxy precedence: explicit, env HTTPS, env HTTP, system default, direct.
- Manual 301, 302, 303, 307, and 308 handling.
- HTTPS-only redirects.
- Same proxy across redirects.
- Loop and 10-hop checks.
- Create-new partial downloads under a private temp root.
- Original network exception type and message preserved.
- No unsafe `Exception.Response` access.
