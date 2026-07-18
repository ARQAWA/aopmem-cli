# AOPMem v0.2.0-rc5 Proof Log

## Stage 001

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- record field findings;
- classify the current worktree;
- inventory implementation, assets, tests, and complexity risks.

Files created:

- `.devplan/RC5_FIELD_FINDINGS.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_PROOF_LOG.md`;
- `.devplan/RC5_HANDOFFS/STAGE_001.md`.

Commands and results:

```text
git status --short --branch
PASS main...origin/main; clean at start

git describe --tags --always --dirty
PASS v0.2.0-rc4

git rev-parse HEAD
PASS 0af9b22c2e4a8217cbf6b1de558eb2181ce79a84

rustc --version
PASS rustc 1.95.0 (59807616e 2026-04-14)

cargo --version
PASS cargo 1.95.0 (f2d3ce0bd 2026-03-21)

complexity scanner
PASS report generated; heuristic leads classified manually

file dist/aopmem-darwin-arm64 dist/aopmem-windows-x86_64.exe
PASS Mach-O arm64; PE32+ x86-64

(cd dist && shasum -a 256 -c SHA256SUMS)
PASS both rc4 assets

git diff --check
PASS
```

### Stage 022 high-review P1/P2 remediation

PowerShell now copies the bounded no-follow tree through exclusive durable
file handles and creates the manifest through an exclusive flushed FileStream.
Its selector is exact case-sensitive and precedes TLS, console, environment,
and filesystem changes. The audit covers all four exact adapter/file pairs and
uppercase/mismatch rejection before home creation.

```text
scripts/audit_v020_installers.sh                     PASS 14 groups
cargo test --locked upgrade::recovery::tests::installer_backup_adoption
                                                    PASS 2/2
cargo fmt --all -- --check                           PASS
cargo clippy --all-targets --locked -- -D warnings   PASS
cargo build --locked                                 PASS
git diff --check                                     PASS
native Windows runtime                               PENDING_DOGFOOD
```

Historical test evidence:

- rc4 `cargo test --locked`: `616/616` PASS;
- rc4 `cargo test --tests --locked`: `616/616` PASS;
- native Windows runtime: `PENDING`.

Cargo tests were not rerun. Stage 01 changes only planning files.

Requirements covered:

- `RC5-FLD-001`;
- `RC5-FLD-002`;
- baseline evidence for `RC5-ARC-001`, `RC5-PERF-001`, and
  `RC5-CMD-001`.

Known limitation:

- scanner does not parse Rust; Rust hot paths were reviewed manually.

## Stage 002

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- freeze RC5 implementation decisions;
- map all specification sections, field facts, tests, documents, and DoD
  items to owner stages and proof.

Files created:

- `.devplan/RC5_FINAL_DECISION_LOG.md`;
- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_HANDOFFS/STAGE_002.md`.

Decisions closed:

- Confluence classification versus exact-only eligibility;
- exact secrets versus redacted audit snapshot;
- authoritative task state versus best-effort observability;
- apply-once recovery before binary publish;
- active adapter selection;
- secure exact task-query transport.

Coverage:

```text
source sections: 34/34
source field statements: 17/17 in 16 finding rows
DoD items: 32/32
required product/devplan documents assigned: 16/16
unresolved requirement owner: 0
unresolved product/security decision: 0
```

Local checks:

```text
jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

ledger stage count
PASS 30

completed local stage count
PASS 2

requirement ID uniqueness
PASS 62/62

decision ID uniqueness
PASS 47/47

field requirement references
PASS all resolve

source-section coverage
PASS 34/34

Definition of Done reverse map
PASS 32/32

required artifact ownership
PASS 16/16

Stage 01/02 artifact existence
PASS 8/8 non-empty

RC5 planning-file whitespace scan
PASS

scope scan
PASS only .devplan/RC5_* and .devplan/RC5_HANDOFFS/*
```

Requirements covered:

- frozen contract for every `RC5-*` matrix row;
- `RC5-GOV-001`;
- `RC5-DERC-001`;
- `RC5-DERC-002`;
- `RC5-OOS-001`;
- `RC5-STOP-001`.

Known limitation:

- Stages 01–02 are not `VERIFIED` until the Stage 05 cumulative audit.

Next stage:

- `STAGE_003`: Managed Block V2 specification.

## Stage 003

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- freeze the normative Managed Block V2 contract before implementation;
- define the hard task-start and Task Context Receipt boundary;
- define exact retrieval, task reuse, approval, secret, and tool rules.

Files created:

- `.devplan/RC5_MANAGED_BLOCK_V2_SPEC.md`;
- `.devplan/RC5_HANDOFFS/STAGE_003.md`.

Contract checks:

```text
contract version marker
PASS AOPMEM CONTRACT VERSION: 2

numbered section headings
PASS exactly 18, numbered 1 through 18 in order

required section names
PASS 18/18 match the supplied RC5 specification

useful-line target
PASS 124 (contract marker + numbered headings + behavior bullets)

UTF-8 byte hard limit
PASS 11430 <= 24576 bytes

hard-gate trigger and pre-receipt action scan
PASS all required triggers, substantive actions, and allowed actions present

task boundary and receipt scan
PASS same-goal reuse and all new-task boundaries present

retrieval order scan
PASS exact nine-step order present

secret/tool/approval scan
PASS no blanket ban; action-based approval and canonical tool rules present

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

Requirements covered:

- specification proof for `RC5-BLK-001`, `RC5-BLK-002`, and `RC5-BLK-003`;
- specification proof for `RC5-RET-001`;
- frozen implementation contract for `RC5-KPR-001..003`, `RC5-SEC-001..004`,
  `RC5-TOL-001`, `RC5-CGD-001`, and `RC5-ADP-001..002`.

Scope:

- only Stage 003 `.devplan` specification and bookkeeping files changed;
- no production, template, documentation, source, test, or installer file was
  changed.

Known limitation:

- this stage specifies but does not install Managed Block V2;
- Stages 001–003 are not `VERIFIED` until the Stage 05 cumulative audit.

Next stage:

- `STAGE_004`: task lifecycle and Local Observability schema v2.

## Stage 004

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- add a typed, fail-closed task lifecycle state model;
- migrate exact Local Observability v1 stores to schema v2;
- keep authoritative task state separate from best-effort factual events;
- prove transition, privacy, migration, and retention behavior.

Files added:

- `src/task/mod.rs`;
- `src/observability/task_state.rs`;
- `docs/TASK_START_PROTOCOL.md`;
- `.devplan/RC5_HANDOFFS/STAGE_004.md`.

Files updated:

- `src/main.rs`;
- `src/observability/mod.rs`;
- `src/observability/report.rs`;
- `docs/LOCAL_OBSERVABILITY.md`;
- one existing CLI read-only schema-version test assertion.

Contract proof:

```text
authoritative state
PASS tasks + task_bundle_nodes + task_applied_nodes in observability schema v2

state/event separation
PASS lifecycle writes do not insert best-effort events

strict migration
PASS exact v1 manifest -> one transactional v2 migration; rows preserved

task identity and transitions
PASS canonical UUID v4 ids, workspace/bundle/revision/membership checks,
     started -> applied -> completed|failed, exact replay, conflict rejection

privacy
PASS state API has no raw query input; raw query/chat/output/reasoning columns
     absent; failure reason bounded and redacted before SQLite

retention
PASS task roots follow 30-day/100-MB policy and cascade task children only
```

Commands and results:

```text
cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

fresh schema v2 focused test
PASS 1/1

v1 -> v2 migration focused test
PASS 1/1

task lifecycle/privacy/retention focused tests
PASS 4/4

task factual-event contract focused test
PASS 1/1

rtk cargo test observability::
PASS 90/90

rtk cargo test --locked
PASS 624/624

git diff --check
PASS
```

Requirements advanced:

- Stage 04 portion of `RC5-TSK-001`;
- Stage 04 portion of `RC5-TSK-006`;
- Stage 04 portion of `RC5-OBS-001`.

Scope:

- no operational-memory task history or schema migration was added;
- no task apply/complete CLI command was added;
- no alias, template, installer, or release work was added.

Known limitations:

- Stage 05 owns complete internal task-start retrieval and its JSON command;
- Stage 06 owns apply/complete command validation and event projection;
- Stage 23 owns task facts in effectiveness reports;
- Stages 001–004 are not `VERIFIED` until the Stage 05 cumulative audit.

Next stage:

- `STAGE_005`: task start internal complete retrieval.

## Stage 005

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- add secure inline/stdin `task start`;
- read one revision-bound operational snapshot without operational writes;
- finish mandatory and task retrieval inside one invocation;
- persist authoritative start state before the response;
- prove privacy, atomic overflow, bounded complexity, and event isolation.

Files added or updated:

- `src/cli/mod.rs`;
- `src/storage/mod.rs`;
- `src/task/mod.rs`;
- `src/observability/mod.rs`;
- `src/observability/task_state.rs`;
- `docs/TASK_START_PROTOCOL.md`;
- `.devplan/RC5_HANDOFFS/STAGE_005.md`.

Implementation proof:

```text
query contract
PASS exactly one of --query/--query-stdin; UTF-8, non-blank, no NUL,
     <= 65,536 bytes

snapshot/read-only
PASS revision + mandatory + four retrieval layers use one deferred read
     transaction; concurrent WAL write is invisible; revision is unchanged

mandatory contract
PASS all active mandatory types; exact 1 MiB canonical JSON limit;
     MANDATORY_CONTEXT_OVERFLOW is atomic and creates no task/event state

internal retrieval
PASS typed roots + FTS/BM25 + direct links + depth-two graph;
     source/trust/confidence ordering; global dedupe; merged reasons;
     >128 candidates in every layer; no returned cursor

structural bound
PASS four streaming SQL statements, no OFFSET/N+1, 2,048 rows/layer,
     shared 16 MiB logical resident candidate payload plus at most one
     temporary complete row

budget state
PASS scan and canonical-byte exhaustion are distinct; complete/false and
     incomplete/true are the only accepted completeness/exhaustion pairs;
     constructor, SQL CHECK, and corrupted-state load enforce the invariant

task authority
PASS fallible task/bundle UUID allocation; durable start before response;
     best-effort event failure cannot undo authoritative state

correction contract
PASS correction/failure graph retrieval; lesson and incident_scar response
     types normalize to correction only in authoritative apply membership

output/privacy
PASS all 17 required fields, no continuation cursor, no raw query in
     operational DB, observability DB, WAL, task state, event, or protocol
```

Complexity change:

```text
before
4 * 2,048 complete candidates could be collected before packing;
worst logical bodies were roughly 8 GiB

after
O(n) streamed row admission under one 16 MiB logical payload budget;
one extra <=1 MiB current row; final stable ordering remains O(n log n);
four SQL statements maximum; no query inside a row loop
```

Commands and results:

```text
cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test task_start --locked
PASS 6/6

rtk cargo test complete_task_recall --locked
PASS 4/4

WAL snapshot focused test
PASS 1/1

fresh schema retrieval/budget invariant test
PASS 1/1

corrupt retrieval/budget load rejection test
PASS 1/1

rtk cargo test storage::tests::task_recall --locked
PASS 7/7

rtk cargo test observability:: --locked
PASS 91/91

rtk cargo test --locked
PASS 633/633

git diff --check
PASS
```

Approved adjacent exceptions:

- `src/observability/mod.rs`: schema CHECK and focused fresh-schema test;
- `src/observability/task_state.rs`: fail-closed stored-row validation and
  corruption test.

Requirements advanced:

- Stage 05 portion of `RC5-TSK-001`;
- `RC5-TSK-002`;
- `RC5-TSK-003`;
- `RC5-TSK-004`;
- `RC5-TSK-005`;
- Stage 05 portion of `RC5-TSK-006`;
- Stage 05 portion of `RC5-GOL-001`;
- Stage 05 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stages 001–005 remain not `VERIFIED`;
- cumulative audit through Stage 005 is next;
- after a passing audit, continue with `STAGE_006`.

## Cumulative Audit Through Stage 005

Status: `PASS`

Verified through: `STAGE_005`

Audit artifact:

- `.devplan/RC5_HANDOFFS/AUDIT_001_005.md`.

Result:

- P1: `0`;
- P2: `0`;
- Stage 001 field evidence and baseline are consistent;
- Stage 002 frozen decisions and requirement traceability are consistent;
- Stage 003 Managed Block V2 specification satisfies its structural bounds;
- Stage 004 task lifecycle and observability v2 contracts are fail-closed;
- Stage 005 task start is revision-bound, read-only, bounded, private, atomic,
  and cursor-free;
- no scope drift was found through Stage 005.

Final command proof:

```text
cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 701/701

rtk cargo test --tests --locked
PASS 701/701

rtk ./scripts/dev_verify.sh
PASS

rtk cargo test task_start --locked
PASS 6/6

rtk cargo test complete_task_recall --locked
PASS 4/4

rtk cargo test storage::tests::task_recall --locked
PASS 7/7

rtk cargo test observability:: --locked
PASS 91/91

rtk cargo test --locked
PASS 633/633

rtk cargo test --tests --locked
PASS 633/633

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Ledger decision:

- Stages 001–005: `VERIFIED`;
- cumulative audit through Stage 005: `PASS`;
- current stage: `STAGE_006`;
- next cumulative audit: `STAGE_010`.

## Stage 006

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- implement `task apply` with revision, workspace, bundle, membership, kind,
  status, none-relevant, replay, and operational no-write enforcement;
- implement `task complete` with derived duration, bounded terminal facts,
  immutable replay, privacy, and factual event projection.

Files changed:

- `src/cli/mod.rs`;
- `src/task/mod.rs`;
- `src/observability/task_state.rs`;
- `docs/TASK_START_PROTOCOL.md`;
- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_HANDOFFS/STAGE_006.md`;
- `.devplan/RC5_PROOF_LOG.md`.

Implementation proof:

- typed apply/complete inputs derive canonical replay fingerprints after
  normalization and stable sort;
- empty apply fails closed; none-relevant is limited to complete,
  non-exhausted zero-task-node retrieval and mandatory gate/rule facts;
- first apply uses a current read-only revision and one bounded bulk node
  query;
- unknown, outside-bundle, wrong-kind, stale, wrong-workspace, wrong-bundle,
  deprecated, superseded, and broken inputs fail with stable errors;
- draft and active nodes are accepted;
- correction, lesson, and incident scar map to one application kind;
- exact apply replay skips operational reread and duplicate event projection;
- complete accepts success, partial, or failed; failed may transition from
  started and requires a stable error code;
- duration is derived from stored start time and workflow/tool ids are derived
  from stored applied nodes;
- raw failure reason is redacted before persistence; distinct normalized raw
  reasons remain distinct replay requests;
- task state remains durable when best-effort projection fails;
- operational revision and table-row facts remain unchanged by apply and
  complete.

Complexity:

```text
typed normalization: O(n log n), bounded at 8,192 nodes
operational bulk query: 1 statement, no query in node loop
stored membership checks: O(n log m)
authoritative sorted validation: O(n + m)
complete applied-id projection: O(n)
```

Commands and results:

```text
cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked task_ -- --nocapture
PASS 45/45

rtk cargo test --locked
PASS 639/639
```

Requirements advanced:

- `RC5-TSK-001`;
- `RC5-TSK-007`;
- `RC5-TSK-008`;
- Stage 06 portion of `RC5-GOL-001`;
- Stage 06 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 006 is not `VERIFIED`;
- verified through remains `STAGE_005`;
- next cumulative audit remains due through `STAGE_010`;
- continue with `STAGE_007`.

## Stage 007

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- replace the obsolete cursor-based Keeper skill with Memory Keeper V2;
- enforce native-subagent, direct-stdin task start and authoritative apply;
- define a compact privacy-safe Task Context Receipt and exact task boundaries.

Files changed:

- `templates/skills/memory-keeper/SKILL.md`;
- `docs/MEMORY_KEEPER_V2.md`;
- `src/adapter/mod.rs`;
- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_HANDOFFS/STAGE_007.md`;
- `.devplan/RC5_PROOF_LOG.md`.

Contract proof:

- the skill has valid `name`/`description` frontmatter and concise imperative
  low-freedom instructions;
- Keeper requires the exact request, repo root, current shell, and current
  instruction file before retrieval;
- start and apply use the exact supplied repo root as `cwd`;
- the fixed process runner uses a separate stdin channel and keeps every
  request-derived byte out of command text, argv, files, environment, and
  receipts;
- unavailable native Keeper or safe stdin returns exact
  `MEMORY_KEEPER_UNAVAILABLE`, with no parent shell fallback;
- all 17 core start fields, UUIDs, workspace, revision, completeness pair,
  array types, bundle membership, node kinds, and missing cursor are checked;
- complete and bounded retrieval are the only accepted state pairs;
- gates and rules are applied, while workflow/tool/correction/failure-mode
  selections remain the smallest returned relevant subset;
- none-relevant requires complete, non-exhausted, empty task context;
- apply uses the exact global bundle and Stage 006 repeatable ID flags;
- receipt content is bounded, factual, source-ordered, and excludes raw
  request, full nodes, transcripts, output, secrets, and hidden reasoning;
- same-goal work reuses the receipt; every frozen new-task boundary starts a
  new task;
- the exact nine-step retrieval order is preserved.

Commands and results:

```text
uv run --with PyYAML python \
  /Users/arkadijcukavin/.codex/skills/.system/skill-creator/scripts/quick_validate.py \
  templates/skills/memory-keeper
PASS Skill is valid!

rtk cargo test --locked \
  memory_keeper_v2_contract_is_fail_closed_and_privacy_safe -- --nocapture
PASS 1/1

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked
PASS 639/639

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

Requirements advanced:

- `RC5-KPR-001`;
- `RC5-KPR-002`;
- Stage 07 portion of `RC5-KPR-003`;
- Stage 07 portion of `RC5-BLK-003`;
- Stage 07 portion of `RC5-RET-001`;
- Stage 07 portion of `RC5-GOL-001`;
- Stage 07 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 007 is not `VERIFIED`;
- verified through remains `STAGE_005`;
- next cumulative audit remains due through `STAGE_010`;
- forward dogfood remains owned by `STAGE_025`;
- continue with `STAGE_008`.

## Stage 008

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- install the exact Managed Block V2 contract from one canonical template;
- keep explicit adapter targets and preserve bytes outside the managed block;
- reject duplicate/damaged blocks and preserve legacy default behavior.

Files changed:

- `templates/managed-block/AGENTS.managed-block.md`;
- `src/adapter/mod.rs`;
- `src/cli/mod.rs`;
- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_HANDOFFS/STAGE_008.md`;
- `.devplan/RC5_PROOF_LOG.md`.

Contract proof:

- `src/adapter/mod.rs` embeds one canonical template with `include_str!`;
- the template has contract version 2, exactly 18 numbered sections, 124
  useful lines, and a total size of 10835 bytes;
- the hard gate, task boundaries, receipt reuse, action-class approval,
  secret handling, tool identity, and exact nine-step source order are present;
- obsolete normal-recall, cursor, and blanket-secret-ban text is absent;
- legacy V1 content upgrades to exact V2 while custom approval text and all
  bytes outside the managed block remain exact;
- a second sync is byte-identical;
- duplicate or damaged marker layouts fail without writing;
- explicit targets cover `AGENTS.md`, `CLAUDE.md`,
  `.cursor/rules/aopmem.mdc`, and
  `.github/copilot-instructions.md`;
- only the selected explicit target changes, while the no-`--file` default
  remains `AGENTS.md`.

Commands and results:

```text
canonical Managed Block V2 parity check
PASS

template structure check
PASS sections=18 useful_lines=124 bytes=10835

rtk cargo test --locked adapter::tests -- --nocapture
PASS 15/15

rtk cargo test --locked \
  adapter_seed_parses_legacy_default_and_all_explicit_stage_008_targets \
  -- --nocapture
PASS 1/1

rtk cargo test --locked \
  adapter_commands_record_seed_sync_and_real_drift_only -- --nocapture
PASS 1/1

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked
PASS 642/642

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

Requirements advanced:

- `RC5-BLK-001`;
- Stage 08 portion of `RC5-KPR-003`;
- Stage 08 portion of `RC5-BLK-002`;
- Stage 08 portion of `RC5-BLK-003`;
- `RC5-RET-001`;
- Stage 08 portion of `RC5-ADP-001`;
- Stage 08 portion of `RC5-ADP-002`;
- Stage 08 portion of `RC5-GOL-001`;
- Stage 08 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 008 is not `VERIFIED`;
- verified through remains `STAGE_005`;
- next cumulative audit remains due through `STAGE_010`;
- installer, secret, redaction, release, and dogfood work was not started;
- continue with `STAGE_009`.

## Stage 009

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- permit user-provided and authorized test credentials without blanket
  refusal, placeholder coercion, lecture, or value removal;
- keep exact operational persistence explicit and atomic;
- preserve action-class approval without a secret detector, secret store,
  schema, or parallel secrets platform.

Files changed:

- `src/cli/mod.rs`;
- `templates/skills/memory-keeper/SKILL.md`;
- `docs/MEMORY_KEEPER_V2.md`;
- `docs/SECRET_HANDLING.md`;
- `.devplan/RC5_REQUIREMENTS_MATRIX.md`;
- `.devplan/RC5_EXECUTION_LEDGER.json`;
- `.devplan/RC5_CURRENT_STAGE.md`;
- `.devplan/RC5_HANDOFFS/STAGE_009.md`;
- `.devplan/RC5_PROOF_LOG.md`.

Contract proof:

- `teach propose --apply` stores and applies one proposal under the same
  `mutation::mutate_workspace` `BEGIN IMMEDIATE` transaction;
- the audit snapshot is attempted only after the proposal, exact target node,
  `sensitivity:test_secret` tag, and apply receipt commit together;
- apply failure rolls back the proposal, node, tag, receipt, revision change,
  and snapshot publication;
- secret-bearing proposal JSON uses bounded direct process stdin through
  `--payload-stdin`, which is mutually exclusive with inline `--payload` and
  requires atomic `--apply`;
- stdin accepts at most 2 MiB of valid UTF-8 JSON and rejects NUL bytes;
- the canonical node has generic title `Authorized test credential`, generic
  provenance, and the exact value only in node bodies;
- command response, error text, and observability payloads contain no
  deterministic canary;
- ordinary inline `--payload` remains compatible for nonsecret flows;
- a user-authorized fake credential reaches an external-read authentication
  tool unchanged without `+++`;
- that read changes no node count, tag count, or operational revision;
- external write with the same fake credential remains blocked without
  approval and runs only with standalone exact `+++`;
- Memory Keeper and product documentation forbid `remember` then `tag add`
  and separate secret-bearing `teach propose` then `teach apply`;
- Managed Block V2 already contained the frozen use and action-approval
  contract, so Stage 009 did not change it;
- redaction and `<TEST_SECRET_REDACTED>` implementation remain owned by
  Stage 010.

Commands and results:

```text
rtk cargo test --locked stage_009 -- --nocapture --test-threads=1
PASS 4/4

rtk cargo test --locked \
  teach_propose_stdin_is_exclusive_bounded_utf8_and_canary_safe_on_error \
  -- --nocapture
PASS 1/1

rtk cargo test --locked teach_ -- --nocapture --test-threads=1
PASS 11/11

uv run --with pyyaml python \
  /Users/arkadijcukavin/.codex/skills/.system/skill-creator/scripts/quick_validate.py \
  templates/skills/memory-keeper
PASS Skill is valid!

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked
PASS 647/647

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

Requirements advanced:

- `RC5-SEC-001`;
- Stage 09 portion of `RC5-SEC-002`;
- Stage 09 portion of `RC5-SEC-004`;
- Stage 09 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 009 is not `VERIFIED`;
- verified through remains `STAGE_005`;
- next cumulative audit remains due through `STAGE_010`;
- continue with `STAGE_010`.

## Stage 010

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- replace every protected copy of an explicitly tagged exact test secret with
  `<TEST_SECRET_REDACTED>`;
- preserve the exact body and tag only in operational SQLite and durable
  SQLite/full-home backup;
- fail closed on invalid, unreadable, or unbounded redaction anchors.

Contract proof:

- one shared `TaggedValueRedactor` loads the exact binary
  `sensitivity:test_secret` tag with one `tags JOIN nodes` query per
  operational read snapshot;
- only the tagged node body is an anchor; there is no detector and no minimum
  anchor length;
- values are bounded, deduplicated, sorted longest-first then by bytes, and
  indexed by first byte;
- matching scans original input left-to-right, applies leftmost-longest
  replacement, and treats the marker atomically for idempotence;
- raw values and canonical JSON-escaped copies compete in the same audit
  pass;
- observability, task failures, reports, Local UI, audit SQL, debug ZIP
  JSON/JSONL, and proposal copies are protected;
- missing operational SQLite is an empty set only before initialization on
  best-effort observability/report paths; an existing invalid source disables
  or fails the protected path;
- task-state lookup failure persists `TASK_REDACTION_UNAVAILABLE` with no
  reason;
- operational reads and online SQLite backup preserve the exact authorized
  body and tag.

Complexity proof:

- source load is bounded to 1024 distinct values and 16 MiB raw bytes;
- sort cost is `O(k log k)` for bounded `k`;
- matching is one pass over input offsets with first-byte candidate buckets;
- worst-case candidate comparison is bounded by the fixed source and body
  limits;
- memory is bounded by raw anchors, separately bounded JSON-escaped anchors,
  indexes, input, and at most 16 MiB output expansion;
- no unbounded nested database scan, N+1 query, or repeated whole-output
  rescan was added.

Commands and results:

```text
rtk cargo test --locked stage_010 -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked stage_009 -- --nocapture --test-threads=1
PASS 4/4

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo test --locked
PASS 661/661

rtk ./scripts/dev_verify.sh
PASS, including build, 661 tests, CLI proof, negative checks, and drift check

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS

git diff --check
PASS
```

Requirements advanced:

- `RC5-SEC-002`;
- `RC5-SEC-003`;
- `RC5-SEC-004`;
- Stage 10 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 010 is not `VERIFIED`;
- verified through remains `STAGE_005`;
- next cumulative audit is due through `STAGE_010`;
- current implementation stage is `STAGE_011`.

## Cumulative Audit 006–010

Status: `PASS`

Audit scope:

- authoritative task apply and complete lifecycle;
- bounded task start and read-only operational retrieval;
- Memory Keeper V2;
- Managed Block V2 and adapter targets;
- atomic exact test-secret teach flow;
- exact tagged-value redaction in protected sinks;
- exact operational SQLite and online backup persistence.

Independent results:

```text
cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

focused task/Keeper/block/secret/redaction suites
PASS 32/32

rtk cargo test --locked
PASS 661/661

rtk ./scripts/dev_verify.sh
PASS, including build, 661 tests, CLI proof, negative checks, and drift check

jq ledger syntax, unique-stage, boundary, status, and artifact checks
PASS stages=5 artifacts=31 missing=0

git diff --check
PASS

fake canary location scan
PASS files=3 unexpected=0
```

Contract result:

- task replay, state transitions, identity, revision, membership, active
  status, none-relevant, duration, and operational no-write rules pass;
- Memory Keeper and Managed Block exact boundaries pass;
- canonical Managed Block parity is 18 sections, 124 contract lines, and
  10835 bytes;
- atomic teach rollback and one-snapshot success rules pass;
- tagged redaction and exact operational/backup persistence pass;
- no P1 or P2 defect was found;
- RC5-D-043 remains pending for native Windows runtime proof.

Audit state:

- P1: `0`;
- P2: `0`;
- Stages 006–010 are `VERIFIED`;
- verified through is `STAGE_010`;
- next cumulative audit is `STAGE_015`;
- continue with `STAGE_011`.

## Stage 011

Status: `DONE_LOCAL_CHECKS_PASSED`

Objective:

- add immutable operational migration
  `004_task_protocol_and_tool_aliases`;
- provide typed direct tool-alias storage and deterministic resolution;
- make alias mutations part of the operational revision without creating
  filesystem copies or adding CLI behavior early.

Contract proof:

- `tool_aliases.alias` is the unique primary key;
- every target is a foreign-key-backed existing active canonical tool;
- persisted alias IDs, target IDs, source, and status are bounded and checked;
- API and SQLite triggers reject alias chains, cycles, and shadowing of every
  non-superseded tool ID;
- a superseded old ID may become a direct alias to an active canonical ID;
- target tools cannot be renamed or made non-active while referenced;
- add/get/list/keyset/bulk/remove/resolve have typed results and failures;
- bulk insertion is bounded, duplicate-checked before writing, and atomic
  under one nested savepoint;
- resolution order is direct non-superseded, active alias, superseded direct
  fallback, then not found;
- alias rows are streamed into `operational_recall_revision`;
- no tool directory, manifest, executable, runtime, or artifact copy occurs.

Complexity proof:

- point operations use primary-key, unique-key, or target/status indexes;
- resolver work is one bounded SQL statement with three indexed candidates;
- keyset pages read at most `limit + 1` under a hard 1000-row limit;
- batch duplicate validation is `O(n log n)` for `n <= 1000`;
- each batch row performs only a constant number of indexed checks;
- there is no pairwise scan, recursive chain traversal, filesystem scan, or
  implementation hash.

Commands and results:

```text
rtk cargo test --locked stage_011 -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked schema::tests -- --nocapture --test-threads=1
PASS 19/19

focused export/audit/verify/upgrade compatibility suites
PASS 84/84

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 670/670

rtk ./scripts/dev_verify.sh
PASS, including build, 670 tests, CLI proof, negative checks, and drift check

jq ledger syntax and Stage 011 boundary checks
PASS

git diff --check
PASS
```

Requirements advanced:

- `RC5-ALS-001`;
- Stage 011 storage portion of `RC5-ALS-002`;
- Stage 011 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 011 is not `VERIFIED`;
- verified through remains `STAGE_010`;
- next cumulative audit remains `STAGE_015`;
- current implementation stage is `STAGE_012`.

## Stage 012

Status: `DONE_LOCAL_CHECKS_PASSED`

Implemented:

- canonical domain-separated SHA-256 tool fingerprint;
- deterministic `platform_launchers` map in the existing `ToolContract`;
- five public duplicate classes with separate exact-only eligibility;
- bounded indexed shortlist before implementation hashing;
- one anchored no-follow hash per shortlisted implementation file;
- deterministic typed plan API;
- strictly read-only `aopmem tool dedupe plan --json`;
- immutable clean-checkpoint SQLite read with no observability;
- full tree/content/mtime zero-write proof;
- fail-closed manifest/tree/symlink/reparse/swap/drift/bound handling.

Commands and results:

```text
rtk cargo test --locked stage_012 -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked tools::tests -- --nocapture --test-threads=1
PASS 75/75

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 679/679

rtk ./scripts/dev_verify.sh
PASS, including build, 679 tests, CLI proof, negative checks, and drift check

jq ledger syntax and Stage 012/013 boundary checks
PASS

git diff --check
PASS
```

Requirements advanced:

- `RC5-DUP-001`;
- Stage 012 plan portion of `RC5-DUP-002`;
- Stage 012 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 012 is not `VERIFIED`;
- verified through remains `STAGE_010`;
- next cumulative audit remains `STAGE_015`;
- current implementation stage is `STAGE_013`.

## Stage 013

Status: `DONE_LOCAL_CHECKS_PASSED`

Implemented:

- alias add/list/remove/resolve CLI;
- alias-aware get, validate, run, canonical paths, and approval;
- canonical list pages with one batched alias lookup;
- optional explicit alias rows without cursor/limit distortion;
- safe `tool.alias_created` and `tool.alias_resolved` facts;
- direct ID/alias collision guard;
- bounded semantic/capability/BM25 overlap guard;
- Stage 012 anchored validation for shortlisted existing implementations;
- bounded private `--technical-distinction`;
- authoritative registry recheck before the first mutation write.

Commands and results:

```text
rtk cargo test --locked stage_013 -- --nocapture --test-threads=1
PASS 13/13

rtk cargo test --locked cli::tests::tool_ -- --nocapture --test-threads=1
PASS 18/18

rtk cargo test --locked tools::tests -- --nocapture --test-threads=1
PASS 83/83

cargo fmt --all -- --check
PASS

rtk cargo check --all-targets --locked
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 691/691

rtk ./scripts/dev_verify.sh
PASS, including build, 691 tests, CLI proof, negative checks, and drift check
```

Requirements advanced:

- `RC5-ALS-002`;
- Stage 013 portions of `RC5-TOL-001`, `RC5-CGD-001`, `RC5-OBS-001`, and
  `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 013 is not `VERIFIED`;
- verified through remains `STAGE_010`;
- next cumulative audit remains `STAGE_015`;
- current implementation stage is `STAGE_014`.

## Stage 014

Status: `DONE_LOCAL_CHECKS_PASSED`.

Implemented exact-only canonicalization inside the coordinated mutation.
Evidence: equal fingerprints are grouped without exact-pair expansion;
non-exact signals are review-only; status, manifests, and direct aliases stay
consistent; a late failure restores changed manifest bytes and SQLite state
through anchored handles.

```text
rtk cargo test --locked stage_014 -- --nocapture --test-threads=1
PASS 10/10

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

cargo fmt --check
PASS

rtk cargo build --locked
PASS

git diff --check
PASS
```

P1: `0`; P2: `0`. Verified through remains `STAGE_010`.

## Stage 012/014 audit remediation

Added a deterministic same-path regular-file replacement regression. The swap
happens after hashing and before the post-check; equal-size content cannot
bypass the reopened anchored identity comparison and returns
`ImplementationDrift`. This is a focused remediation, not a cumulative audit.

The follow-up remediation streams the 1,024 descendant-entry bound before
sorting, rechecks manifest identity, and maps CLI filesystem failures to
`TOOL_DEDUPE_FILESYSTEM_UNSAFE`. The cumulative audit remains `FAIL`.

Public plan, creation preflight, and authoritative apply now each have a
targeted deterministic same-path replacement proof. Apply fails before any
canonicalization and preserves manifest bytes and aliases.

## Stage 015

Status: `DONE_LOCAL_CHECKS_PASSED`

Implemented the deterministic, secret-free two-contract Confluence fixture
and final Managed Block V2 tool-governance proof. The fixture verifies generic
exact-only canonicalization, safe alias resolution, replay idempotency, safe
canonicalization observability, and a non-Confluence control pair. The
managed-block parity test compares the canonical template body with the
normative V2 specification and asserts exact tool-reuse/creation and approval
rules.

```text
rtk cargo test --locked stage_015 -- --nocapture --test-threads=1
PASS 4/4

rtk cargo test --locked managed_block -- --nocapture --test-threads=1
PASS 10/10

rtk cargo test --locked stage_01 -- --nocapture --test-threads=1
PASS 58/58

cargo fmt --check
PASS

rtk cargo clippy --all-targets -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 704 passed

rtk cargo test --tests --locked
PASS 704 passed

rtk ./scripts/dev_verify.sh
PASS `dev verify passed` with 704 tests

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Source review confirms `confluence` occurs only in the Stage 015 test and
fixture paths; production dedupe code has no Confluence branch. The focused
fixture, manifests, runner bytes, and docs contain no secret value. P1: `0`.
P2: `0`. Stages 011–015 remain `DONE_LOCAL_CHECKS_PASSED`; verified through
remains `STAGE_010` pending `CUMULATIVE_AUDIT_011_015`.

## Stage 012/014 cumulative-audit CLI privacy remediation

Status: `DONE_LOCAL_CHECKS_PASSED`.

Added one real compiled-binary integration proof under
`tests/cli_dedupe_privacy.rs`. It uses an isolated repository, `HOME`, and
`AOPMEM_HOME`; registers two tools through public `init` and
`tool create-draft`; then injects an absolute-target unsafe symlink with a
unique raw-path canary.

Captured public commands:

- plan text;
- plan JSON;
- exact-only apply text;
- exact-only apply JSON.

All four return exit `5` and the stable
`TOOL_DEDUPE_FILESYSTEM_UNSAFE` reason. No captured stdout/stderr contains the
canary, isolated root, or absolute target. Both JSON envelopes parse and have
the correct command IDs, `ok=false`, null data, exactly one
`TOOL_DEDUPE_PLAN_FAILED` error, empty warnings, and a version field. Both text
calls have empty stdout and the exact stable stderr line.

A focused mapper test separately proves that direct
`ToolDedupeApplyError::Io` text and JSON rendering cannot expose its absolute
path canary.

The public targeted-file-swap apply proof now uses a same-byte replacement and
asserts unchanged:

- complete SQLite tool records, including status and contract;
- both manifest byte sequences;
- both runner byte sequences;
- both tool directories;
- all aliases.

Commands and exact results:

```text
rtk cargo test --locked \
  stage_012_014_cli_dedupe_filesystem_errors_are_private_in_text_and_json \
  -- --nocapture --test-threads=1
PASS 1/1; 708 filtered

rtk cargo test --locked \
  stage_014_dedupe_apply_io_error_never_exposes_raw_path \
  -- --nocapture --test-threads=1
PASS 1/1; 708 filtered

rtk cargo test --locked \
  stage_012_013_014_public_operations_fail_closed_on_targeted_file_swap \
  -- --nocapture --test-threads=1
PASS 1/1; 708 filtered

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 709/709 (708 unit + 1 compiled-CLI integration)

rtk cargo test --tests --locked
PASS 709/709 (708 unit + 1 compiled-CLI integration)

rtk ./scripts/dev_verify.sh
PASS, including build, 708 unit tests, 1 compiled-CLI integration test,
CLI proof, negative checks, and drift check

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

P1: `0` in this remediation. P2: `0` in this remediation.

The cumulative audit stays `FAIL`; Stages 011–015 remain
`DONE_LOCAL_CHECKS_PASSED`, and verified through remains `STAGE_010`, pending
a fresh independent audit.

## Cumulative Audit 011–015 — fresh independent re-audit

Status: `PASS`.

The fresh auditor independently reviewed the original P1 `1` / P2 `2` FAIL,
all remediation source, public error mappings, fixtures, managed rules, and
the complete Stage 011–015 behavior.

Accepted remediation:

- implementation and manifest reads now finish with an anchored
  logical-path-to-open-file identity comparison;
- targeted same-path replacement fails plan, creation preflight, and
  authoritative apply closed;
- failed apply preserves complete SQLite records, aliases, manifest bytes,
  runner bytes, and directories;
- total implementation-tree work is streamed under a hard 1,024-entry bound
  before allocation, sorting, recursion, or hashing of an over-limit entry;
- real compiled plan/apply text and JSON failures expose only
  `TOOL_DEDUPE_FILESYSTEM_UNSAFE`, never raw path canaries;
- the direct apply `Io` mapper also removes raw path data.

Focused results:

```text
Stage 011: PASS 10/10
Stage 012: PASS 14/14
Stage 013: PASS 13/13
Stage 014: PASS 11/11
Stage 015: PASS 4/4
schema::tests: PASS 19/19
tools::tests: PASS 96/96
cli::tests::tool_: PASS 18/18
managed_block: PASS 10/10
compiled CLI privacy integration: PASS 1/1
targeted plan/preflight/apply swap: PASS 1/1
direct apply Io privacy mapper: PASS 1/1
```

Full sequential gate:

```text
cargo fmt --check
PASS

rtk cargo clippy --all-targets -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 709/709 (708 unit + 1 compiled-CLI integration)

rtk cargo test --tests --locked
PASS 709/709 (708 unit + 1 compiled-CLI integration)

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Requirements verified:

- `RC5-TOL-001`;
- `RC5-ALS-001`;
- `RC5-ALS-002`;
- `RC5-DUP-001`;
- `RC5-DUP-002`;
- `RC5-DUP-003`;
- `RC5-CGD-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stages 011–015: `VERIFIED`;
- cumulative audit through Stage 015: `PASS`;
- verified through: `STAGE_015`;
- current stage: `STAGE_016`;
- next cumulative audit: `STAGE_020`;
- no Stage 016+ requirement was advanced.

## Stage 016

Status: `DONE_LOCAL_CHECKS_PASSED`.

Objective:

- identify the exact Windows error-87 publication request;
- enumerate every required publish caller;
- freeze one typed, ownership-safe Stage 017 migration boundary;
- preserve pending-marker, backup, export, privacy, and complexity semantics.

Source result:

```text
Windows open
CreateFileW with FILE_SHARE_READ | FILE_SHARE_WRITE
FILE_SHARE_DELETE absent

Windows publish
SetFileInformationByHandle(FileRenameInfo)
FILE_RENAME_INFO.RootDirectory = held parent handle
source handle remains open through handle-relative rename

field result
ERROR_INVALID_PARAMETER / os error 87
```

Production publication boundaries:

```text
audit memory.sql replace
audit Git loose-object no-replace
audit Git HEAD/ref replace
SQLite backup no-replace
debug capsule no-replace
upgrade adapter/assets durable replace-or-create
```

Current installer binary publication is outside Rust (`mv -f` or
`[IO.File]::Replace/Move`). Stages 021–022 must route it through the recovery
binary and the same Rust module.

Frozen Stage 017 remediation:

- one `src/platform_publish.rs`;
- owned source `File`;
- `PublishMode::{ReplaceOrCreate, NoReplace}`;
- typed strategy, phase, outcome, and failure details;
- validate and flush exact source;
- close source and destination validation handles;
- Windows existing replace via `ReplaceFileW`;
- Windows absent/no-replace via
  `MoveFileExW(MOVEFILE_WRITE_THROUGH)`;
- reopen and validate destination;
- retain anchored `renameat` / `linkat + unlinkat` semantics on Unix;
- no shell, PowerShell, manual SQLite, admin, or second framework.

Artifacts:

- `.devplan/RC5_WINDOWS_PUBLISH_ROOT_CAUSE.md`;
- `.devplan/RC5_WINDOWS_PUBLISH_REPORT.md`;
- `.devplan/RC5_HANDOFFS/STAGE_016.md`.

Focused checks:

```text
audit snapshot and Git tests
PASS 26/26 audit snapshot tests; 5/5 anchored Git tests

upgrade backup tests
PASS 5/5 on macOS; Windows-only runtime tests not executed

debug capsule export tests
PASS 18/18

upgrade apply tests
PASS 16/16

mutation marker tests
PASS 16/16

cargo fmt --check
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Proof boundary:

- source/static root-cause: `PASS`;
- native Windows runtime: `PENDING_DOGFOOD`;
- Stage 017 implementation: `TODO`;
- verified through remains `STAGE_015`;
- next cumulative audit remains `STAGE_020`;
- P1: `0`;
- P2: `0`.

## Stage 017

Status: `DONE_LOCAL_CHECKS_PASSED`.

Implemented one `src/platform_publish.rs` boundary. Migrated audit
`memory.sql`, Git objects and refs, online SQLite backup, debug capsule, and
managed adapter/assets.

Windows source contract:

```text
existing validated destination
ReplaceFileW(destination, source, NULL, 0, NULL, NULL)

absent or no-replace destination
MoveFileExW(source, destination, MOVEFILE_WRITE_THROUGH)

MOVEFILE_REPLACE_EXISTING
absent

FileRenameInfo regular-file publish
absent
```

The helper owns the source `File`, validates direct children, captures
identity and size, flushes, immediately rechecks, closes conflicting handles,
publishes, reopens, and validates final identity. Errors use private static
endpoint roles and typed partial state. Races are bounded. Unix uses anchored
`renameat` and `linkat + unlinkat`. Complexity remains `O(1)`.

```text
focused platform/audit/Git/backup/export/apply/mutation
PASS 93/93

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 716/716

rtk cargo test --tests --locked
PASS 716/716

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Native Windows and Windows cross-build remain `PENDING_DOGFOOD`: local
`rustup`/target is absent. No Windows runtime PASS is claimed.

Requirements advanced:

- `RC5-WIN-001`;
- `RC5-WIN-002`;
- Stage 017 portion of `RC5-TST-001`.

Audit state:

- P1: `0`;
- P2: `0`;
- Stage 017 is not `VERIFIED`;
- verified through remains `STAGE_015`;
- next cumulative audit remains `STAGE_020`;
- next implementation stage after this Stage 017 proof was `STAGE_018`.

## Stage 018

Status: `DONE_LOCAL_CHECKS_PASSED`.

Implemented `aopmem platform check --json` as an early, non-observed command.
It ignores poisoned `AOPMEM_HOME`, never resolves a workspace or opens either
database, and works only in one private UUID-named OS temp directory.

The shared Atomic Publish V2 helper proves create/flush/no-replace/replace,
reopen bytes, unchanged existing destination, direct-child rejection, shared
reparse guards, and bounded non-recursive cleanup. Error `87` stays structured
and path-private. Cleanup runs on all injected failure phases.

```text
focused platform, reparse, and compiled CLI isolation tests
PASS 5/5

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 720/720

rtk cargo test --tests --locked
PASS 720/720

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Native Windows remains `PENDING_DOGFOOD`. Verified through remains
`STAGE_015`. Current implementation stage is `STAGE_019`. P1: `0`. P2: `0`.

## Stage 019

Status: `DONE_LOCAL_CHECKS_PASSED`.

Implemented official current/all-workspaces audit repair. Per workspace the
command takes only the snapshot lock, opens live operational SQLite in URI
`mode=ro` with `query_only`, streams one canonical redacted snapshot, publishes
through Atomic Publish V2, validates the reopened SHA-256 digest, records
truthful Git `Created`/`Unchanged`, and clears the pending marker last.

Failure injection proves publish error `87`, digest mismatch, Git failure, and
marker-clear failure retain or restore the marker. DB/WAL bytes remain
unchanged with a live WAL-only row. Bounded stable discovery continues after
unsafe/reparse entries and returns partial failure. Observability runs only
after core result.

```text
focused Stage 019
PASS 5/5

doctor/verify exact repair hint
PASS 1/1

cargo fmt --all -- --check
PASS

rtk cargo clippy --all-targets --locked -- -D warnings
PASS

rtk cargo build --locked
PASS

rtk cargo test --locked
PASS 725/725

rtk cargo test --tests --locked
PASS 725/725

rtk ./scripts/dev_verify.sh
PASS

git diff --check
PASS

jq empty .devplan/RC5_EXECUTION_LEDGER.json
PASS
```

Independent review: `PASS`; P1 `0`; P2 `0`.

Native Windows remains `PENDING_DOGFOOD`. Verified through remains
`STAGE_015`. Current implementation stage is `STAGE_020`.

## Stage 020 — Debug capsule and audit snapshot integration

Status: `DONE_LOCAL_CHECKS_PASSED`.

Implemented and proved:

- exact ordered 12-entry deterministic Stored ZIP64 capsule unchanged;
- shared Atomic Publish V2 `NoReplace` boundary with owned source handle;
- one shared Win32 verbatim drive/UNC converter for anchored opens and publish;
- OS temp, Unicode, existing destination, long normal path, unsafe parent;
- private typed error `87`, no destination, no temporary leak, marker unchanged;
- committed/final-validated durability uncertainty remains warning success;
- live-WAL URI read-only, `query_only=ON`, DB/WAL bytes unchanged;
- no observability self-write or missing-store creation;
- raw tagged values and stored canonical JSON-string copies redacted.

```text
focused export 22/22
focused structured CLI error 1/1
focused Windows path conversion 2/2
focused platform publisher 7/7
cargo fmt --all -- --check PASS
rtk cargo clippy --all-targets --locked -- -D warnings PASS
rtk cargo build --locked PASS
rtk cargo test --locked PASS 732/732
rtk cargo test --tests --locked PASS 732/732
rtk ./scripts/dev_verify.sh PASS
git diff --check PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json PASS
```

Independent review: `PASS`; P1 `0`; P2 `0`.

Native Windows runtime: `PENDING_DOGFOOD`.

### Cumulative 016–020 audit remediation

Initial cumulative audit result: `FAIL`; P1 `1`; P2 `0`.

Finding: normal `write_sql_snapshot` cleared `.pending-snapshot` directly.
An unlink followed by failed parent sync could return failure after the marker
was absent.

Remediation: normal snapshots now use the shared restore-on-clear-error helper.
The injected post-remove durability test proves published `memory.sql`,
committed Git (`Unchanged` on replay), returned failure, and restored marker.

```text
focused restoration 2/2 PASS
cargo fmt --all -- --check PASS
rtk cargo clippy --all-targets --locked -- -D warnings PASS
rtk cargo build --locked PASS
rtk cargo test --locked PASS 733/733
rtk cargo test --tests --locked PASS 733/733
rtk ./scripts/dev_verify.sh PASS
git diff --check PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json PASS
```

Cumulative audit status after independent re-audit: `PASS`; P1 `0`; P2 `0`.

## Cumulative audit — Stages 016–020

Status: `PASS`.

The first pass found one P1 marker-durability defect in the normal snapshot
path. The adjacent Stage 020 remediation routed marker clearing through the
shared restore-on-clear-error boundary. Independent re-audit read the exact
patch and accepted the regression proof.

```text
focused marker restoration PASS 2/2
cargo fmt --all -- --check PASS
rtk cargo clippy --all-targets --locked -- -D warnings PASS
rtk cargo build --locked PASS
rtk cargo test --locked PASS 733/733
rtk cargo test --tests --locked PASS 733/733
rtk ./scripts/dev_verify.sh PASS
git diff --check PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json PASS
```

Verified through: `STAGE_020`.

Next implementation stage: `STAGE_021`.

Next cumulative audit: `STAGES_021_025`.

Native Windows runtime remains `PENDING_DOGFOOD`.
# Stage 021 — official RC5 upgrade core

Status: `DONE_LOCAL_CHECKS_PASSED`.

Initial independent review: `FAIL`; P1 `7`; P2 `5`.

P1-A remediation:

- retained staged binary replay is idempotent and keeps executable mode;
- full-home copy is anchored and rejects symlink/reparse traversal;
- journal binds home identity and the semantically checked backup manifest.

P1-B remediation:

- journal freezes exact workspace root, database, Observability, and schema
  identities;
- drift is checked before and inside core apply;
- Observability v2 is mandatory before core and during reconciliation;
- core-only apply keeps post-publish audit, adapter, doctor, verify, and
  `update.completed` work deferred.

P1-C remediation:

- native `backup`, `stage --artifact --sha256`, `apply --all-workspaces`, and
  `publish` are separate commands;
- journal phases are immutable ordered `NoReplace` checkpoints and require
  confirmed durability;
- missing first/middle checkpoints and oversized journals are rejected;
- deterministic backup manifest production streams with bounded entries,
  per-directory entries, depth, and bytes;
- fault hooks prove every effect/checkpoint window, one core invocation,
  binary publication ordering, mixed `001`/`003`, and Observability v1-to-v2;
- operator docs use the exact Stage 022 order.

Focused upgrade coverage:

| Proof ID | Local coverage |
|---|---|
| UPG-01 | integrated schema `001` recovery apply |
| UPG-02 | integrated schema `003` recovery apply |
| UPG-03 | one mixed two-workspace frozen plan and apply |
| UPG-04 | both workspaces prove operational `004` |
| UPG-05 | existing apply logical/byte preservation suite |
| UPG-06 | schema/apply tool-alias preservation suite |
| UPG-07 | integrated exact Observability v1-to-v2 open |
| UPG-08 | Stage 019 pending audit repair suite; installer wiring is Stage 022 |
| UPG-09 | Stage 018 private failed-check proof; installer order is Stage 022 |
| UPG-10 | apply backup/failure evidence suite |
| UPG-11 | publish phase guard and crash-before-checkpoint replay |

```text
cargo test upgrade:: --locked                         PASS 58/58
focused CLI native command parse                      PASS 1/1
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 752/752
cargo test --tests --locked                           PASS 752/752
./scripts/dev_verify.sh                               PASS
./scripts/audit_v020_installers.sh                    PASS 11 groups
git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
native Windows runtime                               PENDING_DOGFOOD
```

The A/B/C remediation has local proof. No final independent re-audit PASS is
claimed here. Verified-through remains `STAGE_020`; next stage is
`STAGE_022`.

## Stage 021 — remaining re-audit remediation

The remaining re-audit found five additional gaps:

1. old `rc1`–`rc4` binaries cannot call a new pre-download backup command;
2. committed/validated/clean Windows `ReplaceFileW` with unconfirmed
   directory durability must not become a manual hard-failure loop;
3. serialized journal bytes need a pre-write 1 MiB limit;
4. recovery publish CLI errors need complete path-private typed details and a
   phase-aware fix hint;
5. idempotent retain/publish returns must clean stale temporaries first.

Local remediation:

- `upgrade backup --adopt <sibling> --manifest-sha256 <hex>` validates the
  anchored backup manifest against the unchanged current home and creates no
  second backup;
- Stage 22 owns the shell/PowerShell pre-download backup producer;
- installed-binary replacement alone accepts committed, final-validated,
  cleaned durability uncertainty as
  `UPGRADE_BINARY_DURABILITY_UNCONFIRMED`, then writes strict immutable
  `published`; every Published replay validates the installed digest;
- journals reject serialized output above 1 MiB before opening a temporary;
- CLI JSON exposes operation, source/destination roles, mode, phase, strategy,
  I/O kind, OS code, final validation, commit, durability, and cleanup without
  paths;
- stale retain and publish temporaries are removed before existing-file
  idempotent success.

```text
cargo test upgrade:: --locked                         PASS 62/62
focused CLI native/adopt/error/warning contract       PASS 3/3
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 758/758
cargo test --tests --locked                           PASS 758/758
./scripts/dev_verify.sh                               PASS
./scripts/audit_v020_installers.sh                    PASS 11 groups
git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
native Windows runtime                               PENDING_DOGFOOD
```

Status remains `DONE_LOCAL_CHECKS_PASSED`. This is remediation evidence, not
a claimed independent re-audit PASS. Verified-through remains `STAGE_020`.

## Stage 021 — final Published replay and cleanup remediation

Final findings:

- P1: phase `published` validated the installed digest but could not repair a
  missing or mismatched installed binary;
- P2: crash temporaries for immutable journal and current-home-manifest work
  had no bounded recovery cleanup.

Local remediation:

- only explicit `upgrade publish` may republish the installed binary from the
  verified retained artifact after `published`;
- repair preserves the retained artifact, does not rewrite the immutable
  chain, reports `binary_published=true`, and never calls core apply;
- every recovery entry runs a capped parent scan and removes only exact direct
  regular UUID temp names for the journal and current-home manifest;
- scan/removal overflow and matching unsafe, symlink, or reparse entries fail
  closed before unsafe removal;
- an idempotent existing checkpoint still performs cleanup.

```text
cargo test upgrade:: --locked                         PASS 63/63
focused CLI native/adopt/error/warning contract       PASS 3/3
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 759/759
cargo test --tests --locked                           PASS 759/759
./scripts/dev_verify.sh                               PASS
./scripts/audit_v020_installers.sh                    PASS 11 groups
git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
native Windows runtime                               PENDING_DOGFOOD
```

Status remains `DONE_LOCAL_CHECKS_PASSED`. No independent re-audit PASS is
claimed. Verified-through remains `STAGE_020`; Stage 022 is next.

Final independent Stage 021 re-audit: `PASS`; P1 `0`; P2 `0`.

Native Windows remains `PENDING_DOGFOOD`.

## Stage 022 — official installer integration

Status: `DONE_LOCAL_CHECKS_PASSED`.

The shell and PowerShell installers now fail before filesystem changes unless
an explicit exact adapter/file pair is supplied. Update orchestration delegates
backup adoption, stage, apply, and publish to the verified RC5 binary; it never
retries apply. Full-home manifest producers are bounded no-follow DFS and the
installer audit proves rc1–rc4/version rejection, failure stops, command order,
task-start smoke, selected adapter handling, observability, and capsule export.

```text
scripts/audit_v020_installers.sh                     PASS 14 groups
cargo test --locked upgrade::recovery::tests::installer_backup_adoption
                                                    PASS 2/2
cargo fmt --all -- --check                           PASS
cargo clippy --all-targets --locked -- -D warnings   PASS
cargo build --locked                                 PASS
git diff --check                                     PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json          PASS
native Windows runtime                               PENDING_DOGFOOD
```

Verified-through remains `STAGE_020`; next stage is `STAGE_023`.

### Stage 022 P1 re-open remediation

The initial shell producer used recursive global variables, which could write
the wrong relative sibling after a directory. It now uses positional recursion
without mutable path state. Shell and PowerShell producers additionally enforce
the exact per-directory, total-entry, depth, and manifest-byte bounds. The
audit's real RC5 adoption case contains hidden, directory/file collision, and
Unicode paths and passed through the actual `target/debug/aopmem` adopter.

```text
scripts/audit_v020_installers.sh                     PASS 14 groups
cargo test --locked upgrade::recovery::tests::installer_backup_adoption
                                                    PASS 2/2
cargo fmt --all -- --check                           PASS
cargo clippy --all-targets --locked -- -D warnings   PASS
cargo build --locked                                 PASS
git diff --check                                     PASS
native Windows runtime                               PENDING_DOGFOOD
```

The final remediation moved the shell source limits and no-follow validation
before backup-root creation and copy. It includes nested `MANIFEST.sha256`
files, rejects the reserved root name without mutation, and creates the backup
manifest no-replace. PowerShell uses bounded per-file durable copies and a
flushed create-new manifest. Exact adapter pairs are case-sensitive and are
validated before TLS, console, environment, or filesystem changes.

Final independent Stage 022 re-audit: `PASS`; P1 `0`; P2 `0`.

## Stage 023 — effectiveness report and minimal UI

Status: `DONE_LOCAL_CHECKS_PASSED`.

The factual report now covers task starts, apply transitions, starts without
apply, completions/failures, six application kinds, both context kinds,
duplicate blocks, alias resolutions, unresolved overlaps, pending snapshots,
and last successful repair. It adds no score and stores no task history in
operational memory.

The minimal UI renders those facts and adds canonical IDs, aliases, duplicate
classes, superseded duplicate evidence, and unresolved overlaps to Tools.
Duplicate planning is bounded and read-only. An unavailable plan produces
empty facts plus `duplicate_analysis_complete=false` and `complete=false`.

The final compatibility review removed three proposed same-version v2 indexes.
Queries instead use the existing task indexes under the unchanged exact schema
manifest and the hard 30-day/100,000,000-byte store bound.

```text
cargo fmt --all -- --check                            PASS
node --check src/ui/assets/app.js                     PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked --all-targets                    PASS
cargo test --locked                                   PASS 763/763
bash scripts/dev_verify.sh                            PASS
focused report/index/UI/read-only tests               PASS 6/6
git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
native Windows runtime                               PENDING_DOGFOOD
```

Self-review: P1 `0`; P2 `0`. Verified-through remains `STAGE_020`; next stage
is `STAGE_024`.

Independent high review: `PASS`; P1 `0`; P2 `0`. The reviewer reran six
mandatory focused tests, 30 report negative/privacy tests, 26 UI tests, and
the exact observability-v2 schema check. All passed. Native Windows remains
`PENDING_DOGFOOD`.

## Stage 024 — native macOS fresh and mixed update proof

Status: `DONE_LOCAL_CHECKS_PASSED`.

Added `scripts/prove_rc5_macos.sh`, a single reproducible native Darwin arm64
proof. It uses the exact tagged v0.1.0-rc3 asset, the exact published rc4
release asset, and a prebuilt rc5 candidate through the installer's isolated
local-asset test boundary. It never opens SQLite directly and never deletes
WAL/SHM or a pending marker.

One clean run passed:

- fresh install with exactly five answers and full health/task/observe/export;
- one shared home with real schema `001` and `003` workspaces;
- a genuine rc4 `AUDIT_SNAPSHOT_PENDING` produced by a failed audit Git commit;
- exact official backup/download/check/repair/prepare/plan/one-apply/publish/
  adapter/repair/health/export order;
- no update init or adapter seed;
- exact database/sidecar bytes in the durable pre-download full-home backup;
- exact CLI node, node-alias, node-tag, and node-source preservation;
- exact tool filesystem bytes;
- only selected `AGENTS.md` updated, with user text preserved;
- both workspaces at schema `004`, observability v2, clean verify, complete
  task start, and successful capsule export;
- pre-download failure with rc4 and both databases unchanged and backup kept.

The selected active rc4 workspace doctor is healthy. The non-selected
schema001 workspace has DB/schema/audit/tools ready and verify clean; only its
adapter is missing, as required by the exact one-selected-adapter contract.

```text
cargo build --locked                              PASS
sh -n scripts/prove_rc5_macos.sh                  PASS
git diff --check -- scripts/prove_rc5_macos.sh    PASS
scripts/prove_rc5_macos.sh                        PASS
cargo fmt --all -- --check                        PASS
cargo test --locked upgrade::                     PASS 63/63
scripts/audit_v020_installers.sh                  PASS 14 groups
git diff --check                                  PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json       PASS
native Windows runtime                            PENDING_DOGFOOD
```

Retained proof root:

```text
/var/folders/cf/2mk2lmy9087c_lw961rpfvz00000gn/T//aopmem-rc5-stage24.q62Phs
```

Summary SHA-256:

```text
0c0f36b0e862f738242df0add5161243381492d588f96404c19db9a70f7dcb33
```

Full details: `.devplan/RC5_MACOS_PROOF_REPORT.md`.

Independent review found one P2 evidence gap: the initially seeded schema001
tag and secondary source were not compared. The harness now extracts both
through bounded CLI lists, normalizes stable fields, and requires exact
pre/post matches. The full clean rerun at the retained root above passed.
Schema003 has no separately seeded node-source row; its node `source_ref` is
already part of the exact node projection.

Self-review: `PASS`; P1 `0`; P2 `0`.

## Stage 025 — clean-agent compliance dogfood

Status: `DONE_LOCAL_CHECKS_PASSED`.

Ten authoritative isolated scenarios prove the complete agent gate across
discussion, clarification, code investigation, code planning, Confluence read,
SMTP/API discussion, authorized test credentials, equivalent-tool reuse,
same-task continuation, and a materially new goal.

```text
authoritative scenarios                            PASS 10/10
task starts before substantive action              PASS 10/10
context applications before substantive action    PASS 10/10
task completions                                   PASS 10/10
started without apply                              PASS 0
mandatory gates and relevant workflow/tool         PASS 10/10
user reminders required                            PASS 0/10
duplicate tools created                            PASS 0
test-secret blanket refusals                       PASS 0
external writes                                    PASS 0
continuation extra task starts/applies             PASS 0/0
materially new goal different task/bundle          PASS
authorized synthetic credential result             PASS AUTH_OK
persistent DOG-09 session                          PASS deleted
native Windows runtime                             PENDING_DOGFOOD
```

The durable transcripts contain normalized facts only. They exclude raw
requests, complete receipts, node bodies, environment dumps, credentials, and
hidden reasoning. Checksums cover all ten JSON records.

Full details: `.devplan/RC5_AGENT_COMPLIANCE_REPORT.md`.

Self-review: `PASS`; P1 `0`; P2 `0`.

## Cumulative audit — Stages 021–025

Status: `PASS`.

Independent review covered the complete Stage 021–025 diff, immutable upgrade
recovery, installer order and exclusions, factual bounded report/UI behavior,
retained native macOS fresh/mixed/failure artifacts, and all ten clean-agent
records with their refreshed checksum manifest.

```text
shell and JavaScript syntax                         PASS
installer static audit                             PASS 14 groups
agent evidence checksums                           PASS 10/10
retained macOS artifact hashes                     PASS 4/4
cargo fmt --all -- --check                         PASS
cargo clippy --all-targets --locked -- -D warnings PASS
cargo build --locked                               PASS
cargo test --locked                                PASS 763/763
cargo test --tests --locked                        PASS 763/763
scripts/dev_verify.sh                              PASS
git diff --check                                   PASS
jq empty RC5_EXECUTION_LEDGER.json                 PASS
native Windows runtime                             PENDING_DOGFOOD
```

Full evidence: `.devplan/RC5_HANDOFFS/AUDIT_021_025.md`.

Audit verdict: `PASS`; P1 `0`; P2 `0`.

## Stage 026 — focused RC5 performance proof

Status: `DONE_LOCAL_CHECKS_PASSED`.

`scripts/benchmark_rc5_stage26.py` ran against the current `target/debug/aopmem`
binary with three isolated CLI-created corpora (16, 64, and 256 extra active
rules), three warmups, and 15 raw samples per operation/corpus. It stores
nearest-rank p95, raw samples, metadata, bounds evidence, and integrity hashes
under `.devplan/benchmarks/rc5_stage26/`.

```text
python3 -m py_compile scripts/benchmark_rc5_stage26.py  PASS
focused benchmark clean run                             PASS
(cd .devplan/benchmarks/rc5_stage26 &&
  shasum -a 256 -c SHA256SUMS)                          PASS 4/4
task start median/p95, small                            20.039 / 22.061 ms
task start median/p95, medium                           25.526 / 27.968 ms
task start median/p95, large                            46.891 / 65.797 ms
real alias-resolution test API benchmark                PASS 3 corpora, 15+3
alias evidence hashes                                   PASS 2/2
bounded scans and no normal-run N+1 hashes              PASS
native Windows runtime                                  PENDING_DOGFOOD
```

No before/after comparison exists, so no percentage improvement is claimed.
No production code changed. Full method, all operation rows, and the exact
canonical fast-path and real-alias scopes are in
`.devplan/RC5_PERFORMANCE_REPORT.md`.

Self-review: P1 `0`; P2 `0`. Verified-through remains `STAGE_025`; next stage
is `STAGE_027`.

## Stage 027 — full negative/security regression suite

Status: `DONE_LOCAL_CHECKS_PASSED`.

The full §24 task, Managed Block, Memory Keeper, secret, tool, Windows,
upgrade, observability, and UI catalog is mapped to exact tests in
`.devplan/RC5_REGRESSION_REPORT.md`.

A focused CLI tool-run regression reproduced `ProcessFailed(-1)` on the second
short macOS execution. PID/identity tracing proved the failing process received
`SIGKILL` while the runner's live cleanup set was empty and no signal function
was called. A standalone reproducer then proved native endpoint-security
rejection of newly executed hardlink anchors (`137` at iteration 9).

The macOS stable launch snapshot now uses fd-bound `fclonefileat` with
`CLONE_NOOWNERCOPY`. Only `ENOTSUP`/`EXDEV` use a byte- and metadata-bounded
fd-copy fallback. Exact source state is checked before and after snapshotting.
Process cleanup uses bounded `proc_listpgrppids`, identity-safe member signals,
and still kills a fast same-pgid orphan and escaped setsid descendants.

```text
isolated repeated short tool run                      PASS 100/100 twice
forced clone fallback                                PASS
in-place source mutation fail-closed                 PASS
fast same-pgid orphan cleanup                        PASS
original CLI regression                              PASS
cargo fmt --all -- --check                           PASS
cargo clippy --all-targets --locked -- -D warnings   PASS
cargo build --locked                                 PASS
cargo test --locked                                  PASS 768/768
cargo test --tests --locked                          PASS 768/768
scripts/dev_verify.sh                                PASS
scripts/audit_v020_installers.sh                     PASS 14 groups
git diff --check                                     PASS
jq empty RC5_EXECUTION_LEDGER.json                   PASS
native Windows runtime                              PENDING_DOGFOOD
```

Stage 026 benchmark artifacts were unchanged. Temporary tracing was removed.

Self-review: `PASS`; P1 `0`; P2 `0`. Verified-through remains `STAGE_025`;
next stage is `STAGE_028`.

## Stage 028 — release assets, documentation, and checksums

Status: `DONE_LOCAL_CHECKS_PASSED`.

The final flat assets were rebuilt from the RC5 source. The Windows build first
exposed three Windows-only compile errors: a path metadata identity call absent
on Windows and two non-const enum comparisons in const strategy selectors. The
adjacent AUTO_PATCH_WINDOW fix uses a handle-derived `WorkspaceIdentity` on
Windows and const-compatible `match` selectors. No test was weakened.

```text
Darwin asset SHA-256  594bb9606bd7f971a0fb97b16916fe2a5da84096e8340a5885c36d7037dd1b5e
Windows asset SHA-256 150db4699c2f41c6e529f9606ac099c9ac6b4771b5084952f2cb5df3226d1b58
SHA256SUMS SHA-256    6236d2cf502df5036609f202f541e38a12173321a0a85fbc83e388ed4548213a
```

```text
cargo fmt --all -- --check                            PASS
cargo test --locked                                   PASS 768/768
cargo clippy --all-targets --locked -- -D warnings    PASS
Darwin release build, Mach-O arm64, min macOS 11.0    PASS
Windows x86-64 release build #1, PE/import check      PASS
Windows x86-64 release build #2, unchanged hash       PASS
(cd dist && shasum -a 256 -c SHA256SUMS)              PASS 2/2
git diff --check                                      PASS
jq empty RC5_EXECUTION_LEDGER.json                    PASS
native Windows runtime                                PENDING_DOGFOOD
```

The second Windows build produced the exact first hash. Its imports are only
`KERNEL32.dll`, `shell32.dll`, `api-ms-win-core-synch-l1-2-0.dll`,
`bcryptprimitives.dll`, `WS2_32.dll`, `userenv.dll`, `ntdll.dll`, and
`advapi32.dll`; it has no dynamic MSVC/UCRT import. Windows product-runtime
proof was not attempted and remains `PENDING_DOGFOOD`.

Stage 026 benchmark evidence was unchanged. Self-review: `PASS`; P1 `0`; P2
`0`. Verified-through remains `STAGE_025`; next stage is `STAGE_029`.

## Stage 029 — independent global audit and 15 requirement sweeps

Status: `DONE_LOCAL_CHECKS_PASSED`.

Independent audit of the full RC5 diff from `0af9b22` passed all 15 separate
requirement sweeps. The detailed evidence, files, focused-test mapping, and
open-finding status are in `.devplan/RC5_GLOBAL_AUDIT_REPORT.md`.

```text
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 768/768
cargo test --tests --locked                           PASS 768/768
scripts/dev_verify.sh                                 PASS
scripts/audit_v020_installers.sh                      PASS 14 groups
scripts/prove_rc5_macos.sh                            PASS
benchmark SHA256SUMS                                  PASS 4/4
dogfood evidence SHA256SUMS                           PASS 10/10
dist SHA256SUMS                                       PASS 2/2
git diff --check                                      PASS
jq empty RC5_EXECUTION_LEDGER.json                    PASS
native Windows runtime                                PENDING_DOGFOOD
```

No product code was changed. The audit found P1 `0` and P2 `0`. It does not
claim native Windows runtime PASS and did not commit, push, tag, release,
install on real Windows, or delete backups.

Next stage: `STAGE_030` final RC report, matrix closure, DoD proof, and local
stop-condition check.

## Stage 030 — final RC report, matrix closure, and local stop condition

Status: `COMPLETE_LOCAL_PENDING_CUMULATIVE_AUDIT`.

The release-candidate report, matrix closure, exact 32-row DoD table, and
local stop-condition proof are complete. The report references Stage025's
10/10 privacy-safe dogfood evidence, Stage026 raw benchmark evidence, Stage027
negative regressions, Stage028 reproducible flat assets, and Stage029's fifteen
independent sweeps.

```text
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 768/768
cargo test --tests --locked                           PASS 768/768
scripts/dev_verify.sh                                 PASS
scripts/audit_v020_installers.sh                      PASS 14 groups
scripts/prove_rc5_macos.sh                            PASS
benchmark SHA256SUMS                                  PASS 4/4
dogfood evidence SHA256SUMS                           PASS 10/10
dist SHA256SUMS                                       PASS 2/2
git diff --check; ledger JSON                         PASS
native Windows runtime                                PENDING_DOGFOOD
```

The Stage029 global audit remains the independent finding authority: P1 `0`,
P2 `0`. Native Windows was not installed or run. No commit, push, tag, GitHub
Release, real Windows install, or backup deletion occurred. Next action is the
independent `STAGES_026_030_CUMULATIVE_AUDIT`.

## Cumulative Audit 026–030

Status: `PASS`.

The independent audit verified Stages 026 through 030. It re-ran the required
local gates and checked the Stage26 main/alias evidence manifests, Stage27
macOS snapshot and process-group regression, Stage28 retained two-build asset
proof plus final bytes/imports, Stage29 fifteen sweeps, and Stage30's 301-line
RC report, matrix, and DoD 32/32 closure.

```text
cargo fmt / clippy / build                            PASS
cargo test --locked / --tests                         PASS 768/768 each
cargo test --locked macos_                            PASS
dev_verify / installer audit / macOS proof            PASS
benchmark manifests                                   PASS 4/4 and 2/2
dogfood evidence / dist manifests                     PASS 10/10 and 2/2
ledger JSON / git diff --check                        PASS
native Windows runtime                                PENDING_DOGFOOD
```

P1 `0`; P2 `0`. Stages 026–030 are `VERIFIED`; local release state is
`COMPLETE_LOCAL_RELEASE_READY`. No commit, push, tag, GitHub Release, real
Windows install, or backup deletion occurred.
