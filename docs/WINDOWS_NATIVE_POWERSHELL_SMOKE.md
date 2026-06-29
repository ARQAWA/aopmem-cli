# Windows Native PowerShell Smoke

Target:

- Windows 11
- x64
- native PowerShell only
- no WSL
- no bash
- no `cargo build`

Run from the AOPMem repo root after this artifact exists:

```text
dist\aopmem-windows-x86_64\aopmem.exe
```

Install target:

```text
$env:USERPROFILE\.aopmem\bin\aopmem.exe
```

## Install And Basic CLI Proof

```powershell
$InstallDir = "$env:USERPROFILE\.aopmem\bin"
New-Item -ItemType Directory -Force $InstallDir | Out-Null
Copy-Item ".\dist\aopmem-windows-x86_64\aopmem.exe" "$InstallDir\aopmem.exe" -Force

& "$InstallDir\aopmem.exe" --version
& "$InstallDir\aopmem.exe" --help
```

## Runtime Smoke

```powershell
$ErrorActionPreference = "Stop"

$env:AOPMEM_HOME = "$env:TEMP\aopmem-rc2-home"
$Repo = "$env:TEMP\aopmem-rc2-repo"

function Assert-JsonOk {
  param(
    [Parameter(Mandatory = $true)]
    [string] $Name,
    [Parameter(Mandatory = $true)]
    [object] $JsonText
  )

  $Parsed = $JsonText | ConvertFrom-Json
  if ($Parsed.ok -ne $true) {
    throw "$Name did not return ok=true"
  }

  return $Parsed
}

Remove-Item -Recurse -Force -Path $env:AOPMEM_HOME, $Repo -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $env:AOPMEM_HOME, $Repo | Out-Null

Set-Content -Path "$Repo\AGENTS.md" -Encoding UTF8 -Value @(
  "# AOPMem Windows rc2 smoke"
  ""
  "Temporary instruction file."
)

Push-Location $Repo
try {
  $InitAnswers = @(
    "no"
    "no"
    "AOPMem Windows rc2 smoke."
    "User checks install. Agent follows repo rules."
    "Only temp repo is in scope."
  )

  $InitJson = $InitAnswers | & "$InstallDir\aopmem.exe" --json init
  if ($LASTEXITCODE -ne 0) { throw "init failed" }
  $Init = Assert-JsonOk "init" $InitJson

  $AdapterSeedJson = & "$InstallDir\aopmem.exe" --json adapter seed --file .\AGENTS.md
  if ($LASTEXITCODE -ne 0) { throw "adapter seed failed" }
  $AdapterSeed = Assert-JsonOk "adapter seed" $AdapterSeedJson

  $AdapterStatusJson = & "$InstallDir\aopmem.exe" --json adapter status --file .\AGENTS.md
  if ($LASTEXITCODE -ne 0) { throw "adapter status failed" }
  $AdapterStatus = Assert-JsonOk "adapter status" $AdapterStatusJson
  if ($AdapterStatus.data.managed_block -ne "in_sync") {
    throw "adapter status did not report managed_block=in_sync"
  }

  $DoctorJson = & "$InstallDir\aopmem.exe" --json doctor
  if ($LASTEXITCODE -ne 0) { throw "doctor failed" }
  $Doctor = Assert-JsonOk "doctor" $DoctorJson

  $RecallJson = & "$InstallDir\aopmem.exe" --json recall
  if ($LASTEXITCODE -ne 0) { throw "recall failed" }
  $Recall = Assert-JsonOk "recall" $RecallJson

  $WorkspacesDir = Join-Path $env:AOPMEM_HOME "workspaces"
  $WorkspaceDirs = @(Get-ChildItem -Path $WorkspacesDir -Directory)
  if ($WorkspaceDirs.Count -ne 1) {
    throw "expected exactly one workspace under AOPMEM_HOME"
  }
  if ($WorkspaceDirs[0].FullName -notlike "$env:AOPMEM_HOME*") {
    throw "workspace is outside AOPMEM_HOME"
  }
  if (-not (Test-Path (Join-Path $WorkspaceDirs[0].FullName "aopmem.sqlite"))) {
    throw "workspace sqlite missing"
  }

  $RepoAopmemExists = Test-Path "$Repo\.aopmem"
  if ($RepoAopmemExists) {
    throw ".aopmem exists inside target repo"
  }

  $AgentsText = Get-Content -Raw "$Repo\AGENTS.md"
  if ($AgentsText -notmatch "<!-- AOPMEM:BEGIN managed block -->") {
    throw "AGENTS.md missing managed AOPMem begin marker"
  }
  if ($AgentsText -notmatch "<!-- AOPMEM:END managed block -->") {
    throw "AGENTS.md missing managed AOPMem end marker"
  }

  Write-Output "PASS JSON commands returned ok=true"
  Write-Output "PASS Test-Path `"$Repo\.aopmem`" returns False"
  Write-Output "PASS workspace under AOPMEM_HOME"
  Write-Output "PASS no .aopmem inside target repo"
  Write-Output "PASS AGENTS.md contains managed AOPMem block"
}
finally {
  Pop-Location
}
```

Expected result:

- `--version` prints `aopmem 0.1.0`.
- `--help` prints CLI help.
- JSON commands return `ok=true`, or documented healthy equivalent:
  - `adapter status` returns `managed_block=in_sync`.
- `Test-Path "$Repo\.aopmem"` returns `False`.
- `$env:AOPMEM_HOME\workspaces\...` exists.
- AGENTS.md contains managed AOPMem block.
