# Windows native update

This path supports Windows 11 x64 with native Windows PowerShell 5.1.
It updates AOPMem v0.1.0-rc3 to v0.2.0-rc2.

## Rules

- Do not use administrator rights.
- Do not use WSL.
- Do not install Cargo, Rustup, Git, Node.js, or Codex CLI.
- Do not clone or build source.
- Do not open another terminal.
- Do not change the machine execution policy.
- Use `%USERPROFILE%\.aopmem`.

The release context must supply the trusted HTTPS asset base URI.
Do not guess or hard-code it.

Required flat assets:

- `aopmem-windows-x86_64.exe`
- `SHA256SUMS`

## Run

Use the existing console in the project.
Set the release-provided URI without adding a guessed URL.
Then run the system Windows PowerShell 5.1 process in the same console:

```powershell
$env:AOPMEM_ASSET_BASE_URI = "<trusted release asset base URI>"
$WindowsPowerShell = Join-Path $env:SystemRoot `
  "System32\WindowsPowerShell\v1.0\powershell.exe"
& $WindowsPowerShell `
  -NoLogo `
  -NoProfile `
  -ExecutionPolicy Bypass `
  -File ".\install\v0.2\install.ps1"
```

Or pass the same value directly:

```powershell
$WindowsPowerShell = Join-Path $env:SystemRoot `
  "System32\WindowsPowerShell\v1.0\powershell.exe"
& $WindowsPowerShell `
  -NoLogo `
  -NoProfile `
  -ExecutionPolicy Bypass `
  -File ".\install\v0.2\install.ps1" `
  -AssetBaseUri "<trusted release asset base URI>"
```

`-ExecutionPolicy Bypass` applies only to this child process.
It does not change user or machine policy.
The child uses the same console, so fresh semantic questions still work.

The script detects Windows, build, process architecture, and PowerShell
version silently. Unsupported systems stop before asset execution.

## What the script enforces

The script:

- selects only `aopmem-windows-x86_64.exe`;
- enables TLS 1.2;
- sets console input, output, and pipeline encoding to UTF-8;
- uses `Invoke-WebRequest -UseBasicParsing`;
- disables automatic redirects and checks each redirect itself;
- rejects HTTP downgrade, credentials, empty host, and excess redirects;
- downloads into a random temporary directory;
- uses `Get-FileHash -Algorithm SHA256`;
- requires one exact unique checksum filename;
- checks exact version before any upgrade command;
- rejects junctions, symlinks, and reparse points in managed paths;
- uses `File.Replace` for same-directory atomic update publication;
- removes temporary downloads in `finally`.

No PowerShell 7 syntax is used. The script has no `&&`, ternary operator,
null-coalescing operator, class declaration, parallel pipeline, or PS7 API.

## Update behavior

For exact v0.1.0-rc3, the script asks no onboarding questions.
That tagged executable reports `aopmem 0.1.0`; the script also checks its
known v0.1.0-rc3 asset hash before accepting the update source.
The accepted Windows SHA-256 is
`01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a`.
It runs:

```text
aopmem upgrade plan --all-workspaces --json
aopmem upgrade apply --all-workspaces --json
```

Both commands use the downloaded and verified v0.2 binary.
The old installed binary is not replaced before apply succeeds.

Successful apply includes its doctor and verify checks. Publication then
uses `System.IO.File.Replace` in `%USERPROFILE%\.aopmem\bin`.

The final line is short:

```text
AOPMem 0.2.0-rc2 updated. doctor=ok verify=ok
binary_backup=<path> upgrade_backup=<path>
```

## Fresh behavior

With no installed AOPMem binary, the script:

1. publishes the verified binary;
2. runs normal `aopmem init`;
3. lets the CLI ask its existing five semantic questions;
4. runs `adapter seed --json` and requires `ok=true`;
5. runs `doctor --json` and requires `healthy=true`;
6. runs `verify --json` and requires `clean=true`.

Fresh install is the only flow that asks onboarding questions.

## Failure recovery

Before apply starts, the installed v0.1 file remains byte-for-byte
unchanged. The script verifies that fact instead of rewriting the file.

After apply starts, do not run or restore v0.1. Some workspace data may
already use v0.2 schema state.

For an apply or atomic publication failure, the script prints and preserves:

- the exact verified v0.2 recovery binary path;
- the durable installer binary backup path;
- upgrade-run backups under
  `%USERPROFILE%\.aopmem\backups\upgrade-0.2.0-rc2-*`;
- the exact command or workspace error.

Keep these files. Fix the reported cause. Then use the printed v0.2 recovery
binary to rerun the read-only plan before another apply attempt.

The installer never deletes workspaces or backups.

## Static proof

Run this repository check on macOS:

```sh
scripts/audit_v020_installers.sh
```

The check does not execute PowerShell or contact a server. It audits the
PowerShell 5.1 contract and runs the macOS flow against safe local fixtures.

A real Windows dogfood run must still execute `install.ps1` in native
Windows PowerShell 5.1. Static proof is not a substitute for that host proof.
