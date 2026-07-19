# RC6 asset report

## Result

Release asset build and supply-chain checks: `PASS`.

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `aopmem-darwin-arm64` | 9747560 | `b933d921ae6ec68ce7e0f118de27fd7eabe9d1c42d715a0a6df8f2ec731cb949` |
| `aopmem-windows-x86_64.exe` | 10572288 | `8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7` |
| `SHA256SUMS` | 178 | `e4e7142e30cb6ef4cac2c7402b8ace8b87fc37df87add59ccb8d79d15d0f3dba` |

## Build and verification

- `scripts/build_macos_arm.sh` built the arm64 macOS artifact.
- `scripts/build_windows_x64_from_macos.sh` built the x64 Windows artifact
  twice without source changes between builds.
- Both Windows builds produced SHA-256
  `8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7`.
- macOS artifact: `Mach-O 64-bit executable arm64`, minimum macOS `11.0`,
  and `aopmem 0.2.0-rc6` from `--version`.
- Windows artifact: `PE32+ executable (console) x86-64, for MS Windows`.
- Windows imports contain no `VCRUNTIME`, `MSVCP`, `UCRTBASE`, or
  `api-ms-win-crt` DLL. Static MSVC CRT build is confirmed by the import scan.
- `shasum -a 256 -c dist/SHA256SUMS` passed for both binary assets.

## Boundary

The macOS host proves artifact construction, type, version, hashes, manifest,
and Windows PE imports. Native Windows PowerShell 5.1 execution remains
`PENDING_DOGFOOD` and is required by the RC6 acceptance prompt.
