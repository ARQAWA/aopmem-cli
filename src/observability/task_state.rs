//! Authoritative task state persistence for Local Observability schema v2.

use super::{open_reader, open_writer, RetentionPolicy, SafeText};
use crate::redaction::TaggedValueRedactor;
use crate::storage::WorkspacePaths;
use crate::task::{
    self, AppliedNodeKind, AppliedTaskNode, TaskApplyInput, TaskBundleId, TaskBundleNode,
    TaskCompletionInput, TaskContextKind, TaskFingerprint, TaskId, TaskResult, TaskStartInput,
    TaskState, TaskStateError, TaskStatus,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

const TASK_REDACTION_UNAVAILABLE_ERROR_CODE: &str = "TASK_REDACTION_UNAVAILABLE";

pub(crate) fn record_started(
    workspace_paths: &WorkspacePaths,
    input: &TaskStartInput,
) -> Result<TaskState, TaskStateError> {
    require_path_workspace(workspace_paths, &input.workspace_key)?;
    let mut writer = open_writer(workspace_paths).map_err(|_| TaskStateError::StoreUnavailable)?;
    writer
        .apply_retention(RetentionPolicy::default())
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    let transaction = writer
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| TaskStateError::StoreUnavailable)?;

    let existing_task_id = transaction
        .query_row(
            "SELECT task_id FROM tasks
             WHERE task_id = ?1 OR bundle_id = ?2
             ORDER BY task_id = ?1 DESC
             LIMIT 1",
            rusqlite::params![input.task_id.as_str(), input.bundle_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    if let Some(existing_task_id) = existing_task_id {
        let existing_id =
            TaskId::parse(&existing_task_id).map_err(|_| TaskStateError::StoreUnavailable)?;
        let state = load_state(&transaction, &existing_id)?;
        if !start_matches(&state, input) {
            return Err(TaskStateError::ConflictingReplay);
        }
        transaction
            .commit()
            .map_err(|_| TaskStateError::StoreUnavailable)?;
        return Ok(state);
    }

    let timestamp =
        super::current_timestamp(&transaction).map_err(|_| TaskStateError::StoreUnavailable)?;
    transaction
        .execute(
            "INSERT INTO tasks (
                task_id, bundle_id, product_version, workspace_key,
                memory_revision, query_fingerprint, status, started_at,
                mandatory_context_complete, retrieval_complete, budget_exhausted
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'started', ?7, ?8, ?9, ?10)",
            rusqlite::params![
                input.task_id.as_str(),
                input.bundle_id.as_str(),
                env!("CARGO_PKG_VERSION"),
                input.workspace_key,
                input.memory_revision.as_str(),
                input.query_fingerprint.as_str(),
                timestamp,
                i64::from(input.mandatory_context_complete),
                i64::from(input.retrieval_complete),
                i64::from(input.budget_exhausted),
            ],
        )
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    for node in &input.nodes {
        transaction
            .execute(
                "INSERT INTO task_bundle_nodes (
                    task_id, node_id, node_type, context_kind
                 ) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    input.task_id.as_str(),
                    node.node_id,
                    node.node_type,
                    node.context_kind.as_str(),
                ],
            )
            .map_err(|_| TaskStateError::StoreUnavailable)?;
    }
    let state = load_state(&transaction, &input.task_id)?;
    transaction
        .commit()
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    Ok(state)
}

pub(crate) fn record_context_applied(
    workspace_paths: &WorkspacePaths,
    input: &TaskApplyInput,
) -> Result<TaskState, TaskStateError> {
    require_path_workspace(workspace_paths, &input.workspace_key)?;
    let mut writer = open_writer(workspace_paths).map_err(|_| TaskStateError::StoreUnavailable)?;
    let transaction = writer
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    let state = load_state(&transaction, &input.task_id)?;
    validate_identity(
        &state,
        &input.bundle_id,
        &input.workspace_key,
        &input.memory_revision,
    )?;

    if let Some(stored) = &state.apply_fingerprint {
        if stored == &input.replay_fingerprint {
            transaction
                .commit()
                .map_err(|_| TaskStateError::StoreUnavailable)?;
            return Ok(state);
        }
        return Err(TaskStateError::ConflictingReplay);
    }
    if state.status != TaskStatus::Started {
        return Err(TaskStateError::InvalidTransition);
    }
    validate_none_relevant(&transaction, &state, input)?;
    validate_applied_nodes(&state.bundle_nodes, &input.nodes)?;

    for node in &input.nodes {
        transaction
            .execute(
                "INSERT INTO task_applied_nodes (task_id, node_id, application_kind)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![input.task_id.as_str(), node.node_id, node.kind.as_str(),],
            )
            .map_err(|_| TaskStateError::StoreUnavailable)?;
    }
    let changed = transaction
        .execute(
            "UPDATE tasks
             SET status = 'applied',
                 applied_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 none_relevant = ?2,
                 apply_fingerprint = ?3
             WHERE task_id = ?1 AND status = 'started'",
            rusqlite::params![
                input.task_id.as_str(),
                i64::from(input.none_relevant),
                input.replay_fingerprint.as_str(),
            ],
        )
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    if changed != 1 {
        return Err(TaskStateError::InvalidTransition);
    }
    let state = load_state(&transaction, &input.task_id)?;
    transaction
        .commit()
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    Ok(state)
}

pub(crate) fn record_completed(
    workspace_paths: &WorkspacePaths,
    input: &TaskCompletionInput,
) -> Result<TaskState, TaskStateError> {
    require_path_workspace(workspace_paths, &input.workspace_key)?;
    let redactor = TaggedValueRedactor::load_workspace(workspace_paths);
    let (redacted_error_code, redacted_reason) =
        safe_completion_details(input, redactor.as_ref().ok());
    let mut writer = open_writer(workspace_paths).map_err(|_| TaskStateError::StoreUnavailable)?;
    let transaction = writer
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    let state = load_state(&transaction, &input.task_id)?;
    validate_identity(
        &state,
        &input.bundle_id,
        &input.workspace_key,
        &input.memory_revision,
    )?;

    if let Some(stored) = &state.completion_fingerprint {
        if stored == &input.replay_fingerprint {
            transaction
                .commit()
                .map_err(|_| TaskStateError::StoreUnavailable)?;
            return Ok(safe_task_state(state, redactor.as_ref().ok()));
        }
        return Err(TaskStateError::ConflictingReplay);
    }
    let transition_allowed = match input.result {
        TaskResult::Success | TaskResult::Partial => state.status == TaskStatus::Applied,
        TaskResult::Failed => {
            matches!(state.status, TaskStatus::Started | TaskStatus::Applied)
        }
    };
    if !transition_allowed {
        return Err(TaskStateError::InvalidTransition);
    }
    let terminal_status = if input.result == TaskResult::Failed {
        TaskStatus::Failed
    } else {
        TaskStatus::Completed
    };
    let duration_ms: i64 = transaction
        .query_row(
            "SELECT MAX(
                0,
                CAST(ROUND(
                    (julianday('now') - julianday(started_at)) * 86400000.0
                ) AS INTEGER)
             )
             FROM tasks WHERE task_id = ?1",
            [input.task_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    let changed = transaction
        .execute(
            "UPDATE tasks
             SET status = ?2,
                 finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 completion_fingerprint = ?3,
                 completion_result = ?4,
                 duration_ms = ?5,
                 error_code = ?6,
                 reason = ?7
             WHERE task_id = ?1
               AND completion_fingerprint IS NULL",
            rusqlite::params![
                input.task_id.as_str(),
                terminal_status.as_str(),
                input.replay_fingerprint.as_str(),
                input.result.as_str(),
                duration_ms,
                redacted_error_code.as_deref(),
                redacted_reason.as_deref(),
            ],
        )
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    if changed != 1 {
        return Err(TaskStateError::InvalidTransition);
    }
    let state = safe_task_state(
        load_state(&transaction, &input.task_id)?,
        redactor.as_ref().ok(),
    );
    transaction
        .commit()
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    Ok(state)
}

pub(crate) fn load(
    workspace_paths: &WorkspacePaths,
    task_id: &TaskId,
) -> Result<TaskState, TaskStateError> {
    let redactor = TaggedValueRedactor::load_workspace(workspace_paths);
    let reader = open_reader(workspace_paths).map_err(|_| TaskStateError::StoreUnavailable)?;
    load_state(&reader.connection, task_id)
        .map(|state| safe_task_state(state, redactor.as_ref().ok()))
}

fn safe_completion_details(
    input: &TaskCompletionInput,
    redactor: Option<&TaggedValueRedactor>,
) -> (Option<String>, Option<String>) {
    let Some(redactor) = redactor else {
        return unavailable_completion_details(input.result);
    };
    let error_code = input
        .error_code
        .as_deref()
        .map(|value| redactor.redact_str_bounded(value, 96))
        .transpose();
    let reason = input
        .reason
        .as_deref()
        .map(|value| {
            redactor
                .redact_str_bounded(value, 1_024)
                .map_err(|_| TaskStateError::InvalidReason)
                .and_then(|value| {
                    SafeText::new("task_reason", &value, 1_024)
                        .map(|safe| safe.rendered().to_string())
                        .map_err(|_| TaskStateError::InvalidReason)
                })
        })
        .transpose();
    match (error_code, reason) {
        (Ok(error_code), Ok(reason)) => (error_code, reason),
        _ => unavailable_completion_details(input.result),
    }
}

fn unavailable_completion_details(result: TaskResult) -> (Option<String>, Option<String>) {
    let error_code =
        (result == TaskResult::Failed).then(|| TASK_REDACTION_UNAVAILABLE_ERROR_CODE.to_string());
    (error_code, None)
}

fn safe_task_state(mut state: TaskState, redactor: Option<&TaggedValueRedactor>) -> TaskState {
    let Some(redactor) = redactor else {
        let (error_code, reason) =
            unavailable_completion_details(state.result.unwrap_or(TaskResult::Success));
        state.error_code = error_code;
        state.reason = reason;
        return state;
    };
    let error_code = state
        .error_code
        .as_deref()
        .map(|value| redactor.redact_str_bounded(value, 96))
        .transpose();
    let reason = state
        .reason
        .as_deref()
        .map(|value| redactor.redact_str_bounded(value, 1_024))
        .transpose();
    match (error_code, reason) {
        (Ok(error_code), Ok(reason)) => {
            state.error_code = error_code;
            state.reason = reason.map(|value| super::redact_sensitive_text(&value));
        }
        _ => {
            let (error_code, reason) =
                unavailable_completion_details(state.result.unwrap_or(TaskResult::Success));
            state.error_code = error_code;
            state.reason = reason;
        }
    }
    state
}

fn require_path_workspace(
    workspace_paths: &WorkspacePaths,
    input_workspace: &str,
) -> Result<(), TaskStateError> {
    if task::workspace_key(workspace_paths)? != input_workspace {
        return Err(TaskStateError::WrongWorkspace);
    }
    Ok(())
}

fn validate_identity(
    state: &TaskState,
    bundle_id: &TaskBundleId,
    workspace_key: &str,
    memory_revision: &TaskFingerprint,
) -> Result<(), TaskStateError> {
    if state.workspace_key != workspace_key {
        return Err(TaskStateError::WrongWorkspace);
    }
    if &state.bundle_id != bundle_id {
        return Err(TaskStateError::ForeignBundle);
    }
    if &state.memory_revision != memory_revision {
        return Err(TaskStateError::StaleRevision);
    }
    Ok(())
}

fn validate_none_relevant(
    transaction: &Transaction<'_>,
    state: &TaskState,
    input: &TaskApplyInput,
) -> Result<(), TaskStateError> {
    if !input.none_relevant {
        return Ok(());
    }
    let task_node_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM task_bundle_nodes
             WHERE task_id = ?1 AND context_kind = 'task'",
            [input.task_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    if !state.retrieval_complete || state.budget_exhausted || task_node_count != 0 {
        return Err(TaskStateError::NoneRelevantConflict);
    }
    Ok(())
}

fn validate_applied_nodes(
    bundle_nodes: &[TaskBundleNode],
    applied_nodes: &[AppliedTaskNode],
) -> Result<(), TaskStateError> {
    let mut bundle_index = 0;
    for applied in applied_nodes {
        while bundle_nodes
            .get(bundle_index)
            .is_some_and(|node| node.node_id < applied.node_id)
        {
            bundle_index += 1;
        }
        let bundle = bundle_nodes
            .get(bundle_index)
            .filter(|node| node.node_id == applied.node_id)
            .ok_or(TaskStateError::NodeOutsideBundle)?;
        if !applied.kind.matches_node_type(&bundle.node_type) {
            return Err(TaskStateError::NodeKindMismatch);
        }
    }
    Ok(())
}

fn start_matches(state: &TaskState, input: &TaskStartInput) -> bool {
    state.task_id == input.task_id
        && state.bundle_id == input.bundle_id
        && state.workspace_key == input.workspace_key
        && state.memory_revision == input.memory_revision
        && state.query_fingerprint == input.query_fingerprint
        && state.mandatory_context_complete == input.mandatory_context_complete
        && state.retrieval_complete == input.retrieval_complete
        && state.budget_exhausted == input.budget_exhausted
        && state.bundle_nodes == input.nodes
}

fn load_state(connection: &Connection, task_id: &TaskId) -> Result<TaskState, TaskStateError> {
    struct StoredTask {
        task_id: String,
        bundle_id: String,
        workspace_key: String,
        memory_revision: String,
        query_fingerprint: String,
        status: String,
        started_at: String,
        applied_at: Option<String>,
        finished_at: Option<String>,
        mandatory_context_complete: i64,
        retrieval_complete: i64,
        budget_exhausted: i64,
        none_relevant: i64,
        apply_fingerprint: Option<String>,
        completion_fingerprint: Option<String>,
        completion_result: Option<String>,
        duration_ms: Option<i64>,
        error_code: Option<String>,
        reason: Option<String>,
    }

    let stored = connection
        .query_row(
            "SELECT
                task_id, bundle_id, workspace_key, memory_revision,
                query_fingerprint, status, started_at, applied_at, finished_at,
                mandatory_context_complete, retrieval_complete, budget_exhausted,
                none_relevant, apply_fingerprint, completion_fingerprint,
                completion_result, duration_ms, error_code, reason
             FROM tasks WHERE task_id = ?1",
            [task_id.as_str()],
            |row| {
                Ok(StoredTask {
                    task_id: row.get(0)?,
                    bundle_id: row.get(1)?,
                    workspace_key: row.get(2)?,
                    memory_revision: row.get(3)?,
                    query_fingerprint: row.get(4)?,
                    status: row.get(5)?,
                    started_at: row.get(6)?,
                    applied_at: row.get(7)?,
                    finished_at: row.get(8)?,
                    mandatory_context_complete: row.get(9)?,
                    retrieval_complete: row.get(10)?,
                    budget_exhausted: row.get(11)?,
                    none_relevant: row.get(12)?,
                    apply_fingerprint: row.get(13)?,
                    completion_fingerprint: row.get(14)?,
                    completion_result: row.get(15)?,
                    duration_ms: row.get(16)?,
                    error_code: row.get(17)?,
                    reason: row.get(18)?,
                })
            },
        )
        .optional()
        .map_err(|_| TaskStateError::StoreUnavailable)?
        .ok_or(TaskStateError::NotFoundOrExpired)?;

    let bundle_nodes = load_bundle_nodes(connection, task_id)?;
    let applied_nodes = load_applied_nodes(connection, task_id)?;
    let duration_ms = stored
        .duration_ms
        .map(|value| u64::try_from(value).map_err(|_| TaskStateError::StoreUnavailable))
        .transpose()?;
    let retrieval_complete = parse_bool(stored.retrieval_complete)?;
    let budget_exhausted = parse_bool(stored.budget_exhausted)?;
    if retrieval_complete == budget_exhausted {
        return Err(TaskStateError::StoreUnavailable);
    }
    Ok(TaskState {
        task_id: TaskId::parse(&stored.task_id).map_err(|_| TaskStateError::StoreUnavailable)?,
        bundle_id: TaskBundleId::parse(&stored.bundle_id)
            .map_err(|_| TaskStateError::StoreUnavailable)?,
        workspace_key: stored.workspace_key,
        memory_revision: TaskFingerprint::parse(&stored.memory_revision)
            .map_err(|_| TaskStateError::StoreUnavailable)?,
        query_fingerprint: TaskFingerprint::parse(&stored.query_fingerprint)
            .map_err(|_| TaskStateError::StoreUnavailable)?,
        status: TaskStatus::parse(&stored.status)?,
        started_at: stored.started_at,
        applied_at: stored.applied_at,
        finished_at: stored.finished_at,
        mandatory_context_complete: parse_bool(stored.mandatory_context_complete)?,
        retrieval_complete,
        budget_exhausted,
        none_relevant: parse_bool(stored.none_relevant)?,
        result: stored
            .completion_result
            .as_deref()
            .map(TaskResult::parse)
            .transpose()?,
        duration_ms,
        error_code: stored.error_code,
        reason: stored.reason,
        bundle_nodes,
        applied_nodes,
        apply_fingerprint: stored
            .apply_fingerprint
            .as_deref()
            .map(TaskFingerprint::parse)
            .transpose()
            .map_err(|_| TaskStateError::StoreUnavailable)?,
        completion_fingerprint: stored
            .completion_fingerprint
            .as_deref()
            .map(TaskFingerprint::parse)
            .transpose()
            .map_err(|_| TaskStateError::StoreUnavailable)?,
    })
}

fn load_bundle_nodes(
    connection: &Connection,
    task_id: &TaskId,
) -> Result<Vec<TaskBundleNode>, TaskStateError> {
    let mut statement = connection
        .prepare(
            "SELECT node_id, node_type, context_kind
             FROM task_bundle_nodes WHERE task_id = ?1 ORDER BY node_id",
        )
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    let rows = statement
        .query_map([task_id.as_str()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    let mut nodes = Vec::new();
    for row in rows {
        let (node_id, node_type, context_kind) =
            row.map_err(|_| TaskStateError::StoreUnavailable)?;
        nodes.push(TaskBundleNode::new(
            node_id,
            &node_type,
            TaskContextKind::parse(&context_kind)?,
        )?);
    }
    Ok(nodes)
}

fn load_applied_nodes(
    connection: &Connection,
    task_id: &TaskId,
) -> Result<Vec<AppliedTaskNode>, TaskStateError> {
    let mut statement = connection
        .prepare(
            "SELECT node_id, application_kind
             FROM task_applied_nodes WHERE task_id = ?1 ORDER BY node_id",
        )
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    let rows = statement
        .query_map([task_id.as_str()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| TaskStateError::StoreUnavailable)?;
    let mut nodes = Vec::new();
    for row in rows {
        let (node_id, kind) = row.map_err(|_| TaskStateError::StoreUnavailable)?;
        nodes.push(AppliedTaskNode::new(
            node_id,
            AppliedNodeKind::parse(&kind)?,
        )?);
    }
    Ok(nodes)
}

fn parse_bool(value: i64) -> Result<bool, TaskStateError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(TaskStateError::StoreUnavailable),
    }
}
