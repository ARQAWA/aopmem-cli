# Windows x64 Build Proof

Date: 2026-06-28

## Scope

Add Windows x64 native PowerShell build/install support for AOPMem
v0.1.0-rc2.

## Target

- OS/runtime target: Windows 11 native PowerShell
- CPU/OS architecture: x64
- Rust target: `x86_64-pc-windows-msvc`
- Output: `dist/aopmem-windows-x86_64/aopmem.exe`

## Files

- `scripts/build_windows_x64_from_macos.sh`
- `dist/aopmem-windows-x86_64/aopmem.exe`
- `install/v0.1/install_prompt.md`
- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`

## Build Environment

Native `cargo xwin --version` did not return a usable command in this Mac
environment.

Docker fallback was available:

```text
Docker version 29.5.3, build d1c06ef
```

The build script used Docker image:

```text
messense/cargo-xwin
```

## Commands

```text
bash scripts/build_windows_x64_from_macos.sh
file dist/aopmem-windows-x86_64/aopmem.exe
shasum -a 256 dist/aopmem-windows-x86_64/aopmem.exe
bash -n scripts/build_windows_x64_from_macos.sh
git diff --check
rtk cargo test
```

## Results

```text
PASS Windows x64 build script completed
PASS dist/aopmem-windows-x86_64/aopmem.exe exists
PASS artifact is non-empty
PASS file reports PE32+ executable
PASS file reports x86-64
PASS bash syntax check
PASS git diff --check
PASS cargo test: 164 passed
```

`file` output:

```text
dist/aopmem-windows-x86_64/aopmem.exe: PE32+ executable (console) x86-64, for MS Windows
```

Size:

```text
4193792 bytes
```

SHA-256:

```text
d7d11a863c65877a31a203626764e3aaa2cc58c1403fbb37d6c1d22cdb17db0e
```

## Runtime Proof

Not run on Mac. No Wine run was attempted.

Native Windows runtime proof belongs to Windows VDI and is documented in:

```text
docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md
```

## Scope Guard

- No `src/**` changes.
- No `tests/**` changes.
- No Linux support added.
- No Windows ARM support added.
- No WSL install path added.
- No CI/GitHub Actions added.
- No runtime behavior changed.
