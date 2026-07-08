# AOPMem v0.1.0-rc3 Release Candidate

## Status

RC ready.

Native Windows runtime proof is pending Windows VDI run.

## Scope

- Windows HOME fallback.
- Workspace key canonicalization.
- PowerShell UTF-8 guard.
- Real AGENTS managed block.
- Windows x64 artifact:
  `dist/aopmem-windows-x86_64/aopmem.exe`.
- macOS ARM regression:
  `dist/aopmem-darwin-arm64/aopmem`.

## Evidence

Cargo:

```text
PASS rtk cargo build
PASS rtk cargo test: 178 passed
PASS rtk cargo test --tests: 178 passed
PASS git diff --check
```

macOS ARM binary:

```text
dist/aopmem-darwin-arm64/aopmem:
  Mach-O 64-bit executable arm64
dist/aopmem-darwin-arm64/aopmem --version:
  aopmem 0.1.0
SHA-256:
  d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607
```

Windows x64 binary:

```text
dist/aopmem-windows-x86_64/aopmem.exe:
  PE32+ executable (console) x86-64, for MS Windows
SHA-256:
  01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a
```

Docs:

```text
PASS install prompt updated:
  install/v0.1/install_prompt.md
PASS smoke doc updated:
  docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md
```

## Windows Native Proof

- Status: pending VDI run.
- Doc path: `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`.
- Required shell: Windows 11 native PowerShell.
- Required artifact:
  `dist\aopmem-windows-x86_64\aopmem.exe`.

## Out Of Scope

- Node.js rewrite.
- WSL.
- Linux.
- Windows ARM.
- CI/GitHub Actions.
- New runtime features.
- Mem0, Hindsight, vector search, semantic search, embeddings, Qdrant, or
  custom MCP server.

## Release Recommendation

- Ready for Windows VDI smoke: yes.
- Ready to tag final after Windows smoke: yes, if native Windows smoke passes.
