use std::collections::BTreeSet;

use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;

use crate::observability::report::ReportTimestamp;
use crate::observability::{open_reader, EventOutcome, EventType, ObservabilityOpenError};
use crate::storage::WorkspacePaths;

const MAX_PAGE_SIZE: usize = 500;
const MAX_CURSOR_BYTES: usize = 4_096;
const OVERVIEW_ERROR_LIMIT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiObservabilityAvailability {
    Missing,
    Present,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiReadError {
    InvalidRequest,
    InvalidCursor,
    NotFound,
    Unavailable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ActivityQuery {
    pub(crate) limit: usize,
    pub(crate) cursor: Option<String>,
    pub(crate) event_type: Option<String>,
    pub(crate) outcome: Option<String>,
    pub(crate) command: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BundleQuery {
    pub(crate) bundle_id: String,
    pub(crate) limit: usize,
    pub(crate) cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiCollectionStatus {
    NotCollected,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ActivityItem {
    id: String,
    timestamp: String,
    product_version: String,
    event_type: String,
    command: String,
    correlation_id: String,
    bundle_id: Option<String>,
    duration_ms: Option<u64>,
    outcome: String,
    error_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ActivityResponse {
    collection_status: UiCollectionStatus,
    limit: usize,
    items: Vec<ActivityItem>,
    more_results: bool,
    next_cursor: Option<String>,
    complete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct BundleSummary {
    bundle_id: String,
    timestamp: String,
    product_version: String,
    correlation_id: String,
    outcome: String,
    error_code: Option<String>,
    duration_ms: u64,
    more_results: bool,
    continuation_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct BundleNodeItem {
    node_id: i64,
    first_seen_at: String,
    node_type: String,
    node_title: String,
    bounded_summary: Option<String>,
    source_ref: Option<String>,
    trust_level: Option<String>,
    confidence: Option<f64>,
    score: Option<f64>,
    selection_reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BundleResponse {
    bundle: BundleSummary,
    limit: usize,
    nodes: Vec<BundleNodeItem>,
    more_results: bool,
    next_cursor: Option<String>,
    complete: bool,
}

#[derive(Serialize)]
pub(crate) struct OverviewObservability {
    collection_status: UiCollectionStatus,
    health: crate::observability::export::HealthSummary,
    last_recall: Option<BundleSummary>,
    last_errors: Vec<ActivityItem>,
    last_errors_more_results: bool,
}

impl OverviewObservability {
    pub(crate) fn is_available(&self) -> bool {
        self.collection_status == UiCollectionStatus::Ready
    }
}

/// Inspects the optional UI observability source without creating it.
pub(crate) fn availability(
    workspace_paths: &WorkspacePaths,
) -> Result<UiObservabilityAvailability, ObservabilityOpenError> {
    match open_reader(workspace_paths) {
        Ok(reader) => {
            drop(reader);
            Ok(UiObservabilityAvailability::Present)
        }
        Err(ObservabilityOpenError::Missing(_)) => Ok(UiObservabilityAvailability::Missing),
        Err(error) => Err(error),
    }
}

pub(crate) fn activity(
    workspace_paths: &WorkspacePaths,
    workspace_key: &str,
    query: &ActivityQuery,
) -> Result<ActivityResponse, UiReadError> {
    validate_activity_query(query)?;
    let scope = activity_scope(query);
    let cursor = decode_activity_cursor(query.cursor.as_deref(), &scope)?;
    let Some(reader) = optional_reader(workspace_paths)? else {
        return Ok(ActivityResponse {
            collection_status: UiCollectionStatus::NotCollected,
            limit: query.limit,
            items: Vec::new(),
            more_results: false,
            next_cursor: None,
            complete: true,
        });
    };
    let transaction = reader
        .connection
        .unchecked_transaction()
        .map_err(|_| UiReadError::Unavailable)?;
    let response = activity_in_transaction(&transaction, workspace_key, query, &scope, cursor)?;
    transaction.commit().map_err(|_| UiReadError::Unavailable)?;
    Ok(response)
}

pub(crate) fn bundle(
    workspace_paths: &WorkspacePaths,
    workspace_key: &str,
    query: &BundleQuery,
) -> Result<BundleResponse, UiReadError> {
    if query.limit == 0 || query.limit > MAX_PAGE_SIZE {
        return Err(UiReadError::InvalidRequest);
    }
    super::validate_uuid_v4(&query.bundle_id).map_err(|_| UiReadError::InvalidRequest)?;
    let scope = format!("bundle={}", query.bundle_id);
    let after_node_id = decode_numeric_cursor(query.cursor.as_deref(), "bundle", &scope)?;
    let Some(reader) = optional_reader(workspace_paths)? else {
        return Err(UiReadError::NotFound);
    };
    let transaction = reader
        .connection
        .unchecked_transaction()
        .map_err(|_| UiReadError::Unavailable)?;
    let bundle = load_bundle_summary(&transaction, workspace_key, &query.bundle_id)?
        .ok_or(UiReadError::NotFound)?;
    let mut statement = transaction
        .prepare(
            "SELECT node.node_id, node.first_seen_at, node.node_type,
                    node.node_title, node.bounded_summary, node.source_ref,
                    node.trust_level, node.confidence, node.score,
                    node.selection_reasons_json
             FROM bundle_nodes AS node
             JOIN recall_bundles AS bundle USING (bundle_id)
             WHERE node.bundle_id = ?1 AND bundle.workspace_key = ?2
               AND node.node_id > ?3
             ORDER BY node.node_id ASC LIMIT ?4",
        )
        .map_err(|_| UiReadError::Unavailable)?;
    let mut nodes = statement
        .query_map(
            params![
                query.bundle_id,
                workspace_key,
                after_node_id,
                fetch_limit(query.limit)?
            ],
            bundle_node_from_row,
        )
        .map_err(|_| UiReadError::Unavailable)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| UiReadError::Unavailable)?;
    drop(statement);
    validate_bundle_nodes(&nodes)?;
    redact_bundle_nodes(&mut nodes);
    let more_results = nodes.len() > query.limit;
    nodes.truncate(query.limit);
    let next_cursor = if more_results {
        let node_id = nodes.last().ok_or(UiReadError::Unavailable)?.node_id;
        Some(encode_numeric_cursor("bundle", &scope, node_id)?)
    } else {
        None
    };
    transaction.commit().map_err(|_| UiReadError::Unavailable)?;
    Ok(BundleResponse {
        bundle,
        limit: query.limit,
        nodes,
        more_results,
        next_cursor,
        complete: !more_results,
    })
}

pub(crate) fn overview(
    workspace_paths: &WorkspacePaths,
    workspace_key: &str,
) -> Result<OverviewObservability, UiReadError> {
    let Some(reader) = optional_reader(workspace_paths)? else {
        let health = crate::observability::export::build_health_summary(
            None,
            workspace_key,
            crate::observability::report::CollectionStatus::NotCollected,
        )
        .map_err(|_| UiReadError::Unavailable)?;
        return Ok(OverviewObservability {
            collection_status: UiCollectionStatus::NotCollected,
            health,
            last_recall: None,
            last_errors: Vec::new(),
            last_errors_more_results: false,
        });
    };
    let transaction = reader
        .connection
        .unchecked_transaction()
        .map_err(|_| UiReadError::Unavailable)?;
    let health = crate::observability::export::build_health_summary(
        Some(&transaction),
        workspace_key,
        crate::observability::report::CollectionStatus::Ready,
    )
    .map_err(|_| UiReadError::Unavailable)?;
    let last_recall = load_latest_bundle_summary(&transaction, workspace_key)?;
    let error_query = ActivityQuery {
        limit: OVERVIEW_ERROR_LIMIT,
        cursor: None,
        event_type: None,
        outcome: None,
        command: None,
    };
    let (last_errors, last_errors_more_results) =
        load_latest_errors(&transaction, workspace_key, error_query.limit)?;
    transaction.commit().map_err(|_| UiReadError::Unavailable)?;
    Ok(OverviewObservability {
        collection_status: UiCollectionStatus::Ready,
        health,
        last_recall,
        last_errors,
        last_errors_more_results,
    })
}

fn optional_reader(
    workspace_paths: &WorkspacePaths,
) -> Result<Option<super::ObservabilityReader>, UiReadError> {
    match open_reader(workspace_paths) {
        Ok(reader) => Ok(Some(reader)),
        Err(ObservabilityOpenError::Missing(_)) => Ok(None),
        Err(_) => Err(UiReadError::Unavailable),
    }
}

fn validate_activity_query(query: &ActivityQuery) -> Result<(), UiReadError> {
    if query.limit == 0 || query.limit > MAX_PAGE_SIZE {
        return Err(UiReadError::InvalidRequest);
    }
    if query.event_type.as_deref().is_some_and(|value| {
        !EventType::ALL
            .iter()
            .any(|event_type| event_type.as_str() == value)
    }) || query.outcome.as_deref().is_some_and(|value| {
        ![
            EventOutcome::Started,
            EventOutcome::Success,
            EventOutcome::Failure,
            EventOutcome::Warning,
            EventOutcome::Empty,
            EventOutcome::Truncated,
            EventOutcome::Overflow,
            EventOutcome::Pending,
            EventOutcome::Blocked,
            EventOutcome::Timeout,
            EventOutcome::Recorded,
            EventOutcome::Proposed,
            EventOutcome::Applied,
            EventOutcome::Drafted,
            EventOutcome::Missing,
            EventOutcome::Configured,
            EventOutcome::ConfiguredUnverified,
        ]
        .iter()
        .any(|outcome| outcome.as_str() == value)
    }) {
        return Err(UiReadError::InvalidRequest);
    }
    if let Some(command) = query.command.as_deref() {
        super::validate_ascii_identifier("command", command, 128)
            .map_err(|_| UiReadError::InvalidRequest)?;
    }
    Ok(())
}

fn activity_scope(query: &ActivityQuery) -> String {
    format!(
        "event={};outcome={};command={}",
        query.event_type.as_deref().unwrap_or(""),
        query.outcome.as_deref().unwrap_or(""),
        query.command.as_deref().unwrap_or("")
    )
}

fn activity_in_transaction(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    query: &ActivityQuery,
    scope: &str,
    cursor: Option<(String, String)>,
) -> Result<ActivityResponse, UiReadError> {
    let (cursor_timestamp, cursor_id) = cursor
        .map(|(timestamp, id)| (Some(timestamp), Some(id)))
        .unwrap_or((None, None));
    let mut statement = transaction
        .prepare(
            "SELECT id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, bundle_id, duration_ms, outcome,
                    error_code, payload_json
             FROM observability_events
             WHERE workspace_key = ?1
               AND (?2 IS NULL OR event_type = ?2)
               AND (?3 IS NULL OR outcome = ?3)
               AND (?4 IS NULL OR command = ?4)
               AND (?5 IS NULL OR timestamp < ?5
                    OR (timestamp = ?5 AND id < ?6))
             ORDER BY timestamp DESC, id DESC LIMIT ?7",
        )
        .map_err(|_| UiReadError::Unavailable)?;
    let mut rows = statement
        .query(params![
            workspace_key,
            query.event_type.as_deref(),
            query.outcome.as_deref(),
            query.command.as_deref(),
            cursor_timestamp.as_deref(),
            cursor_id.as_deref(),
            fetch_limit(query.limit)?
        ])
        .map_err(|_| UiReadError::Unavailable)?;
    let mut items = Vec::new();
    while let Some(row) = rows.next().map_err(|_| UiReadError::Unavailable)? {
        items.push(activity_item_from_row(row, workspace_key)?);
    }
    let more_results = items.len() > query.limit;
    items.truncate(query.limit);
    let next_cursor = if more_results {
        let last = items.last().ok_or(UiReadError::Unavailable)?;
        Some(encode_activity_cursor(scope, &last.timestamp, &last.id)?)
    } else {
        None
    };
    Ok(ActivityResponse {
        collection_status: UiCollectionStatus::Ready,
        limit: query.limit,
        items,
        more_results,
        next_cursor,
        complete: !more_results,
    })
}

fn activity_item_from_row(
    row: &rusqlite::Row<'_>,
    workspace_key: &str,
) -> Result<ActivityItem, UiReadError> {
    super::report::validate_event_row(row, workspace_key).map_err(|_| UiReadError::Unavailable)?;
    let duration_ms = row
        .get::<_, Option<i64>>(8)
        .map_err(|_| UiReadError::Unavailable)?
        .map(|value| u64::try_from(value).map_err(|_| UiReadError::Unavailable))
        .transpose()?;
    Ok(ActivityItem {
        id: row.get(0).map_err(|_| UiReadError::Unavailable)?,
        timestamp: row.get(1).map_err(|_| UiReadError::Unavailable)?,
        product_version: safe_observability_text(
            row.get::<_, String>(2)
                .map_err(|_| UiReadError::Unavailable)?,
            128,
        ),
        event_type: row.get(4).map_err(|_| UiReadError::Unavailable)?,
        command: row.get(5).map_err(|_| UiReadError::Unavailable)?,
        correlation_id: row.get(6).map_err(|_| UiReadError::Unavailable)?,
        bundle_id: row.get(7).map_err(|_| UiReadError::Unavailable)?,
        duration_ms,
        outcome: row.get(9).map_err(|_| UiReadError::Unavailable)?,
        error_code: row
            .get::<_, Option<String>>(10)
            .map_err(|_| UiReadError::Unavailable)?
            .map(|value| safe_observability_text(value, 128)),
    })
}

fn load_latest_errors(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    limit: usize,
) -> Result<(Vec<ActivityItem>, bool), UiReadError> {
    let mut statement = transaction
        .prepare(
            "SELECT id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, bundle_id, duration_ms, outcome,
                    error_code, payload_json
             FROM observability_events
             WHERE workspace_key = ?1
               AND outcome IN ('failure', 'timeout', 'overflow')
             ORDER BY timestamp DESC, id DESC LIMIT ?2",
        )
        .map_err(|_| UiReadError::Unavailable)?;
    let mut rows = statement
        .query(params![workspace_key, fetch_limit(limit)?])
        .map_err(|_| UiReadError::Unavailable)?;
    let mut items = Vec::new();
    while let Some(row) = rows.next().map_err(|_| UiReadError::Unavailable)? {
        items.push(activity_item_from_row(row, workspace_key)?);
    }
    let more_results = items.len() > limit;
    items.truncate(limit);
    Ok((items, more_results))
}

fn load_bundle_summary(
    transaction: &Transaction<'_>,
    workspace_key: &str,
    bundle_id: &str,
) -> Result<Option<BundleSummary>, UiReadError> {
    transaction
        .query_row(
            "SELECT bundle_id, timestamp, product_version, workspace_key,
                    correlation_id, outcome, error_code, duration_ms,
                    more_results, continuation_count
             FROM recall_bundles
             WHERE bundle_id = ?1 AND workspace_key = ?2",
            params![bundle_id, workspace_key],
            |row| bundle_summary_from_row(row, workspace_key).map_err(ui_to_sql_error),
        )
        .optional()
        .map_err(|_| UiReadError::Unavailable)
}

fn load_latest_bundle_summary(
    transaction: &Transaction<'_>,
    workspace_key: &str,
) -> Result<Option<BundleSummary>, UiReadError> {
    transaction
        .query_row(
            "SELECT bundle_id, timestamp, product_version, workspace_key,
                    correlation_id, outcome, error_code, duration_ms,
                    more_results, continuation_count
             FROM recall_bundles
             WHERE workspace_key = ?1
             ORDER BY timestamp DESC, bundle_id DESC LIMIT 1",
            [workspace_key],
            |row| bundle_summary_from_row(row, workspace_key).map_err(ui_to_sql_error),
        )
        .optional()
        .map_err(|_| UiReadError::Unavailable)
}

fn bundle_summary_from_row(
    row: &rusqlite::Row<'_>,
    workspace_key: &str,
) -> Result<BundleSummary, UiReadError> {
    let bundle_id: String = row.get(0).map_err(|_| UiReadError::Unavailable)?;
    let timestamp: String = row.get(1).map_err(|_| UiReadError::Unavailable)?;
    let product_version: String = row.get(2).map_err(|_| UiReadError::Unavailable)?;
    let stored_workspace: String = row.get(3).map_err(|_| UiReadError::Unavailable)?;
    let correlation_id: String = row.get(4).map_err(|_| UiReadError::Unavailable)?;
    let outcome: String = row.get(5).map_err(|_| UiReadError::Unavailable)?;
    let error_code: Option<String> = row.get(6).map_err(|_| UiReadError::Unavailable)?;
    let duration_ms: i64 = row.get(7).map_err(|_| UiReadError::Unavailable)?;
    let more_results: i64 = row.get(8).map_err(|_| UiReadError::Unavailable)?;
    let continuation_count: i64 = row.get(9).map_err(|_| UiReadError::Unavailable)?;
    super::validate_uuid_v4(&bundle_id).map_err(|_| UiReadError::Unavailable)?;
    super::validate_uuid_v4(&correlation_id).map_err(|_| UiReadError::Unavailable)?;
    ReportTimestamp::parse(&timestamp).map_err(|_| UiReadError::Unavailable)?;
    if stored_workspace != workspace_key
        || product_version.trim().is_empty()
        || product_version.len() > 128
        || product_version.as_bytes().contains(&0)
        || !matches!(more_results, 0 | 1)
        || duration_ms < 0
        || continuation_count < 0
        || !match outcome.as_str() {
            "success" => error_code.is_none(),
            "failure" => error_code.as_deref().is_some_and(valid_identifier),
            _ => false,
        }
    {
        return Err(UiReadError::Unavailable);
    }
    Ok(BundleSummary {
        bundle_id,
        timestamp,
        product_version: safe_observability_text(product_version, 128),
        correlation_id,
        outcome,
        error_code: error_code.map(|value| safe_observability_text(value, 128)),
        duration_ms: u64::try_from(duration_ms).map_err(|_| UiReadError::Unavailable)?,
        more_results: more_results == 1,
        continuation_count: u64::try_from(continuation_count)
            .map_err(|_| UiReadError::Unavailable)?,
    })
}

fn bundle_node_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BundleNodeItem> {
    let reasons_json: String = row.get(9)?;
    let selection_reasons = serde_json::from_str(&reasons_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(BundleNodeItem {
        node_id: row.get(0)?,
        first_seen_at: row.get(1)?,
        node_type: row.get(2)?,
        node_title: row.get(3)?,
        bounded_summary: row.get(4)?,
        source_ref: row.get(5)?,
        trust_level: row.get(6)?,
        confidence: row.get(7)?,
        score: row.get(8)?,
        selection_reasons,
    })
}

fn validate_bundle_nodes(nodes: &[BundleNodeItem]) -> Result<(), UiReadError> {
    const REASONS: &[&str] = &[
        "mandatory",
        "typed_root",
        "fts_bm25",
        "direct_link",
        "graph_traversal",
        "workflow",
        "tool",
        "failure_mode",
        "source",
        "trust",
        "confidence",
    ];
    for node in nodes {
        let unique_reasons = node.selection_reasons.iter().collect::<BTreeSet<_>>();
        if node.node_id <= 0
            || !crate::storage::ALLOWED_NODE_TYPES.contains(&node.node_type.as_str())
            || !valid_text(&node.node_title, 512, true)
            || node
                .bounded_summary
                .as_deref()
                .is_some_and(|value| !valid_text(value, 2_048, false))
            || node
                .source_ref
                .as_deref()
                .is_some_and(|value| !valid_text(value, 2_048, false))
            || node
                .trust_level
                .as_deref()
                .is_some_and(|value| !valid_text(value, 256, true))
            || node
                .confidence
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || node.score.is_some_and(|value| !value.is_finite())
            || ReportTimestamp::parse(&node.first_seen_at).is_err()
            || node.selection_reasons.is_empty()
            || node.selection_reasons.len() > 64
            || unique_reasons.len() != node.selection_reasons.len()
            || node
                .selection_reasons
                .iter()
                .any(|reason| !REASONS.contains(&reason.as_str()))
        {
            return Err(UiReadError::Unavailable);
        }
    }
    Ok(())
}

fn redact_bundle_nodes(nodes: &mut [BundleNodeItem]) {
    for node in nodes {
        node.node_title = safe_observability_text(std::mem::take(&mut node.node_title), 512);
        node.bounded_summary = node
            .bounded_summary
            .take()
            .map(|value| safe_observability_text(value, 2_048));
        node.source_ref = node
            .source_ref
            .take()
            .map(|value| safe_observability_text(value, 2_048));
        node.trust_level = node
            .trust_level
            .take()
            .map(|value| safe_observability_text(value, 256));
    }
}

fn fetch_limit(limit: usize) -> Result<i64, UiReadError> {
    i64::try_from(limit)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(UiReadError::InvalidRequest)
}

fn encode_activity_cursor(scope: &str, timestamp: &str, id: &str) -> Result<String, UiReadError> {
    ReportTimestamp::parse(timestamp).map_err(|_| UiReadError::Unavailable)?;
    super::validate_uuid_v4(id).map_err(|_| UiReadError::Unavailable)?;
    let cursor = format!(
        "o1.activity.{}.{}.{}",
        lowercase_hex(scope.as_bytes()),
        lowercase_hex(timestamp.as_bytes()),
        lowercase_hex(id.as_bytes())
    );
    if cursor.len() > MAX_CURSOR_BYTES {
        Err(UiReadError::Unavailable)
    } else {
        Ok(cursor)
    }
}

fn decode_activity_cursor(
    cursor: Option<&str>,
    scope: &str,
) -> Result<Option<(String, String)>, UiReadError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(UiReadError::InvalidCursor);
    }
    let prefix = format!("o1.activity.{}.", lowercase_hex(scope.as_bytes()));
    let payload = cursor
        .strip_prefix(&prefix)
        .ok_or(UiReadError::InvalidCursor)?;
    let (timestamp, id) = payload.split_once('.').ok_or(UiReadError::InvalidCursor)?;
    let timestamp = decode_hex(timestamp)?;
    let id = decode_hex(id)?;
    if encode_activity_cursor(scope, &timestamp, &id).as_deref() != Ok(cursor) {
        return Err(UiReadError::InvalidCursor);
    }
    Ok(Some((timestamp, id)))
}

fn encode_numeric_cursor(kind: &str, scope: &str, id: i64) -> Result<String, UiReadError> {
    if id <= 0 {
        return Err(UiReadError::Unavailable);
    }
    let cursor = format!("o1.{kind}.{}.{}", lowercase_hex(scope.as_bytes()), id);
    if cursor.len() > MAX_CURSOR_BYTES {
        Err(UiReadError::Unavailable)
    } else {
        Ok(cursor)
    }
}

fn decode_numeric_cursor(
    cursor: Option<&str>,
    kind: &str,
    scope: &str,
) -> Result<i64, UiReadError> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(UiReadError::InvalidCursor);
    }
    let prefix = format!("o1.{kind}.{}.", lowercase_hex(scope.as_bytes()));
    let id = cursor
        .strip_prefix(&prefix)
        .ok_or(UiReadError::InvalidCursor)?
        .parse::<i64>()
        .map_err(|_| UiReadError::InvalidCursor)?;
    if encode_numeric_cursor(kind, scope, id).as_deref() != Ok(cursor) {
        return Err(UiReadError::InvalidCursor);
    }
    Ok(id)
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn decode_hex(value: &str) -> Result<String, UiReadError> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err(UiReadError::InvalidCursor);
    }
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        decoded.push(hex_nibble(pair[0])? * 16 + hex_nibble(pair[1])?);
    }
    String::from_utf8(decoded).map_err(|_| UiReadError::InvalidCursor)
}

fn hex_nibble(byte: u8) -> Result<u8, UiReadError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(UiReadError::InvalidCursor),
    }
}

fn safe_observability_text(value: String, maximum_bytes: usize) -> String {
    super::truncate_utf8(super::redact_sensitive_text(&value), maximum_bytes)
}

fn valid_text(value: &str, maximum_bytes: usize, required: bool) -> bool {
    value.len() <= maximum_bytes
        && !value.as_bytes().contains(&0)
        && (!required || !value.trim().is_empty())
}

fn valid_identifier(value: &str) -> bool {
    super::validate_ascii_identifier("stored_identifier", value, 128).is_ok()
}

fn ui_to_sql_error(_error: UiReadError) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
