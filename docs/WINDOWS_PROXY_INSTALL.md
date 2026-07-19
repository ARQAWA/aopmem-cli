# Windows proxy installation

Use this path for AOPMem `v0.2.0-rc7` on native Windows PowerShell 5.1 when
GitHub requires a corporate proxy. Use an ordinary user account. Do not use
administrator rights, WSL, another shell, Cargo, Rustup, or a source build.

Use the generic proxy value supplied by the environment:

```text
<PROXY_URI>
```

The URI must be absolute, use `http` or `https`, and contain no username or
password. Never store proxy credentials in a command, file, log, or report.

## Proxy bootstrap

This downloads the immutable tagged installer into a new unique directory
under `%TEMP%`. `Invoke-WebRequest` uses normal redirects. Do not add
`-MaximumRedirection 0`.

```powershell
$proxyUri = [Uri]"<PROXY_URI>"
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) `
    ("aopmem-rc7-bootstrap-" + [Guid]::NewGuid().ToString("N"))
$null = New-Item -ItemType Directory -Path $tempRoot -ErrorAction Stop
$installer = Join-Path $tempRoot "install.ps1"

Invoke-WebRequest `
    -Uri "https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc7/install/v0.2/install.ps1" `
    -OutFile $installer `
    -UseBasicParsing `
    -Proxy $proxyUri `
    -ProxyUseDefaultCredentials `
    -TimeoutSec 900 `
    -ErrorAction Stop

$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($installerHash -ine "c306d664664852b4f60bf834fa2f5d798312e8646ef9921eae9d14007bd5c949") {
    throw "INSTALLER_SHA256_MISMATCH"
}
```

Omit `-ProxyUseDefaultCredentials` in both bootstrap and installer invocation
when integrated proxy credentials are not required.

## Official installer

Set the exact active adapter pair required by the current project. Codex uses:

```powershell
$env:AOPMEM_ACTIVE_ADAPTER = "codex"
$env:AOPMEM_ACTIVE_INSTRUCTION_FILE = "AGENTS.md"
```

Run the downloaded installer in the same console:

```powershell
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $installer `
    -AssetBaseUri "https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc7" `
    -ProxyUri $proxyUri `
    -ProxyUseDefaultCredentials
```

The installer reuses the same proxy for `SHA256SUMS`, the Windows binary, and
every redirect. Integrated credentials are assigned only to the proxy object.
No credentials are sent directly to GitHub.

## Direct connection

When no proxy is configured, omit every proxy flag:

```powershell
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) `
    ("aopmem-rc7-bootstrap-" + [Guid]::NewGuid().ToString("N"))
$null = New-Item -ItemType Directory -Path $tempRoot -ErrorAction Stop
$installer = Join-Path $tempRoot "install.ps1"
Invoke-WebRequest `
    -Uri "https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc7/install/v0.2/install.ps1" `
    -OutFile $installer `
    -UseBasicParsing `
    -TimeoutSec 900 `
    -ErrorAction Stop
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($installerHash -ine "c306d664664852b4f60bf834fa2f5d798312e8646ef9921eae9d14007bd5c949") {
    throw "INSTALLER_SHA256_MISMATCH"
}
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $installer `
    -AssetBaseUri "https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc7"
```

Do not pass empty `-ProxyUri` or a credential switch without a proxy.

## Transport guarantees

- Proxy precedence: explicit, uppercase/lowercase HTTPS environment,
  uppercase/lowercase HTTP environment, system default, direct.
- `HttpClientHandler.AllowAutoRedirect=false`.
- Manual 301, 302, 303, 307, and 308 handling.
- HTTPS-only redirect targets, no userinfo, loop rejection, maximum 10 hops.
- Streamed downloads and create-new publication under the private temp root.
- Existing destination preserved; owned partial removed after failure.
- Original network exception type and message preserved.
- No unsafe access to an absent `Exception.Response` property.

The target operational schema remains
`004_task_protocol_and_tool_aliases`. Native Windows proxy runtime acceptance
remains pending until the approved RC7 Windows acceptance procedure passes.
