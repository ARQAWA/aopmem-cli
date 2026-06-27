#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

target_triple="aarch64-apple-darwin"
dist_dir="$repo_root/dist/aopmem-darwin-arm64"
dist_bin="$dist_dir/aopmem"

host_triple="$(rustc -vV | awk '/^host: / { print $2 }')"

if [[ "$host_triple" == "$target_triple" ]]; then
  cargo build --release
  built_bin="$repo_root/target/release/aopmem"
else
  cargo build --release --target "$target_triple"
  built_bin="$repo_root/target/$target_triple/release/aopmem"
fi

if [[ ! -f "$built_bin" ]]; then
  echo "missing built binary: $built_bin" >&2
  exit 1
fi

mkdir -p "$dist_dir"
cp "$built_bin" "$dist_bin"
chmod 755 "$dist_bin"

echo "built $dist_bin"
