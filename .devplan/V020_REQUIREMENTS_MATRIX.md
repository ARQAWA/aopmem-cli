# AOPMem v0.2.0-rc1 Requirements Matrix

| ID | Scope | Stages | Required proof | Status |
|---|---|---:|---|---|
| R-01 | Worktree preservation and hunk classification | 01-02 | protected refs; 434 hunk ledger | stage_01_pass |
| R-02 | Windows remediation and legacy key compatibility | 03,32,35 | v0.1 fixture; collision/rollback | PASS_resolver_and_real_peeled_v010_update_exact_data |
| R-03 | Draft approval policy | 04 | five approval/dry-run tests | stage_04_pass |
| R-04 | Safe storage optimizations | 05-06 | focused storage/transaction tests | stages_05_06_pass_reaudit_p1_0_p2_0 |
| R-05 | Audit snapshot | 07-08,24 | streaming, restore, pending, Windows replace, Git commit, typed duration event | stages_07_08_pass_reaudit_p1_0_p2_0_stage_24_observation_466_tests_independent_P1_0_P2_0 |
| R-06 | Final list contract | 09-10 | 100/500, keyset, cursor, all, body contract | stages_09_10_pass_reaudit_p1_0_p2_0 |
| R-07 | Final recall contract | 11-15,25 | mandatory, query, graph, continuation, reasons, full, workspace-bound stale-first cursor | stages_11_15_pass_reaudit_p1_0_p2_0_stage_25_cursor_remediation_484_tests_two_clean_audits_P1_0_P2_0_P3_0 |
| R-08 | Tool resource contract | 16-18,24,35 | legacy defaults, timeout, overflow, artifact stream, typed run facts | PASS_575_tests_and_final_cargo_xwin_PE_x64 |
| R-09 | Reflection inventory/history | 19 | one current node; required events; rollback | stage_19_pass_382_tests_reaudit_p1_0_p2_0 |
| R-10 | Artifact retention | 20,24 | 7 days/1 GB, current-day deletion, protected paths, private cleanup facts | stage_20_pass_413_tests_cumulative_reaudit_p1_0_p2_0_stage_24_observation_466_tests_independent_P1_0_P2_0 |
| R-11 | Separate Local Observability | 21-26 | schema v1, URI/vendor privacy, retention, failure isolation, correlated recall failures, all events, strict read-only facts | stages_21_24_pass_stage_25_privacy_and_failure_linkage_remediation_stage_26_initial_audit_P1_0_P2_2_remediated_509_tests_clean_reaudit_P1_0_P2_0_P3_0 |
| R-12 | Bundle correlation and feedback | 25 | UUID validation; atomic parent/nodes/events; workspace-bound cursor; feedback never writes operational DB | stage_25_pass_484_tests_operational_db_and_snapshot_hash_size_mtime_unchanged_cumulative_audit_pass_P1_0_P2_0_P3_0 |
| R-13 | Effectiveness report | 26 | exact fact-only aggregates; no score | stage_26_pass_after_remediation_one_snapshot_event_time_recall_adapter_failed_top20_no_score_23_report_2_cli_509_full_clean_reaudit_P1_0_P2_0_P3_0 |
| R-14 | Debug capsule | 27 | exact 12 entries; deterministic redaction | stage_27_pass_exact_12_deterministic_private_read_only_no_clobber_15_export_1_cli_524_full_audit_P1_0_P2_0_P3_1_D017 |
| R-15 | Read-only desktop UI | 28-30 | loopback, token, no writes, bounded graph, screenshots | stage_28_server_security_pass_11_ui_3_cli_538_full_locked_xwin_P1_0_P2_0_P3_1_accepted_D021_stage_29_exact_11_get_api_13_focused_546_full_read_only_fingerprint_graph_200_500_P1_0_P2_0_P3_2_accepted_D022_stage_30_six_views_real_API_three_true_PNG_1440x900_main_DB_unchanged_cumulative_P1_0_P2_0 |
| R-16 | Upgrade plan/apply | 31-32 | no-write plan; backups; migration; rollback | stage_31_no_write_plan_9_module_1_CLI_561_full_tests_P1_0_P2_0_stage_32_apply_14_module_1_CLI_575_full_tests_exact_v010_payload_WAL_guard_transactional_migration_safe_recovery_no_cross_workspace_rollback_collector_warning_P1_0_P2_0 |
| R-17 | Install prompt v0.2 | 33,35 | Mac fresh/update; PowerShell 5.1 static audit | PASS_11_group_audit_real_Mac_fresh_adapter_healthy_and_real_peeled_v010_update_zero_onboarding |
| R-18 | Version and assets | 21,35 | v0.2.0-rc1; Mach-O arm64; PE x64; SHA256SUMS | PASS_flat_Mac_b32e918d_Windows_a4e3302d_manifest_verified |
| R-19 | Reproducible benchmark | 34,35 | small/medium/large; raw runs; median/p95 | PASS_1060_samples_source_and_candidate_hash_reproduced_final_asset_parity_documented_no_percentage_claim |
| R-20 | Required checks | 35 | fmt, clippy, build, tests, dev_verify, diff check | PASS_fmt_clippy_build_575_575_dev_verify_diff |
| R-21 | Negative tests | 03-35 | every listed failure contract | PASS_all_required_negative_contracts_in_575_suite_plus_installer_11_groups_UI_live_token_and_method_rejection |
| R-22 | Final documents | 21-35 | exact requested docs/devplan files | PASS_all_required_devplan_docs_install_and_dist_paths_present |
| R-23 | Global audit | 35 | P1=0; P2=0 | PASS_independent_final_OPEN_P1_0_P2_0_P3_0 |
| R-24 | Dogfood decision and stop | 35 | RC report; no external release actions | READY_for_macOS_and_Windows_dogfood_stop_conditions_observed |
