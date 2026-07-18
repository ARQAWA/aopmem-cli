# AOPMem RC5 Stage 027 Regression Report

Status: `DONE_LOCAL_CHECKS_PASSED`

Date: `2026-07-18`

Target: `0.2.0-rc5`

Native Windows runtime: `PENDING_DOGFOOD`

## 1. Scope and verdict

Stage 027 mapped the complete §24 negative/security catalog onto the existing
test inventory before adding coverage. No duplicate catalog tests were added.
The full locked suite passes with `766` unit tests and `2` integration tests.

One reproducible P1 regression was found during the focused tool-run proof.
The second short macOS tool process could exit from `SIGKILL` even though its
first CLI run succeeded. The final root cause was not the process-tree cleanup:
instrumented failing runs had an empty live tree and made no signal call.
A standalone native reproducer showed that repeated execution through newly
created hardlinks is rejected by macOS endpoint security (`137`/`SIGKILL`).

The macOS stable executable anchor now uses:

1. `fclonefileat` from the already identity-checked source fd into the already
   opened tool-root fd with `CLONE_NOOWNERCOPY`;
2. only on `ENOTSUP` or `EXDEV`, a bounded fd-to-fd copy capped at
   `MAX_TOOL_IMPLEMENTATION_BYTES`;
3. exact mode, destination `fsync`, explicit source offset zero, and source
   device/inode/size/mode/mtime/ctime revalidation;
4. automatic removal of every temporary snapshot on success and error.

Process cleanup now enumerates only the bounded Darwin process group through
`proc_listpgrppids`; it does not scan every host process. It signals only
identity-checked live members, preserves escaped-descendant tracking, avoids
signalling the already completed exact root, and keeps fail-closed behavior.

Final severity review: P1 `0`; P2 `0`.

## 2. Exact §24 requirement-to-test map

All named tests below returned `PASS` in the final locked run.

### 2.1 Task protocol

| Catalog | Exact test |
|---|---|
| TP-01 start required | `cli::tests::task_start_is_durable_private_and_operationally_read_only` |
| TP-02 continuation | `cli::tests::task_recall_continues_three_pages_with_same_bundle_exact_dedup_and_budget` |
| TP-03 budget exhaustion | `cli::tests::task_recall_budget_exhaustion_returns_terminal_cursor_without_splitting_node` |
| TP-04 stale revision | `cli::tests::task_apply_tp04_through_tp08_fail_closed_with_stable_errors` |
| TP-05 wrong workspace | `cli::tests::task_apply_tp04_through_tp08_fail_closed_with_stable_errors` |
| TP-06 unknown applied node | `cli::tests::task_apply_tp04_through_tp08_fail_closed_with_stable_errors` |
| TP-07 node outside bundle | `cli::tests::task_apply_tp04_through_tp08_fail_closed_with_stable_errors` |
| TP-08 deprecated node | `cli::tests::task_apply_tp04_through_tp08_fail_closed_with_stable_errors` |
| TP-09 completion | `cli::tests::task_complete_tp09_handles_partial_failure_privacy_and_bundle_binding` |
| TP-10 missing memory fail-closed | `cli::tests::task_start_missing_memory_fails_closed_without_observability_state` |
| TP-11 observability isolation | `cli::tests::task_start_authority_survives_best_effort_projection_failure`; `cli::tests::task_apply_complete_authority_survives_projection_failure` |
| TP-12 no raw query | `observability::tests::task_state_never_persists_raw_query_or_unredacted_failure_reason` |

Additional negative proof:
`task_start_mandatory_overflow_is_exact_and_atomic`,
`task_apply_normalizes_corrections_and_enforces_none_relevant_proof`,
`task_apply_complete_are_exactly_idempotent_and_operationally_read_only`, and
`task_lifecycle_enforces_identity_transitions_membership_and_exact_replay`.

### 2.2 Managed Block V2 and Memory Keeper V2

| Catalog | Exact test |
|---|---|
| MB-01 template parity | `adapter::tests::managed_block_v2_contract_has_exact_structure_order_and_limits` |
| MB-02 contract v2 | `adapter::tests::managed_block_v2_contract_has_exact_structure_order_and_limits` |
| MB-03 hard gate | `adapter::tests::managed_block_v2_has_exact_gate_boundary_secret_and_tool_contracts` |
| MB-04 source hierarchy | `adapter::tests::managed_block_v2_contract_has_exact_structure_order_and_limits` |
| MB-05 no blanket secret ban / no user-facing tool concept | `adapter::tests::managed_block_v2_has_exact_gate_boundary_secret_and_tool_contracts`; `adapter::tests::stage_015_managed_block_tool_governance_matches_spec_and_approval_policy` |
| MB-06 exact sync | `adapter::tests::replaces_only_existing_managed_block`; `adapter::tests::sync_replaces_only_drifted_block` |
| MB-07 user text preserved | `adapter::tests::appends_managed_block_without_overwriting_existing_content` |
| MB-08 damaged/duplicate block fails closed | `adapter::tests::rejects_damaged_managed_block`; `adapter::tests::rejects_duplicate_managed_block_markers_without_writing` |
| MB-09 current shell only | `adapter::tests::explicit_codex_claude_cursor_and_copilot_targets_change_only_selected_file` |
| MK-01..MK-07 complete keeper contract | `adapter::tests::memory_keeper_v2_contract_is_fail_closed_and_privacy_safe` |

The keeper test covers required start/apply/complete order, mandatory-memory
failure, no extra start on continuation, explicit persistence only, secret
handling, and approval by external action class.

### 2.3 Secrets and privacy

| Catalog | Exact test |
|---|---|
| SEC-01 test password usable | `cli::tests::stage_009_secret_contract_allows_use_and_requires_atomic_tagged_persistence` |
| SEC-02 explicit remember persists | `cli::tests::stage_009_explicit_test_secret_is_node_and_tag_atomic_before_one_snapshot` |
| SEC-03 automatic write does not persist | `cli::tests::stage_009_failed_atomic_test_secret_proposal_leaves_no_state_or_snapshot` |
| SEC-04 observability redacts | `observability::tests::stage_010_collector_redacts_tagged_payload_error_and_feedback_copies` |
| SEC-05 export/error redacts | `observability::export::tests::stage_010_export_redacts_tagged_value_before_json_escaping`; `cli::tests::stage_013_technical_distinction_errors_never_echo_raw_canary` |
| SEC-06 audit snapshot redacts | `audit::tests::stage_010_sql_dump_scrubs_tagged_body_and_json_proposal_copy` |
| SEC-07 authentication needs no `+++` | `cli::tests::stage_009_authorized_auth_read_uses_exact_canary_without_memory_write_or_approval` |
| SEC-08 external write needs `+++` | `tools::tests::run_tool_blocks_external_write_without_approval`; `tools::tests::run_tool_runs_external_write_with_approval` |

Negative bounds are also proved by
`redaction::tests::stage_010_tagged_source_count_and_body_bounds_fail_closed`
and `stage_010_tagged_source_total_bytes_bound_fails_closed`.

### 2.4 Tools, aliases, and deduplication

| Catalog | Exact test |
|---|---|
| TOOL-01 alias CRUD/resolve | `tools::tests::stage_011_alias_crud_keyset_bulk_and_resolver_precedence` |
| TOOL-02 cycle/shadow rejection | `tools::tests::stage_011_alias_invariants_reject_shadow_chain_cycle_and_inactive_target` |
| TOOL-03 alias-to-alias forbidden | `tools::tests::stage_011_database_constraints_preserve_direct_active_targets` |
| TOOL-04 runner accepts alias | `cli::tests::stage_013_alias_run_emits_safe_resolution_before_canonical_run` |
| TOOL-05 list remains canonical | `cli::tests::stage_013_tool_list_rows_keep_canonical_page_semantics` |
| TOOL-06 exact duplicate | `tools::tests::stage_012_dedupe_plan_is_deterministic_exact_only_and_zero_write` |
| TOOL-07 same implementation | `tools::tests::stage_012_all_five_classes_keep_exact_eligibility_separate` |
| TOOL-08 overlap is not merged | `tools::tests::stage_014_reports_nonexact_overlap_without_mutation` |
| TOOL-09 exact dedupe preserves aliases | `tools::tests::stage_014_exact_apply_supersedes_aliases_and_replays_without_deleting_files` |
| TOOL-10 no implementation deletion | `tools::tests::stage_014_exact_apply_supersedes_aliases_and_replays_without_deleting_files` |
| TOOL-11 Confluence canonical | `tools::tests::stage_015_confluence_fixture_proves_generic_exact_canonicalization`; `cli::tests::stage_015_confluence_alias_commands_and_canonicalized_event_are_safe` |
| external read/write approvals | `tools::tests::run_tool_allows_external_read_without_approval`; `run_tool_blocks_external_read_when_manual_review_is_required`; `run_tool_blocks_external_write_without_approval`; `run_tool_runs_external_write_with_approval` |

New regression proof:

| Risk | Exact test | Result |
|---|---|---|
| stale hardlink/endpoint signal | `tools::tests::macos_repeated_short_tool_runs_do_not_receive_stale_cleanup_signal` | `PASS`, 100 invocations in one process |
| clone fallback | `tools::tests::macos_executable_snapshot_fallback_is_bounded_and_runnable` | `PASS` |
| in-place mutation | `tools::tests::macos_executable_snapshot_fails_closed_on_in_place_source_mutation` | `PASS`, no anchor leak |
| fast same-pgid orphan | `tools::tests::macos_fast_same_group_orphan_is_killed_after_parent_success` | `PASS` |
| escaped setsid descendants | three `macos_setsid_descendant_is_killed_*` tests | `PASS` |
| opened source/root swap | `tools::tests::macos_prepared_command_anchors_executable_cwd_args_and_sibling_resources` | `PASS` |
| original CLI failure | `cli::tests::tool_run_executes_safe_draft_without_approval` | `PASS` |

The two isolated 100-invocation proofs completed in `11.91 s` and `13.55 s`;
the final clone-backed run completed in `11.84 s`. This is a focused safety
sanity measurement, not an unsupported before/after performance claim.

### 2.5 Windows publish, repair, backup, and export

| Catalog | Exact test |
|---|---|
| WIN-01 platform check | `platform_check::tests::platform_check_passes_repeatedly_and_removes_private_root`; integration `platform_check_json_is_workspace_independent_private_and_repeatable` |
| WIN-02 replace/no-replace | `platform_publish::tests::replace_existing_and_create_absent_preserve_source_identity`; `no_replace_succeeds_and_never_changes_existing_destination` |
| WIN-03 error 87 | `platform_check::tests::every_injected_failure_cleans_up_and_error_87_stays_structured_private` |
| WIN-04 Unicode/long/direct-child paths | `platform_publish::tests::direct_children_non_ascii_long_paths_and_distinct_names_are_enforced` |
| WIN-05 handles and identity | `platform_publish::tests::flush_and_identity_faults_preserve_typed_phase`; `parent_identity_swap_fails_before_publication` |
| WIN-06 source/link swap | `platform_publish::tests::source_swap_and_link_destinations_fail_closed` |
| WIN-07 repair marker only after success | `audit::tests::stage_019_repair_is_read_only_redacted_and_idempotent`; `audit::tests::stage_020_normal_snapshot_restores_marker_after_post_remove_clear_failure` |
| WIN-08 repair idempotence/all-workspaces | `audit_repair::tests::stage_019_all_workspaces_continues_past_unsafe_entry_in_stable_order` |
| WIN-09 debug export | Stage 020 debug-capsule tests in the passing full suite and `dev_verify.sh` CLI proof |
| WIN-10 backup | `upgrade::backup::tests::online_backup_publishes_valid_final_database_and_preserves_source`; failure matrix |
| WIN-11 no manual copy/apply retry | `upgrade::recovery::tests::apply_started_fault_never_auto_retries_unknown_core_outcome`; installer audit group |

All Windows contract/unit/source checks pass. Native Windows runtime remains
explicitly `PENDING_DOGFOOD`; it is not reported as a runtime PASS.

### 2.6 Upgrade

| Catalog | Exact test |
|---|---|
| UPG-01 schema 001 | `schema::tests::stage_011_tool_alias_migration_upgrades_001_and_003_sources` |
| UPG-02 schema 003 | same exact mixed-source migration test |
| UPG-03 mixed homes | `upgrade::recovery::tests::fault_hooks_prove_mixed_schema_core_once_and_publish_ordering` |
| UPG-04 migration 004 | `schema::tests::task_recall_exact_index_migration_creates_all_indexes_and_marker` |
| UPG-05 exact data | `upgrade::apply::tests::apply_preserves_v010_data_backups_and_binary_without_replacing_it` |
| UPG-06 aliases | `schema::tests::stage_011_tool_alias_migration_has_required_schema_and_marker` |
| UPG-07 observability migration | `observability::tests::exact_v1_store_migrates_transactionally_to_v2_and_preserves_rows`; `upgrade::recovery::tests::observability_v2_boundary_is_mandatory_and_idempotent` |
| UPG-08 pending marker | `upgrade::apply::tests::migration_failure_rolls_back_keeps_backup_and_records_exact_failed_workspace` |
| UPG-09 failed platform check writes nothing | `platform_check::tests::every_injected_failure_cleans_up_and_error_87_stays_structured_private`; installer audit |
| UPG-10 failed apply preserves backups | `upgrade::backup::tests::every_backup_phase_failure_preserves_exact_diagnostics_and_blocks_acceptance`; `upgrade::recovery::tests::apply_started_fault_never_auto_retries_unknown_core_outcome` |
| UPG-11 publish only after apply | `upgrade::recovery::tests::apply_and_publish_require_their_exact_prior_phase`; `core_only_defers_completed_event_and_final_audit_git_until_publish` |

### 2.7 Focused observability and UI regression

The final suite passes all observability and UI tests. High-risk exact proofs:

- observability v2 schema/migration/drift:
  `initializes_exact_v2_schema_pragmas_columns_and_indexes`,
  `exact_v1_store_migrates_transactionally_to_v2_and_preserves_rows`,
  `missing_extra_column_index_and_check_drift_are_rejected_unchanged`;
- privacy/failure isolation:
  `payload_shapes_have_no_raw_capture_fields`,
  `collector_failure_warns_once_and_never_changes_core_result`,
  `corrupt_store_and_insert_failure_are_best_effort`;
- UI read-only/private boundary:
  `ui::http::tests::every_ui_get_preserves_databases_and_tool_filesystem_tree`,
  `routing_authenticates_before_method_and_uses_exact_asset_allowlist`,
  `stage_010_ui_data_and_http_redact_tagged_values_before_json_escaping`,
  `ui::assets::tests::javascript_uses_safe_dom_sinks_and_read_only_fetch`.

## 3. Final reproducible command proof

```text
cargo fmt --all -- --check                            PASS
cargo clippy --all-targets --locked -- -D warnings    PASS
cargo build --locked                                  PASS
cargo test --locked                                   PASS 768/768
cargo test --tests --locked                           PASS 768/768
scripts/dev_verify.sh                                 PASS
scripts/audit_v020_installers.sh                      PASS 14 groups
git diff --check                                      PASS
jq empty .devplan/RC5_EXECUTION_LEDGER.json           PASS
native Windows runtime                               PENDING_DOGFOOD
```

`dev_verify.sh` independently reran `766` unit tests, both integration tests,
CLI positive/negative/hunch/drift proofs, and finished with
`dev verify passed`.

## 4. Drift and cleanup

- Stage 026 benchmark files were not modified.
- No raw credential, query, node body, receipt, or private path was added to
  retained regression evidence.
- No test was weakened.
- Temporary diagnostic tracing was removed.
- Native Windows status was not promoted.
- Final self-review: P1 `0`; P2 `0`.
