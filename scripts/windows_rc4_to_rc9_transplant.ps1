[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("Plan", "Execute", "Rollback")]
    [string]$Action,

    [Parameter(Mandatory = $false)]
    [string]$Python = "python",

    [Parameter(Mandatory = $false)]
    [string]$LiveHome,

    [Parameter(Mandatory = $false)]
    [string]$QuarantineRoot,

    [Parameter(Mandatory = $false)]
    [string]$FailedRoot,

    [Parameter(Mandatory = $false)]
    [string]$Report,

    [Parameter(Mandatory = $false)]
    [string]$RepoRoot,

    [Parameter(Mandatory = $false)]
    [string]$Installer,

    [Parameter(Mandatory = $false)]
    [string]$InstallerSha256,

    [Parameter(Mandatory = $false)]
    [string]$AssetBaseUri,

    [Parameter(Mandatory = $false)]
    [string]$SourceHash,

    [Parameter(Mandatory = $false)]
    [string]$TargetHash,

    [Parameter(Mandatory = $false)]
    [switch]$RequireExpectedWorkspaces
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = "Stop"

function Add-Arg {
    param(
        [Parameter(Mandatory = $true)]
        [System.Collections.Generic.List[string]]$List,
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $false)]
        [string]$Value
    )

    if (-not [string]::IsNullOrWhiteSpace($Value)) {
        $List.Add($Name)
        $List.Add([IO.Path]::GetFullPath($Value))
    }
}

$scriptPath = Join-Path $PSScriptRoot "windows_rc4_to_rc9_transplant.py"
$argv = New-Object System.Collections.Generic.List[string]
$argv.Add($scriptPath)
$argv.Add("--action")
$argv.Add($Action)
Add-Arg -List $argv -Name "--live-home" -Value $LiveHome
Add-Arg -List $argv -Name "--quarantine-root" -Value $QuarantineRoot
Add-Arg -List $argv -Name "--failed-root" -Value $FailedRoot
Add-Arg -List $argv -Name "--report" -Value $Report
Add-Arg -List $argv -Name "--repo-root" -Value $RepoRoot
Add-Arg -List $argv -Name "--installer" -Value $Installer
if (-not [string]::IsNullOrWhiteSpace($InstallerSha256)) {
    $argv.Add("--installer-sha256")
    $argv.Add($InstallerSha256)
}
if (-not [string]::IsNullOrWhiteSpace($AssetBaseUri)) {
    $argv.Add("--asset-base-uri")
    $argv.Add($AssetBaseUri)
}
if (-not [string]::IsNullOrWhiteSpace($SourceHash)) {
    $argv.Add("--source-hash")
    $argv.Add($SourceHash)
}
if (-not [string]::IsNullOrWhiteSpace($TargetHash)) {
    $argv.Add("--target-hash")
    $argv.Add($TargetHash)
}
if ($RequireExpectedWorkspaces) {
    $argv.Add("--require-expected-workspaces")
}

& $Python @argv
if ($LASTEXITCODE -ne 0) {
    throw "RC4_TO_RC9_TRANSPLANT_FAILED exit=$LASTEXITCODE"
}
