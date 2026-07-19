#!/bin/sh

# Isolated native macOS proof for RC6. It uses published RC4 and RC5 assets
# only to provision source workspaces; the RC6 candidate is the locally built
# release candidate supplied to the official installer in explicit test mode.

set -eu
umask 077
LC_ALL=C
export LC_ALL

REPO_ROOT=$(CDPATH= cd "$(dirname "$0")/.." && pwd -P)
INSTALLER="$REPO_ROOT/install/v0.2/install.sh"
RC6_BINARY=${AOPMEM_RC6_PROOF_BINARY:-"$REPO_ROOT/target/debug/aopmem"}
RC4_EXPECTED_SHA256="4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e"
RC5_EXPECTED_SHA256="594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e"

fail() {
  printf '%s\n' "RC6 macOS proof failed: $1" >&2
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

assert_trace_before() {
  awk -v first="$2" -v second="$3" '
    $0 == first && first_line == 0 { first_line = NR }
    $0 == second && second_line == 0 { second_line = NR }
    END { exit !(first_line > 0 && second_line > first_line) }
  ' "$1" || fail "trace order is wrong: $2 must precede $3"
}

assert_json() {
  jq -e "$2" "$1" >/dev/null || fail "JSON assertion failed: $2"
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

database_manifest() {
  home=$1
  key_one=$2
  key_two=$3
  output=$4
  : > "$output"
  for key in "$key_one" "$key_two"; do
    for name in aopmem.sqlite aopmem.sqlite-wal aopmem.sqlite-shm; do
      path="$home/workspaces/$key/$name"
      if [ -f "$path" ]; then
        printf '%s  %s/%s\n' "$(sha256_file "$path")" "$key" "$name"
      fi
    done
  done > "$output"
}

download_release_binary() {
  tag=$1
  destination=$2
  expected_sha256=$3
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
}

for command_name in awk cmp find gh grep jq mktemp shasum; do
  need_command "$command_name"
done

[ "$(uname -s)" = "Darwin" ] || fail "native proof requires Darwin"
[ "$(uname -m)" = "arm64" ] || fail "native proof requires Apple Silicon arm64"
assert_file "$INSTALLER"
assert_file "$RC6_BINARY"
[ -x "$RC6_BINARY" ] || fail "RC6 candidate is not executable: $RC6_BINARY"
[ "$("$RC6_BINARY" --version)" = "aopmem 0.2.0-rc6" ] ||
  fail "candidate must report exactly aopmem 0.2.0-rc6"

PROOF_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/aopmem-rc6-stage07.XXXXXX")
PUBLISHED="$PROOF_ROOT/published"
ASSETS="$PROOF_ROOT/rc6-assets"
mkdir -p "$PUBLISHED" "$ASSETS"

download_release_binary v0.2.0-rc4 "$PUBLISHED/rc4" "$RC4_EXPECTED_SHA256"
download_release_binary v0.2.0-rc5 "$PUBLISHED/rc5" "$RC5_EXPECTED_SHA256"
RC4_BINARY="$PUBLISHED/rc4/aopmem-darwin-arm64"
RC5_BINARY="$PUBLISHED/rc5/aopmem-darwin-arm64"
[ "$("$RC4_BINARY" --version)" = "aopmem 0.2.0-rc4" ] ||
  fail "published RC4 macOS asset has an unexpected version"
[ "$("$RC5_BINARY" --version)" = "aopmem 0.2.0-rc5" ] ||
  fail "published RC5 macOS asset has an unexpected version"

cp "$RC6_BINARY" "$ASSETS/aopmem-darwin-arm64"
chmod 700 "$ASSETS/aopmem-darwin-arm64"
RC6_SHA256=$(sha256_file "$ASSETS/aopmem-darwin-arm64")
printf '%s  aopmem-darwin-arm64\n' "$RC6_SHA256" > "$ASSETS/SHA256SUMS"

FRESH_ROOT="$PROOF_ROOT/fresh"
FRESH_HOME="$FRESH_ROOT/aopmem-home"
FRESH_FALLBACK_HOME="$FRESH_ROOT/home"
FRESH_REPO="$FRESH_ROOT/repo"
FRESH_TEMP="$FRESH_ROOT/temp"
mkdir -p "$FRESH_HOME" "$FRESH_FALLBACK_HOME" "$FRESH_REPO" "$FRESH_TEMP"
write_answers "$FRESH_ROOT/answers.txt" "RC6 fresh proof project."

(
  cd "$FRESH_REPO"
  env \
    HOME="$FRESH_FALLBACK_HOME" \
    AOPMEM_HOME="$FRESH_HOME" \
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_OS=Darwin \
    AOPMEM_INSTALL_TEST_ARCH=arm64 \
    AOPMEM_INSTALL_TEST_RUN_ID=70001 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSETS" \
    AOPMEM_INSTALL_TEST_TEMP_ROOT="$FRESH_TEMP" \
    AOPMEM_INSTALL_TEST_TRACE="$FRESH_ROOT/trace.log" \
    AOPMEM_ACTIVE_ADAPTER=codex \
    AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md \
    sh "$INSTALLER" < "$FRESH_ROOT/answers.txt" \
      > "$FRESH_ROOT/stdout.log" 2> "$FRESH_ROOT/stderr.log"
)

[ "$("$FRESH_HOME/bin/aopmem" --version)" = "aopmem 0.2.0-rc6" ] ||
  fail "fresh installer did not publish RC6"
[ "$(sha256_file "$FRESH_HOME/bin/aopmem")" = "$RC6_SHA256" ] ||
  fail "fresh installer binary differs from the verified RC6 candidate"
[ ! -e "$FRESH_REPO/.aopmem" ] || fail "fresh proof created repository-local .aopmem"
assert_file "$FRESH_REPO/AGENTS.md"
assert_file "$FRESH_HOME/debug-capsules/upgrade-70001.zip"
assert_trace_before "$FRESH_ROOT/trace.log" "replacement.published" "init"
assert_trace_before "$FRESH_ROOT/trace.log" "init" "adapter.seed"
assert_trace_before "$FRESH_ROOT/trace.log" "verify" "task.start.smoke"
assert_trace_before "$FRESH_ROOT/trace.log" "observe.report" "debug.capsule.export"

UPDATE_ROOT="$PROOF_ROOT/update"
UPDATE_HOME="$UPDATE_ROOT/aopmem-home"
UPDATE_FALLBACK_HOME="$UPDATE_ROOT/home"
REPO_003="$UPDATE_ROOT/repo-schema003"
REPO_004="$UPDATE_ROOT/repo-schema004"
UPDATE_TEMP="$UPDATE_ROOT/temp"
mkdir -p \
  "$UPDATE_HOME/bin" \
  "$UPDATE_FALLBACK_HOME" \
  "$REPO_003" \
  "$REPO_004" \
  "$UPDATE_TEMP"

# Fixture provisioning may copy published old binaries only inside this
# isolated temporary home. The actual update below is only install.sh.
cp "$RC4_BINARY" "$UPDATE_HOME/bin/aopmem"
chmod 700 "$UPDATE_HOME/bin/aopmem"
write_answers "$UPDATE_ROOT/rc4-answers.txt" "RC6 RC4 schema003 fixture."
run_cli "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" init < "$UPDATE_ROOT/rc4-answers.txt" \
  > "$UPDATE_ROOT/rc4-init.log"
run_cli "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node create \
  --type workflow --status active --title "RC6 RC4 workflow" \
  --summary "schema003 must survive" --body "RC6_RC4_WORKFLOW_CANARY" \
  --source-ref "proof:rc4" --confidence 0.91 --trust-level explicit_user \
  --json > "$UPDATE_ROOT/rc4-node.json"
WORKSPACE_003=$(jq -r '.meta.workspace_key' "$UPDATE_ROOT/rc4-node.json")
[ -n "$WORKSPACE_003" ] && [ "$WORKSPACE_003" != "null" ] ||
  fail "RC4 fixture did not return a workspace key"

cp "$RC5_BINARY" "$UPDATE_HOME/bin/aopmem"
chmod 700 "$UPDATE_HOME/bin/aopmem"
write_answers "$UPDATE_ROOT/rc5-answers.txt" "RC6 RC5 schema004 fixture."
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" init < "$UPDATE_ROOT/rc5-answers.txt" \
  > "$UPDATE_ROOT/rc5-init.log"
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node create \
  --type workflow --status active --title "RC6 RC5 workflow" \
  --summary "schema004 must survive" --body "RC6_RC5_WORKFLOW_CANARY" \
  --source-ref "proof:rc5" --confidence 0.92 --trust-level explicit_user \
  --json > "$UPDATE_ROOT/rc5-node.json"
WORKSPACE_004=$(jq -r '.meta.workspace_key' "$UPDATE_ROOT/rc5-node.json")
[ -n "$WORKSPACE_004" ] && [ "$WORKSPACE_004" != "null" ] ||
  fail "RC5 fixture did not return a workspace key"

database_manifest "$UPDATE_HOME" "$WORKSPACE_003" "$WORKSPACE_004" \
  "$UPDATE_ROOT/pre-backup-db.sha256"
OLD_BINARY_SHA256=$(sha256_file "$UPDATE_HOME/bin/aopmem")
[ "$OLD_BINARY_SHA256" = "$RC5_EXPECTED_SHA256" ] ||
  fail "active update binary is not the published RC5 asset"

(
  cd "$REPO_004"
  env \
    HOME="$UPDATE_FALLBACK_HOME" \
    AOPMEM_HOME="$UPDATE_HOME" \
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_OS=Darwin \
    AOPMEM_INSTALL_TEST_ARCH=arm64 \
    AOPMEM_INSTALL_TEST_RUN_ID=70002 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSETS" \
    AOPMEM_INSTALL_TEST_TEMP_ROOT="$UPDATE_TEMP" \
    AOPMEM_INSTALL_TEST_TRACE="$UPDATE_ROOT/trace.log" \
    AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256="$OLD_BINARY_SHA256" \
    AOPMEM_ACTIVE_ADAPTER=codex \
    AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md \
    sh "$INSTALLER" < /dev/null \
      > "$UPDATE_ROOT/stdout.log" 2> "$UPDATE_ROOT/stderr.log"
)

[ "$("$UPDATE_HOME/bin/aopmem" --version)" = "aopmem 0.2.0-rc6" ] ||
  fail "RC5-to-RC6 installer update did not publish RC6"
[ "$(sha256_file "$UPDATE_HOME/bin/aopmem")" = "$RC6_SHA256" ] ||
  fail "updated binary differs from the verified RC6 candidate"
[ ! -e "$REPO_003/.aopmem" ] || fail "schema003 repo gained local .aopmem"
[ ! -e "$REPO_004/.aopmem" ] || fail "schema004 repo gained local .aopmem"
assert_trace_before "$UPDATE_ROOT/trace.log" "process.gate.clear" "backup.created"
assert_trace_before "$UPDATE_ROOT/trace.log" "backup.home.created" "asset.download.started"
assert_trace_before "$UPDATE_ROOT/trace.log" "platform.check.staged" "audit.repair.staged"
assert_trace_before "$UPDATE_ROOT/trace.log" "audit.repair.staged" "upgrade.prepare"
assert_trace_before "$UPDATE_ROOT/trace.log" "upgrade.prepare" "upgrade.plan"
assert_trace_before "$UPDATE_ROOT/trace.log" "upgrade.plan" "upgrade.apply"
assert_trace_before "$UPDATE_ROOT/trace.log" "upgrade.apply" "upgrade.publish"
assert_trace_before "$UPDATE_ROOT/trace.log" "upgrade.publish" "adapter.sync"
assert_trace_before "$UPDATE_ROOT/trace.log" "adapter.sync" "audit.repair.post-publish"
assert_trace_before "$UPDATE_ROOT/trace.log" "observe.report" "debug.capsule.export"
[ "$(grep -Ec '^upgrade\.apply$' "$UPDATE_ROOT/trace.log")" -eq 1 ] ||
  fail "installer did not invoke apply exactly once"
if grep -Eq '^(init|adapter\.seed)$' "$UPDATE_ROOT/trace.log"; then
  fail "update asked onboarding questions or reinitialized an adapter"
fi

FULL_BACKUP=$(find "$UPDATE_ROOT" -maxdepth 1 -type d \
  -name 'aopmem-home-backup-v0.2.0-rc6-*' -print -quit)
[ -n "$FULL_BACKUP" ] || fail "durable RC6 full-home backup is missing"
assert_file "$FULL_BACKUP/MANIFEST.sha256"
database_manifest "$FULL_BACKUP" "$WORKSPACE_003" "$WORKSPACE_004" \
  "$UPDATE_ROOT/backup-db.sha256"
cmp "$UPDATE_ROOT/pre-backup-db.sha256" "$UPDATE_ROOT/backup-db.sha256" ||
  fail "full-home backup does not preserve exact source SQLite bytes"
[ "$(sha256_file "$FULL_BACKUP/bin/aopmem")" = "$RC5_EXPECTED_SHA256" ] ||
  fail "full-home backup does not retain the exact RC5 executable"

for item in "schema003:$REPO_003:RC6_RC4_WORKFLOW_CANARY" \
            "schema004:$REPO_004:RC6_RC5_WORKFLOW_CANARY"; do
  label=${item%%:*}
  rest=${item#*:}
  repo=${rest%%:*}
  canary=${rest#*:}
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
  if [ "$label" = "schema004" ]; then
    assert_json "$UPDATE_ROOT/$label-doctor.json" '.ok == true and .data.healthy == true'
  else
    assert_json "$UPDATE_ROOT/$label-doctor.json" \
      '.ok == true and .data.checks.db.status == "ready" and .data.checks.schema.status == "ready" and .data.checks.audit_snapshot.status == "ready" and .data.checks.tools_dirs.status == "ready" and .data.checks.adapter_block.status == "missing"'
  fi
done

run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" audit repair --all-workspaces --json \
  > "$UPDATE_ROOT/audit-repair.json"
assert_json "$UPDATE_ROOT/audit-repair.json" '.ok == true'
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" upgrade prepare --all-workspaces --json \
  > "$UPDATE_ROOT/post-prepare.json"
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" upgrade plan --all-workspaces --json \
  > "$UPDATE_ROOT/post-plan.json"
assert_json "$UPDATE_ROOT/post-prepare.json" '.ok == true and .data.success == true'
assert_json "$UPDATE_ROOT/post-plan.json" \
  '.ok == true and .data.ready == true and .data.writes_performed == false and ([.data.workspaces[] | select(.schema.current_version == "004" and .schema.target_version == "004" and (.schema.pending_migrations | length) == 0)] | length == 2)'

printf '%s' 'RC6 RC5 workflow' |
  run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" --json task start --query-stdin \
    > "$UPDATE_ROOT/task-start.json"
assert_json "$UPDATE_ROOT/task-start.json" \
  '.ok == true and .data.mandatory_context_complete == true and .data.retrieval_complete == true'
TASK_ID=$(jq -r '.data.task_id' "$UPDATE_ROOT/task-start.json")
BUNDLE_ID=$(jq -r '.data.bundle_id' "$UPDATE_ROOT/task-start.json")
WORKFLOW_ID=$(jq -r '.data.candidate_workflows[0]' "$UPDATE_ROOT/task-start.json")
[ "$TASK_ID" != "null" ] && [ "$BUNDLE_ID" != "null" ] && [ "$WORKFLOW_ID" != "null" ] ||
  fail "task-start did not return a selectable workflow receipt"
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" --json --bundle-id "$BUNDLE_ID" task apply \
  --task-id "$TASK_ID" --selected-workflow-id "$WORKFLOW_ID" \
  > "$UPDATE_ROOT/task-apply.json"
assert_json "$UPDATE_ROOT/task-apply.json" '.ok == true and .data.status == "applied"'
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" --json task complete --task-id "$TASK_ID" \
  --result success > "$UPDATE_ROOT/task-complete.json"
assert_json "$UPDATE_ROOT/task-complete.json" '.ok == true and .data.status == "completed"'

# Task lifecycle reads can leave ordinary SQLite coordination sidecars. The
# supported prepare command clears only verified empty direct children before
# the immutable dedupe plan; no manual WAL/SHM operation is used.
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" upgrade prepare --all-workspaces --json \
  > "$UPDATE_ROOT/pre-dedupe-prepare.json"
assert_json "$UPDATE_ROOT/pre-dedupe-prepare.json" '.ok == true and .data.success == true'
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tool dedupe plan --json \
  > "$UPDATE_ROOT/dedupe-plan.json"
assert_json "$UPDATE_ROOT/dedupe-plan.json" '.ok == true and .data.writes_performed == false'
run_cli "$REPO_004" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" observe export \
  --output "$UPDATE_ROOT/rc6-debug-capsule.zip" --json \
  > "$UPDATE_ROOT/debug-export.json"
assert_json "$UPDATE_ROOT/debug-export.json" '.ok == true'
assert_file "$UPDATE_ROOT/rc6-debug-capsule.zip"

jq -n \
  --arg proof_root "$PROOF_ROOT" \
  --arg rc6_sha256 "$RC6_SHA256" \
  --arg rc4_sha256 "$RC4_EXPECTED_SHA256" \
  --arg rc5_sha256 "$RC5_EXPECTED_SHA256" \
  --arg workspace_003 "$WORKSPACE_003" \
  --arg workspace_004 "$WORKSPACE_004" \
  --arg full_backup "$FULL_BACKUP" \
  '{
    result: "PASS",
    platform: "Darwin arm64",
    native_windows_runtime: "PENDING_DOGFOOD",
    proof_root: $proof_root,
    binaries: {
      candidate_rc6_sha256: $rc6_sha256,
      source_rc4_sha256: $rc4_sha256,
      source_rc5_sha256: $rc5_sha256
    },
    fresh_rc6: true,
    update_rc4_to_rc6: true,
    update_rc5_to_rc6: true,
    mixed_workspace_schemas_before: [
      {workspace_key: $workspace_003, schema: "003"},
      {workspace_key: $workspace_004, schema: "004"}
    ],
    target_schema: "004",
    apply_invocations: 1,
    onboarding_questions_during_update: 0,
    backup_source_bytes_exact: true,
    task_lifecycle: true,
    tool_dedupe_plan: true,
    debug_export: true,
    ui_loopback: "separate_stage07_check",
    repository_local_aopmem: false,
    full_backup: $full_backup
  }' > "$PROOF_ROOT/summary.json"

printf '%s\n' "RC6 macOS Stage 07 proof: PASS"
printf '%s\n' "proof_root=$PROOF_ROOT"
printf '%s\n' "summary=$PROOF_ROOT/summary.json"
