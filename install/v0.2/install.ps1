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
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

$script:ProductVersion = "0.2.0-rc8"
$script:OldReleaseLabel = "0.1.0-rc3"
$script:KnownSourceHashes = @{
    "aopmem 0.1.0" =
        "01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a"
    "aopmem 0.2.0-rc1" =
        "a4e3302d6f26dd9d16387a075189fec51c469aef9b8d9c730f81001b21b2cf57"
    "aopmem 0.2.0-rc2" =
        "77a2e79162c609ff62dbaa4533c5f7237490c842047485fe79a608a14f57a5f8"
    "aopmem 0.2.0-rc3" =
        "ed59be73d99efd2c1a4fe99e50b85e8b6ce8e8a73b7ff0c96b5327e1c2d39477"
    "aopmem 0.2.0-rc4" =
        "e4442fd06622a6b94f997e23b67a55753f1d841f6570ef20ac72b99083a6cc1c"
    "aopmem 0.2.0-rc5" =
        "150db4699c2f41c6e529f9606ac099c9ac6b4771b5084952f2cb5df3226d1b58"
    "aopmem 0.2.0-rc6" =
        "8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7"
}
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
$script:FullBackupRoot = $null
$script:FullBackupHome = $null
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
$script:ActiveAdapter = $env:AOPMEM_ACTIVE_ADAPTER
$script:ActiveInstructionFile = $env:AOPMEM_ACTIVE_INSTRUCTION_FILE
$script:AssetProxyInitialized = $false
$script:AssetProxy = $null
$script:ProxySource = "none"
$script:ProxyConfigured = "no"
$script:LastTransportResult = $null

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

function Throw-AssetTransportError {
    param(
        [Parameter(Mandatory = $true)][string]$Code,
        [Parameter(Mandatory = $true)][string]$Detail,
        [Parameter(Mandatory = $true)][Uri]$TargetUri,
        [Parameter(Mandatory = $true)][string]$Destination,
        [Parameter(Mandatory = $true)][int]$RedirectHop,
        [Parameter(Mandatory = $true)][string]$PartialState,
        [Parameter(Mandatory = $false)][Exception]$InnerException
    )

    $safeTarget = $TargetUri.Host + $TargetUri.AbsolutePath
    $message = (
        "{0}: {1}; stage=asset_download target={2} proxy_configured={3} " +
        "proxy_source={4} redirect_hop={5} destination={6} partial={7}"
    ) -f
        $Code,
        $Detail,
        $safeTarget,
        $script:ProxyConfigured,
        $script:ProxySource,
        $RedirectHop,
        $Destination,
        $PartialState
    if ($null -ne $InnerException) {
        $message += (
            "; exception_type={0} exception_message={1}" -f
                $InnerException.GetType().FullName,
                $InnerException.Message)
        throw (New-Object System.InvalidOperationException `
            -ArgumentList @($message, $InnerException))
    }
    Throw-InstallError $message
}

function Test-ValidProxyUri {
    param([Parameter(Mandatory = $true)][Uri]$Candidate)

    return (
        $Candidate.IsAbsoluteUri -and
        ($Candidate.Scheme -ieq "http" -or $Candidate.Scheme -ieq "https") -and
        -not [string]::IsNullOrWhiteSpace($Candidate.Host) -and
        [string]::IsNullOrEmpty($Candidate.UserInfo) -and
        [string]::IsNullOrEmpty($Candidate.Query) -and
        [string]::IsNullOrEmpty($Candidate.Fragment) -and
        -not ($Candidate.OriginalString -match "\s"))
}

function Get-OriginalTransportException {
    param([Parameter(Mandatory = $true)][Exception]$Exception)

    $original = $Exception
    while ($null -ne $original.InnerException -and
        ($original -is
            [System.Management.Automation.MethodInvocationException] -or
         $original -is
            [System.Reflection.TargetInvocationException])) {
        $original = $original.InnerException
    }
    return $original
}

function Initialize-AssetProxy {
    param([Parameter(Mandatory = $true)][Uri]$ProbeUri)

    if ($script:AssetProxyInitialized) {
        return
    }

    $selectedUri = $null
    $selectedSource = "none"
    if ($null -ne $ProxyUri) {
        $selectedUri = $ProxyUri
        $selectedSource = "explicit"
    }
    else {
        foreach ($environmentName in @(
                "HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy")) {
            $environmentValue = [Environment]::GetEnvironmentVariable(
                $environmentName)
            if (-not [string]::IsNullOrEmpty($environmentValue)) {
                $parsedEnvironmentUri = $null
                if (-not [Uri]::TryCreate(
                        $environmentValue,
                        [UriKind]::Absolute,
                        [ref]$parsedEnvironmentUri) -or
                    -not (Test-ValidProxyUri -Candidate $parsedEnvironmentUri)) {
                    Throw-InstallError (
                        "PROXY_CONFIGURATION_INVALID: invalid env proxy; " +
                        "proxy_configured=yes proxy_source=env")
                }
                $selectedUri = $parsedEnvironmentUri
                $selectedSource = "env"
                break
            }
        }
    }

    if ($null -ne $selectedUri) {
        if (-not (Test-ValidProxyUri -Candidate $selectedUri)) {
            Throw-InstallError (
                "PROXY_CONFIGURATION_INVALID: invalid proxy; " +
                "proxy_configured=yes proxy_source=$selectedSource")
        }
        $script:AssetProxy = New-Object System.Net.WebProxy($selectedUri)
        $script:ProxySource = $selectedSource
        $script:ProxyConfigured = "yes"
    }
    else {
        $systemProxy = [System.Net.WebRequest]::DefaultWebProxy
        if ($null -ne $systemProxy) {
            $systemProxyUri = $null
            try {
                if (-not $systemProxy.IsBypassed($ProbeUri)) {
                    $systemProxyUri = $systemProxy.GetProxy($ProbeUri)
                }
            }
            catch {
                $systemProxyUri = $null
            }
            if ($null -ne $systemProxyUri -and
                $systemProxyUri.AbsoluteUri -ine $ProbeUri.AbsoluteUri -and
                (Test-ValidProxyUri -Candidate $systemProxyUri)) {
                $script:AssetProxy =
                    New-Object System.Net.WebProxy($systemProxyUri)
                $script:ProxySource = "system"
                $script:ProxyConfigured = "yes"
            }
        }
    }

    if ($ProxyUseDefaultCredentials -and $null -ne $script:AssetProxy) {
        $script:AssetProxy.Credentials =
            [System.Net.CredentialCache]::DefaultNetworkCredentials
    }
    $script:AssetProxyInitialized = $true
}

function Save-HttpsAsset {
    param(
        [Parameter(Mandatory = $true)][Uri]$InitialUri,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    Initialize-AssetProxy -ProbeUri $InitialUri
    Add-Type -AssemblyName System.Net.Http

    $partial = $null
    $partialOwned = $false
    $partialState = "none"
    $handler = $null
    $client = $null
    $response = $null
    $responseStream = $null
    $destinationStream = $null
    $currentUri = $InitialUri
    $redirectCount = 0
    $byteCount = [Int64]0

    Assert-TrustedHttpsUri -Uri $InitialUri -Context "asset URI"
    if (-not [string]::IsNullOrEmpty($InitialUri.Query) -or
        -not [string]::IsNullOrEmpty($InitialUri.Fragment)) {
        Throw-AssetTransportError -Code "UNSAFE_REDIRECT_TARGET" `
            -Detail "initial URI contains query or fragment" `
            -TargetUri $InitialUri -Destination $Destination `
            -RedirectHop 0 -PartialState $partialState
    }

    $destinationParent = [IO.Path]::GetFullPath(
        [IO.Path]::GetDirectoryName($Destination))
    $validatedTempRoot = [IO.Path]::GetFullPath($script:TempRoot)
    if ($destinationParent -ine $validatedTempRoot) {
        Throw-AssetTransportError -Code "ASSET_DOWNLOAD_FAILED" `
            -Detail "destination parent is not the private temporary root" `
            -TargetUri $InitialUri -Destination $Destination `
            -RedirectHop 0 -PartialState $partialState
    }
    if ([IO.File]::Exists($Destination) -or
        [IO.Directory]::Exists($Destination)) {
        Throw-AssetTransportError -Code "ASSET_DOWNLOAD_FAILED" `
            -Detail "destination already exists" -TargetUri $InitialUri `
            -Destination $Destination -RedirectHop 0 `
            -PartialState $partialState
    }

    $initialOrigin = $InitialUri.GetLeftPart([UriPartial]::Authority)
    $initialPath = $InitialUri.AbsolutePath
    $lastSlash = $initialPath.LastIndexOf("/")
    if ($lastSlash -lt 0 -or $lastSlash -eq ($initialPath.Length - 1)) {
        Throw-AssetTransportError -Code "UNSAFE_REDIRECT_TARGET" `
            -Detail "initial asset is not a flat direct child" `
            -TargetUri $InitialUri -Destination $Destination `
            -RedirectHop 0 -PartialState $partialState
    }
    $initialBasePath = $initialPath.Substring(0, $lastSlash + 1)
    $flatAssetName = [Uri]::UnescapeDataString(
        $initialPath.Substring($lastSlash + 1))
    if ([string]::IsNullOrWhiteSpace($flatAssetName) -or
        $flatAssetName.Contains("/") -or $flatAssetName.Contains("\")) {
        Throw-AssetTransportError -Code "UNSAFE_REDIRECT_TARGET" `
            -Detail "initial asset name is not flat" -TargetUri $InitialUri `
            -Destination $Destination -RedirectHop 0 `
            -PartialState $partialState
    }
    $githubRelease = (
        $InitialUri.Host -ieq "github.com" -and
        $initialPath -match
            "^/[^/]+/[^/]+/releases/download/[^/]+/[^/]+$")
    $onReleaseAssets = $false
    $visited = New-Object "System.Collections.Generic.HashSet[string]" (
        [StringComparer]::Ordinal)
    [void]$visited.Add($InitialUri.AbsoluteUri)

    try {
        $handler = New-Object System.Net.Http.HttpClientHandler
        $handler.AllowAutoRedirect = $false
        $handler.UseCookies = $false
        $handler.UseDefaultCredentials = $false
        $handler.PreAuthenticate = $false
        $handler.Credentials = $null
        if ($null -ne $script:AssetProxy) {
            $handler.UseProxy = $true
            $handler.Proxy = $script:AssetProxy
        }
        else {
            $handler.UseProxy = $false
        }
        $client = New-Object System.Net.Http.HttpClient($handler, $false)
        $client.Timeout = [TimeSpan]::FromSeconds(900)

        while ($true) {
            try {
                $response = $client.GetAsync(
                    $currentUri,
                    [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead
                ).GetAwaiter().GetResult()
            }
            catch {
                $requestException = Get-OriginalTransportException `
                    -Exception $_.Exception
                Throw-AssetTransportError -Code "HTTP_REQUEST_FAILED" `
                    -Detail "HTTP request failed" -TargetUri $currentUri `
                    -Destination $Destination -RedirectHop $redirectCount `
                    -PartialState $partialState `
                    -InnerException $requestException
            }

            $statusCode = [int]$response.StatusCode
            # Accepted redirect status codes: 301, 302, 303, 307, 308.
            $isRedirect = $statusCode -eq 301 -or $statusCode -eq 302 -or
                $statusCode -eq 303 -or $statusCode -eq 307 -or
                $statusCode -eq 308
            if ($isRedirect) {
                $location = $response.Headers.Location
                $response.Dispose()
                $response = $null
                if ($null -eq $location -or
                    [string]::IsNullOrWhiteSpace($location.OriginalString)) {
                    Throw-AssetTransportError `
                        -Code "HTTP_REDIRECT_MISSING_LOCATION" `
                        -Detail "HTTP status=$statusCode has no Location" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }
                if ($redirectCount -ge 10) {
                    Throw-AssetTransportError -Code "HTTP_REDIRECT_LIMIT" `
                        -Detail "redirect limit 10 exceeded; HTTP status=$statusCode" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }
                try {
                    $nextUri = New-Object System.Uri($currentUri, $location)
                }
                catch {
                    Throw-AssetTransportError -Code "UNSAFE_REDIRECT_TARGET" `
                        -Detail "redirect Location is malformed" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState `
                        -InnerException $_.Exception
                }
                if (-not $nextUri.IsAbsoluteUri -or
                    $nextUri.Scheme -cne "https" -or
                    [string]::IsNullOrWhiteSpace($nextUri.Host) -or
                    -not [string]::IsNullOrEmpty($nextUri.UserInfo) -or
                    -not [string]::IsNullOrEmpty($nextUri.Fragment)) {
                    Throw-AssetTransportError -Code "UNSAFE_REDIRECT_TARGET" `
                        -Detail "redirect target violates HTTPS boundary" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }

                $nextPath = $nextUri.AbsolutePath
                $decodedPath = [Uri]::UnescapeDataString($nextPath)
                if ($decodedPath.Contains("\") -or
                    $decodedPath -match "(^|/)\.\.?(/|$)") {
                    Throw-AssetTransportError -Code "UNSAFE_REDIRECT_TARGET" `
                        -Detail "redirect target contains path traversal" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }
                $nextOrigin = $nextUri.GetLeftPart([UriPartial]::Authority)
                if ($onReleaseAssets) {
                    $allowedTarget = (
                        $nextUri.Host -ieq "release-assets.githubusercontent.com" -and
                        $nextOrigin -ieq
                            "https://release-assets.githubusercontent.com" -and
                        -not [string]::IsNullOrWhiteSpace($nextPath) -and
                        $nextPath -cne "/")
                }
                elseif ($nextOrigin -ieq $initialOrigin) {
                    $allowedTarget = (
                        $nextPath.StartsWith(
                            $initialBasePath,
                            [StringComparison]::Ordinal) -and
                        [string]::IsNullOrEmpty($nextUri.Query))
                }
                else {
                    $allowedTarget = (
                        $githubRelease -and
                        $nextOrigin -ieq
                            "https://release-assets.githubusercontent.com" -and
                        -not [string]::IsNullOrWhiteSpace($nextPath) -and
                        $nextPath -cne "/")
                    if ($allowedTarget) {
                        $onReleaseAssets = $true
                    }
                }
                if (-not $allowedTarget) {
                    Throw-AssetTransportError -Code "UNSAFE_REDIRECT_TARGET" `
                        -Detail "redirect target escapes release boundary" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }
                if (-not $visited.Add($nextUri.AbsoluteUri)) {
                    Throw-AssetTransportError -Code "HTTP_REDIRECT_LOOP" `
                        -Detail "redirect target was already visited" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }
                $currentUri = $nextUri
                $redirectCount += 1
                continue
            }

            if ($statusCode -ne 200) {
                Throw-AssetTransportError -Code "HTTP_STATUS_REJECTED" `
                    -Detail "HTTP status=$statusCode" -TargetUri $currentUri `
                    -Destination $Destination -RedirectHop $redirectCount `
                    -PartialState $partialState
            }

            $partial = Join-Path $destinationParent (
                ".{0}.{1}.partial" -f
                    [IO.Path]::GetFileName($Destination),
                    [Guid]::NewGuid().ToString("N"))
            try {
                $destinationStream = [IO.File]::Open(
                    $partial,
                    [IO.FileMode]::CreateNew,
                    [IO.FileAccess]::Write,
                    [IO.FileShare]::None)
                $partialOwned = $true
                $partialState = "created"
                $responseStream = $response.Content.ReadAsStreamAsync(
                    ).GetAwaiter().GetResult()
                $buffer = New-Object byte[] 65536
                while (($read = $responseStream.Read(
                            $buffer, 0, $buffer.Length)) -gt 0) {
                    $destinationStream.Write($buffer, 0, $read)
                    $byteCount += [Int64]$read
                }
                if ($byteCount -le 0) {
                    Throw-AssetTransportError -Code "ASSET_DOWNLOAD_FAILED" `
                        -Detail "download returned zero bytes; HTTP status=200" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }
                $contentLength = $response.Content.Headers.ContentLength
                if ($null -ne $contentLength -and
                    [Int64]$contentLength -ne $byteCount) {
                    Throw-AssetTransportError -Code "ASSET_LENGTH_MISMATCH" `
                        -Detail (
                            "Content-Length={0} actual={1}; HTTP status=200" -f
                                $contentLength, $byteCount) `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }
                $destinationStream.Flush($true)
                $destinationStream.Dispose()
                $destinationStream = $null
                $responseStream.Dispose()
                $responseStream = $null
                $response.Dispose()
                $response = $null
                if ([IO.File]::Exists($Destination) -or
                    [IO.Directory]::Exists($Destination)) {
                    Throw-AssetTransportError -Code "ASSET_DOWNLOAD_FAILED" `
                        -Detail "destination appeared before publication" `
                        -TargetUri $currentUri -Destination $Destination `
                        -RedirectHop $redirectCount -PartialState $partialState
                }
                [IO.File]::Move($partial, $Destination)
                $partialOwned = $false
                $partialState = "published"
                $script:LastTransportResult = [PSCustomObject]@{
                    Path = $Destination
                    ByteCount = $byteCount
                    FinalUri = $currentUri.Host + $currentUri.AbsolutePath
                    RedirectCount = $redirectCount
                    PartialState = $partialState
                }
                return
            }
            catch {
                if ($_.Exception.Message -match
                    "^(ASSET_DOWNLOAD_FAILED|ASSET_LENGTH_MISMATCH):") {
                    throw
                }
                $streamException = Get-OriginalTransportException `
                    -Exception $_.Exception
                Throw-AssetTransportError -Code "ASSET_DOWNLOAD_FAILED" `
                    -Detail "stream or publication failed" `
                    -TargetUri $currentUri -Destination $Destination `
                    -RedirectHop $redirectCount -PartialState $partialState `
                    -InnerException $streamException
            }
        }
    }
    finally {
        if ($null -ne $destinationStream) {
            $destinationStream.Dispose()
        }
        if ($null -ne $responseStream) {
            $responseStream.Dispose()
        }
        if ($null -ne $response) {
            $response.Dispose()
        }
        if ($null -ne $client) {
            $client.Dispose()
        }
        if ($null -ne $handler) {
            $handler.Dispose()
        }
        if ($partialOwned -and -not [string]::IsNullOrWhiteSpace($partial)) {
            Remove-Item -LiteralPath $partial -Force -ErrorAction SilentlyContinue
        }
    }
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
        $details = Get-OptionalJsonProperty -Object $firstError -Name "details"
        $backupPhase = Get-OptionalJsonProperty -Object $details -Name "backup_phase"
        $rawOsError = Get-OptionalJsonProperty -Object $details -Name "raw_os_error"
        $partialPath = Get-OptionalJsonProperty -Object $details -Name "temporary_path"
        $partialValidated = Get-OptionalJsonProperty `
            -Object $details -Name "partial_file_validated"
        $migrationStarted = Get-OptionalJsonProperty `
            -Object $details -Name "migration_started"
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
        if (-not [string]::IsNullOrWhiteSpace([string]$backupPhase)) {
            $failureText += " backup_phase=$backupPhase"
        }
        if ($null -ne $rawOsError) {
            $failureText += " raw_os_error=$rawOsError"
        }
        if (-not [string]::IsNullOrWhiteSpace([string]$partialPath)) {
            $failureText += " partial_backup=$partialPath"
        }
        if ($null -ne $partialValidated) {
            $failureText += " partial_validated=$partialValidated"
        }
        if ($null -ne $migrationStarted) {
            $failureText += " migration_started=$migrationStarted"
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

function Assert-NoActiveAopmemProcesses {
    if ($script:TestMode) {
        Write-TestTrace -EventName "process.gate.clear"
        return
    }
    $active = @(Get-Process -Name "aopmem" -ErrorAction SilentlyContinue)
    if ($active.Count -gt 0) {
        $details = [string]::Join(
            ", ",
            [string[]]@($active | ForEach-Object { "PID=$($_.Id)" }))
        Throw-InstallError (
            "AOPMEM_PROCESS_RUNNING: close all AOPMem processes before update: $details")
    }
    Write-TestTrace -EventName "process.gate.clear"
}

function Backup-AopmemHome {
    $stamp = [DateTime]::UtcNow.ToString("yyyyMMddTHHmmssZ")
    # Safety Backup is durable evidence only. Normal RC8 recovery never adopts
    # this PowerShell-created full-home backup.
    $backupParent = Split-Path -Parent $script:AopmemHome
    Assert-SafeDirectory -LiteralPath $backupParent -Label "durable backup parent"
    if ($null -eq (Get-ExistingPathItem -LiteralPath $backupParent)) {
        [IO.Directory]::CreateDirectory($backupParent) | Out-Null
    }
    Assert-SafeDirectory -LiteralPath $backupParent -Label "durable backup parent"
    $reservedHomeManifest = Join-Path $script:AopmemHome "MANIFEST.sha256"
    if ($null -ne (Get-ExistingPathItem -LiteralPath $reservedHomeManifest)) {
        Throw-InstallError (
            "AOPMem home root uses reserved backup manifest name: MANIFEST.sha256")
    }
    $script:FullBackupRoot = Join-Path $backupParent (
        "aopmem-home-backup-v{0}-{1}-{2}" -f
            $script:ProductVersion,
            $stamp,
            [Diagnostics.Process]::GetCurrentProcess().Id)
    Assert-NewDestination `
        -LiteralPath $script:FullBackupRoot `
        -Label "durable backup root"
    [IO.Directory]::CreateDirectory($script:FullBackupRoot) | Out-Null
    $script:FullBackupHome = $script:FullBackupRoot
    # Reject every reparse/non-regular source before copying.
    $script:SourcePreflightEntries = 0
    function Assert-SourceTreeNoFollow {
        param([string]$Directory, [int]$Depth)
        if ($Depth -gt 128) {
            Throw-InstallError "AOPMem home exceeds maximum backup directory depth"
        }
        $names = [string[]]@([IO.Directory]::GetFileSystemEntries($Directory))
        [Array]::Sort($names, [StringComparer]::Ordinal)
        if ($names.Count -gt 10000) {
            Throw-InstallError "durable backup exceeds maximum directory entry count"
        }
        foreach ($path in $names) {
            $script:SourcePreflightEntries += 1
            if ($script:SourcePreflightEntries -gt 100000) {
                Throw-InstallError "AOPMem home exceeds maximum backup entry count"
            }
            $item = Get-Item -LiteralPath $path -Force
            if (Test-IsReparsePoint -Item $item) {
                Throw-InstallError "AOPMem home contains a reparse point"
            }
            if ($item.PSIsContainer) {
                Assert-SourceTreeNoFollow -Directory $item.FullName -Depth ($Depth + 1)
            }
            elseif (-not $item.PSIsContainer -and -not ($item -is [IO.FileInfo])) {
                Throw-InstallError "AOPMem home contains a non-regular file"
            }
        }
    }
    Assert-SourceTreeNoFollow -Directory $script:AopmemHome -Depth 0
    function Copy-HomeTreeDurably {
        param([string]$SourceDirectory, [string]$DestinationDirectory, [int]$Depth)
        if ($Depth -gt 128) {
            Throw-InstallError "AOPMem home exceeds maximum backup directory depth"
        }
        $names = [string[]]@([IO.Directory]::GetFileSystemEntries($SourceDirectory))
        [Array]::Sort($names, [StringComparer]::Ordinal)
        if ($names.Count -gt 10000) {
            Throw-InstallError "AOPMem home exceeds maximum backup directory entry count"
        }
        foreach ($sourcePath in $names) {
            $sourceItem = Get-Item -LiteralPath $sourcePath -Force
            if (Test-IsReparsePoint -Item $sourceItem) {
                Throw-InstallError "AOPMem home contains a reparse point"
            }
            $destinationPath = Join-Path $DestinationDirectory $sourceItem.Name
            if ($sourceItem.PSIsContainer) {
                [IO.Directory]::CreateDirectory($destinationPath) | Out-Null
                Copy-HomeTreeDurably -SourceDirectory $sourceItem.FullName `
                    -DestinationDirectory $destinationPath -Depth ($Depth + 1)
            }
            elseif ($sourceItem -is [IO.FileInfo]) {
                Copy-DurableFile -Source $sourceItem.FullName -Destination $destinationPath
            }
            else {
                Throw-InstallError "AOPMem home contains a non-regular file"
            }
        }
    }
    Copy-HomeTreeDurably -SourceDirectory $script:AopmemHome `
        -DestinationDirectory $script:FullBackupHome -Depth 0
    $backupBinary = Join-Path $script:FullBackupHome "bin\aopmem.exe"
    Assert-SafeRegularFile -LiteralPath $backupBinary -Label "durable backup binary"
    if ((Get-Sha256 -LiteralPath $backupBinary) -cne
        $script:OriginalBinaryHash) {
        Throw-InstallError "durable AOPMem home backup verification failed"
    }
    $workspaceRoot = Join-Path $script:AopmemHome "workspaces"
    $workspaceRootItem = Get-ExistingPathItem -LiteralPath $workspaceRoot
    if ($null -ne $workspaceRootItem) {
        Assert-SafeDirectory -LiteralPath $workspaceRoot -Label "workspace root"
        foreach ($workspace in @(Get-ChildItem -LiteralPath $workspaceRoot -Directory)) {
            if (Test-IsReparsePoint -Item $workspace) {
                Throw-InstallError "workspace must not be a reparse point: $($workspace.FullName)"
            }
            $backupWorkspace = Join-Path (
                Join-Path $script:FullBackupHome "workspaces") $workspace.Name
            Assert-SafeDirectory `
                -LiteralPath $backupWorkspace `
                -Label "durable backup workspace"
            $sourceDatabase = Join-Path $workspace.FullName "aopmem.sqlite"
            if ([IO.File]::Exists($sourceDatabase)) {
                $backupDatabase = Join-Path $backupWorkspace "aopmem.sqlite"
                Assert-SafeRegularFile `
                    -LiteralPath $backupDatabase `
                    -Label "durable backup database"
            }
        }
    }
    # Match Rust `write_tree_manifest`: ordinal per-directory DFS, files only,
    # backslash relative paths on Windows, and LF-only UTF-8 without BOM.
    $manifestText = New-Object System.Text.StringBuilder
    $script:ManifestEntryCount = 0
    $script:ManifestUtf8Bytes = 0
    function Write-BackupManifestTree {
        param([string]$Directory, [string]$Relative, [int]$Depth)
        if ($Depth -gt 128) {
            Throw-InstallError "durable backup exceeds maximum directory depth"
        }
        $names = [string[]]@([IO.Directory]::GetFileSystemEntries($Directory))
        [Array]::Sort($names, [StringComparer]::Ordinal)
        if ($names.Count -gt 10000) {
            Throw-InstallError "durable backup exceeds maximum directory entry count"
        }
        foreach ($path in $names) {
            $name = [IO.Path]::GetFileName($path)
            if ([string]::IsNullOrEmpty($Relative) -and
                $name -ceq "MANIFEST.sha256") {
                continue
            }
            $script:ManifestEntryCount += 1
            if ($script:ManifestEntryCount -gt 100000) {
                Throw-InstallError "durable backup exceeds maximum entry count"
            }
            if ($name.Contains("`n") -or $name.Contains("`r")) {
                Throw-InstallError "backup path is unsafe"
            }
            $item = Get-Item -LiteralPath $path -Force
            if (Test-IsReparsePoint -Item $item) {
                Throw-InstallError "durable backup must not contain a reparse point"
            }
            $childRelative = if ([string]::IsNullOrEmpty($Relative)) {
                $name
            } else {
                $Relative + "\\" + $name
            }
            if ($item.PSIsContainer) {
                Write-BackupManifestTree -Directory $item.FullName `
                    -Relative $childRelative -Depth ($Depth + 1)
            }
            elseif (-not $item.PSIsContainer) {
                $hash = Get-Sha256 -LiteralPath $item.FullName
                $record = ("{0} 0 {1}`n{2}`n" -f
                    $item.Length, $hash, $childRelative)
                $script:ManifestUtf8Bytes += $script:Utf8NoBom.GetByteCount($record)
                if ($script:ManifestUtf8Bytes -gt 33554432) {
                    Throw-InstallError "durable backup manifest exceeds maximum size"
                }
                [void]$manifestText.Append($record)
            }
            else {
                Throw-InstallError "durable backup contains a non-regular file"
            }
        }
    }
    Write-BackupManifestTree -Directory $script:FullBackupRoot -Relative "" -Depth 0
    if ($script:ManifestUtf8Bytes -gt 33554432) {
        Throw-InstallError "durable backup manifest is invalid"
    }
    $manifestPath = Join-Path $script:FullBackupRoot "MANIFEST.sha256"
    $manifestStream = $null
    $manifestWriter = $null
    try {
        $manifestStream = [IO.File]::Open(
            $manifestPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write,
            [IO.FileShare]::None)
        $manifestWriter = New-Object IO.StreamWriter($manifestStream, $script:Utf8NoBom, 4096, $true)
        $manifestWriter.Write($manifestText.ToString())
        $manifestWriter.Flush()
        $manifestStream.Flush($true)
    }
    finally {
        if ($null -ne $manifestWriter) { $manifestWriter.Dispose() }
        if ($null -ne $manifestStream) { $manifestStream.Dispose() }
    }
    $script:FullBackupManifestSha256 = Get-Sha256 -LiteralPath $manifestPath
    Write-TestTrace -EventName "backup.home.created"
}

function Select-ActiveAdapter {
    if ([string]::IsNullOrWhiteSpace($script:ActiveAdapter) -or
        [string]::IsNullOrWhiteSpace($script:ActiveInstructionFile)) {
        Throw-InstallError (
            "AOPMEM_ACTIVE_ADAPTER and AOPMEM_ACTIVE_INSTRUCTION_FILE are required")
    }
    $expectedInstructionFile = $null
    switch -CaseSensitive ($script:ActiveAdapter) {
        "codex" { $expectedInstructionFile = "AGENTS.md"; break }
        "claude" { $expectedInstructionFile = "CLAUDE.md"; break }
        "cursor" { $expectedInstructionFile = ".cursor/rules/aopmem.mdc"; break }
        "copilot" { $expectedInstructionFile = ".github/copilot-instructions.md"; break }
        default { Throw-InstallError "unsupported active adapter: $script:ActiveAdapter" }
    }
    if ($script:ActiveInstructionFile -cne $expectedInstructionFile) {
        Throw-InstallError "active adapter instruction file does not match the selected adapter"
    }
    Write-TestTrace -EventName ("adapter.selected.{0}" -f $script:ActiveAdapter)
}

function Invoke-StagedJson {
    param(
        [Parameter(Mandatory = $true)][string]$Context,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )
    if ($script:TestMode -and $script:TestFailAt -eq "publish" -and
        $Context -eq "upgrade publish") {
        Throw-InstallError "injected native upgrade publish failure"
    }
    $result = Invoke-AopmemJson -Executable $script:DownloadedBinary `
        -Arguments $Arguments -Context $Context
    if (-not [bool]$result.ok) {
        Throw-InstallError "$Context did not report success"
    }
    return $result
}

function Invoke-UpgradeStage {
    Write-TestTrace -EventName "upgrade.stage"
    $null = Invoke-StagedJson -Context "upgrade stage" -Arguments @(
        "upgrade", "stage", "--artifact", $script:DownloadedBinary,
        "--sha256", $script:VerifiedBinaryHash, "--json")
    $script:RecoveryBinary = Join-Path $script:InstallDir (
        ".aopmem-v{0}.staged" -f $script:ProductVersion)
    $script:RecoveryBinaryOwned = $false
}

function Invoke-StagedPlatformCheck {
    Write-TestTrace -EventName "platform.check.staged"
    $null = Invoke-StagedJson -Context "staged platform check" -Arguments @(
        "platform", "check", "--json")
}

function Invoke-UpgradeRecoveryInspect {
    Write-TestTrace -EventName "upgrade.recovery.inspect"
    $inspect = Invoke-StagedJson -Context "upgrade recovery inspect" -Arguments @(
        "upgrade", "recovery", "inspect", "--json")
    if ([bool]$inspect.data.apply_started -and
        -not [bool]$inspect.data.can_resume_publish) {
        [Console]::Error.WriteLine("AOPMem recovery inspect blocking JSON report:")
        [Console]::Error.WriteLine($script:LastAopmemJsonText)
        Throw-InstallError (
            "upgrade recovery has apply-started evidence; automatic fresh run is blocked")
    }
}

function Invoke-UpgradeRecoveryBackup {
    Write-TestTrace -EventName "upgrade.backup.fresh"
    $backup = Invoke-StagedJson -Context "upgrade backup fresh" -Arguments @(
        "upgrade", "backup", "--all-workspaces", "--json")
    $backupRoot = Get-OptionalJsonProperty `
        -Object $backup.data -Name "recovery_backup_root"
    if ([string]::IsNullOrWhiteSpace([string]$backupRoot)) {
        Throw-InstallError "upgrade backup did not report recovery backup root"
    }
    $script:UpgradeBackupRoot = [string]$backupRoot
}

function Invoke-AuditRepair {
    param(
        [Parameter(Mandatory = $true)][string]$Phase,
        [Parameter(Mandatory = $true)][string]$Executable
    )
    Write-TestTrace -EventName ("audit.repair.{0}" -f $Phase)
    $result = Invoke-AopmemJson -Executable $Executable `
        -Arguments @("audit", "repair", "--all-workspaces", "--json") `
        -Context ("audit repair " + $Phase)
    if (-not [bool]$result.ok) {
        Throw-InstallError "audit repair $Phase did not report success"
    }
}

function Invoke-AdapterSync {
    Write-TestTrace -EventName "adapter.sync"
    $result = Invoke-AopmemJson -Executable $script:InstalledBinary `
        -Arguments @("adapter", "sync", "--file", $script:ActiveInstructionFile, "--json") `
        -Context "adapter sync"
    if (-not [bool]$result.ok) {
        Throw-InstallError "adapter sync did not report success"
    }
}

function Invoke-DebugCapsuleExport {
    $directory = Join-Path $script:AopmemHome "debug-capsules"
    $existing = Get-ExistingPathItem -LiteralPath $directory
    if ($null -ne $existing -and
        ($existing -isnot [IO.DirectoryInfo] -or (Test-IsReparsePoint -Item $existing))) {
        Throw-InstallError "debug capsule directory must be a real directory"
    }
    [IO.Directory]::CreateDirectory($directory) | Out-Null
    $verified = Get-ExistingPathItem -LiteralPath $directory
    if ($null -eq $verified -or $verified -isnot [IO.DirectoryInfo] -or
        (Test-IsReparsePoint -Item $verified)) {
        Throw-InstallError "debug capsule directory changed during validation"
    }
    $output = Join-Path $directory (
        "upgrade-{0}.zip" -f [Diagnostics.Process]::GetCurrentProcess().Id)
    Write-TestTrace -EventName "debug.capsule.export"
    $result = Invoke-AopmemJson -Executable $script:InstalledBinary `
        -Arguments @("observe", "export", "--output", $output, "--json") `
        -Context "debug capsule export"
    if (-not [bool]$result.ok) {
        Throw-InstallError "debug capsule export did not report success"
    }
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

function Invoke-UpgradePrepare {
    Write-TestTrace -EventName "upgrade.prepare"
    $prepare = Invoke-AopmemJson `
        -Executable $script:DownloadedBinary `
        -Arguments @("upgrade", "prepare", "--all-workspaces", "--json") `
        -Context "upgrade prepare"
    if (-not [bool]$prepare.ok -or -not [bool]$prepare.data.success) {
        [Console]::Error.WriteLine("AOPMem upgrade prepare unsuccessful JSON report:")
        [Console]::Error.WriteLine($script:LastAopmemJsonText)
        Throw-InstallError "upgrade prepare did not report success"
    }
}

function Invoke-UpgradeApply {
    Write-TestTrace -EventName "upgrade.apply"
    $script:ApplyAttempted = $true
    $apply = Invoke-AopmemJson `
        -Executable $script:DownloadedBinary `
        -Arguments @(
            "upgrade", "apply", "--all-workspaces", "--json", "--approved", "+++") `
        -Context "upgrade apply"
    if (-not [bool]$apply.ok -or
        $apply.data.journal_phase -cne "applied" -or
        -not [bool]$apply.data.apply_invoked) {
        [Console]::Error.WriteLine("AOPMem upgrade apply unsuccessful JSON report:")
        [Console]::Error.WriteLine($script:LastAopmemJsonText)
        Throw-InstallError "upgrade apply did not report success"
    }
    $applyDetails = Get-OptionalJsonProperty -Object $apply.data -Name "apply"
    $script:UpgradeBackupRoot = [string](
        Get-OptionalJsonProperty -Object $applyDetails -Name "backup_root")
    Write-TestTrace -EventName "upgrade.apply.health.ok"
}

function Publish-VerifiedBinary {
    if ($script:Mode -ne "fresh") {
        Throw-InstallError "installer binary publish is allowed only for a fresh install"
    }
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
    [IO.File]::Move($script:InstallStage, $script:InstalledBinary)
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
    Write-TestTrace -EventName "adapter.status"
    $adapterStatus = Invoke-AopmemJson `
        -Executable $script:InstalledBinary `
        -Arguments @("adapter", "status", "--file", $script:ActiveInstructionFile, "--json") `
        -Context "adapter status"
    if (-not [bool]$adapterStatus.ok) {
        Throw-InstallError "adapter status did not report success"
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
    Write-TestTrace -EventName "task.start.smoke"
    $taskStart = "RC8 installer task-start smoke" | & $script:InstalledBinary `
        task start --query-stdin --json
    $taskExit = $LASTEXITCODE
    try {
        $taskStart = ([string]::Join([Environment]::NewLine, [string[]]$taskStart) |
            ConvertFrom-Json -ErrorAction Stop)
    }
    catch {
        Throw-InstallError "task-start smoke returned invalid JSON"
    }
    if ($taskExit -ne 0 -or -not [bool]$taskStart.ok -or
        -not [bool]$taskStart.data.mandatory_context_complete -or
        [string]::IsNullOrWhiteSpace([string]$taskStart.data.bundle_id) -or
        [string]::IsNullOrWhiteSpace([string]$taskStart.data.memory_revision)) {
        Throw-InstallError "task-start smoke did not return a complete bound receipt"
    }
    Write-TestTrace -EventName "observe.status"
    $observeStatus = Invoke-AopmemJson `
        -Executable $script:InstalledBinary `
        -Arguments @("observe", "status", "--json") `
        -Context "observe status"
    Write-TestTrace -EventName "observe.report"
    $observeReport = Invoke-AopmemJson `
        -Executable $script:InstalledBinary `
        -Arguments @("observe", "report", "--json") `
        -Context "observe report"
    foreach ($result in @($taskStart, $observeStatus, $observeReport)) {
        if (-not [bool]$result.ok) {
            Throw-InstallError "post-update workspace check did not report success"
        }
    }
    $workspaceKey = [string](
        Get-OptionalJsonProperty -Object $doctor.meta -Name "workspace_key")
    if ([string]::IsNullOrWhiteSpace($workspaceKey)) {
        Throw-InstallError "doctor did not report current workspace key"
    }
    foreach ($result in @(
            $adapterStatus, $verify, $taskStart, $observeStatus, $observeReport)) {
        $resultWorkspaceKey = [string](
            Get-OptionalJsonProperty -Object $result.meta -Name "workspace_key")
        if ($resultWorkspaceKey -cne $workspaceKey) {
            Throw-InstallError "post-update workspace key changed during checks"
        }
    }
}

function Invoke-FreshAdapterSeed {
    Write-TestTrace -EventName "adapter.seed"
    $adapter = Invoke-AopmemJson `
        -Executable $script:InstalledBinary `
        -Arguments @("adapter", "seed", "--file", $script:ActiveInstructionFile, "--json") `
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
    Select-ActiveAdapter
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

    $existingItem = Get-ExistingPathItem -LiteralPath $script:InstalledBinary
    if ($null -ne $existingItem) {
        if ($existingItem.PSIsContainer -or (Test-IsReparsePoint -Item $existingItem)) {
            Throw-InstallError "installed binary path is not a regular file: $script:InstalledBinary"
        }
        $installedVersion = Get-AopmemVersion -Executable $script:InstalledBinary
        if ($installedVersion -cnotin @(
                "aopmem 0.1.0",
                "aopmem 0.2.0-rc1",
                "aopmem 0.2.0-rc2",
                "aopmem 0.2.0-rc3",
                "aopmem 0.2.0-rc4",
                "aopmem 0.2.0-rc5",
                "aopmem 0.2.0-rc6")) {
            Throw-InstallError "existing version is unsupported: $installedVersion"
        }
        $script:OldReleaseLabel = $installedVersion.Substring("aopmem ".Length)
        $expectedOldHash = $script:KnownSourceHashes[$installedVersion]
        if ($script:TestMode -and -not [string]::IsNullOrWhiteSpace(
                $env:AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256)) {
            $expectedOldHash = $env:AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256
        }
        $installedOldHash = Get-Sha256 -LiteralPath $script:InstalledBinary
        if ([string]::IsNullOrWhiteSpace($expectedOldHash) -or
            $installedOldHash -cne $expectedOldHash.ToLowerInvariant()) {
            $warningCode = "NONCANONICAL_SOURCE_BINARY"
            if ($installedVersion -ceq "aopmem 0.1.0") {
                $warningCode = "NONCANONICAL_V010_BINARY"
            }
            [Console]::Error.WriteLine((
                "WARNING {0}: version={1} sha256={2}; version is compatible; " +
                "workspace compatibility will be decided by upgrade prepare and plan"
            ) -f $warningCode, $installedVersion, $installedOldHash)
            Write-TestTrace -EventName ("warning.{0}" -f $warningCode)
        }
        $script:Mode = "update"
        Assert-NoActiveAopmemProcesses
        Backup-OldBinary
        Backup-AopmemHome
        if ($script:TestMode -and $script:TestFailAt -eq "after_backup") {
            Throw-InstallError "injected failure after old binary backup"
        }
    }
    else {
        $script:Mode = "fresh"
    }

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
    Write-TestTrace -EventName "asset.download.started"
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

    if ($script:Mode -eq "update") {
        Invoke-StagedPlatformCheck
        Invoke-UpgradeRecoveryInspect
        Invoke-UpgradeRecoveryBackup
        Invoke-UpgradeStage
        Invoke-AuditRepair -Phase "staged" -Executable $script:DownloadedBinary
        Invoke-UpgradePrepare
        Invoke-UpgradePlan
        Invoke-UpgradeApply
        Write-TestTrace -EventName "upgrade.publish"
        $publish = Invoke-StagedJson -Context "upgrade publish" -Arguments @(
            "upgrade", "publish", "--json")
        if ($publish.data.journal_phase -cne "published" -or
            -not [bool]$publish.data.binary_published) {
            Throw-InstallError "upgrade publish did not confirm the published binary"
        }
        $script:BinaryPublished = $true
        Invoke-AdapterSync
        Invoke-AuditRepair -Phase "post-publish" -Executable $script:InstalledBinary
        Invoke-CurrentWorkspaceHealth
        Invoke-DebugCapsuleExport
        [Console]::Out.WriteLine(
            "AOPMem {0} updated. doctor=ok verify=ok task_start=ok observability=ok full_backup={1} binary_backup={2} upgrade_backup={3}" -f
                $script:ProductVersion,
                $script:FullBackupRoot,
                $script:BackupPath,
                $script:UpgradeBackupRoot)
    }
    else {
        Prepare-PublishFiles
        Publish-VerifiedBinary
        Write-TestTrace -EventName "init"
        & $script:InstalledBinary init
        if ($LASTEXITCODE -ne 0) {
            Throw-InstallError "fresh workspace semantic initialization failed"
        }
        Invoke-FreshAdapterSeed
        Invoke-CurrentWorkspaceHealth
        Invoke-DebugCapsuleExport
        [Console]::Out.WriteLine(
            "AOPMem {0} installed. doctor=ok verify=ok task_start=ok observability=ok" -f
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
                "AOPMem install failed: {0}; old binary was left unchanged; full backup preserved at {1}; binary backup preserved at {2}" -f
                $failure,
                $script:FullBackupRoot,
                $script:BackupPath)
            [Console]::Error.WriteLine($errorLine)
        }
        catch {
            $integrityFailure = $_.Exception.Message
            $errorLine = (
                (
                    "AOPMem install failed: {0}; old binary integrity check failed: {1}; " +
                    "full backup preserved at {2}; binary backup preserved at {3}"
                ) -f
                    $failure,
                    $integrityFailure,
                    $script:FullBackupRoot,
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
