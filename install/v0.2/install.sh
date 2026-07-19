#!/bin/sh

set -eu
umask 077
LC_ALL=C
export LC_ALL

PRODUCT_VERSION="0.2.0-rc9"
ASSET_NAME="aopmem-darwin-arm64"
CHECKSUM_NAME="SHA256SUMS"
TEST_MODE="${AOPMEM_INSTALL_TEST_MODE:-0}"
TEMP_ROOT=""

trace_install_event() {
  if [ "$TEST_MODE" = "1" ] && [ -n "${AOPMEM_INSTALL_TEST_TRACE:-}" ]; then
    printf '%s\n' "$1" >> "$AOPMEM_INSTALL_TEST_TRACE"
  fi
}

fail_install() {
  printf '%s\n' "AOPMem clean install failed: $1" >&2
  exit 1
}

fail_populated_home() {
  printf '%s\n' \
    "CLEAN_INSTALL_REQUIRES_EMPTY_HOME: existing AOPMem installation detected; quarantine it before clean installation" >&2
  exit 1
}

cleanup() {
  if [ -n "$TEMP_ROOT" ]; then
    rm -rf "$TEMP_ROOT"
  fi
}
trap cleanup EXIT HUP INT TERM

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail_install "required command missing: $1"
}

absolute_path() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s\n' "$(pwd -P)/$1" ;;
  esac
}

assert_platform() {
  os_name=$(uname -s)
  arch_name=$(uname -m)
  if [ "$os_name" != "Darwin" ] || [ "$arch_name" != "arm64" ]; then
    fail_install "unsupported platform: $os_name $arch_name"
  fi
}

resolve_home() {
  if [ -n "${AOPMEM_HOME:-}" ]; then
    absolute_path "$AOPMEM_HOME"
  else
    [ -n "${HOME:-}" ] || fail_install "HOME is not set"
    printf '%s\n' "$HOME/.aopmem"
  fi
}

assert_clean_home() {
  home_path=$1
  repo_root=$2
  repo_local_home="$repo_root/.aopmem"
  if [ "$home_path" = "$repo_local_home" ]; then
    fail_install "repo-local .aopmem is not allowed"
  fi
  if [ -e "$home_path" ] || [ -L "$home_path" ]; then
    if [ ! -d "$home_path" ] || [ -L "$home_path" ]; then
      fail_populated_home
    fi
    if find "$home_path" -mindepth 1 -maxdepth 1 -print -quit | grep . >/dev/null; then
      fail_populated_home
    fi
  fi
}

download_asset() {
  asset_name=$1
  destination=$2
  if [ "$TEST_MODE" = "1" ]; then
    test_asset_dir=${AOPMEM_INSTALL_TEST_ASSET_DIR:-}
    [ -n "$test_asset_dir" ] || fail_install "AOPMEM_INSTALL_TEST_ASSET_DIR is not set"
    [ -f "$test_asset_dir/$asset_name" ] || fail_install "local test asset missing: $asset_name"
    cp "$test_asset_dir/$asset_name" "$destination" ||
      fail_install "local test asset copy failed: $asset_name"
    return 0
  fi

  asset_base_uri=${AOPMEM_ASSET_BASE_URI:-}
  case "$asset_base_uri" in
    https://*) ;;
    "") fail_install "trusted HTTPS asset base URI is missing" ;;
    *) fail_install "asset base URI must use HTTPS" ;;
  esac
  case "$asset_base_uri" in
    *"@"*|*"?"*|*"#"*|*[[:space:]]*)
      fail_install "asset base URI must not contain credentials, query, fragment, or whitespace"
      ;;
  esac
  asset_base_uri=${asset_base_uri%/}
  curl --fail --show-error --silent --location \
    --proto '=https' --proto-redir '=https' --tlsv1.2 \
    --connect-timeout 30 --max-time 900 \
    --output "$destination" "$asset_base_uri/$asset_name" ||
    fail_install "download failed: $asset_name"
}

sha256_file() {
  shasum -a 256 "$1" | awk '{ print tolower($1) }'
}

expected_sha256() {
  checksum_path=$1
  exact_name=$2
  awk -v name="$exact_name" '
    $2 == name && $1 ~ /^[0-9A-Fa-f]{64}$/ { count += 1; hash = tolower($1) }
    END { if (count == 1) print hash; else exit 1 }
  ' "$checksum_path" || fail_install "SHA256SUMS has no unique exact entry for $exact_name"
}

verify_asset_hash() {
  file_path=$1
  checksum_path=$2
  exact_name=$3
  expected=$(expected_sha256 "$checksum_path" "$exact_name")
  actual=$(sha256_file "$file_path")
  [ "$actual" = "$expected" ] || fail_install "SHA256 mismatch for $exact_name"
  printf '%s\n' "$actual"
}

run_aopmem() {
  AOPMEM_HOME="$AOPMEM_HOME_PATH" "$INSTALLED_BINARY" "$@"
}

run_init_defaults() {
  {
    printf '%s\n' "n"
    printf '%s\n' "n"
    printf '%s\n' "Clean AOPMem $PRODUCT_VERSION install for this repository."
    printf '%s\n' "User owns decisions. Agent keeps project memory current."
    printf '%s\n' "Only this repository is initialized by the clean installer."
  } | run_aopmem --json init >/dev/null
}

main() {
  require_command uname
  require_command curl
  require_command shasum
  require_command awk
  require_command find
  assert_platform

  REPO_ROOT=$(pwd -P)
  AOPMEM_HOME_PATH=$(resolve_home)
  export AOPMEM_HOME_PATH
  assert_clean_home "$AOPMEM_HOME_PATH" "$REPO_ROOT"
  trace_install_event "home.clean"

  TEMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/aopmem-rc9-install.XXXXXX")
  binary_stage="$TEMP_ROOT/$ASSET_NAME"
  checksums_stage="$TEMP_ROOT/$CHECKSUM_NAME"
  download_asset "$CHECKSUM_NAME" "$checksums_stage"
  download_asset "$ASSET_NAME" "$binary_stage"
  binary_sha=$(verify_asset_hash "$binary_stage" "$checksums_stage" "$ASSET_NAME")
  trace_install_event "asset.verified"

  chmod 755 "$binary_stage"
  version_output=$("$binary_stage" --version)
  [ "$version_output" = "aopmem $PRODUCT_VERSION" ] ||
    fail_install "binary version mismatch: $version_output"

  mkdir -p "$AOPMEM_HOME_PATH/bin"
  INSTALLED_BINARY="$AOPMEM_HOME_PATH/bin/aopmem"
  export INSTALLED_BINARY
  if [ -e "$INSTALLED_BINARY" ] || [ -L "$INSTALLED_BINARY" ]; then
    fail_populated_home
  fi
  cp "$binary_stage" "$INSTALLED_BINARY"
  chmod 755 "$INSTALLED_BINARY"
  sync || true
  trace_install_event "binary.installed"

  run_aopmem platform check --json >/dev/null
  trace_install_event "platform.check"
  run_init_defaults
  trace_install_event "init"

  if [ -n "${AOPMEM_ACTIVE_INSTRUCTION_FILE:-}" ]; then
    run_aopmem --json adapter sync --file "$AOPMEM_ACTIVE_INSTRUCTION_FILE" >/dev/null
  else
    run_aopmem --json adapter sync >/dev/null
  fi
  trace_install_event "adapter.sync"

  run_aopmem --json doctor >/dev/null
  run_aopmem --json verify >/dev/null
  trace_install_event "doctor.verify"

  if [ -d "$REPO_ROOT/.aopmem" ]; then
    fail_install "repo-local .aopmem was created"
  fi

  printf '%s\n' "AOPMem $PRODUCT_VERSION clean install complete"
  printf '%s\n' "AOPMEM_HOME=$AOPMEM_HOME_PATH"
  printf '%s\n' "$ASSET_NAME sha256=$binary_sha"
}

main "$@"
