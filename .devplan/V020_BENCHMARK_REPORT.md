# AOPMem v0.2.0-rc1 Benchmark Report

Verdict: `PASS`

The benchmark is reproducible and complete for the Stage 34 scope.
No P1 or P2 correctness issue was found. No percentage speed claim is made.

## Provenance

| Item | Value |
|---|---|
| Baseline source | peeled tag `v0.1.0-rc3` |
| Baseline commit | `9877d39a4bc44cf62140aace8755720044c1d41f` |
| Baseline package version | `0.1.0` |
| Baseline binary SHA-256 | `dbc67aa27324310ac35d028cfef1c73e2dfd6308ed4ae73d1314e014a5f5e6d2` |
| Current source | frozen `v0.2.0-rc1` worktree |
| Current package version | `0.2.0-rc1` |
| Current source-tree SHA-256 | `91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4` |
| Current binary SHA-256 | `12ec578dc641373e0e22b67f548fb2862620571eb9777026304cd46e10427e61` |
| Build profile | Cargo `release --locked` |
| Host | macOS `26.5.1`, Apple Silicon `arm64` |
| Rust | `rustc 1.95.0` |
| Python harness | Python `3.14.5`, standard library only |
| End-to-end run | `168.615 s` |

The `v0.1.0-rc3` tag payload identifies itself as package version `0.1.0`.
Raw data keeps that package version. It is not relabeled as `0.1.0-rc3`.

The current build came from the intentionally dirty, classified worktree.
The binary hash and compiled source-tree hash make that input explicit.

## Method

- One isolated tag archive built the baseline. No checkout or reset was used.
- The current frozen worktree built into a separate temporary target directory.
- Every workspace and `AOPMEM_HOME` lived in a disposable temporary root.
- No real user workspace was read or changed.
- Each supported series used 3 warmups and 20 measured samples.
- Release builds took 22.47 s for the tag and 40.17 s for the current tree.
- Timings use `time.perf_counter_ns` and include process startup and JSON output.
- Results report median and nearest-rank p95 in milliseconds.
- Unsupported tag commands were recorded, not emulated.
- Mutation timing used a fresh home clone for every sample.
- Full pagination used explicit 500-node keyset pages with full bodies.
- UI timing covers process start through the first authenticated loopback
  `GET api/v1/overview` response.

The harness loads synthetic rows directly only inside its disposable fixture.
It then performs one real `link add` mutation so the product republishes the
canonical SQL snapshot. `verify` must be clean before measurement begins.

## Corpora

| Corpus | Nodes | Links | Aliases | Tags | Sources | Tools | Obs. events | Logical SHA-256 |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| small | 100 | 300 | 100 | 100 | 100 | 5 | 100 | `6df66b15ebf02bce6eb82ea5bd8664ada961772aa1debc2d6c0ed8011a087f8b` |
| medium | 2,000 | 6,000 | 2,000 | 2,000 | 2,000 | 25 | 2,000 | `77f8cba1a2389a9cd14a75ab6966f371db1f543910d6778514f2fce159e128c7` |
| large | 10,000 | 30,000 | 10,000 | 10,000 | 10,000 | 100 | 10,000 | `6f3b08fec3ac07f9b6dcef7159a4ff9e4f3b97e61977fddab8e91b60be6af9ff` |

All corpora also contain workflows, tool-contract nodes, failure modes,
corrections, lessons, project facts, preferences, skills, incident scars,
decisions, two MCP profiles, and operational events.

The logical hash is equal between the tag fixture and current fixture for
every corpus. Local Observability is absent from the tag by design.

## Results

All values below are milliseconds. `unsupported` means the tag has no
equivalent contract. It does not mean zero time.

Init is corpus-independent:

| Version | Median | p95 |
|---|---:|---:|
| tag payload `0.1.0` | 13.191 | 15.598 |
| current `0.2.0-rc1` | 110.676 | 117.562 |

### Small corpus

| Metric | Tag median | Tag p95 | RC1 median | RC1 p95 |
|---|---:|---:|---:|---:|
| node list first page | unsupported | unsupported | 4.753 | 4.985 |
| node list full pagination | 4.584 | 5.485 | 5.310 | 5.810 |
| recall baseline | 4.220 | 4.399 | 7.235 | 7.650 |
| recall query | unsupported | unsupported | 8.372 | 11.548 |
| tool list | 4.011 | 4.152 | 4.293 | 4.474 |
| doctor | 4.259 | 4.468 | 6.762 | 7.390 |
| verify | 4.170 | 4.363 | 7.067 | 7.583 |
| audit snapshot mutation | 7.186 | 7.479 | 73.300 | 76.557 |
| observability wall | unsupported | unsupported | 6.837 | 7.441 |
| UI initial overview API | unsupported | unsupported | 6.026 | 17.707 |
| export capsule | unsupported | unsupported | 37.612 | 38.676 |

### Medium corpus

| Metric | Tag median | Tag p95 | RC1 median | RC1 p95 |
|---|---:|---:|---:|---:|
| node list first page | unsupported | unsupported | 4.757 | 4.864 |
| node list full pagination | 13.377 | 13.670 | 28.811 | 29.977 |
| recall baseline | 9.387 | 9.933 | 10.662 | 11.751 |
| recall query | unsupported | unsupported | 20.298 | 20.774 |
| tool list | 4.349 | 4.668 | 4.559 | 4.792 |
| doctor | 4.295 | 4.473 | 9.466 | 9.816 |
| verify | 9.853 | 10.003 | 13.531 | 13.740 |
| audit snapshot mutation | 43.827 | 49.333 | 99.618 | 103.253 |
| observability wall | unsupported | unsupported | 9.543 | 9.893 |
| UI initial overview API | unsupported | unsupported | 8.223 | 9.588 |
| export capsule | unsupported | unsupported | 459.171 | 467.815 |

### Large corpus

| Metric | Tag median | Tag p95 | RC1 median | RC1 p95 |
|---|---:|---:|---:|---:|
| node list first page | unsupported | unsupported | 4.841 | 5.235 |
| node list full pagination | 47.275 | 48.723 | 146.786 | 153.772 |
| recall baseline | 32.898 | 33.792 | 24.298 | 25.942 |
| recall query | unsupported | unsupported | 70.721 | 71.303 |
| tool list | 4.724 | 4.969 | 5.120 | 5.316 |
| doctor | 4.581 | 5.016 | 21.648 | 24.860 |
| verify | 34.811 | 35.078 | 43.036 | 45.084 |
| audit snapshot mutation | 196.137 | 202.644 | 206.193 | 218.505 |
| observability wall | unsupported | unsupported | 25.444 | 28.395 |
| UI initial overview API | unsupported | unsupported | 17.859 | 19.534 |
| export capsule | unsupported | unsupported | 2,268.556 | 2,373.789 |

## Result Interpretation

- The current first page stays near 5 ms p95 on all three corpora.
- Full current traversal uses 1, 4, and 20 CLI page invocations. The tag uses
  one unbounded invocation. The totals are therefore product-flow timings,
  not a claim about one equivalent SQLite primitive.
- Bare recall contracts differ. The large absolute result is 32.898 ms for
  the tag and 24.298 ms for RC1, but no speedup claim is made.
- Task query is new. Its medians are 8.372, 20.298, and 70.721 ms.
- Large snapshot mutation is 196.137 ms on the tag and 206.193 ms on RC1.
- RC1 has larger fixed cost for small snapshot mutations and init. The
  measured absolute values are retained above without hiding that cost.
- Large debug capsule export has a 2,373.789 ms p95.
- Large initial UI overview response has a 19.534 ms p95.

No correctness behavior was weakened to improve these timings.

## Local Observability Measurement

The collector has no supported disable switch. A pure on/off collector timer
would require changing product behavior, so the harness does not invent one.

For each measured `doctor` invocation, the collector's terminal event stores
the core command duration before collector I/O. The residual below is wall
time minus that stored integer duration.

| Corpus | Wall median | Wall p95 | Residual median | Residual p95 |
|---|---:|---:|---:|---:|
| small | 6.837 | 7.441 | 5.837 | 6.441 |
| medium | 9.543 | 9.893 | 8.543 | 8.893 |
| large | 25.444 | 28.395 | 24.444 | 27.395 |

This residual is an upper bound. It also contains process startup, JSON
serialization, and output. It must not be described as pure collector cost.
Every sample added exactly one valid local observability event.

## Correctness Proof

| Check | Result |
|---|---|
| Result series | 68 total: 53 supported, 15 unsupported |
| Measured supported samples | 1,060 |
| Sampling contract | every supported series has 3 warmups and 20 samples |
| Corpus parity | all three tag/current logical hashes match |
| Full traversal | exactly 100, 2,000, and 10,000 nodes every sample |
| Current page counts | exactly 1, 4, and 20 pages |
| First-page contract | correct count, `more_results`, and omitted bodies |
| Query recall | exact `Deploy release workflow` selected every sample |
| Tool list | exact 5, 25, and 100 tools every sample |
| Verify | clean before measurement and clean in all samples |
| Observability | one valid event added per measured collector sample |
| UI | all 60 measured responses are HTTP 200 on `127.0.0.1` |
| Export | all 60 ZIPs report durable publication and are non-empty |
| Evidence integrity | every entry in `SHA256SUMS` passes |
| P1/P2 | 0 / 0 |

## Exact Unsupported Tag Operations

- Node first page: tag node list is unbounded and has no page-size or cursor
  contract.
- Recall query: tag recall has no `--query` task-retrieval contract.
- Local Observability: tag has no collector or observability store.
- Desktop UI: tag has no `aopmem ui` command or local HTTP API.
- Debug capsule: tag has no `aopmem observe export` command.

These reasons are stored on every applicable raw result row.

## Evidence

- Runner: `scripts/benchmark_v020.sh`
- Harness: `scripts/benchmark_v020.py`
- Provenance: `.devplan/benchmarks/v020_rc1/run.json`
- Corpus manifests: `.devplan/benchmarks/v020_rc1/corpora/`
- Raw JSON: `.devplan/benchmarks/v020_rc1/raw/samples.json`
- Raw CSV: `.devplan/benchmarks/v020_rc1/raw/samples.csv`
- Summary CSV: `.devplan/benchmarks/v020_rc1/summary.csv`
- Integrity manifest: `.devplan/benchmarks/v020_rc1/SHA256SUMS`

Key evidence hashes:

| File | SHA-256 |
|---|---|
| `scripts/benchmark_v020.py` | `ed84b9294cb10f9fe8736bbe8c3890813bd911eb80499c325a0112d1a2ae3c44` |
| `scripts/benchmark_v020.sh` | `fca995149ead3c30a57aacddf097d5f8a7b104769f955acfe2897a966f21dcee` |
| `raw/samples.json` | `f2cb583eedc3f671636f53888f22cd887edea0c8dbb55a58050ac6b77c92fd26` |
| `raw/samples.csv` | `cfa45e357de023de0828e260620e9d6030fa83ba21df9a3e7c44abfeb255463c` |
| `summary.csv` | `610eb5eec34ced7a6e9b17cadc724c5eb366132862e59d8093b8f8a8cbeab9ab` |
| `SHA256SUMS` | `835fa55302a0f1f1ccccc12a28b9098a5314f4558d80410c7ee2723d606c52bf` |

Reproduction command:

```sh
scripts/benchmark_v020.sh
```

## Limits

- This is one Apple Silicon macOS host, not a cross-host benchmark lab.
- Windows native runtime performance was not measured on macOS.
- Synthetic data is deterministic but is not a production usage trace.
- Measurements are process-level wall time and include filesystem cache state.
- Full pagination intentionally measures repeated CLI invocations.
- New RC1 commands have no tag comparison and remain marked unsupported.
- The release scope defines no numeric latency threshold.

## Stage Decision

Stage 34 passes. The report contains real measured data, exact unsupported
markers, corpus parity proof, raw samples, and no invented score or percentage.
It does not by itself decide the full release-candidate verdict.

## Stage 35 release-input parity

The final release integration recomputed the harness source-tree digest from
`Cargo.toml`, `Cargo.lock`, `src/`, and `templates/`. It remains exactly
`91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4`,
the digest measured above.

Repeating the benchmark build command, `cargo build --release --locked`,
reproduced the measured candidate binary exactly at
`12ec578dc641373e0e22b67f548fb2862620571eb9777026304cd46e10427e61`.
The flat macOS release asset uses the same locked source and release profile,
plus explicit `--target aarch64-apple-darwin`, minimum macOS 11, and
`strip=false`; its SHA-256 is
`b32e918d2a44f0767444e09c84c1ed44fe9177709b2d56b2aa89c300081d4308`.

Therefore the benchmark covers the exact final production source and locked
dependencies, but its timing binary is not byte-identical to the flat
platform asset. Final-asset behavior has separate real fresh-install,
v0.1-update, doctor, verify, UI, and export proofs. No asset-specific speed
claim is made.
