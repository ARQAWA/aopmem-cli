# Stages 26–30 cumulative audit

Date: 2026-07-15

Scope:

- Stage 26: observe status and effectiveness report;
- Stage 27: deterministic debug capsule export;
- Stage 28: loopback/token UI server;
- Stage 29: bounded read-only UI APIs;
- Stage 30: embedded desktop frontend, docs, and screenshot proof.

## Verdict

| Priority | Initial groups | Fixed | Remaining |
|---|---:|---:|---:|
| P1 | 0 | 0 | 0 |
| P2 | 9 | 9 | 0 |
| P3 | 4 | 0 | 4 accepted |

The nine P2 groups comprise two Stage 26 report defects and seven Stage 30
frontend/proof groups. If the two missing recall metrics in Stage 30 are
counted separately, the raw defect count is ten. Grouping does not change the
final verdict.

## Remediation proof

- Stage 26 now derives recall facts from in-period lifecycle timestamps,
  reports terminal continuation state, and exposes failed adapter drift.
- Stage 30 reports the full effectiveness retention reason, both continuation
  facts, correct partial/error Tools/MCP state, and a concise live region.
- The table header ordering defect was removed.
- All three screenshot files were converted from browser-returned JPEG bytes
  to real RGB PNG files. Each is exactly `1440x900`.
- The frontend uses text-only DOM insertion, strict same-origin token-relative
  GET requests, no external assets, no write route, and no tool execution.
- Graph rendering deduplicates the fixed center context and remains bounded by
  200 unique nodes and 500 edges.
- Repeated browser captures were byte-identical for Overview, Graph, and
  Activity.
- Before/after browser proof preserved the operational and observability main
  database bytes, sizes, mtimes, schemas, and row counts. Normal SQLite
  read-only WAL coordination may create or touch the exact `-wal`/`-shm`
  sidecars; this is documented and is not an operational-memory mutation.

## Checks

- `node --check` / JavaScriptCore syntax check: PASS.
- final embedded asset tests: 7/7 PASS.
- Stage 30 scoped UI tests: 24/24 PASS before the final CSS-only fix.
- cumulative full suite checkpoint: 561/561 PASS.
- `cargo fmt --check`: PASS at the stable checkpoint.
- `cargo clippy --all-targets -- -D warnings`: PASS at the stable checkpoint.
- `cargo build`: PASS at the stable checkpoint.
- `scripts/dev_verify.sh`: PASS at the stable checkpoint.
- `git diff --check`: PASS.
- independent static re-audit: P1=0, P2=0.

## Screenshot hashes

- Overview:
  `8272778ccea477fded9586fa2498422b35684b6e6f7ac2b94f544ad79f4394bf`
- Graph:
  `4e82b570306d7b627948ed87c6e43085a2827493605cf59d91c5aa08535d76b3`
- Activity:
  `b3b465c72cba7abe9f4a5bed12e42d579bba3d7af9b5daaace4c607621d2a0ea`

## Accepted P3 items

- D-017: same-UID local export leaf-name race boundary.
- D-021: rare nonfatal `tiny_http` valid-request RST worker panic.
- D-022: empty Memory body-omission flag and duplicated graph center API
  semantics. The frontend deduplicates the latter.

Final cumulative verdict: **P1=0, P2=0**.
