//! Durable, resumable boundary around the RC5 database upgrade.
//!
//! The journal deliberately records state outside `AOPMEM_HOME`: a failed
//! home must never take its only recovery evidence with it.  It is a small
//! state machine, not a second migration engine.  The existing transactional
//! migration remains the only code that changes workspace databases.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::audit::AnchoredDir;
use crate::observability;
use crate::platform_publish::{
    publish_regular, PublishError, PublishMode, PublishOutcome, PublishStrategy,
};
use crate::storage;

use super::{
    apply_core_all_workspaces, plan_all_workspaces, UpgradeApplyExecution, UpgradePlanReport,
};

const JOURNAL_PREFIX: &str = "aopmem-upgrade-recovery-v0.2.0-rc5-";
const STAGED_BINARY_NAME: &str = ".aopmem-v0.2.0-rc5.staged";
const BACKUP_PREFIX: &str = "aopmem-home-backup-v0.2.0-rc5-";
const MAX_BACKUP_ENTRIES: usize = 100_000;
const MAX_BACKUP_DIRECTORY_ENTRIES: usize = 10_000;
const MAX_BACKUP_DEPTH: usize = 128;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_JOURNAL_BYTES: usize = 1024 * 1024;
const HOME_MANIFEST_TEMP_PREFIX: &str = ".aopmem-home-manifest-";
const MAX_RECOVERY_PARENT_SCAN_ENTRIES: usize = 100_000;
const MAX_RECOVERY_TEMP_REMOVALS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryFaultPoint {
    BackupEffect,
    StageEffect,
    PrepareEffect,
    ApplyStarted,
    CoreEffect,
    PublishEffect,
}

trait RecoveryHooks {
    fn checkpoint(&mut self, point: RecoveryFaultPoint) -> Result<(), RecoveryError>;

    fn apply_core(
        &mut self,
        planned: &[PlannedWorkspaceIdentity],
    ) -> Result<UpgradeApplyExecution, RecoveryError>;

    fn publish_binary(
        &mut self,
        staged_name: &str,
        expected_sha256: &str,
        bin: &Path,
    ) -> Result<bool, RecoveryError>;
}

struct LiveRecoveryHooks;

impl RecoveryHooks for LiveRecoveryHooks {
    fn checkpoint(&mut self, _point: RecoveryFaultPoint) -> Result<(), RecoveryError> {
        Ok(())
    }

    fn apply_core(
        &mut self,
        planned: &[PlannedWorkspaceIdentity],
    ) -> Result<UpgradeApplyExecution, RecoveryError> {
        apply_core_all_workspaces(planned).map_err(|error| RecoveryError::Apply(error.to_string()))
    }

    fn publish_binary(
        &mut self,
        staged_name: &str,
        expected_sha256: &str,
        bin: &Path,
    ) -> Result<bool, RecoveryError> {
        publish_staged_binary(staged_name, expected_sha256, bin)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPhase {
    BackupComplete,
    StagedVerified,
    Prepared,
    ApplyStarted,
    Applied,
    Published,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryJournal {
    pub version: String,
    pub phase: RecoveryPhase,
    pub home_identity: String,
    pub home_backup_dir: String,
    pub backup_manifest_sha256: String,
    pub staged_binary_name: String,
    pub staged_sha256: String,
    pub planned_workspaces: Vec<PlannedWorkspaceIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedWorkspaceIdentity {
    pub workspace_key: String,
    pub root_identity: String,
    pub database_identity: String,
    pub observability_identity: String,
    pub schema_before: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryExecution {
    pub journal_phase: RecoveryPhase,
    pub resumed: bool,
    pub apply_invoked: bool,
    pub binary_published: bool,
    pub durability_warning: bool,
    pub home_backup_retained: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply: Option<super::UpgradeApplyReport>,
}

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("cannot resolve AOPMem home: {0}")]
    Paths(#[from] storage::PathResolveError),
    #[error("upgrade recovery needs a verified staged binary for a new run")]
    MissingStagedBinary,
    #[error("staged binary is not a non-empty regular file")]
    InvalidStagedBinary,
    #[error("recovery journal is invalid")]
    InvalidJournal,
    #[error("recovery journal cannot resume an unfinished apply; preserve evidence and start a new verified run")]
    ApplyOutcomeUnknown,
    #[error("workspace set or identity changed after the frozen upgrade plan")]
    WorkspaceDrift,
    #[error("Observability v2 could not be proven for every planned workspace")]
    ObservabilityV2Required,
    #[error("upgrade recovery phase does not allow this command")]
    InvalidPhase,
    #[error("{0}")]
    Publish(Box<RecoveryPublishFailure>),
    #[error("recovery storage error: {0}")]
    Io(#[from] io::Error),
    #[error("upgrade apply failed: {0}")]
    Apply(String),
}

#[derive(Debug, Error)]
#[error(
    "upgrade recovery publish failed (operation={operation}, phase={phase}, strategy={strategy}, kind={io_kind}, os={raw_os_error:?}, committed={committed}, durability={durability_confirmed})"
)]
pub struct RecoveryPublishFailure {
    pub operation: &'static str,
    pub source_role: &'static str,
    pub destination_role: &'static str,
    pub mode: String,
    pub phase: String,
    pub strategy: String,
    pub io_kind: &'static str,
    pub raw_os_error: Option<i32>,
    pub final_validated: bool,
    pub committed: bool,
    pub durability_confirmed: bool,
    pub temporary_cleanup_confirmed: bool,
}

/// Creates the durable full-home rollback boundary before a download is
/// required. Re-entry validates and returns the existing journal.
pub fn backup_home() -> Result<RecoveryExecution, RecoveryError> {
    backup_home_with_hooks(&mut LiveRecoveryHooks)
}

/// Adopts an installer-created sibling full-home backup after the RC5 binary
/// has been downloaded. The backup must exactly match the still-unchanged
/// current home and the caller-provided manifest digest.
pub fn adopt_home_backup(
    backup: &Path,
    expected_manifest_sha256: &str,
) -> Result<RecoveryExecution, RecoveryError> {
    let paths = storage::resolve_paths()?;
    let parent = paths.home().parent().ok_or(RecoveryError::InvalidJournal)?;
    cleanup_recovery_temporaries(parent)?;
    if let Some(journal) = read_journal(parent)? {
        validate_journal(&journal, paths.home(), parent)?;
        if journal.home_backup_dir
            != backup
                .file_name()
                .ok_or(RecoveryError::InvalidJournal)?
                .to_string_lossy()
            || journal.backup_manifest_sha256 != expected_manifest_sha256
        {
            return Err(RecoveryError::InvalidJournal);
        }
        return Ok(execution(&journal, parent, true, false, false, false, None));
    }
    let (backup_name, manifest_sha256) =
        validate_adoptable_backup(paths.home(), parent, backup, expected_manifest_sha256)?;
    let journal = RecoveryJournal {
        version: "v0.2.0-rc5".to_string(),
        phase: RecoveryPhase::BackupComplete,
        home_identity: home_identity(paths.home())?,
        home_backup_dir: backup_name,
        backup_manifest_sha256: manifest_sha256,
        staged_binary_name: String::new(),
        staged_sha256: String::new(),
        planned_workspaces: Vec::new(),
    };
    write_journal(parent, paths.home(), &journal)?;
    Ok(execution(
        &journal, parent, false, false, false, false, None,
    ))
}

fn backup_home_with_hooks(
    hooks: &mut impl RecoveryHooks,
) -> Result<RecoveryExecution, RecoveryError> {
    let paths = storage::resolve_paths()?;
    let parent = paths.home().parent().ok_or(RecoveryError::InvalidJournal)?;
    cleanup_recovery_temporaries(parent)?;
    let (journal, resumed) = match read_journal(parent)? {
        Some(journal) => {
            validate_journal(&journal, paths.home(), parent)?;
            (journal, true)
        }
        None => {
            let backup = create_full_home_backup(paths.home(), parent)?;
            hooks.checkpoint(RecoveryFaultPoint::BackupEffect)?;
            let manifest_sha256 = sha256_regular_nofollow(&backup.join("MANIFEST.sha256"))?;
            let journal = RecoveryJournal {
                version: "v0.2.0-rc5".to_string(),
                phase: RecoveryPhase::BackupComplete,
                home_identity: home_identity(paths.home())?,
                home_backup_dir: backup
                    .file_name()
                    .ok_or(RecoveryError::InvalidJournal)?
                    .to_string_lossy()
                    .into_owned(),
                backup_manifest_sha256: manifest_sha256,
                staged_binary_name: String::new(),
                staged_sha256: String::new(),
                planned_workspaces: Vec::new(),
            };
            write_journal(parent, paths.home(), &journal)?;
            (journal, false)
        }
    };
    Ok(execution(
        &journal, parent, resumed, false, false, false, None,
    ))
}

/// Retains a verified artifact after the pre-download backup boundary.
pub fn stage_binary(staged: &Path, expected: &str) -> Result<RecoveryExecution, RecoveryError> {
    stage_binary_with_hooks(staged, expected, &mut LiveRecoveryHooks)
}

fn stage_binary_with_hooks(
    staged: &Path,
    expected: &str,
    hooks: &mut impl RecoveryHooks,
) -> Result<RecoveryExecution, RecoveryError> {
    let (paths, parent, mut journal) = load_journal()?;
    if journal.phase == RecoveryPhase::BackupComplete {
        if !is_sha256(expected) || sha256_regular_nofollow(staged)? != expected {
            return Err(RecoveryError::InvalidStagedBinary);
        }
        let retained = retain_staged_binary(staged, paths.bin(), expected)?;
        hooks.checkpoint(RecoveryFaultPoint::StageEffect)?;
        journal.staged_binary_name = retained
            .file_name()
            .ok_or(RecoveryError::InvalidJournal)?
            .to_string_lossy()
            .into_owned();
        journal.staged_sha256 = expected.to_string();
        journal.phase = RecoveryPhase::StagedVerified;
        write_journal(&parent, paths.home(), &journal)?;
    } else {
        if journal.staged_sha256 != expected {
            return Err(RecoveryError::InvalidStagedBinary);
        }
        verify_retained_staged(&journal, paths.bin())?;
    }
    Ok(execution(
        &journal, &parent, true, false, false, false, None,
    ))
}

/// Applies or reconciles only the frozen core database set. It never creates a
/// backup, accepts a download path, or publishes the installed binary.
pub fn apply_or_resume() -> Result<RecoveryExecution, RecoveryError> {
    apply_or_resume_with_hooks(&mut LiveRecoveryHooks)
}

fn apply_or_resume_with_hooks(
    hooks: &mut impl RecoveryHooks,
) -> Result<RecoveryExecution, RecoveryError> {
    let (paths, parent, mut journal) = load_journal()?;
    if journal.phase == RecoveryPhase::StagedVerified {
        let plan =
            plan_all_workspaces().map_err(|error| RecoveryError::Apply(error.to_string()))?;
        if !plan.ready {
            return Err(RecoveryError::Apply(
                "upgrade plan is not ready".to_string(),
            ));
        }
        ensure_observability_v2(&paths, &plan)?;
        journal.planned_workspaces = capture_planned_workspaces(&paths, &plan)?;
        hooks.checkpoint(RecoveryFaultPoint::PrepareEffect)?;
        journal.phase = RecoveryPhase::Prepared;
        write_journal(&parent, paths.home(), &journal)?;
    }

    if matches!(
        journal.phase,
        RecoveryPhase::Prepared | RecoveryPhase::ApplyStarted | RecoveryPhase::Applied
    ) {
        verify_retained_staged(&journal, paths.bin())?;
    } else if !matches!(journal.phase, RecoveryPhase::StagedVerified) {
        return Err(RecoveryError::InvalidPhase);
    }
    if journal.phase == RecoveryPhase::ApplyStarted {
        if all_planned_workspaces_applied(&paths, &journal.planned_workspaces)? {
            journal.phase = RecoveryPhase::Applied;
            write_journal(&parent, paths.home(), &journal)?;
        } else {
            // A transition was durable but the database result was not. Never
            // replay core apply automatically: preserve all evidence instead.
            return Err(RecoveryError::ApplyOutcomeUnknown);
        }
    }

    let mut apply = None;
    let mut apply_invoked = false;
    if journal.phase == RecoveryPhase::Prepared {
        validate_frozen_plan(&paths, &journal.planned_workspaces, false)?;
        journal.phase = RecoveryPhase::ApplyStarted;
        write_journal(&parent, paths.home(), &journal)?;
        hooks.checkpoint(RecoveryFaultPoint::ApplyStarted)?;
        apply_invoked = true;
        let execution = hooks.apply_core(&journal.planned_workspaces)?;
        if let Some(failure) = execution.failure {
            return Err(RecoveryError::Apply(format!(
                "{}: {}",
                failure.code, failure.message
            )));
        }
        apply = Some(execution.report);
        hooks.checkpoint(RecoveryFaultPoint::CoreEffect)?;
        journal.phase = RecoveryPhase::Applied;
        write_journal(&parent, paths.home(), &journal)?;
    }
    Ok(execution(
        &journal,
        &parent,
        true,
        apply_invoked,
        false,
        false,
        apply,
    ))
}

/// Publishes only after an `applied` journal. Re-entry after a crash between
/// binary commit and journal advance is idempotent.
pub fn publish_applied() -> Result<RecoveryExecution, RecoveryError> {
    publish_applied_with_hooks(&mut LiveRecoveryHooks)
}

fn publish_applied_with_hooks(
    hooks: &mut impl RecoveryHooks,
) -> Result<RecoveryExecution, RecoveryError> {
    let (paths, parent, mut journal) = load_journal()?;
    let mut durability_warning = false;
    let binary_published = if journal.phase == RecoveryPhase::Applied {
        verify_retained_staged(&journal, paths.bin())?;
        durability_warning = hooks.publish_binary(
            &journal.staged_binary_name,
            &journal.staged_sha256,
            paths.bin(),
        )?;
        hooks.checkpoint(RecoveryFaultPoint::PublishEffect)?;
        journal.phase = RecoveryPhase::Published;
        write_journal(&parent, paths.home(), &journal)?;
        true
    } else if journal.phase == RecoveryPhase::Published {
        verify_retained_staged(&journal, paths.bin())?;
        if verify_installed_binary(&journal.staged_sha256, paths.bin()).is_ok() {
            false
        } else {
            durability_warning = hooks.publish_binary(
                &journal.staged_binary_name,
                &journal.staged_sha256,
                paths.bin(),
            )?;
            true
        }
    } else {
        return Err(RecoveryError::InvalidPhase);
    };
    Ok(execution(
        &journal,
        &parent,
        true,
        false,
        binary_published,
        durability_warning,
        None,
    ))
}

fn load_journal() -> Result<(storage::AopmemPaths, PathBuf, RecoveryJournal), RecoveryError> {
    let paths = storage::resolve_paths()?;
    let parent = paths
        .home()
        .parent()
        .ok_or(RecoveryError::InvalidJournal)?
        .to_path_buf();
    cleanup_recovery_temporaries(&parent)?;
    let journal = read_journal(&parent)?.ok_or(RecoveryError::InvalidPhase)?;
    validate_journal(&journal, paths.home(), &parent)?;
    Ok((paths, parent, journal))
}

fn execution(
    journal: &RecoveryJournal,
    parent: &Path,
    resumed: bool,
    apply_invoked: bool,
    binary_published: bool,
    durability_warning: bool,
    apply: Option<super::UpgradeApplyReport>,
) -> RecoveryExecution {
    RecoveryExecution {
        journal_phase: journal.phase,
        resumed,
        apply_invoked,
        binary_published,
        durability_warning,
        home_backup_retained: parent.join(&journal.home_backup_dir).is_dir(),
        apply,
    }
}

fn require_recovery_publish(
    operation: &'static str,
    result: Result<PublishOutcome, PublishError>,
) -> Result<(), RecoveryError> {
    match result {
        Ok(outcome)
            if outcome.committed
                && outcome.final_validated
                && outcome.durability_confirmed
                && outcome.temporary_cleanup_confirmed =>
        {
            Ok(())
        }
        Ok(outcome) => Err(RecoveryError::Publish(Box::new(RecoveryPublishFailure {
            operation,
            source_role: publish_roles(operation).0,
            destination_role: publish_roles(operation).1,
            mode: format!("{:?}", publish_roles(operation).2),
            phase: "post_publish_validation".to_string(),
            strategy: format!("{:?}", outcome.strategy),
            io_kind: "other",
            raw_os_error: None,
            final_validated: outcome.final_validated,
            committed: outcome.committed,
            durability_confirmed: outcome.durability_confirmed,
            temporary_cleanup_confirmed: outcome.temporary_cleanup_confirmed,
        }))),
        Err(error) => {
            let details = error.details();
            Err(RecoveryError::Publish(Box::new(RecoveryPublishFailure {
                operation,
                source_role: details.source,
                destination_role: details.destination,
                mode: format!("{:?}", details.mode),
                phase: format!("{:?}", details.phase),
                strategy: format!("{:?}", details.strategy),
                io_kind: details.io_kind,
                raw_os_error: details.raw_os_error,
                final_validated: details.final_validated,
                committed: details.committed,
                durability_confirmed: details.durability_confirmed,
                temporary_cleanup_confirmed: details.temporary_cleanup_confirmed,
            })))
        }
    }
}

fn publish_roles(operation: &str) -> (&'static str, &'static str, PublishMode) {
    match operation {
        "journal_transition" => (
            "recovery_checkpoint_temporary",
            "recovery_checkpoint",
            PublishMode::NoReplace,
        ),
        "retain_staged_binary" => (
            "retained_binary_temporary",
            "retained_binary",
            PublishMode::NoReplace,
        ),
        "publish_installed_binary" => (
            "installed_binary_temporary",
            "installed_binary",
            PublishMode::ReplaceOrCreate,
        ),
        "backup_manifest" => (
            "backup_manifest_temporary",
            "backup_manifest",
            PublishMode::NoReplace,
        ),
        "backup_file" => (
            "backup_file_temporary",
            "backup_file",
            PublishMode::NoReplace,
        ),
        _ => ("temporary_file", "published_file", PublishMode::NoReplace),
    }
}

fn cleanup_recovery_temporaries(parent: &Path) -> Result<(), RecoveryError> {
    cleanup_recovery_temporaries_with_limits(
        parent,
        MAX_RECOVERY_PARENT_SCAN_ENTRIES,
        MAX_RECOVERY_TEMP_REMOVALS,
    )
}

fn cleanup_recovery_temporaries_with_limits(
    parent: &Path,
    scan_limit: usize,
    removal_limit: usize,
) -> Result<(), RecoveryError> {
    let root = AnchoredDir::open_workspace(parent, None)?;
    root.verify_logical_identity()?;
    let mut scanned = 0_usize;
    let mut candidates = Vec::new();
    for entry in fs::read_dir(root.logical_path())? {
        let name = entry?.file_name();
        scanned = scanned
            .checked_add(1)
            .ok_or(RecoveryError::InvalidJournal)?;
        if scanned > scan_limit {
            return Err(RecoveryError::InvalidJournal);
        }
        if is_recovery_temporary_name(&name) {
            candidates.push(name);
            if candidates.len() > removal_limit {
                return Err(RecoveryError::InvalidJournal);
            }
        }
    }
    candidates.sort();
    root.verify_logical_identity()?;
    for name in &candidates {
        drop(root.open_regular_os(name)?);
    }
    for name in &candidates {
        root.remove_regular_os(name)?;
    }
    if !candidates.is_empty() {
        root.sync()?;
    }
    Ok(())
}

fn is_recovery_temporary_name(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let journal_prefix = format!(".{JOURNAL_PREFIX}");
    uuid_temporary_name(name, &journal_prefix)
        || uuid_temporary_name(name, HOME_MANIFEST_TEMP_PREFIX)
}

fn uuid_temporary_name(name: &str, prefix: &str) -> bool {
    let Some(value) = name
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(".tmp"))
    else {
        return false;
    };
    uuid::Uuid::parse_str(value).is_ok_and(|parsed| {
        parsed.get_version_num() == 4 && parsed.hyphenated().to_string() == value
    })
}

fn read_journal(parent: &Path) -> Result<Option<RecoveryJournal>, RecoveryError> {
    let root = AnchoredDir::open_workspace(parent, None)?;
    let phases = [
        RecoveryPhase::BackupComplete,
        RecoveryPhase::StagedVerified,
        RecoveryPhase::Prepared,
        RecoveryPhase::ApplyStarted,
        RecoveryPhase::Applied,
        RecoveryPhase::Published,
    ];
    let mut latest: Option<RecoveryJournal> = None;
    let mut missing_predecessor = false;
    for phase in phases {
        let name = journal_name(phase);
        let Some(file) = root.open_regular_optional_os(OsStr::new(&name))? else {
            missing_predecessor = true;
            continue;
        };
        if missing_predecessor {
            return Err(RecoveryError::InvalidJournal);
        }
        let mut bytes = Vec::new();
        file.take((MAX_JOURNAL_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(RecoveryError::Io)?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(RecoveryError::InvalidJournal);
        }
        let current: RecoveryJournal =
            serde_json::from_slice(&bytes).map_err(|_| RecoveryError::InvalidJournal)?;
        if current.phase != phase
            || latest
                .as_ref()
                .is_some_and(|previous| !valid_journal_transition(previous, &current))
        {
            return Err(RecoveryError::InvalidJournal);
        }
        latest = Some(current);
    }
    Ok(latest)
}

fn write_journal(
    parent: &Path,
    home: &Path,
    journal: &RecoveryJournal,
) -> Result<(), RecoveryError> {
    validate_journal(journal, home, parent)?;
    let root = AnchoredDir::open_workspace(parent, None)?;
    let final_name = journal_name(journal.phase);
    let bytes = serialize_journal(journal)?;
    let latest = read_journal(parent)?;
    if let Some(existing) = root.open_regular_optional_os(OsStr::new(&final_name))? {
        let mut existing_bytes = Vec::new();
        existing
            .take((MAX_JOURNAL_BYTES + 1) as u64)
            .read_to_end(&mut existing_bytes)?;
        return if existing_bytes.len() <= MAX_JOURNAL_BYTES
            && existing_bytes == bytes
            && latest.as_ref() == Some(journal)
        {
            Ok(())
        } else {
            Err(RecoveryError::InvalidJournal)
        };
    }
    match (previous_phase(journal.phase), latest.as_ref()) {
        (None, None) => {}
        (Some(previous_phase), Some(previous))
            if previous.phase == previous_phase && valid_journal_transition(previous, journal) => {}
        _ => return Err(RecoveryError::InvalidJournal),
    }
    let temporary = format!(".{JOURNAL_PREFIX}{}.tmp", uuid::Uuid::new_v4().hyphenated());
    let mut file = root.create_new_regular(&temporary)?;
    file.write_all(&bytes)?;
    require_recovery_publish(
        "journal_transition",
        publish_regular(
            &root,
            file,
            OsStr::new(&temporary),
            OsStr::new(&final_name),
            PublishMode::NoReplace,
        ),
    )
}

fn serialize_journal(journal: &RecoveryJournal) -> Result<Vec<u8>, RecoveryError> {
    let bytes = serde_json::to_vec(journal).map_err(|_| RecoveryError::InvalidJournal)?;
    if bytes.len() > MAX_JOURNAL_BYTES {
        return Err(RecoveryError::InvalidJournal);
    }
    Ok(bytes)
}

fn journal_name(phase: RecoveryPhase) -> String {
    let (rank, name) = match phase {
        RecoveryPhase::BackupComplete => (1, "backup-complete"),
        RecoveryPhase::StagedVerified => (2, "staged-verified"),
        RecoveryPhase::Prepared => (3, "prepared"),
        RecoveryPhase::ApplyStarted => (4, "apply-started"),
        RecoveryPhase::Applied => (5, "applied"),
        RecoveryPhase::Published => (6, "published"),
    };
    format!("{JOURNAL_PREFIX}{rank:02}-{name}.json")
}

const fn previous_phase(phase: RecoveryPhase) -> Option<RecoveryPhase> {
    match phase {
        RecoveryPhase::BackupComplete => None,
        RecoveryPhase::StagedVerified => Some(RecoveryPhase::BackupComplete),
        RecoveryPhase::Prepared => Some(RecoveryPhase::StagedVerified),
        RecoveryPhase::ApplyStarted => Some(RecoveryPhase::Prepared),
        RecoveryPhase::Applied => Some(RecoveryPhase::ApplyStarted),
        RecoveryPhase::Published => Some(RecoveryPhase::Applied),
    }
}

fn valid_journal_transition(previous: &RecoveryJournal, current: &RecoveryJournal) -> bool {
    previous_phase(current.phase) == Some(previous.phase)
        && previous.version == current.version
        && previous.home_identity == current.home_identity
        && previous.home_backup_dir == current.home_backup_dir
        && previous.backup_manifest_sha256 == current.backup_manifest_sha256
        && (previous.phase == RecoveryPhase::BackupComplete
            || (previous.staged_binary_name == current.staged_binary_name
                && previous.staged_sha256 == current.staged_sha256))
        && (matches!(
            previous.phase,
            RecoveryPhase::BackupComplete | RecoveryPhase::StagedVerified
        ) || previous.planned_workspaces == current.planned_workspaces)
}

fn retain_staged_binary(
    source: &Path,
    bin: &Path,
    expected_sha256: &str,
) -> Result<PathBuf, RecoveryError> {
    let source_parent = source.parent().ok_or(RecoveryError::InvalidStagedBinary)?;
    let source_name = source
        .file_name()
        .ok_or(RecoveryError::InvalidStagedBinary)?;
    let source_root = AnchoredDir::open_workspace(source_parent, None)?;
    let mut input = source_root.open_regular_os(source_name)?;
    let metadata = input.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(RecoveryError::InvalidStagedBinary);
    }
    fs::create_dir_all(bin)?;
    let root = AnchoredDir::open_workspace(bin, None)?;
    let temporary = ".aopmem-v0.2.0-rc5.retain.tmp";
    if root
        .open_regular_optional_os(OsStr::new(temporary))?
        .is_some()
    {
        root.remove_regular(temporary)?;
    }
    if let Some(existing) = root.open_regular_optional_os(OsStr::new(STAGED_BINARY_NAME))? {
        if sha256_reader(existing)? == expected_sha256 {
            ensure_executable(&bin.join(STAGED_BINARY_NAME))?;
            return Ok(bin.join(STAGED_BINARY_NAME));
        }
        return Err(RecoveryError::InvalidStagedBinary);
    }
    let mut destination = root.create_new_regular(temporary)?;
    io::copy(&mut input, &mut destination)?;
    set_executable_permissions(&destination)?;
    require_recovery_publish(
        "retain_staged_binary",
        publish_regular(
            &root,
            destination,
            OsStr::new(temporary),
            OsStr::new(STAGED_BINARY_NAME),
            PublishMode::NoReplace,
        ),
    )?;
    let destination = bin.join(STAGED_BINARY_NAME);
    if sha256_regular_nofollow(&destination)? != expected_sha256 {
        return Err(RecoveryError::InvalidStagedBinary);
    }
    ensure_executable(&destination)?;
    Ok(destination)
}

fn verify_retained_staged(journal: &RecoveryJournal, bin: &Path) -> Result<(), RecoveryError> {
    let staged = bin.join(&journal.staged_binary_name);
    if sha256_regular_nofollow(&staged)? != journal.staged_sha256 {
        return Err(RecoveryError::InvalidStagedBinary);
    }
    ensure_executable(&staged)?;
    Ok(())
}

fn publish_staged_binary(
    staged_name: &str,
    expected_sha256: &str,
    bin: &Path,
) -> Result<bool, RecoveryError> {
    publish_staged_binary_with(
        staged_name,
        expected_sha256,
        bin,
        |root, source, source_name, destination_name, mode| {
            publish_regular(root, source, source_name, destination_name, mode)
        },
    )
}

fn publish_staged_binary_with(
    staged_name: &str,
    expected_sha256: &str,
    bin: &Path,
    publisher: impl FnOnce(
        &AnchoredDir,
        File,
        &OsStr,
        &OsStr,
        PublishMode,
    ) -> Result<PublishOutcome, PublishError>,
) -> Result<bool, RecoveryError> {
    let root = AnchoredDir::open_workspace(bin, None)?;
    let destination = if cfg!(windows) {
        "aopmem.exe"
    } else {
        "aopmem"
    };
    let temporary = ".aopmem-v0.2.0-rc5.publish.tmp";
    if root
        .open_regular_optional_os(OsStr::new(temporary))?
        .is_some()
    {
        root.remove_regular(temporary)?;
    }
    if let Some(installed) = root.open_regular_optional_os(OsStr::new(destination))? {
        if sha256_reader(installed)? == expected_sha256 {
            ensure_executable(&bin.join(destination))?;
            return Ok(false);
        }
    }
    let mut retained = root.open_regular_os(OsStr::new(staged_name))?;
    let mut source = root.create_new_regular(temporary)?;
    io::copy(&mut retained, &mut source)?;
    set_executable_permissions(&source)?;
    let durability_warning = require_installed_binary_publish(publisher(
        &root,
        source,
        OsStr::new(temporary),
        OsStr::new(destination),
        PublishMode::ReplaceOrCreate,
    ))?;
    if sha256_regular_nofollow(&bin.join(destination))? != expected_sha256
        || sha256_regular_nofollow(&bin.join(staged_name))? != expected_sha256
    {
        return Err(RecoveryError::InvalidStagedBinary);
    }
    ensure_executable(&bin.join(destination))?;
    Ok(durability_warning)
}

fn require_installed_binary_publish(
    result: Result<PublishOutcome, PublishError>,
) -> Result<bool, RecoveryError> {
    match result {
        Ok(outcome)
            if outcome.committed
                && outcome.final_validated
                && outcome.temporary_cleanup_confirmed
                && (outcome.durability_confirmed
                    || outcome.strategy == PublishStrategy::WindowsReplaceFileW) =>
        {
            Ok(!outcome.durability_confirmed)
        }
        Err(error) => {
            let details = error.details();
            if details.committed
                && details.final_validated
                && details.temporary_cleanup_confirmed
                && !details.durability_confirmed
                && details.strategy == PublishStrategy::WindowsReplaceFileW
            {
                return Ok(true);
            }
            require_recovery_publish("publish_installed_binary", Err(error)).map(|()| false)
        }
        outcome => require_recovery_publish("publish_installed_binary", outcome).map(|()| false),
    }
}

fn verify_installed_binary(expected_sha256: &str, bin: &Path) -> Result<(), RecoveryError> {
    let destination = if cfg!(windows) {
        "aopmem.exe"
    } else {
        "aopmem"
    };
    if sha256_regular_nofollow(&bin.join(destination))? != expected_sha256 {
        return Err(RecoveryError::InvalidStagedBinary);
    }
    ensure_executable(&bin.join(destination))
}

fn create_full_home_backup(home: &Path, parent: &Path) -> Result<PathBuf, RecoveryError> {
    let backup_name = format!("{BACKUP_PREFIX}{}", uuid::Uuid::new_v4());
    let parent_root = AnchoredDir::open_workspace(parent, None)?;
    let backup_root = parent_root.create_new_child_dir_os(OsStr::new(&backup_name))?;
    let source_root = AnchoredDir::open_workspace(home, None)?;
    let temporary = ".MANIFEST.sha256.tmp";
    let mut file = backup_root.create_new_regular(temporary)?;
    let mut entries = 0_usize;
    copy_tree_anchored(
        &source_root,
        &backup_root,
        Path::new(""),
        &mut file,
        &mut entries,
        0,
    )?;
    file.sync_all()?;
    require_recovery_publish(
        "backup_manifest",
        publish_regular(
            &backup_root,
            file,
            OsStr::new(temporary),
            OsStr::new("MANIFEST.sha256"),
            PublishMode::NoReplace,
        ),
    )?;
    backup_root.sync()?;
    parent_root.sync()?;
    let backup = parent.join(backup_name);
    Ok(backup)
}

fn validate_adoptable_backup(
    home: &Path,
    parent: &Path,
    backup: &Path,
    expected_manifest_sha256: &str,
) -> Result<(String, String), RecoveryError> {
    if !is_sha256(expected_manifest_sha256) || backup.parent() != Some(parent) {
        return Err(RecoveryError::InvalidJournal);
    }
    let backup_name = backup
        .file_name()
        .ok_or(RecoveryError::InvalidJournal)?
        .to_str()
        .ok_or(RecoveryError::InvalidJournal)?;
    if !valid_direct_name(backup_name) || !backup_name.starts_with(BACKUP_PREFIX) {
        return Err(RecoveryError::InvalidJournal);
    }
    let manifest = backup.join("MANIFEST.sha256");
    let actual_manifest_sha256 = sha256_regular_nofollow(&manifest)?;
    if actual_manifest_sha256 != expected_manifest_sha256 {
        return Err(RecoveryError::InvalidJournal);
    }
    validate_backup_manifest(backup, &manifest)?;
    if current_home_manifest_sha256(home, parent)? != actual_manifest_sha256 {
        return Err(RecoveryError::InvalidJournal);
    }
    Ok((backup_name.to_string(), actual_manifest_sha256))
}

fn current_home_manifest_sha256(home: &Path, parent: &Path) -> Result<String, RecoveryError> {
    let parent_root = AnchoredDir::open_workspace(parent, None)?;
    let source_root = AnchoredDir::open_workspace(home, None)?;
    let temporary = format!(".aopmem-home-manifest-{}.tmp", uuid::Uuid::new_v4());
    let mut manifest = parent_root.create_new_regular(&temporary)?;
    let mut entries = 0_usize;
    let write_result =
        write_tree_manifest(&source_root, Path::new(""), &mut manifest, &mut entries, 0)
            .and_then(|()| manifest.sync_all().map_err(RecoveryError::Io));
    drop(manifest);
    if let Err(error) = write_result {
        let _ = parent_root.remove_regular(&temporary);
        return Err(error);
    }
    let digest = sha256_reader(parent_root.open_regular_os(OsStr::new(&temporary))?)?;
    parent_root.remove_regular(&temporary)?;
    Ok(digest)
}

fn write_tree_manifest(
    source: &AnchoredDir,
    relative: &Path,
    manifest: &mut File,
    entries: &mut usize,
    depth: usize,
) -> Result<(), RecoveryError> {
    if depth > MAX_BACKUP_DEPTH {
        return Err(RecoveryError::InvalidJournal);
    }
    let before = directory_names(source)?;
    for name in &before {
        *entries = entries
            .checked_add(1)
            .ok_or(RecoveryError::InvalidJournal)?;
        if *entries > MAX_BACKUP_ENTRIES {
            return Err(RecoveryError::InvalidJournal);
        }
        if let Ok(source_child) = source.child_dir_os(name, false) {
            write_tree_manifest(
                &source_child,
                &relative.join(name),
                manifest,
                entries,
                depth + 1,
            )?;
            continue;
        }
        let mut input = source.open_regular_os(name)?;
        let metadata_before = input.metadata()?;
        if !metadata_before.is_file() {
            return Err(RecoveryError::InvalidJournal);
        }
        let mut hasher = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 32 * 1024];
        loop {
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            bytes = bytes
                .checked_add(u64::try_from(read).map_err(|_| RecoveryError::InvalidJournal)?)
                .ok_or(RecoveryError::InvalidJournal)?;
        }
        let metadata_after = input.metadata()?;
        if metadata_before.len() != bytes
            || metadata_after.len() != metadata_before.len()
            || metadata_after.modified().ok() != metadata_before.modified().ok()
            || !source.regular_child_matches_open_file(name, &input)?
        {
            return Err(RecoveryError::InvalidJournal);
        }
        let relative_file = relative.join(name);
        let relative_text = relative_file
            .to_str()
            .ok_or(RecoveryError::InvalidJournal)?;
        if relative_text.contains(['\n', '\r']) {
            return Err(RecoveryError::InvalidJournal);
        }
        writeln!(
            manifest,
            "{bytes} {} {:x}",
            mode_bits(&metadata_before),
            hasher.finalize()
        )?;
        writeln!(manifest, "{relative_text}")?;
    }
    if directory_names(source)? != before {
        return Err(RecoveryError::InvalidJournal);
    }
    Ok(())
}

fn copy_tree_anchored(
    source: &AnchoredDir,
    destination: &AnchoredDir,
    relative: &Path,
    manifest: &mut File,
    entries: &mut usize,
    depth: usize,
) -> Result<(), RecoveryError> {
    if depth > MAX_BACKUP_DEPTH {
        return Err(RecoveryError::InvalidJournal);
    }
    let before = directory_names(source)?;
    for name in &before {
        *entries = entries
            .checked_add(1)
            .ok_or(RecoveryError::InvalidJournal)?;
        if *entries > MAX_BACKUP_ENTRIES {
            return Err(RecoveryError::InvalidJournal);
        }
        if let Ok(source_child) = source.child_dir_os(name, false) {
            let destination_child = destination.create_new_child_dir_os(name)?;
            copy_tree_anchored(
                &source_child,
                &destination_child,
                &relative.join(name),
                manifest,
                entries,
                depth + 1,
            )?;
            destination_child.sync()?;
            continue;
        }

        let mut input = source.open_regular_os(name)?;
        let metadata_before = input.metadata()?;
        if !metadata_before.is_file() {
            return Err(RecoveryError::InvalidJournal);
        }
        let temporary = format!(".backup-{}.tmp", uuid::Uuid::new_v4());
        let mut output = destination.create_new_regular(&temporary)?;
        let mut hasher = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 32 * 1024];
        loop {
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
            hasher.update(&buffer[..read]);
            bytes = bytes
                .checked_add(u64::try_from(read).map_err(|_| RecoveryError::InvalidJournal)?)
                .ok_or(RecoveryError::InvalidJournal)?;
        }
        output.set_permissions(metadata_before.permissions())?;
        let mode = mode_bits(&metadata_before);
        require_recovery_publish(
            "backup_file",
            publish_regular(
                destination,
                output,
                OsStr::new(&temporary),
                name,
                PublishMode::NoReplace,
            ),
        )?;
        let metadata_after = input.metadata()?;
        if metadata_before.len() != bytes
            || metadata_after.len() != metadata_before.len()
            || metadata_after.modified().ok() != metadata_before.modified().ok()
            || !source.regular_child_matches_open_file(name, &input)?
        {
            return Err(RecoveryError::InvalidJournal);
        }
        let relative_file = relative.join(name);
        let relative_text = relative_file
            .to_str()
            .ok_or(RecoveryError::InvalidJournal)?;
        if relative_text.contains(['\n', '\r']) {
            return Err(RecoveryError::InvalidJournal);
        }
        writeln!(manifest, "{bytes} {mode} {:x}", hasher.finalize())?;
        writeln!(manifest, "{relative_text}")?;
    }
    if directory_names(source)? != before {
        return Err(RecoveryError::InvalidJournal);
    }
    destination.sync()?;
    Ok(())
}

fn directory_names(directory: &AnchoredDir) -> Result<Vec<std::ffi::OsString>, RecoveryError> {
    directory.verify_logical_identity()?;
    let mut names = fs::read_dir(directory.logical_path())?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    if names.len() > MAX_BACKUP_DIRECTORY_ENTRIES {
        return Err(RecoveryError::InvalidJournal);
    }
    names.sort();
    Ok(names)
}

fn sha256_regular_nofollow(path: &Path) -> Result<String, RecoveryError> {
    let parent = path.parent().ok_or(RecoveryError::InvalidJournal)?;
    let name = path.file_name().ok_or(RecoveryError::InvalidJournal)?;
    let directory = AnchoredDir::open_workspace(parent, None)?;
    let file = directory.open_regular_os(name)?;
    sha256_reader(file).map_err(RecoveryError::Io)
}

fn sha256_reader(mut file: File) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn home_identity(home: &Path) -> Result<String, RecoveryError> {
    let root = AnchoredDir::open_workspace(home, None)?;
    root.stable_identity_token().map_err(RecoveryError::Io)
}

fn validate_journal(
    journal: &RecoveryJournal,
    home: &Path,
    parent: &Path,
) -> Result<(), RecoveryError> {
    if journal.version != "v0.2.0-rc5"
        || journal.home_identity != home_identity(home)?
        || !valid_direct_name(&journal.home_backup_dir)
        || !journal.home_backup_dir.starts_with(BACKUP_PREFIX)
        || !is_sha256(&journal.backup_manifest_sha256)
    {
        return Err(RecoveryError::InvalidJournal);
    }
    let backup = parent.join(&journal.home_backup_dir);
    let manifest = backup.join("MANIFEST.sha256");
    if sha256_regular_nofollow(&manifest)? != journal.backup_manifest_sha256 {
        return Err(RecoveryError::InvalidJournal);
    }
    validate_backup_manifest(&backup, &manifest)?;

    let staged_required = journal.phase != RecoveryPhase::BackupComplete;
    if staged_required
        != (journal.staged_binary_name == STAGED_BINARY_NAME && is_sha256(&journal.staged_sha256))
    {
        return Err(RecoveryError::InvalidJournal);
    }
    if matches!(
        journal.phase,
        RecoveryPhase::BackupComplete | RecoveryPhase::StagedVerified
    ) && !journal.planned_workspaces.is_empty()
    {
        return Err(RecoveryError::InvalidJournal);
    }
    let mut keys = journal
        .planned_workspaces
        .iter()
        .map(|workspace| workspace.workspace_key.clone())
        .collect::<Vec<_>>();
    if journal.planned_workspaces.iter().any(|workspace| {
        !valid_direct_name(&workspace.workspace_key)
            || workspace.root_identity.is_empty()
            || workspace.database_identity.is_empty()
            || workspace.observability_identity.is_empty()
            || workspace.schema_before.is_empty()
    }) {
        return Err(RecoveryError::InvalidJournal);
    }
    keys.sort();
    keys.dedup();
    if keys.len() != journal.planned_workspaces.len() {
        return Err(RecoveryError::InvalidJournal);
    }
    Ok(())
}

fn valid_direct_name(value: &str) -> bool {
    let path = Path::new(value);
    let mut components = path.components();
    !value.is_empty()
        && matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

fn validate_backup_manifest(backup: &Path, manifest: &Path) -> Result<(), RecoveryError> {
    let backup_root = AnchoredDir::open_workspace(backup, None)?;
    if manifest.file_name() != Some(OsStr::new("MANIFEST.sha256"))
        || manifest.parent() != Some(backup)
    {
        return Err(RecoveryError::InvalidJournal);
    }
    let manifest_file = backup_root.open_regular_os(OsStr::new("MANIFEST.sha256"))?;
    let mut text = String::new();
    manifest_file
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(RecoveryError::Io)?;
    if u64::try_from(text.len()).map_err(|_| RecoveryError::InvalidJournal)? > MAX_MANIFEST_BYTES {
        return Err(RecoveryError::InvalidJournal);
    }
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() % 2 != 0 {
        return Err(RecoveryError::InvalidJournal);
    }
    let mut manifest_paths = BTreeSet::new();
    for pair in lines.chunks_exact(2) {
        let mut fields = pair[0].split_whitespace();
        let bytes = fields
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(RecoveryError::InvalidJournal)?;
        let mode = fields
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or(RecoveryError::InvalidJournal)?;
        let digest = fields.next().ok_or(RecoveryError::InvalidJournal)?;
        if fields.next().is_some() || !is_sha256(digest) {
            return Err(RecoveryError::InvalidJournal);
        }
        let relative = Path::new(pair[1]);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || !manifest_paths.insert(pair[1].to_string())
        {
            return Err(RecoveryError::InvalidJournal);
        }
        let file = open_relative_regular(&backup_root, relative)?;
        let metadata = file.metadata().map_err(RecoveryError::Io)?;
        if !metadata.is_file()
            || metadata.len() != bytes
            || mode_bits(&metadata) != mode
            || sha256_reader(file).map_err(RecoveryError::Io)? != digest
        {
            return Err(RecoveryError::InvalidJournal);
        }
    }
    let mut covered_files = 0_usize;
    validate_manifest_coverage(
        &backup_root,
        Path::new(""),
        &manifest_paths,
        &mut covered_files,
        0,
    )?;
    if covered_files != manifest_paths.len() {
        return Err(RecoveryError::InvalidJournal);
    }
    Ok(())
}

fn validate_manifest_coverage(
    directory: &AnchoredDir,
    relative: &Path,
    manifest_paths: &BTreeSet<String>,
    covered_files: &mut usize,
    depth: usize,
) -> Result<(), RecoveryError> {
    if depth > MAX_BACKUP_DEPTH {
        return Err(RecoveryError::InvalidJournal);
    }
    for name in directory_names(directory)? {
        if let Ok(child) = directory.child_dir_os(&name, false) {
            validate_manifest_coverage(
                &child,
                &relative.join(&name),
                manifest_paths,
                covered_files,
                depth + 1,
            )?;
            continue;
        }
        let file = directory.open_regular_os(&name)?;
        if !file.metadata()?.is_file() {
            return Err(RecoveryError::InvalidJournal);
        }
        if relative.as_os_str().is_empty() && name == OsStr::new("MANIFEST.sha256") {
            continue;
        }
        let path = relative
            .join(&name)
            .to_str()
            .ok_or(RecoveryError::InvalidJournal)?
            .to_string();
        if !manifest_paths.contains(&path) {
            return Err(RecoveryError::InvalidJournal);
        }
        *covered_files = covered_files
            .checked_add(1)
            .ok_or(RecoveryError::InvalidJournal)?;
        if *covered_files > MAX_BACKUP_ENTRIES {
            return Err(RecoveryError::InvalidJournal);
        }
    }
    Ok(())
}

fn open_relative_regular(root: &AnchoredDir, relative: &Path) -> Result<File, RecoveryError> {
    let mut components = relative.components().peekable();
    let mut directory = root.clone();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(name) = component else {
            return Err(RecoveryError::InvalidJournal);
        };
        if components.peek().is_none() {
            return directory.open_regular_os(name).map_err(RecoveryError::Io);
        }
        directory = directory
            .child_dir_os(name, false)
            .map_err(RecoveryError::Io)?;
    }
    Err(RecoveryError::InvalidJournal)
}

fn set_executable_permissions(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o755))
    }
    #[cfg(not(unix))]
    {
        let mut permissions = file.metadata()?.permissions();
        permissions.set_readonly(false);
        file.set_permissions(permissions)
    }
}

fn ensure_executable(path: &Path) -> Result<(), RecoveryError> {
    let metadata = fs::symlink_metadata(path).map_err(RecoveryError::Io)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(RecoveryError::InvalidStagedBinary);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(RecoveryError::InvalidStagedBinary);
        }
    }
    Ok(())
}

fn mode_bits(metadata: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.mode()
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        0
    }
}

fn all_planned_workspaces_applied(
    paths: &storage::AopmemPaths,
    planned: &[PlannedWorkspaceIdentity],
) -> Result<bool, RecoveryError> {
    match validate_frozen_plan(paths, planned, true) {
        Ok(()) => Ok(true),
        Err(RecoveryError::WorkspaceDrift | RecoveryError::ObservabilityV2Required) => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn ensure_observability_v2(
    paths: &storage::AopmemPaths,
    plan: &UpgradePlanReport,
) -> Result<(), RecoveryError> {
    for workspace in &plan.workspaces {
        let workspace_paths = storage::workspace_paths_for_key(paths, &workspace.workspace_key);
        drop(
            observability::open_writer(&workspace_paths)
                .map_err(|_| RecoveryError::ObservabilityV2Required)?,
        );
        drop(
            observability::open_reader(&workspace_paths)
                .map_err(|_| RecoveryError::ObservabilityV2Required)?,
        );
    }
    Ok(())
}

pub(super) fn capture_planned_workspaces(
    paths: &storage::AopmemPaths,
    plan: &UpgradePlanReport,
) -> Result<Vec<PlannedWorkspaceIdentity>, RecoveryError> {
    let mut captured = Vec::with_capacity(plan.workspaces.len());
    for workspace in &plan.workspaces {
        let schema = workspace
            .schema
            .as_ref()
            .ok_or(RecoveryError::WorkspaceDrift)?;
        let workspace_paths = storage::workspace_paths_for_key(paths, &workspace.workspace_key);
        let root = AnchoredDir::open_workspace(workspace_paths.root(), None)
            .map_err(|_| RecoveryError::WorkspaceDrift)?;
        let observability_root = AnchoredDir::open_workspace(workspace_paths.observability(), None)
            .map_err(|_| RecoveryError::ObservabilityV2Required)?;
        captured.push(PlannedWorkspaceIdentity {
            workspace_key: workspace.workspace_key.clone(),
            root_identity: root
                .stable_identity_token()
                .map_err(|_| RecoveryError::WorkspaceDrift)?,
            database_identity: root
                .regular_child_identity_token(OsStr::new("aopmem.sqlite"))
                .map_err(|_| RecoveryError::WorkspaceDrift)?,
            observability_identity: format!(
                "{}:{}",
                observability_root
                    .stable_identity_token()
                    .map_err(|_| RecoveryError::ObservabilityV2Required)?,
                observability_root
                    .regular_child_identity_token(
                        workspace_paths
                            .observability_db()
                            .file_name()
                            .ok_or(RecoveryError::ObservabilityV2Required)?,
                    )
                    .map_err(|_| RecoveryError::ObservabilityV2Required)?
            ),
            schema_before: schema.current_version.clone(),
        });
    }
    captured.sort_by(|left, right| left.workspace_key.cmp(&right.workspace_key));
    Ok(captured)
}

pub(super) fn validate_frozen_plan(
    paths: &storage::AopmemPaths,
    expected: &[PlannedWorkspaceIdentity],
    require_target: bool,
) -> Result<(), RecoveryError> {
    let plan = plan_all_workspaces().map_err(|_| RecoveryError::WorkspaceDrift)?;
    if !plan.ready {
        return Err(RecoveryError::WorkspaceDrift);
    }
    for workspace in &plan.workspaces {
        let schema = workspace
            .schema
            .as_ref()
            .ok_or(RecoveryError::WorkspaceDrift)?;
        if require_target
            && (schema.current_version != "004"
                || schema.target_version != "004"
                || !schema.pending_migrations.is_empty())
        {
            return Err(RecoveryError::WorkspaceDrift);
        }
        let workspace_paths = storage::workspace_paths_for_key(paths, &workspace.workspace_key);
        drop(
            observability::open_reader(&workspace_paths)
                .map_err(|_| RecoveryError::ObservabilityV2Required)?,
        );
    }
    let current = capture_planned_workspaces(paths, &plan)?;
    if !frozen_identities_match(&current, expected, require_target) {
        return Err(RecoveryError::WorkspaceDrift);
    }
    Ok(())
}

fn frozen_identities_match(
    current: &[PlannedWorkspaceIdentity],
    expected: &[PlannedWorkspaceIdentity],
    allow_target_schema_change: bool,
) -> bool {
    current.len() == expected.len()
        && current.iter().zip(expected).all(|(current, expected)| {
            current.workspace_key == expected.workspace_key
                && current.root_identity == expected.root_identity
                && current.database_identity == expected.database_identity
                && current.observability_identity == expected.observability_identity
                && (allow_target_schema_change || current.schema_before == expected.schema_before)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::ffi::OsString;

    fn temp_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("aopmem-stage21-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temporary directory should create");
        path
    }

    struct EnvGuard {
        original: Option<OsString>,
    }

    impl EnvGuard {
        fn set(home: &Path) -> Self {
            let original = std::env::var_os("AOPMEM_HOME");
            std::env::set_var("AOPMEM_HOME", home);
            Self { original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.original.as_ref() {
                Some(value) => std::env::set_var("AOPMEM_HOME", value),
                None => std::env::remove_var("AOPMEM_HOME"),
            }
        }
    }

    fn create_schema_001_workspace(home: &Path, key: &str) {
        let root = home.join("workspaces").join(key);
        fs::create_dir_all(&root).expect("workspace root should create");
        let connection =
            Connection::open(root.join("aopmem.sqlite")).expect("workspace database should open");
        connection
            .execute_batch(
                "
                CREATE TABLE schema_migrations (
                    version TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO schema_migrations(version, name)
                VALUES ('001', '001_init');
                ",
            )
            .expect("schema 001 marker should create");
    }

    fn freeze_fixture(home: &Path) -> (storage::AopmemPaths, Vec<PlannedWorkspaceIdentity>) {
        let paths = storage::resolve_paths().expect("paths should resolve");
        let plan = plan_all_workspaces().expect("plan should inspect");
        ensure_observability_v2(&paths, &plan).expect("observability v2 should initialize");
        let frozen =
            capture_planned_workspaces(&paths, &plan).expect("workspace identities should freeze");
        assert_eq!(paths.home(), home);
        (paths, frozen)
    }

    struct TestRecoveryHooks {
        fail_at: Option<RecoveryFaultPoint>,
        core_invocations: usize,
        inject_publish_durability_failure: bool,
    }

    impl TestRecoveryHooks {
        fn new(fail_at: RecoveryFaultPoint) -> Self {
            Self {
                fail_at: Some(fail_at),
                core_invocations: 0,
                inject_publish_durability_failure: false,
            }
        }

        fn arm(&mut self, fail_at: RecoveryFaultPoint) {
            self.fail_at = Some(fail_at);
        }

        fn inject_publish_durability_failure(&mut self) {
            self.inject_publish_durability_failure = true;
        }
    }

    impl RecoveryHooks for TestRecoveryHooks {
        fn checkpoint(&mut self, point: RecoveryFaultPoint) -> Result<(), RecoveryError> {
            if self.fail_at == Some(point) {
                self.fail_at = None;
                return Err(RecoveryError::Io(io::Error::other(format!(
                    "injected recovery fault at {point:?}"
                ))));
            }
            Ok(())
        }

        fn apply_core(
            &mut self,
            planned: &[PlannedWorkspaceIdentity],
        ) -> Result<UpgradeApplyExecution, RecoveryError> {
            self.core_invocations += 1;
            apply_core_all_workspaces(planned)
                .map_err(|error| RecoveryError::Apply(error.to_string()))
        }

        fn publish_binary(
            &mut self,
            staged_name: &str,
            expected_sha256: &str,
            bin: &Path,
        ) -> Result<bool, RecoveryError> {
            if self.inject_publish_durability_failure {
                self.inject_publish_durability_failure = false;
                publish_staged_binary(staged_name, expected_sha256, bin)?;
                return Ok(true);
            }
            publish_staged_binary(staged_name, expected_sha256, bin)
        }
    }

    fn seed_upgrade_workspace(home: &Path, key: &str, source_schema: &str, obs_v1: bool) {
        let paths = storage::resolve_paths().expect("paths should resolve");
        assert_eq!(paths.home(), home);
        storage::ensure_global_dirs(&paths).expect("global directories should create");
        let workspace =
            storage::ensure_workspace_dirs(&paths, key).expect("workspace should create");
        let mut connection =
            Connection::open(workspace.db()).expect("workspace database should open");
        crate::schema::apply_migrations(&mut connection).expect("current schema should initialize");
        connection
            .execute_batch(
                "
                DROP TRIGGER IF EXISTS tool_aliases_validate_insert;
                DROP TRIGGER IF EXISTS tool_aliases_validate_update;
                DROP TRIGGER IF EXISTS tool_contracts_reject_alias_shadow_insert;
                DROP TRIGGER IF EXISTS tool_contracts_reject_alias_shadow_update;
                DROP TRIGGER IF EXISTS tool_contracts_preserve_alias_target;
                DROP TABLE IF EXISTS tool_aliases;
                DELETE FROM schema_migrations WHERE version = '004';
                ",
            )
            .expect("schema 004 should remove");
        if source_schema == "001" {
            connection
                .execute_batch(
                    "
                    DROP INDEX IF EXISTS idx_nodes_summary;
                    DROP INDEX IF EXISTS idx_nodes_title_nocase;
                    DROP INDEX IF EXISTS idx_aliases_alias_nocase;
                    DROP INDEX IF EXISTS idx_tags_tag_nocase;
                    DELETE FROM schema_migrations WHERE version IN ('002', '003');
                    ",
                )
                .expect("schema 002 and 003 should remove");
        } else {
            assert_eq!(source_schema, "003");
        }
        crate::audit::write_sql_snapshot(workspace.audit_git(), &connection)
            .expect("clean audit snapshot should create");
        drop(connection);

        if obs_v1 {
            fs::create_dir_all(workspace.observability())
                .expect("observability directory should create");
            let mut observability_connection = Connection::open(workspace.observability_db())
                .expect("observability database should open");
            observability::initialize_v1_fixture(
                &mut observability_connection,
                workspace.observability_db(),
            )
            .expect("observability v1 should initialize");
        }
    }

    fn seed_staged_recovery(root: &Path, hooks: &mut impl RecoveryHooks) -> (PathBuf, String) {
        let home = root.join("home");
        fs::create_dir_all(home.join("bin")).expect("home should create");
        fs::write(home.join("bin/aopmem"), b"old-binary").expect("old binary should write");
        backup_home_with_hooks(hooks).expect("home backup should complete");
        let artifact = root.join("downloaded-aopmem");
        fs::write(&artifact, b"verified-rc5-binary").expect("artifact should write");
        let digest = sha256_regular_nofollow(&artifact).expect("artifact digest should compute");
        stage_binary_with_hooks(&artifact, &digest, hooks).expect("artifact should stage");
        (artifact, digest)
    }

    #[test]
    fn durable_journal_round_trips_through_atomic_publish() {
        let root = temp_dir("journal");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        fs::write(home.join("value"), b"preserved").expect("home value should write");
        let backup = create_full_home_backup(&home, &root).expect("backup should create");
        let journal = RecoveryJournal {
            version: "v0.2.0-rc5".to_string(),
            phase: RecoveryPhase::BackupComplete,
            home_identity: home_identity(&home).expect("home identity"),
            home_backup_dir: backup
                .file_name()
                .expect("backup name")
                .to_string_lossy()
                .into_owned(),
            backup_manifest_sha256: sha256_regular_nofollow(&backup.join("MANIFEST.sha256"))
                .expect("manifest digest"),
            staged_binary_name: String::new(),
            staged_sha256: String::new(),
            planned_workspaces: Vec::new(),
        };
        write_journal(&root, &home, &journal).expect("journal should publish");
        assert_eq!(
            read_journal(&root).expect("journal should read"),
            Some(journal)
        );
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn installer_backup_adoption_validates_home_and_creates_no_second_backup() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("adopt-backup");
        let home = root.join("home");
        fs::create_dir_all(home.join("nested")).expect("home should create");
        fs::write(home.join("nested/value"), b"preserved").expect("home value should write");
        let backup = create_full_home_backup(&home, &root).expect("installer backup should create");
        let digest = sha256_regular_nofollow(&backup.join("MANIFEST.sha256"))
            .expect("manifest digest should compute");
        let backup_count_before = fs::read_dir(&root)
            .expect("parent should read")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(BACKUP_PREFIX)
            })
            .count();
        let _guard = EnvGuard::set(&home);

        let execution = adopt_home_backup(&backup, &digest).expect("matching backup should adopt");
        assert_eq!(execution.journal_phase, RecoveryPhase::BackupComplete);
        let backup_count_after = fs::read_dir(&root)
            .expect("parent should read")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(BACKUP_PREFIX)
            })
            .count();
        assert_eq!(backup_count_after, backup_count_before);

        let stale_journal = root.join(format!(
            ".{JOURNAL_PREFIX}{}.tmp",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let stale_home_manifest = root.join(format!(
            "{HOME_MANIFEST_TEMP_PREFIX}{}.tmp",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::write(&stale_journal, b"stale").expect("stale journal temp should write");
        fs::write(&stale_home_manifest, b"stale").expect("stale home manifest temp should write");
        let resumed = adopt_home_backup(&backup, &digest)
            .expect("idempotent adoption should clean recovery temps");
        assert!(resumed.resumed);
        assert!(!stale_journal.exists());
        assert!(!stale_home_manifest.exists());
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn recovery_temp_cleanup_rejects_overflow_and_unsafe_matching_entry() {
        let root = temp_dir("cleanup-limits");
        let first = root.join(format!(
            ".{JOURNAL_PREFIX}{}.tmp",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let second = root.join(format!(
            "{HOME_MANIFEST_TEMP_PREFIX}{}.tmp",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::write(&first, b"one").expect("first temp should write");
        fs::write(&second, b"two").expect("second temp should write");
        assert!(matches!(
            cleanup_recovery_temporaries_with_limits(&root, 16, 1),
            Err(RecoveryError::InvalidJournal)
        ));
        assert!(first.exists(), "overflow must fail before removal");
        assert!(second.exists(), "overflow must fail before removal");
        assert!(matches!(
            cleanup_recovery_temporaries_with_limits(&root, 1, 16),
            Err(RecoveryError::InvalidJournal)
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            fs::remove_file(&first).expect("first temp should remove");
            fs::remove_file(&second).expect("second temp should remove");
            let outside = root.join("outside");
            fs::write(&outside, b"preserved").expect("outside fixture should write");
            let linked = root.join(format!(
                ".{JOURNAL_PREFIX}{}.tmp",
                uuid::Uuid::new_v4().hyphenated()
            ));
            symlink(&outside, &linked).expect("unsafe temp symlink should create");
            assert!(cleanup_recovery_temporaries_with_limits(&root, 16, 16).is_err());
            assert_eq!(
                fs::read(&outside).expect("outside fixture should read"),
                b"preserved"
            );
        }
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn installer_backup_adoption_rejects_tampered_foreign_and_changed_home() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");

        {
            let root = temp_dir("adopt-tampered");
            let home = root.join("home");
            fs::create_dir(&home).expect("home should create");
            fs::write(home.join("value"), b"preserved").expect("home value should write");
            let backup =
                create_full_home_backup(&home, &root).expect("installer backup should create");
            let digest = sha256_regular_nofollow(&backup.join("MANIFEST.sha256"))
                .expect("manifest digest should compute");
            fs::write(backup.join("value"), b"tampered").expect("backup should tamper");
            let _guard = EnvGuard::set(&home);
            assert!(matches!(
                adopt_home_backup(&backup, &digest),
                Err(RecoveryError::InvalidJournal)
            ));
            fs::write(backup.join("value"), b"preserved").expect("backup should restore");
            fs::write(backup.join("unlisted"), b"extra").expect("extra file should write");
            assert!(matches!(
                adopt_home_backup(&backup, &digest),
                Err(RecoveryError::InvalidJournal)
            ));
            fs::remove_dir_all(root).expect("temporary directory should remove");
        }

        {
            let root = temp_dir("adopt-foreign");
            let home = root.join("home");
            let foreign_home = root.join("foreign-home");
            fs::create_dir(&home).expect("home should create");
            fs::create_dir(&foreign_home).expect("foreign home should create");
            fs::write(home.join("value"), b"current").expect("home value should write");
            fs::write(foreign_home.join("value"), b"foreign").expect("foreign value should write");
            let backup = create_full_home_backup(&foreign_home, &root)
                .expect("foreign installer backup should create");
            let digest = sha256_regular_nofollow(&backup.join("MANIFEST.sha256"))
                .expect("manifest digest should compute");
            let _guard = EnvGuard::set(&home);
            assert!(matches!(
                adopt_home_backup(&backup, &digest),
                Err(RecoveryError::InvalidJournal)
            ));
            fs::remove_dir_all(root).expect("temporary directory should remove");
        }

        {
            let root = temp_dir("adopt-home-changed");
            let home = root.join("home");
            fs::create_dir(&home).expect("home should create");
            fs::write(home.join("value"), b"before").expect("home value should write");
            let backup =
                create_full_home_backup(&home, &root).expect("installer backup should create");
            let digest = sha256_regular_nofollow(&backup.join("MANIFEST.sha256"))
                .expect("manifest digest should compute");
            fs::write(home.join("value"), b"after").expect("home should change");
            let _guard = EnvGuard::set(&home);
            assert!(matches!(
                adopt_home_backup(&backup, &digest),
                Err(RecoveryError::InvalidJournal)
            ));
            fs::remove_dir_all(root).expect("temporary directory should remove");
        }
    }

    #[test]
    fn oversized_serialized_checkpoint_is_rejected_before_write() {
        let journal = RecoveryJournal {
            version: "v0.2.0-rc5".to_string(),
            phase: RecoveryPhase::Prepared,
            home_identity: "home".to_string(),
            home_backup_dir: "backup".to_string(),
            backup_manifest_sha256: "0".repeat(64),
            staged_binary_name: STAGED_BINARY_NAME.to_string(),
            staged_sha256: "1".repeat(64),
            planned_workspaces: vec![PlannedWorkspaceIdentity {
                workspace_key: "alpha".to_string(),
                root_identity: "x".repeat(MAX_JOURNAL_BYTES),
                database_identity: "database".to_string(),
                observability_identity: "observability".to_string(),
                schema_before: "003".to_string(),
            }],
        };
        assert!(matches!(
            serialize_journal(&journal),
            Err(RecoveryError::InvalidJournal)
        ));
    }

    #[test]
    fn journal_rejects_missing_first_or_middle_checkpoint() {
        let root = temp_dir("journal-gap");
        let base = RecoveryJournal {
            version: "v0.2.0-rc5".to_string(),
            phase: RecoveryPhase::BackupComplete,
            home_identity: "home".to_string(),
            home_backup_dir: "backup".to_string(),
            backup_manifest_sha256: "0".repeat(64),
            staged_binary_name: String::new(),
            staged_sha256: String::new(),
            planned_workspaces: Vec::new(),
        };
        let mut staged = base.clone();
        staged.phase = RecoveryPhase::StagedVerified;
        staged.staged_binary_name = STAGED_BINARY_NAME.to_string();
        staged.staged_sha256 = "1".repeat(64);
        fs::write(
            root.join(journal_name(RecoveryPhase::StagedVerified)),
            serde_json::to_vec(&staged).expect("journal should serialize"),
        )
        .expect("staged checkpoint should write");
        assert!(matches!(
            read_journal(&root),
            Err(RecoveryError::InvalidJournal)
        ));

        fs::remove_file(root.join(journal_name(RecoveryPhase::StagedVerified)))
            .expect("staged checkpoint should remove");
        fs::write(
            root.join(journal_name(RecoveryPhase::BackupComplete)),
            serde_json::to_vec(&base).expect("journal should serialize"),
        )
        .expect("backup checkpoint should write");
        let mut prepared = staged;
        prepared.phase = RecoveryPhase::Prepared;
        fs::write(
            root.join(journal_name(RecoveryPhase::Prepared)),
            serde_json::to_vec(&prepared).expect("journal should serialize"),
        )
        .expect("prepared checkpoint should write");
        assert!(matches!(
            read_journal(&root),
            Err(RecoveryError::InvalidJournal)
        ));
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn backup_and_stage_fault_windows_resume_without_touching_home_payload() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("fault-backup-stage");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        fs::write(home.join("preserved"), b"unchanged").expect("home fixture should write");
        let _guard = EnvGuard::set(&home);
        let mut hooks = TestRecoveryHooks::new(RecoveryFaultPoint::BackupEffect);

        assert!(backup_home_with_hooks(&mut hooks).is_err());
        assert_eq!(
            fs::read(home.join("preserved")).expect("home fixture should read"),
            b"unchanged"
        );
        assert_eq!(
            read_journal(&root).expect("journal lookup should work"),
            None
        );

        backup_home_with_hooks(&mut hooks).expect("backup retry should complete");
        let artifact = root.join("downloaded-aopmem");
        fs::write(&artifact, b"verified-rc5-binary").expect("artifact should write");
        let digest = sha256_regular_nofollow(&artifact).expect("artifact digest should compute");
        hooks.arm(RecoveryFaultPoint::StageEffect);
        assert!(stage_binary_with_hooks(&artifact, &digest, &mut hooks).is_err());
        assert_eq!(
            read_journal(&root)
                .expect("journal should read")
                .expect("backup checkpoint should exist")
                .phase,
            RecoveryPhase::BackupComplete
        );
        assert_eq!(
            sha256_regular_nofollow(&home.join("bin").join(STAGED_BINARY_NAME))
                .expect("retained binary should exist"),
            digest
        );

        let resumed = stage_binary_with_hooks(&artifact, &digest, &mut hooks)
            .expect("stage retry should complete");
        assert_eq!(resumed.journal_phase, RecoveryPhase::StagedVerified);
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn committed_binary_durability_uncertainty_warns_and_checkpoints_published() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("publish-durability-warning");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        let mut hooks = TestRecoveryHooks::new(RecoveryFaultPoint::PrepareEffect);
        hooks.fail_at = None;
        assert!(require_installed_binary_publish(Ok(PublishOutcome {
            strategy: PublishStrategy::WindowsReplaceFileW,
            destination_existed: true,
            committed: true,
            final_validated: true,
            durability_confirmed: false,
            temporary_cleanup_confirmed: true,
        }))
        .expect("validated ReplaceFileW uncertainty should warn"));
        assert!(matches!(
            require_installed_binary_publish(Ok(PublishOutcome {
                strategy: PublishStrategy::UnixRenameAt,
                destination_existed: true,
                committed: true,
                final_validated: true,
                durability_confirmed: false,
                temporary_cleanup_confirmed: true,
            })),
            Err(RecoveryError::Publish(_))
        ));
        let (_artifact, digest) = seed_staged_recovery(&root, &mut hooks);
        let applied = apply_or_resume_with_hooks(&mut hooks).expect("empty apply should complete");
        assert_eq!(applied.journal_phase, RecoveryPhase::Applied);

        hooks.inject_publish_durability_failure();
        let published =
            publish_applied_with_hooks(&mut hooks).expect("committed publish should succeed");
        assert_eq!(published.journal_phase, RecoveryPhase::Published);
        assert!(published.binary_published);
        assert!(published.durability_warning);
        assert_eq!(
            sha256_regular_nofollow(&home.join("bin/aopmem"))
                .expect("installed binary should read"),
            digest
        );
        assert_eq!(
            read_journal(&root)
                .expect("journal should read")
                .expect("published checkpoint should exist")
                .phase,
            RecoveryPhase::Published
        );
        let retained = home.join("bin").join(STAGED_BINARY_NAME);
        assert_eq!(
            sha256_regular_nofollow(&retained).expect("retained binary should read"),
            digest
        );
        let core_invocations = hooks.core_invocations;

        fs::write(home.join("bin/aopmem"), b"tampered").expect("installed binary should tamper");
        let repaired =
            publish_applied_with_hooks(&mut hooks).expect("published replay should repair");
        assert_eq!(repaired.journal_phase, RecoveryPhase::Published);
        assert!(repaired.binary_published);
        assert_eq!(hooks.core_invocations, core_invocations);
        assert_eq!(
            sha256_regular_nofollow(&home.join("bin/aopmem"))
                .expect("repaired installed binary should read"),
            digest
        );
        assert_eq!(
            sha256_regular_nofollow(&retained).expect("retained binary should remain"),
            digest
        );
        assert_eq!(
            read_journal(&root)
                .expect("journal should read")
                .expect("published checkpoint should remain")
                .phase,
            RecoveryPhase::Published
        );
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn fault_hooks_prove_mixed_schema_core_once_and_publish_ordering() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("fault-core-publish");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        seed_upgrade_workspace(&home, "alpha", "001", true);
        seed_upgrade_workspace(&home, "beta", "003", false);
        let mut hooks = TestRecoveryHooks::new(RecoveryFaultPoint::PrepareEffect);
        let (_artifact, digest) = seed_staged_recovery(&root, &mut hooks);

        assert!(apply_or_resume_with_hooks(&mut hooks).is_err());
        assert_eq!(hooks.core_invocations, 0);
        assert_eq!(
            read_journal(&root)
                .expect("journal should read")
                .expect("staged checkpoint should exist")
                .phase,
            RecoveryPhase::StagedVerified
        );

        hooks.arm(RecoveryFaultPoint::CoreEffect);
        assert!(apply_or_resume_with_hooks(&mut hooks).is_err());
        assert_eq!(hooks.core_invocations, 1);
        assert_eq!(
            read_journal(&root)
                .expect("journal should read")
                .expect("apply-started checkpoint should exist")
                .phase,
            RecoveryPhase::ApplyStarted
        );
        for key in ["alpha", "beta"] {
            let workspace = storage::workspace_paths_for_key(
                &storage::resolve_paths().expect("paths should resolve"),
                key,
            );
            let connection =
                Connection::open(workspace.db()).expect("workspace database should reopen");
            let version: String = connection
                .query_row(
                    "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("schema version should read");
            assert_eq!(version, "004");
            let reader =
                observability::open_reader(&workspace).expect("observability v2 should open");
            assert_eq!(reader.schema_version().expect("schema should read"), 2);
        }

        let resumed =
            apply_or_resume_with_hooks(&mut hooks).expect("applied state should reconcile");
        assert_eq!(resumed.journal_phase, RecoveryPhase::Applied);
        assert_eq!(
            hooks.core_invocations, 1,
            "core apply must run exactly once"
        );

        hooks.arm(RecoveryFaultPoint::PublishEffect);
        assert!(publish_applied_with_hooks(&mut hooks).is_err());
        assert_eq!(
            read_journal(&root)
                .expect("journal should read")
                .expect("applied checkpoint should remain")
                .phase,
            RecoveryPhase::Applied
        );
        assert_eq!(
            sha256_regular_nofollow(&home.join("bin/aopmem"))
                .expect("installed binary should read"),
            digest
        );
        let published =
            publish_applied_with_hooks(&mut hooks).expect("publish retry should complete");
        assert_eq!(published.journal_phase, RecoveryPhase::Published);
        assert_eq!(hooks.core_invocations, 1);
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn apply_started_fault_never_auto_retries_unknown_core_outcome() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("fault-apply-started");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        seed_upgrade_workspace(&home, "alpha", "003", false);
        let mut hooks = TestRecoveryHooks::new(RecoveryFaultPoint::ApplyStarted);
        seed_staged_recovery(&root, &mut hooks);

        assert!(apply_or_resume_with_hooks(&mut hooks).is_err());
        assert_eq!(hooks.core_invocations, 0);
        assert_eq!(
            read_journal(&root)
                .expect("journal should read")
                .expect("apply-started checkpoint should exist")
                .phase,
            RecoveryPhase::ApplyStarted
        );
        assert!(matches!(
            apply_or_resume_with_hooks(&mut hooks),
            Err(RecoveryError::ApplyOutcomeUnknown)
        ));
        assert_eq!(hooks.core_invocations, 0);
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn apply_and_publish_require_their_exact_prior_phase() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("phase-guards");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        let mut hooks = LiveRecoveryHooks;

        backup_home_with_hooks(&mut hooks).expect("backup should complete");
        assert!(matches!(
            apply_or_resume_with_hooks(&mut hooks),
            Err(RecoveryError::InvalidPhase)
        ));
        assert!(matches!(
            publish_applied_with_hooks(&mut hooks),
            Err(RecoveryError::InvalidPhase)
        ));

        let artifact = root.join("downloaded-aopmem");
        fs::write(&artifact, b"verified-rc5-binary").expect("artifact should write");
        let digest = sha256_regular_nofollow(&artifact).expect("artifact digest should compute");
        stage_binary_with_hooks(&artifact, &digest, &mut hooks).expect("stage should complete");
        assert!(matches!(
            publish_applied_with_hooks(&mut hooks),
            Err(RecoveryError::InvalidPhase)
        ));
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn full_home_backup_copies_regular_tree_and_rejects_links() {
        let root = temp_dir("backup");
        let home = root.join("home");
        fs::create_dir_all(home.join("nested")).expect("home should create");
        fs::write(home.join("nested/value"), b"preserved").expect("fixture should write");
        let backup = create_full_home_backup(&home, &root).expect("backup should succeed");
        assert_eq!(
            fs::read(backup.join("nested/value")).expect("backup should read"),
            b"preserved"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let linked_home = root.join("linked-home");
            fs::create_dir(&linked_home).expect("linked home should create");
            symlink(&home, linked_home.join("escape")).expect("link fixture should create");
            assert!(create_full_home_backup(&linked_home, &root).is_err());
        }
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn staged_hash_mismatch_fails_before_publish() {
        let root = temp_dir("hash");
        let staged = root.join("staged");
        fs::write(&staged, b"verified").expect("fixture should write");
        let journal = RecoveryJournal {
            version: "v0.2.0-rc5".to_string(),
            phase: RecoveryPhase::Prepared,
            home_identity: "identity".to_string(),
            home_backup_dir: "backup".to_string(),
            backup_manifest_sha256: "0".repeat(64),
            staged_binary_name: staged
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned(),
            staged_sha256: "not-the-file".to_string(),
            planned_workspaces: Vec::new(),
        };
        assert!(matches!(
            verify_retained_staged(&journal, &root),
            Err(RecoveryError::InvalidStagedBinary)
        ));
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    fn planned(key: &str, root: &str, database: &str) -> PlannedWorkspaceIdentity {
        PlannedWorkspaceIdentity {
            workspace_key: key.to_string(),
            root_identity: root.to_string(),
            database_identity: database.to_string(),
            observability_identity: format!("obs-{key}"),
            schema_before: "003".to_string(),
        }
    }

    #[test]
    fn frozen_identity_comparison_rejects_added_removed_and_replaced_entries() {
        let expected = vec![planned("alpha", "root-a", "db-a")];
        let added = vec![
            planned("alpha", "root-a", "db-a"),
            planned("beta", "root-b", "db-b"),
        ];
        let removed = Vec::new();
        let replaced_root = vec![planned("alpha", "root-replaced", "db-a")];
        let replaced_database = vec![planned("alpha", "root-a", "db-replaced")];

        assert!(!frozen_identities_match(&added, &expected, false));
        assert!(!frozen_identities_match(&removed, &expected, false));
        assert!(!frozen_identities_match(&replaced_root, &expected, false));
        assert!(!frozen_identities_match(
            &replaced_database,
            &expected,
            false
        ));
        assert!(frozen_identities_match(&expected, &expected, false));
    }

    #[test]
    fn frozen_plan_rejects_added_workspace_before_core() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("workspace-added");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        create_schema_001_workspace(&home, "alpha");
        let (paths, frozen) = freeze_fixture(&home);

        create_schema_001_workspace(&home, "beta");
        let changed = plan_all_workspaces().expect("changed plan should inspect");
        ensure_observability_v2(&paths, &changed).expect("added obs should initialize");

        assert!(matches!(
            validate_frozen_plan(&paths, &frozen, false),
            Err(RecoveryError::WorkspaceDrift)
        ));
        let execution =
            apply_core_all_workspaces(&frozen).expect("core drift should be a typed report");
        assert_eq!(
            execution
                .failure
                .as_ref()
                .expect("added workspace must block core")
                .code,
            "UPGRADE_WORKSPACE_DRIFT"
        );
        for key in ["alpha", "beta"] {
            let connection =
                Connection::open(home.join("workspaces").join(key).join("aopmem.sqlite"))
                    .expect("workspace DB should remain readable");
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .expect("migration count should read");
            assert_eq!(count, 1, "drift must not migrate {key}");
        }
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn frozen_plan_rejects_removed_workspace_before_core() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("workspace-removed");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        create_schema_001_workspace(&home, "alpha");
        let (paths, frozen) = freeze_fixture(&home);

        fs::remove_dir_all(home.join("workspaces/alpha")).expect("workspace should remove");

        assert!(matches!(
            validate_frozen_plan(&paths, &frozen, false),
            Err(RecoveryError::WorkspaceDrift)
        ));
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn frozen_plan_rejects_replaced_workspace_before_core() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("workspace-replaced");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        create_schema_001_workspace(&home, "alpha");
        let (paths, frozen) = freeze_fixture(&home);

        fs::remove_dir_all(home.join("workspaces/alpha")).expect("old workspace should remove");
        create_schema_001_workspace(&home, "alpha");
        let changed = plan_all_workspaces().expect("replacement plan should inspect");
        ensure_observability_v2(&paths, &changed).expect("replacement obs should initialize");

        assert!(matches!(
            validate_frozen_plan(&paths, &frozen, false),
            Err(RecoveryError::WorkspaceDrift)
        ));
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn observability_v2_boundary_is_mandatory_and_idempotent() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("obs-v2");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        create_schema_001_workspace(&home, "alpha");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let plan = plan_all_workspaces().expect("plan should inspect");

        ensure_observability_v2(&paths, &plan).expect("first migration should pass");
        let first =
            capture_planned_workspaces(&paths, &plan).expect("first identities should capture");
        ensure_observability_v2(&paths, &plan).expect("second migration should be idempotent");
        let second =
            capture_planned_workspaces(&paths, &plan).expect("second identities should capture");

        assert_eq!(first, second);
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn observability_v2_failure_blocks_before_core_state() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("obs-v2-blocked");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        let _guard = EnvGuard::set(&home);
        create_schema_001_workspace(&home, "alpha");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace = storage::workspace_paths_for_key(&paths, "alpha");
        fs::write(workspace.observability(), b"blocked")
            .expect("blocked observability fixture should write");
        let plan = plan_all_workspaces().expect("plan should inspect operational DB");

        assert!(matches!(
            ensure_observability_v2(&paths, &plan),
            Err(RecoveryError::ObservabilityV2Required)
        ));
        let connection =
            Connection::open(workspace.db()).expect("operational database should remain readable");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count should read");
        assert_eq!(migration_count, 1);
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn retain_is_idempotent_after_commit_before_journal_and_restores_stale_temp() {
        let root = temp_dir("retain-idempotent");
        let source_dir = root.join("download");
        let bin = root.join("bin");
        fs::create_dir(&source_dir).expect("download directory should create");
        let source = source_dir.join("aopmem");
        fs::write(&source, b"verified-rc5").expect("staged source should write");
        let digest = sha256_regular_nofollow(&source).expect("source digest");

        let retained =
            retain_staged_binary(&source, &bin, &digest).expect("first retain should publish");
        fs::write(bin.join(".aopmem-v0.2.0-rc5.retain.tmp"), b"stale")
            .expect("stale temp should write");
        let resumed =
            retain_staged_binary(&source, &bin, &digest).expect("resume should be idempotent");

        assert_eq!(resumed, retained);
        assert_eq!(
            sha256_regular_nofollow(&retained).expect("retained digest"),
            digest
        );
        assert!(
            !bin.join(".aopmem-v0.2.0-rc5.retain.tmp").exists(),
            "idempotent retain must clean stale temporary"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&retained)
                    .expect("retained metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
        }
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn publish_is_idempotent_and_preserves_retained_artifact() {
        let root = temp_dir("publish-idempotent");
        let source_dir = root.join("download");
        let bin = root.join("bin");
        fs::create_dir(&source_dir).expect("download directory should create");
        let source = source_dir.join("aopmem");
        fs::write(&source, b"verified-rc5").expect("staged source should write");
        let digest = sha256_regular_nofollow(&source).expect("source digest");
        let retained = retain_staged_binary(&source, &bin, &digest).expect("retain should publish");
        fs::write(bin.join("aopmem"), b"old-binary").expect("old binary should write");
        fs::write(bin.join(".aopmem-v0.2.0-rc5.publish.tmp"), b"stale")
            .expect("stale publish temp should write");

        publish_staged_binary(STAGED_BINARY_NAME, &digest, &bin)
            .expect("first publish should replace old binary");
        publish_staged_binary(STAGED_BINARY_NAME, &digest, &bin)
            .expect("resume after journal crash should be idempotent");

        assert_eq!(
            sha256_regular_nofollow(&bin.join("aopmem")).expect("installed digest"),
            digest
        );
        assert_eq!(
            sha256_regular_nofollow(&retained).expect("retained digest"),
            digest
        );
        assert!(
            !bin.join(".aopmem-v0.2.0-rc5.publish.tmp").exists(),
            "idempotent publish must clean stale temporary"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(bin.join("aopmem"))
                    .expect("installed metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
        }
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn journal_rejects_home_swap_and_backup_manifest_tampering() {
        let root = temp_dir("journal-binding");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        fs::write(home.join("value"), b"preserved").expect("home value should write");
        let backup = create_full_home_backup(&home, &root).expect("backup should create");
        let journal = RecoveryJournal {
            version: "v0.2.0-rc5".to_string(),
            phase: RecoveryPhase::BackupComplete,
            home_identity: home_identity(&home).expect("home identity"),
            home_backup_dir: backup
                .file_name()
                .expect("backup name")
                .to_string_lossy()
                .into_owned(),
            backup_manifest_sha256: sha256_regular_nofollow(&backup.join("MANIFEST.sha256"))
                .expect("manifest digest"),
            staged_binary_name: String::new(),
            staged_sha256: String::new(),
            planned_workspaces: Vec::new(),
        };
        validate_journal(&journal, &home, &root).expect("valid journal should pass");

        fs::write(backup.join("value"), b"tampered").expect("backup should tamper");
        assert!(matches!(
            validate_journal(&journal, &home, &root),
            Err(RecoveryError::InvalidJournal)
        ));
        fs::remove_file(backup.join("value")).expect("tampered backup should remove");
        fs::write(backup.join("value"), b"preserved").expect("backup should restore");

        let moved = root.join("old-home");
        fs::rename(&home, &moved).expect("home should move");
        fs::create_dir(&home).expect("replacement home should create");
        assert!(matches!(
            validate_journal(&journal, &home, &root),
            Err(RecoveryError::InvalidJournal)
        ));
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn journal_write_recovers_valid_stale_temporary_file() {
        let root = temp_dir("journal-stale-temp");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        fs::write(home.join("value"), b"preserved").expect("home value should write");
        let backup = create_full_home_backup(&home, &root).expect("backup should create");
        let journal = RecoveryJournal {
            version: "v0.2.0-rc5".to_string(),
            phase: RecoveryPhase::BackupComplete,
            home_identity: home_identity(&home).expect("home identity"),
            home_backup_dir: backup
                .file_name()
                .expect("backup name")
                .to_string_lossy()
                .into_owned(),
            backup_manifest_sha256: sha256_regular_nofollow(&backup.join("MANIFEST.sha256"))
                .expect("manifest digest"),
            staged_binary_name: String::new(),
            staged_sha256: String::new(),
            planned_workspaces: Vec::new(),
        };
        fs::write(root.join(format!(".{JOURNAL_PREFIX}stale.tmp")), b"stale")
            .expect("stale journal temp should write");

        write_journal(&root, &home, &journal).expect("journal should replace stale temp");

        assert_eq!(
            read_journal(&root).expect("journal should read"),
            Some(journal)
        );
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }
}
