# Windows RC2 Mac Audit Report

## Verdict

CONDITIONAL_PASS

## Summary

Mac-side binary evidence passed for both artifacts.

Windows x64 artifact is present, PE32+, x86-64, and has SHA-256 proof.
Build script and install prompt match the rc2 target.

Full PASS is blocked by required documentation gaps:

- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md` does not check `init`, `doctor`,
  or `recall`.
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md` does not include full macOS ARM
  proof and does not explicitly say no runtime behavior changed.

Windows runtime proof was not run on Mac.

## Commands Run

[.devplan/WINDOWS_RC2_MAC_AUDIT_COMMANDS.log](.devplan/WINDOWS_RC2_MAC_AUDIT_COMMANDS.log)

## macOS ARM Artifact

PASS

Evidence:

- `dist/aopmem-darwin-arm64/aopmem` exists.
- `file dist/aopmem-darwin-arm64/aopmem`:
  `Mach-O 64-bit executable arm64`.
- `dist/aopmem-darwin-arm64/aopmem --version`:
  `aopmem 0.1.0`.
- SHA-256:
  `798af720030081367969fb36a2913de98956d700fbdd6e87ae176d4e05caaefc`.

## Windows x64 Artifact

PASS

Evidence:

- `dist/aopmem-windows-x86_64/aopmem.exe` exists.
- `file dist/aopmem-windows-x86_64/aopmem.exe`:
  `PE32+ executable (console) x86-64, for MS Windows`.
- SHA-256:
  `d7d11a863c65877a31a203626764e3aaa2cc58c1403fbb37d6c1d22cdb17db0e`.

## Build Script

PASS

Evidence:

- `scripts/build_windows_x64_from_macos.sh` exists.
- Script is executable and passes `bash -n`.
- Target is exactly `x86_64-pc-windows-msvc`.
- Output is exactly `dist/aopmem-windows-x86_64/aopmem.exe`.
- Uses native `cargo xwin` or Docker image `messense/cargo-xwin`.
- Has fail-fast behavior via `set -euo pipefail` and explicit missing artifact
  checks.
- No WSL or Windows-native build requirement found in the script.

## Install Prompt

PASS

Evidence:

- macOS ARM branch installs from `dist/aopmem-darwin-arm64/aopmem`.
- Windows x64 branch installs from
  `dist\aopmem-windows-x86_64\aopmem.exe`.
- Windows target path is `%USERPROFILE%\.aopmem\bin\aopmem.exe`.
- Windows commands are PowerShell-native.
- WSL is explicitly not the Windows install path.
- Unsupported platforms are listed.
- Prompt says not to build from source when matching artifact exists.

## Windows PowerShell Smoke Doc

FAIL

Evidence:

- File exists: `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`.
- Uses native PowerShell.
- Says no WSL.
- Uses `$env:USERPROFILE`.
- Uses `$env:AOPMEM_HOME`.
- Checks `--version` and `--help`.

Missing required checks:

- No `init` check.
- No `doctor` check.
- No `recall` check.
- No explicit command that confirms `.aopmem` is absent inside target repo.

## Cargo Checks

PASS

Evidence:

- `rtk cargo build`: PASS.
- `rtk cargo test`: PASS, 164 passed.
- `rtk cargo test --tests`: PASS, 164 passed.
- `git diff --check`: PASS.

## Drift Check

PASS

Evidence:

- No `.github` workflow files found.
- No Node.js rewrite or `npm start` runtime found.
- No WSL primary install path found.
- No Linux support claim found; Linux is only listed as unsupported.
- Forbidden terms found in docs as out-of-scope/history.
- `src/verify/mod.rs` contains forbidden terms only as lint denylist and test
  fixture, not as feature implementation.

## Findings

- P2 required rc2 issue:
  `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md` is incomplete. It lacks `init`,
  `doctor`, `recall`, and explicit no-`.aopmem` verification.
- P2 required rc2 issue:
  `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md` lacks full macOS ARM proof and
  does not explicitly state that no runtime behavior changed.
- P3 minor:
  rc2 status says artifact is ready for Windows VDI smoke, but does not use a
  plain `RC ready` / `RC not ready` status phrase.
- INFO:
  Drift terms in `src/verify/mod.rs` are denylist/test evidence only.
- INFO:
  `git status` after checks showed modified `.devplan/PROOF_LOG.md` and
  `install/v0.1/install_prompt.md`. This audit did not edit them.

## Final Recommendation

- rc2 Mac-side audit passed: no.
- ready for Windows VDI native smoke: no.
- exact next command/doc for Windows user:
  update `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`, then run that doc from
  Windows 11 native PowerShell in the repo root.
