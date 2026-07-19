# RC6 native Windows acceptance prompt

Run this only after the RC6 prerelease exists. This is a native acceptance
check, not a release-build procedure.

## Fixed scope

- Host: Windows 11 Enterprise, build `22631`, x64.
- Shell: native Windows PowerShell `5.1` only.
- Rights: ordinary user; do not request administrator rights.
- Forbidden: WSL, Bash, Cargo, Rustup, source build, manual SQLite/WAL/SHM
  work, manual binary copy, and automatic apply retry.
- Current installed binary: `aopmem 0.2.0-rc4`.
- Current installed SHA-256:
  `E4442FD06622A6B94F997E23B67A55753F1D841F6570EF20AC72B99083A6CC1C`.
- Preserve these workspaces and all existing backups:
  `p-sit-cat-rental-8ef3bf83` and `p-sit-warranty-5708363a`.

## RC6 release inputs

```text
Release base:
https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc6

Windows asset:
https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc6/aopmem-windows-x86_64.exe

Checksum manifest:
https://github.com/ARQAWA/aopmem-cli/releases/download/v0.2.0-rc6/SHA256SUMS

Windows SHA-256:
8CD03FD00FFDAF505D7F31CD1C485FD15179823F84A78061B7BCFC00EE4FD4C7
```

Use the audited installer from the immutable RC6 tag:

```text
https://raw.githubusercontent.com/ARQAWA/aopmem-cli/v0.2.0-rc6/install/v0.2/install.ps1
```

## Acceptance procedure

1. Confirm native PowerShell 5.1 and Windows build 22631. Stop with
   `RC7_REQUIRED` if either differs.
2. Close every AOPMem UI and CLI process. Do not terminate an unknown process
   automatically.
3. Record the RC4 executable SHA-256, both workspace directory names, current
   backup roots, and absence of repository-local `.aopmem`.
4. Download the Windows asset and `SHA256SUMS` to a new private `%TEMP%`
   directory. Require exactly one manifest line for
   `aopmem-windows-x86_64.exe`, verify its SHA-256 equals the value above, and
   require `aopmem 0.2.0-rc6` from `--version`.
5. Run the verified staged binary first:

   ```text
   aopmem-windows-x86_64.exe platform check --json
   ```

   Require exit `0` and `ok=true`. It must pass create, flush, no-replace,
   replace, reopen, and cleanup. If it fails, stop before audit repair,
   prepare, plan, apply, or binary publication. Record the JSON and return
   `RC7_REQUIRED`.
6. Set the explicit active adapter pair. For Codex use
   `AOPMEM_ACTIVE_ADAPTER=codex` and
   `AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md`. Use another documented exact
   pair only when that is the active adapter.
7. Run only the official RC6 updater with the release base URI. It must make
   the durable full-home backup, verify/download RC6, run its own staged
   platform check, repair pending audit state, prepare, plan, apply exactly
   once, publish the binary, sync the adapter, repair audit state again, and
   run health/export checks. Do not manually invoke binary replacement or a
   second apply.
8. Require final `aopmem 0.2.0-rc6`, the release SHA-256, both old workspace
   keys present, all old backups retained, adapter in sync, `doctor` healthy,
   and `verify` clean.
9. In an approved test workspace, complete task start, apply, and complete
   using the returned canonical task and bundle IDs. Verify the receipt has a
   bundle ID, memory revision, and complete mandatory context.
10. Run the approved secret/redaction fixture with a non-production test
    secret. Run `observe export` to a new file and confirm the raw test secret
    is absent from the capsule and command output.
11. Run `aopmem tool dedupe plan --json`. If the approved exact-duplicate
    fixture is present, run only `aopmem tool dedupe apply --exact-only --json`
    and confirm canonical/alias resolution without deleting tool directories.
12. Run `aopmem ui --no-open --port 0` in the approved test workspace. Open
    its printed loopback URL manually, check the read-only UI, then stop the
    foreground process. The UI must not create repository-local `.aopmem`.

## Result

Return exactly one result:

- `RC6_ACCEPTED` only if every required step passes and data/backups are
  preserved.
- `RC7_REQUIRED` on any failure, unexpected mutation, missing backup, wrong
  version/hash, or failed platform check. Preserve JSON, logs, staged binary,
  recovery material, and all backups. Do not retry apply automatically.
