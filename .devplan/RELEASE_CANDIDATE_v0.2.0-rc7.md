# AOPMem v0.2.0-rc7 release candidate

Status: `AUDITED_RELEASE_COMMIT`

## Fix

RC6 native Windows binary platform primitives passed. Its official
PowerShell 5.1 installer failed behind a required corporate proxy while
handling asset transport. The original `System.InvalidOperationException`
had no `Response` property; strict-mode access to
`$_.Exception.Response` caused a secondary `PropertyNotFoundException` and
masked the network error.

RC7 adds one proxy-aware `HttpClient` transport. It supports explicit,
environment, system, and direct modes. Optional default credentials are
isolated to the proxy object. Manual redirect handling validates HTTPS targets,
rejects userinfo, loops, and excessive hops, streams bodies, preserves
destinations, and retains the original network exception.

RC7 also fixes source classification. Exact published RC4, RC5, and RC6
platform binaries are recognized. Compatible unknown RC1-RC6 binaries use
`NONCANONICAL_SOURCE_BINARY`, not the inaccurate v0.1 warning.

## Compatibility

- All RC6 product/runtime features remain unchanged.
- Installer ordering and apply exactly once remain unchanged.
- Target schema remains `004_task_protocol_and_tool_aliases`.
- No migration `005` exists.
- Native Windows RC7 proxy acceptance remains `PENDING_DOGFOOD`.

## Proof

- Deterministic proxy/redirect contract cases: 30/30 PASS.
- Full Rust and installer checks: final clean Stage 07 PASS.
- Isolated macOS fresh/RC4/RC5/RC6 update proof: PASS.
- Native RC6 platform check: field PASS.
- Native RC7 Windows PowerShell 5.1 proxy runtime: pending acceptance.
- Open P1/P2: 0/0.

## Assets

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `aopmem-darwin-arm64` | `9747720` | `8998c88efaa59a9abc4d4ddce01adf67f4b1a47361b01b483053ebe0e3841786` |
| `aopmem-windows-x86_64.exe` | `10571776` | `9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1` |
| `SHA256SUMS` | `178` | `89e59fd7eceed6048d1ef0367bd4cccc32cc40ab692713e4224e60c78b36e0bc` |

Immutable tagged `install.ps1`: `68822` bytes, SHA-256
`c306d664664852b4f60bf834fa2f5d798312e8646ef9921eae9d14007bd5c949`.

Next: pass the standalone external-operation approval gate. Then push main,
create and push the annotated tag, publish the GitHub prerelease, and verify
remote assets.
