use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeMap;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::ffi::CString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as UnixMetadataExt, OpenOptionsExt as UnixOpenOptionsExt};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;
use uuid::Uuid;

use crate::storage::WorkspacePaths;

pub const ARTIFACT_RETENTION_DAYS: u64 = 7;
pub const ARTIFACT_MAX_BYTES: u64 = 1_000_000_000;
pub const ARTIFACT_LOCK_FILE_NAME: &str = ".artifacts.lock";

const TOOL_STDOUT_FILE_NAME: &str = "stdout.bin";
const TOOL_STDERR_FILE_NAME: &str = "stderr.bin";
const TOOL_RUN_DIR_PREFIX: &str = "tool-run-";
const TOOL_RUN_STAGING_SUFFIX: &str = ".tmp";
const ARTIFACT_LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const ARTIFACT_LOCK_RETRY: Duration = Duration::from_millis(25);
const MAX_ARTIFACT_TREE_ENTRIES: usize = 200_000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArtifactDay {
    year: u16,
    month: u8,
    day: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanupReport {
    pub artifact_root: String,
    pub today_dir: String,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub deleted_dirs: Vec<String>,
    pub deleted_files: Vec<String>,
    pub kept_dirs: Vec<String>,
    pub deleted_paths: Vec<String>,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactEntry {
    kind: ArtifactEntryKind,
    bytes: u64,
    modified_at: Option<SystemTime>,
    identity: ArtifactIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArtifactIdentity {
    volume: u64,
    file: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactTreeSnapshot {
    root_identity: ArtifactIdentity,
    entries: BTreeMap<PathBuf, ArtifactEntry>,
    day_dirs: BTreeMap<ArtifactDay, PathBuf>,
    stale_staging_dirs: Vec<PathBuf>,
    bytes: u64,
}

/// Stable artifact-root object used for every destructive cleanup operation.
///
/// Paths remain only logical report labels. Deletion is performed relative to
/// this open root on Unix and against identity-checked handles on Windows.
struct ArtifactRootAnchor {
    logical_root: PathBuf,
    directory: File,
    identity: ArtifactIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactLockMode {
    CaptureShared,
    CleanupExclusive,
}

impl fmt::Display for ArtifactLockMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CaptureShared => formatter.write_str("capture-shared"),
            Self::CleanupExclusive => formatter.write_str("cleanup-exclusive"),
        }
    }
}

/// Permanent advisory lock for all artifact readers and cleanup writers.
///
/// The file is never removed or truncated. Removing it could create a second
/// inode and allow two processes to believe they own the same lock.
#[derive(Debug)]
struct ArtifactTreeLock {
    file: File,
}

/// Writable files owned by one unpublished tool artifact capture.
pub(crate) struct ToolArtifactCaptureFiles {
    pub stdout: File,
    pub stderr: File,
}

/// Workspace-relative paths created by one published tool artifact capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublishedToolArtifactPaths {
    pub stdout: String,
    pub stderr: String,
}

/// RAII guard for one code-owned unpublished tool artifact directory.
pub(crate) struct ToolArtifactStaging {
    _artifact_lock: ArtifactTreeLock,
    day_dir: PathBuf,
    staging_dir: PathBuf,
    final_dir: PathBuf,
    stdout_relative: String,
    stderr_relative: String,
    published: bool,
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error("invalid artifact day format: {0}")]
    InvalidDay(String),
    #[error("artifact {mode} lock was not acquired within {timeout_ms} ms")]
    LockTimeout {
        mode: ArtifactLockMode,
        timeout_ms: u64,
    },
    #[error("artifact cleanup stopped at {failed_path}: {source}")]
    CleanupPartial {
        failed_path: String,
        report: Box<CleanupReport>,
        #[source]
        source: io::Error,
    },
    #[error("artifact cleanup state is unknown after failure at {failed_path}: {source}")]
    CleanupStateUnknown {
        failed_path: String,
        deleted_paths: Vec<String>,
        #[source]
        source: io::Error,
    },
    #[error(
        "artifact retention limit was not met: {bytes_after} bytes remain, limit is {max_bytes}"
    )]
    RetentionLimitNotMet {
        bytes_after: u64,
        max_bytes: u64,
        report: Box<CleanupReport>,
    },
}

impl ArtifactError {
    pub fn cleanup_report(&self) -> Option<&CleanupReport> {
        match self {
            Self::CleanupPartial { report, .. } | Self::RetentionLimitNotMet { report, .. } => {
                Some(report)
            }
            Self::Io(_)
            | Self::Db(_)
            | Self::InvalidDay(_)
            | Self::LockTimeout { .. }
            | Self::CleanupStateUnknown { .. } => None,
        }
    }

    pub fn deleted_paths(&self) -> Option<&[String]> {
        match self {
            Self::CleanupPartial { report, .. } | Self::RetentionLimitNotMet { report, .. } => {
                Some(&report.deleted_paths)
            }
            Self::CleanupStateUnknown { deleted_paths, .. } => Some(deleted_paths),
            Self::Io(_) | Self::Db(_) | Self::InvalidDay(_) | Self::LockTimeout { .. } => None,
        }
    }
}

impl ArtifactTreeLock {
    fn acquire(
        workspace_root: &Path,
        artifacts_root: &Path,
        mode: ArtifactLockMode,
    ) -> Result<Self, ArtifactError> {
        Self::acquire_with_timeout(workspace_root, artifacts_root, mode, ARTIFACT_LOCK_TIMEOUT)
    }

    fn acquire_with_timeout(
        workspace_root: &Path,
        artifacts_root: &Path,
        mode: ArtifactLockMode,
        timeout: Duration,
    ) -> Result<Self, ArtifactError> {
        let artifacts_root = ensure_secure_direct_directory(workspace_root, artifacts_root)?;
        let lock_path = artifacts_root.join(ARTIFACT_LOCK_FILE_NAME);
        crate::storage::validate_optional_regular_file(&lock_path)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        validate_secure_regular_file(&artifacts_root, &lock_path)?;

        let started = Instant::now();
        loop {
            let result = match mode {
                ArtifactLockMode::CaptureShared => File::try_lock_shared(&file),
                ArtifactLockMode::CleanupExclusive => File::try_lock(&file),
            };
            match result {
                Ok(()) => return Ok(Self { file }),
                Err(fs::TryLockError::WouldBlock) => {
                    if started.elapsed() >= timeout {
                        return Err(ArtifactError::LockTimeout {
                            mode,
                            timeout_ms: timeout.as_millis().try_into().unwrap_or(u64::MAX),
                        });
                    }
                    thread::sleep(ARTIFACT_LOCK_RETRY.min(timeout));
                }
                Err(fs::TryLockError::Error(error)) => return Err(error.into()),
            }
        }
    }
}

impl Drop for ArtifactTreeLock {
    fn drop(&mut self) {
        let _ = File::unlock(&self.file);
    }
}

impl ToolArtifactStaging {
    /// Creates one secure same-day staging directory and two new capture files.
    pub(crate) fn create(
        workspace_paths: &WorkspacePaths,
        connection: &Connection,
    ) -> Result<(Self, ToolArtifactCaptureFiles), ArtifactError> {
        let run_id = Uuid::new_v4().simple().to_string();
        Self::create_with_run_id(workspace_paths, connection, &run_id)
    }

    fn create_with_run_id(
        workspace_paths: &WorkspacePaths,
        connection: &Connection,
        run_id: &str,
    ) -> Result<(Self, ToolArtifactCaptureFiles), ArtifactError> {
        if !is_lower_hex_run_id(run_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tool artifact run id is not a code-owned UUID",
            )
            .into());
        }

        let today = sql_artifact_day(connection, "SELECT date('now', 'localtime')")?;
        let artifact_lock = ArtifactTreeLock::acquire(
            workspace_paths.root(),
            workspace_paths.artifacts(),
            ArtifactLockMode::CaptureShared,
        )?;
        let artifacts_root =
            ensure_secure_direct_directory(workspace_paths.root(), workspace_paths.artifacts())?;
        let day_dir = ensure_secure_direct_directory(
            &artifacts_root,
            &artifacts_root.join(today.folder_name()),
        )?;
        let final_name = format!("{TOOL_RUN_DIR_PREFIX}{run_id}");
        let staging_name = format!(".{final_name}{TOOL_RUN_STAGING_SUFFIX}");
        let staging_dir = day_dir.join(&staging_name);
        let final_dir = day_dir.join(&final_name);
        ensure_entry_missing(&final_dir)?;
        fs::create_dir(&staging_dir)?;
        validate_secure_direct_directory(&day_dir, &staging_dir)?;

        let mut staging = Self {
            _artifact_lock: artifact_lock,
            day_dir,
            staging_dir,
            final_dir,
            stdout_relative: format!(
                "artifacts/{}/{final_name}/{TOOL_STDOUT_FILE_NAME}",
                today.folder_name()
            ),
            stderr_relative: format!(
                "artifacts/{}/{final_name}/{TOOL_STDERR_FILE_NAME}",
                today.folder_name()
            ),
            published: false,
        };
        let stdout = open_new_capture_file(&staging.staging_dir.join(TOOL_STDOUT_FILE_NAME))?;
        let stderr = match open_new_capture_file(&staging.staging_dir.join(TOOL_STDERR_FILE_NAME)) {
            Ok(file) => file,
            Err(error) => {
                drop(stdout);
                staging.cleanup();
                return Err(error.into());
            }
        };

        Ok((staging, ToolArtifactCaptureFiles { stdout, stderr }))
    }

    /// Atomically publishes the complete capture without replacing an entry.
    pub(crate) fn publish(mut self) -> Result<PublishedToolArtifactPaths, ArtifactError> {
        validate_secure_direct_directory(&self.day_dir, &self.staging_dir)?;
        validate_secure_regular_file(
            &self.staging_dir,
            &self.staging_dir.join(TOOL_STDOUT_FILE_NAME),
        )?;
        validate_secure_regular_file(
            &self.staging_dir,
            &self.staging_dir.join(TOOL_STDERR_FILE_NAME),
        )?;
        ensure_entry_missing(&self.final_dir)?;
        atomic_publish_directory_no_replace(&self.staging_dir, &self.final_dir)?;
        self.published = true;

        Ok(PublishedToolArtifactPaths {
            stdout: self.stdout_relative.clone(),
            stderr: self.stderr_relative.clone(),
        })
    }

    fn cleanup(&mut self) {
        if self.published {
            return;
        }
        remove_unpublished_staging(&self.day_dir, &self.staging_dir);
    }
}

impl Drop for ToolArtifactStaging {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl ArtifactDay {
    pub fn parse(value: &str) -> Result<Self, ArtifactError> {
        if value.len() != 10 {
            return Err(ArtifactError::InvalidDay(value.to_string()));
        }

        let bytes = value.as_bytes();
        if bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(ArtifactError::InvalidDay(value.to_string()));
        }

        let year = parse_number(&value[0..4])?;
        let month = parse_number(&value[5..7])?;
        let day = parse_number(&value[8..10])?;

        if !(1..=12).contains(&month) {
            return Err(ArtifactError::InvalidDay(value.to_string()));
        }

        let max_day = days_in_month(year, month);
        if day == 0 || day > max_day {
            return Err(ArtifactError::InvalidDay(value.to_string()));
        }

        Ok(Self { year, month, day })
    }

    pub fn folder_name(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for ArtifactDay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

pub fn ensure_daily_artifact_dir(
    workspace_paths: &WorkspacePaths,
    day: &ArtifactDay,
) -> Result<PathBuf, ArtifactError> {
    let _artifact_lock = ArtifactTreeLock::acquire(
        workspace_paths.root(),
        workspace_paths.artifacts(),
        ArtifactLockMode::CaptureShared,
    )?;
    let artifacts_root =
        ensure_secure_direct_directory(workspace_paths.root(), workspace_paths.artifacts())?;
    ensure_daily_artifact_dir_in_root(&artifacts_root, day)
}

pub fn cleanup_workspace_artifacts(
    workspace_paths: &WorkspacePaths,
    connection: &Connection,
) -> Result<CleanupReport, ArtifactError> {
    let today = sql_artifact_day(connection, "SELECT date('now', 'localtime')")?;
    let oldest_kept = sql_artifact_day(connection, "SELECT date('now', 'localtime', '-6 days')")?;

    cleanup_workspace_artifacts_for_day(workspace_paths, &today, &oldest_kept, ARTIFACT_MAX_BYTES)
}

fn cleanup_workspace_artifacts_for_day(
    workspace_paths: &WorkspacePaths,
    today: &ArtifactDay,
    oldest_kept: &ArtifactDay,
    max_bytes: u64,
) -> Result<CleanupReport, ArtifactError> {
    cleanup_artifact_root_for_day(
        workspace_paths.root(),
        workspace_paths.artifacts(),
        today,
        oldest_kept,
        max_bytes,
    )
}

fn ensure_daily_artifact_dir_in_root(
    artifacts_root: &Path,
    day: &ArtifactDay,
) -> Result<PathBuf, ArtifactError> {
    let path = artifacts_root.join(day.folder_name());
    ensure_secure_direct_directory(artifacts_root, &path).map_err(ArtifactError::from)
}

fn is_lower_hex_run_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn strict_staging_name(value: &str) -> bool {
    let Some(run_id) = value
        .strip_prefix(".tool-run-")
        .and_then(|rest| rest.strip_suffix(TOOL_RUN_STAGING_SUFFIX))
    else {
        return false;
    };
    is_lower_hex_run_id(run_id)
}

fn ensure_secure_direct_directory(parent: &Path, directory: &Path) -> io::Result<PathBuf> {
    validate_secure_real_directory(parent)?;
    if directory.parent() != Some(parent) {
        return Err(unsafe_artifact_path(
            directory,
            "managed directory is not a direct child",
        ));
    }
    match fs::create_dir(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    validate_secure_direct_directory(parent, directory)?;
    Ok(directory.to_path_buf())
}

fn validate_secure_direct_directory(parent: &Path, directory: &Path) -> io::Result<()> {
    validate_secure_real_directory(parent)?;
    validate_secure_real_directory(directory)?;
    if directory.parent() != Some(parent) {
        return Err(unsafe_artifact_path(
            directory,
            "managed directory is not a direct child",
        ));
    }
    let canonical_parent = parent.canonicalize()?;
    let canonical_directory = directory.canonicalize()?;
    if canonical_directory.parent() != Some(canonical_parent.as_path()) {
        return Err(unsafe_artifact_path(
            directory,
            "managed directory escapes its parent",
        ));
    }
    Ok(())
}

fn validate_secure_real_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata_is_link_or_reparse(&metadata) {
        Ok(())
    } else {
        Err(unsafe_artifact_path(
            path,
            "managed directory is a link, reparse point, or non-directory",
        ))
    }
}

fn validate_secure_regular_file(parent: &Path, path: &Path) -> io::Result<()> {
    validate_secure_real_directory(parent)?;
    if path.parent() != Some(parent) {
        return Err(unsafe_artifact_path(
            path,
            "capture file is not a direct child",
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata_is_link_or_reparse(&metadata) {
        return Err(unsafe_artifact_path(
            path,
            "capture file is not a real regular file",
        ));
    }
    let canonical_parent = parent.canonicalize()?;
    let canonical_path = path.canonicalize()?;
    if canonical_path.parent() != Some(canonical_parent.as_path()) {
        return Err(unsafe_artifact_path(path, "capture file escapes staging"));
    }
    Ok(())
}

fn open_new_capture_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn ensure_entry_missing(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "tool artifact publish entry already exists: {}",
                path.display()
            ),
        )),
    }
}

fn remove_unpublished_staging(day_dir: &Path, staging_dir: &Path) {
    if staging_dir.parent() != Some(day_dir) {
        return;
    }
    let Some(artifacts_root) = day_dir.parent() else {
        return;
    };
    let Some(workspace_root) = artifacts_root.parent() else {
        return;
    };
    if validate_secure_direct_directory(workspace_root, artifacts_root).is_err()
        || validate_secure_direct_directory(artifacts_root, day_dir).is_err()
    {
        return;
    }
    match fs::symlink_metadata(staging_dir) {
        Ok(metadata) if metadata_is_link_or_reparse(&metadata) => {
            let root_identity = fs::symlink_metadata(artifacts_root)
                .and_then(|metadata| artifact_identity_at(artifacts_root, &metadata));
            if let Ok(root_identity) = root_identity {
                if let Ok(anchor) =
                    ArtifactRootAnchor::open(workspace_root, artifacts_root, root_identity)
                {
                    let _remove_result =
                        anchor.remove_reparse_leaf(day_dir, staging_dir, &metadata);
                }
            }
        }
        Ok(metadata) if metadata.is_dir() => {
            if let Ok(snapshot) = snapshot_artifact_tree(workspace_root, artifacts_root) {
                if let Ok(anchor) =
                    ArtifactRootAnchor::open(workspace_root, artifacts_root, snapshot.root_identity)
                {
                    let mut trace = CleanupTrace::default();
                    let _remove_result =
                        anchor.remove_tree(staging_dir, &snapshot.entries, &mut trace);
                }
            }
        }
        Ok(_) => {}
        Err(_) => {}
    }
}

fn unsafe_artifact_path(path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("unsafe artifact path {}: {reason}", path.display()),
    )
}

#[cfg(windows)]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(target_os = "macos")]
fn atomic_publish_directory_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn renamex_np(old: *const i8, new: *const i8, flags: u32) -> i32;
    }

    const RENAME_EXCL: u32 = 0x0000_0004;
    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "artifact source contains NUL"))?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "artifact destination contains NUL",
        )
    })?;
    let result = unsafe { renamex_np(source.as_ptr(), destination.as_ptr(), RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
fn atomic_publish_directory_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn renameat2(
            old_directory: i32,
            old_path: *const i8,
            new_directory: i32,
            new_path: *const i8,
            flags: u32,
        ) -> i32;
    }

    const AT_FDCWD: i32 = -100;
    const RENAME_NOREPLACE: u32 = 1;
    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "artifact source contains NUL"))?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "artifact destination contains NUL",
        )
    })?;
    let result = unsafe {
        renameat2(
            AT_FDCWD,
            source.as_ptr(),
            AT_FDCWD,
            destination.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn atomic_publish_directory_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn atomic_publish_directory_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    ensure_entry_missing(destination)?;
    fs::rename(source, destination)
}

fn cleanup_artifact_root_for_day(
    workspace_root: &Path,
    artifacts_root: &Path,
    today: &ArtifactDay,
    oldest_kept: &ArtifactDay,
    max_bytes: u64,
) -> Result<CleanupReport, ArtifactError> {
    let _artifact_lock = ArtifactTreeLock::acquire(
        workspace_root,
        artifacts_root,
        ArtifactLockMode::CleanupExclusive,
    )?;

    // Fail closed before creating today's directory or removing any entry.
    snapshot_artifact_tree(workspace_root, artifacts_root)?;
    let today_dir = ensure_daily_artifact_dir_in_root(artifacts_root, today)?;
    let baseline = snapshot_artifact_tree(workspace_root, artifacts_root)?;
    let anchor = ArtifactRootAnchor::open(workspace_root, artifacts_root, baseline.root_identity)?;
    let mut trace = CleanupTrace::default();
    let context = CleanupContext {
        workspace_root,
        artifacts_root,
        today,
        oldest_kept,
        max_bytes,
    };

    let final_snapshot = match apply_retention(&anchor, &context, baseline.clone(), &mut trace) {
        Ok(snapshot) => snapshot,
        Err(failure) => {
            let observed = match snapshot_artifact_tree(workspace_root, artifacts_root) {
                Ok(observed) => observed,
                Err(source) => {
                    return Err(ArtifactError::CleanupStateUnknown {
                        failed_path: failure.path.display().to_string(),
                        deleted_paths: display_paths(&trace.deleted_paths),
                        source,
                    });
                }
            };
            let report = cleanup_report(
                artifacts_root,
                &today_dir,
                &baseline,
                &observed,
                &trace,
                false,
            );
            return Err(ArtifactError::CleanupPartial {
                failed_path: failure.path.display().to_string(),
                report: Box::new(report),
                source: failure.source,
            });
        }
    };

    let complete = final_snapshot.bytes <= max_bytes;
    let report = cleanup_report(
        artifacts_root,
        &today_dir,
        &baseline,
        &final_snapshot,
        &trace,
        complete,
    );
    if !complete {
        return Err(ArtifactError::RetentionLimitNotMet {
            bytes_after: final_snapshot.bytes,
            max_bytes,
            report: Box::new(report),
        });
    }
    Ok(report)
}

#[derive(Debug, Default)]
struct CleanupTrace {
    deleted_dirs: Vec<PathBuf>,
    deleted_files: Vec<PathBuf>,
    deleted_paths: Vec<PathBuf>,
}

#[derive(Debug)]
struct DeleteFailure {
    path: PathBuf,
    source: io::Error,
}

struct CleanupContext<'a> {
    workspace_root: &'a Path,
    artifacts_root: &'a Path,
    today: &'a ArtifactDay,
    oldest_kept: &'a ArtifactDay,
    max_bytes: u64,
}

impl DeleteFailure {
    fn new(path: &Path, source: io::Error) -> Self {
        Self {
            path: path.to_path_buf(),
            source,
        }
    }
}

fn apply_retention(
    anchor: &ArtifactRootAnchor,
    context: &CleanupContext<'_>,
    mut current: ArtifactTreeSnapshot,
    trace: &mut CleanupTrace,
) -> Result<ArtifactTreeSnapshot, DeleteFailure> {
    let expired = current
        .day_dirs
        .iter()
        .filter(|(day, _)| *day < context.oldest_kept)
        .map(|(_, path)| path.clone())
        .collect::<Vec<_>>();
    for path in expired {
        current = remove_directory_candidate(
            anchor,
            context.workspace_root,
            context.artifacts_root,
            &path,
            current,
            trace,
        )?;
    }

    let stale = current.stale_staging_dirs.clone();
    for path in stale {
        current = remove_directory_candidate(
            anchor,
            context.workspace_root,
            context.artifacts_root,
            &path,
            current,
            trace,
        )?;
    }

    while current.bytes > context.max_bytes {
        let Some(path) = current
            .day_dirs
            .iter()
            .find(|(day, _)| *day < context.today)
            .map(|(_, path)| path.clone())
        else {
            break;
        };
        current = remove_directory_candidate(
            anchor,
            context.workspace_root,
            context.artifacts_root,
            &path,
            current,
            trace,
        )?;
    }

    if current.bytes > context.max_bytes {
        let mut files = current
            .entries
            .iter()
            .filter(|(_, entry)| entry.kind == ArtifactEntryKind::File)
            .filter(|(path, _)| {
                current
                    .day_dirs
                    .iter()
                    .any(|(day, day_path)| *day >= *context.today && path.starts_with(day_path))
            })
            .map(|(path, entry)| (entry.modified_at, path.clone()))
            .collect::<Vec<_>>();
        files.sort();

        for (_, path) in files {
            if current.bytes <= context.max_bytes {
                break;
            }
            current = remove_file_candidate(
                anchor,
                context.workspace_root,
                context.artifacts_root,
                &path,
                current,
                trace,
            )?;
        }
    }

    snapshot_artifact_tree(context.workspace_root, context.artifacts_root)
        .map_err(|source| DeleteFailure::new(context.artifacts_root, source))
}

fn remove_directory_candidate(
    anchor: &ArtifactRootAnchor,
    workspace_root: &Path,
    artifacts_root: &Path,
    path: &Path,
    expected: ArtifactTreeSnapshot,
    trace: &mut CleanupTrace,
) -> Result<ArtifactTreeSnapshot, DeleteFailure> {
    let observed = snapshot_artifact_tree(workspace_root, artifacts_root)
        .map_err(|source| DeleteFailure::new(artifacts_root, source))?;
    if observed != expected {
        return Err(DeleteFailure::new(
            artifacts_root,
            tree_changed_error(artifacts_root),
        ));
    }
    if observed.root_identity != anchor.identity {
        return Err(DeleteFailure::new(
            artifacts_root,
            tree_changed_error(artifacts_root),
        ));
    }
    anchor.remove_tree(path, &observed.entries, trace)?;
    trace.deleted_dirs.push(path.to_path_buf());
    snapshot_artifact_tree(workspace_root, artifacts_root)
        .map_err(|source| DeleteFailure::new(artifacts_root, source))
}

fn remove_file_candidate(
    anchor: &ArtifactRootAnchor,
    workspace_root: &Path,
    artifacts_root: &Path,
    path: &Path,
    expected: ArtifactTreeSnapshot,
    trace: &mut CleanupTrace,
) -> Result<ArtifactTreeSnapshot, DeleteFailure> {
    let observed = snapshot_artifact_tree(workspace_root, artifacts_root)
        .map_err(|source| DeleteFailure::new(artifacts_root, source))?;
    if observed != expected {
        return Err(DeleteFailure::new(
            artifacts_root,
            tree_changed_error(artifacts_root),
        ));
    }
    if observed.root_identity != anchor.identity {
        return Err(DeleteFailure::new(
            artifacts_root,
            tree_changed_error(artifacts_root),
        ));
    }
    anchor.remove_file(path, &observed.entries, trace)?;
    trace.deleted_files.push(path.to_path_buf());
    snapshot_artifact_tree(workspace_root, artifacts_root)
        .map_err(|source| DeleteFailure::new(artifacts_root, source))
}

fn snapshot_artifact_tree(
    workspace_root: &Path,
    artifacts_root: &Path,
) -> io::Result<ArtifactTreeSnapshot> {
    validate_secure_direct_directory(workspace_root, artifacts_root)?;
    let root_metadata = fs::symlink_metadata(artifacts_root)?;
    let root_identity = artifact_identity_at(artifacts_root, &root_metadata)?;
    let canonical_root = artifacts_root.canonicalize()?;
    let mut snapshot = ArtifactTreeSnapshot {
        root_identity,
        entries: BTreeMap::new(),
        day_dirs: BTreeMap::new(),
        stale_staging_dirs: Vec::new(),
        bytes: 0,
    };

    for path in sorted_children(artifacts_root)? {
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| unsafe_artifact_path(&path, "root entry name is not UTF-8"))?;
        if file_name == ARTIFACT_LOCK_FILE_NAME {
            validate_secure_regular_file(artifacts_root, &path)?;
            continue;
        }

        let day = ArtifactDay::parse(file_name)
            .map_err(|_| unsafe_artifact_path(&path, "unexpected artifact root entry"))?;
        validate_secure_direct_directory(artifacts_root, &path)?;
        insert_snapshot_entry(
            &mut snapshot,
            path.clone(),
            ArtifactEntryKind::Directory,
            &fs::symlink_metadata(&path)?,
        )?;
        snapshot.day_dirs.insert(day, path.clone());
        scan_day_tree(&canonical_root, &path, &mut snapshot)?;
    }
    snapshot.stale_staging_dirs.sort();
    let final_root_metadata = fs::symlink_metadata(artifacts_root)?;
    if metadata_is_link_or_reparse(&final_root_metadata)
        || !final_root_metadata.is_dir()
        || artifact_identity_at(artifacts_root, &final_root_metadata)? != root_identity
    {
        return Err(tree_changed_error(artifacts_root));
    }
    Ok(snapshot)
}

fn scan_day_tree(
    canonical_root: &Path,
    day_dir: &Path,
    snapshot: &mut ArtifactTreeSnapshot,
) -> io::Result<()> {
    let mut pending = vec![day_dir.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let mut children = sorted_children(&directory)?;
        children.reverse();
        for path in children {
            let metadata = fs::symlink_metadata(&path)?;
            if metadata_is_link_or_reparse(&metadata) {
                return Err(unsafe_artifact_path(
                    &path,
                    "links and reparse points are forbidden",
                ));
            }
            let canonical_path = path.canonicalize()?;
            if !canonical_path.starts_with(canonical_root) || canonical_path == canonical_root {
                return Err(unsafe_artifact_path(&path, "entry escapes artifact root"));
            }

            let file_name = path.file_name().and_then(|value| value.to_str());
            let staging_like = file_name.is_some_and(|value| value.starts_with(".tool-run-"));
            if staging_like {
                let is_direct = path.parent() == Some(day_dir);
                let is_valid = file_name.is_some_and(strict_staging_name);
                if !is_direct || !is_valid || !metadata.is_dir() {
                    return Err(unsafe_artifact_path(
                        &path,
                        "malformed tool artifact staging entry",
                    ));
                }
                snapshot.stale_staging_dirs.push(path.clone());
            }

            if metadata.is_dir() {
                insert_snapshot_entry(
                    snapshot,
                    path.clone(),
                    ArtifactEntryKind::Directory,
                    &metadata,
                )?;
                pending.push(path);
            } else if metadata.is_file() {
                insert_snapshot_entry(snapshot, path, ArtifactEntryKind::File, &metadata)?;
            } else {
                return Err(unsafe_artifact_path(
                    &path,
                    "special filesystem entries are forbidden",
                ));
            }
        }
    }
    Ok(())
}

fn insert_snapshot_entry(
    snapshot: &mut ArtifactTreeSnapshot,
    path: PathBuf,
    kind: ArtifactEntryKind,
    metadata: &fs::Metadata,
) -> io::Result<()> {
    if snapshot.entries.len() >= MAX_ARTIFACT_TREE_ENTRIES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("artifact tree exceeds {MAX_ARTIFACT_TREE_ENTRIES} entries"),
        ));
    }
    let bytes = if kind == ArtifactEntryKind::File {
        metadata.len()
    } else {
        0
    };
    snapshot.bytes = snapshot.bytes.checked_add(bytes).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "artifact byte count overflow")
    })?;
    let identity = artifact_identity_at(&path, metadata)?;
    snapshot.entries.insert(
        path,
        ArtifactEntry {
            kind,
            bytes,
            modified_at: metadata.modified().ok(),
            identity,
        },
    );
    Ok(())
}

fn sorted_children(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    paths.sort();
    Ok(paths)
}

#[cfg(unix)]
fn artifact_identity_at(_path: &Path, metadata: &fs::Metadata) -> io::Result<ArtifactIdentity> {
    Ok(ArtifactIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(windows)]
fn artifact_identity_at(path: &Path, expected: &fs::Metadata) -> io::Result<ArtifactIdentity> {
    let file = windows_open_artifact_handle(path, false)?;
    let opened = file.metadata()?;
    let changed = expected.is_dir() != opened.is_dir()
        || expected.is_file() != opened.is_file()
        || metadata_is_link_or_reparse(expected) != metadata_is_link_or_reparse(&opened)
        || (expected.is_file() && expected.len() != opened.len())
        || expected.modified().ok() != opened.modified().ok();
    if changed {
        return Err(tree_changed_error(path));
    }
    opened_artifact_identity(&file, &opened)
}

#[cfg(not(any(unix, windows)))]
fn artifact_identity_at(_path: &Path, _metadata: &fs::Metadata) -> io::Result<ArtifactIdentity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "artifact cleanup requires stable filesystem identities",
    ))
}

#[cfg(unix)]
fn opened_artifact_identity(_file: &File, metadata: &fs::Metadata) -> io::Result<ArtifactIdentity> {
    artifact_identity_at(Path::new("<opened artifact>"), metadata)
}

#[cfg(windows)]
fn opened_artifact_identity(file: &File, _metadata: &fs::Metadata) -> io::Result<ArtifactIdentity> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let queried = unsafe {
        GetFileInformationByHandle(
            file.as_raw_handle() as *mut std::ffi::c_void,
            information.as_mut_ptr(),
        )
    };
    if queried == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: GetFileInformationByHandle succeeded and initialized the value.
    let information = unsafe { information.assume_init() };
    Ok(ArtifactIdentity {
        volume: u64::from(information.dwVolumeSerialNumber),
        file: (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    })
}

#[cfg(not(any(unix, windows)))]
fn opened_artifact_identity(
    _file: &File,
    _metadata: &fs::Metadata,
) -> io::Result<ArtifactIdentity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "artifact cleanup requires stable filesystem identities",
    ))
}

fn validate_opened_entry(file: &File, path: &Path, expected: &ArtifactEntry) -> io::Result<()> {
    let metadata = file.metadata()?;
    if metadata_is_link_or_reparse(&metadata) {
        return Err(unsafe_artifact_path(path, "delete target became a link"));
    }
    let kind = if metadata.is_dir() {
        ArtifactEntryKind::Directory
    } else if metadata.is_file() {
        ArtifactEntryKind::File
    } else {
        return Err(unsafe_artifact_path(
            path,
            "delete target became a special entry",
        ));
    };
    if kind != expected.kind
        || opened_artifact_identity(file, &metadata)? != expected.identity
        || (kind == ArtifactEntryKind::File && metadata.len() != expected.bytes)
        || (kind == ArtifactEntryKind::File && metadata.modified().ok() != expected.modified_at)
    {
        return Err(tree_changed_error(path));
    }
    Ok(())
}

fn expected_children(path: &Path, expected: &BTreeMap<PathBuf, ArtifactEntry>) -> Vec<PathBuf> {
    expected
        .keys()
        .filter(|candidate| candidate.parent() == Some(path))
        .cloned()
        .collect()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl ArtifactRootAnchor {
    fn open(
        workspace_root: &Path,
        artifacts_root: &Path,
        expected_identity: ArtifactIdentity,
    ) -> io::Result<Self> {
        if artifacts_root.parent() != Some(workspace_root) {
            return Err(unsafe_artifact_path(
                artifacts_root,
                "artifact root is not a direct workspace child",
            ));
        }
        let canonical_workspace = workspace_root.canonicalize()?;
        let workspace = OpenOptions::new()
            .read(true)
            .custom_flags(unix_directory_open_flags())
            .open(canonical_workspace)?;
        let root_name = artifacts_root.file_name().ok_or_else(|| {
            unsafe_artifact_path(artifacts_root, "artifact root has no file name")
        })?;
        let directory = unix_open_child(&workspace, root_name, true)?;
        let metadata = directory.metadata()?;
        if !metadata.is_dir()
            || metadata_is_link_or_reparse(&metadata)
            || opened_artifact_identity(&directory, &metadata)? != expected_identity
        {
            return Err(tree_changed_error(artifacts_root));
        }
        Ok(Self {
            logical_root: artifacts_root.to_path_buf(),
            directory,
            identity: expected_identity,
        })
    }

    fn remove_tree(
        &self,
        path: &Path,
        expected: &BTreeMap<PathBuf, ArtifactEntry>,
        trace: &mut CleanupTrace,
    ) -> Result<(), DeleteFailure> {
        let (parent, name) = self.open_parent(path, expected)?;
        unix_remove_tree_from_parent(&parent, &name, path, expected, trace)
    }

    fn remove_file(
        &self,
        path: &Path,
        expected: &BTreeMap<PathBuf, ArtifactEntry>,
        trace: &mut CleanupTrace,
    ) -> Result<(), DeleteFailure> {
        let (parent, name) = self.open_parent(path, expected)?;
        unix_remove_file_from_parent(&parent, &name, path, expected, trace)
    }

    fn remove_reparse_leaf(
        &self,
        day_dir: &Path,
        path: &Path,
        _expected: &fs::Metadata,
    ) -> io::Result<()> {
        if day_dir.parent() != Some(self.logical_root.as_path()) || path.parent() != Some(day_dir) {
            return Err(unsafe_artifact_path(
                path,
                "staging link is not a direct artifact-day child",
            ));
        }
        let day_metadata = fs::symlink_metadata(day_dir)?;
        if !day_metadata.is_dir() || metadata_is_link_or_reparse(&day_metadata) {
            return Err(tree_changed_error(day_dir));
        }
        let day_name = day_dir
            .file_name()
            .ok_or_else(|| unsafe_artifact_path(day_dir, "artifact day has no name"))?;
        let day = unix_open_child(&self.directory, day_name, true)?;
        let opened_metadata = day.metadata()?;
        if opened_artifact_identity(&day, &opened_metadata)?
            != artifact_identity_at(day_dir, &day_metadata)?
        {
            return Err(tree_changed_error(day_dir));
        }
        let name = path
            .file_name()
            .ok_or_else(|| unsafe_artifact_path(path, "staging link has no name"))?;
        unix_unlink_child(&day, name, false)
    }

    fn open_parent(
        &self,
        path: &Path,
        expected: &BTreeMap<PathBuf, ArtifactEntry>,
    ) -> Result<(File, std::ffi::OsString), DeleteFailure> {
        let relative = path.strip_prefix(&self.logical_root).map_err(|_| {
            DeleteFailure::new(
                path,
                unsafe_artifact_path(path, "delete target escapes artifact root"),
            )
        })?;
        let name = relative.file_name().ok_or_else(|| {
            DeleteFailure::new(
                path,
                unsafe_artifact_path(path, "delete target has no name"),
            )
        })?;
        let mut parent = self
            .directory
            .try_clone()
            .map_err(|source| DeleteFailure::new(path, source))?;
        let mut logical_parent = self.logical_root.clone();
        if let Some(relative_parent) = relative.parent() {
            for component in relative_parent.components() {
                let std::path::Component::Normal(component) = component else {
                    return Err(DeleteFailure::new(
                        path,
                        unsafe_artifact_path(path, "delete target has unsafe components"),
                    ));
                };
                logical_parent.push(component);
                let entry = expected.get(&logical_parent).ok_or_else(|| {
                    DeleteFailure::new(&logical_parent, tree_changed_error(&logical_parent))
                })?;
                if entry.kind != ArtifactEntryKind::Directory {
                    return Err(DeleteFailure::new(
                        &logical_parent,
                        tree_changed_error(&logical_parent),
                    ));
                }
                let child = unix_open_child(&parent, component, true)
                    .map_err(|source| DeleteFailure::new(&logical_parent, source))?;
                validate_opened_entry(&child, &logical_parent, entry)
                    .map_err(|source| DeleteFailure::new(&logical_parent, source))?;
                parent = child;
            }
        }
        Ok((parent, name.to_os_string()))
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_remove_tree_from_parent(
    parent: &File,
    name: &std::ffi::OsStr,
    path: &Path,
    expected: &BTreeMap<PathBuf, ArtifactEntry>,
    trace: &mut CleanupTrace,
) -> Result<(), DeleteFailure> {
    let entry = expected
        .get(path)
        .ok_or_else(|| DeleteFailure::new(path, tree_changed_error(path)))?;
    if entry.kind != ArtifactEntryKind::Directory {
        return Err(DeleteFailure::new(path, tree_changed_error(path)));
    }
    let directory =
        unix_open_child(parent, name, true).map_err(|source| DeleteFailure::new(path, source))?;
    validate_opened_entry(&directory, path, entry)
        .map_err(|source| DeleteFailure::new(path, source))?;

    for child in expected_children(path, expected) {
        let child_name = child.file_name().ok_or_else(|| {
            DeleteFailure::new(&child, unsafe_artifact_path(&child, "child has no name"))
        })?;
        let child_entry = expected
            .get(&child)
            .ok_or_else(|| DeleteFailure::new(&child, tree_changed_error(&child)))?;
        if child_entry.kind == ArtifactEntryKind::Directory {
            unix_remove_tree_from_parent(&directory, child_name, &child, expected, trace)?;
        } else {
            unix_remove_file_from_parent(&directory, child_name, &child, expected, trace)?;
        }
    }

    let current =
        unix_open_child(parent, name, true).map_err(|source| DeleteFailure::new(path, source))?;
    validate_opened_entry(&current, path, entry)
        .map_err(|source| DeleteFailure::new(path, source))?;
    unix_unlink_child(parent, name, true).map_err(|source| DeleteFailure::new(path, source))?;
    trace.deleted_paths.push(path.to_path_buf());
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_remove_file_from_parent(
    parent: &File,
    name: &std::ffi::OsStr,
    path: &Path,
    expected: &BTreeMap<PathBuf, ArtifactEntry>,
    trace: &mut CleanupTrace,
) -> Result<(), DeleteFailure> {
    let entry = expected
        .get(path)
        .ok_or_else(|| DeleteFailure::new(path, tree_changed_error(path)))?;
    if entry.kind != ArtifactEntryKind::File {
        return Err(DeleteFailure::new(path, tree_changed_error(path)));
    }
    let current =
        unix_open_child(parent, name, false).map_err(|source| DeleteFailure::new(path, source))?;
    validate_opened_entry(&current, path, entry)
        .map_err(|source| DeleteFailure::new(path, source))?;
    unix_unlink_child(parent, name, false).map_err(|source| DeleteFailure::new(path, source))?;
    trace.deleted_paths.push(path.to_path_buf());
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_open_child(parent: &File, name: &std::ffi::OsStr, directory: bool) -> io::Result<File> {
    let name = CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "artifact name contains NUL"))?;
    let mut flags = unix_child_open_flags();
    if directory {
        flags |= unix_o_directory();
    }
    let descriptor = system_openat(parent.as_raw_fd(), name.as_ptr(), flags);
    if descriptor < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: openat returned a new owned descriptor on success.
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_unlink_child(parent: &File, name: &std::ffi::OsStr, directory: bool) -> io::Result<()> {
    let name = CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "artifact name contains NUL"))?;
    let flags = if directory { unix_at_removedir() } else { 0 };
    if system_unlinkat(parent.as_raw_fd(), name.as_ptr(), flags) == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn unix_child_open_flags() -> i32 {
    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW
}

#[cfg(target_os = "macos")]
fn unix_directory_open_flags() -> i32 {
    libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW
}

#[cfg(target_os = "macos")]
fn unix_o_directory() -> i32 {
    libc::O_DIRECTORY
}

#[cfg(target_os = "macos")]
fn unix_at_removedir() -> i32 {
    libc::AT_REMOVEDIR
}

#[cfg(target_os = "macos")]
fn system_openat(parent: i32, name: *const i8, flags: i32) -> i32 {
    // SAFETY: name is a live NUL-terminated C string and parent is an open fd.
    unsafe { libc::openat(parent, name, flags) }
}

#[cfg(target_os = "macos")]
fn system_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32 {
    // SAFETY: name is a live NUL-terminated C string and parent is an open fd.
    unsafe { libc::unlinkat(parent, name, flags) }
}

#[cfg(target_os = "linux")]
fn unix_child_open_flags() -> i32 {
    0o2_000_000 | 0o400_000
}

#[cfg(target_os = "linux")]
fn unix_directory_open_flags() -> i32 {
    unix_child_open_flags() | unix_o_directory()
}

#[cfg(target_os = "linux")]
fn unix_o_directory() -> i32 {
    0o200_000
}

#[cfg(target_os = "linux")]
fn unix_at_removedir() -> i32 {
    0x200
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    #[link_name = "openat"]
    fn linux_openat(parent: i32, name: *const i8, flags: i32, ...) -> i32;
    #[link_name = "unlinkat"]
    fn linux_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32;
}

#[cfg(target_os = "linux")]
fn system_openat(parent: i32, name: *const i8, flags: i32) -> i32 {
    // SAFETY: name is a live NUL-terminated C string and parent is an open fd.
    unsafe { linux_openat(parent, name, flags) }
}

#[cfg(target_os = "linux")]
fn system_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32 {
    // SAFETY: name is a live NUL-terminated C string and parent is an open fd.
    unsafe { linux_unlinkat(parent, name, flags) }
}

#[cfg(windows)]
impl ArtifactRootAnchor {
    fn open(
        workspace_root: &Path,
        artifacts_root: &Path,
        expected_identity: ArtifactIdentity,
    ) -> io::Result<Self> {
        if artifacts_root.parent() != Some(workspace_root) {
            return Err(unsafe_artifact_path(
                artifacts_root,
                "artifact root is not a direct workspace child",
            ));
        }
        let directory = windows_open_artifact_handle(artifacts_root, false)?;
        let metadata = directory.metadata()?;
        if !metadata.is_dir()
            || metadata_is_link_or_reparse(&metadata)
            || opened_artifact_identity(&directory, &metadata)? != expected_identity
        {
            return Err(tree_changed_error(artifacts_root));
        }
        Ok(Self {
            logical_root: artifacts_root.to_path_buf(),
            directory,
            identity: expected_identity,
        })
    }

    fn remove_tree(
        &self,
        path: &Path,
        expected: &BTreeMap<PathBuf, ArtifactEntry>,
        trace: &mut CleanupTrace,
    ) -> Result<(), DeleteFailure> {
        let _ancestors = self.open_ancestor_handles(path, expected)?;
        windows_remove_tree(path, expected, trace)
    }

    fn remove_file(
        &self,
        path: &Path,
        expected: &BTreeMap<PathBuf, ArtifactEntry>,
        trace: &mut CleanupTrace,
    ) -> Result<(), DeleteFailure> {
        let _ancestors = self.open_ancestor_handles(path, expected)?;
        windows_remove_file(path, expected, trace)
    }

    fn remove_reparse_leaf(
        &self,
        day_dir: &Path,
        path: &Path,
        expected: &fs::Metadata,
    ) -> io::Result<()> {
        if day_dir.parent() != Some(self.logical_root.as_path()) || path.parent() != Some(day_dir) {
            return Err(unsafe_artifact_path(
                path,
                "staging link is not a direct artifact-day child",
            ));
        }
        let day_metadata = fs::symlink_metadata(day_dir)?;
        let expected_day_identity = artifact_identity_at(day_dir, &day_metadata)?;
        let day = windows_open_artifact_handle(day_dir, false)?;
        let opened_day_metadata = day.metadata()?;
        if !opened_day_metadata.is_dir()
            || opened_artifact_identity(&day, &opened_day_metadata)? != expected_day_identity
        {
            return Err(tree_changed_error(day_dir));
        }
        let expected_link_identity = artifact_identity_at(path, expected)?;
        let link = windows_open_artifact_handle(path, true)?;
        let metadata = link.metadata()?;
        if !metadata_is_link_or_reparse(&metadata)
            || opened_artifact_identity(&link, &metadata)? != expected_link_identity
        {
            return Err(tree_changed_error(path));
        }
        windows_mark_delete(&link)
    }

    fn open_ancestor_handles(
        &self,
        path: &Path,
        expected: &BTreeMap<PathBuf, ArtifactEntry>,
    ) -> Result<Vec<File>, DeleteFailure> {
        let root_metadata = self
            .directory
            .metadata()
            .map_err(|source| DeleteFailure::new(&self.logical_root, source))?;
        if metadata_is_link_or_reparse(&root_metadata)
            || !root_metadata.is_dir()
            || opened_artifact_identity(&self.directory, &root_metadata)
                .map_err(|source| DeleteFailure::new(&self.logical_root, source))?
                != self.identity
        {
            return Err(DeleteFailure::new(
                &self.logical_root,
                tree_changed_error(&self.logical_root),
            ));
        }

        let relative = path.strip_prefix(&self.logical_root).map_err(|_| {
            DeleteFailure::new(
                path,
                unsafe_artifact_path(path, "delete target escapes artifact root"),
            )
        })?;
        let mut handles = Vec::new();
        let mut logical_parent = self.logical_root.clone();
        if let Some(relative_parent) = relative.parent() {
            for component in relative_parent.components() {
                let std::path::Component::Normal(component) = component else {
                    return Err(DeleteFailure::new(
                        path,
                        unsafe_artifact_path(path, "delete target has unsafe components"),
                    ));
                };
                logical_parent.push(component);
                let entry = expected.get(&logical_parent).ok_or_else(|| {
                    DeleteFailure::new(&logical_parent, tree_changed_error(&logical_parent))
                })?;
                if entry.kind != ArtifactEntryKind::Directory {
                    return Err(DeleteFailure::new(
                        &logical_parent,
                        tree_changed_error(&logical_parent),
                    ));
                }
                let handle = windows_open_artifact_handle(&logical_parent, false)
                    .map_err(|source| DeleteFailure::new(&logical_parent, source))?;
                validate_opened_entry(&handle, &logical_parent, entry)
                    .map_err(|source| DeleteFailure::new(&logical_parent, source))?;
                handles.push(handle);
            }
        }
        Ok(handles)
    }
}

#[cfg(windows)]
fn windows_remove_tree(
    path: &Path,
    expected: &BTreeMap<PathBuf, ArtifactEntry>,
    trace: &mut CleanupTrace,
) -> Result<(), DeleteFailure> {
    let entry = expected
        .get(path)
        .ok_or_else(|| DeleteFailure::new(path, tree_changed_error(path)))?;
    if entry.kind != ArtifactEntryKind::Directory {
        return Err(DeleteFailure::new(path, tree_changed_error(path)));
    }
    let directory = windows_open_artifact_handle(path, true)
        .map_err(|source| DeleteFailure::new(path, source))?;
    validate_opened_entry(&directory, path, entry)
        .map_err(|source| DeleteFailure::new(path, source))?;
    for child in expected_children(path, expected) {
        let child_entry = expected
            .get(&child)
            .ok_or_else(|| DeleteFailure::new(&child, tree_changed_error(&child)))?;
        if child_entry.kind == ArtifactEntryKind::Directory {
            windows_remove_tree(&child, expected, trace)?;
        } else {
            windows_remove_file(&child, expected, trace)?;
        }
    }
    windows_mark_delete(&directory).map_err(|source| DeleteFailure::new(path, source))?;
    drop(directory);
    trace.deleted_paths.push(path.to_path_buf());
    Ok(())
}

#[cfg(windows)]
fn windows_remove_file(
    path: &Path,
    expected: &BTreeMap<PathBuf, ArtifactEntry>,
    trace: &mut CleanupTrace,
) -> Result<(), DeleteFailure> {
    let entry = expected
        .get(path)
        .ok_or_else(|| DeleteFailure::new(path, tree_changed_error(path)))?;
    if entry.kind != ArtifactEntryKind::File {
        return Err(DeleteFailure::new(path, tree_changed_error(path)));
    }
    let file = windows_open_artifact_handle(path, true)
        .map_err(|source| DeleteFailure::new(path, source))?;
    validate_opened_entry(&file, path, entry).map_err(|source| DeleteFailure::new(path, source))?;
    windows_mark_delete(&file).map_err(|source| DeleteFailure::new(path, source))?;
    drop(file);
    trace.deleted_paths.push(path.to_path_buf());
    Ok(())
}

#[cfg(windows)]
fn windows_open_artifact_handle(path: &Path, delete_access: bool) -> io::Result<File> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let mut wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "artifact path contains NUL",
        ));
    }
    wide.push(0);
    let access = FILE_READ_ATTRIBUTES | if delete_access { DELETE } else { 0 };
    // SAFETY: wide is NUL-terminated and all pointer arguments remain valid.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: CreateFileW returned a new owned handle on success.
        Ok(unsafe { File::from_raw_handle(handle as _) })
    }
}

#[cfg(windows)]
fn windows_mark_delete(file: &File) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };

    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: file owns a valid handle and disposition points to a live value
    // with the exact structure and byte length required by this info class.
    let deleted = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as _,
            FileDispositionInfo,
            std::ptr::from_ref(&disposition).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if deleted == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn tree_changed_error(path: &Path) -> io::Error {
    io::Error::other(format!(
        "artifact tree changed during cleanup: {}",
        path.display()
    ))
}

fn cleanup_report(
    artifacts_root: &Path,
    today_dir: &Path,
    baseline: &ArtifactTreeSnapshot,
    final_snapshot: &ArtifactTreeSnapshot,
    trace: &CleanupTrace,
    complete: bool,
) -> CleanupReport {
    CleanupReport {
        artifact_root: artifacts_root.display().to_string(),
        today_dir: today_dir.display().to_string(),
        bytes_before: baseline.bytes,
        bytes_after: final_snapshot.bytes,
        deleted_dirs: display_paths(&trace.deleted_dirs),
        deleted_files: display_paths(&trace.deleted_files),
        kept_dirs: final_snapshot
            .day_dirs
            .values()
            .map(|path| path.display().to_string())
            .collect(),
        deleted_paths: display_paths(&trace.deleted_paths),
        complete,
    }
}

fn display_paths(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

fn sql_artifact_day(connection: &Connection, sql: &str) -> Result<ArtifactDay, ArtifactError> {
    let value: String = connection.query_row(sql, [], |row| row.get(0))?;
    ArtifactDay::parse(&value)
}

fn parse_number<T>(value: &str) -> Result<T, ArtifactError>
where
    T: std::str::FromStr,
{
    value
        .parse::<T>()
        .map_err(|_| ArtifactError::InvalidDay(value.to_string()))
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        env::temp_dir().join(format!("aopmem-stage-036-artifacts-{name}-{nanos}"))
    }

    fn write_file_with_size(path: &Path, size: usize) {
        let bytes = vec![b'x'; size];
        fs::write(path, bytes).expect("test file should be written");
    }

    #[test]
    fn ensure_daily_artifact_dir_uses_yyyy_mm_dd_folder() {
        let root = temp_path("daily-dir");
        let artifacts_root = root.join("artifacts");
        fs::create_dir_all(&artifacts_root).expect("artifacts root should exist");

        let day = ArtifactDay::parse("2026-06-08").expect("day should parse");
        let path = ensure_daily_artifact_dir_in_root(&artifacts_root, &day)
            .expect("daily dir should be created");

        assert_eq!(path, artifacts_root.join("2026-06-08"));
        assert!(path.is_dir());

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn cleanup_removes_dirs_older_than_retention_window() {
        let root = temp_path("cleanup-age");
        let artifacts_root = root.join("artifacts");
        fs::create_dir_all(&artifacts_root).expect("artifacts root should exist");
        let old_dir = artifacts_root.join("2026-05-31");
        let kept_dir = artifacts_root.join("2026-06-02");
        fs::create_dir_all(&old_dir).expect("old dir should exist");
        fs::create_dir_all(&kept_dir).expect("kept dir should exist");
        write_file_with_size(&old_dir.join("old.txt"), 8);
        write_file_with_size(&kept_dir.join("keep.txt"), 8);

        let report = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1_000,
        )
        .expect("cleanup should succeed");

        assert!(!old_dir.exists());
        assert!(kept_dir.exists());
        assert!(artifacts_root.join("2026-06-08").is_dir());
        assert_eq!(report.deleted_dirs, vec![old_dir.display().to_string()]);

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn cleanup_removes_oldest_dirs_until_size_limit_is_met() {
        let root = temp_path("cleanup-size");
        let artifacts_root = root.join("artifacts");
        fs::create_dir_all(&artifacts_root).expect("artifacts root should exist");

        for day in ["2026-06-06", "2026-06-07", "2026-06-08"] {
            let dir = artifacts_root.join(day);
            fs::create_dir_all(&dir).expect("artifact dir should exist");
            write_file_with_size(&dir.join("payload.bin"), 5);
        }

        let report = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            10,
        )
        .expect("cleanup should succeed");

        assert!(!artifacts_root.join("2026-06-06").exists());
        assert!(artifacts_root.join("2026-06-07").exists());
        assert!(artifacts_root.join("2026-06-08").exists());
        assert_eq!(report.bytes_before, 15);
        assert_eq!(report.bytes_after, 10);
        assert_eq!(
            report.deleted_dirs,
            vec![artifacts_root.join("2026-06-06").display().to_string()]
        );

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn cleanup_prunes_oldest_regular_file_when_only_today_exceeds_size_limit() {
        let root = temp_path("cleanup-today-only-size");
        let artifacts_root = root.join("artifacts");
        fs::create_dir_all(&artifacts_root).expect("artifacts root should exist");
        let today_dir = artifacts_root.join("2026-06-08");
        fs::create_dir_all(&today_dir).expect("today dir should exist");
        write_file_with_size(&today_dir.join("payload.bin"), 5);

        let report = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1,
        )
        .expect("cleanup should succeed");

        assert!(today_dir.exists());
        assert_eq!(report.deleted_dirs, Vec::<String>::new());
        assert_eq!(report.bytes_before, 5);
        assert_eq!(report.bytes_after, 0);
        assert_eq!(
            report.deleted_files,
            vec![today_dir.join("payload.bin").display().to_string()]
        );
        assert_eq!(report.kept_dirs, vec![today_dir.display().to_string()]);
        assert!(!today_dir.join("payload.bin").exists());

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn cleanup_prunes_oldest_files_first_inside_today_dir() {
        let root = temp_path("cleanup-today-file-order");
        let artifacts_root = root.join("artifacts");
        let today_dir = artifacts_root.join("2026-06-08");
        fs::create_dir_all(today_dir.join("nested")).expect("today dir should exist");
        write_file_with_size(&today_dir.join("first.bin"), 4);
        write_file_with_size(&today_dir.join("nested").join("second.bin"), 4);

        let report = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            4,
        )
        .expect("cleanup should succeed");

        assert_eq!(report.bytes_after, 4);
        assert_eq!(
            report.deleted_files,
            vec![today_dir.join("first.bin").display().to_string()]
        );
        assert!(!today_dir.join("first.bin").exists());
        assert!(today_dir.join("nested").join("second.bin").is_file());

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn cleanup_never_touches_workspace_sibling_dirs() {
        let root = temp_path("cleanup-safety");
        let artifacts_root = root.join("artifacts");
        let db_path = root.join("aopmem.sqlite");
        fs::create_dir_all(&artifacts_root).expect("artifacts root should exist");
        fs::write(&db_path, b"db").expect("db file should exist");
        let protected = [
            "tools",
            "logs",
            "audit-git",
            "observability",
            "exports",
            "templates",
            "skills",
            "runtimes",
        ]
        .map(|name| root.join(name));
        for directory in &protected {
            fs::create_dir_all(directory).expect("protected sibling should exist");
            fs::write(directory.join("sentinel"), b"keep")
                .expect("protected sentinel should exist");
        }
        let old_dir = artifacts_root.join("2026-05-31");
        fs::create_dir_all(&old_dir).expect("old dir should exist");
        write_file_with_size(&old_dir.join("old.txt"), 4);

        cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1_000,
        )
        .expect("cleanup should succeed");

        assert!(db_path.is_file());
        for directory in &protected {
            assert_eq!(
                fs::read(directory.join("sentinel")).expect("protected sentinel should remain"),
                b"keep"
            );
        }

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn artifact_lock_is_permanent_and_cleanup_wait_is_bounded() {
        let root = temp_path("lock-bounded");
        let artifacts_root = root.join("artifacts");
        fs::create_dir_all(&root).expect("workspace root should exist");

        let shared = ArtifactTreeLock::acquire_with_timeout(
            &root,
            &artifacts_root,
            ArtifactLockMode::CaptureShared,
            Duration::from_millis(50),
        )
        .expect("shared capture lock should be acquired");
        let second_shared = ArtifactTreeLock::acquire_with_timeout(
            &root,
            &artifacts_root,
            ArtifactLockMode::CaptureShared,
            Duration::from_millis(50),
        )
        .expect("parallel shared capture lock should be acquired");
        let error = ArtifactTreeLock::acquire_with_timeout(
            &root,
            &artifacts_root,
            ArtifactLockMode::CleanupExclusive,
            Duration::from_millis(50),
        )
        .expect_err("exclusive cleanup lock must time out during capture");
        assert!(matches!(
            error,
            ArtifactError::LockTimeout {
                mode: ArtifactLockMode::CleanupExclusive,
                ..
            }
        ));

        drop(second_shared);
        drop(shared);
        drop(
            ArtifactTreeLock::acquire_with_timeout(
                &root,
                &artifacts_root,
                ArtifactLockMode::CleanupExclusive,
                Duration::from_millis(50),
            )
            .expect("exclusive cleanup lock should succeed after captures finish"),
        );
        assert!(artifacts_root.join(ARTIFACT_LOCK_FILE_NAME).is_file());

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn cleanup_removes_strict_crash_staging_before_retained_days() {
        let root = temp_path("cleanup-stale-staging");
        let artifacts_root = root.join("artifacts");
        let retained = artifacts_root.join("2026-06-07");
        let stale = retained.join(".tool-run-0123456789abcdef0123456789abcdef.tmp");
        fs::create_dir_all(&stale).expect("strict stale staging should exist");
        write_file_with_size(&stale.join("stdout.bin"), 8);
        write_file_with_size(&retained.join("published.bin"), 4);

        let report = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1_000,
        )
        .expect("strict stale staging cleanup should succeed");

        assert!(!stale.exists());
        assert!(retained.join("published.bin").is_file());
        assert_eq!(report.deleted_dirs, vec![stale.display().to_string()]);
        assert!(report
            .deleted_paths
            .contains(&stale.join("stdout.bin").display().to_string()));
        assert!(report.deleted_paths.contains(&stale.display().to_string()));
        assert!(report.complete);
        assert!(artifacts_root.join(ARTIFACT_LOCK_FILE_NAME).is_file());

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn malformed_staging_fails_before_any_deletion_or_today_creation() {
        let root = temp_path("cleanup-malformed-staging");
        let artifacts_root = root.join("artifacts");
        let old_dir = artifacts_root.join("2026-05-31");
        let malformed = artifacts_root
            .join("2026-06-07")
            .join(".tool-run-ABCDEF0123456789ABCDEF0123456789.tmp");
        fs::create_dir_all(&old_dir).expect("expired directory should exist");
        fs::create_dir_all(&malformed).expect("malformed staging should exist");
        write_file_with_size(&old_dir.join("must-remain"), 4);

        let error = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1_000,
        )
        .expect_err("malformed staging must fail closed");

        assert!(matches!(error, ArtifactError::Io(_)));
        assert!(old_dir.join("must-remain").is_file());
        assert!(!artifacts_root.join("2026-06-08").exists());

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn cleanup_includes_future_files_and_does_nothing_at_equal_limit() {
        let root = temp_path("cleanup-future-files");
        let artifacts_root = root.join("artifacts");
        let future = artifacts_root.join("2026-06-09");
        fs::create_dir_all(&future).expect("future day should exist");
        write_file_with_size(&future.join("future.bin"), 5);

        let equal = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            5,
        )
        .expect("equal-size cleanup should succeed without deletion");
        assert_eq!(equal.bytes_after, 5);
        assert!(equal.deleted_paths.is_empty());

        let over = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1,
        )
        .expect("future file may be deleted to satisfy the cap");
        assert_eq!(over.bytes_after, 0);
        assert_eq!(
            over.deleted_files,
            vec![future.join("future.bin").display().to_string()]
        );
        assert!(!future.join("future.bin").exists());

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_rejects_link_before_deleting_expired_sibling() {
        use std::os::unix::fs::symlink;

        let root = temp_path("cleanup-link-preflight");
        let artifacts_root = root.join("artifacts");
        let old_dir = artifacts_root.join("2026-05-31");
        let linked_day = artifacts_root.join("2026-06-07");
        let outside = root.join("outside");
        fs::create_dir_all(&old_dir).expect("expired directory should exist");
        fs::create_dir_all(&outside).expect("outside directory should exist");
        write_file_with_size(&old_dir.join("must-remain"), 4);
        fs::write(outside.join("sentinel"), b"safe").expect("outside sentinel should exist");
        symlink(&outside, &linked_day).expect("linked day should be created");

        let error = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1_000,
        )
        .expect_err("linked day must fail closed");

        assert!(matches!(error, ArtifactError::Io(_)));
        assert!(old_dir.join("must-remain").is_file());
        assert_eq!(
            fs::read(outside.join("sentinel")).expect("outside sentinel should remain"),
            b"safe"
        );

        fs::remove_file(linked_day).expect("linked day should be removed by test cleanup");
        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_rejects_special_entry_before_deletion() {
        use std::os::unix::net::UnixListener;

        let root = PathBuf::from("/tmp").join(format!("ao-sock-{}", Uuid::new_v4().simple()));
        let artifacts_root = root.join("artifacts");
        let old_dir = artifacts_root.join("2026-05-31");
        let retained = artifacts_root.join("2026-06-07");
        fs::create_dir_all(&old_dir).expect("expired directory should exist");
        fs::create_dir_all(&retained).expect("retained directory should exist");
        write_file_with_size(&old_dir.join("must-remain"), 4);
        let socket_path = retained.join("forbidden.sock");
        let listener = UnixListener::bind(&socket_path).expect("socket fixture should bind");

        let error = cleanup_artifact_root_for_day(
            &root,
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1_000,
        )
        .expect_err("special entry must fail closed");

        assert!(matches!(error, ArtifactError::Io(_)));
        assert!(old_dir.join("must-remain").is_file());
        drop(listener);
        fs::remove_file(socket_path).expect("socket fixture should be removed");
        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn unpublished_staging_symlink_is_removed_without_touching_target() {
        use std::os::unix::fs::symlink;

        let root = temp_path("staging-symlink");
        let day_dir = root.join("artifacts/2026-06-08");
        let outside = root.join("outside");
        let staging = day_dir.join(".tool-run-0123456789abcdef0123456789abcdef.tmp");
        fs::create_dir_all(&day_dir).expect("artifact day should exist");
        fs::create_dir_all(&outside).expect("outside target should exist");
        let sentinel = outside.join("sentinel");
        fs::write(&sentinel, b"safe").expect("outside sentinel should be written");
        symlink(&outside, &staging).expect("staging symlink should be created");

        assert!(validate_secure_direct_directory(&day_dir, &staging).is_err());
        remove_unpublished_staging(&day_dir, &staging);

        assert!(!staging.exists());
        assert_eq!(
            fs::read(&sentinel).expect("sentinel should remain"),
            b"safe"
        );
        fs::remove_dir_all(root).expect("temp root should be removed");
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn anchored_file_delete_cannot_follow_swapped_artifact_root() {
        use std::os::unix::fs::symlink;

        let root = temp_path("anchored-file-root-swap");
        let artifacts_root = root.join("artifacts");
        let displaced_root = root.join("displaced-artifacts");
        let day = artifacts_root.join("2026-06-08");
        let target = day.join("old.bin");
        let outside = root.join("outside");
        let sentinel = outside.join("old.bin");
        fs::create_dir_all(&day).expect("artifact day should exist");
        fs::create_dir_all(&outside).expect("outside directory should exist");
        fs::write(&target, b"artifact").expect("artifact target should exist");
        fs::write(&sentinel, b"outside-safe").expect("outside sentinel should exist");

        let snapshot = snapshot_artifact_tree(&root, &artifacts_root)
            .expect("artifact snapshot should succeed");
        let anchor = ArtifactRootAnchor::open(&root, &artifacts_root, snapshot.root_identity)
            .expect("artifact root should be anchored");
        fs::rename(&artifacts_root, &displaced_root).expect("artifact root should move");
        symlink(&outside, &artifacts_root).expect("artifact root replacement should exist");

        let mut trace = CleanupTrace::default();
        anchor
            .remove_file(&target, &snapshot.entries, &mut trace)
            .expect("anchored deletion should use the original root object");

        assert!(!displaced_root.join("2026-06-08/old.bin").exists());
        assert_eq!(
            fs::read(&sentinel).expect("outside sentinel should remain"),
            b"outside-safe"
        );
        assert_eq!(trace.deleted_paths, vec![target]);
        fs::remove_file(&artifacts_root).expect("replacement symlink should remove");
        fs::remove_dir_all(root).expect("temp root should be removed");
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn anchored_directory_delete_cannot_follow_swapped_artifact_root() {
        use std::os::unix::fs::symlink;

        let root = temp_path("anchored-directory-root-swap");
        let artifacts_root = root.join("artifacts");
        let displaced_root = root.join("displaced-artifacts");
        let target = artifacts_root.join("2026-06-08/old-run");
        let outside = root.join("outside");
        let sentinel = outside.join("sentinel");
        fs::create_dir_all(&target).expect("artifact directory should exist");
        fs::create_dir_all(&outside).expect("outside directory should exist");
        fs::write(target.join("stdout.bin"), b"artifact").expect("artifact file should exist");
        fs::write(&sentinel, b"outside-safe").expect("outside sentinel should exist");

        let snapshot = snapshot_artifact_tree(&root, &artifacts_root)
            .expect("artifact snapshot should succeed");
        let anchor = ArtifactRootAnchor::open(&root, &artifacts_root, snapshot.root_identity)
            .expect("artifact root should be anchored");
        fs::rename(&artifacts_root, &displaced_root).expect("artifact root should move");
        symlink(&outside, &artifacts_root).expect("artifact root replacement should exist");

        let mut trace = CleanupTrace::default();
        anchor
            .remove_tree(&target, &snapshot.entries, &mut trace)
            .expect("anchored deletion should use the original root object");

        assert!(!displaced_root.join("2026-06-08/old-run").exists());
        assert_eq!(
            fs::read(&sentinel).expect("outside sentinel should remain"),
            b"outside-safe"
        );
        assert!(trace.deleted_paths.contains(&target));
        fs::remove_file(&artifacts_root).expect("replacement symlink should remove");
        fs::remove_dir_all(root).expect("temp root should be removed");
    }

    #[test]
    fn windows_cleanup_handles_never_share_delete_access() {
        let source = include_str!("mod.rs");
        let forbidden = ["FILE_SHARE_READ | FILE_SHARE_WRITE", " | FILE_SHARE_DELETE"].concat();

        assert!(source.contains("FILE_SHARE_READ | FILE_SHARE_WRITE,"));
        assert!(!source.contains(&forbidden));
    }

    #[test]
    fn atomic_publish_never_replaces_existing_final_directory() {
        let root = temp_path("publish-no-replace");
        let day_dir = root.join("artifacts/2026-06-08");
        let staging = day_dir.join(".tool-run-0123456789abcdef0123456789abcdef.tmp");
        let final_dir = day_dir.join("tool-run-0123456789abcdef0123456789abcdef");
        fs::create_dir_all(&staging).expect("staging directory should exist");
        fs::create_dir_all(&final_dir).expect("existing final directory should exist");
        fs::write(staging.join("new"), b"new").expect("staging fixture should be written");
        fs::write(final_dir.join("old"), b"old").expect("final fixture should be written");

        let error = atomic_publish_directory_no_replace(&staging, &final_dir)
            .expect_err("publish must not replace an existing final directory");

        assert!(matches!(
            error.kind(),
            io::ErrorKind::AlreadyExists | io::ErrorKind::Other
        ));
        assert_eq!(
            fs::read(final_dir.join("old")).expect("existing final data should remain"),
            b"old"
        );
        assert!(staging.join("new").is_file());
        fs::remove_dir_all(root).expect("temp root should be removed");
    }

    #[test]
    fn capture_files_are_create_new_only() {
        let root = temp_path("capture-create-new");
        fs::create_dir_all(&root).expect("capture fixture root should exist");
        let capture = root.join("stdout.bin");
        drop(open_new_capture_file(&capture).expect("first capture create should pass"));

        let error = open_new_capture_file(&capture)
            .expect_err("second capture create must not replace the existing file");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        fs::remove_dir_all(root).expect("temp root should be removed");
    }
}
