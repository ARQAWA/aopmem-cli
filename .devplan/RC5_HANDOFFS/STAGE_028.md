# RC5 Stage 028 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next action: `STAGE_029`

Verified through: `STAGE_025`

Native Windows runtime: `PENDING_DOGFOOD`

## Result

RC5 now has the frozen flat release asset set:

```text
dist/aopmem-darwin-arm64
dist/aopmem-windows-x86_64.exe
dist/SHA256SUMS
```

Both assets verify against `SHA256SUMS`. The Windows asset is PE32+ console
x86-64 and its two unchanged-source sequential builds produced the same hash.
The Windows update and dogfood instructions now name RC5, use the flat asset
path, and explicitly retain `PENDING_DOGFOOD` runtime status.

## Release integrity

| Item | SHA-256 | Result |
|---|---|---|
| Darwin arm64 asset | `594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e` | PASS |
| Windows x86-64 asset | `150db4699c2f41c6e529f9606ac099c9ac6b4771b5084952f2cb5df3226d1b58` | PASS twice |
| `SHA256SUMS` | `6236d2cf502df5036609f202f541e38a12173321a0a85fbc83e388ed4548213a` | PASS 2/2 |

The Windows import list is limited to system DLLs: `KERNEL32.dll`,
`shell32.dll`, `api-ms-win-core-synch-l1-2-0.dll`, `bcryptprimitives.dll`,
`WS2_32.dll`, `userenv.dll`, `ntdll.dll`, and `advapi32.dll`. No dynamic
`VCRUNTIME`, `MSVCP`, `UCRTBASE`, or `api-ms-win-crt` import exists.

## AUTO_PATCH_WINDOW

The first Windows cross-build found three compile failures hidden by macOS
configuration:

1. `AnchoredDir::verify_logical_identity` called a Unix-only metadata identity
   helper on Windows.
2. Two const strategy selectors used a non-const derived `PartialEq` call.

The minimal adjacent patch changes only:

- `src/audit/anchored.rs`: Windows obtains the logical identity through the
  safe opened-handle `WorkspaceIdentity::capture(...).0`; Unix remains on its
  existing metadata identity path.
- `src/platform_check.rs` and `src/platform_publish.rs`: strategy selection is
  a const-compatible `match (mode, destination_exists)`.

No semantic workaround, metadata ID invention, test change, or benchmark
artifact change was made.

## Documentation and files

- `docs/WINDOWS_NATIVE_UPDATE.md`: RC4 references now RC5; native Windows is
  explicitly `PENDING_DOGFOOD`.
- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`: flat `.exe` asset path, RC5
  fixture naming/version, and dogfood proof boundary.
- `.devplan/RELEASE_CANDIDATE_v0.2.0-rc5.md`: RC5 asset scope, hashes, and
  runtime boundary.
- Stage state, ledger, proof log, and requirements matrix reflect Stage 028.

## Checks

```text
cargo fmt --all -- --check                            PASS
cargo test --locked                                   PASS 768/768
cargo clippy --all-targets --locked -- -D warnings    PASS
./scripts/build_macos_arm.sh                          PASS
./scripts/build_windows_x64_from_macos.sh             PASS #1
./scripts/build_windows_x64_from_macos.sh             PASS #2, same SHA-256
file dist/aopmem-darwin-arm64 dist/aopmem-windows-x86_64.exe
                                                       PASS Mach-O arm64; PE32+ x86-64
llvm-readobj --coff-imports dist/aopmem-windows-x86_64.exe
                                                       PASS system DLLs only
(cd dist && shasum -a 256 -c SHA256SUMS)              PASS 2/2
git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
native Windows runtime                                PENDING_DOGFOOD
```

## Self-review

P1 `0`; P2 `0`.

No commit, push, tag, or GitHub Release was created. No temporary release
asset/cache was retained outside the required `dist` files. Stage 026 benchmark
evidence was not touched.
