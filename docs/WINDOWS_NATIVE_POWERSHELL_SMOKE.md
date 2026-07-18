# Windows Native PowerShell Smoke

Target:

- Windows 11
- x64
- native PowerShell only
- no WSL
- no bash
- no `cargo build`

Required prebuilt artifact:

```text
dist\aopmem-windows-x86_64.exe
```

Install target:

```text
$env:USERPROFILE\.aopmem\bin\aopmem.exe
```

## Install And Basic CLI Proof

```powershell
chcp 65001
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new()
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$OutputEncoding = [System.Text.UTF8Encoding]::new()

$InstallDir = "$env:USERPROFILE\.aopmem\bin"
New-Item -ItemType Directory -Force $InstallDir | Out-Null
Copy-Item ".\dist\aopmem-windows-x86_64.exe" "$InstallDir\aopmem.exe" -Force

& "$InstallDir\aopmem.exe" --version
& "$InstallDir\aopmem.exe" --help
```

## Runtime Smoke

```powershell
$ErrorActionPreference = "Stop"

chcp 65001
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new()
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$OutputEncoding = [System.Text.UTF8Encoding]::new()

$InstallDir = "$env:USERPROFILE\.aopmem\bin"
New-Item -ItemType Directory -Force $InstallDir | Out-Null
Copy-Item ".\dist\aopmem-windows-x86_64.exe" "$InstallDir\aopmem.exe" -Force

& "$InstallDir\aopmem.exe" --version
& "$InstallDir\aopmem.exe" --help

$env:AOPMEM_HOME = "$env:TEMP\aopmem-rc5-home"
$Repo = "$env:TEMP\aopmem-rc5-repo"

Remove-Item -Recurse -Force $env:AOPMEM_HOME -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force $Repo -ErrorAction SilentlyContinue

New-Item -ItemType Directory -Force $Repo | Out-Null
Set-Location $Repo

@"
n
n
Тестовый Windows workspace для AOPMem rc5.
Пользователь проверяет установку; агент ведет operational memory.
Вся папка рабочая; ничего запрещенного нет.
"@ | & "$InstallDir\aopmem.exe" --json init

& "$InstallDir\aopmem.exe" --json adapter seed --file AGENTS.md
& "$InstallDir\aopmem.exe" --json adapter status --file AGENTS.md
& "$InstallDir\aopmem.exe" --json doctor
& "$InstallDir\aopmem.exe" --json recall

Test-Path "$Repo\.aopmem"
Get-ChildItem "$env:AOPMEM_HOME\workspaces"
Select-String -Path ".\AGENTS.md" -Pattern "AOPMEM:BEGIN|aopmem recall|Memory Keeper|tool run|\+\+\+"
Select-String -Path "$env:AOPMEM_HOME\workspaces\*\audit-git\memory.sql" -Pattern "????" -SimpleMatch
```

Expected result:

- `aopmem 0.2.0-rc5`
- JSON `ok=true`, or healthy equivalent
- `Test-Path "$Repo\.aopmem"` returns `False`
- `$env:AOPMEM_HOME\workspaces\...` exists
- `AGENTS.md` contains real managed AOPMem block
- Russian text is stored as UTF-8, not `????`

## Proof status

This is a native Windows dogfood procedure. It remains
`PENDING_DOGFOOD` until it is run on Windows 11 x64 with PowerShell 5.1.
The macOS PE, import, and checksum checks do not prove the Windows runtime.
