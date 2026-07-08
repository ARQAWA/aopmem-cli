# Windows RC3 Mac Audit Report

Verdict: `CONDITIONAL_PASS`

Reason: all Mac-side checks passed. Native Windows PowerShell smoke is still
pending VDI run.

## Scope

Read:

- `.devplan/WINDOWS_FIRST_INSTALL_REMEDIATION.md`
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc3.md`
- `.devplan/PROOF_LOG.md`
- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`
- `install/v0.1/install_prompt.md`

## Checks

| # | Check | Result | Evidence |
|---|---|---|---|
| 1 | Windows HOME fallback | PASS | `AOPMEM_HOME` wins; Windows fallback uses `USERPROFILE\.aopmem`; tests cover missing `HOME` and missing `USERPROFILE`. |
| 2 | Workspace key canonicalization | PASS | `init`, `doctor`, `recall` use same canonical git-root key in temp smoke: `repo-6a7725a9`. |
| 3 | Unicode | PASS | Cyrillic persisted; `????` rejected with `SUSPICIOUS_MOJIBAKE_INPUT`; invalid UTF-8 path exists. |
| 4 | AGENTS block | PASS | Managed block contains real AOPMem contract: `aopmem recall`, `tool run`, `Memory Keeper`, `+++`. |
| 5 | No repo `.aopmem` | PASS | Repo scan found no `.aopmem`; temp smoke also kept repo clean. |
| 6 | Windows exe exists | PASS | `dist/aopmem-windows-x86_64/aopmem.exe` present. |
| 7 | Windows exe type | PASS | `PE32+ executable (console) x86-64, for MS Windows`. |
| 8 | macOS binary works | PASS | `dist/aopmem-darwin-arm64/aopmem --version` => `aopmem 0.1.0`. |
| 9 | cargo build/test | PASS | `rtk cargo build`, `rtk cargo test`, `rtk cargo test --tests` passed. |
| 10 | Forbidden drift | PASS | Hits are out-of-scope docs or verifier deny-list/tests only; no product drift found. |

## Command Results

Required commands:

- `rtk cargo build`: PASS
- `rtk cargo test`: PASS, 178 passed
- `rtk cargo test --tests`: PASS, 178 passed
- `git diff --check`: PASS
- `file dist/aopmem-darwin-arm64/aopmem`: Mach-O 64-bit executable arm64
- `file dist/aopmem-windows-x86_64/aopmem.exe`: PE32+ executable x86-64
- `shasum -a 256 dist/aopmem-darwin-arm64/aopmem`:
  `d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607`
- `shasum -a 256 dist/aopmem-windows-x86_64/aopmem.exe`:
  `01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a`

Extra audit smoke:

- macOS dist binary `init + adapter seed/status + doctor + recall`: PASS
- same workspace key across `init/doctor/recall`: PASS
- Cyrillic stdin preserved: PASS
- `????` rejected: PASS
- no repo `.aopmem`: PASS

## Notes

The command log includes two failed audit-harness attempts:

- relative log path after `cd`;
- AGENTS assertion before running `adapter seed`.

Both were auditor harness mistakes, not product failures. The corrected smoke
passed.

## Condition

Before final release, run the documented native Windows 11 PowerShell smoke in
`docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`.
