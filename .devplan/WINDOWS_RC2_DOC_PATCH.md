# Windows RC2 Doc Patch

Date: 2026-06-29

## Verdict

PASS

## Scope

Close Windows rc2 Mac audit documentation findings only.

No runtime behavior changed.

## Files Changed

- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md`
- `.devplan/WINDOWS_RC2_DOC_PATCH.md`
- `.devplan/PROOF_LOG.md`

## Fixes

- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md` now covers native Windows 11
  PowerShell smoke with:
  - existing artifact copy from
    `dist\aopmem-windows-x86_64\aopmem.exe`;
  - install target `$env:USERPROFILE\.aopmem\bin\aopmem.exe`;
  - temp runtime `$env:TEMP\aopmem-rc2-home`;
  - temp repo `$env:TEMP\aopmem-rc2-repo`;
  - `--version`;
  - `--help`;
  - `init`;
  - `adapter seed`;
  - `adapter status`;
  - `doctor`;
  - `recall`;
  - JSON `ok=true` checks;
  - `adapter status` healthy equivalent `managed_block=in_sync`;
  - workspace under `AOPMEM_HOME`;
  - `Test-Path "$Repo\.aopmem"` returns `False`;
  - no `.aopmem` inside the target repo;
  - `AGENTS.md` managed block markers.
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md` now uses the required sections:
  - Status;
  - Scope;
  - Evidence;
  - Windows Native Proof;
  - Explicitly Out Of Scope;
  - Release Recommendation.
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md` includes full macOS ARM proof:
  - `dist/aopmem-darwin-arm64/aopmem`;
  - `Mach-O 64-bit executable arm64`;
  - `aopmem 0.1.0`;
  - SHA-256
    `798af720030081367969fb36a2913de98956d700fbdd6e87ae176d4e05caaefc`.
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md` includes Windows x64 proof:
  - `dist/aopmem-windows-x86_64/aopmem.exe`;
  - `PE32+ executable (console) x86-64, for MS Windows`;
  - SHA-256
    `d7d11a863c65877a31a203626764e3aaa2cc58c1403fbb37d6c1d22cdb17db0e`.
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md` explicitly says no runtime
  behavior changed.

## Scope Guard

- No `src/**` changes.
- No `tests/**` changes.
- No `Cargo.toml` changes.
- No `Cargo.lock` changes.
- No install prompt changes in this patch. Worktree had pre-existing
  `install/v0.1/install_prompt.md` modifications.
- No features added.
- No CI added.
- Native Windows smoke was not run on Mac.
