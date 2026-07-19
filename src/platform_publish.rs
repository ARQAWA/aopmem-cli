//! One anchored, cross-platform boundary for publishing a regular file.
//!
//! The helper owns the writable source handle. It verifies and flushes that
//! exact file, closes publication-conflicting handles, performs one bounded
//! same-parent OS operation, and reopens the destination for identity proof.

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::Path;

use crate::audit::AnchoredDir;

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PublishMode {
    ReplaceOrCreate,
    NoReplace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum PublishStrategy {
    Undetermined,
    WindowsReplaceFileW,
    WindowsMoveFileExW,
    #[allow(dead_code)]
    UnixRenameAt,
    #[allow(dead_code)]
    UnixLinkAtUnlinkAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PublishPhase {
    ValidateParent,
    ValidateSource,
    ValidateDestination,
    FlushSource,
    #[allow(dead_code)]
    CloseHandles,
    OsPublish,
    ReopenDestination,
    ValidatePublishedIdentity,
    SyncParent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PublishOutcome {
    pub(crate) strategy: PublishStrategy,
    pub(crate) destination_existed: bool,
    pub(crate) committed: bool,
    pub(crate) final_validated: bool,
    pub(crate) durability_confirmed: bool,
    pub(crate) temporary_cleanup_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PublishFailureDetails {
    pub(crate) code: &'static str,
    pub(crate) operation: &'static str,
    pub(crate) source: &'static str,
    pub(crate) destination: &'static str,
    pub(crate) mode: PublishMode,
    pub(crate) strategy: PublishStrategy,
    pub(crate) phase: PublishPhase,
    pub(crate) raw_os_error: Option<i32>,
    pub(crate) io_kind: &'static str,
    pub(crate) source_exists: bool,
    pub(crate) destination_exists: bool,
    pub(crate) source_size: Option<u64>,
    pub(crate) final_validated: bool,
    pub(crate) committed: bool,
    pub(crate) durability_confirmed: bool,
    pub(crate) temporary_cleanup_confirmed: bool,
    pub(crate) handle_diagnostics: Option<PublishHandleDiagnostics>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PublishHandleDiagnostics {
    pub(crate) handle_role: &'static str,
    pub(crate) desired_access: &'static str,
    pub(crate) share_mode: &'static str,
    pub(crate) creation_disposition: &'static str,
    pub(crate) flags: &'static str,
    pub(crate) handle_expected_closed: bool,
}

#[derive(Debug)]
pub(crate) struct PublishError {
    details: Box<PublishFailureDetails>,
    source: Box<io::Error>,
}

impl PublishError {
    #[allow(dead_code)]
    pub(crate) fn details(&self) -> PublishFailureDetails {
        *self.details
    }

    pub(crate) fn kind(&self) -> io::ErrorKind {
        self.source.kind()
    }

    pub(crate) fn into_io_error(self) -> io::Error {
        match self.details.raw_os_error {
            Some(code) => io::Error::from_raw_os_error(code),
            None => io::Error::new(self.source.kind(), self),
        }
    }
}

impl fmt::Display for PublishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} during {:?} via {:?} (kind={}, os={:?}, committed={})",
            self.details.code,
            self.details.phase,
            self.details.strategy,
            self.details.io_kind,
            self.details.raw_os_error,
            self.details.committed
        )
    }
}

impl Error for PublishError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
    size: u64,
}

#[derive(Debug, Clone, Copy)]
struct PublishState {
    strategy: PublishStrategy,
    committed: bool,
    temporary_cleanup_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InjectedFault {
    None,
    FlushSource,
    SourceValidationError32,
    OsError87,
    ReopenDestination,
    ValidatePublishedIdentity,
    SyncParent,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleEvent {
    SourceWriterFlushed,
    SourceIdentityCaptured,
    SourceWriterDropped,
    SourceValidationStarted,
    SourceValidationOpened,
    SourceValidationClosed,
    DestinationValidationOpened,
    DestinationValidationClosed,
    OsPublishStarted,
    FinalValidationStarted,
}

#[cfg(test)]
thread_local! {
    static LIFECYCLE_EVENTS: std::cell::RefCell<Vec<LifecycleEvent>> = const {
        std::cell::RefCell::new(Vec::new())
    };
}

#[cfg(test)]
fn record_lifecycle(event: LifecycleEvent) {
    LIFECYCLE_EVENTS.with(|events| events.borrow_mut().push(event));
}

#[cfg(test)]
fn take_lifecycle_events() -> Vec<LifecycleEvent> {
    LIFECYCLE_EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
}

macro_rules! lifecycle_event {
    ($event:ident) => {
        #[cfg(test)]
        record_lifecycle(LifecycleEvent::$event);
    };
}

/// Publishes one already-open direct child to another direct child.
///
/// # Errors
///
/// Returns a path-private structured error when validation, flushing,
/// publication, reopening, identity validation, or durability fails.
pub(crate) fn publish_regular(
    parent: &AnchoredDir,
    source: File,
    source_name: &OsStr,
    destination_name: &OsStr,
    mode: PublishMode,
) -> Result<PublishOutcome, PublishError> {
    publish_regular_inner(
        parent,
        source,
        source_name,
        destination_name,
        mode,
        InjectedFault::None,
    )
}

#[cfg(test)]
pub(crate) fn publish_regular_injected_os_error87(
    parent: &AnchoredDir,
    source: File,
    source_name: &OsStr,
    destination_name: &OsStr,
    mode: PublishMode,
) -> Result<PublishOutcome, PublishError> {
    publish_regular_inner(
        parent,
        source,
        source_name,
        destination_name,
        mode,
        InjectedFault::OsError87,
    )
}

#[cfg(test)]
pub(crate) fn publish_regular_injected_source_validation_error32(
    parent: &AnchoredDir,
    source: File,
    source_name: &OsStr,
    destination_name: &OsStr,
    mode: PublishMode,
) -> Result<PublishOutcome, PublishError> {
    publish_regular_inner(
        parent,
        source,
        source_name,
        destination_name,
        mode,
        InjectedFault::SourceValidationError32,
    )
}

#[cfg(test)]
pub(crate) fn publish_regular_injected_sync_parent(
    parent: &AnchoredDir,
    source: File,
    source_name: &OsStr,
    destination_name: &OsStr,
    mode: PublishMode,
) -> Result<PublishOutcome, PublishError> {
    publish_regular_inner(
        parent,
        source,
        source_name,
        destination_name,
        mode,
        InjectedFault::SyncParent,
    )
}

fn publish_regular_inner(
    parent: &AnchoredDir,
    source: File,
    source_name: &OsStr,
    destination_name: &OsStr,
    mode: PublishMode,
    fault: InjectedFault,
) -> Result<PublishOutcome, PublishError> {
    let mut context = FailureContext::new(mode);
    validate_component(source_name)
        .map_err(|error| context.error(PublishPhase::ValidateSource, error))?;
    validate_component(destination_name)
        .map_err(|error| context.error(PublishPhase::ValidateDestination, error))?;
    if source_name == destination_name {
        return Err(context.error(
            PublishPhase::ValidateDestination,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "publish children must be distinct",
            ),
        ));
    }

    parent
        .verify_logical_identity()
        .map_err(|error| context.error(PublishPhase::ValidateParent, error))?;

    if fault == InjectedFault::FlushSource {
        return Err(context.error(
            PublishPhase::FlushSource,
            io::Error::other("injected source flush failure"),
        ));
    }
    source
        .sync_all()
        .map_err(|error| context.error(PublishPhase::FlushSource, error))?;
    lifecycle_event!(SourceWriterFlushed);
    let expected = file_identity(&source)
        .map_err(|error| context.error(PublishPhase::ValidateSource, error))?;
    lifecycle_event!(SourceIdentityCaptured);
    context.source_size = Some(expected.size);
    drop(source);
    lifecycle_event!(SourceWriterDropped);

    let source_identity = {
        lifecycle_event!(SourceValidationStarted);
        #[cfg(windows)]
        context.source_validation_handle_diagnostics();
        if fault == InjectedFault::SourceValidationError32 {
            context.source_exists = true;
            return Err(context.error(
                PublishPhase::ValidateSource,
                io::Error::from_raw_os_error(32),
            ));
        }
        let source = parent
            .open_regular_os(source_name)
            .map_err(|error| context.error(PublishPhase::ValidateSource, error))?;
        context.source_exists = true;
        lifecycle_event!(SourceValidationOpened);
        file_identity(&source)
            .map_err(|error| context.error(PublishPhase::ValidateSource, error))?
    };
    lifecycle_event!(SourceValidationClosed);
    if source_identity != expected {
        return Err(context.error(
            PublishPhase::ValidateSource,
            io::Error::other("publish source identity changed"),
        ));
    }
    context.handle_diagnostics = None;

    let destination_identity = {
        let destination = parent
            .open_regular_optional_os(destination_name)
            .map_err(|error| context.error(PublishPhase::ValidateDestination, error))?;
        context.destination_existed = destination.is_some();
        context.destination_exists = context.destination_existed;
        lifecycle_event!(DestinationValidationOpened);
        destination
            .as_ref()
            .map(file_identity)
            .transpose()
            .map_err(|error| context.error(PublishPhase::ValidateDestination, error))?
    };
    lifecycle_event!(DestinationValidationClosed);
    context.strategy = strategy(mode, context.destination_existed);
    if destination_identity.is_some_and(|identity| identity == expected) {
        return Err(context.error(
            PublishPhase::ValidateDestination,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "publish source and destination identify the same file",
            ),
        ));
    }
    if mode == PublishMode::NoReplace && context.destination_existed {
        return Err(context.error(
            PublishPhase::ValidateDestination,
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "publish destination already exists",
            ),
        ));
    }

    parent
        .verify_logical_identity()
        .map_err(|error| context.error(PublishPhase::ValidateParent, error))?;

    lifecycle_event!(OsPublishStarted);
    let state = if fault == InjectedFault::OsError87 {
        context.refresh_existence(parent, source_name, destination_name);
        return Err(context.error(PublishPhase::OsPublish, io::Error::from_raw_os_error(87)));
    } else {
        os_publish(
            parent,
            source_name,
            destination_name,
            mode,
            context.destination_existed,
            expected,
        )
        .map_err(|failure| {
            context.strategy = failure.strategy;
            context.committed = failure.committed;
            context.temporary_cleanup_confirmed = failure.temporary_cleanup_confirmed;
            context.source_exists = failure.source_exists;
            context.destination_exists = failure.destination_exists;
            context.final_validated = failure.final_validated;
            context.error(PublishPhase::OsPublish, failure.error)
        })?
    };
    context.strategy = state.strategy;
    context.committed = state.committed;
    lifecycle_event!(FinalValidationStarted);
    context.source_exists = parent
        .open_regular_optional_os(source_name)
        .map_err(|error| context.error(PublishPhase::ValidatePublishedIdentity, error))?
        .is_some();
    context.temporary_cleanup_confirmed =
        state.temporary_cleanup_confirmed && !context.source_exists;
    context.destination_exists = true;

    if fault == InjectedFault::ReopenDestination {
        return Err(context.error(
            PublishPhase::ReopenDestination,
            io::Error::other("injected destination reopen failure"),
        ));
    }
    let published = parent
        .open_regular_os(destination_name)
        .map_err(|error| context.error(PublishPhase::ReopenDestination, error))?;
    if fault == InjectedFault::ValidatePublishedIdentity {
        return Err(context.error(
            PublishPhase::ValidatePublishedIdentity,
            io::Error::other("injected published identity failure"),
        ));
    }
    let actual = file_identity(&published)
        .map_err(|error| context.error(PublishPhase::ValidatePublishedIdentity, error))?;
    if actual != expected {
        return Err(context.error(
            PublishPhase::ValidatePublishedIdentity,
            io::Error::other("published file identity differs from source"),
        ));
    }
    context.final_validated = true;
    drop(published);

    if fault == InjectedFault::SyncParent {
        return Err(context.error(
            PublishPhase::SyncParent,
            io::Error::other("injected parent sync failure"),
        ));
    }
    let parent_durability = parent.sync_for_publish().map_err(|error| {
        context.durability_confirmed = false;
        context.error(PublishPhase::SyncParent, error)
    })?;
    let durability_confirmed = parent_durability || strategy_confirms_durability(state.strategy);
    context.durability_confirmed = durability_confirmed;

    Ok(PublishOutcome {
        strategy: state.strategy,
        destination_existed: context.destination_existed,
        committed: true,
        final_validated: true,
        durability_confirmed,
        temporary_cleanup_confirmed: context.temporary_cleanup_confirmed,
    })
}

pub(crate) fn require_committed_validated_clean(outcome: PublishOutcome) -> io::Result<()> {
    if outcome.committed && outcome.final_validated && outcome.temporary_cleanup_confirmed {
        Ok(())
    } else {
        Err(io::Error::other(
            "publish did not confirm commit, final identity, and temporary cleanup",
        ))
    }
}

const fn strategy_confirms_durability(strategy: PublishStrategy) -> bool {
    matches!(strategy, PublishStrategy::WindowsMoveFileExW)
}

struct FailureContext {
    mode: PublishMode,
    strategy: PublishStrategy,
    source_exists: bool,
    destination_exists: bool,
    destination_existed: bool,
    committed: bool,
    durability_confirmed: bool,
    temporary_cleanup_confirmed: bool,
    source_size: Option<u64>,
    final_validated: bool,
    handle_diagnostics: Option<PublishHandleDiagnostics>,
}

impl FailureContext {
    fn new(mode: PublishMode) -> Self {
        Self {
            mode,
            strategy: PublishStrategy::Undetermined,
            source_exists: false,
            destination_exists: false,
            destination_existed: false,
            committed: false,
            durability_confirmed: false,
            temporary_cleanup_confirmed: false,
            source_size: None,
            final_validated: false,
            handle_diagnostics: None,
        }
    }

    fn refresh_existence(
        &mut self,
        parent: &AnchoredDir,
        source_name: &OsStr,
        destination_name: &OsStr,
    ) {
        self.source_exists = parent
            .open_regular_optional_os(source_name)
            .map_or(true, |entry| entry.is_some());
        self.destination_exists = parent
            .open_regular_optional_os(destination_name)
            .map_or(true, |entry| entry.is_some());
    }

    fn error(&self, phase: PublishPhase, source: io::Error) -> PublishError {
        PublishError {
            details: Box::new(PublishFailureDetails {
                code: "PLATFORM_PUBLISH_FAILED",
                operation: "publish_regular",
                source: "source_child",
                destination: "destination_child",
                mode: self.mode,
                strategy: self.strategy,
                phase,
                raw_os_error: source.raw_os_error(),
                io_kind: io_kind(source.kind()),
                source_exists: self.source_exists,
                destination_exists: self.destination_exists,
                source_size: self.source_size,
                final_validated: self.final_validated,
                committed: self.committed,
                durability_confirmed: self.durability_confirmed,
                temporary_cleanup_confirmed: self.temporary_cleanup_confirmed,
                handle_diagnostics: self.handle_diagnostics,
            }),
            source: Box::new(source),
        }
    }
}

#[cfg(windows)]
impl FailureContext {
    fn source_validation_handle_diagnostics(&mut self) {
        self.handle_diagnostics = Some(PublishHandleDiagnostics {
            handle_role: "source_validation",
            desired_access: "GENERIC_READ | FILE_READ_ATTRIBUTES",
            share_mode: "FILE_SHARE_READ | FILE_SHARE_WRITE",
            creation_disposition: "OPEN_EXISTING",
            flags: "FILE_FLAG_OPEN_REPARSE_POINT",
            handle_expected_closed: true,
        });
    }
}

fn validate_component(name: &OsStr) -> io::Result<()> {
    let mut components = Path::new(name).components();
    if name.is_empty()
        || !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "publish name is not a direct child",
        ));
    }
    Ok(())
}

const fn strategy(mode: PublishMode, destination_exists: bool) -> PublishStrategy {
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

#[cfg(unix)]
fn file_identity(file: &File) -> io::Result<FileIdentity> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "publish source is not regular",
        ));
    }
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.len(),
    })
}

#[cfg(windows)]
fn file_identity(file: &File) -> io::Result<FileIdentity> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: file owns a live handle and information is writable.
    let result =
        unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, information.as_mut_ptr()) };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful GetFileInformationByHandle initialized the value.
    let information = unsafe { information.assume_init() };
    let file_index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    let size = (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow);
    Ok(FileIdentity {
        volume_serial: information.dwVolumeSerialNumber,
        file_index,
        size,
    })
}

struct OsPublishFailure {
    strategy: PublishStrategy,
    committed: bool,
    temporary_cleanup_confirmed: bool,
    source_exists: bool,
    destination_exists: bool,
    final_validated: bool,
    error: io::Error,
}

#[cfg(unix)]
fn os_publish(
    parent: &AnchoredDir,
    source_name: &OsStr,
    destination_name: &OsStr,
    mode: PublishMode,
    destination_existed: bool,
    expected: FileIdentity,
) -> Result<PublishState, OsPublishFailure> {
    let source = c_name(source_name).map_err(|error| OsPublishFailure {
        strategy: strategy(mode, destination_existed),
        committed: false,
        temporary_cleanup_confirmed: false,
        source_exists: true,
        destination_exists: destination_existed,
        final_validated: false,
        error,
    })?;
    let destination = c_name(destination_name).map_err(|error| OsPublishFailure {
        strategy: strategy(mode, destination_existed),
        committed: false,
        temporary_cleanup_confirmed: false,
        source_exists: true,
        destination_exists: destination_existed,
        final_validated: false,
        error,
    })?;
    let directory = parent.raw_directory_fd();

    match mode {
        PublishMode::ReplaceOrCreate => {
            if system_renameat(directory, source.as_ptr(), directory, destination.as_ptr()) == 0 {
                Ok(PublishState {
                    strategy: PublishStrategy::UnixRenameAt,
                    committed: true,
                    temporary_cleanup_confirmed: true,
                })
            } else {
                let error = io::Error::last_os_error();
                let post = post_failure(parent, source_name, destination_name, expected);
                Err(OsPublishFailure {
                    strategy: PublishStrategy::UnixRenameAt,
                    committed: post.committed,
                    temporary_cleanup_confirmed: post.source_known_absent,
                    source_exists: post.source_exists,
                    destination_exists: post.destination_exists,
                    final_validated: post.committed,
                    error,
                })
            }
        }
        PublishMode::NoReplace => {
            if system_linkat(
                directory,
                source.as_ptr(),
                directory,
                destination.as_ptr(),
                0,
            ) != 0
            {
                let error = io::Error::last_os_error();
                let post = post_failure(parent, source_name, destination_name, expected);
                return Err(OsPublishFailure {
                    strategy: PublishStrategy::UnixLinkAtUnlinkAt,
                    committed: post.committed,
                    temporary_cleanup_confirmed: post.source_known_absent,
                    source_exists: post.source_exists,
                    destination_exists: post.destination_exists,
                    final_validated: post.committed,
                    error,
                });
            }
            let cleanup = system_unlinkat(directory, source.as_ptr(), 0) == 0;
            Ok(PublishState {
                strategy: PublishStrategy::UnixLinkAtUnlinkAt,
                committed: true,
                temporary_cleanup_confirmed: cleanup,
            })
        }
    }
}

#[cfg(windows)]
fn os_publish(
    parent: &AnchoredDir,
    source_name: &OsStr,
    destination_name: &OsStr,
    mode: PublishMode,
    destination_existed: bool,
    expected: FileIdentity,
) -> Result<PublishState, OsPublishFailure> {
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_WRITE_THROUGH,
    };

    let source = crate::windows_path::verbatim_wide_path(&parent.logical_path().join(source_name))
        .map_err(|error| OsPublishFailure {
            strategy: strategy(mode, destination_existed),
            committed: false,
            temporary_cleanup_confirmed: false,
            source_exists: true,
            destination_exists: destination_existed,
            final_validated: false,
            error,
        })?;
    let destination =
        crate::windows_path::verbatim_wide_path(&parent.logical_path().join(destination_name))
            .map_err(|error| OsPublishFailure {
                strategy: strategy(mode, destination_existed),
                committed: false,
                temporary_cleanup_confirmed: false,
                source_exists: true,
                destination_exists: destination_existed,
                final_validated: false,
                error,
            })?;

    let mut replace_attempt = mode == PublishMode::ReplaceOrCreate && destination_existed;
    for attempt in 0..2 {
        let current_strategy = if replace_attempt {
            PublishStrategy::WindowsReplaceFileW
        } else {
            PublishStrategy::WindowsMoveFileExW
        };
        // SAFETY: both paths are live, NUL-terminated, same-parent paths.
        let result = unsafe {
            if replace_attempt {
                ReplaceFileW(
                    destination.as_ptr(),
                    source.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
                    std::ptr::null(),
                )
            } else {
                MoveFileExW(
                    source.as_ptr(),
                    destination.as_ptr(),
                    MOVEFILE_WRITE_THROUGH,
                )
            }
        };
        if result != 0 {
            return Ok(PublishState {
                strategy: current_strategy,
                committed: true,
                temporary_cleanup_confirmed: true,
            });
        }
        let error = io::Error::last_os_error();
        let raw_error = error.raw_os_error();
        let post = post_failure(parent, source_name, destination_name, expected);
        let can_retry = attempt == 0
            && mode == PublishMode::ReplaceOrCreate
            && post.source_matches
            && !post.committed
            && if replace_attempt {
                matches!(raw_error, Some(2 | 3)) && !post.destination_exists
            } else {
                matches!(raw_error, Some(80 | 183)) && post.destination_validated
            };
        if can_retry {
            replace_attempt = !replace_attempt;
            continue;
        }
        return Err(OsPublishFailure {
            strategy: current_strategy,
            committed: post.committed,
            temporary_cleanup_confirmed: post.source_known_absent,
            source_exists: post.source_exists,
            destination_exists: post.destination_exists,
            final_validated: post.committed,
            error,
        });
    }
    unreachable!("bounded Windows publish loop returns on its second attempt")
}

struct PostFailure {
    source_exists: bool,
    destination_exists: bool,
    source_known_absent: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    destination_validated: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    source_matches: bool,
    committed: bool,
}

fn post_failure(
    parent: &AnchoredDir,
    source_name: &OsStr,
    destination_name: &OsStr,
    expected: FileIdentity,
) -> PostFailure {
    let source = parent.open_regular_optional_os(source_name);
    let destination = parent.open_regular_optional_os(destination_name);
    let source_known_absent = matches!(source, Ok(None));
    let source_exists = !source_known_absent;
    let destination_exists = !matches!(destination, Ok(None));
    let destination_validated = destination.as_ref().is_ok_and(|entry| entry.is_some());
    let source_matches = source
        .ok()
        .flatten()
        .and_then(|file| file_identity(&file).ok())
        .is_some_and(|identity| identity == expected);
    let committed = destination
        .ok()
        .flatten()
        .and_then(|file| file_identity(&file).ok())
        .is_some_and(|identity| identity == expected);
    PostFailure {
        source_exists,
        destination_exists,
        source_known_absent,
        destination_validated,
        source_matches,
        committed,
    }
}

#[cfg(target_os = "macos")]
fn system_renameat(
    source_parent: i32,
    source: *const i8,
    destination_parent: i32,
    destination: *const i8,
) -> i32 {
    // SAFETY: names are NUL-terminated and parents are open directories.
    unsafe { libc::renameat(source_parent, source, destination_parent, destination) }
}

#[cfg(target_os = "macos")]
fn system_linkat(
    source_parent: i32,
    source: *const i8,
    destination_parent: i32,
    destination: *const i8,
    flags: i32,
) -> i32 {
    // SAFETY: names are NUL-terminated and parents are open directories.
    unsafe {
        libc::linkat(
            source_parent,
            source,
            destination_parent,
            destination,
            flags,
        )
    }
}

#[cfg(target_os = "macos")]
fn system_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32 {
    // SAFETY: name is NUL-terminated and parent is an open directory.
    unsafe { libc::unlinkat(parent, name, flags) }
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    #[link_name = "renameat"]
    fn linux_renameat(
        source_parent: i32,
        source: *const i8,
        destination_parent: i32,
        destination: *const i8,
    ) -> i32;
    #[link_name = "linkat"]
    fn linux_linkat(
        source_parent: i32,
        source: *const i8,
        destination_parent: i32,
        destination: *const i8,
        flags: i32,
    ) -> i32;
    #[link_name = "unlinkat"]
    fn linux_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32;
}

#[cfg(target_os = "linux")]
fn system_renameat(
    source_parent: i32,
    source: *const i8,
    destination_parent: i32,
    destination: *const i8,
) -> i32 {
    // SAFETY: names are NUL-terminated and parents are open directories.
    unsafe { linux_renameat(source_parent, source, destination_parent, destination) }
}

#[cfg(target_os = "linux")]
fn system_linkat(
    source_parent: i32,
    source: *const i8,
    destination_parent: i32,
    destination: *const i8,
    flags: i32,
) -> i32 {
    // SAFETY: names are NUL-terminated and parents are open directories.
    unsafe {
        linux_linkat(
            source_parent,
            source,
            destination_parent,
            destination,
            flags,
        )
    }
}

#[cfg(target_os = "linux")]
fn system_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32 {
    // SAFETY: name is NUL-terminated and parent is an open directory.
    unsafe { linux_unlinkat(parent, name, flags) }
}

#[cfg(unix)]
fn c_name(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "publish name contains NUL"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(label: &str) -> (PathBuf, AnchoredDir) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aopmem-platform-publish-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("root");
        let anchored = AnchoredDir::open_workspace(&root, None).expect("anchor");
        (root, anchored)
    }

    fn source(parent: &AnchoredDir, name: &str, bytes: &[u8]) -> File {
        let mut file = parent
            .create_new_regular_os(OsStr::new(name))
            .expect("source");
        file.write_all(bytes).expect("write");
        file
    }

    #[test]
    fn replace_existing_and_create_absent_preserve_source_identity() {
        let (root, parent) = fixture("replace");
        fs::write(root.join("destination"), b"old").expect("destination");
        let result = publish_regular(
            &parent,
            source(&parent, "source", b"new"),
            OsStr::new("source"),
            OsStr::new("destination"),
            PublishMode::ReplaceOrCreate,
        )
        .expect("replace");
        assert!(result.committed);
        assert!(result.destination_existed);
        assert_eq!(fs::read(root.join("destination")).expect("read"), b"new");

        let created = publish_regular(
            &parent,
            source(&parent, "source-2", b"created"),
            OsStr::new("source-2"),
            OsStr::new("created"),
            PublishMode::ReplaceOrCreate,
        )
        .expect("create");
        assert!(!created.destination_existed);
        assert_eq!(fs::read(root.join("created")).expect("read"), b"created");
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn no_replace_succeeds_and_never_changes_existing_destination() {
        let (root, parent) = fixture("no-replace");
        publish_regular(
            &parent,
            source(&parent, "source", b"first"),
            OsStr::new("source"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
        )
        .expect("publish");
        let error = publish_regular(
            &parent,
            source(&parent, "source-2", b"second"),
            OsStr::new("source-2"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
        )
        .expect_err("must reject");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(root.join("destination")).expect("read"), b"first");
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn lifecycle_closes_validation_handles_before_publish_and_final_validation_after_commit() {
        let (root, parent) = fixture("lifecycle");
        fs::write(root.join("destination"), b"previous").expect("existing destination");
        let _ = take_lifecycle_events();

        let outcome = publish_regular(
            &parent,
            source(&parent, "source", b"lifecycle"),
            OsStr::new("source"),
            OsStr::new("destination"),
            PublishMode::ReplaceOrCreate,
        )
        .expect("publish");

        assert!(outcome.committed);
        assert!(outcome.destination_existed);
        assert_eq!(
            fs::read(root.join("destination")).expect("destination"),
            b"lifecycle"
        );
        assert_eq!(
            take_lifecycle_events(),
            vec![
                LifecycleEvent::SourceWriterFlushed,
                LifecycleEvent::SourceIdentityCaptured,
                LifecycleEvent::SourceWriterDropped,
                LifecycleEvent::SourceValidationStarted,
                LifecycleEvent::SourceValidationOpened,
                LifecycleEvent::SourceValidationClosed,
                LifecycleEvent::DestinationValidationOpened,
                LifecycleEvent::DestinationValidationClosed,
                LifecycleEvent::OsPublishStarted,
                LifecycleEvent::FinalValidationStarted,
            ]
        );
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn injected_source_validation_error_32_follows_writer_close_and_preserves_state() {
        let (root, parent) = fixture("source-error-32");
        let _ = take_lifecycle_events();

        let error = publish_regular_injected_source_validation_error32(
            &parent,
            source(&parent, "source", b"source-error"),
            OsStr::new("source"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
        )
        .expect_err("injected source validation error");

        let details = error.details();
        assert_eq!(details.phase, PublishPhase::ValidateSource);
        assert_eq!(details.raw_os_error, Some(32));
        assert!(details.source_exists);
        assert!(!details.destination_exists);
        assert!(!details.committed);
        assert!(!details.final_validated);
        assert!(!details.temporary_cleanup_confirmed);
        assert_eq!(
            take_lifecycle_events(),
            vec![
                LifecycleEvent::SourceWriterFlushed,
                LifecycleEvent::SourceIdentityCaptured,
                LifecycleEvent::SourceWriterDropped,
                LifecycleEvent::SourceValidationStarted,
            ]
        );
        assert!(!error.to_string().contains(&root.display().to_string()));
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(windows)]
    #[test]
    fn legacy_incompatible_validation_open_fails_while_live_writer_corrected_publish_succeeds() {
        use windows_sys::Win32::Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        };

        let (root, parent) = fixture("windows-sharing");
        let writer = source(&parent, "source", b"windows");
        let source_path = crate::windows_path::verbatim_wide_path(&root.join("source"))
            .expect("verbatim source path");
        // SAFETY: source_path is NUL-terminated and the returned handle is closed below.
        let legacy = unsafe {
            CreateFileW(
                source_path.as_ptr(),
                GENERIC_READ | FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(legacy, INVALID_HANDLE_VALUE);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(32));

        publish_regular(
            &parent,
            writer,
            OsStr::new("source"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
        )
        .expect("corrected publish after writer closes");
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn direct_children_non_ascii_long_paths_and_distinct_names_are_enforced() {
        let (root, parent) = fixture("names");
        publish_regular(
            &parent,
            source(&parent, "источник", b"utf8"),
            OsStr::new("источник"),
            OsStr::new("результат"),
            PublishMode::NoReplace,
        )
        .expect("unicode");
        let nested = publish_regular(
            &parent,
            source(&parent, "source", b"x"),
            OsStr::new("source"),
            OsStr::new("../escape"),
            PublishMode::NoReplace,
        )
        .expect_err("nested destination");
        assert_eq!(nested.details.phase, PublishPhase::ValidateDestination);

        let long_source = "s".repeat(180);
        let long_destination = "d".repeat(180);
        publish_regular(
            &parent,
            source(&parent, &long_source, b"long"),
            OsStr::new(&long_source),
            OsStr::new(&long_destination),
            PublishMode::NoReplace,
        )
        .expect("long normal names");
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn source_swap_and_link_destinations_fail_closed() {
        let (root, parent) = fixture("unsafe");
        let opened = source(&parent, "source", b"opened");
        fs::rename(root.join("source"), root.join("moved")).expect("move");
        fs::write(root.join("source"), b"replacement").expect("replacement");
        let swap = publish_regular(
            &parent,
            opened,
            OsStr::new("source"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
        )
        .expect_err("swap");
        assert_eq!(swap.details.phase, PublishPhase::ValidateSource);

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("moved", root.join("link")).expect("symlink");
            let link = publish_regular(
                &parent,
                source(&parent, "source-2", b"x"),
                OsStr::new("source-2"),
                OsStr::new("link"),
                PublishMode::ReplaceOrCreate,
            )
            .expect_err("destination link");
            assert_eq!(link.details.phase, PublishPhase::ValidateDestination);

            let source_link = source(&parent, "source-link", b"opened");
            fs::rename(root.join("source-link"), root.join("source-link-moved"))
                .expect("source move");
            std::os::unix::fs::symlink("source-link-moved", root.join("source-link"))
                .expect("source symlink");
            let source_error = publish_regular(
                &parent,
                source_link,
                OsStr::new("source-link"),
                OsStr::new("source-link-final"),
                PublishMode::NoReplace,
            )
            .expect_err("source link");
            assert_eq!(source_error.details.phase, PublishPhase::ValidateSource);
        }
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn parent_identity_swap_fails_before_publication() {
        use std::os::unix::fs::symlink;

        let (root, parent) = fixture("parent-swap");
        let opened = source(&parent, "source", b"trusted");
        let moved = root.with_extension("moved");
        let outside = root.with_extension("outside");
        fs::create_dir(&outside).expect("outside");
        fs::rename(&root, &moved).expect("move parent");
        symlink(&outside, &root).expect("replacement parent");

        let error = publish_regular(
            &parent,
            opened,
            OsStr::new("source"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
        )
        .expect_err("parent swap");
        assert_eq!(error.details.phase, PublishPhase::ValidateParent);
        assert!(!outside.join("destination").exists());

        drop(parent);
        fs::remove_file(root).expect("symlink cleanup");
        fs::remove_dir_all(moved).expect("moved cleanup");
        fs::remove_dir_all(outside).expect("outside cleanup");
    }

    #[test]
    fn fault_details_are_private_and_commit_state_is_exact() {
        let (root, parent) = fixture("faults");
        let error = publish_regular_inner(
            &parent,
            source(&parent, "source", b"x"),
            OsStr::new("source"),
            OsStr::new("destination"),
            PublishMode::NoReplace,
            InjectedFault::OsError87,
        )
        .expect_err("error 87");
        assert_eq!(error.details.raw_os_error, Some(87));
        assert_eq!(error.details.phase, PublishPhase::OsPublish);
        assert_eq!(error.details.mode, PublishMode::NoReplace);
        assert_eq!(
            error.details.strategy,
            strategy(PublishMode::NoReplace, false)
        );
        assert_eq!(error.details.code, "PLATFORM_PUBLISH_FAILED");
        assert_eq!(error.details.operation, "publish_regular");
        assert_eq!(error.details.source, "source_child");
        assert_eq!(error.details.destination, "destination_child");
        assert_eq!(error.details.source_size, Some(1));
        assert!(error.details.source_exists);
        assert!(!error.details.destination_exists);
        assert!(!error.details.final_validated);
        assert!(!error.details.committed);
        assert!(!error.details.durability_confirmed);
        assert!(!error.details.temporary_cleanup_confirmed);
        assert_eq!(
            error.details.io_kind,
            io_kind(io::Error::from_raw_os_error(87).kind())
        );
        assert!(!error.to_string().contains(&root.display().to_string()));
        assert_eq!(error.into_io_error().raw_os_error(), Some(87));

        let committed = publish_regular_inner(
            &parent,
            source(&parent, "source-2", b"y"),
            OsStr::new("source-2"),
            OsStr::new("destination-2"),
            PublishMode::NoReplace,
            InjectedFault::ReopenDestination,
        )
        .expect_err("reopen");
        assert_eq!(committed.details.phase, PublishPhase::ReopenDestination);
        assert!(committed.details.committed);
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn flush_and_identity_faults_preserve_typed_phase() {
        let (root, parent) = fixture("phase-faults");
        let flush = publish_regular_inner(
            &parent,
            source(&parent, "flush", b"x"),
            OsStr::new("flush"),
            OsStr::new("flush-final"),
            PublishMode::NoReplace,
            InjectedFault::FlushSource,
        )
        .expect_err("flush");
        assert_eq!(flush.details.phase, PublishPhase::FlushSource);
        assert!(!flush.details.committed);

        let identity = publish_regular_inner(
            &parent,
            source(&parent, "identity", b"x"),
            OsStr::new("identity"),
            OsStr::new("identity-final"),
            PublishMode::NoReplace,
            InjectedFault::ValidatePublishedIdentity,
        )
        .expect_err("identity");
        assert_eq!(
            identity.details.phase,
            PublishPhase::ValidatePublishedIdentity
        );
        assert!(identity.details.committed);

        let sync = publish_regular_inner(
            &parent,
            source(&parent, "sync", b"x"),
            OsStr::new("sync"),
            OsStr::new("sync-final"),
            PublishMode::NoReplace,
            InjectedFault::SyncParent,
        )
        .expect_err("sync");
        assert_eq!(sync.details.phase, PublishPhase::SyncParent);
        assert!(sync.details.committed);
        assert!(sync.details.final_validated);
        assert!(!sync.details.durability_confirmed);
        assert!(sync.details.temporary_cleanup_confirmed);
        drop(parent);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
