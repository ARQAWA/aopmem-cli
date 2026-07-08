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
dist\aopmem-windows-x86_64\aopmem.exe
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
Copy-Item ".\dist\aopmem-windows-x86_64\aopmem.exe" "$InstallDir\aopmem.exe" -Force

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
Copy-Item ".\dist\aopmem-windows-x86_64\aopmem.exe" "$InstallDir\aopmem.exe" -Force

& "$InstallDir\aopmem.exe" --version
& "$InstallDir\aopmem.exe" --help

$env:AOPMEM_HOME = "$env:TEMP\aopmem-rc3-home"
$Repo = "$env:TEMP\aopmem-rc3-repo"

Remove-Item -Recurse -Force $env:AOPMEM_HOME -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force $Repo -ErrorAction SilentlyContinue

New-Item -ItemType Directory -Force $Repo | Out-Null
Set-Location $Repo

@"
n
n
Тестовый Windows workspace для AOPMem rc3.
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

- `aopmem 0.1.0`
- JSON `ok=true`, or healthy equivalent
- `Test-Path "$Repo\.aopmem"` returns `False`
- `$env:AOPMEM_HOME\workspaces\...` exists
- `AGENTS.md` contains real managed AOPMem block
- Russian text is stored as UTF-8, not `????`
