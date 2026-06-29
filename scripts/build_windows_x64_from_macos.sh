#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

target_triple="x86_64-pc-windows-msvc"
rel_dist_bin="dist/aopmem-windows-x86_64/aopmem.exe"
dist_dir="$repo_root/dist/aopmem-windows-x86_64"
dist_bin="$dist_dir/aopmem.exe"
built_bin="$repo_root/target/$target_triple/release/aopmem.exe"

if [[ ! -f "$repo_root/Cargo.toml" ]]; then
  echo "Cargo.toml not found at repo root: $repo_root" >&2
  exit 1
fi

mkdir -p "$dist_dir"

if cargo xwin --version >/dev/null 2>&1; then
  rustup target add "$target_triple"
  cargo xwin build --release --target "$target_triple"
elif docker --version >/dev/null 2>&1; then
  docker run --rm --platform linux/amd64 \
    -v "$PWD":/io \
    -w /io \
    messense/cargo-xwin \
    cargo xwin build --release --target "$target_triple"
else
  cat >&2 <<'EOF'
BLOCKED_BUILD_ENV

Install native cargo-xwin support, then rerun:

brew install llvm
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
EOF
  exit 1
fi

if [[ ! -s "$built_bin" ]]; then
  echo "missing or empty built binary: $built_bin" >&2
  exit 1
fi

cp "$built_bin" "$dist_bin"

if [[ ! -s "$dist_bin" ]]; then
  echo "missing or empty dist binary: $dist_bin" >&2
  exit 1
fi

file_output="$(file "$rel_dist_bin")"
echo "$file_output"

if [[ "$file_output" != *"PE32+ executable"* ]] || \
  [[ "$file_output" != *"x86-64"* ]]; then
  echo "unexpected Windows binary type: $file_output" >&2
  exit 1
fi

shasum -a 256 "$rel_dist_bin"
