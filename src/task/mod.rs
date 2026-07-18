//! Typed, fail-closed task lifecycle state.
//!
//! Authoritative task state is stored in Local Observability schema v2. The
//! best-effort event projection is intentionally a separate API.

use crate::storage::{self, WorkspacePaths};
use serde::Serialize;
use thiserror::Error;

const FINGERPRINT_BYTES: usize = 32;
// The 1 MiB mandatory and 256 KiB task JSON budgets admit more than 4,096
// minimal selected nodes. Keep state validation above that byte-bounded
// maximum while retaining a defensive count cap.
const MAX_TASK_NODES: usize = 8_192;
const MAX_REASON_BYTES: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// Parses a canonical lowercase UUID v4.
    ///
    /// # Errors
    ///
    /// Returns [`TaskStateError::InvalidUuid`] for any other representation.
    pub fn parse(value: &str) -> Result<Self, TaskStateError> {
        validate_uuid_v4(value).map(Self)
    }

    /// Allocates a canonical lowercase UUID v4.
    ///
    /// # Errors
    ///
    /// Returns [`TaskStateError::RandomSourceUnavailable`] when the operating
    /// system random source cannot be read.
    pub fn generate() -> Result<Self, TaskStateError> {
        random_uuid_v4().map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct TaskBundleId(String);

impl TaskBundleId {
    /// Parses a canonical lowercase UUID v4.
    ///
    /// # Errors
    ///
    /// Returns [`TaskStateError::InvalidUuid`] for any other representation.
    pub fn parse(value: &str) -> Result<Self, TaskStateError> {
        validate_uuid_v4(value).map(Self)
    }

    /// Allocates a canonical lowercase UUID v4.
    ///
    /// # Errors
    ///
    /// Returns [`TaskStateError::RandomSourceUnavailable`] when the operating
    /// system random source cannot be read.
    pub fn generate() -> Result<Self, TaskStateError> {
        random_uuid_v4().map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct TaskFingerprint(String);

impl TaskFingerprint {
    /// Parses the stable 32-character lowercase hexadecimal fingerprint used
    /// by recall and task replay contracts.
    ///
    /// # Errors
    ///
    /// Returns [`TaskStateError::InvalidFingerprint`] for malformed input.
    pub fn parse(value: &str) -> Result<Self, TaskStateError> {
        if value.len() != FINGERPRINT_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(TaskStateError::InvalidFingerprint);
        }
        Ok(Self(value.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn stable(bytes: &[u8]) -> Self {
        fn hash(bytes: impl Iterator<Item = u8>, seed: u64) -> u64 {
            bytes.fold(seed, |mut value, byte| {
                value ^= u64::from(byte);
                value.wrapping_mul(0x0000_0100_0000_01b3)
            })
        }

        let first = hash(bytes.iter().copied(), 0xcbf2_9ce4_8422_2325);
        let second = hash(bytes.iter().rev().copied(), 0x8422_2325_cbf2_9ce4);
        Self(format!("{first:016x}{second:016x}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Started,
    Applied,
    Completed,
    Failed,
}

impl TaskStatus {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Applied => "applied",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, TaskStateError> {
        match value {
            "started" => Ok(Self::Started),
            "applied" => Ok(Self::Applied),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(TaskStateError::StoreUnavailable),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskResult {
    Success,
    Partial,
    Failed,
}

impl TaskResult {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, TaskStateError> {
        match value {
            "success" => Ok(Self::Success),
            "partial" => Ok(Self::Partial),
            "failed" => Ok(Self::Failed),
            _ => Err(TaskStateError::StoreUnavailable),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskContextKind {
    Mandatory,
    Task,
}

impl TaskContextKind {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Mandatory => "mandatory",
            Self::Task => "task",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, TaskStateError> {
        match value {
            "mandatory" => Ok(Self::Mandatory),
            "task" => Ok(Self::Task),
            _ => Err(TaskStateError::StoreUnavailable),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppliedNodeKind {
    Gate,
    Rule,
    Workflow,
    Tool,
    Correction,
    FailureMode,
}

impl AppliedNodeKind {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Gate => "gate",
            Self::Rule => "rule",
            Self::Workflow => "workflow",
            Self::Tool => "tool",
            Self::Correction => "correction",
            Self::FailureMode => "failure_mode",
        }
    }

    #[must_use]
    pub(crate) const fn matches_node_type(self, node_type: &str) -> bool {
        match self {
            Self::Gate => matches!(node_type.as_bytes(), b"gate"),
            Self::Rule => matches!(node_type.as_bytes(), b"rule"),
            Self::Workflow => matches!(node_type.as_bytes(), b"workflow"),
            Self::Tool => matches!(node_type.as_bytes(), b"tool_contract"),
            Self::Correction => matches!(
                node_type.as_bytes(),
                b"correction" | b"lesson" | b"incident_scar"
            ),
            Self::FailureMode => matches!(node_type.as_bytes(), b"failure_mode"),
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, TaskStateError> {
        match value {
            "gate" => Ok(Self::Gate),
            "rule" => Ok(Self::Rule),
            "workflow" => Ok(Self::Workflow),
            "tool" => Ok(Self::Tool),
            "correction" => Ok(Self::Correction),
            "failure_mode" => Ok(Self::FailureMode),
            _ => Err(TaskStateError::StoreUnavailable),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskBundleNode {
    pub node_id: i64,
    pub node_type: String,
    pub context_kind: TaskContextKind,
}

impl TaskBundleNode {
    /// Builds one selected bundle node.
    ///
    /// # Errors
    ///
    /// Rejects non-positive IDs and node types outside the operational schema.
    pub fn new(
        node_id: i64,
        node_type: &str,
        context_kind: TaskContextKind,
    ) -> Result<Self, TaskStateError> {
        if node_id <= 0 {
            return Err(TaskStateError::InvalidNodeId);
        }
        if !storage::ALLOWED_NODE_TYPES.contains(&node_type) {
            return Err(TaskStateError::InvalidNodeType);
        }
        Ok(Self {
            node_id,
            node_type: node_type.to_string(),
            context_kind,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppliedTaskNode {
    pub node_id: i64,
    pub kind: AppliedNodeKind,
}

impl AppliedTaskNode {
    /// Builds one factual context-application entry.
    ///
    /// # Errors
    ///
    /// Rejects a non-positive node ID.
    pub fn new(node_id: i64, kind: AppliedNodeKind) -> Result<Self, TaskStateError> {
        if node_id <= 0 {
            return Err(TaskStateError::InvalidNodeId);
        }
        Ok(Self { node_id, kind })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStartInput {
    pub(crate) task_id: TaskId,
    pub(crate) bundle_id: TaskBundleId,
    pub(crate) workspace_key: String,
    pub(crate) memory_revision: TaskFingerprint,
    pub(crate) query_fingerprint: TaskFingerprint,
    pub(crate) mandatory_context_complete: bool,
    pub(crate) retrieval_complete: bool,
    pub(crate) budget_exhausted: bool,
    pub(crate) nodes: Vec<TaskBundleNode>,
}

impl TaskStartInput {
    /// Builds the authoritative state written before `task start` may succeed.
    ///
    /// Raw query text is deliberately absent from this API.
    ///
    /// # Errors
    ///
    /// Rejects an unsafe workspace key, incomplete mandatory context, duplicate
    /// nodes, or an unbounded bundle.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: TaskId,
        bundle_id: TaskBundleId,
        workspace_key: &str,
        memory_revision: TaskFingerprint,
        query_fingerprint: TaskFingerprint,
        mandatory_context_complete: bool,
        retrieval_complete: bool,
        budget_exhausted: bool,
        mut nodes: Vec<TaskBundleNode>,
    ) -> Result<Self, TaskStateError> {
        validate_workspace_key(workspace_key)?;
        if !mandatory_context_complete {
            return Err(TaskStateError::MandatoryContextIncomplete);
        }
        if retrieval_complete == budget_exhausted {
            return Err(TaskStateError::InvalidRetrievalState);
        }
        if nodes.len() > MAX_TASK_NODES {
            return Err(TaskStateError::TooManyNodes);
        }
        nodes.sort_by_key(|node| node.node_id);
        if nodes
            .windows(2)
            .any(|pair| pair[0].node_id == pair[1].node_id)
        {
            return Err(TaskStateError::DuplicateNode);
        }
        Ok(Self {
            task_id,
            bundle_id,
            workspace_key: workspace_key.to_string(),
            memory_revision,
            query_fingerprint,
            mandatory_context_complete,
            retrieval_complete,
            budget_exhausted,
            nodes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskApplyInput {
    pub(crate) task_id: TaskId,
    pub(crate) bundle_id: TaskBundleId,
    pub(crate) workspace_key: String,
    pub(crate) memory_revision: TaskFingerprint,
    pub(crate) replay_fingerprint: TaskFingerprint,
    pub(crate) none_relevant: bool,
    pub(crate) nodes: Vec<AppliedTaskNode>,
}

impl TaskApplyInput {
    /// Builds an authoritative `started -> applied` transition request.
    ///
    /// # Errors
    ///
    /// Rejects an unsafe workspace key, duplicate nodes, or an unbounded list.
    pub fn new(
        task_id: TaskId,
        bundle_id: TaskBundleId,
        workspace_key: &str,
        memory_revision: TaskFingerprint,
        replay_fingerprint: TaskFingerprint,
        none_relevant: bool,
        nodes: Vec<AppliedTaskNode>,
    ) -> Result<Self, TaskStateError> {
        validate_workspace_key(workspace_key)?;
        let nodes = normalize_applied_nodes(nodes)?;
        validate_application_selection(none_relevant, &nodes)?;
        Ok(Self {
            task_id,
            bundle_id,
            workspace_key: workspace_key.to_string(),
            memory_revision,
            replay_fingerprint,
            none_relevant,
            nodes,
        })
    }

    /// Builds a canonical request and derives its replay fingerprint.
    ///
    /// Sorting and fingerprinting happen in one typed boundary, so callers
    /// cannot submit arguments that disagree with the replay identity.
    ///
    /// # Errors
    ///
    /// Rejects an unsafe workspace key, duplicate nodes, or an unbounded list.
    pub fn for_request(
        task_id: TaskId,
        bundle_id: TaskBundleId,
        workspace_key: &str,
        memory_revision: TaskFingerprint,
        none_relevant: bool,
        nodes: Vec<AppliedTaskNode>,
    ) -> Result<Self, TaskStateError> {
        validate_workspace_key(workspace_key)?;
        let nodes = normalize_applied_nodes(nodes)?;
        validate_application_selection(none_relevant, &nodes)?;
        let replay_fingerprint = apply_replay_fingerprint(
            &task_id,
            &bundle_id,
            workspace_key,
            &memory_revision,
            none_relevant,
            &nodes,
        )?;
        Ok(Self {
            task_id,
            bundle_id,
            workspace_key: workspace_key.to_string(),
            memory_revision,
            replay_fingerprint,
            none_relevant,
            nodes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCompletionInput {
    pub(crate) task_id: TaskId,
    pub(crate) bundle_id: TaskBundleId,
    pub(crate) workspace_key: String,
    pub(crate) memory_revision: TaskFingerprint,
    pub(crate) replay_fingerprint: TaskFingerprint,
    pub(crate) result: TaskResult,
    pub(crate) error_code: Option<String>,
    pub(crate) reason: Option<String>,
}

impl TaskCompletionInput {
    /// Builds an authoritative terminal transition request.
    ///
    /// # Errors
    ///
    /// Failure requires a stable error code. Non-failure results reject error
    /// details. Reasons are bounded here and redacted before persistence.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: TaskId,
        bundle_id: TaskBundleId,
        workspace_key: &str,
        memory_revision: TaskFingerprint,
        replay_fingerprint: TaskFingerprint,
        result: TaskResult,
        error_code: Option<&str>,
        reason: Option<&str>,
    ) -> Result<Self, TaskStateError> {
        validate_workspace_key(workspace_key)?;
        let (error_code, reason) = normalize_completion_details(result, error_code, reason)?;
        Ok(Self {
            task_id,
            bundle_id,
            workspace_key: workspace_key.to_string(),
            memory_revision,
            replay_fingerprint,
            result,
            error_code,
            reason,
        })
    }

    /// Builds a canonical terminal request and derives its replay fingerprint.
    ///
    /// The fingerprint covers normalized reason bytes. Only the fingerprint is
    /// persisted; the reason itself is redacted by the state store.
    ///
    /// # Errors
    ///
    /// Rejects unsafe identity fields or inconsistent completion details.
    #[allow(clippy::too_many_arguments)]
    pub fn for_request(
        task_id: TaskId,
        bundle_id: TaskBundleId,
        workspace_key: &str,
        memory_revision: TaskFingerprint,
        result: TaskResult,
        error_code: Option<&str>,
        reason: Option<&str>,
    ) -> Result<Self, TaskStateError> {
        validate_workspace_key(workspace_key)?;
        let (error_code, reason) = normalize_completion_details(result, error_code, reason)?;
        let replay_fingerprint = completion_replay_fingerprint(
            &task_id,
            &bundle_id,
            workspace_key,
            &memory_revision,
            result,
            error_code.as_deref(),
            reason.as_deref(),
        )?;
        Ok(Self {
            task_id,
            bundle_id,
            workspace_key: workspace_key.to_string(),
            memory_revision,
            replay_fingerprint,
            result,
            error_code,
            reason,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskState {
    pub task_id: TaskId,
    pub bundle_id: TaskBundleId,
    pub workspace_key: String,
    pub memory_revision: TaskFingerprint,
    pub query_fingerprint: TaskFingerprint,
    pub status: TaskStatus,
    pub started_at: String,
    pub applied_at: Option<String>,
    pub finished_at: Option<String>,
    pub mandatory_context_complete: bool,
    pub retrieval_complete: bool,
    pub budget_exhausted: bool,
    pub none_relevant: bool,
    pub result: Option<TaskResult>,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
    pub reason: Option<String>,
    pub bundle_nodes: Vec<TaskBundleNode>,
    pub applied_nodes: Vec<AppliedTaskNode>,
    #[serde(skip)]
    pub(crate) apply_fingerprint: Option<TaskFingerprint>,
    #[serde(skip)]
    pub(crate) completion_fingerprint: Option<TaskFingerprint>,
}

/// Stable failures for authoritative task state operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TaskStateError {
    #[error("task id must be a canonical lowercase UUID v4")]
    InvalidUuid,
    #[error("task fingerprint must be 32 lowercase hexadecimal characters")]
    InvalidFingerprint,
    #[error("workspace key is invalid")]
    InvalidWorkspaceKey,
    #[error("task node id must be positive")]
    InvalidNodeId,
    #[error("task node type is invalid")]
    InvalidNodeType,
    #[error("task bundle contains duplicate node ids")]
    DuplicateNode,
    #[error("task bundle exceeds its bounded node count")]
    TooManyNodes,
    #[error("task application must select context or declare --none-relevant")]
    EmptyApplication,
    #[error("mandatory task context is incomplete")]
    MandatoryContextIncomplete,
    #[error("task retrieval completeness and budget facts are inconsistent")]
    InvalidRetrievalState,
    #[error("task completion details are inconsistent")]
    InvalidCompletionDetails,
    #[error("task error code is invalid")]
    InvalidErrorCode,
    #[error("task failure reason is invalid")]
    InvalidReason,
    #[error("operating system random source is unavailable")]
    RandomSourceUnavailable,
    #[error("task state was not found or expired")]
    NotFoundOrExpired,
    #[error("task belongs to another workspace")]
    WrongWorkspace,
    #[error("task bundle does not match")]
    ForeignBundle,
    #[error("task memory revision is stale")]
    StaleRevision,
    #[error("task transition is not allowed")]
    InvalidTransition,
    #[error("task replay conflicts with stored state")]
    ConflictingReplay,
    #[error("applied node is outside the task bundle")]
    NodeOutsideBundle,
    #[error("applied node does not exist in current operational memory")]
    UnknownNode,
    #[error("applied node is not active for task use")]
    InactiveNode,
    #[error("applied node kind does not match the bundle node type")]
    NodeKindMismatch,
    #[error("--none-relevant conflicts with the stored retrieval state")]
    NoneRelevantConflict,
    #[error("authoritative task state store is unavailable")]
    StoreUnavailable,
}

impl TaskStateError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidUuid => "TASK_INVALID_UUID",
            Self::InvalidFingerprint => "TASK_INVALID_FINGERPRINT",
            Self::InvalidWorkspaceKey => "TASK_INVALID_WORKSPACE",
            Self::InvalidNodeId => "TASK_INVALID_NODE_ID",
            Self::InvalidNodeType => "TASK_INVALID_NODE_TYPE",
            Self::DuplicateNode => "TASK_DUPLICATE_NODE",
            Self::TooManyNodes => "TASK_TOO_MANY_NODES",
            Self::EmptyApplication => "TASK_EMPTY_APPLICATION",
            Self::MandatoryContextIncomplete => "TASK_MANDATORY_CONTEXT_INCOMPLETE",
            Self::InvalidRetrievalState => "TASK_INVALID_RETRIEVAL_STATE",
            Self::InvalidCompletionDetails => "TASK_INVALID_COMPLETION",
            Self::InvalidErrorCode => "TASK_INVALID_ERROR_CODE",
            Self::InvalidReason => "TASK_INVALID_REASON",
            Self::RandomSourceUnavailable => "TASK_RANDOM_UNAVAILABLE",
            Self::NotFoundOrExpired => "TASK_NOT_FOUND_OR_EXPIRED",
            Self::WrongWorkspace => "TASK_WRONG_WORKSPACE",
            Self::ForeignBundle => "TASK_FOREIGN_BUNDLE",
            Self::StaleRevision => "TASK_STALE_REVISION",
            Self::InvalidTransition => "TASK_INVALID_TRANSITION",
            Self::ConflictingReplay => "TASK_CONFLICTING_REPLAY",
            Self::NodeOutsideBundle => "TASK_NODE_OUTSIDE_BUNDLE",
            Self::UnknownNode => "TASK_UNKNOWN_NODE",
            Self::InactiveNode => "TASK_NODE_INACTIVE",
            Self::NodeKindMismatch => "TASK_NODE_KIND_MISMATCH",
            Self::NoneRelevantConflict => "TASK_NONE_RELEVANT_CONFLICT",
            Self::StoreUnavailable => "TASK_STATE_STORE_UNAVAILABLE",
        }
    }
}

/// Persists the required start state before a start response may be emitted.
///
/// # Errors
///
/// Fails closed with a typed [`TaskStateError`].
pub fn record_started(
    workspace_paths: &WorkspacePaths,
    input: &TaskStartInput,
) -> Result<TaskState, TaskStateError> {
    crate::observability::task_state::record_started(workspace_paths, input)
}

/// Persists a validated context application.
///
/// # Errors
///
/// Fails closed for stale, foreign, invalid, or conflicting task state.
pub fn record_context_applied(
    workspace_paths: &WorkspacePaths,
    input: &TaskApplyInput,
) -> Result<TaskState, TaskStateError> {
    crate::observability::task_state::record_context_applied(workspace_paths, input)
}

/// Persists an immutable terminal task result.
///
/// # Errors
///
/// Fails closed for stale, foreign, invalid, or conflicting task state.
pub fn record_completed(
    workspace_paths: &WorkspacePaths,
    input: &TaskCompletionInput,
) -> Result<TaskState, TaskStateError> {
    crate::observability::task_state::record_completed(workspace_paths, input)
}

/// Reads authoritative task state without creating or migrating a store.
///
/// # Errors
///
/// Returns a typed missing/expired or store failure.
pub fn load(
    workspace_paths: &WorkspacePaths,
    task_id: &TaskId,
) -> Result<TaskState, TaskStateError> {
    crate::observability::task_state::load(workspace_paths, task_id)
}

fn validate_uuid_v4(value: &str) -> Result<String, TaskStateError> {
    let parsed = uuid::Uuid::parse_str(value).map_err(|_| TaskStateError::InvalidUuid)?;
    if parsed.get_version() != Some(uuid::Version::Random)
        || parsed.get_variant() != uuid::Variant::RFC4122
        || parsed.hyphenated().to_string() != value
    {
        return Err(TaskStateError::InvalidUuid);
    }
    Ok(value.to_string())
}

fn random_uuid_v4() -> Result<String, TaskStateError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| TaskStateError::RandomSourceUnavailable)?;
    Ok(uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .hyphenated()
        .to_string())
}

fn validate_workspace_key(value: &str) -> Result<(), TaskStateError> {
    if value.trim().is_empty() || value.len() > 255 || value.as_bytes().contains(&0) {
        return Err(TaskStateError::InvalidWorkspaceKey);
    }
    Ok(())
}

fn validate_error_code(value: &str) -> Result<&str, TaskStateError> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err(TaskStateError::InvalidErrorCode);
    }
    Ok(value)
}

fn normalize_applied_nodes(
    mut nodes: Vec<AppliedTaskNode>,
) -> Result<Vec<AppliedTaskNode>, TaskStateError> {
    if nodes.len() > MAX_TASK_NODES {
        return Err(TaskStateError::TooManyNodes);
    }
    nodes.sort_by_key(|node| node.node_id);
    if nodes
        .windows(2)
        .any(|pair| pair[0].node_id == pair[1].node_id)
    {
        return Err(TaskStateError::DuplicateNode);
    }
    Ok(nodes)
}

fn validate_application_selection(
    none_relevant: bool,
    nodes: &[AppliedTaskNode],
) -> Result<(), TaskStateError> {
    if !none_relevant && nodes.is_empty() {
        return Err(TaskStateError::EmptyApplication);
    }
    if none_relevant
        && nodes
            .iter()
            .any(|node| !matches!(node.kind, AppliedNodeKind::Gate | AppliedNodeKind::Rule))
    {
        return Err(TaskStateError::NoneRelevantConflict);
    }
    Ok(())
}

fn normalize_completion_details(
    result: TaskResult,
    error_code: Option<&str>,
    reason: Option<&str>,
) -> Result<(Option<String>, Option<String>), TaskStateError> {
    let error_code = error_code
        .map(validate_error_code)
        .transpose()?
        .map(str::to_string);
    if (result == TaskResult::Failed) != error_code.is_some() {
        return Err(TaskStateError::InvalidCompletionDetails);
    }
    if result != TaskResult::Failed && reason.is_some() {
        return Err(TaskStateError::InvalidCompletionDetails);
    }
    let reason = reason
        .map(str::trim)
        .map(|value| {
            if value.is_empty() || value.len() > MAX_REASON_BYTES || value.as_bytes().contains(&0) {
                return Err(TaskStateError::InvalidReason);
            }
            Ok(value.to_string())
        })
        .transpose()?;
    Ok((error_code, reason))
}

fn append_canonical_field(output: &mut Vec<u8>, value: &[u8]) -> Result<(), TaskStateError> {
    let length = u64::try_from(value.len()).map_err(|_| TaskStateError::TooManyNodes)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

fn apply_replay_fingerprint(
    task_id: &TaskId,
    bundle_id: &TaskBundleId,
    workspace_key: &str,
    memory_revision: &TaskFingerprint,
    none_relevant: bool,
    nodes: &[AppliedTaskNode],
) -> Result<TaskFingerprint, TaskStateError> {
    let mut canonical = Vec::new();
    append_canonical_field(&mut canonical, b"aopmem-task-apply-v1")?;
    append_canonical_field(&mut canonical, task_id.as_str().as_bytes())?;
    append_canonical_field(&mut canonical, bundle_id.as_str().as_bytes())?;
    append_canonical_field(&mut canonical, workspace_key.as_bytes())?;
    append_canonical_field(&mut canonical, memory_revision.as_str().as_bytes())?;
    append_canonical_field(&mut canonical, &[u8::from(none_relevant)])?;
    for node in nodes {
        append_canonical_field(&mut canonical, node.kind.as_str().as_bytes())?;
        append_canonical_field(&mut canonical, &node.node_id.to_be_bytes())?;
    }
    Ok(TaskFingerprint::stable(&canonical))
}

#[allow(clippy::too_many_arguments)]
fn completion_replay_fingerprint(
    task_id: &TaskId,
    bundle_id: &TaskBundleId,
    workspace_key: &str,
    memory_revision: &TaskFingerprint,
    result: TaskResult,
    error_code: Option<&str>,
    reason: Option<&str>,
) -> Result<TaskFingerprint, TaskStateError> {
    let mut canonical = Vec::new();
    append_canonical_field(&mut canonical, b"aopmem-task-complete-v1")?;
    append_canonical_field(&mut canonical, task_id.as_str().as_bytes())?;
    append_canonical_field(&mut canonical, bundle_id.as_str().as_bytes())?;
    append_canonical_field(&mut canonical, workspace_key.as_bytes())?;
    append_canonical_field(&mut canonical, memory_revision.as_str().as_bytes())?;
    append_canonical_field(&mut canonical, result.as_str().as_bytes())?;
    append_canonical_field(&mut canonical, error_code.map_or(&[][..], str::as_bytes))?;
    append_canonical_field(&mut canonical, reason.map_or(&[][..], str::as_bytes))?;
    Ok(TaskFingerprint::stable(&canonical))
}

pub(crate) fn workspace_key(workspace_paths: &WorkspacePaths) -> Result<&str, TaskStateError> {
    workspace_paths
        .root()
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(TaskStateError::InvalidWorkspaceKey)
}
