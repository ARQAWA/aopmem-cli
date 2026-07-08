Use this prompt to install and initialize AOPMem v0.1 for the current
repository.

````text
You are installing AOPMem v0.1 into the user's current host and initializing
it for the current repository.

Important rules:

- Do the full install and workspace init flow.
- Stay inside user-level install only.
- Detect technical facts silently.
- Do not ask the user about things you can detect yourself.
- Do not ask any irrelevant technical questionnaire.
- Ask only the 5 semantic install questions listed below.
- No final confirmation ceremony.
- If the managed AOPMem block already exists, update only that block.
- If the managed block is damaged, stop with an explicit error.
- Do not build from source during user install when a matching binary artifact
  exists.
- If the matching binary artifact is missing, fail fast with:
  binary artifact missing for current platform.
- Do not create `.aopmem` in the target repository.
- Do not use WSL as the Windows install path.

Supported platforms:

- macOS Apple Silicon:
  - detect macOS + arm64 silently.
  - install from `dist/aopmem-darwin-arm64/aopmem`.
  - install to `~/.aopmem/bin/aopmem`.
- Windows x64 native PowerShell:
  - detect Windows + x64 silently.
  - use native PowerShell only.
  - do not use WSL.
  - install from `dist\aopmem-windows-x86_64\aopmem.exe`.
  - artifact path in release docs:
    `dist/aopmem-windows-x86_64/aopmem.exe`.
  - install to `%USERPROFILE%\.aopmem\bin\aopmem.exe`.
  - use PowerShell commands only.
  - use backslashes in Windows path examples.
  - set PowerShell UTF-8 encoding before piping semantic answers.

Unsupported platforms:

- Linux is unsupported in v0.1.
- Windows ARM is unsupported in v0.1.
- Intel macOS is unsupported in v0.1.

Silent technical detection:

- current OS and architecture
- current repo root
- current agent environment and instruction file
- existing managed AOPMem block
- whether AOPMem is already installed globally
- workspace key
- backend path under the user-level AOPMem home
- whether .understand.docs already exists

Install flow:

1. Check whether AOPMem is already installed globally.
2. Select the matching prebuilt binary for the current platform.
3. If the matching binary artifact is missing, fail fast with
   `binary artifact missing for current platform`.
4. If AOPMem is missing, install the AOPMem CLI into the user-level bin dir.
5. Create and verify the required global directories under the user-level
   AOPMem home.
6. Create or reuse the workspace for the current repository under
   <AOPMem home>/workspaces/<workspace-key>.
7. Ask whether to enable Understand Anything.
8. If enabled, do best-effort local setup and create .understand.docs.
9. Ask whether to enable Codebase Memory MCP.
10. If enabled, do best-effort local setup.
11. Ask the semantic project onboarding questions.
12. Seed the collected semantic answers into AOPMem.
13. Seed the required default kernel/contracts/gates data.
14. Insert or update the managed AOPMem block in the current instruction file.
15. Initialize local audit snapshots.
16. Run `aopmem doctor`.
17. Run the first recall bundle.

macOS Apple Silicon install commands:

```sh
mkdir -p "$HOME/.aopmem/bin"
test -s "dist/aopmem-darwin-arm64/aopmem" || {
  echo "binary artifact missing for current platform" >&2
  exit 1
}
cp "dist/aopmem-darwin-arm64/aopmem" "$HOME/.aopmem/bin/aopmem"
chmod 755 "$HOME/.aopmem/bin/aopmem"
"$HOME/.aopmem/bin/aopmem" --version
```

Windows x64 native PowerShell install commands:

```powershell
chcp 65001
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new()
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$OutputEncoding = [System.Text.UTF8Encoding]::new()

$InstallDir = "$env:USERPROFILE\.aopmem\bin"
New-Item -ItemType Directory -Force $InstallDir | Out-Null
if (-not (Test-Path ".\dist\aopmem-windows-x86_64\aopmem.exe")) {
  Write-Error "binary artifact missing for current platform"
  exit 1
}
Copy-Item ".\dist\aopmem-windows-x86_64\aopmem.exe" "$InstallDir\aopmem.exe" -Force
& "$InstallDir\aopmem.exe" --version
```

Temp proof paths:

- macOS: use `AOPMEM_HOME` with a temp path outside the target repo.
- Windows PowerShell: use `$env:AOPMEM_HOME`, for example:

```powershell
$env:AOPMEM_HOME = "$env:TEMP\aopmem-proof"
```

Workspace rule:

- Create or reuse the workspace under the selected AOPMem home:
  `workspaces\<workspace-key>` on Windows and `workspaces/<workspace-key>` on
  macOS.
- Do not create `.aopmem` in the target repository.

Ask only these 5 user-facing questions, exactly in this order:

1. Включаем Understand Anything для локального понимания проекта и
   .understand.docs?
2. Включаем Codebase Memory MCP для навигации по коду?
3. Объясни, что это за проект, зачем он нужен и чем мы тут занимаемся.
4. Какая твоя роль в этом проекте и какая роль у агента?
5. Какие части проекта рабочие, какие вспомогательные, какие нельзя трогать?

Do not ask about:

- OS
- shell
- repo path
- workspace id
- database location
- adapter type
- instruction file name
- whether to use the default communication style
- any long preferences/style questionnaire
- any other technical facts you can detect silently

After install:

- briefly report what was done
- report any optional setup that was skipped or failed in best-effort mode
- confirm doctor result
- do not dump unnecessary technical detail unless there is an error
````
