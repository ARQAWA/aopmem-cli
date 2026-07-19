# AOPMem v0.2.0-rc7

- Fixes official Windows PowerShell 5.1 downloads behind corporate proxies.
- Adds explicit `-ProxyUri`, environment/system proxy discovery, and optional
  integrated proxy credentials through `-ProxyUseDefaultCredentials`.
- Replaces exception-driven redirect parsing with bounded manual
  `HttpClient` handling for 301, 302, 303, 307, and 308 responses.
- Preserves original network exception type/message and prevents a missing
  `Exception.Response` property from causing a masking
  `PropertyNotFoundException`.
- Correctly recognizes exact published RC4, RC5, and RC6 source binaries.
  Compatible unknown RC1-RC6 builds now receive the accurate
  `NONCANONICAL_SOURCE_BINARY` warning.
- Preserves all RC6 runtime features and installer ordering. Operational schema
  remains `004_task_protocol_and_tool_aliases`; no migration `005` is added.
- Native Windows 11 / Windows PowerShell 5.1 proxy acceptance remains required
  after publication. macOS-hosted proof is not a native Windows runtime PASS.

## Assets

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `aopmem-darwin-arm64` | `9747720` | `8998c88efaa59a9abc4d4ddce01adf67f4b1a47361b01b483053ebe0e3841786` |
| `aopmem-windows-x86_64.exe` | `10571776` | `9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1` |
| `SHA256SUMS` | `178` | `89e59fd7eceed6048d1ef0367bd4cccc32cc40ab692713e4224e60c78b36e0bc` |

Verify binary downloads with the included `SHA256SUMS` manifest.
Immutable tagged `install.ps1`: `68822` bytes, SHA-256
`c306d664664852b4f60bf834fa2f5d798312e8646ef9921eae9d14007bd5c949`.
