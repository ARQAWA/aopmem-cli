#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

target_triple="aarch64-apple-darwin"
dist_bin="$repo_root/dist/aopmem-darwin-arm64"
built_bin="$repo_root/target/$target_triple/release/aopmem"

if [[ -d "$dist_bin" ]]; then
  echo "legacy nested dist layout blocks flat asset: $dist_bin" >&2
  exit 1
fi

MACOSX_DEPLOYMENT_TARGET=11.0 \
  CARGO_PROFILE_RELEASE_STRIP=false \
  cargo build --locked --release --target "$target_triple"

if [[ ! -f "$built_bin" ]]; then
  echo "missing built binary: $built_bin" >&2
  exit 1
fi

mkdir -p "$repo_root/dist"
cp "$built_bin" "$dist_bin"
chmod 755 "$dist_bin"
codesign --force --sign - "$dist_bin"

file "$dist_bin"
vtool -show-build "$dist_bin" | awk '
  $1 == "minos" { print "minimum macOS " $2; exit }
'
shasum -a 256 "$dist_bin"

echo "built $dist_bin"
