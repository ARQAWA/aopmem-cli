use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const BEGIN_MARKER: &str = "<!-- AOPMEM:BEGIN managed block -->";
pub const END_MARKER: &str = "<!-- AOPMEM:END managed block -->";

const MANAGED_BLOCK_BODY: &str = "\
This block is managed by AOPMem.\n\
AOPMem is installed.\n\
Main work starts with Memory Keeper / recall.\n\
Normal work MUST use `aopmem recall --query \"<current task>\"` before non-trivial work.\n\
The first task recall creates `bundle_id`; do not pass global `--bundle-id` to a first, bare, or `--full` recall.\n\
Memory Keeper follows `continuation_cursor` with the same query and exact `--bundle-id <bundle_id>` until `more_results=false` or `budget.task.exhausted=true`.\n\
Memory Keeper passes global `--bundle-id <bundle_id>` to later AOPMem operations for the same work.\n\
`more_results=true` with a null cursor is a recall contract error.\n\
Never use `aopmem recall --full` in normal task flow; it is debug/audit/export/migration only.\n\
Do not edit AOPMem SQLite directly.\n\
Use `aopmem tool run <tool-id>` for generated tools.\n\
Generated tool runtime limits and output mode come from its validated `tool.json`.\n\
Tool processes use the tool root as cwd; resolve resources through validated `runtime.runtime_dir` relative to that root.\n\
For shebang tools, `$0` and the concrete entrypoint launch path are implementation details; do not use them for resource discovery.\n\
Defaults are 30000 ms and 65536 bytes per stream; hard ceilings are 900000 ms and 10485760 bytes per stream.\n\
`output_mode=inline` returns `TOOL_OUTPUT_OVERFLOW` and writes no artifact when a stream exceeds its limit.\n\
`output_mode=artifact` keeps bounded previews and publishes full output only under `artifacts/YYYY-MM-DD/`.\n\
Artifact capture above 10485760 bytes per stream returns `TOOL_OUTPUT_OVERFLOW` and publishes nothing.\n\
`--dry-run` executes nothing and creates no artifact.\n\
Approval is required when `approval_requirement != none`, for `external_write` or `destructive`, and for explicit high-risk policy.\n\
No approval is required for `none`, `local_read`, contract-safe `local_write_artifact`, or `external_read` with `approval_requirement=none`.\n\
Do not store secrets.\n\
Use `remember`, `teach`, `reflect` only by user trigger.\n\
Feedback is user-triggered or agent post-task: `aopmem --bundle-id <bundle_id> feedback record --outcome useful|partial|wrong [--reason \"<short reason>\"]`.\n\
Feedback stays only in Local Observability; never put the full task, raw chat, raw output, secrets, or hidden reasoning in its reason.\n\
Reflection keeps one current inventory node and append-only operational events; an identical inventory is a no-op.\n\
Reflection inventory, apply receipts, and events never copy node bodies, hidden reasoning, raw complete chat, raw tool output, environment data, credentials, or secrets.\n\
Proposal payloads and applied nodes contain only explicit user-selected structured memory; never put secrets or raw captures into a proposal.\n\
Memory stored under user-level AOPMem workspace, not repo.\n\
Do not create `.aopmem` in repo.\n\
Deprecated memory excluded from normal recall.\n\
Memory Keeper follows list `next_cursor` pages until `more_results=false` whenever a full set is needed.\n\
Artifacts cleanup policy: 7 days OR 1 GB per workspace.\n\
Do not edit inside this block manually.\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedOutcome {
    pub instruction_file: PathBuf,
    pub file_created: bool,
    pub block_updated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub instruction_file: PathBuf,
    pub file_created: bool,
    pub block_present: bool,
    pub block_inserted: bool,
    pub block_updated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedInstructionSync {
    pub bytes: Vec<u8>,
    pub file_created: bool,
    pub block_present: bool,
    pub block_inserted: bool,
    pub block_updated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedBlockStatus {
    Missing,
    InSync,
    Drifted,
}

impl ManagedBlockStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::InSync => "in_sync",
            Self::Drifted => "drifted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusOutcome {
    pub instruction_file: PathBuf,
    pub file_exists: bool,
    pub managed_block: ManagedBlockStatus,
}

#[derive(Debug)]
pub enum SeedError {
    Io(io::Error),
    DamagedManagedBlock,
}

impl fmt::Display for SeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::DamagedManagedBlock => {
                write!(formatter, "managed block markers are damaged or duplicated")
            }
        }
    }
}

impl std::error::Error for SeedError {}

impl From<io::Error> for SeedError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn default_instruction_file(repo_root: &Path) -> PathBuf {
    repo_root.join("AGENTS.md")
}

pub fn seed_instruction_file(path: &Path) -> Result<SeedOutcome, SeedError> {
    let outcome = sync_instruction_file(path)?;

    Ok(SeedOutcome {
        instruction_file: outcome.instruction_file,
        file_created: outcome.file_created,
        block_updated: outcome.block_updated,
    })
}

pub fn sync_instruction_file(path: &Path) -> Result<SyncOutcome, SeedError> {
    let existing = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let prepared = prepare_instruction_sync(existing.as_deref())?;

    if existing.as_deref() != Some(prepared.bytes.as_slice()) {
        fs::write(path, &prepared.bytes)?;
    }

    Ok(SyncOutcome {
        instruction_file: path.to_path_buf(),
        file_created: prepared.file_created,
        block_present: prepared.block_present,
        block_inserted: prepared.block_inserted,
        block_updated: prepared.block_updated,
    })
}

pub(crate) fn prepare_instruction_sync(
    existing: Option<&[u8]>,
) -> Result<PreparedInstructionSync, SeedError> {
    let file_created = existing.is_none();
    let existing = match existing {
        Some(bytes) => std::str::from_utf8(bytes).map_err(|error| {
            SeedError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("instruction file is not UTF-8: {error}"),
            ))
        })?,
        None => "",
    };
    let sync = sync_content(existing)?;
    Ok(PreparedInstructionSync {
        bytes: sync.next.into_bytes(),
        file_created,
        block_present: sync.status != ManagedBlockStatus::Missing,
        block_inserted: sync.status == ManagedBlockStatus::Missing,
        block_updated: sync.status == ManagedBlockStatus::Drifted,
    })
}

pub fn instruction_file_status(path: &Path) -> Result<StatusOutcome, SeedError> {
    let file_exists = path.exists();
    let existing = if file_exists {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    let status = inspect_content(&existing)?;

    Ok(StatusOutcome {
        instruction_file: path.to_path_buf(),
        file_exists,
        managed_block: status,
    })
}

#[cfg(test)]
fn seed_content(existing: &str) -> Result<(String, bool), SeedError> {
    let sync = sync_content(existing)?;
    Ok((sync.next, sync.status == ManagedBlockStatus::Drifted))
}

fn append_managed_block(existing: &str, block: &str) -> String {
    if existing.is_empty() {
        return block.to_string();
    }

    let mut next = String::with_capacity(existing.len() + block.len() + 2);
    next.push_str(existing);

    if !existing.ends_with('\n') {
        next.push('\n');
    }
    next.push('\n');
    next.push_str(block);

    next
}

fn marker_positions(haystack: &str, marker: &str) -> Vec<usize> {
    haystack
        .match_indices(marker)
        .map(|(index, _)| index)
        .collect()
}

fn inspect_content(existing: &str) -> Result<ManagedBlockStatus, SeedError> {
    let block = managed_block_text();
    let begin_positions = marker_positions(existing, BEGIN_MARKER);
    let end_positions = marker_positions(existing, END_MARKER);

    match (begin_positions.as_slice(), end_positions.as_slice()) {
        ([], []) => Ok(ManagedBlockStatus::Missing),
        ([begin], [end]) if begin < end => {
            let end_index = end + END_MARKER.len();
            let existing_block = &existing[*begin..end_index];

            if existing_block == block {
                Ok(ManagedBlockStatus::InSync)
            } else {
                Ok(ManagedBlockStatus::Drifted)
            }
        }
        _ => Err(SeedError::DamagedManagedBlock),
    }
}

fn sync_content(existing: &str) -> Result<ContentSync, SeedError> {
    let status = inspect_content(existing)?;

    let next = match status {
        ManagedBlockStatus::Missing => append_managed_block(existing, &managed_block()),
        ManagedBlockStatus::InSync => existing.to_string(),
        ManagedBlockStatus::Drifted => replace_managed_block(existing),
    };

    Ok(ContentSync { next, status })
}

fn replace_managed_block(existing: &str) -> String {
    let begin = existing
        .find(BEGIN_MARKER)
        .expect("replace_managed_block requires a managed block");
    let end = existing
        .find(END_MARKER)
        .expect("replace_managed_block requires a managed block");
    let end_index = end + END_MARKER.len();
    let block = managed_block_text();
    let mut next = String::with_capacity(existing.len() - (end_index - begin) + block.len());
    next.push_str(&existing[..begin]);
    next.push_str(&block);
    next.push_str(&existing[end_index..]);
    next
}

struct ContentSync {
    next: String,
    status: ManagedBlockStatus,
}

fn managed_block() -> String {
    format!("{}\n", managed_block_text())
}

fn managed_block_text() -> String {
    format!("{BEGIN_MARKER}\n{MANAGED_BLOCK_BODY}{END_MARKER}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("aopmem-stage-023-{name}-{nanos}.md"))
    }

    #[test]
    fn defaults_to_agents_file_for_codex_adapter() {
        let repo_root = Path::new("/tmp/example-repo");

        assert_eq!(
            default_instruction_file(repo_root),
            repo_root.join("AGENTS.md")
        );
    }

    #[test]
    fn appends_managed_block_without_overwriting_existing_content() {
        let existing = "# Local rules\nKeep this text.\n";

        let (seeded, updated) = seed_content(existing).expect("seed should succeed");

        assert!(!updated);
        assert!(seeded.starts_with(existing));
        assert!(seeded.contains(BEGIN_MARKER));
        assert!(seeded.contains(END_MARKER));
        assert!(seeded.contains("AOPMem is installed."));
        assert!(seeded.contains("Normal work MUST use `aopmem recall --query"));
    }

    #[test]
    fn reports_missing_managed_block_in_status() {
        let status = instruction_file_status(Path::new("/tmp/aopmem-stage-024-missing.md"))
            .expect("status should succeed");

        assert!(!status.file_exists);
        assert_eq!(status.managed_block, ManagedBlockStatus::Missing);
    }

    #[test]
    fn reports_in_sync_managed_block_in_status() {
        let status = inspect_content(&managed_block()).expect("status should succeed");

        assert_eq!(status, ManagedBlockStatus::InSync);
    }

    #[test]
    fn reports_drifted_managed_block_in_status() {
        let existing = concat!(
            "<!-- AOPMEM:BEGIN managed block -->\n",
            "manual edit\n",
            "<!-- AOPMEM:END managed block -->\n"
        );

        let status = inspect_content(existing).expect("status should succeed");

        assert_eq!(status, ManagedBlockStatus::Drifted);
    }

    #[test]
    fn replaces_only_existing_managed_block() {
        let existing = concat!(
            "# Header\n",
            "<!-- AOPMEM:BEGIN managed block -->\n",
            "old text\n",
            "<!-- AOPMEM:END managed block -->\n",
            "Footer\n"
        );

        let (seeded, updated) = seed_content(existing).expect("seed should succeed");

        assert!(updated);
        assert!(seeded.starts_with("# Header\n"));
        assert!(seeded.ends_with("\nFooter\n"));
        assert!(seeded.contains(MANAGED_BLOCK_BODY));
        assert!(!seeded.contains("old text"));
    }

    #[test]
    fn rejects_damaged_managed_block() {
        let existing = concat!(
            "# Header\n",
            "<!-- AOPMEM:BEGIN managed block -->\n",
            "orphaned text\n"
        );

        let error = seed_content(existing).expect_err("damaged block should fail");

        assert!(matches!(error, SeedError::DamagedManagedBlock));
    }

    #[test]
    fn sync_replaces_only_drifted_block() {
        let path = temp_path("sync-drifted");
        let existing = concat!(
            "# Header\n",
            "<!-- AOPMEM:BEGIN managed block -->\n",
            "manual edit\n",
            "<!-- AOPMEM:END managed block -->\n",
            "Footer\n"
        );
        fs::write(&path, existing).expect("fixture should be writable");

        let outcome = sync_instruction_file(&path).expect("sync should succeed");
        let written = fs::read_to_string(&path).expect("synced file should be readable");

        assert!(!outcome.file_created);
        assert!(outcome.block_present);
        assert!(!outcome.block_inserted);
        assert!(outcome.block_updated);
        assert_eq!(
            written,
            format!("# Header\n{}\nFooter\n", managed_block_text())
        );
        assert!(written.contains("Memory Keeper / recall"));
        assert!(written.contains("Use `aopmem tool run <tool-id>` for generated tools."));

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn creates_missing_instruction_file() {
        let path = temp_path("create-file");

        let outcome = seed_instruction_file(&path).expect("seed should create file");
        let written = fs::read_to_string(&path).expect("seeded file should be readable");

        assert!(outcome.file_created);
        assert!(!outcome.block_updated);
        assert_eq!(outcome.instruction_file, path);
        assert_eq!(written, managed_block());
        for required_line in [
            "AOPMem is installed.",
            "Main work starts with Memory Keeper / recall.",
            "Normal work MUST use `aopmem recall --query \"<current task>\"` before non-trivial work.",
            "The first task recall creates `bundle_id`; do not pass global `--bundle-id` to a first, bare, or `--full` recall.",
            "Memory Keeper follows `continuation_cursor` with the same query and exact `--bundle-id <bundle_id>` until `more_results=false` or `budget.task.exhausted=true`.",
            "Memory Keeper passes global `--bundle-id <bundle_id>` to later AOPMem operations for the same work.",
            "`more_results=true` with a null cursor is a recall contract error.",
            "Never use `aopmem recall --full` in normal task flow; it is debug/audit/export/migration only.",
            "Do not edit AOPMem SQLite directly.",
            "Use `aopmem tool run <tool-id>` for generated tools.",
            "Generated tool runtime limits and output mode come from its validated `tool.json`.",
            "Tool processes use the tool root as cwd; resolve resources through validated `runtime.runtime_dir` relative to that root.",
            "For shebang tools, `$0` and the concrete entrypoint launch path are implementation details; do not use them for resource discovery.",
            "Defaults are 30000 ms and 65536 bytes per stream; hard ceilings are 900000 ms and 10485760 bytes per stream.",
            "`output_mode=inline` returns `TOOL_OUTPUT_OVERFLOW` and writes no artifact when a stream exceeds its limit.",
            "`output_mode=artifact` keeps bounded previews and publishes full output only under `artifacts/YYYY-MM-DD/`.",
            "Artifact capture above 10485760 bytes per stream returns `TOOL_OUTPUT_OVERFLOW` and publishes nothing.",
            "`--dry-run` executes nothing and creates no artifact.",
            "Approval is required when `approval_requirement != none`, for `external_write` or `destructive`, and for explicit high-risk policy.",
            "No approval is required for `none`, `local_read`, contract-safe `local_write_artifact`, or `external_read` with `approval_requirement=none`.",
            "Do not store secrets.",
            "Use `remember`, `teach`, `reflect` only by user trigger.",
            "Feedback is user-triggered or agent post-task: `aopmem --bundle-id <bundle_id> feedback record --outcome useful|partial|wrong [--reason \"<short reason>\"]`.",
            "Feedback stays only in Local Observability; never put the full task, raw chat, raw output, secrets, or hidden reasoning in its reason.",
            "Memory stored under user-level AOPMem workspace, not repo.",
            "Do not create `.aopmem` in repo.",
            "Deprecated memory excluded from normal recall.",
            "Memory Keeper follows list `next_cursor` pages until `more_results=false` whenever a full set is needed.",
            "Artifacts cleanup policy: 7 days OR 1 GB per workspace.",
        ] {
            assert!(written.contains(required_line), "missing {required_line}");
        }

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn embedded_and_canonical_managed_blocks_match_exactly() {
        assert_eq!(
            include_str!("../../templates/managed-block/AGENTS.managed-block.md"),
            managed_block()
        );
    }

    #[test]
    fn memory_keeper_requires_complete_cursor_traversal() {
        let skill = include_str!("../../templates/skills/memory-keeper/SKILL.md");

        for required in [
            "Read `more_results` after every page.",
            "call the same list with its returned `next_cursor`",
            "Stop only when `more_results` is `false`.",
            "Never treat the first page",
        ] {
            assert!(skill.contains(required), "missing {required}");
        }
        for required in [
            "aopmem recall --query \"<current task>\"",
            "Keep the returned `bundle_id`",
            "budget.task.exhausted=false",
            "more_results=true` with a null cursor",
            "Never use `aopmem recall --full` in normal task flow",
            "exact global\n   `--bundle-id <bundle_id>`",
            "every later AOPMem operation for that work",
            "feedback record --outcome useful|partial|wrong",
            "Feedback stays only in Local Observability",
        ] {
            assert!(skill.contains(required), "missing {required}");
        }
    }

    #[test]
    fn sync_reports_existing_synced_block_without_update() {
        let path = temp_path("sync-noop");
        fs::write(&path, managed_block()).expect("fixture should be writable");

        let outcome = sync_instruction_file(&path).expect("sync should succeed");

        assert!(!outcome.file_created);
        assert!(outcome.block_present);
        assert!(!outcome.block_inserted);
        assert!(!outcome.block_updated);
        let written = fs::read_to_string(&path).expect("synced file should be readable");
        assert_eq!(marker_positions(&written, BEGIN_MARKER).len(), 1);
        assert_eq!(marker_positions(&written, END_MARKER).len(), 1);

        fs::remove_file(path).expect("temp file should be removed");
    }
}
