# AOPMem v0.2.0-rc1 Global Audit Report

Date: 2026-07-15

Status: complete. Ready for macOS and Windows dogfood.

## Scope

This audit covers the classified mixed worktree, all 35 finite stages, the
target product contracts, final platform assets, the real isolated macOS
fresh/update flows, required negative tests, and the stop conditions.

No reset, checkout, push, tag, GitHub Release, real user-workspace install, or
backup deletion was performed.

## Implementation audit verdict

| Severity | Open findings |
|---|---:|
| P1 | 0 |
| P2 | 0 |
| P3 | 0 open |

The independent final audit confirms the implementation verdict.

## Definition-of-done audit

| Contract | Evidence | Result |
|---|---|---|
| Worktree classification | 434 changed hunks classified; mixed files reviewed; recovery ref retained | PASS |
| Safe optimization package | listed SQL, snapshot, transaction, validation, runner, and recall optimizations retained | PASS |
| Draft approval conflict | draft-only `+++`, `draft_review`, and managed-block sentence removed; five policy tests | PASS |
| Pagination | 100 default, 500 maximum, scoped keyset cursors, `--all`, explicit completeness | PASS |
| Recall | complete mandatory context, overflow ids, task query, graph/direct expansion, continuation dedup, reasons | PASS |
| Tool resources | per-tool limits, global ceilings, artifact streaming, inline errors, dry-run and approval policy | PASS |
| Reflection | one current inventory, append-only closed event set, separate proposals/receipts | PASS |
| Artifacts | 7 days or 1 GB, current-day oldest deletion, exact protected roots and cleanup reports | PASS |
| Audit snapshot | streaming SQL, atomic publish, real local Git, pending marker and duration observation | PASS |
| Local Observability | separate schema-v1 DB, 42 typed events, failure isolation, 30 days or 100 MB | PASS |
| Correlation/feedback | UUID v4 bundle id, continuation binding, observability-only feedback | PASS |
| Effectiveness | verifiable fact report, bounded top lists, no product score | PASS |
| Debug capsule | exact 12 entries, deterministic redaction, no DB/bodies/raw output/secrets | PASS |
| Desktop UI | loopback/token/GET-only, 11 APIs, bounded graph, six views, screenshots | PASS |
| Upgrade plan/apply | strict read-only plan; durable backups, guarded Online Backup, migrations, safe stop/recovery | PASS |
| Fresh/update installers | managed fresh adapter and healthy checks; update zero onboarding and safe publish order | PASS |
| Version/assets | v0.2.0-rc1; flat Mach-O arm64 and PE x64; exact SHA256SUMS | PASS |
| Benchmark | 100/2,000/10,000 nodes; 300/6,000/30,000 links; 3 warmups, 20 samples; raw data | PASS |
| Required local gates | fmt, clippy, build, 575 tests twice, dev_verify, diff, fixtures, installers, UI/export | PASS |

## Final gate evidence

| Proof | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo build --locked` | PASS |
| `cargo test --locked` | 575/575 PASS |
| `cargo test --tests --locked` | 575/575 PASS |
| `scripts/dev_verify.sh` | PASS, including its 575-test run |
| v0.1 schema/full upgrade fixtures | 1/1 + 1/1 PASS |
| installer audit | 11/11 groups PASS |
| real final macOS fresh | adapter in-sync, doctor healthy, verify clean |
| real peeled v0.1 macOS update | exact logical/tool/artifact preservation, migrations 001/002/003 |
| Windows PowerShell 5.1 | static contract PASS; no native execution claim on macOS |
| Windows binary | PE32+ x86-64; no dynamic MSVC/UCRT import |
| observability capsule | real 12-entry durable export and deterministic redaction PASS |
| UI | 13 HTTP tests, live token/method/bounds proof, three 1440x900 PNGs |
| forbidden drift | no extra platform/CI/Node/runtime dependency or Python cache |
| `git diff --check` | PASS |

## Data-preservation evidence

The isolated peeled-v0.1 fixture contains 11 nodes, one link, one alias, one
tag, one source, 12 events, one tool contract/tree, and three MCP profiles.
The exact selected v0.1-column digest before migration, after migration, and
inside SQLite Online Backup is:

`4890a73e51a5e0eeb0e283f3127cd5c05e583f13f518d7aefde95180c1ef7c9f`

Tool files retain digest
`5d7ffa2a4357d3072b406f154d17e479a4d8a6d227f37df9c678d97a0ad2babb`.
The artifact retains digest
`b7dfde292eca151e17b48bfa58f7fb397f7789614331d79e4239578aa6d75bad`.
Installer binary, database, adapter, and owned-asset backups remain present.

## Platform assets

| Asset | Type/proof | SHA-256 |
|---|---|---|
| `aopmem-darwin-arm64` | Mach-O arm64, min macOS 11.0, not stripped | `b32e918d2a44f0767444e09c84c1ed44fe9177709b2d56b2aa89c300081d4308` |
| `aopmem-windows-x86_64.exe` | PE32+ console x86-64, static CRT import scan | `a4e3302d6f26dd9d16387a075189fec51c469aef9b8d9c730f81001b21b2cf57` |

Native Windows execution cannot be performed on the macOS build host. The RC
is intended for that Windows dogfood validation; this report does not convert
static/PE evidence into a runtime claim.

## Benchmark provenance

The final source-tree digest remains the exact Stage 34 value
`91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4`.
The measured release build reproduces byte-for-byte at
`12ec578dc641373e0e22b67f548fb2862620571eb9777026304cd46e10427e61`.
The explicit-target flat macOS asset has a different binary hash, so no
asset-specific speed claim is made.

## Accepted residuals

- D-017: active same-UID tampering outside the no-sandbox local boundary.
- D-021: one rare nonfatal tiny_http worker panic under aggressive local RST
  stress; server stayed alive and token remained private.
- D-022: two documented bounded-API representation semantics.
- Native Windows behavior is a dogfood target, not a completed macOS-hosted
  runtime proof.
- Stage 34 measures the exact production source/lock candidate, not the
  byte-identical explicit-target flat asset.

None of these documented boundaries is an open release finding. They do not
change stored user data or expand product scope.

## Independent final audit

Final verdict: **READY for macOS and Windows dogfood**.

| Severity | Open findings |
|---|---:|
| P1 | 0 |
| P2 | 0 |
| P3 | 0 |

The independent read-only audit rechecked the exact source digest, fmt,
clippy, 575 tests, 11 installer groups, manifest and platform types, Windows
imports, dependency coverage, forbidden scope, required paths, JSON ledger,
and final diff. It found one generated Python-cache P3 drift. Stage 35 deleted
that cache and the repeat scan passed.

Production/input freeze marker:

`source_tree_sha256=91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4`
