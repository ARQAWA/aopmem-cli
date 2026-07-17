use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;

use super::WorkspaceSchemaPlan;
use crate::audit::AnchoredDir;

const BACKUP_PAGE_BATCH: i32 = 256;
const BACKUP_BUSY_TIMEOUT: Duration = Duration::from_secs(30);
const BACKUP_BUSY_PAUSE: Duration = Duration::from_millis(10);
const TEMPORARY_NAME_ATTEMPTS: usize = 8;
const RUN_ROOT_NAME_ATTEMPTS: usize = 8;
const REPRESENTATIVE_TABLES: &[&str] = &[
    "schema_migrations",
    "nodes",
    "links",
    "aliases",
    "tags",
    "sources",
    "events",
    "registries",
    "tool_contracts",
    "mcp_profiles",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupPhase {
    CreateBackupRoot,
    CreateTemporaryDatabase,
    OpenSourceDatabase,
    OpenDestinationDatabase,
    SqliteOnlineBackup,
    CloseSqliteHandles,
    ValidateTemporaryDatabase,
    FlushTemporaryFile,
    PublishBackup,
    ValidatePublishedDatabase,
    FinalizeBackupMetadata,
}

impl std::fmt::Display for BackupPhase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_value(self).map_err(|_| std::fmt::Error)?;
        formatter.write_str(value.as_str().ok_or(std::fmt::Error)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WorkspaceBackupFailureDetails {
    pub workspace_key: String,
    pub backup_phase: BackupPhase,
    pub source_path: Option<String>,
    pub temporary_path: Option<String>,
    pub final_path: Option<String>,
    pub raw_os_error: Option<i32>,
    pub io_kind: String,
    pub partial_file_exists: bool,
    pub partial_file_size: Option<u64>,
    pub partial_file_validated: bool,
    pub migration_started: bool,
    pub fix_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BackupArtifact {
    pub final_path: PathBuf,
    pub bytes: u64,
    pub schema: WorkspaceSchemaPlan,
}

#[derive(Debug, Error)]
#[error("workspace backup failed during {phase}: {source}")]
pub(super) struct BackupError {
    phase: BackupPhase,
    source_path: Option<PathBuf>,
    temporary_path: Option<PathBuf>,
    final_path: Option<PathBuf>,
    partial_file_validated: bool,
    #[source]
    source: io::Error,
}

impl BackupError {
    pub(super) fn from_io(
        phase: BackupPhase,
        source_path: Option<&Path>,
        temporary_path: Option<&Path>,
        final_path: Option<&Path>,
        partial_file_validated: bool,
        source: io::Error,
    ) -> Self {
        Self {
            phase,
            source_path: source_path.map(Path::to_path_buf),
            temporary_path: temporary_path.map(Path::to_path_buf),
            final_path: final_path.map(Path::to_path_buf),
            partial_file_validated,
            source,
        }
    }

    pub(super) fn details(
        &self,
        workspace_key: &str,
        migration_started: bool,
    ) -> WorkspaceBackupFailureDetails {
        let partial_path = self
            .temporary_path
            .as_deref()
            .filter(|path| path.exists())
            .or_else(|| self.final_path.as_deref().filter(|path| path.exists()));
        let partial_file_size = partial_path
            .and_then(|path| fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        WorkspaceBackupFailureDetails {
            workspace_key: workspace_key.to_string(),
            backup_phase: self.phase,
            source_path: self.source_path.as_deref().map(display_path),
            temporary_path: self.temporary_path.as_deref().map(display_path),
            final_path: self.final_path.as_deref().map(display_path),
            raw_os_error: self.source.raw_os_error(),
            io_kind: io_kind_name(self.source.kind()).to_string(),
            partial_file_exists: partial_path.is_some(),
            partial_file_size,
            partial_file_validated: self.partial_file_validated,
            migration_started,
            fix_hint: backup_fix_hint(self.phase).to_string(),
        }
    }
}

pub(super) trait BackupFaultInjector {
    fn check(&self, _phase: BackupPhase) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct NoBackupFaults;

impl BackupFaultInjector for NoBackupFaults {}

pub(super) fn create_unique_backup_run_root(backups: &Path, prefix: &str) -> io::Result<PathBuf> {
    for _ in 0..RUN_ROOT_NAME_ATTEMPTS {
        let id = random_id()?;
        let root = backups.join(format!("{prefix}{id}"));
        match crate::storage::create_new_owned_direct_directory(backups, &root) {
            Ok(()) => return Ok(root),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "cannot allocate a unique upgrade backup run root",
    ))
}

pub(super) fn online_backup_to_path_with_faults(
    source: Connection,
    source_path: &Path,
    destination_dir: &Path,
    final_path: &Path,
    expected_schema: &WorkspaceSchemaPlan,
    faults: &dyn BackupFaultInjector,
) -> Result<BackupArtifact, BackupError> {
    let destination_root = AnchoredDir::open_workspace(destination_dir, None).map_err(|error| {
        BackupError::from_io(
            BackupPhase::CreateTemporaryDatabase,
            Some(source_path),
            None,
            Some(final_path),
            false,
            error,
        )
    })?;
    let final_name = final_path.file_name().ok_or_else(|| {
        BackupError::from_io(
            BackupPhase::CreateTemporaryDatabase,
            Some(source_path),
            None,
            Some(final_path),
            false,
            io::Error::other("database backup has no file name"),
        )
    })?;
    let temporary_name = create_unique_temporary_database(&destination_root, final_name, faults)
        .map_err(|(temporary_name, error)| {
            let temporary_path = temporary_name.map(|name| destination_dir.join(Path::new(&name)));
            BackupError::from_io(
                BackupPhase::CreateTemporaryDatabase,
                Some(source_path),
                temporary_path.as_deref(),
                Some(final_path),
                false,
                error,
            )
        })?;
    let temporary_path = destination_dir.join(Path::new(&temporary_name));

    check_fault(
        faults,
        BackupPhase::OpenSourceDatabase,
        source_path,
        &temporary_path,
        final_path,
        false,
    )?;
    check_fault(
        faults,
        BackupPhase::OpenDestinationDatabase,
        source_path,
        &temporary_path,
        final_path,
        false,
    )?;
    let canonical_temporary = temporary_path.canonicalize().map_err(|error| {
        backup_error(
            BackupPhase::OpenDestinationDatabase,
            source_path,
            &temporary_path,
            final_path,
            false,
            error,
        )
    })?;
    let mut destination = Connection::open_with_flags(
        canonical_temporary,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(|error| {
        backup_error(
            BackupPhase::OpenDestinationDatabase,
            source_path,
            &temporary_path,
            final_path,
            false,
            sqlite_io(error),
        )
    })?;
    destination
        .execute_batch("PRAGMA synchronous = FULL; PRAGMA journal_mode = DELETE;")
        .map_err(|error| {
            backup_error(
                BackupPhase::OpenDestinationDatabase,
                source_path,
                &temporary_path,
                final_path,
                false,
                sqlite_io(error),
            )
        })?;

    check_fault(
        faults,
        BackupPhase::SqliteOnlineBackup,
        source_path,
        &temporary_path,
        final_path,
        false,
    )?;
    run_bounded_backup(&source, &mut destination).map_err(|error| {
        backup_error(
            BackupPhase::SqliteOnlineBackup,
            source_path,
            &temporary_path,
            final_path,
            false,
            error,
        )
    })?;

    check_fault(
        faults,
        BackupPhase::CloseSqliteHandles,
        source_path,
        &temporary_path,
        final_path,
        false,
    )?;
    close_connections(destination, source).map_err(|error| {
        backup_error(
            BackupPhase::CloseSqliteHandles,
            source_path,
            &temporary_path,
            final_path,
            false,
            error,
        )
    })?;

    check_fault(
        faults,
        BackupPhase::ValidateTemporaryDatabase,
        source_path,
        &temporary_path,
        final_path,
        false,
    )?;
    let temporary_schema = validate_database(&temporary_path).map_err(|error| {
        backup_error(
            BackupPhase::ValidateTemporaryDatabase,
            source_path,
            &temporary_path,
            final_path,
            false,
            error,
        )
    })?;
    if &temporary_schema != expected_schema {
        return Err(backup_error(
            BackupPhase::ValidateTemporaryDatabase,
            source_path,
            &temporary_path,
            final_path,
            false,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "temporary backup schema identity differs from source",
            ),
        ));
    }

    check_fault(
        faults,
        BackupPhase::FlushTemporaryFile,
        source_path,
        &temporary_path,
        final_path,
        true,
    )?;
    let temporary_file = destination_root
        .open_regular_for_update_os(&temporary_name)
        .map_err(|error| {
            backup_error(
                BackupPhase::FlushTemporaryFile,
                source_path,
                &temporary_path,
                final_path,
                true,
                error,
            )
        })?;
    temporary_file.sync_all().map_err(|error| {
        backup_error(
            BackupPhase::FlushTemporaryFile,
            source_path,
            &temporary_path,
            final_path,
            true,
            error,
        )
    })?;

    check_fault(
        faults,
        BackupPhase::PublishBackup,
        source_path,
        &temporary_path,
        final_path,
        true,
    )?;
    let outcome = destination_root
        .publish_regular_no_replace_committed_os(&temporary_file, &temporary_name, final_name)
        .map_err(|error| {
            backup_error(
                BackupPhase::PublishBackup,
                source_path,
                &temporary_path,
                final_path,
                true,
                error,
            )
        })?;
    drop(temporary_file);
    if !outcome.durability_confirmed || !outcome.temporary_cleanup_confirmed {
        return Err(backup_error(
            BackupPhase::PublishBackup,
            source_path,
            &temporary_path,
            final_path,
            true,
            io::Error::other(
                "backup publish completed without confirmed durability and temporary cleanup",
            ),
        ));
    }

    check_fault(
        faults,
        BackupPhase::ValidatePublishedDatabase,
        source_path,
        &temporary_path,
        final_path,
        true,
    )?;
    let final_schema = validate_database(final_path).map_err(|error| {
        backup_error(
            BackupPhase::ValidatePublishedDatabase,
            source_path,
            &temporary_path,
            final_path,
            true,
            error,
        )
    })?;
    if final_schema != temporary_schema {
        return Err(backup_error(
            BackupPhase::ValidatePublishedDatabase,
            source_path,
            &temporary_path,
            final_path,
            true,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "published backup schema identity differs from validated temporary backup",
            ),
        ));
    }

    check_fault(
        faults,
        BackupPhase::FinalizeBackupMetadata,
        source_path,
        &temporary_path,
        final_path,
        true,
    )?;
    let metadata = fs::metadata(final_path).map_err(|error| {
        backup_error(
            BackupPhase::FinalizeBackupMetadata,
            source_path,
            &temporary_path,
            final_path,
            true,
            error,
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(backup_error(
            BackupPhase::FinalizeBackupMetadata,
            source_path,
            &temporary_path,
            final_path,
            true,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "published backup is not a non-empty regular file",
            ),
        ));
    }

    Ok(BackupArtifact {
        final_path: final_path.to_path_buf(),
        bytes: metadata.len(),
        schema: final_schema,
    })
}

fn create_unique_temporary_database(
    destination_root: &AnchoredDir,
    final_name: &OsStr,
    faults: &dyn BackupFaultInjector,
) -> Result<OsString, (Option<OsString>, io::Error)> {
    faults
        .check(BackupPhase::CreateTemporaryDatabase)
        .map_err(|error| (None, error))?;
    for _ in 0..TEMPORARY_NAME_ATTEMPTS {
        let name = temporary_database_name(final_name).map_err(|error| (None, error))?;
        match destination_root.create_new_regular_os(&name) {
            Ok(file) => {
                drop(file);
                return Ok(name);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err((Some(name), error)),
        }
    }
    Err((
        None,
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "cannot allocate a unique temporary backup database",
        ),
    ))
}

fn temporary_database_name(final_name: &OsStr) -> io::Result<OsString> {
    let final_name = final_name
        .to_str()
        .ok_or_else(|| io::Error::other("database backup file name is not valid Unicode"))?;
    let id = random_id()?;
    Ok(OsString::from(format!(".{final_name}.backup-{id}.tmp")))
}

fn random_id() -> io::Result<String> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| io::Error::other("random backup id failed"))?;
    let mut id = String::with_capacity(32);
    for byte in random {
        use std::fmt::Write as _;
        write!(id, "{byte:02x}").map_err(io::Error::other)?;
    }
    Ok(id)
}

fn check_fault(
    faults: &dyn BackupFaultInjector,
    phase: BackupPhase,
    source_path: &Path,
    temporary_path: &Path,
    final_path: &Path,
    partial_file_validated: bool,
) -> Result<(), BackupError> {
    faults.check(phase).map_err(|error| {
        backup_error(
            phase,
            source_path,
            temporary_path,
            final_path,
            partial_file_validated,
            error,
        )
    })
}

fn backup_error(
    phase: BackupPhase,
    source_path: &Path,
    temporary_path: &Path,
    final_path: &Path,
    partial_file_validated: bool,
    source: io::Error,
) -> BackupError {
    BackupError::from_io(
        phase,
        Some(source_path),
        Some(temporary_path),
        Some(final_path),
        partial_file_validated,
        source,
    )
}

fn run_bounded_backup(source: &Connection, destination: &mut Connection) -> io::Result<()> {
    let backup = Backup::new(source, destination).map_err(sqlite_io)?;
    let started = Instant::now();
    let result = loop {
        match backup.step(BACKUP_PAGE_BATCH).map_err(sqlite_io)? {
            StepResult::Done => break Ok(()),
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => {
                if started.elapsed() >= BACKUP_BUSY_TIMEOUT {
                    break Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "SQLite Online Backup remained busy for 30 seconds",
                    ));
                }
                thread::sleep(BACKUP_BUSY_PAUSE);
            }
            _ => {
                break Err(io::Error::other(
                    "SQLite Online Backup returned an unknown state",
                ));
            }
        }
    };
    drop(backup);
    result
}

fn close_connections(destination: Connection, source: Connection) -> io::Result<()> {
    let destination_result = close_connection(destination);
    let source_result = close_connection(source);
    destination_result.and(source_result)
}

fn close_connection(connection: Connection) -> io::Result<()> {
    connection.close().map_err(|(connection, error)| {
        drop(connection);
        sqlite_io(error)
    })
}

fn validate_database(path: &Path) -> io::Result<WorkspaceSchemaPlan> {
    let canonical = path.canonicalize()?;
    let connection = Connection::open_with_flags(
        canonical,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(sqlite_io)?;
    connection
        .execute_batch("PRAGMA query_only = ON; PRAGMA temp_store = MEMORY;")
        .map_err(sqlite_io)?;
    let schema = super::inspect_schema(&connection)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.message))?;
    for table in REPRESENTATIVE_TABLES {
        let count = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1;",
                [table],
                |row| row.get::<_, i64>(0),
            )
            .map_err(sqlite_io)?;
        if count != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("backup is missing representative table {table}"),
            ));
        }
    }
    close_connection(connection)?;
    Ok(schema)
}

fn sqlite_io(error: rusqlite::Error) -> io::Error {
    io::Error::other(error)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn io_kind_name(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::NotFound => "not_found",
        io::ErrorKind::PermissionDenied => "permission_denied",
        io::ErrorKind::AlreadyExists => "already_exists",
        io::ErrorKind::InvalidInput => "invalid_input",
        io::ErrorKind::InvalidData => "invalid_data",
        io::ErrorKind::TimedOut => "timed_out",
        io::ErrorKind::WriteZero => "write_zero",
        io::ErrorKind::StorageFull => "storage_full",
        io::ErrorKind::ReadOnlyFilesystem => "read_only_filesystem",
        _ => "other",
    }
}

fn backup_fix_hint(phase: BackupPhase) -> &'static str {
    match phase {
        BackupPhase::CreateBackupRoot | BackupPhase::CreateTemporaryDatabase => {
            "preserve every backup root and verify the AOPMem backup directory is writable"
        }
        BackupPhase::OpenSourceDatabase
        | BackupPhase::OpenDestinationDatabase
        | BackupPhase::SqliteOnlineBackup
        | BackupPhase::CloseSqliteHandles => {
            "preserve partial evidence, close active AOPMem processes, and use a newer verified binary for a new upgrade run"
        }
        BackupPhase::ValidateTemporaryDatabase
        | BackupPhase::FlushTemporaryFile
        | BackupPhase::PublishBackup
        | BackupPhase::ValidatePublishedDatabase
        | BackupPhase::FinalizeBackupMetadata => {
            "preserve partial evidence and use a newer verified binary for a new upgrade run"
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::schema;

    const FAULT_PHASES: &[BackupPhase] = &[
        BackupPhase::CreateTemporaryDatabase,
        BackupPhase::OpenSourceDatabase,
        BackupPhase::OpenDestinationDatabase,
        BackupPhase::SqliteOnlineBackup,
        BackupPhase::CloseSqliteHandles,
        BackupPhase::ValidateTemporaryDatabase,
        BackupPhase::FlushTemporaryFile,
        BackupPhase::PublishBackup,
        BackupPhase::ValidatePublishedDatabase,
        BackupPhase::FinalizeBackupMetadata,
    ];

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "aopmem-backup-{name}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test root should create");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct FailAt {
        phase: BackupPhase,
        visits: AtomicUsize,
    }

    impl BackupFaultInjector for FailAt {
        fn check(&self, phase: BackupPhase) -> io::Result<()> {
            self.visits.fetch_add(1, Ordering::SeqCst);
            if phase == self.phase {
                Err(io::Error::from_raw_os_error(5))
            } else {
                Ok(())
            }
        }
    }

    fn create_source(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        let mut connection = Connection::open(&path).expect("source database should open");
        schema::apply_migrations(&mut connection).expect("source schema should create");
        connection
            .execute(
                "INSERT INTO nodes (node_type, status, title, summary)
                 VALUES ('rule', 'active', 'backup proof', 'preserve source');",
                [],
            )
            .expect("source row should insert");
        connection
            .execute(
                "DELETE FROM schema_migrations WHERE version IN ('002', '003');",
                [],
            )
            .expect("source should emulate schema 001");
        drop(connection);
        path
    }

    fn open_source(path: &Path) -> Connection {
        let canonical = path
            .canonicalize()
            .expect("source path should canonicalize");
        Connection::open_with_flags(
            canonical,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .expect("source database should open read-only")
    }

    fn schema(path: &Path) -> WorkspaceSchemaPlan {
        let connection = open_source(path);
        let schema =
            super::super::inspect_schema(&connection).expect("source schema should inspect");
        drop(connection);
        schema
    }

    fn node_count(path: &Path) -> i64 {
        let connection = open_source(path);
        connection
            .query_row("SELECT COUNT(*) FROM nodes;", [], |row| row.get(0))
            .expect("node count should read")
    }

    #[test]
    fn backup_phase_json_contract_is_exact_and_complete() {
        let phases = [
            BackupPhase::CreateBackupRoot,
            BackupPhase::CreateTemporaryDatabase,
            BackupPhase::OpenSourceDatabase,
            BackupPhase::OpenDestinationDatabase,
            BackupPhase::SqliteOnlineBackup,
            BackupPhase::CloseSqliteHandles,
            BackupPhase::ValidateTemporaryDatabase,
            BackupPhase::FlushTemporaryFile,
            BackupPhase::PublishBackup,
            BackupPhase::ValidatePublishedDatabase,
            BackupPhase::FinalizeBackupMetadata,
        ];

        assert_eq!(
            serde_json::to_value(phases).expect("backup phases should serialize"),
            serde_json::json!([
                "create_backup_root",
                "create_temporary_database",
                "open_source_database",
                "open_destination_database",
                "sqlite_online_backup",
                "close_sqlite_handles",
                "validate_temporary_database",
                "flush_temporary_file",
                "publish_backup",
                "validate_published_database",
                "finalize_backup_metadata"
            ])
        );
    }

    #[test]
    fn online_backup_publishes_valid_final_database_and_preserves_source() {
        let root = TestRoot::new("success");
        let source_path = create_source(root.path(), "source.sqlite");
        let destination_dir = root.path().join("backups");
        fs::create_dir(&destination_dir).expect("backup directory should create");
        let final_path = destination_dir.join("aopmem.sqlite");
        let expected_schema = schema(&source_path);

        let artifact = online_backup_to_path_with_faults(
            open_source(&source_path),
            &source_path,
            &destination_dir,
            &final_path,
            &expected_schema,
            &NoBackupFaults,
        )
        .expect("backup should publish");

        assert_eq!(artifact.final_path, final_path);
        assert!(artifact.bytes > 0);
        assert_eq!(artifact.schema, expected_schema);
        assert_eq!(node_count(&source_path), 1);
        assert_eq!(node_count(&final_path), 1);
        assert_eq!(schema(&final_path), expected_schema);
        assert_eq!(
            fs::read_dir(&destination_dir)
                .expect("backup directory should read")
                .count(),
            1,
            "temporary database must be gone after publish"
        );
    }

    #[test]
    fn every_backup_phase_failure_preserves_exact_diagnostics_and_blocks_acceptance() {
        for phase in FAULT_PHASES {
            let root = TestRoot::new(&format!("fault-{phase}"));
            let source_path = create_source(root.path(), "source.sqlite");
            let destination_dir = root.path().join("backups");
            fs::create_dir(&destination_dir).expect("backup directory should create");
            let final_path = destination_dir.join("aopmem.sqlite");
            let expected_schema = schema(&source_path);
            let faults = FailAt {
                phase: *phase,
                visits: AtomicUsize::new(0),
            };

            let error = online_backup_to_path_with_faults(
                open_source(&source_path),
                &source_path,
                &destination_dir,
                &final_path,
                &expected_schema,
                &faults,
            )
            .expect_err("injected phase should fail");
            let details = error.details("workspace-proof", false);

            assert_eq!(details.workspace_key, "workspace-proof");
            assert_eq!(details.backup_phase, *phase);
            assert_eq!(details.raw_os_error, Some(5));
            assert!(!details.migration_started);
            assert!(!details.fix_hint.is_empty());
            assert!(faults.visits.load(Ordering::SeqCst) > 0);
            assert_eq!(node_count(&source_path), 1);
            assert!(
                !final_path.exists()
                    || matches!(
                        phase,
                        BackupPhase::ValidatePublishedDatabase
                            | BackupPhase::FinalizeBackupMetadata
                    ),
                "final path must not be accepted before publish"
            );
            if matches!(
                phase,
                BackupPhase::FlushTemporaryFile
                    | BackupPhase::PublishBackup
                    | BackupPhase::ValidatePublishedDatabase
                    | BackupPhase::FinalizeBackupMetadata
            ) {
                assert!(details.partial_file_exists);
                assert!(details.partial_file_validated);
                assert!(details.partial_file_size.is_some_and(|size| size > 0));
            }
        }
    }

    #[test]
    fn schema_identity_mismatch_keeps_validated_temporary_evidence() {
        let root = TestRoot::new("schema-mismatch");
        let source_path = create_source(root.path(), "source.sqlite");
        let destination_dir = root.path().join("backups");
        fs::create_dir(&destination_dir).expect("backup directory should create");
        let final_path = destination_dir.join("aopmem.sqlite");
        let mut wrong_schema = schema(&source_path);
        wrong_schema.current_version = "003".to_string();

        let error = online_backup_to_path_with_faults(
            open_source(&source_path),
            &source_path,
            &destination_dir,
            &final_path,
            &wrong_schema,
            &NoBackupFaults,
        )
        .expect_err("schema mismatch should fail");
        let details = error.details("workspace-proof", false);

        assert_eq!(details.backup_phase, BackupPhase::ValidateTemporaryDatabase);
        assert!(details.partial_file_exists);
        assert!(!details.partial_file_validated);
        assert!(!final_path.exists());
        assert_eq!(node_count(&source_path), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_open_sqlite_destination_handle_blocks_publish_until_closed() {
        let root = TestRoot::new("windows-open-destination");
        let destination_dir = root.path().join("backups");
        fs::create_dir(&destination_dir).expect("backup directory should create");
        let destination_root =
            AnchoredDir::open_workspace(&destination_dir, None).expect("backup root should open");
        let temporary_name = OsStr::new(".aopmem.sqlite.open-destination.tmp");
        let final_name = OsStr::new("aopmem.sqlite");
        let temporary_path = destination_dir.join(temporary_name);
        let final_path = destination_dir.join(final_name);
        drop(
            destination_root
                .create_new_regular_os(temporary_name)
                .expect("temporary database should create"),
        );
        let destination =
            Connection::open(&temporary_path).expect("destination SQLite should open");
        destination
            .execute_batch("CREATE TABLE proof (id INTEGER PRIMARY KEY);")
            .expect("destination SQLite should write");
        let temporary_file = destination_root
            .open_regular_for_update_os(temporary_name)
            .expect("temporary file should open for publish");

        destination_root
            .publish_regular_no_replace_committed_os(&temporary_file, temporary_name, final_name)
            .expect_err("an open SQLite destination handle must block Windows publish");
        assert!(temporary_path.is_file());
        assert!(!final_path.exists());

        drop(temporary_file);
        close_connection(destination).expect("destination SQLite should close");
        let temporary_file = destination_root
            .open_regular_for_update_os(temporary_name)
            .expect("closed destination should reopen for publish");
        let outcome = destination_root
            .publish_regular_no_replace_committed_os(&temporary_file, temporary_name, final_name)
            .expect("publish should succeed after SQLite destination closes");
        assert!(outcome.durability_confirmed);
        assert!(outcome.temporary_cleanup_confirmed);
        assert!(final_path.is_file());
        assert!(!temporary_path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_writable_flush_and_handle_relative_publish_regression() {
        let root = TestRoot::new("windows-publish");
        let source_path = create_source(root.path(), "source.sqlite");
        let destination_dir = root.path().join("backups");
        fs::create_dir(&destination_dir).expect("backup directory should create");
        let final_path = destination_dir.join("aopmem.sqlite");
        let expected_schema = schema(&source_path);

        let artifact = online_backup_to_path_with_faults(
            open_source(&source_path),
            &source_path,
            &destination_dir,
            &final_path,
            &expected_schema,
            &NoBackupFaults,
        )
        .expect("Windows backup publish should not flush a read-only file handle");

        assert!(artifact.final_path.is_file());
        assert_eq!(node_count(&artifact.final_path), 1);
    }
}
