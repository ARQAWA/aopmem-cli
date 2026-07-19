# Windows native update

This path updates SQLite-backed AOPMem v0.1 through `v0.2.0-rc6` to
`v0.2.0-rc7` on Windows 11
x64 through native Windows PowerShell 5.1.

## Boundaries

- No administrator rights.
- No WSL, Docker, Cargo, Rustup, Node.js, Codex CLI, clone, or source build.
- Use `%USERPROFILE%\.aopmem`.
- Keep temporary update files under `%TEMP%`.
- Keep durable backups under `%LOCALAPPDATA%\AOPMemBackups` or the canonical
  installer backup path.
- Never create repository-local `.aopmem`.
- Never query or edit workspace SQLite manually.
- Never delete WAL, SHM, or journal files manually.

Required flat release assets:

- `aopmem-windows-x86_64.exe` — SHA-256
  `9e957a2b47c7442ab6aff57a8f8d3469b41e158831a55be18218fc239db29ae1`;
- `SHA256SUMS` — SHA-256
  `89e59fd7eceed6048d1ef0367bd4cccc32cc40ab692713e4224e60c78b36e0bc`.

Use only the trusted release asset URI. Verify SHA-256 before executing the
staged binary.

## Corporate proxy

Use `-ProxyUri <PROXY_URI>` when the host requires an explicit proxy. Add
`-ProxyUseDefaultCredentials` only when integrated proxy credentials are
required. Never put credentials in the proxy URI.

The installer resolves one proxy in this order: explicit `-ProxyUri`,
`HTTPS_PROXY`, `https_proxy`, `HTTP_PROXY`, `http_proxy`, usable system
default proxy, then direct connection. Direct mode remains supported.

The RC7 installer uses `HttpClientHandler.AllowAutoRedirect=false` and handles
301, 302, 303, 307, and 308 as normal responses. Every redirect remains HTTPS,
contains no userinfo, reuses the same proxy, and is bounded by loop and
10-hop checks. Downloads stream to a create-new partial file under the private
temporary root. An existing destination is never overwritten.

Errors preserve the original exception type and message. RC7 does not inspect
an absent `Exception.Response` property, so a transport error cannot be masked
by `PropertyNotFoundException`. See
[Windows proxy install](WINDOWS_PROXY_INSTALL.md) for bootstrap commands.

## Before update

Close AOPMem UI and other AOPMem processes. Recheck live processes; do not
reuse old PID values.

Record through filesystem and binary inspection:

- installed version and SHA-256;
- workspace directory names;
- absence of repository-local `.aopmem`;
- target repository Git status.

Do not run unsupported `workspace list`.

Create a durable full backup of `%USERPROFILE%\.aopmem` before preparation.
Verify the copied binary hash, workspace directories, and database files.

## Required update sequence

The official updater keeps this exact order. It uses the downloaded and
verified `v0.2.0-rc7` binary; do not manually copy or replace the installed
binary.

```text
process gate
→ durable full-home backup
→ download and SHA-256/version verification
→ staged platform check --json
→ staged audit repair --all-workspaces --json when pending
→ upgrade prepare --all-workspaces --json
→ upgrade plan --all-workspaces --json
→ upgrade apply --all-workspaces --json --approved "+++" exactly once
→ upgrade publish --json
→ adapter sync
→ post-publish audit repair --all-workspaces --json
→ doctor, verify, task protocol, observability, debug capsule
```

Requirements:

1. Platform check succeeds before audit repair, prepare, plan, apply, or
   binary publish. Its failure changes no user data.
2. The durable full-home backup exists before prepare. Keep every backup.
3. `prepare` succeeds for every workspace.
4. No AOPMem DB read runs between `prepare` and `plan`.
5. `plan` returns `ok=true`, `ready=true`,
   `writes_performed=false`.
6. `apply` starts only after a clean plan and runs once. Never retry it
   automatically.
7. Binary publication starts only after successful apply and uses the
   canonical same-directory atomic replacement flow.
8. Adapter sync, post-publish audit repair, and health checks run only after
   publication.

## WAL preparation

`upgrade prepare` is the supported fix for a zero-byte or committed WAL.
It creates per-workspace backup, checkpoints through SQLite, closes the
connection, then removes only verified empty direct-child coordination files.

Do not use `Remove-Item` on `aopmem.sqlite-wal` or
`aopmem.sqlite-shm`. Do not use `sqlite3`, Python SQL, or another database
client to checkpoint the workspace.

Expected preparation properties:

- active/busy database fails closed;
- unsafe reparse/symlink path fails closed;
- backup failure prevents checkpoint;
- incomplete checkpoint prevents cleanup;
- schema version remains unchanged;
- logical memory remains unchanged;
- repeated preparation is safe.

If sidecars remain, plan returns `ready=false` and directs the operator to
run `upgrade prepare` again after resolving the exact blocker.

## Noncanonical v0.1 build

The canonical source is release `v0.1.0-rc3`, whose binary reports
`aopmem 0.1.0`.

An unknown v0.1 SHA-256 must be reported as:

```text
NONCANONICAL_V010_BINARY
```

Do not replace or relabel the hash. Do not reject a compatible workspace only
because the old binary differs. Require durable backup, successful staged
prepare, and `plan ready=true`.

Corrupt, unsupported, or newer schemas remain blockers.

## Failure recovery

Before apply:

- installed binary remains unchanged;
- migrations have not started;
- keep durable and preparation backups;
- fix the exact preparation or plan blocker;
- rerun staged preparation and fresh read-only plan.

After apply starts:

- never rerun apply automatically;
- never restore v0.1 over migrated data;
- keep every backup and recovery binary;
- keep JSON stdout/stderr and stopped workspace/error;
- resume only through a separately reviewed recovery action.

If apply returns `WORKSPACE_BACKUP_FAILED`, require structured evidence:

- `workspace_key` and exact `backup_phase`;
- `raw_os_error` and normalized `io_kind`;
- temporary and final backup paths;
- partial file existence, size, and validation state;
- `migration_started=false`.

Keep the partial file and every older backup root. Do not retry apply
automatically. A populated partial file is evidence only; it is accepted as a
backup only after writable flush, atomic publish, final read-only reopen, and
schema/integrity validation.

Tooling or PowerShell wrapper errors before mutation are not automatically
product failures. Fix the helper once or use another documented transport.
Do not hide real product errors behind wrapper exception handling.

## Post-update checks

Run in the target repository:

```text
aopmem adapter status --file AGENTS.md --json
aopmem doctor --json
aopmem verify --json
aopmem recall --json
aopmem observe status --json
aopmem observe report --json
```

If the managed adapter is missing or drifted, use the documented adapter sync
command, then repeat status, doctor, and verify.

Require:

- installed `aopmem 0.2.0-rc7` and release SHA-256;
- unchanged workspace keys;
- all old workspace directories present;
- adapter in sync;
- doctor healthy;
- verify clean;
- recall and observability successful;
- repository-local `.aopmem` absent;
- backups retained.

Do not launch UI during the update flow. Test UI separately after successful
update so it cannot hold the executable or recreate sidecars before plan.

## Proof boundary

macOS-hosted checks can prove Rust tests, fixtures, installer structure, PE
type, imports, and release hash. They cannot prove native Windows PowerShell
or executable runtime behavior.

The operational schema remains `004_task_protocol_and_tool_aliases`; RC7 adds
no migration `005`.

Native Windows runtime remains `PENDING_DOGFOOD` until this exact RC7 asset
runs on Windows 11 x64 with PowerShell 5.1 against backed-up dogfood
workspaces. macOS-hosted proof is not a native runtime PASS.
