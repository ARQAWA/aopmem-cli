# PROOF LOG — AOPMem v0.1

## STAGE_001

Status: `DONE`

Requirements covered:

- `REQ-DERC-001`
- `REQ-DERC-002`
- `REQ-DERC-003`
- `REQ-DERC-004`

Files changed:

- `.devplan/MASTER_SPEC.md`
- `.devplan/FINAL_DECISION_LOG.md`
- `.devplan/REQUIREMENTS_MATRIX.md`
- `.devplan/STAGE_GRAPH.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/BLOCKERS.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_001.md`
- `DEPS_JUSTIFICATION.md`

Commands run:

```text
git status --short
test -d .devplan
test -f .devplan/EXECUTION_LEDGER.md
test -f .devplan/CURRENT_STAGE.md
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
cmp -s aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md .devplan/FINAL_DECISION_LOG.md
cmp -s aopmem_v0_1_final_orchestrated_pack/reference/REQUIREMENTS_MATRIX.md .devplan/REQUIREMENTS_MATRIX.md
cmp -s aopmem_v0_1_final_orchestrated_pack/reference/STAGE_GRAPH.md .devplan/STAGE_GRAPH.md
wc -c .devplan/BLOCKERS.md
```

Results:

```text
PASS test -d .devplan
PASS test -f .devplan/EXECUTION_LEDGER.md
PASS test -f .devplan/CURRENT_STAGE.md
PASS json valid
PASS final decision log copied
PASS requirements matrix copied
PASS stage graph copied
PASS .devplan/BLOCKERS.md is empty
```

Final `git status --short`:

```text
?? .DS_Store
?? .devplan/
?? DEPS_JUSTIFICATION.md
?? aopmem_v0_1_final_orchestrated_pack/
```

Known limitations:

- No Rust crate exists yet. This is expected before Stage 002.
- No cargo checks are required or useful in Stage 001.

Next stage:

- `STAGE_002`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_001
Requirements checked: REQ-DERC-001..REQ-DERC-004
Findings: No issues found. Stage stayed inside allowed files.
Required fixes: None.
Out-of-scope drift: None.
Decision log conflicts: None.
Recommended next action: Mark STAGE_001 as VERIFIED, then run STAGE_002.
```

## STAGE_002

Status: `DONE`

Requirements covered:

- `REQ-PROD-001`

Files changed:

- `Cargo.toml`
- `src/main.rs`
- `src/adapter/mod.rs`
- `src/artifacts/mod.rs`
- `src/audit/mod.rs`
- `src/cli/mod.rs`
- `src/install/mod.rs`
- `src/recall/mod.rs`
- `src/reflection/mod.rs`
- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `src/tools/mod.rs`
- `src/verify/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_002.md`

Commands run:

```text
git status --short
rg --files -g 'Cargo.toml' -g 'src/**' -g 'tests/**'
rtk cargo build
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 001 state
PASS no existing Cargo.toml/src/tests found before Stage 002
PASS rtk cargo build
PASS removed generated Cargo.lock and target because they are outside Stage 002 scope
PASS json valid
```

Final `git status --short`:

```text
?? .DS_Store
?? .devplan/
?? Cargo.toml
?? DEPS_JUSTIFICATION.md
?? aopmem_v0_1_final_orchestrated_pack/
?? src/
```

Known limitations:

- The crate is a minimal skeleton only.
- No CLI routing or commands are implemented yet.
- No dependencies were added.
- Stage 002 is not audited yet.

Next stage:

- `STAGE_003`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_002
Requirements checked: REQ-PROD-001
Findings: Single crate is named aopmem. Module skeleton matches decision log.
Required fixes: None.
Out-of-scope drift: None from Stage 002 implementation.
Decision log conflicts: None.
Recommended next action: Mark STAGE_002 as VERIFIED, then run STAGE_003.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit `cargo build` generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 002 allowed files.
- `target/` is generated build output.

## STAGE_003

Status: `DONE`

Requirements covered:

- `REQ-PROD-001`

Files changed:

- `Cargo.toml`
- `DEPS_JUSTIFICATION.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_003.md`

Dependencies added:

- `clap`
- `directories`
- `rusqlite` with `bundled`
- `serde` with `derive`
- `serde_json`
- `thiserror`

Commands run:

```text
git status --short
rtk cargo build
grep -q "crate:" DEPS_JUSTIFICATION.md
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 002 state
PASS rtk cargo build
PASS grep -q "crate:" DEPS_JUSTIFICATION.md
PASS removed generated Cargo.lock and target because they are outside Stage 003 scope
PASS json valid
```

Known limitations:

- Dependencies are declared and justified only.
- No CLI routing, JSON envelope code, path resolver, or SQLite code is implemented yet.
- Stage 003 is not audited yet.

Next stage:

- `STAGE_004`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_003
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None.
Decision log conflicts: None.
Recommended next action: Mark Stage 003 as VERIFIED, then continue with STAGE_004.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit `cargo build` generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 003 allowed files.
- `target/` is generated build output.

## STAGE_022

Status: `DONE`

Requirements covered:

- `REQ-MEM-004`

Files changed:

- `src/recall/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_022.md`

Commands run:

```text
git status --short
rg --files | rg '(^|/)FINAL_DECISION_LOG\.md$|(^|/)NON_NEGOTIABLE_SCOPE\.md$|(^|/)DERC_PROTOCOL\.md$'
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_022.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_021.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery used reference files from aopmem_v0_1_final_orchestrated_pack/reference/
PASS recovery git status matched prior handoff note that repo content is currently untracked in git
PASS cargo test: 62 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- This stage changes normal recall only.
- Lint/audit mode behavior is intentionally not implemented yet.

Next stage:

- `STAGE_023`

Audit follow-up:

- Adjusted `.devplan/**` head-state bookkeeping to keep Stage 022 as `DONE`
  with audit `PENDING`.
- Removed wording that made Stage 023 look ready before Stage 022 audit pass.
- Verified `.devplan/EXECUTION_LEDGER.json` with `python3 -m json.tool`.

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_022
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None found.
Recommended next action: Mark STAGE_022 as VERIFIED, then move CURRENT_STAGE to STAGE_023.
```

## STAGE_004

Status: `DONE`

Requirements covered:

- `REQ-CLI-001`

Files changed:

- `src/cli/mod.rs`
- `src/main.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_004.md`

Commands run:

```text
git status --short
rtk cargo build
rtk cargo test
./target/debug/aopmem --help
./target/debug/aopmem --version
./target/debug/aopmem tool run
rm -rf target Cargo.lock
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 003 state
PASS rtk cargo build
PASS rtk cargo test: 3 passed
PASS --help exits 0 and is served by clap
PASS --version exits 0 and prints aopmem 0.1.0
PASS stub command exits 7 and prints NOT_IMPLEMENTED
PASS removed generated Cargo.lock and target because they are outside Stage 004 scope
PASS json valid
```

Known limitations:

- CLI commands are routing stubs only.
- JSON envelope is not implemented. It is Stage 005 scope.
- No business logic, storage, recall, install, tool, adapter, or audit behavior is implemented.
- Stage 004 is not audited yet.

Next stage:

- `STAGE_005`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_004
Requirements checked: REQ-CLI-001
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found for Stage 004.
Decision log conflicts: None.
Recommended next action: Mark STAGE_004 as VERIFIED, then continue with STAGE_005.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit checks generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 004 allowed files.
- `target/` is generated build output.

## STAGE_005

Status: `DONE`

Requirements covered:

- `REQ-CLI-002`
- `REQ-CLI-003`
- `REQ-CLI-004`

Files changed:

- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_005.md`

Commands run:

```text
git status --short
rtk cargo test
rtk cargo run -- --json tool run
rm -rf target Cargo.lock
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 004 state
PASS rtk cargo test: 6 passed
PASS JSON command proof: exit 7, stdout is valid JSON envelope
PASS JSON envelope contains ok, command, data, warnings, errors, meta.version
PASS removed generated Cargo.lock and target because they are outside Stage 005 scope
PASS json valid
```

Known limitations:

- CLI command business logic remains stubbed.
- Stub commands return `NOT_IMPLEMENTED` with exit code `7`.
- Clap help and version remain clap human output.
- No storage/path logic was added.

Audit patch:

```text
rtk cargo test
rtk cargo run -- --json nope
./target/debug/aopmem --json nope
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf target Cargo.lock
git status --short
```

Audit patch results:

```text
PASS rtk cargo test: 7 passed
PASS --json invalid args proof: exit 2, stdout is valid JSON envelope
PASS direct binary proof: exit 2, stdout JSON only, stderr empty
PASS json valid
PASS removed generated Cargo.lock and target because they are outside Stage 005 scope
```

Next stage:

- `STAGE_006`

Re-audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_005 re-audit after patch.
Requirements checked: REQ-CLI-002, REQ-CLI-003, REQ-CLI-004.
Findings: None. Previous finding is fixed.
Required fixes: None.
Out-of-scope drift: None from Stage 005 patch.
Decision log conflicts: None.
Recommended next action: Mark STAGE_005 as VERIFIED, then continue STAGE_006.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Re-audit checks generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 005 allowed files.
- `target/` is generated build output.

## STAGE_006

Status: `DONE`

Requirements covered:

- `REQ-PROD-002`
- `REQ-STORAGE-001`

Files changed:

- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_006.md`

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
rtk cargo test
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 005 state
PASS rtk cargo test: 10 passed
PASS removed generated Cargo.lock and target because they are outside Stage 006 scope
PASS json valid
```

Known limitations:

- Workspace key generation is not implemented. It belongs to Stage 007.
- Directory creation is not implemented. It belongs to Stage 008.
- SQLite connection and pragmas are not implemented. They belong to Stage 009.

Next stage:

- `STAGE_007`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_006
Requirements checked: REQ-PROD-002, REQ-STORAGE-001
Findings: None.
Required fixes: None.
Out-of-scope drift: None.
Decision log conflicts: None.
Recommended next action: Mark Stage 006 as VERIFIED, then continue STAGE_007.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit `cargo test` generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 006 allowed files.
- `target/` is generated build output.

## STAGE_007

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-001`

Files changed:

- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_007.md`

Commands run:

```text
git status --short
rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 006 state
PASS rtk cargo test: 15 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside Stage 007 scope
```

Implementation:

- Added `workspace_key(repo_root)`.
- Added `WorkspaceKeyError`.
- Implemented `<sanitized-repo-folder-name>-<8-char-path-hash>`.
- Hash input is the absolute repo root path.
- API accepts only repo root path, not user project id.
- Added deterministic tests for sanitization, stable hash, path changes,
  non-ASCII fallback, and relative path rejection.

Known limitations:

- Directory creation is not implemented. It belongs to Stage 008.
- SQLite connection and pragmas are not implemented. They belong to Stage 009.

Next stage:

- `STAGE_008`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_007
Findings: None blocking.
Required fixes: None.
Out-of-scope drift: None from Stage 007.
Decision log conflicts: None.
Recommended next action: Mark STAGE_007 as VERIFIED, then continue with STAGE_008.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit `cargo test` generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 007 allowed files.
- `target/` is generated build output.

## STAGE_008

Status: `DONE`

Requirements covered:

- `REQ-PROD-002`
- `REQ-STORAGE-001`

Files changed:

- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_008.md`

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status matched expected Stage 007 state
PASS rtk cargo test: 18 passed
PASS removed generated Cargo.lock and target because they are outside Stage 008 scope
PASS json valid
PASS final rtk cargo test after test robustness patch: 18 passed
PASS final cleanup removed generated Cargo.lock and target
```

Implementation:

- Added `WorkspacePaths`.
- Added explicit `ensure_global_dirs(paths)`.
- Added explicit `ensure_workspace_dirs(paths, workspace_key)`.
- Created global dirs: `bin`, `skills`, `templates`, `workspaces`.
- Created workspace dirs: `tools`, `artifacts`, `audit-git`, `runtimes`, `logs`.
- Creation is idempotent.
- Tests use temporary `AOPMEM_HOME`.
- No repo-local `.aopmem` is created.
- No SQLite file is created.
- No CLI wiring is added.

Known limitations:

- SQLite connection and pragmas are not implemented. They belong to Stage 009.
- Directory creation is exposed only as direct storage module functions.

Next stage:

- `STAGE_009`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_008
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None.
Decision log conflicts: None.
Recommended next action: Mark Stage 008 as VERIFIED, then start STAGE_009.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit `cargo test` generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 008 allowed files.
- `target/` is generated build output.

## STAGE_009

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-001`

Files changed:

- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_009.md`

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status did not contradict Stage 009; repo content is
     currently untracked in git
PASS rtk cargo test: 19 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 009 scope
PASS json valid
```

Implementation:

- Added `WorkspacePaths::db()` pointing to `<workspace root>/aopmem.sqlite`.
- Added `open_workspace_db(workspace_paths)`.
- Applied `foreign_keys`, `journal_mode`, and `busy_timeout` pragmas on open.
- Added a test that opens the DB file and verifies the pragma values.
- Did not create migrations, schema tables, node/link CRUD, or CLI wiring.
- Did not create repo-local `.aopmem`.

Known limitations:

- Migrations and schema tables are not implemented. They belong to Stage 010.
- No node/link CRUD.
- No CLI wiring.

Next stage:

- `STAGE_010`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_009
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark Stage 009 as VERIFIED, then continue with STAGE_010.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit `cargo test` generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 009 allowed files.
- `target/` is generated build output.

## STAGE_010

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-004`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_010.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
git status --short
```

Results:

```text
PASS recovery git status did not contradict Stage 010; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 22 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 010 scope
PASS json valid
```

Implementation:

- Added `schema_migrations` table creation.
- Added `001_init` migration skeleton marker.
- Added idempotent migration application.
- `open_workspace_db` applies SQLite pragmas, then migrations.
- Added tests for migration marker creation and idempotent re-run.
- Did not implement concrete nodes/links/aliases/events/registries tables.
- Did not add CRUD or CLI wiring.
- Did not create repo-local `.aopmem`.

Known limitations:

- Stage 010 is migration skeleton only.
- Concrete schema tables belong to Stage 011+.
- No CRUD.
- No CLI wiring.

Next stage:

- `STAGE_011`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_010
Findings: None.
Required fixes: None.
Out-of-scope drift: None found for Stage 010.
Decision log conflicts: None.
Recommended next action: Mark STAGE_010 as VERIFIED, then continue with STAGE_011.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit `cargo test` generated `Cargo.lock` and `target/`.
- `Cargo.lock` was not part of Stage 010 allowed files.
- `target/` is generated build output.

## STAGE_011

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-004`
- `REQ-CLI-005`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_011.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 011; repo content is
     currently untracked in git
PASS cargo fmt
FAIL first rtk cargo test found one Rust lifetime error in list_nodes
PASS final rtk cargo test: 28 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 011 scope
PASS json valid
```

Implementation:

- Added `nodes` table in migration `001_init`.
- Added indexes for `node_type` and `status`.
- Added storage models and functions for node create/get/list.
- Added validation for allowed node types and statuses from storage spec.
- Required `source_ref`, `confidence`, and `trust_level` for active nodes.
- Added CLI commands `node create`, `node get`, and `node list`.
- Kept JSON envelope behavior for `--json`.
- CLI uses storage API and does not expose direct SQL.
- Did not add links, aliases, tags, sources, events, FTS, semantic/vector
  search, MCP server, or markdown import/export.

Known limitations:

- Links table and commands belong to Stage 012.
- Aliases, tags, and sources belong to Stage 013.
- Events audit table belongs to Stage 014.
- FTS belongs to Stage 016.

Next stage:

- `STAGE_012`

## STAGE_012

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-004`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_012.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 012; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 32 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 012 scope
PASS json valid
```

Implementation:

- Added `links` table in migration `001_init`.
- Added indexes for source node, target node, and link type.
- Added foreign keys from links to nodes with `ON DELETE RESTRICT`.
- Added storage models and functions for link add/list.
- Validated source and target node IDs exist before insert.
- Validated non-empty link type.
- Added CLI commands `link add` and `link list`.
- Kept JSON envelope behavior for `--json`.
- CLI uses storage API and does not expose direct SQL.
- Did not add aliases, tags, sources, events, FTS, semantic/vector search,
  MCP server, or markdown import/export.

Known limitations:

- Link type is validated as non-empty, but no allowed link type enum exists yet
  in the v0.1 specs.
- Aliases, tags, and sources belong to Stage 013.
- Events audit table belongs to Stage 014.
- FTS belongs to Stage 016.

Next stage:

- `STAGE_013`

## STAGE_013

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-004`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_013.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
cargo fmt && rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 013; repo content is
     currently untracked in git
PASS cargo fmt
FAIL first rtk cargo test found three Rust lifetime errors in metadata list
     functions
PASS final rtk cargo test: 36 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 013 scope
PASS json valid
```

Implementation:

- Added `aliases`, `tags`, and `sources` tables.
- Added node/value indexes and uniqueness per node.
- Added storage add/list APIs for aliases, tags, and sources.
- Added validation for existing node ID and non-empty values.
- Added CLI `alias add/list`, `tag add/list`, and `source add/list`.
- Preserved node/link behavior and JSON envelope.
- Stored aliases in `aliases.alias` so Stage 016 FTS can index them later.
- Did not add events, registries, FTS, semantic/vector search, MCP server, or
  markdown import/export.

Known limitations:

- Metadata commands are intentionally minimal.
- `node update` remains not implemented.
- Events audit table belongs to Stage 014.
- Registries belong to Stage 015.
- FTS table/indexing belongs to Stage 016.

Next stage:

- `STAGE_014`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_013
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None found.
Recommended next action: Mark Stage 013 as VERIFIED, then continue to STAGE_014.
```

Audit cleanup:

```text
rm -rf Cargo.lock target /tmp/aopmem-stage013-audit.8ULybV
```

Reason:

- Audit `cargo test` generated build output.

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_012
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None found.
Recommended next action: Mark Stage 012 as VERIFIED, then continue to STAGE_013.
```

Audit cleanup:

```text
rm -rf Cargo.lock target /var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T/aopmem-stage012-audit-v1lfnkg9
```

Reason:

- Audit `cargo test` generated build output.

- Audit CLI proof created temporary AOPMEM_HOME.

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_011
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark Stage 011 as VERIFIED, then continue with STAGE_012.
```

Audit cleanup:

```text
rm -rf Cargo.lock target /tmp/aopmem-stage011-audit.yGreba
```

Reason:

- Audit `cargo test` / `cargo run` generated build output.
- Audit CLI proof created temporary AOPMEM_HOME under `/tmp`.
## STAGE_014 — Implement events audit table

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-005`

Files changed:

- `src/schema/mod.rs`
- `src/audit/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_014.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
cargo fmt && rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 014; repo content is
     currently untracked in git
PASS cargo fmt
FAIL first rtk cargo test found one Rust lifetime error in audit list_events
PASS final rtk cargo test: 40 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 014 scope
PASS json valid
```

Implementation:

- Added `events` table to migration `001_init`.
- Added required event fields: `type`, `timestamp`, and `source`.
- Added `subject_kind` and `subject_id` for node/link event subjects.
- Added indexes for event type, timestamp, and subject lookup.
- Added `src/audit` API for `node.created` and `link.created` events.
- Patched `src/storage` to auto-record `node.created` and `link.created`
  after successful node/link creation.
- Added focused storage tests for automatic audit events.
- Added validation for event type, source, subject kind, and subject ID.
- Added tests for schema creation and audit event recording.

Known limitations:

- Stage 014 status remains `DONE`.
- Stage 014 audit remains `PENDING`.

Next stage:

- `STAGE_015`

Re-audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_014
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark Stage 014 audit as passed/VERIFIED, then continue to STAGE_015.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Re-audit `cargo test` generated build output.

## STAGE_015 — Implement registries base

Status: `DONE`

Requirements covered:

- `REQ-TOOLS-002`
- `REQ-TOOLS-005`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_015.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 015; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 47 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 015 scope
PASS json valid
```

Implementation:

- Added SQLite registry base tables to migration `001_init`.
- Added `registries`, `tool_contracts`, and `mcp_profiles` tables.
- Added minimal storage API for MCP profile create/get/list.
- Added `aopmem mcp list`, `aopmem mcp add`, and `aopmem mcp get`.
- Preserved the JSON envelope for success and error output.
- Added schema, storage, and CLI parsing tests.

Known limitations:

- Generated tool create/run/validate are not implemented. They belong to
  Stage 032+.
- MCP installation is not implemented.
- Corporate MCP registry starts empty.
- FTS is not implemented. It belongs to Stage 016.

Next stage:

- `STAGE_016`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_015
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark Stage 015 as VERIFIED, then continue with STAGE_016.
```

Audit cleanup:

```text
rm -rf Cargo.lock target /tmp/aopmem-stage015-audit.SwBTUP
```

Reason:

- Audit `cargo test/run` generated build output.
- Audit CLI proof created temporary AOPMEM_HOME.

## STAGE_016 — Add FTS5 table and indexing hooks

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-003`

Files changed:

- `src/schema/mod.rs`
- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_016.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 016; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 50 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 016 scope
PASS json valid
```

Implementation:

- Added SQLite FTS5 virtual table `fts_nodes`.
- Indexed `title`, `summary`, `body`, and `aliases`.
- Added storage refresh hook after successful node create.
- Added storage refresh hook after successful alias create.
- Added schema and storage tests for FTS table creation and indexing hooks.
- Did not add BM25 search CLI, recall logic, semantic search, vectors, or
  embeddings.

Known limitations:

- Node update is not implemented yet, so only node create and alias create
  hooks are implemented and tested.
- BM25 search CLI belongs to Stage 019.
- Recall logic belongs to Stage 017+.

Next stage:

- `STAGE_017`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_016
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark Stage 016 as VERIFIED, then continue to STAGE_017.
```

Audit cleanup:

```text
rm -rf Cargo.lock target
```

Reason:

- Audit `cargo test` generated build output.

## STAGE_017 — Implement structured recall base

Status: `DONE`

Requirements covered:

- `REQ-MEM-002`

Files changed:

- `src/recall/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_017.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status did not contradict Stage 017; repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 52 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 017 scope
PASS json valid
```

Implementation:

- Added structured recall bundle builder.
- Grouped project profiles, gates, and workflows by node status.
- Wired `aopmem recall` to `storage::list_nodes`.
- Returned the bundle through the existing JSON envelope.
- Did not use FTS/BM25, graph traversal, semantic search, vectors, or
  embeddings.

Known limitations:

- Graph traversal belongs to Stage 018.
- FTS/BM25 fallback belongs to Stage 019.
- Hunch selection and shaping/limits belong to Stage 020+.
- Deprecated/superseded exclusion belongs to Stage 022.

Next stage:

- `STAGE_018`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_017
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark Stage 017 as VERIFIED, then continue to STAGE_018.
```

## STAGE_018 — Implement graph traversal

Status: `BLOCKED`

Requirements covered:

- `REQ-MEM-002`

Files changed:

- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_018.md`
- `.devplan/BLOCKERS.md`

Commands run:

```text
git status --short
rtk cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Results:

```text
PASS recovery git status matches prior handoff note that repo content is
     currently untracked in git
PASS rtk cargo test: 52 passed
PASS removed generated Cargo.lock and target because they are outside
     Stage 018 scope
PASS json valid
```

Blocker:

- Stage 018 requires expanding recall through links from selected
  workflow/tool/rules.
- `src/cli/mod.rs::run_recall` calls `storage::list_nodes` and passes only
  nodes to `recall::build_structured_bundle(nodes)`.
- `storage::list_links` exists, but Stage 018 forbids editing `src/cli/**`,
  `src/storage/**`, and `src/schema/**`.
- Implementing traversal only inside `src/recall/**` would not affect the
  product recall path.

Known limitations:

- Graph traversal is not implemented.
- Deprecated/superseded exclusion is not implemented.

Next stage:

- Do not start `STAGE_019`.
- Resolve the `STAGE_018` blocker before starting `STAGE_019`.

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_018
Findings: BLOCKED decision is valid.
Required fixes: None after matching EXECUTION_LEDGER.json `blocked` to true.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Keep STAGE_018 current until scope is expanded or a
patch stage allows recall CLI wiring to pass links.
```

## STAGE_018 PATCH — Unblock graph traversal

Status: `DONE`

Requirements covered:

- `REQ-MEM-002`

Files changed:

- `src/recall/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_018.md`
- `.devplan/BLOCKERS.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
```

Results:

```text
PASS recovery git status matched prior note that repo content is currently
     untracked in git
PASS cargo fmt
PASS rtk cargo test: 54 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside patch
     scope
```

Implementation:

- Added `build_structured_bundle_with_links(nodes, links)`.
- Kept `build_structured_bundle(nodes)` compatible for Stage 017 callers.
- Added bounded directed traversal from `workflow`, `rule`, and
  `tool_contract` roots.
- Limited traversal depth to 2.
- Excluded `deprecated` and `superseded` nodes from traversal.
- Added `linked_nodes` as an additive recall output field.
- Wired `aopmem recall` to call existing `storage::list_links`.
- Did not edit `src/storage/**` or `src/schema/**`.

Known limitations:

- FTS/BM25 fallback belongs to Stage 019.

Next stage:

- `STAGE_019`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_018 patch
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found beyond user-approved minimal CLI recall wiring.
Decision log conflicts: None.
Recommended next action: Mark Stage 018 as VERIFIED, then continue to STAGE_019.
```

## STAGE_019 — FTS/BM25 fallback

Status: `BLOCKED`

Requirements covered:

- `REQ-STORAGE-003`
- `REQ-MEM-002`

Files changed:

- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_019.md`
- `.devplan/BLOCKERS.md`

Commands run:

```text
git status --short
cargo fmt
rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 54 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage
     scope
```

Blocker:

- Stage 019 requires FTS5/BM25 fallback results ordered by `bm25`
  ascending.
- `src/cli/mod.rs::run_recall` passes only nodes and links into
  `src/recall/**`.
- `src/recall/**` has no SQLite connection, no recall query input, no FTS
  result rows, and no public storage FTS search API.
- Editing `src/cli/**`, `src/storage/**`, or `src/schema/**` is forbidden by
  the user unless the stage is impossible in allowed scope.
- A fallback implemented only in `src/recall/**` would not use SQLite
  FTS5/BM25 and would be a fake implementation.

Known limitations:

- FTS/BM25 fallback is not implemented.
- No product files were changed.
- No semantic/vector search was added.

Next stage:

- Do not start `STAGE_020`.
- Keep `STAGE_019` current until scope is expanded or a patch stage allows
  public FTS search wiring.

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_019
Findings: BLOCKED decision is valid.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Keep STAGE_019 current until scope is expanded or a
patch stage allows public FTS search wiring.
```

## STAGE_019 PATCH — FTS/BM25 fallback

Status: `DONE`

Requirements covered:

- `REQ-STORAGE-003`
- `REQ-MEM-002`

Dependency scope use:

- Used DERC Recall stage dependency scope.
- Added minimal public FTS search API in `src/storage/mod.rs`.
- Added minimal recall CLI wiring in `src/cli/mod.rs`.
- Did not edit `src/schema/**`; existing `fts_nodes` table was used.
- Did not start Stage 020.

Files changed:

- `src/storage/mod.rs`
- `src/recall/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_019.md`
- `.devplan/BLOCKERS.md`

Commands run:

```text
git status --short
cargo fmt && rtk cargo test
cargo fmt && rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is
     currently untracked in git
FAIL first rtk cargo test: Rust lifetime error in new FTS search method
PASS cargo fmt
PASS rtk cargo test: 57 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage
     scope
```

Implementation:

- Added `storage::search_nodes_fts(connection, query, limit)`.
- Used SQLite FTS5 `MATCH`.
- Ordered fallback search by `bm25(fts_nodes)` ascending, then `nodes.id`.
- Excluded `deprecated` and `superseded` nodes in storage search.
- Added additive recall JSON field `fts_fallback`.
- Added fallback only when structured recall has fewer than 3 selected nodes.
- Derived fallback query deterministically from structured recall titles.
- De-duplicated fallback nodes already present in structured recall output.
- Added focused tests for storage FTS search and recall fallback filtering.
- No semantic, vector, Mem0, Hindsight, MCP, CI, or markdown export work.

Known limitations:

- Hunch selection belongs to Stage 020.

Next stage:

- `STAGE_020`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_019 patch
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found; dependency scope stayed minimal.
Decision log conflicts: None.
Recommended next action: Mark Stage 019 as VERIFIED, then continue to STAGE_020.
```

## Stage 020 Proof

Status: `DONE`

Requirements covered:

- `REQ-MEM-003`

Dependency scope use:

- Not used.
- Product changes stayed in `src/recall/**`.
- Did not edit `src/cli/**`, `src/storage/**`, or `src/schema/**`.

Files changed:

- `src/recall/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_020.md`

Commands run:

```text
git status --short
cargo fmt && rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 59 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage
     scope
```

Implementation:

- Added additive recall bundle field `hunches`.
- Added compact `RecallHunch` output with required `source_node_id`.
- Kept hunch output non-authoritative by omitting full node body.
- Selected 1-3 hunches from FTS fallback candidates with linked signal metadata.
- Selection is deterministic by linked workflow/tool/failure_mode signal, FTS rank,
  `updated_at` hotness, then source node id.
- Prioritized `failure_mode`, `tool_contract`, and `workflow`.
- Added focused unit tests for hunch ordering, cap, source node id, and linked
  signal metadata.
- No semantic/vector search, Mem0, Hindsight, custom MCP, CI, or markdown
  export work was added.

Known limitations:

- Recall bundle shaping and limits belong to Stage 021.

Next stage:

- `STAGE_021`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_020
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark Stage 020 as VERIFIED, then continue to STAGE_021.
```

## Stage 021 Proof

Status: `DONE`

Requirements covered:

- `REQ-MEM-002`
- `REQ-MEM-003`

Dependency scope use:

- Not used.
- Product changes stayed in `src/recall/**`.
- Did not edit `src/cli/**`, `src/storage/**`, or `src/schema/**`.

Files changed:

- `src/recall/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_021.md`

Commands run:

```text
git status --short
cargo fmt && rtk cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is
     currently untracked in git
PASS cargo fmt
PASS rtk cargo test: 61 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage
     scope
```

Implementation:

- Added additive recall bundle field `compact`.
- Added compact sections for applicable workflows, active gates, tool contracts,
  MCP profiles, project profile facts, relevant corrections/lessons, hunches,
  and source refs.
- Added compact source ref, confidence, and trust level markers.
- Added deterministic caps for compact sections and source refs.
- Kept existing JSON fields intact.
- Added focused unit tests for compact limits, source/trust metadata, and max
  hunch count.
- No semantic/vector search, Mem0, Hindsight, custom MCP, CI, or markdown
  export work was added.

Known limitations:

- Deprecated/superseded exclusion hardening belongs to Stage 022.

Next stage:

- `STAGE_022`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_021
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark Stage 021 as VERIFIED, then continue to STAGE_022.
```

## DERC UPDATE — AUTO_PATCH_WINDOW

Status: `DONE`

Reason:

- The previous DERC model stopped on ordinary adjacent-layer wiring.
- The new model must support non-stop execution to the final stage while
  preserving audit safety.

Files changed:

- `aopmem_v0_1_final_orchestrated_pack/RUN_FIRST.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/ORCHESTRATOR_EXECUTION_MODEL.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md`
- `.devplan/FINAL_DECISION_LOG.md`
- `.devplan/MASTER_SPEC.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/PROOF_LOG.md`

Implementation:

- Added `AUTO_PATCH_WINDOW`.
- Added dependency scope matrix with `primary` and `auto_patch_allowed`.
- Added status values `NEEDS_AUTO_PATCH` and `AUTO_PATCHED`.
- Restricted `BLOCKED` to real blockers: out-of-scope, forbidden feature,
  architecture decision, high-risk contract, missing dependency justification,
  file outside dependency matrix, or external-system side effect.
- Required audit after every auto patch before `VERIFIED`.
- Required proof/handoff records for every auto patch.

Checks:

```text
rg AUTO_PATCH_WINDOW / status rules
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Result:

```text
PASS DERC rule added to prompt-pack and devplan recovery files
PASS EXECUTION_LEDGER.json valid
```

## DERC UPDATE — SUBAGENT MODEL POLICY

Status: `DONE`

Reason:

- Subagents must run on `gpt-5.4`, not a default or newer model selection.
- Reasoning effort must stay role-based and unchanged.

Files changed:

- `aopmem_v0_1_final_orchestrated_pack/RUN_FIRST.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/ORCHESTRATOR_EXECUTION_MODEL.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md`
- `aopmem_v0_1_final_orchestrated_pack/README.md`
- `aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_*.md`
- `aopmem_v0_1_final_orchestrated_pack/audit_prompts/*.md`
- `.devplan/FINAL_DECISION_LOG.md`
- `.devplan/MASTER_SPEC.md`
- `.devplan/PROOF_LOG.md`

Implementation:

- Set implementation subagents to `gpt-5.4` with `reasoning_effort=medium`.
- Set patch subagents to `gpt-5.4` with `reasoning_effort=medium`.
- Set audit subagents to `gpt-5.4` with `reasoning_effort=high`.
- Required explicit model and effort on subagent launch.

Checks:

```text
rg active prompt-pack files for legacy subagent model references
python3 -m json.tool .devplan/EXECUTION_LEDGER.json
```

Result:

```text
PASS active prompt-pack and devplan recovery files use gpt-5.4 subagents
PASS historical handoff records were left unchanged
PASS EXECUTION_LEDGER.json valid
```

## DERC UPDATE — RUST SUBAGENT SKILL POLICY

Status: `DONE`

Reason:

- Rust subagents should use the available `rust-skills` guidance.
- The rule must stay minimal and must not expand non-Rust stages.

Files changed:

- `aopmem_v0_1_final_orchestrated_pack/RUN_FIRST.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/ORCHESTRATOR_EXECUTION_MODEL.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md`
- `.devplan/MASTER_SPEC.md`
- `.devplan/PROOF_LOG.md`

Implementation:

- Added a Rust subagent skill policy for implementation, audit, and patch
  subagents.
- Scoped the rule only to `Cargo.toml`, `src/**/*.rs`, and `tests/**/*.rs`.
- Explicitly excluded bookkeeping-only `.devplan/**` patches from the rule.
- Kept the existing `gpt-5.4` model policy unchanged.

Checks:

```text
read rust-skills SKILL.md
review prompt-pack files for subagent launch policy
```

Result:

```text
PASS rust-skills policy added as a thin-slice process improvement
PASS non-Rust stages and bookkeeping patches remain unchanged
```

## DERC UPDATE — ADAPTER AUTO_PATCH_WINDOW

Status: `DONE`

Reason:

- Adapter stages can require minimal CLI wiring to expose `adapter` commands.
- This is an adjacent-layer scope mismatch and should not stop the goal.

Files changed:

- `aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md`
- `.devplan/MASTER_SPEC.md`
- `.devplan/PROOF_LOG.md`

Implementation:

- Added `Adapter stage` to the dependency scope matrix.
- Allowed only minimal `src/cli/**` auto patch for adapter stages.
- Kept the rule narrow; no other dependency scopes were expanded.

Checks:

```text
review Stage 024 prompt against current cli routing
review dependency matrix in DERC_PROTOCOL
```

Result:

```text
PASS adapter -> cli auto patch added as the minimal scope needed for Stage 024
PASS no unrelated matrix expansion was introduced
```

## DERC UPDATE — INSTALL TO STORAGE AUTO_PATCH_WINDOW

Status: `DONE`

Reason:

- Install stages already depend on workspace/db helpers implemented in
  `src/storage/**`.
- Stage 026 audit found a real verification blocker in shared test process
  state across install, cli, and storage tests.

Files changed:

- `aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md`
- `.devplan/MASTER_SPEC.md`
- `.devplan/PROOF_LOG.md`

Implementation:

- Extended Install stage auto patch scope with minimal `src/storage/**`.
- Kept the scope narrow and tied only to install/workspace/db reuse and
  verification synchronization.

Checks:

```text
review Stage 026 audit finding against install/storage ownership
review dependency matrix in DERC_PROTOCOL
```

Result:

```text
PASS install -> storage auto patch added for minimal Stage 026 fix scope
PASS no unrelated install scope was expanded
```

## STAGE_023

Status: `DONE`

Requirements covered:

- `REQ-INSTALL-005`

Files changed:

- `src/adapter/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_023.md`

Commands run:

```text
pwd
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_023.md
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/PROOF_LOG.md
rg --files aopmem_v0_1_final_orchestrated_pack/reference .devplan/HANDOFFS | rg 'FINAL_DECISION_LOG\.md$|NON_NEGOTIABLE_SCOPE\.md$|DERC_PROTOCOL\.md$|STAGE_022\.md$'
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_022.md
git status --short
rg -n "REQ-INSTALL-005|managed block|adapter seed|AGENTS.md|AOPMEM:BEGIN" .devplan aopmem_v0_1_final_orchestrated_pack/reference aopmem_v0_1_final_orchestrated_pack/stage_prompts
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md
sed -n '1,180p' aopmem_v0_1_final_orchestrated_pack/reference/CLI_CONTRACT.md
sed -n '80,120p' .devplan/MASTER_SPEC.md
cargo test
cargo fmt
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery git status matched prior handoff note that repo content is currently untracked in git
PASS cargo test: 68 passed
PASS cargo fmt
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- Default adapter detection in this stage seeds only Codex/OpenAI `AGENTS.md`.
- Managed block body is a minimal inline seed; template files belong to a later
  stage.
- `adapter sync` and `adapter status` remain unimplemented.

Next stage:

- `STAGE_024`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_023
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None found.
Recommended next action: Mark STAGE_023 as VERIFIED, then continue with STAGE_024.
```

## STAGE_024

Status: `DONE`

Requirements covered:

- `REQ-INSTALL-005`
- `REQ-VERIFY-005`

AUTO_PATCH_WINDOW:

- Used: `yes`
- Reason: minimal Adapter -> CLI wiring was required to expose
  `aopmem adapter sync` and `aopmem adapter status`.
- Touched dependency files: `src/cli/mod.rs`

Files changed:

- `src/adapter/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_024.md`

Commands run:

```text
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_024.md
sed -n '1,320p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_023.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
git status --short
cargo test
rm -rf Cargo.lock target
test ! -e Cargo.lock && echo NO_CARGO_LOCK
test ! -e target && echo NO_TARGET
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery used prompt-pack reference files and matched prior handoff note that repo content is currently untracked in git
PASS cargo test: 74 passed
PASS AUTO_PATCH_WINDOW stayed inside src/cli/mod.rs only
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Implementation:

- Added managed block status detection in `src/adapter/mod.rs` with
  `missing`, `in_sync`, and `drifted` states.
- Added adapter sync logic that appends a missing block, preserves an in-sync
  block, and replaces only a drifted managed block.
- Kept damaged or duplicated markers as fail-fast conflict/drift errors.
- Wired `aopmem adapter sync` and `aopmem adapter status` in `src/cli/mod.rs`
  under the allowed Adapter -> CLI AUTO_PATCH_WINDOW.
- Added focused adapter and CLI tests for sync/status behavior and parsing.

Known limitations:

- Managed block body is still the minimal inline seed from Stage 023.
- This stage does not implement the later broader drift-check work planned for
  `STAGE_048`.

Next stage:

- `STAGE_025`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_024
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None found.
Recommended next action: Mark STAGE_024 as VERIFIED, then allow STAGE_025 to start.
```

## STAGE_025

Status: `DONE`

Requirements covered:

- `REQ-PROD-002`
- `REQ-INSTALL-001`

Files changed:

- `src/install/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_025.md`

Commands run:

```text
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_025.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/PROOF_LOG.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_024.md
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo state and Stage 024 handoff
PASS cargo test: 77 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Known limitations:

- Global install check is presence-only for this stage.
- It does not install, repair, or initialize workspace state.
- Template content is not validated yet.

Next stage:

- `STAGE_026`

Audit result:

```text
AUDIT RESULT: PASS
Stage audited: STAGE_025
Findings: No blocking findings.
Required fixes: None.
Out-of-scope drift: None found.
Decision log conflicts: None.
Recommended next action: Mark STAGE_025 as VERIFIED, then allow STAGE_026 to start.
```

## STAGE_026

Status: `DONE`

Requirements covered:

- `REQ-INSTALL-001`
- `REQ-STORAGE-001`

Files changed:

- `src/install/mod.rs`
- `src/cli/mod.rs`
- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_026.md`

Commands run:

```text
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_026.md
sed -n '1,240p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
git status --short --branch
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_025.md
sed -n '1,260p' .devplan/MASTER_SPEC.md
sed -n '1,260p' .devplan/REQUIREMENTS_MATRIX.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/CLI_CONTRACT.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md
sed -n '60,140p' aopmem_v0_1_final_orchestrated_pack/reference/PRODUCT_SPEC.md
sed -n '60,140p' aopmem_v0_1_final_orchestrated_pack/reference/STORAGE_AND_SQLITE_SPEC.md
cargo test
RUST_TEST_THREADS=1 cargo test
rm -rf Cargo.lock target
find . -maxdepth 2 \( -name Cargo.lock -o -name target \) -print
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery used prompt-pack reference files and matched the prior untracked repo state
PASS implemented workspace init inside allowed stage scope only
PASS cargo test: 80 passed
PASS RUST_TEST_THREADS=1 cargo test: 80 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS no Cargo.lock or target remained after cleanup
```

Implementation:

- Added idempotent workspace init in `src/install/mod.rs`.
- Init now creates global dirs, workspace dirs, and per-workspace SQLite DB.
- Seeded default active base nodes for kernel contract, gates, and default
  communication style.
- Wired `aopmem init` in `src/cli/mod.rs` with stable JSON output and short
  non-JSON output.
- Added focused tests for init creation, idempotency, and CLI routing.

Known limitations:

- Stage 027 semantic onboarding flow was not started.

Patch note:

- Stage 026 audit finding was the missing shared process-wide test lock for
  env/current-dir mutation across install, cli, and storage tests.
- Minimal AUTO_PATCH_WINDOW fix was applied in `src/storage/mod.rs` only.
- After the fix, both `cargo test` and `RUST_TEST_THREADS=1 cargo test`
  passed with 80 tests.

Next stage:

- `STAGE_027`

## STAGE_027

Status: `DONE`

Requirements covered:

- `REQ-INSTALL-001`
- `REQ-INSTALL-002`
- `REQ-INSTALL-003`
- `REQ-INSTALL-004`

Files changed:

- `src/install/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_027.md`

Commands run:

```text
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_027.md
sed -n '1,240p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
git status --short
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,260p' .devplan/MASTER_SPEC.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_026.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/CLI_CONTRACT.md
sed -n '1,240p' .devplan/REQUIREMENTS_MATRIX.md
cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo state and Stage 026 handoff
PASS implemented install flow inside allowed scope only
PASS cargo test: 82 passed
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS json valid
```

Watchdog recovery:

- The original Stage 027 worker exceeded the 10-minute watchdog window and was
  closed.
- The orchestrator continued the same thin slice locally, verified the current
  workspace state, reran `cargo test`, revalidated
  `.devplan/EXECUTION_LEDGER.json`, and removed `Cargo.lock` and `target`.
- No parallel retry agent was kept open.

Implementation:

- Added interactive install flow that asks only the final 5 semantic blocks.
- Kept technical detection silent and reused existing workspace/db init wiring.
- Seeded semantic answers as active `preference` and `project_profile` nodes.
- Kept `--json` init output stable while sending prompts to prompt output.
- Added focused tests for prompt collection, idempotent semantic seeding, and
  CLI init test input wiring.

Known limitations:

- Rerunning init does not update existing semantic nodes yet; it reuses them.
- Understand Anything setup and Codebase Memory MCP registration are still
  deferred to later stages.

Next stage:

- `STAGE_028`

## STAGE_028

Status: `DONE`

Requirements covered:

- `REQ-INSTALL-003`

Files changed:

- `src/install/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_028.md`

Commands run:

```text
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_028.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_027.md
sed -n '1,260p' .devplan/FINAL_DECISION_LOG.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
git status --short
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md
sed -n '260,330p' .devplan/FINAL_DECISION_LOG.md
cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo state and Stage 027 handoff
PASS implemented .understand.docs creation inside allowed scope only
PASS cargo test: 83 passed
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS json valid
```

Implementation:

- Added `.understand.docs` creation only when Understand Anything is enabled.
- Created `SCHEMA.md` and the required directory scaffold:
  `index`, `log`, `raw`, `concepts`, `entities`, `architecture`, `domain`,
  `adr`, `module-notes`, `testing-model`, `maps`.
- Added repo-local exclude support through `.git/info/exclude`.
- Added focused tests for enabled creation, disabled skip behavior, and
  exclude-entry idempotency.

Known limitations:

- `SCHEMA.md` is a minimal scaffold file only.
- Later stages still need Understand registry/profile and Codebase Memory MCP
  setup.

Next stage:

- `STAGE_029`

## DERC PATCH — 2026-06-08

Status: `APPLIED`

Scope:

- DERC / devplan / prompt patch only
- no product-code change was applied by this patch

Commands run:

```text
sed -n ... aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n ... aopmem_v0_1_final_orchestrated_pack/reference/ORCHESTRATOR_EXECUTION_MODEL.md
sed -n ... aopmem_v0_1_final_orchestrated_pack/RUN_FIRST.md
sed -n ... aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n ... aopmem_v0_1_final_orchestrated_pack/reference/STAGE_GRAPH.md
sed -n ... aopmem_v0_1_final_orchestrated_pack/audit_prompts/AUDIT_STAGE_TEMPLATE.md
sed -n ... .devplan/MASTER_SPEC.md
sed -n ... .devplan/FINAL_DECISION_LOG.md
sed -n ... .devplan/STAGE_GRAPH.md
sed -n ... .devplan/REQUIREMENTS_MATRIX.md
python3 stage prompt actor-line rewrite
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
```

Results:

```text
PASS switched from audit-after-every-stage to cumulative milestone audit every 5 stages
PASS new normal completion state is DONE_LOCAL_CHECKS_PASSED
PASS VERIFIED now means covered by cumulative milestone audit
PASS historical VERIFIED/PASS statuses through STAGE_028 were kept intact
PASS verified_through_stage is now recorded as STAGE_028
PASS next cumulative audit is scheduled after STAGE_030 with focus STAGE_026-STAGE_030
PASS dependency scope matrix updated for CLI, Storage, Recall, Tool, Install, Reflection, and Verification stages
PASS audit template and stage prompts updated to the new cadence
PASS json valid
```

## STAGE_029

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-INSTALL-003`

Files changed:

- `src/install/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_029.md`

Commands run:

```text
git status --short
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_029.md
sed -n '1,240p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_028.md
cargo test
rm -rf Cargo.lock target
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo state and Stage 028 handoff
PASS implemented Understand MCP profile registration with installed/missing status
PASS best-effort storage failure does not fail install flow
PASS cargo test: 85 passed
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS json valid
```

Implementation:

- Added best-effort `Understand Anything` MCP profile registration inside the
  install flow.
- Historical Stage 029 status wording is superseded by
  `PATCH_GLOBAL_AUDIT_GA_001_DECISION`: final optional MCP statuses are
  `disabled`, `installed`, `missing`, and `configured_unverified`.
- Kept profile registration non-fatal by swallowing MCP storage errors.
- Added focused install tests for enabled, disabled, and best-effort failure
  behavior.

Known limitations:

- This stage registers only the Understand profile.
- Codebase Memory MCP profile remains for Stage 030.

Next stage:

- `STAGE_030`

## STAGE_030

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-INSTALL-004`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/install/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_030.md`

Commands run:

```text
git status --short
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,260p' .devplan/HANDOFFS/STAGE_029.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_030.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline and Stage 029 handoff
PASS implemented Codebase Memory MCP profile registration with installed/missing status
PASS best-effort storage failure does not fail install flow
PASS cargo test: 86 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Implementation:

- Added best-effort `Codebase Memory MCP` profile registration inside the
  install flow.
- Historical Stage 030 status wording is superseded by
  `PATCH_GLOBAL_AUDIT_GA_001_DECISION`: final optional MCP statuses are
  `disabled`, `installed`, `missing`, and `configured_unverified`.
- Kept profile registration non-fatal by swallowing MCP storage errors.
- Added focused install tests for enabled, disabled, and best-effort failure
  behavior.

Known limitations:

- This stage registers only the Codebase Memory MCP profile.
- Corporate MCP registry CRUD remains for Stage 031.

Next stage:

- `STAGE_031`

## STAGE_034

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements:

- `REQ-TOOLS-003`

AUTO_PATCH_WINDOW:

- Used: `yes`
- Files: `src/cli/mod.rs`
- Reason: minimal CLI wiring for `aopmem tool validate <tool-id>`

Commands run:

```text
git status --short
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
ls -1t .devplan/HANDOFFS | head -n 5
sed -n '1,260p' .devplan/HANDOFFS/STAGE_033.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_034.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md
sed -n '1,260p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 98 passed
PASS AUTO_PATCH_WINDOW used only for minimal src/cli tool validate wiring
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Implementation:

- Added `aopmem tool validate <tool-id>` CLI parsing and routing.
- Added tool validation helper that checks SQLite registry presence,
  validates `tool.json`, and ensures the referenced executable file exists.
- Reused existing contract validation for required fields, `side_effects`,
  and examples.
- Added focused success and negative tests for validate behavior.

Known limitations:

- This stage does not implement `aopmem tool run`.
- Validation checks executable presence, not execution behavior.

Next stage:

- `STAGE_035`

## STAGE_032

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-TOOLS-001`
- `REQ-TOOLS-002`
- `REQ-TOOLS-003`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/tools/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_032.md`

Commands run:

```text
git status --short
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,220p' .devplan/EXECUTION_LEDGER.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_031.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_032.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md
cargo test tools:: -- --nocapture
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test tools::: 3 passed
PASS cargo test: 91 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Implementation:

- Added the base `tool.json` model in `src/tools/mod.rs`.
- Added canonical SQLite create/get/list helpers for `tool_contracts`.
- Added workspace `tool.json` write/read helpers under `tools/<tool-id>/`.
- Added focused round-trip tests for SQLite and local manifest I/O.
- Did not edit `src/storage/**` or `src/cli/**`.
- Did not start Stage 033.

Known limitations:

- This stage does not implement create-draft, validate, or run behavior.

Next stage:

- `STAGE_033`

## STAGE_033

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-TOOLS-001`
- `REQ-TOOLS-003`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/cli/mod.rs`
- `src/tools/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_033.md`

Commands run:

```text
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
ls -1t .devplan/HANDOFFS | head -n 5
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_033.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md
git status --short
sed -n '1,260p' .devplan/HANDOFFS/STAGE_032.md
sed -n '1,260p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 94 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Implementation:

- Implemented `aopmem tool create-draft` with `--id`, `--name`, optional
  `--entrypoint`, optional `--owner-workflow`, and safe defaults for
  `--side-effects` / `--approval-requirement`.
- Added draft creation helper that creates `tools/<tool-id>/`,
  `tools/<tool-id>/bin/`, `tools/<tool-id>/runtime/`, writes `tool.json`,
  and stores the canonical record in SQLite.
- Fixed generated draft contracts to status `draft` only.
- Added a focused CLI parse test and an end-to-end draft creation test.
- Did not edit `src/storage/**`.
- Did not start Stage 034.

Known limitations:

- This stage creates draft tool contracts only.
- Tool validate and tool run remain unimplemented.

Next stage:

- `STAGE_034`

## STAGE_031

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-TOOLS-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/cli/mod.rs`
- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_031.md`

Commands run:

```text
git status --short
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,240p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,240p' .devplan/HANDOFFS/STAGE_030.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_031.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline and Stage 030 handoff
PASS cargo test: 88 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Implementation:

- Confirmed existing MCP storage and CLI wiring already satisfies the thin
  slice for create/get/list/add behavior.
- Added focused CLI tests for empty corporate registry behavior.
- Added focused CLI tests for corporate MCP add/get/list behavior with
  persisted `side_effects` and `approval_requirement`.
- Added explicit storage assertions for MCP profile policy fields.

Known limitations:

- Corporate MCP registry remains allowed to be empty by contract.
- No tool registry or Stage 032 implementation work was started.

Next stage:

- `STAGE_032`

## STAGE_030_MILESTONE_AUDIT

Status: `PASS`

Coverage:

- `STAGE_001`–`STAGE_030`

Focus:

- `STAGE_026`–`STAGE_030`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
```

Results:

```text
PASS cumulative milestone audit through STAGE_030
PASS cargo test: 86 passed
PASS json valid
PASS repo returned to the same untracked baseline after cleanup
PASS verified_through_stage can advance to STAGE_030
```

Findings:

- No blocking findings.
- No out-of-scope drift found in `STAGE_029` or `STAGE_030`.
- `REQ-INSTALL-003` and `REQ-INSTALL-004` are covered and audit-ready.

Next stage:

- `STAGE_031`

## STAGE_035

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-TOOLS-004`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/cli/mod.rs`
- `src/tools/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_035.md`

Commands run:

```text
git status --short
sed -n '1,220p' /Users/arkadijcukavin/.agents/skills/rust-skills/SKILL.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,260p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,220p' .devplan/HANDOFFS/STAGE_034.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_035.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 103 passed
PASS safe tool run parse and end-to-end execution coverage passed
PASS unsafe tool run block coverage passed with EXIT_UNSAFE_ACTION_BLOCKED
PASS AUTO_PATCH_WINDOW not used
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Implementation:

- Added `tool run` CLI parsing for `aopmem tool run <tool-id> -- <args...>`.
- Executed generated tools only through registry + `tool.json` runtime
  metadata.
- Allowed only safe tools for now:
  `side_effects in {none, local_read}` and `approval_requirement == none`.
- Blocked all other tools with a structured unsafe-action error.
- Captured child process stdout/stderr in a deterministic JSON result record.
- Added focused tool-layer and CLI tests for parse, safe execution, and
  blocked unsafe execution.
- Did not start Stage 036.

Known limitations:

- Approval flag handling remains for a later stage.
- `tool list` and `tool get` remain out of scope here.

Next stage:

- `STAGE_036`

## STAGE_045

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-MEM-002`
- `REQ-TOOLS-005`

Files changed:

- `src/storage/mod.rs`
- `src/recall/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_045.md`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 144 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Implementation:

- Added derived source hierarchy parsing from `source_ref` with stable
  root/path/leaf/priority fields in `src/storage/**`.
- Added least-privilege metadata helpers for MCP profiles and node-backed
  tool/MCP records without changing the SQLite schema.
- Extended recall compact output with additive source hierarchy and
  least-privilege metadata fields.
- Updated compact node selection, FTS fallback ordering, and hunch selection
  to prefer higher-priority sources before trust/confidence tie-breaks.
- Added focused storage and recall tests for parsing, metadata extraction, and
  source-priority behavior.
- Did not start Stage 046.

Next stage:

- `STAGE_046`

## STAGE_046

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-VERIFY-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/verify/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_046.md`

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS cargo test: 149 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS repo matched the expected untracked baseline
```

Implementation:

- Added `aopmem verify` lint command.
- Added verify report checks for duplicate ids, broken links, deprecated
  active links, missing source, missing summary, and missing gates.
- Added focused tests for clean and dirty workspace lint behavior.
- Did not start Stage 047.

## STAGE_047

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-VERIFY-004`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_047.md`

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS cargo test: 155 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS repo matched the expected untracked baseline
```

Implementation:

- Added negative CLI tests for missing workspace, bad node type, bad status,
  duplicate id, broken link, unsafe tool run, and deprecated recall exclusion.
- Kept product code unchanged in this stage.
- Did not start Stage 048.

## STAGE_048

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-VERIFY-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/verify/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_048.md`

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS cargo test: 158 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS repo matched the expected untracked baseline
```

Implementation:

- Added drift checks for adapter block drift, schema drift, and forbidden
  feature terms in code paths.
- Added focused unit tests for all three drift cases.
- Did not start Stage 049.

## STAGE_049

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-VERIFY-001`
- `REQ-VERIFY-002`
- `REQ-VERIFY-003`
- `REQ-VERIFY-004`
- `REQ-VERIFY-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `scripts/dev_verify.sh`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_049.md`

Commands run:

```text
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS bash scripts/dev_verify.sh
PASS cargo build inside script
PASS cargo test inside script: 158 passed
PASS clean CLI proof inside script
PASS negative checks inside script
PASS drift check inside script
PASS repo matched the expected untracked baseline
```

Implementation:

- Added deterministic local verification script `scripts/dev_verify.sh`.
- Script covers build, test, clean CLI proof, negative checks, and drift
  scenario.
- Did not start Stage 050.

## STAGE_050

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-VERIFY-003`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `scripts/dev_verify.sh`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_050.md`

Commands run:

```text
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS bash scripts/dev_verify.sh
PASS cargo build inside script
PASS cargo test inside script: 158 passed
PASS runtime proof scenario inside script
PASS repo matched the expected untracked baseline
```

Implementation:

- Extended `scripts/dev_verify.sh` with runtime proof scenario coverage.
- Covered init workspace, node create, recall, isolated hunch fixture, tool
  create-draft, artifacts cleanup, doctor, negative checks, and drift check.
- Did not start Stage 051.

## STAGE_050_MILESTONE_AUDIT

Status: `PASS`

Coverage:

- `STAGE_001`–`STAGE_050`

Focus:

- `STAGE_046`–`STAGE_050`

Commands run:

```text
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
```

Results:

```text
PASS cumulative milestone audit through STAGE_050
PASS bash scripts/dev_verify.sh
PASS cargo build inside script
PASS cargo test inside script: 158 passed
PASS verified_through_stage can advance to STAGE_050
```

Findings:

- Stage 046 verify lint command covers required issue classes.
- Stage 047 scope is aligned through explicit prompt allowance for
  `src/cli/**` testability wiring in the in-file CLI test module layout.
- Stage 048 drift check covers adapter block drift, schema drift, and
  forbidden feature terms.
- Stage 049 and Stage 050 proof script coverage passed end to end.

Next stage:

- `STAGE_051`

## STAGE_051

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-INSTALL-001`
- `REQ-INSTALL-002`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `install/v0.1/install_prompt.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_051.md`

Commands run:

```text
test -f install/v0.1/install_prompt.md
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS install/v0.1/install_prompt.md exists
PASS json valid
PASS repo matched the expected untracked baseline
```

Implementation:

- Added the final install prompt file.
- Prompt keeps technical detection silent and asks only 5 semantic questions.
- Did not start Stage 052.

## STAGE_055_MILESTONE_AUDIT

Status: `PASS`

Coverage:

- `STAGE_001`–`STAGE_055`

Focus:

- `STAGE_051`–`STAGE_055`

Commands run:

```text
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
```

Results:

```text
PASS cumulative milestone audit through STAGE_055
PASS bash scripts/dev_verify.sh
PASS verified_through_stage can advance to STAGE_055
PASS objective can be marked complete after writeback
```

Findings:

- Stage 051-055 match prompt scope and handoff scope.
- Final install prompt, templates, docs, build script, dist binary, and proof
  artifacts are present.
- No DERC blocker remained in the final batch.

## STAGE_052

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-INSTALL-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `docs/stage_052_templates.md`
- `templates/managed-block/AGENTS.managed-block.md`
- `templates/understand-docs/SCHEMA.md`
- `templates/skills/memory-keeper/SKILL.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_052.md`

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS cargo test: 158 passed
PASS json valid
PASS repo matched the expected untracked baseline
```

Implementation:

- Added managed block template.
- Added `.understand.docs` schema template.
- Added Memory Keeper skill contract template.
- Added a short docs index.
- Did not start Stage 053.

## STAGE_053

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-PROD-004`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `scripts/build_macos_arm.sh`
- `dist/aopmem-darwin-arm64/aopmem`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_053.md`

Commands run:

```text
bash scripts/build_macos_arm.sh || true
ls -l dist/aopmem-darwin-arm64/aopmem
file dist/aopmem-darwin-arm64/aopmem
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS build script ran
PASS dist/aopmem-darwin-arm64/aopmem exists
PASS file reports Mach-O 64-bit executable arm64
PASS json valid
PASS repo matched the expected untracked baseline
```

Implementation:

- Added local macOS ARM build script.
- Produced binary at `dist/aopmem-darwin-arm64/aopmem`.
- Did not start Stage 054.

## STAGE_054

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-DERC-001`
- `REQ-DERC-002`
- `REQ-DERC-003`
- `REQ-DERC-004`
- `REQ-DERC-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `.devplan/REQUIREMENTS_MATRIX.md`
- `proof/stage_054_requirements_traceability.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_054.md`

Commands run:

```text
git status --short
python3 - <<'PY'
from pathlib import Path
import re

matrix_text = Path('.devplan/REQUIREMENTS_MATRIX.md').read_text()
all_reqs = sorted(set(re.findall(r'REQ-[A-Z-]+-\\d{3}', matrix_text)))

handoff_reqs = {}
for path in sorted(Path('.devplan/HANDOFFS').glob('STAGE_*.md')):
    text = path.read_text()
    match = re.search(r'Requirements(?: covered)?:\\n\\n((?:- `[^`]+`\\n)+)', text)
    reqs = re.findall(r'`([^`]+)`', match.group(1)) if match else []
    handoff_reqs[path.stem] = reqs

traceability_only = {
    'REQ-PROD-003': ['STAGE_023', 'STAGE_024', 'STAGE_026'],
    'REQ-PROD-005': ['STAGE_001', 'STAGE_048', 'STAGE_054'],
    'REQ-STORAGE-002': ['STAGE_009', 'STAGE_035', 'STAGE_037'],
    'REQ-MEM-001': ['STAGE_052'],
    'REQ-REFLECT-005': ['STAGE_042', 'STAGE_043'],
    'REQ-DERC-005': ['STAGE_005', 'STAGE_010', 'STAGE_015', 'STAGE_020', 'STAGE_025', 'STAGE_030', 'STAGE_035', 'STAGE_040', 'STAGE_045', 'STAGE_050', 'STAGE_054'],
}

covered = set()
for reqs in handoff_reqs.values():
    covered.update(reqs)
covered.update(traceability_only)

missing = [req for req in all_reqs if req not in covered]
assert not missing, missing
assert len([stage for stage in handoff_reqs if stage <= 'STAGE_053']) == 53
print('TRACEABILITY_OK', len(all_reqs), len(handoff_reqs))
PY
python3 - <<'PY'
from pathlib import Path
import re

text = Path('.devplan/REQUIREMENTS_MATRIX.md').read_text()
stage_block = text.split('## Requirement coverage', 1)[0]
stages = sorted(set(re.findall(r'\\| `STAGE_(\\d{3})` \\|', stage_block)))
assert len(stages) == 54, len(stages)
assert stages[0] == '001'
assert stages[-1] == '054'
print('STAGE_ROWS_OK', len(stages), stages[0], stages[-1])
PY
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS traceability audit confirmed 49/49 requirement ids are covered
PASS traceability audit confirmed 53/53 prior stage handoffs were present
PASS 15x checklist is fully PASS with clean drift status
PASS bash scripts/dev_verify.sh
PASS json valid
PASS final git status kept the expected untracked baseline plus Stage 054 bookkeeping files
```

Implementation:

- Expanded `.devplan/REQUIREMENTS_MATRIX.md` with direct stage coverage
  through `STAGE_054`, per-requirement coverage, and traceability-only notes
  for the previously uncovered ids.
- Added `proof/stage_054_requirements_traceability.md` with the 15x checklist,
  coverage summary, and clean drift status.
- Updated ledger, current stage, and handoff bookkeeping for Stage 054 only.
- Did not start Stage 055.

## STAGE_055

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-VERIFY-003`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_055.md`

Commands run:

```text
git status --short
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/NON_NEGOTIABLE_SCOPE.md
sed -n '1,220p' aopmem_v0_1_final_orchestrated_pack/reference/DERC_PROTOCOL.md
sed -n '1,220p' .devplan/CURRENT_STAGE.md
sed -n '1,260p' .devplan/EXECUTION_LEDGER.md
sed -n '1,220p' .devplan/HANDOFFS/STAGE_054.md
sed -n '1,220p' proof/stage_054_requirements_traceability.md
sed -n '1,220p' scripts/dev_verify.sh
bash scripts/dev_verify.sh
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS final decision, scope, and DERC references were reread from the orchestrated pack
PASS bash scripts/dev_verify.sh
PASS final release-candidate proof stayed inside proof/.devplan bookkeeping scope
PASS no out-of-scope features were added in proof or handoff
PASS json valid
PASS final git status kept the expected untracked baseline plus Stage 055 bookkeeping files
```

Implementation:

- Ran the final local release-candidate proof with `bash scripts/dev_verify.sh`.
- Recorded the final proof and release handoff for Stage 055 only.
- Kept the writeback inside `.devplan/**` and did not start any next stage.

## STAGE_044

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-CLI-004`

Files changed:

- `src/cli/mod.rs`
- `src/tools/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_044.md`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 139 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Implementation:

- Added global CLI `--approved` handling and accepted any value that contains
  `+++`.
- Updated `aopmem tool run` policy so external/high-risk actions still block
  without approval and run when valid approval is present.
- Kept safe local tool runs available without approval.
- Added focused CLI and tool tests for blocked and approved execution paths.
- Did not start Stage 045.

Next stage:

- `STAGE_045`

## STAGE_043

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-REFLECT-004`
- `REQ-MEM-005`

AUTO_PATCH_WINDOW:

- Used: `yes`
- Files: `src/cli/mod.rs`
- Reason: route `aopmem reflect proposal apply` through CLI and persist audit
  snapshots

Files changed:

- `src/reflection/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_043.md`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 137 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Implementation:

- Added `apply_proposal` in `src/reflection/**`.
- Auto-applied only low-risk proposal items.
- Kept high-risk items draft.
- Kept dependent low-risk items draft when proposal-local refs were unresolved.
- Stored strict `reflection_apply_v1` receipts with applied indexes, draft
  reasons, created ids, and tracked `session_id`.
- Added minimal CLI routing and end-to-end test coverage for
  `reflect proposal apply`.
- Did not start Stage 044.

Known limitations:

- High-risk items remain in proposal draft state until approval handling lands
  in Stage 044.
- Proposal-local `node_ref` resolution depends on deterministic item order.

## STAGE_042

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-REFLECT-002`
- `REQ-REFLECT-003`

AUTO_PATCH_WINDOW:

- Used: `yes`
- Files: `src/cli/mod.rs`
- Reason: accept `--proposal-file` JSON input for
  `aopmem reflect proposal create`

Files changed:

- `src/reflection/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_042.md`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 134 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Implementation:

- Added structured reflection proposal input/output types and strict stored
  record shape.
- Validated declared low/high risk against supported proposal item types.
- Stored reflection proposals as `reflection_proposal_v1` raw-note records
  with tracked `session_id`.
- Added minimal CLI file intake for `reflect proposal create`.
- Added focused reflection and CLI tests for proposal schema and risk checks.
- Did not start Stage 043.

## STAGE_041

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-REFLECT-001`
- `REQ-REFLECT-003`

Files changed:

- `src/reflection/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_041.md`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 130 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Implementation:

- Implemented `aopmem reflect inventory`.
- Added strict reflection inventory records with `inventory_status` and
  deterministic `reflected_session_ids`.
- Tracked reflected sessions only from owned reflection record summaries.
- Recorded a reflection inventory snapshot as a draft raw-note on each run.
- Refreshed the SQL audit snapshot after successful inventory writes.
- Added focused reflection unit tests and CLI end-to-end coverage.
- Did not start Stage 042.

Known limitations:

- Reflection inventory does not parse arbitrary external chat logs.
- Proposal schema and apply policy remain for Stages 042 and 043.

Next stage:

- `STAGE_042`

## STAGE_040 Audit Patch 001

Status: `DONE_LOCAL_CHECKS_PASSED`

Scope:

- Milestone audit fixups for `STAGE_036`–`STAGE_040`

AUTO_PATCH_WINDOW:

- Used: `yes`
- Reason: milestone audit found narrow wiring/test gaps inside preapproved
  scope for current milestone work

Files changed:

- `src/artifacts/mod.rs`
- `src/cli/mod.rs`

Commands run:

```text
cargo test init_writes_sql_snapshot_under_workspace_audit_git -- --nocapture
cargo test doctor_json_reports_missing_health_for_uninitialized_workspace -- --nocapture
cargo test cleanup_keeps_today_dir_even_when_only_today_exceeds_size_limit -- --nocapture
cargo test
```

Results:

```text
PASS init snapshot test passed
PASS doctor missing JSON test passed
PASS artifacts today-only oversize test passed
PASS cargo test: 127 passed
```

Implementation:

- `init` now refreshes `audit-git/memory.sql` after successful workspace
  initialization writes.
- Artifact cleanup now keeps today's folder even when only today's artifacts
  exceed the byte limit.
- Added focused CLI JSON proof for missing doctor state through the same doctor
  success envelope used by the CLI path.
- Did not start Stage 041.

## STAGE_040_MILESTONE_AUDIT_RERUN

Status: `PASS`

Coverage:

- `STAGE_001`–`STAGE_040`

Focus:

- `STAGE_036`–`STAGE_040`

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
```

Results:

```text
PASS cumulative milestone audit rerun through STAGE_040
PASS cargo test: 127 passed
PASS json valid
PASS verified_through_stage can advance to STAGE_040
```

Findings:

- No blocking findings remained after `STAGE_040 Audit Patch 001`.
- `init` snapshot wiring now covers successful init-path writes.
- Artifact cleanup keeps today's directory in the today-only oversize case.
- Stage 038 CLI JSON evidence now covers both prepared and missing workspace
  states.

Next stage:

- `STAGE_041`

## STAGE_045_MILESTONE_AUDIT

Status: `PASS`

Coverage:

- `STAGE_001`–`STAGE_045`

Focus:

- `STAGE_041`–`STAGE_045`

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
git status --short
```

Results:

```text
PASS cumulative milestone audit through STAGE_045
PASS cargo test: 144 passed
PASS json valid
PASS repo matched the expected untracked baseline
PASS verified_through_stage can advance to STAGE_045
```

Findings:

- Stage 041 reflection inventory stays limited to strict AOPMem reflection
  records only.
- Stage 042 and Stage 043 `src/cli/**` touches stayed inside allowed
  AUTO_PATCH_WINDOW for reflection stages.
- Stage 044 approval handling accepts any approval text containing `+++` and
  still blocks external/high-risk tool runs without approval.
- Stage 045 source hierarchy and least-privilege metadata remain derived-only;
  no schema migration or out-of-scope work was introduced.
- Stage 045 watchdog timeout was non-blocking because files, proof, and handoff
  were fully written before shutdown.

Next stage:

- `STAGE_046`

## STAGE_040

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements:

- `REQ-MEM-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/cli/mod.rs`
- `src/storage/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_040.md`

Commands run:

```text
git status --short
cargo test
cargo fmt
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched the expected untracked repo baseline
PASS cargo test: 124 passed
PASS cargo fmt
PASS cargo test after fmt: 124 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Implementation:

- Added deterministic `teach start`, `teach add`, `teach propose`, and
  `teach apply` CLI flows.
- Stored teach sessions, materials, proposals, and apply receipts as stable
  draft `raw_note` records with summary markers and link wiring.
- Reused existing node/link/alias/tag/source storage for apply actions.
- Added parse coverage and an end-to-end teach flow test proving explicit
  structured apply behavior without hidden classification.
- Did not start Stage 041.

Next stage:

- `STAGE_041`

## STAGE_039

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-MEM-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_039.md`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery git status matched the expected untracked repo baseline
PASS cargo test: 122 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Implementation:

- Implemented `aopmem remember` through the existing node creation workflow.
- Default `remember <note>` now writes a `raw_note` with `draft` status.
- Explicit remember fields now create structured nodes directly, without
  semantic or LLM classification inside the CLI.
- Added focused parse coverage, end-to-end raw_note coverage,
  end-to-end structured node coverage, and a negative no-classification test.
- Did not start Stage 040.

## STAGE_038

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-VERIFY-003`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/verify/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_038.md`

Commands run:

```text
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
```

Results:

```text
PASS cargo test: 118 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Implementation:

- Added doctor health checks for global dirs, workspace, DB, schema, FTS,
  adapter block, artifacts dirs, and tools dirs.
- Added stable JSON health output and plain-text summary via `aopmem doctor`.
- Added focused verify and CLI tests for ready and missing workspace states.
- Did not start Stage 039.

Next stage:

- `STAGE_039`

## STAGE_036

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements:

- `REQ-ART-001`
- `REQ-ART-002`
- `REQ-ART-003`
- `REQ-ART-004`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/artifacts/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_036.md`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 111 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
```

Implementation:

- Added `src/artifacts/mod.rs` with strict `YYYY-MM-DD` artifact day parsing.
- Added daily artifact folder creation under workspace `artifacts/YYYY-MM-DD`.
- Added cleanup that deletes only dated directories inside `artifacts/`.
- Cleanup removes folders older than the retained 7 calendar days, then prunes
  oldest remaining artifact folders until usage is at or below 1 GB.
- Cleanup ignores non-date artifact-root entries and never touches workspace
  DB, `tools`, `logs`, or `audit-git`.
- Wired `aopmem artifacts cleanup` to run the new cleanup logic and return the
  stable JSON envelope.
- Added focused tests for path creation, age pruning, size pruning, sibling-dir
  safety, command parse, and CLI cleanup execution.
- Did not start Stage 037.

Next stage:

- `STAGE_037`

## STAGE_037

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements:

- `REQ-STORAGE-005`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/audit/mod.rs`
- `src/cli/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/CURRENT_STAGE.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_037.md`

Commands run:

```text
git status --short
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS recovery matched prior untracked repo baseline
PASS cargo test: 114 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside stage scope
PASS final git status kept the expected untracked repo baseline
```

Implementation:

- Added deterministic SQL dump generation in `src/audit/mod.rs`.
- Snapshot writes a text file to workspace `audit-git/memory.sql`.
- Dump includes schema objects and ordered row inserts, and does not place the
  binary SQLite DB in `audit-git`.
- Wired SQL snapshot refresh after successful CLI memory writes:
  node create, link add, alias add, tag add, source add, MCP add, and tool
  create-draft.
- Added focused audit tests for SQL dump content and text snapshot writing.
- Added focused CLI proof that node create writes the snapshot under
  workspace `audit-git`.
- Did not edit `src/storage/**`.
- Did not start Stage 038.

Known limitations:

- Stage 037 provides only local text SQL snapshot generation.
- No git commit flow for `audit-git` was added in this stage.

Next stage:

- `STAGE_038`

## STAGE_035 Audit Patch 001

Status: `DONE_LOCAL_CHECKS_PASSED`

Requirements covered:

- `REQ-TOOLS-004`

AUTO_PATCH_WINDOW:

- Used: `no`

Files changed:

- `src/cli/mod.rs`
- `src/tools/mod.rs`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/PROOF_LOG.md`
- `.devplan/HANDOFFS/STAGE_035.md`

Commands run:

```text
git status --short
cargo test validate_tool_rejects_sqlite_and_tool_json_drift
cargo test run_tool_rejects_sqlite_and_tool_json_drift_before_local_policy_override
cargo test
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
rm -rf Cargo.lock target
git status --short
```

Results:

```text
PASS validate drift test passed
PASS run drift test passed
PASS cargo test: 105 passed
PASS json valid
PASS removed generated Cargo.lock and target because they are outside patch scope
```

Implementation:

- Added exact contract drift detection between canonical SQLite tool contract
  and local `tool.json` during both validate and run.
- Switched validate/run to use canonical SQLite contract data after drift
  check passes, including runtime executable path and safety policy.
- Added minimal CLI drift mapping to `EXIT_DRIFT_DETECTED`.
- Added focused negative tests for validate/run mismatch behavior.
- Did not start Stage 036.

## PATCH_GLOBAL_AUDIT_P2

Status: `DONE_LOCAL_CHECKS_PASSED`

Findings closed:

- `GA-001`
- `GA-002`
- `GA-003`
- `GA-004`
- `GA-008`

Files changed:

- `src/audit/mod.rs`
- `src/storage/mod.rs`
- `src/tools/mod.rs`
- `src/install/mod.rs`
- `src/cli/mod.rs`
- `src/adapter/mod.rs`
- `aopmem_v0_1_final_orchestrated_pack/reference/STORAGE_AND_SQLITE_SPEC.md`
- `.devplan/FINAL_DECISION_LOG.md`
- `.devplan/PATCH_GLOBAL_AUDIT_P2.md`
- `.devplan/PROOF_LOG.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`

Commands run:

```text
cargo fmt
rtk cargo build
rtk cargo test
rtk cargo test --tests
CLI probe script for node update, tool list/get, dry-run, approvals, reflection
rg -n -i "mem0|hindsight|qdrant|embedding|vector|semantic search|custom MCP server|migration old MVP|import old MVP|old MVP|background enrichment|current_state|task history|markdown export|markdown import|GitHub Actions|CI workflow" --glob '!target/**' --glob '!dist/**' --glob '!Cargo.lock' .
find . -path './target' -prune -o -path './dist' -prune -o -path './.git' -prune -o -type f \( -path './.github/*' -o -name '*.yml' -o -name '*.yaml' \) -print
```

Results:

```text
PASS cargo build
PASS cargo test: 164 passed
PASS cargo test --tests: 164 passed
PASS CLI probes:
  node_update True
  node_missing False NOT_FOUND
  tool_list True
  tool_get True
  dry_run True
  external_read True
  write_blocked False UNSAFE_ACTION_BLOCKED
  write_allowed True
  reflect_create True
  reflect_apply True
PASS drift scan: docs/spec/scanner-test hits only
PASS no .github/YAML workflow files found
```

Known limitations:

- No new global audit was run.
- Optional MCP `installed` is used only when a direct CLI detector passes.
- Reflection/settings remain node-backed in v0.1.

## PATCH_GLOBAL_AUDIT_GA_001_DECISION

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- Resolve final global audit `GA-001` by final decision/spec update.

Decision:

- `configured_unverified` is accepted as a valid, non-blocking status for
  enabled optional Understand Anything / Codebase Memory MCP capabilities when
  the CLI cannot reliably verify them.
- This covers agent-local, host-global, shell-managed, or otherwise
  non-deterministic capabilities.
- AOPMem CLI must not fake `installed` without deterministic evidence.
- Optional MCP `missing` or `configured_unverified` must not fail install.

Files changed:

- `aopmem_v0_1_final_orchestrated_pack/reference/FINAL_DECISION_LOG.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/TOOLS_AND_MCP_REGISTRY.md`
- `aopmem_v0_1_final_orchestrated_pack/reference/INSTALL_AND_WORKSPACE_INIT.md`
- `aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_029.md`
- `aopmem_v0_1_final_orchestrated_pack/stage_prompts/STAGE_030.md`
- `.devplan/FINAL_DECISION_LOG.md`
- `.devplan/GLOBAL_AUDIT_REPORT.md`
- `.devplan/PATCH_GLOBAL_AUDIT_P2.md`
- `.devplan/EXECUTION_LEDGER.md`
- `.devplan/EXECUTION_LEDGER.json`
- `.devplan/PROOF_LOG.md`

Commands run:

```text
rtk cargo build
rtk cargo test
rtk cargo test --tests
! rg -n -i 'configured_unverified.*(invalid|must[- ]fix|[^-]blocking)|((invalid|must[- ]fix|[^-]blocking).*)configured_unverified' .devplan aopmem_v0_1_final_orchestrated_pack install docs proof templates --glob '!GLOBAL_AUDIT_COMMANDS.log' --glob '!PROOF_LOG.md'
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
```

Results:

```text
PASS cargo build
PASS cargo test
PASS cargo test --tests
PASS no text says configured_unverified is invalid/blocking/must-fix
PASS ledger json valid
```

Remediation note:

- `GA-001` resolved by final decision update.
- No product code changed.
- `configured_unverified` accepted for agent-local optional MCP capabilities.

## STAGE_035_MILESTONE_AUDIT

Status: `PASS`

Coverage:

- `STAGE_001`–`STAGE_035`

Focus:

- `STAGE_031`–`STAGE_035`

Commands run:

```text
git status --short
python3 -m json.tool .devplan/EXECUTION_LEDGER.json >/dev/null
cargo test
rm -rf Cargo.lock target
```

Results:

```text
PASS cumulative milestone audit rerun through STAGE_035
PASS cargo test: 105 passed
PASS json valid
PASS repo returned to the same untracked baseline after cleanup
PASS verified_through_stage can advance to STAGE_035
```

Findings:

- SQLite is now canonical for `tool validate` and `tool run`.
- No out-of-scope drift found in `STAGE_031`–`STAGE_035`.
- `STAGE_034` AUTO_PATCH_WINDOW stayed limited to minimal `src/cli/**` wiring.

Next stage:

- `STAGE_036`

## RC_PREP_v0.1.0-rc1

Status: `PASS`

Objective:

- Prepare AOPMem v0.1.0-rc1 release candidate without product code changes.

Files changed:

- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc1.md`
- `.devplan/PROOF_LOG.md`

Commands run:

```text
rtk cargo build
rtk cargo test
rtk cargo test --tests
git diff --check
rtk bash scripts/build_macos_arm.sh
file dist/aopmem-darwin-arm64/aopmem
shasum -a 256 dist/aopmem-darwin-arm64/aopmem
dist/aopmem-darwin-arm64/aopmem --version
temp AOPMEM_HOME install/init proof with init, adapter seed/status, doctor,
and recall
```

Results:

```text
PASS global audit report is PASS
PASS GA-001 focused re-audit is PASS
PASS all 55 ledger stages are VERIFIED
PASS no new P1/P2 findings
PASS rtk cargo build
PASS rtk cargo test: 164 passed
PASS rtk cargo test --tests: 164 passed
PASS git diff --check
PASS macOS ARM binary build
PASS binary file type: Mach-O 64-bit executable arm64
PASS binary version: aopmem 0.1.0
PASS binary sha256:
  798af720030081367969fb36a2913de98956d700fbdd6e87ae176d4e05caaefc
PASS temp install/init proof through AOPMEM_HOME
```

Release recommendation:

- RC ready for GitHub push and user-style install testing as `v0.1.0-rc1`.

## RC_PREP_v0.1.0-rc2_WINDOWS_X64

Status: `PASS`

Objective:

- Add Windows x64 native PowerShell build/install support without changing
  runtime behavior.

Files changed:

- `scripts/build_windows_x64_from_macos.sh`
- `dist/aopmem-windows-x86_64/aopmem.exe`
- `install/v0.1/install_prompt.md`
- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`
- `.devplan/WINDOWS_BUILD_PROOF.md`
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md`
- `.devplan/PROOF_LOG.md`

Commands run:

```text
bash scripts/build_windows_x64_from_macos.sh
file dist/aopmem-windows-x86_64/aopmem.exe
shasum -a 256 dist/aopmem-windows-x86_64/aopmem.exe
bash -n scripts/build_windows_x64_from_macos.sh
git diff --check
rtk cargo test
```

Results:

```text
PASS Windows build used Docker fallback with messense/cargo-xwin
PASS Windows artifact exists
PASS Windows artifact is non-empty
PASS binary file type: PE32+ executable (console) x86-64, for MS Windows
PASS binary sha256:
  d7d11a863c65877a31a203626764e3aaa2cc58c1403fbb37d6c1d22cdb17db0e
PASS bash syntax check
PASS git diff --check
PASS cargo test: 164 passed
PASS no src/** changes
PASS no tests/** changes
PASS no Linux, Windows ARM, WSL, CI, Node.js rewrite, or runtime feature drift
```

Runtime proof:

- Not run on Mac.
- Native Windows proof is documented in
  `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`.

Release recommendation:

- RC2 is ready for native Windows 11 PowerShell smoke.

## RC2_WINDOWS_DOC_PATCH

Status: `PASS`

Objective:

- Close Windows rc2 Mac audit documentation findings without runtime changes.

Files changed:

- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md`
- `.devplan/WINDOWS_RC2_DOC_PATCH.md`
- `.devplan/PROOF_LOG.md`

Commands run:

```text
rg required Windows smoke terms in docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md
rg required rc2 report terms in .devplan/RELEASE_CANDIDATE_v0.1.0-rc2.md
rg trailing whitespace check on changed docs
git diff --check
git status --short -- src tests Cargo.toml Cargo.lock install/v0.1/install_prompt.md
```

Results:

```text
PASS Windows smoke doc covers --version, --help, init, adapter seed,
adapter status, doctor, recall, JSON ok=true checks, workspace under
AOPMEM_HOME, Test-Path "$Repo\.aopmem" returns False, no .aopmem inside the
target repo, and AGENTS.md managed block checks.
PASS Windows smoke doc uses native PowerShell only, no WSL, no bash, and no
cargo build.
PASS rc2 report uses required Status, Scope, Evidence, Windows Native Proof,
Explicitly Out Of Scope, and Release Recommendation sections.
PASS rc2 report includes full macOS ARM artifact proof.
PASS rc2 report explicitly says no runtime behavior changed.
PASS no src/**, tests/**, Cargo.toml, or Cargo.lock changes in this patch.
INFO install/v0.1/install_prompt.md had pre-existing modified status; this
patch did not edit it.
PASS native Windows smoke was not run on Mac.
```

## WINDOWS_FIRST_INSTALL_REMEDIATION_RC3

Status: `PASS`

Objective:

- Fix first real Windows install bugs and prepare rc3 for native Windows VDI
  smoke.

Files changed:

- `src/storage/mod.rs`
- `src/install/mod.rs`
- `src/cli/mod.rs`
- `src/adapter/mod.rs`
- `templates/managed-block/AGENTS.managed-block.md`
- `docs/WINDOWS_NATIVE_POWERSHELL_SMOKE.md`
- `install/v0.1/install_prompt.md`
- `dist/aopmem-darwin-arm64/aopmem`
- `dist/aopmem-windows-x86_64/aopmem.exe`
- `.devplan/WINDOWS_FIRST_INSTALL_REMEDIATION.md`
- `.devplan/RELEASE_CANDIDATE_v0.1.0-rc3.md`
- `.devplan/PROOF_LOG.md`

Bugs fixed:

- Windows native PowerShell no longer requires `HOME`.
- Workspace key is shared and canonical across init, doctor, recall, adapter,
  node, link, tool, reflect, and artifacts command paths.
- Invalid stdin UTF-8 fails with `INVALID_UTF8_INPUT`.
- Mojibake-like semantic answers fail with `SUSPICIOUS_MOJIBAKE_INPUT`.
- AGENTS managed block now contains the real AOPMem operational contract.
- Windows smoke doc now uses native PowerShell, UTF-8 preamble, and rc3 temp
  proof.

Commands run:

```text
cargo fmt
rtk cargo build
rtk cargo test
rtk cargo test --tests
git diff --check
bash scripts/build_macos_arm.sh
file dist/aopmem-darwin-arm64/aopmem
dist/aopmem-darwin-arm64/aopmem --version
shasum -a 256 dist/aopmem-darwin-arm64/aopmem
bash scripts/build_windows_x64_from_macos.sh
file dist/aopmem-windows-x86_64/aopmem.exe
shasum -a 256 dist/aopmem-windows-x86_64/aopmem.exe
rg -n -i "node.js|npm start|wsl|linux support|windows arm|github actions|ci workflow|mem0|hindsight|qdrant|embedding|vector search|semantic search|custom mcp server|background enrichment|current_state|task history" --glob '!target/**' --glob '!dist/**' --glob '!Cargo.lock' .
```

Results:

```text
PASS cargo fmt
PASS rtk cargo build
PASS rtk cargo test: 178 passed
PASS rtk cargo test --tests: 178 passed
PASS git diff --check
PASS macOS ARM build
PASS macOS binary file type: Mach-O 64-bit executable arm64
PASS macOS binary version: aopmem 0.1.0
PASS macOS binary sha256:
  d238071299d557cfdeabfce75a52b2bcd2f62635802ef34da5ba11767155c607
PASS Windows x64 build
PASS Windows binary file type: PE32+ executable (console) x86-64, for MS Windows
PASS Windows binary sha256:
  01010aeffc20aead5f353353674621b367e6ad590769e4b5915b8d02d62f6d7a
PASS drift scan: hits are negative docs or src/verify denylist/test evidence.
PASS native Windows smoke was not run on Mac.
```

Release recommendation:

- Ready for Windows VDI smoke: yes.
- Ready to tag final after Windows smoke: yes, if native Windows smoke passes.
