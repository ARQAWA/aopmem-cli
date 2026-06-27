use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const BEGIN_MARKER: &str = "<!-- AOPMEM:BEGIN managed block -->";
pub const END_MARKER: &str = "<!-- AOPMEM:END managed block -->";

const MANAGED_BLOCK_BODY: &str = "\
This block is managed by AOPMem.\n\
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
    let file_created = !path.exists();
    let existing = if file_created {
        String::new()
    } else {
        fs::read_to_string(path)?
    };
    let sync = sync_content(&existing)?;

    if file_created || sync.next != existing {
        fs::write(path, sync.next)?;
    }

    Ok(SyncOutcome {
        instruction_file: path.to_path_buf(),
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

        fs::remove_file(path).expect("temp file should be removed");
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

        fs::remove_file(path).expect("temp file should be removed");
    }
}
