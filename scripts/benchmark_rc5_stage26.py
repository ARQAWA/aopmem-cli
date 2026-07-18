#!/usr/bin/env python3
"""Focused, CLI-only RC5 performance evidence generator.

This is deliberately not a general benchmark framework.  It builds three
bounded disposable AOPMem homes through the public CLI, records raw wall-clock
samples, and writes nearest-rank p95 summaries plus integrity hashes.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


HARNESS_VERSION = 1
MARKER = ".aopmem-rc5-stage26-benchmark-output"
INIT_INPUT = b"no\nno\nRC5 performance fixture.\nBounded CLI-only corpus.\nNo secrets.\n"


@dataclass(frozen=True)
class Corpus:
    name: str
    extra_active_rules: int


CORPORA = (Corpus("small", 16), Corpus("medium", 64), Corpus("large", 256))
FIXED_METRICS = ("task_apply", "task_complete", "duplicate_preflight",
                 "canonical_resolution_fast_path", "audit_repair", "debug_export",
                 "platform_check")


class BenchmarkError(RuntimeError):
    pass


def positive(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/aopmem"))
    parser.add_argument("--output-dir", type=Path,
                        default=Path(".devplan/benchmarks/rc5_stage26"))
    parser.add_argument("--samples", type=positive, default=15)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--corpus", choices=("all",) + tuple(c.name for c in CORPORA),
                        default="all")
    parser.add_argument("--keep-work-dir", action="store_true")
    parsed = parser.parse_args()
    if parsed.warmups < 0:
        parser.error("--warmups must be non-negative")
    return parsed


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def environment(home: Path) -> dict[str, str]:
    result = os.environ.copy()
    result.update({"AOPMEM_HOME": str(home), "LC_ALL": "C", "LANG": "C", "TZ": "UTC"})
    return result


def cli(binary: Path, repo: Path, home: Path, command: list[str], *, stdin: bytes | None = None,
        expected: Iterable[int] = (0,)) -> tuple[subprocess.CompletedProcess[bytes], int]:
    started = time.perf_counter_ns()
    process = subprocess.run([str(binary), "--json", *command], cwd=repo,
                             env=environment(home), input=stdin, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, check=False, timeout=120)
    elapsed = time.perf_counter_ns() - started
    if process.returncode not in set(expected):
        raise BenchmarkError("command failed ({}) : {}\nstdout={}\nstderr={}".format(
            process.returncode, " ".join(command), process.stdout.decode("utf-8", "replace")[-1000:],
            process.stderr.decode("utf-8", "replace")[-1000:]))
    return process, elapsed


def envelope(process: subprocess.CompletedProcess[bytes]) -> dict[str, Any]:
    try:
        parsed = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise BenchmarkError("CLI did not return JSON") from error
    if not isinstance(parsed, dict):
        raise BenchmarkError("CLI returned non-object JSON")
    return parsed


def required_ok(process: subprocess.CompletedProcess[bytes]) -> dict[str, Any]:
    parsed = envelope(process)
    if parsed.get("ok") is not True:
        raise BenchmarkError("CLI error: {}".format(parsed.get("errors")))
    return parsed


def selected_corpora(name: str) -> tuple[Corpus, ...]:
    return CORPORA if name == "all" else tuple(c for c in CORPORA if c.name == name)


def setup_fixture(root: Path, binary: Path, corpus: Corpus) -> tuple[Path, Path]:
    repo = root / "repos" / corpus.name
    home = root / "homes" / corpus.name
    repo.mkdir(parents=True)
    subprocess.run(["git", "init", "-q"], cwd=repo, check=True, timeout=30)
    required_ok(cli(binary, repo, home, ["init"], stdin=INIT_INPUT)[0])
    # Public CLI only: no SQLite/API fixture writes and no WAL/SHM handling.
    for index in range(corpus.extra_active_rules):
        required_ok(cli(binary, repo, home, [
            "node", "create", "--type", "rule", "--status", "active",
            "--title", f"RC5 benchmark rule {index:04d}",
            "--summary", "Bounded stage 026 retrieval corpus rule.",
            "--body", f"benchmark-token-{index % 8}",
            "--source-ref", "stage26:fixture", "--confidence", "0.90",
            "--trust-level", "explicit_user",
        ])[0])
    required_ok(cli(binary, repo, home, [
        "tool", "create-draft", "--id", "stage26_tool", "--name", "Stage 026 tool",
        "--entrypoint", "run.sh", "--side-effects", "none",
        "--approval-requirement", "none",
    ])[0])
    required_ok(cli(binary, repo, home, ["doctor"])[0])
    return repo, home


def task_start(binary: Path, repo: Path, home: Path, marker: str) -> tuple[dict[str, Any], int]:
    process, duration = cli(binary, repo, home, ["task", "start", "--query", marker])
    return required_ok(process), duration


def task_id(start: dict[str, Any]) -> str:
    value = start.get("data", {}).get("task_id")
    if not isinstance(value, str):
        raise BenchmarkError("task start has no task_id")
    return value


def apply_command(start: dict[str, Any]) -> list[str]:
    data = start.get("data", {})
    bundle_id = data.get("bundle_id")
    if not isinstance(bundle_id, str):
        raise BenchmarkError("task start has no bundle_id")
    command = ["task", "apply", "--task-id", task_id(start), "--bundle-id", bundle_id]
    # The fixture query is intentionally unmatched.  Mandatory context still
    # loads, while task-specific nodes are empty, so this is the contractually
    # valid no-relevance path.
    if data.get("task_nodes"):
        raise BenchmarkError("fixture query unexpectedly selected task nodes")
    return command + ["--none-relevant"]


def complete(binary: Path, repo: Path, home: Path, start: dict[str, Any]) -> None:
    required_ok(cli(binary, repo, home,
                    ["task", "complete", "--task-id", task_id(start),
                     "--bundle-id", start["data"]["bundle_id"], "--result", "success"])[0])


def record(rows: list[dict[str, Any]], metric: str, corpus: str, phase: str, sample: int,
           elapsed: int, exit_code: int = 0) -> None:
    rows.append({"metric": metric, "corpus": corpus, "phase": phase, "sample": sample,
                 "elapsed_ns": elapsed, "elapsed_ms": round(elapsed / 1_000_000, 3),
                 "exit_code": exit_code})


def run_measured(binary: Path, repo: Path, home: Path, corpus: str, rows: list[dict[str, Any]],
                 samples: int, warmups: int) -> None:
    for phase, total in (("warmup", warmups), ("sample", samples)):
        for sample in range(1, total + 1):
            start, elapsed = task_start(binary, repo, home, f"stage26-{corpus}-{phase}-{sample}")
            if phase == "sample":
                record(rows, "task_start", corpus, phase, sample, elapsed)
            apply, apply_elapsed = cli(binary, repo, home, apply_command(start))
            required_ok(apply)
            if phase == "sample":
                record(rows, "task_apply", corpus, phase, sample, apply_elapsed)
            finish, complete_elapsed = cli(binary, repo, home,
                                           ["task", "complete", "--task-id", task_id(start),
                                            "--bundle-id", start["data"]["bundle_id"],
                                            "--result", "success"])
            required_ok(finish)
            if phase == "sample":
                record(rows, "task_complete", corpus, phase, sample, complete_elapsed)

            duplicate, duplicate_elapsed = cli(binary, repo, home, [
                "tool", "create-draft", "--id", "stage26_tool", "--name", "Stage 026 tool",
                "--entrypoint", "run.sh", "--side-effects", "none",
                "--approval-requirement", "none",
            ], expected=(6,))
            error = envelope(duplicate)
            if error.get("ok") is not False:
                raise BenchmarkError("duplicate preflight unexpectedly succeeded")
            if phase == "sample":
                record(rows, "duplicate_preflight", corpus, phase, sample, duplicate_elapsed,
                       duplicate.returncode)

            alias, alias_elapsed = cli(binary, repo, home,
                                       ["tool", "resolve", "stage26_tool"])
            required_ok(alias)
            if phase == "sample":
                record(rows, "canonical_resolution_fast_path", corpus, phase, sample, alias_elapsed)

            repair, repair_elapsed = cli(binary, repo, home,
                                         ["audit", "repair", "--current-workspace"])
            required_ok(repair)
            if phase == "sample":
                record(rows, "audit_repair", corpus, phase, sample, repair_elapsed)

            output = repo / "capsules" / f"{phase}-{sample}.zip"
            output.parent.mkdir(exist_ok=True)
            export, export_elapsed = cli(binary, repo, home,
                                         ["observe", "export", "--output", str(output)])
            required_ok(export)
            if not output.is_file():
                raise BenchmarkError("debug export reported success without a capsule")
            if phase == "sample":
                record(rows, "debug_export", corpus, phase, sample, export_elapsed)

            check, check_elapsed = cli(binary, repo, home, ["platform", "check"])
            required_ok(check)
            if phase == "sample":
                record(rows, "platform_check", corpus, phase, sample, check_elapsed)


def nearest_rank_p95(values: list[int]) -> int:
    ordered = sorted(values)
    return ordered[(95 * len(ordered) + 99) // 100 - 1]


def summarize(rows: list[dict[str, Any]], samples: int, warmups: int) -> list[dict[str, Any]]:
    result = []
    for corpus in sorted({row["corpus"] for row in rows}):
        for metric in sorted({row["metric"] for row in rows if row["corpus"] == corpus}):
            values = [row["elapsed_ns"] for row in rows
                      if row["corpus"] == corpus and row["metric"] == metric and row["phase"] == "sample"]
            if len(values) != samples:
                raise BenchmarkError(f"incomplete samples for {corpus}/{metric}")
            result.append({"corpus": corpus, "metric": metric, "samples": samples,
                           "warmups": warmups, "median_ns": int(statistics.median(values)),
                           "median_ms": round(statistics.median(values) / 1_000_000, 3),
                           "p95_nearest_rank_ns": nearest_rank_p95(values),
                           "p95_nearest_rank_ms": round(nearest_rank_p95(values) / 1_000_000, 3),
                           "min_ms": round(min(values) / 1_000_000, 3),
                           "max_ms": round(max(values) / 1_000_000, 3)})
    return result


def structural_checks(repo: Path) -> dict[str, Any]:
    tools = (repo / "src/tools/mod.rs").read_text(encoding="utf-8")
    recall = (repo / "src/recall/mod.rs").read_text(encoding="utf-8")
    run_body = tools[tools.index("pub fn run_tool("):tools.index("pub fn dry_run_tool(")]
    required = {
        "tool_candidate_limit": "MAX_TOOL_CREATION_GUARD_CANDIDATES: usize = 64",
        "tool_file_limit": "MAX_TOOL_IMPLEMENTATION_FILES: usize = 256",
        "tool_hash_once": "fn hash_open_file_once",
        "task_budget": "TASK_RECALL_SOFT_BUDGET_BYTES",
    }
    sources = {"tools": tools, "recall": recall}
    missing = [name for name, needle in required.items()
               if not any(needle in text for text in sources.values())]
    if "fingerprint_tool_implementation" in run_body:
        missing.append("normal_run_no_fingerprint")
    if missing:
        raise BenchmarkError("structural bound evidence missing: " + ", ".join(missing))
    return {"status": "PASS", "checks": required,
            "conclusion": "bounded shortlist/file/budget guards present; normal tool run has no fingerprint path"}


def write_output(output: Path, rows: list[dict[str, Any]], summary: list[dict[str, Any]], metadata: dict[str, Any]) -> None:
    output.mkdir(parents=True, exist_ok=True)
    (output / MARKER).write_text("stage26\n", encoding="utf-8")
    raw = output / "raw_samples.csv"
    with raw.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    (output / "summary.json").write_text(json.dumps({"metadata": metadata, "results": summary}, indent=2) + "\n",
                                        encoding="utf-8")
    (output / "structural_checks.json").write_text(json.dumps(metadata["structural_checks"], indent=2) + "\n",
                                                  encoding="utf-8")
    lines = []
    for path in sorted(output.iterdir()):
        if path.is_file() and path.name != "SHA256SUMS":
            lines.append(f"{sha256(path)}  {path.name}")
    (output / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    args = arguments()
    binary = args.binary.resolve()
    repo = Path(__file__).resolve().parents[1]
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise BenchmarkError(f"binary is not executable: {binary}")
    output = args.output_dir.resolve()
    if output.exists() and not (output / MARKER).is_file():
        raise BenchmarkError(f"refusing non-benchmark output directory: {output}")
    if output.exists():
        shutil.rmtree(output)
    rows: list[dict[str, Any]] = []
    temp = Path(tempfile.mkdtemp(prefix="aopmem-rc5-stage26-"))
    try:
        for corpus in selected_corpora(args.corpus):
            fixture_repo, home = setup_fixture(temp, binary, corpus)
            run_measured(binary, fixture_repo, home, corpus.name, rows, args.samples, args.warmups)
        version = subprocess.run([str(binary), "--version"], check=True, capture_output=True,
                                 text=True, timeout=30).stdout.strip()
        metadata = {"harness_version": HARNESS_VERSION, "binary": str(binary),
                    "binary_sha256": sha256(binary), "binary_version": version,
                    "host": {"system": platform.system(), "release": platform.release(),
                             "machine": platform.machine(), "python": sys.version.split()[0]},
                    "samples": args.samples, "warmups": args.warmups,
                    "corpora": [{"name": c.name, "extra_active_rules": c.extra_active_rules}
                                for c in selected_corpora(args.corpus)],
                    "method": "wall-clock subprocess duration; median; nearest-rank p95",
                    "setup": "isolated temporary AOPMEM_HOME and git repo; public CLI commands only",
                    "structural_checks": structural_checks(repo)}
        write_output(output, rows, summarize(rows, args.samples, args.warmups), metadata)
    finally:
        if args.keep_work_dir:
            print(f"kept work directory: {temp}", file=sys.stderr)
        else:
            shutil.rmtree(temp)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (BenchmarkError, subprocess.SubprocessError, OSError) as error:
        print(f"benchmark failed: {error}", file=sys.stderr)
        raise SystemExit(1)
