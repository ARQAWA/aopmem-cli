use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::redaction::{TaggedValueRedactionError, TaggedValueRedactor};

mod anchored;
mod anchored_git;

pub(crate) use anchored::{AnchoredDir, WorkspaceIdentity};

pub const NODE_CREATED_EVENT: &str = "node.created";
pub const NODE_UPDATED_EVENT: &str = "node.updated";
pub const LINK_CREATED_EVENT: &str = "link.created";
pub const REFLECTION_INVENTORY_CREATED_EVENT: &str = "reflection.inventory.created";
pub const REFLECTION_INVENTORY_UPDATED_EVENT: &str = "reflection.inventory.updated";
pub const REFLECTION_PROPOSAL_CREATED_EVENT: &str = "reflection.proposal.created";
pub const REFLECTION_PROPOSAL_APPLIED_EVENT: &str = "reflection.proposal.applied";
pub const REFLECTION_PROPOSAL_DRAFTED_EVENT: &str = "reflection.proposal.drafted";
pub const REFLECTION_APPLY_FAILED_EVENT: &str = "reflection.apply.failed";

const NODE_SUBJECT: &str = "node";
const LINK_SUBJECT: &str = "link";
const REFLECTION_EVENT_SOURCE: &str = "reflection";
const SNAPSHOT_FILE_NAME: &str = "memory.sql";
const SNAPSHOT_LOCK_FILE_NAME: &str = ".snapshot.lock";
pub const PENDING_SNAPSHOT_MARKER_FILE_NAME: &str = ".pending-snapshot";
const FTS_NODES_TABLE: &str = "fts_nodes";
const AUDIT_GIT_AUTHOR_NAME: &str = "AOPMem Audit";
const AUDIT_GIT_AUTHOR_EMAIL: &str = "aopmem-audit@localhost";
const AUDIT_GIT_COMMIT_MESSAGE: &str = "audit: update memory snapshot";
const PENDING_SNAPSHOT_MARKER_CONTENT: &[u8] = b"pending audit snapshot\n";
const SQL_WRITE_CHUNK_BYTES: usize = 8 * 1024;
const HEX_INPUT_CHUNK_BYTES: usize = SQL_WRITE_CHUNK_BYTES / 2;
const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";
const MAX_AUDIT_GIT_METADATA_ENTRIES: usize = 200_000;
const INSERT_EVENT_SQL: &str = "
    INSERT INTO events (type, source, subject_kind, subject_id)
    VALUES (?1, ?2, ?3, ?4);
";
const GET_EVENT_SQL: &str = "
    SELECT id, type, timestamp, source, subject_kind, subject_id
    FROM events
    WHERE id = ?1;
";

/// Process-wide and cross-process serialization for snapshot publication.
///
/// The file is intentionally permanent. Removing a lock file can create two
/// different inodes and let concurrent writers believe they both own the lock.
#[derive(Debug)]
pub(crate) struct SnapshotLock {
    file: File,
    audit_root: AnchoredDir,
}

impl SnapshotLock {
    fn acquire(
        audit_git_dir: &Path,
        expected_workspace: Option<WorkspaceIdentity>,
    ) -> io::Result<Self> {
        let audit_root =
            AnchoredDir::open_or_create_audit_root_with_identity(audit_git_dir, expected_workspace)
                .map_err(|error| anchored_preflight_error(audit_git_dir, error))?;
        Self::acquire_anchored(audit_git_dir, audit_root)
    }

    fn acquire_anchored(audit_git_dir: &Path, audit_root: AnchoredDir) -> io::Result<Self> {
        for name in [SNAPSHOT_FILE_NAME, PENDING_SNAPSHOT_MARKER_FILE_NAME] {
            audit_root
                .open_regular_optional(name)
                .map_err(|error| anchored_preflight_error(&audit_git_dir.join(name), error))?;
        }
        let git_path = audit_git_dir.join(".git");
        if let Ok(metadata) = fs::symlink_metadata(&git_path) {
            if path_is_link_or_reparse_point(&metadata) {
                return Err(anchored_preflight_error(
                    &git_path,
                    io::Error::new(io::ErrorKind::PermissionDenied, "links are forbidden"),
                ));
            }
        }
        let file = audit_root
            .open_or_create_regular(SNAPSHOT_LOCK_FILE_NAME)
            .map_err(|error| {
                anchored_preflight_error(&audit_git_dir.join(SNAPSHOT_LOCK_FILE_NAME), error)
            })?;
        file.lock()?;
        Ok(Self { file, audit_root })
    }

    fn audit_root(&self) -> &AnchoredDir {
        &self.audit_root
    }
}

/// Mutation and snapshot locks bound to one identity-checked workspace
/// capability. This preserves mutation-before-snapshot lock ordering without
/// reopening the workspace by path between validation and lock creation.
#[derive(Debug)]
pub(crate) struct WorkspaceMutationLocks {
    mutation_file: File,
    snapshot_lock: SnapshotLock,
}

impl WorkspaceMutationLocks {
    pub(crate) fn snapshot_lock(&self) -> &SnapshotLock {
        &self.snapshot_lock
    }
}

impl Drop for WorkspaceMutationLocks {
    fn drop(&mut self) {
        let _ = self.mutation_file.unlock();
    }
}

fn anchored_preflight_error(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "unsafe persistent workspace path ({}): {error}",
            path.display()
        ),
    )
}

fn path_is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    #[cfg(not(windows))]
    {
        false
    }
}

impl Drop for SnapshotLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub(crate) fn acquire_snapshot_lock(audit_git_dir: &Path) -> Result<SnapshotLock, SnapshotError> {
    SnapshotLock::acquire(audit_git_dir, None).map_err(Into::into)
}

pub(crate) fn acquire_workspace_mutation_locks(
    workspace_root: &Path,
    audit_git_dir: &Path,
    mutation_lock_name: &str,
    expected_workspace: WorkspaceIdentity,
) -> Result<WorkspaceMutationLocks, SnapshotError> {
    let workspace = AnchoredDir::open_workspace(workspace_root, Some(expected_workspace))
        .map_err(|error| anchored_preflight_error(workspace_root, error))?;
    let mutation_file = workspace
        .open_or_create_regular(mutation_lock_name)
        .map_err(|error| {
            anchored_preflight_error(&workspace_root.join(mutation_lock_name), error)
        })?;
    mutation_file.lock()?;

    let audit_name = audit_git_dir.file_name().ok_or_else(|| {
        anchored_preflight_error(
            audit_git_dir,
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "audit path has no file name",
            ),
        )
    })?;
    if audit_git_dir.parent() != Some(workspace_root) {
        return Err(anchored_preflight_error(
            audit_git_dir,
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "audit directory is not a direct workspace child",
            ),
        )
        .into());
    }
    let audit_root = workspace
        .child_dir_os(audit_name, false)
        .map_err(|error| anchored_preflight_error(audit_git_dir, error))?;
    validate_existing_git_tree(audit_git_dir, &audit_root)?;
    let snapshot_lock = SnapshotLock::acquire_anchored(audit_git_dir, audit_root)?;
    Ok(WorkspaceMutationLocks {
        mutation_file,
        snapshot_lock,
    })
}

fn validate_existing_git_tree(audit_git_dir: &Path, audit_root: &AnchoredDir) -> io::Result<()> {
    let git = match audit_root.child_dir_optional(".git") {
        Ok(Some(git)) => git,
        Ok(None) => return Ok(()),
        Err(directory_error) => {
            // A real regular `.git` is an ordinary snapshot failure: the DB
            // mutation may still commit and return AUDIT_SNAPSHOT_PENDING.
            // Links and special entries fail both anchored opens and must stop
            // before the operation can run.
            return match audit_root.open_regular_optional(".git") {
                Ok(Some(_)) => Ok(()),
                Ok(None) | Err(_) => Err(anchored_preflight_error(
                    &audit_git_dir.join(".git"),
                    directory_error,
                )),
            };
        }
    };
    git.validate_descendant_tree(MAX_AUDIT_GIT_METADATA_ENTRIES)
        .map_err(|error| anchored_preflight_error(&audit_git_dir.join(".git"), error))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingSnapshotMarker {
    Created,
    Existing,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    pub id: i64,
    pub event_type: String,
    pub timestamp: String,
    pub source: String,
    pub subject_kind: String,
    pub subject_id: i64,
}

/// Closed set of durable operational events emitted by reflection flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReflectionEventKind {
    InventoryCreated,
    InventoryUpdated,
    ProposalCreated,
    ProposalApplied,
    ProposalDrafted,
    ApplyFailed,
}

impl ReflectionEventKind {
    const fn event_type(self) -> &'static str {
        match self {
            Self::InventoryCreated => REFLECTION_INVENTORY_CREATED_EVENT,
            Self::InventoryUpdated => REFLECTION_INVENTORY_UPDATED_EVENT,
            Self::ProposalCreated => REFLECTION_PROPOSAL_CREATED_EVENT,
            Self::ProposalApplied => REFLECTION_PROPOSAL_APPLIED_EVENT,
            Self::ProposalDrafted => REFLECTION_PROPOSAL_DRAFTED_EVENT,
            Self::ApplyFailed => REFLECTION_APPLY_FAILED_EVENT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SqlSnapshotReport {
    pub path: PathBuf,
    pub duration_ms: u64,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitCommitOutcome {
    Created,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditRepairStatus {
    AlreadyClean,
    Repaired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditRepairReport {
    pub status: AuditRepairStatus,
    pub duration_ms: u64,
    pub bytes_written: u64,
    pub sha256: Option<String>,
    pub git_commit: Option<GitCommitOutcome>,
    pub marker_present_before: bool,
    pub marker_present_after: bool,
    pub operational_db_written: bool,
}

#[derive(Debug)]
pub enum SnapshotError {
    Db(rusqlite::Error),
    Io(std::io::Error),
    Redaction(TaggedValueRedactionError),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Redaction(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventValidationError {
    MissingType,
    MissingSource,
    InvalidSubjectKind(String),
    InvalidSubjectId(i64),
}

impl fmt::Display for EventValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingType => write!(formatter, "missing required field: type"),
            Self::MissingSource => write!(formatter, "missing required field: source"),
            Self::InvalidSubjectKind(kind) => write!(formatter, "invalid subject kind: {kind}"),
            Self::InvalidSubjectId(id) => write!(formatter, "invalid subject id: {id}"),
        }
    }
}

impl std::error::Error for EventValidationError {}

#[derive(Debug)]
pub enum AuditError {
    Validation(EventValidationError),
    Db(rusqlite::Error),
}

impl fmt::Display for AuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AuditError {}

impl From<EventValidationError> for AuditError {
    fn from(error: EventValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<rusqlite::Error> for AuditError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

impl From<rusqlite::Error> for SnapshotError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

impl From<std::io::Error> for SnapshotError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<TaggedValueRedactionError> for SnapshotError {
    fn from(error: TaggedValueRedactionError) -> Self {
        Self::Redaction(error)
    }
}

pub fn record_node_created(
    connection: &Connection,
    node_id: i64,
    source: &str,
) -> Result<Event, AuditError> {
    record_event(
        connection,
        NODE_CREATED_EVENT,
        source,
        NODE_SUBJECT,
        node_id,
    )
}

pub fn record_node_updated(
    connection: &Connection,
    node_id: i64,
    source: &str,
) -> Result<Event, AuditError> {
    record_event(
        connection,
        NODE_UPDATED_EVENT,
        source,
        NODE_SUBJECT,
        node_id,
    )
}

pub fn record_link_created(
    connection: &Connection,
    link_id: i64,
    source: &str,
) -> Result<Event, AuditError> {
    record_event(
        connection,
        LINK_CREATED_EVENT,
        source,
        LINK_SUBJECT,
        link_id,
    )
}

/// Records one reflection event against its durable node subject.
///
/// The typed kind prevents callers from persisting arbitrary event names or
/// payloads through this path. Reflection history intentionally stores only
/// the event metadata already present in the operational `events` table.
pub(crate) fn record_reflection_event(
    connection: &Connection,
    kind: ReflectionEventKind,
    subject_node_id: i64,
) -> Result<Event, AuditError> {
    record_event(
        connection,
        kind.event_type(),
        REFLECTION_EVENT_SOURCE,
        NODE_SUBJECT,
        subject_node_id,
    )
}

pub fn list_events(connection: &Connection) -> rusqlite::Result<Vec<Event>> {
    let mut statement = connection.prepare(
        "
        SELECT id, type, timestamp, source, subject_kind, subject_id
        FROM events
        ORDER BY id ASC;
        ",
    )?;

    let events = statement.query_map([], row_to_event)?.collect();
    events
}

/// Returns whether an earlier audit snapshot did not finish its Git commit.
pub fn has_pending_snapshot(audit_git_dir: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(audit_git_dir.join(PENDING_SNAPSHOT_MARKER_FILE_NAME)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub fn write_sql_snapshot(
    audit_git_dir: &Path,
    connection: &Connection,
) -> Result<SqlSnapshotReport, SnapshotError> {
    let lock = acquire_snapshot_lock(audit_git_dir)?;
    write_sql_snapshot_locked(audit_git_dir, connection, &lock)
}

pub(crate) fn write_sql_snapshot_locked(
    audit_git_dir: &Path,
    connection: &Connection,
    lock: &SnapshotLock,
) -> Result<SqlSnapshotReport, SnapshotError> {
    write_sql_snapshot_with_hooks_locked(
        audit_git_dir,
        connection,
        lock,
        create_temporary_snapshot,
        write_sql_dump,
        atomic_publish,
        clear_pending_snapshot_marker_locked,
    )
}

/// Repairs one pending audit snapshot without taking the workspace mutation lock.
///
/// The caller must supply a read-only operational connection. An absent marker
/// is a successful no-op. Every failure before the final marker removal leaves
/// the marker in place.
pub fn repair_sql_snapshot(
    audit_git_dir: &Path,
    connection: &Connection,
) -> Result<AuditRepairReport, SnapshotError> {
    let started_at = Instant::now();
    let lock = acquire_snapshot_lock(audit_git_dir)?;
    repair_sql_snapshot_locked(connection, &lock, started_at)
}

pub(crate) fn repair_sql_snapshot_locked(
    connection: &Connection,
    lock: &SnapshotLock,
    started_at: Instant,
) -> Result<AuditRepairReport, SnapshotError> {
    if lock
        .audit_root()
        .open_regular_optional(PENDING_SNAPSHOT_MARKER_FILE_NAME)?
        .is_none()
    {
        return Ok(AuditRepairReport {
            status: AuditRepairStatus::AlreadyClean,
            duration_ms: elapsed_ms(started_at),
            bytes_written: 0,
            sha256: None,
            git_commit: None,
            marker_present_before: false,
            marker_present_after: false,
            operational_db_written: false,
        });
    }

    connection.pragma_update(None, "query_only", true)?;
    let (bytes_written, digest, git_commit) = repair_pending_snapshot_locked(connection, lock)?;
    finish_repair_locked(lock, clear_pending_snapshot_marker_locked)?;
    Ok(AuditRepairReport {
        status: AuditRepairStatus::Repaired,
        duration_ms: elapsed_ms(started_at),
        bytes_written,
        sha256: Some(digest),
        git_commit: Some(git_commit),
        marker_present_before: true,
        marker_present_after: false,
        operational_db_written: false,
    })
}

fn finish_repair_locked(
    lock: &SnapshotLock,
    clear: impl FnOnce(&SnapshotLock) -> Result<(), SnapshotError>,
) -> Result<(), SnapshotError> {
    lock.audit_root().sync()?;
    if let Err(error) = clear(lock) {
        let _ = ensure_pending_snapshot_marker_locked(lock);
        return Err(error);
    }
    Ok(())
}

pub(crate) fn pending_snapshot_marker_locked(lock: &SnapshotLock) -> Result<bool, SnapshotError> {
    Ok(lock
        .audit_root()
        .open_regular_optional(PENDING_SNAPSHOT_MARKER_FILE_NAME)?
        .is_some())
}

fn repair_pending_snapshot_locked(
    connection: &Connection,
    lock: &SnapshotLock,
) -> Result<(u64, String, GitCommitOutcome), SnapshotError> {
    repair_pending_snapshot_with_hooks_locked(
        connection,
        lock,
        atomic_publish,
        |root, expected| {
            let published = digest_regular(root.open_regular(SNAPSHOT_FILE_NAME)?)?;
            if published == expected {
                Ok(())
            } else {
                Err(io::Error::other(
                    "published audit snapshot digest does not match streamed digest",
                ))
            }
        },
        anchored_git::commit_snapshot,
    )
}

fn repair_pending_snapshot_with_hooks_locked(
    connection: &Connection,
    lock: &SnapshotLock,
    publish: impl FnOnce(&AnchoredDir, File, &str) -> io::Result<()>,
    validate: impl FnOnce(&AnchoredDir, &str) -> io::Result<()>,
    commit: impl FnOnce(&AnchoredDir) -> io::Result<GitCommitOutcome>,
) -> Result<(u64, String, GitCommitOutcome), SnapshotError> {
    let root = lock.audit_root();
    let temporary_name = snapshot_temporary_name();
    let result = (|| {
        let file = create_temporary_snapshot(root, &temporary_name)?;
        let mut writer = CountingWriter::new(BufWriter::new(file));
        let read_transaction = connection.unchecked_transaction()?;
        write_sql_dump(&mut writer, &read_transaction)?;
        writer.flush()?;
        writer.inner.get_ref().sync_all()?;
        read_transaction.commit()?;
        let bytes_written = writer.bytes_written;
        let streamed_digest = hex_digest(writer.hasher.clone().finalize().as_slice());
        let temporary = writer
            .inner
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)?;
        publish(root, temporary, &temporary_name)?;
        validate(root, &streamed_digest)?;
        let git_commit = commit(root)?;
        Ok((bytes_written, streamed_digest, git_commit))
    })();
    if result.is_err() {
        let _ = root.remove_regular(&temporary_name);
    }
    result
}

fn digest_regular(mut file: File) -> io::Result<String> {
    use std::io::Read;

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize().as_slice()))
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn create_temporary_snapshot(root: &AnchoredDir, name: &str) -> io::Result<fs::File> {
    root.create_new_regular(name)
}

fn atomic_publish(root: &AnchoredDir, temporary: File, name: &str) -> io::Result<()> {
    crate::platform_publish::publish_regular(
        root,
        temporary,
        OsStr::new(name),
        OsStr::new(SNAPSHOT_FILE_NAME),
        crate::platform_publish::PublishMode::ReplaceOrCreate,
    )
    .map_err(crate::platform_publish::PublishError::into_io_error)
    .and_then(crate::platform_publish::require_committed_validated_clean)
}

#[cfg(test)]
fn write_sql_snapshot_with_hooks(
    audit_git_dir: &Path,
    connection: &Connection,
    open_temporary: impl FnOnce(&AnchoredDir, &str) -> io::Result<fs::File>,
    dump: impl FnOnce(
        &mut CountingWriter<BufWriter<fs::File>>,
        &Connection,
    ) -> Result<(), SnapshotError>,
    publish: impl FnOnce(&AnchoredDir, File, &str) -> io::Result<()>,
) -> Result<SqlSnapshotReport, SnapshotError> {
    let lock = acquire_snapshot_lock(audit_git_dir)?;
    write_sql_snapshot_with_hooks_locked(
        audit_git_dir,
        connection,
        &lock,
        open_temporary,
        dump,
        publish,
        clear_pending_snapshot_marker_locked,
    )
}

fn write_sql_snapshot_with_hooks_locked(
    audit_git_dir: &Path,
    connection: &Connection,
    lock: &SnapshotLock,
    open_temporary: impl FnOnce(&AnchoredDir, &str) -> io::Result<fs::File>,
    dump: impl FnOnce(
        &mut CountingWriter<BufWriter<fs::File>>,
        &Connection,
    ) -> Result<(), SnapshotError>,
    publish: impl FnOnce(&AnchoredDir, File, &str) -> io::Result<()>,
    clear: impl FnOnce(&SnapshotLock) -> Result<(), SnapshotError>,
) -> Result<SqlSnapshotReport, SnapshotError> {
    let started_at = Instant::now();
    let root = lock.audit_root();
    ensure_pending_snapshot_marker_locked(lock)?;

    let path = audit_git_dir.join(SNAPSHOT_FILE_NAME);
    let temporary_name = snapshot_temporary_name();
    let write_result = (|| {
        let file = open_temporary(root, &temporary_name)?;
        let mut writer = CountingWriter::new(BufWriter::new(file));
        let read_transaction = connection.unchecked_transaction()?;

        dump(&mut writer, &read_transaction)?;
        writer.flush()?;
        writer.inner.get_ref().sync_all()?;
        read_transaction.commit()?;
        let bytes_written = writer.bytes_written;
        let buffered = writer.inner;
        let temporary = buffered
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)?;
        publish(root, temporary, &temporary_name)?;
        Ok(bytes_written)
    })();

    let bytes_written = match write_result {
        Ok(bytes_written) => bytes_written,
        Err(error) => {
            let _ = root.remove_regular(&temporary_name);
            return Err(error);
        }
    };

    let _ = anchored_git::commit_snapshot(root)?;
    finish_repair_locked(lock, clear)?;

    Ok(SqlSnapshotReport {
        path,
        duration_ms: u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
        bytes_written,
    })
}

struct CountingWriter<W> {
    inner: W,
    bytes_written: u64,
    hasher: Sha256,
}

impl<W> CountingWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            bytes_written: 0,
            hasher: Sha256::new(),
        }
    }
}

impl<W: IoWrite> IoWrite for CountingWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.hasher.update(&buffer[..written]);
        self.bytes_written = self
            .bytes_written
            .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Durably creates the pending marker without changing an existing marker.
pub fn ensure_pending_snapshot_marker(
    audit_git_dir: &Path,
) -> Result<PendingSnapshotMarker, SnapshotError> {
    let lock = acquire_snapshot_lock(audit_git_dir)?;
    ensure_pending_snapshot_marker_locked(&lock)
}

pub(crate) fn ensure_pending_snapshot_marker_locked(
    lock: &SnapshotLock,
) -> Result<PendingSnapshotMarker, SnapshotError> {
    let marker_result = lock
        .audit_root()
        .create_new_regular(PENDING_SNAPSHOT_MARKER_FILE_NAME);
    let mut marker = match marker_result {
        Ok(marker) => marker,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Ok(PendingSnapshotMarker::Existing);
        }
        Err(error) => return Err(error.into()),
    };
    marker.write_all(PENDING_SNAPSHOT_MARKER_CONTENT)?;
    marker.sync_all()?;
    lock.audit_root().sync()?;
    Ok(PendingSnapshotMarker::Created)
}

pub fn clear_pending_snapshot_marker(audit_git_dir: &Path) -> Result<(), SnapshotError> {
    let lock = acquire_snapshot_lock(audit_git_dir)?;
    clear_pending_snapshot_marker_locked(&lock)
}

pub(crate) fn clear_pending_snapshot_marker_locked(
    lock: &SnapshotLock,
) -> Result<(), SnapshotError> {
    match lock
        .audit_root()
        .remove_regular(PENDING_SNAPSHOT_MARKER_FILE_NAME)
    {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn snapshot_temporary_name() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        ".{SNAPSHOT_FILE_NAME}.{}.{}.tmp",
        std::process::id(),
        timestamp
    )
}

fn record_event(
    connection: &Connection,
    event_type: &str,
    source: &str,
    subject_kind: &str,
    subject_id: i64,
) -> Result<Event, AuditError> {
    validate_event(event_type, source, subject_kind, subject_id)?;

    connection
        .prepare_cached(INSERT_EVENT_SQL)?
        .execute(params![event_type, source, subject_kind, subject_id])?;

    let id = connection.last_insert_rowid();
    get_event(connection, id)?.ok_or(AuditError::Db(rusqlite::Error::QueryReturnedNoRows))
}

fn validate_event(
    event_type: &str,
    source: &str,
    subject_kind: &str,
    subject_id: i64,
) -> Result<(), EventValidationError> {
    if event_type.trim().is_empty() {
        return Err(EventValidationError::MissingType);
    }
    if source.trim().is_empty() {
        return Err(EventValidationError::MissingSource);
    }
    if ![NODE_SUBJECT, LINK_SUBJECT].contains(&subject_kind) {
        return Err(EventValidationError::InvalidSubjectKind(
            subject_kind.to_string(),
        ));
    }
    if subject_id <= 0 {
        return Err(EventValidationError::InvalidSubjectId(subject_id));
    }

    Ok(())
}

fn get_event(connection: &Connection, id: i64) -> rusqlite::Result<Option<Event>> {
    let mut statement = connection.prepare_cached(GET_EVENT_SQL)?;
    statement.query_row([id], row_to_event).optional()
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get(0)?,
        event_type: row.get(1)?,
        timestamp: row.get(2)?,
        source: row.get(3)?,
        subject_kind: row.get(4)?,
        subject_id: row.get(5)?,
    })
}

fn write_sql_dump<W: IoWrite>(
    writer: &mut W,
    connection: &Connection,
) -> Result<(), SnapshotError> {
    let redactor = TaggedValueRedactor::load(connection)?;
    writer.write_all(b"BEGIN TRANSACTION;\nPRAGMA defer_foreign_keys = ON;\n")?;

    let mut schema_statement = connection.prepare(
        "
        SELECT type, name, sql
        FROM sqlite_master
        WHERE sql IS NOT NULL
          AND type IN ('table', 'index', 'trigger', 'view')
          AND name NOT LIKE 'sqlite_%'
          AND (name = ?1 OR name NOT LIKE ?2)
        ORDER BY
            CASE type
                WHEN 'table' THEN 0
                WHEN 'index' THEN 1
                WHEN 'trigger' THEN 2
                WHEN 'view' THEN 3
                ELSE 4
            END,
            name ASC;
        ",
    )?;
    let fts_shadow_pattern = format!("{FTS_NODES_TABLE}_%");
    let schema_rows =
        schema_statement.query_map([FTS_NODES_TABLE, &fts_shadow_pattern], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

    let mut table_names = Vec::new();
    let mut non_table_sql = Vec::new();
    let mut has_fts_nodes = false;
    for row in schema_rows {
        let (object_type, name, sql) = row?;

        if object_type == "table" {
            write_sql_statement(writer, &sql)?;
            has_fts_nodes |= name == FTS_NODES_TABLE;
            table_names.push(name);
        } else {
            non_table_sql.push(sql);
        }
    }

    for table_name in table_names {
        if table_name != FTS_NODES_TABLE {
            append_table_rows(writer, connection, &table_name, &redactor)?;
        }
    }

    if has_table(connection, "sqlite_sequence")? {
        append_sqlite_sequence_rows(writer, connection, &redactor)?;
    }

    if has_fts_nodes {
        write_fts_nodes_rebuild(writer)?;
    }

    for sql in non_table_sql {
        write_sql_statement(writer, &sql)?;
    }

    writer.write_all(b"COMMIT;\n")?;
    Ok(())
}

#[cfg(test)]
fn build_sql_dump(connection: &Connection) -> Result<String, SnapshotError> {
    let read_transaction = connection.unchecked_transaction()?;
    let mut dump = Vec::new();

    write_sql_dump(&mut dump, &read_transaction)?;
    read_transaction.commit()?;

    Ok(String::from_utf8(dump).expect("SQL dump must be valid UTF-8"))
}

fn write_sql_statement<W: IoWrite>(writer: &mut W, sql: &str) -> Result<(), SnapshotError> {
    writer.write_all(sql.as_bytes())?;
    writer.write_all(b";\n")?;
    Ok(())
}

fn write_fts_nodes_rebuild<W: IoWrite>(writer: &mut W) -> Result<(), SnapshotError> {
    writer.write_all(
        b"INSERT INTO \"fts_nodes\" (rowid, title, summary, body, aliases)
        SELECT
            nodes.id,
            nodes.title,
            COALESCE(nodes.summary, ''),
            COALESCE(nodes.body, ''),
            COALESCE((
                SELECT group_concat(alias, ' ')
                FROM (
                    SELECT aliases_for_node.alias
                    FROM aliases AS aliases_for_node
                    WHERE aliases_for_node.node_id = nodes.id
                    ORDER BY aliases_for_node.id ASC, aliases_for_node.alias ASC
                ) AS ordered_aliases
            ), '')
        FROM nodes
        ORDER BY nodes.id ASC;\n",
    )?;
    Ok(())
}

fn has_table(connection: &Connection, table_name: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1);",
        [table_name],
        |row| row.get(0),
    )
}

fn append_sqlite_sequence_rows<W: IoWrite>(
    writer: &mut W,
    connection: &Connection,
    redactor: &TaggedValueRedactor,
) -> Result<(), SnapshotError> {
    writer.write_all(b"DELETE FROM \"sqlite_sequence\";\n")?;
    let mut statement =
        connection.prepare("SELECT name, seq FROM sqlite_sequence ORDER BY name ASC, seq ASC;")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        writer.write_all(b"INSERT INTO \"sqlite_sequence\" (\"name\", \"seq\") VALUES (")?;
        write_sql_value(writer, row.get_ref(0)?, redactor)?;
        writer.write_all(b", ")?;
        write_sql_value(writer, row.get_ref(1)?, redactor)?;
        writer.write_all(b");\n")?;
    }
    Ok(())
}

fn append_table_rows<W: IoWrite>(
    writer: &mut W,
    connection: &Connection,
    table_name: &str,
    redactor: &TaggedValueRedactor,
) -> Result<(), SnapshotError> {
    let preview_query = format!("SELECT * FROM {} LIMIT 0;", quote_identifier(table_name));
    let preview = connection.prepare(&preview_query)?;
    let column_names = preview
        .column_names()
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    drop(preview);

    let order_clause = if column_names.is_empty() {
        String::new()
    } else {
        format!(
            " ORDER BY {}",
            column_names
                .iter()
                .map(|name| quote_identifier(name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let query = format!(
        "SELECT * FROM {}{};",
        quote_identifier(table_name),
        order_clause
    );
    let mut statement = connection.prepare(&query)?;
    let mut rows = statement.query([])?;
    let columns_sql = column_names
        .iter()
        .map(|name| quote_identifier(name))
        .collect::<Vec<_>>()
        .join(", ");
    let insert_prefix = format!(
        "INSERT INTO {} ({columns_sql}) VALUES (",
        quote_identifier(table_name)
    );

    while let Some(row) = rows.next()? {
        writer.write_all(insert_prefix.as_bytes())?;

        for index in 0..column_names.len() {
            if index > 0 {
                writer.write_all(b", ")?;
            }
            write_sql_value(writer, row.get_ref(index)?, redactor)?;
        }

        writer.write_all(b");\n")?;
    }

    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn write_sql_value<W: IoWrite>(
    writer: &mut W,
    value: ValueRef<'_>,
    redactor: &TaggedValueRedactor,
) -> Result<(), SnapshotError> {
    match value {
        ValueRef::Null => writer.write_all(b"NULL")?,
        ValueRef::Integer(number) => write!(writer, "{number}")?,
        ValueRef::Real(number) if number == f64::INFINITY => {
            writer.write_all(b"9.0e999")?;
        }
        ValueRef::Real(number) if number == f64::NEG_INFINITY => {
            writer.write_all(b"-9.0e999")?;
        }
        ValueRef::Real(number) => write!(writer, "{number:?}")?,
        ValueRef::Text(text) => {
            let redacted = redactor.redact_bytes_with_json_copies(text)?;
            write_sql_text(writer, &redacted)?;
        }
        ValueRef::Blob(bytes) => {
            let redacted = redactor.redact_bytes_with_json_copies(bytes)?;
            write_hex_literal(writer, &redacted)?;
        }
    }
    Ok(())
}

fn write_sql_text<W: IoWrite>(writer: &mut W, text: &[u8]) -> Result<(), SnapshotError> {
    if std::str::from_utf8(text).is_err() || text.contains(&0) {
        writer.write_all(b"CAST(")?;
        write_hex_literal(writer, text)?;
        writer.write_all(b" AS TEXT)")?;
        return Ok(());
    }

    writer.write_all(b"'")?;
    let mut start = 0;
    for (index, byte) in text.iter().enumerate() {
        if *byte == b'\'' {
            write_bounded(writer, &text[start..index])?;
            writer.write_all(b"''")?;
            start = index + 1;
        }
    }
    write_bounded(writer, &text[start..])?;
    writer.write_all(b"'")?;
    Ok(())
}

fn write_hex_literal<W: IoWrite>(writer: &mut W, bytes: &[u8]) -> Result<(), SnapshotError> {
    writer.write_all(b"X'")?;
    let mut encoded = [0_u8; SQL_WRITE_CHUNK_BYTES];
    for chunk in bytes.chunks(HEX_INPUT_CHUNK_BYTES) {
        for (index, byte) in chunk.iter().enumerate() {
            encoded[index * 2] = HEX_DIGITS[usize::from(byte >> 4)];
            encoded[index * 2 + 1] = HEX_DIGITS[usize::from(byte & 0x0f)];
        }
        writer.write_all(&encoded[..chunk.len() * 2])?;
    }
    writer.write_all(b"'")?;
    Ok(())
}

fn write_bounded<W: IoWrite>(writer: &mut W, bytes: &[u8]) -> Result<(), SnapshotError> {
    for chunk in bytes.chunks(SQL_WRITE_CHUNK_BYTES) {
        writer.write_all(chunk)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use std::fs;
    use std::io;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    type AuditNodeRow = (i64, String, String, String, Option<String>, Option<String>);

    #[derive(Debug, PartialEq, Eq)]
    enum OwnedSqlValue {
        Null,
        Integer(i64),
        Real(u64),
        Text(Vec<u8>),
        Blob(Vec<u8>),
    }

    #[derive(Default)]
    struct MaxChunkWriter {
        bytes_written: usize,
        max_chunk_bytes: usize,
    }

    impl IoWrite for MaxChunkWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.bytes_written += buffer.len();
            self.max_chunk_bytes = self.max_chunk_bytes.max(buffer.len());
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn migrated_connection() -> Connection {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for audit test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        connection
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("aopmem-stage-037-audit-{name}-{nanos}"))
    }

    fn count_rows(connection: &Connection, table_name: &str) -> i64 {
        connection
            .query_row(
                &format!("SELECT COUNT(*) FROM {};", quote_identifier(table_name)),
                [],
                |row| row.get(0),
            )
            .expect("table row count should be readable")
    }

    fn table_rows(connection: &Connection, table_name: &str) -> Vec<Vec<OwnedSqlValue>> {
        let preview_query = format!("SELECT * FROM {} LIMIT 0;", quote_identifier(table_name));
        let preview = connection
            .prepare(&preview_query)
            .expect("table preview should prepare");
        let column_names = preview
            .column_names()
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        drop(preview);
        let order = column_names
            .iter()
            .map(|name| quote_identifier(name))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT * FROM {} ORDER BY {order};",
            quote_identifier(table_name)
        );
        let mut statement = connection
            .prepare(&query)
            .expect("table row query should prepare");
        let mut rows = statement.query([]).expect("table rows should query");
        let mut result = Vec::new();
        while let Some(row) = rows.next().expect("next table row should read") {
            let mut values = Vec::with_capacity(column_names.len());
            for index in 0..column_names.len() {
                let value = match row.get_ref(index).expect("table value should read") {
                    ValueRef::Null => OwnedSqlValue::Null,
                    ValueRef::Integer(value) => OwnedSqlValue::Integer(value),
                    ValueRef::Real(value) => OwnedSqlValue::Real(value.to_bits()),
                    ValueRef::Text(value) => OwnedSqlValue::Text(value.to_vec()),
                    ValueRef::Blob(value) => OwnedSqlValue::Blob(value.to_vec()),
                };
                values.push(value);
            }
            result.push(values);
        }
        result
    }

    fn node_rows(connection: &Connection) -> Vec<AuditNodeRow> {
        let mut statement = connection
            .prepare(
                "
                SELECT id, node_type, status, title, summary, body
                FROM nodes
                ORDER BY id ASC;
                ",
            )
            .expect("node rows statement should prepare");

        statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })
            .expect("node rows should query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("node rows should collect")
    }

    fn fts_match_row_ids(connection: &Connection, query: &str) -> Vec<i64> {
        let mut statement = connection
            .prepare(
                "
                SELECT rowid
                FROM fts_nodes
                WHERE fts_nodes MATCH ?1
                ORDER BY rowid ASC;
                ",
            )
            .expect("FTS statement should prepare");

        statement
            .query_map([query], |row| row.get(0))
            .expect("FTS rows should query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("FTS rows should collect")
    }

    fn restore_sql_dump(dump: &str) -> Connection {
        let connection =
            Connection::open_in_memory().expect("in-memory DB should open for restore test");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign keys should enable before restore");
        connection
            .execute_batch(dump)
            .expect("generated SQL dump should restore into an empty DB");
        connection
    }

    fn has_temporary_snapshot(audit_git_dir: &Path) -> bool {
        fs::read_dir(audit_git_dir)
            .expect("audit dir should list")
            .any(|entry| {
                entry
                    .expect("audit entry should read")
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp")
            })
    }

    fn run_git_for_test(audit_git_dir: &Path, arguments: &[&str]) -> std::process::Output {
        Command::new("git")
            .current_dir(audit_git_dir)
            .args(arguments)
            .output()
            .expect("git command should start")
    }

    fn git_stdout_for_test(audit_git_dir: &Path, arguments: &[&str]) -> String {
        let output = run_git_for_test(audit_git_dir, arguments);
        assert!(
            output.status.success(),
            "git {} should succeed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout)
            .expect("git stdout should be UTF-8")
            .trim()
            .to_string()
    }

    fn git_success_for_test(audit_git_dir: &Path, arguments: &[&str]) {
        let output = run_git_for_test(audit_git_dir, arguments);
        assert!(
            output.status.success(),
            "git {} should succeed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn records_node_created_event_with_timestamp_and_source() {
        let connection = migrated_connection();

        let event = record_node_created(&connection, 7, "source=user_instruction")
            .expect("node created event should be recorded");
        let events = list_events(&connection).expect("events should list");

        assert_eq!(event.event_type, NODE_CREATED_EVENT);
        assert_eq!(event.source, "source=user_instruction");
        assert_eq!(event.subject_kind, NODE_SUBJECT);
        assert_eq!(event.subject_id, 7);
        assert!(!event.timestamp.trim().is_empty());
        assert_eq!(events, vec![event]);
    }

    #[test]
    fn records_link_created_event_with_timestamp_and_source() {
        let connection = migrated_connection();

        let event = record_link_created(&connection, 11, "source=cli")
            .expect("link created event should be recorded");

        assert_eq!(event.event_type, LINK_CREATED_EVENT);
        assert_eq!(event.source, "source=cli");
        assert_eq!(event.subject_kind, LINK_SUBJECT);
        assert_eq!(event.subject_id, 11);
        assert!(!event.timestamp.trim().is_empty());
    }

    #[test]
    fn records_closed_reflection_event_set_without_payloads() {
        let connection = migrated_connection();
        let kinds = [
            (
                ReflectionEventKind::InventoryCreated,
                REFLECTION_INVENTORY_CREATED_EVENT,
            ),
            (
                ReflectionEventKind::InventoryUpdated,
                REFLECTION_INVENTORY_UPDATED_EVENT,
            ),
            (
                ReflectionEventKind::ProposalCreated,
                REFLECTION_PROPOSAL_CREATED_EVENT,
            ),
            (
                ReflectionEventKind::ProposalApplied,
                REFLECTION_PROPOSAL_APPLIED_EVENT,
            ),
            (
                ReflectionEventKind::ProposalDrafted,
                REFLECTION_PROPOSAL_DRAFTED_EVENT,
            ),
            (
                ReflectionEventKind::ApplyFailed,
                REFLECTION_APPLY_FAILED_EVENT,
            ),
        ];

        for (index, (kind, event_type)) in kinds.into_iter().enumerate() {
            let subject_id = i64::try_from(index + 1).expect("test subject id should fit");
            let event = record_reflection_event(&connection, kind, subject_id)
                .expect("reflection event should record");
            assert_eq!(event.event_type, event_type);
            assert_eq!(event.source, REFLECTION_EVENT_SOURCE);
            assert_eq!(event.subject_kind, NODE_SUBJECT);
            assert_eq!(event.subject_id, subject_id);
        }

        let stored = list_events(&connection).expect("reflection events should list");
        assert_eq!(stored.len(), kinds.len());
        assert!(stored.iter().all(|event| event.source == "reflection"));
    }

    #[test]
    fn rejects_event_without_source_or_valid_subject_id() {
        let connection = migrated_connection();

        assert!(matches!(
            record_node_created(&connection, 1, " "),
            Err(AuditError::Validation(EventValidationError::MissingSource))
        ));
        assert!(matches!(
            record_link_created(&connection, 0, "source=cli"),
            Err(AuditError::Validation(
                EventValidationError::InvalidSubjectId(0)
            ))
        ));
    }

    #[test]
    fn builds_sql_dump_from_migrated_db_with_sample_rows() {
        let connection = migrated_connection();
        connection
            .execute(
                "
                INSERT INTO nodes (
                    node_type,
                    status,
                    title,
                    summary,
                    body,
                    source_ref,
                    confidence,
                    trust_level
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);
                ",
                params![
                    "fact",
                    "active",
                    "O'Hara",
                    "short summary",
                    "body text",
                    "source://note",
                    0.9_f64,
                    "high"
                ],
            )
            .expect("node should insert");
        record_node_created(&connection, 1, "source=cli").expect("event should record");

        let dump = build_sql_dump(&connection).expect("sql dump should build");

        assert!(dump.starts_with("BEGIN TRANSACTION;\n"));
        assert!(dump.contains("CREATE TABLE nodes"));
        assert!(dump.contains("CREATE VIRTUAL TABLE fts_nodes"));
        assert!(dump.contains("INSERT INTO \"nodes\""));
        assert!(dump.contains("'O''Hara'"));
        assert!(dump.contains("INSERT INTO \"events\""));
        assert!(dump.ends_with("COMMIT;\n"));
    }

    #[test]
    fn stage_010_sql_dump_scrubs_tagged_body_and_json_proposal_copy() {
        let source = migrated_connection();
        let secret = "TEST_ONLY_STAGE010_AUDIT_'quote'\\slash\nline";
        source
            .execute(
                "INSERT INTO nodes (
                    node_type, status, title, body, source_ref,
                    confidence, trust_level
                 ) VALUES (
                    'raw_note', 'active', 'Authorized test credential',
                    ?1, 'source=user_instruction', 1.0, 'high'
                 )",
                [secret],
            )
            .expect("tagged secret node should insert");
        let secret_node_id = source.last_insert_rowid();
        source
            .execute(
                "INSERT INTO tags (node_id, tag) VALUES (?1, ?2)",
                params![secret_node_id, crate::redaction::TEST_SECRET_TAG],
            )
            .expect("tagged secret tag should insert");
        let proposal = serde_json::json!({
            "session_id": "stage-010",
            "items": [{"op": "create_node", "body": secret}],
        })
        .to_string();
        source
            .execute(
                "INSERT INTO nodes (
                    node_type, status, title, summary, body
                 ) VALUES (
                    'raw_note', 'draft', 'Reflection proposal stage-010',
                    'reflection_proposal_v1', ?1
                 )",
                [proposal],
            )
            .expect("proposal copy should insert");

        let dump = build_sql_dump(&source).expect("redacted SQL dump should build");
        assert!(!dump.contains(secret));
        let json_escaped = serde_json::to_string(secret)
            .expect("fake secret should encode")
            .trim_matches('"')
            .to_string();
        assert!(!dump.contains(&json_escaped));
        assert!(
            dump.matches(crate::redaction::TEST_SECRET_REDACTION_MARKER)
                .count()
                >= 2,
            "tagged body and proposal copy must both be scrubbed"
        );

        let stored: String = source
            .query_row(
                "SELECT body FROM nodes WHERE id = ?1",
                [secret_node_id],
                |row| row.get(0),
            )
            .expect("operational exact body should remain");
        assert_eq!(stored, secret);

        let restored = restore_sql_dump(&dump);
        let mut statement = restored
            .prepare("SELECT body FROM nodes WHERE body IS NOT NULL ORDER BY id")
            .expect("restored bodies should prepare");
        let restored_bodies = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("restored bodies should query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("restored bodies should collect");
        assert!(restored_bodies.iter().all(|body| !body.contains(secret)));
        assert!(restored_bodies
            .iter()
            .any(|body| body.contains(crate::redaction::TEST_SECRET_REDACTION_MARKER)));
    }

    #[test]
    fn sql_dump_restores_canonical_rows_and_rebuilds_fts() {
        let source = migrated_connection();
        source
            .execute_batch(
                "
                INSERT INTO nodes (
                    id, node_type, status, title, summary, body,
                    source_ref, confidence, trust_level
                ) VALUES
                    (1, 'fact', 'active', 'Snapshot source',
                     'restore summary', 'restore body', 'source://one', 0.9, 'high'),
                    (2, 'workflow', 'active', 'Snapshot target',
                     NULL, NULL, 'source://two', 0.8, 'medium');
                INSERT INTO aliases (id, node_id, alias)
                VALUES
                    (2, 1, 'secondalias'),
                    (1, 1, 'restoretoken');
                INSERT INTO tags (id, node_id, tag)
                VALUES (1, 1, 'audit');
                INSERT INTO sources (id, node_id, source_ref)
                VALUES (1, 1, 'source://one');
                INSERT INTO links (id, source_node_id, target_node_id, link_type)
                VALUES (1, 1, 2, 'supports');
                INSERT INTO registries (id, registry_type, name, status, notes)
                VALUES (1, 'workflow', 'restore-registry', 'active', 'registry note');
                INSERT INTO tool_contracts (
                    id, tool_id, name, status, owner_workflow, side_effects,
                    approval_requirement, contract_json
                ) VALUES (
                    1, 'restore-tool', 'Restore tool', 'active', 'restore-workflow',
                    'local_read', 'none', '{}'
                );
                INSERT INTO mcp_profiles (
                    id, name, kind, status, read_operations, write_operations,
                    side_effects, approval_requirement, credentials_source, notes
                ) VALUES (
                    'restore-mcp', 'Restore MCP', 'local', 'active', 'read', 'write',
                    'local_read', 'none', NULL, 'mcp note'
                );
                INSERT INTO fts_nodes (rowid, title, summary, body, aliases)
                VALUES
                    (1, 'corrupted title', 'corrupted summary', 'corrupted body', 'corrupted alias'),
                    (2, 'corrupted target', '', '', '');
                ",
            )
            .expect("source rows should insert");
        record_node_created(&source, 1, "source=restore-test").expect("source event should insert");

        let dump = build_sql_dump(&source).expect("sql dump should build");

        for shadow_table in [
            "fts_nodes_config",
            "fts_nodes_content",
            "fts_nodes_data",
            "fts_nodes_docsize",
            "fts_nodes_idx",
        ] {
            assert!(
                !dump.contains(shadow_table),
                "dump must not serialize FTS shadow table {shadow_table}"
            );
        }

        let restored = restore_sql_dump(&dump);
        let integrity: String = restored
            .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
            .expect("integrity check should run");
        let foreign_key_violation: Option<String> = restored
            .query_row("PRAGMA foreign_key_check;", [], |row| row.get(0))
            .optional()
            .expect("foreign key check should run");

        assert_eq!(integrity, "ok");
        assert_eq!(foreign_key_violation, None);
        assert_eq!(node_rows(&restored), node_rows(&source));
        assert_eq!(fts_match_row_ids(&restored, "Snapshot"), vec![1, 2]);
        assert_eq!(fts_match_row_ids(&restored, "summary"), vec![1]);
        assert_eq!(fts_match_row_ids(&restored, "body"), vec![1]);
        assert_eq!(fts_match_row_ids(&restored, "restoretoken"), vec![1]);
        assert!(fts_match_row_ids(&restored, "corrupted").is_empty());
        let restored_aliases: String = restored
            .query_row(
                "SELECT aliases FROM fts_nodes WHERE rowid = 1;",
                [],
                |row| row.get(0),
            )
            .expect("rebuilt FTS aliases should read");
        assert_eq!(restored_aliases, "restoretoken secondalias");

        for table_name in [
            "schema_migrations",
            "nodes",
            "links",
            "aliases",
            "tags",
            "sources",
            "events",
            "registries",
            "tool_contracts",
            "tool_aliases",
            "mcp_profiles",
            "sqlite_sequence",
        ] {
            assert_eq!(
                count_rows(&restored, table_name),
                count_rows(&source, table_name),
                "restored row count must match for {table_name}"
            );
            assert_eq!(
                table_rows(&restored, table_name),
                table_rows(&source, table_name),
                "restored data must match for {table_name}"
            );
        }
    }

    #[test]
    fn sql_dump_restores_sqlite_sequence_without_reusing_deleted_ids() {
        let source = migrated_connection();
        source
            .execute(
                "INSERT INTO nodes (id, node_type, status, title) VALUES (500, 'fact', 'active', 'deleted high id');",
                [],
            )
            .expect("high node id should insert");
        source
            .execute("DELETE FROM nodes WHERE id = 500;", [])
            .expect("high node id should delete");
        let source_sequence: i64 = source
            .query_row(
                "SELECT seq FROM sqlite_sequence WHERE name = 'nodes';",
                [],
                |row| row.get(0),
            )
            .expect("source node sequence should read");

        let dump = build_sql_dump(&source).expect("sql dump should build");
        let restored = restore_sql_dump(&dump);
        let restored_sequence: i64 = restored
            .query_row(
                "SELECT seq FROM sqlite_sequence WHERE name = 'nodes';",
                [],
                |row| row.get(0),
            )
            .expect("restored node sequence should read");
        restored
            .execute(
                "INSERT INTO nodes (node_type, status, title) VALUES ('fact', 'active', 'next id');",
                [],
            )
            .expect("post-restore node should insert");

        assert_eq!(source_sequence, 500);
        assert_eq!(restored_sequence, source_sequence);
        assert_eq!(restored.last_insert_rowid(), 501);
    }

    #[test]
    fn sql_values_round_trip_utf8_quotes_nul_invalid_text_blob_and_real() {
        let source = migrated_connection();
        source
            .execute_batch(
                "
                CREATE TABLE edge_values (
                    id INTEGER PRIMARY KEY,
                    safe_text TEXT,
                    nul_text TEXT,
                    invalid_text TEXT,
                    payload BLOB,
                    real_value REAL
                );
                ",
            )
            .expect("edge value table should create");
        let safe_text = "Знание O'Hara";
        let nul_text = "nul\0bytes";
        let payload = [0_u8, 0xff, b'\'', 0x80];
        let real_value = 1.234_567_890_123_456_7_f64;
        source
            .execute(
                "
                INSERT INTO edge_values (
                    id, safe_text, nul_text, invalid_text, payload, real_value
                ) VALUES (1, ?1, ?2, CAST(X'80FF27' AS TEXT), ?3, ?4);
                ",
                params![safe_text, nul_text, payload, real_value],
            )
            .expect("edge values should insert");

        let dump = build_sql_dump(&source).expect("sql dump should build");
        assert!(dump.contains("'Знание O''Hara'"));
        assert!(dump.contains("CAST(X'6E756C006279746573' AS TEXT)"));
        assert!(dump.contains("CAST(X'80FF27' AS TEXT)"));
        assert!(dump.contains("X'00FF2780'"));

        let restored = restore_sql_dump(&dump);
        assert_eq!(
            table_rows(&restored, "edge_values"),
            table_rows(&source, "edge_values")
        );
        let storage_types: (String, String, String, String, String) = restored
            .query_row(
                "
                SELECT typeof(safe_text), typeof(nul_text), typeof(invalid_text),
                       typeof(payload), typeof(real_value)
                FROM edge_values WHERE id = 1;
                ",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("restored storage classes should read");
        assert_eq!(
            storage_types,
            (
                "text".into(),
                "text".into(),
                "text".into(),
                "blob".into(),
                "real".into()
            )
        );
    }

    #[test]
    fn sql_dump_is_byte_deterministic_for_unchanged_state() {
        let connection = migrated_connection();
        connection
            .execute_batch(
                "
                INSERT INTO nodes (id, node_type, status, title, summary, body)
                VALUES (9, 'workflow', 'active', 'deterministic', 'same', 'state');
                INSERT INTO aliases (id, node_id, alias)
                VALUES (20, 9, 'second'), (10, 9, 'first');
                ",
            )
            .expect("deterministic fixture should insert");

        let first = build_sql_dump(&connection).expect("first dump should build");
        let second = build_sql_dump(&connection).expect("second dump should build");

        assert_eq!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn large_text_dump_never_sends_an_unbounded_write_chunk() {
        let connection = migrated_connection();
        let body = "x".repeat(1024 * 1024 + 257);
        connection
            .execute(
                "INSERT INTO nodes (node_type, status, title, body) VALUES ('fact', 'active', 'large', ?1);",
                [&body],
            )
            .expect("large body should insert");
        let mut writer = MaxChunkWriter::default();

        write_sql_dump(&mut writer, &connection).expect("large dump should stream");

        assert!(writer.bytes_written > body.len());
        assert!(
            writer.max_chunk_bytes <= SQL_WRITE_CHUNK_BYTES,
            "maximum write was {} bytes",
            writer.max_chunk_bytes
        );
    }

    #[test]
    fn stage_019_repair_is_read_only_redacted_and_idempotent() {
        let root = temp_path("stage-019-repair");
        let audit_git_dir = root.join("audit-git");
        let db_path = root.join("memory.sqlite");
        fs::create_dir_all(&audit_git_dir).expect("audit dir should create");
        let mut writer = Connection::open(&db_path).expect("fixture DB should open");
        schema::apply_migrations(&mut writer).expect("migrations should apply");
        writer
            .execute(
                "INSERT INTO nodes (id, node_type, status, title, body)
                 VALUES (1, 'fact', 'active', 'secret', 'STAGE019_SECRET');",
                [],
            )
            .expect("secret node should insert");
        writer
            .execute(
                "INSERT INTO tags (node_id, tag) VALUES (1, 'sensitivity:test_secret');",
                [],
            )
            .expect("secret tag should insert");
        drop(writer);
        let before = fs::read(&db_path).expect("DB bytes should read");
        let canonical_db_path = db_path.canonicalize().expect("DB path should canonicalize");
        let reader = Connection::open_with_flags(
            &canonical_db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .expect("read-only DB should open");

        ensure_pending_snapshot_marker(&audit_git_dir).expect("marker should create");
        let first = repair_sql_snapshot(&audit_git_dir, &reader).expect("repair should succeed");
        assert_eq!(first.status, AuditRepairStatus::Repaired);
        assert_eq!(first.git_commit, Some(GitCommitOutcome::Created));
        assert!(!first.marker_present_after);
        let snapshot = fs::read_to_string(audit_git_dir.join(SNAPSHOT_FILE_NAME))
            .expect("snapshot should read");
        assert!(!snapshot.contains("STAGE019_SECRET"));
        assert!(snapshot.contains("<TEST_SECRET_REDACTED>"));
        assert_eq!(fs::read(&db_path).expect("DB bytes should reread"), before);
        assert_eq!(
            reader
                .query_row("PRAGMA query_only;", [], |row| row.get::<_, i64>(0))
                .expect("query_only should read"),
            1
        );

        ensure_pending_snapshot_marker(&audit_git_dir).expect("marker should recreate");
        let second = repair_sql_snapshot(&audit_git_dir, &reader).expect("replay should succeed");
        assert_eq!(second.git_commit, Some(GitCommitOutcome::Unchanged));
        assert_eq!(second.sha256, first.sha256);
        let clean = repair_sql_snapshot(&audit_git_dir, &reader).expect("clean replay should pass");
        assert_eq!(clean.status, AuditRepairStatus::AlreadyClean);
        assert_eq!(clean.bytes_written, 0);
        assert_eq!(clean.git_commit, None);
        assert_eq!(fs::read(&db_path).expect("DB bytes should reread"), before);
        fs::remove_dir_all(root).expect("fixture should remove");
    }

    #[test]
    fn stage_019_core_failures_retain_pending_marker() {
        fn fresh_case(name: &str) -> (PathBuf, Connection, SnapshotLock) {
            let path = temp_path(name);
            fs::create_dir_all(&path).expect("audit root should create");
            let connection = migrated_connection();
            ensure_pending_snapshot_marker(&path).expect("marker should create");
            let lock = acquire_snapshot_lock(&path).expect("snapshot lock should acquire");
            (path, connection, lock)
        }

        let (publish_path, publish_connection, publish_lock) = fresh_case("stage-019-publish-87");
        let publish_error = repair_pending_snapshot_with_hooks_locked(
            &publish_connection,
            &publish_lock,
            |_root, _temporary, _name| Err(io::Error::from_raw_os_error(87)),
            |_root, _digest| Ok(()),
            |_root| Ok(GitCommitOutcome::Created),
        )
        .expect_err("publish error 87 should fail");
        assert_eq!(
            snapshot_io_error_for_test(&publish_error).and_then(io::Error::raw_os_error),
            Some(87)
        );
        assert!(pending_snapshot_marker_locked(&publish_lock).expect("marker should inspect"));
        drop(publish_lock);
        fs::remove_dir_all(publish_path).expect("publish case should remove");

        let (digest_path, digest_connection, digest_lock) = fresh_case("stage-019-digest-mismatch");
        repair_pending_snapshot_with_hooks_locked(
            &digest_connection,
            &digest_lock,
            atomic_publish,
            |_root, _digest| Err(io::Error::other("injected digest mismatch")),
            |_root| Ok(GitCommitOutcome::Created),
        )
        .expect_err("digest mismatch should fail");
        assert!(pending_snapshot_marker_locked(&digest_lock).expect("marker should inspect"));
        drop(digest_lock);
        fs::remove_dir_all(digest_path).expect("digest case should remove");

        let (git_path, git_connection, git_lock) = fresh_case("stage-019-git-failure");
        repair_pending_snapshot_with_hooks_locked(
            &git_connection,
            &git_lock,
            atomic_publish,
            |_root, _digest| Ok(()),
            |_root| Err(io::Error::other("injected Git failure")),
        )
        .expect_err("Git failure should fail");
        assert!(pending_snapshot_marker_locked(&git_lock).expect("marker should inspect"));
        drop(git_lock);
        fs::remove_dir_all(git_path).expect("Git case should remove");

        let (clear_path, _clear_connection, clear_lock) = fresh_case("stage-019-clear-failure");
        finish_repair_locked(&clear_lock, |lock| {
            clear_pending_snapshot_marker_locked(lock)?;
            Err(io::Error::other("injected clear durability failure").into())
        })
        .expect_err("clear failure should fail");
        assert!(pending_snapshot_marker_locked(&clear_lock).expect("marker should restore"));
        drop(clear_lock);
        fs::remove_dir_all(clear_path).expect("clear case should remove");
    }

    #[test]
    fn stage_020_normal_snapshot_restores_marker_after_post_remove_clear_failure() {
        let audit_git_dir = temp_path("stage-020-normal-clear-failure");
        fs::create_dir_all(&audit_git_dir).expect("audit root should create");
        let connection = migrated_connection();
        let lock = acquire_snapshot_lock(&audit_git_dir).expect("snapshot lock should acquire");

        let error = write_sql_snapshot_with_hooks_locked(
            &audit_git_dir,
            &connection,
            &lock,
            create_temporary_snapshot,
            write_sql_dump,
            atomic_publish,
            |lock| {
                clear_pending_snapshot_marker_locked(lock)?;
                assert!(
                    !pending_snapshot_marker_locked(lock)?,
                    "injected failure must happen after marker removal"
                );
                Err(io::Error::other("injected post-remove durability failure").into())
            },
        )
        .expect_err("post-remove clear failure should fail");

        assert!(error
            .to_string()
            .contains("injected post-remove durability failure"));
        assert!(
            pending_snapshot_marker_locked(&lock).expect("restored marker should inspect"),
            "every failed normal snapshot must retain or restore its marker"
        );
        assert!(
            fs::read_to_string(audit_git_dir.join(SNAPSHOT_FILE_NAME))
                .expect("committed snapshot should read")
                .contains("COMMIT;"),
            "snapshot publication should remain committed"
        );
        assert_eq!(
            anchored_git::commit_snapshot(lock.audit_root())
                .expect("replaying committed Git snapshot should succeed"),
            GitCommitOutcome::Unchanged,
            "Git audit commit must already be durable before marker clear"
        );

        drop(lock);
        fs::remove_dir_all(audit_git_dir).expect("fixture should remove");
    }

    fn snapshot_io_error_for_test(error: &SnapshotError) -> Option<&io::Error> {
        match error {
            SnapshotError::Io(error) => Some(error),
            SnapshotError::Db(_) | SnapshotError::Redaction(_) => None,
        }
    }

    #[test]
    fn writer_failure_keeps_old_snapshot_marker_and_removes_temporary_file() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("writer-failure");
        fs::create_dir_all(&audit_git_dir).expect("audit dir should create");
        let snapshot_path = audit_git_dir.join(SNAPSHOT_FILE_NAME);
        fs::write(&snapshot_path, b"known-good snapshot\n").expect("old snapshot should write");

        let error = write_sql_snapshot_with_hooks(
            &audit_git_dir,
            &connection,
            create_temporary_snapshot,
            |writer, _connection| {
                writer.write_all(b"partial dump")?;
                Err(io::Error::other("injected writer failure").into())
            },
            atomic_publish,
        )
        .expect_err("writer failure should fail snapshot");

        assert!(error.to_string().contains("injected writer failure"));
        assert_eq!(
            fs::read(&snapshot_path).expect("old snapshot should read"),
            b"known-good snapshot\n"
        );
        assert!(has_pending_snapshot(&audit_git_dir).expect("marker should read"));
        assert!(!has_temporary_snapshot(&audit_git_dir));
        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should remove");
    }

    #[test]
    fn temporary_file_creation_failure_keeps_old_snapshot_and_marker() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("temporary-file-failure");
        fs::create_dir_all(&audit_git_dir).expect("audit dir should create");
        let snapshot_path = audit_git_dir.join(SNAPSHOT_FILE_NAME);
        fs::write(&snapshot_path, b"known-good snapshot\n").expect("old snapshot should write");

        let error = write_sql_snapshot_with_hooks(
            &audit_git_dir,
            &connection,
            |_root, _name| Err(io::Error::other("injected temp creation failure")),
            write_sql_dump,
            atomic_publish,
        )
        .expect_err("temporary file failure should fail snapshot");

        assert!(error.to_string().contains("injected temp creation failure"));
        assert_eq!(
            fs::read(&snapshot_path).expect("old snapshot should read"),
            b"known-good snapshot\n"
        );
        assert!(has_pending_snapshot(&audit_git_dir).expect("marker should read"));
        assert!(!has_temporary_snapshot(&audit_git_dir));
        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should remove");
    }

    #[test]
    fn publish_failure_keeps_old_snapshot_marker_and_removes_temporary_file() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("publish-failure");
        fs::create_dir_all(&audit_git_dir).expect("audit dir should create");
        let snapshot_path = audit_git_dir.join(SNAPSHOT_FILE_NAME);
        fs::write(&snapshot_path, b"known-good snapshot\n").expect("old snapshot should write");

        let error = write_sql_snapshot_with_hooks(
            &audit_git_dir,
            &connection,
            create_temporary_snapshot,
            write_sql_dump,
            |_root, _temporary, _name| Err(io::Error::other("injected publish failure")),
        )
        .expect_err("publish failure should fail snapshot");

        assert!(error.to_string().contains("injected publish failure"));
        assert_eq!(
            fs::read(&snapshot_path).expect("old snapshot should read"),
            b"known-good snapshot\n"
        );
        assert!(has_pending_snapshot(&audit_git_dir).expect("marker should read"));
        assert!(!has_temporary_snapshot(&audit_git_dir));
        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should remove");
    }

    #[test]
    fn writes_sql_snapshot_as_text_file_under_audit_git_dir() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("audit-git");
        fs::create_dir_all(&audit_git_dir).expect("audit dir should be created");
        fs::write(audit_git_dir.join(SNAPSHOT_FILE_NAME), "old snapshot")
            .expect("old snapshot should write");

        let report =
            write_sql_snapshot(&audit_git_dir, &connection).expect("snapshot should be written");
        let snapshot_text =
            fs::read_to_string(&report.path).expect("snapshot file should be readable as text");

        assert_eq!(report.path, audit_git_dir.join(SNAPSHOT_FILE_NAME));
        assert_eq!(
            report.bytes_written,
            fs::metadata(&report.path)
                .expect("snapshot metadata should read")
                .len()
        );
        assert!(
            serde_json::to_value(&report).expect("snapshot report should serialize")["duration_ms"]
                .is_u64()
        );
        assert!(snapshot_text.contains("BEGIN TRANSACTION;"));
        assert!(snapshot_text.contains("CREATE TABLE schema_migrations"));
        assert_ne!(snapshot_text, "old snapshot");
        assert!(
            !has_pending_snapshot(&audit_git_dir).expect("pending marker should be readable"),
            "successful snapshot must clear its pending marker"
        );
        assert!(
            fs::read_dir(&audit_git_dir)
                .expect("audit dir should list")
                .all(|entry| {
                    !entry
                        .expect("audit entry should read")
                        .file_name()
                        .to_string_lossy()
                        .ends_with(".tmp")
                }),
            "successful snapshot must not leave a temporary file"
        );

        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should be removed");
    }

    #[test]
    fn snapshot_initializes_audit_git_with_local_deterministic_author() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("git-init");

        write_sql_snapshot(&audit_git_dir, &connection).expect("snapshot should be committed");

        assert!(
            audit_git_dir.join(".git").is_dir(),
            "snapshot should initialize a local git repository"
        );
        assert_eq!(
            git_stdout_for_test(&audit_git_dir, &["config", "--local", "--get", "user.name"]),
            AUDIT_GIT_AUTHOR_NAME
        );
        assert_eq!(
            git_stdout_for_test(
                &audit_git_dir,
                &["config", "--local", "--get", "user.email"]
            ),
            AUDIT_GIT_AUTHOR_EMAIL
        );
        assert_eq!(
            git_stdout_for_test(
                &audit_git_dir,
                &["show", "-s", "--format=%an <%ae>", "HEAD"]
            ),
            format!("{AUDIT_GIT_AUTHOR_NAME} <{AUDIT_GIT_AUTHOR_EMAIL}>")
        );
        assert_eq!(
            git_stdout_for_test(&audit_git_dir, &["show", "-s", "--format=%s", "HEAD"]),
            AUDIT_GIT_COMMIT_MESSAGE
        );
        assert_eq!(
            git_stdout_for_test(
                &audit_git_dir,
                &["show", "--format=", "--name-only", "HEAD"]
            ),
            SNAPSHOT_FILE_NAME
        );
        let committed_snapshot = run_git_for_test(
            &audit_git_dir,
            &["show", &format!("HEAD:{SNAPSHOT_FILE_NAME}")],
        );
        assert!(committed_snapshot.status.success());
        assert_eq!(
            committed_snapshot.stdout,
            fs::read(audit_git_dir.join(SNAPSHOT_FILE_NAME))
                .expect("published snapshot should read")
        );
        git_success_for_test(&audit_git_dir, &["fsck", "--full"]);
        assert!(
            audit_git_dir.join(SNAPSHOT_LOCK_FILE_NAME).is_file(),
            "snapshot lock must remain as a permanent inode"
        );

        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should be removed");
    }

    #[test]
    fn snapshot_commits_only_changed_memory_sql_and_preserves_other_staged_files() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("git-stage");

        write_sql_snapshot(&audit_git_dir, &connection).expect("first snapshot should commit");
        let first_head = git_stdout_for_test(&audit_git_dir, &["rev-parse", "HEAD"]);
        git_success_for_test(&audit_git_dir, &["read-tree", "HEAD"]);

        fs::write(audit_git_dir.join("unrelated.txt"), "keep staged content")
            .expect("unrelated file should write");
        git_success_for_test(&audit_git_dir, &["add", "--", "unrelated.txt"]);
        let index_before =
            fs::read(audit_git_dir.join(".git/index")).expect("staged index bytes should read");

        write_sql_snapshot(&audit_git_dir, &connection)
            .expect("unchanged snapshot should not create a commit");
        assert_eq!(
            git_stdout_for_test(&audit_git_dir, &["rev-parse", "HEAD"]),
            first_head,
            "unchanged memory.sql must not create another commit"
        );
        assert_eq!(
            git_stdout_for_test(&audit_git_dir, &["diff", "--cached", "--name-only"]),
            "unrelated.txt",
            "snapshot must not consume another staged file"
        );
        assert_eq!(
            fs::read(audit_git_dir.join(".git/index")).expect("index should still read"),
            index_before,
            "no-op audit must not rewrite the index"
        );

        connection
            .execute(
                "
                INSERT INTO nodes (
                    node_type, status, title, summary, body,
                    source_ref, confidence, trust_level
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);
                ",
                params![
                    "fact",
                    "active",
                    "git snapshot change",
                    "summary",
                    "body",
                    "source=test",
                    0.9_f64,
                    "high"
                ],
            )
            .expect("node should insert");

        write_sql_snapshot(&audit_git_dir, &connection).expect("changed snapshot should commit");
        assert_ne!(
            git_stdout_for_test(&audit_git_dir, &["rev-parse", "HEAD"]),
            first_head
        );
        assert_eq!(
            git_stdout_for_test(&audit_git_dir, &["rev-list", "--count", "HEAD"]),
            "2"
        );
        assert_eq!(
            git_stdout_for_test(
                &audit_git_dir,
                &["show", "--format=", "--name-only", "HEAD"]
            ),
            SNAPSHOT_FILE_NAME,
            "audit commit must contain only memory.sql"
        );
        let staged_names =
            git_stdout_for_test(&audit_git_dir, &["diff", "--cached", "--name-only"]);
        assert!(
            staged_names.lines().any(|name| name == "unrelated.txt"),
            "audit commit must preserve another staged file"
        );
        assert_eq!(
            fs::read(audit_git_dir.join(".git/index")).expect("index should still read"),
            index_before,
            "changed audit commit must not rewrite the index"
        );

        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should be removed");
    }

    #[test]
    fn snapshot_preserves_other_head_tree_entries() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("git-preserve-head-tree");
        write_sql_snapshot(&audit_git_dir, &connection).expect("first snapshot should commit");

        fs::write(audit_git_dir.join("preserved.txt"), b"preserve from HEAD\n")
            .expect("preserved file should write");
        git_success_for_test(&audit_git_dir, &["add", "--", "preserved.txt"]);
        git_success_for_test(
            &audit_git_dir,
            &["commit", "--quiet", "-m", "test: preserved tree entry"],
        );

        connection
            .execute(
                "INSERT INTO nodes (node_type, status, title, body) VALUES ('fact', 'active', 'changed', 'body');",
                [],
            )
            .expect("changed node should insert");
        write_sql_snapshot(&audit_git_dir, &connection).expect("changed snapshot should commit");

        assert_eq!(
            git_stdout_for_test(&audit_git_dir, &["show", "HEAD:preserved.txt"]),
            "preserve from HEAD"
        );
        assert_eq!(
            git_stdout_for_test(
                &audit_git_dir,
                &["show", "--format=", "--name-only", "HEAD"]
            ),
            SNAPSHOT_FILE_NAME
        );
        git_success_for_test(&audit_git_dir, &["fsck", "--full"]);
        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should remove");
    }

    #[test]
    fn snapshot_reads_pack_only_parent_and_tree_without_path_based_writes() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("git-packed-parent");
        write_sql_snapshot(&audit_git_dir, &connection).expect("first snapshot should commit");
        git_success_for_test(&audit_git_dir, &["gc", "--prune=now"]);
        let count_objects = git_stdout_for_test(&audit_git_dir, &["count-objects", "-v"]);
        assert!(
            count_objects
                .lines()
                .any(|line| line.starts_with("packs: ") && line != "packs: 0"),
            "fixture should contain a pack: {count_objects}"
        );

        connection
            .execute(
                "INSERT INTO nodes (node_type, status, title) VALUES ('fact', 'active', 'after gc');",
                [],
            )
            .expect("post-gc node should insert");
        write_sql_snapshot(&audit_git_dir, &connection)
            .expect("packed parent and tree should remain compatible");

        assert_eq!(
            git_stdout_for_test(&audit_git_dir, &["rev-list", "--count", "HEAD"]),
            "2"
        );
        git_success_for_test(&audit_git_dir, &["fsck", "--full"]);
        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should remove");
    }

    #[test]
    fn corrupted_existing_head_fails_closed_without_reinitializing_git() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("git-corrupt-head");
        write_sql_snapshot(&audit_git_dir, &connection).expect("first snapshot should commit");
        let config_before =
            fs::read(audit_git_dir.join(".git/config")).expect("existing git config should read");
        fs::write(audit_git_dir.join(".git/HEAD"), b"not a valid HEAD\n")
            .expect("corrupt HEAD should write");

        connection
            .execute(
                "INSERT INTO nodes (node_type, status, title, body) VALUES ('fact', 'active', 'corrupt-head-change', 'body');",
                [],
            )
            .expect("changed node should insert");
        let error = write_sql_snapshot(&audit_git_dir, &connection)
            .expect_err("corrupt existing HEAD must fail closed");

        assert!(
            error.to_string().contains("local git audit could not"),
            "error should retain Git context: {error}"
        );
        assert!(audit_git_dir.join(".git").is_dir());
        assert_eq!(
            fs::read(audit_git_dir.join(".git/HEAD")).expect("corrupt HEAD should remain"),
            b"not a valid HEAD\n"
        );
        assert_eq!(
            fs::read(audit_git_dir.join(".git/config")).expect("existing git config should remain"),
            config_before,
            "failure must not reinitialize or rewrite the repository"
        );
        assert!(has_pending_snapshot(&audit_git_dir).expect("marker should read"));
        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should remove");
    }

    #[test]
    fn concurrent_snapshot_writers_serialize_on_permanent_lock() {
        let audit_git_dir = temp_path("concurrent-snapshot-lock");
        let start = Arc::new(Barrier::new(3));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();

        for _ in 0..2 {
            let audit_git_dir = audit_git_dir.clone();
            let start = Arc::clone(&start);
            let active = Arc::clone(&active);
            let maximum = Arc::clone(&maximum);
            threads.push(thread::spawn(move || {
                let connection = migrated_connection();
                start.wait();
                write_sql_snapshot_with_hooks(
                    &audit_git_dir,
                    &connection,
                    create_temporary_snapshot,
                    |writer, connection| {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(current, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(75));
                        let result = write_sql_dump(writer, connection);
                        active.fetch_sub(1, Ordering::SeqCst);
                        result
                    },
                    atomic_publish,
                )
                .expect("serialized snapshot should succeed");
            }));
        }

        start.wait();
        for thread in threads {
            thread.join().expect("snapshot thread should join");
        }

        assert_eq!(maximum.load(Ordering::SeqCst), 1);
        assert!(audit_git_dir.join(SNAPSHOT_LOCK_FILE_NAME).is_file());
        assert!(!has_pending_snapshot(&audit_git_dir).expect("marker should read"));
        assert!(!has_temporary_snapshot(&audit_git_dir));
        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should remove");
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_root_swap_cannot_publish_or_commit_into_outside_directory() {
        use std::os::unix::fs::symlink;

        let connection = migrated_connection();
        let audit_git_dir = temp_path("root-swap");
        let moved_audit_dir = temp_path("root-swap-moved");
        let outside = temp_path("root-swap-outside");
        fs::create_dir(&outside).expect("outside directory should create");
        let sentinel = outside.join("sentinel.txt");
        fs::write(&sentinel, b"outside audit sentinel\n").expect("sentinel should write");

        write_sql_snapshot_with_hooks(
            &audit_git_dir,
            &connection,
            create_temporary_snapshot,
            write_sql_dump,
            |root, temporary, name| {
                atomic_publish(root, temporary, name)?;
                fs::rename(&audit_git_dir, &moved_audit_dir)?;
                symlink(&outside, &audit_git_dir)?;
                Ok(())
            },
        )
        .expect_err("changed audit root identity must fail closed before Git publication");

        assert!(moved_audit_dir.join(SNAPSHOT_FILE_NAME).is_file());
        assert!(moved_audit_dir
            .join(PENDING_SNAPSHOT_MARKER_FILE_NAME)
            .is_file());
        assert!(!outside.join(SNAPSHOT_FILE_NAME).exists());
        assert!(!outside.join(".git").exists());
        assert_eq!(
            fs::read(&sentinel).expect("outside sentinel should read"),
            b"outside audit sentinel\n"
        );
        fs::remove_file(&audit_git_dir).expect("replacement symlink should remove");
        fs::remove_dir_all(&moved_audit_dir).expect("moved audit root should remove");
        fs::remove_dir_all(&outside).expect("outside directory should remove");
    }

    #[test]
    fn production_audit_runtime_never_launches_external_git() {
        let source = include_str!("mod.rs");
        let production = source
            .split_once("#[cfg(test)]\nmod tests")
            .expect("audit test boundary should exist")
            .0;
        assert!(!production.contains("Command::new(\"git\")"));
        assert!(!production.contains("std::process::Command"));
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_preflight_rejects_links_for_every_managed_audit_entry() {
        use std::os::unix::fs::symlink;

        for managed_name in [
            SNAPSHOT_FILE_NAME,
            SNAPSHOT_LOCK_FILE_NAME,
            PENDING_SNAPSHOT_MARKER_FILE_NAME,
            ".git",
        ] {
            let audit_git_dir = temp_path(&format!("managed-link-{managed_name}"));
            let outside = temp_path(&format!("managed-link-outside-{managed_name}"));
            fs::create_dir(&audit_git_dir).expect("audit root should create");
            fs::write(&outside, b"outside managed sentinel\n")
                .expect("outside sentinel should write");
            symlink(&outside, audit_git_dir.join(managed_name))
                .expect("managed entry symlink should create");

            let error = acquire_snapshot_lock(&audit_git_dir)
                .expect_err("managed audit entry links must fail closed");

            assert_eq!(
                fs::read(&outside).expect("outside sentinel should remain"),
                b"outside managed sentinel\n"
            );
            assert!(error
                .to_string()
                .contains("unsafe persistent workspace path"));
            fs::remove_dir_all(&audit_git_dir).expect("audit fixture should remove");
            fs::remove_file(outside).expect("outside sentinel should remove");
        }
    }

    #[test]
    fn snapshot_keeps_atomic_file_when_audit_git_initialization_fails() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("git-failure");
        fs::create_dir_all(&audit_git_dir).expect("audit dir should be created");
        fs::write(audit_git_dir.join(".git"), "not a git dir")
            .expect("invalid git file should write");

        let error = write_sql_snapshot(&audit_git_dir, &connection)
            .expect_err("invalid git metadata should fail after snapshot publication");

        assert!(
            error
                .to_string()
                .contains("local git audit metadata is not a directory"),
            "git error should identify invalid existing metadata: {error}"
        );
        assert!(
            fs::read_to_string(audit_git_dir.join(SNAPSHOT_FILE_NAME))
                .expect("published snapshot should remain readable")
                .contains("BEGIN TRANSACTION;"),
            "git failure must not remove the atomically published snapshot"
        );
        assert!(
            has_pending_snapshot(&audit_git_dir).expect("pending marker should be readable"),
            "git failure must leave the pending marker for recovery"
        );
        assert!(
            fs::read_dir(&audit_git_dir)
                .expect("audit dir should list")
                .all(|entry| {
                    !entry
                        .expect("audit entry should read")
                        .file_name()
                        .to_string_lossy()
                        .ends_with(".tmp")
                }),
            "git failure must not leave a snapshot temporary file"
        );

        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should be removed");
    }

    #[test]
    fn snapshot_failure_never_truncates_an_existing_pending_marker() {
        let connection = migrated_connection();
        let audit_git_dir = temp_path("existing-marker-failure");
        fs::create_dir_all(&audit_git_dir).expect("audit dir should create");
        let marker_path = audit_git_dir.join(PENDING_SNAPSHOT_MARKER_FILE_NAME);
        fs::write(&marker_path, b"older pending mutation\n")
            .expect("existing marker fixture should write");
        fs::write(audit_git_dir.join(".git"), b"invalid git metadata")
            .expect("invalid git fixture should write");

        write_sql_snapshot(&audit_git_dir, &connection)
            .expect_err("invalid git metadata should fail snapshot commit");

        assert_eq!(
            fs::read(&marker_path).expect("existing marker should read"),
            b"older pending mutation\n"
        );
        fs::remove_dir_all(&audit_git_dir).expect("temp audit dir should be removed");
    }
}
