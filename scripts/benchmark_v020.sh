#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output_dir=${AOPMEM_BENCHMARK_OUTPUT_DIR:-"$repo_root/.devplan/benchmarks/v020_rc1"}
temporary_root=$(mktemp -d "${TMPDIR:-/tmp}/aopmem-v020-build.XXXXXX")

cleanup() {
    rm -rf "$temporary_root"
}
trap cleanup 0 HUP INT TERM

baseline_source="$temporary_root/baseline-source"
baseline_target="$temporary_root/baseline-target"
current_target="$temporary_root/current-target"
mkdir -p "$baseline_source"

baseline_commit=$(git -C "$repo_root" rev-parse 'v0.1.0-rc3^{commit}')
git -C "$repo_root" archive "$baseline_commit" | tar -x -C "$baseline_source"

(
    cd "$baseline_source"
    CARGO_TARGET_DIR="$baseline_target" cargo build --release --locked
)
(
    cd "$repo_root"
    CARGO_TARGET_DIR="$current_target" cargo build --release --locked
)

python3 "$repo_root/scripts/benchmark_v020.py" \
    --baseline-binary "$baseline_target/release/aopmem" \
    --current-binary "$current_target/release/aopmem" \
    --baseline-commit "$baseline_commit" \
    --build-profile release \
    --output-dir "$output_dir" \
    "$@"
