#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;

use crate::schema;
use crate::storage::{self, AopmemPaths, WorkspacePaths};

mod apply;
mod backup;
mod prepare;
mod recovery;

pub use apply::{
    apply_all_workspaces, apply_core_all_workspaces, UpgradeApplyError, UpgradeApplyExecution,
    UpgradeApplyFailure, UpgradeApplyReport,
};
#[cfg(test)]
pub(crate) use backup::BackupPhase;
pub(crate) use backup::WorkspaceBackupFailureDetails;
pub use prepare::{
    prepare_all_workspaces, UpgradePrepareError, UpgradePrepareExecution, UpgradePrepareFailure,
    UpgradePrepareReport,
};
pub use recovery::{
    adopt_home_backup, apply_or_resume, backup_home, inspect_recovery, publish_applied,
    stage_binary, RecoveryError, RecoveryExecution, RecoveryInspectReport, RecoveryPhase,
    RecoveryPublishFailure,
};

const PLAN_SCOPE: &str = "all_workspaces";
const DATABASE_FILE_NAME: &str = "aopmem.sqlite";
const DATABASE_SIDECAR_SUFFIXES: &[&str] = &["-wal", "-shm", "-journal"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradePlanReport {
    pub plan_only: bool,
    pub writes_performed: bool,
    pub scope: &'static str,
    pub binary_version: &'static str,
    pub aopmem_home: String,
    pub workspaces_root: String,
    pub workspace_count: usize,
    pub ready: bool,
    pub disk_space: DiskSpacePlan,
    pub workspaces: Vec<WorkspacePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiskSpacePlan {
    pub probe_path: String,
    pub available_bytes: u64,
    pub minimum_required_bytes: u64,
    pub workspace_database_backup_bytes: u64,
    pub installed_binary_backup_bytes: u64,
    pub sufficient: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspacePlan {
    pub workspace_key: String,
    pub workspace_path: String,
    pub database_path: String,
    pub database_size_bytes: Option<u64>,
    pub status: WorkspacePlanStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<WorkspaceSchemaPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WorkspacePlanError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePlanStatus {
    Ready,
    MigrationRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceSchemaPlan {
    pub current_version: String,
    pub target_version: String,
    pub applied_migrations: Vec<MigrationPlanItem>,
    pub pending_migrations: Vec<MigrationPlanItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MigrationPlanItem {
    pub version: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspacePlanError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_hint: Option<String>,
}

#[derive(Debug, Error)]
pub enum UpgradePlanError {
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
    #[error("cannot inspect free disk space at {path}: {source}")]
    DiskSpace {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("upgrade backup size exceeds the supported byte range")]
    SizeOverflow,
}

pub trait DiskSpaceProbe {
    fn available_bytes(&self, path: &Path) -> io::Result<u64>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemDiskSpaceProbe;

impl DiskSpaceProbe for SystemDiskSpaceProbe {
    fn available_bytes(&self, path: &Path) -> io::Result<u64> {
        system_available_bytes(path)
    }
}

pub fn plan_all_workspaces() -> Result<UpgradePlanReport, UpgradePlanError> {
    plan_all_workspaces_with_probe(&SystemDiskSpaceProbe)
}

pub fn plan_all_workspaces_with_probe(
    probe: &dyn DiskSpaceProbe,
) -> Result<UpgradePlanReport, UpgradePlanError> {
    let paths = storage::resolve_paths()?;
    validate_existing_root(paths.home())?;
    validate_optional_managed_directory(paths.home(), paths.workspaces())?;

    let entries = enumerate_workspace_entries(&paths)?;
    let mut workspaces = Vec::with_capacity(entries.len());
    let mut database_backup_bytes = 0_u64;
    for entry in entries {
        let workspace_plan = inspect_workspace_entry(&paths, entry);
        database_backup_bytes = database_backup_bytes
            .checked_add(workspace_plan.database_size_bytes.unwrap_or_default())
            .ok_or(UpgradePlanError::SizeOverflow)?;
        workspaces.push(workspace_plan);
    }

    let installed_binary_backup_bytes = installed_binary_size(&paths)?;
    let minimum_required_bytes = database_backup_bytes
        .checked_add(installed_binary_backup_bytes)
        .ok_or(UpgradePlanError::SizeOverflow)?;
    let probe_path = nearest_existing_directory(paths.home())?;
    let available_bytes =
        probe
            .available_bytes(&probe_path)
            .map_err(|source| UpgradePlanError::DiskSpace {
                path: probe_path.clone(),
                source,
            })?;
    let sufficient = available_bytes >= minimum_required_bytes;
    let ready = sufficient
        && workspaces
            .iter()
            .all(|workspace| workspace.status != WorkspacePlanStatus::Blocked);

    Ok(UpgradePlanReport {
        plan_only: true,
        writes_performed: false,
        scope: PLAN_SCOPE,
        binary_version: env!("CARGO_PKG_VERSION"),
        aopmem_home: display_path(paths.home()),
        workspaces_root: display_path(paths.workspaces()),
        workspace_count: workspaces.len(),
        ready,
        disk_space: DiskSpacePlan {
            probe_path: display_path(&probe_path),
            available_bytes,
            minimum_required_bytes,
            workspace_database_backup_bytes: database_backup_bytes,
            installed_binary_backup_bytes,
            sufficient,
        },
        workspaces,
    })
}

fn validate_existing_root(path: &Path) -> Result<(), UpgradePlanError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            storage::validate_real_directory(path).map_err(|source| UpgradePlanError::InspectPath {
                path: path.to_path_buf(),
                source,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(UpgradePlanError::InspectPath {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn validate_optional_managed_directory(
    parent: &Path,
    directory: &Path,
) -> Result<(), UpgradePlanError> {
    match fs::symlink_metadata(directory) {
        Ok(_) => {
            storage::validate_real_directory(directory).map_err(|source| {
                UpgradePlanError::InspectPath {
                    path: directory.to_path_buf(),
                    source,
                }
            })?;
            storage::validate_canonical_direct_child(parent, directory).map_err(|source| {
                UpgradePlanError::InspectPath {
                    path: directory.to_path_buf(),
                    source,
                }
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(UpgradePlanError::InspectPath {
            path: directory.to_path_buf(),
            source,
        }),
    }
}

fn enumerate_workspace_entries(paths: &AopmemPaths) -> Result<Vec<fs::DirEntry>, UpgradePlanError> {
    if !paths.workspaces().exists() {
        return Ok(Vec::new());
    }
    let mut entries = fs::read_dir(paths.workspaces())
        .map_err(|source| UpgradePlanError::EnumerateWorkspaces {
            path: paths.workspaces().clone(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| UpgradePlanError::EnumerateWorkspaces {
            path: paths.workspaces().clone(),
            source,
        })?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn inspect_workspace_entry(paths: &AopmemPaths, entry: fs::DirEntry) -> WorkspacePlan {
    let entry_path = entry.path();
    let database_path = entry_path.join(DATABASE_FILE_NAME);
    let database_size_bytes = regular_file_size(&database_path);
    let workspace_key = match entry.file_name().into_string() {
        Ok(workspace_key) => workspace_key,
        Err(name) => {
            return WorkspacePlan {
                workspace_key: name.to_string_lossy().into_owned(),
                workspace_path: display_path(&entry_path),
                database_path: display_path(&database_path),
                database_size_bytes,
                status: WorkspacePlanStatus::Blocked,
                schema: None,
                error: Some(workspace_error(
                    "UNSUPPORTED_WORKSPACE_NAME",
                    "workspace directory name is not valid Unicode",
                )),
            };
        }
    };
    let workspace_paths = storage::workspace_paths_for_key(paths, &workspace_key);

    let inspection = inspect_workspace(&workspace_paths, &entry);
    match inspection {
        Ok(schema) => WorkspacePlan {
            workspace_key,
            workspace_path: display_path(workspace_paths.root()),
            database_path: display_path(workspace_paths.db()),
            database_size_bytes,
            status: if schema.pending_migrations.is_empty() {
                WorkspacePlanStatus::Ready
            } else {
                WorkspacePlanStatus::MigrationRequired
            },
            schema: Some(schema),
            error: None,
        },
        Err(error) => WorkspacePlan {
            workspace_key,
            workspace_path: display_path(workspace_paths.root()),
            database_path: display_path(workspace_paths.db()),
            database_size_bytes,
            status: WorkspacePlanStatus::Blocked,
            schema: None,
            error: Some(error),
        },
    }
}

fn inspect_workspace(
    workspace_paths: &WorkspacePaths,
    entry: &fs::DirEntry,
) -> Result<WorkspaceSchemaPlan, WorkspacePlanError> {
    let entry_type = entry.file_type().map_err(|error| {
        workspace_error(
            "WORKSPACE_PATH_UNREADABLE",
            format!("cannot inspect workspace entry type: {error}"),
        )
    })?;
    if !entry_type.is_dir() || entry_type.is_symlink() {
        return Err(workspace_error(
            "UNSAFE_WORKSPACE_PATH",
            "workspace entry is not a real directory",
        ));
    }
    storage::validate_workspace_read_paths(workspace_paths).map_err(|error| {
        workspace_error(
            "UNSAFE_WORKSPACE_PATH",
            format!("workspace managed path validation failed: {error}"),
        )
    })?;
    if !workspace_paths.db().exists() {
        return Err(workspace_error(
            "WORKSPACE_DATABASE_MISSING",
            format!("workspace database is missing: {DATABASE_FILE_NAME}"),
        ));
    }
    reject_database_sidecars(workspace_paths.db())?;

    let connection =
        open_immutable_database(workspace_paths.db()).map_err(classify_database_error)?;
    let schema = inspect_schema(&connection);
    drop(connection);
    reject_database_sidecars(workspace_paths.db())?;
    schema
}

fn reject_database_sidecars(database_path: &Path) -> Result<(), WorkspacePlanError> {
    for suffix in DATABASE_SIDECAR_SUFFIXES {
        let sidecar = path_with_suffix(database_path, suffix);
        match fs::symlink_metadata(&sidecar) {
            Ok(_) => {
                let mut error = workspace_error(
                    "UNSAFE_DATABASE_SIDECAR",
                    format!(
                        "database sidecar is present; prepare it before upgrade planning: {}",
                        sidecar.file_name().unwrap_or_default().to_string_lossy()
                    ),
                );
                error.fix_hint = Some(
                    "run `aopmem upgrade prepare --all-workspaces --json` while AOPMem processes are closed"
                        .to_string(),
                );
                return Err(error);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(workspace_error(
                    "DATABASE_SIDECAR_INSPECTION_FAILED",
                    format!(
                        "cannot inspect database sidecar {}: {error}",
                        display_path(&sidecar)
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn open_immutable_database(path: &Path) -> rusqlite::Result<Connection> {
    let canonical = path
        .canonicalize()
        .map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))?;
    let uri = immutable_sqlite_uri(&canonical)
        .ok_or_else(|| rusqlite::Error::InvalidPath(canonical.clone()))?;
    let connection = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    connection.execute_batch(
        "
        PRAGMA query_only = ON;
        PRAGMA temp_store = MEMORY;
        ",
    )?;
    Ok(connection)
}

fn inspect_schema(connection: &Connection) -> Result<WorkspaceSchemaPlan, WorkspacePlanError> {
    let quick_check = connection
        .query_row("PRAGMA quick_check(1);", [], |row| row.get::<_, String>(0))
        .map_err(classify_database_error)?;
    if quick_check != "ok" {
        return Err(workspace_error(
            "CORRUPT_DATABASE",
            format!("SQLite quick_check failed: {quick_check}"),
        ));
    }

    let table_count = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'schema_migrations';",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(classify_database_error)?;
    if table_count != 1 {
        return Err(workspace_error(
            "UNSUPPORTED_SCHEMA",
            "schema_migrations table is missing",
        ));
    }

    let mut statement = connection
        .prepare("SELECT version, name FROM schema_migrations ORDER BY version;")
        .map_err(|error| unsupported_schema_error("cannot read schema_migrations", error))?;
    let applied = statement
        .query_map([], |row| {
            Ok(MigrationPlanItem {
                version: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(|error| unsupported_schema_error("cannot query schema_migrations", error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| unsupported_schema_error("invalid schema_migrations row", error))?;
    let catalog = schema::migration_catalog();
    if applied.is_empty() {
        return Err(workspace_error(
            "UNSUPPORTED_SCHEMA",
            "schema_migrations has no applied AOPMem migration",
        ));
    }
    if applied.len() > catalog.len() {
        return Err(workspace_error(
            "UNSUPPORTED_SCHEMA",
            format!(
                "database schema is newer than this binary: found {} applied migrations, expected at most {}",
                applied.len(),
                catalog.len()
            ),
        ));
    }
    for (index, actual) in applied.iter().enumerate() {
        let expected = catalog[index];
        if actual.version != expected.version || actual.name != expected.name {
            return Err(workspace_error(
                "UNSUPPORTED_SCHEMA",
                format!(
                    "migration sequence mismatch at position {}: expected {} ({}) but found {} ({})",
                    index + 1,
                    expected.version,
                    expected.name,
                    actual.version,
                    actual.name
                ),
            ));
        }
    }

    let pending = catalog
        .iter()
        .skip(applied.len())
        .map(|migration| MigrationPlanItem {
            version: migration.version.to_string(),
            name: migration.name.to_string(),
        })
        .collect::<Vec<_>>();
    let target_version = catalog
        .last()
        .map_or_else(String::new, |migration| migration.version.to_string());
    let current_version = applied
        .last()
        .map_or_else(String::new, |migration| migration.version.clone());

    Ok(WorkspaceSchemaPlan {
        current_version,
        target_version,
        applied_migrations: applied,
        pending_migrations: pending,
    })
}

fn classify_database_error(error: rusqlite::Error) -> WorkspacePlanError {
    let is_corruption = matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase)
    );
    if is_corruption {
        workspace_error(
            "CORRUPT_DATABASE",
            format!("cannot read SQLite database: {error}"),
        )
    } else {
        workspace_error(
            "DATABASE_READ_FAILED",
            format!("cannot read workspace database: {error}"),
        )
    }
}

fn unsupported_schema_error(context: &str, error: rusqlite::Error) -> WorkspacePlanError {
    workspace_error("UNSUPPORTED_SCHEMA", format!("{context}: {error}"))
}

fn workspace_error(code: &'static str, message: impl Into<String>) -> WorkspacePlanError {
    WorkspacePlanError {
        code,
        message: message.into(),
        fix_hint: None,
    }
}

fn regular_file_size(path: &Path) -> Option<u64> {
    fs::symlink_metadata(path)
        .ok()
        .filter(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        .map(|metadata| metadata.len())
}

fn installed_binary_size(paths: &AopmemPaths) -> Result<u64, UpgradePlanError> {
    match fs::symlink_metadata(paths.bin()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(UpgradePlanError::InspectPath {
                path: paths.bin().clone(),
                source,
            });
        }
        Ok(_) => validate_optional_managed_directory(paths.home(), paths.bin())?,
    }

    let binary_path = paths.bin().join(installed_binary_name());
    storage::validate_optional_regular_file(&binary_path).map_err(|source| {
        UpgradePlanError::InspectPath {
            path: binary_path.clone(),
            source,
        }
    })?;
    Ok(regular_file_size(&binary_path).unwrap_or_default())
}

#[cfg(windows)]
fn installed_binary_name() -> &'static str {
    "aopmem.exe"
}

#[cfg(not(windows))]
fn installed_binary_name() -> &'static str {
    "aopmem"
}

fn nearest_existing_directory(path: &Path) -> Result<PathBuf, UpgradePlanError> {
    let absolute_path;
    let path = if path.is_absolute() {
        path
    } else {
        absolute_path = std::env::current_dir()
            .map_err(|source| UpgradePlanError::InspectPath {
                path: path.to_path_buf(),
                source,
            })?
            .join(path);
        absolute_path.as_path()
    };
    let mut candidate = Some(path);
    while let Some(current) = candidate {
        match fs::symlink_metadata(current) {
            Ok(_) => {
                storage::validate_real_directory(current).map_err(|source| {
                    UpgradePlanError::InspectPath {
                        path: current.to_path_buf(),
                        source,
                    }
                })?;
                return Ok(current.to_path_buf());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                candidate = current.parent();
            }
            Err(source) => {
                return Err(UpgradePlanError::InspectPath {
                    path: current.to_path_buf(),
                    source,
                });
            }
        }
    }
    Err(UpgradePlanError::InspectPath {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::NotFound, "no existing directory ancestor"),
    })
}

fn immutable_sqlite_uri(path: &Path) -> Option<String> {
    #[cfg(unix)]
    let raw_path = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(windows)]
    let raw_path = normalize_windows_sqlite_path(path.to_str()?).into_bytes();
    #[cfg(not(any(unix, windows)))]
    let raw_path = path.to_str()?.as_bytes().to_vec();

    let mut encoded = String::with_capacity(raw_path.len() + 32);
    for byte in raw_path {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte))
            }
            _ => {
                use std::fmt::Write as _;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    #[cfg(windows)]
    if encoded.as_bytes().get(1) == Some(&b':') {
        encoded.insert(0, '/');
    }
    Some(format!("file:{encoded}?mode=ro&immutable=1"))
}

#[cfg(any(windows, test))]
fn normalize_windows_sqlite_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if let Some(unc_path) = normalized.strip_prefix("//?/UNC/") {
        format!("//{unc_path}")
    } else if let Some(drive_path) = normalized.strip_prefix("//?/") {
        drive_path.to_string()
    } else {
        normalized
    }
}

fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    value.into()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(target_os = "macos")]
fn system_available_bytes(path: &Path) -> io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is NUL-terminated and `stats` points to writable storage.
    let result = unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful `statvfs` initialized the output structure.
    let stats = unsafe { stats.assume_init() };
    let available = u128::from(stats.f_bavail) * u128::from(stats.f_frsize);
    u64::try_from(available)
        .map_err(|_| io::Error::other("available disk space exceeds supported range"))
}

#[cfg(windows)]
fn system_available_bytes(path: &Path) -> io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut available = 0_u64;
    // SAFETY: `wide` is NUL-terminated and `available` is a valid output pointer.
    let result = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(available)
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn system_available_bytes(_path: &Path) -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "upgrade disk probing is supported only on macOS and Windows",
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::ffi::OsString;
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";

    struct FixedDiskProbe(u64);

    impl DiskSpaceProbe for FixedDiskProbe {
        fn available_bytes(&self, _path: &Path) -> io::Result<u64> {
            Ok(self.0)
        }
    }

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

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        env::temp_dir().join(format!("aopmem-stage-031-{name}-{nanos}"))
    }

    fn create_v010_workspace(home: &Path, key: &str) -> PathBuf {
        let root = home.join("workspaces").join(key);
        fs::create_dir_all(&root).expect("workspace root should create");
        let database = root.join(DATABASE_FILE_NAME);
        let connection = Connection::open(&database).expect("fixture database should open");
        connection
            .execute_batch(
                "
                CREATE TABLE schema_migrations (
                    version TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO schema_migrations (version, name) VALUES ('001', '001_init');
                ",
            )
            .expect("v0.1 migration marker should create");
        drop(connection);
        database
    }

    fn tree_fingerprint(root: &Path) -> BTreeMap<String, (u64, u64, u128)> {
        fn visit(root: &Path, path: &Path, output: &mut BTreeMap<String, (u64, u64, u128)>) {
            let mut entries = fs::read_dir(path)
                .expect("fingerprinted directory should read")
                .collect::<Result<Vec<_>, _>>()
                .expect("directory entries should read");
            entries.sort_by_key(fs::DirEntry::file_name);
            for entry in entries {
                let entry_path = entry.path();
                let relative = entry_path
                    .strip_prefix(root)
                    .expect("entry should remain below root")
                    .to_string_lossy()
                    .into_owned();
                let metadata = fs::symlink_metadata(&entry_path)
                    .expect("fingerprinted entry metadata should read");
                let modified = metadata
                    .modified()
                    .expect("fingerprinted mtime should read")
                    .duration_since(UNIX_EPOCH)
                    .expect("mtime should be after epoch")
                    .as_nanos();
                let hash = if metadata.is_file() {
                    let bytes = fs::read(&entry_path).expect("fingerprinted file should read");
                    let mut hasher = DefaultHasher::new();
                    bytes.hash(&mut hasher);
                    hasher.finish()
                } else {
                    0
                };
                output.insert(relative, (metadata.len(), hash, modified));
                if metadata.is_dir() {
                    visit(root, &entry_path, output);
                }
            }
        }

        let mut output = BTreeMap::new();
        if root.exists() {
            visit(root, root, &mut output);
        }
        output
    }

    #[test]
    fn plan_is_read_only_deterministic_and_lists_v010_workspaces_in_stable_order() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("ordered-read-only");
        let _home = EnvGuard::set(&home);
        let second = create_v010_workspace(&home, "zeta-workspace");
        let first = create_v010_workspace(&home, "alpha-workspace");
        let before = tree_fingerprint(&home);

        let report = plan_all_workspaces_with_probe(&FixedDiskProbe(u64::MAX))
            .expect("upgrade plan should succeed");
        let second_report = plan_all_workspaces_with_probe(&FixedDiskProbe(u64::MAX))
            .expect("repeated upgrade plan should succeed");

        assert_eq!(report, second_report);
        assert!(report.plan_only);
        assert!(!report.writes_performed);
        assert_eq!(report.scope, "all_workspaces");
        assert_eq!(report.binary_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(report.workspace_count, 2);
        assert_eq!(report.workspaces[0].workspace_key, "alpha-workspace");
        assert_eq!(report.workspaces[1].workspace_key, "zeta-workspace");
        assert!(report.ready);
        for workspace in &report.workspaces {
            assert_eq!(workspace.status, WorkspacePlanStatus::MigrationRequired);
            let schema = workspace
                .schema
                .as_ref()
                .expect("schema should be reported");
            assert_eq!(schema.current_version, "001");
            assert_eq!(schema.target_version, "004");
            assert_eq!(
                schema
                    .pending_migrations
                    .iter()
                    .map(|migration| migration.version.as_str())
                    .collect::<Vec<_>>(),
                vec!["002", "003", "004"]
            );
        }
        assert!(!path_with_suffix(&first, "-wal").exists());
        assert!(!path_with_suffix(&second, "-shm").exists());
        assert_eq!(tree_fingerprint(&home), before);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn empty_installation_is_a_successful_plan_and_creates_nothing() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("missing-home");
        let _home = EnvGuard::set(&home);

        let report = plan_all_workspaces_with_probe(&FixedDiskProbe(0))
            .expect("empty installation should plan successfully");

        assert_eq!(report.workspace_count, 0);
        assert!(report.workspaces.is_empty());
        assert!(report.ready);
        assert!(report.disk_space.sufficient);
        assert!(!home.exists(), "plan must not create AOPMEM_HOME");
    }

    #[test]
    fn plan_does_not_scan_or_migrate_legacy_files_outside_workspaces() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("legacy-file-ignored");
        let _home = EnvGuard::set(&home);
        fs::create_dir_all(&home).expect("AOPMem home fixture should create");
        fs::write(home.join("memory.json"), b"legacy file MVP proof")
            .expect("legacy fixture should write");
        let before = tree_fingerprint(&home);

        let report = plan_all_workspaces_with_probe(&FixedDiskProbe(u64::MAX))
            .expect("legacy file should not enter v0.1 workspace planning");

        assert_eq!(report.workspace_count, 0);
        assert!(report.workspaces.is_empty());
        assert_eq!(tree_fingerprint(&home), before);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn corrupt_database_is_an_exact_per_workspace_blocker() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("corrupt");
        let _home = EnvGuard::set(&home);
        let root = home.join("workspaces/corrupt-workspace");
        fs::create_dir_all(&root).expect("workspace root should create");
        fs::write(root.join(DATABASE_FILE_NAME), b"not sqlite")
            .expect("corrupt database should write");
        let before = tree_fingerprint(&home);

        let report = plan_all_workspaces_with_probe(&FixedDiskProbe(u64::MAX))
            .expect("corrupt workspace should remain a plan result");
        let workspace = &report.workspaces[0];

        assert!(!report.ready);
        assert_eq!(workspace.workspace_key, "corrupt-workspace");
        assert_eq!(workspace.status, WorkspacePlanStatus::Blocked);
        assert_eq!(
            workspace.error.as_ref().expect("error should exist").code,
            "CORRUPT_DATABASE"
        );
        assert_eq!(tree_fingerprint(&home), before);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn newer_schema_is_rejected_without_running_migrations() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("newer-schema");
        let _home = EnvGuard::set(&home);
        let database = create_v010_workspace(&home, "newer-workspace");
        let connection = Connection::open(&database).expect("fixture should reopen");
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES ('999', '999_future');",
                [],
            )
            .expect("future migration marker should insert");
        drop(connection);
        let before = tree_fingerprint(&home);

        let report = plan_all_workspaces_with_probe(&FixedDiskProbe(u64::MAX))
            .expect("newer schema should remain a plan result");
        let workspace = &report.workspaces[0];

        assert!(!report.ready);
        assert_eq!(workspace.status, WorkspacePlanStatus::Blocked);
        assert_eq!(
            workspace.error.as_ref().expect("error should exist").code,
            "UNSUPPORTED_SCHEMA"
        );
        assert_eq!(tree_fingerprint(&home), before);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn insufficient_disk_is_reported_without_hiding_workspace_schema() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("insufficient-disk");
        let _home = EnvGuard::set(&home);
        create_v010_workspace(&home, "disk-workspace");

        let report = plan_all_workspaces_with_probe(&FixedDiskProbe(0))
            .expect("disk shortage should remain a plan result");

        assert!(!report.ready);
        assert!(!report.disk_space.sufficient);
        assert!(report.disk_space.minimum_required_bytes > 0);
        assert_eq!(
            report.workspaces[0].status,
            WorkspacePlanStatus::MigrationRequired
        );

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn database_sidecar_blocks_immutable_inspection_and_is_not_changed() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let home = temp_path("sidecar");
        let _home = EnvGuard::set(&home);
        let database = create_v010_workspace(&home, "sidecar-workspace");
        let sidecar = path_with_suffix(&database, "-wal");
        fs::write(&sidecar, b"pending wal proof").expect("sidecar fixture should write");
        let before = tree_fingerprint(&home);

        let report = plan_all_workspaces_with_probe(&FixedDiskProbe(u64::MAX))
            .expect("sidecar should remain a plan result");

        assert!(!report.ready);
        assert_eq!(
            report.workspaces[0]
                .error
                .as_ref()
                .expect("error should exist")
                .code,
            "UNSAFE_DATABASE_SIDECAR"
        );
        assert_eq!(
            report.workspaces[0]
                .error
                .as_ref()
                .and_then(|error| error.fix_hint.as_deref()),
            Some(
                "run `aopmem upgrade prepare --all-workspaces --json` while AOPMem processes are closed"
            )
        );
        assert_eq!(tree_fingerprint(&home), before);

        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn immutable_uri_percent_encodes_reserved_and_unicode_bytes() {
        let uri = immutable_sqlite_uri(Path::new("/tmp/AOPMem #1/данные.sqlite"))
            .expect("URI should encode");
        assert!(uri.starts_with("file:/tmp/AOPMem%20%231/"));
        assert!(uri.ends_with("?mode=ro&immutable=1"));
        assert!(!uri.contains('#'));
        assert!(!uri.contains('д'));
    }

    #[test]
    fn windows_extended_paths_normalize_for_sqlite_uri() {
        assert_eq!(
            normalize_windows_sqlite_path(r"\\?\C:\Users\A OPMem\aopmem.sqlite"),
            "C:/Users/A OPMem/aopmem.sqlite"
        );
        assert_eq!(
            normalize_windows_sqlite_path(r"\\?\UNC\server\share\aopmem.sqlite"),
            "//server/share/aopmem.sqlite"
        );
    }
}
