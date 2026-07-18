//! Private, workspace-independent self-check for the platform publish boundary.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use serde::Serialize;

use crate::audit::AnchoredDir;
use crate::platform_publish::{
    publish_regular, PublishError, PublishFailureDetails, PublishMode, PublishOutcome,
    PublishPhase, PublishStrategy,
};

const SCHEMA_VERSION: u8 = 1;
const MAX_TEMP_ATTEMPTS: usize = 8;
const FIRST_BYTES: &[u8] = b"aopmem-platform-check-no-replace-v1";
const SECOND_BYTES: &[u8] = b"aopmem-platform-check-replace-v1";
const EXISTING_BYTES: &[u8] = b"aopmem-platform-check-existing-v1";
const KNOWN_CHILDREN: &[&str] = &[
    "source-no-replace",
    "destination",
    "source-existing",
    "source-replace",
    "source-escape",
];

#[derive(Debug, Serialize)]
pub(crate) struct PlatformCheckReport {
    schema_version: u8,
    status: &'static str,
    location: &'static str,
    observability_recorded: bool,
    admin_required: bool,
    checks: Vec<PlatformCheckResult>,
    cleanup: CleanupResult,
}

impl PlatformCheckReport {
    pub(crate) fn check_count(&self) -> usize {
        self.checks.len()
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct PlatformCheckResult {
    name: &'static str,
    passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    strategy: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    durability_confirmed: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CleanupResult {
    attempted: bool,
    files_removed: bool,
    directory_empty: bool,
    root_removed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlatformCheckFailure {
    pub(crate) code: &'static str,
    pub(crate) operation: &'static str,
    pub(crate) phase: &'static str,
    pub(crate) raw_os_error: Option<i32>,
    pub(crate) io_kind: &'static str,
    pub(crate) observability_recorded: bool,
    pub(crate) user_data_changed: bool,
    pub(crate) cleanup: CleanupResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) publish: Option<Box<PublishFailureView>>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PublishFailureView {
    code: &'static str,
    operation: &'static str,
    source: &'static str,
    destination: &'static str,
    mode: &'static str,
    strategy: &'static str,
    phase: &'static str,
    raw_os_error: Option<i32>,
    io_kind: &'static str,
    source_exists: bool,
    destination_exists: bool,
    source_size: Option<u64>,
    final_validated: bool,
    committed: bool,
    durability_confirmed: bool,
    temporary_cleanup_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
enum InjectedFault {
    None,
    AfterCreate,
    Error87,
    AfterNoReplace,
}

/// Runs the self-check without resolving AOPMem paths or opening databases.
pub(crate) fn run() -> Result<PlatformCheckReport, PlatformCheckFailure> {
    run_inner(
        #[cfg(test)]
        InjectedFault::None,
    )
}

fn run_inner(
    #[cfg(test)] fault: InjectedFault,
) -> Result<PlatformCheckReport, PlatformCheckFailure> {
    let temp_root = platform_temp_root().map_err(|error| {
        failure(
            "resolve_platform_temp",
            &error,
            CleanupResult {
                attempted: false,
                files_removed: false,
                directory_empty: false,
                root_removed: false,
            },
            None,
        )
    })?;
    let base = AnchoredDir::open_workspace(&temp_root, None).map_err(|error| {
        failure(
            "anchor_platform_temp",
            &error,
            CleanupResult {
                attempted: false,
                files_removed: false,
                directory_empty: false,
                root_removed: false,
            },
            None,
        )
    })?;
    let (name, anchor) = create_private_temp_directory(&base).map_err(|error| {
        failure(
            "create_private_temp",
            &error,
            CleanupResult {
                attempted: false,
                files_removed: false,
                directory_empty: false,
                root_removed: false,
            },
            None,
        )
    })?;
    let mut cleanup = CleanupGuard::new(base, name);

    let check_result = execute_checks(
        &anchor,
        #[cfg(test)]
        fault,
    );
    let cleanup_result = cleanup.finish(anchor);

    match (check_result, cleanup_result.root_removed) {
        (Ok(mut checks), true) => {
            checks.push(passed("bounded_known_file_cleanup"));
            Ok(PlatformCheckReport {
                schema_version: SCHEMA_VERSION,
                status: "pass",
                location: "private_os_temp",
                observability_recorded: false,
                admin_required: false,
                checks,
                cleanup: cleanup_result,
            })
        }
        (Ok(_), false) => Err(PlatformCheckFailure {
            code: "PLATFORM_CHECK_FAILED",
            operation: "platform_check",
            phase: "cleanup",
            raw_os_error: None,
            io_kind: "other",
            observability_recorded: false,
            user_data_changed: false,
            cleanup: cleanup_result,
            publish: None,
        }),
        (Err(mut error), _) => {
            error.cleanup = cleanup_result;
            Err(error)
        }
    }
}

fn execute_checks(
    parent: &AnchoredDir,
    #[cfg(test)] fault: InjectedFault,
) -> Result<Vec<PlatformCheckResult>, PlatformCheckFailure> {
    let mut checks = Vec::with_capacity(9);

    let source = create_source(parent, "source-no-replace", FIRST_BYTES)
        .map_err(|error| failure_without_cleanup("create_and_flush", &error, None))?;
    checks.push(passed("regular_file_create_and_writable_flush"));
    #[cfg(test)]
    if fault == InjectedFault::AfterCreate {
        return Err(failure_without_cleanup(
            "injected_after_create",
            &io::Error::other("injected check failure"),
            None,
        ));
    }

    #[cfg(test)]
    let first = if fault == InjectedFault::Error87 {
        crate::platform_publish::publish_regular_injected_os_error87(
            parent,
            source,
            OsStr::new("source-no-replace"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
        )
    } else {
        publish_regular(
            parent,
            source,
            OsStr::new("source-no-replace"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
        )
    };
    #[cfg(not(test))]
    let first = publish_regular(
        parent,
        source,
        OsStr::new("source-no-replace"),
        OsStr::new("destination"),
        PublishMode::NoReplace,
    );
    let first = first.map_err(|error| publish_failure("no_replace_publish", error))?;
    require_outcome(first, PublishMode::NoReplace)
        .map_err(|error| failure_without_cleanup("no_replace_publish", &error, None))?;
    checks.push(publish_passed("no_replace_publish", first));

    validate_bytes(parent, "destination", FIRST_BYTES)
        .map_err(|error| failure_without_cleanup("no_replace_reopen_validation", &error, None))?;
    checks.push(passed("no_replace_reopen_validation"));
    #[cfg(test)]
    if fault == InjectedFault::AfterNoReplace {
        return Err(failure_without_cleanup(
            "injected_after_no_replace",
            &io::Error::other("injected check failure"),
            None,
        ));
    }

    let existing = create_source(parent, "source-existing", EXISTING_BYTES)
        .map_err(|error| failure_without_cleanup("existing_no_replace_setup", &error, None))?;
    let expected_failure = match publish_regular(
        parent,
        existing,
        OsStr::new("source-existing"),
        OsStr::new("destination"),
        PublishMode::NoReplace,
    ) {
        Err(error) => error,
        Ok(_) => {
            return Err(failure_without_cleanup(
                "existing_no_replace_rejection",
                &io::Error::other("no-replace unexpectedly replaced destination"),
                None,
            ));
        }
    };
    if expected_failure.kind() != io::ErrorKind::AlreadyExists {
        return Err(publish_failure(
            "existing_no_replace_rejection",
            expected_failure,
        ));
    }
    validate_bytes(parent, "destination", FIRST_BYTES)
        .map_err(|error| failure_without_cleanup("existing_no_replace_unchanged", &error, None))?;
    checks.push(passed("existing_no_replace_rejected_unchanged"));

    let replacement = create_source(parent, "source-replace", SECOND_BYTES)
        .map_err(|error| failure_without_cleanup("replace_setup", &error, None))?;
    let replaced = publish_regular(
        parent,
        replacement,
        OsStr::new("source-replace"),
        OsStr::new("destination"),
        PublishMode::ReplaceOrCreate,
    )
    .map_err(|error| publish_failure("replace_existing_publish", error))?;
    require_outcome(replaced, PublishMode::ReplaceOrCreate)
        .map_err(|error| failure_without_cleanup("replace_existing_publish", &error, None))?;
    if !replaced.destination_existed {
        return Err(failure_without_cleanup(
            "replace_existing_publish",
            &io::Error::other("replace destination was not detected"),
            None,
        ));
    }
    checks.push(publish_passed("replace_existing_publish", replaced));

    validate_bytes(parent, "destination", SECOND_BYTES)
        .map_err(|error| failure_without_cleanup("replace_reopen_validation", &error, None))?;
    checks.push(passed("replace_reopen_validation"));

    let escape_source = create_source(parent, "source-escape", b"escape")
        .map_err(|error| failure_without_cleanup("path_escape_setup", &error, None))?;
    let escape = match publish_regular(
        parent,
        escape_source,
        OsStr::new("source-escape"),
        OsStr::new("../outside"),
        PublishMode::NoReplace,
    ) {
        Err(error) => error,
        Ok(_) => {
            return Err(failure_without_cleanup(
                "path_escape_rejection",
                &io::Error::other("non-child destination unexpectedly published"),
                None,
            ));
        }
    };
    if escape.details().phase != PublishPhase::ValidateDestination || escape.details().committed {
        return Err(publish_failure("path_escape_rejection", escape));
    }
    checks.push(passed("direct_child_escape_rejected"));
    parent.verify_logical_identity().map_err(|error| {
        failure_without_cleanup("root_reparse_identity_validation", &error, None)
    })?;
    checks.push(passed("root_reparse_identity_validated"));

    checks.push(passed("reparse_guard_contract_active"));

    Ok(checks)
}

fn create_source(parent: &AnchoredDir, name: &str, bytes: &[u8]) -> io::Result<File> {
    let mut file = parent.create_new_regular_os(OsStr::new(name))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(file)
}

fn validate_bytes(parent: &AnchoredDir, name: &str, expected: &[u8]) -> io::Result<()> {
    let file = parent.open_regular_os(OsStr::new(name))?;
    let mut bytes = Vec::with_capacity(expected.len().saturating_add(1));
    file.take(u64::try_from(expected.len()).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)?;
    if bytes == expected {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "published bytes differ",
        ))
    }
}

fn require_outcome(outcome: PublishOutcome, mode: PublishMode) -> io::Result<()> {
    if !outcome.committed
        || !outcome.final_validated
        || !outcome.temporary_cleanup_confirmed
        || outcome.strategy != expected_strategy(mode, outcome.destination_existed)
    {
        return Err(io::Error::other("publish outcome contract failed"));
    }
    Ok(())
}

const fn expected_strategy(mode: PublishMode, destination_exists: bool) -> PublishStrategy {
    #[cfg(windows)]
    {
        match (mode, destination_exists) {
            (PublishMode::ReplaceOrCreate, true) => PublishStrategy::WindowsReplaceFileW,
            _ => PublishStrategy::WindowsMoveFileExW,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = destination_exists;
        match mode {
            PublishMode::ReplaceOrCreate => PublishStrategy::UnixRenameAt,
            PublishMode::NoReplace => PublishStrategy::UnixLinkAtUnlinkAt,
        }
    }
}

fn passed(name: &'static str) -> PlatformCheckResult {
    PlatformCheckResult {
        name,
        passed: true,
        strategy: None,
        durability_confirmed: None,
    }
}

fn publish_passed(name: &'static str, outcome: PublishOutcome) -> PlatformCheckResult {
    PlatformCheckResult {
        name,
        passed: true,
        strategy: Some(strategy_name(outcome.strategy)),
        durability_confirmed: Some(outcome.durability_confirmed),
    }
}

fn publish_failure(phase: &'static str, error: PublishError) -> PlatformCheckFailure {
    let details = error.details();
    let io_error = error.into_io_error();
    failure_without_cleanup(phase, &io_error, Some(details.into()))
}

fn failure_without_cleanup(
    phase: &'static str,
    error: &io::Error,
    publish: Option<PublishFailureView>,
) -> PlatformCheckFailure {
    failure(
        phase,
        error,
        CleanupResult {
            attempted: false,
            files_removed: false,
            directory_empty: false,
            root_removed: false,
        },
        publish,
    )
}

fn failure(
    phase: &'static str,
    error: &io::Error,
    cleanup: CleanupResult,
    publish: Option<PublishFailureView>,
) -> PlatformCheckFailure {
    PlatformCheckFailure {
        code: "PLATFORM_CHECK_FAILED",
        operation: "platform_check",
        phase,
        raw_os_error: error.raw_os_error(),
        io_kind: io_kind(error.kind()),
        observability_recorded: false,
        user_data_changed: false,
        cleanup,
        publish: publish.map(Box::new),
    }
}

impl From<PublishFailureDetails> for PublishFailureView {
    fn from(details: PublishFailureDetails) -> Self {
        Self {
            code: details.code,
            operation: details.operation,
            source: details.source,
            destination: details.destination,
            mode: mode_name(details.mode),
            strategy: strategy_name(details.strategy),
            phase: publish_phase_name(details.phase),
            raw_os_error: details.raw_os_error,
            io_kind: details.io_kind,
            source_exists: details.source_exists,
            destination_exists: details.destination_exists,
            source_size: details.source_size,
            final_validated: details.final_validated,
            committed: details.committed,
            durability_confirmed: details.durability_confirmed,
            temporary_cleanup_confirmed: details.temporary_cleanup_confirmed,
        }
    }
}

const fn mode_name(mode: PublishMode) -> &'static str {
    match mode {
        PublishMode::ReplaceOrCreate => "replace_or_create",
        PublishMode::NoReplace => "no_replace",
    }
}

const fn strategy_name(strategy: PublishStrategy) -> &'static str {
    match strategy {
        PublishStrategy::Undetermined => "undetermined",
        PublishStrategy::WindowsReplaceFileW => "windows_replace_file_w",
        PublishStrategy::WindowsMoveFileExW => "windows_move_file_ex_w",
        PublishStrategy::UnixRenameAt => "unix_rename_at",
        PublishStrategy::UnixLinkAtUnlinkAt => "unix_link_at_unlink_at",
    }
}

const fn publish_phase_name(phase: PublishPhase) -> &'static str {
    match phase {
        PublishPhase::ValidateParent => "validate_parent",
        PublishPhase::ValidateSource => "validate_source",
        PublishPhase::ValidateDestination => "validate_destination",
        PublishPhase::FlushSource => "flush_source",
        PublishPhase::CloseHandles => "close_handles",
        PublishPhase::OsPublish => "os_publish",
        PublishPhase::ReopenDestination => "reopen_destination",
        PublishPhase::ValidatePublishedIdentity => "validate_published_identity",
        PublishPhase::SyncParent => "sync_parent",
    }
}

const fn io_kind(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::NotFound => "not_found",
        io::ErrorKind::PermissionDenied => "permission_denied",
        io::ErrorKind::AlreadyExists => "already_exists",
        io::ErrorKind::InvalidInput => "invalid_input",
        io::ErrorKind::InvalidData => "invalid_data",
        io::ErrorKind::WriteZero => "write_zero",
        io::ErrorKind::Interrupted => "interrupted",
        io::ErrorKind::Unsupported => "unsupported",
        _ => "other",
    }
}

fn create_private_temp_directory(base: &AnchoredDir) -> io::Result<(String, AnchoredDir)> {
    for _ in 0..MAX_TEMP_ATTEMPTS {
        let name = format!("aopmem-platform-check-{}", uuid::Uuid::new_v4().simple());
        match base.create_new_child_dir_os(OsStr::new(&name)) {
            Ok(root) => return Ok((name, root)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "bounded private temp creation attempts exhausted",
    ))
}

#[cfg(unix)]
fn platform_temp_root() -> io::Result<PathBuf> {
    Ok(PathBuf::from("/tmp"))
}

#[cfg(windows)]
fn platform_temp_root() -> io::Result<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Storage::FileSystem::GetTempPathW;

    let mut buffer = vec![0_u16; 32_768];
    // SAFETY: buffer is writable for its declared length.
    let length = unsafe {
        GetTempPathW(
            u32::try_from(buffer.len()).unwrap_or(u32::MAX),
            buffer.as_mut_ptr(),
        )
    };
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    let length = usize::try_from(length)
        .map_err(|_| io::Error::other("platform temp path length overflow"))?;
    if length >= buffer.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "platform temp path exceeds bounded buffer",
        ));
    }
    buffer.truncate(length);
    Ok(PathBuf::from(OsString::from_wide(&buffer)))
}

struct CleanupGuard {
    base: AnchoredDir,
    name: String,
    completed: bool,
}

impl CleanupGuard {
    fn new(base: AnchoredDir, name: String) -> Self {
        Self {
            base,
            name,
            completed: false,
        }
    }

    fn finish(&mut self, anchor: AnchoredDir) -> CleanupResult {
        let identity_valid = anchor.verify_logical_identity().is_ok();
        let mut files_removed = true;
        if identity_valid {
            for name in KNOWN_CHILDREN {
                match anchor.remove_regular_os(OsStr::new(name)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(_) => files_removed = false,
                }
            }
        } else {
            files_removed = false;
        }
        drop(anchor);
        let root_removed = identity_valid
            && files_removed
            && self
                .base
                .remove_empty_child_dir_os(OsStr::new(&self.name))
                .is_ok();
        let directory_empty = root_removed;
        self.completed = root_removed;
        CleanupResult {
            attempted: true,
            files_removed,
            directory_empty,
            root_removed,
        }
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let _ = self.base.remove_empty_child_dir_os(OsStr::new(&self.name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_check_passes_repeatedly_and_removes_private_root() {
        for _ in 0..3 {
            let report = run().expect("platform check");
            assert_eq!(report.status, "pass");
            assert!(!report.observability_recorded);
            assert!(!report.admin_required);
            assert!(report.cleanup.root_removed);
            assert_eq!(report.checks.len(), 10);
            assert!(report.checks.iter().all(|check| check.passed));
        }
    }

    #[test]
    fn every_injected_failure_cleans_up_and_error_87_stays_structured_private() {
        for fault in [
            InjectedFault::AfterCreate,
            InjectedFault::Error87,
            InjectedFault::AfterNoReplace,
        ] {
            let failure = run_inner(fault).expect_err("injected failure");
            assert!(failure.cleanup.attempted);
            assert!(failure.cleanup.files_removed);
            assert!(failure.cleanup.directory_empty);
            assert!(failure.cleanup.root_removed);
            assert!(!failure.observability_recorded);
            assert!(!failure.user_data_changed);
            let rendered = serde_json::to_string(&failure).expect("failure JSON");
            assert!(!rendered.contains(&std::env::temp_dir().display().to_string()));
            if fault == InjectedFault::Error87 {
                assert_eq!(failure.raw_os_error, Some(87));
                let publish = failure.publish.expect("publish details");
                assert_eq!(publish.raw_os_error, Some(87));
                assert_eq!(publish.phase, "os_publish");
                assert!(!publish.committed);
            }
        }
    }

    #[test]
    fn production_self_check_has_no_workspace_or_external_traversal_entry_points() {
        let source = include_str!("platform_check.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source");
        for forbidden in [
            "storage::",
            "install::",
            "LocalCollector",
            "CommandObservation",
            concat!("remove_dir", "_all"),
            ".exists()",
            "read_dir",
        ] {
            assert!(
                !production.contains(forbidden),
                "production self-check contains forbidden entry point {forbidden}"
            );
        }
    }
}
