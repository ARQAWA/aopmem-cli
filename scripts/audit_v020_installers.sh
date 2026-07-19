#!/bin/sh

set -eu
umask 077
LC_ALL=C
export LC_ALL

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
case "${TMPDIR:-}" in
  "") tmp_parent=/tmp ;;
  *) tmp_parent=${TMPDIR%/} ;;
esac
case_root=$(mktemp -d "$tmp_parent/aopmem-rc9-audit.XXXXXX")

cleanup() {
  rm -rf "$case_root"
}
trap cleanup EXIT HUP INT TERM

fail() {
  printf '%s\n' "FAIL: $1" >&2
  exit 1
}

pass() {
  printf '%s\n' "PASS: $1"
}

write_fake_aopmem() {
  path=$1
  version=$2
  cat > "$path" <<EOF
#!/bin/sh
set -eu
if [ "\${1:-}" = "--version" ]; then
  printf '%s\n' "aopmem $version"
  exit 0
fi
if [ "\${1:-}" = "--json" ]; then
  shift
fi
case "\${1:-} \${2:-}" in
  "platform check"|"adapter sync")
    printf '%s\n' '{"ok":true,"data":{}}'
    ;;
  "doctor "|"verify "|"observe status")
    printf '%s\n' '{"ok":true,"data":{}}'
    ;;
  "init ")
    mkdir -p "\$AOPMEM_HOME/workspaces/fake-workspace"
    printf '%s\n' '{"ok":true,"data":{"workspace_key":"fake-workspace"}}'
    ;;
  *)
    printf '%s\n' '{"ok":true,"data":{}}'
    ;;
esac
EOF
  chmod 755 "$path"
}

make_assets() {
  asset_dir=$1
  version=$2
  mkdir -p "$asset_dir"
  write_fake_aopmem "$asset_dir/aopmem-darwin-arm64" "$version"
  write_fake_aopmem "$asset_dir/aopmem-windows-x86_64.exe" "$version"
  (
    cd "$asset_dir"
    shasum -a 256 aopmem-darwin-arm64 aopmem-windows-x86_64.exe > SHA256SUMS
  )
}

assert_cli_upgrade_absent() {
  bin=${AOPMEM_AUDIT_BIN:-"$repo_root/target/debug/aopmem"}
  (cd "$repo_root" && cargo build --locked >/dev/null)
  "$bin" --help | grep -q 'upgrade' &&
    fail "top-level help contains upgrade"
  if "$bin" upgrade >"$case_root/upgrade.out" 2>"$case_root/upgrade.err"; then
    fail "aopmem upgrade unexpectedly succeeded"
  fi
  grep -Eiq "unrecognized subcommand|invalid subcommand|unexpected argument" \
    "$case_root/upgrade.err" ||
    fail "aopmem upgrade did not return standard unknown-command output"
  pass "upgrade CLI absent"
}

assert_no_current_update_contract() {
  rg -n "aopmem upgrade|upgrade backup|upgrade recovery|upgrade prepare|upgrade plan|upgrade apply|upgrade publish|backup --adopt|publish-resume" \
    "$repo_root/install/v0.2" \
    "$repo_root/docs/WINDOWS_CLEAN_TRANSPLANT.md" \
    "$repo_root/.devplan/RELEASE_CANDIDATE_v0.2.0-rc9.md" \
    "$repo_root/.devplan/GITHUB_RELEASE_NOTES_v0.2.0-rc9.md" \
    "$repo_root/.devplan/RC9_WINDOWS_TRANSPLANT_PROMPT.md" \
    > "$case_root/current-doc-grep.txt" &&
    fail "current installer/docs still mention updater terms"
  pass "current docs contain no updater contract"
}

assert_clean_installer_fresh_home() {
  asset_dir="$case_root/assets-fresh"
  home="$case_root/fresh-home"
  trace="$case_root/fresh.trace"
  repo="$case_root/repo-fresh"
  mkdir -p "$repo"
  make_assets "$asset_dir" "0.2.0-rc9"
  (
    cd "$repo"
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$asset_dir" \
    AOPMEM_INSTALL_TEST_TRACE="$trace" \
    AOPMEM_HOME="$home" \
      sh "$repo_root/install/v0.2/install.sh" >/dev/null
  )
  [ -x "$home/bin/aopmem" ] || fail "fresh install did not install binary"
  [ ! -e "$repo/.aopmem" ] || fail "fresh install created repo-local .aopmem"
  for event in home.clean asset.verified binary.installed platform.check init adapter.sync doctor.verify; do
    grep -qx "$event" "$trace" || fail "missing installer trace event: $event"
  done
  pass "clean installer fresh home"
}

assert_clean_installer_empty_home() {
  asset_dir="$case_root/assets-empty"
  home="$case_root/empty-home"
  repo="$case_root/repo-empty"
  mkdir -p "$home" "$repo"
  make_assets "$asset_dir" "0.2.0-rc9"
  (
    cd "$repo"
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$asset_dir" \
    AOPMEM_HOME="$home" \
      sh "$repo_root/install/v0.2/install.sh" >/dev/null
  )
  [ -x "$home/bin/aopmem" ] || fail "empty-home install did not install binary"
  pass "clean installer existing empty home"
}

assert_clean_installer_populated_home_untouched() {
  asset_dir="$case_root/assets-populated"
  home="$case_root/populated-home"
  repo="$case_root/repo-populated"
  mkdir -p "$home" "$repo"
  printf '%s\n' "keep" > "$home/sentinel.txt"
  before=$(shasum -a 256 "$home/sentinel.txt" | awk '{print $1}')
  make_assets "$asset_dir" "0.2.0-rc9"
  if (
    cd "$repo"
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$asset_dir" \
    AOPMEM_HOME="$home" \
      sh "$repo_root/install/v0.2/install.sh" >"$case_root/populated.out" 2>"$case_root/populated.err"
  ); then
    fail "populated home install unexpectedly succeeded"
  fi
  grep -q "CLEAN_INSTALL_REQUIRES_EMPTY_HOME" "$case_root/populated.err" ||
    fail "populated home did not fail with CLEAN_INSTALL_REQUIRES_EMPTY_HOME"
  after=$(shasum -a 256 "$home/sentinel.txt" | awk '{print $1}')
  [ "$before" = "$after" ] || fail "populated home was mutated"
  [ ! -e "$home/bin/aopmem" ] || fail "populated home binary was written"
  pass "clean installer populated home untouched"
}

assert_clean_installer_hash_verification() {
  asset_dir="$case_root/assets-bad-hash"
  home="$case_root/bad-hash-home"
  repo="$case_root/repo-bad-hash"
  mkdir -p "$repo"
  make_assets "$asset_dir" "0.2.0-rc9"
  printf '%s\n' "0000000000000000000000000000000000000000000000000000000000000000  aopmem-darwin-arm64" \
    > "$asset_dir/SHA256SUMS"
  if (
    cd "$repo"
    AOPMEM_INSTALL_TEST_MODE=1 \
    AOPMEM_INSTALL_TEST_ASSET_DIR="$asset_dir" \
    AOPMEM_HOME="$home" \
      sh "$repo_root/install/v0.2/install.sh" >"$case_root/hash.out" 2>"$case_root/hash.err"
  ); then
    fail "hash mismatch install unexpectedly succeeded"
  fi
  grep -q "SHA256 mismatch" "$case_root/hash.err" ||
    fail "hash mismatch was not reported"
  [ ! -e "$home/bin/aopmem" ] || fail "hash mismatch wrote binary"
  pass "clean installer hash verification"
}

create_transplant_fixture() {
  root=$1
  python3 - "$repo_root" "$root" <<'PY'
import importlib.util
from pathlib import Path
import sqlite3
import sys

repo = Path(sys.argv[1])
root = Path(sys.argv[2])
spec = importlib.util.spec_from_file_location(
    "harness", repo / "scripts/windows_rc4_to_rc9_transplant.py"
)
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)
for key in ["p-sit-cat-rental-8ef3bf83", "p-sit-warranty-5708363a"]:
    db = root / "workspaces" / key / "aopmem.sqlite"
    db.parent.mkdir(parents=True, exist_ok=True)
    con = sqlite3.connect(db)
    con.executescript(harness.SCHEMA_SQL)
    con.execute(
        "INSERT INTO nodes (id,node_type,status,title,summary,body,source_ref,confidence,trust_level) VALUES (1,'raw_note','active',?,?,?,'fixture',1.0,'high')",
        (f"{key} memory", "summary", "body"),
    )
    con.execute(
        "INSERT INTO nodes (id,node_type,status,title) VALUES (2,'lesson','active','linked lesson')"
    )
    con.execute(
        "INSERT INTO links (id,source_node_id,target_node_id,link_type) VALUES (1,1,2,'relates')"
    )
    con.execute("INSERT INTO aliases (id,node_id,alias) VALUES (1,1,?)", (f"{key}-alias",))
    con.execute("INSERT INTO tags (id,node_id,tag) VALUES (1,1,'tag-one')")
    con.execute("INSERT INTO sources (id,node_id,source_ref) VALUES (1,1,'source-one')")
    con.execute(
        "INSERT INTO registries (id,registry_type,name,status,notes) VALUES (1,'tool','reg','active','notes')"
    )
    con.execute(
        "INSERT INTO tool_contracts (id,tool_id,name,status,side_effects,approval_requirement,contract_json) VALUES (1,'reader','Reader','active','none','none','{}')"
    )
    con.execute(
        "INSERT INTO mcp_profiles (id,name,kind,status,read_operations,write_operations,side_effects,approval_requirement) VALUES ('mcp','MCP','optional','installed','read','write','local_read','none')"
    )
    con.execute(
        "INSERT INTO tool_aliases (alias,canonical_tool_id,source,status) VALUES ('read','reader','fixture','active')"
    )
    con.commit()
    con.close()
    for name, text in [
        ("tools/user tool.txt", "tool"),
        ("runtimes/кириллица/runtime.txt", "runtime"),
        ("artifacts/space dir/artifact.txt", "artifact"),
        ("secrets/token.txt", "secret"),
    ]:
        path = db.parent / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    try:
        (db.parent / "tools/link").symlink_to(db)
    except OSError:
        pass
(root / "bin").mkdir(parents=True, exist_ok=True)
PY
  write_fake_aopmem "$root/bin/aopmem" "0.2.0-rc4"
}

assert_transplant_success() {
  live="$case_root/transplant-live"
  quarantine="$case_root/transplant-q/home"
  failed="$case_root/transplant-f/home"
  report="$case_root/transplant-report.json"
  rc9="$case_root/fake-rc9"
  create_transplant_fixture "$live"
  write_fake_aopmem "$rc9" "0.2.0-rc9"
  before=$(shasum -a 256 "$live/workspaces/p-sit-cat-rental-8ef3bf83/aopmem.sqlite" | awk '{print $1}')
  python3 "$repo_root/scripts/windows_rc4_to_rc9_transplant.py" \
    --action Execute \
    --live-home "$live" \
    --quarantine-root "$quarantine" \
    --failed-root "$failed" \
    --rc9-binary "$rc9" \
    --report "$report" \
    --require-expected-workspaces >/dev/null
  python3 - "$report" "$live" "$quarantine" "$before" <<'PY'
import json
import sqlite3
import subprocess
import sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text())
live = Path(sys.argv[2])
quarantine = Path(sys.argv[3])
before = sys.argv[4]
assert report["result"] == "SUCCESS", report
db = live / "workspaces/p-sit-cat-rental-8ef3bf83/aopmem.sqlite"
con = sqlite3.connect(db)
assert con.execute("SELECT title FROM nodes WHERE id=1").fetchone()[0].endswith("memory")
assert con.execute("PRAGMA quick_check").fetchone()[0] == "ok"
con.close()
assert (live / "workspaces/p-sit-cat-rental-8ef3bf83/secrets/token.txt").read_text() == "secret"
assert quarantine.exists()
after = subprocess.check_output(["shasum", "-a", "256", str(quarantine / "workspaces/p-sit-cat-rental-8ef3bf83/aopmem.sqlite")], text=True).split()[0]
assert before == after
assert report["conflicts"] == []
PY
  pass "external transplant success"
}

assert_transplant_rollback() {
  live="$case_root/rollback-live"
  quarantine="$case_root/rollback-q/home"
  failed="$case_root/rollback-f/home"
  report="$case_root/rollback-report.json"
  rc9="$case_root/fake-rc9-rollback"
  create_transplant_fixture "$live"
  write_fake_aopmem "$rc9" "0.2.0-rc9"
  if AOPMEM_TRANSPLANT_FAIL_AFTER_DB_COPY=1 \
    python3 "$repo_root/scripts/windows_rc4_to_rc9_transplant.py" \
      --action Execute \
      --live-home "$live" \
      --quarantine-root "$quarantine" \
      --failed-root "$failed" \
      --rc9-binary "$rc9" \
      --report "$report" >/dev/null; then
    fail "injected transplant failure unexpectedly succeeded"
  fi
  python3 - "$report" "$live" "$failed" <<'PY'
import json
import sqlite3
import sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text())
live = Path(sys.argv[2])
failed = Path(sys.argv[3])
assert report["result"] == "ROLLED_BACK", report
assert live.exists()
assert failed.exists()
db = live / "workspaces/p-sit-cat-rental-8ef3bf83/aopmem.sqlite"
con = sqlite3.connect(db)
assert con.execute("SELECT COUNT(*) FROM nodes").fetchone()[0] == 2
con.close()
PY
  pass "external transplant rollback"
}

assert_cli_upgrade_absent
assert_clean_installer_fresh_home
assert_clean_installer_empty_home
assert_clean_installer_populated_home_untouched
assert_clean_installer_hash_verification
assert_transplant_success
assert_transplant_rollback

if [ -f "$repo_root/docs/WINDOWS_CLEAN_TRANSPLANT.md" ]; then
  assert_no_current_update_contract
fi

pass "RC9 installer audit complete"
