# RC7 proxy and redirect remediation

Status: `IMPLEMENTED; NATIVE_WINDOWS_ACCEPTANCE_PENDING`

## Root cause

RC6 used `Invoke-WebRequest -MaximumRedirection 0` as production redirect
transport. Behind the field corporate proxy, native Windows PowerShell 5.1
raised `System.InvalidOperationException` with message
`Operation is not valid due to the current state of the object.` The exception
had no `Response` property. Strict-mode access to
`$_.Exception.Response` then raised `PropertyNotFoundException` and masked the
original network failure.

The failure occurred after durable backups, during asset download, before
staged platform check, prepare, plan, apply, publication, or adapter sync.
Apply attempts were zero. User data and the installed RC4 binary were
unchanged. The RC6 binary itself passed native `platform check --json`; the
blocker was only official installer transport.

## Remediation

`install/v0.2/install.ps1` now exposes:

```powershell
[Uri]$ProxyUri
[switch]$ProxyUseDefaultCredentials
```

One immutable proxy configuration is resolved in this order:

1. explicit `-ProxyUri`;
2. `HTTPS_PROXY`;
3. `https_proxy`;
4. `HTTP_PROXY`;
5. `http_proxy`;
6. usable system default proxy;
7. direct.

Proxy URIs must be absolute `http` or `https` URIs with no userinfo, query, or
fragment. Default credentials are opt-in and assigned only to the proxy
object. The installer never accepts proxy username/password parameters and
never logs the raw proxy URI or credentials.

The canonical transport uses PowerShell 5.1-compatible
`System.Net.Http.HttpClientHandler` and `HttpClient` with:

- `AllowAutoRedirect=false`;
- `UseCookies=false`;
- `HttpCompletionOption.ResponseHeadersRead`;
- TLS 1.2;
- no certificate-validation bypass;
- no target credentials.

Statuses 301, 302, 303, 307, and 308 are normal redirect responses. Each
`Location` is resolved, HTTPS-validated, checked for userinfo and allowed
origin/path transition, and recorded in a visited set. Loops and more than
10 hops fail closed. The selected proxy remains unchanged across all hops.

Only final HTTP 200 is accepted. The response streams to a create-new partial
file in the destination parent. The installer verifies nonzero and declared
length, flushes, closes handles, then publishes without overwrite. Failure
removes only the owned partial and preserves any existing destination.

## Error preservation

The normal transport never reads `Exception.Response`. Transport failures
carry stable class, sanitized URI host/path, proxy configured/source,
exception type and original message, real HTTP status when present, redirect
hop, destination, and partial-file state. It does not emit credentials,
tokens, cookies, authorization, raw proxy values, or URI userinfo.

Therefore an exception lacking `Response` retains its original
`System.InvalidOperationException` type and message. No secondary
`PropertyNotFoundException` is produced.

## Source classification

Both installers recognize exact published platform hashes for v0.1.0-rc3 and
RC1 through RC6. Exact RC4, RC5, and RC6 binaries produce no warning.
Compatible unknown RC1-RC6 hashes produce
`NONCANONICAL_SOURCE_BINARY`; compatible unknown v0.1 hashes retain
`NONCANONICAL_V010_BINARY`. Actual version/hash remains visible. A hash warning
does not change compatibility planning or apply count.

## Preserved contracts

Installer order and apply-once behavior remain unchanged. All RC6 runtime
features remain. Operational schema remains
`004_task_protocol_and_tool_aliases`; no migration `005` exists.

Static and deterministic transport proof does not establish native Windows
PowerShell 5.1 proxy runtime. That result remains
`NATIVE_WINDOWS_ACCEPTANCE_PENDING`.
