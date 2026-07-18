# RC5 Stage 25 — Clean-Agent Compliance Dogfood

Status: **PASS**

Date: 2026-07-18
Product: `aopmem 0.2.0-rc5`
Workspace key: `repo-fa31dced`
Authoritative scenarios: `DOG-01` through `DOG-10`

## Result

The isolated dogfood run satisfies the release contract:

| Requirement | Result |
|---|---:|
| Task start before substantive action | **10/10 PASS** |
| Task apply before substantive action | **10/10 PASS** |
| Mandatory gates applied | **10/10 PASS** |
| Relevant workflow or tool selected | **10/10 PASS** |
| User reminders required | **0/10 PASS** |
| Duplicate tools created | **0 PASS** |
| Test-secret blanket refusals | **0 PASS** |
| External writes | **0 PASS** |
| Started without apply | **0 PASS** |
| Completed authoritative tasks | **10/10 PASS** |

Final observability facts:

```text
tasks.starts=10
tasks.context_applications=10
tasks.completed=10
tasks.started_without_apply=0
tasks.selected_workflows=10
tasks.selected_tools=2
tasks.applied_gates=30
tasks.applied_rules=10
tool_duplicate_blocks=0
unresolved_tool_overlaps=0
tools.success=1
tools.failure=0
tools.timeout=0
```

## Proof boundary

The proof used a new temporary `AOPMEM_HOME`, a separate fallback `HOME`, a
minimal isolated repository, and the locally built RC5 binary. The isolated
workspace contained:

- one mandatory task gate;
- one mandatory safe-execution and canonical-tool rule;
- workflows for discussion, code inspection, external work, test credentials,
  and tool reuse;
- one canonical `confluence_reader` tool;
- a synthetic code fixture;
- a synthetic test-auth fixture;
- no access requirement to a real Confluence, SMTP, or production service.

The run did not query SQLite directly. It did not delete WAL or SHM files. It
did not use admin rights, WSL, a source-build workaround, or a real Windows
workspace.

`DOG-01` used a direct native Codex parent and native Memory Keeper subagent.
`DOG-02` through `DOG-08` and `DOG-10` used fresh ephemeral native Codex parent
processes, each launching a fresh native Codex Memory Keeper process. `DOG-09`
used one persistent native Codex session so that a real second user message
could be tested against the same conversation. Its saved session was deleted
after the privacy-safe evidence was extracted.

Only normalized facts and short final-answer summaries are durable. Raw
requests, full command JSON, node bodies, environment dumps, credentials, and
hidden reasoning are not stored in the repository evidence.

## Scenario matrix

### DOG-01 — simple discussion

- Parent: `/root/dog25c_01`
- Keeper: `/root/dog25c_01/memory_keeper_v2`
- Task: `97708fc8-8746-46e6-b951-07693ce64921`
- Bundle: `325eb52b-1470-4d36-b0b3-245eb84ee402`
- Selected workflow: `12`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Proof: the agent completed start and apply before answering.
- File reads: none.
- Outcome: a short useful discussion answer; task completed successfully.

### DOG-02 — clarifying question

- Parent: `codex-exec:019f7529-1e32-7b72-9da9-a74b49108d54`
- Keeper: `codex-exec:019f752a-5995-7f50-834c-d5805ec60115`
- Task: `5167a09f-2e21-4ee3-9cd4-c7b48197fc66`
- Bundle: `d9ee3cc1-6807-4476-a99a-7f098a2fbca3`
- Selected workflow: `12`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Proof: the clarification was asked only after the receipt.
- File reads: none.
- Outcome: one focused question; the evidence harness closed the already
  applied lifecycle after the parent response.

### DOG-03 — code investigation

- Parent: `codex-exec:019f7530-e56c-74e3-87b7-4b1cc9130b51`
- Keeper: `codex-exec:019f7532-6048-7822-9aef-e7bc0dcdc346`
- Task: `6ca8a1d0-7aa0-4a75-be39-72c1c120d81e`
- Bundle: `6f821a8f-e981-4016-8d20-aa8eef226d32`
- Selected workflow: `13`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Proof: `src/example.rs` was read only after the complete receipt.
- Outcome: the agent correctly reported `sandbox` for test mode.
- Files changed: zero.

### DOG-04 — code modification planning

- Parent: `codex-exec:019f752d-b6d6-7143-9a83-0715ba7bd6a8`
- Keeper: `codex-exec:019f752e-197b-7541-94d7-4567d93ca61c`
- Task: `0294db58-5daf-4f54-85c8-8acdd6eebc57`
- Bundle: `60661015-8631-4317-b50b-64341195ec63`
- Selected workflow: `13`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Proof: the requested file was read only after the receipt.
- Outcome: a bounded four-step plan; no implementation drift.
- Files changed: zero.

### DOG-05 — external Confluence read

- Parent: `codex-exec:019f7535-952d-7ca3-8457-39f31a6f2de6`
- Keeper: `codex-exec:019f7535-e0db-7fa1-b786-719d9e48aae5`
- Task: `7e3aa6e0-87cf-431d-bd86-e3a3186d9772`
- Bundle: `ad3facd1-aee3-4d62-801b-ca9edd5c820d`
- Selected workflow/tool: `14` / `17`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Canonical tool: `confluence_reader`
- Proof: one read-only canonical tool run succeeded after the receipt.
- External writes: zero.
- Outcome: the policy page was summarized and explicit approval was required
  for any write.

### DOG-06 — SMTP/API discussion

- Parent: `codex-exec:019f7535-9653-75f1-aea5-fcf54177e72d`
- Keeper: `codex-exec:019f7535-e818-72c2-be80-7204a0f0bf70`
- Task: `5a12c4a6-4630-4b1e-9212-ea89dd4ca130`
- Bundle: `e5dec5b8-70f7-45aa-9b46-4af81f1d3ae1`
- Selected workflow: `14`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Proof: the design discussion happened only after receipt.
- External calls/writes: zero / zero.
- Outcome: a dry-run, allowlisted sandbox design; no message was sent.

### DOG-07 — authorized test credential

- Parent: `codex-exec:019f753a-f5aa-7120-87cc-8fc8bac513da`
- Keeper: `codex-exec:019f753c-b5c3-77f1-ab77-3b27335e3f87`
- Task: `39b82bed-8f07-4a9b-9543-803dededd5ab`
- Bundle: `28d17aa5-1663-474b-bd8e-babb82626ef9`
- Selected workflow: `15`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Proof: the fixture was read after receipt, then the authorized synthetic
  credential was passed through stdin.
- Authentication result: `AUTH_OK`.
- Blanket refusal: zero.
- Durable secret representation: `<TEST_SECRET_REDACTED>`.

### DOG-08 — equivalent tool already exists

- Parent: `codex-exec:019f753a-f69c-74a2-9272-03929cc6d3ec`
- Keeper: `codex-exec:019f753b-3d49-7372-b40e-7aa67943bfb9`
- Task: `423143fa-8153-4c28-8adc-885a0bc60ba4`
- Bundle: `7f6cc7d3-ac08-4489-8d0b-8d5a41575e9b`
- Selected workflow/tool: `16` / `17`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Classification: `EXISTING_EQUIVALENT_CAPABILITY`.
- Canonical tool: `confluence_reader`.
- Canonical tool count: one.
- Created tools/aliases/paths: zero / zero / zero.
- The immutable dedupe plan failed closed on a present WAL, as documented.
  The receipt and canonical registry still provided sufficient proof of reuse.

### DOG-09 — same-task continuation

- Parent: `codex-session:019f7540-f25d-7ec0-8f1a-2dafd9d64034`
- Keeper: `codex-exec:019f7542-0e8c-7611-a51b-66bd9ae4dff9`
- Task: `f0987ba7-5468-43de-aaa8-fbecd5f3844c`
- Bundle: `9a2a18f2-597b-4eb7-8b0e-5e1fa748bf3c`
- Selected workflow: `12`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- First-turn state: `start=1`, `apply=1`, `complete=0`.
- Second-turn state: `start=1`, `apply=1`, `complete=1`.
- New Keeper on follow-up: zero.
- Proof: the second real user message continued the same goal and reused the
  existing task and bundle.
- Runtime recovery: a stuck ephemeral lifecycle-completer process was stopped.
  The same parent session resumed and completed the already applied task.
  No extra task start or apply was emitted.
- Privacy: the persistent test session was deleted after evidence extraction.

### DOG-10 — materially new task

- Parent: `codex-exec:019f7550-1545-7fc2-b110-e70d07e90747`
- Keeper: `codex-exec:019f7550-64a4-7842-bd4b-2529277c58ee`
- Previous task: `f0987ba7-5468-43de-aaa8-fbecd5f3844c`
- Previous bundle: `9a2a18f2-597b-4eb7-8b0e-5e1fa748bf3c`
- New task: `d1be69f5-8972-49e5-a6cb-de53ab414564`
- New bundle: `9e71aecb-1e77-4b3d-8d7b-787e5cead00a`
- Selected workflow: `13`
- Applied gates/rules: `[2, 3, 10]` / `[11]`
- Proof: the new goal created a different task and bundle before file access.
- The first empty apply was rejected with `TASK_EMPTY_APPLICATION`. The Keeper
  repaired the same lifecycle by explicitly selecting workflow `13`; it did
  not start another task.
- File read: `src/example.rs`, after receipt.
- Outcome: `delivery_mode(false)` was correctly reported as `production`.

## Ordering proof

For every authoritative scenario, durable evidence records this order:

```text
parent reads governing instruction
→ parent launches native Memory Keeper
→ Keeper performs task start
→ Keeper performs task apply
→ parent receives complete receipt
→ parent performs the substantive action
→ lifecycle completes
```

The two special cases preserve the same contract:

- continuation: the second turn reuses the already applied lifecycle;
- new goal: a different task and bundle are created before the new action.

## Negative and privacy checks

The run explicitly proved:

- no authoritative task was left without context application;
- a missing apply selection fails closed;
- no user had to remind the agent about AOPMem or project rules;
- an authorized synthetic test secret is usable;
- the secret is absent from durable evidence;
- an existing equivalent tool is reused;
- no duplicate tool, alias, wrapper, executable, or directory is created;
- no SMTP send or external write occurs;
- raw node bodies and full receipts are excluded from durable transcripts;
- persistent DOG-09 session data is deleted after normalization.

Infrastructure probes that failed before `task start` are not counted as
scenarios. They changed no authoritative task state. This includes attempts
where collaboration nesting was unavailable, where an outer Codex process did
not launch its Keeper, and where an inner prompt could not be delivered. They
are treated as fail-closed harness diagnostics, not product successes.

## Durable evidence

Privacy-safe records:

```text
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-01.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-02.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-03.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-04.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-05.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-06.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-07.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-08.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-09.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/DOG-10.json
.devplan/RC5_AGENT_COMPLIANCE_EVIDENCE/SHA256SUMS
```

The checksums make later drift detectable. Reproduction should use a new
isolated workspace rather than reusing the temporary run state.

## Finding count

```text
P1=0
P2=0
```

Stage 25 is ready for the cumulative 21–25 audit.
