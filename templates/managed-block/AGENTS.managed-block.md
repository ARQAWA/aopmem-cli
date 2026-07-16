<!-- AOPMEM:BEGIN managed block -->
This block is managed by AOPMem.
AOPMem is installed.
Main work starts with Memory Keeper / recall.
Normal work MUST use `aopmem recall --query "<current task>"` before non-trivial work.
The first task recall creates `bundle_id`; do not pass global `--bundle-id` to a first, bare, or `--full` recall.
Memory Keeper follows `continuation_cursor` with the same query and exact `--bundle-id <bundle_id>` until `more_results=false` or `budget.task.exhausted=true`.
Memory Keeper passes global `--bundle-id <bundle_id>` to later AOPMem operations for the same work.
`more_results=true` with a null cursor is a recall contract error.
Never use `aopmem recall --full` in normal task flow; it is debug/audit/export/migration only.
Do not edit AOPMem SQLite directly.
Use `aopmem tool run <tool-id>` for generated tools.
Generated tool runtime limits and output mode come from its validated `tool.json`.
Tool processes use the tool root as cwd; resolve resources through validated `runtime.runtime_dir` relative to that root.
For shebang tools, `$0` and the concrete entrypoint launch path are implementation details; do not use them for resource discovery.
Defaults are 30000 ms and 65536 bytes per stream; hard ceilings are 900000 ms and 10485760 bytes per stream.
`output_mode=inline` returns `TOOL_OUTPUT_OVERFLOW` and writes no artifact when a stream exceeds its limit.
`output_mode=artifact` keeps bounded previews and publishes full output only under `artifacts/YYYY-MM-DD/`.
Artifact capture above 10485760 bytes per stream returns `TOOL_OUTPUT_OVERFLOW` and publishes nothing.
`--dry-run` executes nothing and creates no artifact.
Approval is required when `approval_requirement != none`, for `external_write` or `destructive`, and for explicit high-risk policy.
No approval is required for `none`, `local_read`, contract-safe `local_write_artifact`, or `external_read` with `approval_requirement=none`.
Do not store secrets.
Use `remember`, `teach`, `reflect` only by user trigger.
Feedback is user-triggered or agent post-task: `aopmem --bundle-id <bundle_id> feedback record --outcome useful|partial|wrong [--reason "<short reason>"]`.
Feedback stays only in Local Observability; never put the full task, raw chat, raw output, secrets, or hidden reasoning in its reason.
Reflection keeps one current inventory node and append-only operational events; an identical inventory is a no-op.
Reflection inventory, apply receipts, and events never copy node bodies, hidden reasoning, raw complete chat, raw tool output, environment data, credentials, or secrets.
Proposal payloads and applied nodes contain only explicit user-selected structured memory; never put secrets or raw captures into a proposal.
Memory stored under user-level AOPMem workspace, not repo.
Do not create `.aopmem` in repo.
Deprecated memory excluded from normal recall.
Memory Keeper follows list `next_cursor` pages until `more_results=false` whenever a full set is needed.
Artifacts cleanup policy: 7 days OR 1 GB per workspace.
Do not edit inside this block manually.
<!-- AOPMEM:END managed block -->
