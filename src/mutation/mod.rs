//! One lifecycle for every operational-memory mutation.

use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

use crate::audit::{self, PendingSnapshotMarker};
use crate::{schema, storage};
use rusqlite::Connection;

pub use crate::output::OutputWarning as MutationWarning;

pub const AUDIT_SNAPSHOT_PENDING: &str = "AUDIT_SNAPSHOT_PENDING";
pub const MUTATION_LOCK_FILE_NAME: &str = ".mutation.lock";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationOutcome<T> {
    pub value: T,
    pub warning: Option<MutationWarning>,
    pub snapshot_report: Option<audit::SqlSnapshotReport>,
    pub snapshot_observation: SnapshotObservation,
}

/// Privacy-safe facts captured around the post-commit audit snapshot attempt.
///
/// The mutation coordinator returns these facts to the CLI. It never opens or
/// writes the observability store while the workspace mutation locks are held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotObservation {
    Completed {
        duration_ms: u64,
        bytes_written: u64,
    },
    Pending {
        duration_ms: u64,
    },
}

#[derive(Debug)]
pub enum MutationError<E> {
    Io(io::Error),
    Db(rusqlite::Error),
    Operation(E),
    Rollback {
        operation: Option<Box<E>>,
        source: rusqlite::Error,
    },
    FilesystemRollback {
        source: io::Error,
    },
}

impl<E: fmt::Display> fmt::Display for MutationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
            Self::Operation(error) => write!(formatter, "{error}"),
            Self::Rollback { source, .. } => {
                write!(formatter, "database rollback failed: {source}")
            }
            Self::FilesystemRollback { source } => {
                write!(formatter, "filesystem rollback failed: {source}")
            }
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for MutationError<E> {}

/// Files created by an operation and removed when its database transaction
/// does not commit. Only trusted feature code can register paths here.
#[derive(Debug, Default)]
pub struct MutationEffects {
    rollback_actions: Vec<RollbackAction>,
}

#[derive(Debug)]
enum RollbackAction {
    RemoveCreatedTree(PathBuf),
    RemoveCreatedDirectory(PathBuf),
    RemoveCreatedFile(PathBuf),
    RestoreFile { path: PathBuf, bytes: Vec<u8> },
}

impl MutationEffects {
    /// Registers a newly-created owned tree such as one draft tool directory.
    pub fn register_created_directory(&mut self, path: PathBuf) {
        self.rollback_actions
            .push(RollbackAction::RemoveCreatedTree(path));
    }

    /// Registers a newly-created directory that must be empty at rollback.
    pub fn register_created_empty_directory(&mut self, path: PathBuf) {
        self.rollback_actions
            .push(RollbackAction::RemoveCreatedDirectory(path));
    }

    /// Registers removal before attempting to create or write a new file.
    pub fn register_created_file(&mut self, path: PathBuf) {
        self.rollback_actions
            .push(RollbackAction::RemoveCreatedFile(path));
    }

    /// Registers exact original bytes before overwriting an existing file.
    pub fn register_file_restore(&mut self, path: PathBuf, bytes: Vec<u8>) {
        self.rollback_actions
            .push(RollbackAction::RestoreFile { path, bytes });
    }

    fn rollback(self) -> io::Result<()> {
        let mut first_error = None;
        for action in self.rollback_actions.into_iter().rev() {
            if let Err(error) = apply_rollback_action(action) {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn rollback_created_directories_best_effort(&mut self) {
        for action in self.rollback_actions.drain(..).rev() {
            let _ = apply_rollback_action(action);
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.rollback_actions.clear();
    }
}

fn apply_rollback_action(action: RollbackAction) -> io::Result<()> {
    match action {
        RollbackAction::RemoveCreatedTree(path) => ignore_not_found(fs::remove_dir_all(path)),
        RollbackAction::RemoveCreatedDirectory(path) => ignore_not_found(fs::remove_dir(path)),
        RollbackAction::RemoveCreatedFile(path) => ignore_not_found(fs::remove_file(path)),
        RollbackAction::RestoreFile { path, bytes } => fs::write(path, bytes),
    }
}

fn ignore_not_found(result: io::Result<()>) -> io::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Runs migrations and one requested write in one IMMEDIATE transaction, then
/// publishes the audit snapshot while still holding the workspace lock.
pub fn mutate_workspace<T, E>(
    workspace_paths: &storage::WorkspacePaths,
    operation: impl FnOnce(&Connection, &mut MutationEffects) -> Result<T, E>,
) -> Result<MutationOutcome<T>, MutationError<E>> {
    let workspace_identity =
        storage::validate_workspace_mutation_paths(workspace_paths).map_err(MutationError::Io)?;
    storage::validate_optional_regular_file(&workspace_paths.root().join(MUTATION_LOCK_FILE_NAME))
        .map_err(MutationError::Io)?;
    let workspace_locks = audit::acquire_workspace_mutation_locks(
        workspace_paths.root(),
        workspace_paths.audit_git(),
        MUTATION_LOCK_FILE_NAME,
        workspace_identity,
    )
    .map_err(snapshot_error_to_mutation)?;
    let snapshot_lock = workspace_locks.snapshot_lock();
    let marker = audit::ensure_pending_snapshot_marker_locked(snapshot_lock)
        .map_err(snapshot_error_to_mutation)?;
    let connection = match storage::open_workspace_db_without_migrations(workspace_paths) {
        Ok(connection) => connection,
        Err(error) => {
            clear_owned_marker(marker, snapshot_lock).map_err(MutationError::Io)?;
            return Err(MutationError::Db(error));
        }
    };

    if let Err(error) = connection.execute_batch("BEGIN IMMEDIATE;") {
        clear_owned_marker(marker, snapshot_lock).map_err(MutationError::Io)?;
        return Err(MutationError::Db(error));
    }

    let mut effects = MutationEffects::default();
    if let Err(error) = schema::apply_pending_migrations_in(&connection) {
        rollback_database(&connection, None)?;
        clear_owned_marker(marker, snapshot_lock).map_err(MutationError::Io)?;
        return Err(MutationError::Db(error));
    }

    let value = match operation(&connection, &mut effects) {
        Ok(value) => value,
        Err(operation) => {
            let operation = rollback_database(&connection, Some(operation))?
                .expect("operation rollback always carries its operation error");
            effects
                .rollback()
                .map_err(|source| MutationError::FilesystemRollback { source })?;
            clear_owned_marker(marker, snapshot_lock).map_err(MutationError::Io)?;
            return Err(MutationError::Operation(operation));
        }
    };

    if let Err(commit_error) = connection.execute_batch("COMMIT;") {
        let rollback_result = connection.execute_batch("ROLLBACK;");
        if let Err(source) = rollback_result {
            return Err(MutationError::Rollback {
                operation: None,
                source,
            });
        }
        effects
            .rollback()
            .map_err(|source| MutationError::FilesystemRollback { source })?;
        clear_owned_marker(marker, snapshot_lock).map_err(MutationError::Io)?;
        return Err(MutationError::Db(commit_error));
    }

    let snapshot_started_at = Instant::now();
    let (snapshot_report, snapshot_observation, warning) = match audit::write_sql_snapshot_locked(
        workspace_paths.audit_git(),
        &connection,
        snapshot_lock,
    ) {
        Ok(report) => {
            let snapshot_observation = SnapshotObservation::Completed {
                duration_ms: report.duration_ms,
                bytes_written: report.bytes_written,
            };
            (Some(report), snapshot_observation, None)
        }
        Err(error) => {
            let duration_ms =
                u64::try_from(snapshot_started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
            (
                None,
                SnapshotObservation::Pending { duration_ms },
                Some(MutationWarning {
                    code: AUDIT_SNAPSHOT_PENDING,
                    message: format!("mutation committed; audit snapshot pending: {error}"),
                }),
            )
        }
    };

    Ok(MutationOutcome {
        value,
        warning,
        snapshot_report,
        snapshot_observation,
    })
}

fn rollback_database<E>(
    connection: &Connection,
    operation: Option<E>,
) -> Result<Option<E>, MutationError<E>> {
    if let Err(source) = connection.execute_batch("ROLLBACK;") {
        return Err(MutationError::Rollback {
            operation: operation.map(Box::new),
            source,
        });
    }
    Ok(operation)
}

fn clear_owned_marker(
    marker: PendingSnapshotMarker,
    snapshot_lock: &audit::SnapshotLock,
) -> io::Result<()> {
    if marker == PendingSnapshotMarker::Created {
        if let Err(error) = audit::clear_pending_snapshot_marker_locked(snapshot_lock) {
            let error = snapshot_error_to_io(error);
            let _ = audit::ensure_pending_snapshot_marker_locked(snapshot_lock);
            return Err(error);
        }
    }
    Ok(())
}

fn snapshot_error_to_mutation<E>(error: audit::SnapshotError) -> MutationError<E> {
    match error {
        audit::SnapshotError::Db(error) => MutationError::Db(error),
        audit::SnapshotError::Io(error) => MutationError::Io(error),
    }
}

fn snapshot_error_to_io(error: audit::SnapshotError) -> io::Error {
    match error {
        audit::SnapshotError::Io(error) => error,
        audit::SnapshotError::Db(error) => io::Error::other(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::Path;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let previous = env::var_os(key);
            env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                env::set_var(self.key, previous);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!(
            "aopmem-stage-006-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn workspace(name: &str) -> (PathBuf, storage::WorkspacePaths, EnvGuard) {
        let home = temp_path(name);
        let guard = EnvGuard::set(AOPMEM_HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("test AOPMEM_HOME should resolve");
        storage::ensure_global_dirs(&paths).expect("global dirs should create");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "mutation-test")
            .expect("workspace dirs should create");
        (home, workspace_paths, guard)
    }

    #[test]
    fn marker_exists_before_migration_and_operation_then_clears_after_snapshot() {
        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("marker-before-operation");

        let outcome = mutate_workspace(&workspace_paths, |connection, _effects| {
            assert!(
                audit::has_pending_snapshot(workspace_paths.audit_git())
                    .expect("marker should be readable")
            );
            let migrations: i64 = connection.query_row(
                "SELECT COUNT(*) FROM schema_migrations;",
                [],
                |row| row.get(0),
            )?;
            connection.execute(
                "INSERT INTO registries (registry_type, name, status) VALUES ('test', 'marker', 'active');",
                [],
            )?;
            Ok::<_, rusqlite::Error>(migrations)
        })
        .expect("coordinated mutation should succeed");

        assert_eq!(outcome.value, 3);
        assert!(outcome.warning.is_none());
        let report = outcome
            .snapshot_report
            .expect("successful snapshot report should be retained");
        assert!(report.bytes_written > 0);
        assert_eq!(
            outcome.snapshot_observation,
            SnapshotObservation::Completed {
                duration_ms: report.duration_ms,
                bytes_written: report.bytes_written,
            }
        );
        assert!(!audit::has_pending_snapshot(workspace_paths.audit_git())
            .expect("marker state should read"));
        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[test]
    fn migration_and_operation_roll_back_together_and_clear_owned_marker() {
        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("atomic-rollback");

        let error = mutate_workspace(&workspace_paths, |connection, _effects| {
            connection
                .execute("INSERT INTO nodes (node_type, status, title) VALUES ('raw_note', 'draft', 'rollback');", [])
                .expect("row should insert inside transaction");
            Err::<(), _>("forced operation failure")
        })
        .expect_err("operation should fail");
        assert!(matches!(
            error,
            MutationError::Operation("forced operation failure")
        ));

        let connection = storage::open_workspace_db_without_migrations(&workspace_paths)
            .expect("rolled-back DB should open");
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'nodes';",
                [],
                |row| row.get(0),
            )
            .expect("sqlite catalog should read");
        assert_eq!(table_count, 0);
        assert!(!audit::has_pending_snapshot(workspace_paths.audit_git())
            .expect("marker state should read"));
        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[test]
    fn operation_failure_never_changes_or_deletes_existing_marker() {
        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("existing-marker");
        let marker_path = workspace_paths
            .audit_git()
            .join(audit::PENDING_SNAPSHOT_MARKER_FILE_NAME);
        fs::write(&marker_path, b"existing marker must survive\n")
            .expect("existing marker should write");

        let error = mutate_workspace(&workspace_paths, |_connection, _effects| {
            Err::<(), _>("forced operation failure")
        })
        .expect_err("operation should fail");

        assert!(matches!(
            error,
            MutationError::Operation("forced operation failure")
        ));
        assert_eq!(
            fs::read(&marker_path).expect("existing marker should read"),
            b"existing marker must survive\n"
        );
        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[test]
    fn invalid_database_path_fails_before_marker_creation() {
        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("open-failure-marker");
        fs::create_dir(workspace_paths.db()).expect("DB path directory fixture should create");

        let error = mutate_workspace(&workspace_paths, |_connection, _effects| {
            Ok::<_, rusqlite::Error>(())
        })
        .expect_err("opening a directory as SQLite must fail");

        assert!(matches!(error, MutationError::Io(_)));
        assert!(!audit::has_pending_snapshot(workspace_paths.audit_git())
            .expect("marker state should read"));
        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[test]
    fn audit_failure_after_commit_returns_structured_warning_and_keeps_marker() {
        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("audit-warning");
        fs::write(
            workspace_paths.audit_git().join(".git"),
            b"not a git directory",
        )
        .expect("invalid git fixture should write");

        let outcome = mutate_workspace(&workspace_paths, |connection, _effects| {
            connection.execute(
                "INSERT INTO registries (registry_type, name, status) VALUES ('test', 'committed', 'active');",
                [],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .expect("audit failure must not change committed command status");

        let warning = outcome.warning.expect("audit warning should be returned");
        assert_eq!(warning.code, AUDIT_SNAPSHOT_PENDING);
        assert!(outcome.snapshot_report.is_none());
        assert!(matches!(
            outcome.snapshot_observation,
            SnapshotObservation::Pending { .. }
        ));
        assert!(audit::has_pending_snapshot(workspace_paths.audit_git())
            .expect("marker state should read"));
        let connection = storage::open_workspace_db_read_only(&workspace_paths)
            .expect("committed DB should open read-only");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM registries WHERE name = 'committed';",
                [],
                |row| row.get(0),
            )
            .expect("committed row should read");
        assert_eq!(count, 1);
        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_audit_root_fails_before_database_mutation_and_preserves_outside_tree() {
        use std::os::unix::fs::symlink;

        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("audit-root-symlink");
        let outside = temp_path("audit-root-symlink-outside");
        fs::create_dir(&outside).expect("outside fixture should create");
        fs::write(outside.join("memory.sql"), b"outside memory sentinel\n")
            .expect("outside memory sentinel should write");
        fs::write(outside.join("keep.bin"), [0_u8, 1, 2, 255])
            .expect("outside binary sentinel should write");
        let outside_before = vec![
            (
                "keep.bin",
                fs::read(outside.join("keep.bin")).expect("binary sentinel should read"),
            ),
            (
                "memory.sql",
                fs::read(outside.join("memory.sql")).expect("memory sentinel should read"),
            ),
        ];

        fs::remove_dir_all(workspace_paths.audit_git())
            .expect("owned audit directory should remove");
        symlink(&outside, workspace_paths.audit_git())
            .expect("outside audit symlink should create");
        assert!(!workspace_paths.db().exists());

        let error = mutate_workspace(&workspace_paths, |connection, _effects| {
            connection.execute(
                "INSERT INTO registries (registry_type, name, status) VALUES ('test', 'must-not-commit', 'active');",
                [],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .expect_err("symlinked audit root must fail before opening the database");

        assert!(matches!(error, MutationError::Io(_)));
        assert!(error
            .to_string()
            .contains("unsafe persistent workspace path"));
        assert!(!workspace_paths.db().exists());
        assert_eq!(
            vec![
                (
                    "keep.bin",
                    fs::read(outside.join("keep.bin")).expect("binary sentinel should remain"),
                ),
                (
                    "memory.sql",
                    fs::read(outside.join("memory.sql")).expect("memory sentinel should remain"),
                ),
            ],
            outside_before
        );
        assert_eq!(
            fs::read_dir(&outside)
                .expect("outside tree should list")
                .count(),
            2,
            "no marker, lock, or Git metadata may be created outside"
        );

        fs::remove_file(workspace_paths.audit_git()).expect("audit symlink should remove");
        fs::remove_dir_all(home).expect("test home should remove");
        fs::remove_dir_all(outside).expect("outside fixture should remove");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_workspace_root_preserves_outside_database() {
        use std::os::unix::fs::symlink;

        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("workspace-root-symlink");
        mutate_workspace(&workspace_paths, |connection, _effects| {
            connection.execute(
                "INSERT INTO registries (registry_type, name, status) VALUES ('test', 'seed', 'active');",
                [],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .expect("seed mutation should succeed");

        let outside = temp_path("workspace-root-symlink-outside");
        fs::rename(workspace_paths.root(), &outside)
            .expect("workspace should move outside for fixture");
        symlink(&outside, workspace_paths.root()).expect("workspace symlink should create");
        let outside_db = outside.join("aopmem.sqlite");
        let db_before = fs::read(&outside_db).expect("outside DB should read");

        let error = mutate_workspace(
            &workspace_paths,
            |_connection, _effects| -> Result<(), rusqlite::Error> {
                panic!("operation must not run through a workspace symlink");
            },
        )
        .expect_err("workspace symlink must fail before database mutation");

        assert!(matches!(error, MutationError::Io(_)));
        assert_eq!(
            fs::read(&outside_db).expect("outside DB should remain readable"),
            db_before
        );
        fs::remove_file(workspace_paths.root()).expect("workspace symlink should remove");
        fs::remove_dir_all(&outside).expect("outside workspace should remove");
        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_database_preserves_outside_database_bytes() {
        use std::os::unix::fs::symlink;

        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("database-symlink");
        mutate_workspace(&workspace_paths, |connection, _effects| {
            connection.execute(
                "INSERT INTO registries (registry_type, name, status) VALUES ('test', 'seed', 'active');",
                [],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .expect("seed mutation should succeed");

        let outside_db = temp_path("database-symlink-outside.sqlite");
        fs::rename(workspace_paths.db(), &outside_db).expect("DB should move outside");
        symlink(&outside_db, workspace_paths.db()).expect("DB symlink should create");
        let db_before = fs::read(&outside_db).expect("outside DB should read");

        let error = mutate_workspace(
            &workspace_paths,
            |_connection, _effects| -> Result<(), rusqlite::Error> {
                panic!("operation must not run through a database symlink");
            },
        )
        .expect_err("database symlink must fail before opening SQLite");

        assert!(matches!(error, MutationError::Io(_)));
        assert_eq!(
            fs::read(&outside_db).expect("outside DB should remain readable"),
            db_before
        );
        fs::remove_file(workspace_paths.db()).expect("DB symlink should remove");
        fs::remove_dir_all(home).expect("test home should remove");
        fs::remove_file(outside_db).expect("outside DB should remove");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_wal_sidecar_preserves_external_file_and_database() {
        use std::os::unix::fs::symlink;

        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("wal-sidecar-symlink");
        mutate_workspace(&workspace_paths, |_connection, _effects| {
            Ok::<_, rusqlite::Error>(())
        })
        .expect("seed mutation should succeed");
        let mut wal_path = workspace_paths.db().as_os_str().to_os_string();
        wal_path.push("-wal");
        let wal_path = PathBuf::from(wal_path);
        if wal_path.exists() {
            fs::remove_file(&wal_path).expect("owned WAL should remove");
        }
        let outside = temp_path("wal-sidecar-symlink-outside");
        fs::write(&outside, b"outside WAL sentinel\n").expect("outside sentinel should write");
        symlink(&outside, &wal_path).expect("WAL symlink should create");
        let db_before = fs::read(workspace_paths.db()).expect("workspace DB should read");

        let error = mutate_workspace(
            &workspace_paths,
            |_connection, _effects| -> Result<(), rusqlite::Error> {
                panic!("operation must not run with a linked WAL sidecar");
            },
        )
        .expect_err("linked WAL must fail before database mutation");

        assert!(matches!(error, MutationError::Io(_)));
        assert_eq!(
            fs::read(workspace_paths.db()).expect("DB should remain"),
            db_before
        );
        assert_eq!(
            fs::read(&outside).expect("outside WAL sentinel should remain"),
            b"outside WAL sentinel\n"
        );
        fs::remove_file(wal_path).expect("WAL symlink should remove");
        fs::remove_dir_all(home).expect("test home should remove");
        fs::remove_file(outside).expect("outside sentinel should remove");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_audit_git_metadata_preserves_external_repo_and_database() {
        use std::os::unix::fs::symlink;

        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("audit-git-metadata-symlink");
        mutate_workspace(&workspace_paths, |_connection, _effects| {
            Ok::<_, rusqlite::Error>(())
        })
        .expect("seed mutation should succeed");

        let outside = temp_path("audit-git-metadata-outside");
        fs::create_dir(&outside).expect("outside root should create");
        let outside_git = outside.join("external.git");
        fs::rename(workspace_paths.audit_git().join(".git"), &outside_git)
            .expect("Git metadata should move outside");
        fs::write(
            outside_git.join("external-sentinel"),
            b"external git sentinel\n",
        )
        .expect("external Git sentinel should write");
        symlink(&outside_git, workspace_paths.audit_git().join(".git"))
            .expect("Git metadata symlink should create");
        let db_before = fs::read(workspace_paths.db()).expect("workspace DB should read");
        let head_before = fs::read(outside_git.join("HEAD")).expect("external HEAD should read");
        let config_before =
            fs::read(outside_git.join("config")).expect("external config should read");

        let error = mutate_workspace(
            &workspace_paths,
            |_connection, _effects| -> Result<(), rusqlite::Error> {
                panic!("operation must not run with linked Git metadata");
            },
        )
        .expect_err("linked Git metadata must fail before database mutation");

        assert!(matches!(error, MutationError::Io(_)));
        assert_eq!(
            fs::read(workspace_paths.db()).expect("DB should remain"),
            db_before
        );
        assert_eq!(
            fs::read(outside_git.join("HEAD")).expect("HEAD should remain"),
            head_before
        );
        assert_eq!(
            fs::read(outside_git.join("config")).expect("config should remain"),
            config_before
        );
        assert_eq!(
            fs::read(outside_git.join("external-sentinel"))
                .expect("external sentinel should remain"),
            b"external git sentinel\n"
        );
        fs::remove_file(workspace_paths.audit_git().join(".git"))
            .expect("Git metadata symlink should remove");
        fs::remove_dir_all(home).expect("test home should remove");
        fs::remove_dir_all(outside).expect("outside Git root should remove");
    }

    #[cfg(unix)]
    #[test]
    fn linked_nested_git_objects_or_refs_preserve_external_tree_and_database() {
        use std::os::unix::fs::symlink;

        fn entry_count(root: &Path) -> usize {
            let mut count = 0_usize;
            let mut pending = vec![root.to_path_buf()];
            while let Some(directory) = pending.pop() {
                for entry in fs::read_dir(directory).expect("Git fixture should list") {
                    let path = entry.expect("Git fixture entry should read").path();
                    count += 1;
                    if fs::symlink_metadata(&path)
                        .expect("Git fixture metadata should read")
                        .is_dir()
                    {
                        pending.push(path);
                    }
                }
            }
            count
        }

        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        for nested_name in ["objects", "refs"] {
            let (home, workspace_paths, _guard) = workspace(&format!("nested-git-{nested_name}"));
            mutate_workspace(&workspace_paths, |_connection, _effects| {
                Ok::<_, rusqlite::Error>(())
            })
            .expect("seed mutation should succeed");

            let nested_path = workspace_paths.audit_git().join(".git").join(nested_name);
            let outside = temp_path(&format!("nested-git-{nested_name}-outside"));
            fs::rename(&nested_path, &outside).expect("nested Git tree should move outside");
            fs::write(outside.join("external-sentinel"), b"nested Git sentinel\n")
                .expect("nested Git sentinel should write");
            symlink(&outside, &nested_path).expect("nested Git symlink should create");
            let db_before = fs::read(workspace_paths.db()).expect("workspace DB should read");
            let entry_count_before = entry_count(&outside);

            let error = mutate_workspace(
                &workspace_paths,
                |_connection, _effects| -> Result<(), rusqlite::Error> {
                    panic!("operation must not run with linked nested Git metadata");
                },
            )
            .expect_err("linked nested Git metadata must fail before database mutation");

            assert!(matches!(error, MutationError::Io(_)));
            assert_eq!(
                fs::read(workspace_paths.db()).expect("DB should remain"),
                db_before
            );
            assert_eq!(entry_count(&outside), entry_count_before);
            assert_eq!(
                fs::read(outside.join("external-sentinel"))
                    .expect("nested Git sentinel should remain"),
                b"nested Git sentinel\n"
            );
            fs::remove_file(nested_path).expect("nested Git symlink should remove");
            fs::remove_dir_all(home).expect("test home should remove");
            fs::remove_dir_all(outside).expect("outside Git tree should remove");
        }
    }

    #[cfg(unix)]
    #[test]
    fn user_writable_intermediate_symlink_is_rejected_before_lock_creation() {
        use std::os::unix::fs::symlink;

        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let base = temp_path("intermediate-workspace-symlink");
        let real_home = base.join("real-home");
        let linked_home = base.join("linked-home");
        fs::create_dir_all(&base).expect("base should create");

        let _real_guard = EnvGuard::set(AOPMEM_HOME_ENV, &real_home);
        let real_paths = storage::resolve_paths().expect("real paths should resolve");
        storage::ensure_global_dirs(&real_paths).expect("real global dirs should create");
        let real_workspace = storage::ensure_workspace_dirs(&real_paths, "mutation-test")
            .expect("real workspace should create");
        let sentinel = real_workspace.root().join("outside-sentinel");
        fs::write(&sentinel, b"outside workspace sentinel\n")
            .expect("outside sentinel should write");
        symlink(&real_home, &linked_home).expect("intermediate symlink should create");

        let _linked_guard = EnvGuard::set(AOPMEM_HOME_ENV, &linked_home);
        let linked_paths = storage::resolve_paths().expect("linked paths should resolve");
        let linked_workspace = storage::workspace_paths_for_key(&linked_paths, "mutation-test");
        let error = mutate_workspace(
            &linked_workspace,
            |_connection, _effects| -> Result<(), rusqlite::Error> {
                panic!("operation must not run through an intermediate symlink");
            },
        )
        .expect_err("user-writable intermediate symlink must fail closed");

        assert!(matches!(error, MutationError::Io(_)));
        assert_eq!(
            fs::read(&sentinel).expect("outside sentinel should remain"),
            b"outside workspace sentinel\n"
        );
        assert!(!real_workspace.root().join(MUTATION_LOCK_FILE_NAME).exists());
        assert!(!real_workspace.audit_git().join(".snapshot.lock").exists());

        fs::remove_file(linked_home).expect("intermediate symlink should remove");
        fs::remove_dir_all(base).expect("test base should remove");
    }

    #[test]
    fn workspace_identity_swap_after_validation_is_rejected_before_lock_creation() {
        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("workspace-identity-swap");
        let identity = storage::validate_workspace_mutation_paths(&workspace_paths)
            .expect("workspace should validate");
        let moved = workspace_paths.root().with_extension("validated-original");
        fs::rename(workspace_paths.root(), &moved).expect("validated workspace should move");
        fs::create_dir_all(workspace_paths.audit_git())
            .expect("replacement audit directory should create");
        let sentinel = workspace_paths.root().join("replacement-sentinel");
        fs::write(&sentinel, b"replacement workspace sentinel\n")
            .expect("replacement sentinel should write");

        let error = audit::acquire_workspace_mutation_locks(
            workspace_paths.root(),
            workspace_paths.audit_git(),
            MUTATION_LOCK_FILE_NAME,
            identity,
        )
        .expect_err("replacement workspace identity must fail closed");

        assert!(error.to_string().contains("workspace identity changed"));
        assert_eq!(
            fs::read(&sentinel).expect("replacement sentinel should remain"),
            b"replacement workspace sentinel\n"
        );
        assert!(!workspace_paths
            .root()
            .join(MUTATION_LOCK_FILE_NAME)
            .exists());
        assert!(!workspace_paths.audit_git().join(".snapshot.lock").exists());

        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_mutation_or_snapshot_lock_preserves_external_file_and_database() {
        use std::os::unix::fs::symlink;

        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("snapshot-lock-symlink");
        mutate_workspace(&workspace_paths, |_connection, _effects| {
            Ok::<_, rusqlite::Error>(())
        })
        .expect("seed mutation should succeed");
        let db_before = fs::read(workspace_paths.db()).expect("workspace DB should read");
        for (name, lock_path) in [
            (
                "mutation",
                workspace_paths.root().join(MUTATION_LOCK_FILE_NAME),
            ),
            (
                "snapshot",
                workspace_paths.audit_git().join(".snapshot.lock"),
            ),
        ] {
            fs::remove_file(&lock_path).expect("owned lock should remove");
            let outside = temp_path(&format!("{name}-lock-symlink-outside"));
            fs::write(&outside, b"outside lock sentinel\n").expect("outside sentinel should write");
            symlink(&outside, &lock_path).expect("lock symlink should create");

            let error = mutate_workspace(
                &workspace_paths,
                |_connection, _effects| -> Result<(), rusqlite::Error> {
                    panic!("operation must not run with a linked lock");
                },
            )
            .expect_err("linked lock must fail before database mutation");

            assert!(matches!(error, MutationError::Io(_)));
            assert_eq!(
                fs::read(workspace_paths.db()).expect("DB should remain"),
                db_before
            );
            assert_eq!(
                fs::read(&outside).expect("outside sentinel should remain"),
                b"outside lock sentinel\n"
            );
            fs::remove_file(&lock_path).expect("lock symlink should remove");
            fs::write(&lock_path, b"").expect("owned lock should restore");
            fs::remove_file(outside).expect("outside sentinel should remove");
        }
        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[test]
    fn direct_snapshot_cannot_clear_marker_while_mutation_is_in_flight() {
        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("snapshot-marker-race");
        let mutation_paths = workspace_paths.clone();
        let snapshot_paths = workspace_paths.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let mutation = thread::spawn(move || {
            mutate_workspace(&mutation_paths, |connection, _effects| {
                entered_tx.send(()).expect("mutation entry should report");
                release_rx.recv().expect("mutation should release");
                connection.execute(
                    "INSERT INTO registries (registry_type, name, status) VALUES ('test', 'race', 'active');",
                    [],
                )?;
                Ok::<_, rusqlite::Error>(())
            })
        });
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("mutation should enter after acquiring snapshot lock");
        assert!(audit::has_pending_snapshot(workspace_paths.audit_git())
            .expect("in-flight marker should read"));

        let (snapshot_done_tx, snapshot_done_rx) = mpsc::channel();
        let snapshot = thread::spawn(move || {
            let mut connection =
                Connection::open_in_memory().expect("direct snapshot connection should open");
            schema::apply_migrations(&mut connection)
                .expect("direct snapshot fixture should migrate");
            let result = audit::write_sql_snapshot(snapshot_paths.audit_git(), &connection);
            snapshot_done_tx
                .send(())
                .expect("direct snapshot completion should report");
            result
        });
        assert!(snapshot_done_rx
            .recv_timeout(Duration::from_millis(200))
            .is_err());
        assert!(audit::has_pending_snapshot(workspace_paths.audit_git())
            .expect("blocked direct snapshot must not clear marker"));

        release_tx.send(()).expect("mutation should release");
        mutation
            .join()
            .expect("mutation thread should join")
            .expect("mutation should succeed");
        snapshot_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("direct snapshot should run after mutation unlocks");
        snapshot
            .join()
            .expect("snapshot thread should join")
            .expect("direct snapshot should succeed");
        assert!(!audit::has_pending_snapshot(workspace_paths.audit_git())
            .expect("completed snapshots should clear marker"));
        fs::remove_dir_all(home).expect("test home should remove");
    }

    #[test]
    fn workspace_lock_serializes_whole_mutation_lifecycle() {
        let _env_lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, workspace_paths, _guard) = workspace("lock-serialization");
        let first_paths = workspace_paths.clone();
        let second_paths = workspace_paths.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let first = thread::spawn(move || {
            mutate_workspace(&first_paths, |connection, _effects| {
                entered_tx.send("first").expect("first entry should report");
                release_rx.recv().expect("first mutation should release");
                connection.execute(
                    "INSERT INTO registries (registry_type, name, status) VALUES ('test', 'first', 'active');",
                    [],
                )?;
                Ok::<_, rusqlite::Error>(())
            })
        });
        assert_eq!(
            entered_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("first mutation should enter"),
            "first"
        );

        let (second_entered_tx, second_entered_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            mutate_workspace(&second_paths, |connection, _effects| {
                second_entered_tx
                    .send(())
                    .expect("second entry should report");
                connection.execute(
                    "INSERT INTO registries (registry_type, name, status) VALUES ('test', 'second', 'active');",
                    [],
                )?;
                Ok::<_, rusqlite::Error>(())
            })
        });
        assert!(second_entered_rx
            .recv_timeout(Duration::from_millis(200))
            .is_err());
        release_tx.send(()).expect("first mutation should release");
        first
            .join()
            .expect("first thread should join")
            .expect("first mutation should succeed");
        second_entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("second mutation should enter after first completes");
        second
            .join()
            .expect("second thread should join")
            .expect("second mutation should succeed");

        fs::remove_dir_all(home).expect("test home should remove");
    }
}
