use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;

use super::{
    backup::online_backup_to_path, enumerate_workspace_entries, installed_binary_name,
    nearest_existing_directory, path_with_suffix, plan_all_workspaces_with_probe,
    regular_file_size, validate_existing_root, validate_optional_managed_directory, DiskSpacePlan,
    DiskSpaceProbe, SystemDiskSpaceProbe, WorkspaceSchemaPlan, DATABASE_FILE_NAME,
    DATABASE_SIDECAR_SUFFIXES,
};
use crate::adapter;
use crate::audit::{self, AnchoredDir, PendingSnapshotMarker};
use crate::mutation;
use crate::observability::{
    CollectorEvent, CountItem, CountsPayload, EventOutcome, EventPayload, EventType, LocalCollector,
};
use crate::output::OutputWarning;
use crate::schema;
use crate::storage::{self, AopmemPaths, WorkspacePaths};
use crate::verify;

const SOURCE_VERSION: &str = "0.1.0-rc3";
const TARGET_VERSION: &str = "0.2.0-rc3";
const BACKUPS_DIRECTORY: &str = "backups";
const BACKUP_RUN_PREFIX: &str = "upgrade-0.2.0-rc3-";
const COMMAND_ID: &str = "upgrade_apply";

const OWNED_GLOBAL_ASSETS: &[OwnedGlobalAsset] = &[
    OwnedGlobalAsset {
        area: GlobalAssetArea::Skills,
        directory: "memory-keeper",
        file_name: "SKILL.md",
        bytes: include_bytes!("../../templates/skills/memory-keeper/SKILL.md"),
    },
    OwnedGlobalAsset {
        area: GlobalAssetArea::Templates,
        directory: "managed-block",
        file_name: "AGENTS.managed-block.md",
        bytes: include_bytes!("../../templates/managed-block/AGENTS.managed-block.md"),
    },
    OwnedGlobalAsset {
        area: GlobalAssetArea::Templates,
        directory: "understand-docs",
        file_name: "SCHEMA.md",
        bytes: include_bytes!("../../templates/understand-docs/SCHEMA.md"),
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeApplyReport {
    pub apply_only: bool,
    pub source_version: &'static str,
    pub target_version: &'static str,
    pub current_binary_version: &'static str,
    pub scope: &'static str,
    pub success: bool,
    pub binary_replaced: bool,
    pub backup_root: Option<String>,
    pub disk_space: DiskSpacePlan,
    pub global_steps: GlobalApplySteps,
    pub workspaces: Vec<WorkspaceApplyReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<UpgradeApplyFailure>,
    pub restoration: ApplyRestorationReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeApplyExecution {
    pub report: UpgradeApplyReport,
    pub warnings: Vec<OutputWarning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<UpgradeApplyFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeApplyFailure {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_key: Option<String>,
}

#[derive(Debug, Error)]
pub enum UpgradeApplyError {
    #[error(transparent)]
    Plan(#[from] super::UpgradePlanError),
    #[error("cannot resolve current repository: {0}")]
    CurrentRepository(#[source] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GlobalApplySteps {
    pub binary_backup: ApplyStep,
    pub adapter_backup: ApplyStep,
    pub owned_assets_backup: ApplyStep,
    pub owned_assets_refresh: ApplyStep,
    pub adapter_sync: ApplyStep,
}

impl Default for GlobalApplySteps {
    fn default() -> Self {
        Self {
            binary_backup: ApplyStep::not_started(),
            adapter_backup: ApplyStep::not_started(),
            owned_assets_backup: ApplyStep::not_started(),
            owned_assets_refresh: ApplyStep::not_started(),
            adapter_sync: ApplyStep::not_started(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceApplyReport {
    pub workspace_key: String,
    pub workspace_path: String,
    pub status: WorkspaceApplyStatus,
    pub database_backup: ApplyStep,
    pub schema_before: Option<WorkspaceSchemaPlan>,
    pub schema_after: Option<WorkspaceSchemaPlan>,
    pub migration: ApplyStep,
    pub audit_snapshot: ApplyStep,
    pub observability: ApplyStep,
    pub doctor: WorkspaceCheckStep,
    pub verify: WorkspaceCheckStep,
    pub restoration: WorkspaceRestorationReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceApplyStatus {
    NotStarted,
    Applied,
    AlreadyCurrent,
    Failed,
    Restored,
    RestoreFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyStep {
    pub status: ApplyStepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<UpgradeApplyFailure>,
}

impl ApplyStep {
    fn not_started() -> Self {
        Self {
            status: ApplyStepStatus::NotStarted,
            path: None,
            bytes: None,
            detail: None,
            error: None,
        }
    }

    fn completed(path: Option<&Path>, bytes: Option<u64>, detail: impl Into<String>) -> Self {
        Self {
            status: ApplyStepStatus::Completed,
            path: path.map(display_path),
            bytes,
            detail: Some(detail.into()),
            error: None,
        }
    }

    fn failed(failure: UpgradeApplyFailure) -> Self {
        Self {
            status: ApplyStepStatus::Failed,
            path: None,
            bytes: None,
            detail: None,
            error: Some(failure),
        }
    }

    fn not_applicable(detail: impl Into<String>) -> Self {
        Self {
            status: ApplyStepStatus::NotApplicable,
            path: None,
            bytes: None,
            detail: Some(detail.into()),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyStepStatus {
    NotStarted,
    Completed,
    Failed,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceCheckStep {
    pub status: ApplyStepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctor: Option<verify::DoctorReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<verify::LintReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<UpgradeApplyFailure>,
}

impl WorkspaceCheckStep {
    fn not_started() -> Self {
        Self {
            status: ApplyStepStatus::NotStarted,
            doctor: None,
            verify: None,
            detail: None,
            error: None,
        }
    }

    fn not_applicable() -> Self {
        Self {
            status: ApplyStepStatus::NotApplicable,
            doctor: None,
            verify: None,
            detail: Some("current repository path is not stored for this workspace".to_string()),
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ApplyRestorationReport {
    pub attempted: bool,
    pub completed: bool,
    pub database_failures: Vec<UpgradeApplyFailure>,
    pub adapter_restored: bool,
    pub owned_assets_restored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_error: Option<UpgradeApplyFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owned_assets_error: Option<UpgradeApplyFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct WorkspaceRestorationReport {
    pub transaction_rolled_back: bool,
    pub backup_restore_attempted: bool,
    pub backup_restore_completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<UpgradeApplyFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplyFaultPoint {
    BeforeBinaryBackup,
    BeforeWorkspaceMigration,
    AfterWorkspaceBackup,
    AfterMigrationBeforeSchemaInspect,
    BeforeMigrationRollback,
    BeforeRollbackRecoveryCheck,
    AfterWorkspaceCommit,
    BeforeOwnedAssetsRefresh,
    BeforeAdapterSync,
    BeforeDoctor,
}

trait ApplyFaultInjector {
    fn check(&self, _point: ApplyFaultPoint, _workspace_key: Option<&str>) -> io::Result<()> {
        Ok(())
    }
}

struct NoFaults;

impl ApplyFaultInjector for NoFaults {}

#[derive(Debug, Clone)]
struct WorkspaceCandidate {
    key: String,
    paths: WorkspacePaths,
}

#[derive(Debug, Clone)]
struct CurrentRepoContext {
    root: PathBuf,
    workspace_key: String,
    instruction_file: PathBuf,
}

struct HealthCheckFailure {
    failure: UpgradeApplyFailure,
    doctor: Option<verify::DoctorReport>,
    verify: Option<verify::LintReport>,
}

type HealthCheckResult =
    Result<(verify::DoctorReport, verify::LintReport), Box<HealthCheckFailure>>;

#[derive(Debug, Clone)]
struct BackupRun {
    root: PathBuf,
    binary_dir: PathBuf,
    adapter_dir: PathBuf,
    assets_dir: PathBuf,
    workspaces_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct FileBackupState {
    original_path: PathBuf,
    backup_path: Option<PathBuf>,
    existed: bool,
}

#[derive(Debug, Error)]
enum BackedUpStateError {
    #[error("managed file changed after its durable backup: {path}")]
    Changed { path: PathBuf },
    #[error("cannot validate managed file {path}: {source}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone)]
struct GlobalAssetState {
    target: PathBuf,
    backup: FileBackupState,
    replacement: &'static [u8],
}

#[derive(Debug, Clone, Copy)]
enum GlobalAssetArea {
    Skills,
    Templates,
}

struct OwnedGlobalAsset {
    area: GlobalAssetArea,
    directory: &'static str,
    file_name: &'static str,
    bytes: &'static [u8],
}

#[derive(Debug)]
struct WorkspaceMigrationResult {
    backup_path: PathBuf,
    backup_bytes: u64,
    schema_before: WorkspaceSchemaPlan,
    schema_after: WorkspaceSchemaPlan,
    mutated: bool,
    snapshot_step: ApplyStep,
    snapshot_duration_ms: Option<u64>,
    snapshot_bytes: Option<u64>,
    snapshot_pending: bool,
    warning: Option<OutputWarning>,
}

struct WorkspaceMigrationFailure {
    failure: UpgradeApplyFailure,
    report: Box<WorkspaceApplyReport>,
}

fn workspace_migration_failure(
    failure: UpgradeApplyFailure,
    report: WorkspaceApplyReport,
) -> WorkspaceMigrationFailure {
    WorkspaceMigrationFailure {
        failure,
        report: Box::new(report),
    }
}

pub fn apply_all_workspaces() -> Result<UpgradeApplyExecution, UpgradeApplyError> {
    apply_all_workspaces_with(&SystemDiskSpaceProbe, &NoFaults)
}

fn apply_all_workspaces_with(
    probe: &dyn DiskSpaceProbe,
    faults: &dyn ApplyFaultInjector,
) -> Result<UpgradeApplyExecution, UpgradeApplyError> {
    let plan = plan_all_workspaces_with_probe(probe)?;
    let paths = storage::resolve_paths().map_err(super::UpgradePlanError::from)?;
    validate_existing_root(paths.home())?;
    validate_optional_managed_directory(paths.home(), paths.workspaces())?;
    let candidates = workspace_candidates(&paths)?;
    let current_repo = current_repo_context(&paths, &candidates)?;
    let mut report = initial_report(&plan.disk_space, &candidates, current_repo.as_ref());

    if let Some(failure) = preflight_failure(&plan, &candidates) {
        return Ok(failed_execution_with_observability(
            report,
            failure,
            &candidates,
        ));
    }

    let required_bytes = apply_required_bytes(&paths, &candidates, current_repo.as_ref())?;
    let disk_probe_path = nearest_existing_directory(paths.home())?;
    let available_bytes = probe.available_bytes(&disk_probe_path).map_err(|source| {
        UpgradeApplyError::Plan(super::UpgradePlanError::DiskSpace {
            path: disk_probe_path.clone(),
            source,
        })
    })?;
    report.disk_space.probe_path = display_path(&disk_probe_path);
    report.disk_space.available_bytes = available_bytes;
    report.disk_space.minimum_required_bytes = required_bytes;
    report.disk_space.workspace_database_backup_bytes = workspace_backup_bytes(&candidates)?;
    report.disk_space.sufficient = report.disk_space.available_bytes >= required_bytes;
    if !report.disk_space.sufficient {
        let failure = failure(
            "INSUFFICIENT_DISK_SPACE",
            format!(
                "upgrade requires at least {required_bytes} free bytes, found {}",
                report.disk_space.available_bytes
            ),
            None,
        );
        return Ok(failed_execution_with_observability(
            report,
            failure,
            &candidates,
        ));
    }

    if let Some(context) = current_repo.as_ref() {
        if let Err(error) = preflight_adapter(context) {
            let failure = failure(
                "ADAPTER_DRIFT",
                error.to_string(),
                Some(&context.workspace_key),
            );
            return Ok(failed_execution_with_observability(
                report,
                failure,
                &candidates,
            ));
        }
    }

    let backup_run = match create_backup_run(&paths) {
        Ok(backup_run) => backup_run,
        Err(error) => {
            let failure = failure("BACKUP_CREATE_FAILED", error.to_string(), None);
            return Ok(failed_execution_with_observability(
                report,
                failure,
                &candidates,
            ));
        }
    };
    report.backup_root = Some(display_path(&backup_run.root));

    if let Err(error) = faults.check(ApplyFaultPoint::BeforeBinaryBackup, None) {
        let failure = failure("OLD_BINARY_BACKUP_FAILED", error.to_string(), None);
        report.global_steps.binary_backup = ApplyStep::failed(failure.clone());
        return Ok(failed_execution_with_observability(
            report,
            failure,
            &candidates,
        ));
    }
    let binary_state = match backup_installed_binary(&paths, &backup_run) {
        Ok(state) => {
            report.global_steps.binary_backup = backup_step(&state, "installed binary backup");
            state
        }
        Err(error) => {
            let failure = failure("OLD_BINARY_BACKUP_FAILED", error.to_string(), None);
            report.global_steps.binary_backup = ApplyStep::failed(failure.clone());
            return Ok(failed_execution_with_observability(
                report,
                failure,
                &candidates,
            ));
        }
    };
    let _preserved_binary = binary_state;

    let adapter_state = match backup_adapter(current_repo.as_ref(), &backup_run) {
        Ok(Some(state)) => {
            report.global_steps.adapter_backup = backup_step(&state, "adapter exact-byte backup");
            Some(state)
        }
        Ok(None) => {
            report.global_steps.adapter_backup =
                ApplyStep::not_applicable("current repository has no matching AOPMem workspace");
            None
        }
        Err(error) => {
            let failure = failure("ADAPTER_BACKUP_FAILED", error.to_string(), None);
            report.global_steps.adapter_backup = ApplyStep::failed(failure.clone());
            return Ok(failed_execution_with_observability(
                report,
                failure,
                &candidates,
            ));
        }
    };

    let asset_states = match backup_owned_assets(&paths, &backup_run) {
        Ok(states) => {
            report.global_steps.owned_assets_backup = ApplyStep::completed(
                Some(&backup_run.assets_dir),
                None,
                format!(
                    "{} owned assets backed up; unrelated files untouched",
                    states.len()
                ),
            );
            states
        }
        Err(error) => {
            let failure = failure("OWNED_ASSET_BACKUP_FAILED", error.to_string(), None);
            report.global_steps.owned_assets_backup = ApplyStep::failed(failure.clone());
            return Ok(failed_execution_with_observability(
                report,
                failure,
                &candidates,
            ));
        }
    };

    let mut warnings = Vec::new();
    let mut mutated_indexes = Vec::new();
    let mut collectors = Vec::with_capacity(candidates.len());
    for (index, candidate) in candidates.iter().enumerate() {
        if let Err(error) = faults.check(
            ApplyFaultPoint::BeforeWorkspaceMigration,
            Some(&candidate.key),
        ) {
            let failure = failure(
                "INJECTED_UPGRADE_FAILURE",
                error.to_string(),
                Some(&candidate.key),
            );
            report.workspaces[index].status = WorkspaceApplyStatus::Failed;
            report.workspaces[index].migration = ApplyStep::failed(failure.clone());
            let mut collector = LocalCollector::new(&candidate.paths, COMMAND_ID).ok();
            record_update_event(
                &mut collector,
                EventType::UpdateStarted,
                EventOutcome::Started,
                None,
                &mut warnings,
            );
            collectors.push(collector);
            finish_failed_apply(
                &mut report,
                failure.clone(),
                &candidates,
                &mutated_indexes,
                adapter_state.as_ref(),
                &asset_states,
                false,
                false,
            );
            record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
            return Ok(UpgradeApplyExecution {
                report,
                warnings,
                failure: Some(failure),
            });
        }

        match backup_and_migrate_workspace(candidate, &backup_run, faults) {
            Ok(result) => {
                let workspace_report = &mut report.workspaces[index];
                workspace_report.database_backup = ApplyStep::completed(
                    Some(&result.backup_path),
                    Some(result.backup_bytes),
                    "WAL-safe SQLite Online Backup completed under mutation lock",
                );
                workspace_report.schema_before = Some(result.schema_before.clone());
                workspace_report.schema_after = Some(result.schema_after.clone());
                workspace_report.migration = ApplyStep::completed(
                    None,
                    None,
                    if result.mutated {
                        "pending migrations committed transactionally"
                    } else {
                        "schema already current; no migration write"
                    },
                );
                workspace_report.audit_snapshot = result.snapshot_step;
                workspace_report.status = if result.mutated {
                    mutated_indexes.push(index);
                    WorkspaceApplyStatus::Applied
                } else {
                    WorkspaceApplyStatus::AlreadyCurrent
                };
                if let Some(warning) = result.warning {
                    warnings.push(warning);
                }
                let mut collector = LocalCollector::new(&candidate.paths, COMMAND_ID).ok();
                let observability_ready = record_update_event(
                    &mut collector,
                    EventType::UpdateStarted,
                    EventOutcome::Started,
                    None,
                    &mut warnings,
                );
                workspace_report.observability = if observability_ready {
                    ApplyStep::completed(
                        Some(candidate.paths.observability_db()),
                        None,
                        "observability schema v1 validated; update.started recorded",
                    )
                } else {
                    push_warning_once(&mut warnings, observability_warning());
                    ApplyStep::completed(
                        None,
                        None,
                        "collector unavailable; core migration status preserved",
                    )
                };
                record_audit_snapshot_event(
                    &mut collector,
                    result.snapshot_duration_ms,
                    result.snapshot_bytes,
                    result.snapshot_pending,
                    &mut warnings,
                );
                collectors.push(collector);

                if let Err(error) =
                    faults.check(ApplyFaultPoint::AfterWorkspaceCommit, Some(&candidate.key))
                {
                    let failure = failure(
                        "INJECTED_LATE_UPGRADE_FAILURE",
                        error.to_string(),
                        Some(&candidate.key),
                    );
                    finish_failed_apply(
                        &mut report,
                        failure.clone(),
                        &candidates,
                        &mutated_indexes,
                        adapter_state.as_ref(),
                        &asset_states,
                        false,
                        false,
                    );
                    record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
                    return Ok(UpgradeApplyExecution {
                        report,
                        warnings,
                        failure: Some(failure),
                    });
                }
            }
            Err(error) => {
                let WorkspaceMigrationFailure {
                    failure,
                    report: partial,
                } = error;
                report.workspaces[index] = *partial;
                finish_failed_apply(
                    &mut report,
                    failure.clone(),
                    &candidates,
                    &mutated_indexes,
                    adapter_state.as_ref(),
                    &asset_states,
                    false,
                    false,
                );
                let mut collector = LocalCollector::new(&candidate.paths, COMMAND_ID).ok();
                record_update_event(
                    &mut collector,
                    EventType::UpdateStarted,
                    EventOutcome::Started,
                    None,
                    &mut warnings,
                );
                collectors.push(collector);
                record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
                return Ok(UpgradeApplyExecution {
                    report,
                    warnings,
                    failure: Some(failure),
                });
            }
        }
    }

    if let Err(error) = faults.check(ApplyFaultPoint::BeforeOwnedAssetsRefresh, None) {
        let failure = failure("OWNED_ASSET_REFRESH_FAILED", error.to_string(), None);
        report.global_steps.owned_assets_refresh = ApplyStep::failed(failure.clone());
        finish_failed_apply(
            &mut report,
            failure.clone(),
            &candidates,
            &mutated_indexes,
            adapter_state.as_ref(),
            &asset_states,
            false,
            false,
        );
        record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
        return Ok(UpgradeApplyExecution {
            report,
            warnings,
            failure: Some(failure),
        });
    }
    if let Err(error) = validate_backed_up_states_unchanged(
        &asset_states
            .iter()
            .map(|state| &state.backup)
            .collect::<Vec<_>>(),
    ) {
        let failure = failure("OWNED_ASSET_CHANGED_SINCE_BACKUP", error.to_string(), None);
        report.global_steps.owned_assets_refresh = ApplyStep::failed(failure.clone());
        finish_failed_apply(
            &mut report,
            failure.clone(),
            &candidates,
            &mutated_indexes,
            adapter_state.as_ref(),
            &asset_states,
            false,
            false,
        );
        record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
        return Ok(UpgradeApplyExecution {
            report,
            warnings,
            failure: Some(failure),
        });
    }
    if let Err(error) = refresh_owned_assets(&asset_states) {
        let failure = failure("OWNED_ASSET_REFRESH_FAILED", error.to_string(), None);
        report.global_steps.owned_assets_refresh = ApplyStep::failed(failure.clone());
        finish_failed_apply(
            &mut report,
            failure.clone(),
            &candidates,
            &mutated_indexes,
            adapter_state.as_ref(),
            &asset_states,
            false,
            true,
        );
        record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
        return Ok(UpgradeApplyExecution {
            report,
            warnings,
            failure: Some(failure),
        });
    }
    report.global_steps.owned_assets_refresh = ApplyStep::completed(
        None,
        None,
        format!("{} owned skills/templates refreshed", asset_states.len()),
    );

    let adapter_changed =
        if let (Some(context), Some(state)) = (current_repo.as_ref(), adapter_state.as_ref()) {
            if let Err(error) = faults.check(
                ApplyFaultPoint::BeforeAdapterSync,
                Some(&context.workspace_key),
            ) {
                let failure = failure(
                    "ADAPTER_SYNC_FAILED",
                    error.to_string(),
                    Some(&context.workspace_key),
                );
                report.global_steps.adapter_sync = ApplyStep::failed(failure.clone());
                finish_failed_apply(
                    &mut report,
                    failure.clone(),
                    &candidates,
                    &mutated_indexes,
                    Some(state),
                    &asset_states,
                    false,
                    true,
                );
                record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
                return Ok(UpgradeApplyExecution {
                    report,
                    warnings,
                    failure: Some(failure),
                });
            }
            if let Err(error) = validate_backed_up_state_unchanged(state) {
                let failure = failure(
                    "ADAPTER_CHANGED_SINCE_BACKUP",
                    error.to_string(),
                    Some(&context.workspace_key),
                );
                report.global_steps.adapter_sync = ApplyStep::failed(failure.clone());
                finish_failed_apply(
                    &mut report,
                    failure.clone(),
                    &candidates,
                    &mutated_indexes,
                    Some(state),
                    &asset_states,
                    false,
                    true,
                );
                record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
                return Ok(UpgradeApplyExecution {
                    report,
                    warnings,
                    failure: Some(failure),
                });
            }
            match sync_adapter(context) {
                Ok(changed) => {
                    report.global_steps.adapter_sync = ApplyStep::completed(
                        Some(&context.instruction_file),
                        None,
                        if changed {
                            "adapter managed block synchronized"
                        } else {
                            "adapter already synchronized"
                        },
                    );
                    changed
                }
                Err(error) => {
                    let failure = failure(
                        "ADAPTER_SYNC_FAILED",
                        error.to_string(),
                        Some(&context.workspace_key),
                    );
                    report.global_steps.adapter_sync = ApplyStep::failed(failure.clone());
                    finish_failed_apply(
                        &mut report,
                        failure.clone(),
                        &candidates,
                        &mutated_indexes,
                        Some(state),
                        &asset_states,
                        true,
                        true,
                    );
                    record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
                    return Ok(UpgradeApplyExecution {
                        report,
                        warnings,
                        failure: Some(failure),
                    });
                }
            }
        } else {
            report.global_steps.adapter_sync =
                ApplyStep::not_applicable("current repository has no matching AOPMem workspace");
            false
        };

    if let Some(context) = current_repo.as_ref() {
        if let Err(error) =
            faults.check(ApplyFaultPoint::BeforeDoctor, Some(&context.workspace_key))
        {
            let failure = failure(
                "DOCTOR_FAILED",
                error.to_string(),
                Some(&context.workspace_key),
            );
            finish_failed_apply(
                &mut report,
                failure.clone(),
                &candidates,
                &mutated_indexes,
                adapter_state.as_ref(),
                &asset_states,
                adapter_changed,
                true,
            );
            record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
            return Ok(UpgradeApplyExecution {
                report,
                warnings,
                failure: Some(failure),
            });
        }
        let index = candidates
            .iter()
            .position(|candidate| candidate.key == context.workspace_key)
            .expect("current repository context is built only from candidates");
        match run_health_checks(context) {
            Ok((doctor, verify)) => {
                let doctor_detail = if doctor.healthy {
                    "doctor passed"
                } else {
                    "doctor completed; only the committed audit snapshot warning remains"
                };
                let verify_detail = if verify.clean {
                    "verify passed"
                } else {
                    "verify completed; only the committed audit snapshot warning remains"
                };
                report.workspaces[index].doctor = WorkspaceCheckStep {
                    status: ApplyStepStatus::Completed,
                    doctor: Some(doctor),
                    verify: None,
                    detail: Some(doctor_detail.to_string()),
                    error: None,
                };
                report.workspaces[index].verify = WorkspaceCheckStep {
                    status: ApplyStepStatus::Completed,
                    doctor: None,
                    verify: Some(verify),
                    detail: Some(verify_detail.to_string()),
                    error: None,
                };
            }
            Err(error) => {
                let HealthCheckFailure {
                    failure,
                    doctor,
                    verify: verify_report,
                } = *error;
                report.workspaces[index].doctor = WorkspaceCheckStep {
                    status: if doctor.is_some() {
                        ApplyStepStatus::Completed
                    } else {
                        ApplyStepStatus::Failed
                    },
                    doctor,
                    verify: None,
                    detail: None,
                    error: Some(failure.clone()),
                };
                report.workspaces[index].verify = WorkspaceCheckStep {
                    status: if verify_report.is_some() {
                        ApplyStepStatus::Completed
                    } else {
                        ApplyStepStatus::Failed
                    },
                    doctor: None,
                    verify: verify_report,
                    detail: None,
                    error: Some(failure.clone()),
                };
                finish_failed_apply(
                    &mut report,
                    failure.clone(),
                    &candidates,
                    &mutated_indexes,
                    adapter_state.as_ref(),
                    &asset_states,
                    adapter_changed,
                    true,
                );
                record_failed_updates(&candidates, &mut collectors, &mut warnings, &failure);
                return Ok(UpgradeApplyExecution {
                    report,
                    warnings,
                    failure: Some(failure),
                });
            }
        }
    }

    for collector in &mut collectors {
        record_update_event(
            collector,
            EventType::UpdateCompleted,
            EventOutcome::Success,
            None,
            &mut warnings,
        );
    }
    report.success = true;
    Ok(UpgradeApplyExecution {
        report,
        warnings,
        failure: None,
    })
}

fn initial_report(
    disk_space: &DiskSpacePlan,
    candidates: &[WorkspaceCandidate],
    current_repo: Option<&CurrentRepoContext>,
) -> UpgradeApplyReport {
    let workspaces = candidates
        .iter()
        .map(|candidate| WorkspaceApplyReport {
            workspace_key: candidate.key.clone(),
            workspace_path: display_path(candidate.paths.root()),
            status: WorkspaceApplyStatus::NotStarted,
            database_backup: ApplyStep::not_started(),
            schema_before: None,
            schema_after: None,
            migration: ApplyStep::not_started(),
            audit_snapshot: ApplyStep::not_started(),
            observability: ApplyStep::not_started(),
            doctor: if current_repo.is_some_and(|repo| repo.workspace_key == candidate.key) {
                WorkspaceCheckStep::not_started()
            } else {
                WorkspaceCheckStep::not_applicable()
            },
            verify: if current_repo.is_some_and(|repo| repo.workspace_key == candidate.key) {
                WorkspaceCheckStep::not_started()
            } else {
                WorkspaceCheckStep::not_applicable()
            },
            restoration: WorkspaceRestorationReport::default(),
        })
        .collect();
    UpgradeApplyReport {
        apply_only: true,
        source_version: SOURCE_VERSION,
        target_version: TARGET_VERSION,
        current_binary_version: env!("CARGO_PKG_VERSION"),
        scope: "all_workspaces",
        success: false,
        binary_replaced: false,
        backup_root: None,
        disk_space: disk_space.clone(),
        global_steps: GlobalApplySteps::default(),
        workspaces,
        stopped_workspace: None,
        stop_reason: None,
        restoration: ApplyRestorationReport::default(),
    }
}

fn workspace_candidates(paths: &AopmemPaths) -> Result<Vec<WorkspaceCandidate>, UpgradeApplyError> {
    let entries = enumerate_workspace_entries(paths)?;
    let mut candidates = Vec::with_capacity(entries.len());
    for entry in entries {
        let key =
            entry
                .file_name()
                .into_string()
                .map_err(|_| super::UpgradePlanError::InspectPath {
                    path: entry.path(),
                    source: io::Error::new(
                        io::ErrorKind::InvalidData,
                        "workspace name is not Unicode",
                    ),
                })?;
        candidates.push(WorkspaceCandidate {
            paths: storage::workspace_paths_for_key(paths, &key),
            key,
        });
    }
    Ok(candidates)
}

fn current_repo_context(
    paths: &AopmemPaths,
    candidates: &[WorkspaceCandidate],
) -> Result<Option<CurrentRepoContext>, UpgradeApplyError> {
    let root =
        storage::resolve_current_workspace_root().map_err(UpgradeApplyError::CurrentRepository)?;
    let key = storage::resolve_workspace_key(paths, &root)
        .map_err(|error| UpgradeApplyError::CurrentRepository(io::Error::other(error)))?;
    if !candidates.iter().any(|candidate| candidate.key == key) {
        return Ok(None);
    }
    Ok(Some(CurrentRepoContext {
        instruction_file: adapter::default_instruction_file(&root),
        root,
        workspace_key: key,
    }))
}

fn preflight_failure(
    plan: &super::UpgradePlanReport,
    candidates: &[WorkspaceCandidate],
) -> Option<UpgradeApplyFailure> {
    if plan.workspaces.len() != candidates.len() {
        return Some(failure(
            "WORKSPACE_SET_CHANGED",
            "workspace set changed after upgrade plan; rerun upgrade plan/apply",
            None,
        ));
    }
    for (planned, candidate) in plan.workspaces.iter().zip(candidates) {
        // Plan and apply intentionally fail closed while any SQLite sidecar is
        // present. A normal WAL may contain committed recovery data, so an
        // immutable plan must never ignore it and apply must not guess whether
        // it is safe to checkpoint. The caller must quiesce SQLite first.
        if planned.workspace_key != candidate.key
            || planned.workspace_path != display_path(candidate.paths.root())
            || planned.database_path != display_path(candidate.paths.db())
        {
            return Some(failure(
                "WORKSPACE_SET_CHANGED",
                "workspace identity or path changed after upgrade plan; rerun upgrade plan/apply",
                Some(&candidate.key),
            ));
        }
        if let Some(error) = &planned.error {
            return Some(failure(error.code, &error.message, Some(&candidate.key)));
        }
        if let Err(error) = storage::validate_workspace_mutation_paths(&candidate.paths) {
            return Some(failure(
                "UNSAFE_WORKSPACE_PATH",
                error.to_string(),
                Some(&candidate.key),
            ));
        }
        match audit::has_pending_snapshot(candidate.paths.audit_git()) {
            Ok(false) => {}
            Ok(true) => {
                return Some(failure(
                    "PENDING_AUDIT_SNAPSHOT",
                    "workspace has a pending audit snapshot",
                    Some(&candidate.key),
                ));
            }
            Err(error) => {
                return Some(failure(
                    "AUDIT_STATUS_FAILED",
                    error.to_string(),
                    Some(&candidate.key),
                ));
            }
        }
        for suffix in DATABASE_SIDECAR_SUFFIXES {
            let sidecar = path_with_suffix(candidate.paths.db(), suffix);
            if let Ok(metadata) = fs::symlink_metadata(&sidecar) {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Some(failure(
                        "UNSAFE_DATABASE_SIDECAR",
                        format!(
                            "database sidecar is not a real regular file: {}",
                            display_path(&sidecar)
                        ),
                        Some(&candidate.key),
                    ));
                }
            }
        }
    }
    None
}

fn apply_required_bytes(
    paths: &AopmemPaths,
    candidates: &[WorkspaceCandidate],
    current_repo: Option<&CurrentRepoContext>,
) -> Result<u64, UpgradeApplyError> {
    let mut required = workspace_backup_bytes(candidates)?;
    required = checked_add(required, binary_source_size(paths)?)?;
    if let Some(context) = current_repo {
        required = checked_add(
            required,
            regular_file_size(&context.instruction_file).unwrap_or_default(),
        )?;
    }
    for asset in OWNED_GLOBAL_ASSETS {
        let target = owned_asset_path(paths, asset);
        required = checked_add(required, regular_file_size(&target).unwrap_or_default())?;
        required = checked_add(
            required,
            u64::try_from(asset.bytes.len()).map_err(|_| super::UpgradePlanError::SizeOverflow)?,
        )?;
    }
    Ok(required)
}

fn workspace_backup_bytes(candidates: &[WorkspaceCandidate]) -> Result<u64, UpgradeApplyError> {
    let mut bytes = 0_u64;
    for candidate in candidates {
        bytes = checked_add(
            bytes,
            regular_file_size(candidate.paths.db()).unwrap_or_default(),
        )?;
        for suffix in DATABASE_SIDECAR_SUFFIXES {
            bytes = checked_add(
                bytes,
                regular_file_size(&path_with_suffix(candidate.paths.db(), suffix))
                    .unwrap_or_default(),
            )?;
        }
    }
    Ok(bytes)
}

fn checked_add(left: u64, right: u64) -> Result<u64, UpgradeApplyError> {
    left.checked_add(right).ok_or(UpgradeApplyError::Plan(
        super::UpgradePlanError::SizeOverflow,
    ))
}

fn binary_source_size(paths: &AopmemPaths) -> Result<u64, UpgradeApplyError> {
    Ok(regular_file_size(&paths.bin().join(installed_binary_name())).unwrap_or_default())
}

fn preflight_adapter(context: &CurrentRepoContext) -> Result<(), adapter::SeedError> {
    storage::validate_optional_regular_file(&context.instruction_file)?;
    let existing = match fs::read(&context.instruction_file) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    adapter::prepare_instruction_sync(existing.as_deref()).map(|_| ())
}

fn create_backup_run(paths: &AopmemPaths) -> io::Result<BackupRun> {
    storage::ensure_owned_direct_directory(paths.home(), &paths.home().join(BACKUPS_DIRECTORY))?;
    let backups = paths.home().join(BACKUPS_DIRECTORY);
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| io::Error::other("random backup id failed"))?;
    let mut id = String::with_capacity(32);
    for byte in random {
        use std::fmt::Write as _;
        write!(id, "{byte:02x}").map_err(io::Error::other)?;
    }
    let root = backups.join(format!("{BACKUP_RUN_PREFIX}{id}"));
    storage::ensure_owned_direct_directory(&backups, &root)?;
    let binary_dir = root.join("binary");
    let adapter_dir = root.join("adapter");
    let assets_dir = root.join("owned-assets");
    let workspaces_dir = root.join("workspaces");
    for directory in [&binary_dir, &adapter_dir, &assets_dir, &workspaces_dir] {
        storage::ensure_owned_direct_directory(&root, directory)?;
    }
    AnchoredDir::open_workspace(&root, None)?.sync()?;
    Ok(BackupRun {
        root,
        binary_dir,
        adapter_dir,
        assets_dir,
        workspaces_dir,
    })
}

fn backup_installed_binary(paths: &AopmemPaths, run: &BackupRun) -> io::Result<FileBackupState> {
    let source = paths.bin().join(installed_binary_name());
    backup_optional_file(
        &source,
        &run.binary_dir,
        installed_binary_name(),
        "binary-receipt.json",
    )
}

fn backup_adapter(
    context: Option<&CurrentRepoContext>,
    run: &BackupRun,
) -> io::Result<Option<FileBackupState>> {
    context
        .map(|context| {
            backup_optional_file(
                &context.instruction_file,
                &run.adapter_dir,
                "AGENTS.md",
                "adapter-receipt.json",
            )
        })
        .transpose()
}

fn backup_owned_assets(paths: &AopmemPaths, run: &BackupRun) -> io::Result<Vec<GlobalAssetState>> {
    let mut states = Vec::with_capacity(OWNED_GLOBAL_ASSETS.len());
    for (index, asset) in OWNED_GLOBAL_ASSETS.iter().enumerate() {
        let target = owned_asset_path(paths, asset);
        let backup = backup_optional_file(
            &target,
            &run.assets_dir,
            &format!("asset-{index}.bin"),
            &format!("asset-{index}-receipt.json"),
        )?;
        states.push(GlobalAssetState {
            target,
            backup,
            replacement: asset.bytes,
        });
    }
    Ok(states)
}

fn backup_optional_file(
    source: &Path,
    destination_dir: &Path,
    destination_name: &str,
    receipt_name: &str,
) -> io::Result<FileBackupState> {
    let parent = source
        .parent()
        .ok_or_else(|| io::Error::other("backup source has no parent"))?;
    let name = source
        .file_name()
        .ok_or_else(|| io::Error::other("backup source has no name"))?;
    let source_file = match fs::symlink_metadata(parent) {
        Ok(_) => AnchoredDir::open_workspace(parent, None)?.open_regular_optional_os(name)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let destination = AnchoredDir::open_workspace(destination_dir, None)?;
    let backup_path = if let Some(mut source_file) = source_file {
        let mut target = destination.create_new_regular_os(OsStr::new(destination_name))?;
        io::copy(&mut source_file, &mut target)?;
        target.sync_all()?;
        destination.sync()?;
        Some(destination_dir.join(destination_name))
    } else {
        None
    };
    let receipt = serde_json::to_vec(&serde_json::json!({
        "source_path": display_path(source),
        "existed": backup_path.is_some(),
        "backup_path": backup_path.as_ref().map(|path| display_path(path)),
    }))
    .map_err(io::Error::other)?;
    write_new_durable(&destination, receipt_name, &receipt)?;
    Ok(FileBackupState {
        original_path: source.to_path_buf(),
        existed: backup_path.is_some(),
        backup_path,
    })
}

fn backup_step(state: &FileBackupState, detail: &str) -> ApplyStep {
    let bytes = state
        .backup_path
        .as_ref()
        .and_then(|path| fs::metadata(path).ok())
        .map(|metadata| metadata.len());
    ApplyStep::completed(
        state.backup_path.as_deref(),
        bytes,
        if state.existed {
            detail.to_string()
        } else {
            format!("{detail}; source absent and absence receipt persisted")
        },
    )
}

fn backup_and_migrate_workspace(
    candidate: &WorkspaceCandidate,
    run: &BackupRun,
    faults: &dyn ApplyFaultInjector,
) -> Result<WorkspaceMigrationResult, WorkspaceMigrationFailure> {
    let mut partial = WorkspaceApplyReport {
        workspace_key: candidate.key.clone(),
        workspace_path: display_path(candidate.paths.root()),
        status: WorkspaceApplyStatus::Failed,
        database_backup: ApplyStep::not_started(),
        schema_before: None,
        schema_after: None,
        migration: ApplyStep::not_started(),
        audit_snapshot: ApplyStep::not_started(),
        observability: ApplyStep::not_started(),
        doctor: WorkspaceCheckStep::not_applicable(),
        verify: WorkspaceCheckStep::not_applicable(),
        restoration: WorkspaceRestorationReport::default(),
    };
    let workspace_backup_dir = run.workspaces_dir.join(&candidate.key);
    if let Err(error) =
        storage::ensure_owned_direct_directory(&run.workspaces_dir, &workspace_backup_dir)
    {
        let failure = failure(
            "WORKSPACE_BACKUP_FAILED",
            error.to_string(),
            Some(&candidate.key),
        );
        partial.database_backup = ApplyStep::failed(failure.clone());
        return Err(workspace_migration_failure(failure, partial));
    }
    let backup_path = workspace_backup_dir.join(DATABASE_FILE_NAME);

    let identity = match storage::validate_workspace_mutation_paths(&candidate.paths) {
        Ok(identity) => identity,
        Err(error) => {
            let failure = failure(
                "UNSAFE_WORKSPACE_PATH",
                error.to_string(),
                Some(&candidate.key),
            );
            partial.migration = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
    };
    if let Err(error) = storage::validate_optional_regular_file(
        &candidate
            .paths
            .root()
            .join(mutation::MUTATION_LOCK_FILE_NAME),
    ) {
        let failure = failure(
            "UNSAFE_WORKSPACE_LOCK",
            error.to_string(),
            Some(&candidate.key),
        );
        partial.migration = ApplyStep::failed(failure.clone());
        return Err(workspace_migration_failure(failure, partial));
    }
    let locks = match audit::acquire_workspace_mutation_locks(
        candidate.paths.root(),
        candidate.paths.audit_git(),
        mutation::MUTATION_LOCK_FILE_NAME,
        identity,
    ) {
        Ok(locks) => locks,
        Err(error) => {
            let failure = failure(
                "WORKSPACE_LOCK_FAILED",
                error.to_string(),
                Some(&candidate.key),
            );
            partial.migration = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
    };
    let connection = match open_locked_operational_database(&candidate.paths) {
        Ok(connection) => connection,
        Err(error) => {
            let failure = failure("CORRUPT_DATABASE", error.to_string(), Some(&candidate.key));
            partial.migration = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
    };
    // BEGIN IMMEDIATE is the compatibility boundary with v0.1 writers, which
    // do not know the v0.2 lock file. It is held across backup and migration,
    // so no commit can land between the restore point and schema commit.
    if let Err(error) = connection.execute_batch("BEGIN IMMEDIATE;") {
        let failure = failure(
            "MIGRATION_BEGIN_FAILED",
            error.to_string(),
            Some(&candidate.key),
        );
        partial.migration = ApplyStep::failed(failure.clone());
        return Err(workspace_migration_failure(failure, partial));
    }
    let rollback_monitor = match open_read_only_operational_database(&candidate.paths) {
        Ok(connection) => connection,
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK;");
            let failure = failure(
                "MIGRATION_BEGIN_FAILED",
                error.to_string(),
                Some(&candidate.key),
            );
            partial.migration = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
    };
    let rollback_data_version = match sqlite_data_version(&rollback_monitor) {
        Ok(version) => version,
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK;");
            let failure = failure(
                "MIGRATION_BEGIN_FAILED",
                error.to_string(),
                Some(&candidate.key),
            );
            partial.migration = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
    };
    let schema_before = match super::inspect_schema(&connection) {
        Ok(schema) => schema,
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK;");
            let failure = failure(error.code, error.message, Some(&candidate.key));
            partial.migration = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
    };
    partial.schema_before = Some(schema_before.clone());

    let marker = if schema_before.pending_migrations.is_empty() {
        None
    } else {
        match audit::ensure_pending_snapshot_marker_locked(locks.snapshot_lock()) {
            Ok(marker) => Some(marker),
            Err(error) => {
                let _ = connection.execute_batch("ROLLBACK;");
                let failure = failure(
                    "AUDIT_MARKER_FAILED",
                    error.to_string(),
                    Some(&candidate.key),
                );
                partial.migration = ApplyStep::failed(failure.clone());
                return Err(workspace_migration_failure(failure, partial));
            }
        }
    };

    let backup_source = match open_read_only_operational_database(&candidate.paths) {
        Ok(connection) => connection,
        Err(error) => {
            if let Some(marker) = marker {
                recover_failed_migration(
                    connection,
                    &rollback_monitor,
                    rollback_data_version,
                    marker,
                    candidate,
                    &locks,
                    &schema_before,
                    &mut partial,
                    faults,
                );
            } else {
                let _ = connection.execute_batch("ROLLBACK;");
            }
            let failure = failure(
                "WORKSPACE_BACKUP_FAILED",
                error.to_string(),
                Some(&candidate.key),
            );
            partial.database_backup = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
    };
    if let Err(error) = online_backup_to_path(&backup_source, &workspace_backup_dir, &backup_path) {
        if let Some(marker) = marker {
            recover_failed_migration(
                connection,
                &rollback_monitor,
                rollback_data_version,
                marker,
                candidate,
                &locks,
                &schema_before,
                &mut partial,
                faults,
            );
        } else {
            let _ = connection.execute_batch("ROLLBACK;");
        }
        let failure = failure(
            "WORKSPACE_BACKUP_FAILED",
            error.to_string(),
            Some(&candidate.key),
        );
        partial.database_backup = ApplyStep::failed(failure.clone());
        return Err(workspace_migration_failure(failure, partial));
    }
    drop(backup_source);
    let backup_bytes = fs::metadata(&backup_path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    partial.database_backup = ApplyStep::completed(
        Some(&backup_path),
        Some(backup_bytes),
        "WAL-safe SQLite Online Backup completed under mutation lock",
    );

    if let Err(error) = faults.check(ApplyFaultPoint::AfterWorkspaceBackup, Some(&candidate.key)) {
        if let Some(marker) = marker {
            recover_failed_migration(
                connection,
                &rollback_monitor,
                rollback_data_version,
                marker,
                candidate,
                &locks,
                &schema_before,
                &mut partial,
                faults,
            );
        } else {
            let _ = connection.execute_batch("ROLLBACK;");
        }
        let failure = failure(
            "WORKSPACE_BACKUP_BOUNDARY_FAILED",
            error.to_string(),
            Some(&candidate.key),
        );
        partial.migration = ApplyStep::failed(failure.clone());
        return Err(workspace_migration_failure(failure, partial));
    }

    if schema_before.pending_migrations.is_empty() {
        if let Err(error) = connection.execute_batch("COMMIT;") {
            let failure = failure(
                "MIGRATION_COMMIT_FAILED",
                error.to_string(),
                Some(&candidate.key),
            );
            partial.migration = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
        return Ok(WorkspaceMigrationResult {
            backup_path,
            backup_bytes,
            schema_before: schema_before.clone(),
            schema_after: schema_before,
            mutated: false,
            snapshot_step: ApplyStep::not_applicable("no operational DB mutation"),
            snapshot_duration_ms: None,
            snapshot_bytes: None,
            snapshot_pending: false,
            warning: None,
        });
    }
    let marker = marker.expect("pending migrations always create an audit marker");
    if let Err(error) = schema::apply_pending_migrations_in(&connection) {
        recover_failed_migration(
            connection,
            &rollback_monitor,
            rollback_data_version,
            marker,
            candidate,
            &locks,
            &schema_before,
            &mut partial,
            faults,
        );
        let failure = failure("MIGRATION_FAILED", error.to_string(), Some(&candidate.key));
        partial.migration = ApplyStep::failed(failure.clone());
        return Err(workspace_migration_failure(failure, partial));
    }
    if let Err(error) = faults.check(
        ApplyFaultPoint::AfterMigrationBeforeSchemaInspect,
        Some(&candidate.key),
    ) {
        recover_failed_migration(
            connection,
            &rollback_monitor,
            rollback_data_version,
            marker,
            candidate,
            &locks,
            &schema_before,
            &mut partial,
            faults,
        );
        let failure = failure(
            "SCHEMA_AFTER_INSPECTION_FAILED",
            error.to_string(),
            Some(&candidate.key),
        );
        partial.migration = ApplyStep::failed(failure.clone());
        return Err(workspace_migration_failure(failure, partial));
    }
    let schema_after = match super::inspect_schema(&connection) {
        Ok(schema) => schema,
        Err(error) => {
            recover_failed_migration(
                connection,
                &rollback_monitor,
                rollback_data_version,
                marker,
                candidate,
                &locks,
                &schema_before,
                &mut partial,
                faults,
            );
            let failure = failure(error.code, error.message, Some(&candidate.key));
            partial.migration = ApplyStep::failed(failure.clone());
            return Err(workspace_migration_failure(failure, partial));
        }
    };
    if let Err(error) = connection.execute_batch("COMMIT;") {
        recover_failed_migration(
            connection,
            &rollback_monitor,
            rollback_data_version,
            marker,
            candidate,
            &locks,
            &schema_before,
            &mut partial,
            faults,
        );
        let failure = failure(
            "MIGRATION_COMMIT_FAILED",
            error.to_string(),
            Some(&candidate.key),
        );
        partial.migration = ApplyStep::failed(failure.clone());
        return Err(workspace_migration_failure(failure, partial));
    }

    let snapshot_started = Instant::now();
    let (snapshot_step, snapshot_duration_ms, snapshot_bytes, snapshot_pending, warning) =
        match audit::write_sql_snapshot_locked(
            candidate.paths.audit_git(),
            &connection,
            locks.snapshot_lock(),
        ) {
            Ok(snapshot) => {
                let duration_ms = snapshot.duration_ms;
                let bytes_written = snapshot.bytes_written;
                (
                    ApplyStep::completed(
                        Some(&snapshot.path),
                        Some(bytes_written),
                        format!("full SQL audit snapshot completed in {} ms", duration_ms),
                    ),
                    Some(duration_ms),
                    Some(bytes_written),
                    false,
                    None,
                )
            }
            Err(error) => {
                let duration_ms =
                    u64::try_from(snapshot_started.elapsed().as_millis()).unwrap_or(u64::MAX);
                (
                    ApplyStep {
                        status: ApplyStepStatus::Completed,
                        path: None,
                        bytes: None,
                        detail: Some(format!(
                        "database committed; audit snapshot remains pending after {duration_ms} ms"
                    )),
                        error: None,
                    },
                    Some(duration_ms),
                    None,
                    true,
                    Some(OutputWarning {
                        code: mutation::AUDIT_SNAPSHOT_PENDING,
                        message: format!("migration committed; audit snapshot pending: {error}"),
                    }),
                )
            }
        };
    Ok(WorkspaceMigrationResult {
        backup_path,
        backup_bytes,
        schema_before,
        schema_after,
        mutated: true,
        snapshot_step,
        snapshot_duration_ms,
        snapshot_bytes,
        snapshot_pending,
        warning,
    })
}

fn open_locked_operational_database(paths: &WorkspacePaths) -> rusqlite::Result<Connection> {
    let canonical = paths
        .db()
        .canonicalize()
        .map_err(|_| rusqlite::Error::InvalidPath(paths.db().clone()))?;
    let connection = Connection::open_with_flags(
        canonical,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA temp_store = MEMORY;",
    )?;
    Ok(connection)
}

fn open_read_only_operational_database(paths: &WorkspacePaths) -> rusqlite::Result<Connection> {
    let canonical = paths
        .db()
        .canonicalize()
        .map_err(|_| rusqlite::Error::InvalidPath(paths.db().clone()))?;
    let connection = Connection::open_with_flags(
        canonical,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    connection.execute_batch("PRAGMA busy_timeout = 5000; PRAGMA query_only = ON;")?;
    Ok(connection)
}

#[allow(clippy::too_many_arguments)]
fn recover_failed_migration(
    connection: Connection,
    rollback_monitor: &Connection,
    rollback_data_version: i64,
    marker: PendingSnapshotMarker,
    candidate: &WorkspaceCandidate,
    locks: &audit::WorkspaceMutationLocks,
    schema_before: &WorkspaceSchemaPlan,
    partial: &mut WorkspaceApplyReport,
    faults: &dyn ApplyFaultInjector,
) {
    let rollback = faults
        .check(
            ApplyFaultPoint::BeforeMigrationRollback,
            Some(&candidate.key),
        )
        .and_then(|()| connection.execute_batch("ROLLBACK;").map_err(sqlite_io));
    if rollback.is_ok() {
        partial.restoration.transaction_rolled_back = true;
        if let Err(error) = clear_created_marker(marker, locks.snapshot_lock()) {
            partial.restoration.error = Some(failure(
                "AUDIT_MARKER_CLEAR_FAILED",
                error.to_string(),
                Some(&candidate.key),
            ));
        }
        return;
    }

    // Closing a SQLite connection rolls back its open transaction. Keep the
    // independent data_version monitor alive across that close, then reacquire
    // SQLite's write reservation. If another process committed in the gap we
    // fail closed and leave the pending marker instead of overwriting data.
    drop(connection);
    partial.restoration.backup_restore_attempted = true;
    if let Err(error) = faults.check(
        ApplyFaultPoint::BeforeRollbackRecoveryCheck,
        Some(&candidate.key),
    ) {
        set_restore_failure(
            partial,
            &candidate.key,
            "DATABASE_RESTORE_BLOCKED_CONCURRENT_CHANGE",
            error.to_string(),
        );
        return;
    }
    let recovery = (|| -> io::Result<()> {
        let recovery_guard =
            open_locked_operational_database(&candidate.paths).map_err(sqlite_io)?;
        recovery_guard
            .execute_batch("BEGIN IMMEDIATE;")
            .map_err(sqlite_io)?;
        let current_data_version = sqlite_data_version(rollback_monitor)?;
        if current_data_version != rollback_data_version {
            let _ = recovery_guard.execute_batch("ROLLBACK;");
            return Err(io::Error::other(
                "another SQLite connection committed before rollback recovery",
            ));
        }
        let recovered_schema = super::inspect_schema(&recovery_guard)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.message))?;
        if &recovered_schema != schema_before {
            let _ = recovery_guard.execute_batch("ROLLBACK;");
            return Err(io::Error::other(
                "database schema differs from the durable pre-upgrade backup boundary",
            ));
        }
        let snapshot_source =
            open_read_only_operational_database(&candidate.paths).map_err(sqlite_io)?;
        audit::write_sql_snapshot_locked(
            candidate.paths.audit_git(),
            &snapshot_source,
            locks.snapshot_lock(),
        )
        .map_err(io::Error::other)?;
        recovery_guard.execute_batch("COMMIT;").map_err(sqlite_io)
    })();
    match recovery {
        Ok(()) => partial.restoration.backup_restore_completed = true,
        Err(error) => set_restore_failure(
            partial,
            &candidate.key,
            "DATABASE_RESTORE_BLOCKED_CONCURRENT_CHANGE",
            error.to_string(),
        ),
    }
}

fn sqlite_data_version(connection: &Connection) -> io::Result<i64> {
    connection
        .query_row("PRAGMA data_version;", [], |row| row.get(0))
        .map_err(sqlite_io)
}

fn set_restore_failure(
    partial: &mut WorkspaceApplyReport,
    workspace_key: &str,
    code: &'static str,
    message: String,
) {
    partial.restoration.error = Some(failure(code, message, Some(workspace_key)));
    partial.status = WorkspaceApplyStatus::RestoreFailed;
}

fn clear_created_marker(
    marker: PendingSnapshotMarker,
    lock: &audit::SnapshotLock,
) -> io::Result<()> {
    if marker == PendingSnapshotMarker::Created {
        audit::clear_pending_snapshot_marker_locked(lock).map_err(io::Error::other)?;
    }
    Ok(())
}

fn sqlite_io(error: rusqlite::Error) -> io::Error {
    io::Error::other(error)
}

fn refresh_owned_assets(states: &[GlobalAssetState]) -> io::Result<()> {
    for state in states {
        ensure_parent_directories(&state.target)?;
        durable_replace(&state.target, state.replacement)?;
    }
    Ok(())
}

fn restore_owned_assets(states: &[GlobalAssetState]) -> io::Result<()> {
    // Validate the whole layer before the first restore. Bytes other than the
    // durable backup or our replacement are a concurrent user edit and must
    // never be overwritten.
    for state in states {
        validate_current_is_backup_or_expected(&state.backup, state.replacement)?;
    }
    let mut first_error = None;
    for state in states.iter().rev() {
        let current = read_optional_regular(&state.backup.original_path)?;
        let expected = Some(state.replacement);
        if current.as_deref() != expected {
            continue;
        }
        if let Err(error) = restore_file_state(&state.backup) {
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

fn restore_adapter(state: &FileBackupState) -> io::Result<()> {
    let backup_bytes = state.backup_path.as_ref().map(fs::read).transpose()?;
    let prepared =
        adapter::prepare_instruction_sync(backup_bytes.as_deref()).map_err(io::Error::other)?;
    validate_current_is_backup_or_expected(state, &prepared.bytes)?;
    let current = read_optional_regular(&state.original_path)?;
    if current.as_deref() == Some(prepared.bytes.as_slice()) {
        restore_file_state(state)?;
    }
    Ok(())
}

fn validate_current_is_backup_or_expected(
    state: &FileBackupState,
    expected: &[u8],
) -> io::Result<()> {
    let current = read_optional_regular(&state.original_path)?;
    let backup = state.backup_path.as_ref().map(fs::read).transpose()?;
    if current == backup || current.as_deref() == Some(expected) {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "managed file changed after AOPMem wrote it; refusing unsafe restore: {}",
            display_path(&state.original_path)
        )))
    }
}

fn validate_backed_up_states_unchanged(
    states: &[&FileBackupState],
) -> Result<(), BackedUpStateError> {
    for state in states {
        validate_backed_up_state_unchanged(state)?;
    }
    Ok(())
}

fn validate_backed_up_state_unchanged(state: &FileBackupState) -> Result<(), BackedUpStateError> {
    let current = read_optional_regular(&state.original_path).map_err(|source| {
        BackedUpStateError::Inspect {
            path: state.original_path.clone(),
            source,
        }
    })?;
    let backed_up = match state.backup_path.as_ref() {
        Some(path) => Some(
            fs::read(path).map_err(|source| BackedUpStateError::Inspect {
                path: path.clone(),
                source,
            })?,
        ),
        None => None,
    };
    if current == backed_up && current.is_some() == state.existed {
        Ok(())
    } else {
        Err(BackedUpStateError::Changed {
            path: state.original_path.clone(),
        })
    }
}

fn sync_adapter(context: &CurrentRepoContext) -> Result<bool, adapter::SeedError> {
    let existing = read_optional_regular(&context.instruction_file)?;
    let prepared = adapter::prepare_instruction_sync(existing.as_deref())?;
    let changed = existing.as_deref() != Some(prepared.bytes.as_slice());
    if changed {
        durable_replace(&context.instruction_file, &prepared.bytes)?;
    }
    Ok(changed)
}

fn restore_file_state(state: &FileBackupState) -> io::Result<()> {
    if state.existed {
        let backup_path = state
            .backup_path
            .as_ref()
            .ok_or_else(|| io::Error::other("existing source has no backup path"))?;
        let bytes = fs::read(backup_path)?;
        ensure_parent_directories(&state.original_path)?;
        durable_replace(&state.original_path, &bytes)
    } else {
        remove_optional_regular(&state.original_path)
    }
}

fn ensure_parent_directories(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("managed file has no parent"))?;
    if parent.exists() {
        storage::validate_real_directory(parent)?;
        return Ok(());
    }
    let grandparent = parent
        .parent()
        .ok_or_else(|| io::Error::other("managed directory has no parent"))?;
    storage::ensure_owned_direct_directory(grandparent, parent)
}

fn durable_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    storage::validate_optional_regular_file(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("managed file has no parent"))?;
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| io::Error::other("managed file name is not UTF-8"))?;
    let directory = AnchoredDir::open_workspace(parent, None)?;
    let temporary_name = temporary_name(name)?;
    let mut temporary = directory.create_new_regular_os(OsStr::new(&temporary_name))?;
    temporary.write_all(bytes)?;
    temporary.sync_all()?;
    if path.exists() {
        directory.replace_regular(&temporary, &temporary_name, name)?;
    } else {
        directory.publish_regular_no_replace_committed_os(
            &temporary,
            OsStr::new(&temporary_name),
            OsStr::new(name),
        )?;
    }
    directory.sync()
}

fn remove_optional_regular(path: &Path) -> io::Result<()> {
    let parent = match path.parent() {
        Some(parent) if parent.exists() => parent,
        _ => return Ok(()),
    };
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("managed file has no name"))?;
    let directory = AnchoredDir::open_workspace(parent, None)?;
    match directory.open_regular_optional_os(name)? {
        Some(_) => {
            directory.remove_regular_os(name)?;
            directory.sync()
        }
        None => Ok(()),
    }
}

fn temporary_name(destination: &str) -> io::Result<String> {
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).map_err(|_| io::Error::other("random temp id failed"))?;
    let mut suffix = String::with_capacity(16);
    for byte in random {
        use std::fmt::Write as _;
        write!(suffix, "{byte:02x}").map_err(io::Error::other)?;
    }
    Ok(format!(".{destination}.upgrade-{suffix}.tmp"))
}

fn write_new_durable(directory: &AnchoredDir, name: &str, bytes: &[u8]) -> io::Result<()> {
    let mut file = directory.create_new_regular_os(OsStr::new(name))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    directory.sync()
}

fn read_optional_regular(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("managed file has no parent"))?;
    match fs::symlink_metadata(parent) {
        Ok(_) => {
            let directory = AnchoredDir::open_workspace(parent, None)?;
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::other("managed file has no name"))?;
            let Some(mut file) = directory.open_regular_optional_os(name)? else {
                return Ok(None);
            };
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn owned_asset_path(paths: &AopmemPaths, asset: &OwnedGlobalAsset) -> PathBuf {
    let root = match asset.area {
        GlobalAssetArea::Skills => paths.skills(),
        GlobalAssetArea::Templates => paths.templates(),
    };
    root.join(asset.directory).join(asset.file_name)
}

fn run_health_checks(context: &CurrentRepoContext) -> HealthCheckResult {
    let doctor = match verify::run_doctor(&context.root) {
        Ok(report) => report,
        Err(error) => {
            return Err(Box::new(HealthCheckFailure {
                failure: failure(
                    "DOCTOR_FAILED",
                    error.to_string(),
                    Some(&context.workspace_key),
                ),
                doctor: None,
                verify: None,
            }));
        }
    };
    if !doctor_is_acceptable(&doctor) {
        return Err(Box::new(HealthCheckFailure {
            failure: failure(
                "DOCTOR_FAILED",
                "doctor reported an unhealthy upgraded workspace",
                Some(&context.workspace_key),
            ),
            doctor: Some(doctor),
            verify: None,
        }));
    }
    let verify = match verify::run_lint(&context.root) {
        Ok(report) => report,
        Err(error) => {
            return Err(Box::new(HealthCheckFailure {
                failure: failure(
                    "VERIFY_FAILED",
                    error.to_string(),
                    Some(&context.workspace_key),
                ),
                doctor: Some(doctor),
                verify: None,
            }));
        }
    };
    if !verify_is_acceptable(&verify) {
        return Err(Box::new(HealthCheckFailure {
            failure: failure(
                "VERIFY_FAILED",
                format!("verify reported {} issue(s)", verify.summary.total),
                Some(&context.workspace_key),
            ),
            doctor: Some(doctor),
            verify: Some(verify),
        }));
    }
    Ok((doctor, verify))
}

fn doctor_is_acceptable(report: &verify::DoctorReport) -> bool {
    report.healthy
        || (report.checks.audit_snapshot.pending
            && [
                report.checks.global_dirs.status,
                report.checks.workspace.status,
                report.checks.db.status,
                report.checks.schema.status,
                report.checks.fts.status,
                report.checks.adapter_block.status,
                report.checks.artifacts_dirs.status,
                report.checks.tools_dirs.status,
            ]
            .into_iter()
            .all(|status| status == verify::DoctorStatus::Ready))
}

fn verify_is_acceptable(report: &verify::LintReport) -> bool {
    report.clean
        || (!report.issues.is_empty()
            && report
                .issues
                .iter()
                .all(|issue| issue.kind == verify::LintIssueKind::PendingAuditSnapshot))
}

#[allow(clippy::too_many_arguments)]
fn finish_failed_apply(
    report: &mut UpgradeApplyReport,
    stop_failure: UpgradeApplyFailure,
    _candidates: &[WorkspaceCandidate],
    _mutated_indexes: &[usize],
    adapter_state: Option<&FileBackupState>,
    asset_states: &[GlobalAssetState],
    adapter_may_have_changed: bool,
    assets_may_have_changed: bool,
) {
    let stopped_workspace = stop_failure.workspace_key.clone();
    report.stopped_workspace = stopped_workspace.clone();
    report.stop_reason = Some(stop_failure);
    // A committed workspace is never restored because a later workspace or
    // global step failed. v0.1 does not honor our file lock and may have
    // committed after this process released SQLite's write reservation.
    // Replaying an older backup would silently destroy that valid commit.
    report.restoration.attempted = adapter_may_have_changed || assets_may_have_changed;
    let mut completed = true;
    if adapter_may_have_changed {
        match adapter_state {
            Some(state) => match restore_adapter(state) {
                Ok(()) => report.restoration.adapter_restored = true,
                Err(error) => {
                    report.restoration.adapter_error = Some(failure(
                        "ADAPTER_RESTORE_FAILED",
                        error.to_string(),
                        stopped_workspace.as_deref(),
                    ));
                }
            },
            None => {
                report.restoration.adapter_error = Some(failure(
                    "ADAPTER_RESTORE_FAILED",
                    "adapter backup state is missing",
                    stopped_workspace.as_deref(),
                ));
            }
        }
        completed &= report.restoration.adapter_restored;
    }
    if assets_may_have_changed {
        match restore_owned_assets(asset_states) {
            Ok(()) => report.restoration.owned_assets_restored = true,
            Err(error) => {
                report.restoration.owned_assets_error = Some(failure(
                    "OWNED_ASSET_RESTORE_FAILED",
                    error.to_string(),
                    None,
                ));
            }
        }
        completed &= report.restoration.owned_assets_restored;
    }
    report.restoration.completed = report.restoration.attempted && completed;
}

fn record_failed_updates(
    candidates: &[WorkspaceCandidate],
    collectors: &mut Vec<Option<LocalCollector>>,
    warnings: &mut Vec<OutputWarning>,
    failure: &UpgradeApplyFailure,
) {
    while collectors.len() < candidates.len() {
        collectors.push(None);
    }
    for collector in collectors {
        record_update_event(
            collector,
            EventType::UpdateFailed,
            EventOutcome::Failure,
            Some(failure.code),
            warnings,
        );
    }
}

fn record_update_event(
    collector: &mut Option<LocalCollector>,
    event_type: EventType,
    outcome: EventOutcome,
    error_code: Option<&str>,
    warnings: &mut Vec<OutputWarning>,
) -> bool {
    let Some(collector) = collector.as_mut() else {
        return false;
    };
    let event = CollectorEvent::new(event_type, outcome, EventPayload::Empty).and_then(|event| {
        match error_code {
            Some(error_code) => event.with_error_code(error_code),
            None => Ok(event),
        }
    });
    if let Some(warning) = collector.record_result(event) {
        push_warning_once(warnings, warning);
        false
    } else {
        true
    }
}

fn record_audit_snapshot_event(
    collector: &mut Option<LocalCollector>,
    duration_ms: Option<u64>,
    bytes_written: Option<u64>,
    pending: bool,
    warnings: &mut Vec<OutputWarning>,
) {
    let Some(duration_ms) = duration_ms else {
        return;
    };
    let mut items = match CountItem::new("duration_ms", duration_ms) {
        Ok(item) => vec![item],
        Err(_) => {
            push_warning_once(warnings, observability_warning());
            return;
        }
    };
    if let Some(bytes_written) = bytes_written {
        match CountItem::new("bytes_written", bytes_written) {
            Ok(item) => items.push(item),
            Err(_) => {
                push_warning_once(warnings, observability_warning());
                return;
            }
        }
    }
    let event = CountsPayload::new(items)
        .and_then(|payload| {
            CollectorEvent::new(
                if pending {
                    EventType::AuditSnapshotPending
                } else {
                    EventType::AuditSnapshotCompleted
                },
                if pending {
                    EventOutcome::Pending
                } else {
                    EventOutcome::Success
                },
                EventPayload::Counts(payload),
            )
        })
        .and_then(|event| {
            if pending {
                event.with_error_code(mutation::AUDIT_SNAPSHOT_PENDING)
            } else {
                Ok(event)
            }
        });
    let Some(collector) = collector.as_mut() else {
        return;
    };
    if let Some(warning) = collector.record_result(event) {
        push_warning_once(warnings, warning);
    }
}

fn observability_warning() -> OutputWarning {
    OutputWarning {
        code: crate::output::OBSERVABILITY_WRITE_FAILED,
        message: "local observability write failed; core command result is unchanged".to_string(),
    }
}

fn push_warning_once(warnings: &mut Vec<OutputWarning>, warning: OutputWarning) {
    if !warnings.iter().any(|current| current.code == warning.code) {
        warnings.push(warning);
    }
}

fn failed_execution(
    mut report: UpgradeApplyReport,
    failure: UpgradeApplyFailure,
    warnings: Vec<OutputWarning>,
) -> UpgradeApplyExecution {
    report.stopped_workspace = failure.workspace_key.clone();
    report.stop_reason = Some(failure.clone());
    UpgradeApplyExecution {
        report,
        warnings,
        failure: Some(failure),
    }
}

fn failed_execution_with_observability(
    report: UpgradeApplyReport,
    failure: UpgradeApplyFailure,
    candidates: &[WorkspaceCandidate],
) -> UpgradeApplyExecution {
    let mut warnings = Vec::new();
    for candidate in candidates {
        // Never create the best-effort store until persistent workspace paths
        // have passed the same no-follow validation as core mutation.
        if storage::validate_workspace_mutation_paths(&candidate.paths).is_err() {
            push_warning_once(&mut warnings, observability_warning());
            continue;
        }
        let mut collector = LocalCollector::new(&candidate.paths, COMMAND_ID).ok();
        record_update_event(
            &mut collector,
            EventType::UpdateStarted,
            EventOutcome::Started,
            None,
            &mut warnings,
        );
        record_update_event(
            &mut collector,
            EventType::UpdateFailed,
            EventOutcome::Failure,
            Some(failure.code),
            &mut warnings,
        );
        if collector.is_none() {
            push_warning_once(&mut warnings, observability_warning());
        }
    }
    failed_execution(report, failure, warnings)
}

fn failure(
    code: &'static str,
    message: impl Into<String>,
    workspace_key: Option<&str>,
) -> UpgradeApplyFailure {
    UpgradeApplyFailure {
        code,
        message: message.into(),
        workspace_key: workspace_key.map(str::to_string),
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";
    const OLD_BINARY: &[u8] = b"v0.1.0-rc3 binary bytes";
    const OLD_ADAPTER: &[u8] = b"user-owned adapter prefix\n";
    const CONCURRENT_ADAPTER: &[u8] = b"concurrent adapter edit\n";
    const CONCURRENT_ASSET: &[u8] = b"concurrent owned-asset edit\n";

    struct FixedDiskProbe(u64);

    impl DiskSpaceProbe for FixedDiskProbe {
        fn available_bytes(&self, _path: &Path) -> io::Result<u64> {
            Ok(self.0)
        }
    }

    struct SequenceDiskProbe(AtomicUsize);

    impl DiskSpaceProbe for SequenceDiskProbe {
        fn available_bytes(&self, _path: &Path) -> io::Result<u64> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(u64::MAX)
            } else {
                Ok(0)
            }
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

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn set(path: &Path) -> Self {
            let original = env::current_dir().expect("current directory should resolve");
            env::set_current_dir(path).expect("test current directory should change");
            Self { original }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.original).expect("current directory should restore");
        }
    }

    struct Fixture {
        root: PathBuf,
        paths: AopmemPaths,
        workspace: WorkspacePaths,
        workspace_key: String,
        binary: PathBuf,
        adapter: PathBuf,
        owned_assets: Vec<PathBuf>,
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        env::temp_dir().join(format!("aopmem-stage-032-{name}-{nanos}"))
    }

    fn create_roots(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = temp_path(name);
        let home = root.join("home");
        let repo = root.join("repo");
        fs::create_dir_all(repo.join(".git")).expect("repository fixture should create");
        (root, home, repo)
    }

    fn create_current_fixture(root: PathBuf, _home: PathBuf, repo: PathBuf) -> Fixture {
        let paths = storage::resolve_paths().expect("fixture paths should resolve");
        storage::ensure_global_dirs(&paths).expect("global directories should create");
        let workspace_key = storage::workspace_key(&repo).expect("workspace key should resolve");
        let workspace = create_v010_workspace(&paths, &workspace_key);
        let binary = paths.bin().join(installed_binary_name());
        fs::write(&binary, OLD_BINARY).expect("old binary should write");
        let adapter = adapter::default_instruction_file(&repo);
        fs::write(&adapter, OLD_ADAPTER).expect("old adapter should write");
        let owned_assets = seed_old_owned_assets(&paths);
        Fixture {
            root,
            paths,
            workspace,
            workspace_key,
            binary,
            adapter,
            owned_assets,
        }
    }

    fn create_v010_workspace(paths: &AopmemPaths, key: &str) -> WorkspacePaths {
        let workspace = storage::ensure_workspace_dirs(paths, key)
            .expect("workspace directories should create");
        let mut connection =
            Connection::open(workspace.db()).expect("fixture database should open");
        schema::apply_migrations(&mut connection).expect("current schema should initialize");
        connection
            .execute_batch(
                r#"
                INSERT INTO nodes (
                    node_type, status, title, summary, body, source_ref,
                    confidence, trust_level
                ) VALUES
                    ('gate', 'active', 'Preserved gate', 'gate summary',
                     'gate body', 'user:test', 0.95, 'user'),
                    ('project_profile', 'active', 'Preserved profile', 'profile summary',
                     'profile body', 'user:test', 0.91, 'user'),
                    ('workflow', 'active', 'Preserved workflow', 'workflow summary',
                     'workflow body', 'user:test', 0.88, 'user'),
                    ('failure_mode', 'active', 'Preserved failure', 'failure summary',
                     'failure body', 'user:test', 0.82, 'user');
                INSERT INTO links (source_node_id, target_node_id, link_type)
                    VALUES (3, 4, 'prevents');
                INSERT INTO aliases (node_id, alias) VALUES (3, 'legacy workflow alias');
                INSERT INTO tags (node_id, tag) VALUES (3, 'legacy-tag');
                INSERT INTO sources (node_id, source_ref) VALUES (3, 'user:fixture-source');
                INSERT INTO events (type, source, subject_kind, subject_id)
                    VALUES ('remember', 'user', 'node', 3);
                INSERT INTO registries (registry_type, name, status, notes)
                    VALUES ('skill', 'legacy-skill', 'installed', 'preserve registry');
                INSERT INTO tool_contracts (
                    tool_id, name, status, owner_workflow, side_effects,
                    approval_requirement, contract_json
                ) VALUES (
                    'legacy.tool', 'Legacy Tool', 'active', 'Preserved workflow',
                    'local_read', 'none', '{"legacy":true}'
                );
                INSERT INTO mcp_profiles (
                    id, name, kind, status, read_operations, write_operations,
                    side_effects, approval_requirement, credentials_source, notes
                ) VALUES (
                    'legacy.mcp', 'Legacy MCP', 'stdio', 'configured_unverified',
                    '["read"]', '[]', 'external_read', 'none', NULL,
                    'preserve MCP profile'
                );
                DELETE FROM schema_migrations WHERE version IN ('002', '003');
                DROP INDEX IF EXISTS idx_nodes_summary;
                DROP INDEX IF EXISTS idx_nodes_title_nocase;
                DROP INDEX IF EXISTS idx_aliases_alias_nocase;
                DROP INDEX IF EXISTS idx_tags_tag_nocase;
                "#,
            )
            .expect("v0.1 fixture data should create");
        audit::write_sql_snapshot(workspace.audit_git(), &connection)
            .expect("fixture audit snapshot should write");
        drop(connection);
        fs::write(
            workspace.artifacts().join("legacy-artifact.txt"),
            b"artifact",
        )
        .expect("legacy artifact should write");
        fs::write(
            workspace.tools().join("legacy-tool.json"),
            b"{\"legacy\":true}",
        )
        .expect("legacy tool file should write");
        workspace
    }

    fn seed_old_owned_assets(paths: &AopmemPaths) -> Vec<PathBuf> {
        let mut targets = Vec::new();
        for (index, asset) in OWNED_GLOBAL_ASSETS.iter().enumerate() {
            let target = owned_asset_path(paths, asset);
            fs::create_dir_all(target.parent().expect("asset parent should exist"))
                .expect("asset parent should create");
            fs::write(&target, format!("old-owned-asset-{index}\n"))
                .expect("old owned asset should write");
            targets.push(target);
        }
        let unrelated = paths.skills().join("unrelated/SKILL.md");
        fs::create_dir_all(unrelated.parent().expect("unrelated parent should exist"))
            .expect("unrelated directory should create");
        fs::write(unrelated, b"unrelated user skill\n").expect("unrelated skill should write");
        targets
    }

    fn migration_versions(database: &Path) -> Vec<String> {
        let connection = Connection::open(database).expect("database should reopen");
        let mut statement = connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")
            .expect("migration query should prepare");
        statement
            .query_map([], |row| row.get(0))
            .expect("migration query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("migration rows should read")
    }

    fn event_types(workspace: &WorkspacePaths) -> Vec<(String, Option<String>)> {
        let connection = Connection::open(workspace.observability_db())
            .expect("observability database should open");
        let mut statement = connection
            .prepare(
                "SELECT event_type, error_code
                 FROM observability_events
                 WHERE command = 'upgrade_apply'
                 ORDER BY rowid",
            )
            .expect("event query should prepare");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("event query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("event rows should read")
    }

    fn assert_v010_data_preserved(database: &Path) {
        let connection = Connection::open(database).expect("database should reopen");
        let checks = [
            ("nodes", 4_i64),
            ("links", 1),
            ("aliases", 1),
            ("tags", 1),
            ("sources", 1),
            ("events", 1),
            ("registries", 1),
            ("tool_contracts", 1),
            ("mcp_profiles", 1),
        ];
        for (table, expected) in checks {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("preserved table should query");
            assert_eq!(count, expected, "unexpected preserved count for {table}");
        }
        let contract: String = connection
            .query_row(
                "SELECT contract_json FROM tool_contracts WHERE tool_id = 'legacy.tool'",
                [],
                |row| row.get(0),
            )
            .expect("tool contract should survive");
        assert_eq!(contract, "{\"legacy\":true}");
    }

    fn logical_payload(
        database: &Path,
    ) -> BTreeMap<&'static str, Vec<Vec<rusqlite::types::Value>>> {
        let connection = Connection::open(database).expect("database should reopen");
        let queries = [
            ("nodes", "SELECT * FROM nodes ORDER BY id"),
            ("links", "SELECT * FROM links ORDER BY id"),
            ("aliases", "SELECT * FROM aliases ORDER BY id"),
            ("tags", "SELECT * FROM tags ORDER BY id"),
            ("sources", "SELECT * FROM sources ORDER BY id"),
            ("events", "SELECT * FROM events ORDER BY id"),
            ("registries", "SELECT * FROM registries ORDER BY id"),
            ("tool_contracts", "SELECT * FROM tool_contracts ORDER BY id"),
            ("mcp_profiles", "SELECT * FROM mcp_profiles ORDER BY id"),
        ];
        queries
            .into_iter()
            .map(|(table, query)| {
                let mut statement = connection
                    .prepare(query)
                    .expect("payload query should prepare");
                let column_count = statement.column_count();
                let rows = statement
                    .query_map([], |row| {
                        (0..column_count)
                            .map(|column| row.get(column))
                            .collect::<rusqlite::Result<Vec<rusqlite::types::Value>>>()
                    })
                    .expect("payload query should run")
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .expect("payload rows should read");
                (table, rows)
            })
            .collect()
    }

    fn audit_head(audit_git: &Path) -> gix::hash::ObjectId {
        gix::open(audit_git)
            .expect("audit repository should open")
            .head_id()
            .expect("audit HEAD should resolve")
            .detach()
    }

    fn backup_root(execution: &UpgradeApplyExecution) -> PathBuf {
        PathBuf::from(
            execution
                .report
                .backup_root
                .as_ref()
                .expect("backup root should be reported"),
        )
    }

    struct FailAt(ApplyFaultPoint);

    impl ApplyFaultInjector for FailAt {
        fn check(&self, point: ApplyFaultPoint, _workspace_key: Option<&str>) -> io::Result<()> {
            if point == self.0 {
                Err(io::Error::other("injected failure"))
            } else {
                Ok(())
            }
        }
    }

    struct EditAt {
        point: ApplyFaultPoint,
        path: PathBuf,
        bytes: &'static [u8],
    }

    impl ApplyFaultInjector for EditAt {
        fn check(&self, point: ApplyFaultPoint, _workspace_key: Option<&str>) -> io::Result<()> {
            if point == self.point {
                fs::write(&self.path, self.bytes)?;
            }
            Ok(())
        }
    }

    struct BackupBoundaryProbe {
        database: PathBuf,
        observed_lock: AtomicBool,
    }

    impl ApplyFaultInjector for BackupBoundaryProbe {
        fn check(&self, point: ApplyFaultPoint, _workspace_key: Option<&str>) -> io::Result<()> {
            if point != ApplyFaultPoint::AfterWorkspaceBackup {
                return Ok(());
            }
            let competing = Connection::open(&self.database).map_err(sqlite_io)?;
            competing.busy_timeout(Duration::ZERO).map_err(sqlite_io)?;
            if competing
                .execute(
                    "INSERT INTO events (type, source, subject_kind, subject_id)
                     VALUES ('competing', 'test', 'node', 1)",
                    [],
                )
                .is_ok()
            {
                return Err(io::Error::other(
                    "competing writer committed inside backup/migration boundary",
                ));
            }
            self.observed_lock.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct RollbackFailure {
        database: Option<PathBuf>,
    }

    impl ApplyFaultInjector for RollbackFailure {
        fn check(&self, point: ApplyFaultPoint, _workspace_key: Option<&str>) -> io::Result<()> {
            match point {
                ApplyFaultPoint::AfterMigrationBeforeSchemaInspect
                | ApplyFaultPoint::BeforeMigrationRollback => {
                    Err(io::Error::other("injected rollback path"))
                }
                ApplyFaultPoint::BeforeRollbackRecoveryCheck => {
                    if let Some(database) = self.database.as_ref() {
                        let connection = Connection::open(database).map_err(sqlite_io)?;
                        connection
                            .execute(
                                "INSERT INTO events (type, source, subject_kind, subject_id)
                                 VALUES ('concurrent', 'test', 'node', 1)",
                                [],
                            )
                            .map_err(sqlite_io)?;
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        }
    }

    struct FailSecondAfterFirstCommit {
        first_database: PathBuf,
    }

    impl ApplyFaultInjector for FailSecondAfterFirstCommit {
        fn check(&self, point: ApplyFaultPoint, workspace_key: Option<&str>) -> io::Result<()> {
            if point == ApplyFaultPoint::BeforeWorkspaceMigration
                && workspace_key == Some("beta-workspace")
            {
                let connection = Connection::open(&self.first_database).map_err(sqlite_io)?;
                connection
                    .execute(
                        "INSERT INTO events (type, source, subject_kind, subject_id)
                         VALUES ('post_commit', 'v0.1', 'node', 1)",
                        [],
                    )
                    .map_err(sqlite_io)?;
                return Err(io::Error::other("stop on second workspace"));
            }
            Ok(())
        }
    }

    #[test]
    fn apply_preserves_v010_data_backups_and_binary_without_replacing_it() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("success");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        let payload_before = logical_payload(fixture.workspace.db());
        let audit_head_before = audit_head(fixture.workspace.audit_git());

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &NoFaults)
            .expect("upgrade apply should run");

        assert!(execution.failure.is_none());
        assert!(execution.report.success);
        assert!(!execution.report.binary_replaced);
        assert_eq!(
            fs::read(&fixture.binary).expect("binary should read"),
            OLD_BINARY
        );
        assert_eq!(
            migration_versions(fixture.workspace.db()),
            vec!["001", "002", "003"]
        );
        assert_v010_data_preserved(fixture.workspace.db());
        assert_eq!(logical_payload(fixture.workspace.db()), payload_before);
        assert_eq!(
            fs::read(fixture.workspace.artifacts().join("legacy-artifact.txt"))
                .expect("legacy artifact should read"),
            b"artifact"
        );
        assert_eq!(
            fs::read(fixture.workspace.tools().join("legacy-tool.json"))
                .expect("legacy tool should read"),
            b"{\"legacy\":true}"
        );
        let audit_head_after = audit_head(fixture.workspace.audit_git());
        assert_ne!(audit_head_after, audit_head_before);
        let audit_repository =
            gix::open(fixture.workspace.audit_git()).expect("audit repository should open");
        let audit_commit = audit_repository
            .find_commit(audit_head_after)
            .expect("new audit commit should read");
        assert!(audit_commit
            .parent_ids()
            .any(|parent| parent.detach() == audit_head_before));
        assert_eq!(
            fs::read(fixture.paths.skills().join("unrelated/SKILL.md"))
                .expect("unrelated skill should read"),
            b"unrelated user skill\n"
        );
        for (target, asset) in fixture.owned_assets.iter().zip(OWNED_GLOBAL_ASSETS) {
            assert_eq!(
                fs::read(target).expect("owned asset should read"),
                asset.bytes
            );
        }
        let backup_root = backup_root(&execution);
        assert!(backup_root.is_dir());
        assert!(backup_root
            .join("binary")
            .join(installed_binary_name())
            .is_file());
        let events = event_types(&fixture.workspace);
        assert!(events.iter().any(|event| event.0 == "update.started"));
        assert!(events.iter().any(|event| event.0 == "update.completed"));
        assert!(events
            .iter()
            .any(|event| event.0 == "audit.snapshot.completed"));
        crate::observability::report::effectiveness_report(
            &fixture.workspace,
            &fixture.workspace_key,
        )
        .expect("upgrade observability rows should satisfy report contracts");

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn migration_failure_rolls_back_keeps_backup_and_records_exact_failed_workspace() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("migration-failure");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        Connection::open(fixture.workspace.db())
            .expect("database should open")
            .execute_batch("PRAGMA foreign_keys = OFF; DROP TABLE nodes;")
            .expect("nodes table should drop");

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &NoFaults)
            .expect("upgrade apply should report failure");
        let failure = execution.failure.as_ref().expect("failure should exist");

        assert_eq!(failure.code, "MIGRATION_FAILED");
        assert_eq!(
            failure.workspace_key.as_deref(),
            Some(fixture.workspace_key.as_str())
        );
        assert_eq!(migration_versions(fixture.workspace.db()), vec!["001"]);
        assert!(
            execution.report.workspaces[0]
                .restoration
                .transaction_rolled_back
        );
        assert!(!audit::has_pending_snapshot(fixture.workspace.audit_git())
            .expect("pending marker should inspect"));
        assert!(backup_root(&execution).is_dir());
        assert!(event_types(&fixture.workspace).iter().any(|event| {
            event.0 == "update.failed" && event.1.as_deref() == Some("MIGRATION_FAILED")
        }));

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn wal_backup_boundary_blocks_competing_writer_and_contains_latest_commit() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("wal-boundary");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        let connection = Connection::open(fixture.workspace.db()).expect("database should open");
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 INSERT INTO events (type, source, subject_kind, subject_id)
                 VALUES ('latest_pre_upgrade', 'v0.1', 'node', 1);
                 PRAGMA wal_checkpoint(TRUNCATE);",
            )
            .expect("WAL fixture should commit");
        drop(connection);
        for suffix in DATABASE_SIDECAR_SUFFIXES {
            let sidecar = path_with_suffix(fixture.workspace.db(), suffix);
            if sidecar.exists() {
                fs::remove_file(sidecar).expect("quiesced sidecar should remove");
            }
        }
        let faults = BackupBoundaryProbe {
            database: fixture.workspace.db().clone(),
            observed_lock: AtomicBool::new(false),
        };

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &faults)
            .expect("upgrade apply should run");

        assert!(execution.report.success);
        assert!(faults.observed_lock.load(Ordering::SeqCst));
        let backup = PathBuf::from(
            execution.report.workspaces[0]
                .database_backup
                .path
                .as_ref()
                .expect("database backup should be reported"),
        );
        let backup_connection = Connection::open(backup).expect("backup should open");
        let preserved: i64 = backup_connection
            .query_row(
                "SELECT COUNT(*) FROM events WHERE type = 'latest_pre_upgrade'",
                [],
                |row| row.get(0),
            )
            .expect("backup event should query");
        assert_eq!(preserved, 1);

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn rollback_failure_uses_safe_recovery_and_clears_marker_only_after_proof() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("rollback-recovery");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);

        let execution = apply_all_workspaces_with(
            &FixedDiskProbe(u64::MAX),
            &RollbackFailure { database: None },
        )
        .expect("upgrade apply should report injected failure");
        let workspace = &execution.report.workspaces[0];

        assert_eq!(
            execution
                .failure
                .as_ref()
                .expect("failure should exist")
                .code,
            "SCHEMA_AFTER_INSPECTION_FAILED"
        );
        assert!(!workspace.restoration.transaction_rolled_back);
        assert!(workspace.restoration.backup_restore_attempted);
        assert!(
            workspace.restoration.backup_restore_completed,
            "{workspace:#?}"
        );
        assert_eq!(migration_versions(fixture.workspace.db()), vec!["001"]);
        assert!(!audit::has_pending_snapshot(fixture.workspace.audit_git())
            .expect("pending marker should inspect"));

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn rollback_recovery_refuses_concurrent_commit_and_keeps_pending_marker() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("rollback-concurrent");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);

        let execution = apply_all_workspaces_with(
            &FixedDiskProbe(u64::MAX),
            &RollbackFailure {
                database: Some(fixture.workspace.db().clone()),
            },
        )
        .expect("upgrade apply should report injected failure");
        let workspace = &execution.report.workspaces[0];

        assert_eq!(workspace.status, WorkspaceApplyStatus::RestoreFailed);
        assert_eq!(
            workspace
                .restoration
                .error
                .as_ref()
                .expect("restore blocker should exist")
                .code,
            "DATABASE_RESTORE_BLOCKED_CONCURRENT_CHANGE"
        );
        assert!(audit::has_pending_snapshot(fixture.workspace.audit_git())
            .expect("pending marker should inspect"));
        let connection = Connection::open(fixture.workspace.db()).expect("database should open");
        let concurrent: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM events WHERE type = 'concurrent'",
                [],
                |row| row.get(0),
            )
            .expect("concurrent event should query");
        assert_eq!(concurrent, 1);

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn later_workspace_failure_never_restores_prior_commit_or_post_commit_write() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("two-workspaces");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let paths = storage::resolve_paths().expect("paths should resolve");
        storage::ensure_global_dirs(&paths).expect("global directories should create");
        let first = create_v010_workspace(&paths, "alpha-workspace");
        let second = create_v010_workspace(&paths, "beta-workspace");
        let third = create_v010_workspace(&paths, "gamma-workspace");
        fs::write(paths.bin().join(installed_binary_name()), OLD_BINARY)
            .expect("old binary should write");
        seed_old_owned_assets(&paths);
        let faults = FailSecondAfterFirstCommit {
            first_database: first.db().clone(),
        };

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &faults)
            .expect("upgrade apply should stop on second workspace");

        assert_eq!(
            execution
                .failure
                .as_ref()
                .expect("failure should exist")
                .workspace_key
                .as_deref(),
            Some("beta-workspace")
        );
        assert_eq!(
            execution.report.workspaces[0].status,
            WorkspaceApplyStatus::Applied
        );
        assert_eq!(migration_versions(first.db()), vec!["001", "002", "003"]);
        assert_eq!(migration_versions(second.db()), vec!["001"]);
        assert_eq!(migration_versions(third.db()), vec!["001"]);
        assert_eq!(
            execution.report.workspaces[2].status,
            WorkspaceApplyStatus::NotStarted
        );
        let connection = Connection::open(first.db()).expect("first database should open");
        let post_commit: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM events WHERE type = 'post_commit'",
                [],
                |row| row.get(0),
            )
            .expect("post-commit event should query");
        assert_eq!(post_commit, 1);
        assert!(backup_root(&execution).is_dir());

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(root).expect("fixture should remove");
    }

    #[test]
    fn concurrent_owned_asset_edit_blocks_layer_and_survives_exactly() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("asset-edit");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        let changed_asset = fixture.owned_assets[1].clone();
        let faults = EditAt {
            point: ApplyFaultPoint::BeforeOwnedAssetsRefresh,
            path: changed_asset.clone(),
            bytes: CONCURRENT_ASSET,
        };

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &faults)
            .expect("upgrade apply should report asset blocker");

        assert_eq!(
            execution
                .failure
                .as_ref()
                .expect("failure should exist")
                .code,
            "OWNED_ASSET_CHANGED_SINCE_BACKUP"
        );
        assert_eq!(
            fs::read(changed_asset).expect("asset should read"),
            CONCURRENT_ASSET
        );
        assert_eq!(
            migration_versions(fixture.workspace.db()),
            vec!["001", "002", "003"]
        );

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn concurrent_adapter_edit_blocks_sync_and_survives_exactly() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("adapter-edit");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        let faults = EditAt {
            point: ApplyFaultPoint::BeforeAdapterSync,
            path: fixture.adapter.clone(),
            bytes: CONCURRENT_ADAPTER,
        };

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &faults)
            .expect("upgrade apply should report adapter blocker");

        assert_eq!(
            execution
                .failure
                .as_ref()
                .expect("failure should exist")
                .code,
            "ADAPTER_CHANGED_SINCE_BACKUP",
            "{execution:#?}"
        );
        assert_eq!(
            fs::read(&fixture.adapter).expect("adapter should read"),
            CONCURRENT_ADAPTER
        );
        assert!(execution.report.restoration.owned_assets_restored);

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn late_doctor_failure_restores_only_files_written_by_apply_and_keeps_db_commit() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("doctor-failure");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        let old_assets = fixture
            .owned_assets
            .iter()
            .map(|path| fs::read(path).expect("old asset should read"))
            .collect::<Vec<_>>();

        let execution = apply_all_workspaces_with(
            &FixedDiskProbe(u64::MAX),
            &FailAt(ApplyFaultPoint::BeforeDoctor),
        )
        .expect("doctor failure should report");

        assert_eq!(
            execution
                .failure
                .as_ref()
                .expect("failure should exist")
                .code,
            "DOCTOR_FAILED"
        );
        assert_eq!(
            migration_versions(fixture.workspace.db()),
            vec!["001", "002", "003"]
        );
        assert_eq!(
            fs::read(&fixture.adapter).expect("adapter should read"),
            OLD_ADAPTER
        );
        for (path, expected) in fixture.owned_assets.iter().zip(old_assets) {
            assert_eq!(fs::read(path).expect("asset should read"), expected);
        }
        assert!(execution.report.restoration.adapter_restored);
        assert!(execution.report.restoration.owned_assets_restored);
        assert!(execution.report.restoration.completed);
        assert!(backup_root(&execution).is_dir());

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn disk_corrupt_adapter_drift_and_old_binary_backup_fail_before_core_mutation() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("early-failures");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);

        let disk_probe = SequenceDiskProbe(AtomicUsize::new(0));
        let disk = apply_all_workspaces_with(&disk_probe, &NoFaults)
            .expect("second disk check failure should report");
        assert_eq!(
            disk.failure.as_ref().expect("failure should exist").code,
            "INSUFFICIENT_DISK_SPACE"
        );
        assert!(disk.report.backup_root.is_none());
        assert_eq!(disk_probe.0.load(Ordering::SeqCst), 2);
        assert_eq!(migration_versions(fixture.workspace.db()), vec!["001"]);
        assert!(event_types(&fixture.workspace).iter().any(|event| {
            event.0 == "update.failed" && event.1.as_deref() == Some("INSUFFICIENT_DISK_SPACE")
        }));

        fs::write(
            &fixture.adapter,
            format!("{}\ndamaged\n", adapter::BEGIN_MARKER),
        )
        .expect("damaged adapter should write");
        let drift = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &NoFaults)
            .expect("adapter drift should report");
        assert_eq!(
            drift.failure.as_ref().expect("failure should exist").code,
            "ADAPTER_DRIFT"
        );
        fs::write(&fixture.adapter, OLD_ADAPTER).expect("adapter should reset");

        let binary = apply_all_workspaces_with(
            &FixedDiskProbe(u64::MAX),
            &FailAt(ApplyFaultPoint::BeforeBinaryBackup),
        )
        .expect("binary backup failure should report");
        assert_eq!(
            binary.failure.as_ref().expect("failure should exist").code,
            "OLD_BINARY_BACKUP_FAILED"
        );
        assert!(backup_root(&binary).is_dir());
        assert_eq!(
            fs::read(&fixture.binary).expect("binary should read"),
            OLD_BINARY
        );

        fs::write(fixture.workspace.db(), b"not sqlite").expect("database should corrupt");
        let corrupt = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &NoFaults)
            .expect("corrupt database should report");
        assert_eq!(
            corrupt.failure.as_ref().expect("failure should exist").code,
            "CORRUPT_DATABASE"
        );

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn collector_unavailable_warns_but_does_not_change_success_status() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("collector-unavailable");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        fs::write(fixture.workspace.observability(), b"not a directory")
            .expect("blocked observability path should write");

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &NoFaults)
            .expect("core upgrade should succeed");

        assert!(execution.report.success);
        assert!(execution.failure.is_none());
        assert_eq!(
            migration_versions(fixture.workspace.db()),
            vec!["001", "002", "003"]
        );
        assert!(execution
            .warnings
            .iter()
            .any(|warning| warning.code == crate::output::OBSERVABILITY_WRITE_FAILED));

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn unsafe_workspace_path_skips_collector_write_and_returns_warning() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("unsafe-observability-skip");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        fs::remove_dir_all(fixture.workspace.audit_git()).expect("audit directory should remove");
        fs::write(fixture.workspace.audit_git(), b"unsafe replacement")
            .expect("unsafe audit path should write");

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &NoFaults)
            .expect("unsafe path should report core failure");

        assert_eq!(
            execution
                .failure
                .as_ref()
                .expect("failure should exist")
                .code,
            "UNSAFE_WORKSPACE_PATH"
        );
        assert!(execution.report.backup_root.is_none());
        assert!(execution
            .warnings
            .iter()
            .any(|warning| warning.code == crate::output::OBSERVABILITY_WRITE_FAILED));
        assert!(!fixture.workspace.observability().exists());

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn audit_snapshot_failure_is_success_with_pending_warning_and_health_proof() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("audit-warning");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let fixture = create_current_fixture(root, home, repo);
        fs::remove_dir_all(fixture.workspace.audit_git().join(".git"))
            .expect("audit metadata should remove");
        fs::write(
            fixture.workspace.audit_git().join(".git"),
            b"blocked git metadata",
        )
        .expect("blocked git metadata should write");

        let execution = apply_all_workspaces_with(&FixedDiskProbe(u64::MAX), &NoFaults)
            .expect("committed migration should remain success");

        assert!(execution.report.success);
        assert!(execution.failure.is_none());
        assert!(execution
            .warnings
            .iter()
            .any(|warning| warning.code == mutation::AUDIT_SNAPSHOT_PENDING));
        assert!(audit::has_pending_snapshot(fixture.workspace.audit_git())
            .expect("pending marker should inspect"));
        assert_eq!(
            execution.report.workspaces[0].doctor.status,
            ApplyStepStatus::Completed
        );
        assert_eq!(
            execution.report.workspaces[0].verify.status,
            ApplyStepStatus::Completed
        );
        assert!(event_types(&fixture.workspace).iter().any(|event| {
            event.0 == "audit.snapshot.pending"
                && event.1.as_deref() == Some(mutation::AUDIT_SNAPSHOT_PENDING)
        }));
        crate::observability::report::effectiveness_report(
            &fixture.workspace,
            &fixture.workspace_key,
        )
        .expect("pending audit event should satisfy report contracts");

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(fixture.root).expect("fixture should remove");
    }

    #[test]
    fn workspace_set_mismatch_is_an_exact_fail_closed_preflight_error() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (root, home, repo) = create_roots("set-mismatch");
        let home_guard = EnvGuard::set(&home);
        let cwd_guard = CurrentDirGuard::set(&repo);
        let paths = storage::resolve_paths().expect("paths should resolve");
        storage::ensure_global_dirs(&paths).expect("global directories should create");
        create_v010_workspace(&paths, "alpha-workspace");
        let plan =
            plan_all_workspaces_with_probe(&FixedDiskProbe(u64::MAX)).expect("plan should succeed");
        let candidates = Vec::new();

        let failure = preflight_failure(&plan, &candidates).expect("mismatch should fail");

        assert_eq!(failure.code, "WORKSPACE_SET_CHANGED");

        drop(cwd_guard);
        drop(home_guard);
        fs::remove_dir_all(root).expect("fixture should remove");
    }
}
