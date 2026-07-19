# RC7 Stage 03 Handoff

Status: `VERIFIED`

`install/v0.2/install.ps1` now uses one self-contained PowerShell 5.1
`System.Net.Http` transport. The 30-case static/model harness passes.

Credential boundary: default credentials are assigned only to a freshly
selected proxy object when `-ProxyUseDefaultCredentials` is present.
`HttpClientHandler` target credentials and preauthentication are disabled.

Redirect responses are disposed after reading status/Location. Every next URI
is validated before another request. Existing destinations are preserved;
owned partials are removed on pre-publication failure.

No local PowerShell runtime exists. Do not claim native PowerShell 5.1.

Next: Stage 04 known-source matrix and cumulative audit.
