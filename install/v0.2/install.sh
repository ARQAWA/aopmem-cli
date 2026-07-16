#!/bin/sh

set -eu
umask 077

PRODUCT_VERSION="0.2.0-rc2"
OLD_RELEASE_LABEL="0.1.0-rc3"
OLD_BINARY_VERSION="0.1.0"
OLD_BINARY_SHA256="d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607"
ASSET_NAME="aopmem-darwin-arm64"
CHECKSUM_NAME="SHA256SUMS"
TEST_MODE="${AOPMEM_INSTALL_TEST_MODE:-0}"
TEST_FAIL_AT="${AOPMEM_INSTALL_TEST_FAIL_AT:-}"
TEMP_ROOT=""
INSTALL_STAGE=""
INSTALL_STAGE_OWNED="0"
RECOVERY_BINARY=""
RECOVERY_BINARY_OWNED="0"
BACKUP_PATH=""
UPGRADE_BACKUP_ROOT=""
BACKUP_READY="0"
APPLY_ATTEMPTED="0"
BINARY_PUBLISHED="0"
PRESERVE_RECOVERY="0"
MODE="unknown"
FAILURE_MESSAGE=""
INSTALL_RUN_ID=$$

trace_install_event() {
  if [ "$TEST_MODE" = "1" ] && [ -n "${AOPMEM_INSTALL_TEST_TRACE:-}" ]; then
    printf '%s\n' "$1" >> "$AOPMEM_INSTALL_TEST_TRACE"
  fi
}

cleanup_temporary_files() {
  if [ -n "$INSTALL_STAGE" ] && [ "$INSTALL_STAGE_OWNED" = "1" ]; then
    rm -f "$INSTALL_STAGE"
  fi
  if [ -n "$RECOVERY_BINARY" ] && [ "$RECOVERY_BINARY_OWNED" = "1" ] \
    && [ "$PRESERVE_RECOVERY" != "1" ]; then
    rm -f "$RECOVERY_BINARY"
  fi
  if [ -n "$TEMP_ROOT" ]; then
    rm -rf "$TEMP_ROOT"
  fi
}

finish_install_process() {
  exit_status=$1
  trap - EXIT HUP INT TERM
  if [ "$exit_status" -ne 0 ] && [ "$MODE" = "update" ] \
    && [ "$APPLY_ATTEMPTED" = "1" ]; then
    PRESERVE_RECOVERY="1"
    if [ -z "$FAILURE_MESSAGE" ]; then
      FAILURE_MESSAGE="unexpected failure after upgrade apply started"
    fi
    printf '%s\n' \
      "AOPMem install failed: $FAILURE_MESSAGE; upgrade apply may have committed v0.2 data; do not run the v0.1 binary; verified v0.2 recovery binary preserved at $RECOVERY_BINARY; binary backup preserved at $BACKUP_PATH; upgrade backup preserved at ${UPGRADE_BACKUP_ROOT:-reported by upgrade apply}" >&2
  elif [ "$exit_status" -ne 0 ] && [ "$MODE" = "update" ] \
    && [ "$BACKUP_READY" = "1" ] && [ "$BINARY_PUBLISHED" = "0" ]; then
    if [ -z "$FAILURE_MESSAGE" ]; then
      FAILURE_MESSAGE="unexpected failure before upgrade apply completed"
    fi
    if verify_old_binary_unchanged; then
      printf '%s\n' \
        "AOPMem install failed: $FAILURE_MESSAGE; old binary was left unchanged; backup preserved at $BACKUP_PATH" >&2
    else
      printf '%s\n' \
        "AOPMem install failed: $FAILURE_MESSAGE; old binary changed unexpectedly; backup preserved at $BACKUP_PATH" >&2
    fi
  elif [ "$exit_status" -ne 0 ]; then
    if [ -z "$FAILURE_MESSAGE" ]; then
      FAILURE_MESSAGE="unexpected installer failure"
    fi
    printf '%s\n' \
      "AOPMem install failed: $FAILURE_MESSAGE; workspace data was preserved" >&2
  fi
  cleanup_temporary_files
  exit "$exit_status"
}

trap 'finish_install_process $?' EXIT
trap 'exit 130' HUP INT TERM

copy_file_durable() {
  source_path=$1
  destination_path=$2
  cp -p -n "$source_path" "$destination_path" || return 1
  sync || return 1
}

validate_regular_file() {
  file_path=$1
  file_label=$2
  if [ ! -f "$file_path" ] || [ -L "$file_path" ]; then
    fail_install "$file_label is not a real regular file: $file_path"
  fi
}

sha256_file() {
  shasum -a 256 "$1" | awk '{ print tolower($1) }'
}

verify_old_binary_unchanged() {
  if [ "$MODE" != "update" ] || [ -z "$BACKUP_PATH" ] \
    || [ ! -f "$BACKUP_PATH" ] || [ ! -f "$INSTALLED_BINARY" ] \
    || [ -L "$INSTALLED_BINARY" ]; then
    return 1
  fi
  current_hash=$(sha256_file "$INSTALLED_BINARY") || return 1
  if [ "$current_hash" != "$ORIGINAL_BINARY_HASH" ]; then
    return 1
  fi
  trace_install_event "rollback.unchanged"
}

fail_install() {
  FAILURE_MESSAGE=$1
  exit 1
}

json_field_or_empty() {
  json_path=$1
  key_path=$2
  if [ ! -x /usr/bin/plutil ]; then
    return 0
  fi
  /usr/bin/plutil -extract "$key_path" raw -o - "$json_path" 2>/dev/null \
    | tr '\r\n' '  ' || true
}

emit_upgrade_report() {
  report_context=$1
  report_path=$2
  printf '%s\n' "AOPMem $report_context JSON report:" >&2
  while IFS= read -r report_line || [ -n "$report_line" ]; do
    printf '%s\n' "$report_line" >&2
  done < "$report_path"
}

set_upgrade_report_failure() {
  report_context=$1
  report_path=$2
  report_code=$(json_field_or_empty "$report_path" "errors.0.code")
  report_message=$(json_field_or_empty "$report_path" "errors.0.message")
  report_workspace=$(json_field_or_empty "$report_path" "data.stopped_workspace")
  report_backup=$(json_field_or_empty "$report_path" "data.backup_root")
  if [ -n "$report_backup" ] && [ "$report_backup" != "null" ]; then
    UPGRADE_BACKUP_ROOT=$report_backup
  fi
  FAILURE_MESSAGE="$report_context failed"
  if [ -n "$report_workspace" ] && [ "$report_workspace" != "null" ]; then
    FAILURE_MESSAGE="$FAILURE_MESSAGE: workspace=$report_workspace"
  fi
  if [ -n "$report_code" ] && [ "$report_code" != "null" ]; then
    FAILURE_MESSAGE="$FAILURE_MESSAGE code=$report_code"
  fi
  if [ -n "$report_message" ] && [ "$report_message" != "null" ]; then
    FAILURE_MESSAGE="$FAILURE_MESSAGE message=$report_message"
  fi
}

validate_asset_base_uri() {
  asset_base_uri=$1
  case "$asset_base_uri" in
    https://*) ;;
    "") fail_install "trusted HTTPS asset base URI is missing from release context" ;;
    *) fail_install "asset base URI must use HTTPS" ;;
  esac
  case "$asset_base_uri" in
    *"@"*|*"?"*|*"#"*|*[[:space:]]*)
      fail_install "asset base URI must not contain credentials, query, fragment, or whitespace"
      ;;
  esac
  asset_authority=${asset_base_uri#https://}
  case "$asset_authority" in
    ""|/*|:*) fail_install "asset base URI has an empty host" ;;
  esac
}

download_asset() {
  asset_name=$1
  destination=$2
  if [ "$TEST_MODE" = "1" ]; then
    test_asset_dir=${AOPMEM_INSTALL_TEST_ASSET_DIR:-}
    if [ -z "$test_asset_dir" ] || [ ! -f "$test_asset_dir/$asset_name" ] \
      || [ -L "$test_asset_dir/$asset_name" ]; then
      fail_install "local test asset missing: $asset_name"
    fi
    cp "$test_asset_dir/$asset_name" "$destination" \
      || fail_install "local test asset copy failed: $asset_name"
    return 0
  fi

  asset_base_uri=${AOPMEM_ASSET_BASE_URI:-}
  validate_asset_base_uri "$asset_base_uri"
  asset_base_uri=${asset_base_uri%/}
  curl --fail --show-error --silent --location \
    --proto '=https' --proto-redir '=https' --tlsv1.2 \
    --connect-timeout 30 --max-time 900 \
    "$asset_base_uri/$asset_name" \
    --output "$destination" || fail_install "download failed for $asset_name"
}

detect_supported_platform() {
  if [ "$TEST_MODE" = "1" ]; then
    detected_os=${AOPMEM_INSTALL_TEST_OS:-$(uname -s)}
    detected_arch=${AOPMEM_INSTALL_TEST_ARCH:-$(uname -m)}
  else
    detected_os=$(uname -s)
    detected_arch=$(uname -m)
  fi
  if [ "$detected_os" != "Darwin" ] || [ "$detected_arch" != "arm64" ]; then
    fail_install "unsupported platform: $detected_os/$detected_arch"
  fi
}

verify_checksum_entry() {
  sums_path=$1
  binary_path=$2
  exact_name=$3
  expected_hash=$(awk -v exact_name="$exact_name" '
    {
      referenced = 0
      for (field = 1; field <= NF; field += 1) {
        if ($field == exact_name) {
          referenced = 1
        }
      }
      if (referenced) {
        count += 1
        if (NF == 2 && $2 == exact_name && length($1) == 64 &&
            $1 !~ /[^0-9A-Fa-f]/) {
          hash = tolower($1)
        } else {
          malformed = 1
        }
      }
    }
    END {
      if (count == 1 && malformed == 0 && hash != "") {
        print hash
      } else {
        exit 1
      }
    }
  ' "$sums_path") || fail_install "SHA256SUMS has no unique exact entry for $exact_name"
  actual_hash=$(sha256_file "$binary_path")
  if [ "$expected_hash" != "$actual_hash" ]; then
    fail_install "SHA-256 mismatch for $exact_name"
  fi
  VERIFIED_BINARY_HASH=$actual_hash
  trace_install_event "sha256.verified"
}

verify_new_binary_version() {
  if ! version_output=$("$DOWNLOADED_BINARY" --version 2>/dev/null); then
    fail_install "verified binary version check failed"
  fi
  if [ "$version_output" != "aopmem $PRODUCT_VERSION" ]; then
    fail_install "verified binary has unexpected version: $version_output"
  fi
  trace_install_event "binary.version.verified"
}

backup_old_binary() {
  if [ "$TEST_MODE" = "1" ] && [ "$TEST_FAIL_AT" = "backup" ]; then
    fail_install "injected old binary backup failure"
  fi
  backup_stamp=$(date -u '+%Y%m%dT%H%M%SZ')
  BACKUP_PATH="$INSTALL_DIR/aopmem.backup-v${OLD_RELEASE_LABEL}-${backup_stamp}-$$"
  if [ -e "$BACKUP_PATH" ] || [ -L "$BACKUP_PATH" ]; then
    fail_install "binary backup path already exists: $BACKUP_PATH"
  fi
  ORIGINAL_BINARY_HASH=$(sha256_file "$INSTALLED_BINARY")
  copy_file_durable "$INSTALLED_BINARY" "$BACKUP_PATH"
  validate_regular_file "$BACKUP_PATH" "old binary backup"
  backup_hash=$(sha256_file "$BACKUP_PATH")
  if [ "$ORIGINAL_BINARY_HASH" != "$backup_hash" ]; then
    fail_install "old binary backup verification failed: $BACKUP_PATH"
  fi
  BACKUP_READY="1"
  trace_install_event "backup.created"
}

run_upgrade_plan() {
  plan_output="$TEMP_ROOT/upgrade-plan.json"
  trace_install_event "upgrade.plan"
  if ! AOPMEM_HOME="$AOPMEM_HOME_PATH" "$DOWNLOADED_BINARY" \
    upgrade plan --all-workspaces --json > "$plan_output"; then
    emit_upgrade_report "upgrade plan failure" "$plan_output"
    set_upgrade_report_failure "upgrade plan" "$plan_output"
    fail_install "$FAILURE_MESSAGE"
  fi
  if ! grep -Eq '"ok"[[:space:]]*:[[:space:]]*true' "$plan_output" \
    || ! grep -Eq '"ready"[[:space:]]*:[[:space:]]*true' "$plan_output" \
    || ! grep -Eq '"writes_performed"[[:space:]]*:[[:space:]]*false' "$plan_output" \
    || grep -Eq '"ok"[[:space:]]*:[[:space:]]*false' "$plan_output" \
    || grep -Eq '"ready"[[:space:]]*:[[:space:]]*false' "$plan_output" \
    || grep -Eq '"writes_performed"[[:space:]]*:[[:space:]]*true' "$plan_output"; then
    emit_upgrade_report "upgrade plan not-ready" "$plan_output"
    fail_install "upgrade plan did not report ready read-only state"
  fi
}

run_upgrade_apply() {
  apply_output="$TEMP_ROOT/upgrade-apply.json"
  trace_install_event "upgrade.apply"
  APPLY_ATTEMPTED="1"
  if ! AOPMEM_HOME="$AOPMEM_HOME_PATH" "$DOWNLOADED_BINARY" \
    upgrade apply --all-workspaces --json > "$apply_output"; then
    emit_upgrade_report "upgrade apply failure" "$apply_output"
    set_upgrade_report_failure "upgrade apply" "$apply_output"
    fail_install "$FAILURE_MESSAGE"
  fi
  if ! grep -Eq '"ok"[[:space:]]*:[[:space:]]*true' "$apply_output" \
    || ! grep -Eq '"success"[[:space:]]*:[[:space:]]*true' "$apply_output" \
    || ! grep -Eq '"binary_replaced"[[:space:]]*:[[:space:]]*false' "$apply_output" \
    || grep -Eq '"ok"[[:space:]]*:[[:space:]]*false' "$apply_output" \
    || grep -Eq '"success"[[:space:]]*:[[:space:]]*false' "$apply_output" \
    || grep -Eq '"binary_replaced"[[:space:]]*:[[:space:]]*true' "$apply_output"; then
    emit_upgrade_report "upgrade apply unsuccessful" "$apply_output"
    fail_install "upgrade apply did not report success"
  fi
  UPGRADE_BACKUP_ROOT=$(json_field_or_empty "$apply_output" "data.backup_root")
  trace_install_event "upgrade.apply.health.ok"
}

prepare_publish_files() {
  INSTALL_STAGE="$INSTALL_DIR/.aopmem-v020-stage-$INSTALL_RUN_ID"
  RECOVERY_BINARY="$INSTALL_DIR/aopmem-v${PRODUCT_VERSION}-recovery-$INSTALL_RUN_ID"
  if [ -e "$INSTALL_STAGE" ] || [ -L "$INSTALL_STAGE" ]; then
    fail_install "binary stage path already exists: $INSTALL_STAGE"
  fi
  if [ -e "$RECOVERY_BINARY" ] || [ -L "$RECOVERY_BINARY" ]; then
    fail_install "recovery binary path already exists: $RECOVERY_BINARY"
  fi
  INSTALL_STAGE_OWNED="1"
  copy_file_durable "$DOWNLOADED_BINARY" "$INSTALL_STAGE" \
    || fail_install "same-directory binary staging failed"
  validate_regular_file "$INSTALL_STAGE" "same-directory binary stage"
  chmod 755 "$INSTALL_STAGE" \
    || fail_install "same-directory binary staging chmod failed"
  stage_hash=$(sha256_file "$INSTALL_STAGE")
  if [ "$stage_hash" != "$VERIFIED_BINARY_HASH" ]; then
    fail_install "same-directory binary staging verification failed"
  fi
  RECOVERY_BINARY_OWNED="1"
  copy_file_durable "$INSTALL_STAGE" "$RECOVERY_BINARY" \
    || fail_install "same-directory recovery binary staging failed"
  validate_regular_file "$RECOVERY_BINARY" "same-directory recovery binary"
  chmod 755 "$RECOVERY_BINARY" \
    || fail_install "same-directory recovery binary chmod failed"
  recovery_hash=$(sha256_file "$RECOVERY_BINARY")
  if [ "$recovery_hash" != "$VERIFIED_BINARY_HASH" ]; then
    fail_install "same-directory recovery binary verification failed"
  fi
  trace_install_event "replacement.staged"
}

publish_verified_binary() {
  if [ "$TEST_MODE" = "1" ] && [ "$TEST_FAIL_AT" = "publish" ]; then
    fail_install "injected atomic replacement failure"
  fi
  validate_regular_file "$INSTALL_STAGE" "same-directory binary stage"
  validate_regular_file "$RECOVERY_BINARY" "same-directory recovery binary"
  prepublish_stage_hash=$(sha256_file "$INSTALL_STAGE")
  prepublish_recovery_hash=$(sha256_file "$RECOVERY_BINARY")
  if [ "$prepublish_stage_hash" != "$VERIFIED_BINARY_HASH" ] \
    || [ "$prepublish_recovery_hash" != "$VERIFIED_BINARY_HASH" ]; then
    fail_install "same-directory publish files changed before atomic replacement"
  fi
  mv -f "$INSTALL_STAGE" "$INSTALLED_BINARY" \
    || fail_install "atomic binary replacement failed"
  INSTALL_STAGE=""
  INSTALL_STAGE_OWNED="0"
  BINARY_PUBLISHED="1"
  sync || fail_install "installed binary sync failed"
  validate_regular_file "$INSTALLED_BINARY" "installed binary"
  installed_hash=$(sha256_file "$INSTALLED_BINARY")
  if [ "$installed_hash" != "$VERIFIED_BINARY_HASH" ]; then
    fail_install "installed binary verification failed after atomic replacement"
  fi
  if ! installed_version=$("$INSTALLED_BINARY" --version 2>/dev/null); then
    fail_install "installed binary version check failed after atomic replacement"
  fi
  if [ "$installed_version" != "aopmem $PRODUCT_VERSION" ]; then
    fail_install "installed binary has unexpected version after atomic replacement: $installed_version"
  fi
  trace_install_event "replacement.published"
}

run_current_workspace_health() {
  doctor_output="$TEMP_ROOT/doctor.json"
  trace_install_event "doctor"
  if [ "$TEST_MODE" = "1" ] && [ "$TEST_FAIL_AT" = "doctor" ]; then
    fail_install "injected doctor failure"
  fi
  if ! AOPMEM_HOME="$AOPMEM_HOME_PATH" "$INSTALLED_BINARY" doctor --json > "$doctor_output"; then
    fail_install "doctor failed for current workspace"
  fi
  if ! grep -Eq '"ok"[[:space:]]*:[[:space:]]*true' "$doctor_output" \
    || ! grep -Eq '"healthy"[[:space:]]*:[[:space:]]*true' "$doctor_output" \
    || grep -Eq '"ok"[[:space:]]*:[[:space:]]*false' "$doctor_output" \
    || grep -Eq '"healthy"[[:space:]]*:[[:space:]]*false' "$doctor_output"; then
    fail_install "doctor did not report healthy state"
  fi
  verify_output="$TEMP_ROOT/verify.json"
  trace_install_event "verify"
  if ! AOPMEM_HOME="$AOPMEM_HOME_PATH" "$INSTALLED_BINARY" verify --json > "$verify_output"; then
    fail_install "verify failed for current workspace"
  fi
  if ! grep -Eq '"ok"[[:space:]]*:[[:space:]]*true' "$verify_output" \
    || ! grep -Eq '"clean"[[:space:]]*:[[:space:]]*true' "$verify_output" \
    || grep -Eq '"ok"[[:space:]]*:[[:space:]]*false' "$verify_output" \
    || grep -Eq '"clean"[[:space:]]*:[[:space:]]*false' "$verify_output"; then
    fail_install "verify did not report clean state"
  fi
}

run_fresh_init() {
  trace_install_event "init"
  if ! AOPMEM_HOME="$AOPMEM_HOME_PATH" "$INSTALLED_BINARY" init; then
    fail_install "fresh workspace semantic initialization failed"
  fi
}

run_fresh_adapter_seed() {
  adapter_output="$TEMP_ROOT/adapter-seed.json"
  trace_install_event "adapter.seed"
  if ! AOPMEM_HOME="$AOPMEM_HOME_PATH" "$INSTALLED_BINARY" \
    adapter seed --json > "$adapter_output"; then
    fail_install "fresh managed adapter seed failed"
  fi
  if ! grep -Eq '"ok"[[:space:]]*:[[:space:]]*true' "$adapter_output" \
    || grep -Eq '"ok"[[:space:]]*:[[:space:]]*false' "$adapter_output"; then
    fail_install "fresh managed adapter seed did not report success"
  fi
}

ensure_install_directories() {
  if [ -L "$AOPMEM_HOME_PATH" ]; then
    fail_install "AOPMem home must not be a symbolic link: $AOPMEM_HOME_PATH"
  fi
  if [ -e "$AOPMEM_HOME_PATH" ] && [ ! -d "$AOPMEM_HOME_PATH" ]; then
    fail_install "AOPMem home is not a directory: $AOPMEM_HOME_PATH"
  fi
  if [ ! -e "$AOPMEM_HOME_PATH" ]; then
    mkdir "$AOPMEM_HOME_PATH" \
      || fail_install "cannot create AOPMem home: $AOPMEM_HOME_PATH"
  fi
  if [ -L "$AOPMEM_HOME_PATH" ] || [ ! -d "$AOPMEM_HOME_PATH" ]; then
    fail_install "AOPMem home path changed during validation: $AOPMEM_HOME_PATH"
  fi

  if [ -L "$INSTALL_DIR" ]; then
    fail_install "AOPMem bin directory must not be a symbolic link: $INSTALL_DIR"
  fi
  if [ -e "$INSTALL_DIR" ] && [ ! -d "$INSTALL_DIR" ]; then
    fail_install "AOPMem bin path is not a directory: $INSTALL_DIR"
  fi
  if [ ! -e "$INSTALL_DIR" ]; then
    mkdir "$INSTALL_DIR" \
      || fail_install "cannot create AOPMem bin directory: $INSTALL_DIR"
  fi
  if [ -L "$INSTALL_DIR" ] || [ ! -d "$INSTALL_DIR" ]; then
    fail_install "AOPMem bin path changed during validation: $INSTALL_DIR"
  fi
}

if [ "$TEST_MODE" = "1" ] && [ -n "${AOPMEM_INSTALL_TEST_RUN_ID:-}" ]; then
  case "$AOPMEM_INSTALL_TEST_RUN_ID" in
    *[!0-9]*) fail_install "test run id must contain decimal digits only" ;;
    *) INSTALL_RUN_ID=$AOPMEM_INSTALL_TEST_RUN_ID ;;
  esac
fi

detect_supported_platform

if [ "$TEST_MODE" = "1" ]; then
  AOPMEM_HOME_PATH=${AOPMEM_HOME:-}
  TEMP_PARENT=${AOPMEM_INSTALL_TEST_TEMP_ROOT:-}
  if [ -z "$AOPMEM_HOME_PATH" ] || [ -z "$TEMP_PARENT" ]; then
    fail_install "test mode requires isolated AOPMEM_HOME and temp root"
  fi
  if [ -L "$TEMP_PARENT" ] || [ ! -d "$TEMP_PARENT" ]; then
    fail_install "test temporary parent must be a real directory: $TEMP_PARENT"
  fi
else
  AOPMEM_HOME_PATH=${HOME:?}/.aopmem
  TEMP_PARENT=${TMPDIR:-/tmp}
fi
INSTALL_DIR="$AOPMEM_HOME_PATH/bin"
INSTALLED_BINARY="$INSTALL_DIR/aopmem"
ensure_install_directories

TEMP_ROOT=$(mktemp -d "$TEMP_PARENT/aopmem-v020.XXXXXX") \
  || fail_install "temporary directory creation failed"
if [ -L "$TEMP_ROOT" ] || [ ! -d "$TEMP_ROOT" ]; then
  fail_install "temporary root is not a private directory: $TEMP_ROOT"
fi
chmod 700 "$TEMP_ROOT" || fail_install "temporary directory permission update failed"
DOWNLOADED_BINARY="$TEMP_ROOT/$ASSET_NAME"
DOWNLOADED_SUMS="$TEMP_ROOT/$CHECKSUM_NAME"

download_asset "$ASSET_NAME" "$DOWNLOADED_BINARY"
download_asset "$CHECKSUM_NAME" "$DOWNLOADED_SUMS"
verify_checksum_entry "$DOWNLOADED_SUMS" "$DOWNLOADED_BINARY" "$ASSET_NAME"
chmod 755 "$DOWNLOADED_BINARY"
verify_new_binary_version

if [ -e "$INSTALLED_BINARY" ] || [ -L "$INSTALLED_BINARY" ]; then
  if [ ! -f "$INSTALLED_BINARY" ] || [ -L "$INSTALLED_BINARY" ]; then
    fail_install "installed binary path is not a regular file: $INSTALLED_BINARY"
  fi
  if ! installed_version=$("$INSTALLED_BINARY" --version 2>/dev/null); then
    fail_install "existing binary version check failed: $INSTALLED_BINARY"
  fi
  if [ "$installed_version" != "aopmem $OLD_BINARY_VERSION" ]; then
    fail_install "existing version is unsupported for this installer: $installed_version"
  fi
  installed_old_hash=$(sha256_file "$INSTALLED_BINARY")
  expected_old_hash=$OLD_BINARY_SHA256
  if [ "$TEST_MODE" = "1" ]; then
    expected_old_hash=${AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256:-}
  fi
  if [ -z "$expected_old_hash" ] || [ "$installed_old_hash" != "$expected_old_hash" ]; then
    fail_install "existing binary is not the supported v${OLD_RELEASE_LABEL} release asset"
  fi
  MODE="update"
  backup_old_binary
  prepare_publish_files
  if [ "$TEST_MODE" = "1" ] && [ "$TEST_FAIL_AT" = "after_backup" ]; then
    false
  fi
  run_upgrade_plan
  run_upgrade_apply
  publish_verified_binary
  printf '%s\n' \
    "AOPMem $PRODUCT_VERSION updated. doctor=ok verify=ok binary_backup=$BACKUP_PATH upgrade_backup=${UPGRADE_BACKUP_ROOT:-none}"
else
  MODE="fresh"
  prepare_publish_files
  publish_verified_binary
  run_fresh_init
  run_fresh_adapter_seed
  run_current_workspace_health
  printf '%s\n' "AOPMem $PRODUCT_VERSION installed. doctor=ok verify=ok"
fi
