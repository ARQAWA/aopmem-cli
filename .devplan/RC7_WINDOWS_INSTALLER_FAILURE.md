# RC7 Windows Installer Failure

Status: `VERIFIED_FIELD_TRANSCRIPTION`

## Native environment

- Windows 11 Enterprise x64, build `22631`
- native Windows PowerShell `5.1.22621.7376`
- ordinary user; no administrator rights
- no WSL, Docker, Cargo, Rustup, or source build
- corporate proxy required for GitHub; exact private hostname omitted
- direct GitHub access unavailable
- process `HTTP_PROXY` and `HTTPS_PROXY` initially absent

Python `requests` downloaded the assets only after the operator explicitly set
the process proxy variables. `requests.Session.trust_env` stayed enabled.
This proves network reachability through the proxy, not installer correctness.

## Exact RC6 installer failure

RC6 `Save-HttpsAsset` invokes:

```powershell
Invoke-WebRequest `
    -Uri $currentUri `
    -OutFile $Destination `
    -UseBasicParsing `
    -MaximumRedirection 0 `
    -TimeoutSec 900 `
    -ErrorAction Stop
```

Native PowerShell returned:

```text
EXCEPTION_TYPE=System.InvalidOperationException
EXCEPTION_MESSAGE=Operation is not valid due to the current state of the object.
HAS_RESPONSE_PROPERTY=False
```

The exception did not expose a `Response` property. RC6 then evaluated
`$_.Exception.Response` under `Set-StrictMode`, causing a secondary
`PropertyNotFoundException`. That secondary exception masked the original
network/redirect failure.

## Mutation boundary

- full-home and binary backups completed
- asset download failed
- staged platform check not run
- audit repair not run
- upgrade prepare not run
- upgrade plan not run
- upgrade apply attempts: `0`
- binary publish not run
- adapter sync not run
- user data changed: `false`
- installed RC4 retained
- both existing workspaces retained

## Binary boundary

The native RC6 Windows binary itself passed:

```text
aopmem 0.2.0-rc6
platform check --json
exit=0
ok=true
status=pass
```

Windows atomic publication, reparse guards, containment, and bounded cleanup
passed. RC7 scope is therefore limited to official installer HTTP transport,
source-binary classification, version/docs, and release evidence.
