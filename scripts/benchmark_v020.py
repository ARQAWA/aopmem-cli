#!/usr/bin/env python3
"""Reproducible AOPMem v0.1.0-rc3 versus v0.2.0-rc1 benchmark.

The harness uses only the Python standard library.  Callers provide two
already-built binaries.  The harness creates and populates benchmark-only
fixtures below a disposable work directory; every command receives an
isolated AOPMEM_HOME.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import platform
import selectors
import shutil
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterable, Sequence


GENERATOR_VERSION = 1
OUTPUT_MARKER = ".aopmem-v020-benchmark-output"
FIXED_TIMESTAMP = "2026-07-15T00:00:00.000Z"
INIT_INPUT = (
    "no\n"
    "no\n"
    "Deterministic local AOPMem benchmark project.\n"
    "The user teaches and the agent reproduces approved workflows.\n"
    "Only the disposable benchmark workspace may be changed.\n"
).encode("utf-8")


@dataclass(frozen=True)
class CorpusSpec:
    name: str
    nodes: int
    links: int
    tools: int
    observability_events: int


CORPORA = (
    CorpusSpec("small", 100, 300, 5, 100),
    CorpusSpec("medium", 2_000, 6_000, 25, 2_000),
    CorpusSpec("large", 10_000, 30_000, 100, 10_000),
)


@dataclass(frozen=True)
class Variant:
    name: str
    binary: Path
    product_version: str
    source_release: str


@dataclass(frozen=True)
class Fixture:
    variant: Variant
    corpus: CorpusSpec
    repo: Path
    home: Path
    workspace_key: str
    workspace: Path
    database: Path
    observability_database: Path | None
    logical_sha256: str
    counts: dict[str, int]


class BenchmarkError(RuntimeError):
    pass


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-binary", type=Path, required=True)
    parser.add_argument("--current-binary", type=Path, required=True)
    parser.add_argument("--baseline-commit", required=True)
    parser.add_argument("--build-profile", default="unknown")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path(".devplan/benchmarks/v020_rc1"),
    )
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--warmups", type=nonnegative_int, default=3)
    parser.add_argument("--samples", type=positive_int, default=20)
    parser.add_argument(
        "--corpus",
        choices=("all",) + tuple(spec.name for spec in CORPORA),
        default="all",
    )
    parser.add_argument("--keep-work-dir", action="store_true")
    return parser.parse_args()


def nonnegative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be non-negative")
    return parsed


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def source_tree_sha256(repo_root: Path) -> str:
    digest = hashlib.sha256()
    candidates = [repo_root / "Cargo.toml", repo_root / "Cargo.lock"]
    for directory in (repo_root / "src", repo_root / "templates"):
        candidates.extend(path for path in directory.rglob("*") if path.is_file())
    for path in sorted(
        candidates, key=lambda item: item.relative_to(repo_root).as_posix()
    ):
        relative = path.relative_to(repo_root).as_posix()
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def command_text(arguments: Sequence[str], cwd: Path) -> str | None:
    try:
        result = subprocess.run(
            list(arguments),
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=True,
            timeout=30,
            text=True,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return result.stdout.strip()


def stable_uuid(namespace: str, number: int) -> str:
    raw = bytearray(hashlib.sha256(f"{namespace}:{number}".encode()).digest()[:16])
    raw[6] = (raw[6] & 0x0F) | 0x40
    raw[8] = (raw[8] & 0x3F) | 0x80
    return str(uuid.UUID(bytes=bytes(raw)))


def command_environment(home: Path) -> dict[str, str]:
    environment = os.environ.copy()
    environment["AOPMEM_HOME"] = str(home)
    environment["LC_ALL"] = "C"
    environment["LANG"] = "C"
    environment["TZ"] = "UTC"
    return environment


def invoke(
    variant: Variant,
    repo: Path,
    home: Path,
    arguments: Sequence[str],
    *,
    stdin: bytes | None = None,
    accepted_exit_codes: Iterable[int] = (0,),
) -> tuple[subprocess.CompletedProcess[bytes], int]:
    command = [str(variant.binary), "--json", *arguments]
    started = time.perf_counter_ns()
    result = subprocess.run(
        command,
        cwd=repo,
        env=command_environment(home),
        input=stdin,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=180,
    )
    duration_ns = time.perf_counter_ns() - started
    if result.returncode not in set(accepted_exit_codes):
        raise BenchmarkError(
            f"command failed ({result.returncode}): {' '.join(command)}\n"
            f"stderr={result.stderr.decode('utf-8', 'replace')[-2000:]}"
        )
    return result, duration_ns


def parse_envelope(result: subprocess.CompletedProcess[bytes]) -> dict[str, Any]:
    try:
        value = json.loads(result.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BenchmarkError(
            f"command did not return one JSON envelope: {result.stdout[:1000]!r}"
        ) from error
    if not isinstance(value, dict) or value.get("ok") is not True:
        raise BenchmarkError(f"command returned a non-success envelope: {value!r}")
    return value


def product_version(binary: Path) -> str:
    result = subprocess.run(
        [str(binary), "--version"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
        timeout=30,
        text=True,
    )
    parts = result.stdout.strip().split()
    if len(parts) < 2:
        raise BenchmarkError(f"unexpected --version output: {result.stdout!r}")
    return parts[-1]


def selected_corpora(name: str) -> tuple[CorpusSpec, ...]:
    if name == "all":
        return CORPORA
    return tuple(spec for spec in CORPORA if spec.name == name)


def prepare_fixture(root: Path, variant: Variant, spec: CorpusSpec) -> Fixture:
    repo = root / "fixtures" / "repos" / spec.name / "repo"
    home = root / "fixtures" / "homes" / variant.name / spec.name
    repo.mkdir(parents=True, exist_ok=True)
    result, _ = invoke(variant, repo, home, ["init"], stdin=INIT_INPUT)
    init = parse_envelope(result)
    workspace_key = init.get("meta", {}).get("workspace_key")
    if not isinstance(workspace_key, str) or not workspace_key:
        raise BenchmarkError("init did not return a workspace key")
    workspace = home / "workspaces" / workspace_key
    database = workspace / "aopmem.sqlite"
    if not database.is_file():
        raise BenchmarkError(f"init did not create {database}")

    populate_operational_database(database, spec)

    # One real product mutation republishes the canonical SQL snapshot after
    # fixture SQL has been loaded.  Counts remain exact because one link was
    # deliberately reserved for this operation.
    final_link, _ = invoke(
        variant,
        repo,
        home,
        [
            "link",
            "add",
            "--source-id",
            "8",
            "--target-id",
            "9",
            "--type",
            "corrected_by",
        ],
    )
    parse_envelope(final_link)

    doctor, _ = invoke(variant, repo, home, ["doctor"])
    parse_envelope(doctor)
    verify, _ = invoke(variant, repo, home, ["verify"], accepted_exit_codes=(0, 8))
    verify_data = parse_envelope(verify).get("data", {})
    if verify_data.get("clean") is not True:
        raise BenchmarkError(
            f"generated {variant.name}/{spec.name} fixture is not clean: {verify_data!r}"
        )

    observability_database = workspace / "observability" / "observability.sqlite"
    if variant.name == "current":
        if not observability_database.is_file():
            raise BenchmarkError("current doctor did not create observability.sqlite")
        populate_observability_database(
            observability_database,
            workspace_key,
            variant.product_version,
            spec.observability_events,
        )
    else:
        observability_database = None

    counts = operational_counts(database)
    expected = {
        "nodes": spec.nodes,
        "links": spec.links,
        "aliases": spec.nodes,
        "tags": spec.nodes,
        "sources": spec.nodes,
        "events": spec.nodes,
        "tool_contracts": spec.tools,
        "mcp_profiles": 2,
    }
    if counts != expected:
        raise BenchmarkError(
            f"fixture counts differ for {variant.name}/{spec.name}: "
            f"expected={expected!r}, actual={counts!r}"
        )
    logical_sha256 = logical_database_sha256(database)
    return Fixture(
        variant=variant,
        corpus=spec,
        repo=repo,
        home=home,
        workspace_key=workspace_key,
        workspace=workspace,
        database=database,
        observability_database=observability_database,
        logical_sha256=logical_sha256,
        counts=counts,
    )


def populate_operational_database(database: Path, spec: CorpusSpec) -> None:
    connection = sqlite3.connect(database)
    try:
        connection.execute("PRAGMA foreign_keys = OFF")
        connection.execute("BEGIN IMMEDIATE")
        for table in (
            "fts_nodes",
            "links",
            "aliases",
            "tags",
            "sources",
            "events",
            "tool_contracts",
            "mcp_profiles",
            "registries",
            "nodes",
        ):
            connection.execute(f"DELETE FROM {table}")
        if table_exists(connection, "sqlite_sequence"):
            connection.execute(
                "DELETE FROM sqlite_sequence WHERE name IN "
                "('nodes','links','aliases','tags','sources','events',"
                "'tool_contracts','registries')"
            )

        node_rows = [node_row(node_id) for node_id in range(1, spec.nodes + 1)]
        connection.executemany(
            """
            INSERT INTO nodes (
                id, node_type, status, title, summary, body, source_ref,
                confidence, trust_level, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            node_rows,
        )
        connection.executemany(
            "INSERT INTO aliases (node_id, alias, created_at) VALUES (?, ?, ?)",
            (
                (node_id, f"benchmark-alias-{node_id:05d}", FIXED_TIMESTAMP)
                for node_id in range(1, spec.nodes + 1)
            ),
        )
        connection.executemany(
            "INSERT INTO tags (node_id, tag, created_at) VALUES (?, ?, ?)",
            (
                (node_id, f"benchmark-tag-{node_id % 97:02d}", FIXED_TIMESTAMP)
                for node_id in range(1, spec.nodes + 1)
            ),
        )
        connection.executemany(
            "INSERT INTO sources (node_id, source_ref, created_at) VALUES (?, ?, ?)",
            (
                (
                    node_id,
                    f"source=user_instruction:{node_id:05d}",
                    FIXED_TIMESTAMP,
                )
                for node_id in range(1, spec.nodes + 1)
            ),
        )

        direct_link_count = spec.links - 1
        connection.executemany(
            """
            INSERT INTO links (
                source_node_id, target_node_id, link_type, created_at
            ) VALUES (?, ?, ?, ?)
            """,
            (link_row(index, spec.nodes) for index in range(direct_link_count)),
        )
        connection.executemany(
            """
            INSERT INTO events (
                type, timestamp, source, subject_kind, subject_id
            ) VALUES (?, ?, ?, ?, ?)
            """,
            (
                (
                    ("node.created", "node.updated", "link.created")[index % 3],
                    FIXED_TIMESTAMP,
                    "benchmark_generator",
                    "node",
                    (index % spec.nodes) + 1,
                )
                for index in range(spec.nodes - 1)
            ),
        )
        connection.executemany(
            """
            INSERT INTO tool_contracts (
                tool_id, name, status, owner_workflow, side_effects,
                approval_requirement, contract_json, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (tool_row(index) for index in range(1, spec.tools + 1)),
        )
        connection.executemany(
            """
            INSERT INTO mcp_profiles (
                id, name, kind, status, read_operations, write_operations,
                side_effects, approval_requirement, credentials_source, notes,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                (
                    "benchmark-local-mcp",
                    "Benchmark Local MCP",
                    "optional",
                    "configured_unverified",
                    "read",
                    "none",
                    "local_read",
                    "none",
                    "none",
                    "deterministic benchmark profile",
                    FIXED_TIMESTAMP,
                    FIXED_TIMESTAMP,
                ),
                (
                    "benchmark-missing-mcp",
                    "Benchmark Missing MCP",
                    "optional",
                    "missing",
                    "read",
                    "none",
                    "local_read",
                    "none",
                    "none",
                    "deterministic benchmark profile",
                    FIXED_TIMESTAMP,
                    FIXED_TIMESTAMP,
                ),
            ),
        )
        connection.executemany(
            """
            INSERT INTO registries (
                registry_type, name, status, notes, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?)
            """,
            (
                (
                    "workflow",
                    "benchmark-workflows",
                    "active",
                    "deterministic benchmark registry",
                    FIXED_TIMESTAMP,
                    FIXED_TIMESTAMP,
                ),
                (
                    "tool",
                    "benchmark-tools",
                    "active",
                    "deterministic benchmark registry",
                    FIXED_TIMESTAMP,
                    FIXED_TIMESTAMP,
                ),
            ),
        )
        connection.execute(
            """
            INSERT INTO fts_nodes(rowid, title, summary, body, aliases)
            SELECT
                nodes.id,
                nodes.title,
                COALESCE(nodes.summary, ''),
                COALESCE(nodes.body, ''),
                COALESCE((
                    SELECT group_concat(aliases.alias, ' ')
                    FROM aliases WHERE aliases.node_id = nodes.id
                ), '')
            FROM nodes ORDER BY nodes.id
            """
        )
        connection.commit()
        connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    finally:
        connection.close()


def node_row(node_id: int) -> tuple[Any, ...]:
    special = {
        1: ("kernel_contract", "Canonical operational memory contract"),
        2: ("gate", "User-triggered memory writes gate"),
        3: ("project_profile", "Benchmark project profile"),
        4: ("source", "Benchmark source hierarchy"),
        5: ("rule", "Verify release artifacts before publish"),
        6: ("workflow", "Deploy release workflow"),
        7: ("tool_contract", "Release validation tool"),
        8: ("failure_mode", "Release checksum mismatch"),
        9: ("correction", "Always verify release checksum"),
    }
    node_types = (
        "workflow",
        "tool_contract",
        "failure_mode",
        "correction",
        "lesson",
        "project_fact",
        "preference",
        "skill",
        "incident_scar",
        "decision",
    )
    node_type, title = special.get(
        node_id,
        (
            node_types[(node_id - 10) % len(node_types)],
            f"Benchmark {node_types[(node_id - 10) % len(node_types)]} node {node_id:05d}",
        ),
    )
    summary = f"Deterministic summary for {title}."
    body = (
        f"Deterministic benchmark body {node_id:05d}. "
        "Use local data, follow the approved workflow, and verify the result."
    )
    return (
        node_id,
        node_type,
        "active",
        title,
        summary,
        body,
        f"source=user_instruction:{node_id:05d}",
        round(0.70 + ((node_id % 30) / 100), 2),
        ("high", "medium", "low")[node_id % 3],
        FIXED_TIMESTAMP,
        FIXED_TIMESTAMP,
    )


def link_row(index: int, node_count: int) -> tuple[int, int, str, str]:
    special = ((6, 7, "uses"), (6, 8, "guards_against"))
    if index < len(special):
        source, target, link_type = special[index]
    else:
        source = ((index * 37) % node_count) + 1
        target = ((index * 91 + 17) % node_count) + 1
        if target == source:
            target = (target % node_count) + 1
        link_type = ("depends_on", "uses", "corrects", "relates_to")[index % 4]
    return source, target, link_type, FIXED_TIMESTAMP


def tool_row(index: int) -> tuple[Any, ...]:
    tool_id = f"benchmark-tool-{index:03d}"
    name = f"Benchmark Tool {index:03d}"
    contract = {
        "tool_id": tool_id,
        "name": name,
        "status": "active",
        "owner_workflow": "Deploy release workflow",
        "command": {"entrypoint": "bin/run"},
        "args_schema": {"type": "object", "additionalProperties": False},
        "output_schema": {"type": "object"},
        "side_effects": "local_read",
        "approval_requirement": "none",
        "examples": [
            {
                "name": "benchmark",
                "args": [],
                "description": "deterministic benchmark example",
            }
        ],
        "runtime": {
            "executable_path": "bin/run",
            "runtime_dir": "runtime",
            "timeout_ms": 30_000,
            "stdout_limit_bytes": 65_536,
            "stderr_limit_bytes": 65_536,
            "supports_dry_run": True,
            "output_mode": "inline",
        },
    }
    return (
        tool_id,
        name,
        "active",
        "Deploy release workflow",
        "local_read",
        "none",
        json.dumps(contract, ensure_ascii=True, sort_keys=True, separators=(",", ":")),
        FIXED_TIMESTAMP,
        FIXED_TIMESTAMP,
    )


def table_exists(connection: sqlite3.Connection, table: str) -> bool:
    return (
        connection.execute(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?", (table,)
        ).fetchone()
        is not None
    )


def populate_observability_database(
    database: Path,
    workspace_key: str,
    version: str,
    event_count: int,
) -> None:
    connection = sqlite3.connect(database)
    try:
        connection.execute("PRAGMA foreign_keys = ON")
        connection.execute("BEGIN IMMEDIATE")
        connection.execute("DELETE FROM feedback")
        connection.execute("DELETE FROM bundle_nodes")
        connection.execute("DELETE FROM recall_bundles")
        connection.execute("DELETE FROM observability_events")
        doctor_payload = json.dumps(
            {
                "kind": "counts",
                "data": {
                    "items": [
                        {"name": "checks", "count": 9},
                        {"name": "ready", "count": 8},
                        {"name": "missing", "count": 1},
                        {"name": "error", "count": 0},
                    ]
                },
            },
            sort_keys=True,
            separators=(",", ":"),
        )
        connection.executemany(
            """
            INSERT INTO observability_events (
                id, timestamp, product_version, workspace_key, event_type,
                command, correlation_id, bundle_id, duration_ms, outcome,
                error_code, payload_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, NULL, ?)
            """,
            (
                (
                    stable_uuid("event", index),
                    FIXED_TIMESTAMP,
                    version,
                    workspace_key,
                    "doctor",
                    "doctor",
                    stable_uuid("correlation", index),
                    index % 250,
                    "warning",
                    doctor_payload,
                )
                for index in range(event_count)
            ),
        )
        connection.commit()
        connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    finally:
        connection.close()


def operational_counts(database: Path) -> dict[str, int]:
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    try:
        return {
            table: connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
            for table in (
                "nodes",
                "links",
                "aliases",
                "tags",
                "sources",
                "events",
                "tool_contracts",
                "mcp_profiles",
            )
        }
    finally:
        connection.close()


def logical_database_sha256(database: Path) -> str:
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    digest = hashlib.sha256()
    queries = (
        (
            "nodes",
            "SELECT id,node_type,status,title,summary,body,source_ref,confidence,"
            "trust_level FROM nodes ORDER BY id",
        ),
        (
            "links",
            "SELECT source_node_id,target_node_id,link_type FROM links ORDER BY id",
        ),
        ("aliases", "SELECT node_id,alias FROM aliases ORDER BY id"),
        ("tags", "SELECT node_id,tag FROM tags ORDER BY id"),
        ("sources", "SELECT node_id,source_ref FROM sources ORDER BY id"),
        (
            "events",
            "SELECT type,source,subject_kind,subject_id FROM events "
            "WHERE source='benchmark_generator' ORDER BY id",
        ),
        (
            "tool_contracts",
            "SELECT tool_id,name,status,owner_workflow,side_effects,"
            "approval_requirement,contract_json FROM tool_contracts ORDER BY tool_id",
        ),
        (
            "mcp_profiles",
            "SELECT id,name,kind,status,read_operations,write_operations,"
            "side_effects,approval_requirement,credentials_source,notes "
            "FROM mcp_profiles ORDER BY id",
        ),
    )
    try:
        for label, query in queries:
            digest.update(label.encode())
            digest.update(b"\0")
            for row in connection.execute(query):
                digest.update(
                    json.dumps(
                        row,
                        ensure_ascii=True,
                        separators=(",", ":"),
                    ).encode()
                )
                digest.update(b"\n")
    finally:
        connection.close()
    return digest.hexdigest()


def clone_home(source: Path, destination: Path) -> None:
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(source, destination, symlinks=True)


def sample_record(
    duration_ns: int,
    result: subprocess.CompletedProcess[bytes],
    **extra: Any,
) -> dict[str, Any]:
    return {
        "duration_ns": duration_ns,
        "duration_ms": duration_ns / 1_000_000,
        "exit_code": result.returncode,
        "stdout_bytes": len(result.stdout),
        "stderr_bytes": len(result.stderr),
        "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
        **extra,
    }


def measure_command(
    fixture: Fixture,
    work_root: Path,
    metric: str,
    arguments: Sequence[str],
    warmups: int,
    samples: int,
    *,
    accepted_exit_codes: Iterable[int] = (0,),
    validator: Callable[[dict[str, Any]], None] | None = None,
    fresh_home_each_time: bool = False,
) -> dict[str, Any]:
    metric_root = (
        work_root / "measure" / fixture.variant.name / fixture.corpus.name / metric
    )
    metric_root.mkdir(parents=True, exist_ok=True)
    shared_home = metric_root / "home"
    if not fresh_home_each_time:
        clone_home(fixture.home, shared_home)
    measured: list[dict[str, Any]] = []
    for index in range(warmups + samples):
        home = shared_home
        if fresh_home_each_time:
            home = metric_root / f"home-{index:03d}"
            clone_home(fixture.home, home)
        result, duration_ns = invoke(
            fixture.variant,
            fixture.repo,
            home,
            arguments,
            accepted_exit_codes=accepted_exit_codes,
        )
        envelope = parse_envelope(result)
        if validator is not None:
            validator(envelope)
        if index >= warmups:
            measured.append(sample_record(duration_ns, result))
        if fresh_home_each_time:
            shutil.rmtree(home)
    return supported_result(fixture, metric, warmups, measured)


def measure_init(
    variant: Variant,
    work_root: Path,
    warmups: int,
    samples: int,
) -> dict[str, Any]:
    measured: list[dict[str, Any]] = []
    root = work_root / "measure" / variant.name / "seed" / "init"
    root.mkdir(parents=True, exist_ok=True)
    for index in range(warmups + samples):
        repo = root / f"repo-{index:03d}"
        home = root / f"home-{index:03d}"
        repo.mkdir()
        result, duration_ns = invoke(
            variant,
            repo,
            home,
            ["init"],
            stdin=INIT_INPUT,
        )
        envelope = parse_envelope(result)
        if envelope.get("data", {}).get("initialized") is not True:
            raise BenchmarkError("init did not report initialized=true")
        if index >= warmups:
            measured.append(sample_record(duration_ns, result))
        shutil.rmtree(repo)
        shutil.rmtree(home)
    return supported_result_for_variant(variant, "seed", "init", warmups, measured)


def measure_full_pagination(
    fixture: Fixture,
    work_root: Path,
    warmups: int,
    samples: int,
) -> dict[str, Any]:
    metric = "node_list_full_pagination"
    root = work_root / "measure" / fixture.variant.name / fixture.corpus.name / metric
    home = root / "home"
    root.mkdir(parents=True, exist_ok=True)
    clone_home(fixture.home, home)
    measured: list[dict[str, Any]] = []
    for index in range(warmups + samples):
        started = time.perf_counter_ns()
        pages = 0
        returned_nodes = 0
        stdout_bytes = 0
        stderr_bytes = 0
        digest = hashlib.sha256()
        if fixture.variant.name == "baseline":
            result, _ = invoke(
                fixture.variant,
                fixture.repo,
                home,
                ["node", "list"],
            )
            envelope = parse_envelope(result)
            nodes = envelope.get("data", {}).get("nodes")
            if not isinstance(nodes, list):
                raise BenchmarkError("baseline node list did not return nodes")
            pages = 1
            returned_nodes = len(nodes)
            stdout_bytes += len(result.stdout)
            stderr_bytes += len(result.stderr)
            digest.update(result.stdout)
        else:
            cursor: str | None = None
            while True:
                arguments = [
                    "node",
                    "list",
                    "--limit",
                    "500",
                    "--include-body",
                ]
                if cursor is not None:
                    arguments.extend(("--cursor", cursor))
                result, _ = invoke(fixture.variant, fixture.repo, home, arguments)
                envelope = parse_envelope(result)
                data = envelope.get("data", {})
                nodes = data.get("nodes")
                if not isinstance(nodes, list):
                    raise BenchmarkError("current node page did not return nodes")
                pages += 1
                returned_nodes += len(nodes)
                stdout_bytes += len(result.stdout)
                stderr_bytes += len(result.stderr)
                digest.update(result.stdout)
                more_results = data.get("more_results")
                cursor = data.get("next_cursor")
                if more_results is False:
                    if cursor is not None:
                        raise BenchmarkError("terminal page returned a cursor")
                    break
                if (
                    more_results is not True
                    or not isinstance(cursor, str)
                    or not cursor
                ):
                    raise BenchmarkError("non-terminal page did not return a cursor")
                if pages > math.ceil(fixture.corpus.nodes / 500) + 1:
                    raise BenchmarkError("pagination did not terminate")
        duration_ns = time.perf_counter_ns() - started
        if returned_nodes != fixture.corpus.nodes:
            raise BenchmarkError(
                f"full traversal returned {returned_nodes}, expected {fixture.corpus.nodes}"
            )
        if index >= warmups:
            measured.append(
                {
                    "duration_ns": duration_ns,
                    "duration_ms": duration_ns / 1_000_000,
                    "exit_code": 0,
                    "stdout_bytes": stdout_bytes,
                    "stderr_bytes": stderr_bytes,
                    "stdout_sha256": digest.hexdigest(),
                    "pages": pages,
                    "returned_nodes": returned_nodes,
                }
            )
    return supported_result(fixture, metric, warmups, measured)


def measure_observability(
    fixture: Fixture,
    work_root: Path,
    warmups: int,
    samples: int,
) -> dict[str, Any]:
    metric = "observability_wall_and_residual"
    if fixture.observability_database is None:
        return unsupported_result(
            fixture.variant,
            fixture.corpus.name,
            metric,
            "v0.1.0-rc3 has no Local Observability collector or store",
        )
    root = work_root / "measure" / fixture.variant.name / fixture.corpus.name / metric
    home = root / "home"
    root.mkdir(parents=True, exist_ok=True)
    clone_home(fixture.home, home)
    obs_db = (
        home
        / "workspaces"
        / fixture.workspace_key
        / "observability"
        / "observability.sqlite"
    )
    measured: list[dict[str, Any]] = []
    for index in range(warmups + samples):
        before_rowid, before_events, before_bytes = observability_state(obs_db)
        result, duration_ns = invoke(
            fixture.variant,
            fixture.repo,
            home,
            ["doctor"],
        )
        parse_envelope(result)
        core_duration_ms = latest_terminal_duration(obs_db, before_rowid)
        if core_duration_ms is None:
            raise BenchmarkError("collector did not persist a terminal duration")
        _, after_events, after_bytes = observability_state(obs_db)
        if after_events != before_events + 1:
            raise BenchmarkError(
                f"doctor collector wrote {after_events - before_events} events, expected one"
            )
        if index >= warmups:
            wall_ms = duration_ns / 1_000_000
            measured.append(
                sample_record(
                    duration_ns,
                    result,
                    core_duration_ms=core_duration_ms,
                    wall_minus_core_ms=max(0.0, wall_ms - core_duration_ms),
                    observability_events_delta=after_events - before_events,
                    observability_bytes_before=before_bytes,
                    observability_bytes_after=after_bytes,
                )
            )
    result = supported_result(fixture, metric, warmups, measured)
    result["measurement_note"] = (
        "wall_minus_core_ms is an upper-bound residual: it includes process startup, "
        "JSON serialization/output, and collector I/O; it is not a pure collector timer"
    )
    return result


def observability_state(database: Path) -> tuple[int, int, int]:
    connection = sqlite3.connect(database)
    try:
        rowid, count = connection.execute(
            "SELECT COALESCE(MAX(rowid), 0), COUNT(*) FROM observability_events"
        ).fetchone()
    finally:
        connection.close()
    family_bytes = sum(
        path.stat().st_size
        for path in (
            database,
            Path(f"{database}-wal"),
            Path(f"{database}-shm"),
        )
        if path.is_file()
    )
    return rowid, count, family_bytes


def latest_terminal_duration(database: Path, after_rowid: int) -> int | None:
    connection = sqlite3.connect(database)
    try:
        row = connection.execute(
            """
            SELECT duration_ms FROM observability_events
            WHERE rowid > ? AND command = 'doctor' AND duration_ms IS NOT NULL
            ORDER BY rowid DESC LIMIT 1
            """,
            (after_rowid,),
        ).fetchone()
        return None if row is None else row[0]
    finally:
        connection.close()


def measure_ui_initial_api(
    fixture: Fixture,
    work_root: Path,
    warmups: int,
    samples: int,
) -> dict[str, Any]:
    metric = "ui_initial_overview_api"
    if fixture.variant.name == "baseline":
        return unsupported_result(
            fixture.variant,
            fixture.corpus.name,
            metric,
            "v0.1.0-rc3 has no `aopmem ui` command or local HTTP API",
        )
    root = work_root / "measure" / fixture.variant.name / fixture.corpus.name / metric
    home = root / "home"
    root.mkdir(parents=True, exist_ok=True)
    clone_home(fixture.home, home)
    measured: list[dict[str, Any]] = []
    for index in range(warmups + samples):
        command = [
            str(fixture.variant.binary),
            "--json",
            "ui",
            "--no-open",
            "--port",
            "0",
        ]
        started = time.perf_counter_ns()
        process = subprocess.Popen(
            command,
            cwd=fixture.repo,
            env=command_environment(home),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            line = read_line_with_timeout(process, 15)
            envelope = json.loads(line)
            if envelope.get("ok") is not True:
                raise BenchmarkError(f"UI start failed: {envelope!r}")
            url = envelope.get("data", {}).get("url")
            parsed_url = urllib.parse.urlsplit(url) if isinstance(url, str) else None
            if (
                parsed_url is None
                or parsed_url.scheme != "http"
                or parsed_url.hostname != "127.0.0.1"
                or parsed_url.port is None
                or parsed_url.username is not None
                or parsed_url.password is not None
            ):
                raise BenchmarkError(f"UI did not bind IPv4 loopback: {url!r}")
            api_url = urllib.parse.urljoin(url, "api/v1/overview")
            request = urllib.request.Request(
                api_url,
                headers={"Accept": "application/json"},
                method="GET",
            )
            with urllib.request.urlopen(request, timeout=15) as response:
                body = response.read()
                status = response.status
            duration_ns = time.perf_counter_ns() - started
            api = json.loads(body)
            if (
                status != 200
                or api.get("product_version") != fixture.variant.product_version
                or api.get("workspace") != fixture.workspace_key
                or api.get("read_only") is not True
            ):
                raise BenchmarkError(
                    f"UI overview API failed: status={status}, body={body[:500]!r}"
                )
            if index >= warmups:
                measured.append(
                    {
                        "duration_ns": duration_ns,
                        "duration_ms": duration_ns / 1_000_000,
                        "exit_code": 0,
                        "stdout_bytes": len(line),
                        "stderr_bytes": 0,
                        "stdout_sha256": hashlib.sha256(body).hexdigest(),
                        "http_status": status,
                        "response_bytes": len(body),
                    }
                )
        finally:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)
    return supported_result(fixture, metric, warmups, measured)


def read_line_with_timeout(process: subprocess.Popen[bytes], timeout: int) -> bytes:
    if process.stdout is None:
        raise BenchmarkError("UI stdout was not captured")
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    try:
        if not selector.select(timeout):
            raise BenchmarkError("timed out waiting for UI startup JSON")
        line = process.stdout.readline()
    finally:
        selector.close()
    if not line:
        stderr = b"" if process.stderr is None else process.stderr.read()
        raise BenchmarkError(
            f"UI stopped before startup JSON: rc={process.poll()}, stderr={stderr[-2000:]!r}"
        )
    return line


def measure_export(
    fixture: Fixture,
    work_root: Path,
    warmups: int,
    samples: int,
) -> dict[str, Any]:
    metric = "export_debug_capsule"
    if fixture.variant.name == "baseline":
        return unsupported_result(
            fixture.variant,
            fixture.corpus.name,
            metric,
            "v0.1.0-rc3 has no `aopmem observe export` command",
        )
    root = work_root / "measure" / fixture.variant.name / fixture.corpus.name / metric
    home = root / "home"
    outputs = root / "outputs"
    root.mkdir(parents=True, exist_ok=True)
    outputs.mkdir()
    clone_home(fixture.home, home)
    measured: list[dict[str, Any]] = []
    for index in range(warmups + samples):
        output = outputs / f"capsule-{index:03d}.zip"
        result, duration_ns = invoke(
            fixture.variant,
            fixture.repo,
            home,
            ["observe", "export", "--output", str(output)],
        )
        envelope = parse_envelope(result)
        if not output.is_file() or output.stat().st_size == 0:
            raise BenchmarkError("observe export did not publish a non-empty ZIP")
        if envelope.get("data", {}).get("publication_status") != "durable":
            raise BenchmarkError("observe export did not report durable publication")
        if index >= warmups:
            measured.append(
                sample_record(
                    duration_ns,
                    result,
                    artifact_bytes=output.stat().st_size,
                    artifact_sha256=sha256_file(output),
                )
            )
        output.unlink()
    return supported_result(fixture, metric, warmups, measured)


def supported_result(
    fixture: Fixture,
    metric: str,
    warmups: int,
    samples: list[dict[str, Any]],
) -> dict[str, Any]:
    return supported_result_for_variant(
        fixture.variant, fixture.corpus.name, metric, warmups, samples
    )


def supported_result_for_variant(
    variant: Variant,
    corpus: str,
    metric: str,
    warmups: int,
    samples: list[dict[str, Any]],
) -> dict[str, Any]:
    durations = [sample["duration_ms"] for sample in samples]
    if not durations:
        raise BenchmarkError(f"supported metric {metric} has no measured samples")
    ordered = sorted(durations)
    p95_index = max(0, math.ceil(0.95 * len(ordered)) - 1)
    return {
        "variant": variant.name,
        "product_version": variant.product_version,
        "corpus": corpus,
        "metric": metric,
        "supported": True,
        "unsupported_reason": None,
        "warmups": warmups,
        "sample_count": len(samples),
        "median_ms": statistics.median(durations),
        "p95_ms": ordered[p95_index],
        "min_ms": min(durations),
        "max_ms": max(durations),
        "samples": samples,
    }


def unsupported_result(
    variant: Variant,
    corpus: str,
    metric: str,
    reason: str,
) -> dict[str, Any]:
    return {
        "variant": variant.name,
        "product_version": variant.product_version,
        "corpus": corpus,
        "metric": metric,
        "supported": False,
        "unsupported_reason": reason,
        "warmups": 0,
        "sample_count": 0,
        "median_ms": None,
        "p95_ms": None,
        "min_ms": None,
        "max_ms": None,
        "samples": [],
    }


def validate_first_page(expected: int) -> Callable[[dict[str, Any]], None]:
    def validator(envelope: dict[str, Any]) -> None:
        data = envelope.get("data", {})
        nodes = data.get("nodes")
        if not isinstance(nodes, list) or len(nodes) != min(100, expected):
            raise BenchmarkError("node first page returned an unexpected count")
        expected_more = expected > 100
        if data.get("more_results") is not expected_more:
            raise BenchmarkError("node first page more_results is incorrect")
        if data.get("body_omitted") is not True:
            raise BenchmarkError("node first page did not omit bodies")

    return validator


def validate_tool_list(expected: int) -> Callable[[dict[str, Any]], None]:
    def validator(envelope: dict[str, Any]) -> None:
        tools = envelope.get("data", {}).get("tools")
        if not isinstance(tools, list) or len(tools) != expected:
            raise BenchmarkError("tool list returned an unexpected count")

    return validator


def validate_recall_query(envelope: dict[str, Any]) -> None:
    data = envelope.get("data", {})
    if data.get("mode") != "task" or not isinstance(data.get("bundle_id"), str):
        raise BenchmarkError("query recall did not return a task bundle id")
    selected = data.get("task", {}).get("nodes")
    if not isinstance(selected, list):
        raise BenchmarkError("query recall did not return selected task nodes")
    titles = {
        item.get("node", {}).get("title") for item in selected if isinstance(item, dict)
    }
    if "Deploy release workflow" not in titles:
        raise BenchmarkError("query recall missed the exact workflow")


def validate_verify_clean(envelope: dict[str, Any]) -> None:
    if envelope.get("data", {}).get("clean") is not True:
        raise BenchmarkError("verify was not clean")


def run_benchmarks(
    variants: tuple[Variant, Variant],
    fixtures: dict[tuple[str, str], Fixture],
    corpora: tuple[CorpusSpec, ...],
    work_root: Path,
    warmups: int,
    samples: int,
) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for variant in variants:
        results.append(measure_init(variant, work_root, warmups, samples))
    for spec in corpora:
        for variant in variants:
            fixture = fixtures[(variant.name, spec.name)]
            if variant.name == "baseline":
                results.append(
                    unsupported_result(
                        variant,
                        spec.name,
                        "node_list_first_page",
                        "v0.1.0-rc3 node list is unbounded and has no page-size or cursor contract",
                    )
                )
            else:
                results.append(
                    measure_command(
                        fixture,
                        work_root,
                        "node_list_first_page",
                        ["node", "list", "--limit", "100"],
                        warmups,
                        samples,
                        validator=validate_first_page(spec.nodes),
                    )
                )
            results.append(
                measure_full_pagination(fixture, work_root, warmups, samples)
            )
            results.append(
                measure_command(
                    fixture,
                    work_root,
                    "recall_baseline",
                    ["recall"],
                    warmups,
                    samples,
                )
            )
            if variant.name == "baseline":
                results.append(
                    unsupported_result(
                        variant,
                        spec.name,
                        "recall_query",
                        "v0.1.0-rc3 recall has no `--query` task-retrieval contract",
                    )
                )
            else:
                results.append(
                    measure_command(
                        fixture,
                        work_root,
                        "recall_query",
                        ["recall", "--query", "Deploy release workflow"],
                        warmups,
                        samples,
                        validator=validate_recall_query,
                    )
                )
            tool_arguments = (
                ["tool", "list"]
                if variant.name == "baseline"
                else ["tool", "list", "--all", "--limit", "500"]
            )
            results.append(
                measure_command(
                    fixture,
                    work_root,
                    "tool_list",
                    tool_arguments,
                    warmups,
                    samples,
                    validator=validate_tool_list(spec.tools),
                )
            )
            results.append(
                measure_command(
                    fixture,
                    work_root,
                    "doctor",
                    ["doctor"],
                    warmups,
                    samples,
                )
            )
            results.append(
                measure_command(
                    fixture,
                    work_root,
                    "verify",
                    ["verify"],
                    warmups,
                    samples,
                    accepted_exit_codes=(0, 8),
                    validator=validate_verify_clean,
                )
            )
            results.append(
                measure_command(
                    fixture,
                    work_root,
                    "audit_snapshot_mutation",
                    [
                        "node",
                        "create",
                        "--type",
                        "lesson",
                        "--status",
                        "active",
                        "--title",
                        "Measured snapshot mutation",
                        "--summary",
                        "One deterministic mutation used only for snapshot timing.",
                        "--body",
                        "Benchmark mutation body.",
                        "--source-ref",
                        "source=user_instruction:benchmark",
                        "--confidence",
                        "1.0",
                        "--trust-level",
                        "high",
                    ],
                    warmups,
                    samples,
                    fresh_home_each_time=True,
                )
            )
            results.append(measure_observability(fixture, work_root, warmups, samples))
            results.append(measure_ui_initial_api(fixture, work_root, warmups, samples))
            results.append(measure_export(fixture, work_root, warmups, samples))
    return results


def write_outputs(
    output_dir: Path,
    metadata: dict[str, Any],
    fixtures: dict[tuple[str, str], Fixture],
    corpora: tuple[CorpusSpec, ...],
    results: list[dict[str, Any]],
) -> None:
    if output_dir.exists():
        marker = output_dir / OUTPUT_MARKER
        if not marker.is_file() or marker.read_text(encoding="utf-8") != "stage34\n":
            raise BenchmarkError(
                f"refusing to replace unmarked benchmark output directory: {output_dir}"
            )
        shutil.rmtree(output_dir)
    corpus_dir = output_dir / "corpora"
    raw_dir = output_dir / "raw"
    corpus_dir.mkdir(parents=True)
    raw_dir.mkdir()
    (output_dir / OUTPUT_MARKER).write_text("stage34\n", encoding="utf-8")
    write_json(output_dir / "run.json", metadata)

    corpus_index: list[dict[str, Any]] = []
    for spec in corpora:
        baseline = fixtures[("baseline", spec.name)]
        current = fixtures[("current", spec.name)]
        if baseline.logical_sha256 != current.logical_sha256:
            raise BenchmarkError(
                f"logical corpus hashes differ for {spec.name}: "
                f"{baseline.logical_sha256} != {current.logical_sha256}"
            )
        manifest = {
            "generator_version": GENERATOR_VERSION,
            "name": spec.name,
            "nodes": spec.nodes,
            "links": spec.links,
            "aliases": spec.nodes,
            "tags": spec.nodes,
            "sources": spec.nodes,
            "operational_events": spec.nodes,
            "generator_events_in_logical_hash": spec.nodes - 1,
            "product_snapshot_finalize_events": 1,
            "tools": spec.tools,
            "mcp_profiles": 2,
            "observability_events_current": spec.observability_events,
            "observability_events_baseline": 0,
            "logical_sha256": baseline.logical_sha256,
            "deterministic_timestamp": FIXED_TIMESTAMP,
            "query_probe": "Deploy release workflow",
            "fixture_persistence": "generated in disposable work dir; not retained",
        }
        write_json(corpus_dir / f"{spec.name}.json", manifest)
        corpus_index.append(manifest)
    write_json(corpus_dir / "index.json", corpus_index)

    write_json(raw_dir / "samples.json", results)
    raw_fields = (
        "variant",
        "product_version",
        "corpus",
        "metric",
        "supported",
        "unsupported_reason",
        "sample_index",
        "duration_ns",
        "duration_ms",
        "exit_code",
        "stdout_bytes",
        "stderr_bytes",
        "stdout_sha256",
        "core_duration_ms",
        "wall_minus_core_ms",
        "pages",
        "returned_nodes",
        "http_status",
        "response_bytes",
        "artifact_bytes",
        "artifact_sha256",
        "observability_events_delta",
        "observability_bytes_before",
        "observability_bytes_after",
    )
    with (raw_dir / "samples.csv").open("w", encoding="utf-8", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=raw_fields)
        writer.writeheader()
        for result in results:
            common = {
                "variant": result["variant"],
                "product_version": result["product_version"],
                "corpus": result["corpus"],
                "metric": result["metric"],
                "supported": result["supported"],
                "unsupported_reason": result["unsupported_reason"],
            }
            if not result["supported"]:
                writer.writerow(common)
                continue
            for sample_index, sample in enumerate(result["samples"], start=1):
                writer.writerow(
                    {
                        **common,
                        "sample_index": sample_index,
                        **{
                            field: sample.get(field)
                            for field in raw_fields
                            if field in sample
                        },
                    }
                )
    summary_fields = (
        "variant",
        "product_version",
        "corpus",
        "metric",
        "supported",
        "unsupported_reason",
        "warmups",
        "sample_count",
        "median_ms",
        "p95_ms",
        "min_ms",
        "max_ms",
    )
    with (output_dir / "summary.csv").open("w", encoding="utf-8", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=summary_fields)
        writer.writeheader()
        for result in results:
            writer.writerow({field: result.get(field) for field in summary_fields})
    (output_dir / "README.md").write_text(
        """# AOPMem v0.2 benchmark raw evidence

- `run.json`: host, toolchain, binary hashes, source provenance, and sampling contract.
- `corpora/*.json`: deterministic logical corpus counts and SHA-256 manifests.
- `raw/samples.json` and `raw/samples.csv`: every measured wall-clock sample and exact unsupported reason.
- `summary.csv`: median, nearest-rank p95, minimum, and maximum in milliseconds.
- `SHA256SUMS`: integrity hashes for every evidence file except itself.

Disposable SQLite workspaces are not retained. Regenerate them with
`scripts/benchmark_v020.sh`; the corpus manifests prove logical parity between
the peeled v0.1.0-rc3 tag fixture and the v0.2.0-rc1 fixture.
""",
        encoding="utf-8",
    )
    evidence_files = sorted(
        (
            path
            for path in output_dir.rglob("*")
            if path.is_file() and path.name != "SHA256SUMS"
        ),
        key=lambda path: path.relative_to(output_dir).as_posix(),
    )
    (output_dir / "SHA256SUMS").write_text(
        "".join(
            f"{sha256_file(path)}  {path.relative_to(output_dir).as_posix()}\n"
            for path in evidence_files
        ),
        encoding="utf-8",
    )


def validate_output_destination(output_dir: Path) -> None:
    if not output_dir.exists():
        return
    marker = output_dir / OUTPUT_MARKER
    if not marker.is_file() or marker.read_text(encoding="utf-8") != "stage34\n":
        raise BenchmarkError(
            f"refusing to replace unmarked benchmark output directory: {output_dir}"
        )


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def metadata(
    baseline: Variant,
    current: Variant,
    baseline_commit: str,
    build_profile: str,
    warmups: int,
    samples: int,
    corpora: tuple[CorpusSpec, ...],
) -> dict[str, Any]:
    repo_root = Path(__file__).resolve().parent.parent
    current_head = command_text(("git", "rev-parse", "HEAD"), repo_root)
    current_status = command_text(("git", "status", "--porcelain"), repo_root)
    return {
        "benchmark_contract": "AOPMem v0.2.0-rc1 stage 34",
        "generator_version": GENERATOR_VERSION,
        "generated_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "warmups": warmups,
        "measured_samples": samples,
        "clock": "time.perf_counter_ns",
        "build_profile": build_profile,
        "toolchain": {
            "rustc": command_text(("rustc", "--version"), repo_root),
            "cargo": command_text(("cargo", "--version"), repo_root),
        },
        "statistics": {
            "median": "statistics.median",
            "p95": "nearest-rank ceil(0.95*n)",
        },
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "python": platform.python_version(),
        },
        "variants": {
            baseline.name: {
                "product_version": baseline.product_version,
                "source_release": baseline.source_release,
                "peeled_commit": baseline_commit,
                "binary_sha256": sha256_file(baseline.binary),
            },
            current.name: {
                "product_version": current.product_version,
                "source_release": current.source_release,
                "head_commit": current_head,
                "worktree_dirty": bool(current_status),
                "source_tree_sha256": source_tree_sha256(repo_root),
                "binary_sha256": sha256_file(current.binary),
            },
        },
        "corpora": [spec.name for spec in corpora],
        "notes": [
            "No retained workspace is used; all fixtures live below a disposable work directory.",
            "Unsupported v0.1 operations are recorded, not emulated.",
            "Every supported metric uses the requested warmup and measured sample counts.",
            "Wall-clock process timings include startup and JSON serialization unless a metric says otherwise.",
        ],
    }


def main() -> int:
    args = parse_args()
    if len(args.baseline_commit) != 40 or any(
        character not in "0123456789abcdef" for character in args.baseline_commit
    ):
        raise BenchmarkError("--baseline-commit must be a lowercase 40-hex commit id")
    baseline_binary = args.baseline_binary.resolve(strict=True)
    current_binary = args.current_binary.resolve(strict=True)
    baseline = Variant(
        "baseline",
        baseline_binary,
        product_version(baseline_binary),
        "v0.1.0-rc3 peeled tag",
    )
    current = Variant(
        "current",
        current_binary,
        product_version(current_binary),
        "v0.2.0-rc1 worktree",
    )
    if baseline.product_version != "0.1.0":
        raise BenchmarkError(
            f"baseline binary is {baseline.product_version}, expected tag payload version 0.1.0"
        )
    if current.product_version != "0.2.0-rc1":
        raise BenchmarkError(
            f"current binary is {current.product_version}, expected 0.2.0-rc1"
        )
    corpora = selected_corpora(args.corpus)
    output_dir = args.output_dir.resolve()
    validate_output_destination(output_dir)

    temporary: tempfile.TemporaryDirectory[str] | None = None
    if args.work_dir is None:
        temporary = tempfile.TemporaryDirectory(prefix="aopmem-v020-benchmark-")
        work_root = Path(temporary.name)
    else:
        work_root = args.work_dir.resolve()
        if work_root.exists():
            if not work_root.is_dir() or any(work_root.iterdir()):
                raise BenchmarkError(
                    f"--work-dir must not exist or must be an empty directory: {work_root}"
                )
        else:
            work_root.mkdir(parents=True)

    try:
        fixtures: dict[tuple[str, str], Fixture] = {}
        for spec in corpora:
            for variant in (baseline, current):
                fixture = prepare_fixture(work_root, variant, spec)
                fixtures[(variant.name, spec.name)] = fixture
            if (
                fixtures[("baseline", spec.name)].logical_sha256
                != fixtures[("current", spec.name)].logical_sha256
            ):
                raise BenchmarkError(f"logical corpus parity failed for {spec.name}")

        results = run_benchmarks(
            (baseline, current),
            fixtures,
            corpora,
            work_root,
            args.warmups,
            args.samples,
        )
        write_outputs(
            output_dir,
            metadata(
                baseline,
                current,
                args.baseline_commit,
                args.build_profile,
                args.warmups,
                args.samples,
                corpora,
            ),
            fixtures,
            corpora,
            results,
        )
        print(output_dir)
    finally:
        if temporary is not None and not args.keep_work_dir:
            temporary.cleanup()
        elif args.keep_work_dir:
            print(f"kept work dir: {work_root}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BenchmarkError as error:
        print(f"benchmark error: {error}", file=sys.stderr)
        raise SystemExit(1)
