# RC7 source classification report

Status: `FOCUSED_TESTS_PASS`

## Defect

RC6 accepted versions through RC5 but assigned an expected hash only to
`aopmem 0.1.0`. Every RC1–RC5 source therefore emitted the inaccurate
`NONCANONICAL_V010_BINARY` warning, including the exact published RC4 Windows
binary.

## Authoritative published matrix

Hashes were derived from the immutable tagged assets and their release
manifests.

| Version | macOS arm64 | Windows x86-64 |
| --- | --- | --- |
| `0.1.0` (`v0.1.0-rc3`) | `d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607` | `01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a` |
| `0.2.0-rc1` | `b32e918d2a44f0767444e09c84c1ed44fe9177709b2d56b2aa89c300081d4308` | `a4e3302d6f26dd9d16387a075189fec51c469aef9b8d9c730f81001b21b2cf57` |
| `0.2.0-rc2` | `d4a3f52aeddd3fd656f46358305a5a4a688868b3f25d4675eb37f6cf223a81d4` | `77a2e79162c609ff62dbaa4533c5f7237490c842047485fe79a608a14f57a5f8` |
| `0.2.0-rc3` | `8bc4d3a7ae38253c1a6e4c653292cf954fb2c8eee916c69a03c6dc5e2484261c` | `ed59be73d99efd2c1a4fe99e50b85e8b6ce8e8a73b7ff0c96b5327e1c2d39477` |
| `0.2.0-rc4` | `4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e` | `e4442fd06622a6b94f997e23b67a55753f1d841f6570ef20ac72b99083a6cc1c` |
| `0.2.0-rc5` | `594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e` | `150db4699c2f41c6e529f9606ac099c9ac6b4771b5084952f2cb5df3226d1b58` |
| `0.2.0-rc6` | `b933d921ae6ec68ce7e0f118de27fd7eabe9d1c42d715a0a6df8f2ec731cb949` | `8cd03fd00ffdaf505d7f31cd1c485fd15179823f84a78061b7bcfc00ee4fd4c7` |

## RC7 behavior

- Exact version/platform hash: no warning.
- Unknown compatible RC1–RC6 hash:
  `NONCANONICAL_SOURCE_BINARY`, with actual version and SHA-256.
- Unknown compatible `0.1.0` hash:
  `NONCANONICAL_V010_BINARY`, with actual version and SHA-256.
- A hash mismatch alone does not block. Staged prepare and plan still decide
  compatibility.
- Unsupported versions remain blocked before download or apply.
- Warning selection does not change ordering or apply count.

Both official installers contain their platform-specific matrix. RC7 accepts
source versions through RC6.

## Proof

```text
sh -n install/v0.2/install.sh
  PASS
sh -n scripts/audit_v020_installers.sh
  PASS
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test_windows_installer_transport.py
  PASS; 30/30
rtk scripts/audit_v020_installers.sh
  PASS; 14 groups
git diff --check
  PASS
```

The installer audit verifies tagged RC1–RC6 hashes for both platforms and
executes exact published RC4, RC5, and RC6 macOS sources without a
noncanonical warning. A modified compatible RC4 fixture emits only the
generic warning, exposes its actual hash, continues to plan, and invokes apply
exactly once.
