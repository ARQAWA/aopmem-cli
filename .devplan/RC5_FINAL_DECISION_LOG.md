# AOPMem v0.2.0-rc5 Final Decision Log

Status: `FROZEN_AT_STAGE_02`

This log resolves RC5 implementation ambiguities without changing the product
goal. The supplied RC5 specification remains authoritative. Older v0.2
decision logs remain historical and are inherited only where they do not
conflict with this file.

## Frozen decisions

| ID | Decision | Owner | Status |
|---|---|---:|---|
| RC5-D-001 | Baseline is clean `v0.2.0-rc4` at `0af9b22`; no recovery or user hunk merge is needed | 01 | frozen |
| RC5-D-002 | RC5 uses prefixed `.devplan/RC5_*` state; completed generic/v0.2 ledgers remain immutable history | 01–02 | frozen |
| RC5-D-003 | Preserve one Rust crate, SQLite/FTS5, local workspaces, separate observability, native Memory Keeper, and read-only UI | all | frozen |
| RC5-D-004 | Authoritative task/bundle state lives only in Local Observability v2, never in operational memory | 04 | frozen |
| RC5-D-005 | A task-start response is returned only after its validation state is durably stored; required state-write failure fails closed | 04–05 | frozen |
| RC5-D-006 | Non-authoritative observability projection failure remains best-effort and cannot mutate or corrupt operational memory | 04–06 | frozen |
| RC5-D-007 | `task_id` and `bundle_id` are lowercase UUID v4; state transitions are `started -> applied -> completed|failed` | 04–06 | frozen |
| RC5-D-008 | Exact replay of apply/complete is idempotent; conflicting replay fails with a typed state error | 06 | frozen |
| RC5-D-009 | Keep required `--query`; add mutually exclusive `--query-stdin`; managed Memory Keeper sends the exact request by direct stdin, never shell interpolation | 05, 07, 10 | frozen |
| RC5-D-010 | Raw task query, raw chat, raw output, and hidden reasoning are never persisted | 04–06, 10 | frozen |
| RC5-D-011 | Recall budget unit remains canonical JSON UTF-8 bytes: mandatory 1 MiB hard, task 256 KiB soft | 05 | frozen |
| RC5-D-012 | Budget exhaustion returns a valid bounded package with `budget_exhausted=true` and `retrieval_complete=false` | 05, 07 | frozen |
| RC5-D-013 | Ordinary `task start` consumes continuation internally on one revision-bound read context; no shell recursion | 05 | frozen |
| RC5-D-014 | `MANDATORY_CONTEXT_OVERFLOW` and memory-unavailable paths fail closed with no partial success advice | 05 | frozen |
| RC5-D-015 | `--none-relevant` is valid only after complete retrieval with zero task nodes; never after budget exhaustion | 06 | frozen |
| RC5-D-016 | Apply validates workspace, task, bundle, revision, membership, and active status before recording facts | 06 | frozen |
| RC5-D-017 | Completion duration is derived from stored start time; bounded redacted reason is optional | 06 | frozen |
| RC5-D-018 | Same goal reuses current receipt; new independent goal, project, work type, or unreliable post-compaction receipt starts a new task | 07–08 | frozen |
| RC5-D-019 | `templates/managed-block/AGENTS.managed-block.md` is the one canonical block body; Rust must not maintain a second handwritten body | 03, 08 | frozen |
| RC5-D-020 | Installer passes the active instruction file explicitly; ambiguity fails closed and never updates multiple adapters | 08, 22 | frozen |
| RC5-D-021 | Standalone explicit `adapter --file` remains supported; legacy default `AGENTS.md` remains compatibility behavior, not active-adapter detection | 08 | frozen |
| RC5-D-022 | Test credentials may be used; exact persistence needs explicit remember/teach trigger; no extra `+++` for secret presence | 09–10 | frozen |
| RC5-D-023 | Exact explicitly stored secret is tagged `sensitivity:test_secret` by the agent flow | 09–10 | frozen |
| RC5-D-024 | Audit snapshot, debug capsule, reports, errors, task summaries, and session evidence are export surfaces and redact tagged exact values | 10, 19–20 | frozen |
| RC5-D-025 | Redaction marker is exactly `<TEST_SECRET_REDACTED>`; operational backup preserves exact authorized values | 10, 21 | frozen |
| RC5-D-026 | Active canonical tool IDs cannot be shadowed; alias resolution may override only a superseded old ID | 11–14 | frozen |
| RC5-D-027 | Alias targets are direct active canonical IDs; alias-to-alias, cycles, duplicate directories, and executable copies are forbidden | 11–14 | frozen |
| RC5-D-028 | Displayed duplicate class and exact-only eligibility are separate facts; eligibility requires equal canonical fingerprint | 12–15 | frozen |
| RC5-D-029 | The field Confluence pair keeps class `SAME_IMPLEMENTATION_DIFFERENT_NAME`; equal canonical fingerprints make it exact-only eligible | 12–15 | frozen |
| RC5-D-030 | Fingerprints run only during create/dedupe preflight; shortlist first and hash each implementation file once per operation | 12–15, 26 | frozen |
| RC5-D-031 | Canonicalization marks duplicate contracts superseded, creates aliases, and never deletes directories or executables | 14–15 | frozen |
| RC5-D-032 | Stage 04 owns only Observability v2; Stage 11 owns immutable operational migration `004_task_protocol_and_tool_aliases` | 04, 11 | frozen |
| RC5-D-033 | One Windows publish module serves backup, audit snapshot, and debug export; no second filesystem framework | 16–20 | frozen |
| RC5-D-034 | Windows replace uses `ReplaceFileW`; no-replace uses `MoveFileExW(WRITE_THROUGH)` after all conflicting handles close | 17 | frozen |
| RC5-D-035 | `audit repair --all-workspaces` attempts every workspace in stable order; overall result fails if any workspace fails; successful repairs remain committed | 19 | frozen |
| RC5-D-036 | Pending marker removal occurs only after publish validation and local Git commit; every failure retains the marker | 19 | frozen |
| RC5-D-037 | Upgrade writes a durable recovery journal and retains a verified staged rc5 binary before apply | 21 | frozen |
| RC5-D-038 | After successful apply, retry may resume publish/post-publish work but must never invoke apply again | 21–22 | frozen |
| RC5-D-039 | Operational `004` and Observability v2 migrations are independently detected and applied once | 21 | frozen |
| RC5-D-040 | Supported sources are v0.1.0-rc3, compatible noncanonical v0.1, rc1–rc4, active side-by-side rc4, and mixed `001/003` workspaces | 21–22 | frozen |
| RC5-D-041 | Dogfood uses isolated local fixtures and deterministic fake credentials; no real external mutation or real credential | 25 | frozen |
| RC5-D-042 | RC5 has no invented latency SLA; Stage 26 records raw median/p95 and proves structural bounds and no normal-run N+1 hashes | 26 | frozen |
| RC5-D-043 | macOS-hosted unit/cross-build/PE/hash proof is distinct from native Windows runtime proof, which remains pending | 17–18, 24, 28–30 | frozen |
| RC5-D-044 | Stages finish as `DONE_LOCAL_CHECKS_PASSED` until covered by the cumulative audit after Stage 05 | 01–05 | frozen |
| RC5-D-045 | P1 means data/security/platform-contract or release-blocking failure; P2 means required behavior/proof gap; both must be zero | 29–30 | frozen |
| RC5-D-046 | Original local-only stop boundary from the RC5 specification | 30 | superseded by RC5-D-047 |
| RC5-D-047 | User explicitly authorizes commit, push, new `v0.2.0-rc5` tag, and GitHub Release only after 100% local proof; real Windows install and backup deletion remain forbidden | 30, release | frozen |

## High-risk resolution notes

### Duplicate class and `--exact-only`

The field audit label is preserved. The dedupe engine also computes a canonical
fingerprint that excludes identity and cosmetic fields.

`dedupe apply --exact-only` eligibility is:

```text
canonical_fingerprint(left) == canonical_fingerprint(right)
```

The displayed class may remain `SAME_IMPLEMENTATION_DIFFERENT_NAME`. This
allows the Confluence fixture to canonicalize without treating
`POSSIBLE_OVERLAP` as exact and without a Confluence-specific branch.

### Exact secrets and audit snapshots

Operational memory may contain an exact value only after the explicit user
trigger. Durable full-home backup preserves it.

`memory.sql` in audit Git is an export surface. It redacts exact values from
nodes tagged `sensitivity:test_secret`. Debug capsules and all other export
surfaces use the same marker. Therefore the audit snapshot is not the recovery
backup for an exact secret; the durable home backup is.

This avoids publishing exact credentials while preserving the explicit local
memory contract.

### Task state and observability failure isolation

Task state is correctness state, not optional telemetry. Start must persist a
bounded record containing:

- task and bundle IDs;
- workspace key;
- memory revision;
- selected bundle node IDs;
- timestamps and lifecycle status;
- no raw query.

Failure to persist this record returns a typed fail-closed error. Best-effort
aggregate/event projection failures remain isolated. Operational memory stays
read-only throughout start/apply/complete.

### Apply-once upgrade recovery

Before migration, the updater retains:

- full-home backup;
- staged verified rc5 binary;
- recovery journal describing pre-apply state.

After successful apply, the journal records the success durably before binary
publish. A later updater run resumes publish and health steps from that
journal. It does not run apply again. The old rc4 binary remains installed but
must not be used against schema `004` during this recovery window.

### Active adapter

Shell name is not a reliable adapter signal. The current instruction file is
the contract.

Install orchestration must pass exactly one explicit file selected from the
active environment:

- Codex/OpenAI: `AGENTS.md`;
- Claude: `CLAUDE.md`;
- Cursor: its active instruction file;
- GitHub Copilot: its active instruction file.

If active context is absent or ambiguous, adapter update fails with a typed
error. It does not guess and does not update all files.

### Secret-bearing task query

Memory Keeper receives the exact user request and sends it through direct
process stdin using the mutually exclusive `--query-stdin` input. It must not
use shell interpolation, a temporary query file, or argv for managed flow.

The required public `--query` form remains supported. Neither input form
persists raw query text. Task state stores only a fingerprint and bounded
redacted summary. Observability, errors, and proof logs never contain the exact
query.

## Error and state rules

- Unknown, expired, wrong-workspace, stale-revision, or foreign-bundle state
  fails closed.
- Active task state follows normal 30-day/100 MB Local Observability
  retention. Missing retained state is an exact error, never reconstructed
  from operational memory.
- Terminal tasks are immutable except exact idempotent replay.
- `task.failed` is used when start/apply/complete cannot satisfy the protocol.
- Error output contains stable code and bounded redacted detail only.
- No failure is converted to success or warning to keep orchestration moving.

## Complexity rules

- Do not optimize benchmark harness or UI scanner leads in RC5.
- Keep current revision fingerprint until measurement proves it unsuitable.
- Add no durable revision counter unless all operational mutation paths update
  it atomically and focused proof shows need.
- Bound internal continuation by emitted-byte budget and explicit scan limits.
- Preserve deterministic source/trust/confidence ordering.
- Keep repair and export streaming.

## Change rule

These decisions are frozen for Stages 03–30.

Change requires:

1. a real product, data-safety, privacy, or platform blocker;
2. exact evidence;
3. explicit decision-log amendment;
4. affected requirement and stage updates;
5. proof and cumulative re-audit.

Implementation preference alone is not a reason to change this log.

## User authorization amendment

On 2026-07-17 the user explicitly requested:

```text
complete the plan
then commit, push, and create the release
```

This supersedes only the original no-push/no-tag/no-release stop boundary.
It does not authorize:

- real Windows workspace installation;
- backup deletion;
- mutation of older published tags;
- release before all local checks, audit, assets, and RC report pass.
