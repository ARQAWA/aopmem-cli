# Windows native PowerShell smoke

The RC7 native smoke uses the official updater only. Do not copy, move, or
replace `aopmem.exe` manually.

## Required host

- Windows 11 x64, build `22631`;
- native Windows PowerShell `5.1`;
- ordinary user account;
- no WSL, Bash, Cargo, Rustup, or source build.

## Procedure

1. Follow the complete immutable-RC7 checklist in
   [RC7_WINDOWS_ACCEPTANCE_PROMPT.md](../.devplan/RC7_WINDOWS_ACCEPTANCE_PROMPT.md).
2. Verify the release `SHA256SUMS` before the staged executable runs.
3. Require staged `platform check --json` to pass before the updater starts.
4. Download immutable RC7 `install.ps1` through the documented proxy-safe
   bootstrap. Run it with the RC7 release base, `-ProxyUri <PROXY_URI>`,
   optional `-ProxyUseDefaultCredentials`, and one explicit active adapter
   pair.
5. Require redirect success, no masked `PropertyNotFoundException`, staged
   platform check success, exactly one apply, and successful publication.
6. Require `aopmem 0.2.0-rc7`, retained backups, schema `004`, healthy doctor,
   clean verify, task protocol, observability export, exact-only dedupe proof,
   and read-only loopback UI smoke.

Native Windows runtime remains `PENDING_DOGFOOD` until this procedure passes
on the specified Windows 11 VDI. macOS tests, PE inspection, imports, and
hashes do not prove Windows runtime behavior.
