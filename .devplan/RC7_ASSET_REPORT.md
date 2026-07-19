# RC7 release asset report

Status: `VERIFIED`

## Build

```text
scripts/build_macos_arm.sh
  PASS
scripts/build_windows_x64_from_macos.sh
  PASS; build 1
scripts/build_windows_x64_from_macos.sh
  PASS; build 2, unchanged source
```

Both Windows builds produced:

```text
9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1
```

## Audited files

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `dist/aopmem-darwin-arm64` | `9747720` | `8998c88efaa59a9abc4d4ddce01adf67f4b1a47361b01b483053ebe0e3841786` |
| `dist/aopmem-windows-x86_64.exe` | `10571776` | `9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1` |
| `dist/SHA256SUMS` | `178` | `89e59fd7eceed6048d1ef0367bd4cccc32cc40ab692713e4224e60c78b36e0bc` |
| `install/v0.2/install.ps1` | `68822` | `c306d664664852b4f60bf834fa2f5d798312e8646ef9921eae9d14007bd5c949` |

## Verification

- macOS asset: Mach-O 64-bit arm64, minimum macOS 11.0;
- macOS version: `aopmem 0.2.0-rc7`;
- Windows asset: PE32+ console executable, x86-64;
- Windows imports contain no dynamic `VCRUNTIME`, `MSVCP`, `UCRTBASE`, or
  `api-ms-win-crt` dependency;
- `shasum -a 256 -c dist/SHA256SUMS`: `PASS`;
- release documents contain no unresolved RC7 hash or byte markers.

Native Windows executable and PowerShell 5.1 runtime remain unproven.
