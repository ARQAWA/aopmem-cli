use std::fs::File;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};

use crate::audit::AnchoredDir;

const BACKUP_PAGE_BATCH: i32 = 256;
const BACKUP_BUSY_TIMEOUT: Duration = Duration::from_secs(30);
const BACKUP_BUSY_PAUSE: Duration = Duration::from_millis(10);

pub(super) fn online_backup_to_path(
    source: &Connection,
    destination_dir: &Path,
    destination_path: &Path,
) -> io::Result<()> {
    // SQLite's Online Backup API includes committed WAL frames. A byte copy of
    // the main database does not, so it is never safe at this boundary.
    let destination_root = AnchoredDir::open_workspace(destination_dir, None)?;
    let destination_name = destination_path
        .file_name()
        .ok_or_else(|| io::Error::other("database backup has no file name"))?;
    let empty = destination_root.create_new_regular_os(destination_name)?;
    empty.sync_all()?;
    drop(empty);
    destination_root.sync()?;
    let canonical_destination = destination_path.canonicalize()?;

    let mut destination = Connection::open_with_flags(
        canonical_destination,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(sqlite_io)?;
    destination
        .execute_batch("PRAGMA synchronous = FULL; PRAGMA journal_mode = DELETE;")
        .map_err(sqlite_io)?;
    run_bounded_backup(source, &mut destination)?;
    let check: String = destination
        .query_row("PRAGMA quick_check(1);", [], |row| row.get(0))
        .map_err(sqlite_io)?;
    if check != "ok" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("backup quick_check failed: {check}"),
        ));
    }
    drop(destination);
    File::open(destination_path)?.sync_all()?;
    destination_root.sync()
}

fn run_bounded_backup(source: &Connection, destination: &mut Connection) -> io::Result<()> {
    let backup = Backup::new(source, destination).map_err(sqlite_io)?;
    let started = Instant::now();
    loop {
        match backup.step(BACKUP_PAGE_BATCH).map_err(sqlite_io)? {
            StepResult::Done => return Ok(()),
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => {
                if started.elapsed() >= BACKUP_BUSY_TIMEOUT {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "SQLite Online Backup remained busy for 30 seconds",
                    ));
                }
                thread::sleep(BACKUP_BUSY_PAUSE);
            }
            _ => {
                return Err(io::Error::other(
                    "SQLite Online Backup returned an unknown state",
                ));
            }
        }
    }
}

fn sqlite_io(error: rusqlite::Error) -> io::Error {
    io::Error::other(error)
}
