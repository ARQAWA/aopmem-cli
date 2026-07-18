# RC5 Stage 023 Handoff

Status: `DONE_LOCAL_CHECKS_PASSED`

Next stage: `STAGE_024`

Verified through stage: `STAGE_020`

Native Windows runtime: `PENDING_DOGFOOD`

## Result

The effectiveness report and minimal desktop UI now expose all required
Stage 23 facts without a synthetic score or operational-memory task history.
All reads stay local, bounded, redacted, token-protected, and read-only.

The report derives task lifecycle facts from the observability v2 `tasks`,
`task_bundle_nodes`, and `task_applied_nodes` tables:

- starts, context applications, starts without apply by report end,
  completions, and failures;
- applied gates and rules;
- selected workflows and tools;
- applied corrections and failure modes;
- applied node counts grouped by `mandatory` and `task` context.

It also reports blocked duplicates, alias resolutions, unresolved overlap
blocks, pending audit snapshots, and the last successful audit repair. The
existing 30-day-or-100,000,000-byte retention policy remains unchanged.

## Minimal UI

The Effectiveness view renders the factual task and RC5 compliance fields.
The Tools view adds canonical tool ID, aliases, duplicate classification,
superseded duplicate evidence, and unresolved overlaps.

Duplicate analysis reuses the bounded read-only dedupe planner. One plan is
calculated per request, aliases and status evidence are loaded in batched
queries, and row rendering performs no database query. If filesystem or
contract evidence prevents a factual plan, classifications remain empty and
both `duplicate_analysis_complete` and top-level `complete` are `false`.

The UI still has only exact GET routes, binds only `127.0.0.1`, requires the
random path token, embeds all assets, has no CDN, and exposes no write action.

## Compatibility and complexity

Stage 23 does not change the observability v2 schema manifest. A review caught
that adding new indexes under the same schema version would reject existing
v2 stores. Those indexes were removed. Task aggregates use the existing
`idx_tasks_started_at`, `idx_tasks_workspace_status`, and child primary-key
indexes. `EXPLAIN QUERY PLAN` tests prove those paths. The remaining
per-workspace filters are bounded by the exact observability retention ceiling.

The complexity scan was treated as advisory. Manual review found no new N+1
query, unbounded filesystem walk, or quadratic database loop. Dedupe remains
bounded by its existing 1,000-tool and 10,000-pair ceilings.

## Privacy and read-only proof

- report task facts come only from the observability store;
- no raw query, chat, task text, node body, stdout, stderr, token, or secret is
  added to report or UI output;
- report output is redacted again;
- the report and every UI GET preserve operational and observability database
  bytes, schema, mtimes, and row counts;
- every UI GET also preserves the complete workspace tools filesystem tree;
- missing or foreign observability data fails closed.

## Proof

```text
cargo fmt --all -- --check                            PASS
node --check src/ui/assets/app.js                     PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked --all-targets                    PASS
cargo test --locked                                   PASS 763/763
bash scripts/dev_verify.sh                            PASS

task_and_rc5_compliance_facts_are_factual_bounded_and_read_only
                                                     PASS
task_aggregates_use_bounded_lifecycle_and_child_indexes
                                                     PASS
foreign_task_history_fails_closed                    PASS
tool_duplicate_facts_expose_factual_classes_and_unresolved_overlaps
                                                     PASS
tools_mcp_and_exact_route_allowlist_are_bounded_and_read_only
                                                     PASS
every_ui_get_preserves_databases_and_tool_filesystem_tree
                                                     PASS

git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
native Windows runtime                               PENDING_DOGFOOD
```

Self-review result: P1 `0`, P2 `0`. This is local Stage 23 proof, not the
Stage 21–25 cumulative audit. Verified-through remains `STAGE_020`.

## Independent high review

The independent Stage 23 review returned `PASS`, with P1 `0` and P2 `0`.
It rechecked the factual/no-score contract, the 30-day-or-100,000,000-byte
bound, indexed SQL and foreign-workspace fail-closed behavior, batched tool
queries, read-only database access, exact GET allowlist, local-only assets,
GET database/tool-tree immutability, and exact observability-v2 schema
compatibility.

```text
focused mandatory tests                              PASS 6/6
report negative/privacy tests                       PASS 30/30
UI assets/data/http tests                           PASS 26/26
exact observability-v2 schema                       PASS
cargo fmt --all -- --check                          PASS
node --check src/ui/assets/app.js                   PASS
git diff --check                                    PASS
independent review P1/P2                            0/0
native Windows runtime                              PENDING_DOGFOOD
```
