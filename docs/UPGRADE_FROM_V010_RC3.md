# Upgrade from AOPMem v0.1.0-rc3

This release upgrades an existing user-level AOPMem v0.1.0-rc3 install.
It does not migrate the old file MVP or any other product.

## Supported path

The only supported source and target are:

| Item | Value |
|---|---|
| Source | `0.1.0-rc3` |
| Target | `0.2.0-rc1` |
| Scope | all existing AOPMem workspaces |
| macOS | Apple Silicon |
| Windows | Windows 11 x64, PowerShell 5.1 |

The tagged v0.1.0-rc3 binaries report `aopmem 0.1.0`.
The installer separates that binary semver from the release label and also
requires the exact tagged release-asset SHA-256.

| Tagged source asset | SHA-256 |
|---|---|
| macOS arm64 | `d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607` |
| Windows x64 | `01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a` |

The installer uses no administrator rights, source build, WSL, Node.js,
daemon, cloud service, or Codex CLI.

## Data preserved

Upgrade keeps:

- nodes, links, aliases, tags, sources, and events;
- tool contracts and MCP profiles;
- statuses, confidence, and trust;
- audit history and artifacts;
- global skills and templates;
- the managed adapter block;
- every old binary and migration backup.

The new observability database is created separately from operational
memory. It is not placed in `memory.sql`.

## Safe update order

The installer performs this fixed sequence:

1. Detect the exact old binary version.
2. Download the selected flat binary and `SHA256SUMS`.
3. Verify one exact checksum line, the binary hash, and exact new version.
4. Copy and verify a durable old binary backup.
5. Create verified v0.2 stage and recovery files beside the installed binary.
6. Run the temporary v0.2 binary with:

   ```text
   aopmem upgrade plan --all-workspaces --json
   ```

7. Require a read-only plan with `ready=true`.
8. Run the temporary v0.2 binary with:

   ```text
   aopmem upgrade apply --all-workspaces --json
   ```

9. Require `success=true` and `binary_replaced=false`.
10. Atomically publish the staged binary in the same directory.
11. Verify the installed hash and exact version.

`upgrade apply` owns database backups, migrations, observability creation,
global asset refresh, adapter sync, doctor, and verify. The installer does
not repeat update health checks after publication.

This order avoids an unsafe rollback. Once apply starts, a workspace may
contain committed v0.2 schema state. A v0.1 binary must not be restored over
that state.

## Backups

There are two backup layers:

- Installer binary backup:
  `~/.aopmem/bin/aopmem.backup-v0.1.0-rc3-*`
- Upgrade run backup:
  `~/.aopmem/backups/upgrade-0.2.0-rc1-*`

Windows uses the same directories under `%USERPROFILE%\.aopmem` and adds
`.exe` to the binary filename.

An upgrade run backup contains the old binary, workspace databases, adapter
state, and owned global assets when those inputs exist. The JSON apply report
returns the exact `backup_root`.

Do not delete backups during RC dogfood.

## Failure behavior

Before `upgrade apply` starts, the installer never replaces the installed
binary. On a plan or early failure, both platforms verify the original type
and hash. The macOS fixture also proves the inode remains unchanged.
The installer keeps the binary backup.

After `upgrade apply` starts:

- keep every backup;
- keep every workspace;
- keep the verified v0.2 recovery binary;
- do not run or restore v0.1;
- stop on the exact reported workspace and error;
- fix that error, then rerun plan before apply.

The recovery binary path is printed in the error. It is a regular verified
file beside the installed binary. This is intentional: a later workspace
failure may occur after an earlier workspace committed a migration.

The installer always removes download temp files. It never deletes the
recovery binary on an apply or publish failure.

## Onboarding behavior

Update asks no semantic questions and never runs `aopmem init`.
Existing memory is reused.

Only a fresh install runs normal `aopmem init` and its existing five
semantic questions. It then seeds the managed adapter block and requires
`doctor` to report `healthy=true` and `verify` to report `clean=true`.

## Manual read-only preflight

Use the downloaded, verified v0.2 binary. Do not use an unverified file.

```text
aopmem upgrade plan --all-workspaces --json
```

The plan must report:

- outer `ok=true`;
- `ready=true`;
- `writes_performed=false`;
- sufficient disk space;
- no unsupported schema, unsafe path, or pending snapshot.

The installer already runs this command. Manual use is for diagnosis.
Adapter drift is checked later by `upgrade apply` preflight.

## Verification

The repository proof is:

```sh
scripts/audit_v020_installers.sh
```

It uses isolated temporary homes and local stub assets. It performs no
network request and does not touch the real AOPMem home.

The proof covers checksum mismatch and duplication, wrong version, unsafe
paths, plan failure, apply failure, publish failure, backup failure,
fresh health failure, temp cleanup, backup retention, recovery retention,
and zero onboarding during update.

This audit uses stub executables. It is not the final real migration proof.

## Stage 35 real macOS proof

After the final v0.2 binary freezes, run both flows with that real binary.
Use only temporary homes and repositories.

Set the final flat binary path:

```sh
V020_BINARY="/absolute/path/to/aopmem-darwin-arm64"
test -x "$V020_BINARY"
test "$("$V020_BINARY" --version)" = "aopmem 0.2.0-rc1"
```

Create local release assets:

```sh
PROOF_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/aopmem-v020-real-proof.XXXXXX")
mkdir "$PROOF_ROOT/assets" "$PROOF_ROOT/temp"
cp "$V020_BINARY" "$PROOF_ROOT/assets/aopmem-darwin-arm64"
V020_SHA=$(shasum -a 256 \
  "$PROOF_ROOT/assets/aopmem-darwin-arm64" | awk '{ print $1 }')
printf '%s  %s\n' "$V020_SHA" "aopmem-darwin-arm64" \
  > "$PROOF_ROOT/assets/SHA256SUMS"
```

Fresh proof:

```sh
mkdir "$PROOF_ROOT/fresh-repo" "$PROOF_ROOT/fresh-home"
git -C "$PROOF_ROOT/fresh-repo" init -q
printf '%s\n' \
  no \
  no \
  "fresh proof project" \
  "user owns product; agent follows stored process" \
  "temporary proof repo only" \
| (
  cd "$PROOF_ROOT/fresh-repo"
  AOPMEM_INSTALL_TEST_MODE=1 \
  AOPMEM_INSTALL_TEST_OS=Darwin \
  AOPMEM_INSTALL_TEST_ARCH=arm64 \
  AOPMEM_INSTALL_TEST_ASSET_DIR="$PROOF_ROOT/assets" \
  AOPMEM_INSTALL_TEST_TEMP_ROOT="$PROOF_ROOT/temp" \
  AOPMEM_HOME="$PROOF_ROOT/fresh-home" \
  sh "/absolute/repo/install/v0.2/install.sh"
)
```

Peeled v0.1.0-rc3 update proof:

```sh
mkdir "$PROOF_ROOT/update-repo" "$PROOF_ROOT/update-home"
mkdir -p "$PROOF_ROOT/update-home/bin"
git -C "$PROOF_ROOT/update-repo" init -q
git -C "/absolute/repo" \
  show "v0.1.0-rc3:dist/aopmem-darwin-arm64/aopmem" \
  > "$PROOF_ROOT/update-home/bin/aopmem"
chmod 755 "$PROOF_ROOT/update-home/bin/aopmem"
OLD_SHA=$(shasum -a 256 \
  "$PROOF_ROOT/update-home/bin/aopmem" | awk '{ print $1 }')
test "$OLD_SHA" = \
  "d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607"
printf '%s\n' \
  no \
  no \
  "peeled v0.1 proof project" \
  "user owns product; agent preserves learned process" \
  "temporary peeled fixture only" \
| (
  cd "$PROOF_ROOT/update-repo"
  AOPMEM_HOME="$PROOF_ROOT/update-home" \
    "$PROOF_ROOT/update-home/bin/aopmem" init
)
(
  cd "$PROOF_ROOT/update-repo"
  AOPMEM_INSTALL_TEST_MODE=1 \
  AOPMEM_INSTALL_TEST_OS=Darwin \
  AOPMEM_INSTALL_TEST_ARCH=arm64 \
  AOPMEM_INSTALL_TEST_ASSET_DIR="$PROOF_ROOT/assets" \
  AOPMEM_INSTALL_TEST_TEMP_ROOT="$PROOF_ROOT/temp" \
  AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256="$OLD_SHA" \
  AOPMEM_INSTALL_TEST_TRACE="$PROOF_ROOT/update-installer.trace" \
  AOPMEM_HOME="$PROOF_ROOT/update-home" \
  sh "/absolute/repo/install/v0.2/install.sh"
)
```

Then run:

```sh
test "$("$PROOF_ROOT/update-home/bin/aopmem" --version)" = \
  "aopmem 0.2.0-rc1"
! grep -Eq '^init$' "$PROOF_ROOT/update-installer.trace"
(
  cd "$PROOF_ROOT/update-repo"
  AOPMEM_HOME="$PROOF_ROOT/update-home" \
    "$PROOF_ROOT/update-home/bin/aopmem" doctor --json
  AOPMEM_HOME="$PROOF_ROOT/update-home" \
    "$PROOF_ROOT/update-home/bin/aopmem" verify --json
)
find "$PROOF_ROOT/update-home/backups" -type f -print
```

For the fresh fixture, also require `adapter status --json` to report
`in_sync`, `doctor --json` to report `healthy=true`, and `verify --json` to
report `clean=true`.

Keep the command output as Stage 35 proof. Delete `PROOF_ROOT` only after
the proof is copied into the release proof log.
