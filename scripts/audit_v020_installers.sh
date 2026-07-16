#!/bin/sh

set -eu
umask 077

SCRIPT_DIR=$(CDPATH= cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd "$SCRIPT_DIR/.." && pwd)
MAC_INSTALLER="$REPO_ROOT/install/v0.2/install.sh"
WINDOWS_INSTALLER="$REPO_ROOT/install/v0.2/install.ps1"
TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/aopmem-installer-audit.XXXXXX")
TEST_COUNT=0

cleanup() {
  rm -rf "$TEST_ROOT"
}
trap cleanup EXIT HUP INT TERM

fail() {
  printf '%s\n' "installer audit failed: $1" >&2
  exit 1
}

pass() {
  TEST_COUNT=$((TEST_COUNT + 1))
}

assert_file() {
  [ -f "$1" ] || fail "expected file: $1"
}

assert_no_file() {
  [ ! -e "$1" ] && [ ! -L "$1" ] || fail "unexpected path: $1"
}

assert_contains() {
  grep -Eq -- "$2" "$1" || fail "missing pattern '$2' in $1"
}

assert_not_contains() {
  if [ ! -e "$1" ]; then
    return
  fi
  if grep -Eq -- "$2" "$1"; then
    fail "forbidden pattern '$2' in $1"
  fi
}

assert_equal() {
  [ "$1" = "$2" ] || fail "expected '$1' to equal '$2': $3"
}

assert_temp_clean() {
  found=$(find "$1" ! -path "$1" -print -quit)
  [ -z "$found" ] || fail "temporary files were not cleaned under $1"
}

assert_trace_order() {
  first_line=$(grep -n -m 1 -F "$2" "$1" | cut -d: -f1)
  second_line=$(grep -n -m 1 -F "$3" "$1" | cut -d: -f1)
  [ -n "$first_line" ] && [ -n "$second_line" ] \
    || fail "trace entries missing: $2 -> $3"
  [ "$first_line" -lt "$second_line" ] \
    || fail "trace order is wrong: $2 -> $3"
}

write_new_stub() {
  destination=$1
  {
    printf '%s\n' '#!/bin/sh'
    printf '%s\n' 'set -eu'
    printf '%s\n' 'if [ -n "${AOPMEM_STUB_TRACE:-}" ]; then'
    printf '%s\n' '  printf "%s\n" "cli:$*" >> "$AOPMEM_STUB_TRACE"'
    printf '%s\n' 'fi'
    printf '%s\n' 'case "$*" in'
    printf '%s\n' '  "--version")'
    printf '%s\n' '    printf "%s\n" "aopmem 0.2.0-rc1"'
    printf '%s\n' '    ;;'
    printf '%s\n' '  "upgrade plan --all-workspaces --json")'
    printf '%s\n' '    if [ "${AOPMEM_STUB_PLAN_NOT_READY:-0}" = "1" ]; then'
    printf '%s\n' '      printf "%s\n" '\''{"ok":true,"data":{"ready":false,"writes_performed":false}}'\'''
    printf '%s\n' '    else'
    printf '%s\n' '      printf "%s\n" '\''{"ok":true,"data":{"ready":true,"writes_performed":false}}'\'''
    printf '%s\n' '    fi'
    printf '%s\n' '    ;;'
    printf '%s\n' '  "upgrade apply --all-workspaces --json")'
    printf '%s\n' '    if [ "${AOPMEM_STUB_APPLY_FAIL:-0}" = "1" ]; then'
    printf '%s\n' '      printf "%s\n" '\''{"ok":false,"data":{"success":false,"binary_replaced":false,"stopped_workspace":"fixture-workspace","backup_root":"/fixture/upgrade-backup"},"errors":[{"code":"FIXTURE_APPLY_FAILED","message":"fixture apply failure"}]}'\'''
    printf '%s\n' '      exit 1'
    printf '%s\n' '    fi'
    printf '%s\n' '    printf "%s\n" '\''{"ok":true,"data":{"success":true,"binary_replaced":false}}'\'''
    printf '%s\n' '    ;;'
    printf '%s\n' '  "init")'
    printf '%s\n' '    ;;'
    printf '%s\n' '  "adapter seed --json")'
    printf '%s\n' '    if [ "${AOPMEM_STUB_ADAPTER_FAIL:-0}" = "1" ]; then'
    printf '%s\n' '      printf "%s\n" '\''{"ok":false,"data":{}}'\'''
    printf '%s\n' '    else'
    printf '%s\n' '      printf "%s\n" '\''{"ok":true,"data":{}}'\'''
    printf '%s\n' '    fi'
    printf '%s\n' '    ;;'
    printf '%s\n' '  "doctor --json")'
    printf '%s\n' '    if [ "${AOPMEM_STUB_DOCTOR_UNHEALTHY:-0}" = "1" ]; then'
    printf '%s\n' '      printf "%s\n" '\''{"ok":true,"data":{"healthy":false}}'\'''
    printf '%s\n' '    else'
    printf '%s\n' '      printf "%s\n" '\''{"ok":true,"data":{"healthy":true}}'\'''
    printf '%s\n' '    fi'
    printf '%s\n' '    ;;'
    printf '%s\n' '  "verify --json")'
    printf '%s\n' '    printf "%s\n" '\''{"ok":true,"data":{"clean":true}}'\'''
    printf '%s\n' '    ;;'
    printf '%s\n' '  *)'
    printf '%s\n' '    printf "%s\n" "unexpected stub command: $*" >&2'
    printf '%s\n' '    exit 2'
    printf '%s\n' '    ;;'
    printf '%s\n' 'esac'
  } > "$destination"
  chmod 755 "$destination"
}

write_old_stub() {
  destination=$1
  {
    printf '%s\n' '#!/bin/sh'
    printf '%s\n' 'set -eu'
    printf '%s\n' 'if [ "$*" = "--version" ]; then'
    printf '%s\n' '  printf "%s\n" "aopmem 0.1.0"'
    printf '%s\n' '  exit 0'
    printf '%s\n' 'fi'
    printf '%s\n' 'exit 2'
  } > "$destination"
  chmod 755 "$destination"
}

setup_case() {
  case_name=$1
  TEST_OLD_SHA=""
  CASE_ROOT="$TEST_ROOT/$case_name"
  ASSET_DIR="$CASE_ROOT/assets"
  TEMP_PARENT="$CASE_ROOT/temp"
  AOPMEM_HOME_PATH="$CASE_ROOT/home"
  REPO_DIR="$CASE_ROOT/repo"
  TRACE_PATH="$CASE_ROOT/trace.log"
  STDOUT_PATH="$CASE_ROOT/stdout.log"
  STDERR_PATH="$CASE_ROOT/stderr.log"
  mkdir -p "$ASSET_DIR" "$TEMP_PARENT" "$REPO_DIR"
  write_new_stub "$ASSET_DIR/aopmem-darwin-arm64"
  write_valid_sums
}

write_valid_sums() {
  asset_hash=$(shasum -a 256 "$ASSET_DIR/aopmem-darwin-arm64" \
    | awk '{ print tolower($1) }')
  printf '%s  %s\n' "$asset_hash" "aopmem-darwin-arm64" \
    > "$ASSET_DIR/SHA256SUMS"
}

install_old_binary() {
  mkdir -p "$AOPMEM_HOME_PATH/bin"
  write_old_stub "$AOPMEM_HOME_PATH/bin/aopmem"
  TEST_OLD_SHA=$(shasum -a 256 "$AOPMEM_HOME_PATH/bin/aopmem" \
    | awk '{ print tolower($1) }')
}

run_installer() {
  (
    cd "$REPO_DIR"
    env \
      AOPMEM_INSTALL_TEST_MODE=1 \
      AOPMEM_INSTALL_TEST_OS=Darwin \
      AOPMEM_INSTALL_TEST_ARCH=arm64 \
      AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSET_DIR" \
      AOPMEM_INSTALL_TEST_TEMP_ROOT="$TEMP_PARENT" \
      AOPMEM_INSTALL_TEST_TRACE="$TRACE_PATH" \
      AOPMEM_STUB_TRACE="$TRACE_PATH" \
      AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256="${TEST_OLD_SHA:-}" \
      AOPMEM_HOME="$AOPMEM_HOME_PATH" \
      "$@" \
      sh "$MAC_INSTALLER"
  ) > "$STDOUT_PATH" 2> "$STDERR_PATH"
}

expect_success() {
  run_installer "$@" || {
    sed -n '1,120p' "$STDERR_PATH" >&2
    fail "installer was expected to succeed"
  }
}

expect_failure() {
  if run_installer "$@"; then
    fail "installer was expected to fail"
  fi
}

run_static_audit() {
  assert_file "$MAC_INSTALLER"
  assert_file "$WINDOWS_INSTALLER"
  sh -n "$MAC_INSTALLER"

  assert_contains "$MAC_INSTALLER" 'aopmem-darwin-arm64'
  assert_contains "$MAC_INSTALLER" "--proto '=https'"
  assert_contains "$MAC_INSTALLER" "--proto-redir '=https'"
  assert_contains "$MAC_INSTALLER" '--tlsv1\.2'
  assert_contains "$MAC_INSTALLER" 'shasum -a 256'
  assert_contains "$MAC_INSTALLER" 'upgrade plan --all-workspaces --json'
  assert_contains "$MAC_INSTALLER" 'upgrade apply --all-workspaces --json'
  assert_contains "$MAC_INSTALLER" 'adapter seed --json'
  assert_contains "$MAC_INSTALLER" 'AOPMem home must not be a symbolic link'
  assert_contains "$MAC_INSTALLER" 'recovery binary path already exists'
  assert_contains "$MAC_INSTALLER" 'must not contain credentials, query, fragment'
  assert_contains "$MAC_INSTALLER" 'asset base URI has an empty host'

  assert_contains "$WINDOWS_INSTALLER" 'aopmem-windows-x86_64\.exe'
  assert_contains "$WINDOWS_INSTALLER" 'Net\.SecurityProtocolType.*Tls12'
  assert_contains "$WINDOWS_INSTALLER" 'chcp\.com.*65001'
  assert_contains "$WINDOWS_INSTALLER" 'Console.*InputEncoding'
  assert_contains "$WINDOWS_INSTALLER" 'Console.*OutputEncoding'
  assert_contains "$WINDOWS_INSTALLER" 'Invoke-WebRequest'
  assert_contains "$WINDOWS_INSTALLER" 'UseBasicParsing'
  assert_contains "$WINDOWS_INSTALLER" 'MaximumRedirection 0'
  assert_contains "$WINDOWS_INSTALLER" 'Assert-TrustedHttpsUri -Uri .nextUri'
  assert_contains "$WINDOWS_INSTALLER" 'Scheme -cne "https"'
  assert_contains "$WINDOWS_INSTALLER" 'parsedBase\.Query'
  assert_contains "$WINDOWS_INSTALLER" 'parsedBase\.Fragment'
  assert_contains "$WINDOWS_INSTALLER" 'Get-FileHash'
  assert_contains "$WINDOWS_INSTALLER" 'File.*Replace'
  assert_contains "$WINDOWS_INSTALLER" 'IsReparsePoint'
  assert_contains "$WINDOWS_INSTALLER" 'upgrade", "plan", "--all-workspaces", "--json"'
  assert_contains "$WINDOWS_INSTALLER" 'upgrade", "apply", "--all-workspaces", "--json"'
  assert_contains "$WINDOWS_INSTALLER" 'adapter", "seed", "--json"'
  assert_contains "$WINDOWS_INSTALLER" 'failure JSON report'
  assert_contains "$MAC_INSTALLER" 'emit_upgrade_report'

  assert_not_contains "$WINDOWS_INSTALLER" '&&'
  assert_not_contains "$WINDOWS_INSTALLER" '\?\?'
  assert_not_contains "$WINDOWS_INSTALLER" '\?\.'
  assert_not_contains "$WINDOWS_INSTALLER" 'ForEach-Object[[:space:]]+-Parallel'
  assert_not_contains "$WINDOWS_INSTALLER" 'Start-ThreadJob'
  assert_not_contains "$WINDOWS_INSTALLER" '(^|[^[:alnum:]_])class[[:space:]]+[[:alnum:]_]+'
  assert_not_contains "$WINDOWS_INSTALLER" 'wsl(\.exe)?'
  assert_not_contains "$WINDOWS_INSTALLER" 'Start-Process.*RunAs'
  assert_not_contains "$WINDOWS_INSTALLER" 'Set-ExecutionPolicy'
  assert_not_contains "$WINDOWS_INSTALLER" 'Invoke-Expression'
  assert_not_contains "$WINDOWS_INSTALLER" 'cmd\.exe'
  assert_not_contains "$WINDOWS_INSTALLER" 'cargo|rustup|git clone|Codex CLI|node(\.exe)?'
  assert_not_contains "$WINDOWS_INSTALLER" 'runas(\.exe)?|IsInRole.*Administrator|elevat'
  assert_not_contains "$MAC_INSTALLER" 'sudo|cargo|rustup|git clone|Codex CLI|node'

  mac_tag_hash=$(git -C "$REPO_ROOT" \
    show 'v0.1.0-rc3:dist/aopmem-darwin-arm64/aopmem' \
    | shasum -a 256 | awk '{ print tolower($1) }')
  windows_tag_hash=$(git -C "$REPO_ROOT" \
    show 'v0.1.0-rc3:dist/aopmem-windows-x86_64/aopmem.exe' \
    | shasum -a 256 | awk '{ print tolower($1) }')
  assert_equal \
    "$mac_tag_hash" \
    "d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607" \
    "tagged macOS v0.1.0-rc3 asset hash"
  assert_equal \
    "$windows_tag_hash" \
    "01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a" \
    "tagged Windows v0.1.0-rc3 asset hash"
  pass
}

test_fresh_success() {
  setup_case fresh-success
  expect_success
  assert_file "$AOPMEM_HOME_PATH/bin/aopmem"
  assert_contains "$TRACE_PATH" '^init$'
  assert_contains "$TRACE_PATH" '^adapter\.seed$'
  assert_contains "$TRACE_PATH" '^doctor$'
  assert_contains "$TRACE_PATH" '^verify$'
  assert_not_contains "$TRACE_PATH" '^upgrade\.plan$'
  assert_trace_order "$TRACE_PATH" "sha256.verified" "binary.version.verified"
  assert_trace_order "$TRACE_PATH" "binary.version.verified" "replacement.published"
  assert_trace_order "$TRACE_PATH" "init" "adapter.seed"
  assert_trace_order "$TRACE_PATH" "adapter.seed" "doctor"
  assert_temp_clean "$TEMP_PARENT"
  recovery=$(find "$AOPMEM_HOME_PATH/bin" -name '*recovery*' -print -quit)
  [ -z "$recovery" ] || fail "fresh success left a recovery binary"
  pass
}

test_fresh_adapter_and_health_contract_failures() {
  setup_case fresh-adapter-failure
  expect_failure AOPMEM_STUB_ADAPTER_FAIL=1
  assert_contains "$TRACE_PATH" '^adapter\.seed$'
  assert_not_contains "$TRACE_PATH" '^doctor$'
  assert_contains "$STDERR_PATH" 'adapter seed did not report success'
  assert_temp_clean "$TEMP_PARENT"

  setup_case fresh-unhealthy-doctor
  expect_failure AOPMEM_STUB_DOCTOR_UNHEALTHY=1
  assert_contains "$TRACE_PATH" '^adapter\.seed$'
  assert_contains "$TRACE_PATH" '^doctor$'
  assert_not_contains "$TRACE_PATH" '^verify$'
  assert_contains "$STDERR_PATH" 'doctor did not report healthy state'
  assert_temp_clean "$TEMP_PARENT"
  pass
}

test_update_success() {
  setup_case update-success
  install_old_binary
  expect_success
  assert_contains "$TRACE_PATH" '^upgrade\.plan$'
  assert_contains "$TRACE_PATH" '^upgrade\.apply$'
  assert_contains "$TRACE_PATH" '^upgrade\.apply\.health\.ok$'
  assert_not_contains "$TRACE_PATH" '^init$'
  assert_not_contains "$TRACE_PATH" '^doctor$'
  assert_not_contains "$TRACE_PATH" '^verify$'
  assert_trace_order "$TRACE_PATH" "upgrade.plan" "upgrade.apply"
  assert_trace_order "$TRACE_PATH" "upgrade.apply.health.ok" "replacement.published"
  backup=$(find "$AOPMEM_HOME_PATH/bin" -name 'aopmem.backup-v0.1.0-rc3-*' -print -quit)
  assert_file "$backup"
  expected=$(shasum -a 256 "$ASSET_DIR/aopmem-darwin-arm64" | awk '{ print $1 }')
  actual=$(shasum -a 256 "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  assert_equal "$actual" "$expected" "published update hash"
  assert_temp_clean "$TEMP_PARENT"
  recovery=$(find "$AOPMEM_HOME_PATH/bin" -name '*recovery*' -print -quit)
  [ -z "$recovery" ] || fail "update success left a recovery binary"
  pass
}

test_tagged_source_acceptance_and_hash_binding() {
  setup_case tagged-source
  mkdir -p "$AOPMEM_HOME_PATH/bin"
  git -C "$REPO_ROOT" \
    show 'v0.1.0-rc3:dist/aopmem-darwin-arm64/aopmem' \
    > "$AOPMEM_HOME_PATH/bin/aopmem"
  chmod 755 "$AOPMEM_HOME_PATH/bin/aopmem"
  TEST_OLD_SHA=$(shasum -a 256 "$AOPMEM_HOME_PATH/bin/aopmem" \
    | awk '{ print tolower($1) }')
  expect_success
  assert_contains "$TRACE_PATH" '^upgrade\.apply$'
  assert_not_contains "$TRACE_PATH" '^init$'

  setup_case wrong-old-hash
  install_old_binary
  expect_failure \
    AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256=ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
  assert_contains "$STDERR_PATH" 'not the supported v0.1.0-rc3 release asset'
  pass
}

test_checksum_failures() {
  setup_case checksum-mismatch
  printf '%064d  %s\n' 0 "aopmem-darwin-arm64" > "$ASSET_DIR/SHA256SUMS"
  expect_failure
  assert_not_contains "$TRACE_PATH" 'binary\.version\.verified'
  assert_contains "$STDERR_PATH" 'SHA-256 mismatch'
  assert_temp_clean "$TEMP_PARENT"

  setup_case checksum-duplicate
  checksum_line=$(sed -n '1p' "$ASSET_DIR/SHA256SUMS")
  printf '%s\n%s\n' "$checksum_line" "$checksum_line" > "$ASSET_DIR/SHA256SUMS"
  expect_failure
  assert_contains "$STDERR_PATH" 'no unique exact entry'
  assert_temp_clean "$TEMP_PARENT"

  setup_case checksum-wrong-name
  asset_hash=$(shasum -a 256 "$ASSET_DIR/aopmem-darwin-arm64" \
    | awk '{ print $1 }')
  printf '%s  %s\n' "$asset_hash" "./aopmem-darwin-arm64" \
    > "$ASSET_DIR/SHA256SUMS"
  expect_failure
  assert_contains "$STDERR_PATH" 'no unique exact entry'
  assert_temp_clean "$TEMP_PARENT"

  setup_case checksum-malformed-exact
  valid_line=$(sed -n '1p' "$ASSET_DIR/SHA256SUMS")
  printf '%s\n%s\n' \
    "$valid_line" \
    "not-a-sha256  aopmem-darwin-arm64" \
    > "$ASSET_DIR/SHA256SUMS"
  expect_failure
  assert_contains "$STDERR_PATH" 'no unique exact entry'
  assert_temp_clean "$TEMP_PARENT"

  setup_case wrong-version
  write_old_stub "$ASSET_DIR/aopmem-darwin-arm64"
  write_valid_sums
  expect_failure
  assert_contains "$STDERR_PATH" 'unexpected version'
  assert_temp_clean "$TEMP_PARENT"
  pass
}

test_asset_base_uri_rejections() {
  for bad_uri in \
    'http://example.invalid/release' \
    'https://user@example.invalid/release' \
    'https:///release' \
    'https://example.invalid/release path' \
    'https://example.invalid/release?token=secret' \
    'https://example.invalid/release#fragment'
  do
    case_name=$(printf '%s' "$bad_uri" | shasum -a 256 | cut -c1-12)
    case_root="$TEST_ROOT/uri-$case_name"
    mkdir -p "$case_root/home" "$case_root/temp"
    if env \
      HOME="$case_root/home" \
      TMPDIR="$case_root/temp" \
      AOPMEM_ASSET_BASE_URI="$bad_uri" \
      sh "$MAC_INSTALLER" \
      > "$case_root/stdout.log" 2> "$case_root/stderr.log"; then
      fail "unsafe asset base URI was accepted: $bad_uri"
    fi
    assert_contains "$case_root/stderr.log" 'asset base URI'
    assert_temp_clean "$case_root/temp"
  done
  pass
}

test_path_rejections() {
  setup_case home-symlink
  mkdir "$CASE_ROOT/home-target"
  ln -s "$CASE_ROOT/home-target" "$AOPMEM_HOME_PATH"
  expect_failure
  assert_contains "$STDERR_PATH" 'home must not be a symbolic link'

  setup_case bin-symlink
  mkdir -p "$AOPMEM_HOME_PATH" "$CASE_ROOT/bin-target"
  ln -s "$CASE_ROOT/bin-target" "$AOPMEM_HOME_PATH/bin"
  expect_failure
  assert_contains "$STDERR_PATH" 'bin directory must not be a symbolic link'

  setup_case binary-symlink
  mkdir -p "$AOPMEM_HOME_PATH/bin"
  write_old_stub "$CASE_ROOT/old-target"
  ln -s "$CASE_ROOT/old-target" "$AOPMEM_HOME_PATH/bin/aopmem"
  expect_failure
  assert_contains "$STDERR_PATH" 'not a regular file'

  setup_case temp-parent-symlink
  rm -rf "$TEMP_PARENT"
  mkdir "$CASE_ROOT/temp-target"
  ln -s "$CASE_ROOT/temp-target" "$TEMP_PARENT"
  expect_failure
  assert_contains "$STDERR_PATH" 'temporary parent must be a real directory'

  setup_case recovery-destination-symlink
  install_old_binary
  recovery_path="$AOPMEM_HOME_PATH/bin/aopmem-v0.2.0-rc1-recovery-424242"
  ln -s "$CASE_ROOT/outside-recovery-target" "$recovery_path"
  expect_failure AOPMEM_INSTALL_TEST_RUN_ID=424242
  [ -L "$recovery_path" ] || fail "installer removed pre-existing recovery symlink"
  assert_contains "$STDERR_PATH" 'recovery binary path already exists'
  pass
}

test_pre_apply_failures_leave_binary_unchanged() {
  setup_case plan-failure
  install_old_binary
  old_inode=$(ls -di "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  old_hash=$(shasum -a 256 "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  expect_failure AOPMEM_STUB_PLAN_NOT_READY=1
  new_inode=$(ls -di "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  new_hash=$(shasum -a 256 "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  assert_equal "$new_inode" "$old_inode" "plan failure inode"
  assert_equal "$new_hash" "$old_hash" "plan failure bytes"
  assert_contains "$TRACE_PATH" '^rollback\.unchanged$'
  backup=$(find "$AOPMEM_HOME_PATH/bin" -name 'aopmem.backup-*' -print -quit)
  assert_file "$backup"
  recovery=$(find "$AOPMEM_HOME_PATH/bin" -name '*recovery*' -print -quit)
  [ -z "$recovery" ] || fail "plan failure retained recovery before apply"
  assert_temp_clean "$TEMP_PARENT"

  setup_case trap-after-backup
  install_old_binary
  old_inode=$(ls -di "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  expect_failure AOPMEM_INSTALL_TEST_FAIL_AT=after_backup
  new_inode=$(ls -di "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  assert_equal "$new_inode" "$old_inode" "trap failure inode"
  assert_contains "$TRACE_PATH" '^rollback\.unchanged$'
  assert_temp_clean "$TEMP_PARENT"

  setup_case backup-failure
  install_old_binary
  old_hash=$(shasum -a 256 "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  expect_failure AOPMEM_INSTALL_TEST_FAIL_AT=backup
  new_hash=$(shasum -a 256 "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  assert_equal "$new_hash" "$old_hash" "backup failure bytes"
  backup=$(find "$AOPMEM_HOME_PATH/bin" -name 'aopmem.backup-*' -print -quit)
  [ -z "$backup" ] || fail "injected backup failure created a backup"
  assert_temp_clean "$TEMP_PARENT"
  pass
}

test_post_apply_failures_preserve_recovery() {
  setup_case apply-failure
  install_old_binary
  old_inode=$(ls -di "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  expect_failure AOPMEM_STUB_APPLY_FAIL=1
  new_inode=$(ls -di "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  assert_equal "$new_inode" "$old_inode" "apply failure inode"
  recovery=$(find "$AOPMEM_HOME_PATH/bin" -name '*recovery*' -print -quit)
  assert_file "$recovery"
  assert_contains "$STDERR_PATH" 'do not run the v0.1 binary'
  assert_contains "$STDERR_PATH" 'recovery binary preserved at'
  assert_contains "$STDERR_PATH" 'workspace=fixture-workspace'
  assert_contains "$STDERR_PATH" 'code=FIXTURE_APPLY_FAILED'
  assert_contains "$STDERR_PATH" '"stopped_workspace":"fixture-workspace"'
  assert_temp_clean "$TEMP_PARENT"

  setup_case publish-failure
  install_old_binary
  old_inode=$(ls -di "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  expect_failure AOPMEM_INSTALL_TEST_FAIL_AT=publish
  new_inode=$(ls -di "$AOPMEM_HOME_PATH/bin/aopmem" | awk '{ print $1 }')
  assert_equal "$new_inode" "$old_inode" "publish failure inode"
  recovery=$(find "$AOPMEM_HOME_PATH/bin" -name '*recovery*' -print -quit)
  assert_file "$recovery"
  expected=$(shasum -a 256 "$ASSET_DIR/aopmem-darwin-arm64" | awk '{ print $1 }')
  actual=$(shasum -a 256 "$recovery" | awk '{ print $1 }')
  assert_equal "$actual" "$expected" "publish failure recovery hash"
  assert_contains "$STDERR_PATH" "$recovery"
  assert_temp_clean "$TEMP_PARENT"
  pass
}

test_fresh_doctor_failure_and_platform_rejection() {
  setup_case fresh-doctor-failure
  expect_failure AOPMEM_INSTALL_TEST_FAIL_AT=doctor
  assert_file "$AOPMEM_HOME_PATH/bin/aopmem"
  assert_contains "$TRACE_PATH" '^init$'
  assert_not_contains "$TRACE_PATH" '^verify$'
  recovery=$(find "$AOPMEM_HOME_PATH/bin" -name '*recovery*' -print -quit)
  [ -z "$recovery" ] || fail "fresh doctor failure left duplicate recovery"
  assert_temp_clean "$TEMP_PARENT"

  setup_case unsupported-platform
  if (
    cd "$REPO_DIR"
    env \
      AOPMEM_INSTALL_TEST_MODE=1 \
      AOPMEM_INSTALL_TEST_OS=Linux \
      AOPMEM_INSTALL_TEST_ARCH=x86_64 \
      AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSET_DIR" \
      AOPMEM_INSTALL_TEST_TEMP_ROOT="$TEMP_PARENT" \
      AOPMEM_HOME="$AOPMEM_HOME_PATH" \
      sh "$MAC_INSTALLER"
  ) > "$STDOUT_PATH" 2> "$STDERR_PATH"; then
    fail "unsupported platform was accepted"
  fi
  assert_contains "$STDERR_PATH" 'unsupported platform'
  pass
}

run_static_audit
test_fresh_success
test_fresh_adapter_and_health_contract_failures
test_update_success
test_tagged_source_acceptance_and_hash_binding
test_checksum_failures
test_asset_base_uri_rejections
test_path_rejections
test_pre_apply_failures_leave_binary_unchanged
test_post_apply_failures_preserve_recovery
test_fresh_doctor_failure_and_platform_rejection

printf '%s\n' "v0.2 installer audit passed: $TEST_COUNT groups"
