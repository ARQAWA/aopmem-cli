use rusqlite::Connection;
use serde::Serialize;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::storage::WorkspacePaths;

pub const ARTIFACT_RETENTION_DAYS: u64 = 7;
pub const ARTIFACT_MAX_BYTES: u64 = 1_000_000_000;

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
    pub kept_dirs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactDirUsage {
    day: ArtifactDay,
    path: PathBuf,
    bytes: u64,
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error("invalid artifact day format: {0}")]
    InvalidDay(String),
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
    ensure_daily_artifact_dir_in_root(workspace_paths.artifacts(), day)
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
    let today_dir = ensure_daily_artifact_dir(workspace_paths, today)?;
    let cleanup =
        cleanup_artifact_root_for_day(workspace_paths.artifacts(), today, oldest_kept, max_bytes)?;

    Ok(CleanupReport {
        artifact_root: workspace_paths.artifacts().display().to_string(),
        today_dir: today_dir.display().to_string(),
        bytes_before: cleanup.bytes_before,
        bytes_after: cleanup.bytes_after,
        deleted_dirs: cleanup.deleted_dirs,
        kept_dirs: cleanup.kept_dirs,
    })
}

fn ensure_daily_artifact_dir_in_root(
    artifacts_root: &Path,
    day: &ArtifactDay,
) -> Result<PathBuf, ArtifactError> {
    let path = artifacts_root.join(day.folder_name());
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn cleanup_artifact_root_for_day(
    artifacts_root: &Path,
    today: &ArtifactDay,
    oldest_kept: &ArtifactDay,
    max_bytes: u64,
) -> Result<CleanupRootReport, ArtifactError> {
    let _today_dir = ensure_daily_artifact_dir_in_root(artifacts_root, today)?;
    let mut usages = dated_artifact_dirs(artifacts_root)?;
    let bytes_before: u64 = usages.iter().map(|usage| usage.bytes).sum();
    let mut bytes_after = bytes_before;
    let mut deleted_dirs = Vec::new();

    let mut retained = Vec::with_capacity(usages.len());
    for usage in usages.drain(..) {
        if usage.day < *oldest_kept {
            fs::remove_dir_all(&usage.path)?;
            bytes_after = bytes_after.saturating_sub(usage.bytes);
            deleted_dirs.push(usage.path.display().to_string());
        } else {
            retained.push(usage);
        }
    }

    retained.sort_by(|left, right| left.day.cmp(&right.day));
    while bytes_after > max_bytes && !retained.is_empty() {
        if retained[0].day == *today {
            break;
        }
        let usage = retained.remove(0);
        fs::remove_dir_all(&usage.path)?;
        bytes_after = bytes_after.saturating_sub(usage.bytes);
        deleted_dirs.push(usage.path.display().to_string());
    }

    let kept_dirs = retained
        .into_iter()
        .map(|usage| usage.path.display().to_string())
        .collect();

    Ok(CleanupRootReport {
        bytes_before,
        bytes_after,
        deleted_dirs,
        kept_dirs,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CleanupRootReport {
    bytes_before: u64,
    bytes_after: u64,
    deleted_dirs: Vec<String>,
    kept_dirs: Vec<String>,
}

fn sql_artifact_day(connection: &Connection, sql: &str) -> Result<ArtifactDay, ArtifactError> {
    let value: String = connection.query_row(sql, [], |row| row.get(0))?;
    ArtifactDay::parse(&value)
}

fn dated_artifact_dirs(root: &Path) -> Result<Vec<ArtifactDirUsage>, ArtifactError> {
    let mut usages = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;

        if !metadata.is_dir() {
            continue;
        }

        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Ok(day) = ArtifactDay::parse(file_name) else {
            continue;
        };

        usages.push(ArtifactDirUsage {
            day,
            bytes: path_size(&path)?,
            path,
        });
    }

    usages.sort_by(|left, right| left.day.cmp(&right.day));
    Ok(usages)
}

fn path_size(path: &Path) -> io::Result<u64> {
    let metadata = fs::symlink_metadata(path)?;

    if metadata.is_file() || metadata.file_type().is_symlink() {
        return Ok(metadata.len());
    }

    if !metadata.is_dir() {
        return Ok(0);
    }

    let mut total = 0_u64;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        total = total.saturating_add(path_size(&entry.path())?);
    }

    Ok(total)
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
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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
    fn cleanup_keeps_today_dir_even_when_only_today_exceeds_size_limit() {
        let root = temp_path("cleanup-today-only-size");
        let artifacts_root = root.join("artifacts");
        fs::create_dir_all(&artifacts_root).expect("artifacts root should exist");
        let today_dir = artifacts_root.join("2026-06-08");
        fs::create_dir_all(&today_dir).expect("today dir should exist");
        write_file_with_size(&today_dir.join("payload.bin"), 5);

        let report = cleanup_artifact_root_for_day(
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1,
        )
        .expect("cleanup should succeed");

        assert!(today_dir.exists());
        assert_eq!(report.deleted_dirs, Vec::<String>::new());
        assert_eq!(report.bytes_before, 5);
        assert_eq!(report.bytes_after, 5);
        assert_eq!(report.kept_dirs, vec![today_dir.display().to_string()]);

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn cleanup_never_touches_workspace_sibling_dirs() {
        let root = temp_path("cleanup-safety");
        let artifacts_root = root.join("artifacts");
        let tools_dir = root.join("tools");
        let logs_dir = root.join("logs");
        let audit_dir = root.join("audit-git");
        let db_path = root.join("aopmem.sqlite");
        fs::create_dir_all(&artifacts_root).expect("artifacts root should exist");
        fs::create_dir_all(&tools_dir).expect("tools dir should exist");
        fs::create_dir_all(&logs_dir).expect("logs dir should exist");
        fs::create_dir_all(&audit_dir).expect("audit dir should exist");
        fs::write(&db_path, b"db").expect("db file should exist");
        fs::write(tools_dir.join("tool.txt"), b"tool").expect("tool file should exist");
        fs::write(logs_dir.join("log.txt"), b"log").expect("log file should exist");
        fs::write(audit_dir.join("audit.txt"), b"audit").expect("audit file should exist");
        let old_dir = artifacts_root.join("2026-05-31");
        fs::create_dir_all(&old_dir).expect("old dir should exist");
        write_file_with_size(&old_dir.join("old.txt"), 4);

        cleanup_artifact_root_for_day(
            &artifacts_root,
            &ArtifactDay::parse("2026-06-08").expect("day should parse"),
            &ArtifactDay::parse("2026-06-02").expect("day should parse"),
            1_000,
        )
        .expect("cleanup should succeed");

        assert!(db_path.is_file());
        assert!(tools_dir.join("tool.txt").is_file());
        assert!(logs_dir.join("log.txt").is_file());
        assert!(audit_dir.join("audit.txt").is_file());

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }
}
