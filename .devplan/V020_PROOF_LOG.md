# AOPMem v0.2.0-rc1 Proof Log

Append-only proof for the finite v0.2.0-rc1 goal.

## Stage 01 — recovery and classification

Status: `PASS_AFTER_REMEDIATION`

Protected objects:

- `refs/aopmem-recovery/v020-current-mixed-20260714` → commit
  `4ec96ba2f2d1de0d226e6234fce0395f34c82f5c` → tree `7c6bf85e...`.
- `refs/aopmem-recovery/v020-archive-incomplete-20260714` → commit
  `1f26f24551114dca308ee11348f87014cc6793dd` → tree `cdad5a9b...`.

Baseline release assets:

- macOS SHA-256: `d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607`.
- Windows SHA-256: `01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a`.
- macOS type: Mach-O 64-bit arm64.
- Windows type: PE32+ console x86-64.
- Binary version: `aopmem 0.1.0`.

AUTO_PATCH_WINDOW:

- Stage subagent protected refs but stalled on the 434-row document.
- Root generated the mechanical hunk ledger through `apply_patch`.
- No source checkpoint changes materialized yet.

Checks:

- `PASS` protected refs resolve to the exact two trees.
- `PASS` classification contains 434 hunk rows.
- `PASS` no hunk row has `UNKNOWN_BLOCKER`.
- `PASS` execution ledger contains 35 stages and valid JSON.
- `PASS` `git diff --check`.

Handoff:

- Stage 02 may materialize `7c6bf85e...` without reset or checkout.
- Preserve both recovery refs through final handoff.

## Stage 02 — materialize classified checkpoint

Status: `PASS`

Materialization:

- Ran binary-safe patch preflight from baseline `v0.1.0-rc3` to recovery
  commit `4ec96ba2f2d1de0d226e6234fce0395f34c82f5c`.
- Applied that patch without reset, checkout, branch switch, staging, tag,
  push, or commit.
- Restored the 16 checkpoint files only. Left all five `V020_*` planning
  files present.
- Kept checkpoint content exact. In particular, deferred the draft approval
  conflict to Stage 04 as planned.

Checks:

- `PASS` `git apply --check` before materialization.
- `PASS` 16/16 working file blob IDs equal the recovery commit blob IDs.
- `PASS` five `V020_*` planning files remain present.
- `PASS` `cargo fmt --check`.
- `PASS` `cargo test`: 225 passed, 0 failed.
- `PASS` `git diff --check`.

Notes:

- `git apply` reported pre-existing trailing whitespace in restored audit log
  files. Those files match the protected checkpoint byte-for-byte; Stage 02
  intentionally made no content cleanup.
- One initial blob-check shell loop used the reserved zsh variable `path` and
  invalidated its own command lookup. Discarded that result. Re-ran with a
  safe variable name; the recorded 16/16 result is valid.

Handoff:

- Stage 03 may implement Windows legacy workspace compatibility.
- Preserve recovery refs and avoid whole-file rollback in mixed files.

## Stage 03 — Windows legacy workspace compatibility

Status: `PASS`

Changes:

- Added the exact v0.1 path-text key algorithm beside the normalized v0.2
  algorithm.
- Added a read-only resolver for current and legacy workspace roots.
- Selected the only root with persistent data; ignored empty directory
  skeletons; selected the current key for a new workspace.
- Returned `WORKSPACE_RESOLVE_ERROR` when both roots contain data.
- Wired install, adapter, read/write CLI contexts, doctor, and verify through
  the same resolver.
- Performed no rename, delete, DB open, or directory creation during resolve.

AUTO_PATCH_WINDOW:

- Stage subagent implemented the storage resolver but stalled before caller
  wiring. Root completed the existing Stage 03 wiring and tests without
  changing the approved contract.

Checks:

- `PASS` legacy/current Windows key regression.
- `PASS` legacy-only, current-only, empty-vs-data, and two-data collision.
- `PASS` resolver no-write proof.
- `PASS` `cargo fmt --check`.
- `PASS` focused resolver tests: 4 passed.
- `PASS` `cargo test`: 229 passed, 0 failed.
- `PASS` `git diff --check`.

Handoff:

- Stage 04 may remove the draft-only approval conflict.
- Upgrade must later enumerate every workspace directory and preserve both
  roots when collision is reported.

## Stage 04 — draft approval correction

Status: `PASS`

Changes:

- Removed draft status from the tool approval decision.
- Removed synthetic `draft_review` approval requirement.
- Restored approval for explicit contract requirements, `external_write`, and
  `destructive` tools only.
- Kept safe drafts and `external_read` drafts with
  `approval_requirement=none` runnable without `+++`.
- Removed the draft-only approval sentence from the managed adapter block and
  its canonical template.
- Updated the CLI error hint and safe-draft end-to-end test.

Checks:

- `PASS` forbidden-string scan for `draft_review`, the removed managed-block
  sentence, and draft-only approval wording.
- `PASS` five required policy cases: safe draft without approval,
  `external_read` without approval, blocked `external_write`, approved
  `external_write`, and dry-run without execution.
- `PASS` focused tool, CLI, adapter, and dry-run tests: 16 passed.
- `PASS` `cargo test`: 228 passed, 0 failed.
- `PASS` `cargo fmt --check`.
- `PASS` `git diff --check`.

Handoff:

- Stage 05 may retain pending-only migrations, read-only DB access, summary
  indexes, targeted metadata SQL, and transactional storage optimizations.
- Do not reintroduce approval based only on draft status.

## Stage 05 — safe storage optimizations

Status: `PASS`

Changes:

- Moved schema marker creation, applied-version validation, pending migration
  SQL, marker inserts, and commit into one `IMMEDIATE` transaction.
- Rejected unknown migration versions and known versions with mismatched names.
  Removed conflict-masking marker insertion while preserving pending-only and
  idempotent migration behavior.
- Preserved the protected recovery-checkpoint bytes of migration `001` and the
  migration `002` node-summary index.
- Validated teach proposals and node metadata before any dependent database
  lookup.
- Required each tool directory to be a real canonical immediate child of the
  workspace tools root. Rejected linked roots and executable path escapes.
- Kept direct rule nodes, as well as direct tools, visible in serialized compact
  recall output.
- Proved an existing database opened read-only rejects inserts without changing
  its data.

Tests added:

- Migration: v0.1 fixture, unknown version, wrong known name, full rollback,
  and existing idempotency coverage.
- Validation: invalid teach proposal and invalid node metadata on an empty
  schema.
- Tool containment: linked executable escape and linked tool-root escape on
  Unix, plus existing normal-path acceptance.
- Recall: serialized compact output contains a directly selected rule.
- Read-only storage: insert rejection on an existing database.

Checks:

- `PASS` migration `001` bytes equal the protected recovery checkpoint.
- `PASS` focused Stage 05 tests: 14 passed across six focused invocations.
- `PASS` `cargo test`: 238 passed, 0 failed.
- `PASS` `cargo fmt --check`.
- `PASS` `cargo clippy --all-targets -- -D warnings`.
- `PASS` `git diff --check`.

Risk boundary:

- Unix symlink behavior has executable tests. Windows junction/reparse-point
  rejection is implemented with native file attributes; Windows binary proof
  remains in the release stages.

Handoff:

- Run the cumulative audit for Stages 01-05.
- Stage 06 may coordinate teach and reflection mutations transactionally.
- Keep Stage 05 validation, containment, and migration failure contracts intact.

## Cumulative audit 01–05 — first pass

Status: `FAIL_REMEDIATION_IN_PROGRESS`

Independent agent: `audit_01_05`

Findings:

- `P1`: replacing `workspace/tools` with a symlink allowed draft staging and
  publication outside the workspace. The audit reproduced an escaped
  `tool.json` write.
- `P2`: invalid node input returned `VALIDATION_ERROR` only after creating the
  workspace database. Equivalent mutation handlers require a pure CLI
  preflight before workspace or DB access.
- `P2`: the `external_write` approval tests also used
  `approval_requirement=manual_review`, so they did not isolate the side-effect
  policy branch.

Confirmed controls:

- Protected recovery refs and all 434 hunk classifications are exact.
- Historical user files remain byte-for-byte unchanged.
- Windows legacy/current workspace resolution and collision blocking passed.
- Draft-only approval and `draft_review` remain absent.
- Transactional migration, rollback, direct-rule recall JSON, read-only DB,
  build, 238 tests, format, clippy, and diff checks passed.

AUTO_PATCH_WINDOW:

- The three bounded audit fixes are assigned to Stage 06 because it already
  rewires draft creation and all mutation entry points.
- Stage 05 is not accepted until focused regression tests and independent
  re-audit close `P1=0` and `P2=0`.

## Stage 06 — transactional mutation coordinator

Status: `PASS`

Changes:

- Added one per-workspace mutation coordinator and process lock.
- Created the durable pending snapshot marker before DB open, migration, and
  operation execution.
- Applied pending migrations and the requested operation in one
  `BEGIN IMMEDIATE` transaction with explicit rollback ownership.
- Kept a pre-existing marker unchanged; removed a newly-owned marker only
  after a proven no-commit rollback.
- Returned committed commands successfully with structured warning
  `AUDIT_SNAPSHOT_PENDING` when snapshot publication failed.
- Wired all 17 production operational-memory writers, including `init` and
  atomic draft tool creation, through the coordinator.
- Added pure preflight validation before workspace or database creation for
  node, update, link, alias, tag, source, MCP, and draft inputs.
- Rejected a symlink/reparse `workspace/tools` root before any staging write.
- Extended rollback effects so failed install setup removes only newly-owned
  `.understand.docs` paths and restores `.git/info/exclude` byte-for-byte.
- Preserved every pre-existing docs file, directory, and binary sentinel.

Checks:

- `PASS` coordinator focused tests: 6.
- `PASS` install repository-side rollback focused tests: 4.
- `PASS` tools-root escape and approval policy focused tests.
- `PASS` `cargo build`.
- `PASS` `cargo test`: 253 passed, 0 failed.
- `PASS` `cargo fmt --check`.
- `PASS` `cargo clippy --all-targets -- -D warnings`.
- `PASS` `git diff --check`.

## Cumulative audit 01–05 — remediation re-audit

Status: `PASS`

Independent agent: `audit_01_05`

Runtime and mutation proof:

- Tools-root symlink repro returned a validation error; outside writes `0`;
  registry rows `0`.
- Seven invalid mutation commands left `AOPMEM_HOME` absent.
- Deleting each of the `external_write`, explicit approval, and destructive
  policy branches caused an independent test failure.
- DB open and `BEGIN` failure marker ownership passed.
- Forced install seed failure left DB partial rows `0`, marker absent, fresh
  docs absent, and the original exclude bytes exact.
- Pre-existing docs tree, binary sentinels, and exclude bytes remained exact.
- Draft filesystem and SQLite rollback stayed consistent.

Final severity gate: `P1=0`, `P2=0`.

Windows native reparse execution remains a required release-stage proof; the
compiled Windows branch is present and its static audit passed.

## Stage 07 — streaming audit snapshot

Status: `PASS`

Changes:

- Streamed SQL `TEXT` and `BLOB` values in chunks no larger than 8192 bytes.
- Encoded invalid UTF-8 and NUL-bearing SQLite text losslessly as
  `CAST(X'...' AS TEXT)`.
- Restored `sqlite_sequence` after canonical rows so deleted high IDs cannot be
  reused after snapshot recovery.
- Excluded FTS shadow tables and rebuilt FTS only from canonical `nodes` and
  deterministically ordered `aliases`.
- Corrected runtime alias aggregation to order aggregate input by alias ID.
- Kept unchanged-state dumps byte-identical.
- Added `duration_ms` and `bytes_written` to the snapshot report and retained
  the successful report in the mutation outcome for later observability.
- Kept the known-good snapshot and pending marker on writer or publish failure;
  removed temporary files.

Proof:

- `PASS` restore of all operational tables, row data, integrity, foreign keys,
  and canonical FTS behavior.
- `PASS` next AUTOINCREMENT ID after restore does not reuse a deleted ID.
- `PASS` UTF-8 quotes, NUL text, invalid UTF-8 text, blobs, and real values
  round-trip with exact SQLite storage classes.
- `PASS` 1 MiB body never produces a writer call above 8192 bytes.
- `PASS` writer and publish failure injection preserves the old snapshot.
- `PASS` focused audit tests: 17; storage: 1; mutation: 6.
- `PASS` root repeated six critical regression tests.
- `PASS` `cargo test`: 261 passed, 0 failed.
- `PASS` format, clippy with denied warnings, and diff checks.

Handoff:

- Stage 08 owns Windows `MoveFileExW`, directory durability, snapshot locking,
  and LocalGitAudit without a runtime `git` subprocess.

## Stage 09 — node pagination

Status: `PASS`

Changes:

- Added versioned node cursor `v1.node.all.<lowercase-hex UTF-8 decimal id>`
  with a 1024-byte maximum and canonical positive-ID validation.
- Rejected malformed, wrong-kind, wrong-scope, uppercase, non-UTF-8, and
  non-canonical cursors before workspace or DB access with `INVALID_CURSOR`.
- Added `node list --cursor` and `--all`; retained hidden legacy
  `--after-id` input with strict conflicts.
- Kept the default page size at 100, maximum at 500, keyset order at `id ASC`,
  and always returned `more_results` plus nullable `next_cursor`.
- Removed the `body` JSON key entirely from default node-list items.
- Returned the complete body with `--include-body`; removed the former 64 KiB
  list truncation; kept `node get` complete.
- Traversed `--all` in one deferred read transaction and failed closed on
  duplicate, non-progressing, or inconsistent pages without partial JSON.

Proof:

- `PASS` exact cursor round-trip and strict negative cases.
- `PASS` invalid cursor leaves missing workspace/home untouched.
- `PASS` empty, final, and three-page keyset completeness behavior.
- `PASS` default body-key absence and full body above 64 KiB.
- `PASS` multi-page `--all` on production read-only connection.
- `PASS` duplicate/non-progress `PAGINATION_ERROR` injection.
- `PASS` root repeated seven critical pagination tests.
- `PASS` `cargo test`: 274 passed, 0 failed.
- `PASS` format, clippy with denied warnings, and diff checks.

Handoff:

- Stage 10 applies the same cursor and `--all` contract to link, alias, tag,
  source, tool, and MCP lists and updates Memory Keeper traversal rules.

## Stage 10 — all list pagination and Memory Keeper traversal

Status: `PASS`

Changes:

- Generalized cursors to
  `v1.<node|link|alias|tag|source|tool|mcp>.<scope>.<hex UTF-8 key>`.
- Bound metadata cursors to `all` or the exact `node-<id>` filter and rejected
  cross-kind/cross-scope reuse before DB access.
- Added Unicode-safe tool and MCP cursor round-trips.
- Added `--cursor` and `--all` to link, alias, tag, source, tool, and MCP list
  commands, with hidden legacy inputs and strict argument conflicts.
- Kept stable 100/500 keyset pages and removed public `next_after_id`; every
  list JSON now exposes `more_results` and nullable `next_cursor`.
- Ran controlled full traversal inside one read transaction and failed closed
  on duplicate, non-progressing, or inconsistent pages.
- Updated Memory Keeper, the embedded managed block, and its canonical
  template: full-set retrieval must follow `next_cursor` until
  `more_results=false` and must not assume the first or short page is complete.

Proof:

- `PASS` cross-kind and cross-filter cursors fail before workspace creation.
- `PASS` all seven public list JSON models declare completeness explicitly.
- `PASS` Unicode tool/MCP cursors and four-page generic traversal.
- `PASS` duplicate string-key traversal fails without partial success.
- `PASS` embedded/canonical managed block and Memory Keeper contract tests.
- `PASS` focused Stage 10 tests: 13; root repeated the same 13.
- `PASS` `cargo test`: 282 passed, 0 failed.
- `PASS` format, clippy with denied warnings, and diff checks.

Handoff:

- Run the cumulative audit for Stages 06–10 after Stage 08 completes.

## Stage 08 — cross-platform audit publish and LocalGitAudit

Status: `PASS`

Changes:

- Published snapshots atomically with Unix rename plus parent sync and Windows
  `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`.
- Added a permanent `.snapshot.lock` and serialized mutation, marker, SQL dump,
  atomic publish, and Git commit without deleting the lock inode.
- Replaced the runtime `git` subprocess with `gix`; the audit repository now
  creates real commits with a fixed local author and message, preserves other
  HEAD entries and the index, and skips no-op commits.
- Kept `memory.sql` streaming into the object store and left a pending marker
  plus structured success warning after post-DB snapshot or Git failure.
- Added fail-closed containment before DB access for workspace roots, SQLite
  DB and WAL/SHM/journal sidecars, audit paths, managed lock/marker/snapshot
  files, and nested Git metadata. Unix links and Windows reparse points are
  rejected; SQLite opens use `SQLITE_OPEN_NOFOLLOW` on a checked canonical
  parent path.
- Applied the same read guard to CLI reads, doctor, verify, and lint.

Proof:

- `PASS` audit tests: 22; mutation tests: 14; read-only storage tests: 3;
  verify tests: 10.
- `PASS` full `cargo test`: 292 passed, 0 failed.
- `PASS` build, check, format, clippy with denied warnings, and diff checks.
- `PASS` LocalGitAudit author/message/tree/index/no-op/corrupt-HEAD proofs.
- `PASS` rebuilt-binary negative cases for workspace, DB, WAL sidecar,
  audit root, nested `.git/objects`, locks, marker, and snapshot paths; DB and
  external sentinels remained byte-exact.
- `PASS` pinned `cargo-xwin` check for `x86_64-pc-windows-msvc` on the stable
  source, exit 0.

## Cumulative audit — Stages 06–10

Status: `PASS_AFTER_REMEDIATION`

- Initial audit reproduced one systemic P1 path-containment defect across
  persistent workspace paths. No file-wide rollback was used.
- The remediation was limited to cross-layer path validation and exact
  negative tests under the approved AUTO_PATCH window.
- Independent audit rebuilt the binary and repeated temporary CLI proofs.
- Final severity counts: `P1=0`, `P2=0`.
- All seven list commands, keyset continuation, `--all`, explicit completeness,
  node body behavior, Memory Keeper traversal, snapshot pending behavior, and
  LocalGitAudit invariants passed.

Handoff:

- Stage 11 starts the finite five-stage final recall contract window.

## Stage 11 — recall contract model

Status: `PASS`

Changes:

- Added additive typed v0.2 request/response, mandatory/task sections, typed
  selection reasons, explicit completeness, and budget metadata.
- Fixed the budget unit to compact canonical JSON UTF-8 bytes, with a 256 KiB
  task soft budget and 1 MiB mandatory hard budget.
- Fixed mandatory types to active kernel contracts, gates, project profiles,
  sources, and rules.
- Added canonical lowercase UUID v4 bundle IDs.
- Added strict parse surfaces for `--full` and query-bound continuation
  cursors. Until wired by later recall stages, both fail explicitly with
  `NOT_IMPLEMENTED` before workspace or DB access instead of being ignored.

Proof:

- `PASS` eight new model and CLI tests, including exact JSON shape, full body
  byte accounting, checked overflow, UUID form, cursor cap/conflicts, and no
  AOPMEM_HOME creation on invalid/unwired input.
- `PASS` root focused recall run: 31 tests.
- `PASS` full `cargo test`: 300 passed, 0 failed.
- `PASS` format, clippy with denied warnings, and diff checks.

Handoff:

- Stage 12 loads complete active mandatory context and implements fail-closed
  `MANDATORY_CONTEXT_OVERFLOW`.

## Stage 12 — mandatory recall and overflow

Status: `PASS`

Changes:

- Added a targeted, stable, read-only query for every active mandatory node of
  the five frozen types, without `LIMIT` and with complete bodies.
- Added an O(n) exact canonical JSON byte counter for the complete mandatory
  section. Stable order is type rank followed by immutable node id.
- Added fail-closed `MANDATORY_CONTEXT_OVERFLOW`; its bounded error payload
  contains stable offending ids, `data=null`, and no node bodies, bundle, or
  partial success.
- Added a UUID v4 bundle id, complete mandatory section, and exact budget
  metadata to both successful bare and query recall paths.

Proof:

- `PASS` mandatory gate/profile retention, full body, inactive exclusion,
  stable order, exact-limit boundary, overflow tail, loader, normal JSON, and
  no-partial-success JSON tests: 9.
- `PASS` root repeated all nine focused tests.
- `PASS` full `cargo test`: 308 passed, 0 failed.
- `PASS` format, clippy with denied warnings, and diff checks.

Handoff:

- Stage 13 replaces the temporary bounded query path with typed roots, FTS5
  BM25, and direct-link selection while retaining mandatory context unchanged.

## Stage 13 — query roots, FTS5/BM25, and direct links

Status: `PASS`

Changes:

- Added bounded exact typed-root lookup over title, aliases, and tags with
  stable workflow/tool/failure/correction/rule/lesson/skill priority.
- Added real FTS5 `bm25` retrieval with full node bodies and stable rank/id
  order, plus one batched outgoing-link query with full target nodes.
- Excluded deprecated, superseded, broken, and already-complete active
  mandatory nodes before each SQL `LIMIT`, preventing candidate starvation.
- Merged first-pass reasons by node id and packed only complete nodes within
  the 256 KiB task budget. No body or node is silently truncated.
- Replaced query-mode legacy JSON with the typed v0.2 response, explicit
  completeness, per-node reasons, exact budget, and an explicit null cursor
  pending Stage 15. Blank queries fail before workspace access.

AUTO_PATCH:

- Added migration `003_task_recall_exact_indexes` with only three required
  `NOCASE` indexes for nodes.title, aliases.alias, and tags.tag.
- Updated the latest verify marker and added v0.1 pending-migration,
  idempotence, rollback, and query-plan proof. No other schema change was made.

Proof:

- `PASS` eleven new tests; query-plan proof names all three exact indexes.
- `PASS` mandatory-starvation, alias/tag, BM25, old direct link, status filter,
  candidate cap, whole-node pack, reason JSON, and no-home blank-query cases.
- `PASS` root focused task recall: 7; typed query response: 1.
- `PASS` full `cargo test`: 319 passed, 0 failed.
- `PASS` format, clippy with denied warnings, and diff checks.

Handoff:

- Stage 14 adds bounded graph expansion, final source/trust/confidence ordering,
  and global reason-preserving deduplication.

## Stage 14 — graph expansion, ordering, and deduplication

Status: `PASS`

Changes:

- Added one recursive SQLite CTE with an internal `LIMIT + 1`, a cycle path
  guard, and a maximum total depth of two.
- Preserved root/depth/link/id order and complete bodies while filtering
  inactive and already-mandatory targets before the traversal cap.
- Added typed graph and workflow/tool/failure-mode expansion reasons.
- Merged every route to the same node, removed semantically duplicate reasons,
  and sorted reasons deterministically.
- Applied final task ordering by retrieval tier, source hierarchy, trust,
  confidence, and id, then performed whole-node budget packing.
- Removed an O(r²) root type lookup in favor of one bounded O(r) pass.

Proof:

- `PASS` depth-two old linked rule, cycle, status/mandatory pre-cap filter,
  extra-row probe, global node/reason dedup, three expansion types,
  source/trust/confidence ordering, and deterministic explained output tests.
- `PASS` root focused task recall: 15.
- `PASS` full `cargo test`: 327 passed, 0 failed.
- `PASS` format, clippy with denied warnings, and diff checks.

Handoff:

- Stage 15 finalizes query-bound continuation, cumulative budget behavior,
  debug-only full recall, and Memory Keeper automatic continuation rules.

## Stage 15 — continuation, correlation, and full recall

Status: `PASS`

Changes:

- Added exact query-bound continuation with one UUID v4 `bundle_id`, stable
  cross-page ordering, exact deduplication, and full mandatory context on every
  page.
- Added cumulative canonical-JSON task budget metadata with used, remaining,
  and exhausted state. `more_results=true` always carries a cursor; final
  retrieval returns `false` and a null cursor.
- Added a canonical binary/base64url cursor with checksum and a 24 KiB hard
  cap. It contains only ids, typed phase, counters, query/database
  fingerprints, and bundle identity; it contains no query, titles, bodies,
  environment values, or secrets.
- Added streaming operational-memory revision proof. A memory mutation returns
  `STALE_RECALL_CURSOR` instead of mixing pages from different revisions.
- Added debug-only, read-only `recall --full` with complete operational nodes,
  links, aliases, tags, sources, events, tool contracts, and MCP profiles.
- Updated the managed block and Memory Keeper contract to continue normal
  query recall until retrieval completion or cumulative budget exhaustion and
  never use `--full` for normal task work.

Proof:

- `PASS` twelve new tests for three-page continuation, same bundle, exact
  deduplication, cumulative budget, exhausted and stale cursors, wrong query,
  tamper/noncanonical wire data, SQL/Rust order parity, full read-only recall,
  template sync, and large bounded retrieval.
- `PASS` Windows-safe cursor stress: 1,600 seen ids plus roots at the complete
  task budget encoded below 12 KiB, under the 24 KiB command-line cap.
- `PASS` full `cargo test`: 339 passed, 0 failed.
- `PASS` full `cargo test --tests`: 339 passed, 0 failed.
- `PASS` format, clippy with denied warnings, build, diff, and forbidden-drift
  checks.

Handoff:

- Cumulative audit covers Stages 11–15 before Stage 16 changes the tool
  resource contract.

## Cumulative audit after Stage 15 — initial finding

Status: `REMEDIATION_IN_PROGRESS`

- Independent audit found one P2 relevance gap: continuation FTS calculated
  BM25 rank but omitted it from the stable FTS-tier order. With enough weak
  same-metadata matches, the cumulative task budget could be exhausted before
  a stronger workflow, tool, or failure mode was emitted.
- Remediation is limited to the FTS tier: preserve source, trust, and
  confidence ordering, then apply BM25 rank before id in both Rust one-shot
  selection and continuation SQL.
- Required proof adds a budget-starvation regression, SQL/Rust order parity,
  and an explicit concurrent read-snapshot regression. No Stage 16 work starts
  until independent re-audit returns final P1=0 and P2=0.

## Cumulative audit after Stage 15 — final

Status: `PASS_AFTER_REMEDIATION`

Remediation:

- Added the frozen FTS-tier order in both Rust and continuation SQL:
  source hierarchy, trust, confidence, BM25 ascending, then id. BM25 affects
  only FTS-tier candidates.
- Added a regression with 64 weak large matches and one later strong workflow.
  The strong workflow is emitted before the cumulative 256 KiB budget is
  exhausted.
- Added SQL/Rust paged-order parity and an explicit WAL concurrency test. The
  read transaction keeps one snapshot while a separate writer commits.

Proof:

- `PASS` full `cargo test`: 343 passed, 0 failed.
- `PASS` full `cargo test --tests`: 343 passed, 0 failed.
- `PASS` format, clippy with denied warnings, build, and diff checks.
- `PASS` independent re-audit focused BM25, parity, starvation, snapshot, and
  diff checks.
- Final audit counts: P1 = 0; P2 = 0.

Handoff:

- Stage 16 may now extend the persisted tool runtime contract without changing
  recall behavior.

## Stage 16 — persisted tool resource contract

Status: `PASS`

Changes:

- Extended `tool.json` runtime metadata with `timeout_ms`, separate stdout and
  stderr byte limits, `supports_dry_run`, and typed `inline|artifact` output
  mode.
- Added exact defaults of 30,000 ms and 65,536 bytes per stream, plus contract
  ceilings of 900,000 ms and 10,485,760 bytes per stream. Zero and values over
  the ceiling fail validation.
- Added serde defaults for legacy v0.1 file and SQLite JSON. Reading old
  contracts applies effective defaults without rewriting data or reporting
  false drift.
- Added `tool create-draft` runtime override flags. Invalid values fail before
  workspace or DB access; valid values are identical in SQLite and `tool.json`.
- Tool validation now reports the effective runtime contract. Managed template
  and embedded adapter block remain byte-identical and the approval policy was
  not changed.
- Kept the production process runner at its prior fixed 30 s / 64 KiB behavior
  for Stage 17, preventing an accidental runtime expansion in this schema-only
  stage.

Proof:

- `PASS` legacy file/SQLite defaults and no-drift, explicit serialization,
  custom round-trip, exact ceilings, zero and ceiling+1 rejection, unknown
  output mode, invalid CLI no-home, SQLite/file parity, validation output, and
  runner non-expansion tests.
- `PASS` full `cargo test`: 349 passed, 0 failed.
- `PASS` full `cargo test --tests`: 349 passed, 0 failed.
- `PASS` format, clippy with denied warnings, build, and diff checks.

Handoff:

- Stage 17 applies persisted inline limits to process execution, structured
  timeout/overflow errors, and cross-platform process-tree termination.

## Stage 17 — persisted inline runner and process-tree limits

Status: `PASS`

Changes:

- Production tool runs now derive timeout and independent stdout/stderr limits
  from the validated persisted runtime contract while preserving v0.1 defaults.
- Added concurrent bounded pipe readers with immediate overflow notification,
  discard-only draining after the limit, fail-fast reader errors, and bounded
  cleanup. No complete oversized stream is retained in RAM.
- Timeout, output overflow, early parent exit, and cleanup failures terminate
  the complete isolated process tree. Unix uses a process group. Windows starts
  suspended, assigns the child to a kill-on-close Job Object, then resumes it,
  closing the spawn-to-assignment escape race.
- Added exact `TOOL_TIMEOUT` and `TOOL_OUTPUT_OVERFLOW` JSON errors with typed
  numeric limits and truncation flags. Error envelopes contain no raw output.
- Dry-run remains a pure execution plan and never spawns implementation code.

Proof:

- `PASS` twelve new regressions for persisted timeout, independent stream
  limits, legacy defaults, exact ceilings, pre-spawn invalid limits, timeout and
  both-stream overflow descendant termination, concurrent streams, inherited
  pipe closure, dry-run, and exact JSON errors.
- `PASS` full `cargo test`: 361 passed, 0 failed.
- `PASS` full `cargo test --tests`: 361 passed, 0 failed.
- `PASS` format, clippy with denied warnings, build, and diff checks.
- `PASS` pinned Windows MSVC cross-check before the final suspended-spawn race
  remediation. The post-remediation retry was bounded and stopped when the
  external MSVC CRT download made no progress; exact post-fix Windows compile
  proof remains assigned to the mandatory Stage 35 release build.

Handoff:

- Stage 18 adds streaming artifact output and atomic publication without
  changing the approval or dry-run policy.

## Stage 18 — streaming tool artifact output

Status: `PASS`

Changes:

- Added conditional artifact fallback: output within its configured limits
  keeps the legacy inline result, while an oversized stream publishes complete
  byte-exact stdout and stderr under one code-owned workspace artifact run.
- Both streams write concurrently from their first byte into `create_new`
  staging files. RAM retains only the independently bounded previews. Invalid
  UTF-8 remains byte-exact on disk and is lossy only in the preview strings.
- Added a global 10 MiB per-stream capture ceiling. Ceiling overflow terminates
  the process tree, returns a typed `TOOL_OUTPUT_OVERFLOW` with a truthful
  global-ceiling fix hint, and publishes nothing.
- Added UUID staging, secure workspace/day containment, file sync, atomic
  no-replace directory publication, relative result paths, and RAII cleanup.
- Approval, contract drift, executable containment, and dry-run checks precede
  staging. Timeout, nonzero exit, I/O, sync, publish, and hard-overflow failures
  leave no published run.
- Updated the managed template and embedded block with exact defaults,
  ceilings, inline/artifact behavior, and unchanged approval policy.

Proof:

- `PASS` twelve Stage 18 tests covering both streams, exact/+1 boundaries,
  invalid UTF-8, timeout/nonzero/write/publish failures, approval/dry-run,
  hard-ceiling descendant termination, bounded RAM, path links/no-replace, and
  exact structured JSON.
- `PASS` full `cargo test`: 373 passed, 0 failed.
- `PASS` full `cargo test --tests`: 373 passed, 0 failed.
- `PASS` format, clippy with denied warnings, build, dev verification, and diff
  checks.
- Final Windows binary build remains an explicit Stage 35 gate because the
  bounded Docker retry again stalled before compile on the external CRT
  download. Static Windows contracts pass.

Cumulative-audit handoff:

- Stage 20 must serialize cleanup against active artifact capture.
- macOS session-escaping descendants and executable path validate/use TOCTOU
  remain explicit audit findings; they cannot be silently declared resolved.

Handoff:

- Stage 19 adds the frozen single-current-inventory and append-only reflection
  event model.

## Stage 19 — reflection inventory and append-only events

Status: `PASS_AFTER_REMEDIATION`

Changes:

- Reflection now maintains one latest current inventory node. It creates once,
  updates that same node only when the derived session set changes, and makes
  identical runs a no-op. Historical duplicate inventory nodes are preserved
  without creating another current node.
- The current inventory is derived from material, proposal, and apply records;
  it does not cite itself or keep stale sessions alive.
- Added the exact durable operational event set:
  `reflection.inventory.created`, `reflection.inventory.updated`,
  `reflection.proposal.created`, `reflection.proposal.applied`,
  `reflection.proposal.drafted`, and `reflection.apply.failed`.
- Proposal and apply receipt remain separate nodes. Proposal lifecycle events
  point to the proposal id and store no payload.
- Inventory and proposal writes own a transaction or nested savepoint, so a
  late event error cannot leave a node behind even if an outer caller commits.
- Apply uses a savepoint inside the mutation transaction. A normal apply error
  rolls back all nodes, metadata, links, and FTS changes, commits only the
  `reflection.apply.failed` event, then returns the original nonzero error.
  Failure-event, rollback, release, and late applied-event faults abort the
  outer transaction without false history.
- The CLI preserves `AUDIT_SNAPSHOT_PENDING` on a failed apply after the
  database event commits; the existing snapshot-warning contract remains the
  command warning channel.
- A three-file AUTO_PATCH clarified the privacy boundary: inventory, receipts,
  and events never copy node bodies or raw inputs; proposal and applied node
  contain only explicit user-selected structured memory. The managed block
  forbids secrets and raw captures in proposal input.

Proof:

- `PASS` inventory create/update/no-op, self-source exclusion, legacy-history
  preservation, exact event types/subjects, proposal/receipt separation,
  nested transaction atomicity, failed-apply rollback, failure-event rollback,
  injected savepoint faults, late applied-event failure, FTS rollback, and
  privacy projection tests.
- `PASS` focused reflection tests: 16 passed.
- `PASS` focused reflection CLI tests: 5 passed.
- `PASS` full `cargo test`: 382 passed, 0 failed.
- `PASS` full `cargo test --tests`: 382 passed, 0 failed.
- `PASS` format, clippy with denied warnings, build, dev verification, adapter
  parity, and diff checks.
- `PASS` independent final audit after the privacy AUTO_PATCH: P1 = 0; P2 = 0.

Cumulative-audit handoff:

- One parallel full-test attempt exposed the known Darwin process-group `EPERM`
  race; an isolated rerun and two complete suites passed. Stage 20 cumulative
  remediation still owns macOS descendant tracking and executable path TOCTOU.

Handoff:

- Stage 20 implements exact artifact retention, capture/cleanup serialization,
  fail-closed path handling, and complete cleanup path reporting.

## Stage 20 — artifact retention and exact cleanup reporting

Status: `PASS_AFTER_REMEDIATION`

Changes:

- Added a permanent `artifacts/.artifacts.lock`. Artifact capture holds a
  shared lock through process output, publish, and Drop cleanup; cleanup holds
  an exclusive lock from preflight through final rescan. Acquisition is bounded
  at five seconds and never creates staging or deletes data after timeout.
- Cleanup performs a complete bounded preflight before the first mutation. It
  rejects malformed root entries, malformed staging names, symlinks, Windows
  reparse points, special files, canonical escapes, entry-count overflow, and
  byte-count overflow.
- Enforced the accepted retention order: expired calendar-day directories,
  crash-stale strict staging directories, retained past directories when over
  the cap, then oldest regular files across current and future days. The exact
  policy is seven local calendar days OR decimal 1,000,000,000 bytes. Equal to
  the cap deletes nothing and current-day files may be deleted.
- Replaced cleanup `remove_dir_all` with checked postorder deletion. Every
  target is revalidated against the current secure snapshot immediately before
  removal. A final secure rescan supplies exact `bytes_after`.
- Cleanup JSON lists every successfully deleted path, including children of a
  removed directory. Success, partial failure, unknown-final-state, lock
  timeout, and unmet-retention errors have stable CLI codes and never claim a
  complete report without a successful rescan.
- Crash cleanup of unpublished tool staging now validates the complete
  workspace-to-artifacts-to-day ancestry and uses the same secure remover.
- Cleanup enumerates only the canonical artifacts child. Database, tools,
  runtimes, logs, audit Git, observability, exports, templates, and skills are
  outside the deletion candidate tree and have sentinel coverage.

Proof:

- `PASS` 18 artifact tests covering lock sharing/timeout/permanence, strict
  stale cleanup, malformed staging, exact day/size boundaries, current/future
  file pruning, deterministic path reporting, link/special-entry preflight,
  protected siblings, safe unpublished staging, and no-replace publish.
- `PASS` focused CLI artifact tests: 4 passed.
- `PASS` focused tool artifact-mode tests: 5 passed; symlink test: 1 passed.
- `PASS` full `cargo test`: 413 passed, 0 failed.
- `PASS` full `cargo test --tests`: 413 passed, 0 failed.
- `PASS` format, clippy with denied warnings, build, dev verification, and diff
  checks.

Cumulative-audit result:

- Initial audit found `P1 = 3` and `P2 = 3`: macOS session-escaping
  descendants, executable validate-to-spawn TOCTOU, artifact/audit ancestor
  swaps, missing nested teach savepoint, and incomplete text cleanup errors.
- Tool execution now anchors executable, cwd, and runtime resources; macOS
  tracks identities beyond process groups, Windows uses a suspended process
  plus Job Object, and dry-run still never spawns.
- Artifact removal and audit snapshot/Git writes use anchored syscalls or
  retained Windows handles. Mutation and snapshot locks share one
  identity-checked workspace capability, and existing Git metadata is checked
  before the database operation.
- The macOS fast-process `EPERM` race is bounded without making generic
  `EPERM`, child-list failures, descendants, or reused PIDs benign. Stress
  proof: 100/100 fast output-overflow runs.
- `PASS` focused proof: audit 31, mutation 16, tools 61, artifacts 18,
  teach 7, and cleanup/parity 14 tests.
- `PASS` `cargo fmt --check`, clippy with denied warnings, build, full tests,
  `scripts/dev_verify.sh`, and `git diff --check`.
- Independent final re-audit: `P1 = 0`; `P2 = 0`.
- Accepted `P3`: active same-UID SQLite parent swap outside the local/no-sandbox
  boundary; same-root Unix leaf-name race; malicious self-detaching tools;
  bounded O(n) Git preflight; incomplete fresh Git repo after crash; external
  Git writer CAS failure; Windows runtime proof deferred to Stage 35.

Handoff:

- Stage 21 may start the isolated Local Observability schema and version work.

## Stage 21 — version and isolated Local Observability schema

Status: `PASS_AFTER_REMEDIATION`

Changes:

- Set the package and lockfile root version to `0.2.0-rc1`. Updated only the
  three CLI tests whose expected value is the live package version; historical
  v0.1 fixture and migration references remain unchanged.
- Added lazy workspace paths for
  `observability/observability.sqlite`. Normal workspace initialization does
  not create the observability directory or database.
- Added a separate schema-v1 store with application id `0x414F504D`, user
  version `1`, incremental auto-vacuum, WAL, foreign keys, 5-second busy
  timeout, and `trusted_schema=OFF`.
- Added exactly five product tables: `observability_events`,
  `recall_bundles`, `bundle_nodes`, `feedback`, and `collector_state`.
  The schema has 15 deterministic named indexes and exactly four allowed
  SQLite primary-key autoindexes.
- Added strict SQL checks for boolean and nonnegative values, feedback
  outcomes, confidence, object/array JSON shape, 16-KiB event payloads, and
  4-KiB selection reasons. The state table has one schema-version singleton.
- Added private writer/reader connection wrappers with no `Deref` or raw
  connection escape. Writer initialization is lazy and transactional for all
  schema objects, state, application id, and version.
- Existing nonempty stores are validated through a true
  `READ_ONLY|NOFOLLOW` connection before a writable connection opens. Wrong or
  future ids/versions, operational DB copies, corrupt files, missing/extra or
  changed objects, columns, indexes, checks, and unexpected `sqlite_*`
  objects fail without changing the main database bytes or mtime.
- Reader open is `READ_ONLY|NOFOLLOW` plus `query_only=ON` and never creates a
  missing directory or database. Standard SQLite WAL locking is retained for
  checkpoint safety; SQLite may maintain WAL/SHM service sidecars, but reader
  tests prove the main database schema, rows, bytes, and mtime do not change.
- Added direct-child, symlink, reparse-point, database, and sidecar guards.
  Operational `memory.sql` snapshots contain no observability schema or data.
- Kept Stage 21 finite: no collector writes, CLI instrumentation, retention,
  report, export, feedback command, or UI was added.

Remediation:

- Rejected an `immutable=1` read shortcut because it can race a legal WAL
  checkpoint. The final reader uses standard SQLite read-only locking.
- Replaced a broad `sqlite_*` exclusion with an exact internal-object
  manifest. A writable-schema injected `sqlite_evil` object is now rejected.
- SQL manifest normalization preserves string-literal case, so a changed
  feedback `CHECK` cannot pass as equivalent.
- Empty-v0 acceptance is limited to `(auto_vacuum, journal_mode)` states
  `(0, delete)`, `(2, delete)`, or `(2, wal)`.

Proof:

- `PASS` 15 focused observability tests. Coverage includes exact DDL, ids,
  columns and indexes; missing, zero-byte and valid empty-v0 initialization;
  idempotence; no-create reader; read-only/query-only enforcement; WAL
  visibility; separate operational storage and snapshot exclusion; wrong and
  future headers; schema/internal-object drift; operational DB copy; corrupt
  and garbage files; symlinked directory/database/sidecar; and SQL bounds.
- `PASS` `cargo test`: 428 passed, 0 failed (final run inside
  `scripts/dev_verify.sh`).
- `PASS` `cargo test --tests`: 428 passed, 0 failed.
- `PASS` `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`,
  `scripts/dev_verify.sh`, and `git diff --check`.

Handoff:

- Stage 22 adds typed, privacy-bounded collector write APIs and best-effort
  failure isolation on top of these private wrappers. It must not expose a raw
  SQLite connection or change the operational memory schema.

## Stage 22 — collector privacy, retention, and failure isolation

Status: `PASS_AFTER_REMEDIATION`

Changes:

- Added one serializable `OutputWarning` and
  `OBSERVABILITY_WRITE_FAILED`. The existing mutation warning is a type alias,
  so current CLI JSON remains compatible. A three-file AUTO_PATCH limited the
  shared-warning wiring to `src/output.rs`, `src/main.rs`, and
  `src/mutation/mod.rs`; its compatibility test passed.
- Added the exact closed 42-event catalog, closed outcomes, and typed payloads
  for nodes, links, recall facts, tools, MCP profiles, artifacts, and counts.
  Payload JSON is capped at 16 KiB and recall selection reasons at 4 KiB.
- Added deterministic secret redaction and UTF-8-safe bounds. The public model
  has no raw task/chat/body/stdout/stderr/tool-output/environment/header,
  cookie, or token field. Workspace-relative artifact paths are validated.
  Valid Unicode link, tool, and MCP ids remain observable within product byte
  limits.
- Added a lazy per-invocation collector with UUIDv4 correlation and event ids,
  database timestamps, package version, workspace key, and a private writer.
  Invalid typed input, unavailable/corrupt stores, insert failures, and
  retention failures emit at most one generic warning, disable later writes,
  and never change the caller-owned core result or exit status.
- Added exact 30-day OR decimal-100,000,000-byte physical retention. Old roots
  are deleted in bounded stable batches; recent feedback protects its bundle,
  then expired feedback is deleted before the bundle and its cascade nodes.
  Checked counters and a monotonic retention floor are persisted.
- Physical size includes the main observability database and managed WAL, SHM,
  and rollback-journal files. Incremental vacuum repeats in bounded page
  batches while size decreases, then performs a checked WAL truncate. A
  no-progress store above the cap fails best-effort instead of looping.
- Retention only mutates the observability database. Operational memory,
  snapshots, exports, tools, artifacts, logs, runtimes, skills, templates, and
  sibling files remain outside its deletion set.

Remediation:

- Independent audit found a privacy leak in quoted JSON and inline sensitive
  headers. The original marker replacement could miss quoted keys and stop an
  escaped value at an inner quote. It was replaced by a deterministic bounded
  range scan over the 65-KiB maximum input, followed by one merged rebuild.
- Plain, single-quoted, escaped JSON, mixed-case token variants, URL tokens,
  bearer values, and inline Authorization, Proxy-Authorization, Cookie, and
  Set-Cookie values are now redacted. Backslash runs distinguish inner escaped
  quotes from the outer delimiter; benign token-budget text and Unicode remain
  unchanged.
- The final privacy probe extended the classifier to normalized camel-case
  private-key and credential fields, whitespace CLI flags such as `--token`
  and `--password`, and complete multiline PEM private-key blocks. Public
  trailing flags and non-sensitive prose remain intact.
- Independent audit also found that the collector's 128-byte workspace-key
  bound was below a valid managed filesystem component. The bound now matches
  the 255-byte managed component limit without truncating workspace identity.

Proof:

- `PASS` 29 focused observability tests, including exact catalog/schema,
  laziness, typed fields, Unicode parity, deterministic redaction, UTF-8 and
  JSON caps, invalid identifiers and paths, unavailable/corrupt/write and
  retention failures, one-warning latching, and core success/error isolation.
- `PASS` plain and escaped quoted JSON, single-quoted assignments, inline
  headers including cookie tails, camel-case and underscore token variants,
  nested escaped quotes, benign-boundary/Unicode preservation, and a valid
  managed workspace key longer than 128 bytes through lazy write/read.
- `PASS` age retention and monotonic state; physical-size oldest-first cleanup
  with a one-page batch and more than 16 allocated pages; feedback ordering;
  bundle-node cascade; inserted-event survival after retention failure; and
  protected-file, operational-DB, and snapshot immutability.
- `PASS` `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- Independent final re-audit: `P1 = 0`; `P2 = 0`.
- Open `P3` proof gaps are retained, not claimed closed: an exact
  30-day-minus/plus-1-ms boundary test, a real concurrent busy-WAL checkpoint
  test, and Windows runtime retention proof. Windows runtime proof remains in
  Stage 35.

Handoff:

- Stage 23 wires lifecycle events through `LocalCollector::new`, `record`, and
  `record_result`; it owns command coverage, not Stage 22. Stage 24 wires tool
  and health facts. Bundle-row correlation and feedback remain Stage 25.
- No known Stage 22 blocker remains. The scheduled cumulative independent
  audit is after Stage 25.

## Stage 23 — Core lifecycle instrumentation

Implementation:

- Added one invocation-scoped `CommandObservation`. It attaches one lazy
  `LocalCollector` only after a safe workspace is known, reuses one
  correlation id, freezes core duration before collector I/O, and latches at
  most one `OBSERVABILITY_WRITE_FAILED` warning.
- JSON warnings remain top-level. Text warnings are printed after core data or
  error. Existing `AUDIT_SNAPSHOT_PENDING` is always ordered before the
  observability warning.
- Wired only the Stage 23 catalog: install/workspace init; adapter seed, sync,
  and real drift; recall lifecycle; direct node create/update/deprecate;
  link; remember; teach start/propose/apply; reflection inventory,
  proposal, applied, and drafted facts. Teach add is deliberately not an
  event. Stage 24 tool/health/audit/artifact/MCP hooks and Stage 25 bundle-row
  persistence/global bundle id/feedback remain out of scope.
- Mutation events are recorded only after `mutate_workspace` returns. Recall
  returns an owned core result and drops its read transaction/connection
  before any collector write. Adapter observation validates and drops an
  existing read-only DB first. Instrumentation never creates a missing
  adapter workspace.
- Install progress retains the committed workspace status and audit warning
  when the final style-note write fails. The CLI then records
  `install.started`, successful `workspace.init`, and `install.failed` while
  preserving the original I/O exit.
- Recall records `started`, real incoming continuation, empty/truncated
  complements, and one terminal completed/failed fact. Mandatory overflow is
  exactly `started -> mandatory_overflow -> failed`, never partial success.
  Operational recall `bundle_id` is intentionally not copied to Stage 23
  observability rows.
- Payloads contain typed ids, bounded redacted node metadata, counts, recall
  selection reasons, and finite scores only. Query text, node bodies, teach
  material, proposal bodies, paths, raw output, and secrets are excluded.
  Task selections above 128 nodes safely fall back to count-only facts without
  changing the core result.

Proof:

- `PASS` lifecycle coverage and exact order for install success/failure,
  adapter missing/drift/seed/sync, recall continuation/truncation/overflow,
  node create/update/deprecate, remember, link, teach, and reflection.
- `PASS` one collector/correlation per invocation, equal frozen duration for
  multi-fact terminal rows, started duration `NULL`, Stage 23 `bundle_id NULL`,
  and reflection applied/drafted count facts under one correlation.
- `PASS` privacy canaries for node bodies, recall queries/bodies, teach input,
  reflection input, adapter paths, and token/Authorization values.
- `PASS` unavailable collector preserves both success and failure exit codes;
  style-output failure preserves committed workspace data; missing adapter
  workspace creates no AOPMem or observability path.
- Audit remediation made collector UUID generation fallible through the OS
  RNG, so RNG failure now produces one `OBSERVABILITY_WRITE_FAILED` warning
  instead of a panic. The deterministic negative helper test passes.
- Audit remediation moved all storage-independent checks before workspace
  creation for teach session/proposal, reflection proposal, node/link/teach/
  reflection mutation ids, and bounded every install answer before path
  resolution. Invalid inputs leave `AOPMEM_HOME` absent.
- At the Stage 23 boundary, a failed recall continuation was recorded as
  `recorded`, not false `success`. Stage 25 cumulative-audit remediation moved
  cursor workspace/revision mismatch ahead of all collector writes, as
  documented below.
- `PASS` 459 full tests, `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`,
  `cargo test --tests`, `scripts/dev_verify.sh`, and `git diff --check`.
- Independent final audit: `P1 = 0`; `P2 = 0`. Remaining `P3` is native
  Windows runtime proof, scheduled for Stage 35.

Handoff:

- Stage 23 is completed and accepted. Stage 24 may wire tool, health, audit,
  artifact, and MCP facts without changing the Stage 23 lifecycle contract.

## Stage 24 — Tool, health, audit, artifact, and MCP instrumentation

Implementation:

- Extended mutation outcomes with a typed snapshot observation. A completed
  snapshot records duration and bytes. A failed snapshot records the real
  attempt duration, then emits adjacent `audit.snapshot.failed` and
  `audit.snapshot.pending` facts under one correlation and frozen duration.
- Added invocation-local `ToolRunTrace`. Validation success and real process
  spawn are derived from the runner, so dry-run and validation failures never
  create a fake run start. A spawned run has one terminal completed, failed,
  or timeout fact. Artifact mode stores only its safe relative path and byte
  count; stdout, stderr, arguments, environment, and executable paths are not
  observed.
- Wired doctor, verify, artifact cleanup, and MCP status/list results through
  the existing single `CommandObservation`. Core handles and locks are dropped
  before collector writes. Missing workspaces are never created for
  observation.
- Doctor and verify payloads use exact bounded count keys. Cleanup reports use
  counts only and never persist deleted paths. MCP get/add records only
  recognized profile states; not-found does not invent a missing profile.
  MCP list records one exact aggregate and marks incomplete pages as
  `truncated`.
- Collector failure remains non-fatal. Core output, data, warnings, and exit
  status are preserved. All Stage 24 rows keep `bundle_id = NULL`; bundle
  correlation remains Stage 25. Schema version 1 and the exact 42-event
  catalog are unchanged.

Proof:

- `PASS` real mutation snapshot success and failure observations, including
  ordered failed/pending facts with one frozen duration.
- `PASS` tool dry-run, validation failure, real spawn, timeout, inline output
  overflow, artifact publication, exact terminal cardinality, and privacy
  canaries.
- `PASS` doctor success/warning and verify success/warning/failure typed
  counts without issue text or paths.
- `PASS` cleanup partial-result exact keys without deleted paths; MCP Unicode
  profile id, exact list aggregate, incomplete-page outcome, and no fake
  not-found state.
- Added 7 focused regression tests. Full suite is `466/466 PASS`.
- `PASS` `cargo fmt -- --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`, and
  `git diff --check` on the stable implementation.

Handoff:

- Independent final audit: `P1 = 0`; `P2 = 0`.
- Stage 24 is completed and accepted. Native Windows runtime proof remains one
  explicit `P3` Stage 35 release-gate item; no Windows-only product behavior
  was claimed here.
- Stage 25 may add bundle rows, global `--bundle-id`, and feedback without
  changing Stage 24 event meanings or payload privacy boundaries.

## Stage 25 — Bundle correlation and feedback

Implementation:

- Added typed, validated `RecallBundleRecord` and `RecallBundleNode` writes.
  One immediate transaction publishes the logical parent, first-seen node
  metadata, and all recall lifecycle rows. Task recall stores mandatory and
  task nodes; bare, full, failed, and mandatory-overflow recall stay
  parent-only.
- Continuation updates preserve the first timestamp and correlation id, sum
  duration, increment `continuation_count` for every valid attempt, keep the
  last successful `more_results` across a failure, and replace the latest
  outcome/error. `(bundle_id, node_id)` first-seen rows never duplicate or
  silently replace earlier metadata.
- Added canonical lowercase UUIDv4 global `--bundle-id`. First task, bare, and
  full recall reject it before workspace access. Continuation accepts only the
  exact id encoded in its cursor. All Stage 23/24 events inherit the optional
  global id; calls without it retain `bundle_id = NULL`. Every persisted recall
  lifecycle row carries its generated or continued bundle id.
- Added `aopmem feedback record --outcome useful|partial|wrong [--reason ...]`.
  It requires global `--bundle-id`, an initialized Local Observability store,
  and a same-workspace recall parent. Feedback and `feedback.recorded` commit
  atomically. Missing stores and parents are not created. Reasons are trimmed,
  nonblank, capped at 1024 UTF-8 bytes, and deterministically redacted.
- Feedback resolves only the existing workspace path. It never opens or
  changes operational memory and never publishes an audit snapshot. A
  post-commit retention failure returns the durable receipt plus the standard
  observability warning.
- Updated the canonical and embedded managed block and Memory Keeper skill.
  Memory Keeper now passes the exact bundle id on continuation and all later
  AOPMem operations for one task, and may record privacy-bounded post-task
  feedback.
- Observability schema version 1 and the exact 42-event catalog are unchanged.

Proof:

- `PASS` atomic first page, failed continuation, successful continuation,
  cumulative duration, first timestamp/correlation preservation, latest
  outcome/error, successful `more_results`, continuation counts, first-seen
  deduplication, and transaction rollback on parent/node/event failure.
- `PASS` task mandatory/node capture with bounded redacted metadata and typed
  reasons; bare/full/overflow parent-only bundles; all recall lifecycle rows
  have the same bundle id.
- `PASS` UUIDv4 parser rejects uppercase, v1, compact, nil, and malformed ids;
  global placement works before or after the subcommand. First/bare/full and
  mismatch rejection happen before AOPMem home access. Exact continuation
  reaches core lookup and the real two-page CLI proof updates one parent.
- `PASS` Stage 23 `remember` and Stage 24 `audit.snapshot.completed` inherit
  the global id, while the identical no-flag invocation keeps both rows NULL.
- `PASS` feedback missing-store/no-parent no-create behavior, atomic event
  rollback, post-commit retention warning, input preflight, and deterministic
  secret redaction. Operational `aopmem.sqlite` and audit `memory.sql` hash,
  size, and mtime remain byte-for-byte unchanged across feedback.
- `PASS` canonical/embedded managed block equality and Memory Keeper contract
  assertions.
- `PASS` 484 full tests, `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`,
  `cargo test --tests`, `scripts/dev_verify.sh`, and `git diff --check`.

Cumulative-audit remediation:

- Initial independent audit severity was `P1 = 1`, `P2 = 3`. The P1 was a
  Local Observability privacy gap for URI userinfo and bounded vendor-token
  shapes. The P2 findings were early operational-DB failure linkage,
  copied-DB cross-workspace cursor binding, and the additional stale-cursor
  versus mandatory-overflow ordering mutant.
- Privacy redaction now covers URI userinfo with a normal host, no host, or a
  truncated host boundary, plus bounded `glpat-`, `sk_live_`, and `sk_test_`
  vendor-token shapes. Deterministic direct and persisted-payload canaries
  prove the secrets are removed while benign lookalikes and public Unicode
  remain intact.
- Recall now resolves a safe workspace key/path without creating anything,
  attaches the lazy collector, and only then opens operational memory. An
  existing but unopenable database atomically records the chosen failed bundle
  with exactly `recall.started` and `recall.failed`; the structured error keeps
  the same `bundle_id`. A missing database remains no-create. Collector failure
  preserves the core exit status and adds one `OBSERVABILITY_WRITE_FAILED`.
- Continuation revisions are deterministically bound to
  `aopmem-recall-workspace-v1 || workspace_key || operational_revision`; only
  the resulting 32-character hash is stored in the cursor. An exact copied
  operational database cannot reuse workspace A's cursor in workspace B, and
  B's absent observability state stays byte-for-byte absent. Revision/workspace
  mismatch fails before bundle, node, or event writes. The real same-workspace
  two-page proof still succeeds and preserves first-seen node deduplication.
- Task continuation validates the operational revision and workspace binding
  immediately after the read transaction starts and `--full` handling, before
  mandatory node loading or any other retrieval. The copied-DB CLI proof then
  mutates B with an active 1 MiB gate that independently overflows mandatory
  context, and finally with a schema-valid BLOB body that makes mandatory row
  decoding fail. All three B invocations return `STALE_RECALL_CURSOR` first
  and leave B's absent observability directory absent.
- Added five focused tests and strengthened existing CLI and privacy regression
  proofs. Full suite is `484/484 PASS`;
  `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`,
  `scripts/dev_verify.sh`, and `git diff --check` pass.
- First independent clean re-audit verdict: `P1 = 0`, `P2 = 0`, `P3 = 0`.
- Second independent clean re-audit verdict: `P1 = 0`, `P2 = 0`, `P3 = 0`.

Handoff:

- Stage 25 and the cumulative Stage 21–25 audit are complete and accepted after
  remediation. No P1, P2, or P3 remains in this audit scope. Stage 26 may
  consume the stable bundle, bundle-node, feedback, and event facts without
  changing this write contract.

## Stage 26 — Observe status and effectiveness report

Implementation:

- Added `aopmem observe status` and `aopmem observe report` with stable JSON
  envelopes and concise human output. Dispatch returns before constructing
  `CommandObservation`; the commands never initialize a collector, create a
  store, write a self-observation, open operational memory, run migrations, or
  invoke retention.
- Missing workspace/store state returns `collection_status=not_collected`,
  `complete=false`, nullable schema/facts, and creates no AOPMem path. An
  initialized store returns schema version 1 and exact table counts/status
  facts.
- A ready report establishes one SQLite read snapshot, captures one canonical
  RFC3339 millisecond clock inside that snapshot, and uses an inclusive
  30-day window. Lifecycle events, first-seen bundle nodes, feedback, and
  collector state are read from the same snapshot. A concurrent continuation
  cannot leak a post-`end_at` event or node.
- Recall period facts derive only from lifecycle events whose own timestamp is
  inside the window. Started, failed, empty, overflow, and continuation events
  are counted exactly; distinct bundle sets supply `more_results`, FTS, graph,
  and continuation usage. Terminal `more_results` uses the last
  `recall.completed` in stable `(timestamp, id)` order. Parent first timestamp,
  latest outcome, and lifetime continuation count do not affect period facts.
- Bundle-node selection uses `first_seen_at` in the report window and validates
  its parent workspace without filtering by parent timestamp. A current
  continuation node for a 31-day-old parent is included; foreign parents fail
  closed.
- Report facts cover recall totals/failures/empty/mandatory overflow,
  continuation and `more_results` use, FTS/graph use, selected node types,
  selected workflows/tools/failure modes, feedback, tool outcomes and repeated
  errors, repeated correction/failure-mode titles, reflection, adapter drift
  missing/drifted/failed events, pending audit snapshots, doctor/verify
  failures, artifact deletions, and MCP missing/configured-unverified
  observations.
- Output is fact-only. Top lists are deterministic, limited to 20, and expose
  `more_results`. There is no product score, advice, hidden task text, raw node
  body, raw chat, raw tool output, secret, or environment value.
- Reader validation is fail-closed for unknown or impossible event/outcome/
  payload tuples, wrong exact count keys, extra JSON fields, invalid calendar
  timestamps, duplicate identifiers where forbidden, unsafe paths,
  incompatible schema, foreign-workspace feedback parents, and malformed
  privacy-bounded fields. Titles, tool ids, and emitted error codes are
  deterministically redacted again at report time.
- The existing observability schema version 1, 42-event catalog, collector,
  retention policy, operational database, snapshot format, and product
  decisions remain unchanged.

Initial independent audit and remediation:

- Initial verdict: `P1 = 0`, `P2 = 2`. One P2 showed that recall totals,
  failures, continuations, and terminal state used the parent bundle's first
  timestamp/latest outcome/lifetime counter instead of each lifecycle event's
  timestamp. The other P2 showed valid `adapter.drift` failure events were
  accepted but omitted from the report.
- Both P2 findings are remediated. Tests prove a current continuation and node
  for a 31-day-old parent, failed-then-successful retry accounting, immunity to
  an inflated parent lifetime continuation count, last-completion terminal
  state, explicit adapter failure facts, and foreign bundle-node fail-closed
  behavior.
- Independent clean re-audit verdict: `P1 = 0`, `P2 = 0`, `P3 = 0`. It
  repeated all five remediation regressions, the 23 report tests, the two CLI
  tests, format, clippy, and diff checks without changing files.

Proof:

- `PASS` 23 focused report tests: missing/no-create, initialized zero facts,
  inclusive start/end boundaries, outside-window exclusion, exact aggregate
  fixture, concurrent continuation snapshot isolation, post-end node
  exclusion, top-20 completeness marker, re-redaction canaries, no score or
  advice, main-DB byte/mtime stability, operational-DB absence, invalid known
  tuple, extra JSON fields, NUL, foreign feedback and bundle-node parents,
  incompatible store, Unix symlink rejection, old-parent current lifecycle,
  failed retry preservation, parent lifetime isolation, and adapter failure.
- `PASS` 2 focused CLI tests: exact global `--json` parsing for `observe status`
  and `observe report`, stable success execution, no collector events, no
  operational or observability schema/data mutation, and missing-home/
  workspace no-create behavior.
- Full suite: `509/509 PASS`.
- `PASS` `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`, `cargo test`,
  `scripts/dev_verify.sh`, `git diff --check`, and ledger JSON validation.

Handoff:

- Stage 26 is complete after remediation and independent clean re-audit.
  Stage 27 may reuse the stable fact model. Export must keep the report
  fail-closed and must not weaken one-snapshot, event-time, or privacy
  contracts.

## Stage 27 — Debug capsule and export

Implementation:

- Added `aopmem observe export --output <file.zip>` with the existing global
  `--json` mode. Dispatch returns before `CommandObservation`; export never
  initializes a collector, records itself, runs retention or migrations,
  invokes tools, or mutates operational memory.
- The operational database is required and opened read-only. The exporter
  validates the exact v0.2 migration/table/column/FTS manifest, runs read-only
  quick and foreign-key integrity checks, and establishes one stable
  operational transaction before reading observability.
- Local Observability is read from its own stable read transaction. Missing
  observability succeeds as explicit `not_collected` with empty JSONL files
  and no store creation. Initialized-empty succeeds as `ready` with zero
  facts. Unsafe, incompatible, corrupt, or foreign-workspace state fails
  closed. No cross-database atomicity is claimed.
- The ZIP contains exactly 12 ordered entries: `manifest.json`,
  `product.json`, `workspace_summary.json`, `memory_summary.json`,
  `health.json`, `events.jsonl`, `recall_bundles.jsonl`,
  `bundle_nodes.jsonl`, `feedback.jsonl`, `tools_summary.json`,
  `mcp_summary.json`, and `README.md`.
- ZIP output is deterministic for unchanged snapshots: Stored ZIP64 entries,
  fixed metadata and permissions, LF output, stable row order, and a reference
  time taken from the latest persisted observability timestamp. Missing or
  initialized-empty observability uses the fixed
  `1970-01-01T00:00:00.000Z` epoch instead of the wall clock.
- `memory_summary.json` streams every node without selecting or retaining body
  values. It includes counts by type/status, broken/orphaned/deprecated/draft
  counts, link count, and privacy-bounded title, summary, source, trust,
  confidence, and incoming/outgoing link facts.
- Event raw payload JSON is parsed and validated but never serialized. Tool
  summaries never select full contract JSON. MCP summaries never select
  credential sources or notes. All exported free text is passed through the
  deterministic Stage 25 redactor.
- `health.json` derives typed doctor/verify state from the latest validated
  persisted observation. It reports `not_collected`, `success`, `warning`, or
  `failure`; absence never becomes a false healthy default.
- Publication uses a private create-new temporary file in an anchored real
  parent, writes and syncs through one handle, verifies that exact open handle,
  and performs anchored no-replace publication. Existing output returns
  `OUTPUT_EXISTS`; pre-publication failures remove the temporary file and never
  clobber a final path.
- A known-visible final path never becomes a core failure. If directory
  durability or temporary cleanup cannot be confirmed after publication, the
  command succeeds with `EXPORT_PUBLISHED_WITH_WARNING`, an honest
  `published_with_warning` status, and the exact cleanup-confirmation fact.
- SQLite read-only WAL handling can create empty `-wal`/`-shm` lock sidecars
  when required by journal mode. Proof holds the operational main DB bytes,
  mtime, schema, and rows unchanged and creates no rollback journal or
  semantic write.
- Exposed typed leaf builders for Stage 29 product, workspace, memory header
  and node, health, tool, and MCP read APIs. These seams do not expose SQL,
  mutation, secrets, bodies, or raw observability payloads.

Rolling review remediation:

- Review found and closed false healthy defaults, workspace-key/path mismatch,
  valid empty optional values, incomplete operational schema/FTS preflight,
  optional empty trust handling, temporary-file close/reopen identity risk,
  and an error-after-known-publication edge.
- The final independent verdict is `P1 = 0`, `P2 = 0`, `P3 = 1`. The remaining
  P3 is the narrow Unix same-UID leaf-name race between inode verification and
  `linkat`. It is explicitly accepted under frozen D-017: active same-user
  path tampering is outside the local/no-sandbox v0.2 boundary. No custom VFS
  or platform-specific rename syscall expansion was added.

Proof:

- `PASS` 15 focused export tests covering exact entry names/order,
  byte-identical repeat export, privacy canaries, missing and empty
  observability, typed health, workspace binding, foreign rows, dropped
  schema/FTS, operational foreign-key violation, existing-output no-clobber,
  late JSONL failure cleanup, post-publication warning, same-handle source
  replacement, unsafe links, corrupt stores, missing parent, and a streamed
  10,000-node/30,000-link corpus.
- `PASS` one focused CLI proof for global `--json`, stable execution, no
  self-observation, and unchanged operational/observability main DB state.
- Full suite: `524/524 PASS` through both `cargo test` and
  `cargo test --tests`.
- `PASS` `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`,
  `scripts/dev_verify.sh`, `git diff --check`, and ledger JSON validation.

Handoff:

- Stage 27 is complete. Stage 28 may reuse the typed privacy-bounded summary
  builders, but UI endpoints must preserve the read-only, workspace-binding,
  validation, and no-raw-payload boundaries.

## Stage 28 — UI server and security

Implementation:

- Added `aopmem ui`, `aopmem ui --no-open`, and `aopmem ui --port 0`.
  Dispatch returns before `CommandObservation`, requires an existing workspace,
  opens its operational database read-only only for preflight, then drops the
  connection. It never creates or writes operational memory or Local
  Observability and never self-observes.
- Added one invocation-scoped synchronous `tiny_http 0.12.0` server with
  default features disabled. The listener is hard-coded to exact IPv4
  `127.0.0.1`; wildcard, adjacent `127/8`, IPv6 loopback, and all non-loopback
  addresses are rejected. Port zero asks the OS for a random free port, while
  an occupied explicit port fails closed.
- Every invocation generates 32 random OS bytes and encodes them as 64
  lowercase hex characters in the first URL path segment. The token is
  constant-time compared and redacted from `Debug`. Missing and wrong tokens
  return the same 404 response, and authorization happens before method
  handling.
- The HTTP surface is an exact GET-only allowlist for compile-time embedded
  `index.html`, `app.css`, and `app.js`. Valid non-GET asset requests return
  405 with `Allow: GET`; unknown, traversal, encoded, queried, fragmented, API,
  and write paths return 404. There is no stop, upload, SQL, tool, mutation, or
  arbitrary file endpoint.
- Every response has `Cache-Control: no-store`, `Referrer-Policy: no-referrer`,
  `X-Content-Type-Options: nosniff`, and the exact local-only CSP. No wildcard
  CORS header exists. Embedded assets contain no CDN, external URL, browser
  storage, Node.js runtime, frontend build, or outbound HTTP client.
- Browser launch uses `/usr/bin/open` with a direct argument on macOS and
  `ShellExecuteW` on Windows. It invokes no shell, PowerShell, or Codex CLI.
  `--no-open` skips the launcher. Launcher failure returns
  `UI_BROWSER_OPEN_FAILED` while leaving the server alive and printing its URL.
- Production lifetime is the blocking CLI invocation. Tests use only an
  internal atomic stop flag plus `tiny_http::Server::unblock`; no test stop
  route or production daemon was added.

Dependency and platform proof:

- `tiny_http` runtime dependencies are exactly `ascii`, `chunked_transfer`,
  `httpdate`, and `log`; there is no TLS backend, async runtime, or HTTP client.
- The final locked cross-check used Cargo 1.89.0 and cargo-xwin 0.23.0:
  `cargo xwin check --locked --target x86_64-pc-windows-msvc`. It exited 0 and
  compiled `tiny_http` plus the Windows `ShellExecuteW` path.
- The cross-check exposed pre-existing Rust 1.89 Windows compile drift. Under
  an approved AUTO_PATCH window, three production files were wired without a
  product change: audit and artifact filesystem identities now use
  `GetFileInformationByHandle` on already-open no-follow handles, generic
  access constants use `Win32::Foundation`, and the tool path conversion has
  the required cfg-Windows `OsStringExt`. One stale static source assertion was
  updated to prove the new high/low file-index fields. Reparse, identity,
  no-delete-share, and fail-closed checks remain in place.
- Final AUTO_PATCH hashes:
  `src/audit/anchored.rs=040f7e25b49b9f986ec27f636177c7eeba7df35d606cc64b0481081dbabb9a2f`,
  `src/audit/anchored_git.rs=c289e0fc2460d89ecf3a624abfbea055606b3f58c23372a9d165074ad8b10569`,
  `src/artifacts/mod.rs=949bdf41ca7356d7a0e8438ec22b2024f7c867c671b401806aa2a05c37333d6a`,
  and
  `src/tools/mod.rs=33dab71523a581aa7edf71b3e5723d90d10d178272d460ce84d20710386cca3c`.
  `src/audit/mod.rs` was not changed by the AUTO_PATCH and sealed as
  `b22d476ba3b90ae6db81a7a1c54480c90d69a06fcfa0b84461c5894f3d31f315`.
- The locked Windows check has one pre-existing warning at
  `src/cli/mod.rs:483`: an `error` binding is unused only on Windows. It does
  not fail the build. CLI was frozen after the UI audit, so the P3 wiring
  cleanup is explicitly deferred to Stage 29, which already changes CLI.

Proof:

- `PASS` 11 UI tests for exact loopback, 32-byte/64-hex token and Debug
  redaction, identical unauthorized 404s, auth-before-method, strict 405/404
  routing, traversal/query rejection, all embedded assets, exact headers, no
  CORS, fixed busy-port failure, deterministic internal shutdown, no-open, and
  nonfatal launcher failure.
- `PASS` 3 CLI tests for exact command forms, missing-workspace exit 3 with no
  path creation, no browser call under `--no-open`, no self-observation, and
  byte/size/mtime-stable operational and observability main databases.
- `PASS` affected regressions: 31 audit tests, 18 artifact tests, and the
  Windows tool process-tree static contract. Full `cargo test` and
  `cargo test --tests` each passed `538/538`.
- `PASS` `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo build`, `scripts/dev_verify.sh`, and `git diff --check`.
- Independent AUTO_PATCH audit matched every final hash and returned
  `P1 = 0`, `P2 = 0`, `P3 = 0`.
- Independent frozen UI audit returned `P1 = 0`, `P2 = 0`, `P3 = 1`. One
  aggressive valid-request `SO_LINGER` RST stress run caused a rare panic in a
  `tiny_http` worker at its internal `remote_addr.as_ref().unwrap()`. The main
  UI process stayed alive, the session token was absent from the panic, and a
  normal authenticated GET still returned 200; repeated stress did not
  reproduce it. This local dependency-hardening item is accepted for rc1 under
  D-021 and does not justify a Stage 28 scope expansion.

Handoff:

- Stage 28 is complete. Stage 29 may add only the bounded, typed, read-only API
  layer behind the frozen loopback/token/router boundary. It must not add a
  write endpoint, self-observation, external network access, or a daemon.

## Stage 29 — UI read APIs

Implementation:

- Added exactly 11 authenticated `GET /api/v1/*` routes: bootstrap, overview,
  memory, node, node-links, graph, activity, bundle, effectiveness, tools, and
  MCP. Authorization and exact route matching happen before method and query
  handling. Valid non-GET requests return 405; unknown write paths return 404.
- Every operational response reopens the existing workspace with
  `READ_ONLY | NOFOLLOW`, uses one deferred read transaction, and never applies
  migrations. Observability reads use the existing strict read-only reader and
  never create a missing store, run retention, or record the UI invocation.
- Added a strict bounded query parser. Unknown or duplicate fields, malformed
  percent encoding, invalid UTF-8, controls, noncanonical numeric values,
  invalid filters, and oversized targets fail closed with fixed safe errors.
- Memory, links, activity, bundle nodes, tools, and MCP use default page 100,
  maximum 500, stable filter-bound keyset cursors, explicit `more_results`,
  `next_cursor`, and `complete`. Memory lists never return body; node detail
  always returns the complete validated body.
- Graph pages are deterministic and bounded at 200 unique node summaries and
  500 edges. A selected center is returned as fixed `center_node` context on
  every page; neighbor traversal is cursor-paginated without skips or
  duplicates. Every returned edge endpoint is present in the page or its
  center context, and both node and edge truncation are explicit.
- Activity returns validated metadata only and never returns `payload_json`.
  Bundle detail is workspace-bound to a canonical UUID v4 and returns only
  bounded, redacted node summaries, scores, and closed selection reasons.
  Effectiveness serializes the same fact-only report as `observe report`.
- Tool SQL never selects `contract_json`. MCP SQL never selects credentials or
  notes. Both use the Stage 27 deterministic redacting summary mappers.

Proof:

- `PASS` 13 focused HTTP/API tests. Coverage includes auth/route/method order,
  parser negatives, empty and paginated memory, complete node body, self-links,
  FTS/status filters, scoped cursors, centered three-page traversal, exact
  200-node/500-edge graph boundaries, missing observability no-create,
  canonical bundle UUIDs, workspace contamination fail-closed behavior,
  deterministic redaction canaries, effectiveness fact parity, Tools/MCP
  secret-field omission, and absence of every candidate write endpoint.
- A full GET traversal over all 11 routes preserved byte-for-byte operational
  and observability database files, sizes, mtimes, complete schema manifests,
  and row counts.
- `PASS` `cargo test`: 546 passed, 0 failed.
- `PASS` `cargo test --tests`: 546 passed, 0 failed.
- `PASS` `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build`,
  `scripts/dev_verify.sh`, and `git diff --check`.
- Independent frozen audit on stable SHA-256 hashes returned `P1 = 0`,
  `P2 = 0`, `P3 = 2`. D-022 accepts the two semantics-only P3 items: an empty
  Memory page still declares the endpoint-wide body omission, and the first
  centered page serializes the center both as fixed context and as its first
  paginated item. Stage 30 must deduplicate graph nodes by id.
- Frozen hashes:
  `src/ui/data.rs=cc2f82021f3c1d21f338fc9ad05a49939460e4c71841498be85e50386db81fd5`,
  `src/ui/http.rs=377d5bd466da9ab5c3ac8dd0bc210d8b99bd9f22cf519d5ffbdc7b7bf1115151`,
  and
  `src/observability/ui.rs=08298edae9d838fcc37f295925133c52dce86bc2d83403a9d2ed5fe54a812b73`.

Handoff:

- Stage 29 is complete. Stage 30 must consume only these frozen DTOs, treat
  `center_node` as fixed graph context, deduplicate graph nodes by id, keep all
  DOM insertion text-only, and prove the real API separately from screenshot
  fixture interception.

## Stage 30 — UI frontend, screenshots, and docs

Implementation:

- Added six embedded desktop views: Overview, Memory, Graph, Activity,
  Effectiveness, and Tools/MCP. The frontend uses vanilla JavaScript, system
  fonts, system light/dark preference, and no external asset or runtime.
- All untrusted values enter the DOM through `textContent` or constructed text
  nodes. There is no `innerHTML`, `eval`, storage, cookie, upload, write route,
  tool execution, or external URL.
- API calls are token-relative, same-origin authenticated GETs with omitted
  credentials, no cache, rejected redirects, and no referrer. Abort and stale
  response guards prevent an old view from overwriting a new view.
- Lists expose loading, ready, empty, partial, error, retry, and explicit
  continuation state. Memory list rows contain no body; node detail fetches the
  complete body separately.
- Graph rendering is deterministic, deduplicates the fixed center context, and
  honors the frozen maximum of 200 unique nodes and 500 edges.
- Effectiveness displays the same verifiable facts as `observe report` without
  a score. Tools/MCP distinguishes complete, partial, and total failure.
- `docs/DESKTOP_UI.md` documents the local/token/read-only boundary, bounded
  graph, troubleshooting, and SQLite WAL/SHM coordination behavior.

Browser proof:

- Created only a temporary workspace under `/tmp`. It contained 16 nodes,
  12 links, and 46 observability events. No real user workspace was opened.
- Drove the real token-authenticated API at `1440x900` through all six views.
  Node body was absent from the list and present only in detail. Bundle detail
  exposed 15 selected nodes with bounded scores/reasons and no raw payload.
- Graph returned 16 unique nodes and 12 edges. No write, run, install, upload,
  external network, console warning, or console error was present.
- Stopping the server made both Tools/MCP requests fail and produced the exact
  overall error state with two bounded `UI_CONNECTION_FAILED` messages.
- Repeated raw captures were byte-identical for Overview, Graph, and Activity.
  Browser-returned JPEG bytes were mechanically converted and rechecked as
  real RGB PNG files at exactly `1440x900`.
- Operational main DB fingerprint remained
  `17dc982de693c8d7ce1c511969b4a1eb81cddf41cbeec4a0caa43735fd21869b`;
  observability main DB fingerprint remained
  `c3162ce4c9acfc1deeac327a1f5f68e312020390c7bf15cff5031aac6c594f52`.
  Bytes, size, mtime, schema, and row counts were unchanged. The exact SQLite
  WAL/SHM coordination sidecars were recorded separately. Port 51443 was
  closed after proof.

Proof:

- Final asset hashes:
  `assets.rs=374afabd99cca73777fc41e05acb5c86a309f9b691edde0601b18c1f849a0d71`,
  `index.html=7756ed20757d238deb69efd64f06b2c3d3d3cf6aaf05565b1342d1d62cf13342`,
  `app.css=a2bb6be69dda3fe4e16d894cf89c9571b20f0ee01339a2019bdb16dc525a2dda`,
  `app.js=8910acb1813fddfe91a8639a90e435243dbe4a60e7c1a11073ca761eec6258d0`,
  and
  `DESKTOP_UI.md=f0f8b2c1b964ab616d958ff2577a3208e18101e6c79173c5818669cd85005897`.
- Screenshot hashes:
  `Overview=8272778ccea477fded9586fa2498422b35684b6e6f7ac2b94f544ad79f4394bf`,
  `Graph=4e82b570306d7b627948ed87c6e43085a2827493605cf59d91c5aa08535d76b3`,
  and
  `Activity=b3b465c72cba7abe9f4a5bed12e42d579bba3d7af9b5daaace4c607621d2a0ea`.
- JavaScript syntax PASS; 7/7 final asset tests PASS; 24/24 scoped UI tests
  PASS before the final CSS-only correction; cumulative full suite checkpoint
  PASS at 561/561.
- Independent cumulative Stage 26–30 audit initially found no P1 and nine P2
  groups: two Stage 26 groups and seven Stage 30 groups. All were fixed.
  Final verdict is P1=0, P2=0. Four existing P3 items remain accepted under
  D-017, D-021, and D-022.

Handoff:

- Stage 30 is complete. The final Stage 35 gate must rerun the whole Rust suite
  after upgrade/install changes stabilize. Do not change the frozen Stage 29
  API or expand the read-only UI product scope.

## Stage 31 — upgrade plan

Implementation:

- Added `aopmem upgrade plan --all-workspaces --json` as a strict no-write and
  no-self-observation command. Omitting `--all-workspaces` fails with
  `INVALID_ARGS` and exit code 2.
- The plan scans only `<AOPMEM_HOME>/workspaces` and intentionally ignores the
  old file MVP. Workspaces are returned in stable order with binary version,
  schema versions, pending migrations, exact blocker codes, and disk facts.
- Existing databases are opened through a validated immutable read-only URI,
  `NOFOLLOW`, `query_only`, and in-memory temp storage. Existing SQLite WAL,
  SHM, or journal sidecars fail the plan rather than being modified.
- Corrupt, unknown, or newer schemas block only the reported workspace. Disk
  capacity is checked through `statvfs` on macOS and
  `GetDiskFreeSpaceExW` on Windows, including the DB backup and installed
  binary requirement.
- A missing AOPMem home returns a ready empty plan and leaves the path absent.

Proof:

- 9/9 upgrade-plan module tests PASS and 1/1 CLI test PASS.
- Full `cargo test` PASS at 561/561.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo build`, and `git diff --check` PASS at the Stage 31 checkpoint.
- A live missing-home proof returned `plan_only=true`,
  `writes_performed=false`, zero workspaces, and did not create the root.
- Independent scope review found P1=0 and P2=0. The Windows disk branch was
  statically verified; the final PE build remains Stage 35 proof.
- Checkpoint hashes before Stage 32 began:
  `upgrade/mod.rs=387865cba38c77f4648524cf39f7ce010786405c8908fa851242b56ef7cbfda0`,
  `cli/mod.rs=5c43fc0fc219b030494eaccb106098f9dd1656a70b897d5129c526632f1d0968`,
  `schema/mod.rs=289b4e71c334309f09bdf88067ad583d38fc406ff460be1c032339215eeb36fc`,
  and
  `main.rs=cbd4e512f5fe23de800df1e319e638cce7b026497928bc9ea3ade01d277b3de1`.

Handoff:

- Stage 31 is complete. Stage 32 owns all writes, durable backups, migrations,
  rollback, asset/adapter refresh, health checks, and update observability.

## Stage 32 — upgrade apply, backups, migration, and recovery

Implementation:

- Added `aopmem upgrade apply --all-workspaces --json`. Each run creates a
  durable backup root and records exact binary, database, adapter, and owned
  asset backup paths. The running binary is never replaced by this command.
- Each source database is backed up with SQLite Online Backup while a separate
  guard connection holds `BEGIN IMMEDIATE`. Pending migrations commit in that
  same guarded transaction, so an old writer cannot commit between backup and
  migration.
- Adapter and owned assets use exact-byte preflight before the first write and
  safe restoration checks. Concurrent user edits block the operation and are
  preserved rather than overwritten.
- A later workspace failure stops the run but never restores an earlier
  committed workspace. Durable backups and the per-workspace report support a
  safe rerun. Forced recovery with an intervening commit fails closed and
  retains the pending marker.
- Update and audit-snapshot observability is best effort. Collector failure
  adds `OBSERVABILITY_WRITE_FAILED` without changing the core command result.
  Snapshot failure after the DB commit is success-with-warning and remains
  visible to doctor/verify.

Proof:

- 14/14 focused apply tests, 1/1 CLI parse test, and 575/575 full Rust tests
  PASS. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check`, `cargo build`, and `git diff --check` PASS.
- Exact logical pre/post data was checked for nodes, links, aliases, tags,
  sources, events, registries, tool contracts, and MCP profiles. Tool/artifact
  bytes and the audit Git parent history were preserved.
- Negative proofs cover WAL competing writers, migration rollback, forced
  rollback recovery, concurrent recovery commit, two-workspace stop/no-loss,
  second disk probe, corrupt DB, adapter drift, old-binary backup failure,
  concurrent adapter/asset edits, collector failure, pending snapshot, and
  workspace-set mismatch. Final audit: P1=0, P2=0.
- Frozen SHA-256:
  `apply.rs=141e5c8b93f0164f9258e451ee6573e6502708e67fcac9a93ad8e528e9513961`,
  `upgrade/mod.rs=b3bb2dcd7b90b07949715494eb8ada9b13faf015661607bbf7109cdf2d82640c`,
  `cli/mod.rs=f827b45e079100dc00a979be81fe355105f5de59bea85646586960af72606f63`,
  `Cargo.toml=f840ce5de520cabbed06642c18aea31d47fb1d132eaa77b26375f495b0243ccf`,
  and
  `Cargo.lock=b0df22f1c3894dd0dc42e721c3e069d80d1809d36105b7ad41fc2ac45835f269`.

Handoff:

- Stage 32 is complete. Backups must remain after final proof. Stage 35 must
  exercise the real final binary against an isolated v0.1 fixture.

## Stage 33 — v0.2 fresh/update installers

Implementation:

- Added native prebuilt-binary installers and the semantic install prompt for
  Apple Silicon macOS and Windows 11 x64 PowerShell 5.1. Update never runs
  onboarding and all tests use isolated homes and workspaces.
- The peeled `v0.1.0-rc3` tag binaries report `aopmem 0.1.0`. Installers bind
  that exact reported version to the exact platform-specific tag SHA-256.
- New assets are downloaded through validated HTTPS, matched to one exact
  checksum entry, staged privately, version-checked, and then used for plan and
  apply. The installed v0.1 binary remains untouched until apply succeeds.
- After apply begins, failures never restore v0.1. Apply or publish failure
  retains the verified v0.2 recovery binary and every durable upgrade backup.
  Failure output keeps the complete JSON plus concise workspace/code/message
  and backup-root facts.
- Windows uses native PowerShell 5.1 rules, TLS 1.2, bounded validated HTTPS
  redirects, UTF-8 setup, and fail-closed reparse checks. It requires no admin,
  WSL, Cargo, Rustup, Git clone, Node.js, Codex CLI, or external terminal.

Proof:

- The isolated installer audit passed all 10 groups: fresh/update, real tagged
  macOS binary, platform SHA binding, malformed/duplicate checksums, unsafe
  URI and link paths, backup/plan/apply/publish failures, cleanup/recovery, and
  no update onboarding. `sh -n` and `git diff --check` PASS.
- PowerShell runtime was not available on this macOS host; Windows execution
  remains a static script plus final PE proof. Final-binary macOS fresh/update
  execution remains Stage 35.
- SHA-256:
  `install.sh=a74b1fa32c4d2e7bf3bc76a5e07dee8dad140554d3c93e177ef389ec176b90ca`,
  `install.ps1=3284a3a4ce6821b854f0718d84534a0e6f6f8f907d68410c89c553e065b7687c`,
  `install_prompt.md=e04e46b8804b78d7665e61cbfc0515b1604b36a68558081e4168352e4a0bfe79`,
  and
  `audit_v020_installers.sh=d2ea9355f00bdf9c43e1299ac146c0cc32a593fc01f4e469460954c8b2fe1194`.

Handoff:

- Stage 33 is complete. Stage 35 must use the flat final assets for real
  isolated macOS fresh/update proof and retain the honest Windows limitation.

## Stage 34 — reproducible corpus and regression benchmark

Implementation:

- Added a standard-library Python harness and shell runner. The baseline is
  built from a peeled tag archive without changing the worktree; the current
  binary is built from the frozen classified worktree in a separate target.
- Deterministic small, medium, and large corpora contain 100/2,000/10,000
  nodes and 300/6,000/30,000 links, plus equal-size aliases, tags, and sources,
  workflows, tools, failure modes, corrections, MCP profiles, and local
  observability events for RC1.
- Each supported series has three warmups and 20 measured samples. Results use
  process-level wall time, median, and nearest-rank p95. Unsupported baseline
  operations are explicit rather than emulated.

Proof:

- Full duration was 168.615 seconds: tag build 22.47 seconds and current build
  40.17 seconds. There are 68 series, 53 supported, 15 explicitly unsupported,
  and 1,060 measured supported samples.
- Every current full traversal returned exactly 100, 2,000, or 10,000 nodes in
  1, 4, or 20 pages. Logical corpus SHA-256 matches tag/current for all sizes.
- Query recall always selected `Deploy release workflow`; all 60 UI responses
  returned HTTP 200 on `127.0.0.1`; all 60 exports were durable non-empty ZIPs;
  every observability sample added exactly one event; verify stayed clean.
- The report keeps the tag package label `0.1.0`, exposes larger RC1 fixed
  costs where measured, makes no percentage claim, and records the collector
  residual only as an upper bound. Integrity manifest, Python lint/compile,
  shell syntax, and scoped diff check PASS. Final verdict: P1=0, P2=0.
- Evidence hashes:
  `report=9e91da01156be4150b5b85b906d142ac40138e9f2646b4cbced8fcdc6e2e40d3`,
  `harness=ed84b9294cb10f9fe8736bbe8c3890813bd911eb80499c325a0112d1a2ae3c44`,
  `raw_json=f2cb583eedc3f671636f53888f22cd887edea0c8dbb55a58050ac6b77c92fd26`,
  and
  `summary=610eb5eec34ced7a6e9b17cadc724c5eb366132862e59d8093b8f8a8cbeab9ab`.

Handoff:

- Stage 34 is complete. Stage 35 may cite the frozen results but must not
  rewrite or rerun them unless a behavior-affecting source change invalidates
  the current source-tree hash.

## Stage 35 — release integration and final local proof

Release assets:

- Replaced only the tracked legacy nested distribution layout with the exact
  flat contract: `dist/aopmem-darwin-arm64`,
  `dist/aopmem-windows-x86_64.exe`, and `dist/SHA256SUMS`.
- macOS was built with `cargo build --locked --release --target
  aarch64-apple-darwin`, `MACOSX_DEPLOYMENT_TARGET=11.0`, and
  `strip=false`. `file` reports Mach-O arm64, `otool` reports minimum 11.0,
  and `nm` exposes 1,563 global symbols.
- Windows was built from macOS through the existing cargo-xwin flow with
  `target-feature=+crt-static`. `file` reports PE32+ console x86-64.
  `llvm-readobj --coff-imports` reports only `advapi32`,
  `api-ms-win-core-synch-l1-2-0`, `bcryptprimitives`, `KERNEL32`, `ntdll`,
  `shell32`, `userenv`, and `WS2_32`; no VCRUNTIME, MSVCP, UCRTBASE, or
  `api-ms-win-crt` import exists.
- Final hashes are
  `macOS=b32e918d2a44f0767444e09c84c1ed44fe9177709b2d56b2aa89c300081d4308`,
  `Windows=a4e3302d6f26dd9d16387a075189fec51c469aef9b8d9c730f81001b21b2cf57`,
  and `SHA256SUMS=4a3f90601ed03de7fb6f07adeef48271b4d6f96d821aefb5030968b8a318eb5f`.
  `shasum -a 256 -c` passes for both entries.

Fresh installer remediation and proof:

- The first real isolated fresh run exposed one P2: `init` did not create the
  managed adapter block, while the installer accepted doctor exit 0 even when
  `healthy=false`. Both native installers now run
  `init -> adapter seed -> doctor -> verify` and require JSON
  `ok=true`, doctor `healthy=true`, and verify `clean=true`.
- Added installer negatives for rejected adapter success and unhealthy doctor.
  The isolated macOS/static-PowerShell audit now passes 11/11 groups.
- A second real final-binary fresh install in a new temporary home asked the
  existing five semantic questions once, created `AGENTS.md`, returned adapter
  `managed_block=in_sync`, doctor `healthy=true`, and verify `clean=true`.
  No real user home or workspace was read or changed.

Real peeled-v0.1 update proof:

- Extracted the exact tagged macOS binary from `v0.1.0-rc3`; it reports
  `aopmem 0.1.0` and hashes to
  `d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607`.
- In an isolated v0.1 workspace, created 11 nodes, one link, one alias, one
  tag, one source, 12 events, one generated tool contract/tree, and three MCP
  profiles, including explicit workflow and failure-mode fixture rows.
- Common v0.1 columns for nodes, links, aliases, tags, sources, events,
  registries, tool contracts, and MCP profiles have the exact same pre-update,
  post-update, and Online Backup digest:
  `4890a73e51a5e0eeb0e283f3127cd5c05e583f13f518d7aefde95180c1ef7c9f`.
- Generated tool files retain exact digest
  `5d7ffa2a4357d3072b406f154d17e479a4d8a6d227f37df9c678d97a0ad2babb`.
  The artifact payload retains SHA-256
  `b7dfde292eca151e17b48bfa58f7fb397f7789614331d79e4239578aa6d75bad`.
- Migrations are exactly `001,002,003`. Update invoked no `init` or fresh
  adapter seed. The final binary, adapter in-sync state, doctor health, and
  verify cleanliness all pass. Installer and upgrade backups remain present;
  both installer binary backups retain the tagged binary hash. The adapter
  backup retains its exact pre-update hash
  `a033814215e9cd0b2c61fa0e4615f4ef8a99c8a5ebfb821c22ff186b8b665733`.
- The separate observability store exists and contains one
  `update.started` plus one `update.completed` success event.

Observability and UI proof:

- Real `observe status`, `observe report`, and `observe export` succeeded on
  the migrated fixture. The capsule is a durable 10,452-byte ZIP with the
  exact required 12 entries and SHA-256
  `b6e9ba1d81225dbca3ada72c35de43ca723fad7e9b0d85c0b185edc9f6730e9f`.
  A direct canary scan found no Authorization header, vendor token, raw tool
  output, or onboarding body. The deterministic/private export test passes.
- A live final-binary UI bound a random `127.0.0.1` port. Embedded asset,
  overview, and bounded graph returned HTTP 200; an invalid token returned
  404; POST returned 405. The graph returned a 10-node page with
  `nodes_more_results=true`. The server was then stopped.
- All 13 UI HTTP tests pass. Overview, Graph, and Activity screenshot files
  remain true PNGs at exactly 1440x900 with the frozen Stage 30 hashes.

Benchmark parity:

- Recomputed source-tree SHA-256 is exactly
  `91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4`.
  No Rust/Cargo/template behavior changed after the Stage 34 measurement.
- Repeating `cargo build --release --locked` reproduced the measured binary
  SHA-256 exactly at
  `12ec578dc641373e0e22b67f548fb2862620571eb9777026304cd46e10427e61`.
  The explicit-target flat macOS asset is not byte-identical; the report now
  states this and makes no asset-specific performance claim.

Final local gate:

- `cargo fmt --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS, no issues.
- `cargo build --locked`: PASS.
- `cargo test --locked`: 575/575 PASS.
- `cargo test --tests --locked`: 575/575 PASS.
- `scripts/dev_verify.sh`: PASS after its own build, 575-test run, positive
  CLI flow, negative flow, and adapter drift proof.
- Focused v0.1 schema and full upgrade migration fixtures: 1/1 plus 1/1 PASS.
- `scripts/audit_v020_installers.sh`: 11/11 groups PASS.
- Deterministic export/redaction: 1/1 focused test plus real capsule PASS.
- UI HTTP: 13/13 focused tests plus live final-binary proof PASS.
- Windows PowerShell 5.1 script static audit, PE type/import proof, release
  checksum verification, forbidden drift scan, and `git diff --check`: PASS.
- Independent drift scan found one generated
  `scripts/__pycache__/benchmark_v020.cpython-314.pyc` created by the Stage 35
  source-digest check. It was deleted; repeat `find`, scoped status, and final
  forbidden scan confirm no `__pycache__` or `.pyc` remains.
- Windows binary execution was not possible on the macOS host. No native
  Windows runtime claim is made; it is the intended Windows dogfood step.

Required negative map:

- Collector unavailable/write failure:
  `collector_unavailable_warns_but_does_not_change_success_status`,
  `collector_failure_warns_once_and_never_changes_core_result`, and
  `corrupt_store_and_insert_failure_are_best_effort`.
- Invalid bundle id and continuation binding:
  `global_bundle_id_parser_accepts_only_canonical_lowercase_uuid_v4` and
  `continuation_requires_exact_global_bundle_match_before_workspace_access`.
- Export redaction:
  `export_is_exact_deterministic_private_and_accepts_empty_optional_text`.
- UI token/bind/method:
  `routing_authenticates_before_method_and_uses_exact_asset_allowlist` and
  `bind_config_rejects_every_address_except_exact_ipv4_localhost`, plus live
  invalid-token 404 and POST 405.
- Mandatory overflow and duplicate-free continuation:
  `mandatory_overflow_json_has_ids_and_no_partial_success_data` and
  `task_recall_continues_three_pages_with_same_bundle_exact_dedup_and_budget`.
- Tool timeout/output overflow:
  `tool_timeout_json_uses_exact_code_and_typed_bounded_details`,
  `tool_output_overflow_json_uses_exact_code_and_no_raw_output`, and the
  descendant-kill tests.
- Migration failure/rollback, corrupt DB, insufficient disk, adapter drift,
  and old-binary backup failure:
  `migration_failure_rolls_back_keeps_backup_and_records_exact_failed_workspace`,
  both rollback-recovery tests,
  `corrupt_database_is_an_exact_per_workspace_blocker`,
  `insufficient_disk_is_reported_without_hiding_workspace_schema`, and
  `disk_corrupt_adapter_drift_and_old_binary_backup_fail_before_core_mutation`.

Freeze hashes:

- `install.sh=451d88696b635aab4a6c8bc5e2de69bb4abc61108ffcaeda8fa6b1b91f180ca2`.
- `install.ps1=d4966dbd3c750e11b972b5f090ddb481d51ae13a124ba07c41ac053e27e6ceca`.
- `install_prompt.md=e84d74b2231af06b8cae6868993e2032b5cc34bc058514a4d81e422feeb33cc0`.
- `audit_v020_installers.sh=249a9394536eccd9e23228219d6a15e73def175419a5f343755532ae02e4aec4`.
- `build_macos_arm.sh=4e17438e7d54e528c9ec05f79e3c9bb9d2c47449f3cc8b38273c32930d67a1c8`.
- `build_windows_x64_from_macos.sh=eb3b437ea754ef91266956250c19e625a77a7f958d7573f6a3fe7a4df445d3ad`.
- `DEPS_JUSTIFICATION.md=019702ad404083b19ac2b1c82188c583da0b18a13a2346ac6f91b50385c34fca`.

Handoff:

- Production, installers, build scripts, dependencies, and flat assets are
  frozen. Independent final audit rechecked the source digest, fmt, clippy,
  575 tests, 11 installer groups, asset hashes/types/imports, dependencies,
  forbidden drift, required paths, ledger JSON, and final diff.
- The only independent finding was generated Python-cache P3 drift. It was
  deleted and the repeat scan passed.
- Final independent verdict: OPEN P1=0, P2=0, P3=0. The candidate is ready
  for macOS and Windows dogfood. Stop conditions remain in force.

## Stage 36 — v0.2.0-rc3 WAL upgrade remediation

Trigger:

- Native Windows dogfood reached the authoritative `upgrade plan` with exit
  `0`, `ok=true`, `writes_performed=false`, and `ready=false`.
- Workspace `p-sit-warranty-5708363a` was blocked by
  `UNSAFE_DATABASE_SIDECAR` for a zero-byte `aopmem.sqlite-wal`.
- No apply, migration, or binary publication started. Existing workspaces and
  durable full backup remained intact.

Accepted remediation:

- Add explicit `upgrade prepare --all-workspaces --json`.
- Create per-workspace backup before SQLite checkpoint.
- Use `PRAGMA wal_checkpoint(TRUNCATE)` through canonical storage.
- Fail closed on busy/incomplete checkpoint and unsafe paths.
- Remove only verified empty direct-child WAL/SHM after connection close.
- Preserve schema version and logical memory.
- Keep `upgrade plan` strictly read-only.
- Change installer order to `backup -> prepare -> plan -> apply -> publish`.
- Allow noncanonical SQLite-backed v0.1 binary through warning plus
  authoritative workspace compatibility checks.

Proof status:

| Proof | Status |
|---|---|
| No-sidecar idempotence | PASS |
| Zero-byte WAL + SHM | PASS |
| Committed non-empty WAL | PASS; committed row preserved |
| Active/busy database | PASS |
| Busy/incomplete checkpoint | PASS |
| Backup-before-checkpoint | PASS |
| Backup failure prevents checkpoint | PASS |
| Symlink/reparse fail-closed | PASS |
| Multi-workspace stable order | PASS |
| Schema unchanged | PASS |
| Logical rows/tools/MCP/artifacts preserved | PASS |
| Plan no-write after prepare | PASS |
| Installer prepare-before-plan/no intervening DB read | PASS |
| Noncanonical v0.1 warning/compatibility | PASS |
| macOS isolated fresh install | PASS |
| macOS isolated zero-WAL update | PASS |
| macOS isolated committed-WAL update | PASS |
| Windows PE/static installer audit | PASS |
| Native Windows retry | PENDING dogfood after release |

Command proof:

- `cargo fmt --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo build --locked`: PASS.
- `cargo test --locked`: 609/609 PASS.
- `cargo test --tests --locked`: 609/609 PASS.
- `scripts/dev_verify.sh`: PASS with 609 tests.
- Focused prepare tests: 9/9 PASS.
- Upgrade tests: 32/32 PASS; independent broad `upgrade` filter: 33/33 PASS.
- `scripts/audit_v020_installers.sh`: 11/11 PASS.
- Installer shell syntax and `git diff --check`: PASS.
- Release-scope version drift scan: clean.

Real proof:

- Root:
  `/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T/aopmem-rc3-real-proof.tPGZ5n`.
- Fresh macOS install: PASS.
- Real v0.1 zero-byte WAL update: PASS.
- Real v0.1 committed-WAL update: PASS; committed rule remains available
  through recall after migration.
- Both update traces prove:
  `process gate -> binary/full-home backups -> asset download -> stage ->
  prepare -> plan -> apply -> publish -> health`.
- No doctor, verify, recall, or observe event appears between prepare and plan.

Release evidence:

- Version: `v0.2.0-rc3`.
- macOS arm64 SHA-256:
  `8bc4d3a7ae38253c1a6e4c653292cf954fb2c8eee916c69a03c6dc5e2484261c`.
- Windows x64 SHA-256:
  `ed59be73d99efd2c1a4fe99e50b85e8b6ce8e8a73b7ff0c96b5327e1c2d39477`.
- `SHA256SUMS` SHA-256:
  `e871a6dcd53909e80b0cd7e1ab794e300fd4faeb961e3bbb83a770c4a8fcb871`.
- `shasum -a 256 -c`: PASS for both assets.
- Final independent audit: open P1=0, P2=0, P3=0.
- Candidate is ready for prerelease and repeat native Windows dogfood.

## Stage 37 — v0.2.0-rc4 Windows backup remediation

Trigger:

- native Windows rc3 apply returned `WORKSPACE_BACKUP_FAILED`,
  `Access is denied. (os error 5)`;
- SQLite had produced a 184,320-byte file;
- no migration or binary publication started.

Code-level root cause:

- rc3 called `File::open(destination_path)?.sync_all()?`;
- this passed a read-only Windows handle to `FlushFileBuffers`;
- Windows can return `ERROR_ACCESS_DENIED=5` for that handle.

Accepted remediation:

- unique temporary backup in the final directory;
- explicit SQLite backup, destination, and source handle closure;
- temporary read-only schema/quick/table validation;
- writable anchored file flush;
- existing anchored Windows no-replace publish;
- final read-only validation and metadata proof;
- pending migration marker only after final backup success;
- typed 11-phase diagnostics with retained evidence;
- fresh create-new rc4 run root; failed rc3 root retained;
- installer one-apply contract preserved.

Command proof:

- `cargo fmt --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo build --locked`: PASS.
- `cargo test --locked`: 616/616 PASS.
- `cargo test --tests --locked`: 616/616 PASS.
- `scripts/dev_verify.sh`: PASS.
- focused upgrade filter: 38/38 PASS.
- installer audit: 11/11 PASS.
- `git diff --check`: PASS.

Real macOS proof:

- root: `/tmp/aopmem-rc4-real-proof.LmFqjz`;
- two workspaces in stable order;
- clean, zero-WAL, and committed-WAL preparation: PASS;
- plan no-write: PASS;
- one apply, both schemas `001 -> 003`: PASS;
- both published backups read-only reopen with schema `001`: PASS;
- logical rows, tools, MCP profiles, and artifacts: PASS;
- audit-state preservation: focused upgrade tests PASS;
- adapter, doctor, verify, recall, observe status/report/export: PASS;
- failed rc3 sentinel root retained: PASS.

Release evidence:

- version: `v0.2.0-rc4`;
- macOS arm64:
  `4812ca6c798cd2460b4b9da468e5f99f433a68907dc40eba257b88d197886e4e`;
- Windows x64:
  `e4442fd06622a6b94f997e23b67a55753f1d841f6570ef20ac72b99083a6cc1c`;
- `SHA256SUMS`:
  `bd456530a2e716575cc97d7306c155f39e583dc36d9ea387b7769ae89bcf4da8`;
- Windows unchanged-source rebuild: identical hash;
- Windows test target `cargo xwin test --no-run`: PASS;
- PE32+ x86-64 and static runtime boundary: PASS;
- open P1=0, P2=0;
- native Windows rc4 runtime retry: PENDING.
