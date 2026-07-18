# RC5 Stage 026 — Focused Performance Proof

Status: `DONE_LOCAL_CHECKS_PASSED`

Binary: `target/debug/aopmem` (`aopmem 0.2.0-rc5`), SHA-256
`245a7efff79119da59f955d4ee489f78321b90e03235512b432181cf4c8feb97`.

## Result

The focused RC5 overhead proof passed. It measures wall-clock subprocess time,
with 3 warmups and 15 samples per row. Median is standard median. P95 is the
nearest-rank value at `ceil(0.95 * 15) = 15`; it is not an interpolated value.
There is no before/after dataset, so this report makes no percentage claim.

| Corpus | Operation | Median ms | P95 ms |
|---|---|---:|---:|
| small (16) | task start | 20.039 | 22.061 |
| medium (64) | task start | 25.526 | 27.968 |
| large (256) | task start | 46.891 | 65.797 |
| small | task apply / complete | 19.215 / 18.782 | 24.078 / 21.873 |
| medium | task apply / complete | 21.971 / 21.161 | 23.712 / 23.341 |
| large | task apply / complete | 35.062 / 32.183 | 40.392 / 81.662 |
| small | duplicate preflight / canonical resolver | 11.979 / 5.577 | 13.564 / 6.442 |
| medium | duplicate preflight / canonical resolver | 13.162 / 5.644 | 14.896 / 6.487 |
| large | duplicate preflight / canonical resolver | 19.464 / 6.163 | 35.033 / 9.453 |
| small | audit repair / debug export / platform check | 10.579 / 25.162 / 37.465 | 11.301 / 28.652 / 40.886 |
| medium | audit repair / debug export / platform check | 11.612 / 33.928 / 39.594 | 14.002 / 42.792 / 45.852 |
| large | audit repair / debug export / platform check | 16.522 / 75.128 / 41.639 | 28.213 / 103.092 / 49.531 |

## Reproduction and evidence

Run:

```text
python3 scripts/benchmark_rc5_stage26.py \
  --binary target/debug/aopmem \
  --output-dir .devplan/benchmarks/rc5_stage26 \
  --samples 15 --warmups 3
(cd .devplan/benchmarks/rc5_stage26 && shasum -a 256 -c SHA256SUMS)
```

The harness creates a fresh temporary `AOPMEM_HOME` and Git repository for
each corpus. It uses production CLI commands for `init`, node creation, task
lifecycle, tool creation/resolution, audit repair, export, and platform check.
It does not open SQLite, manipulate WAL/SHM, use an admin flow, WSL, or a
source-build workaround. `raw_samples.csv`, `summary.json`,
`structural_checks.json`, and `SHA256SUMS` are retained under
`.devplan/benchmarks/rc5_stage26/`.

The CLI table names this row `canonical_resolution_fast_path`. It is retained
as a separate wall-clock process measurement and is not presented as alias
resolution.

## Alias-resolution method (separate in-process evidence)

The test-only benchmark
`tools::tests::stage_026_alias_resolution_benchmark` uses the production
`create_tool_contract`, `add_tool_alias`, and `resolve_tool_id` APIs. For each
16/64/256 active-tool corpus it verifies alias insertion and a real alias
resolution (`kind=Alias`, matching alias present; equivalent to
`resolved_via_alias=true`), then records 3 warmups and 15 samples.

This is in-process nanosecond timing. It must not be compared with the CLI
wall-clock table above.

| Corpus | Median ns | P95 ns | Alias resolved |
|---|---:|---:|---|
| small (16) | 61,458 | 67,084 | true |
| medium (64) | 57,833 | 74,542 | true |
| large (256) | 56,917 | 58,833 | true |

Reproduce the alias evidence:

```text
AOPMEM_STAGE26_ALIAS_BENCH_OUTPUT="$PWD/.devplan/benchmarks/rc5_stage26" \
  cargo test --locked stage_026_alias_resolution_benchmark -- --test-threads=1
(cd .devplan/benchmarks/rc5_stage26 && shasum -a 256 -c alias_SHA256SUMS)
```

`alias_raw_samples.csv`, `alias_summary.json`, and `alias_SHA256SUMS` are the
separate integrity set. They do not use raw SQL, WAL/SHM manipulation, or a
runtime/public product API.

## Bounds and complexity review

`structural_checks.json` passed against the current source:

- task retrieval has `TASK_RECALL_SOFT_BUDGET_BYTES` (256 KiB) and mandatory
  context has its separate hard bound;
- creation preflight shortlists at most 64 candidates;
- implementation files are bounded at 256 per tool;
- implementation hashing is through `hash_open_file_once` per fingerprint
  operation;
- `run_tool` contains no `fingerprint_tool_implementation` call, so normal
  tool runs do not perform N+1 implementation hashes.

The harness adds no production code. No broad optimization work was needed.

## Scope and limits

This is local macOS evidence on one host, not a latency SLA and not native
Windows proof. Native Windows remains `PENDING_DOGFOOD`.

Self-review: P1 `0`; P2 `0`.
