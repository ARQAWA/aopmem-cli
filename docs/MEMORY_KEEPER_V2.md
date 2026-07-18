# Memory Keeper V2

Memory Keeper V2 is the native-subagent boundary between a user request and
substantive agent work. It starts one authoritative AOPMem task, applies the
selected operational context, and returns one compact, privacy-safe Task
Context Receipt.

The Rust task lifecycle contract is defined in
`docs/TASK_START_PROTOCOL.md`. This document defines the agent-side protocol.

## Native-subagent gate

The parent starts Memory Keeper before any substantive action. The parent
supplies:

- the exact current user request;
- the repo root;
- the current shell;
- the current instruction file.

Before task start, Memory Keeper may read only current instructions and the
supplied instruction file. It must not read project code, scan the repository,
perform an external read, or run another task tool.

Validate that the supplied repo root is an existing absolute directory and
keep that exact root as the process `cwd`. Validate the supplied current shell
and its executable-resolution path. Do not infer another shell, repo, or
workspace.

If a native subagent or a safe separate stdin channel is not available,
return exactly:

```text
MEMORY_KEEPER_UNAVAILABLE
```

Do not use a shell fallback. Do not pretend that Keeper ran. The parent must
not begin substantive work.

## Safe stdin start

Use the native Keeper's fixed process runner with a separate stdin channel:

```text
cwd: exact supplied repo root
executable: aopmem
argv: ["task", "start", "--query-stdin", "--json"]
stdin: exact current user request as unchanged UTF-8 bytes
```

Resolve the fixed `aopmem` executable through the supplied current shell/PATH
context and validate that it is launchable. A runner may use that current
shell for fixed-command execution, but its command text must contain no
request-derived bytes.

Write the bytes once and close stdin. Preserve whitespace and do not append a
newline unless it belongs to the request.

The managed flow must not put the request in:

- argv or global `--bundle-id`;
- a shell command string or pipeline;
- a temporary file or environment variable;
- a log, error, receipt, or persistence request.

Normal work must not use `recall`, `recall --full`, a continuation cursor
loop, shell recursion, or parent-driven continuation.

## Start-package validation

The JSON `data` object must contain all 17 core fields:

| Group | Required fields |
|---|---|
| Identity | `task_id`, `bundle_id`, `workspace_key`, `memory_revision` |
| State | `mandatory_context_complete`, `retrieval_complete`, `budget_exhausted` |
| Nodes | `mandatory_nodes`, `task_nodes` |
| Categories | `applicable_gates`, `applicable_rules`, `candidate_workflows`, `candidate_tools`, `relevant_corrections`, `relevant_failure_modes`, `hunches` |
| Evidence | `selection_reasons` |

Validate all of these facts:

- `task_id` and `bundle_id` are canonical lowercase UUID v4 values;
- `workspace_key` is non-empty;
- `memory_revision` is 32 lowercase hexadecimal characters;
- `mandatory_context_complete` is exactly `true`;
- retrieval is exactly complete or explicitly bounded;
- no top-level `continuation_cursor` is present;
- every node/category field has the expected array type;
- every category and selected ID is a unique positive member of the returned
  bundle and matches its node type;
- budget metadata is consistent with the retrieval state.

Only these retrieval pairs are valid:

| `retrieval_complete` | `budget_exhausted` | Meaning |
|---:|---:|---|
| `true` | `false` | Complete package |
| `false` | `true` | Valid bounded package |

Reject `true/true`, `false/false`, nulls, stale revision, malformed IDs,
foreign state, missing fields, and a cursor. A bounded package is valid but
must remain marked bounded in the receipt.

Build the authoritative returned ID set only from
`mandatory_nodes[*].node.id` and `task_nodes[*].node.id`. Category arrays and
Keeper selections may only refer to this set.

Treat `correction`, `lesson`, and `incident_scar` as correction candidates.
Treat hunches as sourced warnings only.

## Selection and apply

Apply every returned applicable gate and rule. Select the smallest sufficient
subset of workflows, tools, corrections, and failure modes. Prefer one
workflow unless more are genuinely needed.

Use `--none-relevant` only when:

- `retrieval_complete=true`;
- `budget_exhausted=false`;
- `task_nodes` is empty.

The none-relevant application may still contain mandatory gate and rule IDs.
Never declare none-relevant for a bounded package.

Run apply through the same fixed runner, safe stdin channel, shell/PATH
context, and exact repo-root `cwd`:

```text
cwd: exact supplied repo root
executable: aopmem
argv: ["--bundle-id", bundle_id, "task", "apply",
       "--task-id", task_id,
       ...repeatable selected ID flags...,
       "--json"]
stdin: empty
```

Use only these repeatable flags:

- `--applied-gate-id`;
- `--applied-rule-id`;
- `--selected-workflow-id`;
- `--selected-tool-id`;
- `--selected-correction-id`;
- `--selected-failure-mode-id`.

After apply, require:

- the exact task, bundle, workspace, and memory revision from start;
- `status=applied`;
- exact applied/selected ID lists and `none_relevant`;
- a boolean `replayed`, where `true` is valid only for exact replay.

Any stale revision, wrong workspace, foreign bundle, unknown or inactive
node, kind mismatch, outside-bundle ID, conflicting replay, or other apply
failure is fail-closed. Do not return a receipt.

## Secret-bearing tasks

User-provided or authorized test, VDI, and closed-contour credentials are
usable task input. Keeper must not refuse, force a placeholder, lecture, or
remove a required value solely because it looks secret.

A secret-bearing request does not authorize an operational-memory write.
Exact local storage requires an explicit `remember`, `teach`, save, or
equivalent user instruction. It needs no additional `+++`.

An explicitly stored test secret uses the atomic flow defined in
`docs/SECRET_HANDLING.md`: one direct-process
`teach propose --apply --payload-stdin` invocation creates a node with the
generic title `Authorized test credential`, keeps the exact value only in
`body`, and adds `sensitivity:test_secret` by `node_ref`. Proposal JSON travels
only through process stdin. It never enters argv, shell text, a temporary
file, an environment value, a log, an error, or a receipt. The proposal and
apply share one mutation transaction and one post-commit audit snapshot.

Never use `remember` then `tag add`, or separate `teach propose` and
`teach apply`, for an exact test secret. Authentication and external-read
approval stay action-based. External writes and high-risk actions still
require standalone exact `+++`.

## Task Context Receipt

Return exactly one bounded receipt after validated apply:

```text
TASK_CONTEXT_RECEIPT_V2
task_id: <canonical UUID v4>
bundle_id: <canonical UUID v4>
workspace_key: <workspace key>
memory_revision: <revision fingerprint>
mandatory_context_complete: true
retrieval_complete: <true|false>
budget_exhausted: <false|true>
none_relevant: <true|false>
applied_gate_ids: [<ids>]
applied_rule_ids: [<ids>]
selected_workflow_ids: [<ids>]
selected_tool_ids: [<ids>]
selected_correction_ids: [<ids>]
selected_failure_mode_ids: [<ids>]
mandatory_constraints:
- <gate|rule id>: <short factual constraint>
selected_context:
- <workflow|tool|correction|failure_mode id>: <short factual use>
source_hierarchy:
- <source_ref in returned priority order>: [<selected node ids>]
further_retrieval_order:
1. applicable workflow/tool/correction
2. Understand Docs when enabled and needed
3. Codebase Memory MCP when code context is needed
4. actual relevant files on disk
5. external read sources
6. external mutations only after approval
apply_status: applied
apply_replayed: <true|false>
```

The receipt carries facts needed by the parent. It is not a database dump.
Keep summaries bounded and redact sensitive values. Never include the raw
request, full node bodies, full recall transcript, raw output, credentials,
secrets, environment captures, or hidden reasoning.

## Retrieval and source order

The parent and Keeper preserve this exact order:

1. Current system, developer, and user instructions.
2. AOPMem mandatory operational memory.
3. AOPMem task-specific retrieval.
4. Applicable workflow, tool, or correction.
5. Understand Docs when enabled and needed.
6. Codebase Memory MCP for code-context retrieval.
7. Actual relevant files on disk as final technical truth.
8. External read sources.
9. External mutations only after approval rules.

Read only relevant files. A retrieval aid does not replace current file
content.

## Task boundaries

| Situation | Action |
|---|---|
| Clarification | Reuse the current receipt and task/bundle IDs |
| Continuation | Reuse the current receipt and task/bundle IDs |
| Same-goal question | Reuse the current receipt and task/bundle IDs |
| Correction of the same result | Reuse the current receipt and task/bundle IDs |
| Long pause, reliable receipt, unchanged goal/project/work type | Reuse after Keeper re-entry |
| New chat | Start a new task |
| Independent goal | Start a new task |
| Project change | Start a new task |
| Work-type change | Start a new task |
| Explicit new task | Start a new task |
| Compaction without a reliable receipt | Start a new task |

Do not start and apply a new task for every short same-goal message. Do not
attach unrelated work to an old task.

## Focused proof map

| Test | Contract |
|---|---|
| `MK-01` | Native subagent runs before substantive action |
| `MK-02` | Unavailable native subagent returns the exact error |
| `MK-03` | Exact request uses direct process stdin |
| `MK-04` | Completeness, budget, IDs, revision, and cursor are validated |
| `MK-05` | Selected returned context is authoritatively applied |
| `MK-06` | Full recall, shell fallback, and continuation are forbidden |
| `MK-07` | Receipt is compact, complete, and privacy-safe |
