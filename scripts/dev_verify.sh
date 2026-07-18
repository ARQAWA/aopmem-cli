#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tmp_base="${TMPDIR:-/tmp}"
if [[ ! -d "$tmp_base" ]]; then
  tmp_base="/tmp"
fi
tmp_root="$(mktemp -d "$tmp_base/aopmem-dev-verify.XXXXXX")"
cleanup() {
  rm -rf "$tmp_root"
}
trap cleanup EXIT

capture_last_line() {
  awk 'NF { line = $0 } END { print line }'
}

current_workspace_dir() {
  local home_root="$1"
  local workspace_root="$home_root/workspaces"
  local workspace_dirs=()
  while IFS= read -r workspace_dir; do
    workspace_dirs+=("$workspace_dir")
  done < <(find "$workspace_root" -mindepth 1 -maxdepth 1 -type d | sort)

  if [[ "${#workspace_dirs[@]}" -ne 1 ]]; then
    echo "expected exactly one workspace dir under $workspace_root" >&2
    exit 1
  fi

  printf '%s\n' "${workspace_dirs[0]}"
}

run_json_ok() {
  local expected_command="$1"
  local working_dir="$2"
  shift 2

  local output
  output="$(cd "$working_dir" && "$@" 2>&1 | capture_last_line)"
  python3 - "$expected_command" "$output" <<'PY'
import json
import sys

expected_command = sys.argv[1]
payload = json.loads(sys.argv[2])

assert payload["ok"] is True, payload
assert payload["command"] == expected_command, payload
PY
}

run_json_ok_from_file() {
  local expected_command="$1"
  local input_file="$2"
  local working_dir="$3"
  shift 3

  local output
  output="$(cd "$working_dir" && "$@" <"$input_file" 2>&1 | capture_last_line)"
  python3 - "$expected_command" "$output" <<'PY'
import json
import sys

expected_command = sys.argv[1]
payload = json.loads(sys.argv[2])

assert payload["ok"] is True, payload
assert payload["command"] == expected_command, payload
PY
}

run_json_fail() {
  local expected_exit="$1"
  local expected_command="$2"
  local expected_code="$3"
  local working_dir="$4"
  shift 4

  local output
  set +e
  output="$(cd "$working_dir" && "$@" 2>&1 | capture_last_line)"
  local status=$?
  set -e

  if [[ "$status" -ne "$expected_exit" ]]; then
    echo "expected exit $expected_exit, got $status" >&2
    echo "$output" >&2
    exit 1
  fi

  python3 - "$expected_command" "$expected_code" "$output" <<'PY'
import json
import sys

expected_command = sys.argv[1]
expected_code = sys.argv[2]
payload = json.loads(sys.argv[3])

assert payload["ok"] is False, payload
assert payload["command"] == expected_command, payload
assert payload["errors"], payload
assert payload["errors"][0]["code"] == expected_code, payload
PY
}

assert_json_field() {
  local working_dir="$1"
  local expression="$2"
  shift 2

  local output
  output="$(cd "$working_dir" && "$@" 2>&1 | capture_last_line)"
  python3 - "$expression" "$output" <<'PY'
import json
import sys

expression = sys.argv[1]
payload = json.loads(sys.argv[2])
scope = {"data": payload["data"], "meta": payload["meta"]}
safe_builtins = {"len": len}

if not eval(expression, {"__builtins__": safe_builtins}, scope):
    raise SystemExit(f"assertion failed: {expression}\n{payload}")
PY
}

assert_json_field_from_file() {
  local input_file="$1"
  local working_dir="$2"
  local expression="$3"
  shift 3

  local output
  output="$(cd "$working_dir" && "$@" <"$input_file" 2>&1 | capture_last_line)"
  python3 - "$expression" "$output" <<'PY'
import json
import sys

expression = sys.argv[1]
payload = json.loads(sys.argv[2])
scope = {"data": payload["data"], "meta": payload["meta"]}
safe_builtins = {"len": len}

if not eval(expression, {"__builtins__": safe_builtins}, scope):
    raise SystemExit(f"assertion failed: {expression}\n{payload}")
PY
}

assert_exit_json_field() {
  local expected_exit="$1"
  local working_dir="$2"
  local expression="$3"
  shift 3

  local output
  set +e
  output="$(cd "$working_dir" && "$@" 2>&1 | capture_last_line)"
  local status=$?
  set -e

  if [[ "$status" -ne "$expected_exit" ]]; then
    echo "expected exit $expected_exit, got $status" >&2
    echo "$output" >&2
    exit 1
  fi

  python3 - "$expression" "$output" <<'PY'
import json
import sys

expression = sys.argv[1]
payload = json.loads(sys.argv[2])
scope = {
    "ok": payload["ok"],
    "command": payload["command"],
    "data": payload["data"],
    "errors": payload["errors"],
    "meta": payload["meta"],
}
safe_builtins = {"len": len}

if not eval(expression, {"__builtins__": safe_builtins}, scope):
    raise SystemExit(f"assertion failed: {expression}\n{payload}")
PY
}

echo "==> cargo build"
cargo build

echo "==> cargo test"
cargo test

bin_path="$repo_root/target/debug/aopmem"
if [[ ! -x "$bin_path" ]]; then
  echo "missing built binary: $bin_path" >&2
  exit 1
fi

proof_home="$tmp_root/proof-home"
hunch_home="$tmp_root/hunch-home"
negative_home="$tmp_root/negative-home"
drift_home="$tmp_root/drift-home"
fallback_home="$tmp_root/fallback-home"
mkdir -p "$proof_home" "$hunch_home" "$negative_home" "$drift_home" "$fallback_home"

proof_repo="$tmp_root/proof-repo"
hunch_repo="$tmp_root/hunch-repo"
negative_repo="$tmp_root/negative-repo"
drift_repo="$tmp_root/drift-repo"
mkdir -p "$proof_repo" "$hunch_repo" "$negative_repo" "$drift_repo"
init_answers_file="$tmp_root/init-answers.txt"
cat >"$init_answers_file" <<'EOF'
no
no
Local AOPMem CLI proof
User drives local verification and agent implements code
Only local repo code is in scope
EOF

common_env=(
  env
  AOPMEM_HOME="$proof_home"
  HOME="$fallback_home"
)
hunch_env=(
  env
  AOPMEM_HOME="$hunch_home"
  HOME="$fallback_home"
)
negative_env=(
  env
  AOPMEM_HOME="$negative_home"
  HOME="$fallback_home"
)
drift_env=(
  env
  AOPMEM_HOME="$drift_home"
  HOME="$fallback_home"
)

echo "==> cli proof: init/node/recall/tool/artifacts/doctor/verify"
assert_json_field_from_file "$init_answers_file" "$proof_repo" \
  'data["initialized"] is True and data["db_created"] is True' \
  "${common_env[@]}" "$bin_path" --json init
proof_workspace_dir="$(current_workspace_dir "$proof_home")"
proof_artifacts_dir="$proof_workspace_dir/artifacts"

assert_json_field "$proof_repo" \
  'data["node_type"] == "workflow" and data["title"] == "Proof workflow" and data["status"] == "draft"' \
  "${common_env[@]}" "$bin_path" --json node create \
  --type workflow \
  --title "Proof workflow" \
  --summary "Runtime proof workflow"
assert_json_field "$proof_repo" \
  'data["block_inserted"] is True and data["block_present"] is False' \
  "${common_env[@]}" "$bin_path" --json adapter sync
assert_json_field "$proof_repo" \
  'len(data["workflows"]["draft"]) == 1 and data["workflows"]["draft"][0]["title"] == "Proof workflow" and len(data["compact"]["applicable_workflows"]) == 1 and data["compact"]["applicable_workflows"][0]["title"] == "Proof workflow"' \
  "${common_env[@]}" "$bin_path" --json recall
assert_json_field "$proof_repo" \
  'data["record"]["tool_id"] == "proof-tool" and data["record"]["status"] == "draft"' \
  "${common_env[@]}" "$bin_path" --json tool create-draft \
  --id proof-tool \
  --name "Proof Tool"
[[ -f "$proof_workspace_dir/tools/proof-tool/tool.json" ]]
[[ -d "$proof_workspace_dir/tools/proof-tool/runtime" ]]
mkdir -p "$proof_artifacts_dir/2000-01-01"
printf 'old' >"$proof_artifacts_dir/2000-01-01/old.txt"
assert_json_field "$proof_repo" \
  'len(data["deleted_dirs"]) == 1 and data["deleted_dirs"][0].endswith("/2000-01-01") and data["bytes_before"] >= 3 and data["bytes_after"] <= data["bytes_before"]' \
  "${common_env[@]}" "$bin_path" --json artifacts cleanup
[[ ! -d "$proof_artifacts_dir/2000-01-01" ]]
assert_json_field "$proof_repo" \
  'data["healthy"] is True and data["checks"]["adapter_block"]["managed_block"] == "in_sync" and data["checks"]["tools_dirs"]["status"] == "ready" and data["checks"]["artifacts_dirs"]["status"] == "ready"' \
  "${common_env[@]}" "$bin_path" --json doctor
assert_json_field "$proof_repo" \
  'data["clean"] is True and data["summary"]["total"] == 0' \
  "${common_env[@]}" "$bin_path" --json verify

echo "==> cli hunch proof"
run_json_ok_from_file "init" "$init_answers_file" "$hunch_repo" \
  "${hunch_env[@]}" "$bin_path" --json init
assert_json_field "$hunch_repo" \
  'data["node_type"] == "failure_mode" and data["status"] == "active"' \
  "${hunch_env[@]}" "$bin_path" --json node create \
  --type failure_mode \
  --status active \
  --title "Project meaning failure" \
  --summary "Runtime hunch proof" \
  --source-ref "source=user_instruction" \
  --confidence 1.0 \
  --trust-level high
hunch_workspace_dir="$(current_workspace_dir "$hunch_home")"
python3 - "$hunch_workspace_dir/aopmem.sqlite" <<'PY'
import sqlite3
import sys

connection = sqlite3.connect(sys.argv[1])
connection.executescript(
    """
    DELETE FROM links;
    DELETE FROM aliases;
    DELETE FROM tags;
    DELETE FROM sources;
    DELETE FROM events;
    DELETE FROM nodes
    WHERE NOT (
        (node_type = 'project_profile' AND title = 'Project meaning')
        OR (node_type = 'gate' AND title = 'Memory writes stay user-triggered')
        OR (node_type = 'failure_mode' AND title = 'Project meaning failure')
    );
    DELETE FROM fts_nodes
    WHERE rowid NOT IN (SELECT id FROM nodes);
    """
)
connection.commit()
connection.close()
PY
assert_json_field "$hunch_repo" \
  'len(data["hunches"]) == 1 and data["hunches"][0]["source_node_type"] == "failure_mode" and data["hunches"][0]["reason"] == "fts_match_failure_mode_hotness" and len(data["compact"]["hunches"]) == 1' \
  "${hunch_env[@]}" "$bin_path" --json recall

echo "==> cli negative checks"
run_json_fail 3 "verify" "WORKSPACE_NOT_FOUND" "$negative_repo" \
  "${negative_env[@]}" "$bin_path" --json verify
run_json_fail 5 "node_create" "VALIDATION_ERROR" "$proof_repo" \
  "${common_env[@]}" "$bin_path" --json node create \
  --type not_allowed \
  --title "Bad node"

echo "==> cli drift check"
run_json_ok_from_file "init" "$init_answers_file" "$drift_repo" \
  "${drift_env[@]}" "$bin_path" --json init
assert_json_field "$drift_repo" \
  'data["block_inserted"] is True' \
  "${drift_env[@]}" "$bin_path" --json adapter sync
python3 - "$drift_repo/AGENTS.md" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
needle = "`AOPMEM CONTRACT VERSION: 2`\n"
if text.count(needle) != 1:
    raise SystemExit("expected exactly one V2 contract marker before drift mutation")
drifted = text.replace(needle, "`AOPMEM CONTRACT VERSION: DRIFTED`\n", 1)
if drifted == text:
    raise SystemExit("drift mutation did not change the managed block")
path.write_text(drifted)
PY
assert_exit_json_field 8 "$drift_repo" \
  'ok is True and command == "verify" and data["clean"] is False and data["summary"]["adapter_block_drift"] == 1 and data["summary"]["total"] >= 1' \
  "${drift_env[@]}" "$bin_path" --json verify

echo "dev verify passed"
