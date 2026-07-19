# Official upgrade to AOPMem v0.2.0-rc7

`v0.2.0-rc7` is a narrow Windows installer release. It adds explicit and
environment proxy support, replaces exception-driven redirect parsing, and
fixes known-source classification. All RC6 runtime features remain unchanged.
The operational schema remains `004_task_protocol_and_tool_aliases`; no
migration `005` exists.

Supported sources are SQLite-backed `v0.1.0-rc3`, compatible local v0.1, and
`v0.2.0-rc1` through `v0.2.0-rc6`. Exact published platform hashes for those
versions are canonical. Another compatible RC1-RC6 hash reports
`NONCANONICAL_SOURCE_BINARY`; another compatible v0.1 hash reports
`NONCANONICAL_V010_BINARY`. The actual version and hash remain visible.
A hash warning alone does not block the update. Staged preparation and plan
decide compatibility.

## Audited RC7 assets

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `aopmem-darwin-arm64` | `9747720` | `8998c88efaa59a9abc4d4ddce01adf67f4b1a47361b01b483053ebe0e3841786` |
| `aopmem-windows-x86_64.exe` | `10571776` | `9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1` |
| `SHA256SUMS` | `178` | `89e59fd7eceed6048d1ef0367bd4cccc32cc40ab692713e4224e60c78b36e0bc` |
| `install.ps1` | `68822` | `c306d664664852b4f60bf834fa2f5d798312e8646ef9921eae9d14007bd5c949` |

Verify the selected binary against the downloaded `SHA256SUMS` before any
execution. Download immutable Windows installer source only from:

```text
https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc7/install/v0.2/install.ps1
```

## Windows proxy transport

Use [Windows proxy install](WINDOWS_PROXY_INSTALL.md) when GitHub requires a
proxy. The installer accepts:

```powershell
-ProxyUri <PROXY_URI>
-ProxyUseDefaultCredentials
```

The credential switch is optional. It applies integrated credentials only to
the selected proxy, never to GitHub. Proxy URIs with userinfo are rejected.
Direct installation remains valid when no proxy is configured.

The Windows PowerShell 5.1 transport uses `System.Net.Http.HttpClient`.
Automatic redirects are disabled. Statuses 301, 302, 303, 307, and 308 are
handled as normal responses; each target is HTTPS-only, userinfo-free, and
bounded by loop and 10-hop checks. Final HTTP 200 bodies stream to private
create-new partial files. Existing destinations remain unchanged.

Network exceptions retain their original type and message. The installer does
not read an absent `Exception.Response` property. A secondary
`PropertyNotFoundException` cannot mask the original transport failure.

## Required order

1. Close every AOPMem UI and CLI process. Do not terminate unknown processes.
2. Create and verify the durable sibling full-home backup and
   `MANIFEST.sha256`. Keep every old backup and workspace.
3. Download RC7 through the canonical transport. Verify manifest, binary
   SHA-256, and exact `aopmem 0.2.0-rc7` version.
4. Adopt the unchanged backup and retain the verified artifact through
   `upgrade backup --adopt` and `upgrade stage`.
5. Run staged `platform check --json`. Stop before data changes on failure.
6. Run staged `audit repair --all-workspaces --json`; `already_clean` is valid.
7. Run staged `upgrade prepare --all-workspaces --json`, then immediately
   staged `upgrade plan --all-workspaces --json`.
8. Require plan `ok=true`, `ready=true`, and `writes_performed=false`.
   Run no AOPMem database read between prepare and plan.
9. Run staged `upgrade apply --all-workspaces --json --approved "+++"`
   exactly once. Never retry automatically.
10. After successful apply only, run native `upgrade publish --json`.
11. Sync exactly one explicitly selected adapter.
12. Run post-publish audit repair, doctor, verify, task protocol,
    observability, and debug capsule export.

Do not copy, move, or replace the installed binary manually. Do not use
administrator rights, WSL, Cargo, Rustup, source builds, manual SQLite, or
manual WAL/SHM cleanup.

## Failure boundary

Before apply, the installed binary stays byte-for-byte unchanged. Keep all
backups and transport evidence. A proxy or download failure has zero apply
attempts.

After apply starts, never restore the old binary over migrated data and never
retry apply automatically. Keep the journal, staged binary, home backup,
workspace backups, JSON output, and exact stopped workspace/error. Continue
only through a separately reviewed native recovery action.

## Proof boundary

macOS checks can prove tests, installer structure, binary formats, and hashes.
They cannot prove native Windows PowerShell 5.1 or corporate proxy behavior.
Native RC7 acceptance remains `PENDING_DOGFOOD` until the standalone Windows
acceptance prompt passes on the approved Windows 11 x64 environment.
