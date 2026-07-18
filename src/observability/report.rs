//! Read-only, fact-only Local Observability status and effectiveness reports.

use super::{
    open_reader, redact_sensitive_text, truncate_utf8, validate_ascii_identifier,
    validate_positive_id, validate_uuid_v4, EventOutcome, EventType, ObservabilityOpenError,
    ObservabilityReader, WorkspacePaths, OBSERVABILITY_RETENTION_DAYS,
    OBSERVABILITY_RETENTION_MAX_BYTES, OBSERVABILITY_SCHEMA_VERSION,
};
use crate::redaction::{TaggedValueRedactor, TEST_SECRET_REDACTION_MARKER};
use crate::storage;
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io;
use thiserror::Error;

const TOP_LIMIT: usize = 20;
#[cfg(test)]
const TOP_QUERY_LIMIT: usize = TOP_LIMIT + 1;
const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CollectionStatus {
    NotCollected,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReportTimestamp(String);

impl ReportTimestamp {
    pub(crate) fn parse(value: &str) -> Result<Self, ObserveReadError> {
        if !has_rfc3339_millis_shape(value) || !has_valid_timestamp_components(value) {
            return Err(ObserveReadError::InvalidStore);
        }
        Ok(Self(value.to_string()))
    }

    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for ReportTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

#[derive(Debug, Error)]
pub(crate) enum ObserveReadError {
    #[error("Local Observability path is unsafe")]
    UnsafePath,
    #[error("Local Observability store is invalid or incompatible")]
    InvalidStore,
    #[error("Local Observability store could not be read")]
    ReadFailed,
    #[error("Local Observability report clock failed")]
    ClockFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ObserveStatusResponse {
    pub product_version: String,
    pub workspace: String,
    pub collection_status: CollectionStatus,
    pub complete: bool,
    pub observability_schema_version: Option<u64>,
    pub facts: Option<ObserveStoreFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ObserveStoreFacts {
    pub observability_events: u64,
    pub recall_bundles: u64,
    pub bundle_nodes: u64,
    pub feedback: u64,
    pub first_recorded_at: Option<String>,
    pub last_recorded_at: Option<String>,
    pub last_retention_at: Option<String>,
    pub retention_floor_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EffectivenessReport {
    pub product_version: String,
    pub workspace: String,
    pub collection_status: CollectionStatus,
    pub complete: bool,
    pub observability_schema_version: Option<u64>,
    pub period: ReportPeriod,
    pub facts: Option<EffectivenessFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReportPeriod {
    pub days: u64,
    pub start_at: String,
    pub end_at: String,
    pub retention_max_bytes: u64,
    pub retention_floor_at: Option<String>,
    pub retention_truncated: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EffectivenessFacts {
    pub tasks: TaskFacts,
    pub recall: RecallFacts,
    pub nodes_selected_by_type: Vec<NamedCount>,
    pub most_selected: MostSelectedFacts,
    pub feedback: FeedbackFacts,
    pub tools: ToolFacts,
    pub repeated_correction_failure_mode_titles: TopList<RepeatedMemoryTitle>,
    pub reflection: ReflectionFacts,
    pub adapter_drift_events: AdapterDriftFacts,
    pub pending_audit_events: u64,
    pub tool_duplicate_blocks: u64,
    pub alias_resolutions: u64,
    pub unresolved_tool_overlaps: u64,
    pub last_successful_audit_repair_at: Option<String>,
    pub doctor_verify_failures: HealthFailureFacts,
    pub artifact_cleanup_deletions: ArtifactCleanupFacts,
    pub mcp: McpFacts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct TaskFacts {
    pub starts: u64,
    pub context_applications: u64,
    pub started_without_apply: u64,
    pub completed: u64,
    pub failed: u64,
    pub applied_gates: u64,
    pub applied_rules: u64,
    pub selected_workflows: u64,
    pub selected_tools: u64,
    pub corrections_applied: u64,
    pub failure_modes_applied: u64,
    pub applied_context_by_type: Vec<NamedCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct RecallFacts {
    pub count: u64,
    pub failed: u64,
    pub empty: u64,
    pub mandatory_overflow: u64,
    pub more_results_bundles: u64,
    pub terminal_more_results_bundles: u64,
    pub continuation_bundles: u64,
    pub continuation_invocations: u64,
    pub fts_fallback_bundles: u64,
    pub graph_traversal_bundles: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NamedCount {
    pub name: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct MostSelectedFacts {
    pub workflows: TopList<SelectedNodeCount>,
    pub tools: TopList<SelectedNodeCount>,
    pub failure_modes: TopList<SelectedNodeCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SelectedNodeCount {
    pub node_id: i64,
    pub title: String,
    pub bundles: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TopList<T> {
    pub limit: usize,
    pub more_results: bool,
    pub items: Vec<T>,
}

impl<T> Default for TopList<T> {
    fn default() -> Self {
        Self {
            limit: TOP_LIMIT,
            more_results: false,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct FeedbackFacts {
    pub useful: u64,
    pub partial: u64,
    pub wrong: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct ToolFacts {
    pub success: u64,
    pub failure: u64,
    pub timeout: u64,
    pub repeated_errors: TopList<RepeatedToolError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RepeatedToolError {
    pub tool_id: String,
    pub error_code: String,
    pub invocations: u64,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RepeatedMemoryTitle {
    pub node_type: String,
    pub title: String,
    pub selections: u64,
    pub distinct_nodes: u64,
    pub bundles: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct ReflectionFacts {
    pub proposed: EventItemCount,
    pub applied: EventItemCount,
    pub drafted: EventItemCount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct EventItemCount {
    pub events: u64,
    pub items: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct AdapterDriftFacts {
    pub missing: u64,
    pub drifted: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct HealthFailureFacts {
    pub doctor: u64,
    pub verify: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct ArtifactCleanupFacts {
    pub cleanup_events: u64,
    pub deleted_paths: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub(crate) struct McpFacts {
    pub missing_status_observations: u64,
    pub configured_unverified_status_observations: u64,
}

pub(crate) fn capture_report_timestamp() -> Result<ReportTimestamp, ObserveReadError> {
    let connection = Connection::open_in_memory().map_err(|_| ObserveReadError::ClockFailed)?;
    connection
        .execute_batch("PRAGMA query_only = ON;")
        .map_err(|_| ObserveReadError::ClockFailed)?;
    let value: String = connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(|_| ObserveReadError::ClockFailed)?;
    ReportTimestamp::parse(&value).map_err(|_| ObserveReadError::ClockFailed)
}

/// Builds a report from one SQLite read snapshot and one clock captured after
/// that snapshot has been established. This prevents a concurrent recall
/// continuation from leaking facts newer than the reported `end_at`.
pub(crate) fn effectiveness_report(
    workspace_paths: &WorkspacePaths,
    workspace_key: &str,
) -> Result<EffectivenessReport, ObserveReadError> {
    let redactor = report_redactor(workspace_paths)?;
    let Some(reader) = open_optional_reader(workspace_paths)? else {
        let captured_at = capture_report_timestamp()?;
        let mut report = not_collected_report(workspace_key, &captured_at)?;
        redact_report_identity(&mut report.workspace, &redactor)?;
        return Ok(report);
    };
    let transaction = reader
        .connection
        .unchecked_transaction()
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let captured_at = capture_snapshot_timestamp(&transaction)?;
    ready_report_from_snapshot(transaction, workspace_key, &captured_at, &redactor)
}

pub(crate) fn observe_status(
    workspace_paths: &WorkspacePaths,
    workspace_key: &str,
) -> Result<ObserveStatusResponse, ObserveReadError> {
    let redactor = report_redactor(workspace_paths)?;
    let Some(reader) = open_optional_reader(workspace_paths)? else {
        let workspace = redact_report_text(workspace_key, &redactor, 512)?;
        return Ok(ObserveStatusResponse {
            product_version: PRODUCT_VERSION.to_string(),
            workspace,
            collection_status: CollectionStatus::NotCollected,
            complete: false,
            observability_schema_version: None,
            facts: None,
        });
    };
    let transaction = reader
        .connection
        .unchecked_transaction()
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let facts = load_status_facts(&transaction)?;
    transaction
        .commit()
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let workspace = redact_report_text(workspace_key, &redactor, 512)?;
    Ok(ObserveStatusResponse {
        product_version: PRODUCT_VERSION.to_string(),
        workspace,
        collection_status: CollectionStatus::Ready,
        complete: true,
        observability_schema_version: Some(schema_version_u64()?),
        facts: Some(facts),
    })
}

#[cfg(test)]
pub(crate) fn effectiveness_report_at(
    workspace_paths: &WorkspacePaths,
    workspace_key: &str,
    captured_at: &ReportTimestamp,
) -> Result<EffectivenessReport, ObserveReadError> {
    let redactor = report_redactor(workspace_paths)?;
    let Some(reader) = open_optional_reader(workspace_paths)? else {
        let mut report = not_collected_report(workspace_key, captured_at)?;
        redact_report_identity(&mut report.workspace, &redactor)?;
        return Ok(report);
    };
    let transaction = reader
        .connection
        .unchecked_transaction()
        .map_err(|_| ObserveReadError::ReadFailed)?;
    establish_read_snapshot(&transaction)?;
    ready_report_from_snapshot(transaction, workspace_key, captured_at, &redactor)
}

pub(super) fn not_collected_report(
    workspace_key: &str,
    captured_at: &ReportTimestamp,
) -> Result<EffectivenessReport, ObserveReadError> {
    Ok(EffectivenessReport {
        product_version: PRODUCT_VERSION.to_string(),
        workspace: workspace_key.to_string(),
        collection_status: CollectionStatus::NotCollected,
        complete: false,
        observability_schema_version: None,
        period: base_period(captured_at)?,
        facts: None,
    })
}

fn ready_report_from_snapshot(
    transaction: Transaction<'_>,
    workspace_key: &str,
    captured_at: &ReportTimestamp,
    redactor: &TaggedValueRedactor,
) -> Result<EffectivenessReport, ObserveReadError> {
    let report =
        effectiveness_report_in_snapshot(&transaction, workspace_key, captured_at, redactor)?;
    transaction
        .commit()
        .map_err(|_| ObserveReadError::ReadFailed)?;
    Ok(report)
}

pub(super) fn effectiveness_report_in_snapshot(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    captured_at: &ReportTimestamp,
    redactor: &TaggedValueRedactor,
) -> Result<EffectivenessReport, ObserveReadError> {
    let start_at = period_start(captured_at)?;
    let base_period = base_period(captured_at)?;
    let (facts, retention_floor_at) = load_effectiveness_facts(
        transaction,
        workspace_key,
        start_at.as_str(),
        captured_at.as_str(),
        redactor,
    )?;
    let retention_truncated = retention_floor_at
        .as_deref()
        .is_some_and(|floor| floor >= start_at.as_str());
    let period = ReportPeriod {
        retention_floor_at,
        retention_truncated: Some(retention_truncated),
        ..base_period
    };
    Ok(EffectivenessReport {
        product_version: PRODUCT_VERSION.to_string(),
        workspace: redact_report_text(workspace_key, redactor, 512)?,
        collection_status: CollectionStatus::Ready,
        complete: !retention_truncated,
        observability_schema_version: Some(schema_version_u64()?),
        period,
        facts: Some(facts),
    })
}

fn base_period(captured_at: &ReportTimestamp) -> Result<ReportPeriod, ObserveReadError> {
    let start_at = period_start(captured_at)?;
    Ok(ReportPeriod {
        days: u64::try_from(OBSERVABILITY_RETENTION_DAYS)
            .map_err(|_| ObserveReadError::InvalidStore)?,
        start_at: start_at.as_str().to_string(),
        end_at: captured_at.as_str().to_string(),
        retention_max_bytes: OBSERVABILITY_RETENTION_MAX_BYTES,
        retention_floor_at: None,
        retention_truncated: None,
    })
}

#[cfg(test)]
fn establish_read_snapshot(transaction: &Transaction<'_>) -> Result<(), ObserveReadError> {
    let singleton_id: i64 = transaction
        .query_row(
            "SELECT singleton_id FROM collector_state WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ObserveReadError::InvalidStore)?;
    if singleton_id != 1 {
        return Err(ObserveReadError::InvalidStore);
    }
    Ok(())
}

fn capture_snapshot_timestamp(
    transaction: &Transaction<'_>,
) -> Result<ReportTimestamp, ObserveReadError> {
    let (singleton_id, value): (i64, String) = transaction
        .query_row(
            "SELECT singleton_id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             FROM collector_state WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ObserveReadError::ClockFailed)?;
    if singleton_id != 1 {
        return Err(ObserveReadError::InvalidStore);
    }
    ReportTimestamp::parse(&value).map_err(|_| ObserveReadError::ClockFailed)
}

fn open_optional_reader(
    workspace_paths: &WorkspacePaths,
) -> Result<Option<ObservabilityReader>, ObserveReadError> {
    match fs::symlink_metadata(workspace_paths.root()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ObserveReadError::UnsafePath),
        Ok(_) => {}
    }
    match open_reader(workspace_paths) {
        Ok(reader) => Ok(Some(reader)),
        Err(ObservabilityOpenError::Missing(_)) => Ok(None),
        Err(ObservabilityOpenError::UnsafePath { .. }) => Err(ObserveReadError::UnsafePath),
        Err(ObservabilityOpenError::InvalidStore { .. }) => Err(ObserveReadError::InvalidStore),
        Err(ObservabilityOpenError::Sqlite(_) | ObservabilityOpenError::Serialization(_)) => {
            Err(ObserveReadError::InvalidStore)
        }
    }
}

fn report_redactor(
    workspace_paths: &WorkspacePaths,
) -> Result<TaggedValueRedactor, ObserveReadError> {
    let mut connection = match storage::open_workspace_db_read_only(workspace_paths) {
        Ok(connection) => connection,
        Err(storage::OpenWorkspaceReadOnlyError::Missing(_)) => {
            return Ok(TaggedValueRedactor::default());
        }
        Err(
            storage::OpenWorkspaceReadOnlyError::UnsafePath(_)
            | storage::OpenWorkspaceReadOnlyError::Db(_),
        ) => return Err(ObserveReadError::ReadFailed),
    };
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let redactor =
        TaggedValueRedactor::load(&transaction).map_err(|_| ObserveReadError::ReadFailed)?;
    transaction
        .commit()
        .map_err(|_| ObserveReadError::ReadFailed)?;
    Ok(redactor)
}

fn redact_report_text(
    value: &str,
    redactor: &TaggedValueRedactor,
    maximum_bytes: usize,
) -> Result<String, ObserveReadError> {
    redactor
        .redact_str_bounded(value, maximum_bytes)
        .map_err(|_| ObserveReadError::ReadFailed)
}

fn redact_report_identity(
    value: &mut String,
    redactor: &TaggedValueRedactor,
) -> Result<(), ObserveReadError> {
    *value = redact_report_text(value, redactor, 512)?;
    Ok(())
}

fn schema_version_u64() -> Result<u64, ObserveReadError> {
    u64::try_from(OBSERVABILITY_SCHEMA_VERSION).map_err(|_| ObserveReadError::InvalidStore)
}

fn period_start(captured_at: &ReportTimestamp) -> Result<ReportTimestamp, ObserveReadError> {
    let connection = Connection::open_in_memory().map_err(|_| ObserveReadError::ClockFailed)?;
    connection
        .execute_batch("PRAGMA query_only = ON;")
        .map_err(|_| ObserveReadError::ClockFailed)?;
    let (normalized, start): (Option<String>, Option<String>) = connection
        .query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', ?1),
                    strftime('%Y-%m-%dT%H:%M:%fZ', ?1, '-30 days')",
            [captured_at.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ObserveReadError::ClockFailed)?;
    if normalized.as_deref() != Some(captured_at.as_str()) {
        return Err(ObserveReadError::ClockFailed);
    }
    ReportTimestamp::parse(start.as_deref().ok_or(ObserveReadError::ClockFailed)?)
        .map_err(|_| ObserveReadError::ClockFailed)
}

fn has_rfc3339_millis_shape(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 24
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'.',
            23 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

fn has_valid_timestamp_components(value: &str) -> bool {
    let parse = |start: usize, end: usize| value[start..end].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        parse(0, 4),
        parse(5, 7),
        parse(8, 10),
        parse(11, 13),
        parse(14, 16),
        parse(17, 19),
    ) else {
        return false;
    };
    let leap_year =
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day) && hour <= 23 && minute <= 59 && second <= 59
}

const STATUS_FACTS_SQL: &str = r#"
WITH stats(source, row_count, first_recorded_at, last_recorded_at) AS (
    SELECT 'observability_events', COUNT(*), MIN(timestamp), MAX(timestamp)
    FROM observability_events
    UNION ALL
    SELECT 'recall_bundles', COUNT(*), MIN(timestamp), MAX(timestamp)
    FROM recall_bundles
    UNION ALL
    SELECT 'bundle_nodes', COUNT(*), MIN(first_seen_at), MAX(first_seen_at)
    FROM bundle_nodes
    UNION ALL
    SELECT 'feedback', COUNT(*), MIN(timestamp), MAX(timestamp)
    FROM feedback
)
SELECT
    MAX(CASE WHEN source = 'observability_events' THEN row_count END),
    MAX(CASE WHEN source = 'recall_bundles' THEN row_count END),
    MAX(CASE WHEN source = 'bundle_nodes' THEN row_count END),
    MAX(CASE WHEN source = 'feedback' THEN row_count END),
    MIN(first_recorded_at),
    MAX(last_recorded_at)
FROM stats
"#;

fn load_status_facts(transaction: &Transaction<'_>) -> Result<ObserveStoreFacts, ObserveReadError> {
    let (
        observability_events,
        recall_bundles,
        bundle_nodes,
        feedback,
        first_recorded_at,
        last_recorded_at,
    ): (i64, i64, i64, i64, Option<String>, Option<String>) = transaction
        .query_row(STATUS_FACTS_SQL, [], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let observability_events = nonnegative(observability_events)?;
    let recall_bundles = nonnegative(recall_bundles)?;
    let bundle_nodes = nonnegative(bundle_nodes)?;
    let feedback = nonnegative(feedback)?;
    let (last_retention_at, retention_floor_at): (Option<String>, Option<String>) = transaction
        .query_row(
            "SELECT last_retention_at, retention_floor_at
             FROM collector_state WHERE singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ObserveReadError::ReadFailed)?;
    for timestamp in [
        first_recorded_at.as_deref(),
        last_recorded_at.as_deref(),
        last_retention_at.as_deref(),
        retention_floor_at.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        ReportTimestamp::parse(timestamp)?;
    }
    Ok(ObserveStoreFacts {
        observability_events,
        recall_bundles,
        bundle_nodes,
        feedback,
        first_recorded_at,
        last_recorded_at,
        last_retention_at,
        retention_floor_at,
    })
}

fn load_effectiveness_facts(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    start_at: &str,
    end_at: &str,
    redactor: &TaggedValueRedactor,
) -> Result<(EffectivenessFacts, Option<String>), ObserveReadError> {
    let tasks = load_task_facts(transaction, workspace_key, start_at, end_at)?;
    let mut event_facts = EventFacts::default();
    load_events(
        transaction,
        workspace_key,
        start_at,
        end_at,
        &mut event_facts,
        redactor,
    )?;
    let recall = RecallFacts {
        count: event_facts.recall_started,
        failed: event_facts.recall_failed,
        empty: event_facts.recall_empty,
        mandatory_overflow: event_facts.recall_mandatory_overflow,
        more_results_bundles: usize_u64(
            event_facts
                .recall_bundles
                .values()
                .filter(|bundle| bundle.ever_more_results)
                .count(),
        )?,
        terminal_more_results_bundles: usize_u64(
            event_facts
                .recall_bundles
                .values()
                .filter(|bundle| bundle.terminal_more_results == Some(true))
                .count(),
        )?,
        continuation_bundles: usize_u64(
            event_facts
                .recall_bundles
                .values()
                .filter(|bundle| bundle.continuation)
                .count(),
        )?,
        continuation_invocations: event_facts.continuation_invocations,
        fts_fallback_bundles: usize_u64(
            event_facts
                .recall_bundles
                .values()
                .filter(|bundle| bundle.fts_fallback)
                .count(),
        )?,
        graph_traversal_bundles: usize_u64(
            event_facts
                .recall_bundles
                .values()
                .filter(|bundle| bundle.graph_traversal)
                .count(),
        )?,
    };
    let selections = load_bundle_nodes(transaction, workspace_key, start_at, end_at, redactor)?;
    let feedback = load_feedback(transaction, workspace_key, start_at, end_at)?;
    let retention_floor_at: Option<String> = transaction
        .query_row(
            "SELECT retention_floor_at FROM collector_state WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ObserveReadError::ReadFailed)?;
    if let Some(floor) = retention_floor_at.as_deref() {
        ReportTimestamp::parse(floor)?;
    }

    Ok((
        EffectivenessFacts {
            tasks,
            recall,
            nodes_selected_by_type: selections.by_type,
            most_selected: selections.most_selected,
            feedback,
            tools: ToolFacts {
                success: event_facts.tool_success,
                failure: event_facts.tool_failure,
                timeout: event_facts.tool_timeout,
                repeated_errors: repeated_tool_errors(event_facts.tool_errors),
            },
            repeated_correction_failure_mode_titles: selections.repeated_titles,
            reflection: event_facts.reflection,
            adapter_drift_events: event_facts.adapter_drift,
            pending_audit_events: event_facts.pending_audit_events,
            tool_duplicate_blocks: event_facts.tool_duplicate_blocks,
            alias_resolutions: event_facts.alias_resolutions,
            unresolved_tool_overlaps: event_facts.unresolved_tool_overlaps,
            last_successful_audit_repair_at: event_facts.last_successful_audit_repair_at,
            doctor_verify_failures: event_facts.health_failures,
            artifact_cleanup_deletions: event_facts.artifact_cleanup,
            mcp: event_facts.mcp,
        },
        retention_floor_at,
    ))
}

const TASK_LIFECYCLE_FACTS_SQL: &str = r#"
SELECT
    (SELECT COUNT(*) FROM tasks INDEXED BY idx_tasks_started_at
     WHERE workspace_key = ?1 AND started_at >= ?2 AND started_at <= ?3),
    (SELECT COUNT(*) FROM tasks INDEXED BY idx_tasks_workspace_status
     WHERE workspace_key = ?1
       AND status IN ('applied', 'completed', 'failed')
       AND applied_at >= ?2 AND applied_at <= ?3),
    (SELECT COUNT(*) FROM tasks INDEXED BY idx_tasks_started_at
     WHERE workspace_key = ?1
       AND started_at >= ?2 AND started_at <= ?3
       AND (applied_at IS NULL OR applied_at > ?3)),
    (SELECT COUNT(*) FROM tasks INDEXED BY idx_tasks_workspace_status
     WHERE workspace_key = ?1 AND status = 'completed'
       AND finished_at >= ?2 AND finished_at <= ?3),
    (SELECT COUNT(*) FROM tasks INDEXED BY idx_tasks_workspace_status
     WHERE workspace_key = ?1 AND status = 'failed'
       AND finished_at >= ?2 AND finished_at <= ?3)
"#;

const TASK_APPLIED_FACTS_SQL: &str = r#"
SELECT applied.application_kind, bundled.context_kind, COUNT(*)
FROM tasks AS task INDEXED BY idx_tasks_workspace_status
JOIN task_applied_nodes AS applied USING (task_id)
JOIN task_bundle_nodes AS bundled USING (task_id, node_id)
WHERE task.workspace_key = ?1
  AND task.status IN ('applied', 'completed', 'failed')
  AND task.applied_at >= ?2
  AND task.applied_at <= ?3
GROUP BY applied.application_kind, bundled.context_kind
ORDER BY applied.application_kind, bundled.context_kind
"#;

fn load_task_facts(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    start_at: &str,
    end_at: &str,
) -> Result<TaskFacts, ObserveReadError> {
    let foreign_tasks: i64 = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM tasks WHERE workspace_key <> ?1 LIMIT 1
             )",
            [workspace_key],
            |row| row.get(0),
        )
        .map_err(|_| ObserveReadError::ReadFailed)?;
    if foreign_tasks != 0 {
        return Err(ObserveReadError::InvalidStore);
    }
    let lifecycle: (i64, i64, i64, i64, i64) = transaction
        .query_row(
            TASK_LIFECYCLE_FACTS_SQL,
            rusqlite::params![workspace_key, start_at, end_at],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let mut facts = TaskFacts {
        starts: nonnegative(lifecycle.0)?,
        context_applications: nonnegative(lifecycle.1)?,
        started_without_apply: nonnegative(lifecycle.2)?,
        completed: nonnegative(lifecycle.3)?,
        failed: nonnegative(lifecycle.4)?,
        ..TaskFacts::default()
    };
    let mut context_counts = BTreeMap::<String, u64>::new();
    let mut statement = transaction
        .prepare(TASK_APPLIED_FACTS_SQL)
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let mut rows = statement
        .query(rusqlite::params![workspace_key, start_at, end_at])
        .map_err(|_| ObserveReadError::ReadFailed)?;
    while let Some(row) = rows.next().map_err(|_| ObserveReadError::ReadFailed)? {
        let application_kind: String = row.get(0).map_err(|_| ObserveReadError::InvalidStore)?;
        let context_kind: String = row.get(1).map_err(|_| ObserveReadError::InvalidStore)?;
        let count = nonnegative(row.get(2).map_err(|_| ObserveReadError::InvalidStore)?)?;
        match application_kind.as_str() {
            "gate" => facts.applied_gates = checked_add(facts.applied_gates, count)?,
            "rule" => facts.applied_rules = checked_add(facts.applied_rules, count)?,
            "workflow" => {
                facts.selected_workflows = checked_add(facts.selected_workflows, count)?;
            }
            "tool" => facts.selected_tools = checked_add(facts.selected_tools, count)?,
            "correction" => {
                facts.corrections_applied = checked_add(facts.corrections_applied, count)?;
            }
            "failure_mode" => {
                facts.failure_modes_applied = checked_add(facts.failure_modes_applied, count)?;
            }
            _ => return Err(ObserveReadError::InvalidStore),
        }
        if !matches!(context_kind.as_str(), "mandatory" | "task") {
            return Err(ObserveReadError::InvalidStore);
        }
        let aggregate = context_counts.entry(context_kind).or_default();
        *aggregate = checked_add(*aggregate, count)?;
    }
    facts.applied_context_by_type = context_counts
        .into_iter()
        .map(|(name, count)| NamedCount { name, count })
        .collect();
    Ok(facts)
}

#[derive(Debug, Default)]
struct EventFacts {
    recall_started: u64,
    recall_failed: u64,
    recall_empty: u64,
    recall_mandatory_overflow: u64,
    recall_bundles: HashMap<String, RecallBundleFacts>,
    continuation_invocations: u64,
    tool_success: u64,
    tool_failure: u64,
    tool_timeout: u64,
    tool_terminals: HashSet<String>,
    tool_errors: HashMap<(String, String), ToolErrorAggregate>,
    reflection: ReflectionFacts,
    adapter_drift: AdapterDriftFacts,
    pending_audit_events: u64,
    tool_duplicate_blocks: u64,
    alias_resolutions: u64,
    unresolved_tool_overlaps: u64,
    last_successful_audit_repair_at: Option<String>,
    health_failures: HealthFailureFacts,
    artifact_cleanup: ArtifactCleanupFacts,
    mcp: McpFacts,
}

#[derive(Debug, Default)]
struct RecallBundleFacts {
    ever_more_results: bool,
    terminal_more_results: Option<bool>,
    continuation: bool,
    fts_fallback: bool,
    graph_traversal: bool,
}

#[derive(Debug)]
struct ToolErrorAggregate {
    invocations: u64,
    last_seen_at: String,
}

fn load_events(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    start_at: &str,
    end_at: &str,
    facts: &mut EventFacts,
    redactor: &TaggedValueRedactor,
) -> Result<(), ObserveReadError> {
    let mut statement = transaction
        .prepare(
            "SELECT id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, bundle_id, duration_ms, outcome,
                    error_code, payload_json
             FROM observability_events
             WHERE timestamp >= ?1 AND timestamp <= ?2
             ORDER BY timestamp, id",
        )
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let mut rows = statement
        .query([start_at, end_at])
        .map_err(|_| ObserveReadError::ReadFailed)?;
    while let Some(row) = rows.next().map_err(|_| ObserveReadError::ReadFailed)? {
        let event = StoredEvent::from_row(row, workspace_key)?;
        validate_event_contract(&event)?;
        accumulate_event(event, facts, redactor)?;
    }
    Ok(())
}

struct StoredEvent {
    timestamp: String,
    event_type: EventType,
    correlation_id: String,
    bundle_id: Option<String>,
    outcome: EventOutcome,
    error_code: Option<String>,
    payload: StoredPayload,
}

impl StoredEvent {
    fn from_row(row: &rusqlite::Row<'_>, workspace_key: &str) -> Result<Self, ObserveReadError> {
        let id: String = row.get(0).map_err(|_| ObserveReadError::InvalidStore)?;
        let timestamp: String = row.get(1).map_err(|_| ObserveReadError::InvalidStore)?;
        let product_version: String = row.get(2).map_err(|_| ObserveReadError::InvalidStore)?;
        let stored_workspace: String = row.get(3).map_err(|_| ObserveReadError::InvalidStore)?;
        let event_type_text: String = row.get(4).map_err(|_| ObserveReadError::InvalidStore)?;
        let command: String = row.get(5).map_err(|_| ObserveReadError::InvalidStore)?;
        let correlation_id: String = row.get(6).map_err(|_| ObserveReadError::InvalidStore)?;
        let bundle_id: Option<String> = row.get(7).map_err(|_| ObserveReadError::InvalidStore)?;
        let duration_ms: Option<i64> = row.get(8).map_err(|_| ObserveReadError::InvalidStore)?;
        let outcome_text: String = row.get(9).map_err(|_| ObserveReadError::InvalidStore)?;
        let error_code: Option<String> = row.get(10).map_err(|_| ObserveReadError::InvalidStore)?;
        let payload_json = row
            .get_ref(11)
            .map_err(|_| ObserveReadError::InvalidStore)?
            .as_str()
            .map_err(|_| ObserveReadError::InvalidStore)?;
        validate_uuid_v4(&id).map_err(|_| ObserveReadError::InvalidStore)?;
        validate_uuid_v4(&correlation_id).map_err(|_| ObserveReadError::InvalidStore)?;
        ReportTimestamp::parse(&timestamp)?;
        if stored_workspace != workspace_key
            || product_version.as_bytes().contains(&0)
            || product_version.trim().is_empty()
            || product_version.len() > 128
            || !is_identifier(&command)
            || duration_ms.is_some_and(|value| value < 0)
            || error_code
                .as_deref()
                .is_some_and(|value| value != TEST_SECRET_REDACTION_MARKER && !is_identifier(value))
        {
            return Err(ObserveReadError::InvalidStore);
        }
        if let Some(bundle_id) = bundle_id.as_deref() {
            validate_uuid_v4(bundle_id).map_err(|_| ObserveReadError::InvalidStore)?;
        }
        let event_type = parse_event_type(&event_type_text)?;
        let outcome = parse_event_outcome(&outcome_text)?;
        let payload: StoredPayload =
            serde_json::from_str(payload_json).map_err(|_| ObserveReadError::InvalidStore)?;
        payload.validate()?;
        Ok(Self {
            timestamp,
            event_type,
            correlation_id,
            bundle_id,
            outcome,
            error_code,
            payload,
        })
    }
}

pub(super) fn validate_event_row(
    row: &rusqlite::Row<'_>,
    workspace_key: &str,
) -> Result<(), ObserveReadError> {
    let event = StoredEvent::from_row(row, workspace_key)?;
    validate_event_contract(&event)
}

fn validate_event_contract(event: &StoredEvent) -> Result<(), ObserveReadError> {
    use EventOutcome as O;
    use EventType as T;
    use StoredPayload as P;

    let valid_shape = matches!(
        (event.event_type, event.outcome, &event.payload),
        (T::InstallStarted | T::UpdateStarted, O::Started, P::Empty)
            | (
                T::InstallCompleted | T::UpdateCompleted,
                O::Success,
                P::Empty
            )
            | (T::InstallFailed | T::UpdateFailed, O::Failure, P::Empty)
            | (T::WorkspaceInit, O::Success, P::Counts(_))
            | (
                T::AdapterSeed | T::AdapterSync,
                O::Success | O::Failure,
                P::Empty
            )
            | (
                T::AdapterDrift,
                O::Missing | O::Warning | O::Failure,
                P::Empty
            )
            | (T::RecallStarted, O::Started, P::Empty)
            | (T::RecallCompleted, O::Success, P::Recall(_))
            | (T::RecallFailed, O::Failure, P::Empty)
            | (T::RecallContinuation, O::Recorded, P::Empty)
            | (T::RecallEmpty, O::Empty, P::Empty)
            | (T::RecallTruncated, O::Truncated, P::Empty)
            | (T::RecallMandatoryOverflow, O::Overflow, P::Counts(_))
            | (T::NodeCreated | T::Remember, O::Recorded, P::Node(_))
            | (T::NodeUpdated | T::NodeDeprecated, O::Success, P::Node(_))
            | (
                T::NodeCreated | T::NodeUpdated | T::NodeDeprecated | T::Remember,
                O::Failure,
                P::Empty
            )
            | (T::LinkCreated, O::Recorded, P::Link(_))
            | (T::LinkCreated, O::Failure, P::Empty)
            | (T::TeachStarted, O::Started | O::Failure, P::Empty)
            | (T::TeachProposed, O::Proposed | O::Failure, P::Empty)
            | (T::TeachApplied, O::Applied | O::Failure, P::Empty)
            | (T::ReflectionInventory, O::Success | O::Failure, P::Empty)
            | (T::ReflectionProposal, O::Proposed, P::Counts(_))
            | (T::ReflectionProposal, O::Failure, P::Empty)
            | (T::ReflectionApplied, O::Applied | O::Drafted, P::Counts(_))
            | (T::ReflectionApplied, O::Failure, P::Empty)
            | (
                T::ToolValidation,
                O::Success | O::Failure | O::Blocked,
                P::Tool(_)
            )
            | (T::ToolRunStarted, O::Started, P::Tool(_))
            | (T::ToolRunCompleted, O::Success, P::Tool(_))
            | (T::ToolRunFailed, O::Failure, P::Tool(_))
            | (T::ToolRunTimeout, O::Timeout, P::Tool(_))
            | (T::ToolOutputArtifact, O::Recorded, P::Artifact(_))
            | (
                T::McpStatus,
                O::Configured | O::Missing | O::ConfiguredUnverified,
                P::Mcp(_)
            )
            | (T::McpStatus, O::Success | O::Truncated, P::Counts(_))
            | (
                T::Doctor | T::Verify,
                O::Success | O::Warning | O::Failure,
                P::Counts(_)
            )
            | (T::AuditSnapshotCompleted, O::Success, P::Counts(_))
            | (T::AuditSnapshotPending, O::Pending, P::Counts(_))
            | (T::AuditSnapshotFailed, O::Failure, P::Counts(_))
            | (
                T::ArtifactsCleanup,
                O::Success | O::Warning | O::Failure,
                P::Counts(_)
            )
            | (T::FeedbackRecorded, O::Recorded, P::Empty)
            | (T::TaskStarted, O::Started, P::Task(_))
            | (T::TaskContextApplied, O::Applied, P::Task(_))
            | (T::TaskCompleted, O::Success, P::Task(_))
            | (T::TaskFailed, O::Failure, P::Task(_))
            | (
                T::ToolDuplicateDetected,
                O::Recorded | O::Warning,
                P::Tool(_)
            )
            | (T::ToolDuplicateBlocked, O::Blocked, P::Tool(_))
            | (
                T::ToolAliasCreated | T::ToolAliasResolved | T::ToolCanonicalized,
                O::Recorded | O::Success,
                P::Tool(_)
            )
            | (T::AuditRepairStarted, O::Started, P::Empty)
            | (T::AuditRepairCompleted, O::Success, P::Empty)
            | (T::AuditRepairFailed, O::Failure, P::Empty)
            | (T::PlatformCheckCompleted, O::Success | O::Failure, P::Empty)
    );
    if !valid_shape {
        return Err(ObserveReadError::InvalidStore);
    }
    if event.event_type.is_recall() && event.bundle_id.is_none() {
        return Err(ObserveReadError::InvalidStore);
    }
    if event.event_type == EventType::FeedbackRecorded && event.bundle_id.is_none() {
        return Err(ObserveReadError::InvalidStore);
    }
    if event.event_type.is_task() && event.bundle_id.is_none() {
        return Err(ObserveReadError::InvalidStore);
    }
    let error_required = event.outcome == EventOutcome::Failure
        || event.outcome == EventOutcome::Timeout
        || event.outcome == EventOutcome::Blocked
        || event.event_type == EventType::AuditSnapshotPending
        || (event.event_type == EventType::ArtifactsCleanup
            && event.outcome == EventOutcome::Warning);
    if error_required != event.error_code.is_some() {
        return Err(ObserveReadError::InvalidStore);
    }
    validate_count_contract(event)
}

fn validate_count_contract(event: &StoredEvent) -> Result<(), ObserveReadError> {
    let StoredPayload::Counts(counts) = &event.payload else {
        return Ok(());
    };
    let keys = counts.map()?;
    let expected: Option<&[&str]> = match event.event_type {
        EventType::WorkspaceInit => Some(&[
            "seeded_nodes_created",
            "seeded_nodes_existing",
            "semantic_nodes_created",
            "semantic_nodes_existing",
        ]),
        EventType::ReflectionProposal | EventType::ReflectionApplied => Some(&["items"]),
        EventType::RecallMandatoryOverflow => Some(&["offending_nodes"]),
        EventType::Doctor if event.outcome != EventOutcome::Failure => {
            Some(&["checks", "ready", "missing", "error"])
        }
        EventType::Verify if event.outcome != EventOutcome::Failure => Some(&[
            "total",
            "duplicate_ids",
            "broken_links",
            "deprecated_active_links",
            "missing_source",
            "missing_summary",
            "missing_gates",
            "adapter_block_drift",
            "schema_drift",
            "forbidden_feature_terms",
            "pending_audit_snapshot",
        ]),
        EventType::Doctor | EventType::Verify => Some(&[]),
        EventType::AuditSnapshotCompleted => Some(&["duration_ms", "bytes_written"]),
        EventType::AuditSnapshotPending | EventType::AuditSnapshotFailed => Some(&["duration_ms"]),
        EventType::McpStatus => Some(&[
            "profiles",
            "installed",
            "missing",
            "configured_unverified",
            "unrecognized",
            "more_results",
        ]),
        EventType::ArtifactsCleanup => None,
        _ => return Err(ObserveReadError::InvalidStore),
    };
    if let Some(expected) = expected {
        require_exact_keys(&keys, expected)?;
    } else {
        let allowed = [
            "bytes_before",
            "bytes_after",
            "deleted_dirs",
            "deleted_files",
            "deleted_paths",
            "kept_dirs",
            "complete",
        ];
        let full = keys.len() == allowed.len() && allowed.iter().all(|key| keys.contains_key(*key));
        let failure_only = keys.len() == 1 && keys.contains_key("deleted_paths");
        let valid = match event.outcome {
            EventOutcome::Success | EventOutcome::Warning => full,
            EventOutcome::Failure => full || failure_only,
            _ => false,
        };
        if !valid {
            return Err(ObserveReadError::InvalidStore);
        }
    }
    Ok(())
}

fn accumulate_event(
    event: StoredEvent,
    facts: &mut EventFacts,
    redactor: &TaggedValueRedactor,
) -> Result<(), ObserveReadError> {
    let StoredEvent {
        timestamp,
        event_type,
        correlation_id,
        bundle_id,
        outcome,
        error_code,
        payload,
    } = event;
    match (event_type, outcome, payload) {
        (EventType::RecallStarted, EventOutcome::Started, StoredPayload::Empty) => {
            facts.recall_started = checked_increment(facts.recall_started)?;
        }
        (EventType::RecallFailed, EventOutcome::Failure, StoredPayload::Empty) => {
            facts.recall_failed = checked_increment(facts.recall_failed)?;
        }
        (EventType::RecallContinuation, EventOutcome::Recorded, StoredPayload::Empty) => {
            let bundle_id = bundle_id.ok_or(ObserveReadError::InvalidStore)?;
            facts
                .recall_bundles
                .entry(bundle_id)
                .or_default()
                .continuation = true;
            facts.continuation_invocations = checked_increment(facts.continuation_invocations)?;
        }
        (EventType::RecallCompleted, EventOutcome::Success, StoredPayload::Recall(payload)) => {
            let bundle_id = bundle_id.ok_or(ObserveReadError::InvalidStore)?;
            let bundle = facts.recall_bundles.entry(bundle_id).or_default();
            bundle.ever_more_results |= payload.more_results;
            bundle.terminal_more_results = Some(payload.more_results);
            bundle.fts_fallback |= payload.fts_fallback_used;
            bundle.graph_traversal |= payload.graph_traversal_used;
        }
        (EventType::RecallEmpty, EventOutcome::Empty, StoredPayload::Empty) => {
            facts.recall_empty = checked_increment(facts.recall_empty)?;
        }
        (EventType::RecallMandatoryOverflow, EventOutcome::Overflow, StoredPayload::Counts(_)) => {
            facts.recall_mandatory_overflow = checked_increment(facts.recall_mandatory_overflow)?;
        }
        (EventType::ToolRunCompleted, EventOutcome::Success, StoredPayload::Tool(_)) => {
            record_tool_terminal(&correlation_id, &mut facts.tool_terminals)?;
            facts.tool_success = checked_increment(facts.tool_success)?;
        }
        (EventType::ToolRunFailed, EventOutcome::Failure, StoredPayload::Tool(payload)) => {
            record_tool_terminal(&correlation_id, &mut facts.tool_terminals)?;
            facts.tool_failure = checked_increment(facts.tool_failure)?;
            record_tool_error(timestamp, error_code, payload, facts, redactor)?;
        }
        (EventType::ToolRunTimeout, EventOutcome::Timeout, StoredPayload::Tool(payload)) => {
            record_tool_terminal(&correlation_id, &mut facts.tool_terminals)?;
            facts.tool_timeout = checked_increment(facts.tool_timeout)?;
            record_tool_error(timestamp, error_code, payload, facts, redactor)?;
        }
        (EventType::ReflectionProposal, EventOutcome::Proposed, StoredPayload::Counts(counts)) => {
            record_event_items(&mut facts.reflection.proposed, counts)?;
        }
        (EventType::ReflectionApplied, EventOutcome::Applied, StoredPayload::Counts(counts)) => {
            record_event_items(&mut facts.reflection.applied, counts)?;
        }
        (EventType::ReflectionApplied, EventOutcome::Drafted, StoredPayload::Counts(counts)) => {
            record_event_items(&mut facts.reflection.drafted, counts)?;
        }
        (EventType::AdapterDrift, EventOutcome::Missing, StoredPayload::Empty) => {
            facts.adapter_drift.missing = checked_increment(facts.adapter_drift.missing)?;
        }
        (EventType::AdapterDrift, EventOutcome::Warning, StoredPayload::Empty) => {
            facts.adapter_drift.drifted = checked_increment(facts.adapter_drift.drifted)?;
        }
        (EventType::AdapterDrift, EventOutcome::Failure, StoredPayload::Empty) => {
            facts.adapter_drift.failed = checked_increment(facts.adapter_drift.failed)?;
        }
        (EventType::AuditSnapshotPending, EventOutcome::Pending, StoredPayload::Counts(_)) => {
            facts.pending_audit_events = checked_increment(facts.pending_audit_events)?;
        }
        (EventType::ToolDuplicateBlocked, EventOutcome::Blocked, StoredPayload::Tool(_)) => {
            facts.tool_duplicate_blocks = checked_increment(facts.tool_duplicate_blocks)?;
            if error_code.as_deref() == Some("TOOL_OVERLAP_REVIEW_REQUIRED") {
                facts.unresolved_tool_overlaps = checked_increment(facts.unresolved_tool_overlaps)?;
            }
        }
        (EventType::ToolAliasResolved, _, StoredPayload::Tool(_)) => {
            facts.alias_resolutions = checked_increment(facts.alias_resolutions)?;
        }
        (EventType::AuditRepairCompleted, EventOutcome::Success, StoredPayload::Empty)
            if facts
                .last_successful_audit_repair_at
                .as_deref()
                .is_none_or(|current| current < timestamp.as_str()) =>
        {
            facts.last_successful_audit_repair_at = Some(timestamp);
        }
        (EventType::Doctor, EventOutcome::Failure, StoredPayload::Counts(_)) => {
            facts.health_failures.doctor = checked_increment(facts.health_failures.doctor)?;
        }
        (EventType::Verify, EventOutcome::Failure, StoredPayload::Counts(_)) => {
            facts.health_failures.verify = checked_increment(facts.health_failures.verify)?;
        }
        (EventType::ArtifactsCleanup, _, StoredPayload::Counts(counts)) => {
            let counts = counts.map()?;
            facts.artifact_cleanup.cleanup_events =
                checked_increment(facts.artifact_cleanup.cleanup_events)?;
            facts.artifact_cleanup.deleted_paths = facts
                .artifact_cleanup
                .deleted_paths
                .checked_add(required_count(&counts, "deleted_paths")?)
                .ok_or(ObserveReadError::InvalidStore)?;
        }
        (EventType::McpStatus, _, StoredPayload::Mcp(payload)) => {
            record_mcp_single(outcome, payload.status, &mut facts.mcp)?;
        }
        (
            EventType::McpStatus,
            EventOutcome::Success | EventOutcome::Truncated,
            StoredPayload::Counts(counts),
        ) => {
            let counts = counts.map()?;
            facts.mcp.missing_status_observations = facts
                .mcp
                .missing_status_observations
                .checked_add(required_count(&counts, "missing")?)
                .ok_or(ObserveReadError::InvalidStore)?;
            facts.mcp.configured_unverified_status_observations = facts
                .mcp
                .configured_unverified_status_observations
                .checked_add(required_count(&counts, "configured_unverified")?)
                .ok_or(ObserveReadError::InvalidStore)?;
        }
        _ => {}
    }
    Ok(())
}

fn record_tool_terminal(
    correlation_id: &str,
    terminals: &mut HashSet<String>,
) -> Result<(), ObserveReadError> {
    if terminals.insert(correlation_id.to_string()) {
        Ok(())
    } else {
        Err(ObserveReadError::InvalidStore)
    }
}

fn record_tool_error(
    timestamp: String,
    error_code: Option<String>,
    payload: StoredTool,
    facts: &mut EventFacts,
    redactor: &TaggedValueRedactor,
) -> Result<(), ObserveReadError> {
    let error_code = safe_title(&error_code.ok_or(ObserveReadError::InvalidStore)?, redactor)?;
    let key = (safe_title(&payload.tool_id, redactor)?, error_code);
    let aggregate = facts.tool_errors.entry(key).or_insert(ToolErrorAggregate {
        invocations: 0,
        last_seen_at: timestamp.clone(),
    });
    aggregate.invocations = checked_increment(aggregate.invocations)?;
    if timestamp > aggregate.last_seen_at {
        aggregate.last_seen_at = timestamp;
    }
    Ok(())
}

fn repeated_tool_errors(
    aggregates: HashMap<(String, String), ToolErrorAggregate>,
) -> TopList<RepeatedToolError> {
    let items = aggregates
        .into_iter()
        .filter(|(_, aggregate)| aggregate.invocations >= 2)
        .map(|((tool_id, error_code), aggregate)| RepeatedToolError {
            tool_id,
            error_code,
            invocations: aggregate.invocations,
            last_seen_at: aggregate.last_seen_at,
        })
        .collect::<Vec<_>>();
    bounded_top_by(items, |left, right| {
        right
            .invocations
            .cmp(&left.invocations)
            .then_with(|| left.tool_id.cmp(&right.tool_id))
            .then_with(|| left.error_code.cmp(&right.error_code))
    })
}

fn record_event_items(
    aggregate: &mut EventItemCount,
    counts: StoredCounts,
) -> Result<(), ObserveReadError> {
    let counts = counts.map()?;
    aggregate.events = checked_increment(aggregate.events)?;
    aggregate.items = aggregate
        .items
        .checked_add(required_count(&counts, "items")?)
        .ok_or(ObserveReadError::InvalidStore)?;
    Ok(())
}

fn record_mcp_single(
    outcome: EventOutcome,
    status: StoredMcpStatus,
    facts: &mut McpFacts,
) -> Result<(), ObserveReadError> {
    match (status, outcome) {
        (StoredMcpStatus::Installed, EventOutcome::Configured) => Ok(()),
        (StoredMcpStatus::Missing, EventOutcome::Missing) => {
            facts.missing_status_observations =
                checked_increment(facts.missing_status_observations)?;
            Ok(())
        }
        (StoredMcpStatus::ConfiguredUnverified, EventOutcome::ConfiguredUnverified) => {
            facts.configured_unverified_status_observations =
                checked_increment(facts.configured_unverified_status_observations)?;
            Ok(())
        }
        _ => Err(ObserveReadError::InvalidStore),
    }
}

struct SelectionFacts {
    by_type: Vec<NamedCount>,
    most_selected: MostSelectedFacts,
    repeated_titles: TopList<RepeatedMemoryTitle>,
}

#[derive(Debug)]
struct SelectedNodeAggregate {
    title: String,
    latest_at: String,
    bundles: u64,
}

#[derive(Debug, Default)]
struct RepeatedTitleAggregate {
    selections: u64,
    nodes: HashSet<i64>,
    bundles: HashSet<String>,
}

fn load_bundle_nodes(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    start_at: &str,
    end_at: &str,
    redactor: &TaggedValueRedactor,
) -> Result<SelectionFacts, ObserveReadError> {
    let mut statement = transaction
        .prepare(
            "SELECT node.bundle_id, node.node_id, node.first_seen_at,
                    node.node_type, node.node_title, node.bounded_summary,
                    node.source_ref, node.trust_level, node.confidence,
                    node.score, node.selection_reasons_json, bundle.workspace_key
             FROM bundle_nodes AS node
             JOIN recall_bundles AS bundle USING (bundle_id)
             WHERE node.first_seen_at >= ?1 AND node.first_seen_at <= ?2
             ORDER BY node.bundle_id, node.node_id",
        )
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let mut rows = statement
        .query([start_at, end_at])
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let mut by_type = BTreeMap::<String, u64>::new();
    let mut selected = HashMap::<(String, i64), SelectedNodeAggregate>::new();
    let mut repeated = HashMap::<(String, String), RepeatedTitleAggregate>::new();
    while let Some(row) = rows.next().map_err(|_| ObserveReadError::ReadFailed)? {
        let bundle_id: String = row.get(0).map_err(|_| ObserveReadError::InvalidStore)?;
        let node_id: i64 = row.get(1).map_err(|_| ObserveReadError::InvalidStore)?;
        let first_seen_at: String = row.get(2).map_err(|_| ObserveReadError::InvalidStore)?;
        let node_type: String = row.get(3).map_err(|_| ObserveReadError::InvalidStore)?;
        let node_title: String = row.get(4).map_err(|_| ObserveReadError::InvalidStore)?;
        let bounded_summary: Option<String> =
            row.get(5).map_err(|_| ObserveReadError::InvalidStore)?;
        let source_ref: Option<String> = row.get(6).map_err(|_| ObserveReadError::InvalidStore)?;
        let trust_level: Option<String> = row.get(7).map_err(|_| ObserveReadError::InvalidStore)?;
        let confidence: Option<f64> = row.get(8).map_err(|_| ObserveReadError::InvalidStore)?;
        let score: Option<f64> = row.get(9).map_err(|_| ObserveReadError::InvalidStore)?;
        let reasons_json: String = row.get(10).map_err(|_| ObserveReadError::InvalidStore)?;
        let stored_workspace: String = row.get(11).map_err(|_| ObserveReadError::InvalidStore)?;
        validate_uuid_v4(&bundle_id).map_err(|_| ObserveReadError::InvalidStore)?;
        validate_positive_id("node_id", node_id).map_err(|_| ObserveReadError::InvalidStore)?;
        ReportTimestamp::parse(&first_seen_at)?;
        if stored_workspace != workspace_key
            || !storage::ALLOWED_NODE_TYPES.contains(&node_type.as_str())
            || node_title.as_bytes().contains(&0)
            || node_title.trim().is_empty()
            || node_title.len() > 512
            || bounded_summary
                .as_ref()
                .is_some_and(|value| value.as_bytes().contains(&0) || value.len() > 2_048)
            || source_ref
                .as_ref()
                .is_some_and(|value| value.as_bytes().contains(&0) || value.len() > 2_048)
            || trust_level.as_ref().is_some_and(|value| {
                value.as_bytes().contains(&0)
                    || value.trim().is_empty()
                    || value.len() > storage::MAX_NODE_TRUST_LEVEL_BYTES
            })
            || confidence.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || score.is_some_and(|value| !value.is_finite())
        {
            return Err(ObserveReadError::InvalidStore);
        }
        let reasons: Vec<StoredSelectionReason> =
            serde_json::from_str(&reasons_json).map_err(|_| ObserveReadError::InvalidStore)?;
        let unique_reasons = reasons.iter().copied().collect::<BTreeSet<_>>();
        if reasons.is_empty() || reasons.len() > 64 || unique_reasons.len() != reasons.len() {
            return Err(ObserveReadError::InvalidStore);
        }
        let title = safe_title(&node_title, redactor)?;
        let count = by_type.entry(node_type.clone()).or_default();
        *count = checked_increment(*count)?;
        if matches!(
            node_type.as_str(),
            "workflow" | "tool_contract" | "failure_mode"
        ) {
            let aggregate =
                selected
                    .entry((node_type.clone(), node_id))
                    .or_insert(SelectedNodeAggregate {
                        title: title.clone(),
                        latest_at: first_seen_at.clone(),
                        bundles: 0,
                    });
            aggregate.bundles = checked_increment(aggregate.bundles)?;
            if first_seen_at > aggregate.latest_at {
                aggregate.latest_at = first_seen_at.clone();
                aggregate.title = title.clone();
            }
        }
        if matches!(node_type.as_str(), "correction" | "failure_mode") {
            let aggregate = repeated.entry((node_type, title)).or_default();
            aggregate.selections = checked_increment(aggregate.selections)?;
            aggregate.nodes.insert(node_id);
            aggregate.bundles.insert(bundle_id);
        }
    }
    let by_type = by_type
        .into_iter()
        .map(|(name, count)| NamedCount { name, count })
        .collect();
    let most_selected = selected_nodes_by_type(selected);
    let repeated_titles = repeated
        .into_iter()
        .filter(|(_, aggregate)| aggregate.selections >= 2)
        .map(|((node_type, title), aggregate)| {
            Ok(RepeatedMemoryTitle {
                node_type,
                title,
                selections: aggregate.selections,
                distinct_nodes: usize_u64(aggregate.nodes.len())?,
                bundles: usize_u64(aggregate.bundles.len())?,
            })
        })
        .collect::<Result<Vec<_>, ObserveReadError>>()?;
    Ok(SelectionFacts {
        by_type,
        most_selected,
        repeated_titles: bounded_top_by(repeated_titles, |left, right| {
            right
                .selections
                .cmp(&left.selections)
                .then_with(|| left.node_type.cmp(&right.node_type))
                .then_with(|| left.title.cmp(&right.title))
        }),
    })
}

fn selected_nodes_by_type(
    selected: HashMap<(String, i64), SelectedNodeAggregate>,
) -> MostSelectedFacts {
    let mut workflows = Vec::new();
    let mut tools = Vec::new();
    let mut failure_modes = Vec::new();
    for ((node_type, node_id), aggregate) in selected {
        let item = SelectedNodeCount {
            node_id,
            title: aggregate.title,
            bundles: aggregate.bundles,
        };
        match node_type.as_str() {
            "workflow" => workflows.push(item),
            "tool_contract" => tools.push(item),
            "failure_mode" => failure_modes.push(item),
            _ => {}
        }
    }
    let order = |left: &SelectedNodeCount, right: &SelectedNodeCount| {
        right
            .bundles
            .cmp(&left.bundles)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.node_id.cmp(&right.node_id))
    };
    MostSelectedFacts {
        workflows: bounded_top_by(workflows, order),
        tools: bounded_top_by(tools, order),
        failure_modes: bounded_top_by(failure_modes, order),
    }
}

fn bounded_top_by<T>(
    mut items: Vec<T>,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> TopList<T> {
    let more_results = items.len() > TOP_LIMIT;
    if more_results {
        {
            let (top, _, _) = items.select_nth_unstable_by(TOP_LIMIT, &compare);
            top.sort_by(&compare);
        }
        items.truncate(TOP_LIMIT);
    } else {
        items.sort_by(&compare);
    }
    TopList {
        limit: TOP_LIMIT,
        more_results,
        items,
    }
}

fn load_feedback(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    start_at: &str,
    end_at: &str,
) -> Result<FeedbackFacts, ObserveReadError> {
    let mut statement = transaction
        .prepare(
            "SELECT feedback.id, feedback.timestamp, feedback.bundle_id,
                    feedback.outcome, feedback.reason, bundle.workspace_key
             FROM feedback
             JOIN recall_bundles AS bundle USING (bundle_id)
             WHERE feedback.timestamp >= ?1 AND feedback.timestamp <= ?2
             ORDER BY feedback.timestamp, feedback.id",
        )
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let mut rows = statement
        .query([start_at, end_at])
        .map_err(|_| ObserveReadError::ReadFailed)?;
    let mut facts = FeedbackFacts::default();
    while let Some(row) = rows.next().map_err(|_| ObserveReadError::ReadFailed)? {
        let id: String = row.get(0).map_err(|_| ObserveReadError::InvalidStore)?;
        let timestamp: String = row.get(1).map_err(|_| ObserveReadError::InvalidStore)?;
        let bundle_id: String = row.get(2).map_err(|_| ObserveReadError::InvalidStore)?;
        let outcome: String = row.get(3).map_err(|_| ObserveReadError::InvalidStore)?;
        let reason: Option<String> = row.get(4).map_err(|_| ObserveReadError::InvalidStore)?;
        let stored_workspace: String = row.get(5).map_err(|_| ObserveReadError::InvalidStore)?;
        validate_uuid_v4(&id).map_err(|_| ObserveReadError::InvalidStore)?;
        validate_uuid_v4(&bundle_id).map_err(|_| ObserveReadError::InvalidStore)?;
        ReportTimestamp::parse(&timestamp)?;
        if stored_workspace != workspace_key
            || reason.as_ref().is_some_and(|value| {
                value.as_bytes().contains(&0) || value.is_empty() || value.len() > 1_024
            })
        {
            return Err(ObserveReadError::InvalidStore);
        }
        match outcome.as_str() {
            "useful" => facts.useful = checked_increment(facts.useful)?,
            "partial" => facts.partial = checked_increment(facts.partial)?,
            "wrong" => facts.wrong = checked_increment(facts.wrong)?,
            _ => return Err(ObserveReadError::InvalidStore),
        }
    }
    Ok(facts)
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum StoredPayload {
    Empty,
    Node(StoredNode),
    Link(StoredLink),
    Recall(StoredRecall),
    Tool(StoredTool),
    Mcp(StoredMcp),
    Artifact(StoredArtifact),
    Counts(StoredCounts),
    Task(StoredTask),
}

impl StoredPayload {
    fn validate(&self) -> Result<(), ObserveReadError> {
        match self {
            Self::Empty => Ok(()),
            Self::Node(value) => value.validate(),
            Self::Link(value) => value.validate(),
            Self::Recall(value) => value.validate(),
            Self::Tool(value) => value.validate(),
            Self::Mcp(value) => value.validate(),
            Self::Artifact(value) => value.validate(),
            Self::Counts(value) => value.validate(),
            Self::Task(value) => value.validate(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTask {
    task_id: String,
}

impl StoredTask {
    fn validate(&self) -> Result<(), ObserveReadError> {
        validate_uuid_v4(&self.task_id).map_err(|_| ObserveReadError::InvalidStore)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredNode {
    node_id: i64,
    node_type: String,
    title: String,
    summary: Option<String>,
    source_ref: Option<String>,
}

impl StoredNode {
    fn validate(&self) -> Result<(), ObserveReadError> {
        validate_positive_id("node_id", self.node_id)
            .map_err(|_| ObserveReadError::InvalidStore)?;
        if !storage::ALLOWED_NODE_TYPES.contains(&self.node_type.as_str())
            || self.title.as_bytes().contains(&0)
            || self.title.trim().is_empty()
            || self.title.len() > 512
            || self
                .summary
                .as_ref()
                .is_some_and(|value| value.as_bytes().contains(&0) || value.len() > 2_048)
            || self
                .source_ref
                .as_ref()
                .is_some_and(|value| value.as_bytes().contains(&0) || value.len() > 2_048)
        {
            return Err(ObserveReadError::InvalidStore);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLink {
    source_node_id: i64,
    target_node_id: i64,
    link_type: String,
}

impl StoredLink {
    fn validate(&self) -> Result<(), ObserveReadError> {
        validate_positive_id("source_node_id", self.source_node_id)
            .map_err(|_| ObserveReadError::InvalidStore)?;
        validate_positive_id("target_node_id", self.target_node_id)
            .map_err(|_| ObserveReadError::InvalidStore)?;
        if self.link_type.as_bytes().contains(&0)
            || self.link_type.trim().is_empty()
            || self.link_type.len() > storage::MAX_LINK_TYPE_BYTES
        {
            return Err(ObserveReadError::InvalidStore);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRecall {
    node_count: u64,
    more_results: bool,
    continuation_count: u64,
    fts_fallback_used: bool,
    graph_traversal_used: bool,
    selected_node_ids: Vec<i64>,
    selection_reasons: Vec<StoredSelectionReason>,
    scores: Vec<StoredScore>,
}

impl StoredRecall {
    fn validate(&self) -> Result<(), ObserveReadError> {
        let selected_ids = self
            .selected_node_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.node_id)
            .collect::<BTreeSet<_>>();
        if self.selected_node_ids.len() > 128
            || self.selection_reasons.len() > 128
            || self.scores.len() > 128
            || selected_ids.len() != self.selected_node_ids.len()
            || score_ids.len() != self.scores.len()
            || self
                .selected_node_ids
                .iter()
                .any(|id| validate_positive_id("node_id", *id).is_err())
            || self.scores.iter().any(|score| !score.valid())
        {
            return Err(ObserveReadError::InvalidStore);
        }
        let _ = (self.node_count, self.continuation_count);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredSelectionReason {
    Mandatory,
    TypedRoot,
    FtsBm25,
    DirectLink,
    GraphTraversal,
    Workflow,
    Tool,
    FailureMode,
    Source,
    Trust,
    Confidence,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredScore {
    node_id: i64,
    score: f64,
}

impl StoredScore {
    fn valid(&self) -> bool {
        self.node_id > 0 && self.score.is_finite()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTool {
    tool_id: String,
    approval_present: bool,
}

impl StoredTool {
    fn validate(&self) -> Result<(), ObserveReadError> {
        let _ = self.approval_present;
        if self.tool_id.as_bytes().contains(&0)
            || self.tool_id.trim().is_empty()
            || self.tool_id.len() > crate::tools::MAX_TOOL_ID_BYTES
        {
            return Err(ObserveReadError::InvalidStore);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredMcp {
    mcp_id: String,
    status: StoredMcpStatus,
}

impl StoredMcp {
    fn validate(&self) -> Result<(), ObserveReadError> {
        if self.mcp_id.as_bytes().contains(&0)
            || self.mcp_id.trim().is_empty()
            || self.mcp_id.len() > storage::MAX_MCP_ID_BYTES
        {
            return Err(ObserveReadError::InvalidStore);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredMcpStatus {
    Installed,
    Missing,
    ConfiguredUnverified,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredArtifact {
    path: String,
    bytes: u64,
}

impl StoredArtifact {
    fn validate(&self) -> Result<(), ObserveReadError> {
        let _ = self.bytes;
        let path = std::path::Path::new(&self.path);
        if path.is_absolute()
            || self.path.is_empty()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err(ObserveReadError::InvalidStore);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCounts {
    items: Vec<StoredCountItem>,
}

impl StoredCounts {
    fn validate(&self) -> Result<(), ObserveReadError> {
        let _ = self.map()?;
        Ok(())
    }

    fn map(&self) -> Result<BTreeMap<String, u64>, ObserveReadError> {
        if self.items.len() > 64 {
            return Err(ObserveReadError::InvalidStore);
        }
        let mut map = BTreeMap::new();
        for item in &self.items {
            if !is_identifier(&item.name) || map.insert(item.name.clone(), item.count).is_some() {
                return Err(ObserveReadError::InvalidStore);
            }
        }
        Ok(map)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCountItem {
    name: String,
    count: u64,
}

fn parse_event_type(value: &str) -> Result<EventType, ObserveReadError> {
    EventType::ALL
        .into_iter()
        .find(|event_type| event_type.as_str() == value)
        .ok_or(ObserveReadError::InvalidStore)
}

fn parse_event_outcome(value: &str) -> Result<EventOutcome, ObserveReadError> {
    match value {
        "started" => Ok(EventOutcome::Started),
        "success" => Ok(EventOutcome::Success),
        "failure" => Ok(EventOutcome::Failure),
        "warning" => Ok(EventOutcome::Warning),
        "empty" => Ok(EventOutcome::Empty),
        "truncated" => Ok(EventOutcome::Truncated),
        "overflow" => Ok(EventOutcome::Overflow),
        "pending" => Ok(EventOutcome::Pending),
        "blocked" => Ok(EventOutcome::Blocked),
        "timeout" => Ok(EventOutcome::Timeout),
        "recorded" => Ok(EventOutcome::Recorded),
        "proposed" => Ok(EventOutcome::Proposed),
        "applied" => Ok(EventOutcome::Applied),
        "drafted" => Ok(EventOutcome::Drafted),
        "missing" => Ok(EventOutcome::Missing),
        "configured" => Ok(EventOutcome::Configured),
        "configured_unverified" => Ok(EventOutcome::ConfiguredUnverified),
        _ => Err(ObserveReadError::InvalidStore),
    }
}

fn require_exact_keys(
    values: &BTreeMap<String, u64>,
    expected: &[&str],
) -> Result<(), ObserveReadError> {
    if values.len() == expected.len() && expected.iter().all(|key| values.contains_key(*key)) {
        Ok(())
    } else {
        Err(ObserveReadError::InvalidStore)
    }
}

fn required_count(values: &BTreeMap<String, u64>, key: &str) -> Result<u64, ObserveReadError> {
    values
        .get(key)
        .copied()
        .ok_or(ObserveReadError::InvalidStore)
}

fn safe_title(value: &str, redactor: &TaggedValueRedactor) -> Result<String, ObserveReadError> {
    if value.as_bytes().contains(&0) || value.trim().is_empty() || value.len() > 65_536 {
        return Err(ObserveReadError::InvalidStore);
    }
    let exact = redactor
        .redact_str_bounded(value, 512)
        .map_err(|_| ObserveReadError::ReadFailed)?;
    Ok(truncate_utf8(redact_sensitive_text(&exact), 512))
}

fn is_identifier(value: &str) -> bool {
    validate_ascii_identifier("stored_identifier", value, 128).is_ok()
}

fn checked_increment(value: u64) -> Result<u64, ObserveReadError> {
    value.checked_add(1).ok_or(ObserveReadError::InvalidStore)
}

fn checked_add(left: u64, right: u64) -> Result<u64, ObserveReadError> {
    left.checked_add(right)
        .ok_or(ObserveReadError::InvalidStore)
}

fn nonnegative(value: i64) -> Result<u64, ObserveReadError> {
    u64::try_from(value).map_err(|_| ObserveReadError::InvalidStore)
}

fn usize_u64(value: usize) -> Result<u64, ObserveReadError> {
    u64::try_from(value).map_err(|_| ObserveReadError::InvalidStore)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::env;
    use std::path::{Path, PathBuf};
    use std::sync::MutexGuard;
    use std::time::{SystemTime, UNIX_EPOCH};

    const WORKSPACE_KEY: &str = "report-workspace";

    struct TestWorkspace {
        paths: WorkspacePaths,
        home: PathBuf,
        original_home: Option<std::ffi::OsString>,
        _lock: MutexGuard<'static, ()>,
    }

    impl TestWorkspace {
        fn new(name: &str) -> Self {
            let lock = crate::install::test_env_lock()
                .lock()
                .expect("environment lock should not be poisoned");
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should follow epoch")
                .as_nanos();
            let home = env::temp_dir().join(format!(
                "aopmem-report-{name}-{}-{nanos}",
                std::process::id()
            ));
            let original_home = env::var_os("AOPMEM_HOME");
            env::set_var("AOPMEM_HOME", &home);
            let global = storage::resolve_paths().expect("test home should resolve");
            storage::ensure_global_dirs(&global).expect("global directories should create");
            let paths = storage::ensure_workspace_dirs(&global, WORKSPACE_KEY)
                .expect("workspace directories should create");
            Self {
                paths,
                home,
                original_home,
                _lock: lock,
            }
        }

        fn initialize(&self) {
            drop(
                crate::observability::open_writer(&self.paths)
                    .expect("observability store should initialize"),
            );
        }

        fn mutate_fixture(&self, action: impl FnOnce(&Connection)) {
            let connection = Connection::open(self.paths.observability_db())
                .expect("fixture database should open");
            connection
                .execute_batch("PRAGMA foreign_keys = ON;")
                .expect("foreign keys should enable");
            action(&connection);
            let checkpoint: (i64, i64, i64) = connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .expect("fixture WAL should checkpoint");
            assert_eq!(checkpoint.0, 0, "checkpoint should not be busy");
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.home);
            match &self.original_home {
                Some(value) => env::set_var("AOPMEM_HOME", value),
                None => env::remove_var("AOPMEM_HOME"),
            }
        }
    }

    fn fixed_uuid(value: u64) -> String {
        format!("00000000-0000-4000-8000-{value:012x}")
    }

    fn query_plan(connection: &Connection, sql: &str, parameters: impl rusqlite::Params) -> String {
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("query plan should prepare");
        statement
            .query_map(parameters, |row| row.get::<_, String>(3))
            .expect("query plan should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("query plan should collect")
            .join("\n")
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_bundle(
        connection: &Connection,
        value: u64,
        timestamp: &str,
        workspace_key: &str,
        outcome: &str,
        error_code: Option<&str>,
        more_results: bool,
        continuation_count: u64,
    ) -> String {
        let bundle_id = fixed_uuid(value);
        connection
            .execute(
                "INSERT INTO recall_bundles (
                    bundle_id, timestamp, product_version, workspace_key,
                    correlation_id, outcome, error_code, duration_ms,
                    more_results, continuation_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 7, ?8, ?9)",
                params![
                    bundle_id,
                    timestamp,
                    PRODUCT_VERSION,
                    workspace_key,
                    fixed_uuid(value + 10_000),
                    outcome,
                    error_code,
                    i64::from(more_results),
                    i64::try_from(continuation_count).expect("fixture count should fit"),
                ],
            )
            .expect("bundle fixture should insert");
        bundle_id
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_event(
        connection: &Connection,
        value: u64,
        timestamp: &str,
        event_type: EventType,
        outcome: EventOutcome,
        payload: serde_json::Value,
        bundle_id: Option<&str>,
        error_code: Option<&str>,
    ) {
        connection
            .execute(
                "INSERT INTO observability_events (
                    id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, bundle_id, duration_ms, outcome,
                    error_code, payload_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'fixture', ?6, ?7, 5, ?8, ?9, ?10)",
                params![
                    fixed_uuid(value),
                    timestamp,
                    PRODUCT_VERSION,
                    WORKSPACE_KEY,
                    event_type.as_str(),
                    fixed_uuid(value + 20_000),
                    bundle_id,
                    outcome.as_str(),
                    error_code,
                    payload.to_string(),
                ],
            )
            .expect("event fixture should insert");
    }

    fn empty_json() -> serde_json::Value {
        serde_json::json!({ "kind": "empty" })
    }

    fn counts_json(items: &[(&str, u64)]) -> serde_json::Value {
        serde_json::json!({
            "kind": "counts",
            "data": {
                "items": items.iter().map(|(name, count)| {
                    serde_json::json!({ "name": name, "count": count })
                }).collect::<Vec<_>>()
            }
        })
    }

    fn tool_json(tool_id: &str) -> serde_json::Value {
        serde_json::json!({
            "kind": "tool",
            "data": { "tool_id": tool_id, "approval_present": false }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_task(
        connection: &Connection,
        value: u64,
        status: &str,
        started_at: &str,
        applied_at: Option<&str>,
        finished_at: Option<&str>,
    ) -> String {
        let task_id = fixed_uuid(value);
        let terminal = matches!(status, "completed" | "failed");
        connection
            .execute(
                "INSERT INTO tasks (
                    task_id, bundle_id, product_version, workspace_key,
                    memory_revision, query_fingerprint, status, started_at,
                    applied_at, finished_at, mandatory_context_complete,
                    retrieval_complete, budget_exhausted, none_relevant,
                    apply_fingerprint, completion_fingerprint,
                    completion_result, duration_ms, error_code
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    1, 1, 0, 0, ?11, ?12, ?13, ?14, ?15
                 )",
                params![
                    task_id,
                    fixed_uuid(value + 10_000),
                    PRODUCT_VERSION,
                    WORKSPACE_KEY,
                    "a".repeat(32),
                    "b".repeat(32),
                    status,
                    started_at,
                    applied_at,
                    finished_at,
                    applied_at.map(|_| "c".repeat(32)),
                    terminal.then(|| "d".repeat(32)),
                    terminal.then_some(if status == "failed" {
                        "failed"
                    } else {
                        "success"
                    }),
                    terminal.then_some(5_i64),
                    (status == "failed").then_some("TASK_FAILED"),
                ],
            )
            .expect("task fixture should insert");
        task_id
    }

    fn insert_applied_task_node(
        connection: &Connection,
        task_id: &str,
        node_id: i64,
        context_kind: &str,
        application_kind: &str,
    ) {
        let node_type = match application_kind {
            "tool" => "tool_contract",
            other => other,
        };
        connection
            .execute(
                "INSERT INTO task_bundle_nodes (
                    task_id, node_id, node_type, context_kind
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![task_id, node_id, node_type, context_kind],
            )
            .expect("task bundle node fixture should insert");
        connection
            .execute(
                "INSERT INTO task_applied_nodes (
                    task_id, node_id, application_kind
                 ) VALUES (?1, ?2, ?3)",
                params![task_id, node_id, application_kind],
            )
            .expect("applied task node fixture should insert");
    }

    fn recall_json(
        more_results: bool,
        continuation_count: u64,
        fts_fallback_used: bool,
        graph_traversal_used: bool,
    ) -> serde_json::Value {
        serde_json::json!({
            "kind": "recall",
            "data": {
                "node_count": 1,
                "more_results": more_results,
                "continuation_count": continuation_count,
                "fts_fallback_used": fts_fallback_used,
                "graph_traversal_used": graph_traversal_used,
                "selected_node_ids": [1],
                "selection_reasons": ["typed_root"],
                "scores": [{"node_id": 1, "score": 1.0}]
            }
        })
    }

    fn insert_bundle_node(
        connection: &Connection,
        bundle_id: &str,
        node_id: i64,
        timestamp: &str,
        node_type: &str,
        title: &str,
    ) {
        connection
            .execute(
                "INSERT INTO bundle_nodes (
                    bundle_id, node_id, first_seen_at, node_type, node_title,
                    bounded_summary, source_ref, trust_level, confidence,
                    score, selection_reasons_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'bounded', 'source:test',
                           'user_taught', 0.9, 1.0, '[\"typed_root\"]')",
                params![bundle_id, node_id, timestamp, node_type, title],
            )
            .expect("bundle node fixture should insert");
    }

    fn insert_feedback(
        connection: &Connection,
        value: u64,
        timestamp: &str,
        bundle_id: &str,
        outcome: &str,
    ) {
        connection
            .execute(
                "INSERT INTO feedback (id, timestamp, bundle_id, outcome, reason)
                 VALUES (?1, ?2, ?3, ?4, 'bounded reason')",
                params![fixed_uuid(value), timestamp, bundle_id, outcome],
            )
            .expect("feedback fixture should insert");
    }

    fn database_snapshot(path: &Path) -> (Vec<u8>, SystemTime) {
        let bytes = fs::read(path).expect("database should read");
        let modified = fs::metadata(path)
            .expect("database metadata should read")
            .modified()
            .expect("database mtime should exist");
        (bytes, modified)
    }

    fn legacy_status_facts(
        connection: &Connection,
    ) -> (u64, u64, u64, u64, Option<String>, Option<String>) {
        let count = |table| {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            let value: i64 = connection
                .query_row(&sql, [], |row| row.get(0))
                .expect("legacy status count should read");
            u64::try_from(value).expect("legacy status count should be nonnegative")
        };
        let (first_recorded_at, last_recorded_at) = connection
            .query_row(
                "SELECT MIN(timestamp), MAX(timestamp) FROM (
                    SELECT timestamp FROM observability_events
                    UNION ALL SELECT timestamp FROM recall_bundles
                    UNION ALL SELECT first_seen_at AS timestamp FROM bundle_nodes
                    UNION ALL SELECT timestamp FROM feedback
                 )",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("legacy status timestamp range should read");
        (
            count("observability_events"),
            count("recall_bundles"),
            count("bundle_nodes"),
            count("feedback"),
            first_recorded_at,
            last_recorded_at,
        )
    }

    #[test]
    fn missing_store_reports_not_collected_without_creating_paths() {
        let _workspace = TestWorkspace::new("missing");
        let global = storage::resolve_paths().expect("test home should resolve");
        let missing_paths = storage::workspace_paths_for_key(&global, "missing-report-workspace");
        assert!(!missing_paths.root().exists());

        let status = observe_status(&missing_paths, "missing-report-workspace")
            .expect("missing status should be successful");
        let captured_at = ReportTimestamp::parse("2026-07-15T12:00:00.000Z")
            .expect("fixture timestamp should parse");
        let report =
            effectiveness_report_at(&missing_paths, "missing-report-workspace", &captured_at)
                .expect("missing report should be successful");

        assert_eq!(status.collection_status, CollectionStatus::NotCollected);
        assert!(!status.complete);
        assert!(status.facts.is_none());
        assert_eq!(report.collection_status, CollectionStatus::NotCollected);
        assert!(!report.complete);
        assert!(report.facts.is_none());
        assert_eq!(report.period.start_at, "2026-06-15T12:00:00.000Z");
        assert_eq!(report.period.end_at, "2026-07-15T12:00:00.000Z");
        assert!(!missing_paths.root().exists());
        assert!(!missing_paths.observability().exists());
        assert!(!missing_paths.observability_db().exists());
        assert!(!missing_paths.db().exists());
    }

    #[test]
    fn initialized_empty_store_is_ready_zero_and_read_does_not_mutate_main_db() {
        let workspace = TestWorkspace::new("empty");
        workspace.initialize();
        let before = database_snapshot(workspace.paths.observability_db());
        assert!(!workspace.paths.db().exists());

        let status = observe_status(&workspace.paths, WORKSPACE_KEY)
            .expect("initialized status should read");
        let captured_at = ReportTimestamp::parse("2026-07-15T12:00:00.000Z")
            .expect("fixture timestamp should parse");
        let report = effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at)
            .expect("initialized report should read");
        let after = database_snapshot(workspace.paths.observability_db());

        let status_facts = status.facts.expect("ready status should contain facts");
        assert_eq!(status.collection_status, CollectionStatus::Ready);
        assert!(status.complete);
        assert_eq!(status.observability_schema_version, Some(2));
        assert_eq!(status_facts.observability_events, 0);
        assert_eq!(status_facts.recall_bundles, 0);
        assert_eq!(status_facts.bundle_nodes, 0);
        assert_eq!(status_facts.feedback, 0);
        assert_eq!(report.collection_status, CollectionStatus::Ready);
        assert!(report.complete);
        assert_eq!(report.observability_schema_version, Some(2));
        assert_eq!(
            report
                .facts
                .expect("ready report should contain facts")
                .recall
                .count,
            0
        );
        assert_eq!(after.0, before.0, "read changed observability DB bytes");
        assert_eq!(after.1, before.1, "read changed observability DB mtime");
        assert!(!workspace.paths.db().exists());
    }

    #[test]
    fn status_facts_cte_matches_legacy_counts_and_range() {
        let workspace = TestWorkspace::new("status-facts-parity");
        workspace.initialize();
        workspace.mutate_fixture(|connection| {
            let bundle_id = insert_bundle(
                connection,
                50,
                "2026-01-02T00:00:00.000Z",
                WORKSPACE_KEY,
                "success",
                None,
                false,
                0,
            );
            insert_event(
                connection,
                51,
                "2026-01-01T00:00:00.000Z",
                EventType::Doctor,
                EventOutcome::Success,
                empty_json(),
                None,
                None,
            );
            insert_event(
                connection,
                52,
                "2026-01-05T00:00:00.000Z",
                EventType::Doctor,
                EventOutcome::Success,
                empty_json(),
                None,
                None,
            );
            insert_bundle_node(
                connection,
                &bundle_id,
                1,
                "2026-01-01T00:00:00.000Z",
                "workflow",
                "Status fixture",
            );
            insert_feedback(
                connection,
                53,
                "2026-01-06T00:00:00.000Z",
                &bundle_id,
                "useful",
            );

            let legacy = legacy_status_facts(connection);
            let transaction = connection
                .unchecked_transaction()
                .expect("status facts transaction should start");
            let facts = load_status_facts(&transaction).expect("status facts CTE should read");
            transaction
                .commit()
                .expect("status facts transaction should commit");

            assert_eq!(
                (
                    facts.observability_events,
                    facts.recall_bundles,
                    facts.bundle_nodes,
                    facts.feedback,
                    facts.first_recorded_at,
                    facts.last_recorded_at,
                ),
                legacy
            );
        });
    }

    #[test]
    fn status_facts_cte_rejects_invalid_timestamp() {
        let workspace = TestWorkspace::new("status-facts-invalid-timestamp");
        workspace.initialize();
        workspace.mutate_fixture(|connection| {
            insert_event(
                connection,
                60,
                "invalid",
                EventType::Doctor,
                EventOutcome::Success,
                empty_json(),
                None,
                None,
            );
            let transaction = connection
                .unchecked_transaction()
                .expect("invalid status facts transaction should start");
            assert!(matches!(
                load_status_facts(&transaction),
                Err(ObserveReadError::InvalidStore)
            ));
        });
    }

    #[test]
    fn status_facts_cte_uses_each_timestamp_index_once() {
        let workspace = TestWorkspace::new("status-facts-plan");
        workspace.initialize();
        workspace.mutate_fixture(|connection| {
            let explain = format!("EXPLAIN QUERY PLAN {STATUS_FACTS_SQL}");
            let mut statement = connection
                .prepare(&explain)
                .expect("status facts query plan should prepare");
            let plan = statement
                .query_map([], |row| row.get::<_, String>(3))
                .expect("status facts query plan should run")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("status facts query plan should collect")
                .join("\n");

            for expected_index in [
                "idx_observability_events_timestamp",
                "idx_recall_bundles_timestamp",
                "idx_bundle_nodes_first_seen_at",
                "idx_feedback_timestamp",
            ] {
                assert!(
                    plan.matches(expected_index).count() == 1,
                    "status facts must use {expected_index} once, got: {plan}"
                );
            }
        });
    }

    #[test]
    fn effectiveness_report_aggregates_inclusive_window_and_redacts_output() {
        let workspace = TestWorkspace::new("aggregate");
        workspace.initialize();
        let start = "2026-06-15T12:00:00.000Z";
        let end = "2026-07-15T12:00:00.000Z";
        let tool_secret = "sk-1234567890abcdef";
        let error_secret = "ghp_1234567890abcdef";
        workspace.mutate_fixture(|connection| {
            let first = insert_bundle(
                connection,
                100,
                start,
                WORKSPACE_KEY,
                "success",
                None,
                true,
                2,
            );
            let second = insert_bundle(
                connection,
                101,
                end,
                WORKSPACE_KEY,
                "failure",
                Some("RECALL_FAILED"),
                false,
                0,
            );
            insert_bundle(
                connection,
                102,
                "2026-06-15T11:59:59.999Z",
                WORKSPACE_KEY,
                "success",
                None,
                false,
                0,
            );
            insert_bundle(
                connection,
                103,
                "2026-07-15T12:00:00.001Z",
                WORKSPACE_KEY,
                "success",
                None,
                false,
                0,
            );

            insert_event(
                connection,
                190,
                start,
                EventType::RecallStarted,
                EventOutcome::Started,
                empty_json(),
                Some(&first),
                None,
            );
            for value in [191, 192] {
                insert_event(
                    connection,
                    value,
                    start,
                    EventType::RecallContinuation,
                    EventOutcome::Recorded,
                    empty_json(),
                    Some(&first),
                    None,
                );
            }
            insert_event(
                connection,
                200,
                start,
                EventType::RecallCompleted,
                EventOutcome::Success,
                serde_json::json!({
                    "kind": "recall",
                    "data": {
                        "node_count": 4,
                        "more_results": true,
                        "continuation_count": 2,
                        "fts_fallback_used": true,
                        "graph_traversal_used": true,
                        "selected_node_ids": [1, 2],
                        "selection_reasons": ["typed_root", "fts_bm25"],
                        "scores": [{"node_id": 1, "score": 1.0}]
                    }
                }),
                Some(&first),
                None,
            );
            insert_event(
                connection,
                193,
                end,
                EventType::RecallStarted,
                EventOutcome::Started,
                empty_json(),
                Some(&second),
                None,
            );
            insert_event(
                connection,
                194,
                end,
                EventType::RecallFailed,
                EventOutcome::Failure,
                empty_json(),
                Some(&second),
                Some("RECALL_FAILED"),
            );
            insert_event(
                connection,
                201,
                start,
                EventType::RecallEmpty,
                EventOutcome::Empty,
                empty_json(),
                Some(&first),
                None,
            );
            insert_event(
                connection,
                202,
                end,
                EventType::RecallMandatoryOverflow,
                EventOutcome::Overflow,
                counts_json(&[("offending_nodes", 3)]),
                Some(&second),
                None,
            );
            insert_event(
                connection,
                203,
                start,
                EventType::ToolRunCompleted,
                EventOutcome::Success,
                tool_json("safe-tool"),
                None,
                None,
            );
            for (value, timestamp) in [(204, start), (205, end)] {
                insert_event(
                    connection,
                    value,
                    timestamp,
                    EventType::ToolRunFailed,
                    EventOutcome::Failure,
                    tool_json(tool_secret),
                    None,
                    Some(error_secret),
                );
            }
            insert_event(
                connection,
                206,
                end,
                EventType::ToolRunTimeout,
                EventOutcome::Timeout,
                tool_json("slow-tool"),
                None,
                Some("TOOL_TIMEOUT"),
            );
            insert_event(
                connection,
                207,
                start,
                EventType::ReflectionProposal,
                EventOutcome::Proposed,
                counts_json(&[("items", 2)]),
                None,
                None,
            );
            insert_event(
                connection,
                208,
                start,
                EventType::ReflectionApplied,
                EventOutcome::Applied,
                counts_json(&[("items", 1)]),
                None,
                None,
            );
            insert_event(
                connection,
                209,
                end,
                EventType::ReflectionApplied,
                EventOutcome::Drafted,
                counts_json(&[("items", 1)]),
                None,
                None,
            );
            insert_event(
                connection,
                210,
                start,
                EventType::AdapterDrift,
                EventOutcome::Missing,
                empty_json(),
                None,
                None,
            );
            insert_event(
                connection,
                211,
                end,
                EventType::AdapterDrift,
                EventOutcome::Warning,
                empty_json(),
                None,
                None,
            );
            insert_event(
                connection,
                218,
                end,
                EventType::AdapterDrift,
                EventOutcome::Failure,
                empty_json(),
                None,
                Some("ADAPTER_DRIFT_FAILED"),
            );
            insert_event(
                connection,
                212,
                end,
                EventType::AuditSnapshotPending,
                EventOutcome::Pending,
                counts_json(&[("duration_ms", 9)]),
                None,
                Some("AUDIT_SNAPSHOT_PENDING"),
            );
            insert_event(
                connection,
                213,
                start,
                EventType::Doctor,
                EventOutcome::Failure,
                counts_json(&[]),
                None,
                Some("DB_SCHEMA_ERROR"),
            );
            insert_event(
                connection,
                214,
                end,
                EventType::Verify,
                EventOutcome::Failure,
                counts_json(&[]),
                None,
                Some("VERIFY_FAILED"),
            );
            insert_event(
                connection,
                215,
                end,
                EventType::ArtifactsCleanup,
                EventOutcome::Success,
                counts_json(&[
                    ("bytes_before", 100),
                    ("bytes_after", 10),
                    ("deleted_dirs", 1),
                    ("deleted_files", 2),
                    ("deleted_paths", 3),
                    ("kept_dirs", 1),
                    ("complete", 1),
                ]),
                None,
                None,
            );
            insert_event(
                connection,
                216,
                start,
                EventType::McpStatus,
                EventOutcome::Missing,
                serde_json::json!({
                    "kind": "mcp",
                    "data": { "mcp_id": "missing-mcp", "status": "missing" }
                }),
                None,
                None,
            );
            insert_event(
                connection,
                217,
                end,
                EventType::McpStatus,
                EventOutcome::ConfiguredUnverified,
                serde_json::json!({
                    "kind": "mcp",
                    "data": {
                        "mcp_id": "pending-mcp",
                        "status": "configured_unverified"
                    }
                }),
                None,
                None,
            );

            insert_bundle_node(connection, &first, 1, start, "workflow", "Deploy flow");
            insert_bundle_node(connection, &first, 2, start, "tool_contract", tool_secret);
            insert_bundle_node(connection, &first, 3, start, "failure_mode", "Repeat me");
            insert_bundle_node(connection, &second, 3, end, "failure_mode", "Repeat me");
            insert_bundle_node(connection, &first, 4, start, "correction", "Correct this");
            insert_bundle_node(connection, &second, 5, end, "correction", "Correct this");
            insert_bundle_node(
                connection,
                &first,
                99,
                "2026-07-15T12:00:00.001Z",
                "workflow",
                "Post-end continuation node",
            );
            insert_feedback(connection, 300, start, &first, "useful");
            insert_feedback(connection, 301, end, &first, "partial");
            insert_feedback(connection, 302, end, &second, "wrong");
        });
        let before = database_snapshot(workspace.paths.observability_db());
        let captured_at = ReportTimestamp::parse(end).expect("fixture timestamp should parse");
        let report = effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at)
            .expect("aggregate report should read");
        let after = database_snapshot(workspace.paths.observability_db());
        let facts = report
            .facts
            .as_ref()
            .expect("ready report should have facts");

        assert_eq!(report.period.start_at, start);
        assert_eq!(report.period.end_at, end);
        assert_eq!(facts.recall.count, 2);
        assert_eq!(facts.recall.failed, 1);
        assert_eq!(facts.recall.empty, 1);
        assert_eq!(facts.recall.mandatory_overflow, 1);
        assert_eq!(facts.recall.more_results_bundles, 1);
        assert_eq!(facts.recall.terminal_more_results_bundles, 1);
        assert_eq!(facts.recall.continuation_bundles, 1);
        assert_eq!(facts.recall.continuation_invocations, 2);
        assert_eq!(facts.recall.fts_fallback_bundles, 1);
        assert_eq!(facts.recall.graph_traversal_bundles, 1);
        assert_eq!(
            facts.feedback,
            FeedbackFacts {
                useful: 1,
                partial: 1,
                wrong: 1
            }
        );
        assert_eq!(facts.tools.success, 1);
        assert_eq!(facts.tools.failure, 2);
        assert_eq!(facts.tools.timeout, 1);
        assert_eq!(facts.tools.repeated_errors.items.len(), 1);
        assert_eq!(
            facts.reflection.proposed,
            EventItemCount {
                events: 1,
                items: 2
            }
        );
        assert_eq!(
            facts.reflection.applied,
            EventItemCount {
                events: 1,
                items: 1
            }
        );
        assert_eq!(
            facts.reflection.drafted,
            EventItemCount {
                events: 1,
                items: 1
            }
        );
        assert_eq!(
            facts.adapter_drift_events,
            AdapterDriftFacts {
                missing: 1,
                drifted: 1,
                failed: 1
            }
        );
        assert_eq!(facts.pending_audit_events, 1);
        assert_eq!(
            facts.doctor_verify_failures,
            HealthFailureFacts {
                doctor: 1,
                verify: 1
            }
        );
        assert_eq!(facts.artifact_cleanup_deletions.deleted_paths, 3);
        assert_eq!(facts.mcp.missing_status_observations, 1);
        assert_eq!(facts.mcp.configured_unverified_status_observations, 1);
        assert_eq!(facts.most_selected.workflows.items.len(), 1);
        assert_eq!(facts.most_selected.tools.items.len(), 1);
        assert_eq!(facts.most_selected.failure_modes.items[0].bundles, 2);
        assert_eq!(facts.repeated_correction_failure_mode_titles.items.len(), 2);

        let json = serde_json::to_string(&report).expect("report should serialize");
        assert!(!json.contains(tool_secret));
        assert!(!json.contains(error_secret));
        assert!(!json.contains("Post-end continuation node"));
        assert!(!json.contains("product_score"));
        assert!(!json.contains("advice"));
        assert_eq!(after.0, before.0, "report changed observability DB bytes");
        assert_eq!(after.1, before.1, "report changed observability DB mtime");
        assert!(!workspace.paths.db().exists());
    }

    #[test]
    fn task_and_rc5_compliance_facts_are_factual_bounded_and_read_only() {
        let workspace = TestWorkspace::new("task-compliance-facts");
        workspace.initialize();
        let start = "2026-06-15T12:00:00.000Z";
        let end = "2026-07-15T12:00:00.000Z";
        workspace.mutate_fixture(|connection| {
            insert_task(connection, 4_000, "started", start, None, None);
            let completed = insert_task(
                connection,
                4_001,
                "completed",
                start,
                Some(start),
                Some(end),
            );
            insert_task(connection, 4_002, "failed", start, None, Some(end));
            let prior = insert_task(
                connection,
                4_003,
                "applied",
                "2026-05-01T00:00:00.000Z",
                Some(start),
                None,
            );
            let future = "2026-07-15T12:00:00.001Z";
            insert_task(connection, 4_004, "applied", start, Some(future), None);
            for (node_id, context, application) in [
                (1, "mandatory", "gate"),
                (2, "task", "workflow"),
                (3, "task", "tool"),
                (4, "task", "correction"),
                (5, "task", "failure_mode"),
            ] {
                insert_applied_task_node(connection, &completed, node_id, context, application);
            }
            insert_applied_task_node(connection, &prior, 6, "mandatory", "rule");

            insert_event(
                connection,
                4_100,
                end,
                EventType::ToolDuplicateBlocked,
                EventOutcome::Blocked,
                tool_json("safe-tool"),
                None,
                Some("TOOL_OVERLAP_REVIEW_REQUIRED"),
            );
            insert_event(
                connection,
                4_101,
                end,
                EventType::ToolAliasResolved,
                EventOutcome::Success,
                tool_json("safe-alias"),
                None,
                None,
            );
            insert_event(
                connection,
                4_102,
                end,
                EventType::AuditRepairCompleted,
                EventOutcome::Success,
                empty_json(),
                None,
                None,
            );
        });
        let before = database_snapshot(workspace.paths.observability_db());
        let captured_at = ReportTimestamp::parse(end).expect("fixture timestamp should parse");
        let facts = effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at)
            .expect("task facts should read")
            .facts
            .expect("ready report should contain facts");
        let after = database_snapshot(workspace.paths.observability_db());

        assert_eq!(facts.tasks.starts, 4);
        assert_eq!(facts.tasks.context_applications, 2);
        assert_eq!(facts.tasks.started_without_apply, 3);
        assert_eq!(facts.tasks.completed, 1);
        assert_eq!(facts.tasks.failed, 1);
        assert_eq!(facts.tasks.applied_gates, 1);
        assert_eq!(facts.tasks.applied_rules, 1);
        assert_eq!(facts.tasks.selected_workflows, 1);
        assert_eq!(facts.tasks.selected_tools, 1);
        assert_eq!(facts.tasks.corrections_applied, 1);
        assert_eq!(facts.tasks.failure_modes_applied, 1);
        assert_eq!(
            facts.tasks.applied_context_by_type,
            vec![
                NamedCount {
                    name: "mandatory".to_string(),
                    count: 2,
                },
                NamedCount {
                    name: "task".to_string(),
                    count: 4,
                },
            ]
        );
        assert_eq!(facts.tool_duplicate_blocks, 1);
        assert_eq!(facts.alias_resolutions, 1);
        assert_eq!(facts.unresolved_tool_overlaps, 1);
        assert_eq!(facts.last_successful_audit_repair_at.as_deref(), Some(end));
        assert_eq!(after.0, before.0, "task report changed database bytes");
        assert_eq!(after.1, before.1, "task report changed database mtime");
    }

    #[test]
    fn task_aggregates_use_bounded_lifecycle_and_child_indexes() {
        let workspace = TestWorkspace::new("task-query-plan");
        workspace.initialize();
        workspace.mutate_fixture(|connection| {
            let lifecycle_plan = query_plan(
                connection,
                TASK_LIFECYCLE_FACTS_SQL,
                rusqlite::params![
                    WORKSPACE_KEY,
                    "2026-06-15T12:00:00.000Z",
                    "2026-07-15T12:00:00.000Z"
                ],
            );
            for index in ["idx_tasks_started_at", "idx_tasks_workspace_status"] {
                assert!(
                    lifecycle_plan.contains(index),
                    "task lifecycle query must use {index}: {lifecycle_plan}"
                );
            }

            let applied_plan = query_plan(
                connection,
                TASK_APPLIED_FACTS_SQL,
                rusqlite::params![
                    WORKSPACE_KEY,
                    "2026-06-15T12:00:00.000Z",
                    "2026-07-15T12:00:00.000Z"
                ],
            );
            assert!(
                applied_plan.contains("idx_tasks_workspace_status"),
                "task applied query must use the existing workspace/status index: {applied_plan}"
            );
            assert!(
                applied_plan.contains("sqlite_autoindex_task_applied_nodes_1"),
                "task applied query must use the bounded child key: {applied_plan}"
            );
        });
    }

    #[test]
    fn foreign_task_history_fails_closed() {
        let workspace = TestWorkspace::new("foreign-task");
        workspace.initialize();
        workspace.mutate_fixture(|connection| {
            let task_id = insert_task(
                connection,
                4_200,
                "started",
                "2026-07-15T12:00:00.000Z",
                None,
                None,
            );
            connection
                .execute(
                    "UPDATE tasks SET workspace_key = 'foreign-workspace'
                     WHERE task_id = ?1",
                    [task_id],
                )
                .expect("foreign task fixture should update");
        });
        let captured_at = ReportTimestamp::parse("2026-07-15T12:00:00.000Z")
            .expect("fixture timestamp should parse");
        assert!(matches!(
            effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at),
            Err(ObserveReadError::InvalidStore)
        ));
    }

    #[test]
    fn report_top_lists_are_bounded_and_mark_more_results() {
        let workspace = TestWorkspace::new("top-list");
        workspace.initialize();
        let timestamp = "2026-07-15T12:00:00.000Z";
        workspace.mutate_fixture(|connection| {
            let bundle = insert_bundle(
                connection,
                400,
                timestamp,
                WORKSPACE_KEY,
                "success",
                None,
                false,
                0,
            );
            for node_id in 1..=21 {
                insert_bundle_node(
                    connection,
                    &bundle,
                    node_id,
                    timestamp,
                    "workflow",
                    &format!("Workflow {node_id:02}"),
                );
            }
        });
        let captured_at =
            ReportTimestamp::parse(timestamp).expect("fixture timestamp should parse");
        let report = effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at)
            .expect("bounded report should read");
        let workflows = &report
            .facts
            .as_ref()
            .expect("ready report should contain facts")
            .most_selected
            .workflows;
        assert_eq!(workflows.limit, 20);
        assert_eq!(workflows.items.len(), 20);
        assert!(workflows.more_results);
        let json = serde_json::to_value(report).expect("report should serialize");
        assert_eq!(
            json.pointer("/facts/most_selected/workflows/more_results"),
            Some(&serde_json::Value::Bool(true))
        );
        assert!(json.get("score").is_none());
        assert!(json.get("advice").is_none());
    }

    #[test]
    fn current_recall_events_and_nodes_count_for_parent_older_than_period() {
        let workspace = TestWorkspace::new("old-parent-current-events");
        workspace.initialize();
        let end = "2026-07-15T12:00:00.000Z";
        workspace.mutate_fixture(|connection| {
            let bundle = insert_bundle(
                connection,
                1_000,
                "2026-05-01T00:00:00.000Z",
                WORKSPACE_KEY,
                "success",
                None,
                false,
                0,
            );
            insert_event(
                connection,
                1_001,
                end,
                EventType::RecallStarted,
                EventOutcome::Started,
                empty_json(),
                Some(&bundle),
                None,
            );
            insert_event(
                connection,
                1_002,
                end,
                EventType::RecallContinuation,
                EventOutcome::Recorded,
                empty_json(),
                Some(&bundle),
                None,
            );
            insert_event(
                connection,
                1_003,
                end,
                EventType::RecallCompleted,
                EventOutcome::Success,
                recall_json(true, 1, true, true),
                Some(&bundle),
                None,
            );
            insert_bundle_node(connection, &bundle, 1, end, "workflow", "Current workflow");
        });
        let captured_at = ReportTimestamp::parse(end).expect("fixture timestamp should parse");
        let report = effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at)
            .expect("current lifecycle should not depend on parent timestamp");
        let facts = report.facts.expect("ready report should contain facts");

        assert_eq!(facts.recall.count, 1);
        assert_eq!(facts.recall.continuation_invocations, 1);
        assert_eq!(facts.recall.continuation_bundles, 1);
        assert_eq!(facts.recall.more_results_bundles, 1);
        assert_eq!(facts.recall.terminal_more_results_bundles, 1);
        assert_eq!(facts.recall.fts_fallback_bundles, 1);
        assert_eq!(facts.recall.graph_traversal_bundles, 1);
        assert_eq!(facts.nodes_selected_by_type[0].name, "workflow");
        assert_eq!(facts.nodes_selected_by_type[0].count, 1);
        assert_eq!(facts.most_selected.workflows.items[0].node_id, 1);
    }

    #[test]
    fn failed_recall_then_successful_retry_stays_failed_in_period() {
        let workspace = TestWorkspace::new("failed-then-success");
        workspace.initialize();
        let start = "2026-07-15T11:00:00.000Z";
        let end = "2026-07-15T12:00:00.000Z";
        workspace.mutate_fixture(|connection| {
            let bundle = insert_bundle(
                connection,
                1_100,
                start,
                WORKSPACE_KEY,
                "success",
                None,
                false,
                0,
            );
            insert_event(
                connection,
                1_101,
                start,
                EventType::RecallStarted,
                EventOutcome::Started,
                empty_json(),
                Some(&bundle),
                None,
            );
            insert_event(
                connection,
                1_102,
                start,
                EventType::RecallFailed,
                EventOutcome::Failure,
                empty_json(),
                Some(&bundle),
                Some("RECALL_FAILED"),
            );
            insert_event(
                connection,
                1_103,
                end,
                EventType::RecallStarted,
                EventOutcome::Started,
                empty_json(),
                Some(&bundle),
                None,
            );
            insert_event(
                connection,
                1_104,
                end,
                EventType::RecallCompleted,
                EventOutcome::Success,
                recall_json(false, 0, false, false),
                Some(&bundle),
                None,
            );
        });
        let captured_at = ReportTimestamp::parse(end).expect("fixture timestamp should parse");
        let facts = effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at)
            .expect("lifecycle report should read")
            .facts
            .expect("ready report should contain facts");

        assert_eq!(facts.recall.count, 2);
        assert_eq!(facts.recall.failed, 1);
    }

    #[test]
    fn parent_lifetime_state_does_not_inflate_period_recall_facts() {
        let workspace = TestWorkspace::new("parent-lifetime-state");
        workspace.initialize();
        let end = "2026-07-15T12:00:00.000Z";
        workspace.mutate_fixture(|connection| {
            let bundle = insert_bundle(
                connection,
                1_200,
                end,
                WORKSPACE_KEY,
                "success",
                None,
                true,
                99,
            );
            insert_event(
                connection,
                1_201,
                end,
                EventType::RecallStarted,
                EventOutcome::Started,
                empty_json(),
                Some(&bundle),
                None,
            );
            insert_event(
                connection,
                1_202,
                end,
                EventType::RecallCompleted,
                EventOutcome::Success,
                recall_json(true, 99, false, false),
                Some(&bundle),
                None,
            );
            insert_event(
                connection,
                1_203,
                end,
                EventType::RecallCompleted,
                EventOutcome::Success,
                recall_json(false, 99, false, false),
                Some(&bundle),
                None,
            );
        });
        let captured_at = ReportTimestamp::parse(end).expect("fixture timestamp should parse");
        let facts = effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at)
            .expect("event-time report should read")
            .facts
            .expect("ready report should contain facts");

        assert_eq!(facts.recall.count, 1);
        assert_eq!(facts.recall.continuation_invocations, 0);
        assert_eq!(facts.recall.continuation_bundles, 0);
        assert_eq!(facts.recall.more_results_bundles, 1);
        assert_eq!(facts.recall.terminal_more_results_bundles, 0);
    }

    #[test]
    fn adapter_drift_failure_is_reported_explicitly() {
        let workspace = TestWorkspace::new("adapter-drift-failure");
        workspace.initialize();
        let end = "2026-07-15T12:00:00.000Z";
        workspace.mutate_fixture(|connection| {
            insert_event(
                connection,
                1_300,
                end,
                EventType::AdapterDrift,
                EventOutcome::Failure,
                empty_json(),
                None,
                Some("ADAPTER_DRIFT_FAILED"),
            );
        });
        let captured_at = ReportTimestamp::parse(end).expect("fixture timestamp should parse");
        let facts = effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at)
            .expect("adapter facts should read")
            .facts
            .expect("ready report should contain facts");

        assert_eq!(facts.adapter_drift_events.missing, 0);
        assert_eq!(facts.adapter_drift_events.drifted, 0);
        assert_eq!(facts.adapter_drift_events.failed, 1);
    }

    #[test]
    fn bundle_node_with_foreign_parent_fails_closed() {
        let workspace = TestWorkspace::new("foreign-bundle-node");
        workspace.initialize();
        let end = "2026-07-15T12:00:00.000Z";
        workspace.mutate_fixture(|connection| {
            let bundle = insert_bundle(
                connection,
                1_400,
                "2026-05-01T00:00:00.000Z",
                "foreign-workspace",
                "success",
                None,
                false,
                0,
            );
            insert_bundle_node(connection, &bundle, 1, end, "workflow", "Foreign workflow");
        });
        let captured_at = ReportTimestamp::parse(end).expect("fixture timestamp should parse");

        assert!(matches!(
            effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at),
            Err(ObserveReadError::InvalidStore)
        ));
    }

    #[test]
    fn report_rejects_known_event_with_impossible_outcome() {
        let workspace = TestWorkspace::new("invalid-event");
        workspace.initialize();
        workspace.mutate_fixture(|connection| {
            insert_event(
                connection,
                500,
                "2026-07-15T12:00:00.000Z",
                EventType::NodeCreated,
                EventOutcome::Timeout,
                empty_json(),
                None,
                Some("TOOL_TIMEOUT"),
            );
        });
        let captured_at = ReportTimestamp::parse("2026-07-15T12:00:00.000Z")
            .expect("fixture timestamp should parse");
        assert!(matches!(
            effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at),
            Err(ObserveReadError::InvalidStore)
        ));
    }

    #[test]
    fn feedback_with_foreign_parent_outside_period_is_rejected() {
        let workspace = TestWorkspace::new("foreign-feedback");
        workspace.initialize();
        workspace.mutate_fixture(|connection| {
            let foreign = insert_bundle(
                connection,
                600,
                "2020-01-01T00:00:00.000Z",
                "foreign-workspace",
                "success",
                None,
                false,
                0,
            );
            insert_feedback(
                connection,
                601,
                "2026-07-15T12:00:00.000Z",
                &foreign,
                "useful",
            );
        });
        let captured_at = ReportTimestamp::parse("2026-07-15T12:00:00.000Z")
            .expect("fixture timestamp should parse");
        assert!(matches!(
            effectiveness_report_at(&workspace.paths, WORKSPACE_KEY, &captured_at),
            Err(ObserveReadError::InvalidStore)
        ));
    }

    #[test]
    fn incompatible_store_fails_closed_without_repair() {
        let workspace = TestWorkspace::new("incompatible");
        workspace.initialize();
        workspace.mutate_fixture(|connection| {
            connection
                .execute_batch("PRAGMA user_version = 3;")
                .expect("fixture version should change");
        });
        let before = database_snapshot(workspace.paths.observability_db());
        assert!(matches!(
            observe_status(&workspace.paths, WORKSPACE_KEY),
            Err(ObserveReadError::InvalidStore)
        ));
        let after = database_snapshot(workspace.paths.observability_db());
        assert_eq!(after.0, before.0, "failed read repaired incompatible store");
        assert_eq!(after.1, before.1, "failed read touched incompatible store");
    }

    #[cfg(unix)]
    #[test]
    fn observability_database_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let workspace = TestWorkspace::new("symlink");
        workspace.initialize();
        let target = workspace.home.join("outside.sqlite");
        fs::copy(workspace.paths.observability_db(), &target).expect("fixture target should copy");
        fs::remove_file(workspace.paths.observability_db())
            .expect("managed database should remove for fixture");
        symlink(&target, workspace.paths.observability_db())
            .expect("fixture database symlink should create");
        assert!(matches!(
            observe_status(&workspace.paths, WORKSPACE_KEY),
            Err(ObserveReadError::UnsafePath)
        ));
    }

    #[test]
    fn established_snapshot_excludes_concurrent_continuation_updates() {
        let workspace = TestWorkspace::new("snapshot-race");
        workspace.initialize();
        let start = "2026-06-15T12:00:00.000Z";
        let end = "2026-07-15T12:00:00.000Z";
        let mut bundle_id = String::new();
        workspace.mutate_fixture(|connection| {
            bundle_id = insert_bundle(
                connection,
                700,
                start,
                WORKSPACE_KEY,
                "success",
                None,
                false,
                0,
            );
        });

        let reader = open_reader(&workspace.paths).expect("reader should open");
        let transaction = reader
            .connection
            .unchecked_transaction()
            .expect("read transaction should begin");
        establish_read_snapshot(&transaction).expect("snapshot should establish");

        let writer = Connection::open(workspace.paths.observability_db())
            .expect("concurrent writer should open");
        writer
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign keys should enable");
        writer
            .execute(
                "UPDATE recall_bundles
                 SET continuation_count = 1, more_results = 1
                 WHERE bundle_id = ?1",
                [&bundle_id],
            )
            .expect("continuation parent should update");
        insert_bundle_node(
            &writer,
            &bundle_id,
            1,
            end,
            "workflow",
            "Concurrent workflow",
        );
        drop(writer);

        let (facts, _) = load_effectiveness_facts(
            &transaction,
            WORKSPACE_KEY,
            start,
            end,
            &TaggedValueRedactor::default(),
        )
        .expect("original snapshot should remain readable");
        assert_eq!(facts.recall.continuation_invocations, 0);
        assert_eq!(facts.recall.terminal_more_results_bundles, 0);
        assert!(facts.most_selected.workflows.items.is_empty());
        transaction
            .commit()
            .expect("read transaction should commit");
    }

    fn stored_event(
        event_type: EventType,
        outcome: EventOutcome,
        payload: StoredPayload,
        error_code: Option<&str>,
    ) -> StoredEvent {
        StoredEvent {
            timestamp: "2026-07-15T12:00:00.000Z".to_string(),
            event_type,
            correlation_id: fixed_uuid(1),
            bundle_id: (event_type.is_recall() || event_type.is_task()).then(|| fixed_uuid(2)),
            outcome,
            error_code: error_code.map(str::to_string),
            payload,
        }
    }

    fn counts_payload(keys: &[&str]) -> StoredPayload {
        StoredPayload::Counts(StoredCounts {
            items: keys
                .iter()
                .map(|key| StoredCountItem {
                    name: (*key).to_string(),
                    count: 1,
                })
                .collect(),
        })
    }

    #[test]
    fn report_timestamp_requires_canonical_milliseconds() {
        assert!(ReportTimestamp::parse("2026-07-15T12:00:00.000Z").is_ok());
        for invalid in [
            "2026-07-15 12:00:00.000Z",
            "2026-07-15T12:00:00Z",
            "2026-07-15T12:00:00.000+00:00",
            "2026-02-29T12:00:00.000Z",
            "2024-13-01T12:00:00.000Z",
            "2024-04-31T12:00:00.000Z",
            "2024-01-01T24:00:00.000Z",
        ] {
            assert!(ReportTimestamp::parse(invalid).is_err());
        }
        assert!(ReportTimestamp::parse("2024-02-29T23:59:59.999Z").is_ok());
    }

    #[test]
    fn bounded_top_by_matches_full_sort_at_boundaries_and_ties() {
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct RankedFixture {
            score: usize,
            name: String,
        }

        fn order(left: &RankedFixture, right: &RankedFixture) -> std::cmp::Ordering {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.name.cmp(&right.name))
        }

        fn legacy(mut items: Vec<RankedFixture>) -> TopList<RankedFixture> {
            items.sort_by(order);
            let more_results = items.len() > TOP_LIMIT;
            items.truncate(TOP_LIMIT);
            TopList {
                limit: TOP_LIMIT,
                more_results,
                items,
            }
        }

        for size in [0, TOP_LIMIT, TOP_QUERY_LIMIT] {
            let items = (0..size)
                .rev()
                .map(|index| RankedFixture {
                    score: index % 4,
                    name: format!("item-{index:03}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(bounded_top_by(items.clone(), order), legacy(items));
        }

        let ties = (0..(TOP_LIMIT + 7))
            .rev()
            .map(|index| RankedFixture {
                score: 1,
                name: format!("tie-{index:03}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(bounded_top_by(ties.clone(), order), legacy(ties));
    }

    #[test]
    fn event_catalog_remains_exactly_55() {
        assert_eq!(EventType::ALL.len(), 55);
        assert_eq!(OBSERVABILITY_SCHEMA_VERSION, 2);
    }

    #[test]
    fn producer_contract_accepts_valid_success_and_failure_events() {
        let doctor_keys = ["checks", "ready", "missing", "error"];
        let events = [
            stored_event(
                EventType::TeachProposed,
                EventOutcome::Proposed,
                StoredPayload::Empty,
                None,
            ),
            stored_event(
                EventType::TeachProposed,
                EventOutcome::Failure,
                StoredPayload::Empty,
                Some("VALIDATION_ERROR"),
            ),
            stored_event(
                EventType::TeachApplied,
                EventOutcome::Applied,
                StoredPayload::Empty,
                None,
            ),
            stored_event(
                EventType::ReflectionProposal,
                EventOutcome::Failure,
                StoredPayload::Empty,
                Some("REFLECTION_ERROR"),
            ),
            stored_event(
                EventType::AdapterDrift,
                EventOutcome::Failure,
                StoredPayload::Empty,
                Some("IO_ERROR"),
            ),
            stored_event(
                EventType::Doctor,
                EventOutcome::Success,
                counts_payload(&doctor_keys),
                None,
            ),
            stored_event(
                EventType::Doctor,
                EventOutcome::Warning,
                counts_payload(&doctor_keys),
                None,
            ),
            stored_event(
                EventType::ToolDuplicateDetected,
                EventOutcome::Warning,
                StoredPayload::Tool(StoredTool {
                    tool_id: "overlap-tool".to_string(),
                    approval_present: false,
                }),
                None,
            ),
            stored_event(
                EventType::ToolAliasResolved,
                EventOutcome::Success,
                StoredPayload::Tool(StoredTool {
                    tool_id: "tool-alias".to_string(),
                    approval_present: false,
                }),
                None,
            ),
        ];
        let mut facts = EventFacts::default();
        for event in events {
            validate_event_contract(&event).expect("producer event should be valid");
            accumulate_event(event, &mut facts, &TaggedValueRedactor::default())
                .expect("valid event should aggregate safely");
        }
        assert_eq!(facts.adapter_drift.failed, 1);
    }

    #[test]
    fn task_event_contract_accepts_only_factual_lifecycle_shapes() {
        let task = || {
            StoredPayload::Task(StoredTask {
                task_id: fixed_uuid(9),
            })
        };
        for event in [
            stored_event(EventType::TaskStarted, EventOutcome::Started, task(), None),
            stored_event(
                EventType::TaskContextApplied,
                EventOutcome::Applied,
                task(),
                None,
            ),
            stored_event(
                EventType::TaskCompleted,
                EventOutcome::Success,
                task(),
                None,
            ),
            stored_event(
                EventType::TaskFailed,
                EventOutcome::Failure,
                task(),
                Some("TASK_FAILED"),
            ),
        ] {
            validate_event_contract(&event).expect("task lifecycle event should validate");
        }

        let mut missing_bundle =
            stored_event(EventType::TaskStarted, EventOutcome::Started, task(), None);
        missing_bundle.bundle_id = None;
        assert!(validate_event_contract(&missing_bundle).is_err());
        assert!(validate_event_contract(&stored_event(
            EventType::TaskCompleted,
            EventOutcome::Failure,
            task(),
            Some("TASK_FAILED"),
        ))
        .is_err());
    }

    #[test]
    fn event_contract_rejects_known_but_impossible_combinations() {
        for event in [
            stored_event(
                EventType::NodeCreated,
                EventOutcome::Timeout,
                StoredPayload::Empty,
                Some("TOOL_TIMEOUT"),
            ),
            stored_event(
                EventType::ReflectionProposal,
                EventOutcome::Proposed,
                StoredPayload::Empty,
                None,
            ),
            stored_event(
                EventType::Doctor,
                EventOutcome::Success,
                counts_payload(&["checks", "ready", "missing", "extra"]),
                None,
            ),
            stored_event(
                EventType::ArtifactsCleanup,
                EventOutcome::Success,
                counts_payload(&["deleted_paths"]),
                None,
            ),
        ] {
            assert!(matches!(
                validate_event_contract(&event),
                Err(ObserveReadError::InvalidStore)
            ));
        }
    }

    #[test]
    fn stored_payload_json_rejects_unknown_fields_at_every_level() {
        for json in [
            r#"{"kind":"empty","extra":1}"#,
            r#"{"kind":"tool","data":{"tool_id":"safe","approval_present":false,"extra":1}}"#,
            r#"{"kind":"counts","data":{"items":[{"name":"items","count":1,"extra":1}]}}"#,
        ] {
            assert!(serde_json::from_str::<StoredPayload>(json).is_err());
        }
    }

    #[test]
    fn report_redacts_tool_ids_and_error_codes_again() {
        let tool_secret = "sk-1234567890abcdef";
        let error_secret = "ghp_1234567890abcdef";
        let mut facts = EventFacts::default();
        for timestamp in ["2026-07-15T10:00:00.000Z", "2026-07-15T11:00:00.000Z"] {
            record_tool_error(
                timestamp.to_string(),
                Some(error_secret.to_string()),
                StoredTool {
                    tool_id: tool_secret.to_string(),
                    approval_present: false,
                },
                &mut facts,
                &TaggedValueRedactor::default(),
            )
            .expect("bounded tool facts should aggregate");
        }
        let json = serde_json::to_string(&repeated_tool_errors(facts.tool_errors))
            .expect("facts should serialize");
        assert!(!json.contains(tool_secret));
        assert!(!json.contains(error_secret));
        assert!(json.contains("[REDACTED]"));
    }

    #[test]
    fn stored_safe_text_rejects_nul() {
        assert!(StoredTool {
            tool_id: "unsafe\0tool".to_string(),
            approval_present: false,
        }
        .validate()
        .is_err());
        assert!(StoredMcp {
            mcp_id: "unsafe\0mcp".to_string(),
            status: StoredMcpStatus::Missing,
        }
        .validate()
        .is_err());
    }

    #[test]
    fn recall_payload_allows_same_reason_for_distinct_selected_nodes() {
        let payload = StoredRecall {
            node_count: 2,
            more_results: false,
            continuation_count: 0,
            fts_fallback_used: false,
            graph_traversal_used: false,
            selected_node_ids: vec![1, 2],
            selection_reasons: vec![
                StoredSelectionReason::TypedRoot,
                StoredSelectionReason::TypedRoot,
            ],
            scores: Vec::new(),
        };
        payload
            .validate()
            .expect("different nodes may share one selection reason");
    }
}
