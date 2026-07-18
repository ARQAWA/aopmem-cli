//! Official, read-only operational audit snapshot repair.

use std::fs;
use std::io;
use std::time::Instant;

use serde::Serialize;

use crate::{audit, storage};

const MAX_WORKSPACES: usize = 10_000;
const MAX_WORKSPACE_KEY_BYTES: usize = 255;
const MAX_DISCOVERY_NAME_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceRepairSuccess {
    pub workspace_key: String,
    #[serde(flatten)]
    pub repair: audit::AuditRepairReport,
    pub observability_recorded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceRepairFailure {
    pub workspace_key: String,
    pub code: &'static str,
    pub message: &'static str,
    pub io_kind: Option<String>,
    pub raw_os_error: Option<i32>,
    pub marker_retained: bool,
    pub operational_db_written: bool,
    pub observability_recorded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum WorkspaceRepairResult {
    Success(WorkspaceRepairSuccess),
    Failure(WorkspaceRepairFailure),
}

impl WorkspaceRepairResult {
    #[must_use]
    pub fn workspace_key(&self) -> &str {
        match self {
            Self::Success(result) => &result.workspace_key,
            Self::Failure(result) => &result.workspace_key,
        }
    }

    #[must_use]
    pub fn succeeded(&self) -> bool {
        matches!(self, Self::Success(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditRepairExecution {
    pub selector: &'static str,
    pub workspace_count: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub partial_failure: bool,
    pub operational_db_written: bool,
    pub workspaces: Vec<WorkspaceRepairResult>,
}

#[derive(Debug)]
pub enum DiscoveryError {
    Io(io::Error),
    TooManyWorkspaces,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::TooManyWorkspaces => {
                write!(formatter, "workspace count exceeds {MAX_WORKSPACES}")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

impl From<io::Error> for DiscoveryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn repair_current(
    workspace_key: String,
    workspace_paths: storage::WorkspacePaths,
) -> AuditRepairExecution {
    execution(
        "current_workspace",
        vec![repair_one(workspace_key, workspace_paths)],
    )
}

pub fn repair_all(paths: &storage::AopmemPaths) -> Result<AuditRepairExecution, DiscoveryError> {
    let targets = discover(paths)?;
    let results = targets
        .into_iter()
        .map(|target| match target {
            Discovered::Valid(key, paths) => repair_one(key, *paths),
            Discovered::Unsafe(key) => WorkspaceRepairResult::Failure(failure(
                key,
                "AUDIT_REPAIR_UNSAFE_WORKSPACE",
                "workspace entry is unsafe",
                None,
                true,
            )),
        })
        .collect();
    Ok(execution("all_workspaces", results))
}

fn execution(
    selector: &'static str,
    workspaces: Vec<WorkspaceRepairResult>,
) -> AuditRepairExecution {
    let succeeded = workspaces
        .iter()
        .filter(|result| result.succeeded())
        .count();
    let failed = workspaces.len().saturating_sub(succeeded);
    AuditRepairExecution {
        selector,
        workspace_count: workspaces.len(),
        succeeded,
        failed,
        partial_failure: failed > 0,
        operational_db_written: false,
        workspaces,
    }
}

enum Discovered {
    Valid(String, Box<storage::WorkspacePaths>),
    Unsafe(String),
}

fn discover(paths: &storage::AopmemPaths) -> Result<Vec<Discovered>, DiscoveryError> {
    let root = paths.workspaces();
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(
            io::Error::new(io::ErrorKind::PermissionDenied, "unsafe workspaces root").into(),
        );
    }

    let mut entries = Vec::new();
    let mut name_bytes = 0_usize;
    let mut unreadable_entries = 0_usize;
    let mut overflow = false;
    for (scanned_entries, entry) in fs::read_dir(root)?.enumerate() {
        if scanned_entries == MAX_WORKSPACES {
            overflow = true;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                unreadable_entries = unreadable_entries.saturating_add(1);
                continue;
            }
        };
        let name = entry.file_name();
        let sort_key = os_name_bytes(&name);
        name_bytes = name_bytes
            .checked_add(sort_key.len())
            .ok_or(DiscoveryError::TooManyWorkspaces)?;
        if name_bytes > MAX_DISCOVERY_NAME_BYTES {
            overflow = true;
            break;
        }
        entries.push((sort_key, name, fs::symlink_metadata(entry.path())));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let mut targets = Vec::with_capacity(entries.len());
    for (index, (_, name, metadata)) in entries.into_iter().enumerate() {
        let safe_label = format!("unsafe_entry_{index:05}");
        let Ok(metadata) = metadata else {
            targets.push(Discovered::Unsafe(safe_label));
            continue;
        };
        let Some(key) = name.to_str().map(str::to_owned) else {
            targets.push(Discovered::Unsafe(safe_label));
            continue;
        };
        if !valid_workspace_key(&key) || !metadata.is_dir() || is_link_or_reparse(&metadata) {
            targets.push(Discovered::Unsafe(if valid_workspace_key(&key) {
                key
            } else {
                safe_label
            }));
            continue;
        }
        targets.push(Discovered::Valid(
            key.clone(),
            Box::new(storage::workspace_paths_for_key(paths, &key)),
        ));
    }
    for index in 0..unreadable_entries {
        targets.push(Discovered::Unsafe(format!("unreadable_entry_{index:05}")));
    }
    if overflow {
        targets.push(Discovered::Unsafe("workspace_limit_exceeded".to_string()));
    }
    Ok(targets)
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
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

#[cfg(unix)]
fn os_name_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    name.as_bytes().to_vec()
}

#[cfg(windows)]
fn os_name_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    name.encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>()
}

fn valid_workspace_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_WORKSPACE_KEY_BYTES
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn repair_one(
    workspace_key: String,
    workspace_paths: storage::WorkspacePaths,
) -> WorkspaceRepairResult {
    let started_at = Instant::now();
    let lock = match audit::acquire_snapshot_lock(workspace_paths.audit_git()) {
        Ok(lock) => lock,
        Err(error) => {
            return WorkspaceRepairResult::Failure(failure(
                workspace_key,
                "AUDIT_REPAIR_LOCK_FAILED",
                "snapshot lock could not be acquired safely",
                snapshot_io_error(&error),
                marker_retained(&workspace_paths),
            ));
        }
    };
    match audit::pending_snapshot_marker_locked(&lock) {
        Ok(false) => {
            return WorkspaceRepairResult::Success(WorkspaceRepairSuccess {
                workspace_key,
                repair: audit::AuditRepairReport {
                    status: audit::AuditRepairStatus::AlreadyClean,
                    duration_ms: elapsed_ms(started_at),
                    bytes_written: 0,
                    sha256: None,
                    git_commit: None,
                    marker_present_before: false,
                    marker_present_after: false,
                    operational_db_written: false,
                },
                observability_recorded: false,
            });
        }
        Ok(true) => {}
        Err(error) => {
            return WorkspaceRepairResult::Failure(failure(
                workspace_key,
                "AUDIT_REPAIR_MARKER_READ_FAILED",
                "pending snapshot marker could not be inspected safely",
                snapshot_io_error(&error),
                true,
            ));
        }
    }
    let connection = match storage::open_workspace_db_for_audit_repair(&workspace_paths) {
        Ok(connection) => connection,
        Err(error) => {
            return WorkspaceRepairResult::Failure(failure(
                workspace_key,
                "AUDIT_REPAIR_DB_READ_FAILED",
                "operational database could not be opened read-only",
                read_only_io_error(&error),
                locked_marker_retained(&lock),
            ));
        }
    };
    match audit::repair_sql_snapshot_locked(&connection, &lock, started_at) {
        Ok(repair) => WorkspaceRepairResult::Success(WorkspaceRepairSuccess {
            workspace_key,
            repair,
            observability_recorded: false,
        }),
        Err(error) => WorkspaceRepairResult::Failure(failure(
            workspace_key,
            "AUDIT_REPAIR_FAILED",
            "audit snapshot repair failed",
            snapshot_io_error(&error),
            locked_marker_retained(&lock),
        )),
    }
}

fn failure(
    workspace_key: String,
    code: &'static str,
    message: &'static str,
    error: Option<&io::Error>,
    marker_retained: bool,
) -> WorkspaceRepairFailure {
    WorkspaceRepairFailure {
        workspace_key,
        code,
        message,
        io_kind: error.map(|error| format!("{:?}", error.kind())),
        raw_os_error: error.and_then(io::Error::raw_os_error),
        marker_retained,
        operational_db_written: false,
        observability_recorded: false,
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn marker_retained(workspace_paths: &storage::WorkspacePaths) -> bool {
    audit::has_pending_snapshot(workspace_paths.audit_git()).unwrap_or(true)
}

fn locked_marker_retained(lock: &audit::SnapshotLock) -> bool {
    audit::pending_snapshot_marker_locked(lock).unwrap_or(true)
}

fn snapshot_io_error(error: &audit::SnapshotError) -> Option<&io::Error> {
    match error {
        audit::SnapshotError::Io(error) => Some(error),
        audit::SnapshotError::Db(_) | audit::SnapshotError::Redaction(_) => None,
    }
}

fn read_only_io_error(error: &storage::OpenWorkspaceReadOnlyError) -> Option<&io::Error> {
    match error {
        storage::OpenWorkspaceReadOnlyError::UnsafePath(error) => Some(error),
        storage::OpenWorkspaceReadOnlyError::Missing(_)
        | storage::OpenWorkspaceReadOnlyError::Db(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct EnvGuard {
        key: &'static str,
        old: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &OsStr) -> Self {
            let old = std::env::var_os(key);
            // SAFETY: tests serialize process environment changes with the
            // install test lock.
            unsafe { std::env::set_var(key, value) };
            Self { key, old }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(value) => {
                    // SAFETY: guarded by the same process-wide test lock.
                    unsafe { std::env::set_var(self.key, value) };
                }
                None => {
                    // SAFETY: guarded by the same process-wide test lock.
                    unsafe { std::env::remove_var(self.key) };
                }
            }
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should follow UNIX epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("aopmem-stage-019-{name}-{nanos}"))
    }

    #[cfg(unix)]
    #[test]
    fn stage_019_all_workspaces_continues_past_unsafe_entry_in_stable_order() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let home = temp_path("discovery");
        let outside = temp_path("outside");
        let _home = EnvGuard::set("AOPMEM_HOME", home.as_os_str());
        let paths = storage::resolve_paths().expect("paths should resolve");
        storage::ensure_global_dirs(&paths).expect("global dirs should create");
        storage::ensure_workspace_dirs(&paths, "valid-b").expect("workspace B should create");
        storage::ensure_workspace_dirs(&paths, "valid-a").expect("workspace A should create");
        fs::create_dir_all(&outside).expect("outside should create");
        symlink(&outside, paths.workspaces().join("unsafe-link"))
            .expect("unsafe link should create");

        let first = repair_all(&paths).expect("discovery should continue");
        let second = repair_all(&paths).expect("replay should continue");
        let keys = first
            .workspaces
            .iter()
            .map(WorkspaceRepairResult::workspace_key)
            .collect::<Vec<_>>();
        let replay_keys = second
            .workspaces
            .iter()
            .map(WorkspaceRepairResult::workspace_key)
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["unsafe-link", "valid-a", "valid-b"]);
        assert_eq!(replay_keys, keys);
        assert_eq!(first.succeeded, 2);
        assert_eq!(first.failed, 1);

        fs::remove_dir_all(home).expect("home should remove");
        fs::remove_dir_all(outside).expect("outside should remove");
    }
}
