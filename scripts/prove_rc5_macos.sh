#!/bin/sh

# Reproducible native macOS proof for RC5 Stage 24.
#
# The harness never opens SQLite directly. Source workspaces are created only
# through the exact published v0.1.0-rc3 and v0.2.0-rc4 CLIs. The RC5
# candidate must already be built; using it as a local installer asset is
# limited to the installer's explicit isolated test mode.

set -eu
umask 077
LC_ALL=C
export LC_ALL

REPO_ROOT=$(CDPATH= cd "$(dirname "$0")/.." && pwd -P)
INSTALLER="$REPO_ROOT/install/v0.2/install.sh"
RC5_BINARY=${AOPMEM_RC5_PROOF_BINARY:-"$REPO_ROOT/target/debug/aopmem"}
V01_EXPECTED_SHA256="d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607"
RC4_EXPECTED_SHA256="4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e"

fail() {
  printf '%s\n' "RC5 macOS proof failed: $1" >&2
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

assert_contains() {
  grep -Fq "$2" "$1" || fail "expected text is missing from $1: $2"
}

assert_not_contains() {
  if grep -Eq "$2" "$1"; then
    fail "unexpected text is present in $1: $2"
  fi
}

assert_count() {
  actual=$(grep -Fc "$2" "$1" || true)
  [ "$actual" -eq "$3" ] ||
    fail "unexpected count in $1 for '$2': expected $3, got $actual"
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
  destination=$1
  project=$2
  cat > "$destination" <<EOF
нет
нет
$project
Пользователь ведет проект, агент помогает.
Рабочий код, вспомогательные docs, архив нельзя менять.
EOF
}

normalize_stage24_nodes() {
  jq -S '
    [.data.nodes[]
      | select(.title | startswith("Stage24"))
      | {
          id,
          node_type,
          status,
          title,
          summary,
          body,
          source_ref,
          confidence,
          trust_level
        }]
  ' "$1" > "$2"
}

normalize_node_aliases() {
  jq -S '[.data.aliases[] | {id, node_id, alias}]' "$1" > "$2"
}

normalize_node_tags() {
  jq -S '[.data.tags[] | {id, node_id, tag}]' "$1" > "$2"
}

normalize_node_sources() {
  jq -S '[.data.sources[] | {id, node_id, source_ref}]' "$1" > "$2"
}

tool_tree_manifest() {
  home=$1
  output=$2
  : > "$output"
  find "$home/workspaces" -path '*/tools/*' -type f -print |
    LC_ALL=C sort |
    while IFS= read -r path; do
      relative=${path#"$home/"}
      printf '%s  %s\n' "$(sha256_file "$path")" "$relative"
    done > "$output"
}

workspace_database_manifest() {
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

for command_name in awk cmp find git grep jq mktemp shasum sort; do
  need_command "$command_name"
done
need_command gh

[ "$(uname -s)" = "Darwin" ] || fail "native proof requires Darwin"
[ "$(uname -m)" = "arm64" ] || fail "native proof requires Apple Silicon arm64"
assert_file "$INSTALLER"
assert_file "$RC5_BINARY"
[ -x "$RC5_BINARY" ] || fail "RC5 candidate is not executable: $RC5_BINARY"
[ "$("$RC5_BINARY" --version)" = "aopmem 0.2.0-rc5" ] ||
  fail "candidate must report exactly aopmem 0.2.0-rc5"

PROOF_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/aopmem-rc5-stage24.XXXXXX")
SOURCE_DIR="$PROOF_ROOT/published"
ASSET_DIR="$PROOF_ROOT/rc5-assets"
mkdir -p "$SOURCE_DIR" "$ASSET_DIR"

V01_BINARY="$SOURCE_DIR/aopmem-v0.1.0-rc3"
git -C "$REPO_ROOT" show \
  'v0.1.0-rc3:dist/aopmem-darwin-arm64/aopmem' > "$V01_BINARY"
chmod 700 "$V01_BINARY"
[ "$(sha256_file "$V01_BINARY")" = "$V01_EXPECTED_SHA256" ] ||
  fail "tagged v0.1.0-rc3 asset hash is wrong"
[ "$("$V01_BINARY" --version)" = "aopmem 0.1.0" ] ||
  fail "tagged v0.1.0-rc3 asset reports an unexpected version"

RC4_DOWNLOAD="$SOURCE_DIR/rc4-download"
mkdir "$RC4_DOWNLOAD"
gh release download v0.2.0-rc4 \
  --repo ARQAWA/aopmem-cli \
  --pattern aopmem-darwin-arm64 \
  --dir "$RC4_DOWNLOAD"
RC4_BINARY="$RC4_DOWNLOAD/aopmem-darwin-arm64"
chmod 700 "$RC4_BINARY"
[ "$(sha256_file "$RC4_BINARY")" = "$RC4_EXPECTED_SHA256" ] ||
  fail "published v0.2.0-rc4 asset hash is wrong"
[ "$("$RC4_BINARY" --version)" = "aopmem 0.2.0-rc4" ] ||
  fail "published v0.2.0-rc4 asset reports an unexpected version"

cp "$RC5_BINARY" "$ASSET_DIR/aopmem-darwin-arm64"
chmod 700 "$ASSET_DIR/aopmem-darwin-arm64"
RC5_SHA256=$(sha256_file "$ASSET_DIR/aopmem-darwin-arm64")
printf '%s  aopmem-darwin-arm64\n' "$RC5_SHA256" > "$ASSET_DIR/SHA256SUMS"

FRESH_ROOT="$PROOF_ROOT/fresh"
FRESH_HOME="$FRESH_ROOT/aopmem-home"
FRESH_FALLBACK_HOME="$FRESH_ROOT/home"
FRESH_REPO="$FRESH_ROOT/repo"
FRESH_TEMP="$FRESH_ROOT/temp"
mkdir -p "$FRESH_HOME" "$FRESH_FALLBACK_HOME" "$FRESH_REPO" "$FRESH_TEMP"
write_answers "$FRESH_ROOT/answers.txt" "Stage24 fresh project."

(
  cd "$FRESH_REPO"
  env \
    HOME="$FRESH_FALLBACK_HOME" \
    AOPMEM_HOME="$FRESH_HOME" \
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_OS=Darwin \
    AOPMEM_INSTALL_TEST_ARCH=arm64 \
    AOPMEM_INSTALL_TEST_RUN_ID=24001 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSET_DIR" \
    AOPMEM_INSTALL_TEST_TEMP_ROOT="$FRESH_TEMP" \
    AOPMEM_INSTALL_TEST_TRACE="$FRESH_ROOT/trace.log" \
    AOPMEM_ACTIVE_ADAPTER=codex \
    AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md \
    sh "$INSTALLER" \
      < "$FRESH_ROOT/answers.txt" \
      > "$FRESH_ROOT/stdout.log" \
      2> "$FRESH_ROOT/stderr.log"
)

[ "$("$FRESH_HOME/bin/aopmem" --version)" = "aopmem 0.2.0-rc5" ] ||
  fail "fresh installer did not publish RC5"
[ "$(sha256_file "$FRESH_HOME/bin/aopmem")" = "$RC5_SHA256" ] ||
  fail "fresh installed binary bytes differ from the verified candidate"
assert_file "$FRESH_REPO/AGENTS.md"
assert_contains "$FRESH_REPO/AGENTS.md" "AOPMEM CONTRACT VERSION: 2"
assert_count "$FRESH_ROOT/stdout.log" \
  "Включаем Understand Anything для локального понимания проекта и .understand.docs?" 1
assert_count "$FRESH_ROOT/stdout.log" \
  "Включаем Codebase Memory MCP для навигации по коду?" 1
assert_count "$FRESH_ROOT/stdout.log" \
  "Объясни, что это за проект, зачем он нужен и чем мы тут занимаемся." 1
assert_count "$FRESH_ROOT/stdout.log" \
  "Какая твоя роль в этом проекте и какая роль у агента?" 1
assert_count "$FRESH_ROOT/stdout.log" \
  "Какие части проекта рабочие, какие вспомогательные, какие нельзя трогать?" 1
assert_trace_before "$FRESH_ROOT/trace.log" "replacement.published" "init"
assert_trace_before "$FRESH_ROOT/trace.log" "init" "adapter.seed"
assert_trace_before "$FRESH_ROOT/trace.log" "adapter.seed" "doctor"
assert_trace_before "$FRESH_ROOT/trace.log" "doctor" "verify"
assert_trace_before "$FRESH_ROOT/trace.log" "verify" "task.start.smoke"
assert_trace_before "$FRESH_ROOT/trace.log" "observe.report" "debug.capsule.export"
assert_file "$FRESH_HOME/debug-capsules/upgrade-24001.zip"

UPDATE_ROOT="$PROOF_ROOT/update"
UPDATE_HOME="$UPDATE_ROOT/aopmem-home"
UPDATE_FALLBACK_HOME="$UPDATE_ROOT/home"
REPO_001="$UPDATE_ROOT/repo-schema001"
REPO_003="$UPDATE_ROOT/repo-schema003"
UPDATE_TEMP="$UPDATE_ROOT/temp"
mkdir -p \
  "$UPDATE_HOME/bin" \
  "$UPDATE_FALLBACK_HOME" \
  "$REPO_001" \
  "$REPO_003" \
  "$UPDATE_TEMP"

# Fixture provisioning only: the published old binaries create their own
# schemas and data. The update itself is performed only by install.sh.
cp "$V01_BINARY" "$UPDATE_HOME/bin/aopmem"
chmod 700 "$UPDATE_HOME/bin/aopmem"
write_answers "$UPDATE_ROOT/schema001-answers.txt" "Stage24 schema001 project."
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" init \
  < "$UPDATE_ROOT/schema001-answers.txt" \
  > "$UPDATE_ROOT/schema001-init.log"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node create \
  --type rule \
  --status active \
  --title "Stage24 V01 Rule" \
  --summary "Preserve schema001 logical data" \
  --body "STAGE24_V01_BODY_CANARY" \
  --source-ref "stage24:v01" \
  --confidence 0.91 \
  --trust-level explicit_user \
  --json > "$UPDATE_ROOT/v01-node.json"
V01_NODE_ID=$(jq -r '.data.id' "$UPDATE_ROOT/v01-node.json")
WORKSPACE_001=$(jq -r '.meta.workspace_key' "$UPDATE_ROOT/v01-node.json")
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" alias add \
  --node-id "$V01_NODE_ID" \
  --alias stage24-v01-rule \
  --json > "$UPDATE_ROOT/v01-alias.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tag add \
  --node-id "$V01_NODE_ID" \
  --tag stage24 \
  --json > "$UPDATE_ROOT/v01-tag.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" source add \
  --node-id "$V01_NODE_ID" \
  --source-ref stage24:v01:secondary \
  --json > "$UPDATE_ROOT/v01-source.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tool create-draft \
  --id stage24_v01_tool \
  --name "Stage24 v01 tool" \
  --entrypoint run.sh \
  --side-effects none \
  --approval-requirement none \
  --json > "$UPDATE_ROOT/v01-tool.json"
V01_TOOL_ENTRYPOINT="$UPDATE_HOME/workspaces/$WORKSPACE_001/tools/stage24_v01_tool/run.sh"
printf '#!/bin/sh\nexit 0\n' > "$V01_TOOL_ENTRYPOINT"
chmod 700 "$V01_TOOL_ENTRYPOINT"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tool validate \
  stage24_v01_tool \
  --json > "$UPDATE_ROOT/v01-tool-validate.json"

cp "$RC4_BINARY" "$UPDATE_HOME/bin/aopmem"
chmod 700 "$UPDATE_HOME/bin/aopmem"
write_answers "$UPDATE_ROOT/schema003-answers.txt" "Stage24 schema003 project."
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" init \
  < "$UPDATE_ROOT/schema003-answers.txt" \
  > "$UPDATE_ROOT/schema003-init.log"
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node create \
  --type workflow \
  --status active \
  --title "Stage24 RC4 Workflow" \
  --summary "Preserve schema003 logical data" \
  --body "STAGE24_RC4_BODY_CANARY" \
  --source-ref "stage24:rc4" \
  --confidence 0.92 \
  --trust-level explicit_user \
  --json > "$UPDATE_ROOT/rc4-node.json"
RC4_NODE_ID=$(jq -r '.data.id' "$UPDATE_ROOT/rc4-node.json")
WORKSPACE_003=$(jq -r '.meta.workspace_key' "$UPDATE_ROOT/rc4-node.json")
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" alias add \
  --node-id "$RC4_NODE_ID" \
  --alias stage24-rc4-workflow \
  --json > "$UPDATE_ROOT/rc4-alias.json"
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tool create-draft \
  --id stage24_rc4_tool \
  --name "Stage24 rc4 tool" \
  --entrypoint run.sh \
  --side-effects none \
  --approval-requirement none \
  --json > "$UPDATE_ROOT/rc4-tool.json"
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" adapter seed \
  --file AGENTS.md \
  --json > "$UPDATE_ROOT/rc4-adapter-seed.json"
printf '\nSTAGE24 USER TEXT MUST SURVIVE\n' >> "$REPO_003/AGENTS.md"
mkdir -p "$REPO_003/.cursor/rules" "$REPO_003/.github"
printf '%s\n' "CLAUDE_STAGE24_UNSELECTED" > "$REPO_003/CLAUDE.md"
printf '%s\n' "CURSOR_STAGE24_UNSELECTED" > "$REPO_003/.cursor/rules/aopmem.mdc"
printf '%s\n' "COPILOT_STAGE24_UNSELECTED" \
  > "$REPO_003/.github/copilot-instructions.md"
printf '%s\n' "OTHER_STAGE24_UNCHANGED" > "$REPO_003/OTHER.txt"

# Produce a genuine rc4 pending marker by making one audit Git commit fail.
# The operational mutation must commit with AUDIT_SNAPSHOT_PENDING. The
# temporary obstruction is restored before the RC5 installer runs.
AUDIT_003="$UPDATE_HOME/workspaces/$WORKSPACE_003/audit-git"
mv "$AUDIT_003/.git/objects" "$AUDIT_003/.git/objects.stage24-saved"
printf '%s\n' "stage24 obstruction" > "$AUDIT_003/.git/objects"
set +e
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node create \
  --type rule \
  --status active \
  --title "Stage24 pending repair canary" \
  --summary "Must be recovered by official audit repair" \
  --source-ref stage24:pending-repair \
  --confidence 0.9 \
  --trust-level explicit_user \
  --json \
  > "$UPDATE_ROOT/pending-trigger.json" \
  2> "$UPDATE_ROOT/pending-trigger.stderr"
pending_status=$?
set -e
rm "$AUDIT_003/.git/objects"
mv "$AUDIT_003/.git/objects.stage24-saved" "$AUDIT_003/.git/objects"
[ "$pending_status" -eq 0 ] || fail "rc4 pending-marker mutation failed"
assert_contains "$UPDATE_ROOT/pending-trigger.json" "AUDIT_SNAPSHOT_PENDING"
assert_file "$AUDIT_003/.pending-snapshot"

run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node list \
  --all --include-body --json > "$UPDATE_ROOT/pre-v01-nodes.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" alias list \
  --node-id "$V01_NODE_ID" --all --json > "$UPDATE_ROOT/pre-v01-aliases.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tag list \
  --node-id "$V01_NODE_ID" --all --json > "$UPDATE_ROOT/pre-v01-tags.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" source list \
  --node-id "$V01_NODE_ID" --all --json > "$UPDATE_ROOT/pre-v01-sources.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tool list \
  --all --json > "$UPDATE_ROOT/pre-v01-tools.json"
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node list \
  --all --include-body --json > "$UPDATE_ROOT/pre-rc4-nodes.json"
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" alias list \
  --node-id "$RC4_NODE_ID" --all --json > "$UPDATE_ROOT/pre-rc4-aliases.json"
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tool list \
  --all --json > "$UPDATE_ROOT/pre-rc4-tools.json"

normalize_stage24_nodes \
  "$UPDATE_ROOT/pre-v01-nodes.json" "$UPDATE_ROOT/pre-v01-nodes.norm.json"
normalize_stage24_nodes \
  "$UPDATE_ROOT/pre-rc4-nodes.json" "$UPDATE_ROOT/pre-rc4-nodes.norm.json"
normalize_node_aliases \
  "$UPDATE_ROOT/pre-v01-aliases.json" "$UPDATE_ROOT/pre-v01-aliases.norm.json"
normalize_node_tags \
  "$UPDATE_ROOT/pre-v01-tags.json" "$UPDATE_ROOT/pre-v01-tags.norm.json"
normalize_node_sources \
  "$UPDATE_ROOT/pre-v01-sources.json" "$UPDATE_ROOT/pre-v01-sources.norm.json"
normalize_node_aliases \
  "$UPDATE_ROOT/pre-rc4-aliases.json" "$UPDATE_ROOT/pre-rc4-aliases.norm.json"
tool_tree_manifest "$UPDATE_HOME" "$UPDATE_ROOT/pre-tools.sha256"
workspace_database_manifest \
  "$UPDATE_HOME" "$WORKSPACE_001" "$WORKSPACE_003" \
  "$UPDATE_ROOT/pre-database-files.sha256"

SELECTED_ADAPTER_BEFORE=$(sha256_file "$REPO_003/AGENTS.md")
CLAUDE_BEFORE=$(sha256_file "$REPO_003/CLAUDE.md")
CURSOR_BEFORE=$(sha256_file "$REPO_003/.cursor/rules/aopmem.mdc")
COPILOT_BEFORE=$(sha256_file "$REPO_003/.github/copilot-instructions.md")
OTHER_BEFORE=$(sha256_file "$REPO_003/OTHER.txt")
OLD_BINARY_SHA256=$(sha256_file "$UPDATE_HOME/bin/aopmem")
[ "$OLD_BINARY_SHA256" = "$RC4_EXPECTED_SHA256" ] ||
  fail "active update binary is not the published rc4 asset"

(
  cd "$REPO_003"
  env \
    HOME="$UPDATE_FALLBACK_HOME" \
    AOPMEM_HOME="$UPDATE_HOME" \
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_OS=Darwin \
    AOPMEM_INSTALL_TEST_ARCH=arm64 \
    AOPMEM_INSTALL_TEST_RUN_ID=24002 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSET_DIR" \
    AOPMEM_INSTALL_TEST_TEMP_ROOT="$UPDATE_TEMP" \
    AOPMEM_INSTALL_TEST_TRACE="$UPDATE_ROOT/update-trace.log" \
    AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256="$OLD_BINARY_SHA256" \
    AOPMEM_ACTIVE_ADAPTER=codex \
    AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md \
    sh "$INSTALLER" \
      < /dev/null \
      > "$UPDATE_ROOT/update-stdout.log" \
      2> "$UPDATE_ROOT/update-stderr.log"
)

[ "$("$UPDATE_HOME/bin/aopmem" --version)" = "aopmem 0.2.0-rc5" ] ||
  fail "update installer did not publish RC5"
[ "$(sha256_file "$UPDATE_HOME/bin/aopmem")" = "$RC5_SHA256" ] ||
  fail "updated binary bytes differ from the verified candidate"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "process.gate.clear" "backup.created"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "backup.created" "backup.home.created"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "backup.home.created" "asset.download.started"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "platform.check.staged" "audit.repair.staged"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "audit.repair.staged" "upgrade.prepare"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "upgrade.prepare" "upgrade.plan"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "upgrade.plan" "upgrade.apply"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "upgrade.apply" "upgrade.publish"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "upgrade.publish" "adapter.sync"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "adapter.sync" "audit.repair.post-publish"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "verify" "task.start.smoke"
assert_trace_before \
  "$UPDATE_ROOT/update-trace.log" \
  "observe.report" "debug.capsule.export"
assert_not_contains "$UPDATE_ROOT/update-trace.log" '^(init|adapter\.seed)$'
[ "$(grep -Ec '^upgrade\.apply$' "$UPDATE_ROOT/update-trace.log")" -eq 1 ] ||
  fail "official update did not invoke apply exactly once"
[ ! -e "$AUDIT_003/.pending-snapshot" ] ||
  fail "official audit repair did not clear the real pending marker"

FULL_BACKUP=$(find "$UPDATE_ROOT" -maxdepth 1 -type d \
  -name 'aopmem-home-backup-v0.2.0-rc5-*' -print -quit)
[ -n "$FULL_BACKUP" ] || fail "durable full-home backup is missing"
assert_file "$FULL_BACKUP/MANIFEST.sha256"
assert_file "$FULL_BACKUP/workspaces/$WORKSPACE_003/audit-git/.pending-snapshot"
workspace_database_manifest \
  "$FULL_BACKUP" "$WORKSPACE_001" "$WORKSPACE_003" \
  "$UPDATE_ROOT/backup-database-files.sha256"
cmp \
  "$UPDATE_ROOT/pre-database-files.sha256" \
  "$UPDATE_ROOT/backup-database-files.sha256" ||
  fail "source SQLite files and durable backup bytes differ"
[ "$(sha256_file "$FULL_BACKUP/bin/aopmem")" = "$RC4_EXPECTED_SHA256" ] ||
  fail "full-home backup did not retain the exact rc4 binary"
OLD_BINARY_BACKUP=$(find "$UPDATE_HOME/bin" -maxdepth 1 -type f \
  -name 'aopmem.backup-v0.2.0-rc4-*' -print -quit)
[ -n "$OLD_BINARY_BACKUP" ] || fail "old binary backup is missing"
[ "$(sha256_file "$OLD_BINARY_BACKUP")" = "$RC4_EXPECTED_SHA256" ] ||
  fail "old binary backup bytes differ from rc4"

run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node list \
  --all --include-body --json > "$UPDATE_ROOT/post-v01-nodes.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" alias list \
  --node-id "$V01_NODE_ID" --all --json > "$UPDATE_ROOT/post-v01-aliases.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" tag list \
  --node-id "$V01_NODE_ID" --all --json > "$UPDATE_ROOT/post-v01-tags.json"
run_cli \
  "$REPO_001" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" source list \
  --node-id "$V01_NODE_ID" --all --json > "$UPDATE_ROOT/post-v01-sources.json"
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" node list \
  --all --include-body --json > "$UPDATE_ROOT/post-rc4-nodes.json"
run_cli \
  "$REPO_003" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
  "$UPDATE_HOME/bin/aopmem" alias list \
  --node-id "$RC4_NODE_ID" --all --json > "$UPDATE_ROOT/post-rc4-aliases.json"
normalize_stage24_nodes \
  "$UPDATE_ROOT/post-v01-nodes.json" "$UPDATE_ROOT/post-v01-nodes.norm.json"
normalize_stage24_nodes \
  "$UPDATE_ROOT/post-rc4-nodes.json" "$UPDATE_ROOT/post-rc4-nodes.norm.json"
normalize_node_aliases \
  "$UPDATE_ROOT/post-v01-aliases.json" "$UPDATE_ROOT/post-v01-aliases.norm.json"
normalize_node_tags \
  "$UPDATE_ROOT/post-v01-tags.json" "$UPDATE_ROOT/post-v01-tags.norm.json"
normalize_node_sources \
  "$UPDATE_ROOT/post-v01-sources.json" "$UPDATE_ROOT/post-v01-sources.norm.json"
normalize_node_aliases \
  "$UPDATE_ROOT/post-rc4-aliases.json" "$UPDATE_ROOT/post-rc4-aliases.norm.json"
cmp "$UPDATE_ROOT/pre-v01-nodes.norm.json" "$UPDATE_ROOT/post-v01-nodes.norm.json" ||
  fail "schema001 logical node canaries changed"
cmp "$UPDATE_ROOT/pre-rc4-nodes.norm.json" "$UPDATE_ROOT/post-rc4-nodes.norm.json" ||
  fail "schema003 logical node canaries changed"
cmp "$UPDATE_ROOT/pre-v01-aliases.norm.json" "$UPDATE_ROOT/post-v01-aliases.norm.json" ||
  fail "schema001 node aliases changed"
cmp "$UPDATE_ROOT/pre-v01-tags.norm.json" "$UPDATE_ROOT/post-v01-tags.norm.json" ||
  fail "schema001 node tags changed"
cmp "$UPDATE_ROOT/pre-v01-sources.norm.json" "$UPDATE_ROOT/post-v01-sources.norm.json" ||
  fail "schema001 node sources changed"
cmp "$UPDATE_ROOT/pre-rc4-aliases.norm.json" "$UPDATE_ROOT/post-rc4-aliases.norm.json" ||
  fail "schema003 node aliases changed"
tool_tree_manifest "$UPDATE_HOME" "$UPDATE_ROOT/post-tools.sha256"
cmp "$UPDATE_ROOT/pre-tools.sha256" "$UPDATE_ROOT/post-tools.sha256" ||
  fail "tool filesystem bytes changed"

[ "$SELECTED_ADAPTER_BEFORE" != "$(sha256_file "$REPO_003/AGENTS.md")" ] ||
  fail "selected rc4 adapter was not upgraded"
[ "$CLAUDE_BEFORE" = "$(sha256_file "$REPO_003/CLAUDE.md")" ] ||
  fail "unselected Claude adapter changed"
[ "$CURSOR_BEFORE" = "$(sha256_file "$REPO_003/.cursor/rules/aopmem.mdc")" ] ||
  fail "unselected Cursor adapter changed"
[ "$COPILOT_BEFORE" = \
  "$(sha256_file "$REPO_003/.github/copilot-instructions.md")" ] ||
  fail "unselected Copilot adapter changed"
[ "$OTHER_BEFORE" = "$(sha256_file "$REPO_003/OTHER.txt")" ] ||
  fail "unrelated project file changed"
assert_contains "$REPO_003/AGENTS.md" "STAGE24 USER TEXT MUST SURVIVE"
assert_count "$REPO_003/AGENTS.md" "AOPMEM CONTRACT VERSION: 2" 1

for item in "v01:$REPO_001" "rc4:$REPO_003"; do
  label=${item%%:*}
  repo=${item#*:}
  run_cli \
    "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" doctor \
    --json > "$UPDATE_ROOT/$label-doctor.json"
  run_cli \
    "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" verify \
    --json > "$UPDATE_ROOT/$label-verify.json"
  printf 'Stage24 %s task start' "$label" |
    run_cli \
      "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
      "$UPDATE_HOME/bin/aopmem" task start \
      --query-stdin --json > "$UPDATE_ROOT/$label-task-start.json"
  run_cli \
    "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" observe status \
    --json > "$UPDATE_ROOT/$label-observe-status.json"
  run_cli \
    "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" observe report \
    --json > "$UPDATE_ROOT/$label-observe-report.json"
  run_cli \
    "$repo" "$UPDATE_HOME" "$UPDATE_FALLBACK_HOME" \
    "$UPDATE_HOME/bin/aopmem" observe export \
    --output "$UPDATE_ROOT/$label-capsule.zip" \
    --json > "$UPDATE_ROOT/$label-export.json"
  [ "$(jq -r '.data.clean' "$UPDATE_ROOT/$label-verify.json")" = "true" ] ||
    fail "$label workspace verify is not clean"
  [ "$(jq -r '.data.mandatory_context_complete' \
    "$UPDATE_ROOT/$label-task-start.json")" = "true" ] ||
    fail "$label task-start mandatory context is incomplete"
  [ "$(jq -r '.data.retrieval_complete' \
    "$UPDATE_ROOT/$label-task-start.json")" = "true" ] ||
    fail "$label task-start retrieval is incomplete"
  [ "$(jq -r '.data.observability_schema_version' \
    "$UPDATE_ROOT/$label-observe-status.json")" = "2" ] ||
    fail "$label observability store is not schema v2"
  [ "$(jq -r '.ok' "$UPDATE_ROOT/$label-export.json")" = "true" ] ||
    fail "$label debug capsule export failed"
  assert_file "$UPDATE_ROOT/$label-capsule.zip"
done

[ "$(jq -r '.data.healthy' "$UPDATE_ROOT/rc4-doctor.json")" = "true" ] ||
  fail "selected active rc4 workspace doctor is not healthy"
# Only the explicitly selected adapter may change. The other workspace has no
# instruction file, so its adapter check is intentionally missing while every
# operational health component is ready.
[ "$(jq -r '.data.checks.db.status' "$UPDATE_ROOT/v01-doctor.json")" = "ready" ] ||
  fail "schema001 workspace DB is not ready"
[ "$(jq -r '.data.checks.schema.status' "$UPDATE_ROOT/v01-doctor.json")" = "ready" ] ||
  fail "schema001 workspace schema is not ready"
[ "$(jq -r '.data.checks.audit_snapshot.status' \
  "$UPDATE_ROOT/v01-doctor.json")" = "ready" ] ||
  fail "schema001 workspace audit snapshot is not ready"
[ "$(jq -r '.data.checks.tools_dirs.status' "$UPDATE_ROOT/v01-doctor.json")" = "ready" ] ||
  fail "schema001 workspace tools path is not ready"
[ "$(jq -r '.data.checks.adapter_block.status' \
  "$UPDATE_ROOT/v01-doctor.json")" = "missing" ] ||
  fail "unselected schema001 adapter state is unexpected"

# CLI commands can leave ordinary SQLite coordination sidecars. Use the
# official prepare command, never manual WAL/SHM deletion, before the final
# read-only all-workspace schema plan.
HOME="$UPDATE_FALLBACK_HOME" AOPMEM_HOME="$UPDATE_HOME" \
  "$UPDATE_HOME/bin/aopmem" upgrade prepare \
  --all-workspaces --json > "$UPDATE_ROOT/post-prepare.json"
HOME="$UPDATE_FALLBACK_HOME" AOPMEM_HOME="$UPDATE_HOME" \
  "$UPDATE_HOME/bin/aopmem" upgrade plan \
  --all-workspaces --json > "$UPDATE_ROOT/post-plan.json"
[ "$(jq -r '.data.success' "$UPDATE_ROOT/post-prepare.json")" = "true" ] ||
  fail "post-proof official prepare failed"
[ "$(jq -r '.data.ready' "$UPDATE_ROOT/post-plan.json")" = "true" ] ||
  fail "post-proof all-workspace plan is not ready"
[ "$(jq -r '.data.writes_performed' "$UPDATE_ROOT/post-plan.json")" = "false" ] ||
  fail "post-proof plan performed writes"
[ "$(jq '[.data.workspaces[] |
  select(.schema.current_version == "004" and
         .schema.target_version == "004" and
         (.schema.pending_migrations | length) == 0)] | length' \
  "$UPDATE_ROOT/post-plan.json")" -eq 2 ] ||
  fail "both workspaces are not exactly schema 004"

FAILURE_ROOT="$PROOF_ROOT/failure-before-download"
FAILURE_HOME="$FAILURE_ROOT/aopmem-home"
FAILURE_FALLBACK_HOME="$FAILURE_ROOT/home"
FAILURE_TEMP="$FAILURE_ROOT/temp"
mkdir -p "$FAILURE_HOME" "$FAILURE_FALLBACK_HOME" "$FAILURE_TEMP"
for entry in bin skills templates workspaces; do
  if [ -e "$FULL_BACKUP/$entry" ]; then
    cp -Rp "$FULL_BACKUP/$entry" "$FAILURE_HOME/$entry"
  fi
done
FAILURE_BINARY_BEFORE=$(sha256_file "$FAILURE_HOME/bin/aopmem")
FAILURE_DB_001_BEFORE=$(
  sha256_file "$FAILURE_HOME/workspaces/$WORKSPACE_001/aopmem.sqlite"
)
FAILURE_DB_003_BEFORE=$(
  sha256_file "$FAILURE_HOME/workspaces/$WORKSPACE_003/aopmem.sqlite"
)
FAILURE_ADAPTER_BEFORE=$(sha256_file "$REPO_003/AGENTS.md")
set +e
(
  cd "$REPO_003"
  env \
    HOME="$FAILURE_FALLBACK_HOME" \
    AOPMEM_HOME="$FAILURE_HOME" \
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_OS=Darwin \
    AOPMEM_INSTALL_TEST_ARCH=arm64 \
    AOPMEM_INSTALL_TEST_RUN_ID=24003 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$ASSET_DIR" \
    AOPMEM_INSTALL_TEST_TEMP_ROOT="$FAILURE_TEMP" \
    AOPMEM_INSTALL_TEST_TRACE="$FAILURE_ROOT/trace.log" \
    AOPMEM_INSTALL_TEST_OLD_BINARY_SHA256="$FAILURE_BINARY_BEFORE" \
    AOPMEM_INSTALL_TEST_FAIL_AT=after_backup \
    AOPMEM_ACTIVE_ADAPTER=codex \
    AOPMEM_ACTIVE_INSTRUCTION_FILE=AGENTS.md \
    sh "$INSTALLER" \
      < /dev/null \
      > "$FAILURE_ROOT/stdout.log" \
      2> "$FAILURE_ROOT/stderr.log"
)
failure_status=$?
set -e
[ "$failure_status" -ne 0 ] || fail "injected pre-download failure unexpectedly succeeded"
[ "$FAILURE_BINARY_BEFORE" = "$(sha256_file "$FAILURE_HOME/bin/aopmem")" ] ||
  fail "pre-download failure changed the installed binary"
[ "$FAILURE_DB_001_BEFORE" = \
  "$(sha256_file "$FAILURE_HOME/workspaces/$WORKSPACE_001/aopmem.sqlite")" ] ||
  fail "pre-download failure changed the schema001 database"
[ "$FAILURE_DB_003_BEFORE" = \
  "$(sha256_file "$FAILURE_HOME/workspaces/$WORKSPACE_003/aopmem.sqlite")" ] ||
  fail "pre-download failure changed the schema003 database"
[ "$FAILURE_ADAPTER_BEFORE" = "$(sha256_file "$REPO_003/AGENTS.md")" ] ||
  fail "pre-download failure changed the selected adapter"
assert_not_contains \
  "$FAILURE_ROOT/trace.log" \
  '^(asset\.download\.started|upgrade\.prepare|upgrade\.plan|upgrade\.apply|upgrade\.publish)$'
assert_contains "$FAILURE_ROOT/stderr.log" "old binary was left unchanged"
FAILURE_BACKUP=$(find "$FAILURE_ROOT" -maxdepth 1 -type d \
  -name 'aopmem-home-backup-v0.2.0-rc5-*' -print -quit)
[ -n "$FAILURE_BACKUP" ] || fail "pre-download failure did not retain its backup"

jq -n \
  --arg proof_root "$PROOF_ROOT" \
  --arg rc5_sha256 "$RC5_SHA256" \
  --arg v01_sha256 "$V01_EXPECTED_SHA256" \
  --arg rc4_sha256 "$RC4_EXPECTED_SHA256" \
  --arg workspace_001 "$WORKSPACE_001" \
  --arg workspace_003 "$WORKSPACE_003" \
  --arg full_backup "$FULL_BACKUP" \
  --arg failure_backup "$FAILURE_BACKUP" \
  '{
    result: "PASS",
    platform: "Darwin arm64",
    native_windows_runtime: "PENDING_DOGFOOD",
    proof_root: $proof_root,
    binaries: {
      source_v01_sha256: $v01_sha256,
      source_rc4_sha256: $rc4_sha256,
      candidate_rc5_sha256: $rc5_sha256
    },
    fresh: {
      five_answers: true,
      installed_rc5: true,
      adapter_v2: true,
      doctor_verify_task_observe_export: true
    },
    update: {
      source_workspaces: [
        {workspace_key: $workspace_001, schema: "001"},
        {workspace_key: $workspace_003, schema: "003", active_binary: "rc4"}
      ],
      target_schema: "004",
      observability_schema: 2,
      apply_invocations: 1,
      onboarding_questions: 0,
      real_pending_marker_repaired: true,
      logical_nodes_preserved: true,
      node_aliases_preserved: true,
      node_tags_preserved: true,
      node_sources_preserved: true,
      tool_bytes_preserved: true,
      selected_adapter_only: true,
      source_backup_database_bytes_exact: true,
      full_backup: $full_backup
    },
    failure_safety: {
      stopped_before_download: true,
      binary_and_databases_unchanged: true,
      backup_retained: $failure_backup
    },
    prohibited_methods: {
      manual_sqlite: false,
      manual_wal_shm_deletion: false,
      admin: false,
      wsl: false,
      source_build_install: false
    }
  }' > "$PROOF_ROOT/summary.json"

printf '%s\n' "RC5 macOS Stage 24 proof: PASS"
printf '%s\n' "proof_root=$PROOF_ROOT"
printf '%s\n' "summary=$PROOF_ROOT/summary.json"
