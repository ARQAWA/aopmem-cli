//! Strict, workspace-local storage for Local Observability.
//!
//! The store is deliberately separate from operational memory. This module
//! owns the version-1 schema, safe reader/writer opening rules, and the typed,
//! privacy-bounded best-effort collector. Product lifecycle wiring and
//! product-facing commands are added by later stages.

use crate::output::{OutputWarning, OBSERVABILITY_WRITE_FAILED};
use crate::storage::{self, WorkspacePaths};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;

pub(crate) mod export;
pub(crate) mod report;
pub(crate) mod ui;

pub const OBSERVABILITY_SCHEMA_VERSION: i64 = 1;
pub const OBSERVABILITY_APPLICATION_ID: i64 = 0x414F_504D;
pub const OBSERVABILITY_RETENTION_DAYS: i64 = 30;
pub const OBSERVABILITY_RETENTION_MAX_BYTES: u64 = 100_000_000;
const OBSERVABILITY_RETENTION_BATCH_SIZE: usize = 256;

#[derive(Debug, Clone, Copy)]
struct RetentionPolicy {
    max_age_days: i64,
    max_bytes: u64,
    batch_size: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_days: OBSERVABILITY_RETENTION_DAYS,
            max_bytes: OBSERVABILITY_RETENTION_MAX_BYTES,
            batch_size: OBSERVABILITY_RETENTION_BATCH_SIZE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    InstallStarted,
    InstallCompleted,
    InstallFailed,
    UpdateStarted,
    UpdateCompleted,
    UpdateFailed,
    WorkspaceInit,
    AdapterSeed,
    AdapterSync,
    AdapterDrift,
    RecallStarted,
    RecallCompleted,
    RecallFailed,
    RecallContinuation,
    RecallEmpty,
    RecallTruncated,
    RecallMandatoryOverflow,
    NodeCreated,
    NodeUpdated,
    NodeDeprecated,
    LinkCreated,
    Remember,
    TeachStarted,
    TeachProposed,
    TeachApplied,
    ReflectionInventory,
    ReflectionProposal,
    ReflectionApplied,
    ToolValidation,
    ToolRunStarted,
    ToolRunCompleted,
    ToolRunFailed,
    ToolRunTimeout,
    ToolOutputArtifact,
    McpStatus,
    Doctor,
    Verify,
    AuditSnapshotCompleted,
    AuditSnapshotPending,
    AuditSnapshotFailed,
    ArtifactsCleanup,
    FeedbackRecorded,
}

impl EventType {
    pub const ALL: [Self; 42] = [
        Self::InstallStarted,
        Self::InstallCompleted,
        Self::InstallFailed,
        Self::UpdateStarted,
        Self::UpdateCompleted,
        Self::UpdateFailed,
        Self::WorkspaceInit,
        Self::AdapterSeed,
        Self::AdapterSync,
        Self::AdapterDrift,
        Self::RecallStarted,
        Self::RecallCompleted,
        Self::RecallFailed,
        Self::RecallContinuation,
        Self::RecallEmpty,
        Self::RecallTruncated,
        Self::RecallMandatoryOverflow,
        Self::NodeCreated,
        Self::NodeUpdated,
        Self::NodeDeprecated,
        Self::LinkCreated,
        Self::Remember,
        Self::TeachStarted,
        Self::TeachProposed,
        Self::TeachApplied,
        Self::ReflectionInventory,
        Self::ReflectionProposal,
        Self::ReflectionApplied,
        Self::ToolValidation,
        Self::ToolRunStarted,
        Self::ToolRunCompleted,
        Self::ToolRunFailed,
        Self::ToolRunTimeout,
        Self::ToolOutputArtifact,
        Self::McpStatus,
        Self::Doctor,
        Self::Verify,
        Self::AuditSnapshotCompleted,
        Self::AuditSnapshotPending,
        Self::AuditSnapshotFailed,
        Self::ArtifactsCleanup,
        Self::FeedbackRecorded,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstallStarted => "install.started",
            Self::InstallCompleted => "install.completed",
            Self::InstallFailed => "install.failed",
            Self::UpdateStarted => "update.started",
            Self::UpdateCompleted => "update.completed",
            Self::UpdateFailed => "update.failed",
            Self::WorkspaceInit => "workspace.init",
            Self::AdapterSeed => "adapter.seed",
            Self::AdapterSync => "adapter.sync",
            Self::AdapterDrift => "adapter.drift",
            Self::RecallStarted => "recall.started",
            Self::RecallCompleted => "recall.completed",
            Self::RecallFailed => "recall.failed",
            Self::RecallContinuation => "recall.continuation",
            Self::RecallEmpty => "recall.empty",
            Self::RecallTruncated => "recall.truncated",
            Self::RecallMandatoryOverflow => "recall.mandatory_overflow",
            Self::NodeCreated => "node.created",
            Self::NodeUpdated => "node.updated",
            Self::NodeDeprecated => "node.deprecated",
            Self::LinkCreated => "link.created",
            Self::Remember => "remember",
            Self::TeachStarted => "teach.started",
            Self::TeachProposed => "teach.proposed",
            Self::TeachApplied => "teach.applied",
            Self::ReflectionInventory => "reflection.inventory",
            Self::ReflectionProposal => "reflection.proposal",
            Self::ReflectionApplied => "reflection.applied",
            Self::ToolValidation => "tool.validation",
            Self::ToolRunStarted => "tool.run.started",
            Self::ToolRunCompleted => "tool.run.completed",
            Self::ToolRunFailed => "tool.run.failed",
            Self::ToolRunTimeout => "tool.run.timeout",
            Self::ToolOutputArtifact => "tool.output.artifact",
            Self::McpStatus => "mcp.status",
            Self::Doctor => "doctor",
            Self::Verify => "verify",
            Self::AuditSnapshotCompleted => "audit.snapshot.completed",
            Self::AuditSnapshotPending => "audit.snapshot.pending",
            Self::AuditSnapshotFailed => "audit.snapshot.failed",
            Self::ArtifactsCleanup => "artifacts.cleanup",
            Self::FeedbackRecorded => "feedback.recorded",
        }
    }

    #[must_use]
    const fn is_recall(self) -> bool {
        matches!(
            self,
            Self::RecallStarted
                | Self::RecallCompleted
                | Self::RecallFailed
                | Self::RecallContinuation
                | Self::RecallEmpty
                | Self::RecallTruncated
                | Self::RecallMandatoryOverflow
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    Started,
    Success,
    Failure,
    Warning,
    Empty,
    Truncated,
    Overflow,
    Pending,
    Blocked,
    Timeout,
    Recorded,
    Proposed,
    Applied,
    Drafted,
    Missing,
    Configured,
    ConfiguredUnverified,
}

impl EventOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Warning => "warning",
            Self::Empty => "empty",
            Self::Truncated => "truncated",
            Self::Overflow => "overflow",
            Self::Pending => "pending",
            Self::Blocked => "blocked",
            Self::Timeout => "timeout",
            Self::Recorded => "recorded",
            Self::Proposed => "proposed",
            Self::Applied => "applied",
            Self::Drafted => "drafted",
            Self::Missing => "missing",
            Self::Configured => "configured",
            Self::ConfiguredUnverified => "configured_unverified",
        }
    }
}

const MAX_RAW_TEXT_BYTES: usize = 65_536;
const MAX_EVENT_PAYLOAD_BYTES: usize = 16_384;
const MAX_SELECTION_REASONS_BYTES: usize = 4_096;
const MAX_MANAGED_WORKSPACE_KEY_BYTES: usize = 255;
const REDACTED: &str = "[REDACTED]";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
struct SafeText(String);

impl SafeText {
    fn new(
        field: &'static str,
        value: &str,
        maximum_bytes: usize,
    ) -> Result<Self, CollectorInputError> {
        if value.as_bytes().contains(&0) {
            return Err(CollectorInputError::Nul { field });
        }
        if value.len() > MAX_RAW_TEXT_BYTES {
            return Err(CollectorInputError::TextTooLarge { field });
        }
        let redacted = redact_sensitive_text(value);
        Ok(Self(truncate_utf8(redacted, maximum_bytes)))
    }

    fn product_id(
        field: &'static str,
        value: &str,
        maximum_bytes: usize,
    ) -> Result<Self, CollectorInputError> {
        if value.trim().is_empty() {
            return Err(CollectorInputError::Blank { field });
        }
        if value.len() > maximum_bytes {
            return Err(CollectorInputError::TextTooLarge { field });
        }
        Self::new(field, value, maximum_bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodePayload {
    node_id: i64,
    node_type: String,
    title: SafeText,
    summary: Option<SafeText>,
    source_ref: Option<SafeText>,
}

impl NodePayload {
    pub fn new(
        node_id: i64,
        node_type: &str,
        title: &str,
        summary: Option<&str>,
        source_ref: Option<&str>,
    ) -> Result<Self, CollectorInputError> {
        validate_positive_id("node_id", node_id)?;
        validate_ascii_identifier("node_type", node_type, 64)?;
        Ok(Self {
            node_id,
            node_type: node_type.to_string(),
            title: SafeText::new("node_title", title, 512)?,
            summary: summary
                .map(|value| SafeText::new("bounded_summary", value, 2_048))
                .transpose()?,
            source_ref: source_ref
                .map(|value| SafeText::new("source_ref", value, 2_048))
                .transpose()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkPayload {
    source_node_id: i64,
    target_node_id: i64,
    link_type: SafeText,
}

impl LinkPayload {
    pub fn new(
        source_node_id: i64,
        target_node_id: i64,
        link_type: &str,
    ) -> Result<Self, CollectorInputError> {
        validate_positive_id("source_node_id", source_node_id)?;
        validate_positive_id("target_node_id", target_node_id)?;
        Ok(Self {
            source_node_id,
            target_node_id,
            link_type: SafeText::product_id("link_type", link_type, storage::MAX_LINK_TYPE_BYTES)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecallScore {
    node_id: i64,
    score: f64,
}

impl RecallScore {
    pub fn new(node_id: i64, score: f64) -> Result<Self, CollectorInputError> {
        validate_positive_id("score_node_id", node_id)?;
        if !score.is_finite() {
            return Err(CollectorInputError::InvalidScore);
        }
        Ok(Self { node_id, score })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecallPayload {
    node_count: u64,
    more_results: bool,
    continuation_count: u64,
    fts_fallback_used: bool,
    graph_traversal_used: bool,
    selected_node_ids: Vec<i64>,
    selection_reasons: Vec<SelectionReason>,
    scores: Vec<RecallScore>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionReason {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallBundleOutcome {
    Success,
    Failure,
}

impl RecallBundleOutcome {
    #[must_use]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

/// Privacy-bounded first-seen node metadata for one task recall bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct RecallBundleNode {
    node_id: i64,
    node_type: String,
    node_title: SafeText,
    bounded_summary: Option<SafeText>,
    source_ref: Option<SafeText>,
    trust_level: Option<SafeText>,
    confidence: Option<f64>,
    score: Option<f64>,
    selection_reasons: Vec<SelectionReason>,
}

impl RecallBundleNode {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node_id: i64,
        node_type: &str,
        node_title: &str,
        bounded_summary: Option<&str>,
        source_ref: Option<&str>,
        trust_level: Option<&str>,
        confidence: Option<f64>,
        score: Option<f64>,
        selection_reasons: Vec<SelectionReason>,
    ) -> Result<Self, CollectorInputError> {
        validate_positive_id("node_id", node_id)?;
        validate_ascii_identifier("node_type", node_type, 64)?;
        if node_title.trim().is_empty() {
            return Err(CollectorInputError::Blank {
                field: "node_title",
            });
        }
        if confidence.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
            return Err(CollectorInputError::InvalidConfidence);
        }
        if score.is_some_and(|value| !value.is_finite()) {
            return Err(CollectorInputError::InvalidScore);
        }
        if selection_reasons.is_empty() {
            return Err(CollectorInputError::Blank {
                field: "selection_reasons",
            });
        }
        let mut unique_reasons = Vec::with_capacity(selection_reasons.len());
        for reason in selection_reasons {
            if !unique_reasons.contains(&reason) {
                unique_reasons.push(reason);
            }
        }
        if unique_reasons.len() > 64 {
            return Err(CollectorInputError::TooManyItems {
                field: "selection_reasons",
            });
        }
        if serde_json::to_vec(&unique_reasons)
            .map_err(|_| CollectorInputError::Serialization)?
            .len()
            > MAX_SELECTION_REASONS_BYTES
        {
            return Err(CollectorInputError::SelectionReasonsTooLarge);
        }

        Ok(Self {
            node_id,
            node_type: node_type.to_string(),
            node_title: SafeText::new("node_title", node_title, 512)?,
            bounded_summary: bounded_summary
                .map(|value| SafeText::new("bounded_summary", value, 2_048))
                .transpose()?,
            source_ref: source_ref
                .map(|value| SafeText::new("source_ref", value, 2_048))
                .transpose()?,
            trust_level: trust_level
                .map(|value| SafeText::product_id("trust_level", value, 256))
                .transpose()?,
            confidence,
            score,
            selection_reasons: unique_reasons,
        })
    }
}

/// One logical recall bundle update. Nodes are present only for task recall.
#[derive(Debug, Clone, PartialEq)]
pub struct RecallBundleRecord {
    bundle_id: String,
    outcome: RecallBundleOutcome,
    error_code: Option<String>,
    duration_ms: u64,
    more_results: Option<bool>,
    incoming_continuation: bool,
    nodes: Vec<RecallBundleNode>,
}

impl RecallBundleRecord {
    pub fn success(
        bundle_id: &str,
        duration_ms: u64,
        more_results: bool,
        incoming_continuation: bool,
        nodes: Vec<RecallBundleNode>,
    ) -> Result<Self, CollectorInputError> {
        Self::new(
            bundle_id,
            RecallBundleOutcome::Success,
            None,
            duration_ms,
            Some(more_results),
            incoming_continuation,
            nodes,
        )
    }

    pub fn failure(
        bundle_id: &str,
        duration_ms: u64,
        error_code: &str,
        incoming_continuation: bool,
    ) -> Result<Self, CollectorInputError> {
        Self::new(
            bundle_id,
            RecallBundleOutcome::Failure,
            Some(error_code),
            duration_ms,
            None,
            incoming_continuation,
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        bundle_id: &str,
        outcome: RecallBundleOutcome,
        error_code: Option<&str>,
        duration_ms: u64,
        more_results: Option<bool>,
        incoming_continuation: bool,
        nodes: Vec<RecallBundleNode>,
    ) -> Result<Self, CollectorInputError> {
        let bundle_id = validate_uuid_v4(bundle_id)?;
        i64::try_from(duration_ms).map_err(|_| CollectorInputError::DurationTooLarge)?;
        let error_code = error_code
            .map(|value| {
                validate_ascii_identifier("error_code", value, 96)?;
                Ok(value.to_string())
            })
            .transpose()?;
        Ok(Self {
            bundle_id,
            outcome,
            error_code,
            duration_ms,
            more_results,
            incoming_continuation,
            nodes,
        })
    }

    #[must_use]
    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackOutcome {
    Useful,
    Partial,
    Wrong,
}

impl FeedbackOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::Partial => "partial",
            Self::Wrong => "wrong",
        }
    }
}

impl FromStr for FeedbackOutcome {
    type Err = CollectorInputError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "useful" => Ok(Self::Useful),
            "partial" => Ok(Self::Partial),
            "wrong" => Ok(Self::Wrong),
            _ => Err(CollectorInputError::InvalidFeedbackOutcome),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackRecordInput {
    bundle_id: String,
    outcome: FeedbackOutcome,
    reason: Option<SafeText>,
}

impl FeedbackRecordInput {
    pub fn new(
        bundle_id: &str,
        outcome: FeedbackOutcome,
        reason: Option<&str>,
    ) -> Result<Self, CollectorInputError> {
        let reason = reason
            .map(str::trim)
            .map(|value| {
                if value.is_empty() {
                    return Err(CollectorInputError::Blank { field: "reason" });
                }
                if value.len() > 1_024 {
                    return Err(CollectorInputError::TextTooLarge { field: "reason" });
                }
                SafeText::new("reason", value, 1_024)
            })
            .transpose()?;
        Ok(Self {
            bundle_id: validate_uuid_v4(bundle_id)?,
            outcome,
            reason,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedbackReceipt {
    pub feedback_id: String,
    pub bundle_id: String,
    pub outcome: FeedbackOutcome,
    pub reason_recorded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackWriteOutcome {
    pub receipt: FeedbackReceipt,
    pub warning: Option<OutputWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FeedbackWriteError {
    #[error("recall bundle was not found in the local observability store")]
    BundleNotFound,
    #[error("local observability feedback write failed")]
    StoreUnavailable,
}

impl RecallPayload {
    pub fn new(
        node_count: u64,
        more_results: bool,
        continuation_count: u64,
        fts_fallback_used: bool,
        graph_traversal_used: bool,
    ) -> Self {
        Self {
            node_count,
            more_results,
            continuation_count,
            fts_fallback_used,
            graph_traversal_used,
            selected_node_ids: Vec::new(),
            selection_reasons: Vec::new(),
            scores: Vec::new(),
        }
    }

    pub fn with_selected_node_ids(
        mut self,
        node_ids: Vec<i64>,
    ) -> Result<Self, CollectorInputError> {
        if node_ids.len() > 128 {
            return Err(CollectorInputError::TooManyItems {
                field: "selected_node_ids",
            });
        }
        for node_id in &node_ids {
            validate_positive_id("selected_node_id", *node_id)?;
        }
        self.selected_node_ids = node_ids;
        Ok(self)
    }

    pub fn with_selection_reasons(
        mut self,
        reasons: Vec<SelectionReason>,
    ) -> Result<Self, CollectorInputError> {
        if reasons.len() > 128 {
            return Err(CollectorInputError::TooManyItems {
                field: "selection_reasons",
            });
        }
        let bytes = serde_json::to_vec(&reasons)
            .map_err(|_| CollectorInputError::Serialization)?
            .len();
        if bytes > MAX_SELECTION_REASONS_BYTES {
            return Err(CollectorInputError::SelectionReasonsTooLarge);
        }
        self.selection_reasons = reasons;
        Ok(self)
    }

    pub fn with_scores(mut self, scores: Vec<RecallScore>) -> Result<Self, CollectorInputError> {
        if scores.len() > 128 {
            return Err(CollectorInputError::TooManyItems { field: "scores" });
        }
        self.scores = scores;
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolPayload {
    tool_id: SafeText,
    approval_present: bool,
}

impl ToolPayload {
    pub fn new(tool_id: &str, approval_present: bool) -> Result<Self, CollectorInputError> {
        Ok(Self {
            tool_id: SafeText::product_id("tool_id", tool_id, crate::tools::MAX_TOOL_ID_BYTES)?,
            approval_present,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpPayload {
    mcp_id: SafeText,
    status: McpStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpStatus {
    Installed,
    Missing,
    ConfiguredUnverified,
}

impl McpPayload {
    pub fn new(mcp_id: &str, status: McpStatus) -> Result<Self, CollectorInputError> {
        Ok(Self {
            mcp_id: SafeText::product_id("mcp_id", mcp_id, storage::MAX_MCP_ID_BYTES)?,
            status,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactPayload {
    path: SafeText,
    bytes: u64,
}

impl ArtifactPayload {
    pub fn new(path: &str, bytes: u64) -> Result<Self, CollectorInputError> {
        validate_workspace_relative_path(path)?;
        Ok(Self {
            path: SafeText::new("artifact_path", path, 2_048)?,
            bytes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CountItem {
    name: String,
    count: u64,
}

impl CountItem {
    pub fn new(name: &str, count: u64) -> Result<Self, CollectorInputError> {
        validate_ascii_identifier("count_name", name, 64)?;
        Ok(Self {
            name: name.to_string(),
            count,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CountsPayload {
    items: Vec<CountItem>,
}

impl CountsPayload {
    pub fn new(items: Vec<CountItem>) -> Result<Self, CollectorInputError> {
        if items.len() > 64 {
            return Err(CollectorInputError::TooManyItems {
                field: "count_items",
            });
        }
        Ok(Self { items })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum EventPayload {
    Empty,
    Node(NodePayload),
    Link(LinkPayload),
    Recall(RecallPayload),
    Tool(ToolPayload),
    Mcp(McpPayload),
    Artifact(ArtifactPayload),
    Counts(CountsPayload),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CollectorEvent {
    event_type: EventType,
    outcome: EventOutcome,
    payload: EventPayload,
    duration_ms: Option<u64>,
    bundle_id: Option<String>,
    error_code: Option<String>,
}

impl CollectorEvent {
    pub fn new(
        event_type: EventType,
        outcome: EventOutcome,
        payload: EventPayload,
    ) -> Result<Self, CollectorInputError> {
        let event = Self {
            event_type,
            outcome,
            payload,
            duration_ms: None,
            bundle_id: None,
            error_code: None,
        };
        event.validate_payload_size()?;
        Ok(event)
    }

    pub fn with_duration_ms(mut self, duration_ms: u64) -> Result<Self, CollectorInputError> {
        i64::try_from(duration_ms).map_err(|_| CollectorInputError::DurationTooLarge)?;
        self.duration_ms = Some(duration_ms);
        Ok(self)
    }

    pub fn with_bundle_id(mut self, bundle_id: &str) -> Result<Self, CollectorInputError> {
        self.bundle_id = Some(validate_uuid_v4(bundle_id)?);
        Ok(self)
    }

    pub fn with_error_code(mut self, error_code: &str) -> Result<Self, CollectorInputError> {
        validate_ascii_identifier("error_code", error_code, 96)?;
        self.error_code = Some(error_code.to_string());
        Ok(self)
    }

    fn validate_payload_size(&self) -> Result<(), CollectorInputError> {
        let size = serde_json::to_vec(&self.payload)
            .map_err(|_| CollectorInputError::Serialization)?
            .len();
        if size > MAX_EVENT_PAYLOAD_BYTES {
            return Err(CollectorInputError::PayloadTooLarge);
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CollectorInputError {
    #[error("{field} must be a non-empty ASCII identifier")]
    InvalidIdentifier { field: &'static str },
    #[error("{field} must not be blank")]
    Blank { field: &'static str },
    #[error("{field} contains a NUL byte")]
    Nul { field: &'static str },
    #[error("{field} exceeds the bounded collector input size")]
    TextTooLarge { field: &'static str },
    #[error("{field} must be a positive id")]
    InvalidId { field: &'static str },
    #[error("UUID must be canonical lowercase version 4")]
    InvalidUuid,
    #[error("artifact path must be a safe workspace-relative path")]
    InvalidWorkspaceRelativePath,
    #[error("duration exceeds the SQLite integer range")]
    DurationTooLarge,
    #[error("recall score must be finite")]
    InvalidScore,
    #[error("recall confidence must be finite and between zero and one")]
    InvalidConfidence,
    #[error("feedback outcome must be useful, partial, or wrong")]
    InvalidFeedbackOutcome,
    #[error("{field} has too many items")]
    TooManyItems { field: &'static str },
    #[error("selection reasons exceed 4096 JSON bytes")]
    SelectionReasonsTooLarge,
    #[error("event payload exceeds 16384 JSON bytes")]
    PayloadTooLarge,
    #[error("typed event serialization failed")]
    Serialization,
    #[error("workspace path has no UTF-8 workspace key")]
    MissingWorkspaceKey,
    #[error("operating system random source is unavailable")]
    RandomSourceUnavailable,
}

fn random_uuid_v4() -> Result<String, CollectorInputError> {
    random_uuid_v4_with(|bytes| getrandom::fill(bytes))
}

fn random_uuid_v4_with<E>(
    fill: impl FnOnce(&mut [u8; 16]) -> Result<(), E>,
) -> Result<String, CollectorInputError> {
    let mut bytes = [0_u8; 16];
    fill(&mut bytes).map_err(|_| CollectorInputError::RandomSourceUnavailable)?;
    Ok(uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .hyphenated()
        .to_string())
}

pub struct LocalCollector {
    workspace_paths: WorkspacePaths,
    workspace_key: String,
    command: String,
    correlation_id: String,
    writer: Option<ObservabilityWriter>,
    disabled: bool,
    warning_emitted: bool,
    retention_policy: RetentionPolicy,
}

impl LocalCollector {
    pub fn new(
        workspace_paths: &WorkspacePaths,
        command: impl Into<String>,
    ) -> Result<Self, CollectorInputError> {
        let command = command.into();
        validate_ascii_identifier("command", &command, 128)?;
        let workspace_key = workspace_paths
            .root()
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(CollectorInputError::MissingWorkspaceKey)?
            .to_string();
        validate_ascii_identifier(
            "workspace_key",
            &workspace_key,
            MAX_MANAGED_WORKSPACE_KEY_BYTES,
        )?;

        Ok(Self {
            workspace_paths: workspace_paths.clone(),
            workspace_key,
            command,
            correlation_id: random_uuid_v4()?,
            writer: None,
            disabled: false,
            warning_emitted: false,
            retention_policy: RetentionPolicy::default(),
        })
    }

    #[must_use]
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    pub fn record(&mut self, event: &CollectorEvent) -> Option<OutputWarning> {
        if self.disabled {
            return None;
        }
        let event_id = match random_uuid_v4() {
            Ok(event_id) => event_id,
            Err(_) => return self.disable_with_warning(),
        };
        if self.writer.is_none() {
            match open_writer(&self.workspace_paths) {
                Ok(writer) => self.writer = Some(writer),
                Err(_) => return self.disable_with_warning(),
            }
        }

        let Some(writer) = self.writer.as_mut() else {
            return self.disable_with_warning();
        };
        let write_and_retention = writer
            .insert_event(
                &event_id,
                &self.workspace_key,
                &self.command,
                &self.correlation_id,
                event,
            )
            .and_then(|()| writer.apply_retention(self.retention_policy));
        if write_and_retention.is_err() {
            return self.disable_with_warning();
        }
        None
    }

    /// Atomically publishes one logical recall update and all lifecycle rows.
    ///
    /// The caller must release every operational-memory handle before calling
    /// this method. Collector failures remain best effort and return one
    /// warning without changing the recall result.
    pub fn record_recall_bundle(
        &mut self,
        record: &RecallBundleRecord,
        events: &[CollectorEvent],
    ) -> Option<OutputWarning> {
        if self.disabled {
            return None;
        }
        if self.writer.is_none() {
            match open_writer(&self.workspace_paths) {
                Ok(writer) => self.writer = Some(writer),
                Err(_) => return self.disable_with_warning(),
            }
        }
        let Some(writer) = self.writer.as_mut() else {
            return self.disable_with_warning();
        };
        let result = writer
            .insert_recall_bundle(
                &self.workspace_key,
                &self.command,
                &self.correlation_id,
                record,
                events,
            )
            .and_then(|()| writer.apply_retention(self.retention_policy));
        if result.is_err() {
            return self.disable_with_warning();
        }
        None
    }

    /// Records explicit user/agent feedback only in an existing store.
    ///
    /// Missing stores and parents are not created. A post-commit retention
    /// failure returns a durable receipt plus the standard collector warning.
    pub fn record_feedback(
        &mut self,
        input: FeedbackRecordInput,
    ) -> Result<FeedbackWriteOutcome, FeedbackWriteError> {
        if self.disabled {
            return Err(FeedbackWriteError::StoreUnavailable);
        }
        if self.writer.is_none() {
            self.writer = Some(match open_existing_writer(&self.workspace_paths) {
                Ok(writer) => writer,
                Err(ObservabilityOpenError::Missing(_)) => {
                    return Err(FeedbackWriteError::BundleNotFound)
                }
                Err(_) => return Err(FeedbackWriteError::StoreUnavailable),
            });
        }
        let feedback_id = random_uuid_v4().map_err(|_| FeedbackWriteError::StoreUnavailable)?;
        let event_id = random_uuid_v4().map_err(|_| FeedbackWriteError::StoreUnavailable)?;
        let event = CollectorEvent::new(
            EventType::FeedbackRecorded,
            EventOutcome::Recorded,
            EventPayload::Empty,
        )
        .and_then(|event| event.with_bundle_id(&input.bundle_id))
        .map_err(|_| FeedbackWriteError::StoreUnavailable)?;
        let Some(writer) = self.writer.as_mut() else {
            return Err(FeedbackWriteError::StoreUnavailable);
        };
        let receipt = match writer.insert_feedback(
            &feedback_id,
            &event_id,
            &self.workspace_key,
            &self.command,
            &self.correlation_id,
            &input,
            &event,
        ) {
            Ok(receipt) => receipt,
            Err(FeedbackPersistError::BundleNotFound) => {
                return Err(FeedbackWriteError::BundleNotFound)
            }
            Err(FeedbackPersistError::Store(_)) => {
                self.disabled = true;
                self.writer = None;
                return Err(FeedbackWriteError::StoreUnavailable);
            }
        };
        let warning = if writer.apply_retention(self.retention_policy).is_err() {
            self.disable_with_warning()
        } else {
            None
        };
        Ok(FeedbackWriteOutcome { receipt, warning })
    }

    pub fn record_result(
        &mut self,
        event: Result<CollectorEvent, CollectorInputError>,
    ) -> Option<OutputWarning> {
        match event {
            Ok(event) => self.record(&event),
            Err(_) => self.disable_with_warning(),
        }
    }

    fn disable_with_warning(&mut self) -> Option<OutputWarning> {
        self.disabled = true;
        self.writer = None;
        if self.warning_emitted {
            return None;
        }
        self.warning_emitted = true;
        Some(OutputWarning {
            code: OBSERVABILITY_WRITE_FAILED,
            message: "local observability write failed; core command result is unchanged"
                .to_string(),
        })
    }

    #[cfg(test)]
    fn set_retention_policy(&mut self, policy: RetentionPolicy) {
        self.retention_policy = policy;
    }
}

fn validate_ascii_identifier(
    field: &'static str,
    value: &str,
    maximum_bytes: usize,
) -> Result<(), CollectorInputError> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(CollectorInputError::InvalidIdentifier { field });
    }
    Ok(())
}

fn validate_positive_id(field: &'static str, value: i64) -> Result<(), CollectorInputError> {
    if value <= 0 {
        return Err(CollectorInputError::InvalidId { field });
    }
    Ok(())
}

fn validate_workspace_relative_path(value: &str) -> Result<(), CollectorInputError> {
    if value.is_empty()
        || value.starts_with(['/', '\\'])
        || value.contains(['\\', ':', '\0'])
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(CollectorInputError::InvalidWorkspaceRelativePath);
    }
    Ok(())
}

fn validate_uuid_v4(value: &str) -> Result<String, CollectorInputError> {
    use uuid::{Variant, Version};

    let parsed = uuid::Uuid::parse_str(value).map_err(|_| CollectorInputError::InvalidUuid)?;
    let canonical = parsed.hyphenated().to_string();
    if value != canonical
        || parsed.get_version() != Some(Version::Random)
        || parsed.get_variant() != Variant::RFC4122
    {
        return Err(CollectorInputError::InvalidUuid);
    }
    Ok(canonical)
}

fn redact_sensitive_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut inside_private_key = false;
    for line in value.split_inclusive('\n') {
        let (content, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |content| (content, "\n"));
        let lower = content.to_ascii_lowercase();
        let begins_private_key =
            lower.contains("-----begin ") && lower.contains("private key-----");
        let ends_private_key = lower.contains("-----end ") && lower.contains("private key-----");
        if inside_private_key || begins_private_key {
            output.push_str(REDACTED);
            inside_private_key = !ends_private_key;
        } else {
            output.push_str(&redact_sensitive_line(content));
        }
        output.push_str(newline);
    }
    output
}

fn redact_sensitive_line(line: &str) -> String {
    let trimmed = line.trim_start();
    let indentation_bytes = line.len() - trimmed.len();
    let lower = trimmed.to_ascii_lowercase();
    for header in [
        "authorization:",
        "proxy-authorization:",
        "cookie:",
        "set-cookie:",
    ] {
        if lower.starts_with(header) {
            let mut redacted = line[..indentation_bytes].to_string();
            redacted.push_str(&trimmed[..header.len()]);
            redacted.push(' ');
            redacted.push_str(REDACTED);
            return redacted;
        }
    }

    let lower = line.to_ascii_lowercase();
    let mut ranges = Vec::new();
    collect_assignment_redactions(line, &mut ranges);
    collect_flag_redactions(line, &mut ranges);
    collect_bearer_redactions(line, &lower, &mut ranges);
    collect_uri_userinfo_redactions(line, &mut ranges);
    let redacted = apply_redaction_ranges(line, ranges);
    redact_secret_like_tokens(&redacted)
}

fn collect_assignment_redactions(value: &str, ranges: &mut Vec<(usize, usize)>) {
    let bytes = value.as_bytes();
    for (separator, byte) in bytes.iter().enumerate() {
        if !matches!(byte, b'=' | b':') {
            continue;
        }
        let Some(normalized_key) = normalized_assignment_key_before(bytes, separator) else {
            continue;
        };
        if !is_sensitive_assignment_key(&normalized_key) {
            continue;
        }

        let mut cursor = separator + 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let (secret_start, secret_end) = quoted_or_unquoted_value_range(
            bytes,
            cursor,
            normalized_key.contains("authorization")
                || normalized_key.contains("cookie")
                || normalized_key.contains("bearer"),
        );
        if secret_end > secret_start {
            ranges.push((secret_start, secret_end));
        }
    }
}

fn normalized_assignment_key_before(bytes: &[u8], separator: usize) -> Option<String> {
    let mut end = separator;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end >= 2 && bytes[end - 2] == b'\\' && matches!(bytes[end - 1], b'\'' | b'"') {
        end -= 2;
    } else if end > 0 && matches!(bytes[end - 1], b'\'' | b'"') {
        end -= 1;
    }

    let key_end = end;
    while end > 0
        && (bytes[end - 1].is_ascii_alphanumeric() || matches!(bytes[end - 1], b'_' | b'-'))
    {
        end -= 1;
    }
    if end == key_end {
        return None;
    }
    Some(
        bytes[end..key_end]
            .iter()
            .filter(|byte| byte.is_ascii_alphanumeric())
            .map(|byte| byte.to_ascii_lowercase() as char)
            .collect(),
    )
}

fn is_sensitive_assignment_key(normalized_key: &str) -> bool {
    if matches!(normalized_key, "tokenizer" | "secretary") {
        return false;
    }
    [
        "token",
        "secret",
        "cookie",
        "password",
        "passwd",
        "authorization",
        "apikey",
        "accesskey",
        "privatekey",
        "credential",
        "bearer",
    ]
    .iter()
    .any(|marker| normalized_key.contains(marker))
}

fn collect_flag_redactions(value: &str, ranges: &mut Vec<(usize, usize)>) {
    let bytes = value.as_bytes();
    let mut cursor = 0;
    while cursor + 2 < bytes.len() {
        if bytes[cursor] != b'-' || bytes[cursor + 1] != b'-' {
            cursor += 1;
            continue;
        }
        let key_start = cursor + 2;
        let mut key_end = key_start;
        while key_end < bytes.len()
            && (bytes[key_end].is_ascii_alphanumeric() || matches!(bytes[key_end], b'_' | b'-'))
        {
            key_end += 1;
        }
        if key_end == key_start {
            cursor += 2;
            continue;
        }
        let normalized_key: String = bytes[key_start..key_end]
            .iter()
            .filter(|byte| byte.is_ascii_alphanumeric())
            .map(|byte| byte.to_ascii_lowercase() as char)
            .collect();
        if !is_sensitive_assignment_key(&normalized_key)
            || key_end >= bytes.len()
            || !bytes[key_end].is_ascii_whitespace()
        {
            cursor = key_end;
            continue;
        }
        let mut value_start = key_end;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        if value_start >= bytes.len()
            || (value_start + 1 < bytes.len()
                && bytes[value_start] == b'-'
                && bytes[value_start + 1] == b'-')
        {
            cursor = key_end;
            continue;
        }
        let range = quoted_or_unquoted_value_range(
            bytes,
            value_start,
            normalized_key.contains("authorization")
                || normalized_key.contains("cookie")
                || normalized_key.contains("bearer"),
        );
        if range.1 > range.0 {
            ranges.push(range);
            cursor = range.1;
        } else {
            cursor = key_end;
        }
    }
}

fn collect_bearer_redactions(value: &str, lower: &str, ranges: &mut Vec<(usize, usize)>) {
    const MARKER: &str = "bearer";
    let bytes = value.as_bytes();
    let mut search_from = 0;
    while let Some(relative) = lower[search_from..].find(MARKER) {
        let marker_start = search_from + relative;
        let marker_end = marker_start + MARKER.len();
        let boundary_before =
            marker_start == 0 || !lower.as_bytes()[marker_start - 1].is_ascii_alphanumeric();
        let boundary_after = marker_end < bytes.len() && bytes[marker_end].is_ascii_whitespace();
        if boundary_before && boundary_after {
            let mut cursor = marker_end;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            let (secret_start, secret_end) = quoted_or_unquoted_value_range(bytes, cursor, false);
            if secret_end > secret_start {
                ranges.push((secret_start, secret_end));
                search_from = secret_end;
                continue;
            }
        }
        search_from = marker_end;
    }
}

fn collect_uri_userinfo_redactions(value: &str, ranges: &mut Vec<(usize, usize)>) {
    let bytes = value.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if !bytes[cursor].is_ascii_alphabetic()
            || (cursor > 0 && is_uri_scheme_byte(bytes[cursor - 1]))
        {
            cursor += 1;
            continue;
        }

        let mut scheme_end = cursor + 1;
        while scheme_end < bytes.len() && is_uri_scheme_byte(bytes[scheme_end]) {
            scheme_end += 1;
        }
        if !bytes[scheme_end..].starts_with(b"://") {
            cursor = scheme_end;
            continue;
        }

        let authority_start = scheme_end + 3;
        let mut authority_end = authority_start;
        while authority_end < bytes.len() && !is_uri_authority_terminator(bytes[authority_end]) {
            authority_end += 1;
        }

        let authority = &bytes[authority_start..authority_end];
        if let Some(relative_at) = authority.iter().rposition(|byte| *byte == b'@') {
            let at = authority_start + relative_at;
            if let Some(relative_colon) = bytes[authority_start..at]
                .iter()
                .position(|byte| *byte == b':')
            {
                let secret_start = authority_start + relative_colon + 1;
                if secret_start < at {
                    ranges.push((secret_start, at));
                }
            }
        }

        cursor = authority_end;
    }
}

fn is_uri_scheme_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')
}

fn is_uri_authority_terminator(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'/' | b'?'
                | b'#'
                | b'\\'
                | b'"'
                | b'\''
                | b'`'
                | b'<'
                | b'>'
                | b','
                | b';'
                | b')'
                | b']'
                | b'}'
        )
}

fn quoted_or_unquoted_value_range(
    bytes: &[u8],
    cursor: usize,
    allow_unquoted_spaces: bool,
) -> (usize, usize) {
    if cursor >= bytes.len() {
        return (cursor, cursor);
    }
    let (secret_start, quote) = if matches!(bytes[cursor], b'\'' | b'"') {
        (cursor + 1, Some((bytes[cursor], false)))
    } else if cursor + 1 < bytes.len()
        && bytes[cursor] == b'\\'
        && matches!(bytes[cursor + 1], b'\'' | b'"')
    {
        (cursor + 2, Some((bytes[cursor + 1], true)))
    } else {
        (cursor, None)
    };

    if quote.is_none() && allow_unquoted_spaces {
        return (secret_start, bytes.len());
    }

    let mut secret_end = secret_start;
    if let Some((quote, escaped_quote)) = quote {
        while secret_end < bytes.len() {
            if escaped_quote {
                if escaped_quote_delimiter_at(bytes, secret_end, quote) {
                    break;
                }
            } else if bytes[secret_end] == quote && !byte_is_backslash_escaped(bytes, secret_end) {
                break;
            }
            secret_end += 1;
        }
    } else {
        while secret_end < bytes.len()
            && !matches!(bytes[secret_end], b',' | b';' | b'&' | b'}' | b']')
            && (allow_unquoted_spaces || !bytes[secret_end].is_ascii_whitespace())
        {
            secret_end += 1;
        }
    }
    (secret_start, secret_end)
}

fn byte_is_backslash_escaped(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index;
    let mut backslashes = 0_usize;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        backslashes += 1;
        cursor -= 1;
    }
    backslashes % 2 == 1
}

fn escaped_quote_delimiter_at(bytes: &[u8], index: usize, quote: u8) -> bool {
    if index >= bytes.len() || bytes[index] != b'\\' || (index > 0 && bytes[index - 1] == b'\\') {
        return false;
    }
    let mut cursor = index;
    while cursor < bytes.len() && bytes[cursor] == b'\\' {
        cursor += 1;
    }
    cursor == index + 1 && cursor < bytes.len() && bytes[cursor] == quote
}

fn apply_redaction_ranges(value: &str, mut ranges: Vec<(usize, usize)>) -> String {
    if ranges.is_empty() {
        return value.to_string();
    }
    ranges.sort_unstable();
    let mut merged = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some((_, previous_end)) = merged.last_mut() {
            if start <= *previous_end {
                *previous_end = (*previous_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    for (start, end) in merged {
        output.push_str(&value[cursor..start]);
        output.push_str(REDACTED);
        cursor = end;
    }
    output.push_str(&value[cursor..]);
    output
}

fn redact_secret_like_tokens(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut start = 0;
    while start < value.len() {
        let Some(character) = value[start..].chars().next() else {
            break;
        };
        if !character.is_ascii_alphanumeric() {
            output.push(character);
            start += character.len_utf8();
            continue;
        }
        let token_start = start;
        start += character.len_utf8();
        while start < value.len() {
            let Some(next) = value[start..].chars().next() else {
                break;
            };
            if !(next.is_ascii_alphanumeric() || matches!(next, '.' | '_' | '-')) {
                break;
            }
            start += next.len_utf8();
        }
        let token = &value[token_start..start];
        if is_secret_like_token(token) {
            output.push_str(REDACTED);
        } else {
            output.push_str(token);
        }
    }
    output
}

fn is_secret_like_token(value: &str) -> bool {
    // A closed provider-prefix catalog avoids entropy guesses that would redact
    // ordinary node titles and summaries. Keep additions explicit and testable.
    const TOKEN_PREFIXES: &[&str] = &[
        "sk-",
        "ghp_",
        "github_pat_",
        "glpat-",
        "xoxb-",
        "xoxp-",
        "sk_live_",
        "sk_test_",
    ];

    let lower = value.to_ascii_lowercase();
    if value.len() >= 12
        && TOKEN_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
    {
        return true;
    }
    if value.len() == 20
        && value.starts_with("AKIA")
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return true;
    }
    let segments = value.split('.').collect::<Vec<_>>();
    segments.len() == 3
        && segments.iter().all(|segment| {
            segment.len() >= 8
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
}

fn truncate_utf8(mut value: String, maximum_bytes: usize) -> String {
    if value.len() <= maximum_bytes {
        return value;
    }
    const ELLIPSIS: &str = "…";
    let mut end = maximum_bytes.saturating_sub(ELLIPSIS.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push_str(ELLIPSIS);
    value
}

const EVENTS_TABLE_SQL: &str = r#"
CREATE TABLE observability_events (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) BETWEEN 1 AND 256),
    timestamp TEXT NOT NULL CHECK(length(timestamp) > 0),
    product_version TEXT NOT NULL CHECK(length(product_version) > 0),
    workspace_key TEXT NOT NULL CHECK(length(workspace_key) > 0),
    event_type TEXT NOT NULL CHECK(length(event_type) > 0),
    command TEXT NOT NULL CHECK(length(command) > 0),
    correlation_id TEXT NOT NULL CHECK(length(correlation_id) BETWEEN 1 AND 256),
    bundle_id TEXT CHECK(bundle_id IS NULL OR length(bundle_id) BETWEEN 1 AND 256),
    duration_ms INTEGER CHECK(duration_ms IS NULL OR duration_ms >= 0),
    outcome TEXT NOT NULL CHECK(length(outcome) > 0),
    error_code TEXT CHECK(error_code IS NULL OR length(error_code) > 0),
    payload_json TEXT NOT NULL DEFAULT '{}'
        CHECK(length(CAST(payload_json AS BLOB)) <= 16384
            AND json_valid(payload_json)
            AND json_type(payload_json) = 'object')
)
"#;

const RECALL_BUNDLES_TABLE_SQL: &str = r#"
CREATE TABLE recall_bundles (
    bundle_id TEXT PRIMARY KEY NOT NULL CHECK(length(bundle_id) BETWEEN 1 AND 256),
    timestamp TEXT NOT NULL CHECK(length(timestamp) > 0),
    product_version TEXT NOT NULL CHECK(length(product_version) > 0),
    workspace_key TEXT NOT NULL CHECK(length(workspace_key) > 0),
    correlation_id TEXT NOT NULL CHECK(length(correlation_id) BETWEEN 1 AND 256),
    outcome TEXT NOT NULL CHECK(length(outcome) > 0),
    error_code TEXT CHECK(error_code IS NULL OR length(error_code) > 0),
    duration_ms INTEGER CHECK(duration_ms IS NULL OR duration_ms >= 0),
    more_results INTEGER NOT NULL DEFAULT 0 CHECK(more_results IN (0, 1)),
    continuation_count INTEGER NOT NULL DEFAULT 0 CHECK(continuation_count >= 0)
)
"#;

const BUNDLE_NODES_TABLE_SQL: &str = r#"
CREATE TABLE bundle_nodes (
    bundle_id TEXT NOT NULL CHECK(length(bundle_id) BETWEEN 1 AND 256),
    node_id INTEGER NOT NULL CHECK(node_id > 0),
    first_seen_at TEXT NOT NULL CHECK(length(first_seen_at) > 0),
    node_type TEXT NOT NULL CHECK(length(node_type) > 0),
    node_title TEXT NOT NULL CHECK(length(node_title) BETWEEN 1 AND 512),
    bounded_summary TEXT CHECK(bounded_summary IS NULL OR length(bounded_summary) <= 2048),
    source_ref TEXT CHECK(source_ref IS NULL OR length(source_ref) <= 2048),
    trust_level TEXT CHECK(trust_level IS NULL OR length(trust_level) > 0),
    confidence REAL CHECK(confidence IS NULL OR confidence BETWEEN 0.0 AND 1.0),
    score REAL,
    selection_reasons_json TEXT NOT NULL
        CHECK(length(CAST(selection_reasons_json AS BLOB)) <= 4096
            AND json_valid(selection_reasons_json)
            AND json_type(selection_reasons_json) = 'array'),
    PRIMARY KEY (bundle_id, node_id),
    FOREIGN KEY (bundle_id) REFERENCES recall_bundles(bundle_id)
        ON UPDATE RESTRICT ON DELETE CASCADE
)
"#;

const FEEDBACK_TABLE_SQL: &str = r#"
CREATE TABLE feedback (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) BETWEEN 1 AND 256),
    timestamp TEXT NOT NULL CHECK(length(timestamp) > 0),
    bundle_id TEXT NOT NULL CHECK(length(bundle_id) BETWEEN 1 AND 256),
    outcome TEXT NOT NULL CHECK(outcome IN ('useful', 'partial', 'wrong')),
    reason TEXT CHECK(reason IS NULL OR length(reason) BETWEEN 1 AND 1024),
    FOREIGN KEY (bundle_id) REFERENCES recall_bundles(bundle_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT
)
"#;

const COLLECTOR_STATE_TABLE_SQL: &str = r#"
CREATE TABLE collector_state (
    singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    last_retention_at TEXT,
    retention_floor_at TEXT,
    last_error_code TEXT CHECK(last_error_code IS NULL OR length(last_error_code) > 0)
)
"#;

const INDEX_DEFINITIONS: &[(&str, &str)] = &[
    (
        "idx_observability_events_timestamp",
        "CREATE INDEX idx_observability_events_timestamp ON observability_events(timestamp)",
    ),
    (
        "idx_observability_events_event_type",
        "CREATE INDEX idx_observability_events_event_type ON observability_events(event_type, timestamp)",
    ),
    (
        "idx_observability_events_command",
        "CREATE INDEX idx_observability_events_command ON observability_events(command, timestamp)",
    ),
    (
        "idx_observability_events_outcome",
        "CREATE INDEX idx_observability_events_outcome ON observability_events(outcome, timestamp)",
    ),
    (
        "idx_observability_events_correlation_id",
        "CREATE INDEX idx_observability_events_correlation_id ON observability_events(correlation_id)",
    ),
    (
        "idx_observability_events_bundle_id",
        "CREATE INDEX idx_observability_events_bundle_id ON observability_events(bundle_id)",
    ),
    (
        "idx_observability_events_error_code",
        "CREATE INDEX idx_observability_events_error_code ON observability_events(error_code, timestamp)",
    ),
    (
        "idx_recall_bundles_timestamp",
        "CREATE INDEX idx_recall_bundles_timestamp ON recall_bundles(timestamp)",
    ),
    (
        "idx_recall_bundles_outcome",
        "CREATE INDEX idx_recall_bundles_outcome ON recall_bundles(outcome, timestamp)",
    ),
    (
        "idx_bundle_nodes_first_seen_at",
        "CREATE INDEX idx_bundle_nodes_first_seen_at ON bundle_nodes(first_seen_at)",
    ),
    (
        "idx_bundle_nodes_node_type_bundle_id",
        "CREATE INDEX idx_bundle_nodes_node_type_bundle_id ON bundle_nodes(node_type, bundle_id)",
    ),
    (
        "idx_bundle_nodes_type_title_node",
        "CREATE INDEX idx_bundle_nodes_type_title_node ON bundle_nodes(node_type, node_title, node_id)",
    ),
    (
        "idx_feedback_timestamp",
        "CREATE INDEX idx_feedback_timestamp ON feedback(timestamp)",
    ),
    (
        "idx_feedback_outcome",
        "CREATE INDEX idx_feedback_outcome ON feedback(outcome, timestamp)",
    ),
    (
        "idx_feedback_bundle_id",
        "CREATE INDEX idx_feedback_bundle_id ON feedback(bundle_id)",
    ),
];

const TABLE_DEFINITIONS: &[(&str, &str)] = &[
    ("recall_bundles", RECALL_BUNDLES_TABLE_SQL),
    ("observability_events", EVENTS_TABLE_SQL),
    ("bundle_nodes", BUNDLE_NODES_TABLE_SQL),
    ("feedback", FEEDBACK_TABLE_SQL),
    ("collector_state", COLLECTOR_STATE_TABLE_SQL),
];

const INTERNAL_AUTOINDEXES: &[(&str, &str)] = &[
    (
        "sqlite_autoindex_observability_events_1",
        "observability_events",
    ),
    ("sqlite_autoindex_recall_bundles_1", "recall_bundles"),
    ("sqlite_autoindex_bundle_nodes_1", "bundle_nodes"),
    ("sqlite_autoindex_feedback_1", "feedback"),
];

type InternalSchemaObject = (String, String, Option<String>);
type InternalSchemaManifest = BTreeMap<String, InternalSchemaObject>;

#[derive(Debug, Error)]
pub enum ObservabilityOpenError {
    #[error("observability store does not exist: {0}")]
    Missing(PathBuf),
    #[error("unsafe observability path {}: {source}", path.display())]
    UnsafePath {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid observability store {}: {reason}", path.display())]
    InvalidStore { path: PathBuf, reason: String },
    #[error("observability SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("observability serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Writable handle with no escape hatch to the underlying SQLite connection.
pub struct ObservabilityWriter {
    connection: Connection,
    path: PathBuf,
}

/// Read-only handle with no escape hatch to the underlying SQLite connection.
pub struct ObservabilityReader {
    connection: Connection,
    path: PathBuf,
}

#[derive(Debug, Error)]
enum FeedbackPersistError {
    #[error("recall bundle parent is missing")]
    BundleNotFound,
    #[error(transparent)]
    Store(#[from] ObservabilityOpenError),
}

fn current_timestamp(connection: &Connection) -> Result<String, ObservabilityOpenError> {
    Ok(
        connection.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })?,
    )
}

#[allow(clippy::too_many_arguments)]
fn insert_event_at(
    connection: &Connection,
    path: &Path,
    event_id: &str,
    timestamp: &str,
    workspace_key: &str,
    command: &str,
    correlation_id: &str,
    event: &CollectorEvent,
) -> Result<(), ObservabilityOpenError> {
    let payload_json = serde_json::to_string(&event.payload)?;
    let duration_ms = event
        .duration_ms
        .map(i64::try_from)
        .transpose()
        .map_err(|_| invalid_store(path, "event duration exceeds SQLite range"))?;
    connection.execute(
        "INSERT INTO observability_events (
            id, timestamp, product_version, workspace_key, event_type,
            command, correlation_id, bundle_id, duration_ms, outcome,
            error_code, payload_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            event_id,
            timestamp,
            env!("CARGO_PKG_VERSION"),
            workspace_key,
            event.event_type.as_str(),
            command,
            correlation_id,
            event.bundle_id.as_deref(),
            duration_ms,
            event.outcome.as_str(),
            event.error_code.as_deref(),
            payload_json,
        ],
    )?;
    Ok(())
}

impl ObservabilityWriter {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn schema_version(&self) -> Result<i64, ObservabilityOpenError> {
        Ok(self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    fn insert_event(
        &self,
        event_id: &str,
        workspace_key: &str,
        command: &str,
        correlation_id: &str,
        event: &CollectorEvent,
    ) -> Result<(), ObservabilityOpenError> {
        let timestamp = current_timestamp(&self.connection)?;
        insert_event_at(
            &self.connection,
            &self.path,
            event_id,
            &timestamp,
            workspace_key,
            command,
            correlation_id,
            event,
        )?;
        Ok(())
    }

    fn insert_recall_bundle(
        &mut self,
        workspace_key: &str,
        command: &str,
        correlation_id: &str,
        record: &RecallBundleRecord,
        events: &[CollectorEvent],
    ) -> Result<(), ObservabilityOpenError> {
        let mut event_ids = Vec::with_capacity(events.len());
        let mut correlated_events = Vec::with_capacity(events.len());
        for event in events {
            if !event.event_type.is_recall() {
                return Err(invalid_store(
                    &self.path,
                    "recall bundle write received a non-recall lifecycle event",
                ));
            }
            if event
                .bundle_id
                .as_deref()
                .is_some_and(|bundle_id| bundle_id != record.bundle_id)
            {
                return Err(invalid_store(
                    &self.path,
                    "recall lifecycle event bundle id does not match its parent",
                ));
            }
            let mut event = event.clone();
            event.bundle_id = Some(record.bundle_id.clone());
            event_ids
                .push(random_uuid_v4().map_err(|_| {
                    invalid_store(&self.path, "could not allocate recall event id")
                })?);
            correlated_events.push(event);
        }

        let duration_ms = i64::try_from(record.duration_ms)
            .map_err(|_| invalid_store(&self.path, "bundle duration exceeds SQLite range"))?;
        let continuation_increment = i64::from(record.incoming_continuation);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let timestamp = current_timestamp(&transaction)?;
        let existing: Option<(String, Option<i64>, i64, i64)> = transaction
            .query_row(
                "SELECT workspace_key, duration_ms, more_results, continuation_count
                 FROM recall_bundles WHERE bundle_id = ?1",
                [&record.bundle_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        if let Some((existing_workspace, existing_duration, existing_more, existing_count)) =
            existing
        {
            if existing_workspace != workspace_key {
                return Err(invalid_store(
                    &self.path,
                    "recall bundle belongs to a different workspace",
                ));
            }
            let duration_ms = existing_duration
                .unwrap_or(0)
                .checked_add(duration_ms)
                .ok_or_else(|| invalid_store(&self.path, "bundle duration overflow"))?;
            let continuation_count = existing_count
                .checked_add(continuation_increment)
                .ok_or_else(|| invalid_store(&self.path, "bundle continuation count overflow"))?;
            let more_results = record.more_results.map_or(existing_more, i64::from);
            transaction.execute(
                "UPDATE recall_bundles
                 SET outcome = ?2, error_code = ?3, duration_ms = ?4,
                     more_results = ?5, continuation_count = ?6
                 WHERE bundle_id = ?1",
                rusqlite::params![
                    record.bundle_id,
                    record.outcome.as_str(),
                    record.error_code.as_deref(),
                    duration_ms,
                    more_results,
                    continuation_count,
                ],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO recall_bundles (
                    bundle_id, timestamp, product_version, workspace_key,
                    correlation_id, outcome, error_code, duration_ms,
                    more_results, continuation_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    record.bundle_id,
                    timestamp,
                    env!("CARGO_PKG_VERSION"),
                    workspace_key,
                    correlation_id,
                    record.outcome.as_str(),
                    record.error_code.as_deref(),
                    duration_ms,
                    i64::from(record.more_results.unwrap_or(false)),
                    continuation_increment,
                ],
            )?;
        }

        for node in &record.nodes {
            let selection_reasons_json = serde_json::to_string(&node.selection_reasons)?;
            transaction.execute(
                "INSERT INTO bundle_nodes (
                    bundle_id, node_id, first_seen_at, node_type, node_title,
                    bounded_summary, source_ref, trust_level, confidence,
                    score, selection_reasons_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(bundle_id, node_id) DO NOTHING",
                rusqlite::params![
                    record.bundle_id,
                    node.node_id,
                    timestamp,
                    node.node_type,
                    node.node_title.0,
                    node.bounded_summary.as_ref().map(|value| value.0.as_str()),
                    node.source_ref.as_ref().map(|value| value.0.as_str()),
                    node.trust_level.as_ref().map(|value| value.0.as_str()),
                    node.confidence,
                    node.score,
                    selection_reasons_json,
                ],
            )?;
        }
        for (event_id, event) in event_ids.iter().zip(&correlated_events) {
            insert_event_at(
                &transaction,
                &self.path,
                event_id,
                &timestamp,
                workspace_key,
                command,
                correlation_id,
                event,
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_feedback(
        &mut self,
        feedback_id: &str,
        event_id: &str,
        workspace_key: &str,
        command: &str,
        correlation_id: &str,
        input: &FeedbackRecordInput,
        event: &CollectorEvent,
    ) -> Result<FeedbackReceipt, FeedbackPersistError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(ObservabilityOpenError::from)?;
        let parent_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM recall_bundles
                    WHERE bundle_id = ?1 AND workspace_key = ?2
                 )",
                rusqlite::params![input.bundle_id, workspace_key],
                |row| row.get(0),
            )
            .map_err(ObservabilityOpenError::from)?;
        if !parent_exists {
            return Err(FeedbackPersistError::BundleNotFound);
        }
        let timestamp = current_timestamp(&transaction)?;
        transaction
            .execute(
                "INSERT INTO feedback (id, timestamp, bundle_id, outcome, reason)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    feedback_id,
                    timestamp,
                    input.bundle_id,
                    input.outcome.as_str(),
                    input.reason.as_ref().map(|value| value.0.as_str()),
                ],
            )
            .map_err(ObservabilityOpenError::from)?;
        insert_event_at(
            &transaction,
            &self.path,
            event_id,
            &timestamp,
            workspace_key,
            command,
            correlation_id,
            event,
        )?;
        transaction.commit().map_err(ObservabilityOpenError::from)?;
        Ok(FeedbackReceipt {
            feedback_id: feedback_id.to_string(),
            bundle_id: input.bundle_id.clone(),
            outcome: input.outcome,
            reason_recorded: input.reason.is_some(),
        })
    }

    fn apply_retention(&mut self, policy: RetentionPolicy) -> Result<(), ObservabilityOpenError> {
        if policy.max_age_days <= 0 || policy.max_bytes == 0 || policy.batch_size == 0 {
            return Err(invalid_store(&self.path, "invalid retention policy"));
        }
        let limit = i64::try_from(policy.batch_size)
            .map_err(|_| invalid_store(&self.path, "retention batch size exceeds SQLite range"))?;
        let age_modifier = format!("-{} days", policy.max_age_days);
        let threshold: String = self.connection.query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)",
            [age_modifier],
            |row| row.get(0),
        )?;

        let mut deletion = RetentionDeletion::default();
        loop {
            let batch = self.delete_retention_batch(Some(&threshold), limit)?;
            if batch.deleted_count == 0 {
                break;
            }
            deletion.merge(batch)?;
            self.reclaim_retention_pages(policy.batch_size)?;
        }

        self.connection.execute(
            "UPDATE collector_state
             SET last_retention_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 retention_floor_at = CASE
                     WHEN ?1 IS NULL THEN retention_floor_at
                     WHEN retention_floor_at IS NULL OR retention_floor_at < ?1 THEN ?1
                     ELSE retention_floor_at
                 END
             WHERE singleton_id = 1",
            [deletion.floor_at.as_deref()],
        )?;

        let mut physical_bytes =
            self.reclaim_until_size_stable(policy.max_bytes, policy.batch_size)?;
        while physical_bytes > policy.max_bytes {
            let batch = self.delete_retention_batch(None, limit)?;
            if batch.deleted_count == 0 {
                let reclaimed_bytes =
                    self.reclaim_until_size_stable(policy.max_bytes, policy.batch_size)?;
                if reclaimed_bytes < physical_bytes {
                    physical_bytes = reclaimed_bytes;
                    continue;
                }
                return Err(invalid_store(
                    &self.path,
                    "observability store cannot be reduced below retention size limit",
                ));
            }
            deletion.merge(batch)?;
            physical_bytes = self.reclaim_until_size_stable(policy.max_bytes, policy.batch_size)?;
        }
        Ok(())
    }

    fn delete_retention_batch(
        &mut self,
        before: Option<&str>,
        limit: i64,
    ) -> Result<RetentionDeletion, ObservabilityOpenError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let roots = load_oldest_retention_roots(&transaction, before, limit)?;
        let mut deletion = RetentionDeletion::default();
        delete_retention_roots(&transaction, &roots, &mut deletion)?;
        if deletion.deleted_count > 0 {
            transaction.execute(
                "UPDATE collector_state
                 SET last_retention_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                     retention_floor_at = CASE
                         WHEN retention_floor_at IS NULL OR retention_floor_at < ?1 THEN ?1
                         ELSE retention_floor_at
                     END
                 WHERE singleton_id = 1",
                [deletion.floor_at.as_deref()],
            )?;
        }
        transaction.commit()?;
        Ok(deletion)
    }

    fn reclaim_retention_pages(&self, batch_size: usize) -> Result<(), ObservabilityOpenError> {
        self.connection
            .execute_batch(&format!("PRAGMA incremental_vacuum({batch_size});"))?;
        let checkpoint: (i64, i64, i64) =
            self.connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
        if checkpoint.0 != 0 {
            return Err(invalid_store(
                &self.path,
                "observability WAL checkpoint is busy",
            ));
        }
        Ok(())
    }

    fn reclaim_until_size_stable(
        &self,
        max_bytes: u64,
        batch_size: usize,
    ) -> Result<u64, ObservabilityOpenError> {
        let mut physical_bytes = physical_store_bytes(&self.path)?;
        while physical_bytes > max_bytes {
            self.reclaim_retention_pages(batch_size)?;
            let reclaimed_bytes = physical_store_bytes(&self.path)?;
            if reclaimed_bytes >= physical_bytes {
                return Ok(reclaimed_bytes);
            }
            physical_bytes = reclaimed_bytes;
        }
        Ok(physical_bytes)
    }
}

#[derive(Debug)]
enum RetentionRootKind {
    Event,
    Feedback,
    Bundle,
}

#[derive(Debug)]
struct RetentionRoot {
    kind: RetentionRootKind,
    id: String,
    timestamp: String,
}

#[derive(Debug, Default)]
struct RetentionDeletion {
    deleted_count: u64,
    floor_at: Option<String>,
}

impl RetentionDeletion {
    fn record(&mut self, timestamp: &str, changed: usize) -> Result<(), rusqlite::Error> {
        if changed == 0 {
            return Ok(());
        }
        let changed = u64::try_from(changed)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.deleted_count = self.deleted_count.checked_add(changed).ok_or_else(|| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(io::Error::other(
                "retention deletion count overflow",
            )))
        })?;
        if self
            .floor_at
            .as_deref()
            .is_none_or(|current| current < timestamp)
        {
            self.floor_at = Some(timestamp.to_string());
        }
        Ok(())
    }

    fn merge(&mut self, other: Self) -> Result<(), ObservabilityOpenError> {
        self.deleted_count = self
            .deleted_count
            .checked_add(other.deleted_count)
            .ok_or_else(|| {
                invalid_store(
                    Path::new("observability.sqlite"),
                    "retention deletion count overflow",
                )
            })?;
        if let Some(floor) = other.floor_at {
            if self
                .floor_at
                .as_deref()
                .is_none_or(|current| current < floor.as_str())
            {
                self.floor_at = Some(floor);
            }
        }
        Ok(())
    }
}

fn load_oldest_retention_roots(
    connection: &Connection,
    before: Option<&str>,
    limit: i64,
) -> rusqlite::Result<Vec<RetentionRoot>> {
    let mut statement = connection.prepare(
        "SELECT kind, id, timestamp FROM (
            SELECT 0 AS kind_order, 'event' AS kind, id, timestamp
            FROM observability_events
            UNION ALL
            SELECT 1, 'feedback', id, timestamp
            FROM feedback
            UNION ALL
            SELECT 2, 'bundle', bundle_id, timestamp
            FROM recall_bundles AS bundle
            WHERE NOT EXISTS (
                SELECT 1 FROM feedback WHERE feedback.bundle_id = bundle.bundle_id
            )
         )
         WHERE (?1 IS NULL OR timestamp < ?1)
         ORDER BY timestamp, kind_order, id
         LIMIT ?2",
    )?;
    let rows = statement.query_map(rusqlite::params![before, limit], |row| {
        let kind = match row.get::<_, String>(0)?.as_str() {
            "event" => RetentionRootKind::Event,
            "feedback" => RetentionRootKind::Feedback,
            "bundle" => RetentionRootKind::Bundle,
            _ => return Err(rusqlite::Error::InvalidQuery),
        };
        Ok(RetentionRoot {
            kind,
            id: row.get(1)?,
            timestamp: row.get(2)?,
        })
    })?;
    rows.collect()
}

fn delete_retention_roots(
    connection: &Connection,
    roots: &[RetentionRoot],
    deletion: &mut RetentionDeletion,
) -> rusqlite::Result<()> {
    for root in roots {
        let changed = match root.kind {
            RetentionRootKind::Event => {
                connection.execute("DELETE FROM observability_events WHERE id = ?1", [&root.id])?
            }
            RetentionRootKind::Feedback => {
                connection.execute("DELETE FROM feedback WHERE id = ?1", [&root.id])?
            }
            RetentionRootKind::Bundle => connection.execute(
                "DELETE FROM recall_bundles
                 WHERE bundle_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM feedback WHERE feedback.bundle_id = ?1
                   )",
                [&root.id],
            )?,
        };
        deletion.record(&root.timestamp, changed)?;
    }
    Ok(())
}

fn physical_store_bytes(database: &Path) -> Result<u64, ObservabilityOpenError> {
    let mut bytes = 0_u64;
    for path in managed_database_paths(database) {
        storage::validate_optional_regular_file(&path)
            .map_err(|source| unsafe_path(&path, source))?;
        match fs::metadata(&path) {
            Ok(metadata) => {
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| invalid_store(database, "database size overflow"))?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(unsafe_path(&path, source)),
        }
    }
    Ok(bytes)
}

impl ObservabilityReader {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn schema_version(&self) -> Result<i64, ObservabilityOpenError> {
        Ok(self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreKind {
    EmptyV0,
    InitializedV1,
}

/// Lazily creates and initializes the separate observability store.
///
/// Existing stores are inspected through a read-only connection before any
/// writable connection is opened. A malformed or incompatible store is never
/// migrated or repaired implicitly.
pub fn open_writer(
    workspace_paths: &WorkspacePaths,
) -> Result<ObservabilityWriter, ObservabilityOpenError> {
    validate_layout(workspace_paths, false)?;
    storage::ensure_owned_direct_directory(workspace_paths.root(), workspace_paths.observability())
        .map_err(|source| unsafe_path(workspace_paths.observability(), source))?;
    validate_layout(workspace_paths, true)?;

    let database_exists = workspace_paths.observability_db().exists();
    let is_zero_length = database_exists
        && fs::metadata(workspace_paths.observability_db())
            .map_err(|source| unsafe_path(workspace_paths.observability_db(), source))?
            .len()
            == 0;

    let store_kind = if !database_exists || is_zero_length {
        StoreKind::EmptyV0
    } else {
        let validation_connection = open_read_only_connection(workspace_paths)?;
        inspect_store(&validation_connection, workspace_paths.observability_db())?
    };

    let database_path = canonical_database_open_path(workspace_paths)?;
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    if !database_exists {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let mut connection = Connection::open_with_flags(&database_path, flags)?;
    configure_writer_connection(&connection)?;

    if store_kind == StoreKind::EmptyV0 {
        initialize_v1(&mut connection, workspace_paths.observability_db())?;
    }
    inspect_store(&connection, workspace_paths.observability_db())?;

    Ok(ObservabilityWriter {
        connection,
        path: workspace_paths.observability_db().clone(),
    })
}

/// Opens an initialized store for writes without creating or repairing it.
fn open_existing_writer(
    workspace_paths: &WorkspacePaths,
) -> Result<ObservabilityWriter, ObservabilityOpenError> {
    match fs::symlink_metadata(workspace_paths.observability_db()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ObservabilityOpenError::Missing(
                workspace_paths.observability_db().clone(),
            ));
        }
        Err(source) => {
            return Err(unsafe_path(workspace_paths.observability_db(), source));
        }
        Ok(_) => {}
    }
    validate_layout(workspace_paths, true)?;
    let validation_connection = open_read_only_connection(workspace_paths)?;
    if inspect_store(&validation_connection, workspace_paths.observability_db())?
        != StoreKind::InitializedV1
    {
        return Err(invalid_store(
            workspace_paths.observability_db(),
            "feedback requires an initialized observability store",
        ));
    }
    drop(validation_connection);

    let database_path = canonical_database_open_path(workspace_paths)?;
    let connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    configure_writer_connection(&connection)?;
    if inspect_store(&connection, workspace_paths.observability_db())? != StoreKind::InitializedV1 {
        return Err(invalid_store(
            workspace_paths.observability_db(),
            "feedback requires an initialized observability store",
        ));
    }
    Ok(ObservabilityWriter {
        connection,
        path: workspace_paths.observability_db().clone(),
    })
}

/// Opens an existing store without creating its directory or main database.
///
/// SQLite may maintain WAL/SHM lock sidecars, but this handle cannot change
/// the observability schema or data.
pub fn open_reader(
    workspace_paths: &WorkspacePaths,
) -> Result<ObservabilityReader, ObservabilityOpenError> {
    validate_layout(workspace_paths, true)?;
    match fs::symlink_metadata(workspace_paths.observability_db()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ObservabilityOpenError::Missing(
                workspace_paths.observability_db().clone(),
            ));
        }
        Err(source) => {
            return Err(unsafe_path(workspace_paths.observability_db(), source));
        }
        Ok(_) => {}
    }

    let connection = open_read_only_connection(workspace_paths)?;
    if inspect_store(&connection, workspace_paths.observability_db())? != StoreKind::InitializedV1 {
        return Err(invalid_store(
            workspace_paths.observability_db(),
            "empty version-0 store cannot be opened for reading",
        ));
    }

    Ok(ObservabilityReader {
        connection,
        path: workspace_paths.observability_db().clone(),
    })
}

fn initialize_v1(connection: &mut Connection, path: &Path) -> Result<(), ObservabilityOpenError> {
    connection.execute_batch(
        "PRAGMA auto_vacuum = INCREMENTAL;
         VACUUM;",
    )?;
    let auto_vacuum: i64 = connection.query_row("PRAGMA auto_vacuum", [], |row| row.get(0))?;
    if auto_vacuum != 2 {
        return Err(invalid_store(
            path,
            format!("could not enable incremental auto-vacuum; SQLite returned {auto_vacuum}"),
        ));
    }
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(invalid_store(
            path,
            format!("could not enable WAL journal mode; SQLite returned {journal_mode}"),
        ));
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for (_, sql) in TABLE_DEFINITIONS {
        transaction.execute_batch(sql)?;
    }
    for (_, sql) in INDEX_DEFINITIONS {
        transaction.execute_batch(sql)?;
    }
    transaction.execute(
        "INSERT INTO collector_state (
            singleton_id, schema_version, last_retention_at,
            retention_floor_at, last_error_code
         ) VALUES (1, ?1, NULL, NULL, NULL)",
        [OBSERVABILITY_SCHEMA_VERSION],
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA application_id = {OBSERVABILITY_APPLICATION_ID};
         PRAGMA user_version = {OBSERVABILITY_SCHEMA_VERSION};"
    ))?;
    transaction.commit()?;
    Ok(())
}

fn inspect_store(
    connection: &Connection,
    path: &Path,
) -> Result<StoreKind, ObservabilityOpenError> {
    let quick_check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if quick_check != "ok" {
        return Err(invalid_store(
            path,
            format!("SQLite quick_check failed: {quick_check}"),
        ));
    }

    let application_id: i64 =
        connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    let user_version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let objects = read_schema_objects(connection)?;
    let internal_objects = read_internal_schema_objects(connection)?;
    let auto_vacuum: i64 = connection.query_row("PRAGMA auto_vacuum", [], |row| row.get(0))?;
    let journal_mode: String = connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;

    if application_id == 0 && user_version == 0 && objects.is_empty() && internal_objects.is_empty()
    {
        if valid_empty_v0_pragmas(auto_vacuum, journal_mode.as_str()) {
            return Ok(StoreKind::EmptyV0);
        }
        return Err(invalid_store(
            path,
            format!(
                "empty version-0 store has unsupported auto_vacuum={auto_vacuum}, \
                 journal_mode={journal_mode}"
            ),
        ));
    }
    if application_id != OBSERVABILITY_APPLICATION_ID {
        return Err(invalid_store(
            path,
            format!("application_id is {application_id}, expected {OBSERVABILITY_APPLICATION_ID}"),
        ));
    }
    if user_version != OBSERVABILITY_SCHEMA_VERSION {
        return Err(invalid_store(
            path,
            format!("user_version is {user_version}, expected {OBSERVABILITY_SCHEMA_VERSION}"),
        ));
    }

    if auto_vacuum != 2 {
        return Err(invalid_store(
            path,
            format!("auto_vacuum is {auto_vacuum}, expected incremental (2)"),
        ));
    }
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(invalid_store(
            path,
            format!("journal_mode is {journal_mode}, expected wal"),
        ));
    }

    let expected = expected_schema_objects();
    if objects != expected {
        return Err(invalid_store(
            path,
            describe_schema_difference(&expected, &objects),
        ));
    }
    let expected_internal = expected_internal_schema_objects();
    if internal_objects != expected_internal {
        return Err(invalid_store(
            path,
            format!(
                "internal schema mismatch (expected: {expected_internal:?}; \
                 actual: {internal_objects:?})"
            ),
        ));
    }

    let state_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM collector_state", [], |row| row.get(0))?;
    let state_version: Option<i64> = connection
        .query_row(
            "SELECT schema_version FROM collector_state WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if state_count != 1 || state_version != Some(OBSERVABILITY_SCHEMA_VERSION) {
        return Err(invalid_store(
            path,
            "collector_state must contain exactly the version-1 singleton",
        ));
    }

    let mut foreign_key_check = connection.prepare("PRAGMA foreign_key_check")?;
    if foreign_key_check.query([])?.next()?.is_some() {
        return Err(invalid_store(path, "foreign_key_check failed"));
    }

    Ok(StoreKind::InitializedV1)
}

fn valid_empty_v0_pragmas(auto_vacuum: i64, journal_mode: &str) -> bool {
    matches!(
        (auto_vacuum, journal_mode.to_ascii_lowercase().as_str()),
        (0 | 2, "delete") | (2, "wal")
    )
}

fn read_schema_objects(
    connection: &Connection,
) -> Result<BTreeMap<(String, String), String>, ObservabilityOpenError> {
    let mut statement = connection.prepare(
        "SELECT type, name, COALESCE(sql, '')
         FROM sqlite_schema
         WHERE substr(name, 1, 7) <> 'sqlite_'
         ORDER BY type, name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut objects = BTreeMap::new();
    for row in rows {
        let (kind, name, sql) = row?;
        objects.insert((kind, name), normalize_sql(&sql));
    }
    Ok(objects)
}

fn read_internal_schema_objects(
    connection: &Connection,
) -> Result<InternalSchemaManifest, ObservabilityOpenError> {
    let mut statement = connection.prepare(
        "SELECT name, type, tbl_name, sql
         FROM sqlite_schema
         WHERE substr(name, 1, 7) = 'sqlite_'
         ORDER BY name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut objects = BTreeMap::new();
    for row in rows {
        let (name, kind, table, sql) = row?;
        objects.insert(name, (kind, table, sql));
    }
    Ok(objects)
}

fn expected_schema_objects() -> BTreeMap<(String, String), String> {
    let mut objects = BTreeMap::new();
    for (name, sql) in TABLE_DEFINITIONS {
        objects.insert(
            ("table".to_string(), (*name).to_string()),
            normalize_sql(sql),
        );
    }
    for (name, sql) in INDEX_DEFINITIONS {
        objects.insert(
            ("index".to_string(), (*name).to_string()),
            normalize_sql(sql),
        );
    }
    objects
}

fn expected_internal_schema_objects() -> InternalSchemaManifest {
    INTERNAL_AUTOINDEXES
        .iter()
        .map(|(name, table)| {
            (
                (*name).to_string(),
                ("index".to_string(), (*table).to_string(), None),
            )
        })
        .collect()
}

fn normalize_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn describe_schema_difference(
    expected: &BTreeMap<(String, String), String>,
    actual: &BTreeMap<(String, String), String>,
) -> String {
    let missing = expected
        .keys()
        .filter(|key| !actual.contains_key(*key))
        .map(|(_, name)| name.as_str())
        .collect::<Vec<_>>();
    let extra = actual
        .keys()
        .filter(|key| !expected.contains_key(*key))
        .map(|(_, name)| name.as_str())
        .collect::<Vec<_>>();
    let changed = expected
        .iter()
        .filter(|(key, sql)| {
            actual
                .get(*key)
                .is_some_and(|actual_sql| actual_sql != *sql)
        })
        .map(|((_, name), _)| name.as_str())
        .collect::<Vec<_>>();
    format!("schema mismatch (missing: {missing:?}; extra: {extra:?}; changed: {changed:?})")
}

fn configure_writer_connection(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA trusted_schema = OFF;
        PRAGMA query_only = OFF;
        ",
    )
}

fn configure_reader_connection(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA trusted_schema = OFF;
        PRAGMA query_only = ON;
        ",
    )
}

fn open_read_only_connection(
    workspace_paths: &WorkspacePaths,
) -> Result<Connection, ObservabilityOpenError> {
    let database_path = canonical_database_open_path(workspace_paths)?;
    // Do not use SQLite's `immutable=1` URI here. It bypasses WAL locking and
    // can race a legal concurrent checkpoint. A true READ_ONLY connection may
    // maintain SQLite's WAL/SHM lock sidecars, but cannot change schema or data.
    let connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    configure_reader_connection(&connection)?;
    Ok(connection)
}

fn validate_layout(
    workspace_paths: &WorkspacePaths,
    observability_must_exist: bool,
) -> Result<(), ObservabilityOpenError> {
    storage::validate_real_directory(workspace_paths.root())
        .map_err(|source| unsafe_path(workspace_paths.root(), source))?;

    let expected_directory = workspace_paths.root().join("observability");
    let expected_database = expected_directory.join("observability.sqlite");
    if workspace_paths.observability() != &expected_directory
        || workspace_paths.observability_db() != &expected_database
    {
        return Err(invalid_store(
            workspace_paths.root(),
            "observability paths are not direct managed children",
        ));
    }

    match fs::symlink_metadata(workspace_paths.observability()) {
        Ok(_) => {
            storage::validate_real_directory(workspace_paths.observability())
                .map_err(|source| unsafe_path(workspace_paths.observability(), source))?;
            storage::validate_canonical_direct_child(
                workspace_paths.root(),
                workspace_paths.observability(),
            )
            .map_err(|source| unsafe_path(workspace_paths.observability(), source))?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound && !observability_must_exist => {
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ObservabilityOpenError::Missing(
                workspace_paths.observability().clone(),
            ));
        }
        Err(source) => return Err(unsafe_path(workspace_paths.observability(), source)),
    }

    for path in managed_database_paths(workspace_paths.observability_db()) {
        storage::validate_optional_regular_file(&path)
            .map_err(|source| unsafe_path(&path, source))?;
        if path.exists() && path.parent() != Some(workspace_paths.observability()) {
            return Err(invalid_store(
                &path,
                "managed database file is not a direct child",
            ));
        }
    }
    Ok(())
}

fn managed_database_paths(database: &Path) -> [PathBuf; 4] {
    [
        database.to_path_buf(),
        with_database_suffix(database, "-wal"),
        with_database_suffix(database, "-shm"),
        with_database_suffix(database, "-journal"),
    ]
}

fn with_database_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    value.into()
}

fn canonical_database_open_path(
    workspace_paths: &WorkspacePaths,
) -> Result<PathBuf, ObservabilityOpenError> {
    let parent = workspace_paths.observability();
    let file_name = workspace_paths
        .observability_db()
        .file_name()
        .ok_or_else(|| {
            invalid_store(
                workspace_paths.observability_db(),
                "database path has no file name",
            )
        })?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|source| unsafe_path(parent, source))?;
    Ok(canonical_parent.join(file_name))
}

fn unsafe_path(path: &Path, source: io::Error) -> ObservabilityOpenError {
    ObservabilityOpenError::UnsafePath {
        path: path.to_path_buf(),
        source,
    }
}

fn invalid_store(path: &Path, reason: impl Into<String>) -> ObservabilityOpenError {
    ObservabilityOpenError::InvalidStore {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::ffi::OsString;
    use std::sync::MutexGuard;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestWorkspace {
        paths: WorkspacePaths,
        home: PathBuf,
        original_home: Option<OsString>,
        _lock: MutexGuard<'static, ()>,
    }

    impl TestWorkspace {
        fn new(name: &str) -> Self {
            let lock = crate::install::test_env_lock()
                .lock()
                .expect("environment lock should not be poisoned");
            let home = temp_path(name);
            let original_home = env::var_os("AOPMEM_HOME");
            env::set_var("AOPMEM_HOME", &home);
            let global_paths = storage::resolve_paths().expect("test AOPMEM_HOME should resolve");
            storage::ensure_global_dirs(&global_paths).expect("global directories should create");
            let paths = storage::ensure_workspace_dirs(&global_paths, format!("{name}-workspace"))
                .expect("workspace directories should create");
            Self {
                paths,
                home,
                original_home,
                _lock: lock,
            }
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

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the epoch")
            .as_nanos();
        env::temp_dir().join(format!(
            "aopmem-observability-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn fallible_uuid_generation_reports_rng_failure_without_panicking() {
        let failure = random_uuid_v4_with(|_| Err::<(), ()>(()));
        assert_eq!(failure, Err(CollectorInputError::RandomSourceUnavailable));

        let generated = random_uuid_v4_with(|bytes| {
            bytes.copy_from_slice(&[0x5a; 16]);
            Ok::<(), ()>(())
        })
        .expect("deterministic bytes should build a UUID");
        let parsed = uuid::Uuid::parse_str(&generated).expect("generated UUID should parse");
        assert_eq!(parsed.get_version(), Some(uuid::Version::Random));
        assert_eq!(parsed.get_variant(), uuid::Variant::RFC4122);
    }

    fn create_initialized_workspace(name: &str) -> TestWorkspace {
        let workspace = TestWorkspace::new(name);
        drop(open_writer(&workspace.paths).expect("observability writer should initialize"));
        workspace
    }

    fn database_snapshot(path: &Path) -> (Vec<u8>, SystemTime) {
        let bytes = fs::read(path).expect("database bytes should read");
        let modified = fs::metadata(path)
            .expect("database metadata should read")
            .modified()
            .expect("database modification time should exist");
        (bytes, modified)
    }

    fn assert_writer_rejected_without_main_file_mutation(workspace: &TestWorkspace) {
        let before = database_snapshot(workspace.paths.observability_db());
        let error = open_writer(&workspace.paths)
            .err()
            .expect("invalid store should be rejected");
        assert!(
            matches!(
                error,
                ObservabilityOpenError::InvalidStore { .. } | ObservabilityOpenError::Sqlite(_)
            ),
            "unexpected rejection: {error}"
        );
        let after = database_snapshot(workspace.paths.observability_db());
        assert_eq!(after.0, before.0, "rejection changed database bytes");
        assert_eq!(after.1, before.1, "rejection changed database mtime");
    }

    fn with_test_connection(workspace: &TestWorkspace, action: impl FnOnce(&Connection)) {
        let connection = Connection::open(workspace.paths.observability_db())
            .expect("test database should open for fixture mutation");
        action(&connection);
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("fixture mutation should checkpoint");
    }

    fn insert_event(connection: &Connection, id: &str) {
        insert_event_at(connection, id, "2026-07-15T00:00:00Z", "{}");
    }

    fn insert_event_at(connection: &Connection, id: &str, timestamp: &str, payload_json: &str) {
        connection
            .execute(
                "INSERT INTO observability_events (
                    id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, bundle_id, duration_ms, outcome,
                    error_code, payload_json
                 ) VALUES (?1, ?2, '0.2.0-rc1',
                    'test-workspace', 'doctor', 'doctor', 'correlation-1',
                    NULL, 12, 'success', NULL, ?3)",
                rusqlite::params![id, timestamp, payload_json],
            )
            .expect("valid event should insert");
    }

    fn insert_bundle(connection: &Connection, id: &str) {
        insert_bundle_at(connection, id, "2026-07-15T00:00:00Z");
    }

    fn insert_bundle_at(connection: &Connection, id: &str, timestamp: &str) {
        connection
            .execute(
                "INSERT INTO recall_bundles (
                    bundle_id, timestamp, product_version, workspace_key,
                    correlation_id, outcome, error_code, duration_ms,
                    more_results, continuation_count
                 ) VALUES (?1, ?2, '0.2.0-rc1',
                    'test-workspace', 'correlation-1', 'success', NULL, 5, 0, 0)",
                rusqlite::params![id, timestamp],
            )
            .expect("valid recall bundle should insert");
    }

    fn checkpoint(connection: &Connection) {
        let result: (i64, i64, i64) = connection
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .expect("test WAL checkpoint should run");
        assert_eq!(result.0, 0, "test WAL checkpoint should not be busy");
    }

    fn schema_columns(connection: &Connection, table: &str) -> Vec<String> {
        let mut statement = connection
            .prepare("SELECT name FROM pragma_table_xinfo(?1) ORDER BY cid")
            .expect("table_xinfo statement should prepare");
        statement
            .query_map([table], |row| row.get(0))
            .expect("table_xinfo should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("table_xinfo rows should read")
    }

    fn directory_manifest(path: &Path) -> Vec<(OsString, u64, SystemTime)> {
        let mut manifest = fs::read_dir(path)
            .expect("directory should read")
            .map(|entry| {
                let entry = entry.expect("directory entry should read");
                let metadata = entry.metadata().expect("entry metadata should read");
                (
                    entry.file_name(),
                    metadata.len(),
                    metadata.modified().expect("entry mtime should exist"),
                )
            })
            .collect::<Vec<_>>();
        manifest.sort_by(|left, right| left.0.cmp(&right.0));
        manifest
    }

    fn empty_collector_event() -> CollectorEvent {
        CollectorEvent::new(
            EventType::Doctor,
            EventOutcome::Success,
            EventPayload::Empty,
        )
        .expect("empty typed event should validate")
    }

    #[test]
    fn event_catalog_is_closed_and_exact() {
        let actual = EventType::ALL
            .iter()
            .map(|event_type| event_type.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            [
                "install.started",
                "install.completed",
                "install.failed",
                "update.started",
                "update.completed",
                "update.failed",
                "workspace.init",
                "adapter.seed",
                "adapter.sync",
                "adapter.drift",
                "recall.started",
                "recall.completed",
                "recall.failed",
                "recall.continuation",
                "recall.empty",
                "recall.truncated",
                "recall.mandatory_overflow",
                "node.created",
                "node.updated",
                "node.deprecated",
                "link.created",
                "remember",
                "teach.started",
                "teach.proposed",
                "teach.applied",
                "reflection.inventory",
                "reflection.proposal",
                "reflection.applied",
                "tool.validation",
                "tool.run.started",
                "tool.run.completed",
                "tool.run.failed",
                "tool.run.timeout",
                "tool.output.artifact",
                "mcp.status",
                "doctor",
                "verify",
                "audit.snapshot.completed",
                "audit.snapshot.pending",
                "audit.snapshot.failed",
                "artifacts.cleanup",
                "feedback.recorded",
            ]
        );
    }

    #[test]
    fn collector_is_lazy_until_first_record() {
        let workspace = TestWorkspace::new("collector-lazy");
        let collector = LocalCollector::new(&workspace.paths, "doctor")
            .expect("collector input should validate");

        assert!(!workspace.paths.observability().exists());
        let correlation = uuid::Uuid::parse_str(collector.correlation_id())
            .expect("correlation id should be UUID");
        assert_eq!(correlation.get_version(), Some(uuid::Version::Random));
    }

    #[test]
    fn collector_accepts_long_valid_managed_workspace_key() {
        let _workspace = TestWorkspace::new("collector-long-key");
        let global_paths = storage::resolve_paths().expect("test AOPMEM_HOME should resolve");
        let workspace_key = format!("{}-01234567", "a".repeat(140));
        assert!(workspace_key.len() > 128);
        assert!(workspace_key.len() <= MAX_MANAGED_WORKSPACE_KEY_BYTES);
        let paths = storage::ensure_workspace_dirs(&global_paths, &workspace_key)
            .expect("long managed workspace should create");

        let mut collector =
            LocalCollector::new(&paths, "doctor").expect("long workspace key should be valid");
        assert_eq!(collector.record(&empty_collector_event()), None);
        let stored_key: String = collector
            .writer
            .as_ref()
            .expect("collector writer should exist")
            .connection
            .query_row(
                "SELECT workspace_key FROM observability_events LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("stored workspace key should read");
        assert_eq!(stored_key, workspace_key);
    }

    #[test]
    fn collector_writes_typed_bounded_fields() {
        let workspace = TestWorkspace::new("collector-write");
        let payload = NodePayload::new(
            7,
            "workflow",
            "Registry https://reader:top-secret@registry.example/Привет",
            Some(concat!(
                "token=abc123 normal summary ",
                "glpat-1234567890abcdef safe https://example.com/путь ",
                "postgres://audit-user:NOHOST_PERSISTED_URI_CANARY_SECRET@/app?label=Привет"
            )),
            Some(concat!(
                "https://source-user:hunter2@source.example/path ",
                "sk_live_1234567890abcdef sk_test_1234567890abcdef ",
                "custom://truncated-user:TRUNCATED_PERSISTED_URI_CANARY_SECRET@ tail=видимый"
            )),
        )
        .expect("node payload should validate");
        let event = CollectorEvent::new(
            EventType::NodeCreated,
            EventOutcome::Recorded,
            EventPayload::Node(payload),
        )
        .expect("event should validate")
        .with_duration_ms(17)
        .expect("duration should validate")
        .with_bundle_id("550e8400-e29b-41d4-a716-446655440000")
        .expect("bundle id should validate")
        .with_error_code("NO_ERROR")
        .expect("error code should validate");
        let mut collector = LocalCollector::new(&workspace.paths, "node_create")
            .expect("collector should initialize lazily");
        let correlation_id = collector.correlation_id().to_string();

        assert_eq!(collector.record(&event), None);
        let connection = &collector
            .writer
            .as_ref()
            .expect("first record should open writer")
            .connection;
        let row: (String, String, String, String, String, String, i64, String) = connection
            .query_row(
                "SELECT id, timestamp, product_version, workspace_key, event_type,
                        correlation_id, duration_ms, payload_json
                 FROM observability_events",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .expect("typed event should read");
        let event_id = uuid::Uuid::parse_str(&row.0).expect("event id should be UUID");
        assert_eq!(event_id.get_version(), Some(uuid::Version::Random));
        assert!(!row.1.is_empty());
        assert_eq!(row.2, env!("CARGO_PKG_VERSION"));
        assert_eq!(row.3, "collector-write-workspace");
        assert_eq!(row.4, "node.created");
        assert_eq!(row.5, correlation_id);
        assert_eq!(row.6, 17);
        assert!(!row.7.contains("top-secret"));
        assert!(!row.7.contains("abc123"));
        assert!(!row.7.contains("hunter2"));
        assert!(!row.7.contains("glpat-1234567890abcdef"));
        assert!(!row.7.contains("sk_live_1234567890abcdef"));
        assert!(!row.7.contains("sk_test_1234567890abcdef"));
        assert!(!row.7.contains("NOHOST_PERSISTED_URI_CANARY_SECRET"));
        assert!(!row.7.contains("TRUNCATED_PERSISTED_URI_CANARY_SECRET"));
        assert!(row.7.contains(REDACTED));
        assert!(row
            .7
            .contains("https://reader:[REDACTED]@registry.example/Привет"));
        assert!(row.7.contains("normal summary"));
        assert!(row.7.contains("safe https://example.com/путь"));
        assert!(row
            .7
            .contains("postgres://audit-user:[REDACTED]@/app?label=Привет"));
        assert!(row
            .7
            .contains("https://source-user:[REDACTED]@source.example/path"));
        assert!(row
            .7
            .contains("custom://truncated-user:[REDACTED]@ tail=видимый"));
    }

    #[test]
    fn privacy_redaction_caps_and_validators_are_deterministic() {
        let text = concat!(
            "Authorization: Bearer abc\n",
            "Cookie: session=cookie-value\n",
            "password=hunter token=token-value secret=secret-value\n",
            "Bearer standalone-value\n",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signature123\n",
            "sk-1234567890abcdef"
        );
        let first = SafeText::new("summary", text, 2_048).expect("text should redact");
        let second = SafeText::new("summary", text, 2_048).expect("text should redact");
        assert_eq!(first, second);
        for secret in [
            "abc",
            "cookie-value",
            "hunter",
            "token-value",
            "secret-value",
            "standalone-value",
            "eyJhbGciOiJIUzI1NiJ9",
            "sk-1234567890abcdef",
        ] {
            assert!(!first.0.contains(secret), "secret survived: {secret}");
        }

        let unicode = "Ж".repeat(600);
        let bounded = SafeText::new("title", &unicode, 512).expect("unicode should bound");
        assert!(bounded.0.len() <= 512);
        assert!(bounded.0.ends_with('…'));
        assert!(matches!(
            SafeText::new("summary", "bad\0value", 2_048),
            Err(CollectorInputError::Nul { .. })
        ));
        assert!(SafeText::new("summary", &"x".repeat(65_537), 2_048).is_err());
        assert!(RecallScore::new(1, f64::NAN).is_err());
        assert!(ArtifactPayload::new("../outside", 1).is_err());
        assert!(ArtifactPayload::new("C:\\outside", 1).is_err());
        assert!(ArtifactPayload::new("artifacts/output.txt", 1).is_ok());
        assert!(McpPayload::new("corp", McpStatus::ConfiguredUnverified).is_ok());
        assert!(LinkPayload::new(1, 2, "поддерживает").is_ok());
        assert!(ToolPayload::new("инструмент", false).is_ok());
        assert!(McpPayload::new("локальный-профиль", McpStatus::Installed).is_ok());
        assert!(ToolPayload::new("   ", false).is_err());
        assert!(McpPayload::new("\0", McpStatus::Missing).is_err());
        assert!(ToolPayload::new(&"Ж".repeat(65), false).is_err());
        assert!(McpPayload::new(&"Ж".repeat(129), McpStatus::Missing).is_err());
        assert!(RecallPayload::new(0, false, 0, false, false)
            .with_selection_reasons(vec![SelectionReason::FtsBm25; 129])
            .is_err());

        let event = empty_collector_event();
        assert!(event.clone().with_duration_ms(u64::MAX).is_err());
        assert!(event.clone().with_bundle_id("NOT-A-UUID").is_err());
        assert!(event.with_error_code("bad code").is_err());
    }

    #[test]
    fn privacy_redaction_covers_quoted_and_inline_secret_assignments() {
        let json = r#"{"Authorization":"Basic auth-secret","Proxy-Authorization":"Basic proxy-secret","Cookie":"sid=cookie-secret","Set-Cookie":"sid=set-cookie-secret","password":"password-secret","passwd":"passwd-secret","token":"token-secret","refresh_token":"refresh-secret","authToken":"auth-token-secret","sessionToken":"session-token-secret","clientSecret":"camel-client-secret","apiSecret":"camel-api-secret","sessionCookie":"camel-session-cookie","setCookie":"camel-set-cookie","accessKey":"camel-access-key","privateKey":"private-key-secret","credentials":"credential-secret","secret":"generic-secret","api_key":"api-underscore-secret","api-key":"api-hyphen-secret","apikey":"api-plain-secret","client_secret":"client-secret-value","access_key":"access-key-secret","bearer":"bearer-secret","tokenizer":"обычный токенизатор","secretary":"секретарь","public":"Привет, мир"}"#;
        let redacted_json = redact_sensitive_line(json);
        let parsed: serde_json::Value =
            serde_json::from_str(&redacted_json).expect("redacted JSON should stay valid");
        for key in [
            "Authorization",
            "Proxy-Authorization",
            "Cookie",
            "Set-Cookie",
            "password",
            "passwd",
            "token",
            "refresh_token",
            "authToken",
            "sessionToken",
            "clientSecret",
            "apiSecret",
            "sessionCookie",
            "setCookie",
            "accessKey",
            "privateKey",
            "credentials",
            "secret",
            "api_key",
            "api-key",
            "apikey",
            "client_secret",
            "access_key",
            "bearer",
        ] {
            assert_eq!(parsed[key], REDACTED, "field was not redacted: {key}");
        }
        assert_eq!(parsed["tokenizer"], "обычный токенизатор");
        assert_eq!(parsed["secretary"], "секретарь");
        assert_eq!(parsed["public"], "Привет, мир");

        let text = concat!(
            r#"{\"token\":\"escaped-prefix\\\"inner\\\"-escaped-suffix\",\"public\":\"keep-escaped-unicode-Ж\"}"#,
            "\n",
            "{'api_key':'single-quoted-secret','public':'keep-single'}\n",
            "prefix Authorization: Basic inline-auth-secret\n",
            "prefix Proxy-Authorization=Bearer proxy-auth-secret\n",
            "payload Cookie: sid=inline-cookie-secret; tail=cookie-tail-secret\n",
            "payload Set-Cookie=session=inline-set-cookie-secret; HttpOnly\n",
            "TOKEN = MixedCaseSecret public=keep-inline\n",
            "authToken=camel-auth-secret public=keep-auth\n",
            "sessionToken=camel-session-secret public=keep-session\n",
            "?access_token=url-secret&ok=1\n",
            "--token cli-token-secret --public keep-cli-public\n",
            "--password 'cli password secret'\n",
            "--secret cli-secret\n",
            "--cookie cli-cookie-secret\n",
            "--credential cli-credential-secret\n",
            "-----BEGIN RSA PRIVATE KEY-----\n",
            "PEM-PRIVATE-BODY-SECRET\n",
            "-----END RSA PRIVATE KEY-----\n",
            "Bearer standalone-bearer-secret\n",
            "token budget is benign"
        );
        let first = SafeText::new("summary", text, 4_096).expect("text should redact");
        let second = SafeText::new("summary", text, 4_096).expect("text should redact");
        assert_eq!(first, second);
        for secret in [
            "escaped-prefix",
            "inner",
            "escaped-suffix",
            "single-quoted-secret",
            "inline-auth-secret",
            "proxy-auth-secret",
            "inline-cookie-secret",
            "cookie-tail-secret",
            "inline-set-cookie-secret",
            "MixedCaseSecret",
            "camel-auth-secret",
            "camel-session-secret",
            "url-secret",
            "cli-token-secret",
            "cli password secret",
            "cli-secret",
            "cli-cookie-secret",
            "cli-credential-secret",
            "PEM-PRIVATE-BODY-SECRET",
            "BEGIN RSA PRIVATE KEY",
            "END RSA PRIVATE KEY",
            "standalone-bearer-secret",
        ] {
            assert!(!first.0.contains(secret), "secret survived: {secret}");
        }
        for public in [
            "keep-escaped-unicode-Ж",
            "keep-single",
            "keep-inline",
            "keep-auth",
            "keep-session",
            "ok=1",
            "--public keep-cli-public",
            "token budget is benign",
        ] {
            assert!(first.0.contains(public), "public text was lost: {public}");
        }
        assert!(first.0.matches(REDACTED).count() >= 11);
    }

    #[test]
    fn privacy_redaction_covers_uri_userinfo_and_bounded_vendor_tokens() {
        let text = concat!(
            "registry=https://build-user:uri-password@packages.example.com/v1?public=Привет\n",
            "postgresql://юзер:p%40ss:word@db.example:5432/app\n",
            "safe=https://example.com/путь?name=Привет\n",
            "public-user=https://reader@example.com/docs\n",
            "gitlab glpat-1234567890abcdef\n",
            "stripe-live sk_live_1234567890abcdef\n",
            "stripe-test sk_test_1234567890abcdef\n",
            "no-host postgres://audit-user:NOHOST_URI_CANARY_SECRET@/app?label=Привет\n",
            "truncated custom://truncated-user:TRUNCATED_URI_CANARY_SECRET@ tail=видимый\n",
            "benign glpattern-safe sk_lively_public sk_testimony_public"
        );

        let first = SafeText::new("summary", text, 4_096).expect("text should redact");
        let second = SafeText::new("summary", text, 4_096).expect("text should redact");
        assert_eq!(first, second);
        for secret in [
            "uri-password",
            "p%40ss:word",
            "glpat-1234567890abcdef",
            "sk_live_1234567890abcdef",
            "sk_test_1234567890abcdef",
            "NOHOST_URI_CANARY_SECRET",
            "TRUNCATED_URI_CANARY_SECRET",
        ] {
            assert!(!first.0.contains(secret), "secret survived: {secret}");
        }
        for public in [
            "https://build-user:[REDACTED]@packages.example.com/v1?public=Привет",
            "postgresql://юзер:[REDACTED]@db.example:5432/app",
            "safe=https://example.com/путь?name=Привет",
            "public-user=https://reader@example.com/docs",
            "no-host postgres://audit-user:[REDACTED]@/app?label=Привет",
            "truncated custom://truncated-user:[REDACTED]@ tail=видимый",
            "benign glpattern-safe sk_lively_public sk_testimony_public",
        ] {
            assert!(first.0.contains(public), "public text was lost: {public}");
        }
    }

    #[test]
    fn collector_failure_warns_once_and_never_changes_core_result() {
        let workspace = TestWorkspace::new("collector-warning-once");
        fs::write(workspace.paths.observability(), b"not a directory")
            .expect("blocking fixture should write");
        let mut collector =
            LocalCollector::new(&workspace.paths, "doctor").expect("collector should remain lazy");
        let core_success: Result<u32, &str> = Ok(7);
        let core_error: Result<u32, &str> = Err("core failure");

        let warning = collector
            .record(&empty_collector_event())
            .expect("first collector failure should warn");
        assert_eq!(warning.code, OBSERVABILITY_WRITE_FAILED);
        assert_eq!(
            warning.message,
            "local observability write failed; core command result is unchanged"
        );
        assert_eq!(core_success, Ok(7));
        assert_eq!(core_error, Err("core failure"));
        assert_eq!(collector.record(&empty_collector_event()), None);
        assert_eq!(
            serde_json::to_value(&warning).unwrap()["code"],
            OBSERVABILITY_WRITE_FAILED
        );
        let compatible: crate::mutation::MutationWarning = warning;
        assert_eq!(compatible.code, OBSERVABILITY_WRITE_FAILED);
    }

    #[test]
    fn typed_input_failure_warns_once_without_opening_the_store() {
        let workspace = TestWorkspace::new("collector-input-failure");
        let mut collector =
            LocalCollector::new(&workspace.paths, "tool_run").expect("collector should construct");
        let core_error: Result<(), &str> = Err("tool validation failed");

        let warning = collector
            .record_result(Err(CollectorInputError::PayloadTooLarge))
            .expect("first typed input failure should warn");
        assert_eq!(warning.code, OBSERVABILITY_WRITE_FAILED);
        assert_eq!(core_error, Err("tool validation failed"));
        assert!(!workspace.paths.observability().exists());
        assert_eq!(collector.record_result(Ok(empty_collector_event())), None);
        assert!(!workspace.paths.observability().exists());
    }

    #[test]
    fn corrupt_store_and_insert_failure_are_best_effort() {
        {
            let workspace = TestWorkspace::new("collector-corrupt");
            fs::create_dir(workspace.paths.observability())
                .expect("observability directory should create");
            fs::write(workspace.paths.observability_db(), b"garbage")
                .expect("corrupt fixture should write");
            let before = fs::read(workspace.paths.observability_db()).unwrap();
            let mut collector = LocalCollector::new(&workspace.paths, "verify")
                .expect("collector should construct");
            assert!(collector.record(&empty_collector_event()).is_some());
            assert_eq!(
                fs::read(workspace.paths.observability_db()).unwrap(),
                before
            );
        }
        {
            let workspace = TestWorkspace::new("collector-insert-failure");
            let mut collector = LocalCollector::new(&workspace.paths, "verify")
                .expect("collector should construct");
            assert_eq!(collector.record(&empty_collector_event()), None);
            collector
                .writer
                .as_ref()
                .expect("writer should exist")
                .connection
                .execute_batch(
                    "CREATE TRIGGER injected_event_failure
                     BEFORE INSERT ON observability_events
                     BEGIN SELECT RAISE(FAIL, 'injected'); END;",
                )
                .expect("failure trigger should create");
            assert!(collector.record(&empty_collector_event()).is_some());
            assert_eq!(collector.record(&empty_collector_event()), None);
        }
    }

    #[test]
    fn payload_shapes_have_no_raw_capture_fields() {
        let payloads = [
            EventPayload::Node(NodePayload::new(1, "rule", "Title", None, None).unwrap()),
            EventPayload::Link(LinkPayload::new(1, 2, "supports").unwrap()),
            EventPayload::Recall(RecallPayload::new(2, true, 1, true, true)),
            EventPayload::Tool(ToolPayload::new("tool-one", false).unwrap()),
            EventPayload::Mcp(McpPayload::new("mcp-one", McpStatus::Missing).unwrap()),
            EventPayload::Artifact(ArtifactPayload::new("artifacts/out.txt", 7).unwrap()),
            EventPayload::Counts(
                CountsPayload::new(vec![CountItem::new("nodes", 3).unwrap()]).unwrap(),
            ),
        ];
        for payload in payloads {
            let json = serde_json::to_string(&payload).expect("payload should serialize");
            assert!(json.len() <= MAX_EVENT_PAYLOAD_BYTES);
            for forbidden in [
                "raw_chat",
                "task_text",
                "node_body",
                "stdout",
                "stderr",
                "environment",
                "headers",
                "cookies",
                "tokens",
            ] {
                assert!(
                    !json.contains(forbidden),
                    "forbidden payload field: {forbidden}"
                );
            }
        }
    }

    #[test]
    fn retention_age_updates_monotonic_state_and_preserves_non_observability_data() {
        let workspace = TestWorkspace::new("retention-age");
        let mut writer = open_writer(&workspace.paths).expect("writer should initialize");

        let operational = storage::open_workspace_db(&workspace.paths)
            .expect("operational database should initialize");
        let snapshot = crate::audit::write_sql_snapshot(workspace.paths.audit_git(), &operational)
            .expect("operational snapshot should write");
        drop(operational);
        let operational_before =
            fs::read(workspace.paths.db()).expect("operational DB should read");
        let snapshot_before = fs::read(&snapshot.path).expect("snapshot should read");

        let preserved_paths = [
            workspace.paths.tools().join("keep.tool"),
            workspace.paths.artifacts().join("keep.artifact"),
            workspace.paths.logs().join("keep.log"),
            workspace.paths.runtimes().join("keep.runtime"),
            workspace.paths.root().join("exports").join("keep.zip"),
            workspace.paths.observability().join("export.zip"),
            workspace.home.join("skills").join("keep.skill"),
            workspace.home.join("templates").join("keep.template"),
        ];
        for (index, path) in preserved_paths.iter().enumerate() {
            fs::create_dir_all(path.parent().expect("sentinel should have a parent"))
                .expect("sentinel parent should create");
            fs::write(path, format!("preserve-{index}")).expect("sentinel should write");
        }
        let preserved_before = preserved_paths
            .iter()
            .map(|path| fs::read(path).expect("sentinel should read"))
            .collect::<Vec<_>>();

        insert_event_at(
            &writer.connection,
            "age-old-one",
            "2000-01-02T00:00:00Z",
            "{}",
        );
        insert_event_at(
            &writer.connection,
            "age-current",
            "9999-01-01T00:00:00Z",
            "{}",
        );
        writer
            .apply_retention(RetentionPolicy::default())
            .expect("age retention should succeed");

        let old_count: i64 = writer
            .connection
            .query_row(
                "SELECT COUNT(*) FROM observability_events WHERE id = 'age-old-one'",
                [],
                |row| row.get(0),
            )
            .expect("old event count should read");
        let current_count: i64 = writer
            .connection
            .query_row(
                "SELECT COUNT(*) FROM observability_events WHERE id = 'age-current'",
                [],
                |row| row.get(0),
            )
            .expect("current event count should read");
        assert_eq!(old_count, 0);
        assert_eq!(current_count, 1);
        let first_state: (Option<String>, Option<String>) = writer
            .connection
            .query_row(
                "SELECT last_retention_at, retention_floor_at FROM collector_state",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("collector state should read");
        assert!(first_state.0.is_some());
        assert_eq!(first_state.1.as_deref(), Some("2000-01-02T00:00:00Z"));

        insert_event_at(
            &writer.connection,
            "age-even-older",
            "1999-01-01T00:00:00Z",
            "{}",
        );
        writer
            .apply_retention(RetentionPolicy::default())
            .expect("second age retention should succeed");
        let second_floor: Option<String> = writer
            .connection
            .query_row(
                "SELECT retention_floor_at FROM collector_state",
                [],
                |row| row.get(0),
            )
            .expect("retention floor should read");
        assert_eq!(
            second_floor, first_state.1,
            "retention floor moved backward"
        );

        assert_eq!(
            fs::read(workspace.paths.db()).expect("operational DB should read"),
            operational_before,
            "retention changed the operational database"
        );
        assert_eq!(
            fs::read(&snapshot.path).expect("snapshot should read"),
            snapshot_before,
            "retention changed the operational SQL snapshot"
        );
        for (index, path) in preserved_paths.iter().enumerate() {
            assert_eq!(
                fs::read(path).expect("sentinel should remain"),
                preserved_before[index],
                "retention changed {}",
                path.display()
            );
        }
    }

    #[test]
    fn retention_keeps_feedback_parent_then_deletes_bundle_and_cascades_nodes() {
        let workspace = TestWorkspace::new("retention-cascade");
        let mut writer = open_writer(&workspace.paths).expect("writer should initialize");
        insert_bundle_at(&writer.connection, "old-bundle", "2000-01-01T00:00:00Z");
        writer
            .connection
            .execute(
                "INSERT INTO bundle_nodes (
                    bundle_id, node_id, first_seen_at, node_type, node_title,
                    selection_reasons_json
                 ) VALUES ('old-bundle', 1, '2000-01-01T00:00:00Z',
                    'workflow', 'Old workflow', '[\"mandatory\"]')",
                [],
            )
            .expect("bundle node should insert");
        writer
            .connection
            .execute(
                "INSERT INTO feedback (id, timestamp, bundle_id, outcome, reason)
                 VALUES ('recent-feedback', '9999-01-01T00:00:00Z',
                    'old-bundle', 'useful', 'bounded reason')",
                [],
            )
            .expect("recent feedback should insert");

        writer
            .apply_retention(RetentionPolicy::default())
            .expect("retention should preserve a bundle referenced by recent feedback");
        let retained: (i64, i64, i64) = writer
            .connection
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM recall_bundles),
                    (SELECT COUNT(*) FROM bundle_nodes),
                    (SELECT COUNT(*) FROM feedback)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("retained counts should read");
        assert_eq!(retained, (1, 1, 1));

        writer
            .connection
            .execute(
                "UPDATE feedback
                 SET timestamp = '2000-01-03T00:00:00Z'
                 WHERE id = 'recent-feedback'",
                [],
            )
            .expect("feedback timestamp should update");
        writer
            .apply_retention(RetentionPolicy::default())
            .expect("old feedback then bundle retention should succeed");
        let deleted: (i64, i64, i64) = writer
            .connection
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM recall_bundles),
                    (SELECT COUNT(*) FROM bundle_nodes),
                    (SELECT COUNT(*) FROM feedback)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("deleted counts should read");
        assert_eq!(deleted, (0, 0, 0));
    }

    #[test]
    fn size_retention_deletes_oldest_and_reclaims_more_than_one_page_batch() {
        const FIXTURE_EVENTS: usize = 24;
        let workspace = TestWorkspace::new("retention-size");
        let writer = open_writer(&workspace.paths).expect("writer should initialize");
        checkpoint(&writer.connection);
        let base_bytes = physical_store_bytes(workspace.paths.observability_db())
            .expect("base physical size should read");
        let page_size: u64 = writer
            .connection
            .query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
            .expect("page size should read")
            .try_into()
            .expect("page size should be positive");
        let payload = format!(r#"{{"padding":"{}"}}"#, "x".repeat(15_000));
        assert!(payload.len() <= MAX_EVENT_PAYLOAD_BYTES);
        assert!(payload.len() as u64 > page_size * 2);
        for index in 0..FIXTURE_EVENTS {
            insert_event_at(
                &writer.connection,
                &format!("size-{index:02}"),
                &format!("2020-01-01T00:00:{index:02}Z"),
                &payload,
            );
        }
        checkpoint(&writer.connection);
        let expanded_bytes = physical_store_bytes(workspace.paths.observability_db())
            .expect("expanded physical size should read");
        let growth = expanded_bytes
            .checked_sub(base_bytes)
            .expect("fixture should grow the physical store");
        assert!(
            growth > page_size * 16,
            "fixture did not allocate enough pages"
        );
        let max_bytes = base_bytes
            .checked_add(growth / 2)
            .expect("test size limit should fit");
        drop(writer);

        let mut collector =
            LocalCollector::new(&workspace.paths, "doctor").expect("collector should construct");
        collector.set_retention_policy(RetentionPolicy {
            max_age_days: 36_500,
            max_bytes,
            batch_size: 1,
        });
        assert_eq!(
            collector.record(&empty_collector_event()),
            None,
            "multi-page incremental vacuum must not produce a false warning"
        );
        let writer = collector
            .writer
            .as_ref()
            .expect("successful collector should retain its writer");
        let physical_bytes = physical_store_bytes(workspace.paths.observability_db())
            .expect("retained physical size should read");
        assert!(
            physical_bytes <= max_bytes,
            "physical store {physical_bytes} exceeds cap {max_bytes}"
        );
        let mut statement = writer
            .connection
            .prepare(
                "SELECT id FROM observability_events
                 WHERE id LIKE 'size-%'
                 ORDER BY timestamp, id",
            )
            .expect("remaining fixture statement should prepare");
        let remaining = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("remaining fixture ids should query")
            .map(|row| {
                row.expect("remaining fixture id should read")
                    .strip_prefix("size-")
                    .expect("fixture id should have prefix")
                    .parse::<usize>()
                    .expect("fixture id suffix should parse")
            })
            .collect::<Vec<_>>();
        assert!(
            !remaining.is_empty(),
            "size retention deleted every fixture"
        );
        assert!(
            remaining.len() < FIXTURE_EVENTS,
            "size retention deleted no fixture"
        );
        let expected = (remaining[0]..FIXTURE_EVENTS).collect::<Vec<_>>();
        assert_eq!(remaining, expected, "size retention was not oldest-first");
    }

    #[test]
    fn retention_failure_keeps_inserted_event_and_disables_later_writes() {
        let workspace = TestWorkspace::new("retention-failure");
        let mut collector =
            LocalCollector::new(&workspace.paths, "doctor").expect("collector should construct");
        assert_eq!(collector.record(&empty_collector_event()), None);
        let writer = collector
            .writer
            .as_ref()
            .expect("first record should create writer");
        insert_event_at(
            &writer.connection,
            "retention-trigger-old",
            "2000-01-01T00:00:00Z",
            "{}",
        );
        writer
            .connection
            .execute_batch(
                "CREATE TRIGGER injected_retention_failure
                 BEFORE DELETE ON observability_events
                 BEGIN SELECT RAISE(FAIL, 'injected retention failure'); END;",
            )
            .expect("retention failure trigger should create");
        collector.set_retention_policy(RetentionPolicy::default());

        let warning = collector
            .record(&empty_collector_event())
            .expect("retention failure should warn");
        assert_eq!(warning.code, OBSERVABILITY_WRITE_FAILED);
        assert!(!warning
            .message
            .contains(&workspace.home.to_string_lossy()[..]));
        let connection = Connection::open(workspace.paths.observability_db())
            .expect("fixture database should reopen directly");
        let after_failure: i64 = connection
            .query_row("SELECT COUNT(*) FROM observability_events", [], |row| {
                row.get(0)
            })
            .expect("event count should read");
        assert_eq!(
            after_failure, 3,
            "inserted event should survive retention failure"
        );
        assert_eq!(collector.record(&empty_collector_event()), None);
        let after_disabled: i64 = connection
            .query_row("SELECT COUNT(*) FROM observability_events", [], |row| {
                row.get(0)
            })
            .expect("disabled event count should read");
        assert_eq!(after_disabled, after_failure);
    }

    #[test]
    fn initializes_exact_v1_schema_pragmas_columns_and_indexes() {
        let workspace = TestWorkspace::new("exact-schema");
        let writer = open_writer(&workspace.paths).expect("writer should initialize");

        assert_eq!(writer.schema_version().expect("version should read"), 1);
        assert_eq!(writer.path(), workspace.paths.observability_db());
        assert_eq!(
            writer
                .connection
                .query_row::<i64, _, _>("PRAGMA application_id", [], |row| row.get(0))
                .expect("application id should read"),
            OBSERVABILITY_APPLICATION_ID
        );
        assert_eq!(
            writer
                .connection
                .query_row::<i64, _, _>("PRAGMA auto_vacuum", [], |row| row.get(0))
                .expect("auto-vacuum should read"),
            2
        );
        let journal_mode: String = writer
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode should read");
        assert_eq!(journal_mode, "wal");
        assert_eq!(
            read_schema_objects(&writer.connection).expect("schema should read"),
            expected_schema_objects()
        );
        assert_eq!(
            read_internal_schema_objects(&writer.connection).expect("internal schema should read"),
            expected_internal_schema_objects()
        );
        assert_eq!(
            schema_columns(&writer.connection, "observability_events"),
            [
                "id",
                "timestamp",
                "product_version",
                "workspace_key",
                "event_type",
                "command",
                "correlation_id",
                "bundle_id",
                "duration_ms",
                "outcome",
                "error_code",
                "payload_json",
            ]
        );
        assert_eq!(
            schema_columns(&writer.connection, "collector_state"),
            [
                "singleton_id",
                "schema_version",
                "last_retention_at",
                "retention_floor_at",
                "last_error_code",
            ]
        );
        let state: (i64, i64) = writer
            .connection
            .query_row(
                "SELECT singleton_id, schema_version FROM collector_state",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("collector state should read");
        assert_eq!(state, (1, 1));
    }

    #[test]
    fn zero_byte_store_initializes() {
        let workspace = TestWorkspace::new("zero-byte");
        fs::create_dir(workspace.paths.observability())
            .expect("observability directory should create");
        fs::File::create(workspace.paths.observability_db())
            .expect("zero-byte database should create");

        let writer = open_writer(&workspace.paths).expect("zero-byte store should initialize");
        assert_eq!(writer.schema_version().expect("version should read"), 1);
        assert!(
            fs::metadata(workspace.paths.observability_db())
                .expect("database metadata should read")
                .len()
                > 0
        );
    }

    #[test]
    fn valid_empty_v0_store_initializes() {
        let workspace = TestWorkspace::new("empty-v0");
        fs::create_dir(workspace.paths.observability())
            .expect("observability directory should create");
        let connection = Connection::open(workspace.paths.observability_db())
            .expect("empty SQLite database should create");
        connection
            .execute_batch("VACUUM;")
            .expect("empty database header should persist");
        drop(connection);
        assert!(
            fs::metadata(workspace.paths.observability_db())
                .expect("empty database metadata should read")
                .len()
                > 0
        );

        let writer = open_writer(&workspace.paths).expect("empty v0 store should initialize");
        assert_eq!(writer.schema_version().expect("version should read"), 1);
    }

    #[test]
    fn writer_open_is_idempotent_and_preserves_rows() {
        let workspace = TestWorkspace::new("writer-idempotent");
        let first = open_writer(&workspace.paths).expect("first writer should initialize");
        insert_event(&first.connection, "event-idempotent");
        drop(first);
        let before = database_snapshot(workspace.paths.observability_db());

        let second = open_writer(&workspace.paths).expect("second writer should open");
        let count: i64 = second
            .connection
            .query_row("SELECT COUNT(*) FROM observability_events", [], |row| {
                row.get(0)
            })
            .expect("event count should read");
        assert_eq!(count, 1);
        drop(second);
        assert_eq!(
            fs::read(workspace.paths.observability_db()).unwrap(),
            before.0
        );
    }

    #[test]
    fn missing_reader_creates_no_directory_database_or_sidecars() {
        let workspace = TestWorkspace::new("missing-reader");
        let root_before = directory_manifest(workspace.paths.root());
        let error = open_reader(&workspace.paths)
            .err()
            .expect("missing reader should fail");
        assert!(matches!(error, ObservabilityOpenError::Missing(_)));
        assert_eq!(directory_manifest(workspace.paths.root()), root_before);
        assert!(!workspace.paths.observability().exists());

        fs::create_dir(workspace.paths.observability())
            .expect("empty observability directory should create");
        let before = directory_manifest(workspace.paths.observability());
        assert!(open_reader(&workspace.paths).is_err());
        assert_eq!(directory_manifest(workspace.paths.observability()), before);
        for path in managed_database_paths(workspace.paths.observability_db()) {
            assert!(!path.exists(), "reader created {}", path.display());
        }
    }

    #[test]
    fn reader_is_read_only_and_query_only() {
        let workspace = create_initialized_workspace("reader-readonly");
        assert!(!with_database_suffix(workspace.paths.observability_db(), "-wal").exists());
        assert!(!with_database_suffix(workspace.paths.observability_db(), "-shm").exists());
        let database_before = database_snapshot(workspace.paths.observability_db());
        let reader = open_reader(&workspace.paths).expect("reader should open");
        assert_eq!(reader.schema_version().expect("version should read"), 1);
        assert_eq!(reader.path(), workspace.paths.observability_db());
        assert_eq!(
            reader
                .connection
                .query_row::<i64, _, _>("PRAGMA query_only", [], |row| row.get(0))
                .expect("query_only should read"),
            1
        );
        assert!(reader
            .connection
            .execute("CREATE TABLE forbidden_write (id INTEGER)", [])
            .is_err());
        let event_count: i64 = reader
            .connection
            .query_row("SELECT COUNT(*) FROM observability_events", [], |row| {
                row.get(0)
            })
            .expect("event count should read");
        assert_eq!(event_count, 0);
        drop(reader);
        assert_eq!(
            database_snapshot(workspace.paths.observability_db()),
            database_before,
            "read-only open changed observability database"
        );
    }

    #[test]
    fn wal_reader_sees_committed_rows_and_store_is_separate() {
        let workspace = TestWorkspace::new("wal-separate");
        let writer = open_writer(&workspace.paths).expect("writer should initialize");
        insert_event(&writer.connection, "event-visible");
        let reader = open_reader(&workspace.paths).expect("reader should open during WAL writer");
        let count: i64 = reader
            .connection
            .query_row("SELECT COUNT(*) FROM observability_events", [], |row| {
                row.get(0)
            })
            .expect("reader should see committed event");
        assert_eq!(count, 1);
        assert!(
            writer
                .connection
                .query_row::<i64, _, _>(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'nodes'",
                    [],
                    |row| row.get(0)
                )
                .expect("observability schema should inspect")
                == 0
        );

        let operational = storage::open_workspace_db(&workspace.paths)
            .expect("operational database should open separately");
        let observability_tables: i64 = operational
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE name IN ('observability_events', 'recall_bundles',
                    'bundle_nodes', 'feedback', 'collector_state')",
                [],
                |row| row.get(0),
            )
            .expect("operational schema should inspect");
        assert_eq!(observability_tables, 0);
    }

    #[test]
    fn operational_snapshot_excludes_observability_store() {
        let workspace = TestWorkspace::new("snapshot-exclusion");
        drop(open_writer(&workspace.paths).expect("observability store should initialize"));
        let operational =
            storage::open_workspace_db(&workspace.paths).expect("operational database should open");
        let report = crate::audit::write_sql_snapshot(workspace.paths.audit_git(), &operational)
            .expect("operational snapshot should write");
        let snapshot = fs::read_to_string(report.path).expect("snapshot should read");

        assert!(snapshot.contains("CREATE TABLE nodes"));
        assert!(!snapshot.contains("observability_events"));
        assert!(!snapshot.contains("recall_bundles"));
        assert!(!snapshot.contains("collector_state"));
        assert!(!snapshot.contains("observability.sqlite"));
    }

    #[test]
    fn incompatible_application_id_and_versions_are_rejected_unchanged() {
        for (name, pragma) in [
            ("wrong-app", "PRAGMA application_id = 123456;"),
            ("wrong-version", "PRAGMA user_version = 0;"),
            ("future-version", "PRAGMA user_version = 2;"),
        ] {
            let workspace = create_initialized_workspace(name);
            with_test_connection(&workspace, |connection| {
                connection
                    .execute_batch(pragma)
                    .expect("header fixture should mutate");
            });
            assert_writer_rejected_without_main_file_mutation(&workspace);
        }
    }

    fn remove_required_table(connection: &Connection) {
        connection
            .execute_batch("DROP TABLE feedback;")
            .expect("required table should drop");
    }

    fn add_extra_object(connection: &Connection) {
        connection
            .execute_batch("CREATE VIEW unexpected_view AS SELECT 1 AS value;")
            .expect("extra view should create");
    }

    fn add_wrong_column(connection: &Connection) {
        connection
            .execute_batch("ALTER TABLE collector_state ADD COLUMN unexpected TEXT;")
            .expect("wrong column should add");
    }

    fn replace_index_definition(connection: &Connection) {
        connection
            .execute_batch(
                "DROP INDEX idx_feedback_outcome;
                 CREATE INDEX idx_feedback_outcome ON feedback(outcome, bundle_id);",
            )
            .expect("wrong index should replace expected index");
    }

    fn change_check_literal_case(connection: &Connection) {
        connection
            .execute_batch("PRAGMA writable_schema = ON;")
            .expect("writable_schema should enable for fixture");
        connection
            .execute(
                "UPDATE sqlite_schema
                 SET sql = replace(sql, '''useful''', '''USEFUL''')
                 WHERE type = 'table' AND name = 'feedback'",
                [],
            )
            .expect("CHECK literal fixture should change");
        connection
            .execute_batch(
                "PRAGMA writable_schema = OFF;
                 PRAGMA schema_version = 999;",
            )
            .expect("fixture schema cache should invalidate");
    }

    fn add_reserved_internal_object(connection: &Connection) {
        connection
            .execute_batch("PRAGMA writable_schema = ON;")
            .expect("writable_schema should enable for fixture");
        connection
            .execute(
                "INSERT INTO sqlite_schema (type, name, tbl_name, rootpage, sql)
                 VALUES ('view', 'sqlite_evil', 'sqlite_evil', 0,
                    'CREATE VIEW sqlite_evil AS SELECT 1')",
                [],
            )
            .expect("reserved internal fixture should insert");
        connection
            .execute_batch(
                "PRAGMA writable_schema = OFF;
                 PRAGMA schema_version = 1000;",
            )
            .expect("fixture schema cache should invalidate");
    }

    #[test]
    fn missing_extra_column_index_and_check_drift_are_rejected_unchanged() {
        type SchemaTamper = fn(&Connection);
        let cases: [(&str, SchemaTamper); 6] = [
            ("missing-table", remove_required_table),
            ("extra-object", add_extra_object),
            ("wrong-column", add_wrong_column),
            ("wrong-index", replace_index_definition),
            ("changed-check", change_check_literal_case),
            ("reserved-internal", add_reserved_internal_object),
        ];
        for (name, tamper) in cases {
            let workspace = create_initialized_workspace(name);
            with_test_connection(&workspace, tamper);
            assert_writer_rejected_without_main_file_mutation(&workspace);
        }
    }

    #[test]
    fn unsupported_empty_v0_pragma_states_are_rejected_unchanged() {
        assert!(valid_empty_v0_pragmas(0, "delete"));
        assert!(valid_empty_v0_pragmas(2, "DELETE"));
        assert!(valid_empty_v0_pragmas(2, "wal"));
        for (auto_vacuum, journal_mode) in [
            (0, "wal"),
            (1, "delete"),
            (0, "full"),
            (0, "off"),
            (0, "memory"),
            (0, "truncate"),
        ] {
            assert!(!valid_empty_v0_pragmas(auto_vacuum, journal_mode));
        }

        let workspace = TestWorkspace::new("empty-v0-wal-without-auto-vacuum");
        fs::create_dir(workspace.paths.observability())
            .expect("observability directory should create");
        let connection = Connection::open(workspace.paths.observability_db())
            .expect("empty SQLite database should create");
        let mode: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .expect("WAL fixture should enable");
        assert_eq!(mode, "wal");
        drop(connection);
        assert_writer_rejected_without_main_file_mutation(&workspace);
    }

    #[test]
    fn copied_operational_database_is_rejected_unchanged() {
        let workspace = TestWorkspace::new("operational-copy");
        drop(
            storage::open_workspace_db(&workspace.paths)
                .expect("operational database should initialize"),
        );
        fs::create_dir(workspace.paths.observability())
            .expect("observability directory should create");
        fs::copy(workspace.paths.db(), workspace.paths.observability_db())
            .expect("operational database fixture should copy");

        assert_writer_rejected_without_main_file_mutation(&workspace);
    }

    #[test]
    fn garbage_and_corrupt_stores_are_rejected_unchanged() {
        {
            let workspace = TestWorkspace::new("garbage-store");
            fs::create_dir(workspace.paths.observability())
                .expect("observability directory should create");
            fs::write(workspace.paths.observability_db(), b"not a sqlite database")
                .expect("garbage fixture should write");
            assert_writer_rejected_without_main_file_mutation(&workspace);
        }
        {
            let workspace = create_initialized_workspace("corrupt-store");
            let mut bytes = fs::read(workspace.paths.observability_db())
                .expect("valid database should read for corruption fixture");
            bytes[..16].fill(0xFF);
            fs::write(workspace.paths.observability_db(), bytes)
                .expect("corrupt fixture should write");
            assert_writer_rejected_without_main_file_mutation(&workspace);
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_directory_database_and_sidecar_are_rejected() {
        use std::os::unix::fs::symlink;

        {
            let workspace = TestWorkspace::new("linked-directory");
            let outside = temp_path("outside-directory");
            fs::create_dir(&outside).expect("outside directory should create");
            fs::write(outside.join("sentinel"), b"preserve")
                .expect("outside sentinel should write");
            symlink(&outside, workspace.paths.observability())
                .expect("observability symlink should create");
            assert!(open_writer(&workspace.paths).is_err());
            assert_eq!(
                fs::read(outside.join("sentinel")).expect("sentinel should read"),
                b"preserve"
            );
            assert!(!outside.join("observability.sqlite").exists());
            fs::remove_dir_all(outside).expect("outside directory should remove");
        }
        {
            let workspace = TestWorkspace::new("linked-database");
            fs::create_dir(workspace.paths.observability())
                .expect("observability directory should create");
            let outside = temp_path("outside-database");
            fs::write(&outside, b"preserve").expect("outside database should write");
            symlink(&outside, workspace.paths.observability_db())
                .expect("database symlink should create");
            assert!(open_writer(&workspace.paths).is_err());
            assert_eq!(
                fs::read(&outside).expect("outside file should read"),
                b"preserve"
            );
            fs::remove_file(outside).expect("outside database should remove");
        }
        {
            let workspace = create_initialized_workspace("linked-sidecar");
            let outside = temp_path("outside-sidecar");
            fs::write(&outside, b"preserve").expect("outside sidecar should write");
            let journal = managed_database_paths(workspace.paths.observability_db())[3].clone();
            symlink(&outside, &journal).expect("journal symlink should create");
            assert!(open_reader(&workspace.paths).is_err());
            assert_eq!(
                fs::read(&outside).expect("outside file should read"),
                b"preserve"
            );
            fs::remove_file(outside).expect("outside sidecar should remove");
        }
    }

    #[test]
    fn schema_checks_reject_invalid_boolean_counts_json_and_feedback() {
        let workspace = TestWorkspace::new("schema-checks");
        let writer = open_writer(&workspace.paths).expect("writer should initialize");
        assert!(writer
            .connection
            .execute(
                "INSERT INTO observability_events (
                    id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, outcome, payload_json
                 ) VALUES ('bad-json', 'now', 'v', 'w', 'event', 'command',
                    'correlation', 'success', '[]')",
                []
            )
            .is_err());
        let oversized_payload = format!(r#"{{"value":"{}"}}"#, "x".repeat(16_384));
        assert!(writer
            .connection
            .execute(
                "INSERT INTO observability_events (
                    id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, outcome, payload_json
                 ) VALUES ('oversized-json', 'now', 'v', 'w', 'event', 'command',
                    'correlation', 'success', ?1)",
                [&oversized_payload]
            )
            .is_err());
        assert!(writer
            .connection
            .execute(
                "INSERT INTO recall_bundles (
                    bundle_id, timestamp, product_version, workspace_key,
                    correlation_id, outcome, more_results, continuation_count
                 ) VALUES ('bad-bool', 'now', 'v', 'w', 'c', 'success', 2, 0)",
                []
            )
            .is_err());
        insert_bundle(&writer.connection, "valid-bundle");
        assert!(writer
            .connection
            .execute(
                "INSERT INTO bundle_nodes (
                    bundle_id, node_id, first_seen_at, node_type, node_title,
                    selection_reasons_json
                 ) VALUES ('valid-bundle', 1, 'now', 'workflow', 'Title', '{}')",
                []
            )
            .is_err());
        let oversized_reasons = format!(r#"["{}"]"#, "x".repeat(4096));
        assert!(writer
            .connection
            .execute(
                "INSERT INTO bundle_nodes (
                    bundle_id, node_id, first_seen_at, node_type, node_title,
                    selection_reasons_json
                 ) VALUES ('valid-bundle', 2, 'now', 'workflow', 'Title', ?1)",
                [&oversized_reasons]
            )
            .is_err());
        assert!(writer
            .connection
            .execute(
                "INSERT INTO feedback (id, timestamp, bundle_id, outcome)
                 VALUES ('feedback-1', 'now', 'valid-bundle', 'great')",
                []
            )
            .is_err());
        assert!(writer
            .connection
            .execute(
                "INSERT INTO recall_bundles (
                    bundle_id, timestamp, product_version, workspace_key,
                    correlation_id, outcome, duration_ms,
                    more_results, continuation_count
                 ) VALUES ('negative', 'now', 'v', 'w', 'c', 'success', -1, 0, 0)",
                []
            )
            .is_err());
    }

    fn recall_lifecycle_events() -> Vec<CollectorEvent> {
        vec![
            CollectorEvent::new(
                EventType::RecallStarted,
                EventOutcome::Started,
                EventPayload::Empty,
            )
            .expect("started event should validate"),
            CollectorEvent::new(
                EventType::RecallCompleted,
                EventOutcome::Success,
                EventPayload::Recall(RecallPayload::new(1, true, 0, true, false)),
            )
            .expect("completed event should validate")
            .with_duration_ms(5)
            .expect("duration should validate"),
        ]
    }

    fn recall_failure_events(error_code: &str) -> Vec<CollectorEvent> {
        vec![
            CollectorEvent::new(
                EventType::RecallStarted,
                EventOutcome::Started,
                EventPayload::Empty,
            )
            .expect("started event should validate"),
            CollectorEvent::new(
                EventType::RecallContinuation,
                EventOutcome::Recorded,
                EventPayload::Empty,
            )
            .and_then(|event| event.with_duration_ms(6))
            .expect("continuation event should validate"),
            CollectorEvent::new(
                EventType::RecallFailed,
                EventOutcome::Failure,
                EventPayload::Empty,
            )
            .and_then(|event| event.with_error_code(error_code))
            .and_then(|event| event.with_duration_ms(6))
            .expect("failed event should validate"),
        ]
    }

    fn recall_bundle_node(id: i64, title: &str) -> RecallBundleNode {
        RecallBundleNode::new(
            id,
            "workflow",
            title,
            Some("token=private-summary safe"),
            Some("Authorization: Bearer private-source"),
            Some("high"),
            Some(0.8),
            Some(-2.5),
            vec![SelectionReason::FtsBm25, SelectionReason::Workflow],
        )
        .expect("bundle node should validate")
    }

    #[test]
    fn recall_bundle_upsert_is_atomic_first_seen_and_continuation_safe() {
        const BUNDLE_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
        let workspace = TestWorkspace::new("recall-bundle-upsert");
        let mut first = LocalCollector::new(&workspace.paths, "recall")
            .expect("first collector should construct");
        let first_correlation = first.correlation_id().to_string();
        let first_record = RecallBundleRecord::success(
            BUNDLE_ID,
            5,
            true,
            false,
            vec![recall_bundle_node(1, "First title")],
        )
        .expect("first record should validate");
        assert_eq!(
            first.record_recall_bundle(&first_record, &recall_lifecycle_events()),
            None
        );

        let connection = Connection::open(workspace.paths.observability_db())
            .expect("observability should open");
        let first_seen: (String, String) = connection
            .query_row(
                "SELECT timestamp, correlation_id FROM recall_bundles WHERE bundle_id = ?1",
                [BUNDLE_ID],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("first parent should query");
        drop(connection);

        let mut failed_continuation = LocalCollector::new(&workspace.paths, "recall")
            .expect("failed continuation collector should construct");
        let failed_record = RecallBundleRecord::failure(BUNDLE_ID, 6, "STALE_RECALL_CURSOR", true)
            .expect("failed continuation record should validate");
        assert_eq!(
            failed_continuation.record_recall_bundle(
                &failed_record,
                &recall_failure_events("STALE_RECALL_CURSOR"),
            ),
            None
        );
        let connection = Connection::open(workspace.paths.observability_db())
            .expect("observability should open after failed continuation");
        let failed_parent: (String, String, i64, i64, i64, String, String) = connection
            .query_row(
                "SELECT timestamp, correlation_id, duration_ms, more_results,
                        continuation_count, outcome, error_code
                 FROM recall_bundles WHERE bundle_id = ?1",
                [BUNDLE_ID],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("failed parent should query");
        assert_eq!(
            (&failed_parent.0, &failed_parent.1),
            (&first_seen.0, &first_seen.1)
        );
        assert_eq!(
            (
                failed_parent.2,
                failed_parent.3,
                failed_parent.4,
                failed_parent.5.as_str(),
                failed_parent.6.as_str(),
            ),
            (11, 1, 1, "failure", "STALE_RECALL_CURSOR")
        );
        drop(connection);

        let mut continuation = LocalCollector::new(&workspace.paths, "recall")
            .expect("continuation collector should construct");
        assert_ne!(continuation.correlation_id(), first_correlation);
        let continuation_record = RecallBundleRecord::success(
            BUNDLE_ID,
            7,
            false,
            true,
            vec![
                recall_bundle_node(1, "Changed title must not replace first"),
                recall_bundle_node(2, "Second title"),
            ],
        )
        .expect("continuation record should validate");
        assert_eq!(
            continuation.record_recall_bundle(&continuation_record, &recall_lifecycle_events()),
            None
        );

        let connection = Connection::open(workspace.paths.observability_db())
            .expect("observability should reopen");
        let parent: (String, String, i64, i64, i64, String, Option<String>) = connection
            .query_row(
                "SELECT timestamp, correlation_id, duration_ms, more_results,
                        continuation_count, outcome, error_code
                 FROM recall_bundles WHERE bundle_id = ?1",
                [BUNDLE_ID],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("updated parent should query");
        assert_eq!((&parent.0, &parent.1), (&first_seen.0, &first_seen.1));
        assert_eq!(
            (parent.2, parent.3, parent.4, parent.5.as_str()),
            (18, 0, 2, "success")
        );
        assert_eq!(parent.6, None, "latest success clears the prior error code");
        let nodes: Vec<(i64, String, String, String, String)> = connection
            .prepare(
                "SELECT node_id, node_title, bounded_summary, source_ref,
                        selection_reasons_json
                 FROM bundle_nodes WHERE bundle_id = ?1 ORDER BY node_id",
            )
            .expect("node query should prepare")
            .query_map([BUNDLE_ID], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .expect("node rows should query")
            .collect::<Result<_, _>>()
            .expect("node rows should collect");
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].1, "First title");
        assert!(!nodes[0].2.contains("private-summary"));
        assert!(!nodes[0].3.contains("private-source"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&nodes[0].4)
                .expect("reason json should parse"),
            serde_json::json!(["fts_bm25", "workflow"])
        );
        let event_rows: (i64, i64, i64) = connection
            .query_row(
                "SELECT COUNT(*), COUNT(DISTINCT bundle_id),
                        COUNT(DISTINCT correlation_id)
                 FROM observability_events WHERE event_type LIKE 'recall.%'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("event counts should query");
        assert_eq!(event_rows, (7, 1, 3));
    }

    #[test]
    fn recall_bundle_constraint_failure_rolls_back_parent_nodes_and_events() {
        const BUNDLE_ID: &str = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
        let workspace = TestWorkspace::new("recall-bundle-rollback");
        let mut invalid_node = recall_bundle_node(1, "Valid before injection");
        invalid_node.node_id = -1;
        let record = RecallBundleRecord::success(BUNDLE_ID, 1, false, false, vec![invalid_node])
            .expect("record envelope should validate");
        let mut collector =
            LocalCollector::new(&workspace.paths, "recall").expect("collector should construct");
        let warning = collector
            .record_recall_bundle(&record, &recall_lifecycle_events())
            .expect("constraint failure should emit one warning");
        assert_eq!(warning.code, OBSERVABILITY_WRITE_FAILED);

        let connection =
            Connection::open(workspace.paths.observability_db()).expect("failed store should open");
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM recall_bundles),
                        (SELECT COUNT(*) FROM bundle_nodes),
                        (SELECT COUNT(*) FROM observability_events)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("rollback counts should query");
        assert_eq!(counts, (0, 0, 0));
    }

    #[test]
    fn feedback_is_parent_bound_redacted_atomic_and_missing_store_safe() {
        const BUNDLE_ID: &str = "6ba7b811-9dad-41d1-80b4-00c04fd430c8";
        let missing = TestWorkspace::new("feedback-missing-store");
        let input = FeedbackRecordInput::new(BUNDLE_ID, FeedbackOutcome::Useful, None)
            .expect("feedback input should validate");
        let mut missing_collector = LocalCollector::new(&missing.paths, "feedback_record")
            .expect("missing collector should construct");
        assert_eq!(
            missing_collector.record_feedback(input.clone()),
            Err(FeedbackWriteError::BundleNotFound)
        );
        assert!(!missing.paths.observability_db().exists());
        drop(missing);

        let workspace = TestWorkspace::new("feedback-record");
        let mut recall_collector = LocalCollector::new(&workspace.paths, "recall")
            .expect("recall collector should construct");
        let parent = RecallBundleRecord::success(BUNDLE_ID, 2, false, false, Vec::new())
            .expect("parent should validate");
        assert_eq!(
            recall_collector.record_recall_bundle(&parent, &recall_lifecycle_events()),
            None
        );

        let input = FeedbackRecordInput::new(
            BUNDLE_ID,
            FeedbackOutcome::Partial,
            Some("Authorization: Bearer feedback-secret; useful detail"),
        )
        .expect("redacted feedback should validate");
        let mut collector = LocalCollector::new(&workspace.paths, "feedback_record")
            .expect("feedback collector should construct");
        let correlation = collector.correlation_id().to_string();
        let outcome = collector
            .record_feedback(input)
            .expect("feedback should record");
        assert!(outcome.warning.is_none());
        assert_eq!(outcome.receipt.bundle_id, BUNDLE_ID);
        assert_eq!(outcome.receipt.outcome, FeedbackOutcome::Partial);
        assert!(outcome.receipt.reason_recorded);

        let connection = Connection::open(workspace.paths.observability_db())
            .expect("feedback store should open");
        let feedback: (String, String, String) = connection
            .query_row(
                "SELECT outcome, reason, bundle_id FROM feedback",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("feedback should query");
        assert_eq!(feedback.0, "partial");
        assert_eq!(feedback.2, BUNDLE_ID);
        assert!(!feedback.1.contains("feedback-secret"));
        assert!(feedback.1.contains(REDACTED));
        let event: (String, String, String) = connection
            .query_row(
                "SELECT bundle_id, correlation_id, payload_json
                 FROM observability_events WHERE event_type = 'feedback.recorded'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("feedback event should query");
        assert_eq!(
            (event.0.as_str(), event.1.as_str()),
            (BUNDLE_ID, correlation.as_str())
        );
        assert!(!event.2.contains("feedback-secret"));
    }

    #[test]
    fn feedback_event_failure_rolls_back_and_retention_failure_is_postcommit_warning() {
        const BUNDLE_ID: &str = "6ba7b812-9dad-41d1-80b4-00c04fd430c8";
        let workspace = TestWorkspace::new("feedback-atomic");
        let mut recall_collector = LocalCollector::new(&workspace.paths, "recall")
            .expect("recall collector should construct");
        let parent = RecallBundleRecord::success(BUNDLE_ID, 1, false, false, Vec::new())
            .expect("parent should validate");
        assert_eq!(
            recall_collector.record_recall_bundle(&parent, &recall_lifecycle_events()),
            None
        );

        let mut failing = LocalCollector::new(&workspace.paths, "feedback_record")
            .expect("failing collector should construct");
        let writer = open_existing_writer(&workspace.paths).expect("existing writer should open");
        writer
            .connection
            .execute_batch(
                "CREATE TEMP TRIGGER fail_feedback_event
                 BEFORE INSERT ON main.observability_events
                 WHEN NEW.event_type = 'feedback.recorded'
                 BEGIN SELECT RAISE(ABORT, 'feedback event injection'); END;",
            )
            .expect("temporary failure trigger should create");
        failing.writer = Some(writer);
        let input = FeedbackRecordInput::new(BUNDLE_ID, FeedbackOutcome::Wrong, Some("wrong"))
            .expect("feedback input should validate");
        assert_eq!(
            failing.record_feedback(input),
            Err(FeedbackWriteError::StoreUnavailable)
        );
        let connection = Connection::open(workspace.paths.observability_db())
            .expect("store should open after rollback");
        let feedback_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM feedback", [], |row| row.get(0))
            .expect("feedback count should query");
        assert_eq!(feedback_count, 0);
        drop(connection);

        let mut postcommit = LocalCollector::new(&workspace.paths, "feedback_record")
            .expect("postcommit collector should construct");
        postcommit.set_retention_policy(RetentionPolicy {
            max_age_days: 0,
            ..RetentionPolicy::default()
        });
        let input = FeedbackRecordInput::new(BUNDLE_ID, FeedbackOutcome::Useful, None)
            .expect("postcommit input should validate");
        let outcome = postcommit
            .record_feedback(input)
            .expect("committed feedback remains success");
        assert_eq!(
            outcome.warning.as_ref().map(|warning| warning.code),
            Some(OBSERVABILITY_WRITE_FAILED)
        );
        let connection = Connection::open(workspace.paths.observability_db())
            .expect("postcommit store should open");
        let counts: (i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM feedback),
                        (SELECT COUNT(*) FROM observability_events
                         WHERE event_type = 'feedback.recorded')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("postcommit counts should query");
        assert_eq!(counts, (1, 1));
    }
}
