# AOPMem v0.2.0-rc1 — Complete Report

Generated: 2026-07-16

Status: complete. Ready for macOS and Windows dogfood.

This file consolidates the nine v0.2.0-rc1 planning, proof, audit,
benchmark, and release reports. Source reports remain authoritative and
unchanged. Raw benchmark samples, binaries, and PNG evidence are referenced
by those reports and are not duplicated here.

## Included source files

- `.devplan/V020_CURRENT_DIFF_CLASSIFICATION.md`
- `.devplan/V020_FINAL_DECISION_LOG.md`
- `.devplan/V020_REQUIREMENTS_MATRIX.md`
- `.devplan/V020_EXECUTION_LEDGER.json`
- `.devplan/V020_PROOF_LOG.md`
- `.devplan/AUDITS/STAGE_26_30_CUMULATIVE_AUDIT.md`
- `.devplan/V020_BENCHMARK_REPORT.md`
- `.devplan/V020_GLOBAL_AUDIT_REPORT.md`
- `.devplan/RELEASE_CANDIDATE_v0.2.0-rc1.md`

## Source SHA-256

```text
a2185666544c113d8491b16531f6c7ff812253ea3678dbf99aa7bb5beadf39ac  .devplan/V020_CURRENT_DIFF_CLASSIFICATION.md
c5ff4217f8e09943bb443992bdb9f2a2bda6a92b6330a790c3f0c8472afb9786  .devplan/V020_FINAL_DECISION_LOG.md
bf98ab2645889821f2e91e0f1dc5f32a18793c08d0b673c71236de26fe6dd310  .devplan/V020_REQUIREMENTS_MATRIX.md
d2af5cd6072c8ffcdcd23e08cf4fe9b730dc6faa9921bd743ae425bb5319261f  .devplan/V020_EXECUTION_LEDGER.json
89044a8a244b98116da83a02189c318df12b2b8ca565879c3f0af2fc2bf73c17  .devplan/V020_PROOF_LOG.md
06b4e9ffef03f556722b5ee8ccea92adedc8a396a30a0f78d6d1c0c5266e3f48  .devplan/AUDITS/STAGE_26_30_CUMULATIVE_AUDIT.md
80a197a2685ddff982db1dd38ecd7de5b26dfb970d43d6729d0bc0ac6773909a  .devplan/V020_BENCHMARK_REPORT.md
ac3ce7fe3f647c33805c11cc35a644ad676acc34b7563b024a6425971c1d1acb  .devplan/V020_GLOBAL_AUDIT_REPORT.md
da479705dd2bf3591213106aa3ea849827c1dd905b2cddc081ef2f18827ee097  .devplan/RELEASE_CANDIDATE_v0.2.0-rc1.md
```


---

## 1. Current diff classification

Source: `.devplan/V020_CURRENT_DIFF_CLASSIFICATION.md`

# V020 Current Diff Classification

## Provenance

- Baseline commit: `9877d39a4bc44cf62140aace8755720044c1d41f`
- Baseline tag: `v0.1.0-rc3`
- Authoritative mixed tree: `7c6bf85e90833fba169c58f0b60a697054310909`
- Protected ref: `refs/aopmem-recovery/v020-current-mixed-20260714`
- Archive-only tree: `cdad5a9bd2d85a2850c7867b816e7a03a2a33272`
- Archive ref: `refs/aopmem-recovery/v020-archive-incomplete-20260714`
- Raw worktree before recovery: clean.
- Reflog evidence: reset to HEAD on 2026-07-14.
- Diff size: 16 files, 434 hunks.

## Classification rule

Every hunk has one primary class. Mixed hunks use `MODIFY`; accepted lines
survive, conflicting lines change locally. No whole-file revert.

- `KEEP`: 315
- `MODIFY`: 65
- `PREEXISTING_PRESERVE`: 43
- `REVERT_OPTIMIZATION_ONLY`: 11

## Mixed-file decisions

- `src/storage/mod.rs`: preserve Windows/root remediation; keep read-only,
  validation, targeted SQL, transactions, batched FTS; replace bounded recall
  and cursor/body truncation; add legacy workspace-key resolution.
- `src/install/mod.rs`: preserve strict UTF-8, mojibake guard, USERPROFILE,
  root resolution, validation before workspace creation.
- `src/adapter/mod.rs`: preserve managed contract; remove only draft approval
  line; add v0.2 recall/bundle/tool-resource contract.
- `src/cli/mod.rs`: preserve Windows/root and transaction/read wiring;
  replace bounded recall/pagination; remove draft-only approval text/tests.
- `templates/managed-block/AGENTS.managed-block.md`: preserve accepted lines
  except `Draft tool execution requires +++`.
- `.devplan/PROOF_LOG.md`: preserve rc3 history; use
  `.devplan/V020_PROOF_LOG.md` for new proof.

## Hunk ledger

| ID | File | Hunk | Class | Decision |
|---:|---|---|---|---|
| H001 | `.devplan/PROOF_LOG.md` | `@@ -4923,0 +4924,79 @@ PASS native Windows smoke was not run on Mac.` | `PREEXISTING_PRESERVE` | исторические Windows/user proof; сохранить без подмены proof v0.2 |
| H002 | `.devplan/WINDOWS_FIRST_INSTALL_REMEDIATION.md` | `@@ -0,0 +1,108 @@` | `PREEXISTING_PRESERVE` | исторические Windows/user proof; сохранить без подмены proof v0.2 |
| H003 | `.devplan/WINDOWS_RC2_MAC_AUDIT_COMMANDS.log` | `@@ -0,0 +1,1427 @@` | `PREEXISTING_PRESERVE` | исторические Windows/user proof; сохранить без подмены proof v0.2 |
| H004 | `.devplan/WINDOWS_RC3_MAC_AUDIT_COMMANDS.log` | `@@ -0,0 +1,91 @@` | `PREEXISTING_PRESERVE` | исторические Windows/user proof; сохранить без подмены proof v0.2 |
| H005 | `src/adapter/mod.rs` | `@@ -10,0 +11,14 @@ This block is managed by AOPMem.\n\` | `MODIFY` | сохранить managed contract; удалить draft +++ и добавить v0.2 contract |
| H006 | `src/adapter/mod.rs` | `@@ -256,0 +271,2 @@ mod tests {` | `MODIFY` | сохранить managed contract; удалить draft +++ и добавить v0.2 contract |
| H007 | `src/adapter/mod.rs` | `@@ -342,0 +359,2 @@ mod tests {` | `MODIFY` | сохранить managed contract; удалить draft +++ и добавить v0.2 contract |
| H008 | `src/adapter/mod.rs` | `@@ -357,0 +376,18 @@ mod tests {` | `MODIFY` | сохранить managed contract; удалить draft +++ и добавить v0.2 contract |
| H009 | `src/adapter/mod.rs` | `@@ -372,0 +409,3 @@ mod tests {` | `MODIFY` | сохранить managed contract; удалить draft +++ и добавить v0.2 contract |
| H010 | `src/artifacts/mod.rs` | `@@ -6,0 +7 @@ use std::path::{Path, PathBuf};` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H011 | `src/artifacts/mod.rs` | `@@ -27,0 +29 @@ pub struct CleanupReport {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H012 | `src/artifacts/mod.rs` | `@@ -37,0 +40,7 @@ struct ArtifactDirUsage {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H013 | `src/artifacts/mod.rs` | `@@ -122,0 +132 @@ fn cleanup_workspace_artifacts_for_day(` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H014 | `src/artifacts/mod.rs` | `@@ -146,0 +157 @@ fn cleanup_artifact_root_for_day(` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H015 | `src/artifacts/mod.rs` | `@@ -169,0 +181,13 @@ fn cleanup_artifact_root_for_day(` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H016 | `src/artifacts/mod.rs` | `@@ -178,0 +203 @@ fn cleanup_artifact_root_for_day(` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H017 | `src/artifacts/mod.rs` | `@@ -187,0 +213 @@ struct CleanupRootReport {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H018 | `src/artifacts/mod.rs` | `@@ -230 +256 @@ fn path_size(path: &Path) -> io::Result<u64> {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H019 | `src/artifacts/mod.rs` | `@@ -233,0 +260,4 @@ fn path_size(path: &Path) -> io::Result<u64> {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H020 | `src/artifacts/mod.rs` | `@@ -246,0 +277,31 @@ fn path_size(path: &Path) -> io::Result<u64> {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H021 | `src/artifacts/mod.rs` | `@@ -267 +328 @@ fn is_leap_year(year: u16) -> bool {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H022 | `src/artifacts/mod.rs` | `@@ -368 +429 @@ mod tests {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H023 | `src/artifacts/mod.rs` | `@@ -387 +448,5 @@ mod tests {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H024 | `src/artifacts/mod.rs` | `@@ -388,0 +454,29 @@ mod tests {` | `KEEP` | приняты cleanup, symlink safety, deleted paths; позже observability hook |
| H025 | `src/audit/mod.rs` | `@@ -3 +3,2 @@ use std::fmt::Write as _;` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H026 | `src/audit/mod.rs` | `@@ -4,0 +6,2 @@ use std::path::{Path, PathBuf};` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H027 | `src/audit/mod.rs` | `@@ -16,0 +20,6 @@ const SNAPSHOT_FILE_NAME: &str = "memory.sql";` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H028 | `src/audit/mod.rs` | `@@ -166,0 +176,9 @@ pub fn list_events(connection: &Connection) -> rusqlite::Result<Vec<Event>> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H029 | `src/audit/mod.rs` | `@@ -171,0 +190 @@ pub fn write_sql_snapshot(` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H030 | `src/audit/mod.rs` | `@@ -173 +191,0 @@ pub fn write_sql_snapshot(` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H031 | `src/audit/mod.rs` | `@@ -175 +193,30 @@ pub fn write_sql_snapshot(` | `MODIFY` | сохранить intent; исправить Windows replace и external git runtime |
| H032 | `src/audit/mod.rs` | `@@ -179,0 +227,114 @@ pub fn write_sql_snapshot(` | `MODIFY` | сохранить intent; исправить Windows replace и external git runtime |
| H033 | `src/audit/mod.rs` | `@@ -250,3 +411,5 @@ fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H034 | `src/audit/mod.rs` | `@@ -260,0 +424 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H035 | `src/audit/mod.rs` | `@@ -272,7 +436,9 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H036 | `src/audit/mod.rs` | `@@ -280,0 +447,2 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H037 | `src/audit/mod.rs` | `@@ -283,2 +450,0 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H038 | `src/audit/mod.rs` | `@@ -286,0 +453,2 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H039 | `src/audit/mod.rs` | `@@ -287,0 +456,2 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H040 | `src/audit/mod.rs` | `@@ -292 +462,11 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H041 | `src/audit/mod.rs` | `@@ -295,2 +475,19 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H042 | `src/audit/mod.rs` | `@@ -299,2 +496,23 @@ fn build_sql_dump(connection: &Connection) -> rusqlite::Result<String> {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H043 | `src/audit/mod.rs` | `@@ -303 +521 @@ fn append_table_rows(` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H044 | `src/audit/mod.rs` | `@@ -331,0 +550,9 @@ fn append_table_rows(` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H045 | `src/audit/mod.rs` | `@@ -334,11 +561 @@ fn append_table_rows(` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H046 | `src/audit/mod.rs` | `@@ -348 +565 @@ fn append_table_rows(` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H047 | `src/audit/mod.rs` | `@@ -350 +567 @@ fn append_table_rows(` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H048 | `src/audit/mod.rs` | `@@ -353 +570 @@ fn append_table_rows(` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H049 | `src/audit/mod.rs` | `@@ -391,0 +609,2 @@ mod tests {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H050 | `src/audit/mod.rs` | `@@ -407,0 +627,101 @@ mod tests {` | `MODIFY` | сохранить intent; исправить Windows replace и external git runtime |
| H051 | `src/audit/mod.rs` | `@@ -495,0 +816,80 @@ mod tests {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H052 | `src/audit/mod.rs` | `@@ -500,0 +901,2 @@ mod tests {` | `KEEP` | приняты streaming dump, canonical FTS, pending marker и tests |
| H053 | `src/audit/mod.rs` | `@@ -509,0 +912,174 @@ mod tests {` | `MODIFY` | сохранить intent; исправить Windows replace и external git runtime |
| H054 | `src/cli/mod.rs` | `@@ -7,0 +8 @@ use std::io::BufRead;` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H055 | `src/cli/mod.rs` | `@@ -32,0 +34,9 @@ pub const EXIT_IO_ERROR: u8 = 9;` | `MODIFY` | заменить temporary recall final engine |
| H056 | `src/cli/mod.rs` | `@@ -79 +89 @@ enum Command {` | `MODIFY` | заменить temporary recall final engine |
| H057 | `src/cli/mod.rs` | `@@ -112 +122 @@ enum NodeCommand {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H058 | `src/cli/mod.rs` | `@@ -148,0 +159,12 @@ struct NodeGetArgs {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H059 | `src/cli/mod.rs` | `@@ -175,0 +198,26 @@ struct NodeUpdateArgs {` | `MODIFY` | заменить temporary recall final engine |
| H060 | `src/cli/mod.rs` | `@@ -210 +258 @@ enum LinkCommand {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H061 | `src/cli/mod.rs` | `@@ -276,0 +325,54 @@ struct NodeMetadataListArgs {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H062 | `src/cli/mod.rs` | `@@ -351 +453 @@ enum ToolCommand {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H063 | `src/cli/mod.rs` | `@@ -402,2 +504,2 @@ enum McpCommand {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H064 | `src/cli/mod.rs` | `@@ -465,0 +568,5 @@ struct AdapterTargetArgs {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H065 | `src/cli/mod.rs` | `@@ -502,2 +609,2 @@ fn run_command_with_approval(command: &Command, json: bool, approved: Option<&st` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H066 | `src/cli/mod.rs` | `@@ -511,2 +618,2 @@ fn run_command_with_approval(command: &Command, json: bool, approved: Option<&st` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H067 | `src/cli/mod.rs` | `@@ -532,2 +639,2 @@ fn run_command_with_approval(command: &Command, json: bool, approved: Option<&st` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H068 | `src/cli/mod.rs` | `@@ -540 +647 @@ fn run_command_with_approval(command: &Command, json: bool, approved: Option<&st` | `MODIFY` | заменить temporary recall final engine |
| H069 | `src/cli/mod.rs` | `@@ -582,2 +689,2 @@ fn run_command_with_approval(command: &Command, json: bool, approved: Option<&st` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H070 | `src/cli/mod.rs` | `@@ -667 +774 @@ fn run_tool_validate(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H071 | `src/cli/mod.rs` | `@@ -710,2 +817,3 @@ fn run_tool_validate(` | `MODIFY` | next_cursor, --all, explicit completeness |
| H072 | `src/cli/mod.rs` | `@@ -716,2 +824,2 @@ fn run_tool_list(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H073 | `src/cli/mod.rs` | `@@ -719 +827,5 @@ fn run_tool_list(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H074 | `src/cli/mod.rs` | `@@ -729 +841 @@ fn run_tool_get(command_id: &'static str, args: &ToolGetArgs, json_output: bool)` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H075 | `src/cli/mod.rs` | `@@ -768 +880,7 @@ fn run_tool_run(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H076 | `src/cli/mod.rs` | `@@ -839,0 +958,20 @@ fn run_tool_run(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H077 | `src/cli/mod.rs` | `@@ -958 +1096 @@ fn run_teach_start(command_id: &'static str, args: &TeachStartArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H078 | `src/cli/mod.rs` | `@@ -963,7 +1101,9 @@ fn run_teach_start(command_id: &'static str, args: &TeachStartArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H079 | `src/cli/mod.rs` | `@@ -988 +1128,6 @@ fn run_teach_start(command_id: &'static str, args: &TeachStartArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H080 | `src/cli/mod.rs` | `@@ -997 +1142 @@ fn run_teach_add(command_id: &'static str, args: &TeachPayloadArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H081 | `src/cli/mod.rs` | `@@ -1002 +1147,3 @@ fn run_teach_add(command_id: &'static str, args: &TeachPayloadArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H082 | `src/cli/mod.rs` | `@@ -1021 +1168,6 @@ fn run_teach_add(command_id: &'static str, args: &TeachPayloadArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H083 | `src/cli/mod.rs` | `@@ -1034 +1186 @@ fn run_teach_propose(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H084 | `src/cli/mod.rs` | `@@ -1039 +1191,3 @@ fn run_teach_propose(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H085 | `src/cli/mod.rs` | `@@ -1058 +1212,6 @@ fn run_teach_propose(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H086 | `src/cli/mod.rs` | `@@ -1063 +1222 @@ fn run_teach_apply(command_id: &'static str, args: &TeachApplyArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H087 | `src/cli/mod.rs` | `@@ -1068 +1227,3 @@ fn run_teach_apply(command_id: &'static str, args: &TeachApplyArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H088 | `src/cli/mod.rs` | `@@ -1087 +1248,6 @@ fn run_teach_apply(command_id: &'static str, args: &TeachApplyArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H089 | `src/cli/mod.rs` | `@@ -1092 +1258 @@ fn run_reflect_inventory(command_id: &'static str, json_output: bool) -> ExitCod` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H090 | `src/cli/mod.rs` | `@@ -1097 +1263 @@ fn run_reflect_inventory(command_id: &'static str, json_output: bool) -> ExitCod` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H091 | `src/cli/mod.rs` | `@@ -1116 +1282,6 @@ fn run_reflect_inventory(command_id: &'static str, json_output: bool) -> ExitCod` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H092 | `src/cli/mod.rs` | `@@ -1129 +1300 @@ fn run_reflect_proposal_create(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H093 | `src/cli/mod.rs` | `@@ -1134 +1305,3 @@ fn run_reflect_proposal_create(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H094 | `src/cli/mod.rs` | `@@ -1153 +1326,6 @@ fn run_reflect_proposal_create(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H095 | `src/cli/mod.rs` | `@@ -1162 +1340 @@ fn run_reflect_proposal_apply(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H096 | `src/cli/mod.rs` | `@@ -1167 +1345,3 @@ fn run_reflect_proposal_apply(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H097 | `src/cli/mod.rs` | `@@ -1186 +1366,6 @@ fn run_reflect_proposal_apply(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H098 | `src/cli/mod.rs` | `@@ -1223,0 +1409 @@ fn parse_teach_payload(payload: &str) -> Result<Value, CliError> {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H099 | `src/cli/mod.rs` | `@@ -1227,0 +1414 @@ fn parse_teach_proposal(payload: &str) -> Result<storage::TeachProposalInput, Cl` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H100 | `src/cli/mod.rs` | `@@ -1234 +1421 @@ fn parse_reflect_proposal_file(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H101 | `src/cli/mod.rs` | `@@ -1237,0 +1425,27 @@ fn parse_reflect_proposal_file(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H102 | `src/cli/mod.rs` | `@@ -1243 +1457 @@ fn run_node_create_input(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H103 | `src/cli/mod.rs` | `@@ -1248 +1462,3 @@ fn run_node_create_input(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H104 | `src/cli/mod.rs` | `@@ -1267 +1483 @@ fn run_node_create_input(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H105 | `src/cli/mod.rs` | `@@ -1270,3 +1486,5 @@ fn run_node_create_input(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H106 | `src/cli/mod.rs` | `@@ -1277 +1495 @@ fn run_init(command_id: &'static str, json_output: bool) -> ExitCode {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H107 | `src/cli/mod.rs` | `@@ -1377,0 +1596,8 @@ where` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H108 | `src/cli/mod.rs` | `@@ -1412 +1638 @@ fn run_node_get(command_id: &'static str, args: &NodeGetArgs, json_output: bool)` | `MODIFY` | next_cursor, --all, explicit completeness |
| H109 | `src/cli/mod.rs` | `@@ -1418,8 +1644,26 @@ fn run_node_list(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H110 | `src/cli/mod.rs` | `@@ -1431 +1675 @@ fn run_node_update(command_id: &'static str, args: &NodeUpdateArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H111 | `src/cli/mod.rs` | `@@ -1446 +1690,3 @@ fn run_node_update(command_id: &'static str, args: &NodeUpdateArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H112 | `src/cli/mod.rs` | `@@ -1466 +1712 @@ fn run_node_update(command_id: &'static str, args: &NodeUpdateArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H113 | `src/cli/mod.rs` | `@@ -1469,3 +1715,5 @@ fn run_node_update(command_id: &'static str, args: &NodeUpdateArgs, json_output:` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H114 | `src/cli/mod.rs` | `@@ -1508 +1756 @@ fn run_doctor(command_id: &'static str, json_output: bool) -> ExitCode {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H115 | `src/cli/mod.rs` | `@@ -1520 +1768 @@ fn run_doctor(command_id: &'static str, json_output: bool) -> ExitCode {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H116 | `src/cli/mod.rs` | `@@ -1528,0 +1777 @@ fn run_doctor(command_id: &'static str, json_output: bool) -> ExitCode {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H117 | `src/cli/mod.rs` | `@@ -1548 +1797 @@ fn run_verify(command_id: &'static str, json_output: bool) -> ExitCode {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H118 | `src/cli/mod.rs` | `@@ -1565 +1814 @@ fn run_verify(command_id: &'static str, json_output: bool) -> ExitCode {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H119 | `src/cli/mod.rs` | `@@ -1573,0 +1823 @@ fn run_verify(command_id: &'static str, json_output: bool) -> ExitCode {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H120 | `src/cli/mod.rs` | `@@ -1600 +1850 @@ fn run_link_add(command_id: &'static str, args: &LinkAddArgs, json_output: bool)` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H121 | `src/cli/mod.rs` | `@@ -1610 +1860,3 @@ fn run_link_add(command_id: &'static str, args: &LinkAddArgs, json_output: bool)` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H122 | `src/cli/mod.rs` | `@@ -1629 +1881 @@ fn run_link_add(command_id: &'static str, args: &LinkAddArgs, json_output: bool)` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H123 | `src/cli/mod.rs` | `@@ -1632,3 +1884,5 @@ fn run_link_add(command_id: &'static str, args: &LinkAddArgs, json_output: bool)` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H124 | `src/cli/mod.rs` | `@@ -1638 +1892 @@ fn run_link_add(command_id: &'static str, args: &LinkAddArgs, json_output: bool)` | `MODIFY` | next_cursor, --all, explicit completeness |
| H125 | `src/cli/mod.rs` | `@@ -1644,2 +1898,2 @@ fn run_link_list(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H126 | `src/cli/mod.rs` | `@@ -1647 +1901,5 @@ fn run_link_list(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H127 | `src/cli/mod.rs` | `@@ -1657 +1915 @@ fn run_alias_add(command_id: &'static str, args: &AliasAddArgs, json_output: boo` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H128 | `src/cli/mod.rs` | `@@ -1666 +1924,3 @@ fn run_alias_add(command_id: &'static str, args: &AliasAddArgs, json_output: boo` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H129 | `src/cli/mod.rs` | `@@ -1685,7 +1945,6 @@ fn run_alias_add(command_id: &'static str, args: &AliasAddArgs, json_output: boo` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H130 | `src/cli/mod.rs` | `@@ -1692,0 +1952,5 @@ fn run_alias_add(command_id: &'static str, args: &AliasAddArgs, json_output: boo` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H131 | `src/cli/mod.rs` | `@@ -1706,2 +1970,2 @@ fn run_alias_list(` | `MODIFY` | next_cursor, --all, explicit completeness |
| H132 | `src/cli/mod.rs` | `@@ -1709 +1973,5 @@ fn run_alias_list(` | `MODIFY` | next_cursor, --all, explicit completeness |
| H133 | `src/cli/mod.rs` | `@@ -1719 +1987 @@ fn run_tag_add(command_id: &'static str, args: &TagAddArgs, json_output: bool) -` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H134 | `src/cli/mod.rs` | `@@ -1728 +1996,3 @@ fn run_tag_add(command_id: &'static str, args: &TagAddArgs, json_output: bool) -` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H135 | `src/cli/mod.rs` | `@@ -1747,7 +2017,6 @@ fn run_tag_add(command_id: &'static str, args: &TagAddArgs, json_output: bool) -` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H136 | `src/cli/mod.rs` | `@@ -1754,0 +2024,5 @@ fn run_tag_add(command_id: &'static str, args: &TagAddArgs, json_output: bool) -` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H137 | `src/cli/mod.rs` | `@@ -1768,2 +2042,2 @@ fn run_tag_list(` | `MODIFY` | next_cursor, --all, explicit completeness |
| H138 | `src/cli/mod.rs` | `@@ -1771 +2045,5 @@ fn run_tag_list(` | `MODIFY` | next_cursor, --all, explicit completeness |
| H139 | `src/cli/mod.rs` | `@@ -1781 +2059 @@ fn run_source_add(command_id: &'static str, args: &SourceAddArgs, json_output: b` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H140 | `src/cli/mod.rs` | `@@ -1790 +2068,3 @@ fn run_source_add(command_id: &'static str, args: &SourceAddArgs, json_output: b` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H141 | `src/cli/mod.rs` | `@@ -1809,7 +2089,6 @@ fn run_source_add(command_id: &'static str, args: &SourceAddArgs, json_output: b` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H142 | `src/cli/mod.rs` | `@@ -1816,0 +2096,5 @@ fn run_source_add(command_id: &'static str, args: &SourceAddArgs, json_output: b` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H143 | `src/cli/mod.rs` | `@@ -1830,2 +2114,2 @@ fn run_source_list(` | `MODIFY` | next_cursor, --all, explicit completeness |
| H144 | `src/cli/mod.rs` | `@@ -1833 +2117,5 @@ fn run_source_list(` | `MODIFY` | next_cursor, --all, explicit completeness |
| H145 | `src/cli/mod.rs` | `@@ -1842 +2130 @@ fn run_source_list(` | `MODIFY` | next_cursor, --all, explicit completeness |
| H146 | `src/cli/mod.rs` | `@@ -1848,2 +2136,2 @@ fn run_mcp_list(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H147 | `src/cli/mod.rs` | `@@ -1851 +2139,5 @@ fn run_mcp_list(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H148 | `src/cli/mod.rs` | `@@ -1861 +2153 @@ fn run_mcp_add(command_id: &'static str, args: &McpAddArgs, json_output: bool) -` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H149 | `src/cli/mod.rs` | `@@ -1878 +2170,3 @@ fn run_mcp_add(command_id: &'static str, args: &McpAddArgs, json_output: bool) -` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H150 | `src/cli/mod.rs` | `@@ -1897,7 +2191,6 @@ fn run_mcp_add(command_id: &'static str, args: &McpAddArgs, json_output: bool) -` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H151 | `src/cli/mod.rs` | `@@ -1904,0 +2198,5 @@ fn run_mcp_add(command_id: &'static str, args: &McpAddArgs, json_output: bool) -` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H152 | `src/cli/mod.rs` | `@@ -1931 +2229 @@ fn run_mcp_get(command_id: &'static str, args: &McpGetArgs, json_output: bool) -` | `MODIFY` | заменить temporary recall final engine |
| H153 | `src/cli/mod.rs` | `@@ -1937,6 +2235,18 @@ fn run_recall(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | заменить temporary recall final engine |
| H154 | `src/cli/mod.rs` | `@@ -1944,0 +2255,6 @@ fn run_recall(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | заменить temporary recall final engine |
| H155 | `src/cli/mod.rs` | `@@ -1949,2 +2265,2 @@ fn run_recall(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | заменить temporary recall final engine |
| H156 | `src/cli/mod.rs` | `@@ -1955 +2271,3 @@ fn run_recall(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | заменить temporary recall final engine |
| H157 | `src/cli/mod.rs` | `@@ -1961 +2279 @@ fn run_recall(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | заменить temporary recall final engine |
| H158 | `src/cli/mod.rs` | `@@ -1967,0 +2286,35 @@ fn run_recall(command_id: &'static str, json_output: bool) -> ExitCode {` | `MODIFY` | заменить temporary recall final engine |
| H159 | `src/cli/mod.rs` | `@@ -1973,5 +2326,4 @@ fn run_adapter_seed(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H160 | `src/cli/mod.rs` | `@@ -1979 +2331 @@ fn run_adapter_seed(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H161 | `src/cli/mod.rs` | `@@ -1987 +2339 @@ fn run_adapter_seed(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H162 | `src/cli/mod.rs` | `@@ -2005,5 +2357,4 @@ fn run_adapter_sync(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H163 | `src/cli/mod.rs` | `@@ -2011 +2362 @@ fn run_adapter_sync(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H164 | `src/cli/mod.rs` | `@@ -2021 +2372 @@ fn run_adapter_sync(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H165 | `src/cli/mod.rs` | `@@ -2039,5 +2390,4 @@ fn run_adapter_status(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H166 | `src/cli/mod.rs` | `@@ -2045 +2395 @@ fn run_adapter_status(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H167 | `src/cli/mod.rs` | `@@ -2053 +2403 @@ fn run_adapter_status(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H168 | `src/cli/mod.rs` | `@@ -2070,2 +2420,2 @@ fn resolve_adapter_instruction_file(` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H169 | `src/cli/mod.rs` | `@@ -2074,0 +2425,10 @@ fn resolve_adapter_instruction_file(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H170 | `src/cli/mod.rs` | `@@ -2076 +2436 @@ fn resolve_adapter_instruction_file(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H171 | `src/cli/mod.rs` | `@@ -2078 +2438,6 @@ fn resolve_adapter_instruction_file(` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H172 | `src/cli/mod.rs` | `@@ -2082 +2447 @@ fn open_current_workspace() -> Result<(String, rusqlite::Connection), CliError>` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H173 | `src/cli/mod.rs` | `@@ -2085,0 +2451,20 @@ fn open_current_workspace() -> Result<(String, rusqlite::Connection), CliError>` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H174 | `src/cli/mod.rs` | `@@ -2088 +2473 @@ fn open_current_workspace_context(` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H175 | `src/cli/mod.rs` | `@@ -2098,0 +2484,19 @@ fn open_current_workspace_context(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H176 | `src/cli/mod.rs` | `@@ -2103 +2507 @@ fn write_audit_snapshot(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H177 | `src/cli/mod.rs` | `@@ -2105,10 +2509,6 @@ fn write_audit_snapshot(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H178 | `src/cli/mod.rs` | `@@ -2115,0 +2516,2 @@ fn write_audit_snapshot(` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H179 | `src/cli/mod.rs` | `@@ -2160 +2562 @@ fn command_id(command: &Command) -> &'static str {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H180 | `src/cli/mod.rs` | `@@ -2165 +2567 @@ fn command_id(command: &Command) -> &'static str {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H181 | `src/cli/mod.rs` | `@@ -2179 +2581 @@ fn command_id(command: &Command) -> &'static str {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H182 | `src/cli/mod.rs` | `@@ -2196 +2598 @@ fn command_id(command: &Command) -> &'static str {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H183 | `src/cli/mod.rs` | `@@ -2202 +2604 @@ fn command_id(command: &Command) -> &'static str {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H184 | `src/cli/mod.rs` | `@@ -2234,0 +2637,18 @@ impl CliError {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H185 | `src/cli/mod.rs` | `@@ -2322 +2742 @@ impl CliError {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval text/tests |
| H186 | `src/cli/mod.rs` | `@@ -2466,0 +2887,18 @@ impl CliError {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H187 | `src/cli/mod.rs` | `@@ -2482 +2920 @@ impl CliError {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H188 | `src/cli/mod.rs` | `@@ -2617,4 +3055,4 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H189 | `src/cli/mod.rs` | `@@ -2622,7 +3060,2 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H190 | `src/cli/mod.rs` | `@@ -2630,7 +3063,80 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H191 | `src/cli/mod.rs` | `@@ -2658 +3164,12 @@ mod tests {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H192 | `src/cli/mod.rs` | `@@ -3109,0 +3627,70 @@ mod tests {` | `MODIFY` | next_cursor, --all, explicit completeness |
| H193 | `src/cli/mod.rs` | `@@ -3463,0 +4051,2 @@ mod tests {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval text/tests |
| H194 | `src/cli/mod.rs` | `@@ -3472 +4061 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H195 | `src/cli/mod.rs` | `@@ -3478 +4067 @@ mod tests {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval text/tests |
| H196 | `src/cli/mod.rs` | `@@ -3480 +4069 @@ mod tests {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval text/tests |
| H197 | `src/cli/mod.rs` | `@@ -3657,0 +4247 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H198 | `src/cli/mod.rs` | `@@ -3758,0 +4349,10 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H199 | `src/cli/mod.rs` | `@@ -3760,0 +4361,58 @@ mod tests {` | `MODIFY` | заменить temporary recall final engine |
| H200 | `src/cli/mod.rs` | `@@ -4093,0 +4752,61 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H201 | `src/cli/mod.rs` | `@@ -4352,0 +5072,45 @@ mod tests {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H202 | `src/cli/mod.rs` | `@@ -4397,0 +5162,12 @@ mod tests {` | `PREEXISTING_PRESERVE` | Windows/root remediation; сохранить |
| H203 | `src/cli/mod.rs` | `@@ -4437,0 +5214,78 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H204 | `src/cli/mod.rs` | `@@ -4655 +5509 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H205 | `src/cli/mod.rs` | `@@ -4704 +5558,19 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H206 | `src/cli/mod.rs` | `@@ -4712,0 +5585,243 @@ mod tests {` | `KEEP` | приняты transaction/read-only/validation/boxed enum/warning основы |
| H207 | `src/install/mod.rs` | `@@ -102,0 +103,2 @@ pub enum WorkspaceInitError {` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H208 | `src/install/mod.rs` | `@@ -231 +233 @@ pub fn init_workspace(` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H209 | `src/install/mod.rs` | `@@ -233 +235 @@ pub fn init_workspace(` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H210 | `src/install/mod.rs` | `@@ -262,2 +264 @@ where` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H211 | `src/install/mod.rs` | `@@ -264,0 +266 @@ where` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H212 | `src/install/mod.rs` | `@@ -270 +272 @@ where` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H213 | `src/install/mod.rs` | `@@ -616,0 +619,3 @@ where` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H214 | `src/install/mod.rs` | `@@ -640,2 +645,2 @@ where` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H215 | `src/install/mod.rs` | `@@ -648,0 +654 @@ where` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H216 | `src/install/mod.rs` | `@@ -651,0 +658,10 @@ where` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H217 | `src/install/mod.rs` | `@@ -913,0 +930,92 @@ mod tests {` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H218 | `src/install/mod.rs` | `@@ -978,2 +1086,2 @@ mod tests {` | `PREEXISTING_PRESERVE` | Windows UTF-8/USERPROFILE/root remediation; сохранить |
| H219 | `src/recall/mod.rs` | `@@ -23,0 +24,9 @@ pub struct StructuredRecallBundle {` | `KEEP` | принят direct tool/rule fix; перенести в final pipeline |
| H220 | `src/recall/mod.rs` | `@@ -146,0 +156 @@ pub fn build_structured_bundle_with_links(` | `KEEP` | принят direct tool/rule fix; перенести в final pipeline |
| H221 | `src/recall/mod.rs` | `@@ -157,0 +168 @@ pub fn build_structured_bundle_with_links(` | `KEEP` | принят direct tool/rule fix; перенести в final pipeline |
| H222 | `src/recall/mod.rs` | `@@ -180,0 +192 @@ pub fn derive_fts_fallback_query(bundle: &StructuredRecallBundle) -> Option<Stri` | `KEEP` | принят direct tool/rule fix; перенести в final pipeline |
| H223 | `src/recall/mod.rs` | `@@ -206,0 +219,11 @@ pub fn add_fts_fallback(` | `MODIFY` | временный bounded recall заменить final recall |
| H224 | `src/recall/mod.rs` | `@@ -362,0 +386 @@ fn all_bundle_nodes(bundle: &StructuredRecallBundle) -> Vec<&Node> {` | `KEEP` | принят direct tool/rule fix; перенести в final pipeline |
| H225 | `src/recall/mod.rs` | `@@ -563,0 +588 @@ fn structured_node_ids(bundle: &StructuredRecallBundle) -> HashSet<i64> {` | `KEEP` | принят direct tool/rule fix; перенести в final pipeline |
| H226 | `src/recall/mod.rs` | `@@ -659,0 +685,18 @@ mod tests {` | `KEEP` | принят direct tool/rule fix; перенести в final pipeline |
| H227 | `src/recall/mod.rs` | `@@ -735,0 +779,34 @@ mod tests {` | `MODIFY` | временный bounded recall заменить final recall |
| H228 | `src/reflection/mod.rs` | `@@ -1 +1 @@` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H229 | `src/reflection/mod.rs` | `@@ -185,0 +186,2 @@ pub enum ReflectionValidationError {` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H230 | `src/reflection/mod.rs` | `@@ -195,0 +198,2 @@ pub enum ReflectionValidationError {` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H231 | `src/reflection/mod.rs` | `@@ -253 +257,58 @@ pub fn inventory_sessions(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H232 | `src/reflection/mod.rs` | `@@ -260 +321 @@ pub fn inventory_sessions(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H233 | `src/reflection/mod.rs` | `@@ -265,9 +326 @@ pub fn inventory_sessions(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H234 | `src/reflection/mod.rs` | `@@ -315,0 +369,14 @@ pub fn apply_proposal(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H235 | `src/reflection/mod.rs` | `@@ -329,0 +397 @@ pub fn apply_proposal(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H236 | `src/reflection/mod.rs` | `@@ -340 +408,6 @@ pub fn apply_proposal(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H237 | `src/reflection/mod.rs` | `@@ -389,0 +463 @@ pub fn apply_proposal(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H238 | `src/reflection/mod.rs` | `@@ -409 +483,10 @@ pub fn list_reflected_sessions(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H239 | `src/reflection/mod.rs` | `@@ -489,0 +573,6 @@ fn validate_reflection_proposal(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H240 | `src/reflection/mod.rs` | `@@ -591,0 +681,6 @@ fn validate_optional_ref(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H241 | `src/reflection/mod.rs` | `@@ -636,0 +732 @@ fn apply_low_risk_item(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H242 | `src/reflection/mod.rs` | `@@ -690 +786 @@ fn apply_low_risk_item(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H243 | `src/reflection/mod.rs` | `@@ -696,0 +793 @@ fn apply_low_risk_item(` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H244 | `src/reflection/mod.rs` | `@@ -897,0 +995,22 @@ mod tests {` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H245 | `src/reflection/mod.rs` | `@@ -902 +1021 @@ mod tests {` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H246 | `src/reflection/mod.rs` | `@@ -957,0 +1077 @@ mod tests {` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H247 | `src/reflection/mod.rs` | `@@ -964,0 +1085,8 @@ mod tests {` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H248 | `src/reflection/mod.rs` | `@@ -1050,0 +1179,48 @@ mod tests {` | `KEEP` | приняты current inventory, transaction и batched FTS; добавить events |
| H249 | `src/schema/mod.rs` | `@@ -0,0 +1,2 @@` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H250 | `src/schema/mod.rs` | `@@ -3,0 +6 @@ pub const MIGRATION_001_INIT: &str = "001_init";` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H251 | `src/schema/mod.rs` | `@@ -11,4 +14,5 @@ struct Migration {` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H252 | `src/schema/mod.rs` | `@@ -139,2 +143,10 @@ const MIGRATIONS: &[Migration] = &[Migration {` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H253 | `src/schema/mod.rs` | `@@ -144,0 +157,9 @@ pub fn apply_migrations(connection: &mut Connection) -> rusqlite::Result<()> {` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H254 | `src/schema/mod.rs` | `@@ -146 +167 @@ pub fn apply_migrations(connection: &mut Connection) -> rusqlite::Result<()> {` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H255 | `src/schema/mod.rs` | `@@ -158,0 +180,8 @@ pub fn apply_migrations(connection: &mut Connection) -> rusqlite::Result<()> {` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H256 | `src/schema/mod.rs` | `@@ -159,0 +189,15 @@ fn ensure_schema_migrations_table(connection: &Connection) -> rusqlite::Result<(` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H257 | `src/schema/mod.rs` | `@@ -192,0 +237,18 @@ mod tests {` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H258 | `src/schema/mod.rs` | `@@ -207 +269,35 @@ mod tests {` | `KEEP` | приняты pending-only migrations и idx_nodes_summary; 001 immutable |
| H259 | `src/storage/mod.rs` | `@@ -0,0 +1 @@` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H260 | `src/storage/mod.rs` | `@@ -9 +10 @@ use std::path::PathBuf;` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H261 | `src/storage/mod.rs` | `@@ -18,0 +20 @@ const HOME_ENV: &str = "HOME";` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H262 | `src/storage/mod.rs` | `@@ -21,0 +24,9 @@ const STORAGE_AUDIT_SOURCE: &str = "aopmem_cli";` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H263 | `src/storage/mod.rs` | `@@ -30,0 +42,18 @@ const TEACH_CREATED_LINK_TYPE: &str = "teach_created_node";` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H264 | `src/storage/mod.rs` | `@@ -150,0 +180,33 @@ pub struct FtsNodeSearchResult {` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H265 | `src/storage/mod.rs` | `@@ -423,0 +486 @@ pub enum PathResolveError {` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H266 | `src/storage/mod.rs` | `@@ -429,0 +493,4 @@ impl fmt::Display for PathResolveError {` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H267 | `src/storage/mod.rs` | `@@ -452,0 +520,21 @@ impl std::error::Error for WorkspaceKeyError {}` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H268 | `src/storage/mod.rs` | `@@ -457,0 +546,4 @@ pub enum NodeValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H269 | `src/storage/mod.rs` | `@@ -468,0 +561,3 @@ impl fmt::Display for NodeValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H270 | `src/storage/mod.rs` | `@@ -486,0 +582 @@ pub enum LinkValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H271 | `src/storage/mod.rs` | `@@ -494,0 +591,3 @@ impl fmt::Display for LinkValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H272 | `src/storage/mod.rs` | `@@ -508,0 +608,4 @@ pub enum MetadataValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H273 | `src/storage/mod.rs` | `@@ -517,0 +621,3 @@ impl fmt::Display for MetadataValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H274 | `src/storage/mod.rs` | `@@ -533,0 +640,4 @@ pub enum McpProfileValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H275 | `src/storage/mod.rs` | `@@ -552,0 +663,3 @@ impl fmt::Display for McpProfileValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H276 | `src/storage/mod.rs` | `@@ -654,0 +768,2 @@ pub enum TeachValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H277 | `src/storage/mod.rs` | `@@ -682,0 +798,9 @@ impl fmt::Display for TeachValidationError {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H278 | `src/storage/mod.rs` | `@@ -798 +922,19 @@ pub fn resolve_paths() -> Result<AopmemPaths, PathResolveError> {` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H279 | `src/storage/mod.rs` | `@@ -802,2 +944,3 @@ pub fn workspace_key(repo_root: impl AsRef<Path>) -> Result<String, WorkspaceKey` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H280 | `src/storage/mod.rs` | `@@ -807,5 +950,4 @@ pub fn workspace_key(repo_root: impl AsRef<Path>) -> Result<String, WorkspaceKey` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H281 | `src/storage/mod.rs` | `@@ -841,0 +984,7 @@ pub fn ensure_workspace_dirs(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H282 | `src/storage/mod.rs` | `@@ -849,0 +999,13 @@ pub fn open_workspace_db(workspace_paths: &WorkspacePaths) -> rusqlite::Result<C` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H283 | `src/storage/mod.rs` | `@@ -991,0 +1154,128 @@ pub fn list_nodes(connection: &Connection) -> rusqlite::Result<Vec<Node>> {` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H284 | `src/storage/mod.rs` | `@@ -1034,0 +1325,288 @@ pub fn search_nodes_fts(` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H285 | `src/storage/mod.rs` | `@@ -1090,0 +1669,27 @@ pub fn list_links(connection: &Connection) -> rusqlite::Result<Vec<Link>> {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H286 | `src/storage/mod.rs` | `@@ -1093,0 +1699,10 @@ pub fn create_alias(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H287 | `src/storage/mod.rs` | `@@ -1109 +1723,0 @@ pub fn create_alias(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H288 | `src/storage/mod.rs` | `@@ -1115,7 +1729,69 @@ pub fn list_aliases(connection: &Connection, node_id: Option<i64>) -> rusqlite::` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H289 | `src/storage/mod.rs` | `@@ -1123,2 +1799 @@ pub fn list_aliases(connection: &Connection, node_id: Option<i64>) -> rusqlite::` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H290 | `src/storage/mod.rs` | `@@ -1145,10 +1820,25 @@ pub fn list_tags(connection: &Connection, node_id: Option<i64>) -> rusqlite::Res` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H291 | `src/storage/mod.rs` | `@@ -1157 +1847 @@ pub fn list_tags(connection: &Connection, node_id: Option<i64>) -> rusqlite::Res` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H292 | `src/storage/mod.rs` | `@@ -1159,5 +1849,51 @@ pub fn create_source(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H293 | `src/storage/mod.rs` | `@@ -1186,7 +1922,69 @@ pub fn list_sources(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H294 | `src/storage/mod.rs` | `@@ -1194,2 +1992 @@ pub fn list_sources(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H295 | `src/storage/mod.rs` | `@@ -1297,0 +2095,15 @@ pub fn apply_teach_proposal(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H296 | `src/storage/mod.rs` | `@@ -1314,0 +2127 @@ pub fn apply_teach_proposal(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H297 | `src/storage/mod.rs` | `@@ -1360 +2173 @@ pub fn apply_teach_proposal(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H298 | `src/storage/mod.rs` | `@@ -1366,0 +2180 @@ pub fn apply_teach_proposal(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H299 | `src/storage/mod.rs` | `@@ -1463,0 +2278 @@ pub fn apply_teach_proposal(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H300 | `src/storage/mod.rs` | `@@ -1620,0 +2436,65 @@ pub fn list_mcp_profiles(connection: &Connection) -> rusqlite::Result<Vec<McpPro` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H301 | `src/storage/mod.rs` | `@@ -1724,0 +2605,6 @@ fn validate_teach_proposal(proposal: &TeachProposalInput) -> Result<(), TeachVal` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H302 | `src/storage/mod.rs` | `@@ -1729,0 +2616 @@ fn validate_teach_proposal(proposal: &TeachProposalInput) -> Result<(), TeachVal` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H303 | `src/storage/mod.rs` | `@@ -1759,0 +2647,10 @@ fn validate_teach_proposal(proposal: &TeachProposalInput) -> Result<(), TeachVal` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H304 | `src/storage/mod.rs` | `@@ -1769,0 +2667 @@ fn validate_teach_node_target(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H305 | `src/storage/mod.rs` | `@@ -1799,0 +2698,13 @@ fn validate_new_node(node: &NewNode) -> Result<(), NodeValidationError> {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H306 | `src/storage/mod.rs` | `@@ -1814,0 +2726,24 @@ fn validate_new_node(node: &NewNode) -> Result<(), NodeValidationError> {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H307 | `src/storage/mod.rs` | `@@ -1839,0 +2775,50 @@ fn validate_new_mcp_profile(profile: &NewMcpProfile) -> Result<(), McpProfileVal` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H308 | `src/storage/mod.rs` | `@@ -1847,0 +2833,6 @@ fn validate_new_link(connection: &Connection, link: &NewLink) -> Result<(), Link` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H309 | `src/storage/mod.rs` | `@@ -1880,0 +2872,12 @@ fn validate_node_metadata(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H310 | `src/storage/mod.rs` | `@@ -1906,0 +2910,11 @@ fn refresh_fts_node(connection: &Connection, node_id: i64) -> rusqlite::Result<(` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H311 | `src/storage/mod.rs` | `@@ -1935,0 +2950,41 @@ fn fts_match_query(query: &str) -> Option<String> {` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H312 | `src/storage/mod.rs` | `@@ -2156,3 +3211,5 @@ fn side_effects_rank(side_effects: &str) -> u8 {` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H313 | `src/storage/mod.rs` | `@@ -2161,8 +3218,3 @@ fn resolve_paths_from_env(` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H314 | `src/storage/mod.rs` | `@@ -2179,0 +3232,18 @@ fn resolve_paths_from_env(` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H315 | `src/storage/mod.rs` | `@@ -2229 +3299,72 @@ fn sanitize_repo_folder_name(folder_name: &str) -> String {` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H316 | `src/storage/mod.rs` | `@@ -2231 +3372 @@ fn hash_absolute_path(repo_root: &Path) -> u32 {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H317 | `src/storage/mod.rs` | `@@ -2335,0 +3477,55 @@ mod tests {` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H318 | `src/storage/mod.rs` | `@@ -2354,0 +3551,23 @@ mod tests {` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H319 | `src/storage/mod.rs` | `@@ -2366,0 +3586,12 @@ mod tests {` | `PREEXISTING_PRESERVE` | Windows/root remediation; добавить legacy-key compatibility |
| H320 | `src/storage/mod.rs` | `@@ -2382,0 +3614,37 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H321 | `src/storage/mod.rs` | `@@ -2498 +3766 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H322 | `src/storage/mod.rs` | `@@ -2503,0 +3772,22 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H323 | `src/storage/mod.rs` | `@@ -2525 +3815 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H324 | `src/storage/mod.rs` | `@@ -2557,0 +3848,67 @@ mod tests {` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H325 | `src/storage/mod.rs` | `@@ -2726 +4083 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H326 | `src/storage/mod.rs` | `@@ -2728 +4085 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H327 | `src/storage/mod.rs` | `@@ -2731,3 +4088,3 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H328 | `src/storage/mod.rs` | `@@ -2735 +4092 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H329 | `src/storage/mod.rs` | `@@ -2744 +4101,4 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H330 | `src/storage/mod.rs` | `@@ -2746,0 +4107,3 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H331 | `src/storage/mod.rs` | `@@ -2750,5 +4113,3 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H332 | `src/storage/mod.rs` | `@@ -2757 +4118 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H333 | `src/storage/mod.rs` | `@@ -2764 +4125,72 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H334 | `src/storage/mod.rs` | `@@ -2796,0 +4229,50 @@ mod tests {` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H335 | `src/storage/mod.rs` | `@@ -2908,0 +4391,26 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H336 | `src/storage/mod.rs` | `@@ -2918 +4426 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H337 | `src/storage/mod.rs` | `@@ -2922 +4430 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H338 | `src/storage/mod.rs` | `@@ -2926 +4434,162 @@ mod tests {` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H339 | `src/storage/mod.rs` | `@@ -2987,0 +4657,71 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H340 | `src/storage/mod.rs` | `@@ -3022,0 +4763,26 @@ mod tests {` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H341 | `src/storage/mod.rs` | `@@ -3094,0 +4861,24 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H342 | `src/storage/mod.rs` | `@@ -3213,0 +5004,15 @@ mod tests {` | `KEEP` | safe storage optimization: validation/SQL/transaction/FTS/read-only |
| H343 | `src/storage/mod.rs` | `@@ -3266,0 +5072,255 @@ mod tests {` | `MODIFY` | сохранить targeted/keyset основу; final cursor/body/recall contract |
| H344 | `src/tools/mod.rs` | `@@ -4,3 +4,5 @@ use std::fs;` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H345 | `src/tools/mod.rs` | `@@ -24,0 +27,18 @@ pub const TOOL_RUNTIME_DIR_NAME: &str = "runtime";` | `MODIFY` | сохранить drain; tool-specific limits, 15m/10MiB, artifact mode |
| H346 | `src/tools/mod.rs` | `@@ -82,0 +103,8 @@ pub struct ToolContractRecord {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H347 | `src/tools/mod.rs` | `@@ -126,0 +155,52 @@ pub struct ToolRunRecord {` | `MODIFY` | сохранить drain; tool-specific limits, 15m/10MiB, artifact mode |
| H348 | `src/tools/mod.rs` | `@@ -152,0 +233,12 @@ pub enum ToolContractValidationError {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H349 | `src/tools/mod.rs` | `@@ -170,0 +263,2 @@ pub enum ToolContractValidationError {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H350 | `src/tools/mod.rs` | `@@ -174,0 +269,2 @@ pub enum ToolContractValidationError {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H351 | `src/tools/mod.rs` | `@@ -251,0 +348,13 @@ pub enum RunToolError {` | `MODIFY` | сохранить drain; tool-specific limits, 15m/10MiB, artifact mode |
| H352 | `src/tools/mod.rs` | `@@ -386,0 +496,89 @@ pub fn list_tool_contracts(connection: &Connection) -> rusqlite::Result<Vec<Tool` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H353 | `src/tools/mod.rs` | `@@ -420,0 +619,14 @@ pub fn create_draft_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H354 | `src/tools/mod.rs` | `@@ -423 +635,2 @@ pub fn create_draft_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H355 | `src/tools/mod.rs` | `@@ -425,8 +638,7 @@ pub fn create_draft_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H356 | `src/tools/mod.rs` | `@@ -434,2 +646,6 @@ pub fn create_draft_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H357 | `src/tools/mod.rs` | `@@ -437,5 +653,9 @@ pub fn create_draft_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H358 | `src/tools/mod.rs` | `@@ -444,0 +665,14 @@ pub fn create_draft_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H359 | `src/tools/mod.rs` | `@@ -453,0 +688,46 @@ pub fn create_draft_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H360 | `src/tools/mod.rs` | `@@ -461,4 +741,2 @@ pub fn validate_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H361 | `src/tools/mod.rs` | `@@ -469,0 +748,6 @@ pub fn validate_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H362 | `src/tools/mod.rs` | `@@ -515,0 +800,19 @@ pub fn run_tool(` | `MODIFY` | сохранить drain; tool-specific limits, 15m/10MiB, artifact mode |
| H363 | `src/tools/mod.rs` | `@@ -518,0 +822 @@ pub fn run_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H364 | `src/tools/mod.rs` | `@@ -522 +826 @@ pub fn run_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H365 | `src/tools/mod.rs` | `@@ -532,0 +837,6 @@ pub fn run_tool(` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H366 | `src/tools/mod.rs` | `@@ -534,4 +844 @@ pub fn run_tool(` | `MODIFY` | сохранить drain; tool-specific limits, 15m/10MiB, artifact mode |
| H367 | `src/tools/mod.rs` | `@@ -551,2 +858,92 @@ pub fn run_tool(` | `MODIFY` | сохранить drain; tool-specific limits, 15m/10MiB, artifact mode |
| H368 | `src/tools/mod.rs` | `@@ -555,0 +953,117 @@ pub fn run_tool(` | `MODIFY` | сохранить drain; tool-specific limits, 15m/10MiB, artifact mode |
| H369 | `src/tools/mod.rs` | `@@ -592,0 +1107,6 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H370 | `src/tools/mod.rs` | `@@ -595,0 +1116 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H371 | `src/tools/mod.rs` | `@@ -598,0 +1120 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H372 | `src/tools/mod.rs` | `@@ -605,0 +1128,3 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H373 | `src/tools/mod.rs` | `@@ -608,0 +1134,5 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H374 | `src/tools/mod.rs` | `@@ -611,0 +1142 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H375 | `src/tools/mod.rs` | `@@ -614,0 +1146 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H376 | `src/tools/mod.rs` | `@@ -622,0 +1155,5 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H377 | `src/tools/mod.rs` | `@@ -625,0 +1163,5 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H378 | `src/tools/mod.rs` | `@@ -632,0 +1175,9 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H379 | `src/tools/mod.rs` | `@@ -635,0 +1187,27 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H380 | `src/tools/mod.rs` | `@@ -639,0 +1218,23 @@ fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractVal` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H381 | `src/tools/mod.rs` | `@@ -641,3 +1242,29 @@ fn resolve_executable_path(tool_root: &Path, executable_path: &str) -> PathBuf {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H382 | `src/tools/mod.rs` | `@@ -645 +1272,5 @@ fn resolve_executable_path(tool_root: &Path, executable_path: &str) -> PathBuf {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H383 | `src/tools/mod.rs` | `@@ -675 +1306,2 @@ fn requires_approval(contract: &ToolContract) -> bool {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval; вернуть risk/contract policy |
| H384 | `src/tools/mod.rs` | `@@ -681,0 +1314,8 @@ fn requires_approval(contract: &ToolContract) -> bool {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval; вернуть risk/contract policy |
| H385 | `src/tools/mod.rs` | `@@ -785,0 +1426,31 @@ mod tests {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H386 | `src/tools/mod.rs` | `@@ -806,0 +1478,65 @@ mod tests {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H387 | `src/tools/mod.rs` | `@@ -857,0 +1594,63 @@ mod tests {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H388 | `src/tools/mod.rs` | `@@ -900,0 +1700,56 @@ mod tests {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H389 | `src/tools/mod.rs` | `@@ -1071 +1926 @@ mod tests {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval; вернуть risk/contract policy |
| H390 | `src/tools/mod.rs` | `@@ -1073 +1928 @@ mod tests {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval; вернуть risk/contract policy |
| H391 | `src/tools/mod.rs` | `@@ -1083,0 +1939,48 @@ mod tests {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval; вернуть risk/contract policy |
| H392 | `src/tools/mod.rs` | `@@ -1249 +2152 @@ mod tests {` | `KEEP` | приняты validation, atomic draft, containment, pagination, dry-run |
| H393 | `src/tools/mod.rs` | `@@ -1290 +2193 @@ mod tests {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval; вернуть risk/contract policy |
| H394 | `src/tools/mod.rs` | `@@ -1292 +2195 @@ mod tests {` | `REVERT_OPTIMIZATION_ONLY` | удалить draft-only approval; вернуть risk/contract policy |
| H395 | `src/tools/mod.rs` | `@@ -1402,0 +2306,118 @@ mod tests {` | `MODIFY` | сохранить drain; tool-specific limits, 15m/10MiB, artifact mode |
| H396 | `src/verify/mod.rs` | `@@ -9,0 +10 @@ use crate::adapter;` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H397 | `src/verify/mod.rs` | `@@ -23,2 +24,2 @@ const REQUIRED_SCHEMA_TABLES: &[&str] = &[` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H398 | `src/verify/mod.rs` | `@@ -82,0 +84 @@ pub struct DoctorChecks {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H399 | `src/verify/mod.rs` | `@@ -152,0 +155,8 @@ pub struct ArtifactsDirsHealth {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H400 | `src/verify/mod.rs` | `@@ -173,0 +184 @@ pub struct LintSummary {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H401 | `src/verify/mod.rs` | `@@ -194,0 +206,10 @@ pub enum LintIssueKind {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H402 | `src/verify/mod.rs` | `@@ -221 +241,0 @@ pub enum LintError {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H403 | `src/verify/mod.rs` | `@@ -240,3 +260,2 @@ pub fn run_doctor(repo_root: &Path) -> Result<DoctorReport, DoctorError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H404 | `src/verify/mod.rs` | `@@ -252,0 +272 @@ pub fn run_doctor(repo_root: &Path) -> Result<DoctorReport, DoctorError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H405 | `src/verify/mod.rs` | `@@ -269,0 +290 @@ pub fn run_doctor(repo_root: &Path) -> Result<DoctorReport, DoctorError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H406 | `src/verify/mod.rs` | `@@ -275 +295,0 @@ pub fn run_doctor(repo_root: &Path) -> Result<DoctorReport, DoctorError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H407 | `src/verify/mod.rs` | `@@ -280,4 +300,3 @@ pub fn run_lint(repo_root: &Path) -> Result<LintReport, LintError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H408 | `src/verify/mod.rs` | `@@ -289 +308 @@ pub fn run_lint(repo_root: &Path) -> Result<LintReport, LintError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H409 | `src/verify/mod.rs` | `@@ -293 +311,0 @@ pub fn run_lint(repo_root: &Path) -> Result<LintReport, LintError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H410 | `src/verify/mod.rs` | `@@ -322,0 +341 @@ pub fn run_lint(repo_root: &Path) -> Result<LintReport, LintError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H411 | `src/verify/mod.rs` | `@@ -335,0 +355,23 @@ pub fn run_lint(repo_root: &Path) -> Result<LintReport, LintError> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H412 | `src/verify/mod.rs` | `@@ -393 +435 @@ fn find_broken_link_issues(` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H413 | `src/verify/mod.rs` | `@@ -425 +467 @@ fn find_deprecated_active_link_issues(` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H414 | `src/verify/mod.rs` | `@@ -460 +502 @@ fn find_deprecated_active_link_issues(` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H415 | `src/verify/mod.rs` | `@@ -472 +514 @@ fn find_missing_source_issues(nodes: &[storage::Node]) -> Vec<LintIssue> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H416 | `src/verify/mod.rs` | `@@ -484 +526 @@ fn find_missing_summary_issues(nodes: &[storage::Node]) -> Vec<LintIssue> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H417 | `src/verify/mod.rs` | `@@ -504,2 +546,4 @@ fn summarize_lint_issues(issues: &[LintIssue]) -> LintSummary {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H418 | `src/verify/mod.rs` | `@@ -517,0 +562 @@ fn summarize_lint_issues(issues: &[LintIssue]) -> LintSummary {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H419 | `src/verify/mod.rs` | `@@ -523,0 +569,18 @@ fn summarize_lint_issues(issues: &[LintIssue]) -> LintSummary {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H420 | `src/verify/mod.rs` | `@@ -563 +626 @@ fn find_schema_drift_issues(connection: &Connection) -> Vec<LintIssue> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H421 | `src/verify/mod.rs` | `@@ -569 +632 @@ fn find_schema_drift_issues(connection: &Connection) -> Vec<LintIssue> {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H422 | `src/verify/mod.rs` | `@@ -693 +756 @@ fn inspect_path(path: &Path) -> PathHealth {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H423 | `src/verify/mod.rs` | `@@ -696,7 +759,11 @@ fn inspect_db(path: &Path) -> DbHealth {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H424 | `src/verify/mod.rs` | `@@ -706,14 +773,25 @@ fn inspect_db(path: &Path) -> DbHealth {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H425 | `src/verify/mod.rs` | `@@ -723,23 +801,7 @@ fn inspect_db(path: &Path) -> DbHealth {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H426 | `src/verify/mod.rs` | `@@ -784,21 +846,6 @@ fn inspect_schema_with_connection(connection: &Connection) -> SchemaHealth {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H427 | `src/verify/mod.rs` | `@@ -887,0 +935,24 @@ fn inspect_artifacts_dirs(` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H428 | `src/verify/mod.rs` | `@@ -930 +1001 @@ fn combine_statuses(statuses: &[DoctorStatus]) -> DoctorStatus {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H429 | `src/verify/mod.rs` | `@@ -932,4 +1003 @@ fn combine_statuses(statuses: &[DoctorStatus]) -> DoctorStatus {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H430 | `src/verify/mod.rs` | `@@ -1040,0 +1109 @@ mod tests {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H431 | `src/verify/mod.rs` | `@@ -1046,0 +1116,45 @@ mod tests {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H432 | `src/verify/mod.rs` | `@@ -1079,0 +1194,25 @@ mod tests {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H433 | `src/verify/mod.rs` | `@@ -1336 +1475 @@ mod tests {` | `KEEP` | приняты read-only health, lean projection, pending snapshot checks |
| H434 | `templates/managed-block/AGENTS.managed-block.md` | `@@ -2,0 +3,14 @@ This block is managed by AOPMem.` | `MODIFY` | сохранить managed contract; удалить draft +++ и добавить v0.2 contract |

## Materialization gate

- `UNKNOWN_BLOCKER` must equal zero before materialization.
- Apply recovery without reset or checkout.
- Never revert a mixed file as one unit.
- Keep recovery refs through final handoff.


---

## 2. Final decision log

Source: `.devplan/V020_FINAL_DECISION_LOG.md`

# AOPMem v0.2.0-rc1 Final Decision Log

## Frozen decisions

| ID | Decision | Status |
|---|---|---|
| D-001 | `7c6bf85e...` is authoritative mixed checkpoint | accepted |
| D-002 | `cdad5a9b...` is archive-only; never restore wholesale | accepted |
| D-003 | Recall budget unit is canonical JSON UTF-8 bytes | accepted |
| D-004 | Task soft budget is 256 KiB | accepted |
| D-005 | Mandatory hard budget is 1 MiB | accepted |
| D-006 | Mandatory active types: kernel_contract, gate, project_profile, source, rule | accepted |
| D-007 | Mandatory overflow returns `MANDATORY_CONTEXT_OVERFLOW` and node ids | accepted |
| D-008 | Bundle and correlation ids are lowercase UUID v4 | accepted |
| D-009 | Legacy tool output mode defaults to `inline` | accepted |
| D-010 | `recall --full` is debug/audit/export/migration mode only | accepted |
| D-011 | `--all-workspaces` migrates every DB; adapter/doctor/verify target current repo | accepted |
| D-012 | `upgrade plan`, observe read commands, and UI do not write observability | accepted |
| D-013 | Normal list page is 100; maximum is 500; JSON always exposes completeness | accepted |
| D-014 | Tool ceilings: 15 minutes and 10 MiB per output stream | accepted |
| D-015 | No daemon, cloud, telemetry, Node.js, WSL, CI, or extra release targets | accepted |
| D-016 | Stop after RC proof; no push, tag, GitHub Release, real workspace install, backup deletion | accepted |
| D-017 | Active same-UID DB/path tampering, including the Unix leaf-name race between inode verification and `linkat`, is outside the local/no-sandbox v0.2 boundary; do not add a custom SQLite VFS or platform-specific rename syscall expansion | accepted |
| D-018 | Effectiveness facts use one inclusive 30-day SQLite read snapshot, bounded top-20 lists with explicit `more_results`, and no synthetic score or advice | accepted |
| D-019 | Recall report facts use lifecycle-event timestamps, terminal `more_results` uses the last in-window completion, selected nodes use `first_seen_at`, and adapter drift exposes missing/drifted/failed counts | accepted; clean re-audit P1=0 P2=0 P3=0 |
| D-020 | Debug capsules use the exact ordered 12-entry deterministic Stored ZIP64 contract, derive reference time from persisted observability or the fixed epoch when missing/empty, never overwrite output, and represent missing observability as `not_collected` | accepted; independent audit P1=0 P2=0 P3=1 under D-017 |
| D-021 | Stage 28 UI is invocation-scoped, binds only exact IPv4 `127.0.0.1`, uses a random 32-byte URL token and an embedded exact GET allowlist, and never writes memory or observability. Accept the rare nonfatal `tiny_http` worker panic seen once under aggressive local valid-request `SO_LINGER` RST stress for rc1: the process stayed alive, the token did not leak, and a normal GET still returned 200. Revisit dependency hardening after RC; do not expand Stage 28 scope. | accepted; independent audit P1=0 P2=0 P3=1 |
| D-022 | Stage 29 UI exposes exactly 11 authenticated GET-only read APIs. Lists use scoped keyset cursors; graph responses cap the page plus fixed center context at 200 unique nodes and 500 edges. `center_node` is fixed context and may duplicate the center on the first page; Memory reports `body_omitted=true` even for an empty page because body is absent from the endpoint schema. | accepted; independent audit P1=0 P2=0 P3=2 semantics-only |
| D-023 | Stage 30 ships six embedded, read-only desktop views with safe text-only DOM insertion, bounded deterministic graph rendering, and real temporary-workspace API screenshot proof. Normal SQLite read-only WAL coordination may touch the exact `-wal`/`-shm` sidecars; proof requires unchanged main DB bytes, schema, rows, size, and mtime. `immutable=1` is rejected because the invocation may coexist with legitimate writers. Browser-returned JPEG bytes are mechanically converted and verified as true PNG. | accepted; cumulative audit P1=0 P2=0 after remediation |
| D-024 | `upgrade plan --all-workspaces --json` is a strict no-write, no-self-observation inspection of only v0.1 AOPMem workspaces. It rejects sidecars, corrupt or unsupported schemas, and insufficient disk space per workspace while returning a stable complete plan. Upgrade writes, backups, and rollback belong only to `upgrade apply`. | accepted; Stage 31 P1=0 P2=0 |
| D-025 | Release label `v0.1.0-rc3` contains binaries that report package version `0.1.0`. Update installers therefore accept only that exact reported semver together with the platform-specific SHA-256 from the peeled tag; the benchmark preserves the reported `0.1.0` label and never relabels it. | accepted; tagged macOS binary executed and both tagged assets hash-bound in Stage 33 |
| D-026 | `upgrade apply` holds a write guard across each SQLite Online Backup and migration, never rolls back a previously committed workspace when a later workspace fails, and leaves durable backups plus an exact resumable report. Unsafe recovery after a concurrent commit fails closed and retains the pending marker. | accepted; Stage 32 P1=0 P2=0 after remediation |
| D-027 | The v0.2 installer verifies and stages the new binary before apply, keeps the old installed binary untouched until apply succeeds, and publishes v0.2 atomically only after successful apply. Once apply begins it never restores v0.1; apply or publish failure retains a verified v0.2 recovery binary and all upgrade backups. | accepted; Stage 33 static/fixture audit 10 groups PASS |
| D-028 | Stage 34 timings cover the exact final source tree, Cargo.lock, and release profile. Its release candidate binary is reproducible but not byte-identical to the explicit-target/minimum-macOS-11 flat asset, so no asset-specific speed claim is made. | accepted; source digest and candidate hash reproduced in Stage 35; final asset has separate real install/update/health proof |
| D-029 | Fresh install completes `init`, then creates the managed adapter block with `adapter seed`, and accepts health only when doctor reports `healthy=true` and verify reports `clean=true`. Update asks no onboarding questions and leaves adapter sync to `upgrade apply`. | accepted; Stage 35 real fresh proof and 11-group installer audit PASS |
| D-030 | Windows RC readiness is based on the native PowerShell 5.1 static contract plus a cargo-xwin PE x64 build with no dynamic MSVC/UCRT imports. Native execution remains Windows dogfood work and is not claimed as a macOS-hosted runtime proof. | accepted; no unsupported cross-host runtime claim |

## Change rule

New product decisions require a real blocker. Implementation choices may change only
when contracts, data preservation, platform support, and proof remain unchanged.


---

## 3. Requirements matrix

Source: `.devplan/V020_REQUIREMENTS_MATRIX.md`

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


---

## 4. Execution ledger

Source: `.devplan/V020_EXECUTION_LEDGER.json`

```json
{
  "goal": "AOPMem v0.2.0-rc1",
  "status": "completed",
  "baseline": "v0.1.0-rc3",
  "authoritative_tree": "7c6bf85e90833fba169c58f0b60a697054310909",
  "auto_patch_window": {
    "max_files": 3,
    "scope": "minimal cross-layer wiring only",
    "requires_proof": true
  },
  "stages": [
    {"id":1,"name":"recovery and classification","status":"completed","agent":"stage_01_recovery","proof":"434 hunks classified; recovery refs protected; jq and git diff --check passed"},
    {"id":2,"name":"materialize classified checkpoint","status":"completed","agent":"stage_02_materialize","proof":"16/16 checkpoint blobs exact; V020 files preserved; fmt and 225 tests passed"},
    {"id":3,"name":"Windows legacy workspace compatibility","status":"completed","agent":"stage_03_impl","proof":"collision-safe current/legacy resolver; 229 tests passed"},
    {"id":4,"name":"draft approval correction","status":"completed","agent":"stage_04_prep","proof":"draft-only gate and draft_review removed; five policy tests plus CLI E2E passed; 228 tests passed"},
    {"id":5,"name":"safe storage optimizations","status":"completed","agent":"stage_05_prep","proof":"implementation plus Stage06 AUTO_PATCH independently re-audited; P1=0 P2=0"},
    {"id":6,"name":"transactional mutation coordinator","status":"completed","agent":"stage_06_impl","proof":"17 production writers coordinated; repo-side init rollback; 253 tests; build/fmt/clippy/diff PASS; independent re-audit PASS"},
    {"id":7,"name":"streaming audit snapshot","status":"completed","agent":"stage_07_impl","proof":"streaming 8KiB writes, lossless TEXT/BLOB, sqlite_sequence, canonical FTS rebuild, deterministic restore; 261 tests and all checks PASS"},
    {"id":8,"name":"cross-platform audit publish and local Git","status":"completed","agent":"stage_08_impl","proof":"atomic Unix/Windows publish; real gix commit; permanent lock; fail-closed workspace/DB/sidecar/audit Git containment; 292 tests and stable cargo-xwin check PASS"},
    {"id":9,"name":"node pagination","status":"completed","agent":"stage_20_prep","proof":"versioned cursor, pre-DB validation, 100/500 keyset, explicit completeness, --all read snapshot, body contract; 274 tests and all checks PASS"},
    {"id":10,"name":"all list pagination and Memory Keeper traversal","status":"completed","agent":"stage_20_prep","proof":"six remaining lists use bound cursors/--all/explicit completeness; Memory Keeper loops pages; 282 tests and all checks PASS"},
    {"id":11,"name":"recall contract model","status":"completed","agent":"stage_11_impl","proof":"typed stable JSON model, canonical byte budgets, UUIDv4 bundle id, strict v0.2 CLI preflight; 300 tests and all checks PASS"},
    {"id":12,"name":"mandatory recall and overflow","status":"completed","agent":"stage_12_impl","proof":"complete active mandatory load, exact O(n) JSON budget, fail-closed overflow ids and no partial success; 308 tests PASS"},
    {"id":13,"name":"query roots FTS direct links","status":"completed","agent":"stage_13_impl","proof":"typed exact roots, full-body BM25, batched one-hop links, 256KiB whole-node packing; migration 003 AUTO_PATCH; 319 tests PASS"},
    {"id":14,"name":"graph expansion ordering deduplication","status":"completed","agent":"stage_14_impl","proof":"bounded depth-2 recursive CTE, cycle guard, expansion reasons, tier/source/trust/confidence order and global dedup; 327 tests PASS"},
    {"id":15,"name":"continuation reasons bundle full recall","status":"completed","agent":"stage_15_impl","proof":"query-bound continuation/full recall plus BM25 starvation and concurrent snapshot remediation; 343 tests; independent re-audit P1=0 P2=0"},
    {"id":16,"name":"tool resource JSON contract","status":"completed","agent":"stage_16_impl","proof":"legacy-safe persisted defaults and overrides, exact 15min/10MiB ceilings, create-draft preflight and parity; 349 tests and all checks PASS"},
    {"id":17,"name":"tool timeout inline runner","status":"completed","agent":"stage_17_impl","proof":"persisted inline limits, concurrent bounded drains, Unix process groups, race-free Windows suspended Job Object assignment, structured timeout/overflow; 361 tests and local gates PASS; final post-fix xwin compile deferred to Stage35 after bounded external CRT download stall"},
    {"id":18,"name":"tool artifact output","status":"completed","agent":"stage_17_impl","proof":"conditional artifact fallback streams byte-exact stdout/stderr with bounded previews, secure atomic publish, 10MiB hard ceiling and cleanup; 373 tests, dev_verify and local gates PASS; final xwin build remains Stage35"},
    {"id":19,"name":"reflection inventory and events","status":"completed","agent":"audit_01_05","proof":"one current inventory projection, exact append-only events, separate proposal/receipt nodes, nested savepoint rollback and durable failed attempts; privacy boundary AUTO_PATCH independently re-audited P1=0 P2=0; 382 tests and all local gates PASS"},
    {"id":20,"name":"artifact retention and report","status":"completed","agent":"stage_24_prep + stage21_prep_final","proof":"exact 7-day/decimal-1GB retention and complete cleanup reports; cumulative remediation anchored artifact/audit/Git/mutation paths, fixed nested teach savepoint and macOS process-tree/executable races; 413 tests, dev_verify and all local gates PASS; independent final P1=0 P2=0"},
    {"id":21,"name":"version and observability schema","status":"completed","agent":"stage21_prep_final","proof":"v0.2.0-rc1 package version and isolated observability.sqlite schema v1; exact 5-table/15-index plus 4-autoindex manifest, strict pre-write validation, true RO reader and private wrappers; 15 focused and 428 full tests, clippy/dev_verify/diff gates PASS"},
    {"id":22,"name":"collector privacy and failure isolation","status":"completed","agent":"stage21_prep_final","proof":"exact 42-event typed privacy-bounded collector, lazy one-warning failure isolation, and 30-day/decimal-100MB physical retention; privacy and workspace-key audit findings remediated; 29 focused tests plus fmt/check/clippy/diff PASS; independent final re-audit P1=0 P2=0 with P3 retained for boundary/concurrency/Windows proof"},
    {"id":23,"name":"core lifecycle instrumentation","status":"completed","agent":"stage21_prep_final","proof":"invocation-scoped collector and typed install/adapter/recall/node/link/remember/teach/reflection lifecycle facts; core locks dropped before writes; fallible UUID RNG; bounded install and pure mutation preflight; audit-before-OBS warnings; privacy/order/duration/failure tests; independent P1=0 P2=0; 459 tests plus fmt/check/clippy/build/dev_verify/diff PASS"},
    {"id":24,"name":"tool health audit instrumentation","status":"completed","agent":"stage24_instrumentation_impl","proof":"typed post-core audit snapshot, tool, artifact, doctor, verify, cleanup, and MCP observability facts; invocation-local tool spawn trace; frozen terminal duration; strict privacy and collector failure isolation; 7 focused regressions plus 466 full tests, fmt/check/clippy/build/diff PASS; independent audit P1=0 P2=0; native Windows runtime proof remains Stage35"},
    {"id":25,"name":"bundle tables global id feedback","status":"completed","agent":"stage25_refresh_prep","proof":"atomic recall parent/nodes/events with first-seen continuation semantics; canonical global UUIDv4 propagation; observability-only atomic feedback with redaction and no-create parent binding; managed Memory Keeper handoff; cumulative remediation closed URI/userinfo/vendor/no-host/truncated privacy, early unreadable-DB bundle linkage, workspace-bound cursor, and stale-before-retrieval ordering findings; 484 tests plus fmt/check/clippy/build/dev_verify/diff gates PASS; two independent clean audits final P1=0 P2=0 P3=0"},
    {"id":26,"name":"observe status and report","status":"completed_after_remediation","agent":"stage26_report_refresh","proof":"strict read-only status and inclusive 30-day fact report from one SQLite snapshot; exact recall facts derive from in-window lifecycle events, terminal more_results from the last completion, and node selection from in-window first_seen_at independent of parent timestamp; AdapterDrift Failure has an explicit failed fact; initial independent audit P1=0 P2=2, both remediated; 23 report plus 2 CLI focused tests and 509 full tests; fmt/check/clippy/build/dev_verify/diff/jq PASS; independent clean re-audit P1=0 P2=0 P3=0"},
    {"id":27,"name":"debug capsule and export","status":"completed","agent":"stage26_report_refresh","proof":"exact deterministic 12-entry read-only ZIP64 capsule; strict operational/observability snapshots, schema/integrity/workspace binding, privacy redaction, same-handle anchored no-clobber publish, and typed post-publish warning; 15 export plus 1 CLI focused proofs, 524 full tests through both suite commands, clippy/dev_verify/diff PASS; independent audit P1=0 P2=0 P3=1 accepted under D-017"},
    {"id":28,"name":"UI server and security","status":"completed","agent":"stage28_ui_prep","proof":"invocation-scoped embedded tiny_http UI with exact IPv4 127.0.0.1 bind, random 32-byte URL token, strict GET asset allowlist, security headers, read-only/no-observability CLI dispatch, deterministic test shutdown, and shell-free macOS/Windows browser launch; 11 UI plus 3 CLI focused tests, 538 full tests through both suite commands, fmt/clippy/build/dev_verify/diff PASS; locked cargo-xwin check exit 0 after approved compile-only Windows identity wiring; independent AUTO_PATCH audit P1=0 P2=0 P3=0; independent frozen UI audit P1=0 P2=0 P3=1 accepted under D-021"},
    {"id":29,"name":"UI read APIs","status":"completed","agent":"stage26_independent_audit","proof":"exact authenticated GET-only 11-route API; strict read-only operational and observability snapshots; scoped stable keyset pagination; body-free memory list and full node; graph 200-node/500-edge ceilings with fixed center context; private activity/bundle/effectiveness/tools/MCP DTOs; all GETs preserve DB bytes, size, mtime, schema, and counts; 13 focused and 546 full tests through both suite commands; fmt/clippy/build/dev_verify/diff PASS; independent audit P1=0 P2=0 P3=2 semantics-only accepted under D-022"},
    {"id":30,"name":"UI frontend screenshots docs","status":"completed_after_remediation","agent":"stage28_ui_prep","proof":"six embedded desktop views consume only frozen read-only APIs; safe text-only DOM and strict same-origin token-relative GETs; bounded pagination and deterministic 200-node/500-edge graph with center dedup; real temporary-workspace browser proof at 1440x900 produced three true RGB PNGs and preserved operational/observability main DB bytes, schemas, rows, sizes, and mtimes; 7 final asset tests, 24 scoped UI tests before the final CSS-only fix, and 561 full tests at the cumulative checkpoint; cumulative Stage26-30 audit initial P1=0 P2=9 groups, all remediated, final P1=0 P2=0"},
    {"id":31,"name":"upgrade plan","status":"completed","agent":"stage26_independent_audit","proof":"strict no-write and no-self-observation all-workspaces plan; stable ordered JSON with binary/schema/migration/disk facts; only v0.1 AOPMem workspaces, immutable read-only inspection, path and sidecar rejection, corrupt/newer schema and insufficient-space blockers; missing home remains absent; 9 module plus 1 CLI focused tests, 561 full tests, fmt/clippy/build/diff PASS; P1=0 P2=0"},
    {"id":32,"name":"upgrade apply backups rollback","status":"completed_after_remediation","agent":"stage26_independent_audit","proof":"upgrade apply uses durable per-run backups, SQLite Online Backup under BEGIN IMMEDIATE guard, one-transaction pending migrations, exact adapter/owned-asset preflight, current-repo adapter sync plus doctor/verify, best-effort update observability, and success-with-warning pending snapshot semantics; unsafe recovery fails closed, prior committed workspaces are never cross-restored, and backups remain resumable; 14 focused apply plus 1 CLI tests, 575 full tests, fmt/clippy/check/build/diff PASS; final P1=0 P2=0"},
    {"id":33,"name":"install prompt and platform install proofs","status":"completed","agent":"stage28_ui_prep","proof":"fresh and update installers for Apple Silicon macOS and native Windows x64 PowerShell 5.1 use prebuilt SHA-bound assets, private temp roots, safe staging/apply/atomic publish, v0.2 recovery retention, no onboarding on update, no admin/WSL/source build/Codex launch; exact v0.1 tag binary semver and both platform hashes bound; isolated 10-group audit, sh -n and diff check PASS; real final-binary macOS fresh/update deferred to Stage35"},
    {"id":34,"name":"corpora and benchmarks","status":"completed","agent":"stage34_benchmarks","proof":"reproducible peeled-tag versus frozen-RC harness generated logical-hash-identical 100/2000/10000-node and 300/6000/30000-link corpora; 3 warmups and 20 samples per supported series, 1060 measured supported invocations, exact pagination/query/tool/verify/observability/UI/export assertions, raw JSON/CSV and medians/p95; unsupported tag operations are explicit and no percentage claim is made; P1=0 P2=0"},
    {"id":35,"name":"assets full gates global audit RC report","status":"completed_after_remediation","agent":"stage35_release_integration","proof":"flat Mach-O arm64/minOS11 and cargo-xwin PE x64/static-CRT assets with verified SHA256SUMS; fresh installer P2 remediated with adapter seed and strict healthy/clean JSON; isolated 11-group installer audit plus real final-binary macOS fresh and peeled-v0.1 update preserve exact logical/tool/artifact data; real observability capsule and live UI proofs; source digest and benchmark candidate reproduced; fmt/clippy/build/575 tests/575 test-only/dev_verify/migration/export/UI/forbidden/diff gates PASS; independent cache-drift P3 removed; final independent OPEN P1=0 P2=0 P3=0; READY for macOS and Windows dogfood"}
  ],
  "cumulative_audits_after": [5,10,15,20,25,30,35],
  "cumulative_audits": [
    {
      "after_stage": 5,
      "status": "passed_after_remediation",
      "agent": "audit_01_05",
      "findings": {
        "p1": 1,
        "p2": 2
      },
      "proof": "initial P1/P2 repros fixed; independent runtime re-audit killed three approval mutants, checked seven invalid commands, tools-root escape, marker ownership, draft/init rollback; final P1=0 P2=0"
    },
    {
      "after_stage": 10,
      "status": "passed_after_remediation",
      "agent": "audit_06_10_final",
      "findings": {
        "p1": 1,
        "p2": 0
      },
      "proof": "initial systemic persistent-path escape reproduced for workspace/DB/audit paths; remediation added fail-closed link/reparse and SQLite sidecar guards; independent rebuilt-binary audit, 292 tests, seven list E2E, LocalGitAudit proofs and Windows xwin check passed; final P1=0 P2=0"
    },
    {
      "after_stage": 15,
      "status": "passed_after_remediation",
      "agent": "audit_11_15",
      "findings": {
        "p1": 0,
        "p2": 1
      },
      "proof": "initial BM25 continuation-order P2 fixed in Rust and SQL with starvation and WAL snapshot regressions; 343 tests and independent focused re-audit passed; final P1=0 P2=0"
    },
    {
      "after_stage": 20,
      "status": "passed_after_remediation",
      "agent": "audit_16_20",
      "findings": {
        "p1": 3,
        "p2": 3
      },
      "proof": "initial tool descendant/executable, artifact cleanup, audit snapshot/Git, nested teach, and cleanup-report findings remediated; component-wise and handle-relative containment, workspace identity handoff, composite mutation/snapshot locks, strict Git preflight, bounded macOS EPERM handling and negative swaps proved; 413 tests plus dev_verify passed; final P1=0 P2=0"
    },
    {
      "after_stage": 25,
      "status": "passed_after_remediation",
      "agent": "stage25_refresh_prep + two independent re-audits",
      "findings": {
        "p1": 1,
        "p2": 3
      },
      "proof": "initial P1 privacy finding covered URI userinfo and bounded vendor tokens; initial P2 findings covered early operational-DB failure linkage, copied-DB cross-workspace cursor binding, and the additional stale-cursor versus mandatory-overflow ordering mutant; remediation added deterministic URI/vendor/no-host/truncated canaries, atomic correlated read-open failures, domain-bound revision hashes, and stale-first retrieval ordering; 484 tests plus all local gates passed; two independent clean audits final P1=0 P2=0 P3=0"
    },
    {
      "after_stage": 26,
      "status": "passed_after_remediation",
      "agent": "stage26_report_refresh + stage26_independent_audit",
      "findings": {
        "p1": 0,
        "p2": 2
      },
      "proof": "initial P2 findings were parent-row/lifetime recall period semantics and omitted AdapterDrift Failure facts; remediation counts lifecycle events by their own timestamps, derives terminal more_results from stable completion order, admits current first-seen nodes for older parents, and exposes adapter failed; 23 report tests, 2 CLI tests, 509 full tests and all local gates pass; independent clean re-audit final P1=0 P2=0 P3=0"
    },
    {
      "after_stage": 30,
      "status": "passed_after_remediation",
      "agent": "stage29_live_review",
      "findings": {
        "p1": 0,
        "p2": 9
      },
      "proof": "two Stage26 report groups and seven Stage30 frontend/proof groups were remediated; final UI reports complete effectiveness/continuation facts and correct partial/error state, uses concise accessibility status, and provides visually checked true 1440x900 PNGs from real temporary APIs; main operational and observability DB bytes, schemas, rows, sizes, and mtimes remained unchanged; 561 full tests at the stable checkpoint plus final 7 asset tests; independent final audit P1=0 P2=0 with four accepted P3 items under D-017, D-021, and D-022"
    },
    {
      "after_stage": 35,
      "status": "passed_after_remediation",
      "agent": "stage35_global_audit",
      "findings": {
        "p1": 0,
        "p2": 0,
        "p3": 1
      },
      "proof": "independent audit rechecked exact source digest, fmt, clippy, 575 tests, 11 installer groups, asset manifest/types/imports, dependency justification, forbidden drift, jq, diff, and every required path; one generated Python cache P3 was deleted and repeat scan passed; final OPEN P1=0 P2=0 P3=0 and READY for macOS/Windows dogfood"
    }
  ]
}
```


---

## 5. Proof log

Source: `.devplan/V020_PROOF_LOG.md`

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


---

## 6. Stages 26–30 cumulative audit

Source: `.devplan/AUDITS/STAGE_26_30_CUMULATIVE_AUDIT.md`

# Stages 26–30 cumulative audit

Date: 2026-07-15

Scope:

- Stage 26: observe status and effectiveness report;
- Stage 27: deterministic debug capsule export;
- Stage 28: loopback/token UI server;
- Stage 29: bounded read-only UI APIs;
- Stage 30: embedded desktop frontend, docs, and screenshot proof.

## Verdict

| Priority | Initial groups | Fixed | Remaining |
|---|---:|---:|---:|
| P1 | 0 | 0 | 0 |
| P2 | 9 | 9 | 0 |
| P3 | 4 | 0 | 4 accepted |

The nine P2 groups comprise two Stage 26 report defects and seven Stage 30
frontend/proof groups. If the two missing recall metrics in Stage 30 are
counted separately, the raw defect count is ten. Grouping does not change the
final verdict.

## Remediation proof

- Stage 26 now derives recall facts from in-period lifecycle timestamps,
  reports terminal continuation state, and exposes failed adapter drift.
- Stage 30 reports the full effectiveness retention reason, both continuation
  facts, correct partial/error Tools/MCP state, and a concise live region.
- The table header ordering defect was removed.
- All three screenshot files were converted from browser-returned JPEG bytes
  to real RGB PNG files. Each is exactly `1440x900`.
- The frontend uses text-only DOM insertion, strict same-origin token-relative
  GET requests, no external assets, no write route, and no tool execution.
- Graph rendering deduplicates the fixed center context and remains bounded by
  200 unique nodes and 500 edges.
- Repeated browser captures were byte-identical for Overview, Graph, and
  Activity.
- Before/after browser proof preserved the operational and observability main
  database bytes, sizes, mtimes, schemas, and row counts. Normal SQLite
  read-only WAL coordination may create or touch the exact `-wal`/`-shm`
  sidecars; this is documented and is not an operational-memory mutation.

## Checks

- `node --check` / JavaScriptCore syntax check: PASS.
- final embedded asset tests: 7/7 PASS.
- Stage 30 scoped UI tests: 24/24 PASS before the final CSS-only fix.
- cumulative full suite checkpoint: 561/561 PASS.
- `cargo fmt --check`: PASS at the stable checkpoint.
- `cargo clippy --all-targets -- -D warnings`: PASS at the stable checkpoint.
- `cargo build`: PASS at the stable checkpoint.
- `scripts/dev_verify.sh`: PASS at the stable checkpoint.
- `git diff --check`: PASS.
- independent static re-audit: P1=0, P2=0.

## Screenshot hashes

- Overview:
  `8272778ccea477fded9586fa2498422b35684b6e6f7ac2b94f544ad79f4394bf`
- Graph:
  `4e82b570306d7b627948ed87c6e43085a2827493605cf59d91c5aa08535d76b3`
- Activity:
  `b3b465c72cba7abe9f4a5bed12e42d579bba3d7af9b5daaace4c607621d2a0ea`

## Accepted P3 items

- D-017: same-UID local export leaf-name race boundary.
- D-021: rare nonfatal `tiny_http` valid-request RST worker panic.
- D-022: empty Memory body-omission flag and duplicated graph center API
  semantics. The frontend deduplicates the latter.

Final cumulative verdict: **P1=0, P2=0**.


---

## 7. Benchmark report

Source: `.devplan/V020_BENCHMARK_REPORT.md`

# AOPMem v0.2.0-rc1 Benchmark Report

Verdict: `PASS`

The benchmark is reproducible and complete for the Stage 34 scope.
No P1 or P2 correctness issue was found. No percentage speed claim is made.

## Provenance

| Item | Value |
|---|---|
| Baseline source | peeled tag `v0.1.0-rc3` |
| Baseline commit | `9877d39a4bc44cf62140aace8755720044c1d41f` |
| Baseline package version | `0.1.0` |
| Baseline binary SHA-256 | `dbc67aa27324310ac35d028cfef1c73e2dfd6308ed4ae73d1314e014a5f5e6d2` |
| Current source | frozen `v0.2.0-rc1` worktree |
| Current package version | `0.2.0-rc1` |
| Current source-tree SHA-256 | `91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4` |
| Current binary SHA-256 | `12ec578dc641373e0e22b67f548fb2862620571eb9777026304cd46e10427e61` |
| Build profile | Cargo `release --locked` |
| Host | macOS `26.5.1`, Apple Silicon `arm64` |
| Rust | `rustc 1.95.0` |
| Python harness | Python `3.14.5`, standard library only |
| End-to-end run | `168.615 s` |

The `v0.1.0-rc3` tag payload identifies itself as package version `0.1.0`.
Raw data keeps that package version. It is not relabeled as `0.1.0-rc3`.

The current build came from the intentionally dirty, classified worktree.
The binary hash and compiled source-tree hash make that input explicit.

## Method

- One isolated tag archive built the baseline. No checkout or reset was used.
- The current frozen worktree built into a separate temporary target directory.
- Every workspace and `AOPMEM_HOME` lived in a disposable temporary root.
- No real user workspace was read or changed.
- Each supported series used 3 warmups and 20 measured samples.
- Release builds took 22.47 s for the tag and 40.17 s for the current tree.
- Timings use `time.perf_counter_ns` and include process startup and JSON output.
- Results report median and nearest-rank p95 in milliseconds.
- Unsupported tag commands were recorded, not emulated.
- Mutation timing used a fresh home clone for every sample.
- Full pagination used explicit 500-node keyset pages with full bodies.
- UI timing covers process start through the first authenticated loopback
  `GET api/v1/overview` response.

The harness loads synthetic rows directly only inside its disposable fixture.
It then performs one real `link add` mutation so the product republishes the
canonical SQL snapshot. `verify` must be clean before measurement begins.

## Corpora

| Corpus | Nodes | Links | Aliases | Tags | Sources | Tools | Obs. events | Logical SHA-256 |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| small | 100 | 300 | 100 | 100 | 100 | 5 | 100 | `6df66b15ebf02bce6eb82ea5bd8664ada961772aa1debc2d6c0ed8011a087f8b` |
| medium | 2,000 | 6,000 | 2,000 | 2,000 | 2,000 | 25 | 2,000 | `77f8cba1a2389a9cd14a75ab6966f371db1f543910d6778514f2fce159e128c7` |
| large | 10,000 | 30,000 | 10,000 | 10,000 | 10,000 | 100 | 10,000 | `6f3b08fec3ac07f9b6dcef7159a4ff9e4f3b97e61977fddab8e91b60be6af9ff` |

All corpora also contain workflows, tool-contract nodes, failure modes,
corrections, lessons, project facts, preferences, skills, incident scars,
decisions, two MCP profiles, and operational events.

The logical hash is equal between the tag fixture and current fixture for
every corpus. Local Observability is absent from the tag by design.

## Results

All values below are milliseconds. `unsupported` means the tag has no
equivalent contract. It does not mean zero time.

Init is corpus-independent:

| Version | Median | p95 |
|---|---:|---:|
| tag payload `0.1.0` | 13.191 | 15.598 |
| current `0.2.0-rc1` | 110.676 | 117.562 |

### Small corpus

| Metric | Tag median | Tag p95 | RC1 median | RC1 p95 |
|---|---:|---:|---:|---:|
| node list first page | unsupported | unsupported | 4.753 | 4.985 |
| node list full pagination | 4.584 | 5.485 | 5.310 | 5.810 |
| recall baseline | 4.220 | 4.399 | 7.235 | 7.650 |
| recall query | unsupported | unsupported | 8.372 | 11.548 |
| tool list | 4.011 | 4.152 | 4.293 | 4.474 |
| doctor | 4.259 | 4.468 | 6.762 | 7.390 |
| verify | 4.170 | 4.363 | 7.067 | 7.583 |
| audit snapshot mutation | 7.186 | 7.479 | 73.300 | 76.557 |
| observability wall | unsupported | unsupported | 6.837 | 7.441 |
| UI initial overview API | unsupported | unsupported | 6.026 | 17.707 |
| export capsule | unsupported | unsupported | 37.612 | 38.676 |

### Medium corpus

| Metric | Tag median | Tag p95 | RC1 median | RC1 p95 |
|---|---:|---:|---:|---:|
| node list first page | unsupported | unsupported | 4.757 | 4.864 |
| node list full pagination | 13.377 | 13.670 | 28.811 | 29.977 |
| recall baseline | 9.387 | 9.933 | 10.662 | 11.751 |
| recall query | unsupported | unsupported | 20.298 | 20.774 |
| tool list | 4.349 | 4.668 | 4.559 | 4.792 |
| doctor | 4.295 | 4.473 | 9.466 | 9.816 |
| verify | 9.853 | 10.003 | 13.531 | 13.740 |
| audit snapshot mutation | 43.827 | 49.333 | 99.618 | 103.253 |
| observability wall | unsupported | unsupported | 9.543 | 9.893 |
| UI initial overview API | unsupported | unsupported | 8.223 | 9.588 |
| export capsule | unsupported | unsupported | 459.171 | 467.815 |

### Large corpus

| Metric | Tag median | Tag p95 | RC1 median | RC1 p95 |
|---|---:|---:|---:|---:|
| node list first page | unsupported | unsupported | 4.841 | 5.235 |
| node list full pagination | 47.275 | 48.723 | 146.786 | 153.772 |
| recall baseline | 32.898 | 33.792 | 24.298 | 25.942 |
| recall query | unsupported | unsupported | 70.721 | 71.303 |
| tool list | 4.724 | 4.969 | 5.120 | 5.316 |
| doctor | 4.581 | 5.016 | 21.648 | 24.860 |
| verify | 34.811 | 35.078 | 43.036 | 45.084 |
| audit snapshot mutation | 196.137 | 202.644 | 206.193 | 218.505 |
| observability wall | unsupported | unsupported | 25.444 | 28.395 |
| UI initial overview API | unsupported | unsupported | 17.859 | 19.534 |
| export capsule | unsupported | unsupported | 2,268.556 | 2,373.789 |

## Result Interpretation

- The current first page stays near 5 ms p95 on all three corpora.
- Full current traversal uses 1, 4, and 20 CLI page invocations. The tag uses
  one unbounded invocation. The totals are therefore product-flow timings,
  not a claim about one equivalent SQLite primitive.
- Bare recall contracts differ. The large absolute result is 32.898 ms for
  the tag and 24.298 ms for RC1, but no speedup claim is made.
- Task query is new. Its medians are 8.372, 20.298, and 70.721 ms.
- Large snapshot mutation is 196.137 ms on the tag and 206.193 ms on RC1.
- RC1 has larger fixed cost for small snapshot mutations and init. The
  measured absolute values are retained above without hiding that cost.
- Large debug capsule export has a 2,373.789 ms p95.
- Large initial UI overview response has a 19.534 ms p95.

No correctness behavior was weakened to improve these timings.

## Local Observability Measurement

The collector has no supported disable switch. A pure on/off collector timer
would require changing product behavior, so the harness does not invent one.

For each measured `doctor` invocation, the collector's terminal event stores
the core command duration before collector I/O. The residual below is wall
time minus that stored integer duration.

| Corpus | Wall median | Wall p95 | Residual median | Residual p95 |
|---|---:|---:|---:|---:|
| small | 6.837 | 7.441 | 5.837 | 6.441 |
| medium | 9.543 | 9.893 | 8.543 | 8.893 |
| large | 25.444 | 28.395 | 24.444 | 27.395 |

This residual is an upper bound. It also contains process startup, JSON
serialization, and output. It must not be described as pure collector cost.
Every sample added exactly one valid local observability event.

## Correctness Proof

| Check | Result |
|---|---|
| Result series | 68 total: 53 supported, 15 unsupported |
| Measured supported samples | 1,060 |
| Sampling contract | every supported series has 3 warmups and 20 samples |
| Corpus parity | all three tag/current logical hashes match |
| Full traversal | exactly 100, 2,000, and 10,000 nodes every sample |
| Current page counts | exactly 1, 4, and 20 pages |
| First-page contract | correct count, `more_results`, and omitted bodies |
| Query recall | exact `Deploy release workflow` selected every sample |
| Tool list | exact 5, 25, and 100 tools every sample |
| Verify | clean before measurement and clean in all samples |
| Observability | one valid event added per measured collector sample |
| UI | all 60 measured responses are HTTP 200 on `127.0.0.1` |
| Export | all 60 ZIPs report durable publication and are non-empty |
| Evidence integrity | every entry in `SHA256SUMS` passes |
| P1/P2 | 0 / 0 |

## Exact Unsupported Tag Operations

- Node first page: tag node list is unbounded and has no page-size or cursor
  contract.
- Recall query: tag recall has no `--query` task-retrieval contract.
- Local Observability: tag has no collector or observability store.
- Desktop UI: tag has no `aopmem ui` command or local HTTP API.
- Debug capsule: tag has no `aopmem observe export` command.

These reasons are stored on every applicable raw result row.

## Evidence

- Runner: `scripts/benchmark_v020.sh`
- Harness: `scripts/benchmark_v020.py`
- Provenance: `.devplan/benchmarks/v020_rc1/run.json`
- Corpus manifests: `.devplan/benchmarks/v020_rc1/corpora/`
- Raw JSON: `.devplan/benchmarks/v020_rc1/raw/samples.json`
- Raw CSV: `.devplan/benchmarks/v020_rc1/raw/samples.csv`
- Summary CSV: `.devplan/benchmarks/v020_rc1/summary.csv`
- Integrity manifest: `.devplan/benchmarks/v020_rc1/SHA256SUMS`

Key evidence hashes:

| File | SHA-256 |
|---|---|
| `scripts/benchmark_v020.py` | `ed84b9294cb10f9fe8736bbe8c3890813bd911eb80499c325a0112d1a2ae3c44` |
| `scripts/benchmark_v020.sh` | `fca995149ead3c30a57aacddf097d5f8a7b104769f955acfe2897a966f21dcee` |
| `raw/samples.json` | `f2cb583eedc3f671636f53888f22cd887edea0c8dbb55a58050ac6b77c92fd26` |
| `raw/samples.csv` | `cfa45e357de023de0828e260620e9d6030fa83ba21df9a3e7c44abfeb255463c` |
| `summary.csv` | `610eb5eec34ced7a6e9b17cadc724c5eb366132862e59d8093b8f8a8cbeab9ab` |
| `SHA256SUMS` | `835fa55302a0f1f1ccccc12a28b9098a5314f4558d80410c7ee2723d606c52bf` |

Reproduction command:

```sh
scripts/benchmark_v020.sh
```

## Limits

- This is one Apple Silicon macOS host, not a cross-host benchmark lab.
- Windows native runtime performance was not measured on macOS.
- Synthetic data is deterministic but is not a production usage trace.
- Measurements are process-level wall time and include filesystem cache state.
- Full pagination intentionally measures repeated CLI invocations.
- New RC1 commands have no tag comparison and remain marked unsupported.
- The release scope defines no numeric latency threshold.

## Stage Decision

Stage 34 passes. The report contains real measured data, exact unsupported
markers, corpus parity proof, raw samples, and no invented score or percentage.
It does not by itself decide the full release-candidate verdict.

## Stage 35 release-input parity

The final release integration recomputed the harness source-tree digest from
`Cargo.toml`, `Cargo.lock`, `src/`, and `templates/`. It remains exactly
`91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4`,
the digest measured above.

Repeating the benchmark build command, `cargo build --release --locked`,
reproduced the measured candidate binary exactly at
`12ec578dc641373e0e22b67f548fb2862620571eb9777026304cd46e10427e61`.
The flat macOS release asset uses the same locked source and release profile,
plus explicit `--target aarch64-apple-darwin`, minimum macOS 11, and
`strip=false`; its SHA-256 is
`b32e918d2a44f0767444e09c84c1ed44fe9177709b2d56b2aa89c300081d4308`.

Therefore the benchmark covers the exact final production source and locked
dependencies, but its timing binary is not byte-identical to the flat
platform asset. Final-asset behavior has separate real fresh-install,
v0.1-update, doctor, verify, UI, and export proofs. No asset-specific speed
claim is made.


---

## 8. Global audit report

Source: `.devplan/V020_GLOBAL_AUDIT_REPORT.md`

# AOPMem v0.2.0-rc1 Global Audit Report

Date: 2026-07-15

Status: complete. Ready for macOS and Windows dogfood.

## Scope

This audit covers the classified mixed worktree, all 35 finite stages, the
target product contracts, final platform assets, the real isolated macOS
fresh/update flows, required negative tests, and the stop conditions.

No reset, checkout, push, tag, GitHub Release, real user-workspace install, or
backup deletion was performed.

## Implementation audit verdict

| Severity | Open findings |
|---|---:|
| P1 | 0 |
| P2 | 0 |
| P3 | 0 open |

The independent final audit confirms the implementation verdict.

## Definition-of-done audit

| Contract | Evidence | Result |
|---|---|---|
| Worktree classification | 434 changed hunks classified; mixed files reviewed; recovery ref retained | PASS |
| Safe optimization package | listed SQL, snapshot, transaction, validation, runner, and recall optimizations retained | PASS |
| Draft approval conflict | draft-only `+++`, `draft_review`, and managed-block sentence removed; five policy tests | PASS |
| Pagination | 100 default, 500 maximum, scoped keyset cursors, `--all`, explicit completeness | PASS |
| Recall | complete mandatory context, overflow ids, task query, graph/direct expansion, continuation dedup, reasons | PASS |
| Tool resources | per-tool limits, global ceilings, artifact streaming, inline errors, dry-run and approval policy | PASS |
| Reflection | one current inventory, append-only closed event set, separate proposals/receipts | PASS |
| Artifacts | 7 days or 1 GB, current-day oldest deletion, exact protected roots and cleanup reports | PASS |
| Audit snapshot | streaming SQL, atomic publish, real local Git, pending marker and duration observation | PASS |
| Local Observability | separate schema-v1 DB, 42 typed events, failure isolation, 30 days or 100 MB | PASS |
| Correlation/feedback | UUID v4 bundle id, continuation binding, observability-only feedback | PASS |
| Effectiveness | verifiable fact report, bounded top lists, no product score | PASS |
| Debug capsule | exact 12 entries, deterministic redaction, no DB/bodies/raw output/secrets | PASS |
| Desktop UI | loopback/token/GET-only, 11 APIs, bounded graph, six views, screenshots | PASS |
| Upgrade plan/apply | strict read-only plan; durable backups, guarded Online Backup, migrations, safe stop/recovery | PASS |
| Fresh/update installers | managed fresh adapter and healthy checks; update zero onboarding and safe publish order | PASS |
| Version/assets | v0.2.0-rc1; flat Mach-O arm64 and PE x64; exact SHA256SUMS | PASS |
| Benchmark | 100/2,000/10,000 nodes; 300/6,000/30,000 links; 3 warmups, 20 samples; raw data | PASS |
| Required local gates | fmt, clippy, build, 575 tests twice, dev_verify, diff, fixtures, installers, UI/export | PASS |

## Final gate evidence

| Proof | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo build --locked` | PASS |
| `cargo test --locked` | 575/575 PASS |
| `cargo test --tests --locked` | 575/575 PASS |
| `scripts/dev_verify.sh` | PASS, including its 575-test run |
| v0.1 schema/full upgrade fixtures | 1/1 + 1/1 PASS |
| installer audit | 11/11 groups PASS |
| real final macOS fresh | adapter in-sync, doctor healthy, verify clean |
| real peeled v0.1 macOS update | exact logical/tool/artifact preservation, migrations 001/002/003 |
| Windows PowerShell 5.1 | static contract PASS; no native execution claim on macOS |
| Windows binary | PE32+ x86-64; no dynamic MSVC/UCRT import |
| observability capsule | real 12-entry durable export and deterministic redaction PASS |
| UI | 13 HTTP tests, live token/method/bounds proof, three 1440x900 PNGs |
| forbidden drift | no extra platform/CI/Node/runtime dependency or Python cache |
| `git diff --check` | PASS |

## Data-preservation evidence

The isolated peeled-v0.1 fixture contains 11 nodes, one link, one alias, one
tag, one source, 12 events, one tool contract/tree, and three MCP profiles.
The exact selected v0.1-column digest before migration, after migration, and
inside SQLite Online Backup is:

`4890a73e51a5e0eeb0e283f3127cd5c05e583f13f518d7aefde95180c1ef7c9f`

Tool files retain digest
`5d7ffa2a4357d3072b406f154d17e479a4d8a6d227f37df9c678d97a0ad2babb`.
The artifact retains digest
`b7dfde292eca151e17b48bfa58f7fb397f7789614331d79e4239578aa6d75bad`.
Installer binary, database, adapter, and owned-asset backups remain present.

## Platform assets

| Asset | Type/proof | SHA-256 |
|---|---|---|
| `aopmem-darwin-arm64` | Mach-O arm64, min macOS 11.0, not stripped | `b32e918d2a44f0767444e09c84c1ed44fe9177709b2d56b2aa89c300081d4308` |
| `aopmem-windows-x86_64.exe` | PE32+ console x86-64, static CRT import scan | `a4e3302d6f26dd9d16387a075189fec51c469aef9b8d9c730f81001b21b2cf57` |

Native Windows execution cannot be performed on the macOS build host. The RC
is intended for that Windows dogfood validation; this report does not convert
static/PE evidence into a runtime claim.

## Benchmark provenance

The final source-tree digest remains the exact Stage 34 value
`91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4`.
The measured release build reproduces byte-for-byte at
`12ec578dc641373e0e22b67f548fb2862620571eb9777026304cd46e10427e61`.
The explicit-target flat macOS asset has a different binary hash, so no
asset-specific speed claim is made.

## Accepted residuals

- D-017: active same-UID tampering outside the no-sandbox local boundary.
- D-021: one rare nonfatal tiny_http worker panic under aggressive local RST
  stress; server stayed alive and token remained private.
- D-022: two documented bounded-API representation semantics.
- Native Windows behavior is a dogfood target, not a completed macOS-hosted
  runtime proof.
- Stage 34 measures the exact production source/lock candidate, not the
  byte-identical explicit-target flat asset.

None of these documented boundaries is an open release finding. They do not
change stored user data or expand product scope.

## Independent final audit

Final verdict: **READY for macOS and Windows dogfood**.

| Severity | Open findings |
|---|---:|
| P1 | 0 |
| P2 | 0 |
| P3 | 0 |

The independent read-only audit rechecked the exact source digest, fmt,
clippy, 575 tests, 11 installer groups, manifest and platform types, Windows
imports, dependency coverage, forbidden scope, required paths, JSON ledger,
and final diff. It found one generated Python-cache P3 drift. Stage 35 deleted
that cache and the repeat scan passed.

Production/input freeze marker:

`source_tree_sha256=91976686ab74fa5b85b4d1c43419268ca3e508d606e1cd1da65f2b309ca7abc4`


---

## 9. Release candidate report

Source: `.devplan/RELEASE_CANDIDATE_v0.2.0-rc1.md`

# AOPMem v0.2.0-rc1 Release Candidate

Status: ready for macOS and Windows dogfood.

## Candidate outcome

The implementation is ready for macOS and Windows dogfood. The independent
final audit reports open P1=0, P2=0, and P3=0. This is not a public release.

## Preserved optimization work

- streaming SQL dump and canonical FTS rebuild;
- atomic snapshot publication, real LocalGitAudit commits, and pending marker;
- read-only DB opens, pending-only migrations, summary/direct metadata indexes;
- transactional teach/reflect, batched FTS, and atomic draft tool creation;
- tool path containment and concurrent stdout/stderr drains;
- lean lint projection, direct tool/rule recall, boxed large CLI variant;
- dry-run without execution and validation before DB access.

## Removed conflicting optimization behavior

- removed the rule that every draft tool requires `+++`;
- removed `draft_review`;
- removed `Draft tool execution requires +++.` from the managed block;
- restored approval by actual contract, side effect, and explicit risk.

## Added in v0.2.0-rc1

- explicit complete keyset pagination and controlled `--all` traversal;
- mandatory-safe query recall with continuation, selection reasons, and
  debug-only `--full`;
- per-tool timeout/output/artifact contracts and global ceilings;
- one reflection inventory projection with append-only event history;
- separate local observability, bundle correlation, feedback, and fact report;
- deterministic redacted 12-entry debug capsule;
- embedded loopback/token/read-only six-view desktop UI;
- read-only upgrade planning, guarded backups/migrations, safe resumable apply;
- native prebuilt-binary fresh/update installers for the two supported targets.

## Final proof summary

| Item | Result |
|---|---|
| Rust tests | 575/575, twice |
| `dev_verify` | PASS, including another 575-test run |
| Installer audit | 11/11 groups |
| Real macOS fresh | adapter in-sync; doctor healthy; verify clean |
| Real v0.1 update | exact logical/tool/artifact preservation; 001/002/003 |
| Observability | real status/report/export; 12-entry redacted capsule |
| UI | live 200/invalid-token 404/POST 405; 13 HTTP tests; 3 screenshots |
| Benchmark | 1,060 measured samples; raw JSON/CSV; no percentage claim |
| Independent findings | open P1=0, P2=0, P3=0 |

## Binaries

| Platform | Asset | SHA-256 |
|---|---|---|
| macOS Apple Silicon | `dist/aopmem-darwin-arm64` | `b32e918d2a44f0767444e09c84c1ed44fe9177709b2d56b2aa89c300081d4308` |
| Windows 11 x64 | `dist/aopmem-windows-x86_64.exe` | `a4e3302d6f26dd9d16387a075189fec51c469aef9b8d9c730f81001b21b2cf57` |

The macOS asset is Mach-O arm64, minimum macOS 11.0, and not stripped. The
Windows asset is PE32+ x86-64 and has no dynamic MSVC/UCRT import. Native
Windows execution remains the first Windows dogfood task; it was not possible
on the macOS build host.

## Migration status

The exact peeled-tag v0.1 fixture updates to schema migrations `001,002,003`.
Nodes, links, aliases, tags, sources, events, registries, tool contracts, MCP
profiles, generated tool bytes, artifacts, and adapter backup are preserved.
Update asks no onboarding questions. All backups remain present.

## UI proof

The final binary served embedded assets and bounded read APIs only on a random
`127.0.0.1` port with a random token. Invalid token returned 404 and POST
returned 405. Overview, Graph, and Activity proofs are true 1440x900 PNGs.

## Observability proof

The migrated fixture has a separate schema-v1 observability database with
`update.started` and `update.completed`. Status/report/export work without
changing operational memory. The real capsule contains exactly 12 redacted
entries and no raw task, node body, tool output, token, or environment dump.

## Remaining validation boundary

- Independent final audit has no open P1, P2, or P3.
- Native Windows execution is intentionally deferred to Windows dogfood.
- Accepted P3 boundaries remain documented in the global audit and decisions
  D-017, D-021, D-022, D-028, and D-030.

## Stop condition

Do not push, tag, create a GitHub Release, install into a real user workspace,
or delete backups. The independent audit and ledger are complete; stop here.


---

## External evidence index

- Benchmark raw data: `.devplan/benchmarks/v020_rc1/`
- UI screenshots: `.devplan/AUDITS/ui-*-1440x900.png`
- Release binaries and checksums: `dist/`
- Product documentation: `docs/`
- Install prompt and native installers: `install/v0.2/`

