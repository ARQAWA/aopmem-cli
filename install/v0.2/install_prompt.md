# AOPMem v0.2.0-rc4 install prompt

Use this prompt to install AOPMem v0.2.0-rc4 for the current project.
It supports a fresh install and an update from compatible AOPMem v0.1.0
SQLite workspaces.

````text
You are installing AOPMem v0.2.0-rc4 for the user's current project.

Complete the whole safe flow without pausing between normal steps.
Do not run Codex CLI during installation.
Do not open another terminal.
Do not ask technical questions which can be detected.
Do not ask for a final confirmation.

Supported hosts:

- macOS Apple Silicon: Darwin arm64.
- Windows 11 x64: native Windows PowerShell 5.1.

Fail with an exact unsupported-platform error on every other host.
Do not use Linux, WSL, Windows ARM, Intel Mac, or Windows PowerShell 7.

Forbidden install methods:

- administrator rights;
- WSL;
- cargo or rustup;
- git clone;
- source builds;
- Node.js;
- Codex CLI;
- external terminals.

Release inputs:

- The trusted release context supplies an HTTPS asset base URI.
- Do not invent, guess, search for, or hard-code a release URL.
- The base URI must contain no credentials, query, or fragment.
- Only test mode may inject assets from a local fixture directory.
- Use exactly these flat release assets:
  - aopmem-darwin-arm64
  - aopmem-windows-x86_64.exe
  - SHA256SUMS

Integrity rules:

- Download into a new private temporary directory.
- Find exactly one SHA256SUMS line whose filename exactly matches the
  selected flat asset name.
- Reject a missing, duplicate, malformed, or differently named line.
- Verify SHA-256 before chmod or any binary execution.
- Verify the downloaded binary reports exactly:
  aopmem 0.2.0-rc4
- Never execute an unverified file.

Path rules:

- macOS home: $HOME/.aopmem
- Windows home: %USERPROFILE%\.aopmem
- macOS binary: $HOME/.aopmem/bin/aopmem
- Windows binary: %USERPROFILE%\.aopmem\bin\aopmem.exe
- Reject a symlink, junction, reparse point, directory, or other unsafe
  object where AOPMem home, bin, binary, stage, or recovery file must be.
- Never create a project-local .aopmem directory.

Select the flow silently:

1. No installed binary means fresh install.
2. A compatible v0.1 binary reports exactly `aopmem 0.1.0`.
   Recognize the known v0.1.0-rc3 release SHA-256. For another SHA-256,
   emit `NONCANONICAL_V010_BINARY`, require the durable full backup, and let
   staged `upgrade prepare` plus `upgrade plan` decide workspace compatibility.
3. Any other installed version is unsupported. Stop without changing it.

For macOS, use the supplied install/v0.2/install.sh.
Pass the trusted base through AOPMEM_ASSET_BASE_URI.
The script must use curl with fail, TLS 1.2, HTTPS-only initial and redirect
protocols, shasum -a 256, chmod, private temp files, and same-directory mv.

For Windows, use the supplied install/v0.2/install.ps1.
Pass the trusted base through -AssetBaseUri or AOPMEM_ASSET_BASE_URI.
Use native Windows PowerShell 5.1 only.
Invoke the system Windows PowerShell executable in the same console with
-NoProfile and process-only -ExecutionPolicy Bypass. This does not change
the user or machine execution policy and must not open a new window.
The script must configure TLS 1.2 and UTF-8, use
Invoke-WebRequest -UseBasicParsing, inspect each redirect with automatic
redirects disabled, use Get-FileHash, and publish with same-directory
File.Replace.

Fresh flow:

1. Verify and stage the new binary.
2. Publish it atomically in the user-level bin directory.
3. Run the normal aopmem init flow in the current project.
4. Let that CLI ask its existing five semantic questions.
5. Run `aopmem adapter seed --json` and require `ok=true`.
6. Run `aopmem doctor --json` and require `ok=true`, `healthy=true`.
7. Run `aopmem verify --json` and require `ok=true`, `clean=true`.
8. Print one short final status.

The existing five questions are:

1. Включаем Understand Anything для локального понимания проекта и
   .understand.docs?
2. Включаем Codebase Memory MCP для навигации по коду?
3. Объясни, что это за проект, зачем он нужен и чем мы тут занимаемся.
4. Какая твоя роль в этом проекте и какая роль у агента?
5. Какие части проекта рабочие, какие вспомогательные, какие нельзя трогать?

Do not add, remove, reorder, or paraphrase these questions.

Update flow:

1. Ask zero onboarding questions.
2. Require all AOPMem UI and CLI processes to be closed. Do not terminate an
   unknown process automatically.
3. Create and verify a durable full backup of AOPMem home plus the old binary.
4. Prepare verified v0.2 stage and recovery binaries in the install
   directory.
5. Run the downloaded temporary v0.2 binary:
   aopmem upgrade prepare --all-workspaces --json
6. Require exit code 0, ok=true, and success=true. Preparation may checkpoint
   SQLite and create its own per-workspace backups. On failure, do not run plan,
   apply, or publish.
7. Immediately run the same temporary v0.2 binary:
   aopmem upgrade plan --all-workspaces --json
8. Run no AOPMem DB-read command between prepare and plan. Require ok=true,
   ready=true, and writes_performed=false.
9. Then run the same temporary v0.2 binary:
   aopmem upgrade apply --all-workspaces --json --approved "+++"
10. Require exit code 0, ok=true, success=true, and
   binary_replaced=false.
11. Only after successful apply, replace the installed binary atomically.
12. Verify the installed SHA-256 and exact version.
13. Run `aopmem adapter status --json`, `doctor --json`, `verify --json`,
   `recall --json`, `observe status --json`, and `observe report --json` with
   the installed binary. Require ok=true, doctor healthy=true, verify
   clean=true, and one stable non-empty current workspace key across reports.
   Do not sync the adapter automatically; report a real status failure.
14. Print one short final status and all durable backup paths.

Failure rules:

- Before upgrade apply starts, the installed v0.1 binary must stay
  byte-for-byte unchanged. Keep its backup.
- Before upgrade prepare starts, create the durable full backup. Preparation
  failure must block plan, apply, and publish.
- After upgrade apply starts, some v0.2 data may already be committed.
- Never restore or republish v0.1 after that point.
- On any apply or later publish failure, keep the verified same-directory
  v0.2 recovery binary and print its exact path.
- Tell the user not to run v0.1 after such a failure.
- Keep every upgrade backup and every workspace.
- Remove only installer temporary files.
- Print the exact workspace and error returned by upgrade.
- Do not continue other workspaces silently after a failure.

Success report:

- version;
- fresh or updated;
- doctor=ok;
- verify=ok;
- recall=ok;
- observability=ok;
- binary backup path for update.
- durable full-backup path for update.
- upgrade-run backup path for update.

Do not push, tag, create a release, or install into any workspace other than
the user's selected current installation.
````

Implementation files:

- `install/v0.2/install.sh`
- `install/v0.2/install.ps1`
- `scripts/audit_v020_installers.sh`
