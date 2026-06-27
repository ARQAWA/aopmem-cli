# BLOCKER — STAGE_018

Status: `RESOLVED`

Date: `2026-06-07`

Requirement:

- `REQ-MEM-002`

Reason:

- Stage 018 requires graph traversal in recall.
- Real traversal needs both selected nodes and links.
- Current `aopmem recall` wiring calls `storage::list_nodes` and then
  `recall::build_structured_bundle(nodes)`.
- Links are available through `storage::list_links`, but they are not passed
  into `src/recall/**`.
- Stage 018 product scope allows only `src/recall/**` and `tests/cli/**`.
- The user also forbids changing `src/cli/**`, `src/storage/**`, and
  `src/schema/**`.

Decision:

- User explicitly allowed the minimal `src/cli/**` recall wiring patch.
- `aopmem recall` now passes links from existing `storage::list_links` into
  recall.
- Stage 018 is unblocked and implemented as `DONE`.
- Stage 018 is not `VERIFIED`; audit is still required before Stage 019 work.

Resolution:

- Closed on `2026-06-07`.
- Product files changed only within allowed patch scope:
  `src/recall/mod.rs` and `src/cli/mod.rs`.

# BLOCKER — STAGE_019

Status: `RESOLVED`

Date: `2026-06-07`

Requirement:

- `REQ-STORAGE-003`
- `REQ-MEM-002`

Reason:

- Stage 019 requires FTS5/BM25 fallback results ordered by `bm25`
  ascending.
- `src/cli/mod.rs::run_recall` currently calls recall with nodes and links
  only.
- `src/recall/**` has no SQLite connection, no recall query input, no FTS
  result rows, and no public storage FTS search API.
- Existing FTS helpers in `src/storage/**` are private.
- The allowed product scope for Stage 019 is only `src/recall/**` and
  `tests/cli/**`.
- The user forbids editing `src/cli/**`, `src/storage/**`, or `src/schema/**`
  unless Stage 019 is impossible in allowed scope.

Decision:

- User explicitly allowed minimal `src/storage/**` and `src/cli/**`
  dependency scope for this patch.
- Added a public FTS/BM25 storage search API.
- Wired `aopmem recall` to add fallback results when structured recall is
  insufficient.
- Stage 019 is implemented as `DONE`.
- Stage 019 is not `VERIFIED`; audit is still required before Stage 020.

Resolution:

- Closed on `2026-06-07`.
- Product files changed only within approved patch scope:
  `src/recall/mod.rs`, `src/storage/mod.rs`, and `src/cli/mod.rs`.
