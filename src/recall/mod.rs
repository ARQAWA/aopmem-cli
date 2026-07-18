use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Write};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::{Uuid, Variant, Version};

use crate::storage::{
    FtsNodeSearchResult, LeastPrivilegeMetadata, Link, Node, SourceHierarchy, TaskRecallCandidates,
};

const RECALL_TRAVERSAL_MAX_DEPTH: usize = 2;
const STRUCTURED_RECALL_SUFFICIENT_NODE_COUNT: usize = 3;
const MAX_HUNCHES: usize = 3;
const MAX_COMPACT_APPLICABLE_WORKFLOWS: usize = 1;
const MAX_COMPACT_SECTION_NODES: usize = 3;
const MAX_COMPACT_SOURCE_REFS: usize = 12;

/// Canonical JSON UTF-8 bytes are the only budget unit for v0.2 recall.
pub const TASK_RECALL_SOFT_BUDGET_BYTES: usize = 256 * 1024;
/// Mandatory context must fit this hard budget or recall fails as a whole.
pub const MANDATORY_RECALL_HARD_BUDGET_BYTES: usize = 1024 * 1024;
/// Recall continuation carries exact deduplication state and therefore has a
/// separate bound from the compact 1024-byte list cursor contract.
pub const MAX_RECALL_CONTINUATION_CURSOR_BYTES: usize = 24 * 1024;
const RECALL_CURSOR_VERSION: u8 = 1;
const RECALL_CURSOR_PREFIX: &str = "v1.recall.";
/// Active node types that can contribute mandatory v0.2 context.
pub const MANDATORY_CONTEXT_NODE_TYPES: &[&str] = &[
    "kernel_contract",
    "gate",
    "project_profile",
    "source",
    "rule",
];

/// A type-safe v0.2 recall request. The enum shape prevents invalid mode
/// combinations from reaching the retrieval pipeline.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RecallRequestV2 {
    Task {
        query: String,
        continuation_cursor: Option<String>,
        limit: Option<usize>,
    },
    Full,
    LegacyCompatibility,
}

/// Stable response contract for v0.2 recall bundles.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecallResponseV2 {
    pub bundle_id: RecallBundleId,
    pub mode: RecallMode,
    pub mandatory: RecallSection,
    pub task: RecallSection,
    pub more_results: bool,
    pub continuation_cursor: Option<String>,
    pub budget: RecallBudgetMetadata,
}

/// Unbounded operational export intended only for local debug, audit,
/// migration, and export proof. It is not a normal task recall bundle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FullRecallResponse {
    pub bundle_id: RecallBundleId,
    pub mode: RecallMode,
    pub debug_only: bool,
    pub nodes: Vec<crate::storage::Node>,
    pub links: Vec<crate::storage::Link>,
    pub aliases: Vec<crate::storage::Alias>,
    pub tags: Vec<crate::storage::Tag>,
    pub sources: Vec<crate::storage::Source>,
    pub events: Vec<crate::audit::Event>,
    pub tool_contracts: Vec<crate::tools::ToolContractRecord>,
    pub mcp_profiles: Vec<crate::storage::McpProfile>,
    pub more_results: bool,
    pub continuation_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallMode {
    Task,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecallSection {
    pub complete: bool,
    pub nodes: Vec<SelectedRecallNode>,
}

/// A complete node plus every typed reason that caused its selection.
/// `Node::body` is deliberately not skipped, including for mandatory context.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SelectedRecallNode {
    pub node: Node,
    pub selection_reasons: Vec<RecallSelectionReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecallSelectionReason {
    MandatoryContext {
        mandatory_type: MandatoryContextType,
    },
    TypedRoot {
        node_type: String,
    },
    FtsBm25 {
        rank: f64,
    },
    DirectLink {
        source_node_id: i64,
        link_type: String,
    },
    GraphTraversal {
        root_node_id: i64,
        root_node_type: String,
        source_node_id: i64,
        link_type: String,
        depth: usize,
    },
    Expansion {
        source_node_id: i64,
        expansion_type: RecallExpansionType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallExpansionType {
    Workflow,
    Tool,
    FailureMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MandatoryContextType {
    KernelContract,
    Gate,
    ProjectProfile,
    Source,
    Rule,
}

impl MandatoryContextType {
    #[must_use]
    pub fn from_node_type(node_type: &str) -> Option<Self> {
        match node_type {
            "kernel_contract" => Some(Self::KernelContract),
            "gate" => Some(Self::Gate),
            "project_profile" => Some(Self::ProjectProfile),
            "source" => Some(Self::Source),
            "rule" => Some(Self::Rule),
            _ => None,
        }
    }
}

#[must_use]
pub fn is_mandatory_context_node_type(node_type: &str) -> bool {
    MandatoryContextType::from_node_type(node_type).is_some()
}

#[must_use]
pub fn is_active_mandatory_context_node(node: &Node) -> bool {
    node.status == "active" && is_mandatory_context_node_type(&node.node_type)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallBudgetUnit {
    CanonicalJsonUtf8Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecallBudgetMetadata {
    pub unit: RecallBudgetUnit,
    pub mandatory: MandatoryBudgetUsage,
    pub task: TaskBudgetUsage,
    pub total_used_bytes: usize,
}

impl RecallBudgetMetadata {
    /// Builds budget metadata without allowing the aggregate counter to wrap.
    pub fn new(
        mandatory_used_bytes: usize,
        task_used_bytes: usize,
    ) -> Result<Self, RecallModelError> {
        Self::with_task_state(mandatory_used_bytes, task_used_bytes, false)
    }

    /// Builds cumulative task-budget metadata for a logical recall bundle.
    pub fn with_task_state(
        mandatory_used_bytes: usize,
        task_used_bytes: usize,
        exhausted: bool,
    ) -> Result<Self, RecallModelError> {
        if task_used_bytes > TASK_RECALL_SOFT_BUDGET_BYTES {
            return Err(RecallModelError::TaskBudgetExceeded {
                used_bytes: task_used_bytes,
            });
        }
        let total_used_bytes = mandatory_used_bytes
            .checked_add(task_used_bytes)
            .ok_or(RecallModelError::ByteCountOverflow)?;
        Ok(Self {
            unit: RecallBudgetUnit::CanonicalJsonUtf8Bytes,
            mandatory: MandatoryBudgetUsage {
                hard_limit_bytes: MANDATORY_RECALL_HARD_BUDGET_BYTES,
                used_bytes: mandatory_used_bytes,
            },
            task: TaskBudgetUsage {
                soft_limit_bytes: TASK_RECALL_SOFT_BUDGET_BYTES,
                used_bytes: task_used_bytes,
                remaining_bytes: TASK_RECALL_SOFT_BUDGET_BYTES - task_used_bytes,
                exhausted: exhausted || task_used_bytes == TASK_RECALL_SOFT_BUDGET_BYTES,
            },
            total_used_bytes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MandatoryBudgetUsage {
    pub hard_limit_bytes: usize,
    pub used_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskBudgetUsage {
    pub soft_limit_bytes: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
    pub exhausted: bool,
}

/// Lowercase, hyphenated RFC 4122 UUID v4 used to correlate one recall bundle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RecallBundleId(String);

impl RecallBundleId {
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().hyphenated().to_string())
    }

    pub fn parse(value: &str) -> Result<Self, RecallBundleIdError> {
        let parsed = Uuid::parse_str(value).map_err(|_| RecallBundleIdError)?;
        let canonical = parsed.hyphenated().to_string();
        if parsed.get_version() != Some(Version::Random)
            || parsed.get_variant() != Variant::RFC4122
            || value != canonical
        {
            return Err(RecallBundleIdError);
        }
        Ok(Self(canonical))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("bundle id must be a lowercase hyphenated UUID v4")]
pub struct RecallBundleIdError;

/// Ordered task-recall layer that the next continuation page must read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallContinuationPhase {
    TypedRoots,
    Fts,
    DirectLinks,
    Graph,
}

/// Minimal root identity needed to expand direct and graph candidates later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecallContinuationRoot {
    pub node_id: i64,
    pub node_type: String,
}

/// Exact, serverless state for one logical recall retrieval.
///
/// It deliberately contains identities and counters only. Query text, node
/// content, titles, environment values, and workspace paths never enter it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecallContinuationState {
    pub version: u8,
    pub bundle_id: String,
    pub query_fingerprint: String,
    pub database_revision: String,
    pub phase: RecallContinuationPhase,
    pub offset: u64,
    pub task_node_bytes: u64,
    pub emitted_count: u64,
    pub seen_node_ids: Vec<i64>,
    pub roots: Vec<RecallContinuationRoot>,
    pub exhausted: bool,
}

impl RecallContinuationState {
    pub fn new(query: &str, database_revision: String) -> Result<Self, RecallCursorError> {
        Self::new_with_bundle_id(query, database_revision, RecallBundleId::generate())
    }

    pub fn new_with_bundle_id(
        query: &str,
        database_revision: String,
        bundle_id: RecallBundleId,
    ) -> Result<Self, RecallCursorError> {
        validate_lowercase_fingerprint(&database_revision)?;
        Ok(Self {
            version: RECALL_CURSOR_VERSION,
            bundle_id: bundle_id.as_str().to_string(),
            query_fingerprint: normalized_query_fingerprint(query),
            database_revision,
            phase: RecallContinuationPhase::TypedRoots,
            offset: 0,
            task_node_bytes: 0,
            emitted_count: 0,
            seen_node_ids: Vec::new(),
            roots: Vec::new(),
            exhausted: false,
        })
    }

    pub fn bundle_id(&self) -> Result<RecallBundleId, RecallCursorError> {
        RecallBundleId::parse(&self.bundle_id).map_err(|_| RecallCursorError::InvalidBundleId)
    }

    #[must_use]
    pub fn contains_node(&self, node_id: i64) -> bool {
        self.seen_node_ids.binary_search(&node_id).is_ok()
    }

    pub fn insert_seen_node(&mut self, node_id: i64) -> Result<(), RecallCursorError> {
        if node_id <= 0 {
            return Err(RecallCursorError::InvalidNodeId);
        }
        match self.seen_node_ids.binary_search(&node_id) {
            Ok(_) => Ok(()),
            Err(index) => {
                self.seen_node_ids.insert(index, node_id);
                Ok(())
            }
        }
    }

    pub fn insert_root(&mut self, node: &Node) -> Result<(), RecallCursorError> {
        if node.id <= 0 {
            return Err(RecallCursorError::InvalidNodeId);
        }
        if self.roots.iter().any(|root| root.node_id == node.id) {
            return Ok(());
        }
        self.roots.push(RecallContinuationRoot {
            node_id: node.id,
            node_type: node.node_type.clone(),
        });
        Ok(())
    }

    pub(crate) fn insert_root_indexed(
        &mut self,
        node: &Node,
        root_ids: &mut HashSet<i64>,
    ) -> Result<(), RecallCursorError> {
        if node.id <= 0 {
            return Err(RecallCursorError::InvalidNodeId);
        }
        if root_ids.insert(node.id) {
            self.roots.push(RecallContinuationRoot {
                node_id: node.id,
                node_type: node.node_type.clone(),
            });
        }
        Ok(())
    }

    pub fn task_used_bytes(&self, complete: bool) -> Result<usize, RecallModelError> {
        let envelope_bytes = empty_task_section_byte_len(complete)?;
        let node_bytes = usize::try_from(self.task_node_bytes)
            .map_err(|_| RecallModelError::ByteCountOverflow)?;
        envelope_bytes
            .checked_add(node_bytes)
            .ok_or(RecallModelError::ByteCountOverflow)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecallCursorError {
    #[error("recall continuation cursor exceeds {MAX_RECALL_CONTINUATION_CURSOR_BYTES} bytes")]
    TooLong,
    #[error("recall continuation cursor has an invalid version or shape")]
    InvalidShape,
    #[error("recall continuation cursor must use canonical URL-safe encoding")]
    NonCanonicalEncoding,
    #[error("recall continuation cursor checksum does not match")]
    ChecksumMismatch,
    #[error("recall continuation cursor payload is not canonical")]
    NonCanonicalPayload,
    #[error("recall continuation cursor does not belong to this normalized query")]
    QueryMismatch,
    #[error("recall continuation cursor contains an invalid bundle id")]
    InvalidBundleId,
    #[error("recall continuation cursor contains an invalid database fingerprint")]
    InvalidDatabaseRevision,
    #[error("recall continuation cursor contains invalid node identity state")]
    InvalidNodeId,
    #[error("recall continuation cursor contains duplicate root state")]
    DuplicateRoot,
    #[error("recall continuation cursor is exhausted")]
    Exhausted,
}

/// Encodes canonical, URL-safe continuation state with an integrity checksum.
pub fn encode_recall_continuation_cursor(
    state: &RecallContinuationState,
) -> Result<String, RecallCursorError> {
    validate_continuation_state(state)?;
    let bundle_id = state.bundle_id()?;
    let bundle_uuid =
        Uuid::parse_str(bundle_id.as_str()).map_err(|_| RecallCursorError::InvalidBundleId)?;
    let query_fingerprint = decode_fingerprint(&state.query_fingerprint)?;
    let database_revision = decode_fingerprint(&state.database_revision)
        .map_err(|_| RecallCursorError::InvalidDatabaseRevision)?;
    let mut payload =
        Vec::with_capacity(64 + state.seen_node_ids.len() * 3 + state.roots.len() * 4);
    payload.push(RECALL_CURSOR_VERSION);
    payload.extend_from_slice(bundle_uuid.as_bytes());
    payload.extend_from_slice(&query_fingerprint);
    payload.extend_from_slice(&database_revision);
    payload.push(continuation_phase_code(state.phase));
    append_varint(&mut payload, state.offset);
    append_varint(&mut payload, state.task_node_bytes);
    append_varint(&mut payload, state.emitted_count);
    payload.push(u8::from(state.exhausted));
    append_varint(&mut payload, state.seen_node_ids.len() as u64);
    let mut previous_id = 0_u64;
    for node_id in &state.seen_node_ids {
        let node_id = u64::try_from(*node_id).map_err(|_| RecallCursorError::InvalidNodeId)?;
        let delta = node_id
            .checked_sub(previous_id)
            .filter(|delta| *delta > 0)
            .ok_or(RecallCursorError::InvalidNodeId)?;
        append_varint(&mut payload, delta);
        previous_id = node_id;
    }
    append_varint(&mut payload, state.roots.len() as u64);
    for root in &state.roots {
        let node_id = u64::try_from(root.node_id).map_err(|_| RecallCursorError::InvalidNodeId)?;
        append_varint(&mut payload, node_id);
        payload.push(node_type_code(&root.node_type)?);
    }
    let checksum = stable_fingerprint(&payload);
    let encoded = encode_base64_url(&payload);
    let cursor = format!("{RECALL_CURSOR_PREFIX}{encoded}.{checksum}");
    if cursor.len() > MAX_RECALL_CONTINUATION_CURSOR_BYTES {
        return Err(RecallCursorError::TooLong);
    }
    Ok(cursor)
}

/// Decodes and validates query binding before any workspace path is resolved.
pub fn decode_recall_continuation_cursor(
    cursor: &str,
    query: &str,
) -> Result<RecallContinuationState, RecallCursorError> {
    if cursor.len() > MAX_RECALL_CONTINUATION_CURSOR_BYTES {
        return Err(RecallCursorError::TooLong);
    }
    let encoded = cursor
        .strip_prefix(RECALL_CURSOR_PREFIX)
        .ok_or(RecallCursorError::InvalidShape)?;
    let (payload_encoded, checksum) = encoded
        .rsplit_once('.')
        .ok_or(RecallCursorError::InvalidShape)?;
    validate_lowercase_fingerprint(checksum)?;
    let payload = decode_base64_url(payload_encoded)?;
    if stable_fingerprint(&payload) != checksum {
        return Err(RecallCursorError::ChecksumMismatch);
    }
    let mut reader = CursorPayloadReader::new(&payload);
    let version = reader.read_u8()?;
    let bundle_bytes = reader.read_array::<16>()?;
    let bundle_id = Uuid::from_bytes(bundle_bytes).hyphenated().to_string();
    let query_fingerprint = encode_fingerprint(reader.read_array::<16>()?);
    let database_revision = encode_fingerprint(reader.read_array::<16>()?);
    let phase = continuation_phase_from_code(reader.read_u8()?)?;
    let offset = reader.read_varint()?;
    let task_node_bytes = reader.read_varint()?;
    let emitted_count = reader.read_varint()?;
    let exhausted = match reader.read_u8()? {
        0 => false,
        1 => true,
        _ => return Err(RecallCursorError::NonCanonicalPayload),
    };
    let seen_count = reader.read_bounded_count()?;
    let mut seen_node_ids = Vec::with_capacity(seen_count);
    let mut previous_id = 0_u64;
    for _ in 0..seen_count {
        let delta = reader.read_varint()?;
        if delta == 0 {
            return Err(RecallCursorError::InvalidNodeId);
        }
        let node_id = previous_id
            .checked_add(delta)
            .ok_or(RecallCursorError::InvalidNodeId)?;
        seen_node_ids.push(i64::try_from(node_id).map_err(|_| RecallCursorError::InvalidNodeId)?);
        previous_id = node_id;
    }
    let root_count = reader.read_bounded_count()?;
    let mut roots = Vec::with_capacity(root_count);
    for _ in 0..root_count {
        let node_id = reader.read_varint()?;
        let node_id = i64::try_from(node_id).map_err(|_| RecallCursorError::InvalidNodeId)?;
        let node_type = node_type_from_code(reader.read_u8()?)?.to_string();
        roots.push(RecallContinuationRoot { node_id, node_type });
    }
    if !reader.is_finished() {
        return Err(RecallCursorError::NonCanonicalPayload);
    }
    let state = RecallContinuationState {
        version,
        bundle_id,
        query_fingerprint,
        database_revision,
        phase,
        offset,
        task_node_bytes,
        emitted_count,
        seen_node_ids,
        roots,
        exhausted,
    };
    validate_continuation_state(&state)?;
    if encode_recall_continuation_cursor(&state)? != cursor {
        return Err(RecallCursorError::NonCanonicalPayload);
    }
    if state.query_fingerprint != normalized_query_fingerprint(query) {
        return Err(RecallCursorError::QueryMismatch);
    }
    if state.exhausted {
        return Err(RecallCursorError::Exhausted);
    }
    Ok(state)
}

fn validate_continuation_state(state: &RecallContinuationState) -> Result<(), RecallCursorError> {
    if state.version != RECALL_CURSOR_VERSION {
        return Err(RecallCursorError::InvalidShape);
    }
    state.bundle_id()?;
    validate_lowercase_fingerprint(&state.query_fingerprint)?;
    validate_lowercase_fingerprint(&state.database_revision)
        .map_err(|_| RecallCursorError::InvalidDatabaseRevision)?;
    if !state.seen_node_ids.windows(2).all(|ids| ids[0] < ids[1])
        || state.seen_node_ids.iter().any(|id| *id <= 0)
    {
        return Err(RecallCursorError::InvalidNodeId);
    }
    let mut root_ids = HashSet::with_capacity(state.roots.len());
    for root in &state.roots {
        if root.node_id <= 0 || node_type_code(&root.node_type).is_err() {
            return Err(RecallCursorError::InvalidNodeId);
        }
        if !root_ids.insert(root.node_id) {
            return Err(RecallCursorError::DuplicateRoot);
        }
    }
    let task_used = state
        .task_used_bytes(false)
        .map_err(|_| RecallCursorError::InvalidShape)?;
    if task_used > TASK_RECALL_SOFT_BUDGET_BYTES
        || state.emitted_count != state.seen_node_ids.len() as u64
    {
        return Err(RecallCursorError::InvalidShape);
    }
    Ok(())
}

#[must_use]
pub fn normalize_recall_query(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

const RECALL_WORKSPACE_BINDING_DOMAIN: &[u8] = b"aopmem-recall-workspace-v1";

/// Binds an operational-memory revision to one managed workspace without
/// placing the workspace key or path in the continuation cursor.
pub fn bind_recall_revision_to_workspace(
    workspace_key: &str,
    operational_revision: &str,
) -> Result<String, RecallCursorError> {
    if workspace_key.is_empty() {
        return Err(RecallCursorError::InvalidShape);
    }
    validate_lowercase_fingerprint(operational_revision)
        .map_err(|_| RecallCursorError::InvalidDatabaseRevision)?;
    let capacity = RECALL_WORKSPACE_BINDING_DOMAIN
        .len()
        .checked_add(workspace_key.len())
        .and_then(|size| size.checked_add(operational_revision.len()))
        .ok_or(RecallCursorError::InvalidShape)?;
    let mut input = Vec::with_capacity(capacity);
    input.extend_from_slice(RECALL_WORKSPACE_BINDING_DOMAIN);
    input.extend_from_slice(workspace_key.as_bytes());
    input.extend_from_slice(operational_revision.as_bytes());
    Ok(stable_fingerprint(&input))
}

pub(crate) fn normalized_query_fingerprint(query: &str) -> String {
    stable_fingerprint(normalize_recall_query(query).as_bytes())
}

fn stable_fingerprint(bytes: &[u8]) -> String {
    fn hash(bytes: impl Iterator<Item = u8>, seed: u64) -> u64 {
        bytes.fold(seed, |mut value, byte| {
            value ^= u64::from(byte);
            value.wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    let first = hash(bytes.iter().copied(), 0xcbf2_9ce4_8422_2325);
    let second = hash(bytes.iter().rev().copied(), 0x8422_2325_cbf2_9ce4);
    format!("{first:016x}{second:016x}")
}

fn validate_lowercase_fingerprint(value: &str) -> Result<(), RecallCursorError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RecallCursorError::NonCanonicalEncoding);
    }
    Ok(())
}

fn decode_fingerprint(value: &str) -> Result<[u8; 16], RecallCursorError> {
    validate_lowercase_fingerprint(value)?;
    let mut decoded = [0_u8; 16];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = lowercase_hex_nibble(pair[0])?;
        let low = lowercase_hex_nibble(pair[1])?;
        decoded[index] = (high << 4) | low;
    }
    Ok(decoded)
}

fn encode_fingerprint(value: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(32);
    for byte in value {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn continuation_phase_code(phase: RecallContinuationPhase) -> u8 {
    match phase {
        RecallContinuationPhase::TypedRoots => 0,
        RecallContinuationPhase::Fts => 1,
        RecallContinuationPhase::DirectLinks => 2,
        RecallContinuationPhase::Graph => 3,
    }
}

fn continuation_phase_from_code(code: u8) -> Result<RecallContinuationPhase, RecallCursorError> {
    match code {
        0 => Ok(RecallContinuationPhase::TypedRoots),
        1 => Ok(RecallContinuationPhase::Fts),
        2 => Ok(RecallContinuationPhase::DirectLinks),
        3 => Ok(RecallContinuationPhase::Graph),
        _ => Err(RecallCursorError::InvalidShape),
    }
}

fn node_type_code(node_type: &str) -> Result<u8, RecallCursorError> {
    crate::storage::ALLOWED_NODE_TYPES
        .iter()
        .position(|allowed| *allowed == node_type)
        .and_then(|index| u8::try_from(index).ok())
        .ok_or(RecallCursorError::InvalidShape)
}

fn node_type_from_code(code: u8) -> Result<&'static str, RecallCursorError> {
    crate::storage::ALLOWED_NODE_TYPES
        .get(usize::from(code))
        .copied()
        .ok_or(RecallCursorError::InvalidShape)
}

fn append_varint(output: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

const BASE64_URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn encode_base64_url(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut chunks = bytes.chunks_exact(3);
    for chunk in &mut chunks {
        encoded.push(BASE64_URL_ALPHABET[(chunk[0] >> 2) as usize] as char);
        encoded.push(
            BASE64_URL_ALPHABET[(((chunk[0] & 0x03) << 4) | (chunk[1] >> 4)) as usize] as char,
        );
        encoded.push(
            BASE64_URL_ALPHABET[(((chunk[1] & 0x0f) << 2) | (chunk[2] >> 6)) as usize] as char,
        );
        encoded.push(BASE64_URL_ALPHABET[(chunk[2] & 0x3f) as usize] as char);
    }
    match chunks.remainder() {
        [first] => {
            encoded.push(BASE64_URL_ALPHABET[(first >> 2) as usize] as char);
            encoded.push(BASE64_URL_ALPHABET[((first & 0x03) << 4) as usize] as char);
        }
        [first, second] => {
            encoded.push(BASE64_URL_ALPHABET[(first >> 2) as usize] as char);
            encoded.push(
                BASE64_URL_ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char,
            );
            encoded.push(BASE64_URL_ALPHABET[((second & 0x0f) << 2) as usize] as char);
        }
        [] => {}
        _ => unreachable!("chunks_exact remainder is shorter than three bytes"),
    }
    encoded
}

fn decode_base64_url(value: &str) -> Result<Vec<u8>, RecallCursorError> {
    if value.is_empty() || value.len() % 4 == 1 {
        return Err(RecallCursorError::NonCanonicalEncoding);
    }
    let mut decoded = Vec::with_capacity(value.len() / 4 * 3 + 2);
    let mut chunks = value.as_bytes().chunks_exact(4);
    for chunk in &mut chunks {
        let a = base64_url_value(chunk[0])?;
        let b = base64_url_value(chunk[1])?;
        let c = base64_url_value(chunk[2])?;
        let d = base64_url_value(chunk[3])?;
        decoded.push((a << 2) | (b >> 4));
        decoded.push((b << 4) | (c >> 2));
        decoded.push((c << 6) | d);
    }
    match chunks.remainder() {
        [first, second] => {
            let a = base64_url_value(*first)?;
            let b = base64_url_value(*second)?;
            if b & 0x0f != 0 {
                return Err(RecallCursorError::NonCanonicalEncoding);
            }
            decoded.push((a << 2) | (b >> 4));
        }
        [first, second, third] => {
            let a = base64_url_value(*first)?;
            let b = base64_url_value(*second)?;
            let c = base64_url_value(*third)?;
            if c & 0x03 != 0 {
                return Err(RecallCursorError::NonCanonicalEncoding);
            }
            decoded.push((a << 2) | (b >> 4));
            decoded.push((b << 4) | (c >> 2));
        }
        [] => {}
        _ => return Err(RecallCursorError::NonCanonicalEncoding),
    }
    if encode_base64_url(&decoded) != value {
        return Err(RecallCursorError::NonCanonicalEncoding);
    }
    Ok(decoded)
}

fn base64_url_value(byte: u8) -> Result<u8, RecallCursorError> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'-' => Ok(62),
        b'_' => Ok(63),
        _ => Err(RecallCursorError::NonCanonicalEncoding),
    }
}

fn lowercase_hex_nibble(byte: u8) -> Result<u8, RecallCursorError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(RecallCursorError::NonCanonicalEncoding),
    }
}

struct CursorPayloadReader<'a> {
    payload: &'a [u8],
    offset: usize,
}

impl<'a> CursorPayloadReader<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, offset: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, RecallCursorError> {
        let byte = *self
            .payload
            .get(self.offset)
            .ok_or(RecallCursorError::InvalidShape)?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], RecallCursorError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(RecallCursorError::InvalidShape)?;
        let bytes = self
            .payload
            .get(self.offset..end)
            .ok_or(RecallCursorError::InvalidShape)?;
        self.offset = end;
        bytes
            .try_into()
            .map_err(|_| RecallCursorError::InvalidShape)
    }

    fn read_varint(&mut self) -> Result<u64, RecallCursorError> {
        let start = self.offset;
        let mut value = 0_u128;
        let mut shift = 0_u32;
        loop {
            if shift >= 70 {
                return Err(RecallCursorError::NonCanonicalPayload);
            }
            let byte = self.read_u8()?;
            value |= u128::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        let value = u64::try_from(value).map_err(|_| RecallCursorError::InvalidShape)?;
        let mut canonical = Vec::with_capacity(10);
        append_varint(&mut canonical, value);
        if self.payload.get(start..self.offset) != Some(canonical.as_slice()) {
            return Err(RecallCursorError::NonCanonicalPayload);
        }
        Ok(value)
    }

    fn read_bounded_count(&mut self) -> Result<usize, RecallCursorError> {
        let count =
            usize::try_from(self.read_varint()?).map_err(|_| RecallCursorError::InvalidShape)?;
        if count > self.payload.len().saturating_sub(self.offset) {
            return Err(RecallCursorError::InvalidShape);
        }
        Ok(count)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.payload.len()
    }
}

#[derive(Debug, Error)]
pub enum RecallModelError {
    #[error("canonical JSON byte count overflow")]
    ByteCountOverflow,
    #[error("recall task payload must be a JSON object")]
    InvalidTaskPayload,
    #[error("recall JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("recall task budget contains {used_bytes} bytes, above the hard cumulative limit")]
    TaskBudgetExceeded { used_bytes: usize },
}

/// A fully validated mandatory section and its exact canonical JSON size.
#[derive(Debug, Clone, PartialEq)]
pub struct MandatoryRecallContext {
    pub section: RecallSection,
    pub used_bytes: usize,
}

/// A byte-bounded task section plus the exact canonical JSON bytes it uses.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRecallContext {
    pub section: RecallSection,
    pub used_bytes: usize,
    pub more_results: bool,
    /// True only when at least one selected complete node did not fit the
    /// canonical JSON byte budget. Storage row limits are reported separately.
    pub byte_budget_exhausted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MandatoryContextBuildError {
    #[error("mandatory context exceeds {hard_limit_bytes} canonical JSON UTF-8 bytes")]
    Overflow {
        hard_limit_bytes: usize,
        used_bytes_before_overflow: usize,
        offending_node_ids: Vec<i64>,
    },
    #[error("node {node_id} is not active mandatory context")]
    InvalidNode { node_id: i64 },
    #[error("mandatory context byte accounting failed: {message}")]
    ByteAccounting { message: String },
}

impl MandatoryContextBuildError {
    #[must_use]
    pub fn offending_node_ids(&self) -> Option<&[i64]> {
        match self {
            Self::Overflow {
                offending_node_ids, ..
            } => Some(offending_node_ids),
            Self::InvalidNode { .. } | Self::ByteAccounting { .. } => None,
        }
    }
}

/// Builds mandatory recall context without truncation.
///
/// Overflow ids are the stable canonical tail beginning with the first node
/// whose complete JSON representation cannot fit. The builder returns no
/// partial section on overflow.
pub fn build_mandatory_recall_context(
    nodes: Vec<Node>,
) -> Result<MandatoryRecallContext, MandatoryContextBuildError> {
    build_mandatory_recall_context_with_limit(nodes, MANDATORY_RECALL_HARD_BUDGET_BYTES)
}

fn build_mandatory_recall_context_with_limit(
    mut nodes: Vec<Node>,
    hard_limit_bytes: usize,
) -> Result<MandatoryRecallContext, MandatoryContextBuildError> {
    nodes.sort_by(|left, right| {
        mandatory_type_rank(&left.node_type)
            .cmp(&mandatory_type_rank(&right.node_type))
            .then_with(|| left.id.cmp(&right.id))
    });

    if let Some(node) = nodes
        .iter()
        .find(|node| !is_active_mandatory_context_node(node))
    {
        return Err(MandatoryContextBuildError::InvalidNode { node_id: node.id });
    }
    let ordered_node_ids = nodes.iter().map(|node| node.id).collect::<Vec<_>>();

    let empty_complete_section = RecallSection {
        complete: true,
        nodes: Vec::new(),
    };
    let mut used_bytes = canonical_json_byte_len(&empty_complete_section).map_err(|error| {
        MandatoryContextBuildError::ByteAccounting {
            message: error.to_string(),
        }
    })?;
    if used_bytes > hard_limit_bytes {
        return Err(MandatoryContextBuildError::Overflow {
            hard_limit_bytes,
            used_bytes_before_overflow: 0,
            offending_node_ids: ordered_node_ids,
        });
    }

    let mut selected_nodes = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.into_iter().enumerate() {
        let mandatory_type = MandatoryContextType::from_node_type(&node.node_type)
            .ok_or(MandatoryContextBuildError::InvalidNode { node_id: node.id })?;
        let selected = SelectedRecallNode {
            node,
            selection_reasons: vec![RecallSelectionReason::MandatoryContext { mandatory_type }],
        };
        let selected_bytes = canonical_json_byte_len(&selected).map_err(|error| {
            MandatoryContextBuildError::ByteAccounting {
                message: error.to_string(),
            }
        })?;
        let separator_bytes = usize::from(!selected_nodes.is_empty());
        let candidate_bytes = used_bytes
            .checked_add(separator_bytes)
            .and_then(|bytes| bytes.checked_add(selected_bytes));
        let Some(candidate_bytes) = candidate_bytes else {
            return Err(MandatoryContextBuildError::Overflow {
                hard_limit_bytes,
                used_bytes_before_overflow: used_bytes,
                offending_node_ids: ordered_node_ids[index..].to_vec(),
            });
        };
        if candidate_bytes > hard_limit_bytes {
            return Err(MandatoryContextBuildError::Overflow {
                hard_limit_bytes,
                used_bytes_before_overflow: used_bytes,
                offending_node_ids: ordered_node_ids[index..].to_vec(),
            });
        }

        used_bytes = candidate_bytes;
        selected_nodes.push(selected);
    }

    Ok(MandatoryRecallContext {
        section: RecallSection {
            complete: true,
            nodes: selected_nodes,
        },
        used_bytes,
    })
}

fn mandatory_type_rank(node_type: &str) -> usize {
    match node_type {
        "kernel_contract" => 0,
        "gate" => 1,
        "project_profile" => 2,
        "source" => 3,
        "rule" => 4,
        _ => usize::MAX,
    }
}

/// Builds the v0.2 first-pass task section in retrieval order.
///
/// Mandatory ids are omitted because their complete nodes already appear in
/// the mandatory section. Candidate nodes are never shortened or split. Once
/// the next complete node cannot fit, packing stops and reports more results.
pub fn build_task_recall_context(
    candidates: TaskRecallCandidates,
    mandatory: &RecallSection,
) -> Result<TaskRecallContext, RecallModelError> {
    let storage_more_results = candidates.more_results;
    let selected_nodes = select_task_recall_candidates(candidates, mandatory);

    let complete_used_bytes = recall_section_byte_len(true, &selected_nodes)?;
    if !storage_more_results && complete_used_bytes <= TASK_RECALL_SOFT_BUDGET_BYTES {
        return Ok(TaskRecallContext {
            section: RecallSection {
                complete: true,
                nodes: selected_nodes,
            },
            used_bytes: complete_used_bytes,
            more_results: false,
            byte_budget_exhausted: false,
        });
    }

    let selected_count = selected_nodes.len();
    let mut used_bytes = empty_task_section_byte_len(false)?;
    let mut packed_nodes = Vec::with_capacity(selected_nodes.len());
    for selected in selected_nodes {
        let selected_bytes = canonical_json_byte_len(&selected)?;
        let separator_bytes = usize::from(!packed_nodes.is_empty());
        let Some(candidate_bytes) = used_bytes
            .checked_add(separator_bytes)
            .and_then(|bytes| bytes.checked_add(selected_bytes))
        else {
            break;
        };
        if candidate_bytes > TASK_RECALL_SOFT_BUDGET_BYTES {
            break;
        }
        used_bytes = candidate_bytes;
        packed_nodes.push(selected);
    }

    let byte_budget_exhausted = selected_count > packed_nodes.len();
    Ok(TaskRecallContext {
        section: RecallSection {
            complete: false,
            nodes: packed_nodes,
        },
        used_bytes,
        more_results: true,
        byte_budget_exhausted,
    })
}

/// Reusable mandatory-node filter for paged task-recall selection.
pub(crate) struct TaskRecallCandidateSelector {
    mandatory_ids: HashSet<i64>,
}

impl TaskRecallCandidateSelector {
    #[must_use]
    pub(crate) fn new(mandatory: &RecallSection) -> Self {
        Self {
            mandatory_ids: mandatory
                .nodes
                .iter()
                .map(|selected| selected.node.id)
                .collect(),
        }
    }

    #[must_use]
    pub(crate) fn select(&self, candidates: TaskRecallCandidates) -> Vec<SelectedRecallNode> {
        select_task_recall_candidates_with_mandatory_ids(candidates, &self.mandatory_ids)
    }
}

/// Applies v0.2 layer priority, global node deduplication, and typed reasons
/// without doing page or budget packing.
#[must_use]
pub fn select_task_recall_candidates(
    candidates: TaskRecallCandidates,
    mandatory: &RecallSection,
) -> Vec<SelectedRecallNode> {
    TaskRecallCandidateSelector::new(mandatory).select(candidates)
}

fn select_task_recall_candidates_with_mandatory_ids(
    candidates: TaskRecallCandidates,
    mandatory_ids: &HashSet<i64>,
) -> Vec<SelectedRecallNode> {
    let mut selected_nodes = Vec::with_capacity(
        candidates.typed_roots.len()
            + candidates.fts_results.len()
            + candidates.direct_nodes.len()
            + candidates.graph_nodes.len(),
    );
    let mut selected_positions = HashMap::new();
    for node in candidates.typed_roots {
        let reason = RecallSelectionReason::TypedRoot {
            node_type: node.node_type.clone(),
        };
        merge_task_candidate(
            &mut selected_nodes,
            &mut selected_positions,
            mandatory_ids,
            node,
            reason,
            TaskRetrievalTier::TypedRoot,
        );
    }
    for result in candidates.fts_results {
        merge_task_candidate(
            &mut selected_nodes,
            &mut selected_positions,
            mandatory_ids,
            result.node,
            RecallSelectionReason::FtsBm25 { rank: result.rank },
            TaskRetrievalTier::Fts,
        );
    }
    for linked in candidates.direct_nodes {
        merge_task_candidate(
            &mut selected_nodes,
            &mut selected_positions,
            mandatory_ids,
            linked.node,
            RecallSelectionReason::DirectLink {
                source_node_id: linked.root_node_id,
                link_type: linked.link.link_type,
            },
            TaskRetrievalTier::Direct,
        );
    }
    for traversed in candidates.graph_nodes {
        let expansion_type = expansion_type_for_root(&traversed.root_node_type);
        let target_is_expansion = is_expansion_target_type(&traversed.node.node_type);
        let node_id = traversed.node.id;
        merge_task_candidate(
            &mut selected_nodes,
            &mut selected_positions,
            mandatory_ids,
            traversed.node,
            RecallSelectionReason::GraphTraversal {
                root_node_id: traversed.root_node_id,
                root_node_type: traversed.root_node_type,
                source_node_id: traversed.edge_source_node_id,
                link_type: traversed.link.link_type,
                depth: traversed.depth,
            },
            TaskRetrievalTier::Graph,
        );
        if let Some(expansion_type) = expansion_type.filter(|_| target_is_expansion) {
            add_task_candidate_reason(
                &mut selected_nodes,
                &selected_positions,
                node_id,
                RecallSelectionReason::Expansion {
                    source_node_id: traversed.root_node_id,
                    expansion_type,
                },
            );
        }
    }

    selected_nodes.sort_by(compare_task_candidates);
    for candidate in &mut selected_nodes {
        candidate
            .selected
            .selection_reasons
            .sort_by(compare_selection_reasons);
        candidate
            .selected
            .selection_reasons
            .dedup_by(|left, right| selection_reasons_semantically_equal(left, right));
    }
    selected_nodes
        .into_iter()
        .map(|candidate| candidate.selected)
        .collect()
}

fn merge_task_candidate(
    selected_nodes: &mut Vec<MergedTaskCandidate>,
    selected_positions: &mut HashMap<i64, usize>,
    mandatory_ids: &HashSet<i64>,
    node: Node,
    reason: RecallSelectionReason,
    tier: TaskRetrievalTier,
) {
    let fts_rank = match &reason {
        RecallSelectionReason::FtsBm25 { rank } => Some(*rank),
        _ => None,
    };
    if mandatory_ids.contains(&node.id) {
        return;
    }
    if let Some(index) = selected_positions.get(&node.id).copied() {
        let candidate = &mut selected_nodes[index];
        candidate.tier = candidate.tier.min(tier);
        if let Some(rank) = fts_rank {
            candidate.fts_rank = Some(candidate.fts_rank.map_or(rank, |current| {
                if rank.total_cmp(&current).is_lt() {
                    rank
                } else {
                    current
                }
            }));
        }
        let reasons = &mut candidate.selected.selection_reasons;
        if !reasons
            .iter()
            .any(|existing| selection_reasons_semantically_equal(existing, &reason))
        {
            reasons.push(reason);
        }
        return;
    }

    selected_positions.insert(node.id, selected_nodes.len());
    selected_nodes.push(MergedTaskCandidate {
        selected: SelectedRecallNode {
            node,
            selection_reasons: vec![reason],
        },
        tier,
        fts_rank,
    });
}

fn add_task_candidate_reason(
    selected_nodes: &mut [MergedTaskCandidate],
    selected_positions: &HashMap<i64, usize>,
    node_id: i64,
    reason: RecallSelectionReason,
) {
    let Some(index) = selected_positions.get(&node_id).copied() else {
        return;
    };
    let reasons = &mut selected_nodes[index].selected.selection_reasons;
    if !reasons
        .iter()
        .any(|existing| selection_reasons_semantically_equal(existing, &reason))
    {
        reasons.push(reason);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TaskRetrievalTier {
    TypedRoot,
    Fts,
    Direct,
    Graph,
}

#[derive(Debug, Clone, PartialEq)]
struct MergedTaskCandidate {
    selected: SelectedRecallNode,
    tier: TaskRetrievalTier,
    fts_rank: Option<f64>,
}

fn compare_task_candidates(left: &MergedTaskCandidate, right: &MergedTaskCandidate) -> Ordering {
    left.tier
        .cmp(&right.tier)
        .then_with(|| {
            task_source_hierarchy_priority(&left.selected.node)
                .cmp(&task_source_hierarchy_priority(&right.selected.node))
        })
        .then_with(|| {
            trust_level_priority(&left.selected.node)
                .cmp(&trust_level_priority(&right.selected.node))
        })
        .then_with(|| {
            right
                .selected
                .node
                .confidence
                .partial_cmp(&left.selected.node.confidence)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| {
            if left.tier == TaskRetrievalTier::Fts && right.tier == TaskRetrievalTier::Fts {
                left.fts_rank
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&right.fts_rank.unwrap_or(f64::INFINITY))
            } else {
                Ordering::Equal
            }
        })
        .then_with(|| left.selected.node.id.cmp(&right.selected.node.id))
}

fn task_source_hierarchy_priority(node: &Node) -> u8 {
    node.source_hierarchy()
        .map(|hierarchy| hierarchy.priority)
        .unwrap_or(u8::MAX)
}

fn expansion_type_for_root(node_type: &str) -> Option<RecallExpansionType> {
    match node_type {
        "workflow" => Some(RecallExpansionType::Workflow),
        "tool_contract" => Some(RecallExpansionType::Tool),
        "failure_mode" => Some(RecallExpansionType::FailureMode),
        _ => None,
    }
}

fn is_expansion_target_type(node_type: &str) -> bool {
    matches!(
        node_type,
        "workflow" | "tool_contract" | "failure_mode" | "correction" | "rule" | "lesson" | "skill"
    )
}

fn compare_selection_reasons(
    left: &RecallSelectionReason,
    right: &RecallSelectionReason,
) -> Ordering {
    selection_reason_rank(left)
        .cmp(&selection_reason_rank(right))
        .then_with(|| match (left, right) {
            (
                RecallSelectionReason::MandatoryContext {
                    mandatory_type: left,
                },
                RecallSelectionReason::MandatoryContext {
                    mandatory_type: right,
                },
            ) => mandatory_context_type_rank(*left).cmp(&mandatory_context_type_rank(*right)),
            (
                RecallSelectionReason::TypedRoot { node_type: left },
                RecallSelectionReason::TypedRoot { node_type: right },
            ) => left.cmp(right),
            (
                RecallSelectionReason::FtsBm25 { rank: left },
                RecallSelectionReason::FtsBm25 { rank: right },
            ) => left.total_cmp(right),
            (
                RecallSelectionReason::DirectLink {
                    source_node_id: left_source,
                    link_type: left_link,
                },
                RecallSelectionReason::DirectLink {
                    source_node_id: right_source,
                    link_type: right_link,
                },
            ) => left_source
                .cmp(right_source)
                .then_with(|| left_link.cmp(right_link)),
            (
                RecallSelectionReason::GraphTraversal {
                    root_node_id: left_root,
                    root_node_type: left_root_type,
                    source_node_id: left_source,
                    link_type: left_link,
                    depth: left_depth,
                },
                RecallSelectionReason::GraphTraversal {
                    root_node_id: right_root,
                    root_node_type: right_root_type,
                    source_node_id: right_source,
                    link_type: right_link,
                    depth: right_depth,
                },
            ) => left_root
                .cmp(right_root)
                .then_with(|| left_root_type.cmp(right_root_type))
                .then_with(|| left_depth.cmp(right_depth))
                .then_with(|| left_source.cmp(right_source))
                .then_with(|| left_link.cmp(right_link)),
            (
                RecallSelectionReason::Expansion {
                    source_node_id: left_source,
                    expansion_type: left_type,
                },
                RecallSelectionReason::Expansion {
                    source_node_id: right_source,
                    expansion_type: right_type,
                },
            ) => left_source.cmp(right_source).then_with(|| {
                expansion_type_rank(*left_type).cmp(&expansion_type_rank(*right_type))
            }),
            _ => Ordering::Equal,
        })
}

fn selection_reasons_semantically_equal(
    left: &RecallSelectionReason,
    right: &RecallSelectionReason,
) -> bool {
    compare_selection_reasons(left, right) == Ordering::Equal
}

fn selection_reason_rank(reason: &RecallSelectionReason) -> usize {
    match reason {
        RecallSelectionReason::MandatoryContext { .. } => 0,
        RecallSelectionReason::TypedRoot { .. } => 1,
        RecallSelectionReason::FtsBm25 { .. } => 2,
        RecallSelectionReason::DirectLink { .. } => 3,
        RecallSelectionReason::GraphTraversal { .. } => 4,
        RecallSelectionReason::Expansion { .. } => 5,
    }
}

fn mandatory_context_type_rank(mandatory_type: MandatoryContextType) -> usize {
    match mandatory_type {
        MandatoryContextType::KernelContract => 0,
        MandatoryContextType::Gate => 1,
        MandatoryContextType::ProjectProfile => 2,
        MandatoryContextType::Source => 3,
        MandatoryContextType::Rule => 4,
    }
}

fn expansion_type_rank(expansion_type: RecallExpansionType) -> usize {
    match expansion_type {
        RecallExpansionType::Workflow => 0,
        RecallExpansionType::Tool => 1,
        RecallExpansionType::FailureMode => 2,
    }
}

fn recall_section_byte_len(
    complete: bool,
    nodes: &[SelectedRecallNode],
) -> Result<usize, RecallModelError> {
    #[derive(Serialize)]
    struct BorrowedRecallSection<'a> {
        complete: bool,
        nodes: &'a [SelectedRecallNode],
    }

    canonical_json_byte_len(&BorrowedRecallSection { complete, nodes })
}

pub fn empty_task_section_byte_len(complete: bool) -> Result<usize, RecallModelError> {
    recall_section_byte_len(complete, &[])
}

/// Counts the compact serde JSON representation without allocating the output.
/// Struct field order is declaration order, so v0.2 budget accounting must use
/// structs (and never unordered maps) for all budgeted values.
pub fn canonical_json_byte_len<T: Serialize>(value: &T) -> Result<usize, RecallModelError> {
    canonical_json_byte_len_from(value, 0)
}

fn canonical_json_byte_len_from<T: Serialize>(
    value: &T,
    initial_bytes: usize,
) -> Result<usize, RecallModelError> {
    let mut counter = CanonicalJsonByteCounter {
        bytes: initial_bytes,
        overflowed: false,
    };
    if let Err(error) = serde_json::to_writer(&mut counter, value) {
        if counter.overflowed {
            return Err(RecallModelError::ByteCountOverflow);
        }
        return Err(RecallModelError::Json(error));
    }
    Ok(counter.bytes)
}

struct CanonicalJsonByteCounter {
    bytes: usize,
    overflowed: bool,
}

impl Write for CanonicalJsonByteCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self.bytes.checked_add(buffer.len()) {
            Some(bytes) => {
                self.bytes = bytes;
                Ok(buffer.len())
            }
            None => {
                self.overflowed = true;
                Err(io::Error::other("canonical JSON byte count overflow"))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructuredRecallBundle {
    pub project_profiles: RecallNodesByStatus,
    pub gates: RecallNodesByStatus,
    pub workflows: RecallNodesByStatus,
    pub linked_nodes: Vec<RecallLinkedNode>,
    pub fts_fallback: Vec<FtsNodeSearchResult>,
    pub hunches: Vec<RecallHunch>,
    pub compact: CompactRecallBundle,
    #[serde(skip)]
    context_nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoundedQueryRecall {
    pub matches: Vec<FtsNodeSearchResult>,
    pub hunches: Vec<RecallHunch>,
    pub compact: CompactRecallBundle,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RecallNodesByStatus {
    pub draft: Vec<Node>,
    pub active: Vec<Node>,
    pub deprecated: Vec<Node>,
    pub superseded: Vec<Node>,
    pub broken: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecallLinkedNode {
    pub depth: usize,
    pub source_node_id: i64,
    pub link_type: String,
    pub node: Node,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecallHunch {
    pub source_node_id: i64,
    pub source_node_type: String,
    pub linked_signal_node_id: Option<i64>,
    pub linked_signal_node_type: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub reason: String,
    pub source_updated_at: String,
    pub source_hierarchy: Option<SourceHierarchy>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CompactRecallBundle {
    pub applicable_workflows: Vec<CompactNodeRef>,
    pub active_gates: Vec<CompactNodeRef>,
    pub tool_contracts: Vec<CompactNodeRef>,
    pub rules: Vec<CompactNodeRef>,
    pub mcp_profiles: Vec<CompactNodeRef>,
    pub project_profile_facts: Vec<CompactNodeRef>,
    pub relevant_corrections_lessons: Vec<CompactNodeRef>,
    pub hunches: Vec<CompactHunch>,
    pub source_refs: Vec<CompactSourceRef>,
    pub limits: CompactRecallLimits,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompactNodeRef {
    pub node_id: i64,
    pub node_type: String,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
    pub source_hierarchy: Option<SourceHierarchy>,
    pub least_privilege: Option<LeastPrivilegeMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompactHunch {
    pub source_node_id: i64,
    pub title: String,
    pub reason: String,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
    pub source_hierarchy: Option<SourceHierarchy>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompactSourceRef {
    pub node_id: i64,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
    pub source_hierarchy: Option<SourceHierarchy>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompactRecallLimits {
    pub applicable_workflows: usize,
    pub active_gates: usize,
    pub section_nodes: usize,
    pub hunches: usize,
    pub source_refs: usize,
}

impl Default for CompactRecallLimits {
    fn default() -> Self {
        Self {
            applicable_workflows: MAX_COMPACT_APPLICABLE_WORKFLOWS,
            active_gates: MAX_COMPACT_SECTION_NODES,
            section_nodes: MAX_COMPACT_SECTION_NODES,
            hunches: MAX_HUNCHES,
            source_refs: MAX_COMPACT_SOURCE_REFS,
        }
    }
}

#[derive(Debug, Clone)]
struct HunchLinkedSignal {
    node_id: i64,
    node_type: String,
    priority: u8,
}

pub fn build_structured_bundle(nodes: Vec<Node>) -> StructuredRecallBundle {
    build_structured_bundle_with_links(nodes, Vec::new())
}

pub fn build_structured_bundle_with_links(
    nodes: Vec<Node>,
    links: Vec<Link>,
) -> StructuredRecallBundle {
    let mut bundle = StructuredRecallBundle {
        project_profiles: RecallNodesByStatus::default(),
        gates: RecallNodesByStatus::default(),
        workflows: RecallNodesByStatus::default(),
        linked_nodes: traverse_links(&nodes, &links),
        fts_fallback: Vec::new(),
        hunches: Vec::new(),
        compact: CompactRecallBundle::default(),
        context_nodes: Vec::new(),
    };

    for node in nodes {
        if should_exclude_from_normal_recall(&node) {
            continue;
        }

        match node.node_type.as_str() {
            "project_profile" => bundle.project_profiles.push(node),
            "gate" => bundle.gates.push(node),
            "workflow" => bundle.workflows.push(node),
            "tool_contract" | "rule" => bundle.context_nodes.push(node),
            _ => {}
        }
    }

    bundle.compact = build_compact_bundle(&bundle);
    bundle
}

pub fn needs_fts_fallback(bundle: &StructuredRecallBundle) -> bool {
    !has_at_least_structured_nodes(bundle, STRUCTURED_RECALL_SUFFICIENT_NODE_COUNT)
}

pub fn derive_fts_fallback_query(bundle: &StructuredRecallBundle) -> Option<String> {
    status_titles(&bundle.project_profiles)
        .chain(status_titles(&bundle.gates))
        .chain(status_titles(&bundle.workflows))
        .chain(
            bundle
                .linked_nodes
                .iter()
                .map(|linked| linked.node.title.as_str()),
        )
        .chain(bundle.context_nodes.iter().map(|node| node.title.as_str()))
        .map(str::trim)
        .find(|title| !title.is_empty())
        .map(ToOwned::to_owned)
}

pub fn add_fts_fallback(
    mut bundle: StructuredRecallBundle,
    results: Vec<FtsNodeSearchResult>,
) -> StructuredRecallBundle {
    let existing_ids = structured_node_ids(&bundle);
    bundle.fts_fallback = results
        .into_iter()
        .filter(|result| !should_exclude_from_normal_recall(&result.node))
        .filter(|result| !existing_ids.contains(&result.node.id))
        .collect();
    bundle
        .fts_fallback
        .sort_by(|left, right| compare_nodes_for_recall_priority(&left.node, &right.node));
    bundle.hunches = select_hunches(&bundle);
    bundle.compact = build_compact_bundle(&bundle);
    bundle
}

pub fn build_bounded_query_recall(results: Vec<FtsNodeSearchResult>) -> BoundedQueryRecall {
    let matches = results.clone();
    let bundle = add_fts_fallback(build_structured_bundle(Vec::new()), results);

    BoundedQueryRecall {
        matches,
        hunches: bundle.hunches,
        compact: bundle.compact,
    }
}

fn build_compact_bundle(bundle: &StructuredRecallBundle) -> CompactRecallBundle {
    let node_index = BundleNodeIndex::new(bundle);
    let mut compact = CompactRecallBundle {
        applicable_workflows: collect_compact_nodes(
            node_index.nodes_by_type("workflow"),
            MAX_COMPACT_APPLICABLE_WORKFLOWS,
        ),
        active_gates: collect_compact_nodes(
            bundle.gates.active.iter().collect(),
            MAX_COMPACT_SECTION_NODES,
        ),
        tool_contracts: collect_compact_nodes(
            node_index.nodes_by_type("tool_contract"),
            MAX_COMPACT_SECTION_NODES,
        ),
        rules: collect_compact_nodes(node_index.nodes_by_type("rule"), MAX_COMPACT_SECTION_NODES),
        mcp_profiles: collect_compact_nodes(
            node_index.nodes_by_type("mcp_profile"),
            MAX_COMPACT_SECTION_NODES,
        ),
        project_profile_facts: collect_compact_nodes(
            bundle.project_profiles.active.iter().collect(),
            MAX_COMPACT_SECTION_NODES,
        ),
        relevant_corrections_lessons: collect_compact_nodes(
            node_index.nodes_by_types(&["correction", "lesson"]),
            MAX_COMPACT_SECTION_NODES,
        ),
        hunches: collect_compact_hunches(bundle, &node_index, MAX_HUNCHES),
        source_refs: Vec::new(),
        limits: CompactRecallLimits::default(),
    };

    compact.source_refs = collect_compact_source_refs(&compact, MAX_COMPACT_SOURCE_REFS);
    compact.limits = CompactRecallLimits {
        applicable_workflows: MAX_COMPACT_APPLICABLE_WORKFLOWS,
        active_gates: MAX_COMPACT_SECTION_NODES,
        section_nodes: MAX_COMPACT_SECTION_NODES,
        hunches: MAX_HUNCHES,
        source_refs: MAX_COMPACT_SOURCE_REFS,
    };
    compact
}

fn collect_compact_nodes(nodes: Vec<&Node>, limit: usize) -> Vec<CompactNodeRef> {
    let mut nodes = nodes;
    nodes.sort_by(|left, right| compare_nodes_for_recall_priority(left, right));

    nodes
        .into_iter()
        .take(limit)
        .map(|node| CompactNodeRef {
            node_id: node.id,
            node_type: node.node_type.clone(),
            status: node.status.clone(),
            title: node.title.clone(),
            summary: node.summary.clone(),
            source_ref: node.source_ref.clone(),
            confidence: node.confidence,
            trust_level: node.trust_level.clone(),
            source_hierarchy: node.source_hierarchy(),
            least_privilege: node.least_privilege_metadata(),
        })
        .collect()
}

fn collect_compact_hunches(
    bundle: &StructuredRecallBundle,
    node_index: &BundleNodeIndex<'_>,
    limit: usize,
) -> Vec<CompactHunch> {
    bundle
        .hunches
        .iter()
        .take(limit)
        .map(|hunch| {
            let source = node_index.node_by_id(hunch.source_node_id);
            CompactHunch {
                source_node_id: hunch.source_node_id,
                title: hunch.title.clone(),
                reason: hunch.reason.clone(),
                source_ref: source.and_then(|node| node.source_ref.clone()),
                confidence: source.and_then(|node| node.confidence),
                trust_level: source.and_then(|node| node.trust_level.clone()),
                source_hierarchy: source.and_then(Node::source_hierarchy),
            }
        })
        .collect()
}

fn collect_compact_source_refs(
    compact: &CompactRecallBundle,
    limit: usize,
) -> Vec<CompactSourceRef> {
    let mut seen = HashSet::new();
    let mut refs = Vec::new();

    for node in compact_node_refs(compact) {
        if seen.insert(node.node_id) {
            refs.push(CompactSourceRef {
                node_id: node.node_id,
                source_ref: node.source_ref.clone(),
                confidence: node.confidence,
                trust_level: node.trust_level.clone(),
                source_hierarchy: node.source_hierarchy.clone(),
            });
        }

        if refs.len() >= limit {
            return refs;
        }
    }

    for hunch in &compact.hunches {
        if seen.insert(hunch.source_node_id) {
            refs.push(CompactSourceRef {
                node_id: hunch.source_node_id,
                source_ref: hunch.source_ref.clone(),
                confidence: hunch.confidence,
                trust_level: hunch.trust_level.clone(),
                source_hierarchy: hunch.source_hierarchy.clone(),
            });
        }

        if refs.len() >= limit {
            break;
        }
    }

    refs
}

fn compact_node_refs(compact: &CompactRecallBundle) -> impl Iterator<Item = &CompactNodeRef> {
    compact
        .applicable_workflows
        .iter()
        .chain(compact.active_gates.iter())
        .chain(compact.tool_contracts.iter())
        .chain(compact.rules.iter())
        .chain(compact.mcp_profiles.iter())
        .chain(compact.project_profile_facts.iter())
        .chain(compact.relevant_corrections_lessons.iter())
}

struct BundleNodeIndex<'a> {
    all: Vec<&'a Node>,
    by_type: HashMap<&'a str, Vec<&'a Node>>,
    by_id: HashMap<i64, &'a Node>,
}

impl<'a> BundleNodeIndex<'a> {
    fn new(bundle: &'a StructuredRecallBundle) -> Self {
        let all = all_bundle_nodes(bundle).collect::<Vec<_>>();
        let mut by_type = HashMap::<&str, Vec<&Node>>::new();
        let mut by_id = HashMap::new();
        for &node in &all {
            by_type
                .entry(node.node_type.as_str())
                .or_default()
                .push(node);
            by_id.entry(node.id).or_insert(node);
        }
        Self {
            all,
            by_type,
            by_id,
        }
    }

    fn nodes_by_type(&self, node_type: &str) -> Vec<&'a Node> {
        self.by_type.get(node_type).cloned().unwrap_or_default()
    }

    fn nodes_by_types(&self, node_types: &[&str]) -> Vec<&'a Node> {
        self.all
            .iter()
            .copied()
            .filter(|node| node_types.contains(&node.node_type.as_str()))
            .collect()
    }

    fn node_by_id(&self, node_id: i64) -> Option<&'a Node> {
        self.by_id.get(&node_id).copied()
    }
}

fn all_bundle_nodes(bundle: &StructuredRecallBundle) -> impl Iterator<Item = &Node> {
    status_nodes(&bundle.workflows)
        .chain(status_nodes(&bundle.gates))
        .chain(status_nodes(&bundle.project_profiles))
        .chain(&bundle.context_nodes)
        .chain(bundle.linked_nodes.iter().map(|linked| &linked.node))
        .chain(bundle.fts_fallback.iter().map(|result| &result.node))
}

fn status_nodes(nodes: &RecallNodesByStatus) -> impl Iterator<Item = &Node> {
    nodes.active.iter().chain(&nodes.draft).chain(&nodes.broken)
}

fn select_hunches(bundle: &StructuredRecallBundle) -> Vec<RecallHunch> {
    let mut candidates: Vec<&FtsNodeSearchResult> = bundle.fts_fallback.iter().collect();
    let linked_signal = strongest_linked_hunch_signal(bundle);

    candidates.sort_by(|left, right| {
        hunch_signal_priority(right, linked_signal.as_ref())
            .cmp(&hunch_signal_priority(left, linked_signal.as_ref()))
            .then_with(|| compare_nodes_for_recall_priority(&left.node, &right.node))
            .then_with(|| {
                left.rank
                    .partial_cmp(&right.rank)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| right.node.updated_at.cmp(&left.node.updated_at))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });

    candidates
        .into_iter()
        .take(MAX_HUNCHES)
        .map(|result| RecallHunch {
            source_node_id: result.node.id,
            source_node_type: result.node.node_type.clone(),
            linked_signal_node_id: linked_signal.as_ref().map(|signal| signal.node_id),
            linked_signal_node_type: linked_signal
                .as_ref()
                .map(|signal| signal.node_type.clone()),
            title: result.node.title.clone(),
            summary: result.node.summary.clone(),
            reason: hunch_reason(&result.node.node_type, linked_signal.as_ref()).to_string(),
            source_updated_at: result.node.updated_at.clone(),
            source_hierarchy: result.node.source_hierarchy(),
        })
        .collect()
}

fn compare_nodes_for_recall_priority(left: &Node, right: &Node) -> Ordering {
    source_priority(left)
        .cmp(&source_priority(right))
        .then_with(|| trust_level_priority(left).cmp(&trust_level_priority(right)))
        .then_with(|| {
            right
                .confidence
                .partial_cmp(&left.confidence)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.id.cmp(&right.id))
}

fn source_priority(node: &Node) -> (u8, u8) {
    let hierarchy_priority = node
        .source_hierarchy()
        .map(|hierarchy| hierarchy.priority)
        .unwrap_or(u8::MAX);
    let privilege_priority = node
        .least_privilege_metadata()
        .map(|metadata| metadata.privilege_rank)
        .unwrap_or(0);

    (hierarchy_priority, privilege_priority)
}

fn trust_level_priority(node: &Node) -> u8 {
    match node.trust_level.as_deref() {
        Some("high") => 0,
        Some("medium") => 1,
        Some("low") => 2,
        Some(_) => 3,
        None => 4,
    }
}

fn strongest_linked_hunch_signal(bundle: &StructuredRecallBundle) -> Option<HunchLinkedSignal> {
    bundle
        .linked_nodes
        .iter()
        .filter_map(|linked| {
            let priority = hunch_type_priority(&linked.node.node_type);
            (priority > 0).then(|| HunchLinkedSignal {
                node_id: linked.node.id,
                node_type: linked.node.node_type.clone(),
                priority,
            })
        })
        .max_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| right.node_id.cmp(&left.node_id))
        })
}

fn hunch_signal_priority(
    result: &FtsNodeSearchResult,
    linked_signal: Option<&HunchLinkedSignal>,
) -> u8 {
    hunch_type_priority(&result.node.node_type).max(
        linked_signal
            .map(|signal| signal.priority)
            .unwrap_or_default(),
    )
}

fn hunch_type_priority(node_type: &str) -> u8 {
    match node_type {
        "failure_mode" => 3,
        "tool_contract" => 2,
        "workflow" => 1,
        _ => 0,
    }
}

fn hunch_reason(node_type: &str, linked_signal: Option<&HunchLinkedSignal>) -> &'static str {
    if linked_signal.is_some() {
        return "fts_match_linked_signal_hotness";
    }

    match node_type {
        "failure_mode" => "fts_match_failure_mode_hotness",
        "tool_contract" => "fts_match_tool_hotness",
        "workflow" => "fts_match_workflow_hotness",
        _ => "fts_match_hotness",
    }
}

fn traverse_links(nodes: &[Node], links: &[Link]) -> Vec<RecallLinkedNode> {
    let nodes_by_id: HashMap<i64, &Node> = nodes.iter().map(|node| (node.id, node)).collect();
    let mut outgoing: HashMap<i64, Vec<&Link>> = HashMap::new();

    for link in links {
        outgoing.entry(link.source_node_id).or_default().push(link);
    }

    let mut visited: HashSet<i64> = HashSet::new();
    let mut queue = VecDeque::new();

    for node in nodes.iter().filter(|node| is_traversal_root(node)) {
        visited.insert(node.id);
        queue.push_back((node.id, 0));
    }

    let mut linked_nodes = Vec::new();

    while let Some((source_node_id, depth)) = queue.pop_front() {
        if depth >= RECALL_TRAVERSAL_MAX_DEPTH {
            continue;
        }

        for link in outgoing.get(&source_node_id).into_iter().flatten() {
            let Some(target) = nodes_by_id.get(&link.target_node_id) else {
                continue;
            };

            if should_exclude_from_normal_recall(target) || !visited.insert(target.id) {
                continue;
            }

            let next_depth = depth + 1;
            linked_nodes.push(RecallLinkedNode {
                depth: next_depth,
                source_node_id,
                link_type: link.link_type.clone(),
                node: (*target).clone(),
            });
            queue.push_back((target.id, next_depth));
        }
    }

    linked_nodes
}

fn has_at_least_structured_nodes(bundle: &StructuredRecallBundle, minimum: usize) -> bool {
    if minimum == 0 {
        return true;
    }

    let mut ids = HashSet::with_capacity(minimum);
    for id in structured_node_id_iter(bundle) {
        ids.insert(id);
        if ids.len() >= minimum {
            return true;
        }
    }

    false
}

fn structured_node_ids(bundle: &StructuredRecallBundle) -> HashSet<i64> {
    structured_node_id_iter(bundle).collect()
}

fn structured_node_id_iter(bundle: &StructuredRecallBundle) -> impl Iterator<Item = i64> + '_ {
    status_node_ids(&bundle.project_profiles)
        .chain(status_node_ids(&bundle.gates))
        .chain(status_node_ids(&bundle.workflows))
        .chain(bundle.context_nodes.iter().map(|node| node.id))
        .chain(bundle.linked_nodes.iter().map(|linked| linked.node.id))
}

fn status_node_ids(nodes: &RecallNodesByStatus) -> impl Iterator<Item = i64> + '_ {
    nodes
        .draft
        .iter()
        .chain(&nodes.active)
        .chain(&nodes.deprecated)
        .chain(&nodes.superseded)
        .chain(&nodes.broken)
        .map(|node| node.id)
}

fn status_titles(nodes: &RecallNodesByStatus) -> impl Iterator<Item = &str> {
    nodes
        .active
        .iter()
        .chain(&nodes.draft)
        .chain(&nodes.broken)
        .map(|node| node.title.as_str())
}

fn is_traversal_root(node: &Node) -> bool {
    matches!(
        node.node_type.as_str(),
        "workflow" | "tool_contract" | "rule"
    ) && !should_exclude_from_normal_recall(node)
}

fn should_exclude_from_normal_recall(node: &Node) -> bool {
    matches!(node.status.as_str(), "deprecated" | "superseded")
}

impl RecallNodesByStatus {
    fn push(&mut self, node: Node) {
        match node.status.as_str() {
            "draft" => self.draft.push(node),
            "active" => self.active.push(node),
            "deprecated" => self.deprecated.push(node),
            "superseded" => self.superseded.push(node),
            "broken" => self.broken.push(node),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: i64, node_type: &str, status: &str, title: &str) -> Node {
        Node {
            id,
            node_type: node_type.to_string(),
            status: status.to_string(),
            title: title.to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn graph_candidate(
        node: Node,
        root_node_id: i64,
        root_node_type: &str,
        edge_source_node_id: i64,
        link_id: i64,
        link_type: &str,
        depth: usize,
    ) -> crate::storage::GraphRecallNode {
        crate::storage::GraphRecallNode {
            root_node_id,
            root_node_type: root_node_type.to_string(),
            edge_source_node_id,
            link: Link {
                id: link_id,
                source_node_id: edge_source_node_id,
                target_node_id: node.id,
                link_type: link_type.to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
            },
            depth,
            node,
        }
    }

    #[test]
    fn v2_request_and_response_have_stable_explicit_json_shapes() {
        let request = RecallRequestV2::Task {
            query: "ship release".to_string(),
            continuation_cursor: Some("v1.recall.next".to_string()),
            limit: Some(12),
        };
        assert_eq!(
            serde_json::to_string(&request).expect("request should serialize"),
            r#"{"mode":"task","query":"ship release","continuation_cursor":"v1.recall.next","limit":12}"#
        );

        let mut mandatory_node = node(1, "gate", "active", "Release gate");
        mandatory_node.body = Some("Never skip proof".to_string());
        let response = RecallResponseV2 {
            bundle_id: RecallBundleId::parse("550e8400-e29b-41d4-a716-446655440000")
                .expect("fixture should be a canonical UUID v4"),
            mode: RecallMode::Task,
            mandatory: RecallSection {
                complete: true,
                nodes: vec![SelectedRecallNode {
                    node: mandatory_node,
                    selection_reasons: vec![RecallSelectionReason::MandatoryContext {
                        mandatory_type: MandatoryContextType::Gate,
                    }],
                }],
            },
            task: RecallSection {
                complete: false,
                nodes: Vec::new(),
            },
            more_results: true,
            continuation_cursor: Some("v1.recall.next".to_string()),
            budget: RecallBudgetMetadata::new(300, 400)
                .expect("small counters should not overflow"),
        };

        assert_eq!(
            serde_json::to_string(&response).expect("response should serialize"),
            r#"{"bundle_id":"550e8400-e29b-41d4-a716-446655440000","mode":"task","mandatory":{"complete":true,"nodes":[{"node":{"id":1,"node_type":"gate","status":"active","title":"Release gate","summary":null,"body":"Never skip proof","source_ref":null,"confidence":null,"trust_level":null,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"},"selection_reasons":[{"kind":"mandatory_context","mandatory_type":"gate"}]}]},"task":{"complete":false,"nodes":[]},"more_results":true,"continuation_cursor":"v1.recall.next","budget":{"unit":"canonical_json_utf8_bytes","mandatory":{"hard_limit_bytes":1048576,"used_bytes":300},"task":{"soft_limit_bytes":262144,"used_bytes":400,"remaining_bytes":261744,"exhausted":false},"total_used_bytes":700}}"#
        );
    }

    #[test]
    fn recall_cursor_is_canonical_query_bound_and_tamper_evident() {
        let mut state = RecallContinuationState::new(
            "  Deploy   RELEASE  ",
            "0123456789abcdef0123456789abcdef".to_string(),
        )
        .expect("state should build");
        state
            .insert_seen_node(7)
            .expect("positive node should insert");
        state.emitted_count = 1;
        state.task_node_bytes = 100;
        state.roots.push(RecallContinuationRoot {
            node_id: 7,
            node_type: "workflow".to_string(),
        });

        let cursor = encode_recall_continuation_cursor(&state).expect("cursor should encode");
        let decoded = decode_recall_continuation_cursor(&cursor, "deploy release")
            .expect("normalized query should match");

        assert_eq!(decoded, state);
        assert!(cursor
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')));
        assert!(matches!(
            decode_recall_continuation_cursor(&cursor, "different task"),
            Err(RecallCursorError::QueryMismatch)
        ));

        let mut tampered = cursor.into_bytes();
        let index = tampered
            .iter()
            .position(|byte| *byte == b'a')
            .expect("canonical cursor should contain a hexadecimal a");
        tampered[index] = b'b';
        let tampered = String::from_utf8(tampered).expect("cursor remains ASCII");
        assert!(decode_recall_continuation_cursor(&tampered, "deploy release").is_err());
    }

    #[test]
    fn recall_revision_binding_is_deterministic_domain_separated_and_workspace_specific() {
        let revision = "0123456789abcdef0123456789abcdef";
        let first = bind_recall_revision_to_workspace("project-a-12345678", revision)
            .expect("binding should build");
        let repeated = bind_recall_revision_to_workspace("project-a-12345678", revision)
            .expect("binding should repeat");
        let other_workspace = bind_recall_revision_to_workspace("project-b-12345678", revision)
            .expect("other workspace binding should build");
        let other_revision = bind_recall_revision_to_workspace(
            "project-a-12345678",
            "1123456789abcdef0123456789abcdef",
        )
        .expect("other revision binding should build");

        assert_eq!(first, repeated);
        assert_ne!(first, other_workspace);
        assert_ne!(first, other_revision);
        assert_eq!(first.len(), 32);
        assert!(!first.contains("project-a"));
        assert!(bind_recall_revision_to_workspace("", revision).is_err());
        assert!(bind_recall_revision_to_workspace("project", "not-a-revision").is_err());
    }

    #[test]
    fn recall_cursor_rejects_noncanonical_overlong_and_exhausted_state() {
        let mut state =
            RecallContinuationState::new("query", "0123456789abcdef0123456789abcdef".to_string())
                .expect("state should build");
        state.exhausted = true;
        let cursor = encode_recall_continuation_cursor(&state)
            .expect("exhausted terminal cursor should encode");
        assert!(matches!(
            decode_recall_continuation_cursor(&cursor, "query"),
            Err(RecallCursorError::Exhausted)
        ));

        let uppercase = cursor.replacen('a', "A", 1);
        assert!(decode_recall_continuation_cursor(&uppercase, "query").is_err());
        let overlong = "x".repeat(MAX_RECALL_CONTINUATION_CURSOR_BYTES + 1);
        assert!(matches!(
            decode_recall_continuation_cursor(&overlong, "query"),
            Err(RecallCursorError::TooLong)
        ));
    }

    #[test]
    fn recall_cursor_binary_wire_rejects_nonminimal_varint_and_trailing_bytes() {
        let state =
            RecallContinuationState::new("query", "0123456789abcdef0123456789abcdef".to_string())
                .expect("state should build");
        let cursor = encode_recall_continuation_cursor(&state).expect("cursor should encode");
        let encoded = cursor
            .strip_prefix(RECALL_CURSOR_PREFIX)
            .expect("cursor prefix")
            .rsplit_once('.')
            .expect("cursor checksum")
            .0;
        let payload = decode_base64_url(encoded).expect("payload should decode");

        let offset_position = 1 + 16 + 16 + 16 + 1;
        let mut nonminimal = payload.clone();
        assert_eq!(nonminimal[offset_position], 0);
        nonminimal.splice(offset_position..=offset_position, [0x80, 0x00]);
        let nonminimal_cursor = format!(
            "{RECALL_CURSOR_PREFIX}{}.{}",
            encode_base64_url(&nonminimal),
            stable_fingerprint(&nonminimal)
        );
        assert!(matches!(
            decode_recall_continuation_cursor(&nonminimal_cursor, "query"),
            Err(RecallCursorError::NonCanonicalPayload)
        ));

        let mut trailing = payload;
        trailing.push(0);
        let trailing_cursor = format!(
            "{RECALL_CURSOR_PREFIX}{}.{}",
            encode_base64_url(&trailing),
            stable_fingerprint(&trailing)
        );
        assert!(matches!(
            decode_recall_continuation_cursor(&trailing_cursor, "query"),
            Err(RecallCursorError::NonCanonicalPayload)
        ));
    }

    #[test]
    fn recall_cursor_max_budget_identity_state_fits_windows_safe_cap() {
        let mut state = RecallContinuationState::new(
            "large query",
            "0123456789abcdef0123456789abcdef".to_string(),
        )
        .expect("state should build");
        let identity_count = 1_600_u64;
        for node_id in 1..=identity_count {
            state
                .insert_seen_node(node_id as i64)
                .expect("identity should insert");
            state.roots.push(RecallContinuationRoot {
                node_id: node_id as i64,
                node_type: "raw_note".to_string(),
            });
        }
        state.emitted_count = identity_count;
        state.task_node_bytes = (TASK_RECALL_SOFT_BUDGET_BYTES
            - empty_task_section_byte_len(false).expect("empty section should count"))
            as u64;

        let cursor = encode_recall_continuation_cursor(&state)
            .expect("max-budget identity state should fit");

        assert!(cursor.len() <= MAX_RECALL_CONTINUATION_CURSOR_BYTES);
        assert!(cursor.len() < 12 * 1024, "binary cursor unexpectedly grew");
        assert_eq!(
            decode_recall_continuation_cursor(&cursor, "large query")
                .expect("stress cursor should decode"),
            state
        );
    }

    #[test]
    fn canonical_json_byte_count_matches_complete_utf8_node_serialization() {
        let mut mandatory_node = node(7, "project_profile", "active", "Проект");
        mandatory_node.body = Some("Полное тело памяти".to_string());
        let selected = SelectedRecallNode {
            node: mandatory_node,
            selection_reasons: vec![RecallSelectionReason::MandatoryContext {
                mandatory_type: MandatoryContextType::ProjectProfile,
            }],
        };
        let serialized = serde_json::to_vec(&selected).expect("selected node should serialize");

        assert_eq!(
            canonical_json_byte_len(&selected).expect("byte count should pass"),
            serialized.len()
        );
        assert!(String::from_utf8(serialized)
            .expect("JSON should be UTF-8")
            .contains("Полное тело памяти"));
    }

    #[test]
    fn reusable_task_selector_matches_public_wrapper_exact_json_and_order() {
        let mandatory_node = node(1, "rule", "active", "Mandatory rule");
        let mandatory = build_mandatory_recall_context(vec![mandatory_node.clone()])
            .expect("mandatory fixture should fit");
        let typed = node(2, "workflow", "draft", "Deploy release");
        let fts_only = node(3, "raw_note", "draft", "Release notes");
        let candidates = TaskRecallCandidates {
            typed_roots: vec![typed.clone()],
            fts_results: vec![
                FtsNodeSearchResult {
                    rank: -1.0,
                    node: typed,
                },
                FtsNodeSearchResult {
                    rank: -100.0,
                    node: mandatory_node,
                },
                FtsNodeSearchResult {
                    rank: -2.0,
                    node: fts_only,
                },
            ],
            ..TaskRecallCandidates::default()
        };
        let selector = TaskRecallCandidateSelector::new(&mandatory.section);

        let wrapper_selected =
            select_task_recall_candidates(candidates.clone(), &mandatory.section);
        let first_selected = selector.select(candidates.clone());
        let repeated_selected = selector.select(candidates);

        assert_eq!(first_selected, wrapper_selected);
        assert_eq!(repeated_selected, wrapper_selected);
        assert_eq!(
            serde_json::to_value(&wrapper_selected).expect("selection should serialize"),
            serde_json::json!([
                {
                    "node": {
                        "id": 2,
                        "node_type": "workflow",
                        "status": "draft",
                        "title": "Deploy release",
                        "summary": null,
                        "body": null,
                        "source_ref": null,
                        "confidence": null,
                        "trust_level": null,
                        "created_at": "2026-01-01T00:00:00Z",
                        "updated_at": "2026-01-01T00:00:00Z"
                    },
                    "selection_reasons": [
                        {"kind": "typed_root", "node_type": "workflow"},
                        {"kind": "fts_bm25", "rank": -1.0}
                    ]
                },
                {
                    "node": {
                        "id": 3,
                        "node_type": "raw_note",
                        "status": "draft",
                        "title": "Release notes",
                        "summary": null,
                        "body": null,
                        "source_ref": null,
                        "confidence": null,
                        "trust_level": null,
                        "created_at": "2026-01-01T00:00:00Z",
                        "updated_at": "2026-01-01T00:00:00Z"
                    },
                    "selection_reasons": [
                        {"kind": "fts_bm25", "rank": -2.0}
                    ]
                }
            ])
        );
    }

    #[test]
    fn task_recall_keeps_pipeline_order_merges_reasons_and_skips_mandatory_ids() {
        let mandatory_node = node(1, "rule", "active", "Mandatory rule");
        let mandatory = build_mandatory_recall_context(vec![mandatory_node.clone()])
            .expect("mandatory fixture should fit");
        let mut typed = node(2, "workflow", "draft", "Deploy release");
        typed.body = Some("complete workflow body".to_string());
        let mut weaker_fts = node(3, "raw_note", "draft", "Release notes");
        weaker_fts.body = Some("complete FTS body".to_string());
        let mut linked = node(4, "lesson", "draft", "Old linked lesson");
        linked.body = Some("complete direct body".to_string());
        let candidates = TaskRecallCandidates {
            typed_roots: vec![typed.clone()],
            fts_results: vec![
                FtsNodeSearchResult {
                    rank: -100.0,
                    node: weaker_fts.clone(),
                },
                FtsNodeSearchResult {
                    rank: -1.0,
                    node: typed,
                },
                FtsNodeSearchResult {
                    rank: -0.5,
                    node: mandatory_node,
                },
            ],
            direct_nodes: vec![crate::storage::DirectRecallNode {
                root_node_id: 2,
                link: Link {
                    id: 8,
                    source_node_id: 2,
                    target_node_id: 4,
                    link_type: "has_lesson".to_string(),
                    created_at: "2026-01-01T00:00:00Z".to_string(),
                },
                node: linked,
            }],
            graph_nodes: Vec::new(),
            more_results: false,
        };

        let task = build_task_recall_context(candidates, &mandatory.section)
            .expect("task context should build");

        assert!(task.section.complete);
        assert!(!task.more_results);
        assert_eq!(
            task.section
                .nodes
                .iter()
                .map(|selected| selected.node.id)
                .collect::<Vec<_>>(),
            [2, 3, 4]
        );
        assert_eq!(
            task.section.nodes[0].node.body.as_deref(),
            Some("complete workflow body")
        );
        assert!(matches!(
            task.section.nodes[0].selection_reasons[0],
            RecallSelectionReason::TypedRoot { .. }
        ));
        assert!(matches!(
            task.section.nodes[0].selection_reasons[1],
            RecallSelectionReason::FtsBm25 { .. }
        ));
        assert!(matches!(
            task.section.nodes[2].selection_reasons[0],
            RecallSelectionReason::DirectLink {
                source_node_id: 2,
                ..
            }
        ));
        assert_eq!(
            task.used_bytes,
            serde_json::to_vec(&task.section)
                .expect("task section should serialize")
                .len()
        );
    }

    #[test]
    fn task_recall_packing_never_splits_nodes_and_reports_exact_incomplete_budget() {
        let mut first = node(10, "workflow", "draft", "First workflow");
        first.body = Some("a".repeat(150_000));
        let first_body = first.body.clone();
        let mut second = node(11, "tool_contract", "draft", "Second tool");
        second.body = Some("b".repeat(150_000));
        let candidates = TaskRecallCandidates {
            typed_roots: vec![first, second],
            ..TaskRecallCandidates::default()
        };

        let task = build_task_recall_context(
            candidates,
            &RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("bounded task context should build");

        assert!(!task.section.complete);
        assert!(task.more_results);
        assert_eq!(task.section.nodes.len(), 1);
        assert_eq!(task.section.nodes[0].node.body, first_body);
        assert!(task.used_bytes <= TASK_RECALL_SOFT_BUDGET_BYTES);
        assert_eq!(
            task.used_bytes,
            serde_json::to_vec(&task.section)
                .expect("task section should serialize")
                .len()
        );
    }

    #[test]
    fn task_recall_globally_deduplicates_node_and_sorts_distinct_reasons() {
        let shared = node(20, "workflow", "draft", "Shared workflow");
        let candidates = TaskRecallCandidates {
            typed_roots: vec![shared.clone()],
            fts_results: vec![FtsNodeSearchResult {
                rank: -3.0,
                node: shared.clone(),
            }],
            direct_nodes: vec![crate::storage::DirectRecallNode {
                root_node_id: 10,
                link: Link {
                    id: 1,
                    source_node_id: 10,
                    target_node_id: shared.id,
                    link_type: "uses".to_string(),
                    created_at: "2026-01-01T00:00:00Z".to_string(),
                },
                node: shared.clone(),
            }],
            graph_nodes: vec![
                graph_candidate(shared.clone(), 10, "workflow", 10, 1, "uses", 1),
                graph_candidate(shared, 10, "workflow", 10, 1, "uses", 1),
            ],
            more_results: false,
        };

        let task = build_task_recall_context(
            candidates,
            &RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("task context should build");

        assert_eq!(task.section.nodes.len(), 1);
        let reasons = &task.section.nodes[0].selection_reasons;
        assert_eq!(reasons.len(), 5);
        assert!(matches!(
            reasons[0],
            RecallSelectionReason::TypedRoot { .. }
        ));
        assert!(matches!(reasons[1], RecallSelectionReason::FtsBm25 { .. }));
        assert!(matches!(
            reasons[2],
            RecallSelectionReason::DirectLink { .. }
        ));
        assert!(matches!(
            reasons[3],
            RecallSelectionReason::GraphTraversal { .. }
        ));
        assert!(matches!(
            reasons[4],
            RecallSelectionReason::Expansion { .. }
        ));
    }

    #[test]
    fn task_recall_adds_workflow_tool_and_failure_expansion_reasons() {
        let workflow = node(1, "workflow", "draft", "Workflow root");
        let tool = node(2, "tool_contract", "draft", "Tool root");
        let failure = node(3, "failure_mode", "draft", "Failure root");
        let workflow_rule = node(11, "rule", "draft", "Workflow rule");
        let tool_skill = node(12, "skill", "draft", "Tool skill");
        let failure_correction = node(13, "correction", "draft", "Failure correction");
        let candidates = TaskRecallCandidates {
            typed_roots: vec![workflow, tool, failure],
            graph_nodes: vec![
                graph_candidate(workflow_rule, 1, "workflow", 1, 1, "rule", 1),
                graph_candidate(tool_skill, 2, "tool_contract", 2, 2, "skill", 1),
                graph_candidate(
                    failure_correction,
                    3,
                    "failure_mode",
                    3,
                    3,
                    "corrected_by",
                    1,
                ),
            ],
            ..TaskRecallCandidates::default()
        };

        let task = build_task_recall_context(
            candidates,
            &RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("task context should build");

        for (node_id, expected_type) in [
            (11, RecallExpansionType::Workflow),
            (12, RecallExpansionType::Tool),
            (13, RecallExpansionType::FailureMode),
        ] {
            let selected = task
                .section
                .nodes
                .iter()
                .find(|selected| selected.node.id == node_id)
                .expect("expanded node should be selected");
            assert!(selected.selection_reasons.iter().any(|reason| {
                matches!(
                    reason,
                    RecallSelectionReason::Expansion { expansion_type, .. }
                        if *expansion_type == expected_type
                )
            }));
            assert!(selected
                .selection_reasons
                .iter()
                .any(|reason| matches!(reason, RecallSelectionReason::GraphTraversal { .. })));
        }
    }

    #[test]
    fn task_recall_orders_tier_by_source_then_trust_confidence_and_id() {
        let mut external = node(1, "lesson", "draft", "External");
        external.source_ref = Some("source=external/docs".to_string());
        external.trust_level = Some("high".to_string());
        external.confidence = Some(1.0);
        let mut low_trust = node(10, "lesson", "draft", "Low trust");
        low_trust.source_ref = Some("source=user_instruction/chat".to_string());
        low_trust.trust_level = Some("low".to_string());
        low_trust.confidence = Some(1.0);
        let mut lower_confidence = node(11, "lesson", "draft", "Lower confidence");
        lower_confidence.source_ref = Some("source=user_instruction/chat".to_string());
        lower_confidence.trust_level = Some("high".to_string());
        lower_confidence.confidence = Some(0.2);
        let mut higher_confidence = node(12, "lesson", "draft", "Higher confidence");
        higher_confidence.source_ref = Some("source=user_instruction/chat".to_string());
        higher_confidence.trust_level = Some("high".to_string());
        higher_confidence.confidence = Some(0.9);
        let candidates = TaskRecallCandidates {
            fts_results: vec![
                FtsNodeSearchResult {
                    rank: -4.0,
                    node: external,
                },
                FtsNodeSearchResult {
                    rank: -3.0,
                    node: low_trust,
                },
                FtsNodeSearchResult {
                    rank: -2.0,
                    node: lower_confidence,
                },
                FtsNodeSearchResult {
                    rank: -1.0,
                    node: higher_confidence,
                },
            ],
            ..TaskRecallCandidates::default()
        };

        let task = build_task_recall_context(
            candidates,
            &RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("task context should build");

        assert_eq!(
            task.section
                .nodes
                .iter()
                .map(|selected| selected.node.id)
                .collect::<Vec<_>>(),
            [12, 11, 10, 1]
        );
    }

    #[test]
    fn task_recall_uses_bm25_before_id_only_inside_equal_fts_priority() {
        let mut weak = node(1, "raw_note", "draft", "Weak early row");
        weak.source_ref = Some("source=teach/session".to_string());
        weak.trust_level = Some("high".to_string());
        weak.confidence = Some(0.8);
        let mut strong = node(99, "workflow", "draft", "Strong late row");
        strong.source_ref = weak.source_ref.clone();
        strong.trust_level = weak.trust_level.clone();
        strong.confidence = weak.confidence;
        let candidates = TaskRecallCandidates {
            fts_results: vec![
                FtsNodeSearchResult {
                    rank: -0.1,
                    node: weak,
                },
                FtsNodeSearchResult {
                    rank: -12.0,
                    node: strong,
                },
            ],
            ..TaskRecallCandidates::default()
        };

        let task = build_task_recall_context(
            candidates,
            &RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("FTS ordering should build");

        assert_eq!(
            task.section
                .nodes
                .iter()
                .map(|selected| selected.node.id)
                .collect::<Vec<_>>(),
            [99, 1]
        );
        assert!(matches!(
            task.section.nodes[0].selection_reasons.as_slice(),
            [RecallSelectionReason::FtsBm25 { rank }] if *rank == -12.0
        ));
    }

    #[test]
    fn task_recall_builder_is_deterministic_and_every_node_explains_selection() {
        let root = node(1, "failure_mode", "draft", "Root");
        let related = node(2, "correction", "draft", "Related");
        let candidates = TaskRecallCandidates {
            typed_roots: vec![root],
            graph_nodes: vec![graph_candidate(
                related,
                1,
                "failure_mode",
                1,
                9,
                "corrected_by",
                1,
            )],
            more_results: true,
            ..TaskRecallCandidates::default()
        };

        let first = build_task_recall_context(
            candidates.clone(),
            &RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("first task context should build");
        let second = build_task_recall_context(
            candidates,
            &RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("second task context should build");

        assert_eq!(first, second);
        assert!(first.more_results);
        assert!(!first.section.complete);
        assert_eq!(first.section.complete, !first.more_results);
        assert!(first.used_bytes <= TASK_RECALL_SOFT_BUDGET_BYTES);
        assert!(first
            .section
            .nodes
            .iter()
            .all(|selected| !selected.selection_reasons.is_empty()));
    }

    #[test]
    fn task_response_json_has_typed_selection_reasons_and_explicit_null_cursor() {
        let selected = SelectedRecallNode {
            node: node(7, "workflow", "draft", "Deploy release"),
            selection_reasons: vec![
                RecallSelectionReason::TypedRoot {
                    node_type: "workflow".to_string(),
                },
                RecallSelectionReason::FtsBm25 { rank: -2.5 },
                RecallSelectionReason::DirectLink {
                    source_node_id: 3,
                    link_type: "uses".to_string(),
                },
            ],
        };
        let response = RecallResponseV2 {
            bundle_id: RecallBundleId::parse("550e8400-e29b-41d4-a716-446655440000")
                .expect("fixture should be UUID v4"),
            mode: RecallMode::Task,
            mandatory: RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
            task: RecallSection {
                complete: true,
                nodes: vec![selected],
            },
            more_results: false,
            continuation_cursor: None,
            budget: RecallBudgetMetadata::new(28, 400).expect("budget should build"),
        };

        let value = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(value["mode"], "task");
        assert_eq!(value["continuation_cursor"], serde_json::Value::Null);
        assert_eq!(value["more_results"], false);
        assert_eq!(value["task"]["complete"], true);
        assert_eq!(
            value["task"]["nodes"][0]["selection_reasons"][0]["kind"],
            "typed_root"
        );
        assert_eq!(
            value["task"]["nodes"][0]["selection_reasons"][1]["kind"],
            "fts_bm25"
        );
        assert_eq!(
            value["task"]["nodes"][0]["selection_reasons"][2]["kind"],
            "direct_link"
        );
    }

    #[test]
    fn canonical_json_and_budget_counters_fail_on_overflow() {
        assert!(matches!(
            canonical_json_byte_len_from(&"x", usize::MAX),
            Err(RecallModelError::ByteCountOverflow)
        ));
        assert!(matches!(
            RecallBudgetMetadata::new(usize::MAX, 1),
            Err(RecallModelError::ByteCountOverflow)
        ));
    }

    #[test]
    fn bundle_ids_are_canonical_lowercase_uuid_v4() {
        let generated = RecallBundleId::generate();
        assert_eq!(
            RecallBundleId::parse(generated.as_str()).expect("generated id should validate"),
            generated
        );
        assert_eq!(generated.as_str(), generated.as_str().to_ascii_lowercase());
        assert_eq!(generated.as_str().len(), 36);

        for invalid in [
            "550E8400-E29B-41D4-A716-446655440000",
            "550e8400e29b41d4a716446655440000",
            "550e8400-e29b-11d4-a716-446655440000",
            "550e8400-e29b-41d4-7716-446655440000",
            "not-a-uuid",
        ] {
            assert!(
                RecallBundleId::parse(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn mandatory_context_types_are_exact_and_typed() {
        let expected = [
            ("kernel_contract", MandatoryContextType::KernelContract),
            ("gate", MandatoryContextType::Gate),
            ("project_profile", MandatoryContextType::ProjectProfile),
            ("source", MandatoryContextType::Source),
            ("rule", MandatoryContextType::Rule),
        ];

        assert_eq!(
            MANDATORY_CONTEXT_NODE_TYPES,
            expected.map(|(node_type, _)| node_type)
        );
        for (node_type, mandatory_type) in expected {
            assert_eq!(
                MandatoryContextType::from_node_type(node_type),
                Some(mandatory_type)
            );
            assert!(is_mandatory_context_node_type(node_type));
        }
        assert_eq!(MandatoryContextType::from_node_type("workflow"), None);
        assert!(!is_mandatory_context_node_type("workflow"));
        assert!(is_active_mandatory_context_node(&node(
            1,
            "gate",
            "active",
            "Active gate"
        )));
        assert!(!is_active_mandatory_context_node(&node(
            2,
            "gate",
            "draft",
            "Draft gate"
        )));
    }

    #[test]
    fn mandatory_builder_keeps_full_bodies_and_uses_stable_type_then_id_order() {
        let mut rule = node(1, "rule", "active", "Rule");
        rule.body = Some("full-rule-body-".repeat(400));
        let gate_later = node(9, "gate", "active", "Later gate");
        let gate_earlier = node(3, "gate", "active", "Earlier gate");
        let project = node(4, "project_profile", "active", "Project");

        let context =
            build_mandatory_recall_context(vec![rule.clone(), gate_later, project, gate_earlier])
                .expect("mandatory context should fit");

        assert!(context.section.complete);
        assert_eq!(
            context
                .section
                .nodes
                .iter()
                .map(|selected| selected.node.id)
                .collect::<Vec<_>>(),
            [3, 9, 4, 1]
        );
        assert_eq!(
            context.section.nodes[3].node.body, rule.body,
            "mandatory bodies must not use the old recall truncation limit"
        );
        assert_eq!(
            context.used_bytes,
            serde_json::to_vec(&context.section)
                .expect("mandatory section should serialize")
                .len()
        );
    }

    #[test]
    fn mandatory_builder_accepts_exact_budget_and_fails_closed_one_byte_over() {
        let gate = node(10, "gate", "active", "Gate");
        let project = node(20, "project_profile", "active", "Project");
        let nodes = vec![project, gate];
        let exact = build_mandatory_recall_context_with_limit(nodes.clone(), usize::MAX)
            .expect("fixture should serialize")
            .used_bytes;

        let at_budget = build_mandatory_recall_context_with_limit(nodes.clone(), exact)
            .expect("exact budget must pass");
        let overflow = build_mandatory_recall_context_with_limit(nodes, exact - 1)
            .expect_err("one byte below exact size must fail");

        assert_eq!(at_budget.used_bytes, exact);
        assert_eq!(overflow.offending_node_ids(), Some([20_i64].as_slice()));
        assert!(matches!(
            overflow,
            MandatoryContextBuildError::Overflow {
                hard_limit_bytes,
                ..
            } if hard_limit_bytes == exact - 1
        ));
    }

    #[test]
    fn mandatory_overflow_reports_only_the_stable_tail_ids() {
        let first = node(10, "gate", "active", "First gate");
        let mut second = node(20, "project_profile", "active", "Large project");
        second.body = Some("x".repeat(2_000));
        let third = node(30, "rule", "active", "Rule after overflow");
        let first_only = build_mandatory_recall_context_with_limit(vec![first.clone()], usize::MAX)
            .expect("first node should serialize")
            .used_bytes;

        let error =
            build_mandatory_recall_context_with_limit(vec![third, second, first], first_only)
                .expect_err("remaining mandatory nodes must not be partially returned");

        assert_eq!(error.offending_node_ids(), Some([20_i64, 30].as_slice()));
        assert!(!error.to_string().contains("Large project"));
        assert!(!error.to_string().contains(&"x".repeat(128)));
    }

    #[test]
    fn mandatory_hard_budget_rejects_a_single_max_size_body() {
        let mut gate = node(44, "gate", "active", "Large gate");
        gate.body = Some("x".repeat(MANDATORY_RECALL_HARD_BUDGET_BYTES));

        let error = build_mandatory_recall_context(vec![gate])
            .expect_err("JSON framing must put the max-size body over the hard budget");

        assert_eq!(error.offending_node_ids(), Some([44_i64].as_slice()));
        assert!(matches!(
            error,
            MandatoryContextBuildError::Overflow {
                hard_limit_bytes: MANDATORY_RECALL_HARD_BUDGET_BYTES,
                ..
            }
        ));
    }

    #[test]
    fn mandatory_builder_rejects_inactive_or_non_mandatory_input() {
        for invalid in [
            node(1, "gate", "draft", "Draft gate"),
            node(2, "workflow", "active", "Workflow"),
        ] {
            assert!(matches!(
                build_mandatory_recall_context(vec![invalid]),
                Err(MandatoryContextBuildError::InvalidNode { .. })
            ));
        }
    }

    #[test]
    fn structured_bundle_groups_project_profiles_gates_and_workflows_by_status() {
        let bundle = build_structured_bundle(vec![
            node(1, "project_profile", "active", "Project profile"),
            node(2, "gate", "draft", "Draft gate"),
            node(3, "workflow", "broken", "Broken workflow"),
            node(4, "decision", "active", "Ignored decision"),
            node(5, "workflow", "superseded", "Old workflow"),
        ]);

        assert_eq!(bundle.project_profiles.active[0].title, "Project profile");
        assert_eq!(bundle.gates.draft[0].title, "Draft gate");
        assert_eq!(bundle.workflows.broken[0].title, "Broken workflow");
        assert!(bundle.workflows.superseded.is_empty());
        assert!(bundle.gates.active.is_empty());
        assert!(bundle.project_profiles.draft.is_empty());
        assert!(bundle.linked_nodes.is_empty());
        assert!(bundle.fts_fallback.is_empty());
        assert!(bundle.hunches.is_empty());
    }

    #[test]
    fn compact_node_index_preserves_bundle_order_and_first_id_match() {
        let bundle = build_structured_bundle(vec![
            node(7, "gate", "active", "Later gate"),
            node(7, "workflow", "active", "First workflow"),
            node(8, "project_profile", "active", "Profile"),
        ]);
        let index = BundleNodeIndex::new(&bundle);

        assert_eq!(
            index
                .node_by_id(7)
                .expect("duplicate id should resolve")
                .title,
            "First workflow"
        );
        assert_eq!(
            index
                .nodes_by_types(&["workflow", "gate"])
                .into_iter()
                .map(|node| node.title.as_str())
                .collect::<Vec<_>>(),
            vec!["First workflow", "Later gate"]
        );
    }

    #[test]
    fn structured_bundle_excludes_deprecated_and_superseded_from_normal_sections() {
        let bundle = build_structured_bundle(vec![
            node(1, "project_profile", "deprecated", "Old profile"),
            node(2, "gate", "superseded", "Old gate"),
            node(3, "workflow", "active", "Live workflow"),
        ]);

        assert!(bundle.project_profiles.deprecated.is_empty());
        assert!(bundle.gates.superseded.is_empty());
        assert_eq!(bundle.workflows.active[0].title, "Live workflow");
    }

    #[test]
    fn structured_bundle_keeps_direct_tool_contract_in_compact_output() {
        let bundle = build_structured_bundle(vec![node(
            1,
            "tool_contract",
            "active",
            "Direct tool contract",
        )]);
        let serialized = serde_json::to_value(&bundle).expect("bundle should serialize");

        assert_eq!(bundle.compact.tool_contracts.len(), 1);
        assert_eq!(
            bundle.compact.tool_contracts[0].title,
            "Direct tool contract"
        );
        assert!(serialized.get("context_nodes").is_none());
    }

    #[test]
    fn structured_bundle_keeps_direct_rule_in_serialized_compact_output() {
        let bundle = build_structured_bundle(vec![node(1, "rule", "active", "Direct safety rule")]);
        let serialized = serde_json::to_value(&bundle).expect("bundle should serialize");

        assert_eq!(bundle.compact.rules.len(), 1);
        assert_eq!(bundle.compact.rules[0].title, "Direct safety rule");
        assert_eq!(
            serialized["compact"]["rules"][0]["title"],
            "Direct safety rule"
        );
        assert!(serialized.get("context_nodes").is_none());
    }

    #[test]
    fn structured_bundle_traverses_links_from_selected_nodes_with_depth_limit() {
        let bundle = build_structured_bundle_with_links(
            vec![
                node(1, "workflow", "active", "Workflow"),
                node(2, "decision", "active", "Depth one"),
                node(3, "lesson", "active", "Depth two"),
                node(4, "project_fact", "active", "Too deep"),
            ],
            vec![
                link(1, 1, 2, "supports"),
                link(2, 2, 3, "supports"),
                link(3, 3, 4, "supports"),
            ],
        );

        assert_eq!(bundle.linked_nodes.len(), 2);
        assert_eq!(bundle.linked_nodes[0].depth, 1);
        assert_eq!(bundle.linked_nodes[0].node.title, "Depth one");
        assert_eq!(bundle.linked_nodes[1].depth, 2);
        assert_eq!(bundle.linked_nodes[1].node.title, "Depth two");
    }

    #[test]
    fn structured_bundle_excludes_deprecated_and_superseded_from_traversal() {
        let bundle = build_structured_bundle_with_links(
            vec![
                node(1, "workflow", "active", "Workflow"),
                node(2, "decision", "deprecated", "Deprecated"),
                node(3, "lesson", "superseded", "Superseded"),
                node(4, "project_fact", "active", "Active"),
            ],
            vec![
                link(1, 1, 2, "supports"),
                link(2, 1, 3, "supports"),
                link(3, 1, 4, "supports"),
            ],
        );

        assert_eq!(bundle.linked_nodes.len(), 1);
        assert_eq!(bundle.linked_nodes[0].node.title, "Active");
    }

    #[test]
    fn fts_fallback_is_used_only_when_structured_recall_is_small() {
        let small = build_structured_bundle(vec![node(1, "workflow", "active", "Needle")]);
        let enough = build_structured_bundle(vec![
            node(1, "workflow", "active", "One"),
            node(2, "gate", "active", "Two"),
            node(3, "project_profile", "active", "Three"),
        ]);
        let duplicate_ids = build_structured_bundle(vec![
            node(1, "workflow", "active", "One"),
            node(1, "gate", "active", "Duplicate one"),
            node(2, "project_profile", "active", "Two"),
        ]);

        assert!(needs_fts_fallback(&small));
        assert!(!needs_fts_fallback(&enough));
        assert!(needs_fts_fallback(&duplicate_ids));
        assert_eq!(
            derive_fts_fallback_query(&small),
            Some("Needle".to_string())
        );
    }

    #[test]
    fn fts_fallback_query_preserves_status_and_section_priority() {
        let bundle = build_structured_bundle(vec![
            node(1, "project_profile", "active", "  "),
            node(2, "project_profile", "draft", " Draft profile "),
            node(3, "gate", "active", "Active gate"),
        ]);

        assert_eq!(
            derive_fts_fallback_query(&bundle),
            Some("Draft profile".to_string())
        );
    }

    #[test]
    fn fts_fallback_is_additive_and_filters_existing_and_old_nodes() {
        let bundle = build_structured_bundle(vec![node(1, "workflow", "active", "Root")]);
        let with_fallback = add_fts_fallback(
            bundle,
            vec![
                fts_result(1, "active", "Existing"),
                fts_result(2, "deprecated", "Old"),
                fts_result(3, "active", "Fallback"),
            ],
        );

        assert_eq!(with_fallback.fts_fallback.len(), 1);
        assert_eq!(with_fallback.fts_fallback[0].node.title, "Fallback");
    }

    #[test]
    fn bounded_query_recall_preserves_fts_matches_without_node_bodies() {
        let results = vec![
            fts_result_with_rank(
                2,
                "lesson",
                "draft",
                "First match",
                0.1,
                "2026-01-02T00:00:00Z",
            ),
            fts_result_with_rank(
                3,
                "workflow",
                "active",
                "Second match",
                0.2,
                "2026-01-01T00:00:00Z",
            ),
        ];

        let bounded = build_bounded_query_recall(results);

        assert_eq!(bounded.matches.len(), 2);
        assert_eq!(bounded.matches[0].node.id, 2);
        assert_eq!(bounded.matches[1].node.id, 3);
        assert!(bounded
            .matches
            .iter()
            .all(|result| result.node.body.is_none()));
        assert_eq!(bounded.compact.applicable_workflows.len(), 1);
        assert_eq!(bounded.compact.applicable_workflows[0].node_id, 3);
    }

    #[test]
    fn hunches_are_selected_from_fts_by_type_rank_hotness_and_id() {
        let bundle = build_structured_bundle(vec![node(1, "workflow", "active", "Root")]);
        let with_hunches = add_fts_fallback(
            bundle,
            vec![
                fts_result_with_rank(2, "raw_note", "active", "Raw", 0.1, "2026-01-03T00:00:00Z"),
                fts_result_with_rank(
                    3,
                    "workflow",
                    "active",
                    "Workflow",
                    0.4,
                    "2026-01-01T00:00:00Z",
                ),
                fts_result_with_rank(
                    4,
                    "failure_mode",
                    "active",
                    "Failure",
                    0.9,
                    "2026-01-02T00:00:00Z",
                ),
                fts_result_with_rank(
                    5,
                    "tool_contract",
                    "active",
                    "Tool",
                    0.2,
                    "2026-01-04T00:00:00Z",
                ),
                fts_result_with_rank(
                    6,
                    "tool_contract",
                    "active",
                    "Older tool",
                    0.2,
                    "2026-01-02T00:00:00Z",
                ),
            ],
        );

        assert_eq!(with_hunches.hunches.len(), 3);
        assert_eq!(with_hunches.hunches[0].source_node_id, 4);
        assert_eq!(
            with_hunches.hunches[0].reason,
            "fts_match_failure_mode_hotness"
        );
        assert_eq!(with_hunches.hunches[1].source_node_id, 5);
        assert_eq!(with_hunches.hunches[2].source_node_id, 6);
    }

    #[test]
    fn hunches_include_linked_workflow_tool_or_failure_mode_signal() {
        let bundle = build_structured_bundle_with_links(
            vec![
                node(1, "workflow", "active", "Root"),
                node(2, "failure_mode", "active", "Linked failure"),
            ],
            vec![link(1, 1, 2, "warns")],
        );
        let with_hunches = add_fts_fallback(
            bundle,
            vec![fts_result_with_rank(
                3,
                "raw_note",
                "active",
                "FTS match",
                0.1,
                "2026-01-03T00:00:00Z",
            )],
        );

        assert_eq!(with_hunches.hunches.len(), 1);
        assert_eq!(with_hunches.hunches[0].source_node_id, 3);
        assert_eq!(with_hunches.hunches[0].linked_signal_node_id, Some(2));
        assert_eq!(
            with_hunches.hunches[0].linked_signal_node_type,
            Some("failure_mode".to_string())
        );
        assert_eq!(
            with_hunches.hunches[0].reason,
            "fts_match_linked_signal_hotness"
        );
    }

    #[test]
    fn compact_bundle_has_source_confidence_trust_and_section_limits() {
        let bundle = build_structured_bundle(vec![
            sourced_node(1, "workflow", "active", "Workflow 1"),
            sourced_node(2, "workflow", "active", "Workflow 2"),
            sourced_node(3, "gate", "active", "Gate 1"),
            sourced_node(4, "gate", "active", "Gate 2"),
            sourced_node(5, "gate", "active", "Gate 3"),
            sourced_node(6, "gate", "active", "Gate 4"),
            sourced_node(7, "project_profile", "active", "Profile"),
        ]);
        let bundle = add_fts_fallback(
            bundle,
            vec![
                sourced_fts_result(8, "tool_contract", "Tool"),
                sourced_fts_result(9, "mcp_profile", "MCP"),
                sourced_fts_result(10, "lesson", "Lesson"),
                sourced_fts_result(11, "correction", "Correction"),
            ],
        );

        assert_eq!(bundle.compact.applicable_workflows.len(), 1);
        assert_eq!(bundle.compact.active_gates.len(), 3);
        assert_eq!(bundle.compact.tool_contracts.len(), 1);
        assert_eq!(bundle.compact.mcp_profiles.len(), 1);
        assert_eq!(bundle.compact.project_profile_facts.len(), 1);
        assert_eq!(bundle.compact.relevant_corrections_lessons.len(), 2);
        assert_eq!(
            bundle.compact.applicable_workflows[0].source_ref,
            Some("source=user_instruction".to_string())
        );
        assert_eq!(bundle.compact.applicable_workflows[0].confidence, Some(0.8));
        assert_eq!(
            bundle.compact.applicable_workflows[0].trust_level,
            Some("high".to_string())
        );
        assert_eq!(
            bundle.compact.applicable_workflows[0]
                .source_hierarchy
                .as_ref()
                .map(|hierarchy| hierarchy.source_root.as_str()),
            Some("user_instruction")
        );
        assert!(bundle.compact.source_refs.len() <= bundle.compact.limits.source_refs);
    }

    #[test]
    fn compact_bundle_prefers_higher_priority_sources() {
        let bundle = build_structured_bundle(vec![
            source_tuned_node(
                1,
                "workflow",
                "active",
                "External",
                "source=external/api",
                0.99,
            ),
            source_tuned_node(
                2,
                "workflow",
                "active",
                "Instruction",
                "source=user_instruction",
                0.70,
            ),
        ]);

        assert_eq!(bundle.compact.applicable_workflows.len(), 1);
        assert_eq!(bundle.compact.applicable_workflows[0].node_id, 2);
    }

    #[test]
    fn compact_tool_and_mcp_nodes_include_least_privilege_metadata() {
        let bundle = add_fts_fallback(
            build_structured_bundle(vec![sourced_node(99, "workflow", "active", "Root")]),
            vec![
                FtsNodeSearchResult {
                    rank: 0.0,
                    node: privileged_node(
                        1,
                        "tool_contract",
                        "Tool",
                        "source=tool/context-export",
                        r#"{
                            "side_effects":"local_write_artifact",
                            "approval_requirement":"manual_review",
                            "read_operations":["memory.read"],
                            "write_operations":["artifact.write"]
                        }"#,
                    ),
                },
                FtsNodeSearchResult {
                    rank: 0.0,
                    node: privileged_node(
                        2,
                        "mcp_profile",
                        "MCP",
                        "source=mcp/corporate/github",
                        r#"{
                            "side_effects":"external_read",
                            "approval_requirement":"none",
                            "read_operations":["repos.read"],
                            "write_operations":[]
                        }"#,
                    ),
                },
            ],
        );

        assert_eq!(
            bundle.compact.tool_contracts[0]
                .least_privilege
                .as_ref()
                .map(|metadata| metadata.side_effects.as_str()),
            Some("local_write_artifact")
        );
        assert_eq!(
            bundle.compact.mcp_profiles[0]
                .least_privilege
                .as_ref()
                .map(|metadata| metadata.side_effects.as_str()),
            Some("external_read")
        );
    }

    #[test]
    fn hunches_and_compact_hunches_are_capped_at_max_count() {
        let bundle = build_structured_bundle(vec![node(1, "workflow", "active", "Root")]);
        let with_hunches = add_fts_fallback(
            bundle,
            vec![
                fts_result_with_rank(
                    2,
                    "failure_mode",
                    "active",
                    "One",
                    0.1,
                    "2026-01-06T00:00:00Z",
                ),
                fts_result_with_rank(
                    3,
                    "failure_mode",
                    "active",
                    "Two",
                    0.1,
                    "2026-01-05T00:00:00Z",
                ),
                fts_result_with_rank(
                    4,
                    "failure_mode",
                    "active",
                    "Three",
                    0.1,
                    "2026-01-04T00:00:00Z",
                ),
                fts_result_with_rank(
                    5,
                    "failure_mode",
                    "active",
                    "Four",
                    0.1,
                    "2026-01-03T00:00:00Z",
                ),
                fts_result_with_rank(
                    6,
                    "failure_mode",
                    "active",
                    "Five",
                    0.1,
                    "2026-01-02T00:00:00Z",
                ),
            ],
        );

        assert_eq!(with_hunches.hunches.len(), MAX_HUNCHES);
        assert_eq!(with_hunches.compact.hunches.len(), MAX_HUNCHES);
        assert_eq!(with_hunches.hunches[0].source_node_id, 2);
        assert_eq!(with_hunches.hunches[2].source_node_id, 4);
    }

    fn link(id: i64, source_node_id: i64, target_node_id: i64, link_type: &str) -> Link {
        Link {
            id,
            source_node_id,
            target_node_id,
            link_type: link_type.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn fts_result(id: i64, status: &str, title: &str) -> FtsNodeSearchResult {
        fts_result_with_rank(id, "raw_note", status, title, 0.0, "2026-01-01T00:00:00Z")
    }

    fn sourced_node(id: i64, node_type: &str, status: &str, title: &str) -> Node {
        let mut node = node(id, node_type, status, title);
        node.source_ref = Some("source=user_instruction".to_string());
        node.confidence = Some(0.8);
        node.trust_level = Some("high".to_string());
        node
    }

    fn source_tuned_node(
        id: i64,
        node_type: &str,
        status: &str,
        title: &str,
        source_ref: &str,
        confidence: f64,
    ) -> Node {
        let mut node = node(id, node_type, status, title);
        node.source_ref = Some(source_ref.to_string());
        node.confidence = Some(confidence);
        node.trust_level = Some("high".to_string());
        node
    }

    fn privileged_node(
        id: i64,
        node_type: &str,
        title: &str,
        source_ref: &str,
        body: &str,
    ) -> Node {
        let mut node = source_tuned_node(id, node_type, "active", title, source_ref, 0.8);
        node.body = Some(body.to_string());
        node
    }

    fn sourced_fts_result(id: i64, node_type: &str, title: &str) -> FtsNodeSearchResult {
        FtsNodeSearchResult {
            rank: 0.0,
            node: sourced_node(id, node_type, "active", title),
        }
    }

    fn fts_result_with_rank(
        id: i64,
        node_type: &str,
        status: &str,
        title: &str,
        rank: f64,
        updated_at: &str,
    ) -> FtsNodeSearchResult {
        let mut node = node(id, node_type, status, title);
        node.updated_at = updated_at.to_string();
        FtsNodeSearchResult { rank, node }
    }
}
