[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [string]$AssetBaseUri,

    [Parameter(Mandatory = $false)]
    [Uri]$ProxyUri,

    [Parameter(Mandatory = $false)]
    [switch]$ProxyUseDefaultCredentials
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = "Stop"
$script:ProductVersion = "0.2.0-rc9"
$script:AssetName = "aopmem-windows-x86_64.exe"
$script:ChecksumName = "SHA256SUMS"
$script:TestMode = ($env:AOPMEM_INSTALL_TEST_MODE -eq "1")
$script:TempRoot = $null
$script:Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Write-TestTrace {
    param([Parameter(Mandatory = $true)][string]$EventName)

    if ($script:TestMode -and -not [string]::IsNullOrWhiteSpace($env:AOPMEM_INSTALL_TEST_TRACE)) {
        [IO.File]::AppendAllText(
            $env:AOPMEM_INSTALL_TEST_TRACE,
            $EventName + [Environment]::NewLine,
            $script:Utf8NoBom)
    }
}

function Throw-InstallError {
    param([Parameter(Mandatory = $true)][string]$Message)

    throw (New-Object System.InvalidOperationException($Message))
}

function Throw-PopulatedHome {
    Throw-InstallError `
        "CLEAN_INSTALL_REQUIRES_EMPTY_HOME: existing AOPMem installation detected; quarantine it before clean installation"
}

function Test-IsReparsePoint {
    param([Parameter(Mandatory = $true)][IO.FileSystemInfo]$Item)

    return (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Get-ExistingPathItem {
    param([Parameter(Mandatory = $true)][string]$LiteralPath)

    return Get-Item -LiteralPath $LiteralPath -Force -ErrorAction SilentlyContinue
}

function Assert-Platform {
    if ($PSVersionTable.PSVersion.Major -lt 5) {
        Throw-InstallError "Windows PowerShell 5.1 or newer is required"
    }
    if (-not [Environment]::Is64BitOperatingSystem) {
        Throw-InstallError "Windows x64 is required"
    }
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Throw-InstallError "run the installer from a non-admin PowerShell"
    }
}

function Resolve-AopmemHome {
    if (-not [string]::IsNullOrWhiteSpace($env:AOPMEM_HOME)) {
        return [IO.Path]::GetFullPath($env:AOPMEM_HOME)
    }
    if ([string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
        Throw-InstallError "USERPROFILE is not set"
    }
    return (Join-Path $env:USERPROFILE ".aopmem")
}

function Assert-CleanHome {
    param(
        [Parameter(Mandatory = $true)][string]$HomePath,
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    $repoLocalHome = Join-Path $RepoRoot ".aopmem"
    if ([String]::Equals($HomePath, $repoLocalHome, [StringComparison]::OrdinalIgnoreCase)) {
        Throw-InstallError "repo-local .aopmem is not allowed"
    }

    $item = Get-ExistingPathItem -LiteralPath $HomePath
    if ($null -eq $item) {
        return
    }
    if (-not $item.PSIsContainer -or (Test-IsReparsePoint -Item $item)) {
        Throw-PopulatedHome
    }
    $firstChild = Get-ChildItem -LiteralPath $HomePath -Force -ErrorAction Stop |
        Select-Object -First 1
    if ($null -ne $firstChild) {
        Throw-PopulatedHome
    }
}

function Assert-TrustedHttpsUri {
    param(
        [Parameter(Mandatory = $true)][Uri]$Uri,
        [Parameter(Mandatory = $true)][string]$Context
    )

    if (-not $Uri.IsAbsoluteUri -or
        $Uri.Scheme -cne "https" -or
        [string]::IsNullOrWhiteSpace($Uri.Host) -or
        -not [string]::IsNullOrEmpty($Uri.UserInfo) -or
        -not [string]::IsNullOrEmpty($Uri.Query) -or
        -not [string]::IsNullOrEmpty($Uri.Fragment) -or
        ($Uri.OriginalString -match "\s")) {
        Throw-InstallError "$Context must use trusted HTTPS without credentials, query, fragment, or whitespace"
    }
}

function Copy-DurableFile {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    $sourceStream = $null
    $destinationStream = $null
    try {
        $sourceStream = [IO.File]::Open(
            $Source,
            [IO.FileMode]::Open,
            [IO.FileAccess]::Read,
            [IO.FileShare]::Read)
        $destinationStream = [IO.File]::Open(
            $Destination,
            [IO.FileMode]::CreateNew,
            [IO.FileAccess]::Write,
            [IO.FileShare]::None)
        $sourceStream.CopyTo($destinationStream)
        $destinationStream.Flush($true)
    }
    finally {
        if ($null -ne $destinationStream) {
            $destinationStream.Dispose()
        }
        if ($null -ne $sourceStream) {
            $sourceStream.Dispose()
        }
    }
}

function Download-Asset {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if ($script:TestMode) {
        if ([string]::IsNullOrWhiteSpace($env:AOPMEM_INSTALL_TEST_ASSET_DIR)) {
            Throw-InstallError "AOPMEM_INSTALL_TEST_ASSET_DIR is not set"
        }
        $source = Join-Path $env:AOPMEM_INSTALL_TEST_ASSET_DIR $Name
        if (-not [IO.File]::Exists($source)) {
            Throw-InstallError "local test asset missing: $Name"
        }
        Copy-DurableFile -Source $source -Destination $Destination
        return
    }

    if ([string]::IsNullOrWhiteSpace($AssetBaseUri)) {
        Throw-InstallError "trusted HTTPS asset base URI is missing"
    }
    $base = [Uri]$AssetBaseUri
    Assert-TrustedHttpsUri -Uri $base -Context "AssetBaseUri"
    if ($null -ne $ProxyUri) {
        if (-not ($ProxyUri.IsAbsoluteUri) -or
            ($ProxyUri.Scheme -ine "http" -and $ProxyUri.Scheme -ine "https") -or
            -not [string]::IsNullOrEmpty($ProxyUri.UserInfo) -or
            -not [string]::IsNullOrEmpty($ProxyUri.Query) -or
            -not [string]::IsNullOrEmpty($ProxyUri.Fragment)) {
            Throw-InstallError "ProxyUri must be HTTP(S) without credentials, query, or fragment"
        }
    }
    $target = [Uri](($base.AbsoluteUri.TrimEnd("/")) + "/" + $Name)
    $parameters = @{
        Uri = $target
        OutFile = $Destination
        UseBasicParsing = $true
        TimeoutSec = 900
        MaximumRedirection = 10
        ErrorAction = "Stop"
    }
    if ($null -ne $ProxyUri) {
        $parameters.Proxy = $ProxyUri
        if ($ProxyUseDefaultCredentials) {
            $parameters.ProxyUseDefaultCredentials = $true
        }
    }
    Invoke-WebRequest @parameters
}

function Get-Sha256 {
    param([Parameter(Mandatory = $true)][string]$LiteralPath)

    return (Get-FileHash -LiteralPath $LiteralPath -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-ExactChecksum {
    param(
        [Parameter(Mandatory = $true)][string]$ChecksumPath,
        [Parameter(Mandatory = $true)][string]$ExactName
    )

    $matches = New-Object System.Collections.Generic.List[string]
    foreach ($line in [IO.File]::ReadAllLines($ChecksumPath)) {
        $match = [regex]::Match(
            $line,
            "^(?<hash>[0-9A-Fa-f]{64})[ `t]+(?<name>[^ `t]+)[ `t]*$")
        if ($match.Success -and $match.Groups["name"].Value -ceq $ExactName) {
            $matches.Add($match.Groups["hash"].Value.ToLowerInvariant())
        }
    }
    if ($matches.Count -ne 1) {
        Throw-InstallError "SHA256SUMS has no unique exact entry for $ExactName"
    }
    return $matches[0]
}

function Invoke-Aopmem {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

    $oldHome = $env:AOPMEM_HOME
    try {
        $env:AOPMEM_HOME = $script:AopmemHome
        & $script:InstalledBinary @Arguments
        if ($LASTEXITCODE -ne 0) {
            Throw-InstallError "aopmem command failed: $($Arguments -join ' ')"
        }
    }
    finally {
        $env:AOPMEM_HOME = $oldHome
    }
}

function Invoke-InitDefaults {
    $answers = @(
        "n",
        "n",
        "Clean AOPMem $script:ProductVersion install for this repository.",
        "User owns decisions. Agent keeps project memory current.",
        "Only this repository is initialized by the clean installer."
    )
    $oldHome = $env:AOPMEM_HOME
    try {
        $env:AOPMEM_HOME = $script:AopmemHome
        (($answers -join [Environment]::NewLine) + [Environment]::NewLine) |
            & $script:InstalledBinary --json init | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Throw-InstallError "aopmem init failed"
        }
    }
    finally {
        $env:AOPMEM_HOME = $oldHome
    }
}

try {
    Assert-Platform
    $script:RepoRoot = [IO.Directory]::GetCurrentDirectory()
    $script:AopmemHome = Resolve-AopmemHome
    Assert-CleanHome -HomePath $script:AopmemHome -RepoRoot $script:RepoRoot
    Write-TestTrace "home.clean"

    $script:TempRoot = Join-Path ([IO.Path]::GetTempPath()) `
        ("aopmem-rc9-install-" + [Guid]::NewGuid().ToString("N"))
    [IO.Directory]::CreateDirectory($script:TempRoot) | Out-Null
    $binaryStage = Join-Path $script:TempRoot $script:AssetName
    $checksumsStage = Join-Path $script:TempRoot $script:ChecksumName
    Download-Asset -Name $script:ChecksumName -Destination $checksumsStage
    Download-Asset -Name $script:AssetName -Destination $binaryStage
    $expectedHash = Get-ExactChecksum -ChecksumPath $checksumsStage -ExactName $script:AssetName
    $actualHash = Get-Sha256 -LiteralPath $binaryStage
    if ($actualHash -cne $expectedHash) {
        Throw-InstallError "SHA256 mismatch for $script:AssetName"
    }
    Write-TestTrace "asset.verified"

    $versionOutput = (& $binaryStage --version)
    if ($LASTEXITCODE -ne 0 -or $versionOutput -cne "aopmem $script:ProductVersion") {
        Throw-InstallError "binary version mismatch: $versionOutput"
    }

    $binDir = Join-Path $script:AopmemHome "bin"
    [IO.Directory]::CreateDirectory($binDir) | Out-Null
    $script:InstalledBinary = Join-Path $binDir "aopmem.exe"
    if ($null -ne (Get-ExistingPathItem -LiteralPath $script:InstalledBinary)) {
        Throw-PopulatedHome
    }
    Copy-DurableFile -Source $binaryStage -Destination $script:InstalledBinary
    Write-TestTrace "binary.installed"

    Invoke-Aopmem platform check --json | Out-Null
    Write-TestTrace "platform.check"
    Invoke-InitDefaults
    Write-TestTrace "init"

    if (-not [string]::IsNullOrWhiteSpace($env:AOPMEM_ACTIVE_INSTRUCTION_FILE)) {
        Invoke-Aopmem --json adapter sync --file $env:AOPMEM_ACTIVE_INSTRUCTION_FILE | Out-Null
    }
    else {
        Invoke-Aopmem --json adapter sync | Out-Null
    }
    Write-TestTrace "adapter.sync"

    Invoke-Aopmem --json doctor | Out-Null
    Invoke-Aopmem --json verify | Out-Null
    Write-TestTrace "doctor.verify"

    if ([IO.Directory]::Exists((Join-Path $script:RepoRoot ".aopmem"))) {
        Throw-InstallError "repo-local .aopmem was created"
    }

    Write-Output "AOPMem $script:ProductVersion clean install complete"
    Write-Output "AOPMEM_HOME=$script:AopmemHome"
    Write-Output "$script:AssetName sha256=$actualHash"
}
finally {
    if (-not [string]::IsNullOrWhiteSpace($script:TempRoot) -and
        [IO.Directory]::Exists($script:TempRoot)) {
        Remove-Item -LiteralPath $script:TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
