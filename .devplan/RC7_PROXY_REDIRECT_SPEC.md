# RC7 Windows installer proxy and redirect specification

Status: `IMPLEMENTED_AND_VERIFIED`

## Scope

This specification defines the single HTTP asset transport used by
`install/v0.2/install.ps1` in AOPMem `v0.2.0-rc7`.

It changes only Windows installer transport and its diagnostics. It does not
change upgrade ordering, product behavior, schemas, workspace data, or the
macOS installer.

The implementation must remain self-contained in `install.ps1`. It must use
Windows PowerShell 5.1-compatible syntax and APIs. It must not use
`Invoke-WebRequest -MaximumRedirection 0`, `curl`, Python, BITS, a browser, or a
second downloaded helper.

## Public parameters

Preserve every existing installer parameter. Add:

```powershell
[Uri]$ProxyUri

[switch]$ProxyUseDefaultCredentials
```

`ProxyUseDefaultCredentials` is an opt-in credential boundary. It does not
select a proxy by itself.

## Proxy resolution

Resolve one immutable proxy configuration before the first asset request.
Reuse it for `SHA256SUMS`, the platform binary, and every redirect hop.

Precedence:

1. explicit `-ProxyUri`;
2. `HTTPS_PROXY`;
3. `https_proxy`;
4. `HTTP_PROXY`;
5. `http_proxy`;
6. `[System.Net.WebRequest]::DefaultWebProxy`, when non-null and usable;
7. no proxy, direct HTTPS.

An absent proxy is valid. Direct HTTPS must remain supported.

For explicit or environment proxy values:

- require an absolute URI;
- require scheme `http` or `https`;
- require a non-empty host;
- reject URI user information;
- reject query and fragment;
- reject whitespace and malformed values;
- never log the raw proxy URI;
- report only source `explicit`, `env`, `system`, or `none`;
- report only whether a proxy is configured.

The installer must not add proxy username or password parameters. It must not
read, print, persist, or return proxy credentials.

When `-ProxyUseDefaultCredentials` is supplied and a proxy is selected, assign
`[System.Net.CredentialCache]::DefaultNetworkCredentials` to that proxy object.
Without the switch, do not assign default credentials. Never assign credentials
to the target server, `HttpClientHandler.Credentials`, request headers, or an
`Authorization` header.

If the system default proxy is unusable, continue as direct only when its
absence can be established without a network request. Do not silently ignore a
malformed explicit or environment proxy.

## Canonical HTTP client

Load the PowerShell 5.1 assembly compatibly:

```powershell
Add-Type -AssemblyName System.Net.Http
```

Retain TLS 1.2 configuration through
`[Net.ServicePointManager]::SecurityProtocol`.

Create one `System.Net.Http.HttpClientHandler` and one
`System.Net.Http.HttpClient` for an asset request. Required handler state:

- `AllowAutoRedirect = false`;
- `UseCookies = false`;
- explicit `WebProxy` object when a proxy URI was resolved;
- `UseProxy = false` for a direct connection;
- proxy default credentials only after explicit opt-in;
- no certificate callback or validation bypass;
- no target-server credentials.

Send with:

```text
HttpCompletionOption.ResponseHeadersRead
```

Dispose client, handler, every response, response stream, and file stream on
all paths.

## Target URI validation

The initial asset URI and every resolved redirect URI must:

- be absolute;
- use `https`;
- have a non-empty host;
- contain no URI user information;
- satisfy the release origin and path boundary below;
- contain no untrusted query or fragment introduced by installer input.

Never allow HTTPS downgrade. Never copy credentials or sensitive headers to a
redirect target. Log only sanitized target host/path, never user information,
query secrets, or fragments.

The initial asset URI must be an exact direct child of the validated asset base
and must end in the selected flat asset name.

A redirect is allowed only when either:

1. it remains on the initial HTTPS origin and its normalized path remains
   inside the initial release-base path; or
2. the initial origin is `https://github.com`, its path matches the trusted
   GitHub release-download form for the selected repository/tag/asset, and the
   response redirects to `https://release-assets.githubusercontent.com`.

After transition to `release-assets.githubusercontent.com`, further redirects
must remain on that exact HTTPS origin. Its opaque non-empty CDN object path is
accepted after normalization and must not be required to contain the flat asset
filename. A query supplied by the validated GitHub redirect may carry signed
CDN data; it may be sent but must never be logged. Fragments, path traversal,
userinfo, another host transition, and a later return to an earlier URI are
rejected.

`-AssetBaseUri` continues to override `AOPMEM_ASSET_BASE_URI`. Both release
assets must use the canonical transport.

## Manual redirect state machine

Recognize only `301`, `302`, `303`, `307`, and `308` as redirects.

For every response:

1. Read status and required headers.
2. For a redirect, require one usable `Location`.
3. Resolve a relative `Location` against the current URI.
4. Validate the resulting URI before sending another request.
5. Dispose the current response.
6. Reuse the same proxy configuration and credential boundary.
7. Canonicalize the next absolute URI and record it in a visited set.
8. Reject a repeated URI with `HTTP_REDIRECT_LOOP`.
9. Reject more than 10 redirect hops with `HTTP_REDIRECT_LIMIT`.

Do not treat redirects as exceptions. A missing `Location` is
`HTTP_REDIRECT_MISSING_LOCATION`. Any redirect target failing URI validation is
`UNSAFE_REDIRECT_TARGET`.

The only accepted final status is `200`. Reject `206`, other 2xx statuses, and
all other statuses with `HTTP_STATUS_REJECTED`.

## File download and publication

For a final `200`:

1. Verify the destination parent is the already validated private temp root.
2. Require the destination to be absent.
3. Create a unique partial file in that same parent.
4. Open it with `FileMode.CreateNew`, write access, and no sharing.
5. Stream the response body in bounded chunks; never buffer the whole asset.
6. Count actual bytes.
7. Reject zero bytes with `ASSET_DOWNLOAD_FAILED`.
8. When `Content-Length` is present, require exact equality; otherwise use the
   streamed count.
9. Flush durably with `Flush($true)`.
10. Close response and file streams.
11. Publish by no-overwrite move in the same parent.
12. If the destination appeared concurrently, preserve it and fail.
13. On pre-publication failure, remove only the owned partial file.
14. Never remove or truncate an existing destination.

Return internal transport facts sufficient for installer diagnostics: final
path, byte count, sanitized final URI, redirect count, and partial publication
state. SHA-256 verification remains the existing separate step after download.

## Error contract

Stable classes:

- `PROXY_CONFIGURATION_INVALID`;
- `HTTP_REQUEST_FAILED`;
- `HTTP_STATUS_REJECTED`;
- `HTTP_REDIRECT_MISSING_LOCATION`;
- `HTTP_REDIRECT_LIMIT`;
- `HTTP_REDIRECT_LOOP`;
- `UNSAFE_REDIRECT_TARGET`;
- `ASSET_DOWNLOAD_FAILED`;
- `ASSET_LENGTH_MISMATCH`.

Every transport failure must preserve:

- installer stage;
- sanitized target host/path;
- proxy configured `yes` or `no`;
- proxy source `explicit`, `env`, `system`, or `none`;
- original exception type;
- original exception message;
- real HTTP status, when a response exists;
- redirect hop;
- destination path;
- partial file state;
- the existing top-level old-binary and backup preservation report.

Never read:

```powershell
$_.Exception.Response
```

without first checking:

```powershell
$_.Exception.PSObject.Properties['Response']
```

The canonical `HttpClient` path should not need `Exception.Response`. When an
exception has no `Response` property, preserve its exact type and message. Do
not replace it with `PropertyNotFoundException`.

Zero references to `Exception.Response` is the preferred result. The
`PSObject.Properties['Response']` guard is required only if a compatibility
path still reads `Exception.Response`.

Never log proxy credentials, tokens, cookies, `Authorization`, raw environment
dumps, URI user information, query secrets, or the field proxy hostname.

## Installer ordering

Transport changes do not alter the update state machine:

```text
process gate
→ durable full-home backup
→ download and verify
→ staged platform check
→ staged audit repair
→ upgrade prepare
→ upgrade plan
→ upgrade apply exactly once
→ binary publish
→ adapter sync
→ post-publish audit repair
→ doctor, verify, task smoke, observability, debug capsule
```

Any transport failure occurs before staged platform check and before apply.
The installed binary remains byte-for-byte unchanged. Apply attempts remain
zero. Existing backups remain retained.

## Bootstrap contract

The install prompt must provide a native Windows PowerShell 5.1 bootstrap that
downloads the immutable raw `install.ps1` to a private `%TEMP%` path.

The proxy form uses `Invoke-WebRequest -UseBasicParsing -Proxy <synthetic-or-
operator-provided-uri>` and optional `-ProxyUseDefaultCredentials`, with normal
redirect behavior and no `-MaximumRedirection 0`. Then invoke the downloaded
installer with matching `-ProxyUri` and optional
`-ProxyUseDefaultCredentials`.

The direct form omits both proxy parameters. Proxy setup is technical
environment configuration, not an onboarding question.

Public docs, fixtures, reports, and source must not contain the observed field
proxy hostname.

## Proof contract

`scripts/test_windows_installer_transport.py` defines exactly these 30 cases:

1. direct HTTPS, no proxy;
2. explicit proxy URI;
3. `HTTPS_PROXY`;
4. `HTTP_PROXY` fallback;
5. lowercase environment variants;
6. explicit proxy wins;
7. absent proxy remains valid;
8. invalid proxy scheme rejected;
9. proxy user information rejected;
10. default credentials only with the switch;
11. absolute `302`;
12. relative `302`;
13. `303`;
14. `307`;
15. `308`;
16. missing `Location`;
17. HTTP downgrade;
18. redirect user information;
19. redirect loop;
20. redirect limit;
21. final non-200;
22. empty body;
23. `Content-Length` mismatch;
24. existing destination preserved;
25. partial file cleanup;
26. original exception type/message;
27. exception without `Response`;
28. no unsafe direct `.Exception.Response`;
29. no production `MaximumRedirection 0`;
30. same proxy across every redirect.

The Python harness uses only the standard library and synthetic
`example.invalid` / loopback values. Its pure model proves the state-machine
contract deterministically. Static assertions bind every case to production
`install.ps1`.

The implemented Stage 03 transport passes all 30 cases and the complete
installer audit. A passing pure model alone is never an installer PASS; static
assertions bind every case to the production script. macOS static checks do
not prove native Windows PowerShell 5.1 runtime; native acceptance remains
pending.
