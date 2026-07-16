[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [string]$AssetBaseUri
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = "Stop"
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

$script:ProductVersion = "0.2.0-rc1"
$script:OldReleaseLabel = "0.1.0-rc3"
$script:OldBinaryVersion = "0.1.0"
$script:OldBinarySha256 = "01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a"
$script:AssetName = "aopmem-windows-x86_64.exe"
$script:ChecksumName = "SHA256SUMS"
$script:TestMode = ($env:AOPMEM_INSTALL_TEST_MODE -eq "1")
$script:TestFailAt = $env:AOPMEM_INSTALL_TEST_FAIL_AT
$script:Mode = "unknown"
$script:TempRoot = $null
$script:TempRootOwned = $false
$script:InstallStage = $null
$script:InstallStageOwned = $false
$script:RecoveryBinary = $null
$script:RecoveryBinaryOwned = $false
$script:BackupPath = $null
$script:UpgradeBackupRoot = $null
$script:BackupReady = $false
$script:ApplyAttempted = $false
$script:BinaryPublished = $false
$script:PreserveRecovery = $false
$script:InstallSucceeded = $false
$script:InstalledBinary = $null
$script:InstallDir = $null
$script:AopmemHome = $null
$script:VerifiedBinaryHash = $null
$script:LastAopmemJsonText = $null

function Write-TestTrace {
    param([Parameter(Mandatory = $true)][string]$EventName)

    if ($script:TestMode -and -not [string]::IsNullOrWhiteSpace(
            $env:AOPMEM_INSTALL_TEST_TRACE)) {
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

function Test-IsReparsePoint {
    param([Parameter(Mandatory = $true)][IO.FileSystemInfo]$Item)

    return (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Get-ExistingPathItem {
    param([Parameter(Mandatory = $true)][string]$LiteralPath)

    return (Get-Item -LiteralPath $LiteralPath -Force -ErrorAction SilentlyContinue)
}

function Assert-SafeDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$LiteralPath,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $item = Get-ExistingPathItem -LiteralPath $LiteralPath
    if ($null -eq $item) {
        return
    }
    if (-not $item.PSIsContainer -or (Test-IsReparsePoint -Item $item)) {
        Throw-InstallError "$Label must be a real directory, not a reparse point: $LiteralPath"
    }
}

function Assert-NewDestination {
    param(
        [Parameter(Mandatory = $true)][string]$LiteralPath,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $item = Get-ExistingPathItem -LiteralPath $LiteralPath
    if ($null -ne $item) {
        Throw-InstallError "$Label already exists: $LiteralPath"
    }
}

function Assert-SafeRegularFile {
    param(
        [Parameter(Mandatory = $true)][string]$LiteralPath,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $item = Get-ExistingPathItem -LiteralPath $LiteralPath
    if ($null -eq $item -or $item.PSIsContainer -or
        (Test-IsReparsePoint -Item $item)) {
        Throw-InstallError "$Label is not a real regular file: $LiteralPath"
    }
}

function Initialize-InstallDirectories {
    Assert-SafeDirectory -LiteralPath $script:AopmemHome -Label "AOPMem home"
    if ($null -eq (Get-ExistingPathItem -LiteralPath $script:AopmemHome)) {
        [IO.Directory]::CreateDirectory($script:AopmemHome) | Out-Null
    }
    Assert-SafeDirectory -LiteralPath $script:AopmemHome -Label "AOPMem home"

    Assert-SafeDirectory -LiteralPath $script:InstallDir -Label "AOPMem bin directory"
    if ($null -eq (Get-ExistingPathItem -LiteralPath $script:InstallDir)) {
        [IO.Directory]::CreateDirectory($script:InstallDir) | Out-Null
    }
    Assert-SafeDirectory -LiteralPath $script:InstallDir -Label "AOPMem bin directory"
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
    $referencedLines = 0
    $malformedReference = $false
    foreach ($line in [IO.File]::ReadAllLines($ChecksumPath)) {
        $tokens = @($line.Trim() -split "[ `t]+")
        $mentionsExactName = $tokens -ccontains $ExactName
        if ($mentionsExactName) {
            $referencedLines += 1
        }
        $match = [regex]::Match(
            $line,
            "^(?<hash>[0-9A-Fa-f]{64})[ `t]+(?<name>[^ `t]+)[ `t]*$")
        if ($match.Success -and $match.Groups["name"].Value -ceq $ExactName) {
            $matches.Add($match.Groups["hash"].Value.ToLowerInvariant())
        }
        elseif ($mentionsExactName) {
            $malformedReference = $true
        }
    }
    if ($referencedLines -ne 1 -or $matches.Count -ne 1 -or
        $malformedReference) {
        Throw-InstallError "SHA256SUMS has no unique exact entry for $ExactName"
    }
    return $matches[0]
}

function Assert-TrustedHttpsUri {
    param(
        [Parameter(Mandatory = $true)][Uri]$Uri,
        [Parameter(Mandatory = $true)][string]$Context
    )

    if (-not $Uri.IsAbsoluteUri -or
        $Uri.Scheme -cne "https" -or
        [string]::IsNullOrWhiteSpace($Uri.Host) -or
        -not [string]::IsNullOrEmpty($Uri.UserInfo)) {
        Throw-InstallError "$Context must use trusted HTTPS without user information"
    }
}

function Save-HttpsAsset {
    param(
        [Parameter(Mandatory = $true)][Uri]$InitialUri,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    $currentUri = $InitialUri
    for ($redirectCount = 0; $redirectCount -le 10; $redirectCount += 1) {
        Assert-TrustedHttpsUri -Uri $currentUri -Context "asset URI"
        try {
            $null = Invoke-WebRequest `
                -Uri $currentUri `
                -OutFile $Destination `
                -UseBasicParsing `
                -MaximumRedirection 0 `
                -TimeoutSec 900 `
                -ErrorAction Stop
            if (-not [IO.File]::Exists($Destination) -or
                (Get-Item -LiteralPath $Destination).Length -le 0) {
                Throw-InstallError "download returned no data: $currentUri"
            }
            return
        }
        catch {
            Remove-Item -LiteralPath $Destination -Force -ErrorAction SilentlyContinue
            $response = $_.Exception.Response
            if ($null -eq $response) {
                throw
            }
            try {
                $statusCode = [int]$response.StatusCode
                $location = $response.Headers["Location"]
            }
            finally {
                $response.Close()
            }
            if ($statusCode -lt 300 -or $statusCode -ge 400 -or
                [string]::IsNullOrWhiteSpace($location)) {
                throw
            }
            if ($redirectCount -ge 10) {
                Throw-InstallError "download redirect limit exceeded: $InitialUri"
            }
            $nextUri = New-Object System.Uri($currentUri, $location)
            Assert-TrustedHttpsUri -Uri $nextUri -Context "download redirect"
            $currentUri = $nextUri
        }
    }
    Throw-InstallError "download redirect limit exceeded: $InitialUri"
}

function Copy-ReleaseAsset {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    Assert-NewDestination -LiteralPath $Destination -Label "temporary asset destination"
    if ($script:TestMode) {
        if ([string]::IsNullOrWhiteSpace($env:AOPMEM_INSTALL_TEST_ASSET_DIR)) {
            Throw-InstallError "local test asset directory is missing"
        }
        $testSource = Join-Path $env:AOPMEM_INSTALL_TEST_ASSET_DIR $Name
        $testItem = Get-ExistingPathItem -LiteralPath $testSource
        if ($null -eq $testItem -or $testItem.PSIsContainer -or
            (Test-IsReparsePoint -Item $testItem)) {
            Throw-InstallError "local test asset missing or unsafe: $Name"
        }
        Copy-DurableFile -Source $testSource -Destination $Destination
        return
    }

    $base = $AssetBaseUri
    if ([string]::IsNullOrWhiteSpace($base)) {
        $base = $env:AOPMEM_ASSET_BASE_URI
    }
    $parsedBase = $null
    if (-not [Uri]::TryCreate($base, [UriKind]::Absolute, [ref]$parsedBase)) {
        Throw-InstallError "trusted HTTPS asset base URI is missing or invalid"
    }
    Assert-TrustedHttpsUri -Uri $parsedBase -Context "asset base URI"
    if (-not [string]::IsNullOrEmpty($parsedBase.Query) -or
        -not [string]::IsNullOrEmpty($parsedBase.Fragment) -or
        $parsedBase.OriginalString.Contains("@") -or
        $parsedBase.OriginalString -match "\s") {
        Throw-InstallError (
            "asset base URI must not contain credentials, query, fragment, or whitespace")
    }
    $assetUri = New-Object Uri(
        $base.TrimEnd("/") + "/" + [Uri]::EscapeDataString($Name))
    Assert-TrustedHttpsUri -Uri $assetUri -Context "asset URI"
    Save-HttpsAsset -InitialUri $assetUri -Destination $Destination
}

function Invoke-AopmemJson {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Context
    )

    $lines = @(& $Executable @Arguments)
    $nativeExit = $LASTEXITCODE
    $jsonText = [string]::Join([Environment]::NewLine, [string[]]$lines)
    $script:LastAopmemJsonText = $jsonText
    try {
        $json = $jsonText | ConvertFrom-Json -ErrorAction Stop
    }
    catch {
        [Console]::Error.WriteLine($jsonText)
        Throw-InstallError "$Context returned invalid JSON with exit code $nativeExit"
    }
    if ($nativeExit -ne 0) {
        [Console]::Error.WriteLine("AOPMem $Context failure JSON report:")
        [Console]::Error.WriteLine($jsonText)
        $errorCode = Get-OptionalJsonProperty -Object $json -Name "errors"
        $firstError = $null
        if ($null -ne $errorCode -and @($errorCode).Count -gt 0) {
            $firstError = @($errorCode)[0]
        }
        $code = Get-OptionalJsonProperty -Object $firstError -Name "code"
        $message = Get-OptionalJsonProperty -Object $firstError -Name "message"
        $data = Get-OptionalJsonProperty -Object $json -Name "data"
        $workspace = Get-OptionalJsonProperty -Object $data -Name "stopped_workspace"
        $backupRoot = Get-OptionalJsonProperty -Object $data -Name "backup_root"
        $failureText = "$Context failed with exit code $nativeExit"
        if (-not [string]::IsNullOrWhiteSpace([string]$workspace)) {
            $failureText += ": workspace=$workspace"
        }
        if (-not [string]::IsNullOrWhiteSpace([string]$code)) {
            $failureText += " code=$code"
        }
        if (-not [string]::IsNullOrWhiteSpace([string]$message)) {
            $failureText += " message=$message"
        }
        if (-not [string]::IsNullOrWhiteSpace([string]$backupRoot)) {
            $script:UpgradeBackupRoot = [string]$backupRoot
            $failureText += " upgrade_backup=$backupRoot"
        }
        Throw-InstallError $failureText
    }
    return $json
}

function Get-OptionalJsonProperty {
    param(
        [Parameter(Mandatory = $false)]$Object,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if ($null -eq $Object) {
        return $null
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Get-AopmemVersion {
    param([Parameter(Mandatory = $true)][string]$Executable)

    $lines = @(& $Executable --version)
    if ($LASTEXITCODE -ne 0) {
        Throw-InstallError "binary version check failed: $Executable"
    }
    return [string]::Join([Environment]::NewLine, [string[]]$lines).Trim()
}

function Backup-OldBinary {
    if ($script:TestMode -and $script:TestFailAt -eq "backup") {
        Throw-InstallError "injected old binary backup failure"
    }
    $stamp = [DateTime]::UtcNow.ToString("yyyyMMddTHHmmssZ")
    $script:BackupPath = Join-Path $script:InstallDir (
        "aopmem.backup-v{0}-{1}-{2}.exe" -f
            $script:OldReleaseLabel, $stamp,
            [Diagnostics.Process]::GetCurrentProcess().Id)
    Assert-NewDestination -LiteralPath $script:BackupPath -Label "binary backup path"
    $oldHash = Get-Sha256 -LiteralPath $script:InstalledBinary
    $script:OriginalBinaryHash = $oldHash
    Copy-DurableFile -Source $script:InstalledBinary -Destination $script:BackupPath
    Assert-SafeRegularFile -LiteralPath $script:BackupPath -Label "old binary backup"
    $backupHash = Get-Sha256 -LiteralPath $script:BackupPath
    if ($oldHash -cne $backupHash) {
        Throw-InstallError "old binary backup verification failed: $script:BackupPath"
    }
    $script:BackupReady = $true
    Write-TestTrace -EventName "backup.created"
}

function Test-OldBinaryUnchanged {
    if (-not $script:BackupReady -or
        -not [IO.File]::Exists($script:BackupPath) -or
        -not [IO.File]::Exists($script:InstalledBinary)) {
        return $false
    }
    $item = Get-ExistingPathItem -LiteralPath $script:InstalledBinary
    if ($null -eq $item -or $item.PSIsContainer -or
        (Test-IsReparsePoint -Item $item)) {
        return $false
    }
    $currentHash = Get-Sha256 -LiteralPath $script:InstalledBinary
    if ($currentHash -cne $script:OriginalBinaryHash) {
        return $false
    }
    Write-TestTrace -EventName "rollback.unchanged"
    return $true
}

function Prepare-PublishFiles {
    $processId = [Diagnostics.Process]::GetCurrentProcess().Id
    if ($script:TestMode -and
        -not [string]::IsNullOrWhiteSpace($env:AOPMEM_INSTALL_TEST_RUN_ID)) {
        if ($env:AOPMEM_INSTALL_TEST_RUN_ID -notmatch "^[0-9]+$") {
            Throw-InstallError "test run id must contain decimal digits only"
        }
        $processId = $env:AOPMEM_INSTALL_TEST_RUN_ID
    }
    $script:InstallStage = Join-Path $script:InstallDir (
        ".aopmem-v020-stage-{0}.exe" -f $processId)
    $script:RecoveryBinary = Join-Path $script:InstallDir (
        "aopmem-v{0}-recovery-{1}.exe" -f $script:ProductVersion, $processId)
    Assert-NewDestination -LiteralPath $script:InstallStage -Label "binary stage path"
    Assert-NewDestination -LiteralPath $script:RecoveryBinary -Label "recovery binary path"
    $script:InstallStageOwned = $true
    Copy-DurableFile -Source $script:DownloadedBinary -Destination $script:InstallStage
    Assert-SafeRegularFile `
        -LiteralPath $script:InstallStage `
        -Label "same-directory binary stage"
    $script:RecoveryBinaryOwned = $true
    Copy-DurableFile -Source $script:DownloadedBinary -Destination $script:RecoveryBinary
    Assert-SafeRegularFile `
        -LiteralPath $script:RecoveryBinary `
        -Label "same-directory recovery binary"
    if ((Get-Sha256 -LiteralPath $script:InstallStage) -cne
        $script:VerifiedBinaryHash -or
        (Get-Sha256 -LiteralPath $script:RecoveryBinary) -cne
        $script:VerifiedBinaryHash) {
        Throw-InstallError "same-directory binary staging verification failed"
    }
    Write-TestTrace -EventName "replacement.staged"
}

function Invoke-UpgradePlan {
    Write-TestTrace -EventName "upgrade.plan"
    $plan = Invoke-AopmemJson `
        -Executable $script:DownloadedBinary `
        -Arguments @("upgrade", "plan", "--all-workspaces", "--json") `
        -Context "upgrade plan"
    if (-not [bool]$plan.ok -or
        -not [bool]$plan.data.ready -or
        [bool]$plan.data.writes_performed) {
        [Console]::Error.WriteLine("AOPMem upgrade plan not-ready JSON report:")
        [Console]::Error.WriteLine($script:LastAopmemJsonText)
        Throw-InstallError "upgrade plan did not report ready read-only state"
    }
}

function Invoke-UpgradeApply {
    Write-TestTrace -EventName "upgrade.apply"
    $script:ApplyAttempted = $true
    $apply = Invoke-AopmemJson `
        -Executable $script:DownloadedBinary `
        -Arguments @("upgrade", "apply", "--all-workspaces", "--json") `
        -Context "upgrade apply"
    if (-not [bool]$apply.ok -or
        -not [bool]$apply.data.success -or
        [bool]$apply.data.binary_replaced) {
        [Console]::Error.WriteLine("AOPMem upgrade apply unsuccessful JSON report:")
        [Console]::Error.WriteLine($script:LastAopmemJsonText)
        Throw-InstallError "upgrade apply did not report success"
    }
    $script:UpgradeBackupRoot = [string](
        Get-OptionalJsonProperty -Object $apply.data -Name "backup_root")
    Write-TestTrace -EventName "upgrade.apply.health.ok"
}

function Publish-VerifiedBinary {
    if ($script:TestMode -and $script:TestFailAt -eq "publish") {
        Throw-InstallError "injected atomic replacement failure"
    }
    Assert-SafeRegularFile `
        -LiteralPath $script:InstallStage `
        -Label "same-directory binary stage"
    Assert-SafeRegularFile `
        -LiteralPath $script:RecoveryBinary `
        -Label "same-directory recovery binary"
    if ((Get-Sha256 -LiteralPath $script:InstallStage) -cne
        $script:VerifiedBinaryHash -or
        (Get-Sha256 -LiteralPath $script:RecoveryBinary) -cne
        $script:VerifiedBinaryHash) {
        Throw-InstallError "same-directory publish files changed before atomic replacement"
    }
    if ($script:Mode -eq "update") {
        [IO.File]::Replace(
            $script:InstallStage,
            $script:InstalledBinary,
            $null,
            $true)
    }
    else {
        [IO.File]::Move($script:InstallStage, $script:InstalledBinary)
    }
    $script:InstallStage = $null
    $script:InstallStageOwned = $false
    $script:BinaryPublished = $true
    Assert-SafeRegularFile `
        -LiteralPath $script:InstalledBinary `
        -Label "installed binary"
    if ((Get-Sha256 -LiteralPath $script:InstalledBinary) -cne
        $script:VerifiedBinaryHash) {
        Throw-InstallError "installed binary verification failed after atomic replacement"
    }
    $installedVersion = Get-AopmemVersion -Executable $script:InstalledBinary
    if ($installedVersion -cne "aopmem $script:ProductVersion") {
        Throw-InstallError "installed binary has unexpected version: $installedVersion"
    }
    Write-TestTrace -EventName "replacement.published"
}

function Invoke-CurrentWorkspaceHealth {
    if ($script:TestMode -and $script:TestFailAt -eq "doctor") {
        Throw-InstallError "injected doctor failure"
    }
    Write-TestTrace -EventName "doctor"
    $doctor = Invoke-AopmemJson `
        -Executable $script:InstalledBinary `
        -Arguments @("doctor", "--json") `
        -Context "doctor"
    if (-not [bool]$doctor.ok -or -not [bool]$doctor.data.healthy) {
        Throw-InstallError "doctor did not report healthy state"
    }
    Write-TestTrace -EventName "verify"
    $verify = Invoke-AopmemJson `
        -Executable $script:InstalledBinary `
        -Arguments @("verify", "--json") `
        -Context "verify"
    if (-not [bool]$verify.ok -or -not [bool]$verify.data.clean) {
        Throw-InstallError "verify did not report clean state"
    }
}

function Invoke-FreshAdapterSeed {
    Write-TestTrace -EventName "adapter.seed"
    $adapter = Invoke-AopmemJson `
        -Executable $script:InstalledBinary `
        -Arguments @("adapter", "seed", "--json") `
        -Context "fresh managed adapter seed"
    if (-not [bool]$adapter.ok) {
        Throw-InstallError "fresh managed adapter seed did not report success"
    }
}

function Remove-InstallerTemporaryFiles {
    if (-not [string]::IsNullOrWhiteSpace($script:InstallStage) -and
        $script:InstallStageOwned) {
        Remove-Item -LiteralPath $script:InstallStage -Force -ErrorAction SilentlyContinue
    }
    if (-not [string]::IsNullOrWhiteSpace($script:RecoveryBinary) -and
        $script:RecoveryBinaryOwned -and
        -not $script:PreserveRecovery) {
        Remove-Item -LiteralPath $script:RecoveryBinary -Force -ErrorAction SilentlyContinue
    }
    if (-not [string]::IsNullOrWhiteSpace($script:TempRoot) -and
        $script:TempRootOwned) {
        Remove-Item -LiteralPath $script:TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$failure = $null
try {
    $windowsVersion = [Environment]::OSVersion.Version
    if ($PSVersionTable.PSVersion.Major -ne 5 -or
        $PSVersionTable.PSVersion.Minor -lt 1 -or
        -not [Environment]::Is64BitOperatingSystem -or
        -not [Environment]::Is64BitProcess -or
        [Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT -or
        $windowsVersion.Major -ne 10 -or
        $windowsVersion.Build -lt 22000) {
        Throw-InstallError "unsupported platform: Windows 11 x64 native PowerShell is required"
    }
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $null = & "$env:SystemRoot\System32\chcp.com" 65001
    if ($LASTEXITCODE -ne 0) {
        Throw-InstallError "cannot configure Windows console for UTF-8"
    }
    [Console]::InputEncoding = $script:Utf8NoBom
    [Console]::OutputEncoding = $script:Utf8NoBom
    $global:OutputEncoding = $script:Utf8NoBom

    if ($script:TestMode) {
        if ([string]::IsNullOrWhiteSpace($env:AOPMEM_HOME) -or
            [string]::IsNullOrWhiteSpace($env:AOPMEM_INSTALL_TEST_TEMP_ROOT)) {
            Throw-InstallError "test mode requires isolated AOPMEM_HOME and temp root"
        }
        $script:AopmemHome = $env:AOPMEM_HOME
        $tempParent = $env:AOPMEM_INSTALL_TEST_TEMP_ROOT
    }
    else {
        if ([string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
            Throw-InstallError "USERPROFILE is missing"
        }
        $script:AopmemHome = Join-Path $env:USERPROFILE ".aopmem"
        $tempParent = [IO.Path]::GetTempPath()
    }
    $env:AOPMEM_HOME = $script:AopmemHome
    $script:InstallDir = Join-Path $script:AopmemHome "bin"
    $script:InstalledBinary = Join-Path $script:InstallDir "aopmem.exe"
    Initialize-InstallDirectories

    Assert-SafeDirectory -LiteralPath $tempParent -Label "temporary parent"
    if ($null -eq (Get-ExistingPathItem -LiteralPath $tempParent)) {
        Throw-InstallError "temporary parent does not exist: $tempParent"
    }
    $script:TempRoot = Join-Path $tempParent (
        "aopmem-v020-{0}" -f [Guid]::NewGuid().ToString("N"))
    Assert-NewDestination -LiteralPath $script:TempRoot -Label "temporary root"
    [IO.Directory]::CreateDirectory($script:TempRoot) | Out-Null
    $script:TempRootOwned = $true
    Assert-SafeDirectory -LiteralPath $script:TempRoot -Label "temporary root"

    $script:DownloadedBinary = Join-Path $script:TempRoot $script:AssetName
    $downloadedSums = Join-Path $script:TempRoot $script:ChecksumName
    Copy-ReleaseAsset -Name $script:AssetName -Destination $script:DownloadedBinary
    Copy-ReleaseAsset -Name $script:ChecksumName -Destination $downloadedSums

    $expectedHash = Get-ExactChecksum `
        -ChecksumPath $downloadedSums `
        -ExactName $script:AssetName
    $script:VerifiedBinaryHash = Get-Sha256 -LiteralPath $script:DownloadedBinary
    if ($expectedHash -cne $script:VerifiedBinaryHash) {
        Throw-InstallError "SHA-256 mismatch for $script:AssetName"
    }
    Write-TestTrace -EventName "sha256.verified"

    $newVersion = Get-AopmemVersion -Executable $script:DownloadedBinary
    if ($newVersion -cne "aopmem $script:ProductVersion") {
        Throw-InstallError "verified binary has unexpected version: $newVersion"
    }
    Write-TestTrace -EventName "binary.version.verified"

    $existingItem = Get-ExistingPathItem -LiteralPath $script:InstalledBinary
    if ($null -ne $existingItem) {
        if ($existingItem.PSIsContainer -or (Test-IsReparsePoint -Item $existingItem)) {
            Throw-InstallError "installed binary path is not a regular file: $script:InstalledBinary"
        }
        $installedVersion = Get-AopmemVersion -Executable $script:InstalledBinary
        if ($installedVersion -cne "aopmem $script:OldBinaryVersion") {
            Throw-InstallError "existing version is unsupported: $installedVersion"
        }
        $expectedOldHash = $script:OldBinarySha256
        if ($script:TestMode) {
            $expectedOldHash = $env:AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256
        }
        $installedOldHash = Get-Sha256 -LiteralPath $script:InstalledBinary
        if ([string]::IsNullOrWhiteSpace($expectedOldHash) -or
            $installedOldHash -cne $expectedOldHash.ToLowerInvariant()) {
            Throw-InstallError (
                "existing binary is not the supported v{0} release asset" -f
                    $script:OldReleaseLabel)
        }
        $script:Mode = "update"
        Backup-OldBinary
        Prepare-PublishFiles
        if ($script:TestMode -and $script:TestFailAt -eq "after_backup") {
            Throw-InstallError "injected failure after old binary backup"
        }
        Invoke-UpgradePlan
        Invoke-UpgradeApply
        Publish-VerifiedBinary
        [Console]::Out.WriteLine(
            "AOPMem {0} updated. doctor=ok verify=ok binary_backup={1} upgrade_backup={2}" -f
                $script:ProductVersion,
                $script:BackupPath,
                $script:UpgradeBackupRoot)
    }
    else {
        $script:Mode = "fresh"
        Prepare-PublishFiles
        Publish-VerifiedBinary
        Write-TestTrace -EventName "init"
        & $script:InstalledBinary init
        if ($LASTEXITCODE -ne 0) {
            Throw-InstallError "fresh workspace semantic initialization failed"
        }
        Invoke-FreshAdapterSeed
        Invoke-CurrentWorkspaceHealth
        [Console]::Out.WriteLine(
            "AOPMem {0} installed. doctor=ok verify=ok" -f
                $script:ProductVersion)
    }
    $script:InstallSucceeded = $true
}
catch {
    $failure = $_.Exception.Message
    if ($script:Mode -eq "update" -and $script:ApplyAttempted) {
        $script:PreserveRecovery = $true
        $displayUpgradeBackup = $script:UpgradeBackupRoot
        if ([string]::IsNullOrWhiteSpace($displayUpgradeBackup)) {
            $displayUpgradeBackup = "none (apply stopped before upgrade backup creation)"
        }
        $errorLine = (
            (
                "AOPMem install failed: {0}; upgrade apply may have committed v0.2 data; " +
                "do not run the v0.1 binary; verified v0.2 recovery binary preserved at " +
                "{1}; binary backup preserved at {2}; upgrade backup preserved at {3}"
            ) -f
                $failure,
                $script:RecoveryBinary,
                $script:BackupPath,
                $displayUpgradeBackup)
        [Console]::Error.WriteLine($errorLine)
    }
    elseif ($script:Mode -eq "update" -and $script:BackupReady -and
        -not $script:BinaryPublished) {
        try {
            if (-not (Test-OldBinaryUnchanged)) {
                Throw-InstallError "old binary changed unexpectedly"
            }
            $errorLine = (
                "AOPMem install failed: {0}; old binary was left unchanged; backup preserved at {1}" -f
                $failure,
                $script:BackupPath)
            [Console]::Error.WriteLine($errorLine)
        }
        catch {
            $integrityFailure = $_.Exception.Message
            $errorLine = (
                (
                    "AOPMem install failed: {0}; old binary integrity check failed: {1}; " +
                    "backup preserved at {2}"
                ) -f
                    $failure,
                    $integrityFailure,
                    $script:BackupPath)
            [Console]::Error.WriteLine($errorLine)
        }
    }
    else {
        $errorLine = (
            "AOPMem install failed: {0}; workspace data was preserved" -f
            $failure)
        [Console]::Error.WriteLine($errorLine)
    }
}
finally {
    Remove-InstallerTemporaryFiles
}

if (-not $script:InstallSucceeded) {
    exit 1
}
exit 0
