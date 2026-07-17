use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;

use super::{
    backup::online_backup_to_path, display_path, enumerate_workspace_entries,
    open_immutable_database, path_with_suffix, validate_existing_root,
    validate_optional_managed_directory, DATABASE_FILE_NAME,
};
use crate::audit::{self, AnchoredDir};
use crate::mutation;
use crate::storage::{self, AopmemPaths, WorkspacePaths};

const PREPARE_SCOPE: &str = "all_workspaces";
const BACKUPS_DIRECTORY: &str = "backups";
const BACKUP_RUN_PREFIX: &str = "upgrade-prepare-rc3-";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradePrepareReport {
    pub prepare_only: bool,
    pub scope: &'static str,
    pub binary_version: &'static str,
    pub success: bool,
    pub writes_performed: bool,
    pub backup_root: Option<String>,
    pub workspace_count: usize,
    pub workspaces: Vec<WorkspacePrepareReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<UpgradePrepareFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradePrepareExecution {
    pub report: UpgradePrepareReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<UpgradePrepareFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradePrepareFailure {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspacePrepareReport {
    pub workspace_key: String,
    pub workspace_path: String,
    pub status: WorkspacePrepareStatus,
    pub backup_path: Option<String>,
    pub backup_bytes: Option<u64>,
    pub checkpoint_attempted: bool,
    pub checkpoint_busy: Option<i64>,
    pub wal_frames: Option<i64>,
    pub checkpointed_frames: Option<i64>,
    pub empty_sidecars_removed: Vec<String>,
    pub schema_changed: bool,
    pub logical_data_changed: bool,
    pub ready_for_plan: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<UpgradePrepareFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePrepareStatus {
    NotStarted,
    AlreadyClean,
    Prepared,
    Failed,
}

#[derive(Debug, Error)]
pub enum UpgradePrepareError {
    #[error(transparent)]
    ResolvePaths(#[from] storage::PathResolveError),
    #[error("cannot inspect AOPMem path {path}: {source}")]
    InspectPath {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot enumerate AOPMem workspaces at {path}: {source}")]
    EnumerateWorkspaces {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug)]
struct PrepareBackupRun {
    root: PathBuf,
    workspaces: PathBuf,
}

type BackupOperation = fn(&Connection, &Path, &Path) -> io::Result<()>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckpointValidationError {
    Busy {
        busy: i64,
        wal_frames: i64,
        checkpointed_frames: i64,
    },
    Incomplete {
        wal_frames: i64,
        checkpointed_frames: i64,
    },
}

impl CheckpointValidationError {
    fn code(self) -> &'static str {
        match self {
            Self::Busy { .. } => "CHECKPOINT_BUSY",
            Self::Incomplete { .. } => "CHECKPOINT_INCOMPLETE",
        }
    }
}

impl std::fmt::Display for CheckpointValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy {
                busy,
                wal_frames,
                checkpointed_frames,
            } => write!(
                formatter,
                "checkpoint busy={busy} wal_frames={wal_frames} checkpointed_frames={checkpointed_frames}"
            ),
            Self::Incomplete {
                wal_frames,
                checkpointed_frames,
            } => write!(
                formatter,
                "checkpoint incomplete: wal_frames={wal_frames} checkpointed_frames={checkpointed_frames}"
            ),
        }
    }
}

pub fn prepare_all_workspaces() -> Result<UpgradePrepareExecution, UpgradePrepareError> {
    prepare_all_workspaces_with_backup(online_backup_to_path)
}

fn prepare_all_workspaces_with_backup(
    backup_operation: BackupOperation,
) -> Result<UpgradePrepareExecution, UpgradePrepareError> {
    let paths = storage::resolve_paths()?;
    validate_existing_root(paths.home()).map_err(map_plan_error)?;
    validate_optional_managed_directory(paths.home(), paths.workspaces())
        .map_err(map_plan_error)?;
    let entries = enumerate_workspace_entries(&paths).map_err(map_plan_error)?;
    let workspace_count = entries.len();
    let mut candidates = Vec::with_capacity(workspace_count);
    for entry in entries {
        let name = entry.file_name();
        let key = match name.into_string() {
            Ok(key) => key,
            Err(name) => {
                return Ok(root_failure_execution(
                    workspace_count,
                    "UNSUPPORTED_WORKSPACE_NAME",
                    format!(
                        "workspace directory name is not valid Unicode: {}",
                        name.to_string_lossy()
                    ),
                ));
            }
        };
        candidates.push((key.clone(), storage::workspace_paths_for_key(&paths, key)));
    }

    let mut report = UpgradePrepareReport {
        prepare_only: true,
        scope: PREPARE_SCOPE,
        binary_version: env!("CARGO_PKG_VERSION"),
        success: true,
        writes_performed: false,
        backup_root: None,
        workspace_count,
        workspaces: candidates
            .iter()
            .map(|(key, workspace)| empty_workspace_report(key, workspace))
            .collect(),
        stopped_workspace: None,
        stop_reason: None,
    };
    let mut backup_run = None;

    for (index, (key, workspace)) in candidates.iter().enumerate() {
        if let Err(failure) = prepare_workspace(
            &paths,
            key,
            workspace,
            &mut backup_run,
            &mut report.workspaces[index],
            &mut report.writes_performed,
            backup_operation,
        ) {
            report.success = false;
            report.stopped_workspace = Some(key.clone());
            report.stop_reason = Some(failure.clone());
            report.backup_root = backup_run.as_ref().map(|run| display_path(&run.root));
            return Ok(UpgradePrepareExecution {
                report,
                failure: Some(failure),
            });
        }
    }
    report.backup_root = backup_run.as_ref().map(|run| display_path(&run.root));
    Ok(UpgradePrepareExecution {
        report,
        failure: None,
    })
}

fn prepare_workspace(
    paths: &AopmemPaths,
    key: &str,
    workspace: &WorkspacePaths,
    backup_run: &mut Option<PrepareBackupRun>,
    report: &mut WorkspacePrepareReport,
    writes_performed: &mut bool,
    backup_operation: BackupOperation,
) -> Result<(), UpgradePrepareFailure> {
    let identity = storage::validate_workspace_mutation_paths(workspace)
        .map_err(|error| workspace_failure("UNSAFE_WORKSPACE_PATH", error, key, report))?;
    storage::validate_optional_regular_file(
        &workspace.root().join(mutation::MUTATION_LOCK_FILE_NAME),
    )
    .map_err(|error| workspace_failure("UNSAFE_WORKSPACE_LOCK", error, key, report))?;
    if !workspace.db().exists() {
        return Err(workspace_failure(
            "WORKSPACE_DATABASE_MISSING",
            io::Error::new(io::ErrorKind::NotFound, "workspace database is missing"),
            key,
            report,
        ));
    }
    let has_coordination_sidecar = inspect_prepare_sidecars(workspace.db(), key, report)?;
    if !has_coordination_sidecar {
        let connection = open_immutable_database(workspace.db()).map_err(|error| {
            workspace_failure("DATABASE_READ_FAILED", io::Error::other(error), key, report)
        })?;
        super::inspect_schema(&connection).map_err(|error| {
            workspace_failure(error.code, io::Error::other(error.message), key, report)
        })?;
        report.status = WorkspacePrepareStatus::AlreadyClean;
        report.ready_for_plan = true;
        return Ok(());
    }

    let locks = audit::acquire_workspace_mutation_locks(
        workspace.root(),
        workspace.audit_git(),
        mutation::MUTATION_LOCK_FILE_NAME,
        identity,
    )
    .map_err(|error| {
        workspace_failure(
            "WORKSPACE_LOCK_FAILED",
            io::Error::other(error),
            key,
            report,
        )
    })?;
    let connection = open_operational_database(workspace).map_err(|error| {
        workspace_failure("DATABASE_OPEN_FAILED", io::Error::other(error), key, report)
    })?;
    connection
        .execute_batch("BEGIN IMMEDIATE;")
        .map_err(|error| {
            workspace_failure("DATABASE_BUSY", io::Error::other(error), key, report)
        })?;
    let schema_before = match super::inspect_schema(&connection) {
        Ok(schema) => schema,
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK;");
            return Err(workspace_failure(
                error.code,
                io::Error::other(error.message),
                key,
                report,
            ));
        }
    };

    if backup_run.is_none() {
        let created = create_prepare_backup_run(paths).map_err(|error| {
            let _ = connection.execute_batch("ROLLBACK;");
            workspace_failure("BACKUP_CREATE_FAILED", error, key, report)
        })?;
        *backup_run = Some(created);
        *writes_performed = true;
    }
    let run = backup_run.as_ref().expect("backup run initialized above");
    let workspace_backup_dir = run.workspaces.join(key);
    storage::ensure_owned_direct_directory(&run.workspaces, &workspace_backup_dir).map_err(
        |error| {
            let _ = connection.execute_batch("ROLLBACK;");
            workspace_failure("WORKSPACE_BACKUP_FAILED", error, key, report)
        },
    )?;
    let backup_path = workspace_backup_dir.join(DATABASE_FILE_NAME);
    let backup_source = open_read_only_database(workspace).map_err(|error| {
        let _ = connection.execute_batch("ROLLBACK;");
        workspace_failure(
            "WORKSPACE_BACKUP_FAILED",
            io::Error::other(error),
            key,
            report,
        )
    })?;
    if let Err(error) = backup_operation(&backup_source, &workspace_backup_dir, &backup_path) {
        let _ = connection.execute_batch("ROLLBACK;");
        return Err(workspace_failure(
            "WORKSPACE_BACKUP_FAILED",
            error,
            key,
            report,
        ));
    }
    drop(backup_source);
    *writes_performed = true;
    report.backup_bytes = fs::metadata(&backup_path)
        .ok()
        .map(|metadata| metadata.len());
    report.backup_path = Some(display_path(&backup_path));

    if let Err(error) = connection.execute_batch("COMMIT;") {
        return Err(workspace_failure(
            "PREPARE_BOUNDARY_COMMIT_FAILED",
            io::Error::other(error),
            key,
            report,
        ));
    }
    report.checkpoint_attempted = true;
    let checkpoint = connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE);", [], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| {
            workspace_failure("CHECKPOINT_FAILED", io::Error::other(error), key, report)
        })?;
    report.checkpoint_busy = Some(checkpoint.0);
    report.wal_frames = Some(checkpoint.1);
    report.checkpointed_frames = Some(checkpoint.2);
    if let Err(error) = validate_checkpoint_result(checkpoint) {
        return Err(workspace_failure(
            error.code(),
            io::Error::other(error.to_string()),
            key,
            report,
        ));
    }
    let schema_after = super::inspect_schema(&connection).map_err(|error| {
        workspace_failure(error.code, io::Error::other(error.message), key, report)
    })?;
    if schema_after != schema_before {
        report.schema_changed = true;
        return Err(workspace_failure(
            "SCHEMA_CHANGED_DURING_PREPARE",
            io::Error::other("schema changed during upgrade preparation"),
            key,
            report,
        ));
    }
    drop(connection);

    report.empty_sidecars_removed = cleanup_empty_sidecars(workspace, identity, key, report)?;
    *writes_performed = true;
    ensure_no_sidecars(workspace.db(), key, report)?;
    drop(locks);
    report.status = WorkspacePrepareStatus::Prepared;
    report.ready_for_plan = true;
    Ok(())
}

fn validate_checkpoint_result(
    (busy, wal_frames, checkpointed_frames): (i64, i64, i64),
) -> Result<(), CheckpointValidationError> {
    if busy != 0 {
        return Err(CheckpointValidationError::Busy {
            busy,
            wal_frames,
            checkpointed_frames,
        });
    }
    if wal_frames != checkpointed_frames {
        return Err(CheckpointValidationError::Incomplete {
            wal_frames,
            checkpointed_frames,
        });
    }
    Ok(())
}

fn inspect_prepare_sidecars(
    database: &Path,
    key: &str,
    report: &mut WorkspacePrepareReport,
) -> Result<bool, UpgradePrepareFailure> {
    let mut found = false;
    for suffix in ["-wal", "-shm"] {
        let path = path_with_suffix(database, suffix);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                found = true
            }
            Ok(_) => {
                return Err(workspace_failure(
                    "UNSAFE_DATABASE_SIDECAR",
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "database sidecar is not a real regular file: {}",
                            display_path(&path)
                        ),
                    ),
                    key,
                    report,
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(workspace_failure(
                    "DATABASE_SIDECAR_INSPECTION_FAILED",
                    error,
                    key,
                    report,
                ));
            }
        }
    }
    let journal = path_with_suffix(database, "-journal");
    match fs::symlink_metadata(&journal) {
        Ok(_) => Err(workspace_failure(
            "UNSAFE_DATABASE_JOURNAL",
            io::Error::other(format!(
                "rollback journal requires manual process quiescence: {}",
                display_path(&journal)
            )),
            key,
            report,
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(found),
        Err(error) => Err(workspace_failure(
            "DATABASE_SIDECAR_INSPECTION_FAILED",
            error,
            key,
            report,
        )),
    }
}

fn cleanup_empty_sidecars(
    workspace: &WorkspacePaths,
    identity: audit::WorkspaceIdentity,
    key: &str,
    report: &mut WorkspacePrepareReport,
) -> Result<Vec<String>, UpgradePrepareFailure> {
    let root = AnchoredDir::open_workspace(workspace.root(), Some(identity))
        .map_err(|error| workspace_failure("UNSAFE_WORKSPACE_PATH", error, key, report))?;
    let mut removed = Vec::new();
    for suffix in ["-wal", "-shm"] {
        let path = path_with_suffix(workspace.db(), suffix);
        let name: OsString = path
            .file_name()
            .ok_or_else(|| {
                workspace_failure(
                    "UNSAFE_DATABASE_SIDECAR",
                    io::Error::other("database sidecar has no file name"),
                    key,
                    report,
                )
            })?
            .to_os_string();
        let Some(file) = root
            .open_regular_optional_os(&name)
            .map_err(|error| workspace_failure("UNSAFE_DATABASE_SIDECAR", error, key, report))?
        else {
            continue;
        };
        let metadata = file.metadata().map_err(|error| {
            workspace_failure("DATABASE_SIDECAR_INSPECTION_FAILED", error, key, report)
        })?;
        if metadata.len() != 0 {
            return Err(workspace_failure(
                "DATABASE_SIDECAR_NOT_EMPTY",
                io::Error::other(format!(
                    "checkpoint left non-empty sidecar {} ({} bytes)",
                    name.to_string_lossy(),
                    metadata.len()
                )),
                key,
                report,
            ));
        }
        drop(file);
        root.remove_regular_os(&name)
            .map_err(|error| workspace_failure("SIDECAR_CLEANUP_FAILED", error, key, report))?;
        removed.push(name.to_string_lossy().into_owned());
    }
    root.sync()
        .map_err(|error| workspace_failure("SIDECAR_CLEANUP_FAILED", error, key, report))?;
    Ok(removed)
}

fn ensure_no_sidecars(
    database: &Path,
    key: &str,
    report: &mut WorkspacePrepareReport,
) -> Result<(), UpgradePrepareFailure> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = path_with_suffix(database, suffix);
        match fs::symlink_metadata(&sidecar) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(workspace_failure(
                    "CHECKPOINT_REQUIRED",
                    io::Error::other(format!(
                        "database sidecar remains after preparation: {}",
                        sidecar.file_name().unwrap_or_default().to_string_lossy()
                    )),
                    key,
                    report,
                ));
            }
            Err(error) => {
                return Err(workspace_failure(
                    "DATABASE_SIDECAR_INSPECTION_FAILED",
                    error,
                    key,
                    report,
                ));
            }
        }
    }
    Ok(())
}

fn open_operational_database(workspace: &WorkspacePaths) -> rusqlite::Result<Connection> {
    let canonical = workspace
        .db()
        .canonicalize()
        .map_err(|_| rusqlite::Error::InvalidPath(workspace.db().clone()))?;
    let connection = Connection::open_with_flags(
        canonical,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA temp_store = MEMORY;",
    )?;
    Ok(connection)
}

fn open_read_only_database(workspace: &WorkspacePaths) -> rusqlite::Result<Connection> {
    let canonical = workspace
        .db()
        .canonicalize()
        .map_err(|_| rusqlite::Error::InvalidPath(workspace.db().clone()))?;
    let connection = Connection::open_with_flags(
        canonical,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    connection.execute_batch("PRAGMA busy_timeout = 5000; PRAGMA query_only = ON;")?;
    Ok(connection)
}

fn create_prepare_backup_run(paths: &AopmemPaths) -> io::Result<PrepareBackupRun> {
    let backups = paths.home().join(BACKUPS_DIRECTORY);
    storage::ensure_owned_direct_directory(paths.home(), &backups)?;
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| io::Error::other("random backup id failed"))?;
    let mut id = String::with_capacity(32);
    for byte in random {
        use std::fmt::Write as _;
        write!(id, "{byte:02x}").map_err(io::Error::other)?;
    }
    let root = backups.join(format!("{BACKUP_RUN_PREFIX}{id}"));
    storage::ensure_owned_direct_directory(&backups, &root)?;
    let workspaces = root.join("workspaces");
    storage::ensure_owned_direct_directory(&root, &workspaces)?;
    AnchoredDir::open_workspace(&root, None)?.sync()?;
    Ok(PrepareBackupRun { root, workspaces })
}

fn empty_workspace_report(key: &str, workspace: &WorkspacePaths) -> WorkspacePrepareReport {
    WorkspacePrepareReport {
        workspace_key: key.to_string(),
        workspace_path: display_path(workspace.root()),
        status: WorkspacePrepareStatus::NotStarted,
        backup_path: None,
        backup_bytes: None,
        checkpoint_attempted: false,
        checkpoint_busy: None,
        wal_frames: None,
        checkpointed_frames: None,
        empty_sidecars_removed: Vec::new(),
        schema_changed: false,
        logical_data_changed: false,
        ready_for_plan: false,
        error: None,
    }
}

fn workspace_failure(
    code: &'static str,
    error: io::Error,
    key: &str,
    report: &mut WorkspacePrepareReport,
) -> UpgradePrepareFailure {
    let failure = UpgradePrepareFailure {
        code,
        message: error.to_string(),
        workspace_key: Some(key.to_string()),
    };
    report.status = WorkspacePrepareStatus::Failed;
    report.error = Some(failure.clone());
    failure
}

fn root_failure_execution(
    workspace_count: usize,
    code: &'static str,
    message: String,
) -> UpgradePrepareExecution {
    let failure = UpgradePrepareFailure {
        code,
        message,
        workspace_key: None,
    };
    UpgradePrepareExecution {
        report: UpgradePrepareReport {
            prepare_only: true,
            scope: PREPARE_SCOPE,
            binary_version: env!("CARGO_PKG_VERSION"),
            success: false,
            writes_performed: false,
            backup_root: None,
            workspace_count,
            workspaces: Vec::new(),
            stopped_workspace: None,
            stop_reason: Some(failure.clone()),
        },
        failure: Some(failure),
    }
}

fn map_plan_error(error: super::UpgradePlanError) -> UpgradePrepareError {
    match error {
        super::UpgradePlanError::ResolvePaths(error) => UpgradePrepareError::ResolvePaths(error),
        super::UpgradePlanError::InspectPath { path, source }
        | super::UpgradePlanError::DiskSpace { path, source } => {
            UpgradePrepareError::InspectPath { path, source }
        }
        super::UpgradePlanError::EnumerateWorkspaces { path, source } => {
            UpgradePrepareError::EnumerateWorkspaces { path, source }
        }
        super::UpgradePlanError::SizeOverflow => UpgradePrepareError::InspectPath {
            path: PathBuf::new(),
            source: io::Error::other("upgrade size overflow"),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::ffi::OsString;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::schema;

    const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";

    struct EnvGuard {
        original: Option<OsString>,
    }

    impl EnvGuard {
        fn set(path: &Path) -> Self {
            let original = env::var_os(AOPMEM_HOME_ENV);
            env::set_var(AOPMEM_HOME_ENV, path);
            Self { original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => env::set_var(AOPMEM_HOME_ENV, value),
                None => env::remove_var(AOPMEM_HOME_ENV),
            }
        }
    }

    struct UnlimitedDisk;

    impl super::super::DiskSpaceProbe for UnlimitedDisk {
        fn available_bytes(&self, _path: &Path) -> io::Result<u64> {
            Ok(u64::MAX)
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        env::temp_dir().join(format!("aopmem-upgrade-prepare-{name}-{nanos}"))
    }

    fn create_workspace(_home: &Path, key: &str) -> WorkspacePaths {
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace = storage::workspace_paths_for_key(&paths, key);
        fs::create_dir_all(workspace.root()).expect("workspace root should create");
        fs::create_dir_all(workspace.audit_git()).expect("audit directory should create");
        let mut connection = Connection::open(workspace.db()).expect("database should open");
        schema::apply_migrations(&mut connection).expect("fixture schema should create");
        connection
            .execute(
                "INSERT INTO nodes (node_type, status, title) VALUES ('rule', 'active', ?1)",
                [key],
            )
            .expect("node should insert");
        connection
            .execute(
                "DELETE FROM schema_migrations WHERE version IN ('002', '003')",
                [],
            )
            .expect("fixture should emulate v0.1 markers");
        drop(connection);
        workspace
    }

    fn node_count(path: &Path) -> i64 {
        let connection = Connection::open(path).expect("database should open");
        connection
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .expect("node count should read")
    }

    fn schema_markers(path: &Path) -> Vec<(String, String)> {
        let connection = Connection::open(path).expect("database should open");
        let mut statement = connection
            .prepare("SELECT version, name FROM schema_migrations ORDER BY version")
            .expect("schema markers should prepare");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("schema markers should query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("schema markers should collect")
    }

    fn logical_counts(path: &Path) -> Vec<(&'static str, i64)> {
        let connection = Connection::open(path).expect("database should open");
        [
            "nodes",
            "links",
            "aliases",
            "tags",
            "sources",
            "events",
            "tool_contracts",
            "mcp_profiles",
            "fts_nodes",
        ]
        .into_iter()
        .map(|table| {
            let count = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("logical count should read");
            (table, count)
        })
        .collect()
    }

    fn fail_backup(_source: &Connection, _directory: &Path, _path: &Path) -> io::Result<()> {
        Err(io::Error::other("injected backup failure"))
    }

    #[test]
    fn zero_byte_wal_is_backed_up_removed_and_plan_becomes_ready() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("zero-wal");
        fs::create_dir_all(home.join("workspaces")).expect("workspaces should create");
        let _home = EnvGuard::set(&home);
        let workspace = create_workspace(&home, "zero-wal-workspace");
        let wal = path_with_suffix(workspace.db(), "-wal");
        let shm = path_with_suffix(workspace.db(), "-shm");
        fs::write(&wal, []).expect("empty WAL should create");
        fs::write(&shm, []).expect("empty SHM should create");
        let before_nodes = node_count(workspace.db());
        let before_schema = schema_markers(workspace.db());

        let execution = prepare_all_workspaces().expect("prepare should run");

        assert!(execution.failure.is_none());
        assert!(execution.report.success);
        assert!(execution.report.writes_performed);
        assert!(execution.report.backup_root.is_some());
        let prepared = &execution.report.workspaces[0];
        assert_eq!(prepared.status, WorkspacePrepareStatus::Prepared);
        assert!(prepared.ready_for_plan);
        assert!(!prepared.schema_changed);
        assert!(!prepared.logical_data_changed);
        assert!(!wal.exists());
        assert!(!shm.exists());
        assert_eq!(node_count(workspace.db()), before_nodes);
        assert_eq!(schema_markers(workspace.db()), before_schema);
        let backup = PathBuf::from(
            prepared
                .backup_path
                .as_ref()
                .expect("backup path should exist"),
        );
        assert!(backup.is_file());
        assert_eq!(node_count(&backup), before_nodes);
        assert_eq!(schema_markers(&backup), before_schema);

        let plan = super::super::plan_all_workspaces_with_probe(&UnlimitedDisk)
            .expect("plan should run after prepare");
        assert!(plan.ready);
        assert!(!plan.writes_performed);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn clean_workspace_is_idempotent_and_creates_no_backup() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("already-clean");
        fs::create_dir_all(home.join("workspaces")).expect("workspaces should create");
        let _home = EnvGuard::set(&home);
        create_workspace(&home, "clean-workspace");

        let first = prepare_all_workspaces().expect("first prepare should run");
        let second = prepare_all_workspaces().expect("second prepare should run");

        for execution in [first, second] {
            assert!(execution.report.success);
            assert!(!execution.report.writes_performed);
            assert!(execution.report.backup_root.is_none());
            assert_eq!(
                execution.report.workspaces[0].status,
                WorkspacePrepareStatus::AlreadyClean
            );
            assert!(execution.report.workspaces[0].ready_for_plan);
        }

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn journal_blocks_before_backup_or_checkpoint() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("journal");
        fs::create_dir_all(home.join("workspaces")).expect("workspaces should create");
        let _home = EnvGuard::set(&home);
        let workspace = create_workspace(&home, "journal-workspace");
        let journal = path_with_suffix(workspace.db(), "-journal");
        fs::write(&journal, b"uncommitted journal").expect("journal should create");

        let execution = prepare_all_workspaces().expect("prepare should report blocker");

        let failure = execution.failure.expect("failure should exist");
        assert_eq!(failure.code, "UNSAFE_DATABASE_JOURNAL");
        assert!(!execution.report.writes_performed);
        assert!(execution.report.backup_root.is_none());
        assert!(journal.exists());
        assert!(!execution.report.workspaces[0].checkpoint_attempted);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn multiple_workspaces_are_prepared_in_stable_order() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("stable-order");
        fs::create_dir_all(home.join("workspaces")).expect("workspaces should create");
        let _home = EnvGuard::set(&home);
        let zeta = create_workspace(&home, "zeta-workspace");
        let alpha = create_workspace(&home, "alpha-workspace");
        fs::write(path_with_suffix(zeta.db(), "-wal"), []).expect("zeta WAL should create");
        fs::write(path_with_suffix(alpha.db(), "-wal"), []).expect("alpha WAL should create");

        let execution = prepare_all_workspaces().expect("prepare should run");

        assert!(execution.report.success);
        assert_eq!(execution.report.workspace_count, 2);
        assert_eq!(
            execution
                .report
                .workspaces
                .iter()
                .map(|workspace| workspace.workspace_key.as_str())
                .collect::<Vec<_>>(),
            ["alpha-workspace", "zeta-workspace"]
        );
        assert!(execution
            .report
            .workspaces
            .iter()
            .all(|workspace| workspace.ready_for_plan));

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn committed_non_empty_wal_is_checkpointed_without_losing_rows() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("committed-wal");
        fs::create_dir_all(home.join("workspaces")).expect("workspaces should create");
        let _home = EnvGuard::set(&home);
        let workspace = create_workspace(&home, "committed-wal-workspace");
        let wal = path_with_suffix(workspace.db(), "-wal");
        let shm = path_with_suffix(workspace.db(), "-shm");

        let connection = Connection::open(workspace.db()).expect("WAL fixture should open");
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
            .expect("WAL mode should enable");
        let main_before = fs::read(workspace.db()).expect("pre-WAL main DB should read");
        connection
            .execute_batch(
                "
                INSERT INTO nodes (node_type, status, title, summary, body, source_ref)
                VALUES ('rule', 'active', 'wal-row', 'summary', 'body', 'source://wal');
                INSERT INTO links (source_node_id, target_node_id, link_type)
                VALUES (1, 2, 'supports');
                INSERT INTO aliases (node_id, alias) VALUES (2, 'wal-alias');
                INSERT INTO tags (node_id, tag) VALUES (2, 'wal-tag');
                INSERT INTO sources (node_id, source_ref) VALUES (2, 'source://wal');
                INSERT INTO events (type, source, subject_kind, subject_id)
                VALUES ('node.created', 'test', 'node', 2);
                INSERT INTO tool_contracts (
                    tool_id, name, status, side_effects, approval_requirement, contract_json
                ) VALUES ('wal-tool', 'WAL Tool', 'active', 'local', 'none', '{}');
                INSERT INTO mcp_profiles (
                    id, name, kind, status, read_operations, write_operations,
                    side_effects, approval_requirement
                ) VALUES (
                    'wal-mcp', 'WAL MCP', 'stdio', 'active', 'read', '', 'none', 'none'
                );
                INSERT INTO fts_nodes (rowid, title, summary, body, aliases)
                VALUES (2, 'wal-row', 'summary', 'body', 'wal-alias');
                ",
            )
            .expect("representative WAL records should commit");
        let wal_bytes = fs::read(&wal).expect("committed WAL should read");
        let shm_bytes = fs::read(&shm).expect("WAL index should read");
        assert!(!wal_bytes.is_empty());
        drop(connection);

        // Restore the exact crash boundary: old main DB plus committed WAL and
        // its matching index, with no live SQLite process.
        fs::write(workspace.db(), main_before).expect("main DB should restore");
        fs::write(&wal, wal_bytes).expect("WAL should restore");
        fs::write(&shm, shm_bytes).expect("WAL index should restore");

        let execution = prepare_all_workspaces().expect("prepare should checkpoint WAL");

        assert!(execution.report.success, "{execution:?}");
        let expected_counts = vec![
            ("nodes", 2),
            ("links", 1),
            ("aliases", 1),
            ("tags", 1),
            ("sources", 1),
            ("events", 1),
            ("tool_contracts", 1),
            ("mcp_profiles", 1),
            ("fts_nodes", 1),
        ];
        assert_eq!(logical_counts(workspace.db()), expected_counts);
        assert_eq!(
            schema_markers(workspace.db()),
            [("001".into(), "001_init".into())]
        );
        assert!(!wal.exists());
        assert!(!shm.exists());
        let backup = PathBuf::from(
            execution.report.workspaces[0]
                .backup_path
                .as_ref()
                .expect("backup should exist"),
        );
        assert_eq!(logical_counts(&backup), expected_counts);
        assert_eq!(schema_markers(&backup), [("001".into(), "001_init".into())]);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn active_writer_fails_before_backup_and_keeps_sidecars() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("active-writer");
        fs::create_dir_all(home.join("workspaces")).expect("workspaces should create");
        let _home = EnvGuard::set(&home);
        let workspace = create_workspace(&home, "active-writer-workspace");
        let writer = Connection::open(workspace.db()).expect("writer should open");
        writer
            .execute_batch("PRAGMA journal_mode=WAL; BEGIN IMMEDIATE;")
            .expect("writer should reserve database");

        let execution = prepare_all_workspaces().expect("prepare should report busy database");

        let failure = execution.failure.expect("failure should exist");
        assert_eq!(failure.code, "DATABASE_BUSY");
        assert!(!execution.report.writes_performed);
        assert!(execution.report.backup_root.is_none());
        assert!(path_with_suffix(workspace.db(), "-wal").exists());
        writer
            .execute_batch("ROLLBACK;")
            .expect("writer should rollback");
        drop(writer);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[cfg(unix)]
    #[test]
    fn linked_wal_fails_closed_without_touching_outside_file() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("linked-wal");
        fs::create_dir_all(home.join("workspaces")).expect("workspaces should create");
        let _home = EnvGuard::set(&home);
        let workspace = create_workspace(&home, "linked-wal-workspace");
        let outside = home.join("outside-wal");
        fs::write(&outside, b"outside proof").expect("outside file should create");
        symlink(&outside, path_with_suffix(workspace.db(), "-wal"))
            .expect("WAL symlink should create");

        let execution = prepare_all_workspaces().expect("prepare should fail closed");

        let failure = execution.failure.expect("failure should exist");
        assert_eq!(failure.code, "UNSAFE_WORKSPACE_PATH");
        assert_eq!(
            fs::read(&outside).expect("outside file should read"),
            b"outside proof"
        );
        assert!(!execution.report.writes_performed);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn checkpoint_validator_distinguishes_busy_and_incomplete() {
        let busy = validate_checkpoint_result((1, 7, 7)).expect_err("busy should fail");
        assert_eq!(busy.code(), "CHECKPOINT_BUSY");
        assert!(busy.to_string().contains("busy=1"));

        let incomplete = validate_checkpoint_result((0, 7, 6)).expect_err("incomplete should fail");
        assert_eq!(incomplete.code(), "CHECKPOINT_INCOMPLETE");
        assert!(incomplete.to_string().contains("wal_frames=7"));

        assert_eq!(validate_checkpoint_result((0, 7, 7)), Ok(()));
        assert_eq!(validate_checkpoint_result((0, -1, -1)), Ok(()));
    }

    #[test]
    fn backup_failure_prevents_checkpoint_and_retains_sidecars() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("backup-failure");
        fs::create_dir_all(home.join("workspaces")).expect("workspaces should create");
        let _home = EnvGuard::set(&home);
        let workspace = create_workspace(&home, "backup-failure-workspace");
        let wal = path_with_suffix(workspace.db(), "-wal");
        fs::write(&wal, []).expect("empty WAL should create");
        let schema_before = schema_markers(workspace.db());

        let execution = prepare_all_workspaces_with_backup(fail_backup)
            .expect("prepare should report backup failure");

        let failure = execution.failure.expect("failure should exist");
        assert_eq!(failure.code, "WORKSPACE_BACKUP_FAILED");
        assert!(execution.report.backup_root.is_some());
        assert!(execution.report.writes_performed);
        assert!(!execution.report.workspaces[0].checkpoint_attempted);
        assert!(wal.exists());
        assert_eq!(schema_markers(workspace.db()), schema_before);

        fs::remove_dir_all(home).expect("fixture should remove");
    }
}
