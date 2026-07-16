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

