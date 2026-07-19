#!/bin/sh

# Isolated native macOS proof for RC7. Published RC4, RC5, and RC6 assets
# provision three independent source fixtures. The local RC7 candidate is
# supplied only through the official installer's explicit test asset path.

set -eu
umask 077
LC_ALL=C
export LC_ALL

REPO_ROOT=$(CDPATH= cd "$(dirname "$0")/.." && pwd -P)
INSTALLER="$REPO_ROOT/install/v0.2/install.sh"
RC7_BINARY=${AOPMEM_RC7_PROOF_BINARY:-"$REPO_ROOT/target/debug/aopmem"}
RC4_EXPECTED_SHA256="4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e"
RC5_EXPECTED_SHA256="594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e"
RC6_EXPECTED_SHA256="b933d921ae6ec68ce7e0f118de27fd7eabe9d1c42d715a0a6df8f2ec731cb949"
UI_PID=""

cleanup() {
  if [ -n "$UI_PID" ] && kill -0 "$UI_PID" >/dev/null 2>&1; then
    kill "$UI_PID" >/dev/null 2>&1 || true
    wait "$UI_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT HUP INT TERM

fail() {
  printf '%s\n' "RC7 macOS proof failed: $1" >&2
  exit 1
}

need_command() {
  command -v "$1" >/dev/null 2>&1 || fail "required command is missing: $1"
}

sha256_file() {
  shasum -a 256 "$1" | awk '{ print tolower($1) }'
}

assert_file() {
  [ -f "$1" ] || fail "required file is missing: $1"
}

assert_json() {
  jq -e "$2" "$1" >/dev/null || fail "JSON assertion failed: $2"
}

assert_trace_before() {
  awk -v first="$2" -v second="$3" '
    $0 == first && first_line == 0 { first_line = NR }
    $0 == second && second_line == 0 { second_line = NR }
    END { exit !(first_line > 0 && second_line > first_line) }
  ' "$1" || fail "trace order is wrong: $2 must precede $3"
}

run_cli() {
  repo=$1
  home=$2
  fallback_home=$3
  binary=$4
  shift 4
  (
    cd "$repo"
    HOME="$fallback_home" AOPMEM_HOME="$home" "$binary" "$@"
  )
}

write_answers() {
  printf '%s\n' \
    "нет" \
    "нет" \
    "$2" \
    "Пользователь ведет проект, агент помогает." \
    "Рабочий код, вспомогательные docs, архив нельзя менять." > "$1"
}

download_release_binary() {
  tag=$1
  destination=$2
  expected_sha256=$3
  expected_version=$4
  mkdir -p "$destination"
  gh release download "$tag" \
    --repo ARQAWA/aopmem-cli \
    --pattern aopmem-darwin-arm64 \
    --pattern SHA256SUMS \
    --dir "$destination"
  assert_file "$destination/aopmem-darwin-arm64"
  assert_file "$destination/SHA256SUMS"
  actual_sha256=$(sha256_file "$destination/aopmem-darwin-arm64")
  [ "$actual_sha256" = "$expected_sha256" ] ||
    fail "$tag macOS asset SHA-256 differs from the published digest"
  grep -Eq "^${expected_sha256}[[:space:]][[:space:]]aopmem-darwin-arm64$" \
    "$destination/SHA256SUMS" ||
    fail "$tag SHA256SUMS does not bind the macOS asset"
  chmod 700 "$destination/aopmem-darwin-arm64"
  [ "$("$destination/aopmem-darwin-arm64" --version)" = "$expected_version" ] ||
    fail "$tag macOS asset has an unexpected version"
}

tree_manifest() {
  home=$1
  kind=$2
  output=$3
  : > "$output"
  find "$home/workspaces" -path "*/$kind/*" -type f -print |
    LC_ALL=C sort |
    while IFS= read -r path; do
      relative=${path#"$home/"}
      printf '%s  %s\n' "$(sha256_file "$path")" "$relative"
    done > "$output"
}

database_manifest() {
  home=$1
  output=$2
  shift 2
  : > "$output"
  for key in "$@"; do
    for name in aopmem.sqlite aopmem.sqlite-wal aopmem.sqlite-shm; do
      path="$home/workspaces/$key/$name"
      if [ -f "$path" ]; then
        printf '%s  %s/%s\n' "$(sha256_file "$path")" "$key" "$name"
      fi
    done
  done > "$output"
}

for command_name in awk cmp curl find gh grep jq kill mktemp shasum sort; do
  need_command "$command_name"
done

[ "$(uname -s)" = "Darwin" ] || fail "native proof requires Darwin"
[ "$(uname -m)" = "arm64" ] || fail "native proof requires Apple Silicon arm64"
assert_file "$INSTALLER"
assert_file "$RC7_BINARY"
[ -x "$RC7_BINARY" ] || fail "RC7 candidate is not executable: $RC7_BINARY"
[ "$("$RC7_BINARY" --version)" = "aopmem 0.2.0-rc7" ] ||
  fail "candidate must report exactly aopmem 0.2.0-rc7"

PROOF_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/aopmem-rc7-stage05.XXXXXX")
PUBLISHED="$PROOF_ROOT/published"
ASSETS="$PROOF_ROOT/rc7-assets"
mkdir -p "$PUBLISHED" "$ASSETS"

download_release_binary v0.2.0-rc4 "$PUBLISHED/rc4" \
  "$RC4_EXPECTED_SHA256" "aopmem 0.2.0-rc4"
download_release_binary v0.2.0-rc5 "$PUBLISHED/rc5" \
  "$RC5_EXPECTED_SHA256" "aopmem 0.2.0-rc5"
download_release_binary v0.2.0-rc6 "$PUBLISHED/rc6" \
  "$RC6_EXPECTED_SHA256" "aopmem 0.2.0-rc6"
RC4_BINARY="$PUBLISHED/rc4/aopmem-darwin-arm64"
RC5_BINARY="$PUBLISHED/rc5/aopmem-darwin-arm64"
RC6_BINARY="$PUBLISHED/rc6/aopmem-darwin-arm64"

cp "$RC7_BINARY" "$ASSETS/aopmem-darwin-arm64"
chmod 700 "$ASSETS/aopmem-darwin-arm64"
RC7_SHA256=$(sha256_file "$ASSETS/aopmem-darwin-arm64")
printf '%s  aopmem-darwin-arm64\n' "$RC7_SHA256" > "$ASSETS/SHA256SUMS"

FRESH_ROOT="$PROOF_ROOT/fresh"
FRESH_HOME="$FRESH_ROOT/aopmem-home"
FRESH_FALLBACK_HOME="$FRESH_ROOT/home"
FRESH_REPO="$FRESH_ROOT/repo"
FRESH_TEMP="$FRESH_ROOT/temp"
mkdir -p "$FRESH_HOME" "$FRESH_FALLBACK_HOME" "$FRESH_REPO" "$FRESH_TEMP"
write_answers "$FRESH_ROOT/answers.txt" "RC7 fresh proof project."

(
  cd "$FRESH_REPO"
  env \
    -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY \
    -u http_proxy -u https_proxy -u all_proxy \
    HOME="$FRESH_FALLBACK_HOME" \
    AOPMEM_HOME="$FRESH_HOME" \
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_OS=Darwin \
    AOPMEM_INSTALL_TEST_ARCH=arm64 \
    AOPMEM_INSTALL_TEST_RUN_ID=75001 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSETS" \
    AOPMEM_INSTALL_TEST_TEMP_ROOT="$FRESH_TEMP" \
    AOPMEM_INSTALL_TEST_TRACE="$FRESH_ROOT/trace.log" \
    AOPMEM_ACTIVE_ADAPTER=codex \
    AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md \
    sh "$INSTALLER" < "$FRESH_ROOT/answers.txt" \
      > "$FRESH_ROOT/stdout.log" 2> "$FRESH_ROOT/stderr.log"
)

[ "$("$FRESH_HOME/bin/aopmem" --version)" = "aopmem 0.2.0-rc7" ] ||
  fail "fresh installer did not publish RC7"
[ "$(sha256_file "$FRESH_HOME/bin/aopmem")" = "$RC7_SHA256" ] ||
  fail "fresh installer binary differs from the verified RC7 candidate"
[ ! -e "$FRESH_REPO/.aopmem" ] || fail "fresh proof created repository-local .aopmem"
assert_file "$FRESH_REPO/AGENTS.md"
assert_file "$FRESH_HOME/debug-capsules/upgrade-75001.zip"
assert_trace_before "$FRESH_ROOT/trace.log" "replacement.published" "init"
assert_trace_before "$FRESH_ROOT/trace.log" "init" "adapter.seed"
assert_trace_before "$FRESH_ROOT/trace.log" "verify" "task.start.smoke"
assert_trace_before "$FRESH_ROOT/trace.log" "observe.report" "debug.capsule.export"

UPDATE_ROOT="$PROOF_ROOT/update"
UPDATE_HOME="$UPDATE_ROOT/aopmem-home"
UPDATE_FALLBACK_HOME="$UPDATE_ROOT/home"
UPDATE_TEMP="$UPDATE_ROOT/temp"
REPO_RC4="$UPDATE_ROOT/repo-rc4"
REPO_RC5="$UPDATE_ROOT/repo-rc5"
REPO_RC6="$UPDATE_ROOT/repo-rc6"
mkdir -p "$UPDATE_HOME/bin" "$UPDATE_FALLBACK_HOME" "$UPDATE_TEMP" \
  "$REPO_RC4" "$REPO_RC5" "$REPO_RC6"

# Provision independent source fixtures only with exact published binaries.
for fixture in \
  "rc4:$RC4_BINARY:$REPO_RC4:RC7_RC4_DATA_CANARY:rc7_rc4_tool" \
  "rc5:$RC5_BINARY:$REPO_RC5:RC7_RC5_DATA_CANARY:rc7_rc5_tool" \
  "rc6:$RC6_BINARY:$REPO_RC6:RC7_RC6_DATA_CANARY:rc7_rc6_tool"; do
  label=${fixture%%:*}
  rest=${fixture#*:}
  source_binary=${rest%%:*}
  rest=${rest#*:}
  repo=${rest%%:*}
  rest=${rest#*:}
  canary=${rest%%:*}
  tool_id=${rest#*:}

  cp "$source_binary" "$UPDATE_HOME/bin/aopmem"
  chmod 700 "$UPDATE_HOME/bin/aopmem"
  write_answers "$UPDATE_ROOT/$label-answers.txt" "RC7 $label source fixture."
  run_cli "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" init < "$UPDATE_ROOT/$label-answers.txt" \
    > "$UPDATE_ROOT/$label-init.log"
  run_cli "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" node create \
    --type workflow --status active --title "RC7 $label workflow" \
    --summary "source data must survive" --body "$canary" \
    --source-ref "proof:$label" --confidence 0.91 \
    --trust-level explicit_user --json > "$UPDATE_ROOT/$label-node.json"
  workspace_key=$(jq -r '.meta.workspace_key' "$UPDATE_ROOT/$label-node.json")
  [ -n "$workspace_key" ] && [ "$workspace_key" != "null" ] ||
    fail "$label fixture did not return a workspace key"
  case "$label" in
    rc4) WORKSPACE_RC4=$workspace_key ;;
    rc5) WORKSPACE_RC5=$workspace_key ;;
    rc6) WORKSPACE_RC6=$workspace_key ;;
    *) fail "unexpected fixture label: $label" ;;
  esac
  run_cli "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" tool create-draft \
    --id "$tool_id" --name "RC7 $label tool" --entrypoint run.sh \
    --side-effects none --approval-requirement none --json \
    > "$UPDATE_ROOT/$label-tool.json"
  tool_entry="$UPDATE_HOME/workspaces/$workspace_key/tools/$tool_id/run.sh"
  printf '#!/bin/sh\nprintf "%%s\\n" "%s"\n' "$canary" > "$tool_entry"
  chmod 700 "$tool_entry"
  run_cli "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" tool validate "$tool_id" --json \
    > "$UPDATE_ROOT/$label-tool-validate.json"
  artifact_dir="$UPDATE_HOME/workspaces/$workspace_key/artifacts/rc7-source"
  mkdir -p "$artifact_dir"
  printf '%s\n' "$canary" > "$artifact_dir/$label-artifact.txt"
done

[ "$WORKSPACE_RC4" != "$WORKSPACE_RC5" ] &&
  [ "$WORKSPACE_RC5" != "$WORKSPACE_RC6" ] &&
  [ "$WORKSPACE_RC4" != "$WORKSPACE_RC6" ] ||
  fail "source fixtures did not create separate workspaces"

database_manifest "$UPDATE_HOME" "$UPDATE_ROOT/pre-data.sha256" \
  "$WORKSPACE_RC4" "$WORKSPACE_RC5" "$WORKSPACE_RC6"
tree_manifest "$UPDATE_HOME" tools "$UPDATE_ROOT/pre-tools.sha256"
tree_manifest "$UPDATE_HOME" artifacts "$UPDATE_ROOT/pre-artifacts.sha256"
OLD_BINARY_SHA256=$(sha256_file "$UPDATE_HOME/bin/aopmem")
[ "$OLD_BINARY_SHA256" = "$RC6_EXPECTED_SHA256" ] ||
  fail "active update binary is not the published RC6 asset"

(
  cd "$REPO_RC6"
  env \
    -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY \
    -u http_proxy -u https_proxy -u all_proxy \
    HOME="$UPDATE_FALLBACK_HOME" \
    AOPMEM_HOME="$UPDATE_HOME" \
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_OS=Darwin \
    AOPMEM_INSTALL_TEST_ARCH=arm64 \
    AOPMEM_INSTALL_TEST_RUN_ID=75002 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSETS" \
    AOPMEM_INSTALL_TEST_TEMP_ROOT="$UPDATE_TEMP" \
    AOPMEM_INSTALL_TEST_TRACE="$UPDATE_ROOT/trace.log" \
    AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256="$OLD_BINARY_SHA256" \
    AOPMEM_ACTIVE_ADAPTER=codex \
    AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md \
    sh "$INSTALLER" < /dev/null \
      > "$UPDATE_ROOT/stdout.log" 2> "$UPDATE_ROOT/stderr.log"
)

[ "$("$UPDATE_HOME/bin/aopmem" --version)" = "aopmem 0.2.0-rc7" ] ||
  fail "installer update did not publish RC7"
[ "$(sha256_file "$UPDATE_HOME/bin/aopmem")" = "$RC7_SHA256" ] ||
  fail "updated binary differs from the verified RC7 candidate"
for repo in "$REPO_RC4" "$REPO_RC5" "$REPO_RC6"; do
  [ ! -e "$repo/.aopmem" ] || fail "update created repository-local .aopmem"
done

assert_trace_before "$UPDATE_ROOT/trace.log" "process.gate.clear" "backup.created"
assert_trace_before "$UPDATE_ROOT/trace.log" "backup.home.created" "asset.download.started"
assert_trace_before "$UPDATE_ROOT/trace.log" "platform.check.staged" "audit.repair.staged"
assert_trace_before "$UPDATE_ROOT/trace.log" "audit.repair.staged" "upgrade.prepare"
assert_trace_before "$UPDATE_ROOT/trace.log" "upgrade.prepare" "upgrade.plan"
assert_trace_before "$UPDATE_ROOT/trace.log" "upgrade.plan" "upgrade.apply"
assert_trace_before "$UPDATE_ROOT/trace.log" "upgrade.apply" "upgrade.publish"
assert_trace_before "$UPDATE_ROOT/trace.log" "upgrade.publish" "adapter.sync"
assert_trace_before "$UPDATE_ROOT/trace.log" "adapter.sync" "audit.repair.post-publish"
assert_trace_before "$UPDATE_ROOT/trace.log" "verify" "task.start.smoke"
assert_trace_before "$UPDATE_ROOT/trace.log" "observe.report" "debug.capsule.export"
[ "$(grep -Ec '^upgrade\.apply$' "$UPDATE_ROOT/trace.log")" -eq 1 ] ||
  fail "installer did not invoke apply exactly once"
if grep -Eq '^(init|adapter\.seed)$' "$UPDATE_ROOT/trace.log"; then
  fail "update asked onboarding questions or reinitialized an adapter"
fi

FULL_BACKUP=$(find "$UPDATE_ROOT" -maxdepth 1 -type d \
  -name 'aopmem-home-backup-v0.2.0-rc7-*' -print -quit)
[ -n "$FULL_BACKUP" ] || fail "durable RC7 full-home backup is missing"
assert_file "$FULL_BACKUP/MANIFEST.sha256"
database_manifest "$FULL_BACKUP" "$UPDATE_ROOT/backup-data.sha256" \
  "$WORKSPACE_RC4" "$WORKSPACE_RC5" "$WORKSPACE_RC6"
tree_manifest "$FULL_BACKUP" tools "$UPDATE_ROOT/backup-tools.sha256"
tree_manifest "$FULL_BACKUP" artifacts "$UPDATE_ROOT/backup-artifacts.sha256"
cmp "$UPDATE_ROOT/pre-data.sha256" "$UPDATE_ROOT/backup-data.sha256" ||
  fail "full-home backup does not preserve exact source data bytes"
cmp "$UPDATE_ROOT/pre-tools.sha256" "$UPDATE_ROOT/backup-tools.sha256" ||
  fail "full-home backup does not preserve exact source tool bytes"
cmp "$UPDATE_ROOT/pre-artifacts.sha256" "$UPDATE_ROOT/backup-artifacts.sha256" ||
  fail "full-home backup does not preserve exact source artifact bytes"
[ "$(sha256_file "$FULL_BACKUP/bin/aopmem")" = "$RC6_EXPECTED_SHA256" ] ||
  fail "full-home backup does not retain the exact RC6 executable"
tree_manifest "$UPDATE_HOME" tools "$UPDATE_ROOT/post-tools.sha256"
tree_manifest "$UPDATE_HOME" artifacts "$UPDATE_ROOT/post-artifacts.sha256"
cmp "$UPDATE_ROOT/pre-tools.sha256" "$UPDATE_ROOT/post-tools.sha256" ||
  fail "update changed source tool bytes"
cmp "$UPDATE_ROOT/pre-artifacts.sha256" "$UPDATE_ROOT/post-artifacts.sha256" ||
  fail "update changed source artifact bytes"

for fixture in \
  "rc4:$REPO_RC4:RC7_RC4_DATA_CANARY:$WORKSPACE_RC4" \
  "rc5:$REPO_RC5:RC7_RC5_DATA_CANARY:$WORKSPACE_RC5" \
  "rc6:$REPO_RC6:RC7_RC6_DATA_CANARY:$WORKSPACE_RC6"; do
  label=${fixture%%:*}
  rest=${fixture#*:}
  repo=${rest%%:*}
  rest=${rest#*:}
  canary=${rest%%:*}
  workspace_key=${rest#*:}
  run_cli "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" node list --all --include-body --json \
    > "$UPDATE_ROOT/$label-nodes.json"
  assert_json "$UPDATE_ROOT/$label-nodes.json" \
    ".ok == true and ([.data.nodes[] | select(.body == \"$canary\")] | length == 1)"
  run_cli "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" doctor --json > "$UPDATE_ROOT/$label-doctor.json"
  run_cli "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" verify --json > "$UPDATE_ROOT/$label-verify.json"
  assert_json "$UPDATE_ROOT/$label-verify.json" '.ok == true and .data.clean == true'
  assert_file "$UPDATE_HOME/workspaces/$workspace_key/artifacts/rc7-source/$label-artifact.txt"
done

assert_json "$UPDATE_ROOT/rc6-doctor.json" '.ok == true and .data.healthy == true'
for label in rc4 rc5; do
  assert_json "$UPDATE_ROOT/$label-doctor.json" \
    '.ok == true and .data.checks.db.status == "ready" and
     .data.checks.schema.status == "ready" and
     .data.checks.audit_snapshot.status == "ready" and
     .data.checks.tools_dirs.status == "ready"'
done

run_cli "$REPO_RC6" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" audit repair --all-workspaces --json \
  > "$UPDATE_ROOT/audit-repair.json"
assert_json "$UPDATE_ROOT/audit-repair.json" '.ok == true'
run_cli "$REPO_RC6" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" upgrade prepare --all-workspaces --json \
  > "$UPDATE_ROOT/post-prepare.json"
run_cli "$REPO_RC6" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" upgrade plan --all-workspaces --json \
  > "$UPDATE_ROOT/post-plan.json"
assert_json "$UPDATE_ROOT/post-prepare.json" '.ok == true and .data.success == true'
assert_json "$UPDATE_ROOT/post-plan.json" \
  '.ok == true and .data.ready == true and .data.writes_performed == false and
   ([.data.workspaces[] |
     select(.schema.current_version == "004" and
            .schema.target_version == "004" and
            (.schema.pending_migrations | length) == 0)] | length == 3)'

printf '%s' 'RC7 RC6 workflow' |
  run_cli "$REPO_RC6" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" --json task start --query-stdin \
    > "$UPDATE_ROOT/task-start.json"
assert_json "$UPDATE_ROOT/task-start.json" \
  '.ok == true and .data.mandatory_context_complete == true and
   .data.retrieval_complete == true'
TASK_ID=$(jq -r '.data.task_id' "$UPDATE_ROOT/task-start.json")
BUNDLE_ID=$(jq -r '.data.bundle_id' "$UPDATE_ROOT/task-start.json")
WORKFLOW_ID=$(jq -r '.data.candidate_workflows[0]' "$UPDATE_ROOT/task-start.json")
[ "$TASK_ID" != "null" ] && [ "$BUNDLE_ID" != "null" ] &&
  [ "$WORKFLOW_ID" != "null" ] ||
  fail "task start did not return a selectable workflow receipt"
run_cli "$REPO_RC6" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" --json --bundle-id "$BUNDLE_ID" task apply \
  --task-id "$TASK_ID" --selected-workflow-id "$WORKFLOW_ID" \
  > "$UPDATE_ROOT/task-apply.json"
assert_json "$UPDATE_ROOT/task-apply.json" '.ok == true and .data.status == "applied"'
run_cli "$REPO_RC6" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" --json task complete --task-id "$TASK_ID" \
  --result success > "$UPDATE_ROOT/task-complete.json"
assert_json "$UPDATE_ROOT/task-complete.json" \
  '.ok == true and .data.status == "completed"'

run_cli "$REPO_RC6" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" observe export \
  --output "$UPDATE_ROOT/rc7-debug-capsule.zip" --json \
  > "$UPDATE_ROOT/debug-export.json"
assert_json "$UPDATE_ROOT/debug-export.json" '.ok == true'
assert_file "$UPDATE_ROOT/rc7-debug-capsule.zip"

(
  cd "$REPO_RC6"
  HOME="$UPDATE_FALLBACK_HOME" AOPMEM_HOME="$UPDATE_HOME" \
    "$UPDATE_HOME/bin/aopmem" ui --no-open --port 0 \
    > "$UPDATE_ROOT/ui.log" 2> "$UPDATE_ROOT/ui-error.log"
) &
UI_PID=$!
attempt=0
while [ "$attempt" -lt 100 ]; do
  if grep -Eq '^AOPMem UI: http://127\.0\.0\.1:[0-9]+/' "$UPDATE_ROOT/ui.log" \
    2>/dev/null; then
    break
  fi
  kill -0 "$UI_PID" >/dev/null 2>&1 || fail "UI exited before publishing its URL"
  attempt=$((attempt + 1))
  sleep 0.05
done
UI_URL=$(awk '/^AOPMem UI: http:\/\/127\.0\.0\.1:[0-9]+\// {
  sub(/^AOPMem UI: /, ""); print; exit
}' "$UPDATE_ROOT/ui.log")
[ -n "$UI_URL" ] || fail "UI did not publish a loopback URL"
curl --fail --silent --show-error --noproxy '*' "$UI_URL" \
  > "$UPDATE_ROOT/ui.html"
grep -Fq '<span class="readonly-badge">Read-only</span>' "$UPDATE_ROOT/ui.html" ||
  fail "UI shell does not contain the read-only badge"
kill "$UI_PID"
wait "$UI_PID" 2>/dev/null || true
UI_PID=""

jq -n \
  --arg proof_root "$PROOF_ROOT" \
  --arg rc7_sha256 "$RC7_SHA256" \
  --arg rc4_sha256 "$RC4_EXPECTED_SHA256" \
  --arg rc5_sha256 "$RC5_EXPECTED_SHA256" \
  --arg rc6_sha256 "$RC6_EXPECTED_SHA256" \
  --arg full_backup "$FULL_BACKUP" \
  '{
    result: "PASS",
    platform: "Darwin arm64",
    proof_root: $proof_root,
    binaries: {
      candidate_rc7_sha256: $rc7_sha256,
      source_rc4_sha256: $rc4_sha256,
      source_rc5_sha256: $rc5_sha256,
      source_rc6_sha256: $rc6_sha256
    },
    fresh_rc7: true,
    update_rc4_to_rc7: true,
    update_rc5_to_rc7: true,
    update_rc6_to_rc7: true,
    target_schema: "004",
    apply_invocations: 1,
    onboarding_questions_during_update: 0,
    source_data_tool_artifact_bytes_exact_in_backup: true,
    platform_check: true,
    prepare_plan_publish_adapter: true,
    doctor_verify: true,
    task_start_apply_complete: true,
    audit_repair: true,
    debug_export: true,
    ui_loopback_smoke: true,
    direct_no_proxy_installer_test_path: true,
    repository_local_aopmem: false,
    full_backup: $full_backup
  }' > "$PROOF_ROOT/summary.json"

printf '%s\n' "RC7 macOS Stage 05 proof: PASS"
printf '%s\n' "proof_root=$PROOF_ROOT"
printf '%s\n' "summary=$PROOF_ROOT/summary.json"
