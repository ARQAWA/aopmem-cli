# RC7 native Windows acceptance prompt

Use this complete prompt only after the RC7 prerelease and all assets exist.
This is native acceptance, not a release-build procedure.

## Mission

Validate the official AOPMem `v0.2.0-rc7` updater on the approved native
Windows environment where direct GitHub access is unavailable and a corporate
proxy is required. Preserve every user workspace, backup, file, and Git state.
Do not claim acceptance from macOS-hosted evidence.

## Fixed environment

- Windows 11 Enterprise x64, build `22631`.
- Native Windows PowerShell `5.1.22621.7376`.
- Ordinary user; no administrator rights.
- No WSL, Bash, Docker, Cargo, Rustup, source build, Node.js, Codex CLI, or
  external terminal.
- Active installed binary: `aopmem 0.2.0-rc4`.
- Expected installed RC4 Windows SHA-256:
  `E4442FD06622A6B94F997E23B67A55753F1D841F6570EF20AC72B99083A6CC1C`.
- Existing workspaces:
  `p-sit-cat-rental-8ef3bf83` and `p-sit-warranty-5708363a`.
- Many old backups exist. Preserve every one.
- Proxy: `<PROXY_URI>`.
- Direct GitHub access is unavailable.

Do not put proxy credentials in the URI, shell history, environment dump,
logs, evidence, or report. Use `-ProxyUseDefaultCredentials` only for
integrated proxy authentication.

## Immutable RC7 inputs

```text
Release base:
https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc7

Windows asset:
https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc7/aopmem-windows-x86_64.exe

Checksum manifest:
https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc7/SHA256SUMS

Installer:
https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc7/install/v0.2/install.ps1

Windows SHA-256:
9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1

SHA256SUMS SHA-256:
89e59fd7eceed6048d1ef0367bd4cccc32cc40ab692713e4224e60c78b36e0bc

Installer SHA-256:
c306d664664852b4f60bf834fa2f5d798312e8646ef9921eae9d14007bd5c949
```

## Mandatory rules

1. Read all applicable global and repository `AGENTS.md` rules before any
   action.
2. Work only in the selected target repository and user-level
   `%USERPROFILE%\.aopmem`.
3. Never create repository-local `.aopmem`.
4. Never query or edit SQLite directly. Never delete WAL, SHM, journal,
   recovery, rollback, or backup files.
5. Never copy, move, replace, or restore `aopmem.exe` manually.
6. Never rerun `upgrade apply`; the official installer may attempt it once.
7. Do not terminate unknown processes automatically.
8. Keep all stdout, stderr, JSON, hashes, paths, and timestamps. Redact proxy
   URI, credentials, tokens, cookies, authorization, and test secrets.
9. Do not launch another installer, helper, browser downloader, Python
   downloader, BITS, or curl.

## Evidence root and preflight

1. Create a new timestamped, unique root below `%TEMP%`. Record its full path.
   Confirm it is outside the repository and is not a reparse point.
2. Record Windows edition/build/architecture, `$PSVersionTable`, current user,
   and non-admin status. If the fixed environment differs, stop before update.
3. Read and record the target repository Git branch and `git status --short`.
   Preserve every pre-existing change and every path outside managed adapter
   blocks.
4. Close AOPMem UI and CLI processes. Recheck live processes. Do not reuse old
   PID values.
5. Record:
   - installed RC4 path, exact `--version`, size, and SHA-256;
   - both expected workspace directory names and all workspace directory names;
   - schema only through supported AOPMem commands, never direct SQLite;
   - all existing backup/recovery roots and contents;
   - absence of repository-local `.aopmem`;
   - active adapter pair and instruction file.
6. Require the installed RC4 SHA-256 above. On mismatch, preserve evidence and
   stop; do not relabel the hash.

## Independent durable backup

Before downloading RC7:

1. Create a new timestamped durable full-home backup under
   `%LOCALAPPDATA%\AOPMemBackups`.
2. Copy `%USERPROFILE%\.aopmem` without following reparse points.
3. Create a deterministic SHA-256 manifest.
4. Verify the copied old binary hash, both workspace directories, database
   files, tool/artifact files, and readable backup content.
5. Record the backup path and manifest hash. Never delete or reuse it.

The official installer must also create and retain its own sibling
`aopmem-home-backup-v0.2.0-rc7-*` backup.

## Proxy-safe installer bootstrap

Run in the current native PowerShell 5.1 console:

```powershell
$proxyUri = [Uri]"<PROXY_URI>"
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) `
    ("aopmem-rc7-acceptance-" + [DateTime]::UtcNow.ToString("yyyyMMddTHHmmssZ") + `
    "-" + [Guid]::NewGuid().ToString("N"))
$null = New-Item -ItemType Directory -Path $tempRoot -ErrorAction Stop
$installer = Join-Path $tempRoot "install.ps1"

Invoke-WebRequest `
    -Uri "https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc7/install/v0.2/install.ps1" `
    -OutFile $installer `
    -UseBasicParsing `
    -Proxy $proxyUri `
    -ProxyUseDefaultCredentials `
    -TimeoutSec 900 `
    -ErrorAction Stop

$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($installerHash -ine "c306d664664852b4f60bf834fa2f5d798312e8646ef9921eae9d14007bd5c949") {
    throw "INSTALLER_SHA256_MISMATCH"
}
```

Normal `Invoke-WebRequest` redirects are required for bootstrap. Do not use
`-MaximumRedirection 0`. If integrated proxy credentials are not required,
omit `-ProxyUseDefaultCredentials` from both bootstrap and installer command.

If the published installer hash is unavailable or still contains a marker,
return `RELEASE_NOT_READY`. Do not execute an unverified installer.

## Official update

Set the exact active adapter pair. For Codex:

```powershell
$env:AOPMEM_ACTIVE_ADAPTER = "codex"
$env:AOPMEM_ACTIVE_INSTRUCTION_FILE = "AGENTS.md"
```

Use another documented exact pair only when it is the actual active adapter.
Run only the official installer in the same console:

```powershell
& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -NoProfile `
    -ExecutionPolicy Bypass `
    -File $installer `
    -AssetBaseUri "https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc7" `
    -ProxyUri $proxyUri `
    -ProxyUseDefaultCredentials
```

Capture exit code, stdout, stderr, and all generated JSON. Do not run a second
apply or manual publication.

## Required update proof

Require all:

1. Proxy redirect download succeeds for `SHA256SUMS` and the Windows asset.
2. No `PropertyNotFoundException` appears.
3. No original network error is hidden or replaced. If transport fails, its
   exact original exception type/message remains visible and apply attempts
   equal zero.
4. Downloaded manifest hash equals
   `89e59fd7eceed6048d1ef0367bd4cccc32cc40ab692713e4224e60c78b36e0bc`.
5. Downloaded Windows asset hash equals
   `9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1`
   and manifest.
6. Staged binary reports exactly `aopmem 0.2.0-rc7`.
7. Staged `platform check --json`: exit `0`, `ok=true`, `status=pass`, all
   native create/flush/no-replace/replace/reopen/reparse/cleanup primitives
   pass.
8. Staged `audit repair --all-workspaces --json`: `ok=true`; clean no-op is
   allowed.
9. Staged `upgrade prepare --all-workspaces --json`: exit `0`, `ok=true`,
   `success=true`.
10. No AOPMem DB-read command runs between prepare and plan.
11. Staged `upgrade plan --all-workspaces --json`: `ok=true`, `ready=true`,
    `writes_performed=false`.
12. `upgrade apply --all-workspaces --json --approved "+++"` has exactly one
    attempt, exit `0`, and `ok=true`. Never retry it.
13. Native `upgrade publish --json` succeeds after apply only.
14. Installed binary reports `aopmem 0.2.0-rc7`, has
    `9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1`,
    and is the published RC7 asset.
15. Both expected workspace keys and all old workspace directories remain.
16. Every workspace is at operational schema
    `004_task_protocol_and_tool_aliases`; no migration `005`.
17. Independent backup, installer full-home backup, migration backups,
    recovery journal/material, rollback homes, and every old backup remain.

## Product and preservation proof

In an approved test workspace:

1. Require Managed Block V2 and selected adapter status in sync. Confirm only
   the managed block changed; preserve all text outside it byte-for-byte.
2. Run doctor and verify. Require `ok=true`, `healthy=true`, and `clean=true`.
3. Complete `task start`, `task apply`, and `task complete` using returned
   canonical IDs. Require bundle id, memory revision, mandatory context, and
   valid receipts.
4. Use only an approved non-production secrets canary. Prove raw canary absent
   from stdout, stderr, logs, JSON, observability, and exported debug capsule.
5. Run `aopmem tool dedupe plan --json`. If the approved exact-duplicate
   fixture exists, run only `aopmem tool dedupe apply --exact-only --json`.
   Prove canonical/alias resolution and no tool-directory deletion.
6. Run `observe status --json`, `observe report --json`, and `observe export`
   to a new capsule. Require one stable non-empty current workspace key and
   successful redacted export.
7. Run debug capsule export to a new file. Preserve it as redacted evidence.
8. Run `aopmem ui --no-open --port 0`, inspect the printed loopback read-only
   UI manually, then stop the foreground process. Do not leave UI running.
9. Recheck absence of repository-local `.aopmem`.
10. Recheck installed version/hash, both workspaces, schema `004`, every old
    backup, and new backups.
11. Recheck repository Git status. It must match the captured preflight state
    except the explicitly selected managed adapter block. Every outside-block
    byte and every pre-existing untracked file must remain preserved.

## Failure boundary

Before apply, require old installed binary byte-for-byte unchanged, zero apply
attempts, all data unchanged, and all backups retained.

After apply starts, never restore or republish RC4, never retry apply, and
never delete recovery evidence. Preserve exact stopped workspace/error,
journal, retained staged binary, home backups, workspace backups, stdout,
stderr, JSON, and hashes. Continue only through separately reviewed recovery.

## Final report

Return a compact evidence table containing host, PowerShell version, proxy
configured yes/no without URI, installer hash, release hashes, old/new binary
hashes, workspace keys, schema, backup paths, redirect count/facts, apply
attempt count, publish status, adapter, doctor, verify, task protocol,
redaction, dedupe, observability, debug capsule, UI, repository `.aopmem`, and
Git outside-block preservation.

Return exactly one final result enum:

- `RC7_ACCEPTED` — every requirement passed; apply attempts exactly one; data,
  backups, and Git content preserved.
- `RC8_REQUIRED` — RC7 product/installer defect, unexpected mutation, masked
  error, failed platform/update/product proof, or wrong final version/hash.
- `ENVIRONMENT_BLOCKED` — fixed host, proxy access, permissions, or required
  native environment prevented execution before any product conclusion.
- `RELEASE_NOT_READY` — immutable RC7 installer/release/assets/hashes are
  missing, unpublished, unresolved, or inconsistent.

Do not return another enum. Do not claim `RC7_ACCEPTED` from partial evidence.
