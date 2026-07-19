#!/usr/bin/env python3
"""External one-shot RC4 to RC9 transplant harness.

This tool is intentionally outside the `aopmem` binary. It quarantines the
whole old home, performs a clean RC9 install, copies only logical user data into
fresh RC9 stores, validates the result, and rolls back automatically on failure.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import time
import uuid


PRODUCT_VERSION = "0.2.0-rc9"
DATABASE_NAME = "aopmem.sqlite"
REPORT_NAME = "RC4_TO_RC9_TRANSPLANT_REPORT.json"
DEFAULT_EXPECTED_WORKSPACES = [
    "p-sit-cat-rental-8ef3bf83",
    "p-sit-warranty-5708363a",
]
TRANSFER_TABLES = [
    (
        "nodes",
        [
            "id",
            "node_type",
            "status",
            "title",
            "summary",
            "body",
            "source_ref",
            "confidence",
            "trust_level",
            "created_at",
            "updated_at",
        ],
    ),
    ("links", ["id", "source_node_id", "target_node_id", "link_type", "created_at"]),
    ("aliases", ["id", "node_id", "alias", "created_at"]),
    ("tags", ["id", "node_id", "tag", "created_at"]),
    ("sources", ["id", "node_id", "source_ref", "created_at"]),
    ("events", ["id", "type", "timestamp", "source", "subject_kind", "subject_id"]),
    (
        "registries",
        [
            "id",
            "registry_type",
            "name",
            "status",
            "notes",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "tool_contracts",
        [
            "id",
            "tool_id",
            "name",
            "status",
            "owner_workflow",
            "side_effects",
            "approval_requirement",
            "contract_json",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "mcp_profiles",
        [
            "id",
            "name",
            "kind",
            "status",
            "read_operations",
            "write_operations",
            "side_effects",
            "approval_requirement",
            "credentials_source",
            "notes",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "tool_aliases",
        ["alias", "canonical_tool_id", "created_at", "source", "status"],
    ),
]
WORKSPACE_USER_DIRS = ["tools", "runtimes", "artifacts", "secrets"]
GLOBAL_USER_DIRS = ["tools", "runtimes", "artifacts", "secrets", "registries"]
DERIVED_STATE = [
    "schema_migrations",
    "fts_nodes",
    "wal",
    "shm",
    "observability",
    "audit-git",
    "logs",
    "task_runtime_state",
    "built_in_templates",
    "built_in_skills",
]

SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS nodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node_type TEXT NOT NULL,
    status TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT,
    body TEXT,
    source_ref TEXT,
    confidence REAL,
    trust_level TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_nodes_type ON nodes(node_type);
CREATE INDEX IF NOT EXISTS idx_nodes_status ON nodes(status);
CREATE INDEX IF NOT EXISTS idx_nodes_summary ON nodes(summary);
CREATE INDEX IF NOT EXISTS idx_nodes_title_nocase ON nodes(title COLLATE NOCASE);
CREATE TABLE IF NOT EXISTS links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_node_id INTEGER NOT NULL,
    target_node_id INTEGER NOT NULL,
    link_type TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (source_node_id) REFERENCES nodes(id) ON DELETE RESTRICT,
    FOREIGN KEY (target_node_id) REFERENCES nodes(id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_node_id);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_node_id);
CREATE INDEX IF NOT EXISTS idx_links_type ON links(link_type);
CREATE TABLE IF NOT EXISTS aliases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id INTEGER NOT NULL,
    alias TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE RESTRICT,
    UNIQUE (node_id, alias)
);
CREATE INDEX IF NOT EXISTS idx_aliases_node ON aliases(node_id);
CREATE INDEX IF NOT EXISTS idx_aliases_alias ON aliases(alias);
CREATE INDEX IF NOT EXISTS idx_aliases_alias_nocase ON aliases(alias COLLATE NOCASE);
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id INTEGER NOT NULL,
    tag TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE RESTRICT,
    UNIQUE (node_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_tags_node ON tags(node_id);
CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);
CREATE INDEX IF NOT EXISTS idx_tags_tag_nocase ON tags(tag COLLATE NOCASE);
CREATE TABLE IF NOT EXISTS sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id INTEGER NOT NULL,
    source_ref TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE RESTRICT,
    UNIQUE (node_id, source_ref)
);
CREATE INDEX IF NOT EXISTS idx_sources_node ON sources(node_id);
CREATE INDEX IF NOT EXISTS idx_sources_ref ON sources(source_ref);
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    source TEXT NOT NULL,
    subject_kind TEXT NOT NULL,
    subject_id INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(type);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
CREATE INDEX IF NOT EXISTS idx_events_subject ON events(subject_kind, subject_id);
CREATE TABLE IF NOT EXISTS registries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    registry_type TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (registry_type, name)
);
CREATE INDEX IF NOT EXISTS idx_registries_type ON registries(registry_type);
CREATE INDEX IF NOT EXISTS idx_registries_status ON registries(status);
CREATE TABLE IF NOT EXISTS tool_contracts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_id TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    owner_workflow TEXT,
    side_effects TEXT NOT NULL,
    approval_requirement TEXT NOT NULL,
    contract_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_tool_contracts_status ON tool_contracts(status);
CREATE TABLE IF NOT EXISTS mcp_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    read_operations TEXT NOT NULL,
    write_operations TEXT NOT NULL,
    side_effects TEXT NOT NULL,
    approval_requirement TEXT NOT NULL,
    credentials_source TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_mcp_profiles_kind ON mcp_profiles(kind);
CREATE INDEX IF NOT EXISTS idx_mcp_profiles_status ON mcp_profiles(status);
CREATE VIRTUAL TABLE IF NOT EXISTS fts_nodes USING fts5(
    title,
    summary,
    body,
    aliases
);
CREATE TABLE IF NOT EXISTS tool_aliases (
    alias TEXT PRIMARY KEY,
    canonical_tool_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    FOREIGN KEY (canonical_tool_id) REFERENCES tool_contracts(tool_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_tool_aliases_status_alias
    ON tool_aliases(status, alias);
CREATE INDEX IF NOT EXISTS idx_tool_aliases_canonical_status_alias
    ON tool_aliases(canonical_tool_id, status, alias);
INSERT OR IGNORE INTO schema_migrations (version, name) VALUES
    ('001', '001_init'),
    ('002', '002_nodes_summary_index'),
    ('003', '003_task_recall_exact_indexes'),
    ('004', '004_task_protocol_and_tool_aliases');
"""


class TransplantError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def short_id() -> str:
    return uuid.uuid4().hex[:10]


def default_live_home() -> Path:
    if os.name == "nt":
        profile = os.environ.get("USERPROFILE")
        if not profile:
            raise TransplantError("USERPROFILE is not set")
        return Path(profile) / ".aopmem"
    return Path.home() / ".aopmem"


def default_quarantine_home(live_home: Path, prefix: str) -> Path:
    run = short_id()
    if os.name == "nt":
        drive = live_home.drive or "C:"
        preferred = Path(drive + "\\") / prefix / run / "home"
        fallback_root = Path(os.environ.get("USERPROFILE", str(Path.home()))) / prefix / run / "home"
        try:
            preferred.parent.mkdir(parents=True, exist_ok=False)
            return preferred
        except OSError:
            fallback_root.parent.mkdir(parents=True, exist_ok=False)
            return fallback_root
    root = Path.home() / prefix / run / "home"
    root.parent.mkdir(parents=True, exist_ok=False)
    return root


def is_reparse_or_symlink(path: Path) -> bool:
    try:
        stat_result = path.lstat()
    except OSError:
        return True
    if path.is_symlink():
        return True
    attrs = getattr(stat_result, "st_file_attributes", 0)
    return bool(attrs & getattr(stat_result, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400))


def safe_relative_paths(root: Path) -> list[Path]:
    paths: list[Path] = []
    if not root.exists():
        return paths
    for current, dirs, files in os.walk(root):
        current_path = Path(current)
        kept_dirs = []
        for name in dirs:
            candidate = current_path / name
            if not is_reparse_or_symlink(candidate):
                kept_dirs.append(name)
        dirs[:] = kept_dirs
        for name in files:
            candidate = current_path / name
            if is_reparse_or_symlink(candidate):
                continue
            rel = candidate.relative_to(root)
            if ".." in rel.parts:
                continue
            paths.append(rel)
    return sorted(paths, key=lambda p: p.as_posix())


def inventory(root: Path) -> dict[str, object]:
    files = []
    for rel in safe_relative_paths(root):
        path = root / rel
        files.append(
            {
                "path": rel.as_posix(),
                "bytes": path.stat().st_size,
                "sha256": sha256_file(path),
            }
        )
    return {"root": str(root), "file_count": len(files), "files": files}


def check_processes_absent() -> None:
    if os.name != "nt":
        return
    result = subprocess.run(
        ["tasklist.exe", "/FI", "IMAGENAME eq aopmem.exe", "/FO", "CSV"],
        check=False,
        text=True,
        capture_output=True,
    )
    output = result.stdout.lower()
    if "aopmem.exe" in output:
        raise TransplantError("AOPMem process is still running")


def run_checked(argv: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(argv, check=False, text=True, capture_output=True, env=env)
    if result.returncode != 0:
        raise TransplantError(
            "command failed: "
            + json.dumps(argv)
            + "\nstdout="
            + result.stdout[-4000:]
            + "\nstderr="
            + result.stderr[-4000:]
        )
    return result


def open_readonly_sqlite(path: Path) -> sqlite3.Connection:
    uri = path.resolve().as_uri() + "?mode=ro"
    connection = sqlite3.connect(uri, uri=True)
    connection.row_factory = sqlite3.Row
    return connection


def open_writable_sqlite(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(path)
    connection.row_factory = sqlite3.Row
    connection.execute("PRAGMA foreign_keys = ON")
    return connection


def table_columns(connection: sqlite3.Connection, table: str) -> set[str]:
    rows = connection.execute(f"PRAGMA table_info({quote_ident(table)})").fetchall()
    return {str(row["name"]) for row in rows}


def table_exists(connection: sqlite3.Connection, table: str) -> bool:
    row = connection.execute(
        "SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ? LIMIT 1",
        (table,),
    ).fetchone()
    return row is not None


def quote_ident(value: str) -> str:
    return '"' + value.replace('"', '""') + '"'


def row_count(connection: sqlite3.Connection, table: str) -> int:
    if not table_exists(connection, table):
        return 0
    return int(connection.execute(f"SELECT COUNT(*) FROM {quote_ident(table)}").fetchone()[0])


def canonical_rows(connection: sqlite3.Connection, table: str, columns: list[str]) -> list[dict[str, object]]:
    if not table_exists(connection, table):
        return []
    existing = [column for column in columns if column in table_columns(connection, table)]
    if not existing:
        return []
    order = existing[0]
    sql = (
        "SELECT "
        + ", ".join(quote_ident(column) for column in existing)
        + " FROM "
        + quote_ident(table)
        + " ORDER BY "
        + quote_ident(order)
        + " ASC"
    )
    return [dict(row) for row in connection.execute(sql).fetchall()]


def semantic_fingerprint(connection: sqlite3.Connection) -> str:
    digest = hashlib.sha256()
    for table, columns in TRANSFER_TABLES:
        digest.update(table.encode("utf-8"))
        rows = canonical_rows(connection, table, columns)
        encoded = json.dumps(rows, sort_keys=True, ensure_ascii=False, separators=(",", ":"))
        digest.update(encoded.encode("utf-8"))
    return digest.hexdigest()


def workspace_db_paths(home: Path) -> dict[str, Path]:
    root = home / "workspaces"
    if not root.exists():
        return {}
    result = {}
    for child in sorted(root.iterdir(), key=lambda p: p.name):
        if child.is_dir() and not is_reparse_or_symlink(child):
            db = child / DATABASE_NAME
            if db.is_file():
                result[child.name] = db
    return result


def workspace_counts(connection: sqlite3.Connection) -> dict[str, int]:
    return {table: row_count(connection, table) for table, _columns in TRANSFER_TABLES}


def ensure_fresh_schema(connection: sqlite3.Connection) -> None:
    connection.executescript(SCHEMA_SQL)
    connection.execute("PRAGMA foreign_keys = ON")


def clear_destination_tables(connection: sqlite3.Connection) -> None:
    for table, _columns in reversed(TRANSFER_TABLES):
        if table_exists(connection, table):
            connection.execute(f"DELETE FROM {quote_ident(table)}")
    if table_exists(connection, "fts_nodes"):
        connection.execute("DELETE FROM fts_nodes")


def copy_table(source: sqlite3.Connection, target: sqlite3.Connection, table: str, columns: list[str]) -> int:
    if not table_exists(source, table) or not table_exists(target, table):
        return 0
    source_columns = table_columns(source, table)
    target_columns = table_columns(target, table)
    selected = [column for column in columns if column in source_columns and column in target_columns]
    if not selected:
        return 0
    select_sql = "SELECT " + ", ".join(quote_ident(column) for column in selected)
    select_sql += " FROM " + quote_ident(table)
    placeholders = ", ".join("?" for _ in selected)
    insert_sql = (
        "INSERT INTO "
        + quote_ident(table)
        + " ("
        + ", ".join(quote_ident(column) for column in selected)
        + ") VALUES ("
        + placeholders
        + ")"
    )
    rows = source.execute(select_sql).fetchall()
    target.executemany(insert_sql, ([row[column] for column in selected] for row in rows))
    return len(rows)


def rebuild_fts(connection: sqlite3.Connection) -> None:
    if not table_exists(connection, "fts_nodes"):
        return
    connection.execute("DELETE FROM fts_nodes")
    connection.execute(
        """
        INSERT INTO fts_nodes(rowid, title, summary, body, aliases)
        SELECT
            nodes.id,
            nodes.title,
            nodes.summary,
            nodes.body,
            COALESCE((
                SELECT group_concat(alias, ' ')
                FROM aliases
                WHERE aliases.node_id = nodes.id
                ORDER BY aliases.id ASC, aliases.alias ASC
            ), '')
        FROM nodes
        ORDER BY nodes.id ASC
        """
    )


def transplant_database(source_db: Path, target_db: Path) -> dict[str, object]:
    with contextlib.closing(open_readonly_sqlite(source_db)) as source:
        source_counts = workspace_counts(source)
        source_fingerprint = semantic_fingerprint(source)
        with contextlib.closing(open_writable_sqlite(target_db)) as target:
            ensure_fresh_schema(target)
            target.execute("BEGIN IMMEDIATE")
            try:
                clear_destination_tables(target)
                copied = {}
                for table, columns in TRANSFER_TABLES:
                    copied[table] = copy_table(source, target, table, columns)
                if os.environ.get("AOPMEM_TRANSPLANT_FAIL_AFTER_DB_COPY") == "1":
                    raise TransplantError("injected failure after DB copy")
                rebuild_fts(target)
                fk = [dict(row) for row in target.execute("PRAGMA foreign_key_check").fetchall()]
                quick = target.execute("PRAGMA quick_check").fetchone()[0]
                target_fingerprint = semantic_fingerprint(target)
                target_counts = workspace_counts(target)
                if fk:
                    raise TransplantError(f"foreign_key_check failed: {fk}")
                if quick != "ok":
                    raise TransplantError(f"quick_check failed: {quick}")
                if source_fingerprint != target_fingerprint:
                    raise TransplantError("semantic fingerprint mismatch")
                target.commit()
            except Exception:
                target.rollback()
                raise
    return {
        "source_counts": source_counts,
        "target_counts": target_counts,
        "semantic_fingerprint": source_fingerprint,
        "copied_rows": copied,
        "foreign_key_check": "ok",
        "quick_check": "ok",
        "fts_rebuilt": True,
    }


def robocopy_clean(source: Path, target: Path) -> bool:
    if os.name != "nt" or target.exists():
        return False
    target.parent.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        [
            "robocopy.exe",
            str(source),
            str(target),
            "/E",
            "/XJ",
            "/R:1",
            "/W:1",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode >= 8:
        raise TransplantError(f"robocopy failed for {source}: exit={result.returncode}")
    return True


def copy_tree_user_only(source: Path, target: Path, label: str) -> tuple[list[dict[str, object]], list[dict[str, object]], list[dict[str, object]]]:
    copied: list[dict[str, object]] = []
    skipped: list[dict[str, object]] = []
    conflicts: list[dict[str, object]] = []
    if not source.exists():
        return copied, skipped, conflicts
    if is_reparse_or_symlink(source):
        skipped.append({"path": str(source), "reason": "reparse_or_symlink"})
        return copied, skipped, conflicts
    if robocopy_clean(source, target):
        for rel in safe_relative_paths(target):
            path = target / rel
            copied.append({"area": label, "path": rel.as_posix(), "bytes": path.stat().st_size})
        return copied, skipped, conflicts

    for rel in safe_relative_paths(source):
        src = source / rel
        dst = target / rel
        try:
            resolved = dst.parent.resolve()
            root = target.resolve()
            if root not in [resolved, *resolved.parents]:
                skipped.append({"area": label, "path": rel.as_posix(), "reason": "path_escape"})
                continue
        except OSError:
            skipped.append({"area": label, "path": rel.as_posix(), "reason": "path_resolution_failed"})
            continue
        if dst.exists() or dst.is_symlink():
            same_size = dst.is_file() and dst.stat().st_size == src.stat().st_size
            same_hash = same_size and sha256_file(dst) == sha256_file(src)
            conflicts.append(
                {
                    "area": label,
                    "path": rel.as_posix(),
                    "reason": "target_exists",
                    "same_bytes": bool(same_hash),
                }
            )
            continue
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)
        copied.append({"area": label, "path": rel.as_posix(), "bytes": dst.stat().st_size})
    return copied, skipped, conflicts


def copy_user_files(source_home: Path, target_home: Path, workspaces: list[str]) -> dict[str, object]:
    copied: list[dict[str, object]] = []
    skipped: list[dict[str, object]] = []
    conflicts: list[dict[str, object]] = []
    for name in GLOBAL_USER_DIRS:
        c, s, f = copy_tree_user_only(source_home / name, target_home / name, f"global/{name}")
        copied.extend(c)
        skipped.extend(s)
        conflicts.extend(f)
    for workspace in workspaces:
        source_root = source_home / "workspaces" / workspace
        target_root = target_home / "workspaces" / workspace
        for name in WORKSPACE_USER_DIRS:
            c, s, f = copy_tree_user_only(
                source_root / name,
                target_root / name,
                f"workspace/{workspace}/{name}",
            )
            copied.extend(c)
            skipped.extend(s)
            conflicts.extend(f)
    return {"copied": copied, "skipped": skipped, "conflicts": conflicts}


def verify_binary(binary: Path, expected_version: str, expected_hash: str | None) -> dict[str, object]:
    if not binary.is_file():
        raise TransplantError(f"binary missing: {binary}")
    result = run_checked([str(binary), "--version"])
    version = result.stdout.strip()
    if version != f"aopmem {expected_version}":
        raise TransplantError(f"unexpected binary version: {version}")
    digest = sha256_file(binary)
    if expected_hash and digest.lower() != expected_hash.lower():
        raise TransplantError("binary hash mismatch")
    return {"version": version, "sha256": digest}


def run_clean_installer(args: argparse.Namespace, live_home: Path) -> None:
    if args.clean_home_ready:
        return
    if args.rc9_binary:
        binary = Path(args.rc9_binary)
        bin_dir = live_home / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        destination = bin_dir / ("aopmem.exe" if os.name == "nt" else "aopmem")
        if destination.exists():
            raise TransplantError(f"target binary already exists: {destination}")
        shutil.copy2(binary, destination)
        return
    installer = Path(args.installer) if args.installer else None
    if not installer or not installer.is_file():
        raise TransplantError("clean installer path is required")
    if args.installer_sha256 and sha256_file(installer).lower() != args.installer_sha256.lower():
        raise TransplantError("installer hash mismatch")
    if os.name != "nt":
        raise TransplantError("native Windows clean installer execution is required")
    powershell = os.path.join(
        os.environ.get("SystemRoot", r"C:\Windows"),
        "System32",
        "WindowsPowerShell",
        "v1.0",
        "powershell.exe",
    )
    env = os.environ.copy()
    env["AOPMEM_HOME"] = str(live_home)
    command = [
        powershell,
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        str(installer),
    ]
    if args.asset_base_uri:
        command += ["-AssetBaseUri", args.asset_base_uri]
    run_checked(command, env=env)


def run_aopmem_checks(binary: Path, home: Path, repo_root: Path | None) -> dict[str, object]:
    if not binary.is_file() or repo_root is None or not repo_root.exists():
        return {
            "doctor": "not_run",
            "verify": "not_run",
            "recall": "not_run",
            "task_smoke": "not_run",
            "observability": "not_run",
            "debug_capsule": "not_run",
        }
    env = os.environ.copy()
    env["AOPMEM_HOME"] = str(home)

    def run_repo(command: list[str]) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(command, cwd=repo_root, env=env, text=True, capture_output=True)
        if result.returncode != 0:
            raise TransplantError(
                "aopmem check failed: "
                + json.dumps(command)
                + "\nstdout="
                + result.stdout[-2000:]
                + "\nstderr="
                + result.stderr[-2000:]
            )
        return result

    run_repo([str(binary), "--json", "doctor"])
    run_repo([str(binary), "--json", "verify"])
    recall = run_repo([str(binary), "--json", "recall", "--query", "AOPMem", "--limit", "1"])
    task = run_repo([str(binary), "--json", "task", "start", "--query", "RC9 transplant smoke"])
    payload = json.loads(task.stdout)
    task_id = payload.get("data", {}).get("task_id")
    bundle_id = payload.get("data", {}).get("bundle_id")
    if task_id and bundle_id:
        run_repo(
            [
                str(binary),
                "--json",
                "--bundle-id",
                bundle_id,
                "task",
                "apply",
                "--task-id",
                task_id,
            ]
        )
        run_repo(
            [
                str(binary),
                "--json",
                "--bundle-id",
                bundle_id,
                "task",
                "complete",
                "--task-id",
                task_id,
            ]
        )
    run_repo([str(binary), "--json", "observe", "status"])
    capsule = repo_root / "RC4_TO_RC9_debug_capsule.zip"
    run_repo([str(binary), "--json", "observe", "export", "--output", str(capsule)])
    return {
        "doctor": "PASS",
        "verify": "PASS",
        "recall": "PASS" if json.loads(recall.stdout).get("ok") else "FAIL",
        "task_smoke": "PASS",
        "observability": "PASS",
        "debug_capsule": "PASS",
    }


def write_report(report: dict[str, object], report_path: Path) -> None:
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")


def build_base_report(source_home: Path, target_home: Path, quarantine_home: Path | None) -> dict[str, object]:
    source_dbs = workspace_db_paths(source_home)
    workspace_reports = []
    source_counts = {}
    semantic = {}
    for key, db in source_dbs.items():
        with contextlib.closing(open_readonly_sqlite(db)) as connection:
            source_counts[key] = workspace_counts(connection)
            semantic[key] = semantic_fingerprint(connection)
        workspace_reports.append({"workspace_key": key, "source_db": str(db), "db_sha256": sha256_file(db)})
    return {
        "result": "PLAN_ONLY",
        "source_version": None,
        "source_hash": None,
        "target_version": PRODUCT_VERSION,
        "target_hash": None,
        "quarantine_root": str(quarantine_home) if quarantine_home else None,
        "failed_rc9_root": None,
        "workspaces": workspace_reports,
        "source_counts": source_counts,
        "target_counts": {},
        "semantic_fingerprints": semantic,
        "filesystem_copied": [],
        "filesystem_skipped": [],
        "conflicts": [],
        "derived_state_rebuilt": DERIVED_STATE,
        "secrets_preserved": [],
        "rollback_available": quarantine_home is not None,
        "doctor": "not_run",
        "verify": "not_run",
        "recall": "not_run",
        "task_smoke": "not_run",
        "observability": "not_run",
    }


def rollback_home(live_home: Path, quarantine_home: Path, failed_home: Path, report: dict[str, object]) -> None:
    if live_home.exists():
        failed_home.parent.mkdir(parents=True, exist_ok=True)
        if failed_home.exists():
            raise TransplantError(f"failed RC9 root already exists: {failed_home}")
        live_home.replace(failed_home)
        report["failed_rc9_root"] = str(failed_home)
    if quarantine_home.exists() and not live_home.exists():
        quarantine_home.replace(live_home)
    report["rollback_available"] = True


def execute(args: argparse.Namespace) -> dict[str, object]:
    live_home = Path(args.live_home) if args.live_home else default_live_home()
    quarantine_home = Path(args.quarantine_root) if args.quarantine_root else default_quarantine_home(live_home, "a4")
    failed_home = Path(args.failed_root) if args.failed_root else default_quarantine_home(live_home, "a9f")
    report = build_base_report(live_home, live_home, quarantine_home)
    report["result"] = "STARTED"
    quarantined = False
    try:
        check_processes_absent()
        source_binary = live_home / "bin" / ("aopmem.exe" if os.name == "nt" else "aopmem")
        if source_binary.exists():
            source_info = verify_binary(source_binary, args.source_version, args.source_hash)
            report["source_version"] = source_info["version"]
            report["source_hash"] = source_info["sha256"]
        if not live_home.exists():
            raise TransplantError(f"source live home missing: {live_home}")
        quarantine_home.parent.mkdir(parents=True, exist_ok=True)
        if quarantine_home.exists():
            raise TransplantError(f"quarantine root already exists: {quarantine_home}")
        report["source_inventory"] = inventory(live_home)
        live_home.replace(quarantine_home)
        quarantined = True
        if live_home.exists():
            raise TransplantError("live home still exists after quarantine rename")
        report["quarantine_inventory"] = inventory(quarantine_home)

        run_clean_installer(args, live_home)
        if not live_home.exists():
            live_home.mkdir(parents=True, exist_ok=True)
        target_binary = live_home / "bin" / ("aopmem.exe" if os.name == "nt" else "aopmem")
        if target_binary.exists():
            target_info = verify_binary(target_binary, PRODUCT_VERSION, args.target_hash)
            report["target_version"] = target_info["version"]
            report["target_hash"] = target_info["sha256"]

        source_dbs = workspace_db_paths(quarantine_home)
        expected = args.expected_workspace or DEFAULT_EXPECTED_WORKSPACES
        missing = [key for key in expected if key not in source_dbs]
        if missing and args.require_expected_workspaces:
            raise TransplantError(f"expected workspaces missing: {missing}")
        transferred = {}
        target_counts = {}
        semantic = {}
        for key, source_db in source_dbs.items():
            target_db = live_home / "workspaces" / key / DATABASE_NAME
            transferred[key] = transplant_database(source_db, target_db)
            target_counts[key] = transferred[key]["target_counts"]
            semantic[key] = transferred[key]["semantic_fingerprint"]
        files = copy_user_files(quarantine_home, live_home, sorted(source_dbs))
        report["target_counts"] = target_counts
        report["semantic_fingerprints"] = semantic
        report["filesystem_copied"] = files["copied"]
        report["filesystem_skipped"] = files["skipped"]
        report["conflicts"] = files["conflicts"]
        report["secrets_preserved"] = [
            item for item in files["copied"] if "/secrets" in item["area"] or item["area"].endswith("/secrets")
        ]
        checks = run_aopmem_checks(
            target_binary,
            live_home,
            Path(args.repo_root) if args.repo_root else None,
        )
        report.update(checks)
        report["workspaces"] = [
            {"workspace_key": key, "source_db": str(db), "target_db": str(live_home / "workspaces" / key / DATABASE_NAME)}
            for key, db in sorted(source_dbs.items())
        ]
        report["result"] = "SUCCESS"
        return report
    except Exception as error:
        report["result"] = "ROLLED_BACK" if quarantined else "FAILED_PRE_QUARANTINE"
        report["error"] = str(error)
        if quarantined:
            rollback_home(live_home, quarantine_home, failed_home, report)
        return report


def plan(args: argparse.Namespace) -> dict[str, object]:
    live_home = Path(args.live_home) if args.live_home else default_live_home()
    quarantine_home = Path(args.quarantine_root) if args.quarantine_root else None
    return build_base_report(live_home, live_home, quarantine_home)


def rollback(args: argparse.Namespace) -> dict[str, object]:
    live_home = Path(args.live_home) if args.live_home else default_live_home()
    quarantine_home = Path(args.quarantine_root) if args.quarantine_root else None
    if quarantine_home is None:
        raise TransplantError("--quarantine-root is required for rollback")
    failed_home = Path(args.failed_root) if args.failed_root else default_quarantine_home(live_home, "a9f")
    report = build_base_report(quarantine_home, live_home, quarantine_home)
    rollback_home(live_home, quarantine_home, failed_home, report)
    report["result"] = "ROLLBACK_COMPLETE"
    return report


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="External RC4 to RC9 transplant harness")
    parser.add_argument("--action", choices=["Plan", "Execute", "Rollback"], required=True)
    parser.add_argument("--live-home")
    parser.add_argument("--quarantine-root")
    parser.add_argument("--failed-root")
    parser.add_argument("--report")
    parser.add_argument("--repo-root")
    parser.add_argument("--installer")
    parser.add_argument("--installer-sha256")
    parser.add_argument("--asset-base-uri")
    parser.add_argument("--rc9-binary")
    parser.add_argument("--source-version", default="0.2.0-rc4")
    parser.add_argument("--source-hash")
    parser.add_argument("--target-hash")
    parser.add_argument("--expected-workspace", action="append")
    parser.add_argument("--require-expected-workspaces", action="store_true")
    parser.add_argument("--clean-home-ready", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    report_path = Path(args.report) if args.report else Path.cwd() / REPORT_NAME
    if args.action == "Plan":
        report = plan(args)
    elif args.action == "Execute":
        report = execute(args)
    else:
        report = rollback(args)
    write_report(report, report_path)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["result"] in {"PLAN_ONLY", "SUCCESS", "ROLLBACK_COMPLETE"} else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
