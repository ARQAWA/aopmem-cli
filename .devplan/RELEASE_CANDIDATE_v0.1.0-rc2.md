# AOPMem v0.1.0-rc2 Release Candidate

## Status

RC ready.

Native Windows runtime proof is pending user VDI run.

## Scope

- macOS ARM artifact:
  `dist/aopmem-darwin-arm64/aopmem`.
- Windows x64 artifact:
  `dist/aopmem-windows-x86_64/aopmem.exe`.
- Install prompt platform selection:
  - macOS Apple Silicon uses `dist/aopmem-darwin-arm64/aopmem`.
  - Windows x64 native PowerShell uses
    `dist\aopmem-windows-x86_64\aopmem.exe`.
- Windows native PowerShell smoke doc:
  `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`.

No runtime behavior changed from rc1.

## Evidence

macOS ARM binary path:

```text
dist/aopmem-darwin-arm64/aopmem
```

Windows x64 binary path:

```text
dist/aopmem-windows-x86_64/aopmem.exe
```

File output:

```text
dist/aopmem-darwin-arm64/aopmem: Mach-O 64-bit executable arm64
dist/aopmem-windows-x86_64/aopmem.exe: PE32+ executable (console) x86-64, for MS Windows
```

SHA-256:

```text
798af720030081367969fb36a2913de98956d700fbdd6e87ae176d4e05caaefc  dist/aopmem-darwin-arm64/aopmem
d7d11a863c65877a31a203626764e3aaa2cc58c1403fbb37d6c1d22cdb17db0e  dist/aopmem-windows-x86_64/aopmem.exe
```

Cargo build/test results:

```text
PASS rtk cargo build
PASS rtk cargo test: 164 passed
PASS rtk cargo test --tests: 164 passed
PASS git diff --check
```

Drift check:

```text
PASS no .github workflow files found
PASS no Node.js rewrite or npm start runtime found
PASS no WSL primary install path found
PASS no Linux support claim found
PASS src/verify/mod.rs forbidden terms are denylist/test evidence only
```

Install prompt updated:

```text
PASS macOS ARM branch installs from dist/aopmem-darwin-arm64/aopmem
PASS Windows x64 branch installs from dist\aopmem-windows-x86_64\aopmem.exe
PASS Windows target path is %USERPROFILE%\.aopmem\bin\aopmem.exe
PASS Windows commands are PowerShell-native
PASS WSL is explicitly not the Windows install path
PASS matching artifact install does not build from source
```

## Windows Native Proof

- Status: pending user VDI run.
- Required OS/shell: Windows 11 native PowerShell.
- Doc path: `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`.
- Expected checks:
  - `aopmem 0.1.0`;
  - JSON commands return `ok=true`, or documented healthy equivalent;
  - `Test-Path "$Repo\.aopmem"` returns `False`;
  - `$env:AOPMEM_HOME\workspaces\...` exists;
  - `AGENTS.md` contains managed AOPMem block.

## Explicitly Out Of Scope

- Linux.
- Windows ARM.
- WSL.
- Node.js rewrite.
- CI/GitHub Actions.
- New runtime features.

## Release Recommendation

- Ready for Windows VDI smoke: yes.
- Ready to tag rc2 after Windows smoke: yes, if native Windows smoke passes.
