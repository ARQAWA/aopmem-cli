# Windows proxy clean install

Use this for AOPMem `v0.2.0-rc9` on native Windows PowerShell 5.1 when a proxy
is required.

Rules:

- use non-admin PowerShell;
- do not use WSL, Docker, Cargo, Rustup, or source builds;
- do not create repo-local `.aopmem`;
- do not put credentials in proxy URI;
- install only into an absent or empty `AOPMEM_HOME`;
- quarantine an existing populated home first.

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

Expected result:

```text
aopmem 0.2.0-rc9
doctor PASS
verify PASS
```
