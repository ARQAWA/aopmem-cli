//! Durable, resumable boundary around the RC8 database upgrade.
//!
//! The journal deliberately records state outside `AOPMEM_HOME`: a failed
//! home must never take its only recovery evidence with it.  It is a small
//! state machine, not a second migration engine.  The existing transactional
//! migration remains the only code that changes workspace databases.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
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

const JOURNAL_SCHEMA_VERSION: u32 = 1;
const TARGET_VERSION: &str = env!("CARGO_PKG_VERSION");
const JOURNAL_PREFIX: &str = "aopmem-upgrade-recovery-v1-";
const LEGACY_RC7_JOURNAL_PREFIX: &str = "aopmem-upgrade-recovery-v0.2.0-rc7-";
const STAGED_BINARY_NAME: &str = concat!(".aopmem-v", env!("CARGO_PKG_VERSION"), ".staged");
const RETAIN_TEMP_NAME: &str = concat!(".aopmem-v", env!("CARGO_PKG_VERSION"), ".retain.tmp");
const PUBLISH_TEMP_NAME: &str = concat!(".aopmem-v", env!("CARGO_PKG_VERSION"), ".publish.tmp");
const BACKUP_PREFIX: &str = "aopmem-upgrade-recovery-v1-r8-";
const LEGACY_RC7_BACKUP_PREFIX: &str = "aopmem-home-backup-v0.2.0-rc7-";
const MAX_BACKUP_ENTRIES: usize = 100_000;
const MAX_BACKUP_DIRECTORY_ENTRIES: usize = 10_000;
const MAX_BACKUP_DEPTH: usize = 128;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_JOURNAL_BYTES: usize = 1024 * 1024;
const HOME_MANIFEST_TEMP_PREFIX: &str = ".aopmem-home-manifest-";
const MAX_RECOVERY_PARENT_SCAN_ENTRIES: usize = 100_000;
const MAX_RECOVERY_TEMP_REMOVALS: usize = 128;
const RECOVERY_SQLITE_BACKUP_PAGE_BATCH: i32 = 256;
const RECOVERY_SQLITE_BUSY_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);
const RECOVERY_SQLITE_BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    pub journal_schema_version: u32,
    pub target_version: String,
    pub source_version: String,
    pub run_id: String,
    pub phase: RecoveryPhase,
    pub home_identity: String,
    pub safety_backup_root: Option<String>,
    pub recovery_backup_root: String,
    pub source_manifest_sha256: String,
    pub backup_manifest_sha256: String,
    pub staged_binary_name: String,
    pub staged_sha256: String,
    pub planned_workspaces: Vec<PlannedWorkspaceIdentity>,
    pub apply_attempts: u32,
    pub binary_replaced: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedWorkspaceIdentity {
    pub workspace_key: String,
    pub root_identity: String,
    pub database_identity: String,
    pub observability_identity: String,
    pub schema_before: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecoveryClassification {
    CleanNoEvidence,
    StalePreApplyBackup,
    MalformedPreApplyJournal,
    PreApplyPhaseGap,
    ActivePreApplyRun,
    ApplyStarted,
    AppliedNotPublished,
    PublishedComplete,
    UnknownApplyOutcome,
    LegacyHistoricalEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryEvidenceRun {
    pub run_id: String,
    pub journal_schema_version: Option<u32>,
    pub target_version: Option<String>,
    pub phase: Option<RecoveryPhase>,
    pub backup_root: Option<String>,
    pub apply_started: bool,
    pub apply_attempts: u32,
    pub binary_replaced: bool,
    pub evidence_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryInspectReport {
    pub classification: RecoveryClassification,
    pub active_run: Option<RecoveryEvidenceRun>,
    pub historical_runs: Vec<RecoveryEvidenceRun>,
    pub blocking_evidence: Vec<String>,
    pub ignored_pre_apply_evidence: Vec<String>,
    pub apply_started: bool,
    pub can_start_fresh: bool,
    pub can_resume_publish: bool,
    pub recommended_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryManifest {
    pub inventory_policy_version: u32,
    pub run_id: String,
    pub source_home_identity: String,
    pub created_at: String,
    pub entries: Vec<InventoryEntry>,
    pub excluded_counts_by_reason: BTreeMap<String, usize>,
    pub workspace_identities: Vec<WorkspaceManifestIdentity>,
    pub source_binary_version: String,
    pub source_binary_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub relative_path: String,
    pub entry_kind: InventoryEntryKind,
    pub size: u64,
    pub sha256: String,
    pub persistence: PersistenceClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryEntryKind {
    RegularFile,
    SqliteDatabase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceClass {
    Persistent,
    Ephemeral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceManifestIdentity {
    pub workspace_key: String,
    pub root_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryExecution {
    pub journal_phase: RecoveryPhase,
    pub run_id: String,
    pub recovery_backup_root: String,
    pub resumed: bool,
    pub apply_invoked: bool,
    pub binary_published: bool,
    pub durability_warning: bool,
    pub home_backup_retained: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply: Option<super::UpgradeApplyReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CreatedRecoveryBackup {
    path: PathBuf,
    source_manifest_sha256: String,
    backup_manifest_sha256: String,
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
    #[error("RECOVERY_STALE_PRE_APPLY_EVIDENCE: stale pre-apply recovery evidence was preserved; start a fresh verified recovery backup")]
    StalePreApplyEvidence,
    #[error("RECOVERY_JOURNAL_MALFORMED_PRE_APPLY: malformed pre-apply recovery journal was preserved; start a fresh verified recovery backup")]
    JournalMalformedPreApply,
    #[error("RECOVERY_PHASE_GAP_PRE_APPLY: pre-apply recovery phase gap was preserved; start a fresh verified recovery backup")]
    PhaseGapPreApply,
    #[error(
        "RECOVERY_APPLY_STARTED: apply-started recovery evidence exists; do not start a fresh run"
    )]
    ApplyStarted,
    #[error("RECOVERY_BACKUP_MANIFEST_MISMATCH: backup manifest does not match expected recovery evidence")]
    BackupManifestMismatch,
    #[error("RECOVERY_BACKUP_INCOMPLETE: recovery backup is incomplete")]
    BackupIncomplete,
    #[error(
        "RECOVERY_HOME_CHANGED_DURING_BACKUP: persistent home inventory changed during backup"
    )]
    HomeChangedDuringBackup,
    #[error("RECOVERY_LONG_PATH_FAILURE: long-path-safe recovery filesystem operation failed")]
    LongPathFailure,
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

pub fn inspect_recovery() -> Result<RecoveryInspectReport, RecoveryError> {
    let paths = storage::resolve_paths()?;
    let parent = paths.home().parent().ok_or(RecoveryError::InvalidJournal)?;
    inspect_recovery_parent(parent)
}

/// Adopts an installer-created sibling full-home backup after the RC8 binary
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
        if journal.recovery_backup_root
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
    let now = timestamp_utc()?;
    let journal = RecoveryJournal {
        journal_schema_version: JOURNAL_SCHEMA_VERSION,
        target_version: TARGET_VERSION.to_string(),
        source_version: current_binary_version(paths.bin()),
        run_id: backup_name
            .strip_prefix(BACKUP_PREFIX)
            .unwrap_or(&backup_name)
            .to_string(),
        phase: RecoveryPhase::BackupComplete,
        home_identity: home_identity(paths.home())?,
        safety_backup_root: None,
        recovery_backup_root: backup_name,
        source_manifest_sha256: manifest_sha256.clone(),
        backup_manifest_sha256: manifest_sha256,
        staged_binary_name: String::new(),
        staged_sha256: String::new(),
        planned_workspaces: Vec::new(),
        apply_attempts: 0,
        binary_replaced: false,
        created_at: now.clone(),
        updated_at: now,
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
    let journal_read = read_journal(parent);
    let (journal, resumed) = match journal_read {
        Ok(Some(journal)) => {
            validate_journal(&journal, paths.home(), parent)?;
            (journal, true)
        }
        Err(error) => {
            let inspect = inspect_recovery_parent(parent)?;
            if !inspect.can_start_fresh {
                return Err(error);
            }
            create_new_backup_journal(paths.home(), paths.bin(), parent, hooks)?
        }
        Ok(None) => create_new_backup_journal(paths.home(), paths.bin(), parent, hooks)?,
    };
    Ok(execution(
        &journal, parent, resumed, false, false, false, None,
    ))
}

fn create_new_backup_journal(
    home: &Path,
    bin: &Path,
    parent: &Path,
    hooks: &mut impl RecoveryHooks,
) -> Result<(RecoveryJournal, bool), RecoveryError> {
    let backup = create_full_home_backup_record(home, parent)?;
    hooks.checkpoint(RecoveryFaultPoint::BackupEffect)?;
    let backup_name = backup
        .path
        .file_name()
        .ok_or(RecoveryError::InvalidJournal)?
        .to_string_lossy()
        .into_owned();
    let now = timestamp_utc()?;
    let journal = RecoveryJournal {
        journal_schema_version: JOURNAL_SCHEMA_VERSION,
        target_version: TARGET_VERSION.to_string(),
        source_version: current_binary_version(bin),
        run_id: backup_name
            .strip_prefix(BACKUP_PREFIX)
            .unwrap_or(&backup_name)
            .to_string(),
        phase: RecoveryPhase::BackupComplete,
        home_identity: home_identity(home)?,
        safety_backup_root: None,
        recovery_backup_root: backup_name,
        source_manifest_sha256: backup.source_manifest_sha256,
        backup_manifest_sha256: backup.backup_manifest_sha256,
        staged_binary_name: String::new(),
        staged_sha256: String::new(),
        planned_workspaces: Vec::new(),
        apply_attempts: 0,
        binary_replaced: false,
        created_at: now.clone(),
        updated_at: now,
    };
    write_journal(parent, home, &journal)?;
    Ok((journal, false))
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
        touch_journal(&mut journal)?;
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
        touch_journal(&mut journal)?;
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
            touch_journal(&mut journal)?;
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
        journal.apply_attempts = journal
            .apply_attempts
            .checked_add(1)
            .ok_or(RecoveryError::InvalidJournal)?;
        touch_journal(&mut journal)?;
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
        touch_journal(&mut journal)?;
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
        journal.binary_replaced = true;
        touch_journal(&mut journal)?;
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
        run_id: journal.run_id.clone(),
        recovery_backup_root: parent
            .join(&journal.recovery_backup_root)
            .to_string_lossy()
            .into_owned(),
        resumed,
        apply_invoked,
        binary_published,
        durability_warning,
        home_backup_retained: parent.join(&journal.recovery_backup_root).is_dir(),
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

fn inspect_recovery_parent(parent: &Path) -> Result<RecoveryInspectReport, RecoveryError> {
    let root = AnchoredDir::open_workspace(parent, None)?;
    let mut current_journal_paths = Vec::new();
    let mut legacy_paths = Vec::new();
    let mut orphan_backups = Vec::new();
    let mut current_phases = BTreeSet::new();
    let mut legacy_apply_started = false;

    for name in directory_names(&root)? {
        let name_text = name.to_string_lossy().into_owned();
        let path_text = super::display_path(&parent.join(&name));
        if name_text.starts_with(BACKUP_PREFIX) || name_text.starts_with(LEGACY_RC7_BACKUP_PREFIX) {
            orphan_backups.push(path_text);
        } else if name_text.starts_with(JOURNAL_PREFIX) {
            if let Some(phase) = phase_from_journal_name(&name_text) {
                current_phases.insert(phase);
            }
            current_journal_paths.push(path_text);
        } else if name_text.starts_with(LEGACY_RC7_JOURNAL_PREFIX) {
            if name_text.contains("04-apply-started")
                || name_text.contains("05-applied")
                || name_text.contains("06-published")
            {
                legacy_apply_started = true;
            }
            legacy_paths.push(path_text);
        }
    }
    current_journal_paths.sort();
    legacy_paths.sort();
    orphan_backups.sort();

    let read = read_journal(parent);
    match read {
        Ok(Some(journal)) => {
            let evidence = RecoveryEvidenceRun {
                run_id: journal.run_id.clone(),
                journal_schema_version: Some(journal.journal_schema_version),
                target_version: Some(journal.target_version.clone()),
                phase: Some(journal.phase),
                backup_root: Some(super::display_path(
                    &parent.join(&journal.recovery_backup_root),
                )),
                apply_started: matches!(
                    journal.phase,
                    RecoveryPhase::ApplyStarted | RecoveryPhase::Applied | RecoveryPhase::Published
                ),
                apply_attempts: journal.apply_attempts,
                binary_replaced: journal.binary_replaced,
                evidence_paths: current_journal_paths.clone(),
            };
            let classification = match journal.phase {
                RecoveryPhase::BackupComplete
                | RecoveryPhase::StagedVerified
                | RecoveryPhase::Prepared => RecoveryClassification::ActivePreApplyRun,
                RecoveryPhase::ApplyStarted => RecoveryClassification::ApplyStarted,
                RecoveryPhase::Applied => RecoveryClassification::AppliedNotPublished,
                RecoveryPhase::Published => RecoveryClassification::PublishedComplete,
            };
            let can_start_fresh = matches!(
                classification,
                RecoveryClassification::PublishedComplete
                    | RecoveryClassification::LegacyHistoricalEvidence
            );
            let can_resume_publish = classification == RecoveryClassification::AppliedNotPublished;
            let blocking_evidence = if matches!(
                classification,
                RecoveryClassification::ApplyStarted
                    | RecoveryClassification::AppliedNotPublished
                    | RecoveryClassification::ActivePreApplyRun
            ) {
                current_journal_paths.clone()
            } else {
                Vec::new()
            };
            Ok(RecoveryInspectReport {
                classification,
                active_run: Some(evidence),
                historical_runs: legacy_evidence_runs(&legacy_paths, &orphan_backups),
                blocking_evidence,
                ignored_pre_apply_evidence: Vec::new(),
                apply_started: matches!(
                    journal.phase,
                    RecoveryPhase::ApplyStarted | RecoveryPhase::Applied | RecoveryPhase::Published
                ),
                can_start_fresh,
                can_resume_publish,
                recommended_action: recommended_action(classification).to_string(),
            })
        }
        Ok(None) => {
            let classification = if legacy_apply_started {
                RecoveryClassification::ApplyStarted
            } else if orphan_backups.is_empty() && legacy_paths.is_empty() {
                RecoveryClassification::CleanNoEvidence
            } else {
                RecoveryClassification::StalePreApplyBackup
            };
            let can_start_fresh = !legacy_apply_started;
            Ok(RecoveryInspectReport {
                classification,
                active_run: None,
                historical_runs: legacy_evidence_runs(&legacy_paths, &orphan_backups),
                blocking_evidence: if legacy_apply_started {
                    legacy_paths.clone()
                } else {
                    Vec::new()
                },
                ignored_pre_apply_evidence: if legacy_apply_started {
                    Vec::new()
                } else {
                    orphan_backups.clone()
                },
                apply_started: legacy_apply_started,
                can_start_fresh,
                can_resume_publish: false,
                recommended_action: recommended_action(classification).to_string(),
            })
        }
        Err(_) => {
            let has_apply_started = legacy_apply_started
                || current_phases.contains(&RecoveryPhase::ApplyStarted)
                || current_phases.contains(&RecoveryPhase::Applied)
                || current_phases.contains(&RecoveryPhase::Published);
            let has_gap = has_phase_gap(&current_phases);
            let classification = if has_apply_started {
                RecoveryClassification::ApplyStarted
            } else if has_gap {
                RecoveryClassification::PreApplyPhaseGap
            } else {
                RecoveryClassification::MalformedPreApplyJournal
            };
            let can_start_fresh = !has_apply_started;
            Ok(RecoveryInspectReport {
                classification,
                active_run: None,
                historical_runs: legacy_evidence_runs(&legacy_paths, &orphan_backups),
                blocking_evidence: if has_apply_started {
                    current_journal_paths
                        .iter()
                        .chain(legacy_paths.iter())
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                },
                ignored_pre_apply_evidence: if can_start_fresh {
                    current_journal_paths.clone()
                } else {
                    Vec::new()
                },
                apply_started: has_apply_started,
                can_start_fresh,
                can_resume_publish: false,
                recommended_action: recommended_action(classification).to_string(),
            })
        }
    }
}

fn legacy_evidence_runs(
    legacy_paths: &[String],
    orphan_backups: &[String],
) -> Vec<RecoveryEvidenceRun> {
    let mut runs = Vec::new();
    if !legacy_paths.is_empty() {
        runs.push(RecoveryEvidenceRun {
            run_id: "legacy-rc7-journals".to_string(),
            journal_schema_version: None,
            target_version: Some("0.2.0-rc7".to_string()),
            phase: None,
            backup_root: None,
            apply_started: legacy_paths.iter().any(|path| {
                path.contains("04-apply-started")
                    || path.contains("05-applied")
                    || path.contains("06-published")
            }),
            apply_attempts: 0,
            binary_replaced: false,
            evidence_paths: legacy_paths.to_vec(),
        });
    }
    for (index, backup) in orphan_backups.iter().enumerate() {
        runs.push(RecoveryEvidenceRun {
            run_id: format!("orphan-backup-{index}"),
            journal_schema_version: None,
            target_version: None,
            phase: None,
            backup_root: Some(backup.clone()),
            apply_started: false,
            apply_attempts: 0,
            binary_replaced: false,
            evidence_paths: vec![backup.clone()],
        });
    }
    runs
}

fn recommended_action(classification: RecoveryClassification) -> &'static str {
    match classification {
        RecoveryClassification::CleanNoEvidence => "start fresh recovery backup",
        RecoveryClassification::StalePreApplyBackup => {
            "preserve stale evidence and start fresh recovery backup"
        }
        RecoveryClassification::MalformedPreApplyJournal => {
            "preserve malformed pre-apply journal and start fresh recovery backup"
        }
        RecoveryClassification::PreApplyPhaseGap => {
            "preserve pre-apply phase gap and start fresh recovery backup"
        }
        RecoveryClassification::ActivePreApplyRun => "continue current pre-apply recovery run",
        RecoveryClassification::ApplyStarted => {
            "stop; preserve evidence; do not retry apply automatically"
        }
        RecoveryClassification::AppliedNotPublished => "resume publish only",
        RecoveryClassification::PublishedComplete => "treat as historical evidence",
        RecoveryClassification::UnknownApplyOutcome => {
            "stop; preserve evidence; manual recovery review required"
        }
        RecoveryClassification::LegacyHistoricalEvidence => "treat as historical evidence",
    }
}

fn phase_from_journal_name(name: &str) -> Option<RecoveryPhase> {
    [
        RecoveryPhase::BackupComplete,
        RecoveryPhase::StagedVerified,
        RecoveryPhase::Prepared,
        RecoveryPhase::ApplyStarted,
        RecoveryPhase::Applied,
        RecoveryPhase::Published,
    ]
    .into_iter()
    .find(|phase| journal_name(*phase) == name)
}

fn has_phase_gap(phases: &BTreeSet<RecoveryPhase>) -> bool {
    let ordered = [
        RecoveryPhase::BackupComplete,
        RecoveryPhase::StagedVerified,
        RecoveryPhase::Prepared,
        RecoveryPhase::ApplyStarted,
        RecoveryPhase::Applied,
        RecoveryPhase::Published,
    ];
    let mut missing_seen = false;
    for phase in ordered {
        if phases.contains(&phase) {
            if missing_seen {
                return true;
            }
        } else {
            missing_seen = true;
        }
    }
    false
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

fn touch_journal(journal: &mut RecoveryJournal) -> Result<(), RecoveryError> {
    journal.updated_at = timestamp_utc()?;
    Ok(())
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
        && previous.journal_schema_version == current.journal_schema_version
        && previous.target_version == current.target_version
        && previous.source_version == current.source_version
        && previous.run_id == current.run_id
        && previous.home_identity == current.home_identity
        && previous.safety_backup_root == current.safety_backup_root
        && previous.recovery_backup_root == current.recovery_backup_root
        && previous.source_manifest_sha256 == current.source_manifest_sha256
        && previous.backup_manifest_sha256 == current.backup_manifest_sha256
        && (previous.phase == RecoveryPhase::BackupComplete
            || (previous.staged_binary_name == current.staged_binary_name
                && previous.staged_sha256 == current.staged_sha256))
        && (matches!(
            previous.phase,
            RecoveryPhase::BackupComplete | RecoveryPhase::StagedVerified
        ) || previous.planned_workspaces == current.planned_workspaces)
        && current.apply_attempts >= previous.apply_attempts
        && (!previous.binary_replaced || current.binary_replaced)
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
    let temporary = RETAIN_TEMP_NAME;
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
    let temporary = PUBLISH_TEMP_NAME;
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

#[cfg(test)]
fn create_full_home_backup(home: &Path, parent: &Path) -> Result<PathBuf, RecoveryError> {
    Ok(create_full_home_backup_record(home, parent)?.path)
}

fn create_full_home_backup_record(
    home: &Path,
    parent: &Path,
) -> Result<CreatedRecoveryBackup, RecoveryError> {
    let mut last_error = None;
    for attempt in 0..2 {
        match create_full_home_backup_once(home, parent) {
            Ok(backup) => return Ok(backup),
            Err(RecoveryError::HomeChangedDuringBackup) if attempt == 0 => {
                last_error = Some(RecoveryError::HomeChangedDuringBackup);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or(RecoveryError::HomeChangedDuringBackup))
}

fn create_full_home_backup_once(
    home: &Path,
    parent: &Path,
) -> Result<CreatedRecoveryBackup, RecoveryError> {
    let run_id = short_run_id()?;
    let created_at = timestamp_utc()?;
    let source_binary_sha256 = installed_binary_sha256(home)?;
    let source = build_inventory(
        home,
        &run_id,
        &created_at,
        &current_binary_version(&home.join("bin")),
        source_binary_sha256.clone(),
        false,
    )?;
    let source_digest = inventory_digest(&source)?;
    let backup_name = format!("{BACKUP_PREFIX}{run_id}");
    let parent_root = AnchoredDir::open_workspace(parent, None)?;
    let backup_root = parent_root.create_new_child_dir_os(OsStr::new(&backup_name))?;
    copy_inventory(home, &parent.join(&backup_name), &source.entries)?;

    let after = build_inventory(
        home,
        &run_id,
        &created_at,
        &current_binary_version(&home.join("bin")),
        source_binary_sha256,
        false,
    )?;
    if !inventory_entries_equivalent(&source.entries, &after.entries, false) {
        return Err(RecoveryError::HomeChangedDuringBackup);
    }

    let backup_inventory = build_inventory(
        &parent.join(&backup_name),
        &run_id,
        &created_at,
        &source.source_binary_version,
        source.source_binary_sha256.clone(),
        true,
    )?;
    if !inventory_entries_equivalent(&source.entries, &backup_inventory.entries, true) {
        return Err(RecoveryError::BackupIncomplete);
    }

    write_json_manifest(&backup_root, &backup_inventory, &source_digest)?;
    write_legacy_manifest(&backup_root)?;
    let backup_path = parent.join(&backup_name);
    let backup_manifest_sha256 = sha256_regular_nofollow(&backup_path.join("MANIFEST.sha256"))?;
    backup_root.sync()?;
    parent_root.sync()?;
    Ok(CreatedRecoveryBackup {
        path: backup_path,
        source_manifest_sha256: source_digest,
        backup_manifest_sha256,
    })
}

fn short_run_id() -> Result<String, RecoveryError> {
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).map_err(|_| RecoveryError::InvalidJournal)?;
    let mut id = String::from("r8-");
    for byte in random {
        use std::fmt::Write as _;
        write!(id, "{byte:02x}").map_err(|_| RecoveryError::InvalidJournal)?;
    }
    Ok(id)
}

fn build_inventory(
    home: &Path,
    run_id: &str,
    created_at: &str,
    source_binary_version: &str,
    source_binary_sha256: Option<String>,
    backup_root: bool,
) -> Result<InventoryManifest, RecoveryError> {
    let root = AnchoredDir::open_workspace(home, None)?;
    let mut entries = Vec::new();
    let mut excluded_counts_by_reason = BTreeMap::new();
    collect_inventory(
        &root,
        Path::new(""),
        &mut entries,
        &mut excluded_counts_by_reason,
        0,
        backup_root,
    )?;
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(InventoryManifest {
        inventory_policy_version: 1,
        run_id: run_id.to_string(),
        source_home_identity: root.stable_identity_token().map_err(RecoveryError::Io)?,
        created_at: created_at.to_string(),
        entries,
        excluded_counts_by_reason,
        workspace_identities: workspace_manifest_identities(&root)?,
        source_binary_version: source_binary_version.to_string(),
        source_binary_sha256,
    })
}

fn collect_inventory(
    directory: &AnchoredDir,
    relative: &Path,
    entries: &mut Vec<InventoryEntry>,
    excluded_counts_by_reason: &mut BTreeMap<String, usize>,
    depth: usize,
    backup_root: bool,
) -> Result<(), RecoveryError> {
    if depth > MAX_BACKUP_DEPTH {
        return Err(RecoveryError::BackupIncomplete);
    }
    for name in directory_names(directory)? {
        let child_relative = relative.join(&name);
        let relative_text = relative_path_text(&child_relative)?;
        if backup_root && matches!(relative_text.as_str(), "MANIFEST.sha256" | "MANIFEST.json") {
            continue;
        }
        if let Some(reason) = excluded_inventory_reason(&relative_text) {
            *excluded_counts_by_reason
                .entry(reason.to_string())
                .or_default() += 1;
            continue;
        }
        if let Ok(child) = directory.child_dir_os(&name, false) {
            collect_inventory(
                &child,
                &child_relative,
                entries,
                excluded_counts_by_reason,
                depth + 1,
                backup_root,
            )?;
            continue;
        }
        let file = directory.open_regular_os(&name)?;
        let metadata = file.metadata().map_err(RecoveryError::Io)?;
        if !metadata.is_file() {
            return Err(RecoveryError::BackupIncomplete);
        }
        let kind = if is_workspace_database(&relative_text) {
            InventoryEntryKind::SqliteDatabase
        } else {
            InventoryEntryKind::RegularFile
        };
        entries.push(InventoryEntry {
            relative_path: relative_text,
            entry_kind: kind,
            size: metadata.len(),
            sha256: sha256_reader(file).map_err(RecoveryError::Io)?,
            persistence: PersistenceClass::Persistent,
        });
    }
    Ok(())
}

fn workspace_manifest_identities(
    root: &AnchoredDir,
) -> Result<Vec<WorkspaceManifestIdentity>, RecoveryError> {
    let workspaces = match root.child_dir_os(OsStr::new("workspaces"), false) {
        Ok(workspaces) => workspaces,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(RecoveryError::Io(error)),
    };
    let mut identities = Vec::new();
    for name in directory_names(&workspaces)? {
        if let Ok(workspace) = workspaces.child_dir_os(&name, false) {
            identities.push(WorkspaceManifestIdentity {
                workspace_key: name.to_string_lossy().into_owned(),
                root_identity: workspace
                    .stable_identity_token()
                    .map_err(RecoveryError::Io)?,
            });
        }
    }
    identities.sort_by(|left, right| left.workspace_key.cmp(&right.workspace_key));
    Ok(identities)
}

fn excluded_inventory_reason(relative: &str) -> Option<&'static str> {
    let parts = relative.split('/').collect::<Vec<_>>();
    if parts.len() == 3 && parts[0] == "workspaces" && parts[2] == ".mutation.lock" {
        return Some("workspace_mutation_lock_ephemeral");
    }
    if parts.len() == 3
        && parts[0] == "workspaces"
        && matches!(parts[2], "aopmem.sqlite-wal" | "aopmem.sqlite-shm")
    {
        return Some("sqlite_sidecar_captured_by_online_backup");
    }
    if relative.starts_with("bin/.aopmem-v") && relative.ends_with(".tmp") {
        return Some("product_temporary_publish_file");
    }
    None
}

fn is_workspace_database(relative: &str) -> bool {
    let parts = relative.split('/').collect::<Vec<_>>();
    parts.len() == 3 && parts[0] == "workspaces" && parts[2] == "aopmem.sqlite"
}

fn inventory_entries_equivalent(
    left: &[InventoryEntry],
    right: &[InventoryEntry],
    allow_sqlite_transform: bool,
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.relative_path == right.relative_path
                && left.entry_kind == right.entry_kind
                && (allow_sqlite_transform && left.entry_kind == InventoryEntryKind::SqliteDatabase
                    || (left.size == right.size && left.sha256 == right.sha256))
        })
}

fn inventory_digest(manifest: &InventoryManifest) -> Result<String, RecoveryError> {
    let bytes = serde_json::to_vec(manifest).map_err(|_| RecoveryError::InvalidJournal)?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn copy_inventory(
    source_home: &Path,
    backup_home: &Path,
    entries: &[InventoryEntry],
) -> Result<(), RecoveryError> {
    let source_root = AnchoredDir::open_workspace(source_home, None)?;
    let backup_root = AnchoredDir::open_workspace(backup_home, None)?;
    for entry in entries {
        let relative = Path::new(&entry.relative_path);
        if entry.entry_kind == InventoryEntryKind::SqliteDatabase {
            copy_sqlite_database(source_home, backup_home, relative)?;
        } else {
            copy_regular_inventory_file(&source_root, &backup_root, relative)?;
        }
    }
    backup_root.sync()?;
    Ok(())
}

fn copy_regular_inventory_file(
    source_root: &AnchoredDir,
    backup_root: &AnchoredDir,
    relative: &Path,
) -> Result<(), RecoveryError> {
    let source_file = open_relative_regular(source_root, relative)?;
    let metadata = source_file.metadata().map_err(RecoveryError::Io)?;
    let (destination_parent, final_name) = ensure_relative_parent(backup_root, relative)?;
    let temporary = format!(".backup-{}.tmp", uuid::Uuid::new_v4().hyphenated());
    let mut output = destination_parent.create_new_regular(&temporary)?;
    let mut input = source_file;
    io::copy(&mut input, &mut output).map_err(RecoveryError::Io)?;
    output
        .set_permissions(metadata.permissions())
        .map_err(RecoveryError::Io)?;
    require_recovery_publish(
        "backup_file",
        publish_regular(
            &destination_parent,
            output,
            OsStr::new(&temporary),
            &final_name,
            PublishMode::NoReplace,
        ),
    )
}

fn copy_sqlite_database(
    source_home: &Path,
    backup_home: &Path,
    relative: &Path,
) -> Result<(), RecoveryError> {
    let source_path = source_home.join(relative);
    let final_path = backup_home.join(relative);
    let destination_parent_path = final_path
        .parent()
        .ok_or(RecoveryError::BackupIncomplete)?
        .to_path_buf();
    let backup_root = AnchoredDir::open_workspace(backup_home, None)?;
    let source_root = AnchoredDir::open_workspace(source_home, None)?;
    drop(open_relative_regular(&source_root, relative)?);
    let _ = ensure_relative_parent(&backup_root, relative)?;
    let expected_schema = inspect_recovery_database(&source_path)?;
    let source = Connection::open_with_flags(&source_path, sqlite_read_only_flags())
        .map_err(|error| RecoveryError::Io(sqlite_io(error)))?;
    online_backup_recovery_database(source, &source_path, &destination_parent_path, &final_path)?;
    let final_schema = inspect_recovery_database(&final_path)?;
    if final_schema != expected_schema {
        return Err(RecoveryError::BackupIncomplete);
    }
    Ok(())
}

fn inspect_recovery_database(path: &Path) -> Result<super::WorkspaceSchemaPlan, RecoveryError> {
    let connection = Connection::open_with_flags(path, sqlite_read_only_flags())
        .map_err(|error| RecoveryError::Io(sqlite_io(error)))?;
    connection
        .execute_batch("PRAGMA query_only = ON; PRAGMA temp_store = MEMORY;")
        .map_err(|error| RecoveryError::Io(sqlite_io(error)))?;
    let schema = super::inspect_schema(&connection).map_err(|error| {
        RecoveryError::Io(io::Error::new(io::ErrorKind::InvalidData, error.message))
    })?;
    close_connection_recovery(connection)?;
    Ok(schema)
}

fn online_backup_recovery_database(
    source: Connection,
    source_path: &Path,
    destination_dir: &Path,
    final_path: &Path,
) -> Result<(), RecoveryError> {
    let destination_root = AnchoredDir::open_workspace(destination_dir, None)?;
    let final_name = final_path
        .file_name()
        .ok_or(RecoveryError::BackupIncomplete)?;
    let temporary = format!(
        ".{}.backup-{}.tmp",
        final_name.to_string_lossy(),
        uuid::Uuid::new_v4().hyphenated()
    );
    let file = destination_root.create_new_regular(&temporary)?;
    drop(file);
    let temporary_path = destination_dir.join(&temporary);
    let mut destination = Connection::open_with_flags(
        &temporary_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(|error| RecoveryError::Io(sqlite_io(error)))?;
    destination
        .execute_batch("PRAGMA synchronous = FULL; PRAGMA journal_mode = DELETE;")
        .map_err(|error| RecoveryError::Io(sqlite_io(error)))?;
    run_recovery_sqlite_backup(&source, &mut destination)?;
    close_connection_recovery(destination)?;
    close_connection_recovery(source)?;
    inspect_recovery_database(&temporary_path)?;
    let temporary_file = destination_root
        .open_regular_for_update_os(OsStr::new(&temporary))
        .map_err(RecoveryError::Io)?;
    temporary_file.sync_all().map_err(RecoveryError::Io)?;
    require_recovery_publish(
        "backup_file",
        publish_regular(
            &destination_root,
            temporary_file,
            OsStr::new(&temporary),
            final_name,
            PublishMode::NoReplace,
        ),
    )?;
    if destination_root.open_regular_os(final_name).is_err() {
        return Err(RecoveryError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "SQLite backup was not published for {}",
                source_path.display()
            ),
        )));
    }
    Ok(())
}

fn sqlite_read_only_flags() -> OpenFlags {
    OpenFlags::SQLITE_OPEN_READ_ONLY
}

fn run_recovery_sqlite_backup(
    source: &Connection,
    destination: &mut Connection,
) -> Result<(), RecoveryError> {
    let backup =
        Backup::new(source, destination).map_err(|error| RecoveryError::Io(sqlite_io(error)))?;
    let started = std::time::Instant::now();
    loop {
        match backup
            .step(RECOVERY_SQLITE_BACKUP_PAGE_BATCH)
            .map_err(|error| RecoveryError::Io(sqlite_io(error)))?
        {
            StepResult::Done => break,
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => {
                if started.elapsed() >= RECOVERY_SQLITE_BUSY_TIMEOUT {
                    return Err(RecoveryError::Io(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "SQLite Online Backup remained busy for 30 seconds",
                    )));
                }
                std::thread::sleep(RECOVERY_SQLITE_BUSY_PAUSE);
            }
            _ => {
                return Err(RecoveryError::Io(io::Error::other(
                    "SQLite Online Backup returned an unknown state",
                )));
            }
        }
    }
    drop(backup);
    Ok(())
}

fn close_connection_recovery(connection: Connection) -> Result<(), RecoveryError> {
    connection.close().map_err(|(connection, error)| {
        drop(connection);
        RecoveryError::Io(sqlite_io(error))
    })
}

fn ensure_relative_parent(
    root: &AnchoredDir,
    relative: &Path,
) -> Result<(AnchoredDir, OsString), RecoveryError> {
    let final_name = relative
        .file_name()
        .ok_or(RecoveryError::BackupIncomplete)?
        .to_os_string();
    let mut directory = root.clone();
    if let Some(parent) = relative.parent() {
        for component in parent.components() {
            let std::path::Component::Normal(name) = component else {
                return Err(RecoveryError::BackupIncomplete);
            };
            directory = directory
                .child_dir_os(name, true)
                .map_err(RecoveryError::Io)?;
        }
    }
    Ok((directory, final_name))
}

fn write_json_manifest(
    backup_root: &AnchoredDir,
    manifest: &InventoryManifest,
    source_manifest_sha256: &str,
) -> Result<(), RecoveryError> {
    let temporary = ".MANIFEST.json.tmp";
    let mut bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "source_manifest_sha256": source_manifest_sha256,
        "backup_manifest": manifest,
    }))
    .map_err(|_| RecoveryError::InvalidJournal)?;
    bytes.push(b'\n');
    let mut file = backup_root.create_new_regular(temporary)?;
    file.write_all(&bytes).map_err(RecoveryError::Io)?;
    file.sync_all().map_err(RecoveryError::Io)?;
    require_recovery_publish(
        "backup_manifest",
        publish_regular(
            backup_root,
            file,
            OsStr::new(temporary),
            OsStr::new("MANIFEST.json"),
            PublishMode::NoReplace,
        ),
    )
}

fn write_legacy_manifest(backup_root: &AnchoredDir) -> Result<(), RecoveryError> {
    let temporary = ".MANIFEST.sha256.tmp";
    let mut bytes = Vec::new();
    let mut entries = 0_usize;
    write_tree_manifest(backup_root, Path::new(""), &mut bytes, &mut entries, 0)?;
    let mut file = backup_root.create_new_regular(temporary)?;
    file.write_all(&bytes).map_err(RecoveryError::Io)?;
    file.sync_all().map_err(RecoveryError::Io)?;
    require_recovery_publish(
        "backup_manifest",
        publish_regular(
            backup_root,
            file,
            OsStr::new(temporary),
            OsStr::new("MANIFEST.sha256"),
            PublishMode::NoReplace,
        ),
    )
}

fn installed_binary_sha256(home: &Path) -> Result<Option<String>, RecoveryError> {
    let path = if cfg!(windows) {
        home.join("bin/aopmem.exe")
    } else {
        home.join("bin/aopmem")
    };
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => sha256_regular_nofollow(&path).map(Some),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RecoveryError::Io(error)),
    }
}

fn relative_path_text(path: &Path) -> Result<String, RecoveryError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(RecoveryError::BackupIncomplete);
        };
        let text = name.to_str().ok_or(RecoveryError::BackupIncomplete)?;
        if text.contains(['\n', '\r']) {
            return Err(RecoveryError::BackupIncomplete);
        }
        parts.push(text);
    }
    Ok(parts.join("/"))
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
    manifest: &mut impl Write,
    entries: &mut usize,
    depth: usize,
) -> Result<(), RecoveryError> {
    if depth > MAX_BACKUP_DEPTH {
        return Err(RecoveryError::InvalidJournal);
    }
    let before = directory_names(source)?;
    for name in &before {
        if relative.as_os_str().is_empty()
            && matches!(
                name.to_str(),
                Some("MANIFEST.sha256") | Some("MANIFEST.json")
            )
        {
            continue;
        }
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

#[allow(dead_code)]
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
    #[cfg(windows)]
    let read_path = crate::windows_path::verbatim_path(directory.logical_path())
        .map_err(|_| RecoveryError::LongPathFailure)?;
    #[cfg(not(windows))]
    let read_path = directory.logical_path().to_path_buf();
    let mut names = fs::read_dir(read_path)
        .map_err(|error| {
            if cfg!(windows) && error.raw_os_error() == Some(206) {
                RecoveryError::LongPathFailure
            } else {
                RecoveryError::Io(error)
            }
        })?
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

fn sqlite_io(error: rusqlite::Error) -> io::Error {
    match error {
        rusqlite::Error::SqliteFailure(inner, message) => io::Error::other(format!(
            "SQLite error {:?}: {}",
            inner.code,
            message.unwrap_or_default()
        )),
        other => io::Error::other(other),
    }
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

fn timestamp_utc() -> Result<String, RecoveryError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RecoveryError::InvalidJournal)?
        .as_secs();
    Ok(format!("unix:{seconds}"))
}

fn current_binary_version(bin: &Path) -> String {
    let destination = if cfg!(windows) {
        bin.join("aopmem.exe")
    } else {
        bin.join("aopmem")
    };
    fs::metadata(&destination)
        .ok()
        .filter(|metadata| metadata.is_file())
        .map(|_| env!("CARGO_PKG_VERSION").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn validate_journal(
    journal: &RecoveryJournal,
    home: &Path,
    parent: &Path,
) -> Result<(), RecoveryError> {
    if journal.journal_schema_version != JOURNAL_SCHEMA_VERSION
        || journal.target_version != TARGET_VERSION
        || journal.run_id.is_empty()
        || journal.home_identity != home_identity(home)?
        || !valid_direct_name(&journal.recovery_backup_root)
        || !(journal.recovery_backup_root.starts_with(BACKUP_PREFIX)
            || journal
                .recovery_backup_root
                .starts_with(LEGACY_RC7_BACKUP_PREFIX))
        || !is_sha256(&journal.source_manifest_sha256)
        || !is_sha256(&journal.backup_manifest_sha256)
    {
        return Err(RecoveryError::InvalidJournal);
    }
    let backup = parent.join(&journal.recovery_backup_root);
    let manifest = backup.join("MANIFEST.sha256");
    if sha256_regular_nofollow(&manifest)? != journal.backup_manifest_sha256 {
        return Err(RecoveryError::BackupManifestMismatch);
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
        if relative.as_os_str().is_empty()
            && matches!(
                name.to_str(),
                Some("MANIFEST.sha256") | Some("MANIFEST.json")
            )
        {
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

    fn journal_fixture(
        phase: RecoveryPhase,
        home_identity: String,
        backup_name: String,
        manifest_sha256: String,
        staged_binary_name: String,
        staged_sha256: String,
        planned_workspaces: Vec<PlannedWorkspaceIdentity>,
    ) -> RecoveryJournal {
        RecoveryJournal {
            journal_schema_version: JOURNAL_SCHEMA_VERSION,
            target_version: TARGET_VERSION.to_string(),
            source_version: "test-source".to_string(),
            run_id: backup_name
                .strip_prefix(BACKUP_PREFIX)
                .unwrap_or(&backup_name)
                .to_string(),
            phase,
            home_identity,
            safety_backup_root: None,
            recovery_backup_root: backup_name,
            source_manifest_sha256: manifest_sha256.clone(),
            backup_manifest_sha256: manifest_sha256,
            staged_binary_name,
            staged_sha256,
            planned_workspaces,
            apply_attempts: 0,
            binary_replaced: false,
            created_at: "unix:0".to_string(),
            updated_at: "unix:0".to_string(),
        }
    }

    #[test]
    fn durable_journal_round_trips_through_atomic_publish() {
        let root = temp_dir("journal");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        fs::write(home.join("value"), b"preserved").expect("home value should write");
        let backup = create_full_home_backup(&home, &root).expect("backup should create");
        let journal = journal_fixture(
            RecoveryPhase::BackupComplete,
            home_identity(&home).expect("home identity"),
            backup
                .file_name()
                .expect("backup name")
                .to_string_lossy()
                .into_owned(),
            sha256_regular_nofollow(&backup.join("MANIFEST.sha256")).expect("manifest digest"),
            String::new(),
            String::new(),
            Vec::new(),
        );
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
        let journal = journal_fixture(
            RecoveryPhase::Prepared,
            "home".to_string(),
            format!("{BACKUP_PREFIX}backup"),
            "0".repeat(64),
            STAGED_BINARY_NAME.to_string(),
            "1".repeat(64),
            vec![PlannedWorkspaceIdentity {
                workspace_key: "alpha".to_string(),
                root_identity: "x".repeat(MAX_JOURNAL_BYTES),
                database_identity: "database".to_string(),
                observability_identity: "observability".to_string(),
                schema_before: "003".to_string(),
            }],
        );
        assert!(matches!(
            serialize_journal(&journal),
            Err(RecoveryError::InvalidJournal)
        ));
    }

    #[test]
    fn journal_rejects_missing_first_or_middle_checkpoint() {
        let root = temp_dir("journal-gap");
        let base = journal_fixture(
            RecoveryPhase::BackupComplete,
            "home".to_string(),
            format!("{BACKUP_PREFIX}backup"),
            "0".repeat(64),
            String::new(),
            String::new(),
            Vec::new(),
        );
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
    fn recovery_inspect_classifies_orphan_backup_as_stale_pre_apply() {
        let root = temp_dir("inspect-orphan");
        let home = root.join("home");
        fs::create_dir(&home).expect("home should create");
        fs::write(home.join("value"), b"preserved").expect("home value should write");
        let backup = create_full_home_backup(&home, &root).expect("orphan backup should create");

        let report = inspect_recovery_parent(&root).expect("inspect should classify");

        assert_eq!(
            report.classification,
            RecoveryClassification::StalePreApplyBackup
        );
        assert!(report.can_start_fresh);
        assert!(!report.apply_started);
        assert!(report
            .ignored_pre_apply_evidence
            .iter()
            .any(|path| path.contains(backup.file_name().unwrap().to_string_lossy().as_ref())));
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn recovery_inspect_malformed_pre_apply_starts_fresh_but_apply_started_blocks() {
        let root = temp_dir("inspect-malformed");
        fs::write(
            root.join(journal_name(RecoveryPhase::BackupComplete)),
            b"not-json",
        )
        .expect("malformed journal should write");
        let report = inspect_recovery_parent(&root).expect("inspect should classify malformed");
        assert_eq!(
            report.classification,
            RecoveryClassification::MalformedPreApplyJournal
        );
        assert!(report.can_start_fresh);
        assert!(!report.apply_started);

        fs::remove_file(root.join(journal_name(RecoveryPhase::BackupComplete)))
            .expect("malformed journal should remove");
        fs::write(
            root.join(journal_name(RecoveryPhase::ApplyStarted)),
            b"not-json",
        )
        .expect("apply-started journal should write");
        let report = inspect_recovery_parent(&root).expect("inspect should classify apply-started");
        assert_eq!(report.classification, RecoveryClassification::ApplyStarted);
        assert!(!report.can_start_fresh);
        assert!(report.apply_started);
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn recovery_backup_inventory_preserves_runtime_tools_and_excludes_ephemeral_sidecars() {
        let root = temp_dir("inventory-policy");
        let home = root.join("home");
        let workspace = home.join("workspaces/alpha");
        fs::create_dir_all(workspace.join("runtimes/tool/.venv/Lib/site-packages/pkg"))
            .expect("runtime tree should create");
        fs::create_dir_all(workspace.join("tools/tool-a")).expect("tool tree should create");
        fs::write(
            workspace.join("runtimes/tool/.venv/Lib/site-packages/pkg/module.py"),
            b"runtime",
        )
        .expect("runtime file should write");
        fs::write(workspace.join("tools/tool-a/tool.json"), b"tool")
            .expect("tool file should write");
        fs::write(workspace.join(".pending-snapshot"), b"pending")
            .expect("pending marker should write");
        fs::write(workspace.join(".mutation.lock"), b"").expect("lock should write");
        fs::write(workspace.join("aopmem.sqlite-wal"), b"wal").expect("wal should write");
        fs::write(workspace.join("aopmem.sqlite-shm"), b"shm").expect("shm should write");

        let backup = create_full_home_backup(&home, &root).expect("backup should create");

        assert!(backup
            .join("workspaces/alpha/runtimes/tool/.venv/Lib/site-packages/pkg/module.py")
            .is_file());
        assert!(backup
            .join("workspaces/alpha/tools/tool-a/tool.json")
            .is_file());
        assert!(backup.join("workspaces/alpha/.pending-snapshot").is_file());
        assert!(!backup.join("workspaces/alpha/.mutation.lock").exists());
        assert!(!backup.join("workspaces/alpha/aopmem.sqlite-wal").exists());
        assert!(!backup.join("workspaces/alpha/aopmem.sqlite-shm").exists());
        fs::remove_dir_all(root).expect("temporary directory should remove");
    }

    #[test]
    fn safety_backup_name_is_not_a_normal_adopt_source() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let root = temp_dir("safety-backup-adopt");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("home should create");
        fs::write(home.join("value.txt"), b"preserved").expect("home value should write");
        let _guard = EnvGuard::set(&home);

        for name in [
            "aopmem-home-backup-v0.2.0-rc8-fixture",
            "aopmem-home-backup-v0.2.0-rc7-fixture",
        ] {
            let safety = root.join(name);
            fs::create_dir_all(&safety).expect("safety backup should create");
            fs::write(safety.join("value.txt"), b"preserved").expect("safety value should write");
            let safety_root =
                AnchoredDir::open_workspace(&safety, None).expect("safety root should open");
            write_legacy_manifest(&safety_root).expect("safety manifest should write");
            let digest = sha256_regular_nofollow(&safety.join("MANIFEST.sha256"))
                .expect("manifest digest should compute");

            assert!(matches!(
                adopt_home_backup(&safety, &digest),
                Err(RecoveryError::InvalidJournal)
            ));
        }
        assert_eq!(read_journal(&root).expect("journal read should work"), None);
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
        let journal = journal_fixture(
            RecoveryPhase::Prepared,
            "identity".to_string(),
            format!("{BACKUP_PREFIX}backup"),
            "0".repeat(64),
            staged
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned(),
            "not-the-file".to_string(),
            Vec::new(),
        );
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
        fs::write(bin.join(RETAIN_TEMP_NAME), b"stale").expect("stale temp should write");
        let resumed =
            retain_staged_binary(&source, &bin, &digest).expect("resume should be idempotent");

        assert_eq!(resumed, retained);
        assert_eq!(
            sha256_regular_nofollow(&retained).expect("retained digest"),
            digest
        );
        assert!(
            !bin.join(RETAIN_TEMP_NAME).exists(),
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
        fs::write(bin.join(PUBLISH_TEMP_NAME), b"stale").expect("stale publish temp should write");

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
            !bin.join(PUBLISH_TEMP_NAME).exists(),
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
        let journal = journal_fixture(
            RecoveryPhase::BackupComplete,
            home_identity(&home).expect("home identity"),
            backup
                .file_name()
                .expect("backup name")
                .to_string_lossy()
                .into_owned(),
            sha256_regular_nofollow(&backup.join("MANIFEST.sha256")).expect("manifest digest"),
            String::new(),
            String::new(),
            Vec::new(),
        );
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
        let journal = journal_fixture(
            RecoveryPhase::BackupComplete,
            home_identity(&home).expect("home identity"),
            backup
                .file_name()
                .expect("backup name")
                .to_string_lossy()
                .into_owned(),
            sha256_regular_nofollow(&backup.join("MANIFEST.sha256")).expect("manifest digest"),
            String::new(),
            String::new(),
            Vec::new(),
        );
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
