# Windows First Install Remediation rc3

## Status

PASS for Mac-side remediation and artifact proof.

Windows native smoke is pending VDI run.

## Bugs Fixed

- Windows HOME fallback:
  - `AOPMEM_HOME` wins when set and non-empty.
  - Windows fallback uses `USERPROFILE\.aopmem`.
  - Windows no longer requires `HOME`.
  - Missing Windows `USERPROFILE` without `AOPMEM_HOME` returns structured
    path error.
- Workspace key mismatch:
  - CLI commands use one shared workspace root resolver.
  - Git root is preferred when current directory is inside a git repo.
  - Paths are canonicalized before command workspace key use.
  - Windows hash input normalizes `\` to `/`, strips `\\?\`, removes trailing
    slash, and normalizes drive letter case.
- PowerShell Unicode onboarding:
  - init stdin is decoded as UTF-8.
  - invalid UTF-8 fails with `INVALID_UTF8_INPUT`.
  - mojibake-like semantic answers fail with
    `SUSPICIOUS_MOJIBAKE_INPUT`.
  - semantic `????` is not stored as `project_profile`.
- AGENTS managed block:
  - managed block now contains the real AOPMem operational contract.
  - sync updates only the managed block and preserves user content.
- Windows native smoke:
  - doc now uses native PowerShell only.
  - doc includes UTF-8 preamble and rc3 temp proof.

## Files Changed

- `src/storage/mod.rs`
- `src/install/mod.rs`
- `src/cli/mod.rs`
- `src/adapter/mod.rs`
- `templates/managed-block/AGENTS.managed-block.md`
- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`
- `install/v0.1/install_prompt.md`
- `dist/aopmem-darwin-arm64/aopmem`
- `dist/aopmem-windows-x86_64/aopmem.exe`
- `.devplan/WINDOWS_FIRST_INSTALL_REMEDIATION.md`
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc3.md`
- `.devplan/PROOF_LOG.md`

## Tests Run

```text
PASS rtk cargo build
PASS rtk cargo test: 178 passed
PASS rtk cargo test --tests: 178 passed
PASS git diff --check
```

## macOS Proof

```text
PASS bash scripts/build_macos_arm.sh
PASS file dist/aopmem-darwin-arm64/aopmem:
  Mach-O 64-bit executable arm64
PASS dist/aopmem-darwin-arm64/aopmem --version:
  aopmem 0.1.0
PASS shasum -a 256 dist/aopmem-darwin-arm64/aopmem:
  d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607
```

## Windows Artifact Proof

```text
PASS bash scripts/build_windows_x64_from_macos.sh
PASS file dist/aopmem-windows-x86_64/aopmem.exe:
  PE32+ executable (console) x86-64, for MS Windows
PASS shasum -a 256 dist/aopmem-windows-x86_64/aopmem.exe:
  01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a
```

## Windows Native Smoke

Status: pending VDI run.

Doc path:

```text
docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md
```

Exact VDI step:

```powershell
Set-Location <AOPMem repo root>
Get-Content .\docs\WINDOWS_NATIVE_POWERSHELL_SMOKE.md
```

Then run the commands in that doc from Windows 11 native PowerShell.

Expected:

- `aopmem 0.1.0`
- JSON `ok=true`, or healthy equivalent
- `Test-Path "$Repo\.aopmem"` returns `False`
- `$env:AOPMEM_HOME\workspaces\...` exists
- `AGENTS.md` contains real managed AOPMem block
- Russian text is not stored as `????`
