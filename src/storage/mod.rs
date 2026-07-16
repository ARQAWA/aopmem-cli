use std::collections::{BTreeSet, HashMap};
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use rusqlite::functions::FunctionFlags;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit;
use crate::audit::AuditError;
use crate::schema;

const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";
const HOME_ENV: &str = "HOME";
const USERPROFILE_ENV: &str = "USERPROFILE";
const FNV1A_32_OFFSET: u32 = 0x811c9dc5;
const FNV1A_32_PRIME: u32 = 0x01000193;
const STORAGE_AUDIT_SOURCE: &str = "aopmem_cli";
const BOUNDED_RECALL_FIELD_MAX_CHARS: i64 = 1024;
const LEGACY_RECALL_ROOT_TYPES: &[&str] = &[
    "project_profile",
    "gate",
    "workflow",
    "tool_contract",
    "rule",
];
const TEACH_SESSION_SUMMARY: &str = "teach_session_v1";
const TEACH_MATERIAL_SUMMARY: &str = "teach_material_v1";
const TEACH_PROPOSAL_SUMMARY: &str = "teach_proposal_v1";
const TEACH_APPLY_SUMMARY: &str = "teach_apply_v1";
const TEACH_MATERIAL_LINK_TYPE: &str = "teach_has_material";
const TEACH_PROPOSAL_LINK_TYPE: &str = "teach_has_proposal";
const TEACH_APPLY_LINK_TYPE: &str = "teach_has_apply";
const TEACH_CREATED_LINK_TYPE: &str = "teach_created_node";
const TEACH_APPLY_SAVEPOINT: &str = "aopmem_teach_apply";
const INSERT_NODE_SQL: &str = "
    INSERT INTO nodes (
        node_type,
        status,
        title,
        summary,
        body,
        source_ref,
        confidence,
        trust_level
    )
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);
";
const GET_NODE_SQL: &str = "
    SELECT
        id,
        node_type,
        status,
        title,
        summary,
        body,
        source_ref,
        confidence,
        trust_level,
        created_at,
        updated_at
    FROM nodes
    WHERE id = ?1;
";
const INSERT_LINK_SQL: &str = "
    INSERT INTO links (source_node_id, target_node_id, link_type)
    VALUES (?1, ?2, ?3);
";
const GET_LINK_SQL: &str = "
    SELECT id, source_node_id, target_node_id, link_type, created_at
    FROM links
    WHERE id = ?1;
";
const INSERT_ALIAS_SQL: &str = "
    INSERT INTO aliases (node_id, alias)
    VALUES (?1, ?2);
";
const GET_ALIAS_SQL: &str = "
    SELECT id, node_id, alias, created_at
    FROM aliases
    WHERE id = ?1;
";
const INSERT_TAG_SQL: &str = "
    INSERT INTO tags (node_id, tag)
    VALUES (?1, ?2);
";
const GET_TAG_SQL: &str = "
    SELECT id, node_id, tag, created_at
    FROM tags
    WHERE id = ?1;
";
const INSERT_SOURCE_SQL: &str = "
    INSERT INTO sources (node_id, source_ref)
    VALUES (?1, ?2);
";
const GET_SOURCE_SQL: &str = "
    SELECT id, node_id, source_ref, created_at
    FROM sources
    WHERE id = ?1;
";
const FTS_ALIASES_SQL: &str = "
    SELECT COALESCE(group_concat(alias, ' '), '')
    FROM (
        SELECT alias
        FROM aliases
        WHERE node_id = ?1
        ORDER BY id ASC, alias ASC
    ) AS ordered_aliases;
";
const DELETE_FTS_NODE_SQL: &str = "DELETE FROM fts_nodes WHERE rowid = ?1;";
const INSERT_FTS_NODE_SQL: &str = "
    INSERT INTO fts_nodes(rowid, title, summary, body, aliases)
    VALUES (?1, ?2, ?3, ?4, ?5);
";

/// Upper bounds keep one memory record from making normal reads, FTS refreshes,
/// or audit snapshots unexpectedly large. Limits are measured in UTF-8 bytes.
pub const MAX_NODE_TITLE_BYTES: usize = 4 * 1024;
pub const MAX_NODE_SUMMARY_BYTES: usize = 16 * 1024;
pub const MAX_NODE_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_NODE_SOURCE_REF_BYTES: usize = 16 * 1024;
pub const MAX_NODE_TRUST_LEVEL_BYTES: usize = 256;
pub const MAX_LINK_TYPE_BYTES: usize = 256;
pub const MAX_METADATA_VALUE_BYTES: usize = 16 * 1024;
pub const MAX_PROPOSAL_ITEMS: usize = 1_000;
pub const MAX_PROPOSAL_NODE_REF_BYTES: usize = 256;
pub const MAX_MCP_ID_BYTES: usize = 256;
pub const MAX_MCP_NAME_BYTES: usize = 4 * 1024;
pub const MAX_MCP_FIELD_BYTES: usize = 16 * 1024;
pub const MAX_MCP_NOTES_BYTES: usize = 64 * 1024;
pub const LEGACY_RECALL_ROOT_LIMIT_PER_TYPE: usize = 12;
pub const LEGACY_RECALL_LINK_LIMIT: usize = 64;

pub const ALLOWED_NODE_TYPES: &[&str] = &[
    "kernel_contract",
    "gate",
    "rule",
    "workflow",
    "skill",
    "tool_contract",
    "mcp_profile",
    "project_profile",
    "project_fact",
    "decision",
    "correction",
    "lesson",
    "failure_mode",
    "incident_scar",
    "preference",
    "reflection_observation",
    "raw_note",
    "hunch_source",
    "source",
];
pub const ALLOWED_NODE_STATUSES: &[&str] =
    &["draft", "active", "deprecated", "superseded", "broken"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AopmemPaths {
    home: PathBuf,
    bin: PathBuf,
    skills: PathBuf,
    templates: PathBuf,
    workspaces: PathBuf,
}

impl AopmemPaths {
    pub fn home(&self) -> &PathBuf {
        &self.home
    }

    pub fn bin(&self) -> &PathBuf {
        &self.bin
    }

    pub fn skills(&self) -> &PathBuf {
        &self.skills
    }

    pub fn templates(&self) -> &PathBuf {
        &self.templates
    }

    pub fn workspaces(&self) -> &PathBuf {
        &self.workspaces
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePaths {
    root: PathBuf,
    db: PathBuf,
    observability: PathBuf,
    observability_db: PathBuf,
    tools: PathBuf,
    artifacts: PathBuf,
    audit_git: PathBuf,
    runtimes: PathBuf,
    logs: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Node {
    pub id: i64,
    pub node_type: String,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewNode {
    pub node_type: String,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BorrowedNodeInput<'a> {
    pub node_type: &'a str,
    pub status: &'a str,
    pub title: &'a str,
    pub summary: Option<&'a str>,
    pub body: Option<&'a str>,
    pub source_ref: Option<&'a str>,
    pub confidence: Option<f64>,
    pub trust_level: Option<&'a str>,
}

impl<'a> From<&'a NewNode> for BorrowedNodeInput<'a> {
    fn from(node: &'a NewNode) -> Self {
        Self {
            node_type: &node.node_type,
            status: &node.status,
            title: &node.title,
            summary: node.summary.as_deref(),
            body: node.body.as_deref(),
            source_ref: node.source_ref.as_deref(),
            confidence: node.confidence,
            trust_level: node.trust_level.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeUpdate {
    pub id: i64,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Link {
    pub id: i64,
    pub source_node_id: i64,
    pub target_node_id: i64,
    pub link_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FtsNodeSearchResult {
    pub rank: f64,
    pub node: Node,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoundedRecallSearch {
    pub results: Vec<FtsNodeSearchResult>,
    pub more_results: bool,
    pub content_truncated: bool,
}

/// One complete node reached by a direct outgoing link from a task root.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectRecallNode {
    pub root_node_id: i64,
    pub link: Link,
    pub node: Node,
}

/// One complete node reached by the bounded task-recall graph traversal.
///
/// `root_node_id` and `root_node_type` identify the deduplicated typed/FTS
/// root. `edge_source_node_id` and `link` describe the final edge that selected
/// the node. The traversal is limited to depths one and two.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphRecallNode {
    pub root_node_id: i64,
    pub root_node_type: String,
    pub edge_source_node_id: i64,
    pub link: Link,
    pub depth: usize,
    pub node: Node,
}

/// Bounded first-pass candidates for v0.2 task recall.
///
/// Each layer fetches at most the caller's limit and probes one extra row, so
/// the caller can report incomplete retrieval without loading an unbounded
/// result set. Every returned node contains its complete body.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskRecallCandidates {
    pub typed_roots: Vec<Node>,
    pub fts_results: Vec<FtsNodeSearchResult>,
    pub direct_nodes: Vec<DirectRecallNode>,
    pub graph_nodes: Vec<GraphRecallNode>,
    pub more_results: bool,
}

/// One bounded, stable slice of a task-recall candidate layer.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRecallLayerPage<T> {
    pub items: Vec<T>,
    pub more_results: bool,
}

impl<T> Default for TaskRecallLayerPage<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            more_results: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoundedLegacyRecall {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub more_results: bool,
    pub content_truncated: bool,
}

/// A stable, keyset-paginated result. `next_after_id` is present only when
/// another page exists, so callers cannot accidentally issue an empty probe.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T, Cursor = i64> {
    pub items: Vec<T>,
    pub next_after_id: Option<Cursor>,
    pub more_results: bool,
}

/// A node page can omit bodies to keep list reads bounded even when individual
/// nodes contain large payloads.
#[derive(Debug, Clone, PartialEq)]
pub struct NodePage {
    pub page: Page<Node>,
    pub body_omitted: bool,
    pub content_truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewLink {
    pub source_node_id: i64,
    pub target_node_id: i64,
    pub link_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alias {
    pub id: i64,
    pub node_id: i64,
    pub alias: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewAlias {
    pub node_id: i64,
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tag {
    pub id: i64,
    pub node_id: i64,
    pub tag: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTag {
    pub node_id: i64,
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Source {
    pub id: i64,
    pub node_id: i64,
    pub source_ref: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceHierarchy {
    pub source_root: String,
    pub source_path: Vec<String>,
    pub source_leaf: String,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeastPrivilegeMetadata {
    pub side_effects: String,
    pub approval_requirement: String,
    pub read_operations: Vec<String>,
    pub write_operations: Vec<String>,
    pub privilege_rank: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewSource {
    pub node_id: i64,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTeachSession {
    pub title: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeachSession {
    pub session_id: i64,
    pub title: String,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeachMaterial {
    pub material_id: i64,
    pub session_id: i64,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeachProposalInput {
    pub items: Vec<TeachProposalItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeachProposal {
    pub proposal_id: i64,
    pub session_id: i64,
    pub items: Vec<TeachProposalItem>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TeachProposalItem {
    CreateNode {
        #[serde(default)]
        node_ref: Option<String>,
        node_type: String,
        status: String,
        title: String,
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        source_ref: Option<String>,
        #[serde(default)]
        confidence: Option<f64>,
        #[serde(default)]
        trust_level: Option<String>,
    },
    AddAlias {
        #[serde(default)]
        node_id: Option<i64>,
        #[serde(default)]
        node_ref: Option<String>,
        alias: String,
    },
    AddTag {
        #[serde(default)]
        node_id: Option<i64>,
        #[serde(default)]
        node_ref: Option<String>,
        tag: String,
    },
    AddSource {
        #[serde(default)]
        node_id: Option<i64>,
        #[serde(default)]
        node_ref: Option<String>,
        source_ref: String,
    },
    AddLink {
        #[serde(default)]
        source_node_id: Option<i64>,
        #[serde(default)]
        source_node_ref: Option<String>,
        #[serde(default)]
        target_node_id: Option<i64>,
        #[serde(default)]
        target_node_ref: Option<String>,
        link_type: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeachApplyReport {
    pub session_id: i64,
    pub proposal_id: i64,
    pub receipt_id: i64,
    pub created_node_ids: Vec<i64>,
    pub created_alias_ids: Vec<i64>,
    pub created_tag_ids: Vec<i64>,
    pub created_source_ids: Vec<i64>,
    pub created_link_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct McpProfile {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub read_operations: String,
    pub write_operations: String,
    pub side_effects: String,
    pub approval_requirement: String,
    pub credentials_source: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewMcpProfile {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub read_operations: String,
    pub write_operations: String,
    pub side_effects: String,
    pub approval_requirement: String,
    pub credentials_source: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LeastPrivilegeNodeBody {
    side_effects: String,
    approval_requirement: String,
    #[serde(default)]
    read_operations: Vec<String>,
    #[serde(default)]
    write_operations: Vec<String>,
}

impl Node {
    #[must_use]
    pub fn source_hierarchy(&self) -> Option<SourceHierarchy> {
        parse_source_hierarchy(self.source_ref.as_deref()?)
    }

    #[must_use]
    pub fn least_privilege_metadata(&self) -> Option<LeastPrivilegeMetadata> {
        parse_node_least_privilege_metadata(self.node_type.as_str(), self.body.as_deref()?)
    }
}

impl Source {
    #[must_use]
    pub fn hierarchy(&self) -> Option<SourceHierarchy> {
        parse_source_hierarchy(&self.source_ref)
    }
}

impl McpProfile {
    #[must_use]
    pub fn least_privilege_metadata(&self) -> LeastPrivilegeMetadata {
        LeastPrivilegeMetadata {
            side_effects: self.side_effects.clone(),
            approval_requirement: self.approval_requirement.clone(),
            read_operations: split_operations(&self.read_operations),
            write_operations: split_operations(&self.write_operations),
            privilege_rank: side_effects_rank(&self.side_effects),
        }
    }
}

impl WorkspacePaths {
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn db(&self) -> &PathBuf {
        &self.db
    }

    pub fn observability(&self) -> &PathBuf {
        &self.observability
    }

    pub fn observability_db(&self) -> &PathBuf {
        &self.observability_db
    }

    pub fn tools(&self) -> &PathBuf {
        &self.tools
    }

    pub fn artifacts(&self) -> &PathBuf {
        &self.artifacts
    }

    pub fn audit_git(&self) -> &PathBuf {
        &self.audit_git
    }

    pub fn runtimes(&self) -> &PathBuf {
        &self.runtimes
    }

    pub fn logs(&self) -> &PathBuf {
        &self.logs
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathResolveError {
    MissingHome,
    MissingUserProfile,
}

impl fmt::Display for PathResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHome => write!(formatter, "HOME is required when AOPMEM_HOME is not set"),
            Self::MissingUserProfile => write!(
                formatter,
                "USERPROFILE is required on Windows when AOPMEM_HOME is not set"
            ),
        }
    }
}

impl std::error::Error for PathResolveError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceKeyError {
    MissingRepoFolderName,
    RelativeRepoRoot,
}

impl fmt::Display for WorkspaceKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRepoFolderName => write!(formatter, "repo root folder name is required"),
            Self::RelativeRepoRoot => write!(formatter, "absolute repo root path is required"),
        }
    }
}

impl std::error::Error for WorkspaceKeyError {}

#[derive(Debug)]
pub enum WorkspaceResolveError {
    WorkspaceKey(WorkspaceKeyError),
    Inspect {
        path: PathBuf,
        source: io::Error,
    },
    Ambiguous {
        current_key: String,
        current_root: PathBuf,
        legacy_key: String,
        legacy_root: PathBuf,
    },
}

impl fmt::Display for WorkspaceResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceKey(error) => write!(formatter, "{error}"),
            Self::Inspect { path, source } => write!(
                formatter,
                "failed to inspect workspace root {}: {source}",
                path.display()
            ),
            Self::Ambiguous {
                current_key,
                current_root,
                legacy_key,
                legacy_root,
            } => write!(
                formatter,
                "both current and legacy workspace roots contain persistent data: \
                 current={current_key} ({}) legacy={legacy_key} ({})",
                current_root.display(),
                legacy_root.display()
            ),
        }
    }
}

impl std::error::Error for WorkspaceResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WorkspaceKey(error) => Some(error),
            Self::Inspect { source, .. } => Some(source),
            Self::Ambiguous { .. } => None,
        }
    }
}

impl From<WorkspaceKeyError> for WorkspaceResolveError {
    fn from(error: WorkspaceKeyError) -> Self {
        Self::WorkspaceKey(error)
    }
}

#[derive(Debug)]
pub enum OpenWorkspaceReadOnlyError {
    Missing(PathBuf),
    UnsafePath(io::Error),
    Db(rusqlite::Error),
}

impl fmt::Display for OpenWorkspaceReadOnlyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => write!(
                formatter,
                "workspace database not found: {}",
                path.display()
            ),
            Self::UnsafePath(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for OpenWorkspaceReadOnlyError {}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeValidationError {
    InvalidType(String),
    InvalidStatus(String),
    MissingTitle,
    FieldTooLong {
        field: &'static str,
        max_bytes: usize,
    },
    MissingActiveSourceRef,
    MissingActiveConfidence,
    MissingActiveTrustLevel,
}

impl fmt::Display for NodeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidType(node_type) => write!(formatter, "invalid node type: {node_type}"),
            Self::InvalidStatus(status) => write!(formatter, "invalid node status: {status}"),
            Self::MissingTitle => write!(formatter, "missing required field: title"),
            Self::FieldTooLong { field, max_bytes } => {
                write!(formatter, "field {field} exceeds {max_bytes} bytes")
            }
            Self::MissingActiveSourceRef => {
                write!(formatter, "active nodes require source_ref")
            }
            Self::MissingActiveConfidence => {
                write!(formatter, "active nodes require confidence")
            }
            Self::MissingActiveTrustLevel => {
                write!(formatter, "active nodes require trust_level")
            }
        }
    }
}

impl std::error::Error for NodeValidationError {}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkValidationError {
    MissingType,
    TypeTooLong { max_bytes: usize },
    SourceNodeNotFound(i64),
    TargetNodeNotFound(i64),
}

impl fmt::Display for LinkValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingType => write!(formatter, "missing required field: link_type"),
            Self::TypeTooLong { max_bytes } => {
                write!(formatter, "field link_type exceeds {max_bytes} bytes")
            }
            Self::SourceNodeNotFound(id) => write!(formatter, "source node not found: {id}"),
            Self::TargetNodeNotFound(id) => write!(formatter, "target node not found: {id}"),
        }
    }
}

impl std::error::Error for LinkValidationError {}

#[derive(Debug, Clone, PartialEq)]
pub enum MetadataValidationError {
    NodeNotFound(i64),
    MissingAlias,
    MissingTag,
    MissingSourceRef,
    ValueTooLong {
        kind: &'static str,
        max_bytes: usize,
    },
}

impl fmt::Display for MetadataValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(formatter, "node not found: {id}"),
            Self::MissingAlias => write!(formatter, "missing required field: alias"),
            Self::MissingTag => write!(formatter, "missing required field: tag"),
            Self::MissingSourceRef => write!(formatter, "missing required field: source_ref"),
            Self::ValueTooLong { kind, max_bytes } => {
                write!(formatter, "field {kind} exceeds {max_bytes} bytes")
            }
        }
    }
}

impl std::error::Error for MetadataValidationError {}

#[derive(Debug, Clone, PartialEq)]
pub enum McpProfileValidationError {
    MissingId,
    MissingName,
    MissingKind,
    MissingStatus,
    MissingReadOperations,
    MissingWriteOperations,
    MissingSideEffects,
    MissingApprovalRequirement,
    FieldTooLong {
        field: &'static str,
        max_bytes: usize,
    },
}

impl fmt::Display for McpProfileValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingId => write!(formatter, "missing required field: id"),
            Self::MissingName => write!(formatter, "missing required field: name"),
            Self::MissingKind => write!(formatter, "missing required field: kind"),
            Self::MissingStatus => write!(formatter, "missing required field: status"),
            Self::MissingReadOperations => {
                write!(formatter, "missing required field: read_operations")
            }
            Self::MissingWriteOperations => {
                write!(formatter, "missing required field: write_operations")
            }
            Self::MissingSideEffects => write!(formatter, "missing required field: side_effects"),
            Self::MissingApprovalRequirement => {
                write!(formatter, "missing required field: approval_requirement")
            }
            Self::FieldTooLong { field, max_bytes } => {
                write!(formatter, "field {field} exceeds {max_bytes} bytes")
            }
        }
    }
}

impl std::error::Error for McpProfileValidationError {}

#[derive(Debug)]
pub enum NodeStorageError {
    Validation(NodeValidationError),
    Db(rusqlite::Error),
}

impl fmt::Display for NodeStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for NodeStorageError {}

impl From<NodeValidationError> for NodeStorageError {
    fn from(error: NodeValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<rusqlite::Error> for NodeStorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

#[derive(Debug)]
pub enum LinkStorageError {
    Validation(LinkValidationError),
    Db(rusqlite::Error),
}

impl fmt::Display for LinkStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for LinkStorageError {}

impl From<LinkValidationError> for LinkStorageError {
    fn from(error: LinkValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<rusqlite::Error> for LinkStorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

#[derive(Debug)]
pub enum MetadataStorageError {
    Validation(MetadataValidationError),
    Db(rusqlite::Error),
}

impl fmt::Display for MetadataStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for MetadataStorageError {}

impl From<MetadataValidationError> for MetadataStorageError {
    fn from(error: MetadataValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<rusqlite::Error> for MetadataStorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeachValidationError {
    MissingSessionTitle,
    SessionNotFound(i64),
    InvalidSessionRecord(i64),
    ProposalNotFound(i64),
    InvalidProposalRecord(i64),
    ProposalSessionMismatch { session_id: i64, proposal_id: i64 },
    EmptyProposalItems,
    TooManyProposalItems { max_items: usize, actual: usize },
    NodeRefTooLong { max_bytes: usize },
    DuplicateNodeRef(String),
    MissingNodeTarget,
    AmbiguousNodeTarget,
    UnknownNodeRef(String),
}

impl fmt::Display for TeachValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSessionTitle => write!(formatter, "missing required field: title"),
            Self::SessionNotFound(id) => write!(formatter, "teach session not found: {id}"),
            Self::InvalidSessionRecord(id) => {
                write!(formatter, "invalid teach session record: {id}")
            }
            Self::ProposalNotFound(id) => write!(formatter, "teach proposal not found: {id}"),
            Self::InvalidProposalRecord(id) => {
                write!(formatter, "invalid teach proposal record: {id}")
            }
            Self::ProposalSessionMismatch {
                session_id,
                proposal_id,
            } => write!(
                formatter,
                "teach proposal {proposal_id} does not belong to session {session_id}"
            ),
            Self::EmptyProposalItems => {
                write!(formatter, "teach proposal must contain at least one item")
            }
            Self::TooManyProposalItems { max_items, actual } => {
                write!(
                    formatter,
                    "teach proposal has {actual} items; maximum is {max_items}"
                )
            }
            Self::NodeRefTooLong { max_bytes } => {
                write!(formatter, "teach node_ref exceeds {max_bytes} bytes")
            }
            Self::DuplicateNodeRef(node_ref) => {
                write!(formatter, "duplicate teach node_ref: {node_ref}")
            }
            Self::MissingNodeTarget => {
                write!(
                    formatter,
                    "teach proposal item requires exactly one node target"
                )
            }
            Self::AmbiguousNodeTarget => {
                write!(
                    formatter,
                    "teach proposal item cannot use node id and node_ref together"
                )
            }
            Self::UnknownNodeRef(node_ref) => {
                write!(formatter, "unknown teach node_ref: {node_ref}")
            }
        }
    }
}

impl std::error::Error for TeachValidationError {}

#[derive(Debug)]
pub enum TeachStorageError {
    Validation(TeachValidationError),
    Node(NodeStorageError),
    Link(LinkStorageError),
    Metadata(MetadataStorageError),
    Json(serde_json::Error),
    Db(rusqlite::Error),
}

impl fmt::Display for TeachStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Node(error) => write!(formatter, "{error}"),
            Self::Link(error) => write!(formatter, "{error}"),
            Self::Metadata(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for TeachStorageError {}

impl From<TeachValidationError> for TeachStorageError {
    fn from(error: TeachValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<NodeStorageError> for TeachStorageError {
    fn from(error: NodeStorageError) -> Self {
        Self::Node(error)
    }
}

impl From<LinkStorageError> for TeachStorageError {
    fn from(error: LinkStorageError) -> Self {
        Self::Link(error)
    }
}

impl From<MetadataStorageError> for TeachStorageError {
    fn from(error: MetadataStorageError) -> Self {
        Self::Metadata(error)
    }
}

impl From<serde_json::Error> for TeachStorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<rusqlite::Error> for TeachStorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

#[derive(Debug)]
pub enum McpProfileStorageError {
    Validation(McpProfileValidationError),
    Db(rusqlite::Error),
}

impl fmt::Display for McpProfileStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for McpProfileStorageError {}

impl From<McpProfileValidationError> for McpProfileStorageError {
    fn from(error: McpProfileValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<rusqlite::Error> for McpProfileStorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

pub fn resolve_paths() -> Result<AopmemPaths, PathResolveError> {
    let aopmem_home = env::var_os(AOPMEM_HOME_ENV);
    let home = env::var_os(HOME_ENV);
    let userprofile = env::var_os(USERPROFILE_ENV);

    resolve_paths_from_env_for_platform(
        aopmem_home.as_deref(),
        home.as_deref(),
        userprofile.as_deref(),
        cfg!(windows),
    )
}

pub fn resolve_current_workspace_root() -> io::Result<PathBuf> {
    resolve_workspace_root_from(env::current_dir()?)
}

pub fn resolve_workspace_root_from(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let canonical = path.as_ref().canonicalize()?;
    Ok(find_git_root(&canonical).unwrap_or(canonical))
}

pub fn workspace_key(repo_root: impl AsRef<Path>) -> Result<String, WorkspaceKeyError> {
    let repo_root = canonicalize_existing_path(repo_root.as_ref());
    let normalized_path = normalize_workspace_path_for_key(&repo_root);
    if !is_absolute_workspace_path(&normalized_path) {
        return Err(WorkspaceKeyError::RelativeRepoRoot);
    }

    let folder_name =
        workspace_folder_name(&normalized_path).ok_or(WorkspaceKeyError::MissingRepoFolderName)?;
    let sanitized = sanitize_repo_folder_name(folder_name);
    let path_hash = hash_normalized_path(&normalized_path);

    Ok(format!("{sanitized}-{path_hash:08x}"))
}

/// Reproduces the v0.1 workspace key from the path text supplied by that
/// binary. Unlike [`workspace_key`], this deliberately does not canonicalize
/// or normalize separators, drive-letter case, or the hash input.
pub fn legacy_workspace_key(repo_root: impl AsRef<Path>) -> Result<String, WorkspaceKeyError> {
    legacy_workspace_key_from_text(&repo_root.as_ref().as_os_str().to_string_lossy())
}

/// Resolves the current repository to an existing v0.1 or v0.2 workspace
/// without creating directories or opening a database.
pub fn resolve_workspace_key(
    paths: &AopmemPaths,
    repo_root: impl AsRef<Path>,
) -> Result<String, WorkspaceResolveError> {
    let repo_root = repo_root.as_ref();
    let current_key = workspace_key(repo_root)?;
    let legacy_text = legacy_workspace_path_text(repo_root);
    let legacy_key = legacy_workspace_key_from_text(&legacy_text)?;

    if current_key == legacy_key {
        return Ok(current_key);
    }

    let current_root = paths.workspaces().join(&current_key);
    let legacy_root = paths.workspaces().join(&legacy_key);
    let current_has_data = workspace_root_has_persistent_data(&current_root).map_err(|source| {
        WorkspaceResolveError::Inspect {
            path: current_root.clone(),
            source,
        }
    })?;
    let legacy_has_data = workspace_root_has_persistent_data(&legacy_root).map_err(|source| {
        WorkspaceResolveError::Inspect {
            path: legacy_root.clone(),
            source,
        }
    })?;

    match (current_has_data, legacy_has_data) {
        (true, true) => Err(WorkspaceResolveError::Ambiguous {
            current_key,
            current_root,
            legacy_key,
            legacy_root,
        }),
        (false, true) => Ok(legacy_key),
        (true, false) | (false, false) => Ok(current_key),
    }
}

pub fn ensure_global_dirs(paths: &AopmemPaths) -> io::Result<()> {
    fs::create_dir_all(paths.home())?;
    fs::create_dir_all(paths.bin())?;
    fs::create_dir_all(paths.skills())?;
    fs::create_dir_all(paths.templates())?;
    fs::create_dir_all(paths.workspaces())?;

    Ok(())
}

pub fn ensure_workspace_dirs(
    paths: &AopmemPaths,
    workspace_key: impl AsRef<str>,
) -> io::Result<WorkspacePaths> {
    let workspace_paths = workspace_paths(paths, workspace_key.as_ref());

    fs::create_dir_all(paths.home())?;
    validate_real_directory(paths.home())?;
    ensure_owned_direct_directory(paths.home(), paths.workspaces())?;
    ensure_owned_direct_directory(paths.workspaces(), workspace_paths.root())?;
    for directory in [
        workspace_paths.tools(),
        workspace_paths.artifacts(),
        workspace_paths.audit_git(),
        workspace_paths.runtimes(),
        workspace_paths.logs(),
    ] {
        ensure_owned_direct_directory(workspace_paths.root(), directory)?;
    }

    Ok(workspace_paths)
}

/// Rejects persistent mutation paths that can redirect writes outside the
/// selected workspace. Missing database and lock files are valid for a fresh
/// workspace; every existing managed entry must be a real local entry.
pub(crate) fn validate_workspace_mutation_paths(
    workspace_paths: &WorkspacePaths,
) -> io::Result<audit::WorkspaceIdentity> {
    validate_workspace_read_paths(workspace_paths)?;
    validate_real_directory(workspace_paths.audit_git())?;
    validate_canonical_direct_child(workspace_paths.root(), workspace_paths.audit_git())?;
    audit::WorkspaceIdentity::capture(workspace_paths.root())
}

pub(crate) fn validate_workspace_read_paths(workspace_paths: &WorkspacePaths) -> io::Result<()> {
    let workspaces_root = workspace_paths.root().parent().ok_or_else(|| {
        persistent_path_error(workspace_paths.root(), "workspace root has no parent")
    })?;
    validate_real_directory(workspaces_root)?;
    validate_real_directory(workspace_paths.root())?;
    validate_canonical_direct_child(workspaces_root, workspace_paths.root())?;
    validate_optional_regular_file(workspace_paths.db())?;
    validate_lexical_and_canonical_parent(workspace_paths.root(), workspace_paths.db(), true)?;
    for sidecar in workspace_db_sidecar_paths(workspace_paths.db()) {
        validate_optional_regular_file(&sidecar)?;
        validate_lexical_and_canonical_parent(workspace_paths.root(), &sidecar, true)?;
    }
    Ok(())
}

fn workspace_db_sidecar_paths(db_path: &Path) -> [PathBuf; 3] {
    fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
        let mut value = path.as_os_str().to_os_string();
        value.push(suffix);
        value.into()
    }

    [
        with_suffix(db_path, "-wal"),
        with_suffix(db_path, "-shm"),
        with_suffix(db_path, "-journal"),
    ]
}

pub(crate) fn validate_optional_regular_file(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_file() && !persistent_path_is_link_or_reparse_point(&metadata) =>
        {
            Ok(())
        }
        Ok(_) => Err(persistent_path_error(
            path,
            "managed file is not a real regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn ensure_owned_direct_directory(parent: &Path, directory: &Path) -> io::Result<()> {
    validate_real_directory(parent)?;
    if directory.parent() != Some(parent) {
        return Err(persistent_path_error(
            directory,
            "managed directory is not a direct child",
        ));
    }
    match fs::create_dir(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    validate_real_directory(directory)?;
    validate_canonical_direct_child(parent, directory)
}

pub(crate) fn validate_real_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() && !persistent_path_is_link_or_reparse_point(&metadata) {
        Ok(())
    } else {
        Err(persistent_path_error(
            path,
            "managed directory is not a real directory",
        ))
    }
}

pub(crate) fn validate_canonical_direct_child(parent: &Path, child: &Path) -> io::Result<()> {
    validate_lexical_and_canonical_parent(parent, child, false)
}

fn validate_lexical_and_canonical_parent(
    parent: &Path,
    child: &Path,
    allow_missing: bool,
) -> io::Result<()> {
    if child.parent() != Some(parent) {
        return Err(persistent_path_error(
            child,
            "managed path is not a direct child",
        ));
    }
    if allow_missing && !child.exists() {
        return Ok(());
    }
    let canonical_parent = parent.canonicalize()?;
    let canonical_child = child.canonicalize()?;
    if canonical_child.parent() == Some(canonical_parent.as_path()) {
        Ok(())
    } else {
        Err(persistent_path_error(
            child,
            "managed path resolves outside its parent",
        ))
    }
}

fn persistent_path_is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    #[cfg(not(windows))]
    {
        false
    }
}

fn persistent_path_error(path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "unsafe persistent workspace path ({}): {reason}",
            path.display()
        ),
    )
}

pub fn workspace_paths_for_key(
    paths: &AopmemPaths,
    workspace_key: impl AsRef<str>,
) -> WorkspacePaths {
    workspace_paths(paths, workspace_key.as_ref())
}

pub fn open_workspace_db(workspace_paths: &WorkspacePaths) -> rusqlite::Result<Connection> {
    let mut connection = open_workspace_db_without_migrations(workspace_paths)?;
    schema::apply_migrations(&mut connection)?;

    Ok(connection)
}

/// Opens a writable workspace database without applying migrations.
///
/// Mutating production paths must call this only after the pending snapshot
/// marker and process lock are in place, then migrate inside their transaction.
pub fn open_workspace_db_without_migrations(
    workspace_paths: &WorkspacePaths,
) -> rusqlite::Result<Connection> {
    let db_path = canonical_db_open_path(workspace_paths)
        .map_err(|_| rusqlite::Error::InvalidPath(workspace_paths.db().clone()))?;
    let connection = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    apply_connection_pragmas(&connection)?;
    prepare_task_recall_connection(&connection)?;
    Ok(connection)
}

pub fn open_workspace_db_read_only(
    workspace_paths: &WorkspacePaths,
) -> Result<Connection, OpenWorkspaceReadOnlyError> {
    match fs::symlink_metadata(workspace_paths.db()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpenWorkspaceReadOnlyError::Missing(
                workspace_paths.db().clone(),
            ));
        }
        Err(error) => return Err(OpenWorkspaceReadOnlyError::UnsafePath(error)),
        Ok(_) => {}
    }
    validate_workspace_read_paths(workspace_paths)
        .map_err(OpenWorkspaceReadOnlyError::UnsafePath)?;
    let db_path =
        canonical_db_open_path(workspace_paths).map_err(OpenWorkspaceReadOnlyError::UnsafePath)?;

    let connection = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(OpenWorkspaceReadOnlyError::Db)?;
    prepare_task_recall_connection(&connection).map_err(OpenWorkspaceReadOnlyError::Db)?;
    Ok(connection)
}

fn canonical_db_open_path(workspace_paths: &WorkspacePaths) -> io::Result<PathBuf> {
    let file_name = workspace_paths.db().file_name().ok_or_else(|| {
        persistent_path_error(workspace_paths.db(), "database path has no file name")
    })?;
    let parent = workspace_paths.db().parent().ok_or_else(|| {
        persistent_path_error(workspace_paths.db(), "database path has no parent")
    })?;
    Ok(parent.canonicalize()?.join(file_name))
}

pub fn create_node(connection: &Connection, node: &NewNode) -> Result<Node, NodeStorageError> {
    create_node_borrowed(connection, node.into())
}

pub(crate) fn create_node_borrowed(
    connection: &Connection,
    node: BorrowedNodeInput<'_>,
) -> Result<Node, NodeStorageError> {
    validate_node_input(node)?;

    connection
        .prepare_cached(INSERT_NODE_SQL)?
        .execute(params![
            node.node_type,
            node.status,
            node.title,
            node.summary,
            node.body,
            node.source_ref,
            node.confidence,
            node.trust_level
        ])?;

    let id = connection.last_insert_rowid();
    let created = get_node(connection, id)?
        .ok_or(NodeStorageError::Db(rusqlite::Error::QueryReturnedNoRows))?;
    refresh_fts_node(connection, created.id)?;
    audit::record_node_created(connection, created.id, STORAGE_AUDIT_SOURCE)
        .map_err(audit_error_to_db)?;

    Ok(created)
}

pub fn validate_new_node_input(node: &NewNode) -> Result<(), NodeValidationError> {
    validate_node_input(node.into())
}

pub fn validate_node_update_input(update: &NodeUpdate) -> Result<(), NodeValidationError> {
    validate_node_input(BorrowedNodeInput {
        node_type: "raw_note",
        status: &update.status,
        title: &update.title,
        summary: update.summary.as_deref(),
        body: update.body.as_deref(),
        source_ref: update.source_ref.as_deref(),
        confidence: update.confidence,
        trust_level: update.trust_level.as_deref(),
    })
}

pub fn update_node(
    connection: &Connection,
    update: &NodeUpdate,
) -> Result<Option<Node>, NodeStorageError> {
    let existing = match get_node(connection, update.id)? {
        Some(node) => node,
        None => return Ok(None),
    };
    validate_node_input(BorrowedNodeInput {
        node_type: &existing.node_type,
        status: &update.status,
        title: &update.title,
        summary: update.summary.as_deref(),
        body: update.body.as_deref(),
        source_ref: update.source_ref.as_deref(),
        confidence: update.confidence,
        trust_level: update.trust_level.as_deref(),
    })?;

    connection.execute(
        "
            UPDATE nodes
            SET
                status = ?2,
                title = ?3,
                summary = ?4,
                body = ?5,
                source_ref = ?6,
                confidence = ?7,
                trust_level = ?8,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1;
            ",
        params![
            update.id,
            &update.status,
            &update.title,
            &update.summary,
            &update.body,
            &update.source_ref,
            update.confidence,
            &update.trust_level
        ],
    )?;

    let updated = get_node(connection, update.id)?
        .ok_or(NodeStorageError::Db(rusqlite::Error::QueryReturnedNoRows))?;
    refresh_fts_node(connection, updated.id)?;
    audit::record_node_updated(connection, updated.id, STORAGE_AUDIT_SOURCE)
        .map_err(audit_error_to_db)?;

    Ok(Some(updated))
}

pub fn get_node(connection: &Connection, id: i64) -> rusqlite::Result<Option<Node>> {
    let mut statement = connection.prepare_cached(GET_NODE_SQL)?;
    statement.query_row([id], row_to_node).optional()
}

pub fn list_nodes(connection: &Connection) -> rusqlite::Result<Vec<Node>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            node_type,
            status,
            title,
            summary,
            body,
            source_ref,
            confidence,
            trust_level,
            created_at,
            updated_at
        FROM nodes
        ORDER BY id ASC;
        ",
    )?;

    let nodes = statement.query_map([], row_to_node)?.collect();
    nodes
}

/// Loads every active node that belongs to the v0.2 mandatory recall context.
///
/// This query is deliberately independent from normal list and recall limits:
/// mandatory context must either be returned in full or rejected by the recall
/// budget validator. Type rank followed by the immutable node id defines the
/// stable canonical order used for budget accounting and overflow reporting.
pub fn load_active_mandatory_recall_nodes(connection: &Connection) -> rusqlite::Result<Vec<Node>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            node_type,
            status,
            title,
            summary,
            body,
            source_ref,
            confidence,
            trust_level,
            created_at,
            updated_at
        FROM nodes
        WHERE status = 'active'
          AND node_type IN (
              'kernel_contract',
              'gate',
              'project_profile',
              'source',
              'rule'
          )
        ORDER BY
            CASE node_type
                WHEN 'kernel_contract' THEN 0
                WHEN 'gate' THEN 1
                WHEN 'project_profile' THEN 2
                WHEN 'source' THEN 3
                WHEN 'rule' THEN 4
            END ASC,
            id ASC;
        ",
    )?;

    let nodes = statement.query_map([], row_to_node)?.collect();
    nodes
}

/// Streams every canonical operational table that can affect recall and
/// returns a deterministic revision fingerprint without retaining row data.
/// A continuation cursor must match this value before it can read a next page.
pub fn operational_recall_revision(connection: &Connection) -> rusqlite::Result<String> {
    let mut hasher = OperationalRevisionHasher::new();
    for (table, query) in [
        (
            "nodes",
            "SELECT id, node_type, status, title, summary, body, source_ref, confidence, trust_level, created_at, updated_at FROM nodes ORDER BY id ASC",
        ),
        (
            "links",
            "SELECT id, source_node_id, target_node_id, link_type, created_at FROM links ORDER BY id ASC",
        ),
        (
            "aliases",
            "SELECT id, node_id, alias, created_at FROM aliases ORDER BY id ASC",
        ),
        (
            "tags",
            "SELECT id, node_id, tag, created_at FROM tags ORDER BY id ASC",
        ),
        (
            "sources",
            "SELECT id, node_id, source_ref, created_at FROM sources ORDER BY id ASC",
        ),
        (
            "tool_contracts",
            "SELECT tool_id, name, status, owner_workflow, side_effects, approval_requirement, contract_json, created_at, updated_at FROM tool_contracts ORDER BY tool_id ASC",
        ),
        (
            "mcp_profiles",
            "SELECT id, name, kind, status, read_operations, write_operations, side_effects, approval_requirement, credentials_source, notes, created_at, updated_at FROM mcp_profiles ORDER BY id ASC",
        ),
        (
            "schema_migrations",
            "SELECT version, name, applied_at FROM schema_migrations ORDER BY version ASC",
        ),
    ] {
        hasher.update(table.as_bytes());
        let mut statement = connection.prepare(query)?;
        let column_count = statement.column_count();
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            hasher.update(&[0xff]);
            for index in 0..column_count {
                match row.get_ref(index)? {
                    rusqlite::types::ValueRef::Null => hasher.update(&[0]),
                    rusqlite::types::ValueRef::Integer(value) => {
                        hasher.update(&[1]);
                        hasher.update(&value.to_le_bytes());
                    }
                    rusqlite::types::ValueRef::Real(value) => {
                        hasher.update(&[2]);
                        hasher.update(&value.to_bits().to_le_bytes());
                    }
                    rusqlite::types::ValueRef::Text(value) => {
                        hasher.update(&[3]);
                        hasher.update(&(value.len() as u64).to_le_bytes());
                        hasher.update(value);
                    }
                    rusqlite::types::ValueRef::Blob(value) => {
                        hasher.update(&[4]);
                        hasher.update(&(value.len() as u64).to_le_bytes());
                        hasher.update(value);
                    }
                }
            }
        }
    }
    Ok(hasher.finish())
}

struct OperationalRevisionHasher {
    first: u64,
    second: u64,
}

impl OperationalRevisionHasher {
    fn new() -> Self {
        Self {
            first: 0xcbf2_9ce4_8422_2325,
            second: 0x8422_2325_cbf2_9ce4,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.first ^= u64::from(*byte);
            self.first = self.first.wrapping_mul(0x0000_0100_0000_01b3);
            self.second ^= u64::from(*byte).rotate_left(1);
            self.second = self.second.wrapping_mul(0x9e37_79b1_85eb_ca87);
        }
    }

    fn finish(self) -> String {
        format!("{:016x}{:016x}", self.first, self.second)
    }
}

/// Loads bounded task-recall candidates in pipeline order: typed exact roots,
/// FTS5/BM25 candidates, one-hop outgoing links, and a depth-two graph walk.
pub fn load_task_recall_candidates(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> rusqlite::Result<TaskRecallCandidates> {
    if limit == 0 {
        return Ok(TaskRecallCandidates::default());
    }

    let (typed_roots, typed_more_results) = list_task_typed_roots(connection, query, limit)?;
    let (fts_results, fts_more_results) = search_task_recall_fts(connection, query, limit)?;

    let mut seen_root_ids = BTreeSet::new();
    let mut ordered_roots = Vec::with_capacity(typed_roots.len() + fts_results.len());
    for root in typed_roots
        .iter()
        .chain(fts_results.iter().map(|result| &result.node))
    {
        if seen_root_ids.insert(root.id) {
            ordered_roots.push((root.id, root.node_type.clone()));
        }
    }
    let ordered_root_ids = ordered_roots
        .iter()
        .map(|(root_id, _)| *root_id)
        .collect::<Vec<_>>();
    let (direct_nodes, direct_more_results) =
        list_task_direct_nodes(connection, &ordered_root_ids, limit)?;
    let (graph_nodes, graph_more_results) =
        list_task_graph_nodes(connection, &ordered_roots, limit)?;

    Ok(TaskRecallCandidates {
        typed_roots,
        fts_results,
        direct_nodes,
        graph_nodes,
        more_results: typed_more_results
            || fts_more_results
            || direct_more_results
            || graph_more_results,
    })
}

/// Reads one globally ordered typed-root page for continuation recall.
pub fn load_task_typed_roots_page(
    connection: &Connection,
    query: &str,
    offset: u64,
    limit: usize,
) -> rusqlite::Result<TaskRecallLayerPage<Node>> {
    let fetch_limit = page_fetch_limit(limit)?;
    let offset = i64::try_from(offset).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let mut statement = connection.prepare_cached(
        "
        WITH exact_ids(id) AS (
            SELECT id FROM nodes WHERE title = ?1 COLLATE NOCASE
            UNION
            SELECT node_id FROM aliases WHERE alias = ?1 COLLATE NOCASE
            UNION
            SELECT node_id FROM tags WHERE tag = ?1 COLLATE NOCASE
        )
        SELECT
            nodes.id, nodes.node_type, nodes.status, nodes.title, nodes.summary,
            nodes.body, nodes.source_ref, nodes.confidence, nodes.trust_level,
            nodes.created_at, nodes.updated_at
        FROM exact_ids
        JOIN nodes ON nodes.id = exact_ids.id
        WHERE nodes.node_type IN (
            'workflow', 'tool_contract', 'failure_mode', 'correction', 'rule',
            'lesson', 'skill', 'incident_scar', 'decision', 'project_fact',
            'preference'
        )
          AND nodes.status NOT IN ('deprecated', 'superseded', 'broken')
          AND NOT (
              nodes.status = 'active'
              AND nodes.node_type IN (
                  'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
              )
          )
        ORDER BY
            aopmem_source_priority(nodes.source_ref) ASC,
            CASE
                WHEN nodes.trust_level = 'high' THEN 0
                WHEN nodes.trust_level = 'medium' THEN 1
                WHEN nodes.trust_level = 'low' THEN 2
                WHEN nodes.trust_level IS NULL THEN 4 ELSE 3
            END ASC,
            nodes.confidence DESC NULLS LAST,
            nodes.id ASC
        LIMIT ?2 OFFSET ?3;
        ",
    )?;
    let mut items = statement
        .query_map(params![query, fetch_limit, offset], row_to_node)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = items.len() > limit;
    items.truncate(limit);
    Ok(TaskRecallLayerPage {
        items,
        more_results,
    })
}

/// Reads one globally ordered FTS/BM25 page for continuation recall.
pub fn load_task_fts_page(
    connection: &Connection,
    query: &str,
    offset: u64,
    limit: usize,
) -> rusqlite::Result<TaskRecallLayerPage<FtsNodeSearchResult>> {
    let Some(match_query) = fts_match_query(query) else {
        return Ok(TaskRecallLayerPage::default());
    };
    let fetch_limit = page_fetch_limit(limit)?;
    let offset = i64::try_from(offset).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let mut statement = connection.prepare_cached(
        "
        SELECT
            nodes.id, nodes.node_type, nodes.status, nodes.title, nodes.summary,
            nodes.body, nodes.source_ref, nodes.confidence, nodes.trust_level,
            nodes.created_at, nodes.updated_at,
            bm25(fts_nodes) AS rank
        FROM fts_nodes
        JOIN nodes ON nodes.id = fts_nodes.rowid
        WHERE fts_nodes MATCH ?1
          AND nodes.status NOT IN ('deprecated', 'superseded', 'broken')
          AND NOT (
              nodes.status = 'active'
              AND nodes.node_type IN (
                  'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
              )
          )
        ORDER BY
            aopmem_source_priority(nodes.source_ref) ASC,
            CASE
                WHEN nodes.trust_level = 'high' THEN 0
                WHEN nodes.trust_level = 'medium' THEN 1
                WHEN nodes.trust_level = 'low' THEN 2
                WHEN nodes.trust_level IS NULL THEN 4 ELSE 3
            END ASC,
            nodes.confidence DESC NULLS LAST,
            rank ASC,
            nodes.id ASC
        LIMIT ?2 OFFSET ?3;
        ",
    )?;
    let mut items = statement
        .query_map(params![match_query, fetch_limit, offset], |row| {
            Ok(FtsNodeSearchResult {
                rank: row.get(11)?,
                node: row_to_node(row)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = items.len() > limit;
    items.truncate(limit);
    Ok(TaskRecallLayerPage {
        items,
        more_results,
    })
}

/// Reads one target-priority ordered direct-link page from all discovered
/// typed/FTS roots. Duplicate targets remain adjacent and are removed by the
/// cursor's exact seen-id set.
pub fn load_task_direct_page(
    connection: &Connection,
    ordered_roots: &[(i64, String)],
    offset: u64,
    limit: usize,
) -> rusqlite::Result<TaskRecallLayerPage<DirectRecallNode>> {
    if ordered_roots.is_empty() || limit == 0 {
        return Ok(TaskRecallLayerPage::default());
    }
    let fetch_limit = page_fetch_limit(limit)?;
    let offset = i64::try_from(offset).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let root_rows = ordered_roots
        .iter()
        .enumerate()
        .map(|(index, _)| format!("(?{}, {index})", index + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let limit_parameter = ordered_roots.len() + 1;
    let offset_parameter = limit_parameter + 1;
    let sql = format!(
        "
        WITH root_order(root_id, ordinal) AS (VALUES {root_rows})
        SELECT
            root_order.root_id,
            links.id, links.source_node_id, links.target_node_id,
            links.link_type, links.created_at,
            nodes.id, nodes.node_type, nodes.status, nodes.title, nodes.summary,
            nodes.body, nodes.source_ref, nodes.confidence, nodes.trust_level,
            nodes.created_at, nodes.updated_at
        FROM root_order
        JOIN links ON links.source_node_id = root_order.root_id
        JOIN nodes ON nodes.id = links.target_node_id
        WHERE nodes.status NOT IN ('deprecated', 'superseded', 'broken')
          AND NOT (
              nodes.status = 'active'
              AND nodes.node_type IN (
                  'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
              )
          )
        ORDER BY
            aopmem_source_priority(nodes.source_ref) ASC,
            CASE
                WHEN nodes.trust_level = 'high' THEN 0
                WHEN nodes.trust_level = 'medium' THEN 1
                WHEN nodes.trust_level = 'low' THEN 2
                WHEN nodes.trust_level IS NULL THEN 4 ELSE 3
            END ASC,
            nodes.confidence DESC NULLS LAST,
            nodes.id ASC,
            root_order.ordinal ASC,
            links.id ASC
        LIMIT ?{limit_parameter} OFFSET ?{offset_parameter};
        "
    );
    let parameters = ordered_roots
        .iter()
        .map(|(root_id, _)| *root_id)
        .chain([fetch_limit, offset]);
    let mut statement = connection.prepare_cached(&sql)?;
    let mut items = statement
        .query_map(rusqlite::params_from_iter(parameters), |row| {
            Ok(DirectRecallNode {
                root_node_id: row.get(0)?,
                link: Link {
                    id: row.get(1)?,
                    source_node_id: row.get(2)?,
                    target_node_id: row.get(3)?,
                    link_type: row.get(4)?,
                    created_at: row.get(5)?,
                },
                node: row_to_node_at(row, 6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = items.len() > limit;
    items.truncate(limit);
    Ok(TaskRecallLayerPage {
        items,
        more_results,
    })
}

/// Reads one target-priority ordered page of the depth-two graph expansion.
pub fn load_task_graph_page(
    connection: &Connection,
    ordered_roots: &[(i64, String)],
    offset: u64,
    limit: usize,
) -> rusqlite::Result<TaskRecallLayerPage<GraphRecallNode>> {
    if ordered_roots.is_empty() || limit == 0 {
        return Ok(TaskRecallLayerPage::default());
    }
    let fetch_limit = page_fetch_limit(limit)?;
    let offset = i64::try_from(offset).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let root_rows = ordered_roots
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let id_parameter = index * 2 + 1;
            let type_parameter = id_parameter + 1;
            format!("(?{id_parameter}, ?{type_parameter}, {index})")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let limit_parameter = ordered_roots.len() * 2 + 1;
    let offset_parameter = limit_parameter + 1;
    let sql = format!(
        "
        WITH RECURSIVE
        root_order(root_id, root_type, ordinal) AS (VALUES {root_rows}),
        graph(
            root_id, root_type, root_ordinal, depth, link_id,
            edge_source_node_id, target_node_id, link_type,
            link_created_at, visited_path
        ) AS (
            SELECT
                root_order.root_id, root_order.root_type, root_order.ordinal, 1,
                links.id, links.source_node_id, links.target_node_id,
                links.link_type, links.created_at,
                ',' || root_order.root_id || ',' || links.target_node_id || ','
            FROM root_order
            JOIN links ON links.source_node_id = root_order.root_id
            JOIN nodes ON nodes.id = links.target_node_id
            WHERE nodes.status NOT IN ('deprecated', 'superseded', 'broken')
              AND NOT (
                  nodes.status = 'active'
                  AND nodes.node_type IN (
                      'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
                  )
              )

            UNION ALL

            SELECT
                graph.root_id, graph.root_type, graph.root_ordinal,
                graph.depth + 1, links.id, links.source_node_id,
                links.target_node_id, links.link_type, links.created_at,
                graph.visited_path || links.target_node_id || ','
            FROM graph
            JOIN links ON links.source_node_id = graph.target_node_id
            JOIN nodes ON nodes.id = links.target_node_id
            WHERE graph.depth < 2
              AND instr(graph.visited_path, ',' || links.target_node_id || ',') = 0
              AND nodes.status NOT IN ('deprecated', 'superseded', 'broken')
              AND NOT (
                  nodes.status = 'active'
                  AND nodes.node_type IN (
                      'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
                  )
              )
        )
        SELECT
            graph.root_id, graph.root_type, graph.edge_source_node_id,
            graph.depth, graph.link_id, graph.link_type, graph.link_created_at,
            nodes.id, nodes.node_type, nodes.status, nodes.title, nodes.summary,
            nodes.body, nodes.source_ref, nodes.confidence, nodes.trust_level,
            nodes.created_at, nodes.updated_at
        FROM graph
        JOIN nodes ON nodes.id = graph.target_node_id
        ORDER BY
            aopmem_source_priority(nodes.source_ref) ASC,
            CASE
                WHEN nodes.trust_level = 'high' THEN 0
                WHEN nodes.trust_level = 'medium' THEN 1
                WHEN nodes.trust_level = 'low' THEN 2
                WHEN nodes.trust_level IS NULL THEN 4 ELSE 3
            END ASC,
            nodes.confidence DESC NULLS LAST,
            nodes.id ASC,
            graph.root_ordinal ASC,
            graph.depth ASC,
            graph.link_id ASC,
            graph.edge_source_node_id ASC
        LIMIT ?{limit_parameter} OFFSET ?{offset_parameter};
        "
    );
    let parameters = ordered_roots
        .iter()
        .flat_map(|(root_id, root_type)| {
            [
                rusqlite::types::Value::Integer(*root_id),
                rusqlite::types::Value::Text(root_type.clone()),
            ]
        })
        .chain([
            rusqlite::types::Value::Integer(fetch_limit),
            rusqlite::types::Value::Integer(offset),
        ]);
    let mut statement = connection.prepare_cached(&sql)?;
    let mut items = statement
        .query_map(rusqlite::params_from_iter(parameters), |row| {
            let depth = usize::try_from(row.get::<_, i64>(3)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?;
            let node = row_to_node_at(row, 7)?;
            Ok(GraphRecallNode {
                root_node_id: row.get(0)?,
                root_node_type: row.get(1)?,
                edge_source_node_id: row.get(2)?,
                link: Link {
                    id: row.get(4)?,
                    source_node_id: row.get(2)?,
                    target_node_id: node.id,
                    link_type: row.get(5)?,
                    created_at: row.get(6)?,
                },
                depth,
                node,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = items.len() > limit;
    items.truncate(limit);
    Ok(TaskRecallLayerPage {
        items,
        more_results,
    })
}

fn list_task_typed_roots(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> rusqlite::Result<(Vec<Node>, bool)> {
    let fetch_limit = page_fetch_limit(limit)?;
    let mut statement = connection.prepare(
        "
        WITH exact_ids(id) AS (
            SELECT id
            FROM nodes
            WHERE title = ?1 COLLATE NOCASE
            UNION
            SELECT node_id
            FROM aliases
            WHERE alias = ?1 COLLATE NOCASE
            UNION
            SELECT node_id
            FROM tags
            WHERE tag = ?1 COLLATE NOCASE
        )
        SELECT
            nodes.id,
            nodes.node_type,
            nodes.status,
            nodes.title,
            nodes.summary,
            nodes.body,
            nodes.source_ref,
            nodes.confidence,
            nodes.trust_level,
            nodes.created_at,
            nodes.updated_at
        FROM exact_ids
        JOIN nodes ON nodes.id = exact_ids.id
        WHERE nodes.node_type IN (
            'workflow',
            'tool_contract',
            'failure_mode',
            'correction',
            'rule',
            'lesson',
            'skill',
            'incident_scar',
            'decision',
            'project_fact',
            'preference'
        )
          AND nodes.status NOT IN ('deprecated', 'superseded', 'broken')
          AND NOT (
              nodes.status = 'active'
              AND nodes.node_type IN (
                  'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
              )
          )
        ORDER BY
            CASE nodes.node_type
                WHEN 'workflow' THEN 0
                WHEN 'tool_contract' THEN 1
                WHEN 'failure_mode' THEN 2
                WHEN 'correction' THEN 3
                WHEN 'rule' THEN 4
                WHEN 'lesson' THEN 5
                WHEN 'skill' THEN 6
                WHEN 'incident_scar' THEN 7
                WHEN 'decision' THEN 8
                WHEN 'project_fact' THEN 9
                WHEN 'preference' THEN 10
            END ASC,
            nodes.id ASC
        LIMIT ?2;
        ",
    )?;
    let mut nodes = statement
        .query_map(params![query, fetch_limit], row_to_node)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = nodes.len() > limit;
    nodes.truncate(limit);

    Ok((nodes, more_results))
}

fn search_task_recall_fts(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> rusqlite::Result<(Vec<FtsNodeSearchResult>, bool)> {
    let Some(match_query) = fts_match_query(query) else {
        return Ok((Vec::new(), false));
    };
    let fetch_limit = page_fetch_limit(limit)?;
    let mut statement = connection.prepare(
        "
        SELECT
            nodes.id,
            nodes.node_type,
            nodes.status,
            nodes.title,
            nodes.summary,
            nodes.body,
            nodes.source_ref,
            nodes.confidence,
            nodes.trust_level,
            nodes.created_at,
            nodes.updated_at,
            bm25(fts_nodes) AS rank
        FROM fts_nodes
        JOIN nodes ON nodes.id = fts_nodes.rowid
        WHERE fts_nodes MATCH ?1
          AND nodes.status NOT IN ('deprecated', 'superseded', 'broken')
          AND NOT (
              nodes.status = 'active'
              AND nodes.node_type IN (
                  'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
              )
          )
        ORDER BY rank ASC, nodes.id ASC
        LIMIT ?2;
        ",
    )?;
    let mut results = statement
        .query_map(params![match_query, fetch_limit], |row| {
            Ok(FtsNodeSearchResult {
                rank: row.get(11)?,
                node: row_to_node(row)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = results.len() > limit;
    results.truncate(limit);

    Ok((results, more_results))
}

fn list_task_direct_nodes(
    connection: &Connection,
    ordered_root_ids: &[i64],
    limit: usize,
) -> rusqlite::Result<(Vec<DirectRecallNode>, bool)> {
    if ordered_root_ids.is_empty() {
        return Ok((Vec::new(), false));
    }
    let fetch_limit = page_fetch_limit(limit)?;
    let root_rows = ordered_root_ids
        .iter()
        .enumerate()
        .map(|(index, _)| format!("(?{}, {index})", index + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let limit_parameter = ordered_root_ids.len() + 1;
    let sql = format!(
        "
        WITH root_order(root_id, ordinal) AS (
            VALUES {root_rows}
        )
        SELECT
            root_order.root_id,
            links.id,
            links.source_node_id,
            links.target_node_id,
            links.link_type,
            links.created_at,
            nodes.id,
            nodes.node_type,
            nodes.status,
            nodes.title,
            nodes.summary,
            nodes.body,
            nodes.source_ref,
            nodes.confidence,
            nodes.trust_level,
            nodes.created_at,
            nodes.updated_at
        FROM root_order
        JOIN links ON links.source_node_id = root_order.root_id
        JOIN nodes ON nodes.id = links.target_node_id
        WHERE nodes.status NOT IN ('deprecated', 'superseded', 'broken')
          AND NOT (
              nodes.status = 'active'
              AND nodes.node_type IN (
                  'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
              )
          )
        ORDER BY root_order.ordinal ASC, links.id ASC, nodes.id ASC
        LIMIT ?{limit_parameter};
        "
    );
    let parameters = ordered_root_ids
        .iter()
        .copied()
        .chain(std::iter::once(fetch_limit));
    let mut statement = connection.prepare(&sql)?;
    let mut nodes = statement
        .query_map(rusqlite::params_from_iter(parameters), |row| {
            Ok(DirectRecallNode {
                root_node_id: row.get(0)?,
                link: Link {
                    id: row.get(1)?,
                    source_node_id: row.get(2)?,
                    target_node_id: row.get(3)?,
                    link_type: row.get(4)?,
                    created_at: row.get(5)?,
                },
                node: row_to_node_at(row, 6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = nodes.len() > limit;
    nodes.truncate(limit);

    Ok((nodes, more_results))
}

fn list_task_graph_nodes(
    connection: &Connection,
    ordered_roots: &[(i64, String)],
    limit: usize,
) -> rusqlite::Result<(Vec<GraphRecallNode>, bool)> {
    if ordered_roots.is_empty() || limit == 0 {
        return Ok((Vec::new(), false));
    }

    let fetch_limit = page_fetch_limit(limit)?;
    let root_rows = ordered_roots
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let id_parameter = index * 2 + 1;
            let type_parameter = id_parameter + 1;
            format!("(?{id_parameter}, ?{type_parameter}, {index})")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let limit_parameter = ordered_roots.len() * 2 + 1;
    let sql = format!(
        "
        WITH RECURSIVE
        root_order(root_id, root_type, ordinal) AS (
            VALUES {root_rows}
        ),
        graph(
            root_id,
            root_type,
            root_ordinal,
            depth,
            link_id,
            edge_source_node_id,
            target_node_id,
            link_type,
            link_created_at,
            visited_path
        ) AS (
            SELECT
                root_order.root_id,
                root_order.root_type,
                root_order.ordinal,
                1,
                links.id,
                links.source_node_id,
                links.target_node_id,
                links.link_type,
                links.created_at,
                ',' || root_order.root_id || ',' || links.target_node_id || ','
            FROM root_order
            JOIN links ON links.source_node_id = root_order.root_id
            JOIN nodes ON nodes.id = links.target_node_id
            WHERE nodes.status NOT IN ('deprecated', 'superseded', 'broken')
              AND NOT (
                  nodes.status = 'active'
                  AND nodes.node_type IN (
                      'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
                  )
              )

            UNION ALL

            SELECT
                graph.root_id,
                graph.root_type,
                graph.root_ordinal,
                graph.depth + 1,
                links.id,
                links.source_node_id,
                links.target_node_id,
                links.link_type,
                links.created_at,
                graph.visited_path || links.target_node_id || ','
            FROM graph
            JOIN links ON links.source_node_id = graph.target_node_id
            JOIN nodes ON nodes.id = links.target_node_id
            WHERE graph.depth < 2
              AND instr(
                  graph.visited_path,
                  ',' || links.target_node_id || ','
              ) = 0
              AND nodes.status NOT IN ('deprecated', 'superseded', 'broken')
              AND NOT (
                  nodes.status = 'active'
                  AND nodes.node_type IN (
                      'kernel_contract', 'gate', 'project_profile', 'source', 'rule'
                  )
              )
            ORDER BY 3 ASC, 4 ASC, 5 ASC, 7 ASC
            LIMIT ?{limit_parameter}
        )
        SELECT
            graph.root_id,
            graph.root_type,
            graph.edge_source_node_id,
            graph.depth,
            graph.link_id,
            graph.link_type,
            graph.link_created_at,
            nodes.id,
            nodes.node_type,
            nodes.status,
            nodes.title,
            nodes.summary,
            nodes.body,
            nodes.source_ref,
            nodes.confidence,
            nodes.trust_level,
            nodes.created_at,
            nodes.updated_at
        FROM graph
        JOIN nodes ON nodes.id = graph.target_node_id
        ORDER BY
            graph.root_ordinal ASC,
            graph.depth ASC,
            graph.link_id ASC,
            nodes.id ASC;
        "
    );
    let parameters = ordered_roots
        .iter()
        .flat_map(|(root_id, root_type)| {
            [
                rusqlite::types::Value::Integer(*root_id),
                rusqlite::types::Value::Text(root_type.clone()),
            ]
        })
        .chain(std::iter::once(rusqlite::types::Value::Integer(
            fetch_limit,
        )));
    let mut statement = connection.prepare(&sql)?;
    let mut nodes = statement
        .query_map(rusqlite::params_from_iter(parameters), |row| {
            let depth = row.get::<_, i64>(3)?;
            let depth = usize::try_from(depth).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?;
            let node = row_to_node_at(row, 7)?;
            Ok(GraphRecallNode {
                root_node_id: row.get(0)?,
                root_node_type: row.get(1)?,
                edge_source_node_id: row.get(2)?,
                link: Link {
                    id: row.get(4)?,
                    source_node_id: row.get(2)?,
                    target_node_id: node.id,
                    link_type: row.get(5)?,
                    created_at: row.get(6)?,
                },
                depth,
                node,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = nodes.len() > limit;
    nodes.truncate(limit);

    Ok((nodes, more_results))
}

pub fn list_nodes_page(
    connection: &Connection,
    after_id: Option<i64>,
    limit: usize,
    include_body: bool,
) -> rusqlite::Result<NodePage> {
    let fetch_limit = page_fetch_limit(limit)?;
    let after_id = after_id.unwrap_or(0);

    if include_body {
        let mut statement = connection.prepare(
            "
            SELECT
                id,
                node_type,
                status,
                title,
                summary,
                body,
                source_ref,
                confidence,
                trust_level,
                created_at,
                updated_at
            FROM nodes
            WHERE id > ?1
            ORDER BY id ASC
            LIMIT ?2;
            ",
        )?;
        let mut rows = statement
            .query_map(params![after_id, fetch_limit], row_to_node)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let more_results = rows.len() > limit;
        rows.truncate(limit);

        return Ok(NodePage {
            page: make_page_with_more_results(rows, more_results, |node| node.id),
            body_omitted: false,
            content_truncated: false,
        });
    }

    let mut statement = connection.prepare(
        "
        SELECT
            id,
            node_type,
            status,
            title,
            summary,
            NULL AS body,
            source_ref,
            confidence,
            trust_level,
            created_at,
            updated_at,
            body IS NOT NULL AS body_omitted
        FROM nodes
        WHERE id > ?1
        ORDER BY id ASC
        LIMIT ?2;
        ",
    )?;
    let mut rows = statement
        .query_map(params![after_id, fetch_limit], |row| {
            Ok((row_to_node(row)?, row.get::<_, i64>(11)? != 0))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = rows.len() > limit;
    rows.truncate(limit);
    let body_omitted = rows.iter().any(|(_, omitted)| *omitted);
    let nodes = rows.into_iter().map(|(node, _)| node).collect();

    Ok(NodePage {
        page: make_page_with_more_results(nodes, more_results, |node| node.id),
        body_omitted,
        content_truncated: false,
    })
}

pub fn list_nodes_with_summaries(
    connection: &Connection,
    summaries: &[&str],
) -> rusqlite::Result<Vec<Node>> {
    if summaries.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = (1..=summaries.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "
        SELECT
            id,
            node_type,
            status,
            title,
            summary,
            body,
            source_ref,
            confidence,
            trust_level,
            created_at,
            updated_at
        FROM nodes
        WHERE summary IN ({placeholders})
        ORDER BY id ASC;
        "
    );
    let mut statement = connection.prepare(&sql)?;

    let nodes = statement
        .query_map(
            rusqlite::params_from_iter(summaries.iter().copied()),
            row_to_node,
        )?
        .collect();
    nodes
}

pub fn search_nodes_fts(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> rusqlite::Result<Vec<FtsNodeSearchResult>> {
    let Some(match_query) = fts_match_query(query) else {
        return Ok(Vec::new());
    };
    let mut statement = connection.prepare(
        "
        SELECT
            nodes.id,
            nodes.node_type,
            nodes.status,
            nodes.title,
            nodes.summary,
            nodes.body,
            nodes.source_ref,
            nodes.confidence,
            nodes.trust_level,
            nodes.created_at,
            nodes.updated_at,
            bm25(fts_nodes) AS rank
        FROM fts_nodes
        JOIN nodes ON nodes.id = fts_nodes.rowid
        WHERE fts_nodes MATCH ?1
            AND nodes.status NOT IN ('deprecated', 'superseded')
        ORDER BY rank ASC, nodes.id ASC
        LIMIT ?2;
        ",
    )?;

    let results = statement
        .query_map(params![match_query, limit as i64], |row| {
            Ok(FtsNodeSearchResult {
                rank: row.get(11)?,
                node: row_to_node(row)?,
            })
        })?
        .collect();
    results
}

pub fn search_recall_query_fts(
    connection: &Connection,
    query: &str,
    limit: usize,
) -> rusqlite::Result<BoundedRecallSearch> {
    if limit == 0 {
        return Ok(BoundedRecallSearch::default());
    }
    let Some(match_query) = fts_match_query(query) else {
        return Ok(BoundedRecallSearch::default());
    };
    let fetch_limit = limit.saturating_add(1).min(i64::MAX as usize) as i64;
    let mut statement = connection.prepare(
        "
        SELECT
            nodes.id,
            nodes.node_type,
            nodes.status,
            substr(nodes.title, 1, ?2),
            substr(nodes.summary, 1, ?2),
            NULL AS body,
            substr(nodes.source_ref, 1, ?2),
            nodes.confidence,
            nodes.trust_level,
            nodes.created_at,
            nodes.updated_at,
            bm25(fts_nodes) AS rank,
            (
                nodes.body IS NOT NULL
                OR length(nodes.title) > ?2
                OR COALESCE(length(nodes.summary) > ?2, 0)
                OR COALESCE(length(nodes.source_ref) > ?2, 0)
            ) AS content_truncated
        FROM fts_nodes
        JOIN nodes ON nodes.id = fts_nodes.rowid
        WHERE fts_nodes MATCH ?1
            AND nodes.status NOT IN ('deprecated', 'superseded')
        ORDER BY rank ASC, nodes.id ASC
        LIMIT ?3;
        ",
    )?;

    let mut rows = statement
        .query_map(
            params![match_query, BOUNDED_RECALL_FIELD_MAX_CHARS, fetch_limit],
            |row| {
                Ok((
                    FtsNodeSearchResult {
                        rank: row.get(11)?,
                        node: row_to_node(row)?,
                    },
                    row.get::<_, i64>(12)? != 0,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = rows.len() > limit;
    rows.truncate(limit);
    let content_truncated = rows.iter().any(|(_, truncated)| *truncated);
    let results = rows.into_iter().map(|(result, _)| result).collect();

    Ok(BoundedRecallSearch {
        results,
        more_results,
        content_truncated,
    })
}

pub fn load_bounded_legacy_recall(
    connection: &Connection,
) -> rusqlite::Result<BoundedLegacyRecall> {
    let (mut nodes, mut more_results, mut content_truncated) =
        list_bounded_legacy_recall_roots(connection)?;
    let root_ids = nodes.iter().map(|node| node.id).collect::<BTreeSet<_>>();

    let (mut links, links_more, links_truncated) =
        list_bounded_outgoing_links(connection, &root_ids, LEGACY_RECALL_LINK_LIMIT)?;
    more_results |= links_more;
    content_truncated |= links_truncated;

    let depth_one_target_ids = links
        .iter()
        .map(|link| link.target_node_id)
        .filter(|id| !root_ids.contains(id))
        .collect::<BTreeSet<_>>();
    let (depth_one_nodes, depth_one_truncated) =
        list_bounded_recall_nodes_by_id(connection, &depth_one_target_ids)?;
    content_truncated |= depth_one_truncated;

    let depth_two_source_ids = depth_one_nodes
        .iter()
        .map(|node| node.id)
        .collect::<BTreeSet<_>>();
    nodes.extend(depth_one_nodes);

    let remaining_link_limit = LEGACY_RECALL_LINK_LIMIT.saturating_sub(links.len());
    let (depth_two_links, depth_two_more, depth_two_links_truncated) =
        list_bounded_outgoing_links(connection, &depth_two_source_ids, remaining_link_limit)?;
    more_results |= depth_two_more;
    content_truncated |= depth_two_links_truncated;

    let selected_node_ids = nodes.iter().map(|node| node.id).collect::<BTreeSet<_>>();
    let depth_two_target_ids = depth_two_links
        .iter()
        .map(|link| link.target_node_id)
        .filter(|id| !selected_node_ids.contains(id))
        .collect::<BTreeSet<_>>();
    let (depth_two_nodes, depth_two_truncated) =
        list_bounded_recall_nodes_by_id(connection, &depth_two_target_ids)?;
    content_truncated |= depth_two_truncated;

    links.extend(depth_two_links);
    nodes.extend(depth_two_nodes);
    nodes.sort_by_key(|node| node.id);

    Ok(BoundedLegacyRecall {
        nodes,
        links,
        more_results,
        content_truncated,
    })
}

fn list_bounded_legacy_recall_roots(
    connection: &Connection,
) -> rusqlite::Result<(Vec<Node>, bool, bool)> {
    let fetch_limit = LEGACY_RECALL_ROOT_LIMIT_PER_TYPE.saturating_add(1) as i64;
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            node_type,
            status,
            substr(title, 1, ?2),
            substr(summary, 1, ?2),
            substr(body, 1, ?2),
            substr(source_ref, 1, ?2),
            confidence,
            substr(trust_level, 1, ?2),
            created_at,
            updated_at,
            (
                length(title) > ?2
                OR COALESCE(length(summary) > ?2, 0)
                OR COALESCE(length(body) > ?2, 0)
                OR COALESCE(length(source_ref) > ?2, 0)
                OR COALESCE(length(trust_level) > ?2, 0)
            ) AS content_truncated
        FROM nodes
        WHERE node_type = ?1
            AND status NOT IN ('deprecated', 'superseded')
        ORDER BY id ASC
        LIMIT ?3;
        ",
    )?;
    let mut nodes = Vec::new();
    let mut more_results = false;
    let mut content_truncated = false;

    for node_type in LEGACY_RECALL_ROOT_TYPES {
        let mut rows = statement
            .query_map(
                params![node_type, BOUNDED_RECALL_FIELD_MAX_CHARS, fetch_limit],
                row_to_bounded_recall_node,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        more_results |= rows.len() > LEGACY_RECALL_ROOT_LIMIT_PER_TYPE;
        rows.truncate(LEGACY_RECALL_ROOT_LIMIT_PER_TYPE);
        content_truncated |= rows.iter().any(|(_, truncated)| *truncated);
        nodes.extend(rows.into_iter().map(|(node, _)| node));
    }

    nodes.sort_by_key(|node| node.id);
    Ok((nodes, more_results, content_truncated))
}

fn list_bounded_recall_nodes_by_id(
    connection: &Connection,
    node_ids: &BTreeSet<i64>,
) -> rusqlite::Result<(Vec<Node>, bool)> {
    if node_ids.is_empty() {
        return Ok((Vec::new(), false));
    }

    let placeholders = (2..node_ids.len() + 2)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "
        SELECT
            id,
            node_type,
            status,
            substr(title, 1, ?1),
            substr(summary, 1, ?1),
            substr(body, 1, ?1),
            substr(source_ref, 1, ?1),
            confidence,
            substr(trust_level, 1, ?1),
            created_at,
            updated_at,
            (
                length(title) > ?1
                OR COALESCE(length(summary) > ?1, 0)
                OR COALESCE(length(body) > ?1, 0)
                OR COALESCE(length(source_ref) > ?1, 0)
                OR COALESCE(length(trust_level) > ?1, 0)
            ) AS content_truncated
        FROM nodes
        WHERE id IN ({placeholders})
            AND status NOT IN ('deprecated', 'superseded')
        ORDER BY id ASC;
        "
    );
    let mut parameters = Vec::with_capacity(node_ids.len() + 1);
    parameters.push(BOUNDED_RECALL_FIELD_MAX_CHARS);
    parameters.extend(node_ids.iter().copied());
    let mut statement = connection.prepare(&sql)?;
    let rows = statement
        .query_map(
            rusqlite::params_from_iter(parameters),
            row_to_bounded_recall_node,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let content_truncated = rows.iter().any(|(_, truncated)| *truncated);

    Ok((
        rows.into_iter().map(|(node, _)| node).collect(),
        content_truncated,
    ))
}

fn list_bounded_outgoing_links(
    connection: &Connection,
    source_node_ids: &BTreeSet<i64>,
    limit: usize,
) -> rusqlite::Result<(Vec<Link>, bool, bool)> {
    if source_node_ids.is_empty() {
        return Ok((Vec::new(), false, false));
    }

    let source_placeholders = (1..=source_node_ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let field_limit_parameter = source_node_ids.len() + 1;
    let fetch_limit_parameter = source_node_ids.len() + 2;
    let sql = format!(
        "
        SELECT
            links.id,
            links.source_node_id,
            links.target_node_id,
            substr(links.link_type, 1, ?{field_limit_parameter}),
            links.created_at,
            length(links.link_type) > ?{field_limit_parameter} AS content_truncated
        FROM links
        JOIN nodes AS targets ON targets.id = links.target_node_id
        WHERE links.source_node_id IN ({source_placeholders})
            AND targets.status NOT IN ('deprecated', 'superseded')
        ORDER BY links.source_node_id ASC, links.id ASC
        LIMIT ?{fetch_limit_parameter};
        "
    );
    let fetch_limit = limit.saturating_add(1).min(i64::MAX as usize) as i64;
    let mut parameters = Vec::with_capacity(source_node_ids.len() + 2);
    parameters.extend(source_node_ids.iter().copied());
    parameters.push(BOUNDED_RECALL_FIELD_MAX_CHARS);
    parameters.push(fetch_limit);
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement
        .query_map(rusqlite::params_from_iter(parameters), |row| {
            Ok((row_to_link(row)?, row.get::<_, i64>(5)? != 0))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more_results = rows.len() > limit;
    rows.truncate(limit);
    let content_truncated = rows.iter().any(|(_, truncated)| *truncated);
    let links = rows.into_iter().map(|(link, _)| link).collect();

    Ok((links, more_results, content_truncated))
}

fn row_to_bounded_recall_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<(Node, bool)> {
    Ok((row_to_node(row)?, row.get::<_, i64>(11)? != 0))
}

pub fn create_link(connection: &Connection, link: &NewLink) -> Result<Link, LinkStorageError> {
    validate_new_link(connection, link)?;

    connection
        .prepare_cached(INSERT_LINK_SQL)?
        .execute(params![
            link.source_node_id,
            link.target_node_id,
            &link.link_type
        ])?;

    let id = connection.last_insert_rowid();
    let created = get_link(connection, id)?
        .ok_or(LinkStorageError::Db(rusqlite::Error::QueryReturnedNoRows))?;
    audit::record_link_created(connection, created.id, STORAGE_AUDIT_SOURCE)
        .map_err(audit_error_to_link_db)?;

    Ok(created)
}

pub fn validate_new_link_input(link: &NewLink) -> Result<(), LinkValidationError> {
    if link.link_type.trim().is_empty() {
        return Err(LinkValidationError::MissingType);
    }
    if link.link_type.len() > MAX_LINK_TYPE_BYTES {
        return Err(LinkValidationError::TypeTooLong {
            max_bytes: MAX_LINK_TYPE_BYTES,
        });
    }
    Ok(())
}

fn audit_error_to_db(error: AuditError) -> NodeStorageError {
    NodeStorageError::Db(match error {
        AuditError::Db(error) => error,
        AuditError::Validation(error) => rusqlite::Error::ToSqlConversionFailure(Box::new(error)),
    })
}

fn audit_error_to_link_db(error: AuditError) -> LinkStorageError {
    LinkStorageError::Db(match error {
        AuditError::Db(error) => error,
        AuditError::Validation(error) => rusqlite::Error::ToSqlConversionFailure(Box::new(error)),
    })
}

pub fn list_links(connection: &Connection) -> rusqlite::Result<Vec<Link>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            source_node_id,
            target_node_id,
            link_type,
            created_at
        FROM links
        ORDER BY id ASC;
        ",
    )?;

    let links = statement.query_map([], row_to_link)?.collect();
    links
}

pub fn list_links_page(
    connection: &Connection,
    after_id: Option<i64>,
    limit: usize,
) -> rusqlite::Result<Page<Link>> {
    let fetch_limit = page_fetch_limit(limit)?;
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            source_node_id,
            target_node_id,
            link_type,
            created_at
        FROM links
        WHERE id > ?1
        ORDER BY id ASC
        LIMIT ?2;
        ",
    )?;
    let links = statement
        .query_map(params![after_id.unwrap_or(0), fetch_limit], row_to_link)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(make_page(links, limit, |link| link.id))
}

pub fn create_alias(
    connection: &Connection,
    alias: &NewAlias,
) -> Result<Alias, MetadataStorageError> {
    let created = create_alias_deferred_fts(connection, alias)?;
    refresh_fts_node(connection, created.node_id)?;

    Ok(created)
}

pub(crate) fn create_alias_deferred_fts(
    connection: &Connection,
    alias: &NewAlias,
) -> Result<Alias, MetadataStorageError> {
    validate_node_metadata(connection, alias.node_id, &alias.alias, MetadataKind::Alias)?;

    connection
        .prepare_cached(INSERT_ALIAS_SQL)?
        .execute(params![alias.node_id, &alias.alias])?;

    let id = connection.last_insert_rowid();
    let created = get_alias(connection, id)?.ok_or(MetadataStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))?;

    Ok(created)
}

pub fn list_aliases(connection: &Connection, node_id: Option<i64>) -> rusqlite::Result<Vec<Alias>> {
    match node_id {
        Some(node_id) => {
            let mut statement = connection.prepare(
                "
                    SELECT id, node_id, alias, created_at
                    FROM aliases
                    WHERE node_id = ?1
                    ORDER BY id ASC;
                ",
            )?;
            let aliases = statement.query_map([node_id], row_to_alias)?.collect();
            aliases
        }
        None => {
            let mut statement = connection.prepare(
                "
                    SELECT id, node_id, alias, created_at
                    FROM aliases
                    ORDER BY id ASC;
                ",
            )?;
            let aliases = statement.query_map([], row_to_alias)?.collect();
            aliases
        }
    }
}

pub fn list_aliases_page(
    connection: &Connection,
    node_id: Option<i64>,
    after_id: Option<i64>,
    limit: usize,
) -> rusqlite::Result<Page<Alias>> {
    let fetch_limit = page_fetch_limit(limit)?;
    let aliases = match node_id {
        Some(node_id) => {
            let mut statement = connection.prepare(
                "
                SELECT id, node_id, alias, created_at
                FROM aliases
                WHERE node_id = ?1 AND id > ?2
                ORDER BY id ASC
                LIMIT ?3;
                ",
            )?;
            let aliases = statement
                .query_map(
                    params![node_id, after_id.unwrap_or(0), fetch_limit],
                    row_to_alias,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            aliases
        }
        None => {
            let mut statement = connection.prepare(
                "
                SELECT id, node_id, alias, created_at
                FROM aliases
                WHERE id > ?1
                ORDER BY id ASC
                LIMIT ?2;
                ",
            )?;
            let aliases = statement
                .query_map(params![after_id.unwrap_or(0), fetch_limit], row_to_alias)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            aliases
        }
    };

    Ok(make_page(aliases, limit, |alias| alias.id))
}

pub fn create_tag(connection: &Connection, tag: &NewTag) -> Result<Tag, MetadataStorageError> {
    validate_node_metadata(connection, tag.node_id, &tag.tag, MetadataKind::Tag)?;

    connection
        .prepare_cached(INSERT_TAG_SQL)?
        .execute(params![tag.node_id, &tag.tag])?;

    let id = connection.last_insert_rowid();
    get_tag(connection, id)?.ok_or(MetadataStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))
}

pub fn list_tags(connection: &Connection, node_id: Option<i64>) -> rusqlite::Result<Vec<Tag>> {
    match node_id {
        Some(node_id) => {
            let mut statement = connection.prepare(
                "
                    SELECT id, node_id, tag, created_at
                    FROM tags
                    WHERE node_id = ?1
                    ORDER BY id ASC;
                ",
            )?;
            let tags = statement.query_map([node_id], row_to_tag)?.collect();
            tags
        }
        None => {
            let mut statement = connection.prepare(
                "
                    SELECT id, node_id, tag, created_at
                    FROM tags
                    ORDER BY id ASC;
                ",
            )?;
            let tags = statement.query_map([], row_to_tag)?.collect();
            tags
        }
    }
}

pub fn list_tags_page(
    connection: &Connection,
    node_id: Option<i64>,
    after_id: Option<i64>,
    limit: usize,
) -> rusqlite::Result<Page<Tag>> {
    let fetch_limit = page_fetch_limit(limit)?;
    let tags = match node_id {
        Some(node_id) => {
            let mut statement = connection.prepare(
                "
                SELECT id, node_id, tag, created_at
                FROM tags
                WHERE node_id = ?1 AND id > ?2
                ORDER BY id ASC
                LIMIT ?3;
                ",
            )?;
            let tags = statement
                .query_map(
                    params![node_id, after_id.unwrap_or(0), fetch_limit],
                    row_to_tag,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            tags
        }
        None => {
            let mut statement = connection.prepare(
                "
                SELECT id, node_id, tag, created_at
                FROM tags
                WHERE id > ?1
                ORDER BY id ASC
                LIMIT ?2;
                ",
            )?;
            let tags = statement
                .query_map(params![after_id.unwrap_or(0), fetch_limit], row_to_tag)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            tags
        }
    };

    Ok(make_page(tags, limit, |tag| tag.id))
}

pub fn create_source(
    connection: &Connection,
    source: &NewSource,
) -> Result<Source, MetadataStorageError> {
    validate_node_metadata(
        connection,
        source.node_id,
        &source.source_ref,
        MetadataKind::Source,
    )?;

    connection
        .prepare_cached(INSERT_SOURCE_SQL)?
        .execute(params![source.node_id, &source.source_ref])?;

    let id = connection.last_insert_rowid();
    get_source(connection, id)?.ok_or(MetadataStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))
}

pub fn list_sources(
    connection: &Connection,
    node_id: Option<i64>,
) -> rusqlite::Result<Vec<Source>> {
    match node_id {
        Some(node_id) => {
            let mut statement = connection.prepare(
                "
                    SELECT id, node_id, source_ref, created_at
                    FROM sources
                    WHERE node_id = ?1
                    ORDER BY id ASC;
                ",
            )?;
            let sources = statement.query_map([node_id], row_to_source)?.collect();
            sources
        }
        None => {
            let mut statement = connection.prepare(
                "
                    SELECT id, node_id, source_ref, created_at
                    FROM sources
                    ORDER BY id ASC;
                ",
            )?;
            let sources = statement.query_map([], row_to_source)?.collect();
            sources
        }
    }
}

pub fn list_sources_page(
    connection: &Connection,
    node_id: Option<i64>,
    after_id: Option<i64>,
    limit: usize,
) -> rusqlite::Result<Page<Source>> {
    let fetch_limit = page_fetch_limit(limit)?;
    let sources = match node_id {
        Some(node_id) => {
            let mut statement = connection.prepare(
                "
                SELECT id, node_id, source_ref, created_at
                FROM sources
                WHERE node_id = ?1 AND id > ?2
                ORDER BY id ASC
                LIMIT ?3;
                ",
            )?;
            let sources = statement
                .query_map(
                    params![node_id, after_id.unwrap_or(0), fetch_limit],
                    row_to_source,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            sources
        }
        None => {
            let mut statement = connection.prepare(
                "
                SELECT id, node_id, source_ref, created_at
                FROM sources
                WHERE id > ?1
                ORDER BY id ASC
                LIMIT ?2;
                ",
            )?;
            let sources = statement
                .query_map(params![after_id.unwrap_or(0), fetch_limit], row_to_source)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            sources
        }
    };

    Ok(make_page(sources, limit, |source| source.id))
}

pub fn create_teach_session(
    connection: &Connection,
    session: &NewTeachSession,
) -> Result<TeachSession, TeachStorageError> {
    validate_new_teach_session_input(session)?;

    let record = TeachSessionRecordRef {
        session_title: &session.title,
        session_summary: session.summary.as_deref(),
    };
    let node = create_teach_record_node(
        connection,
        &session.title,
        Some(TEACH_SESSION_SUMMARY.to_string()),
        &record,
    )?;

    Ok(TeachSession {
        session_id: node.id,
        title: node.title,
        summary: session.summary.clone(),
        created_at: node.created_at,
        updated_at: node.updated_at,
    })
}

pub fn validate_new_teach_session_input(
    session: &NewTeachSession,
) -> Result<(), TeachStorageError> {
    if session.title.trim().is_empty() {
        return Err(TeachValidationError::MissingSessionTitle.into());
    }

    let record = TeachSessionRecordRef {
        session_title: &session.title,
        session_summary: session.summary.as_deref(),
    };
    let body = serde_json::to_string(&record)?;
    validate_new_node_input(&NewNode {
        node_type: "raw_note".to_string(),
        status: "draft".to_string(),
        title: session.title.clone(),
        summary: Some(TEACH_SESSION_SUMMARY.to_string()),
        body: Some(body),
        source_ref: None,
        confidence: None,
        trust_level: None,
    })
    .map_err(NodeStorageError::Validation)?;
    Ok(())
}

pub fn add_teach_material(
    connection: &Connection,
    session_id: i64,
    payload: &Value,
) -> Result<TeachMaterial, TeachStorageError> {
    let session = require_teach_session(connection, session_id)?;
    let record = TeachMaterialRecordRef {
        session_id,
        payload,
    };
    let node = create_teach_record_node(
        connection,
        &format!("Teach material {}", session.session_id),
        Some(TEACH_MATERIAL_SUMMARY.to_string()),
        &record,
    )?;
    create_link(
        connection,
        &NewLink {
            source_node_id: session_id,
            target_node_id: node.id,
            link_type: TEACH_MATERIAL_LINK_TYPE.to_string(),
        },
    )?;

    Ok(TeachMaterial {
        material_id: node.id,
        session_id,
        payload: payload.clone(),
        created_at: node.created_at,
    })
}

pub fn store_teach_proposal(
    connection: &Connection,
    session_id: i64,
    proposal: &TeachProposalInput,
) -> Result<TeachProposal, TeachStorageError> {
    validate_teach_proposal_input(session_id, proposal)?;
    let session = require_teach_session(connection, session_id)?;

    let record = TeachProposalRecordRef {
        session_id,
        items: &proposal.items,
    };
    let node = create_teach_record_node(
        connection,
        &format!("Teach proposal {}", session.session_id),
        Some(TEACH_PROPOSAL_SUMMARY.to_string()),
        &record,
    )?;
    create_link(
        connection,
        &NewLink {
            source_node_id: session_id,
            target_node_id: node.id,
            link_type: TEACH_PROPOSAL_LINK_TYPE.to_string(),
        },
    )?;

    Ok(TeachProposal {
        proposal_id: node.id,
        session_id,
        items: proposal.items.clone(),
        created_at: node.created_at,
    })
}

pub fn validate_teach_proposal_input(
    session_id: i64,
    proposal: &TeachProposalInput,
) -> Result<(), TeachStorageError> {
    validate_teach_proposal(proposal)?;
    let record = TeachProposalRecordRef {
        session_id,
        items: &proposal.items,
    };
    let body = serde_json::to_string(&record)?;
    validate_new_node_input(&NewNode {
        node_type: "raw_note".to_string(),
        status: "draft".to_string(),
        title: format!("Teach proposal {session_id}"),
        summary: Some(TEACH_PROPOSAL_SUMMARY.to_string()),
        body: Some(body),
        source_ref: None,
        confidence: None,
        trust_level: None,
    })
    .map_err(NodeStorageError::Validation)?;
    Ok(())
}

pub fn apply_teach_proposal(
    connection: &Connection,
    session_id: i64,
    proposal_id: i64,
) -> Result<TeachApplyReport, TeachStorageError> {
    if connection.is_autocommit() {
        let transaction = connection.unchecked_transaction()?;
        let report = apply_teach_proposal_in_transaction(&transaction, session_id, proposal_id)?;
        transaction.commit()?;
        Ok(report)
    } else {
        connection.execute_batch(&format!("SAVEPOINT {TEACH_APPLY_SAVEPOINT};"))?;
        match apply_teach_proposal_in_transaction(connection, session_id, proposal_id) {
            Ok(report) => {
                connection.execute_batch(&format!("RELEASE SAVEPOINT {TEACH_APPLY_SAVEPOINT};"))?;
                Ok(report)
            }
            Err(error) => {
                connection.execute_batch(&format!(
                    "ROLLBACK TO SAVEPOINT {TEACH_APPLY_SAVEPOINT};\
                     RELEASE SAVEPOINT {TEACH_APPLY_SAVEPOINT};"
                ))?;
                Err(error)
            }
        }
    }
}

fn apply_teach_proposal_in_transaction(
    connection: &Connection,
    session_id: i64,
    proposal_id: i64,
) -> Result<TeachApplyReport, TeachStorageError> {
    require_teach_session(connection, session_id)?;
    let proposal = load_teach_proposal(connection, proposal_id)?;
    if proposal.session_id != session_id {
        return Err(TeachValidationError::ProposalSessionMismatch {
            session_id,
            proposal_id,
        }
        .into());
    }

    let mut resolved_node_refs = HashMap::with_capacity(proposal.items.len());
    let mut created_node_ids = Vec::new();
    let mut created_alias_ids = Vec::new();
    let mut created_tag_ids = Vec::new();
    let mut created_source_ids = Vec::new();
    let mut created_link_ids = Vec::new();
    let mut alias_node_ids = BTreeSet::new();

    for item in &proposal.items {
        match item {
            TeachProposalItem::CreateNode {
                node_ref,
                node_type,
                status,
                title,
                summary,
                body,
                source_ref,
                confidence,
                trust_level,
            } => {
                if let Some(node_ref) = node_ref {
                    if resolved_node_refs.contains_key(node_ref) {
                        return Err(TeachValidationError::DuplicateNodeRef(node_ref.clone()).into());
                    }
                }

                let created = create_node_borrowed(
                    connection,
                    BorrowedNodeInput {
                        node_type,
                        status,
                        title,
                        summary: summary.as_deref(),
                        body: body.as_deref(),
                        source_ref: source_ref.as_deref(),
                        confidence: *confidence,
                        trust_level: trust_level.as_deref(),
                    },
                )?;
                if let Some(node_ref) = node_ref {
                    resolved_node_refs.insert(node_ref.clone(), created.id);
                }
                created_node_ids.push(created.id);
            }
            TeachProposalItem::AddAlias {
                node_id,
                node_ref,
                alias,
            } => {
                let target_id =
                    resolve_teach_node_target(*node_id, node_ref.as_deref(), &resolved_node_refs)?;
                let created = create_alias_deferred_fts(
                    connection,
                    &NewAlias {
                        node_id: target_id,
                        alias: alias.clone(),
                    },
                )?;
                alias_node_ids.insert(created.node_id);
                created_alias_ids.push(created.id);
            }
            TeachProposalItem::AddTag {
                node_id,
                node_ref,
                tag,
            } => {
                let target_id =
                    resolve_teach_node_target(*node_id, node_ref.as_deref(), &resolved_node_refs)?;
                let created = create_tag(
                    connection,
                    &NewTag {
                        node_id: target_id,
                        tag: tag.clone(),
                    },
                )?;
                created_tag_ids.push(created.id);
            }
            TeachProposalItem::AddSource {
                node_id,
                node_ref,
                source_ref,
            } => {
                let target_id =
                    resolve_teach_node_target(*node_id, node_ref.as_deref(), &resolved_node_refs)?;
                let created = create_source(
                    connection,
                    &NewSource {
                        node_id: target_id,
                        source_ref: source_ref.clone(),
                    },
                )?;
                created_source_ids.push(created.id);
            }
            TeachProposalItem::AddLink {
                source_node_id,
                source_node_ref,
                target_node_id,
                target_node_ref,
                link_type,
            } => {
                let source_id = resolve_teach_node_target(
                    *source_node_id,
                    source_node_ref.as_deref(),
                    &resolved_node_refs,
                )?;
                let target_id = resolve_teach_node_target(
                    *target_node_id,
                    target_node_ref.as_deref(),
                    &resolved_node_refs,
                )?;
                let created = create_link(
                    connection,
                    &NewLink {
                        source_node_id: source_id,
                        target_node_id: target_id,
                        link_type: link_type.clone(),
                    },
                )?;
                created_link_ids.push(created.id);
            }
        }
    }

    let receipt = TeachApplyReceiptRecordRef {
        session_id,
        proposal_id,
        created_node_ids: &created_node_ids,
        created_alias_ids: &created_alias_ids,
        created_tag_ids: &created_tag_ids,
        created_source_ids: &created_source_ids,
        created_link_ids: &created_link_ids,
    };
    let receipt_node = create_teach_record_node(
        connection,
        &format!("Teach apply {session_id}/{proposal_id}"),
        Some(TEACH_APPLY_SUMMARY.to_string()),
        &receipt,
    )?;
    create_link(
        connection,
        &NewLink {
            source_node_id: session_id,
            target_node_id: receipt_node.id,
            link_type: TEACH_APPLY_LINK_TYPE.to_string(),
        },
    )?;
    for node_id in &created_node_ids {
        create_link(
            connection,
            &NewLink {
                source_node_id: receipt_node.id,
                target_node_id: *node_id,
                link_type: TEACH_CREATED_LINK_TYPE.to_string(),
            },
        )?;
    }
    refresh_fts_nodes(connection, &alias_node_ids)?;

    Ok(TeachApplyReport {
        session_id,
        proposal_id,
        receipt_id: receipt_node.id,
        created_node_ids,
        created_alias_ids,
        created_tag_ids,
        created_source_ids,
        created_link_ids,
    })
}

pub fn create_mcp_profile(
    connection: &Connection,
    profile: &NewMcpProfile,
) -> Result<McpProfile, McpProfileStorageError> {
    validate_new_mcp_profile(profile)?;

    connection.execute(
        "
            INSERT INTO mcp_profiles (
                id,
                name,
                kind,
                status,
                read_operations,
                write_operations,
                side_effects,
                approval_requirement,
                credentials_source,
                notes
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);
            ",
        params![
            &profile.id,
            &profile.name,
            &profile.kind,
            &profile.status,
            &profile.read_operations,
            &profile.write_operations,
            &profile.side_effects,
            &profile.approval_requirement,
            &profile.credentials_source,
            &profile.notes
        ],
    )?;

    get_mcp_profile(connection, &profile.id)?.ok_or(McpProfileStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))
}

pub fn validate_new_mcp_profile_input(
    profile: &NewMcpProfile,
) -> Result<(), McpProfileValidationError> {
    validate_new_mcp_profile(profile)
}

pub fn upsert_mcp_profile(
    connection: &Connection,
    profile: &NewMcpProfile,
) -> Result<McpProfile, McpProfileStorageError> {
    validate_new_mcp_profile(profile)?;

    connection.execute(
        "
            INSERT INTO mcp_profiles (
                id,
                name,
                kind,
                status,
                read_operations,
                write_operations,
                side_effects,
                approval_requirement,
                credentials_source,
                notes
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                kind = excluded.kind,
                status = excluded.status,
                read_operations = excluded.read_operations,
                write_operations = excluded.write_operations,
                side_effects = excluded.side_effects,
                approval_requirement = excluded.approval_requirement,
                credentials_source = excluded.credentials_source,
                notes = excluded.notes,
                updated_at = CURRENT_TIMESTAMP;
            ",
        params![
            &profile.id,
            &profile.name,
            &profile.kind,
            &profile.status,
            &profile.read_operations,
            &profile.write_operations,
            &profile.side_effects,
            &profile.approval_requirement,
            &profile.credentials_source,
            &profile.notes
        ],
    )?;

    get_mcp_profile(connection, &profile.id)?.ok_or(McpProfileStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))
}

pub fn get_mcp_profile(connection: &Connection, id: &str) -> rusqlite::Result<Option<McpProfile>> {
    connection
        .query_row(
            "
            SELECT
                id,
                name,
                kind,
                status,
                read_operations,
                write_operations,
                side_effects,
                approval_requirement,
                credentials_source,
                notes,
                created_at,
                updated_at
            FROM mcp_profiles
            WHERE id = ?1;
            ",
            [id],
            row_to_mcp_profile,
        )
        .optional()
}

pub fn list_mcp_profiles(connection: &Connection) -> rusqlite::Result<Vec<McpProfile>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            name,
            kind,
            status,
            read_operations,
            write_operations,
            side_effects,
            approval_requirement,
            credentials_source,
            notes,
            created_at,
            updated_at
        FROM mcp_profiles
        ORDER BY id ASC;
        ",
    )?;

    let profiles = statement.query_map([], row_to_mcp_profile)?.collect();
    profiles
}

pub fn list_mcp_profiles_page(
    connection: &Connection,
    after_id: Option<&str>,
    limit: usize,
) -> rusqlite::Result<Page<McpProfile, String>> {
    let fetch_limit = page_fetch_limit(limit)?;
    let profiles = match after_id {
        Some(after_id) => {
            let mut statement = connection.prepare(
                "
                SELECT
                    id,
                    name,
                    kind,
                    status,
                    read_operations,
                    write_operations,
                    side_effects,
                    approval_requirement,
                    credentials_source,
                    notes,
                    created_at,
                    updated_at
                FROM mcp_profiles
                WHERE id > ?1
                ORDER BY id ASC
                LIMIT ?2;
                ",
            )?;
            let profiles = statement
                .query_map(params![after_id, fetch_limit], row_to_mcp_profile)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            profiles
        }
        None => {
            let mut statement = connection.prepare(
                "
                SELECT
                    id,
                    name,
                    kind,
                    status,
                    read_operations,
                    write_operations,
                    side_effects,
                    approval_requirement,
                    credentials_source,
                    notes,
                    created_at,
                    updated_at
                FROM mcp_profiles
                ORDER BY id ASC
                LIMIT ?1;
                ",
            )?;
            let profiles = statement
                .query_map([fetch_limit], row_to_mcp_profile)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            profiles
        }
    };

    Ok(make_page(profiles, limit, |profile| profile.id.clone()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TeachSessionRecord {
    session_title: String,
    session_summary: Option<String>,
}

#[derive(Serialize)]
struct TeachSessionRecordRef<'a> {
    session_title: &'a str,
    session_summary: Option<&'a str>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TeachMaterialRecord {
    session_id: i64,
    payload: Value,
}

#[derive(Serialize)]
struct TeachMaterialRecordRef<'a> {
    session_id: i64,
    payload: &'a Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TeachProposalRecord {
    session_id: i64,
    items: Vec<TeachProposalItem>,
}

#[derive(Serialize)]
struct TeachProposalRecordRef<'a> {
    session_id: i64,
    items: &'a [TeachProposalItem],
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TeachApplyReceiptRecord {
    session_id: i64,
    proposal_id: i64,
    created_node_ids: Vec<i64>,
    created_alias_ids: Vec<i64>,
    created_tag_ids: Vec<i64>,
    created_source_ids: Vec<i64>,
    created_link_ids: Vec<i64>,
}

#[derive(Serialize)]
struct TeachApplyReceiptRecordRef<'a> {
    session_id: i64,
    proposal_id: i64,
    created_node_ids: &'a [i64],
    created_alias_ids: &'a [i64],
    created_tag_ids: &'a [i64],
    created_source_ids: &'a [i64],
    created_link_ids: &'a [i64],
}

fn create_teach_record_node<T: Serialize>(
    connection: &Connection,
    title: &str,
    summary: Option<String>,
    record: &T,
) -> Result<Node, TeachStorageError> {
    let body = serde_json::to_string(record)?;
    let node = create_node(
        connection,
        &NewNode {
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: title.to_string(),
            summary,
            body: Some(body),
            source_ref: None,
            confidence: None,
            trust_level: None,
        },
    )?;

    Ok(node)
}

fn require_teach_session(
    connection: &Connection,
    session_id: i64,
) -> Result<TeachSession, TeachStorageError> {
    let node = get_node(connection, session_id)?
        .ok_or(TeachValidationError::SessionNotFound(session_id))?;
    if node.summary.as_deref() != Some(TEACH_SESSION_SUMMARY) {
        return Err(TeachValidationError::InvalidSessionRecord(session_id).into());
    }
    let record: TeachSessionRecord = serde_json::from_str(
        node.body
            .as_deref()
            .ok_or(TeachValidationError::InvalidSessionRecord(session_id))?,
    )?;

    Ok(TeachSession {
        session_id: node.id,
        title: record.session_title,
        summary: record.session_summary,
        created_at: node.created_at,
        updated_at: node.updated_at,
    })
}

fn load_teach_proposal(
    connection: &Connection,
    proposal_id: i64,
) -> Result<TeachProposal, TeachStorageError> {
    let node = get_node(connection, proposal_id)?
        .ok_or(TeachValidationError::ProposalNotFound(proposal_id))?;
    if node.summary.as_deref() != Some(TEACH_PROPOSAL_SUMMARY) {
        return Err(TeachValidationError::InvalidProposalRecord(proposal_id).into());
    }
    let record: TeachProposalRecord = serde_json::from_str(
        node.body
            .as_deref()
            .ok_or(TeachValidationError::InvalidProposalRecord(proposal_id))?,
    )?;

    Ok(TeachProposal {
        proposal_id: node.id,
        session_id: record.session_id,
        items: record.items,
        created_at: node.created_at,
    })
}

fn validate_teach_proposal(proposal: &TeachProposalInput) -> Result<(), TeachValidationError> {
    if proposal.items.is_empty() {
        return Err(TeachValidationError::EmptyProposalItems);
    }
    if proposal.items.len() > MAX_PROPOSAL_ITEMS {
        return Err(TeachValidationError::TooManyProposalItems {
            max_items: MAX_PROPOSAL_ITEMS,
            actual: proposal.items.len(),
        });
    }

    for item in &proposal.items {
        match item {
            TeachProposalItem::CreateNode { node_ref, .. } => {
                if let Some(node_ref) = node_ref {
                    validate_teach_node_ref(node_ref)?;
                    if node_ref.trim().is_empty() {
                        return Err(TeachValidationError::DuplicateNodeRef(node_ref.clone()));
                    }
                }
            }
            TeachProposalItem::AddAlias {
                node_id, node_ref, ..
            }
            | TeachProposalItem::AddTag {
                node_id, node_ref, ..
            }
            | TeachProposalItem::AddSource {
                node_id, node_ref, ..
            } => validate_teach_node_target(*node_id, node_ref.as_deref())?,
            TeachProposalItem::AddLink {
                source_node_id,
                source_node_ref,
                target_node_id,
                target_node_ref,
                ..
            } => {
                validate_teach_node_target(*source_node_id, source_node_ref.as_deref())?;
                validate_teach_node_target(*target_node_id, target_node_ref.as_deref())?;
            }
        }
    }

    Ok(())
}

fn validate_teach_node_ref(node_ref: &str) -> Result<(), TeachValidationError> {
    if node_ref.len() > MAX_PROPOSAL_NODE_REF_BYTES {
        return Err(TeachValidationError::NodeRefTooLong {
            max_bytes: MAX_PROPOSAL_NODE_REF_BYTES,
        });
    }

    Ok(())
}

fn validate_teach_node_target(
    node_id: Option<i64>,
    node_ref: Option<&str>,
) -> Result<(), TeachValidationError> {
    match (node_id, node_ref) {
        (Some(_), Some(_)) => Err(TeachValidationError::AmbiguousNodeTarget),
        (None, None) => Err(TeachValidationError::MissingNodeTarget),
        (None, Some(node_ref)) if node_ref.trim().is_empty() => {
            Err(TeachValidationError::MissingNodeTarget)
        }
        (_, Some(node_ref)) => validate_teach_node_ref(node_ref),
        _ => Ok(()),
    }
}

fn resolve_teach_node_target(
    node_id: Option<i64>,
    node_ref: Option<&str>,
    resolved_node_refs: &HashMap<String, i64>,
) -> Result<i64, TeachStorageError> {
    validate_teach_node_target(node_id, node_ref)?;
    match (node_id, node_ref) {
        (Some(node_id), None) => Ok(node_id),
        (None, Some(node_ref)) => resolved_node_refs
            .get(node_ref)
            .copied()
            .ok_or_else(|| TeachValidationError::UnknownNodeRef(node_ref.to_string()).into()),
        _ => Err(TeachValidationError::MissingNodeTarget.into()),
    }
}

fn validate_node_input(node: BorrowedNodeInput<'_>) -> Result<(), NodeValidationError> {
    if !ALLOWED_NODE_TYPES.contains(&node.node_type) {
        return Err(NodeValidationError::InvalidType(node.node_type.to_string()));
    }
    if !ALLOWED_NODE_STATUSES.contains(&node.status) {
        return Err(NodeValidationError::InvalidStatus(node.status.to_string()));
    }
    if node.title.trim().is_empty() {
        return Err(NodeValidationError::MissingTitle);
    }
    validate_node_field_size("title", node.title, MAX_NODE_TITLE_BYTES)?;
    validate_optional_node_field_size("summary", node.summary, MAX_NODE_SUMMARY_BYTES)?;
    validate_optional_node_field_size("body", node.body, MAX_NODE_BODY_BYTES)?;
    validate_optional_node_field_size("source_ref", node.source_ref, MAX_NODE_SOURCE_REF_BYTES)?;
    validate_optional_node_field_size("trust_level", node.trust_level, MAX_NODE_TRUST_LEVEL_BYTES)?;
    if node.status == "active" {
        if node.source_ref.unwrap_or("").trim().is_empty() {
            return Err(NodeValidationError::MissingActiveSourceRef);
        }
        if node.confidence.is_none() {
            return Err(NodeValidationError::MissingActiveConfidence);
        }
        if node.trust_level.unwrap_or("").trim().is_empty() {
            return Err(NodeValidationError::MissingActiveTrustLevel);
        }
    }

    Ok(())
}

fn validate_node_field_size(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), NodeValidationError> {
    if value.len() > max_bytes {
        return Err(NodeValidationError::FieldTooLong { field, max_bytes });
    }

    Ok(())
}

fn validate_optional_node_field_size(
    field: &'static str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), NodeValidationError> {
    if let Some(value) = value {
        validate_node_field_size(field, value, max_bytes)?;
    }

    Ok(())
}

fn validate_new_mcp_profile(profile: &NewMcpProfile) -> Result<(), McpProfileValidationError> {
    if profile.id.trim().is_empty() {
        return Err(McpProfileValidationError::MissingId);
    }
    if profile.name.trim().is_empty() {
        return Err(McpProfileValidationError::MissingName);
    }
    if profile.kind.trim().is_empty() {
        return Err(McpProfileValidationError::MissingKind);
    }
    if profile.status.trim().is_empty() {
        return Err(McpProfileValidationError::MissingStatus);
    }
    if profile.read_operations.trim().is_empty() {
        return Err(McpProfileValidationError::MissingReadOperations);
    }
    if profile.write_operations.trim().is_empty() {
        return Err(McpProfileValidationError::MissingWriteOperations);
    }
    if profile.side_effects.trim().is_empty() {
        return Err(McpProfileValidationError::MissingSideEffects);
    }
    if profile.approval_requirement.trim().is_empty() {
        return Err(McpProfileValidationError::MissingApprovalRequirement);
    }
    validate_mcp_field_size("id", &profile.id, MAX_MCP_ID_BYTES)?;
    validate_mcp_field_size("name", &profile.name, MAX_MCP_NAME_BYTES)?;
    validate_mcp_field_size("kind", &profile.kind, MAX_MCP_FIELD_BYTES)?;
    validate_mcp_field_size("status", &profile.status, MAX_MCP_FIELD_BYTES)?;
    validate_mcp_field_size(
        "read_operations",
        &profile.read_operations,
        MAX_MCP_FIELD_BYTES,
    )?;
    validate_mcp_field_size(
        "write_operations",
        &profile.write_operations,
        MAX_MCP_FIELD_BYTES,
    )?;
    validate_mcp_field_size("side_effects", &profile.side_effects, MAX_MCP_FIELD_BYTES)?;
    validate_mcp_field_size(
        "approval_requirement",
        &profile.approval_requirement,
        MAX_MCP_FIELD_BYTES,
    )?;
    validate_optional_mcp_field_size(
        "credentials_source",
        profile.credentials_source.as_deref(),
        MAX_MCP_FIELD_BYTES,
    )?;
    validate_optional_mcp_field_size("notes", profile.notes.as_deref(), MAX_MCP_NOTES_BYTES)?;

    Ok(())
}

fn validate_mcp_field_size(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), McpProfileValidationError> {
    if value.len() > max_bytes {
        return Err(McpProfileValidationError::FieldTooLong { field, max_bytes });
    }

    Ok(())
}

fn validate_optional_mcp_field_size(
    field: &'static str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), McpProfileValidationError> {
    if let Some(value) = value {
        validate_mcp_field_size(field, value, max_bytes)?;
    }

    Ok(())
}

fn validate_new_link(connection: &Connection, link: &NewLink) -> Result<(), LinkStorageError> {
    validate_new_link_input(link)?;
    if get_node(connection, link.source_node_id)?.is_none() {
        return Err(LinkValidationError::SourceNodeNotFound(link.source_node_id).into());
    }
    if get_node(connection, link.target_node_id)?.is_none() {
        return Err(LinkValidationError::TargetNodeNotFound(link.target_node_id).into());
    }

    Ok(())
}

enum MetadataKind {
    Alias,
    Tag,
    Source,
}

pub fn validate_new_alias_input(alias: &NewAlias) -> Result<(), MetadataValidationError> {
    validate_metadata_value(&alias.alias, MetadataKind::Alias)
}

pub fn validate_new_tag_input(tag: &NewTag) -> Result<(), MetadataValidationError> {
    validate_metadata_value(&tag.tag, MetadataKind::Tag)
}

pub fn validate_new_source_input(source: &NewSource) -> Result<(), MetadataValidationError> {
    validate_metadata_value(&source.source_ref, MetadataKind::Source)
}

fn validate_metadata_value(value: &str, kind: MetadataKind) -> Result<(), MetadataValidationError> {
    if value.trim().is_empty() {
        return Err(match kind {
            MetadataKind::Alias => MetadataValidationError::MissingAlias,
            MetadataKind::Tag => MetadataValidationError::MissingTag,
            MetadataKind::Source => MetadataValidationError::MissingSourceRef,
        });
    }
    if value.len() > MAX_METADATA_VALUE_BYTES {
        let kind = match kind {
            MetadataKind::Alias => "alias",
            MetadataKind::Tag => "tag",
            MetadataKind::Source => "source_ref",
        };
        return Err(MetadataValidationError::ValueTooLong {
            kind,
            max_bytes: MAX_METADATA_VALUE_BYTES,
        });
    }
    Ok(())
}

fn validate_node_metadata(
    connection: &Connection,
    node_id: i64,
    value: &str,
    kind: MetadataKind,
) -> Result<(), MetadataStorageError> {
    validate_metadata_value(value, kind)?;
    if get_node(connection, node_id)?.is_none() {
        return Err(MetadataValidationError::NodeNotFound(node_id).into());
    }

    Ok(())
}

fn refresh_fts_node(connection: &Connection, node_id: i64) -> rusqlite::Result<()> {
    let node = get_node(connection, node_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let aliases = aliases_for_fts(connection, node_id)?;

    connection
        .prepare_cached(DELETE_FTS_NODE_SQL)?
        .execute([node_id])?;
    connection
        .prepare_cached(INSERT_FTS_NODE_SQL)?
        .execute(params![
            node.id,
            node.title,
            node.summary.unwrap_or_default(),
            node.body.unwrap_or_default(),
            aliases
        ])?;

    Ok(())
}

pub(crate) fn refresh_fts_nodes(
    connection: &Connection,
    node_ids: &BTreeSet<i64>,
) -> rusqlite::Result<()> {
    for node_id in node_ids {
        refresh_fts_node(connection, *node_id)?;
    }

    Ok(())
}

fn aliases_for_fts(connection: &Connection, node_id: i64) -> rusqlite::Result<String> {
    let mut statement = connection.prepare_cached(FTS_ALIASES_SQL)?;
    statement.query_row([node_id], |row| row.get(0))
}

pub(crate) fn fts_match_query(query: &str) -> Option<String> {
    let terms = query
        .split_whitespace()
        .map(|term| term.trim_matches(|character: char| !character.is_alphanumeric()))
        .filter(|term| !term.is_empty())
        .take(8)
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" OR "))
    }
}

fn page_fetch_limit(limit: usize) -> rusqlite::Result<i64> {
    let limit = i64::try_from(limit).map_err(|_| invalid_page_limit())?;
    if !(1..i64::MAX).contains(&limit) {
        return Err(invalid_page_limit());
    }

    Ok(limit + 1)
}

fn invalid_page_limit() -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName("page limit must be positive and below i64::MAX".into())
}

fn make_page<T, Cursor>(
    mut items: Vec<T>,
    limit: usize,
    cursor: impl Fn(&T) -> Cursor,
) -> Page<T, Cursor> {
    let more_results = items.len() > limit;
    items.truncate(limit);
    make_page_with_more_results(items, more_results, cursor)
}

fn make_page_with_more_results<T, Cursor>(
    items: Vec<T>,
    more_results: bool,
    cursor: impl Fn(&T) -> Cursor,
) -> Page<T, Cursor> {
    let next_after_id = if more_results {
        items.last().map(cursor)
    } else {
        None
    };

    Page {
        items,
        next_after_id,
        more_results,
    }
}

fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
    row_to_node_at(row, 0)
}

fn row_to_node_at(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<Node> {
    Ok(Node {
        id: row.get(offset)?,
        node_type: row.get(offset + 1)?,
        status: row.get(offset + 2)?,
        title: row.get(offset + 3)?,
        summary: row.get(offset + 4)?,
        body: row.get(offset + 5)?,
        source_ref: row.get(offset + 6)?,
        confidence: row.get(offset + 7)?,
        trust_level: row.get(offset + 8)?,
        created_at: row.get(offset + 9)?,
        updated_at: row.get(offset + 10)?,
    })
}

fn get_link(connection: &Connection, id: i64) -> rusqlite::Result<Option<Link>> {
    let mut statement = connection.prepare_cached(GET_LINK_SQL)?;
    statement.query_row([id], row_to_link).optional()
}

fn row_to_link(row: &rusqlite::Row<'_>) -> rusqlite::Result<Link> {
    Ok(Link {
        id: row.get(0)?,
        source_node_id: row.get(1)?,
        target_node_id: row.get(2)?,
        link_type: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn get_alias(connection: &Connection, id: i64) -> rusqlite::Result<Option<Alias>> {
    let mut statement = connection.prepare_cached(GET_ALIAS_SQL)?;
    statement.query_row([id], row_to_alias).optional()
}

fn row_to_alias(row: &rusqlite::Row<'_>) -> rusqlite::Result<Alias> {
    Ok(Alias {
        id: row.get(0)?,
        node_id: row.get(1)?,
        alias: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn get_tag(connection: &Connection, id: i64) -> rusqlite::Result<Option<Tag>> {
    let mut statement = connection.prepare_cached(GET_TAG_SQL)?;
    statement.query_row([id], row_to_tag).optional()
}

fn row_to_tag(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        node_id: row.get(1)?,
        tag: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn get_source(connection: &Connection, id: i64) -> rusqlite::Result<Option<Source>> {
    let mut statement = connection.prepare_cached(GET_SOURCE_SQL)?;
    statement.query_row([id], row_to_source).optional()
}

fn row_to_source(row: &rusqlite::Row<'_>) -> rusqlite::Result<Source> {
    Ok(Source {
        id: row.get(0)?,
        node_id: row.get(1)?,
        source_ref: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn row_to_mcp_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<McpProfile> {
    Ok(McpProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        status: row.get(3)?,
        read_operations: row.get(4)?,
        write_operations: row.get(5)?,
        side_effects: row.get(6)?,
        approval_requirement: row.get(7)?,
        credentials_source: row.get(8)?,
        notes: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn parse_source_hierarchy(source_ref: &str) -> Option<SourceHierarchy> {
    let source_value = source_ref
        .split(',')
        .find_map(|segment| {
            let (key, value) = segment.split_once('=')?;
            (key.trim() == "source").then_some(value.trim())
        })
        .or_else(|| {
            (!source_ref.trim().is_empty() && !source_ref.contains('='))
                .then_some(source_ref.trim())
        })?;

    let source_path: Vec<String> = source_value
        .split(['/', ':', '.'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let source_root = source_path.first()?.clone();
    let source_leaf = source_path.last()?.clone();

    Some(SourceHierarchy {
        priority: source_priority_for_root(&source_root),
        source_root,
        source_leaf,
        source_path,
    })
}

fn parse_node_least_privilege_metadata(
    node_type: &str,
    body: &str,
) -> Option<LeastPrivilegeMetadata> {
    if !matches!(node_type, "tool_contract" | "mcp_profile") {
        return None;
    }

    let parsed: LeastPrivilegeNodeBody = serde_json::from_str(body).ok()?;
    if parsed.side_effects.trim().is_empty() || parsed.approval_requirement.trim().is_empty() {
        return None;
    }

    Some(LeastPrivilegeMetadata {
        privilege_rank: side_effects_rank(&parsed.side_effects),
        side_effects: parsed.side_effects,
        approval_requirement: parsed.approval_requirement,
        read_operations: normalize_operations(parsed.read_operations),
        write_operations: normalize_operations(parsed.write_operations),
    })
}

fn split_operations(value: &str) -> Vec<String> {
    normalize_operations(value.split(',').map(str::to_owned).collect())
}

fn normalize_operations(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn source_priority_for_root(source_root: &str) -> u8 {
    match source_root {
        "user_instruction" => 0,
        "reflection" => 1,
        "teach" => 2,
        "tool" | "tool_contract" => 3,
        "mcp" | "codebase_memory" | "understand_anything" => 4,
        "artifact" | "audit" => 5,
        "external" => 6,
        _ => 7,
    }
}

fn side_effects_rank(side_effects: &str) -> u8 {
    match side_effects {
        "none" => 0,
        "local_read" => 1,
        "local_write_artifact" => 2,
        "local_write_memory" => 3,
        "external_read" => 4,
        "external_write" => 5,
        "destructive" => 6,
        _ => 7,
    }
}

fn resolve_paths_from_env_for_platform(
    aopmem_home: Option<&OsStr>,
    home: Option<&OsStr>,
    userprofile: Option<&OsStr>,
    windows: bool,
) -> Result<AopmemPaths, PathResolveError> {
    let root = match aopmem_home {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ if windows => userprofile_home(userprofile)?,
        _ => home_aopmem_home(home)?,
    };

    Ok(AopmemPaths {
        bin: root.join("bin"),
        skills: root.join("skills"),
        templates: root.join("templates"),
        workspaces: root.join("workspaces"),
        home: root,
    })
}

fn home_aopmem_home(home: Option<&OsStr>) -> Result<PathBuf, PathResolveError> {
    let home = home.ok_or(PathResolveError::MissingHome)?;
    if home.is_empty() {
        return Err(PathResolveError::MissingHome);
    }

    Ok(PathBuf::from(home).join(".aopmem"))
}

fn userprofile_home(userprofile: Option<&OsStr>) -> Result<PathBuf, PathResolveError> {
    let userprofile = userprofile.ok_or(PathResolveError::MissingUserProfile)?;
    if userprofile.is_empty() {
        return Err(PathResolveError::MissingUserProfile);
    }

    Ok(PathBuf::from(userprofile).join(".aopmem"))
}

fn workspace_paths(paths: &AopmemPaths, workspace_key: &str) -> WorkspacePaths {
    let root = paths.workspaces().join(workspace_key);
    let observability = root.join("observability");

    WorkspacePaths {
        db: root.join("aopmem.sqlite"),
        observability_db: observability.join("observability.sqlite"),
        observability,
        tools: root.join("tools"),
        artifacts: root.join("artifacts"),
        audit_git: root.join("audit-git"),
        runtimes: root.join("runtimes"),
        logs: root.join("logs"),
        root,
    }
}

fn apply_connection_pragmas(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA busy_timeout = 5000;
        ",
    )
}

pub fn prepare_task_recall_connection(connection: &Connection) -> rusqlite::Result<()> {
    connection.create_scalar_function(
        "aopmem_source_priority",
        1,
        FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_UTF8,
        |context| {
            let source_ref = context.get::<Option<String>>(0)?;
            Ok(source_ref
                .as_deref()
                .and_then(parse_source_hierarchy)
                .map_or(i64::from(u8::MAX), |hierarchy| {
                    i64::from(hierarchy.priority)
                }))
        },
    )
}

fn sanitize_repo_folder_name(folder_name: &str) -> String {
    let mut sanitized = String::new();
    let mut previous_was_separator = false;

    for character in folder_name.chars() {
        if character.is_ascii_alphanumeric() {
            sanitized.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator && !sanitized.is_empty() {
            sanitized.push('-');
            previous_was_separator = true;
        }
    }

    while sanitized.ends_with('-') {
        sanitized.pop();
    }

    if sanitized.is_empty() {
        "workspace".to_string()
    } else {
        sanitized
    }
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut cursor = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if cursor.join(".git").exists() {
            return Some(cursor);
        }
        if !cursor.pop() {
            return None;
        }
    }
}

fn canonicalize_existing_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn legacy_workspace_path_text(repo_root: &Path) -> String {
    let raw = repo_root.as_os_str().to_string_lossy();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        raw.into_owned()
    }
}

fn legacy_workspace_key_from_text(path: &str) -> Result<String, WorkspaceKeyError> {
    if !is_absolute_legacy_workspace_path(path) {
        return Err(WorkspaceKeyError::RelativeRepoRoot);
    }

    let folder_name = path
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .ok_or(WorkspaceKeyError::MissingRepoFolderName)?;
    if folder_name.ends_with(':') {
        return Err(WorkspaceKeyError::MissingRepoFolderName);
    }

    let sanitized = sanitize_repo_folder_name(folder_name);
    let path_hash = hash_path_text(path);
    Ok(format!("{sanitized}-{path_hash:08x}"))
}

fn is_absolute_legacy_workspace_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with(r"\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
}

fn workspace_root_has_persistent_data(root: &Path) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !metadata.is_dir() {
        return Ok(true);
    }

    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn normalize_workspace_path_for_key(repo_root: &Path) -> String {
    let mut normalized = repo_root.as_os_str().to_string_lossy().replace('\\', "/");

    if let Some(rest) = normalized.strip_prefix("//?/UNC/") {
        normalized = format!("//{rest}");
    } else if let Some(rest) = normalized.strip_prefix("//?/") {
        normalized = rest.to_string();
    }

    while should_trim_trailing_slash(&normalized) {
        normalized.pop();
    }

    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        let drive = normalized[..1].to_ascii_lowercase();
        normalized.replace_range(0..1, &drive);
    }

    normalized
}

fn should_trim_trailing_slash(path: &str) -> bool {
    path.len() > 1 && path.ends_with('/') && !is_windows_drive_root(path)
}

fn is_windows_drive_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

fn is_absolute_workspace_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'/')
}

fn workspace_folder_name(normalized_path: &str) -> Option<&str> {
    let folder = normalized_path
        .rsplit('/')
        .find(|segment| !segment.is_empty())?;
    if folder.ends_with(':') {
        None
    } else {
        Some(folder)
    }
}

fn hash_normalized_path(normalized_path: &str) -> u32 {
    hash_path_text(normalized_path)
}

fn hash_path_text(path: &str) -> u32 {
    let mut hash = FNV1A_32_OFFSET;
    for byte in path.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(FNV1A_32_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{list_events, LINK_CREATED_EVENT, NODE_CREATED_EVENT};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        env::temp_dir().join(format!("aopmem-stage-006-{name}-{nanos}"))
    }

    struct EnvGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &PathBuf) -> Self {
            let original = env::var_os(key);
            env::set_var(key, value);
            Self { key, original }
        }

        fn remove(key: &'static str) -> Self {
            let original = env::var_os(key);
            env::remove_var(key);
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn borrowed_teach_records_preserve_exact_json() {
        let owned_session = TeachSessionRecord {
            session_title: "Teach session".to_string(),
            session_summary: Some("Summary".to_string()),
        };
        let borrowed_session = TeachSessionRecordRef {
            session_title: "Teach session",
            session_summary: Some("Summary"),
        };
        assert_eq!(
            serde_json::to_vec(&borrowed_session).expect("borrowed session should serialize"),
            serde_json::to_vec(&owned_session).expect("owned session should serialize")
        );

        let payload = serde_json::json!({"text": "material", "weight": 2});
        let owned_material = TeachMaterialRecord {
            session_id: 3,
            payload: payload.clone(),
        };
        let borrowed_material = TeachMaterialRecordRef {
            session_id: 3,
            payload: &payload,
        };
        assert_eq!(
            serde_json::to_vec(&borrowed_material).expect("borrowed material should serialize"),
            serde_json::to_vec(&owned_material).expect("owned material should serialize")
        );

        let items = vec![TeachProposalItem::CreateNode {
            node_ref: Some("lesson".to_string()),
            node_type: "lesson".to_string(),
            status: "draft".to_string(),
            title: "Learned lesson".to_string(),
            summary: None,
            body: Some("Body".to_string()),
            source_ref: None,
            confidence: Some(0.8),
            trust_level: Some("verified".to_string()),
        }];
        let owned_proposal = TeachProposalRecord {
            session_id: 3,
            items: items.clone(),
        };
        let borrowed_proposal = TeachProposalRecordRef {
            session_id: 3,
            items: &items,
        };
        assert_eq!(
            serde_json::to_vec(&borrowed_proposal).expect("borrowed proposal should serialize"),
            serde_json::to_vec(&owned_proposal).expect("owned proposal should serialize")
        );

        let created_node_ids = vec![10, 11];
        let created_alias_ids = vec![20];
        let created_tag_ids = vec![30];
        let created_source_ids = vec![40];
        let created_link_ids = vec![50];
        let owned_receipt = TeachApplyReceiptRecord {
            session_id: 3,
            proposal_id: 4,
            created_node_ids: created_node_ids.clone(),
            created_alias_ids: created_alias_ids.clone(),
            created_tag_ids: created_tag_ids.clone(),
            created_source_ids: created_source_ids.clone(),
            created_link_ids: created_link_ids.clone(),
        };
        let borrowed_receipt = TeachApplyReceiptRecordRef {
            session_id: 3,
            proposal_id: 4,
            created_node_ids: &created_node_ids,
            created_alias_ids: &created_alias_ids,
            created_tag_ids: &created_tag_ids,
            created_source_ids: &created_source_ids,
            created_link_ids: &created_link_ids,
        };
        assert_eq!(
            serde_json::to_vec(&borrowed_receipt).expect("borrowed receipt should serialize"),
            serde_json::to_vec(&owned_receipt).expect("owned receipt should serialize")
        );
    }

    #[test]
    fn resolves_aopmem_home_override_without_creating_paths() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("override");
        let regular_home = temp_path("home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &regular_home);

        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");

        assert_eq!(paths.home(), &override_home);
        assert_eq!(paths.bin(), &override_home.join("bin"));
        assert_eq!(paths.skills(), &override_home.join("skills"));
        assert_eq!(paths.templates(), &override_home.join("templates"));
        assert_eq!(paths.workspaces(), &override_home.join("workspaces"));
        assert!(!override_home.exists());
        assert!(!regular_home.exists());
    }

    #[test]
    fn defaults_to_dot_aopmem_under_home_without_creating_paths() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let home = temp_path("default-home");
        let _aopmem_home = EnvGuard::remove(AOPMEM_HOME_ENV);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let expected = home.join(".aopmem");

        let paths = resolve_paths().expect("HOME should resolve default AOPMem home");

        assert_eq!(paths.home(), &expected);
        assert_eq!(paths.bin(), &expected.join("bin"));
        assert_eq!(paths.skills(), &expected.join("skills"));
        assert_eq!(paths.templates(), &expected.join("templates"));
        assert_eq!(paths.workspaces(), &expected.join("workspaces"));
        assert!(!home.exists());
        assert!(!expected.exists());
    }

    #[test]
    fn requires_home_when_override_is_missing() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let _aopmem_home = EnvGuard::remove(AOPMEM_HOME_ENV);
        let _home = EnvGuard::remove(HOME_ENV);

        let error = resolve_paths().expect_err("missing HOME should fail");

        assert_eq!(error, PathResolveError::MissingHome);
    }

    #[test]
    fn windows_resolver_prefers_aopmem_home_override() {
        let override_home = temp_path("windows-override");
        let userprofile = temp_path("windows-userprofile");

        let paths = resolve_paths_from_env_for_platform(
            Some(override_home.as_os_str()),
            None,
            Some(userprofile.as_os_str()),
            true,
        )
        .expect("AOPMEM_HOME should win on Windows");

        assert_eq!(paths.home(), &override_home);
        assert_eq!(paths.workspaces(), &override_home.join("workspaces"));
    }

    #[test]
    fn windows_resolver_uses_userprofile_without_home() {
        let userprofile = temp_path("windows-userprofile-default");
        let expected = userprofile.join(".aopmem");

        let paths =
            resolve_paths_from_env_for_platform(None, None, Some(userprofile.as_os_str()), true)
                .expect("USERPROFILE should resolve Windows AOPMem home");

        assert_eq!(paths.home(), &expected);
        assert_eq!(paths.bin(), &expected.join("bin"));
        assert_eq!(paths.workspaces(), &expected.join("workspaces"));
        assert!(!expected.exists());
    }

    #[test]
    fn windows_resolver_does_not_require_home() {
        let userprofile = temp_path("windows-no-home-required");

        let paths = resolve_paths_from_env_for_platform(
            None,
            Some(OsStr::new("")),
            Some(userprofile.as_os_str()),
            true,
        )
        .expect("empty HOME should not matter on Windows");

        assert_eq!(paths.home(), &userprofile.join(".aopmem"));
    }

    #[test]
    fn windows_resolver_requires_userprofile_without_override() {
        let error = resolve_paths_from_env_for_platform(None, None, None, true)
            .expect_err("missing USERPROFILE should fail on Windows");

        assert_eq!(error, PathResolveError::MissingUserProfile);
    }

    #[test]
    fn workspace_key_uses_sanitized_repo_folder_and_absolute_path_hash() {
        let key = workspace_key("/Users/alice/Code/My Repo!")
            .expect("absolute repo root should produce a workspace key");

        assert_eq!(key, "my-repo-264e3f9d");
    }

    #[test]
    fn workspace_key_is_deterministic_for_same_absolute_path() {
        let first = workspace_key("/Users/alice/Code/aopmem-cli")
            .expect("absolute repo root should produce a workspace key");
        let second = workspace_key("/Users/alice/Code/aopmem-cli")
            .expect("absolute repo root should produce a workspace key");

        assert_eq!(first, second);
        assert_eq!(first, "aopmem-cli-7d9f780e");
    }

    #[test]
    fn workspace_key_normalizes_windows_separators_and_trailing_slash() {
        let backslash = workspace_key(r"C:\Users\alice\Code\aopmem-cli")
            .expect("Windows absolute path should produce a workspace key");
        let slash = workspace_key("C:/Users/alice/Code/aopmem-cli/")
            .expect("Windows absolute path should produce a workspace key");
        let verbatim = workspace_key(r"\\?\C:\Users\alice\Code\aopmem-cli\")
            .expect("Windows verbatim path should produce a workspace key");

        assert_eq!(backslash, slash);
        assert_eq!(slash, verbatim);
    }

    #[test]
    fn workspace_key_normalizes_windows_drive_letter_case() {
        let upper = workspace_key("C:/Users/alice/Code/aopmem-cli")
            .expect("Windows absolute path should produce a workspace key");
        let lower = workspace_key("c:/Users/alice/Code/aopmem-cli")
            .expect("Windows absolute path should produce a workspace key");

        assert_eq!(upper, lower);
    }

    fn workspace_test_paths(root: &Path) -> AopmemPaths {
        AopmemPaths {
            home: root.to_path_buf(),
            bin: root.join("bin"),
            skills: root.join("skills"),
            templates: root.join("templates"),
            workspaces: root.join("workspaces"),
        }
    }

    #[test]
    fn legacy_windows_workspace_key_preserves_v010_path_text() {
        let path = r"C:\Users\alice\Code\aopmem-cli";
        let legacy = legacy_workspace_key(path).expect("legacy key should resolve");
        let current = workspace_key(path).expect("current key should resolve");

        assert_ne!(legacy, current);
        assert_eq!(legacy, "aopmem-cli-1cde3e0d");
    }

    #[test]
    fn workspace_resolver_selects_only_persistent_root() {
        let root = temp_path("workspace-resolver-only-root");
        let paths = workspace_test_paths(&root);
        let repo = Path::new(r"C:\Users\alice\Code\aopmem-cli");
        let current = workspace_key(repo).expect("current key should resolve");
        let legacy = legacy_workspace_key(repo).expect("legacy key should resolve");

        fs::create_dir_all(paths.workspaces().join(&legacy)).expect("legacy root should create");
        fs::write(
            paths.workspaces().join(&legacy).join("aopmem.sqlite"),
            b"legacy",
        )
        .expect("legacy marker should write");
        assert_eq!(
            resolve_workspace_key(&paths, repo).expect("legacy root should resolve"),
            legacy
        );

        fs::remove_dir_all(paths.workspaces().join(&legacy)).expect("legacy root should remove");
        fs::create_dir_all(paths.workspaces().join(&current)).expect("current root should create");
        fs::write(
            paths.workspaces().join(&current).join("aopmem.sqlite"),
            b"current",
        )
        .expect("current marker should write");
        assert_eq!(
            resolve_workspace_key(&paths, repo).expect("current root should resolve"),
            current
        );

        fs::remove_dir_all(root).expect("resolver fixture should remove");
    }

    #[test]
    fn workspace_resolver_ignores_empty_skeleton_and_blocks_two_data_roots() {
        let root = temp_path("workspace-resolver-collision");
        let paths = workspace_test_paths(&root);
        let repo = Path::new(r"C:\Users\alice\Code\aopmem-cli");
        let current = workspace_key(repo).expect("current key should resolve");
        let legacy = legacy_workspace_key(repo).expect("legacy key should resolve");
        let current_root = paths.workspaces().join(&current);
        let legacy_root = paths.workspaces().join(&legacy);

        fs::create_dir_all(current_root.join("tools")).expect("empty skeleton should create");
        fs::create_dir_all(&legacy_root).expect("legacy root should create");
        fs::write(legacy_root.join("aopmem.sqlite"), b"legacy")
            .expect("legacy marker should write");
        assert_eq!(
            resolve_workspace_key(&paths, repo).expect("data root should win"),
            legacy
        );

        fs::write(current_root.join("aopmem.sqlite"), b"current")
            .expect("current marker should write");
        assert!(matches!(
            resolve_workspace_key(&paths, repo),
            Err(WorkspaceResolveError::Ambiguous {
                current_key,
                legacy_key,
                ..
            }) if current_key == current && legacy_key == legacy
        ));

        fs::remove_dir_all(root).expect("resolver fixture should remove");
    }

    #[test]
    fn workspace_resolver_does_not_create_missing_home() {
        let root = temp_path("workspace-resolver-no-write");
        let paths = workspace_test_paths(&root);
        let repo = Path::new(r"C:\Users\alice\Code\aopmem-cli");

        let resolved = resolve_workspace_key(&paths, repo).expect("new key should resolve");

        assert_eq!(
            resolved,
            workspace_key(repo).expect("current key should resolve")
        );
        assert!(!root.exists());
    }

    #[test]
    fn workspace_key_hash_changes_when_absolute_path_changes() {
        let first = workspace_key("/Users/alice/Code/aopmem-cli")
            .expect("absolute repo root should produce a workspace key");
        let second = workspace_key("/Users/bob/Code/aopmem-cli")
            .expect("absolute repo root should produce a workspace key");

        assert_ne!(first, second);
        assert!(first.starts_with("aopmem-cli-"));
        assert!(second.starts_with("aopmem-cli-"));
    }

    #[test]
    fn workspace_key_differs_for_different_paths_with_same_basename() {
        let first = workspace_key("C:/Users/alice/Code/aopmem-cli")
            .expect("Windows absolute path should produce a workspace key");
        let second = workspace_key("C:/Users/bob/Code/aopmem-cli")
            .expect("Windows absolute path should produce a workspace key");

        assert_ne!(first, second);
        assert!(first.starts_with("aopmem-cli-"));
        assert!(second.starts_with("aopmem-cli-"));
    }

    #[test]
    fn workspace_key_uses_fallback_when_folder_name_has_no_ascii_slug() {
        let key = workspace_key("/Users/alice/Code/Привет")
            .expect("absolute repo root should produce a workspace key");

        assert_eq!(key, "workspace-59dde0a2");
    }

    #[test]
    fn workspace_key_rejects_relative_repo_root() {
        let error = workspace_key("relative/repo")
            .expect_err("relative repo root should not produce a workspace key");

        assert_eq!(error, WorkspaceKeyError::RelativeRepoRoot);
    }

    #[test]
    fn workspace_root_prefers_git_root_from_subdirectory() {
        let repo_root = temp_path("git-root");
        let nested = repo_root.join("src").join("bin");
        fs::create_dir_all(repo_root.join(".git")).expect("git dir should be created");
        fs::create_dir_all(&nested).expect("nested dir should be created");

        let resolved = resolve_workspace_root_from(&nested).expect("workspace root should resolve");

        assert_eq!(
            resolved,
            repo_root
                .canonicalize()
                .expect("repo root should canonicalize")
        );

        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn workspace_root_uses_current_directory_without_git_root() {
        let workspace = temp_path("no-git-root");
        fs::create_dir_all(&workspace).expect("workspace should be created");

        let resolved =
            resolve_workspace_root_from(&workspace).expect("workspace root should resolve");

        assert_eq!(
            resolved,
            workspace
                .canonicalize()
                .expect("workspace should canonicalize")
        );

        fs::remove_dir_all(&workspace).expect("temp workspace should be removed");
    }

    #[test]
    fn ensure_global_dirs_creates_expected_user_level_structure() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("global-dirs");
        let repo_local_aopmem = env::current_dir()
            .expect("current dir should resolve")
            .join(".aopmem");
        let repo_local_existed_before = repo_local_aopmem.exists();
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);

        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");

        ensure_global_dirs(&paths).expect("global dirs should be created");

        assert!(paths.home().is_dir());
        assert!(paths.bin().is_dir());
        assert!(paths.skills().is_dir());
        assert!(paths.templates().is_dir());
        assert!(paths.workspaces().is_dir());
        assert_eq!(repo_local_aopmem.exists(), repo_local_existed_before);

        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn ensure_workspace_dirs_creates_expected_workspace_structure() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("workspace-dirs");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");
        let key = workspace_key("/Users/alice/Code/aopmem-cli")
            .expect("absolute repo root should produce a workspace key");

        ensure_global_dirs(&paths).expect("global dirs should be created");
        let workspace_paths =
            ensure_workspace_dirs(&paths, &key).expect("workspace dirs should be created");

        assert_eq!(workspace_paths.root(), &paths.workspaces().join(&key));
        assert_eq!(
            workspace_paths.db(),
            &workspace_paths.root().join("aopmem.sqlite")
        );
        assert_eq!(
            workspace_paths.observability(),
            &workspace_paths.root().join("observability")
        );
        assert_eq!(
            workspace_paths.observability_db(),
            &workspace_paths
                .root()
                .join("observability")
                .join("observability.sqlite")
        );
        assert!(workspace_paths.root().is_dir());
        assert!(workspace_paths.tools().is_dir());
        assert!(workspace_paths.artifacts().is_dir());
        assert!(workspace_paths.audit_git().is_dir());
        assert!(workspace_paths.runtimes().is_dir());
        assert!(workspace_paths.logs().is_dir());
        assert!(!workspace_paths.root().join("aopmem.sqlite").exists());
        assert!(!workspace_paths.observability().exists());

        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn ensure_workspace_dirs_is_idempotent() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("idempotent-dirs");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");
        let key = "aopmem-cli-7d9f780e";

        let first = ensure_workspace_dirs(&paths, key).expect("first create should pass");
        let second = ensure_workspace_dirs(&paths, key).expect("second create should pass");

        assert_eq!(first, second);
        assert!(second.root().is_dir());
        assert!(!second.root().join("aopmem.sqlite").exists());

        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn open_workspace_db_creates_db_and_applies_pragmas() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("sqlite-pragmas");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");
        let key = "aopmem-cli-7d9f780e";
        let workspace_paths =
            ensure_workspace_dirs(&paths, key).expect("workspace dirs should be created");

        let connection =
            open_workspace_db(&workspace_paths).expect("workspace DB should open with pragmas");

        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .expect("foreign_keys pragma should be readable");
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .expect("journal_mode pragma should be readable");
        let busy_timeout: i64 = connection
            .query_row("PRAGMA busy_timeout;", [], |row| row.get(0))
            .expect("busy_timeout pragma should be readable");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations;", [], |row| {
                row.get(0)
            })
            .expect("schema_migrations should be readable");

        assert_eq!(
            workspace_paths.db(),
            &workspace_paths.root().join("aopmem.sqlite")
        );
        assert!(workspace_paths.db().is_file());
        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode, "wal");
        assert_eq!(busy_timeout, 5000);
        assert_eq!(migration_count, 3);

        drop(connection);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn read_only_open_requires_existing_db_without_creating_workspace_paths() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("readonly-missing-db");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = workspace_paths_for_key(&paths, "aopmem-cli-7d9f780e");

        let error = open_workspace_db_read_only(&workspace_paths)
            .expect_err("read-only open should reject a missing database");

        assert!(matches!(
            error,
            OpenWorkspaceReadOnlyError::Missing(path) if path == *workspace_paths.db()
        ));
        assert!(!override_home.exists());
        assert!(!workspace_paths.root().exists());
        assert!(!workspace_paths.db().exists());
    }

    #[cfg(unix)]
    #[test]
    fn read_only_open_rejects_linked_database_without_reading_or_writing_outside() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("readonly-linked-db");
        let outside_db = temp_path("readonly-linked-db-outside.sqlite");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");
        ensure_global_dirs(&paths).expect("global dirs should create");
        let workspace_paths = ensure_workspace_dirs(&paths, "aopmem-cli-readonly-linked")
            .expect("workspace dirs should create");
        let writable = open_workspace_db(&workspace_paths).expect("workspace DB should initialize");
        drop(writable);
        fs::rename(workspace_paths.db(), &outside_db).expect("DB should move outside");
        symlink(&outside_db, workspace_paths.db()).expect("DB symlink should create");
        let outside_before = fs::read(&outside_db).expect("outside DB should read");

        let error = open_workspace_db_read_only(&workspace_paths)
            .expect_err("read-only open must reject a linked database");

        assert!(matches!(error, OpenWorkspaceReadOnlyError::UnsafePath(_)));
        assert_eq!(
            fs::read(&outside_db).expect("outside DB should remain readable"),
            outside_before
        );
        fs::remove_file(workspace_paths.db()).expect("DB symlink should remove");
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_file(outside_db).expect("outside DB should remove");
    }

    #[test]
    fn read_only_open_rejects_insert_on_existing_database() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("readonly-existing-db");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = ensure_workspace_dirs(&paths, "aopmem-cli-readonly")
            .expect("workspace dirs should be created");
        let writable = open_workspace_db(&workspace_paths).expect("workspace DB should initialize");
        drop(writable);

        let read_only = open_workspace_db_read_only(&workspace_paths)
            .expect("existing DB should open read-only");
        let error = read_only
            .execute(
                "INSERT INTO nodes (node_type, status, title) VALUES ('raw_note', 'draft', 'blocked');",
                [],
            )
            .expect_err("read-only connection must reject inserts");
        let node_count: i64 = read_only
            .query_row("SELECT COUNT(*) FROM nodes;", [], |row| row.get(0))
            .expect("read-only query should still work");

        assert!(matches!(
            error,
            rusqlite::Error::SqliteFailure(details, _)
                if details.code == rusqlite::ErrorCode::ReadOnly
        ));
        assert_eq!(node_count, 0);

        drop(read_only);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn open_workspace_db_applies_migrations_idempotently() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("sqlite-migrations");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = ensure_workspace_dirs(&paths, "aopmem-cli-7d9f780e")
            .expect("workspace dirs should be created");

        let first = open_workspace_db(&workspace_paths).expect("first DB open should migrate");
        drop(first);
        let second = open_workspace_db(&workspace_paths).expect("second DB open should migrate");

        let migration_count: i64 = second
            .query_row("SELECT COUNT(*) FROM schema_migrations;", [], |row| {
                row.get(0)
            })
            .expect("schema_migrations should be readable");

        assert_eq!(migration_count, 3);

        drop(second);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn create_get_and_list_nodes() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = NewNode {
            node_type: "decision".to_string(),
            status: "active".to_string(),
            title: "Use SQLite nodes".to_string(),
            summary: Some("Store cards as rows".to_string()),
            body: Some("Node memory is canonical SQLite data.".to_string()),
            source_ref: Some("source=user_instruction".to_string()),
            confidence: Some(0.9),
            trust_level: Some("high".to_string()),
        };

        let created = create_node(&connection, &input).expect("valid node should be created");
        let fetched = get_node(&connection, created.id)
            .expect("node get query should pass")
            .expect("created node should exist");
        let listed = list_nodes(&connection).expect("node list query should pass");

        assert_eq!(created.node_type, "decision");
        assert_eq!(fetched.id, created.id);
        assert_eq!(listed, vec![created]);
    }

    #[test]
    fn list_nodes_page_uses_keyset_cursor_and_can_omit_bodies() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node page test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let large_body = "x".repeat(64 * 1024 + 1);

        let first = create_node(
            &connection,
            &NewNode {
                body: Some("first body".to_string()),
                ..draft_node("First")
            },
        )
        .expect("first node should create");
        let second = create_node(
            &connection,
            &NewNode {
                body: Some("second body".to_string()),
                ..draft_node("Second")
            },
        )
        .expect("second node should create");
        let third = create_node(
            &connection,
            &NewNode {
                body: Some(large_body.clone()),
                ..draft_node("Third")
            },
        )
        .expect("third node should create");

        let first_page =
            list_nodes_page(&connection, None, 2, false).expect("first node page should list");

        assert_eq!(
            first_page
                .page
                .items
                .iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![first.id, second.id]
        );
        assert!(first_page.page.more_results);
        assert_eq!(first_page.page.next_after_id, Some(second.id));
        assert!(first_page.body_omitted);
        assert!(!first_page.content_truncated);
        assert!(first_page.page.items.iter().all(|node| node.body.is_none()));

        let second_page = list_nodes_page(&connection, first_page.page.next_after_id, 2, true)
            .expect("second node page should list");

        assert_eq!(second_page.page.items[0].id, third.id);
        assert!(!second_page.page.more_results);
        assert_eq!(second_page.page.next_after_id, None);
        assert!(!second_page.body_omitted);
        assert!(!second_page.content_truncated);
        assert_eq!(
            second_page.page.items[0].body.as_deref(),
            Some(large_body.as_str())
        );
        assert_eq!(
            get_node(&connection, third.id)
                .expect("node get should query")
                .expect("third node should exist")
                .body
                .as_deref(),
            Some(large_body.as_str())
        );

        let error = list_nodes_page(&connection, None, 0, false)
            .expect_err("zero page limit should be rejected");
        assert!(matches!(error, rusqlite::Error::InvalidParameterName(_)));
    }

    #[test]
    fn create_node_indexes_title_summary_and_body_in_fts() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node FTS test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = NewNode {
            node_type: "decision".to_string(),
            status: "active".to_string(),
            title: "FTS stage marker".to_string(),
            summary: Some("Searchable summary token".to_string()),
            body: Some("Searchable body token".to_string()),
            source_ref: Some("source=user_instruction".to_string()),
            confidence: Some(0.9),
            trust_level: Some("high".to_string()),
        };

        let created = create_node(&connection, &input).expect("valid node should be created");
        let title_match = count_fts_matches(&connection, "marker");
        let summary_match = count_fts_matches(&connection, "summary");
        let body_match = count_fts_matches(&connection, "body");

        assert_eq!(created.id, 1);
        assert_eq!(title_match, 1);
        assert_eq!(summary_match, 1);
        assert_eq!(body_match, 1);
    }

    #[test]
    fn create_alias_updates_node_aliases_in_fts() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for alias FTS test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let node =
            create_node(&connection, &draft_node("Memory node")).expect("node should be created");

        assert_eq!(count_fts_matches(&connection, "nickname"), 0);

        create_alias(
            &connection,
            &NewAlias {
                node_id: node.id,
                alias: "nickname".to_string(),
            },
        )
        .expect("alias should be created");

        assert_eq!(count_fts_matches(&connection, "nickname"), 1);
    }

    #[test]
    fn aliases_for_fts_follow_stable_id_order() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for alias order test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let node =
            create_node(&connection, &draft_node("Ordered aliases")).expect("node should create");
        connection
            .execute(
                "
                INSERT INTO aliases (id, node_id, alias)
                VALUES (20, ?1, 'second'), (10, ?1, 'first');
                ",
                [node.id],
            )
            .expect("out-of-order aliases should insert");

        refresh_fts_node(&connection, node.id).expect("FTS should refresh");
        let aliases: String = connection
            .query_row(
                "SELECT aliases FROM fts_nodes WHERE rowid = ?1;",
                [node.id],
                |row| row.get(0),
            )
            .expect("FTS aliases should read");

        assert_eq!(aliases, "first second");
    }

    #[test]
    fn batch_fts_refresh_stops_at_missing_node_after_preserving_prior_refresh() {
        let mut connection = Connection::open_in_memory()
            .expect("in-memory DB should open for batch FTS boundary test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let node = create_node(&connection, &draft_node("Before batch refresh"))
            .expect("node should create");
        connection
            .execute(
                "UPDATE nodes SET title = 'After batch refresh' WHERE id = ?1",
                [node.id],
            )
            .expect("node title should change without refreshing FTS");
        let missing_id = node.id + 1;
        let node_ids = BTreeSet::from([node.id, missing_id]);

        let error = refresh_fts_nodes(&connection, &node_ids)
            .expect_err("missing second node should stop the batch");

        assert!(matches!(error, rusqlite::Error::QueryReturnedNoRows));
        assert_eq!(count_fts_matches(&connection, "before"), 0);
        assert_eq!(count_fts_matches(&connection, "after"), 1);
    }

    #[test]
    fn create_node_records_node_created_event() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node audit test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");

        let created =
            create_node(&connection, &draft_node("Audited node")).expect("node should be created");
        let events = list_events(&connection).expect("events should list");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, NODE_CREATED_EVENT);
        assert_eq!(events[0].source, STORAGE_AUDIT_SOURCE);
        assert_eq!(events[0].subject_kind, "node");
        assert_eq!(events[0].subject_id, created.id);
        assert!(!events[0].timestamp.trim().is_empty());
    }

    #[test]
    fn update_node_updates_fields_fts_and_records_event() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node update test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let created =
            create_node(&connection, &draft_node("Before title")).expect("node should create");

        let updated = update_node(
            &connection,
            &NodeUpdate {
                id: created.id,
                status: "active".to_string(),
                title: "After title".to_string(),
                summary: Some("after summary token".to_string()),
                body: Some("after body token".to_string()),
                source_ref: Some("source=user_instruction".to_string()),
                confidence: Some(0.8),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("node update should pass")
        .expect("node should exist");
        let events = list_events(&connection).expect("events should list");

        assert_eq!(updated.id, created.id);
        assert_eq!(updated.status, "active");
        assert_eq!(updated.title, "After title");
        assert_ne!(updated.updated_at, "");
        assert_eq!(count_fts_matches(&connection, "after"), 1);
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].event_type, crate::audit::NODE_UPDATED_EVENT);
        assert_eq!(events[1].subject_id, created.id);
    }

    #[test]
    fn update_node_returns_none_for_unknown_node() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node update test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");

        let updated = update_node(
            &connection,
            &NodeUpdate {
                id: 404,
                status: "draft".to_string(),
                title: "Missing".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("unknown update should not fail");

        assert!(updated.is_none());
    }

    #[test]
    fn create_node_rejects_invalid_type_and_status() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");

        let invalid_type = NewNode {
            node_type: "unknown".to_string(),
            status: "draft".to_string(),
            title: "Title".to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
        };
        let invalid_status = NewNode {
            node_type: "decision".to_string(),
            status: "unknown".to_string(),
            title: "Title".to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
        };

        assert!(matches!(
            create_node(&connection, &invalid_type),
            Err(NodeStorageError::Validation(
                NodeValidationError::InvalidType(_)
            ))
        ));
        assert!(matches!(
            create_node(&connection, &invalid_status),
            Err(NodeStorageError::Validation(
                NodeValidationError::InvalidStatus(_)
            ))
        ));
    }

    #[test]
    fn create_node_rejects_oversized_text_before_writing() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node size test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = NewNode {
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: "Bounded body".to_string(),
            summary: None,
            body: Some("x".repeat(MAX_NODE_BODY_BYTES + 1)),
            source_ref: None,
            confidence: None,
            trust_level: None,
        };

        assert!(matches!(
            create_node(&connection, &input),
            Err(NodeStorageError::Validation(
                NodeValidationError::FieldTooLong {
                    field: "body",
                    max_bytes: MAX_NODE_BODY_BYTES,
                }
            ))
        ));
        assert!(list_nodes(&connection)
            .expect("nodes should list")
            .is_empty());
    }

    #[test]
    fn teach_proposal_rejects_excess_items_and_long_node_refs() {
        let item = TeachProposalItem::CreateNode {
            node_ref: Some("n".to_string()),
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: "Bounded item".to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
        };
        let too_many = TeachProposalInput {
            items: vec![item.clone(); MAX_PROPOSAL_ITEMS + 1],
        };
        let long_ref = TeachProposalInput {
            items: vec![TeachProposalItem::CreateNode {
                node_ref: Some("n".repeat(MAX_PROPOSAL_NODE_REF_BYTES + 1)),
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Bounded item".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };

        assert!(matches!(
            validate_teach_proposal(&too_many),
            Err(TeachValidationError::TooManyProposalItems {
                max_items: MAX_PROPOSAL_ITEMS,
                actual,
            }) if actual == MAX_PROPOSAL_ITEMS + 1
        ));
        assert!(matches!(
            validate_teach_proposal(&long_ref),
            Err(TeachValidationError::NodeRefTooLong {
                max_bytes: MAX_PROPOSAL_NODE_REF_BYTES,
            })
        ));
    }

    #[test]
    fn invalid_teach_proposal_is_rejected_before_session_database_lookup() {
        let connection =
            Connection::open_in_memory().expect("empty DB should open for validation-order test");
        let proposal = TeachProposalInput { items: Vec::new() };

        let error = store_teach_proposal(&connection, 42, &proposal)
            .expect_err("empty proposal should fail before querying the empty DB");

        assert!(matches!(
            error,
            TeachStorageError::Validation(TeachValidationError::EmptyProposalItems)
        ));
    }

    #[test]
    fn active_nodes_require_source_confidence_and_trust() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = NewNode {
            node_type: "decision".to_string(),
            status: "active".to_string(),
            title: "Title".to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
        };

        assert!(matches!(
            create_node(&connection, &input),
            Err(NodeStorageError::Validation(
                NodeValidationError::MissingActiveSourceRef
            ))
        ));
    }

    #[test]
    fn draft_nodes_do_not_require_source_confidence_or_trust() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for node test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = NewNode {
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: "Draft note".to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
        };

        let node = create_node(&connection, &input).expect("draft node should be created");

        assert_eq!(node.status, "draft");
        assert_eq!(node.source_ref, None);
        assert_eq!(node.confidence, None);
        assert_eq!(node.trust_level, None);
    }

    #[test]
    fn create_and_list_links_between_existing_nodes() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for link test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let source =
            create_node(&connection, &draft_node("Source")).expect("source node should be created");
        let target =
            create_node(&connection, &draft_node("Target")).expect("target node should be created");
        let input = NewLink {
            source_node_id: source.id,
            target_node_id: target.id,
            link_type: "supports".to_string(),
        };

        let created = create_link(&connection, &input).expect("valid link should be created");
        let listed = list_links(&connection).expect("link list query should pass");

        assert_eq!(created.source_node_id, source.id);
        assert_eq!(created.target_node_id, target.id);
        assert_eq!(created.link_type, "supports");
        assert_eq!(listed, vec![created]);
    }

    #[test]
    fn list_links_page_uses_stable_keyset_cursor() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for link page test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let source =
            create_node(&connection, &draft_node("Source")).expect("source node should create");
        let target =
            create_node(&connection, &draft_node("Target")).expect("target node should create");
        let first = create_link(
            &connection,
            &NewLink {
                source_node_id: source.id,
                target_node_id: target.id,
                link_type: "first".to_string(),
            },
        )
        .expect("first link should create");
        let second = create_link(
            &connection,
            &NewLink {
                source_node_id: source.id,
                target_node_id: target.id,
                link_type: "second".to_string(),
            },
        )
        .expect("second link should create");
        let third = create_link(
            &connection,
            &NewLink {
                source_node_id: source.id,
                target_node_id: target.id,
                link_type: "third".to_string(),
            },
        )
        .expect("third link should create");

        let first_page =
            list_links_page(&connection, None, 2).expect("first link page should list");
        let second_page = list_links_page(&connection, first_page.next_after_id, 2)
            .expect("second link page should list");

        assert_eq!(first_page.items, vec![first, second.clone()]);
        assert!(first_page.more_results);
        assert_eq!(first_page.next_after_id, Some(second.id));
        assert_eq!(second_page.items, vec![third]);
        assert!(!second_page.more_results);
        assert_eq!(second_page.next_after_id, None);
    }

    #[test]
    fn create_link_records_link_created_event() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for link audit test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let source =
            create_node(&connection, &draft_node("Source")).expect("source node should be created");
        let target =
            create_node(&connection, &draft_node("Target")).expect("target node should be created");

        let created = create_link(
            &connection,
            &NewLink {
                source_node_id: source.id,
                target_node_id: target.id,
                link_type: "supports".to_string(),
            },
        )
        .expect("link should be created");
        let events = list_events(&connection).expect("events should list");

        let event = events
            .iter()
            .find(|event| event.event_type == LINK_CREATED_EVENT)
            .expect("link.created event should exist");
        assert_eq!(event.source, STORAGE_AUDIT_SOURCE);
        assert_eq!(event.subject_kind, "link");
        assert_eq!(event.subject_id, created.id);
        assert!(!event.timestamp.trim().is_empty());
    }

    #[test]
    fn create_link_rejects_missing_nodes_and_type() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for link test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let source =
            create_node(&connection, &draft_node("Source")).expect("source node should be created");

        assert!(matches!(
            create_link(
                &connection,
                &NewLink {
                    source_node_id: source.id,
                    target_node_id: source.id,
                    link_type: " ".to_string(),
                }
            ),
            Err(LinkStorageError::Validation(
                LinkValidationError::MissingType
            ))
        ));
        assert!(matches!(
            create_link(
                &connection,
                &NewLink {
                    source_node_id: 999,
                    target_node_id: source.id,
                    link_type: "supports".to_string(),
                }
            ),
            Err(LinkStorageError::Validation(
                LinkValidationError::SourceNodeNotFound(999)
            ))
        ));
        assert!(matches!(
            create_link(
                &connection,
                &NewLink {
                    source_node_id: source.id,
                    target_node_id: 999,
                    link_type: "supports".to_string(),
                }
            ),
            Err(LinkStorageError::Validation(
                LinkValidationError::TargetNodeNotFound(999)
            ))
        ));
    }

    #[test]
    fn create_and_list_aliases_tags_and_sources() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for metadata test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let node =
            create_node(&connection, &draft_node("Memory node")).expect("node should be created");

        let alias = create_alias(
            &connection,
            &NewAlias {
                node_id: node.id,
                alias: "nickname".to_string(),
            },
        )
        .expect("alias should be created");
        let tag = create_tag(
            &connection,
            &NewTag {
                node_id: node.id,
                tag: "storage".to_string(),
            },
        )
        .expect("tag should be created");
        let source = create_source(
            &connection,
            &NewSource {
                node_id: node.id,
                source_ref: "source=user_instruction".to_string(),
            },
        )
        .expect("source should be created");
        let other_node =
            create_node(&connection, &draft_node("Other memory node")).expect("node should create");
        let other_alias = create_alias(
            &connection,
            &NewAlias {
                node_id: other_node.id,
                alias: "other-nickname".to_string(),
            },
        )
        .expect("other alias should be created");
        let other_tag = create_tag(
            &connection,
            &NewTag {
                node_id: other_node.id,
                tag: "other-storage".to_string(),
            },
        )
        .expect("other tag should be created");
        let other_source = create_source(
            &connection,
            &NewSource {
                node_id: other_node.id,
                source_ref: "source=other_instruction".to_string(),
            },
        )
        .expect("other source should be created");

        assert_eq!(alias.node_id, node.id);
        assert_eq!(alias.alias, "nickname");
        assert_eq!(tag.node_id, node.id);
        assert_eq!(tag.tag, "storage");
        assert_eq!(source.node_id, node.id);
        assert_eq!(source.source_ref, "source=user_instruction");
        assert_eq!(
            list_aliases(&connection, Some(node.id)).expect("alias list should pass"),
            vec![alias.clone()]
        );
        assert_eq!(
            list_tags(&connection, Some(node.id)).expect("tag list should pass"),
            vec![tag.clone()]
        );
        assert_eq!(
            list_sources(&connection, Some(node.id)).expect("source list should pass"),
            vec![source.clone()]
        );
        assert_eq!(
            list_aliases(&connection, None).expect("unfiltered alias list should pass"),
            vec![alias.clone(), other_alias]
        );
        assert_eq!(
            list_tags(&connection, None).expect("unfiltered tag list should pass"),
            vec![tag.clone(), other_tag]
        );
        assert_eq!(
            list_sources(&connection, None).expect("unfiltered source list should pass"),
            vec![source, other_source]
        );
    }

    #[test]
    fn metadata_pages_use_node_filter_keyset_and_index() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for metadata page test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let node =
            create_node(&connection, &draft_node("Metadata node")).expect("node should create");
        let other_node = create_node(&connection, &draft_node("Other metadata node"))
            .expect("other node should create");

        let aliases = ["alias-one", "alias-two", "alias-three"]
            .into_iter()
            .map(|alias| {
                create_alias(
                    &connection,
                    &NewAlias {
                        node_id: node.id,
                        alias: alias.to_string(),
                    },
                )
                .expect("alias should create")
            })
            .collect::<Vec<_>>();
        let tags = ["tag-one", "tag-two", "tag-three"]
            .into_iter()
            .map(|tag| {
                create_tag(
                    &connection,
                    &NewTag {
                        node_id: node.id,
                        tag: tag.to_string(),
                    },
                )
                .expect("tag should create")
            })
            .collect::<Vec<_>>();
        let sources = ["source=one", "source=two", "source=three"]
            .into_iter()
            .map(|source_ref| {
                create_source(
                    &connection,
                    &NewSource {
                        node_id: node.id,
                        source_ref: source_ref.to_string(),
                    },
                )
                .expect("source should create")
            })
            .collect::<Vec<_>>();
        create_alias(
            &connection,
            &NewAlias {
                node_id: other_node.id,
                alias: "other-alias".to_string(),
            },
        )
        .expect("other alias should create");

        let alias_page =
            list_aliases_page(&connection, Some(node.id), None, 2).expect("alias page should list");
        let tag_page =
            list_tags_page(&connection, Some(node.id), None, 2).expect("tag page should list");
        let source_page = list_sources_page(&connection, Some(node.id), None, 2)
            .expect("source page should list");

        assert_eq!(alias_page.items, aliases[..2]);
        assert_eq!(alias_page.next_after_id, Some(aliases[1].id));
        assert!(alias_page.more_results);
        assert_eq!(tag_page.items, tags[..2]);
        assert_eq!(tag_page.next_after_id, Some(tags[1].id));
        assert!(tag_page.more_results);
        assert_eq!(source_page.items, sources[..2]);
        assert_eq!(source_page.next_after_id, Some(sources[1].id));
        assert!(source_page.more_results);

        for (table, index) in [
            ("aliases", "idx_aliases_node"),
            ("tags", "idx_tags_node"),
            ("sources", "idx_sources_node"),
        ] {
            let query = format!(
                "EXPLAIN QUERY PLAN SELECT id FROM {table} \
                 WHERE node_id = ?1 AND id > ?2 ORDER BY id ASC LIMIT ?3;"
            );
            let plan: String = connection
                .query_row(&query, params![node.id, 0_i64, 3_i64], |row| row.get(3))
                .expect("metadata page query plan should be readable");

            assert!(
                plan.contains(index),
                "{table} page must use {index}, got: {plan}"
            );
        }
    }

    #[test]
    fn list_nodes_with_summaries_returns_only_requested_records() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for summary query");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let matching = create_node(
            &connection,
            &NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Reflection material".to_string(),
                summary: Some("reflection_material_v1".to_string()),
                body: Some("{\"session_id\":\"codex-chat-1\"}".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("matching node should create");
        create_node(
            &connection,
            &NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Unrelated node".to_string(),
                summary: Some("other_record".to_string()),
                body: Some("x".repeat(8_192)),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("unrelated node should create");

        let nodes = list_nodes_with_summaries(&connection, &["reflection_material_v1"])
            .expect("summary query should pass");
        let empty =
            list_nodes_with_summaries(&connection, &[]).expect("empty summary query should pass");
        let query_plan: String = connection
            .query_row(
                "EXPLAIN QUERY PLAN SELECT id FROM nodes WHERE summary IN (?1);",
                ["reflection_material_v1"],
                |row| row.get(3),
            )
            .expect("summary query plan should be readable");

        assert_eq!(nodes, vec![matching]);
        assert!(empty.is_empty());
        assert!(
            query_plan.contains("INDEX idx_nodes_summary"),
            "summary query must use idx_nodes_summary, got: {query_plan}"
        );
    }

    #[test]
    fn metadata_rejects_missing_node_and_empty_values() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for metadata test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let node =
            create_node(&connection, &draft_node("Memory node")).expect("node should be created");

        assert!(matches!(
            create_alias(
                &connection,
                &NewAlias {
                    node_id: 999,
                    alias: "nickname".to_string(),
                }
            ),
            Err(MetadataStorageError::Validation(
                MetadataValidationError::NodeNotFound(999)
            ))
        ));
        assert!(matches!(
            create_alias(
                &connection,
                &NewAlias {
                    node_id: node.id,
                    alias: " ".to_string(),
                }
            ),
            Err(MetadataStorageError::Validation(
                MetadataValidationError::MissingAlias
            ))
        ));
        assert!(matches!(
            create_tag(
                &connection,
                &NewTag {
                    node_id: node.id,
                    tag: " ".to_string(),
                }
            ),
            Err(MetadataStorageError::Validation(
                MetadataValidationError::MissingTag
            ))
        ));
        assert!(matches!(
            create_source(
                &connection,
                &NewSource {
                    node_id: node.id,
                    source_ref: " ".to_string(),
                }
            ),
            Err(MetadataStorageError::Validation(
                MetadataValidationError::MissingSourceRef
            ))
        ));
    }

    #[test]
    fn invalid_metadata_is_rejected_before_node_database_lookup() {
        let connection =
            Connection::open_in_memory().expect("empty DB should open for validation-order test");

        assert!(matches!(
            create_alias(
                &connection,
                &NewAlias {
                    node_id: 1,
                    alias: " ".to_string(),
                }
            ),
            Err(MetadataStorageError::Validation(
                MetadataValidationError::MissingAlias
            ))
        ));
        assert!(matches!(
            create_tag(
                &connection,
                &NewTag {
                    node_id: 1,
                    tag: " ".to_string(),
                }
            ),
            Err(MetadataStorageError::Validation(
                MetadataValidationError::MissingTag
            ))
        ));
        assert!(matches!(
            create_source(
                &connection,
                &NewSource {
                    node_id: 1,
                    source_ref: " ".to_string(),
                }
            ),
            Err(MetadataStorageError::Validation(
                MetadataValidationError::MissingSourceRef
            ))
        ));
        assert!(matches!(
            create_alias(
                &connection,
                &NewAlias {
                    node_id: 1,
                    alias: "x".repeat(MAX_METADATA_VALUE_BYTES + 1),
                }
            ),
            Err(MetadataStorageError::Validation(
                MetadataValidationError::ValueTooLong {
                    kind: "alias",
                    max_bytes: MAX_METADATA_VALUE_BYTES,
                }
            ))
        ));
    }

    #[test]
    fn teach_apply_refreshes_fts_after_all_aliases_for_a_node() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for teach apply");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let session = create_teach_session(
            &connection,
            &NewTeachSession {
                title: "Batch aliases".to_string(),
                summary: None,
            },
        )
        .expect("teach session should create");
        let proposal = store_teach_proposal(
            &connection,
            session.session_id,
            &TeachProposalInput {
                items: vec![
                    TeachProposalItem::CreateNode {
                        node_ref: Some("lesson".to_string()),
                        node_type: "lesson".to_string(),
                        status: "draft".to_string(),
                        title: "Batch alias lesson".to_string(),
                        summary: None,
                        body: None,
                        source_ref: None,
                        confidence: None,
                        trust_level: None,
                    },
                    TeachProposalItem::AddAlias {
                        node_id: None,
                        node_ref: Some("lesson".to_string()),
                        alias: "batch-alias-one".to_string(),
                    },
                    TeachProposalItem::AddAlias {
                        node_id: None,
                        node_ref: Some("lesson".to_string()),
                        alias: "batch-alias-two".to_string(),
                    },
                    TeachProposalItem::AddAlias {
                        node_id: None,
                        node_ref: Some("lesson".to_string()),
                        alias: "batch-alias-three".to_string(),
                    },
                ],
            },
        )
        .expect("teach proposal should create");

        let report = apply_teach_proposal(&connection, session.session_id, proposal.proposal_id)
            .expect("teach proposal should apply atomically");
        let created_node_id = report.created_node_ids[0];
        let aliases =
            list_aliases(&connection, Some(created_node_id)).expect("created aliases should list");

        assert_eq!(report.created_alias_ids.len(), 3);
        assert_eq!(aliases.len(), 3);
        for alias in ["batch-alias-one", "batch-alias-two", "batch-alias-three"] {
            let results =
                search_nodes_fts(&connection, alias, 5).expect("alias FTS search should pass");
            assert_eq!(
                results
                    .iter()
                    .filter(|result| result.node.id == created_node_id)
                    .count(),
                1,
                "alias should be searchable after batch refresh"
            );
        }
    }

    #[test]
    fn nested_teach_apply_error_rolls_back_only_its_partial_writes() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for nested teach apply");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let session = create_teach_session(
            &connection,
            &NewTeachSession {
                title: "Nested atomic teach".to_string(),
                summary: None,
            },
        )
        .expect("teach session should create");
        let proposal = store_teach_proposal(
            &connection,
            session.session_id,
            &TeachProposalInput {
                items: vec![
                    TeachProposalItem::CreateNode {
                        node_ref: Some("nested-node".to_string()),
                        node_type: "lesson".to_string(),
                        status: "draft".to_string(),
                        title: "Must roll back".to_string(),
                        summary: None,
                        body: None,
                        source_ref: None,
                        confidence: None,
                        trust_level: None,
                    },
                    TeachProposalItem::AddAlias {
                        node_id: None,
                        node_ref: Some("nested-node".to_string()),
                        alias: "duplicate-late-alias".to_string(),
                    },
                    TeachProposalItem::AddAlias {
                        node_id: None,
                        node_ref: Some("nested-node".to_string()),
                        alias: "duplicate-late-alias".to_string(),
                    },
                ],
            },
        )
        .expect("late-failing teach proposal should store");
        let tables = ["nodes", "aliases", "tags", "sources", "links", "events"];
        let baseline = tables
            .iter()
            .map(|table| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("baseline table count should query")
            })
            .collect::<Vec<_>>();

        let transaction = connection
            .unchecked_transaction()
            .expect("outer transaction should begin");
        let error = apply_teach_proposal(&transaction, session.session_id, proposal.proposal_id)
            .expect_err("duplicate alias should fail after earlier writes");
        assert!(matches!(
            error,
            TeachStorageError::Metadata(MetadataStorageError::Db(_))
        ));
        transaction
            .execute("CREATE TABLE outer_work_survives (id INTEGER);", [])
            .expect("caller should remain able to use its transaction");
        transaction
            .commit()
            .expect("caller may intentionally commit after catching teach error");

        let after = tables
            .iter()
            .map(|table| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("post-error table count should query")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            after, baseline,
            "failed nested apply must be function-atomic"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'outer_work_survives'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("outer transaction marker should query"),
            1,
            "savepoint rollback must not roll back caller-owned work"
        );
    }

    #[test]
    fn create_get_and_list_mcp_profiles() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for MCP test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = NewMcpProfile {
            id: "codebase-memory".to_string(),
            name: "Codebase Memory MCP".to_string(),
            kind: "optional".to_string(),
            status: "missing".to_string(),
            read_operations: "search_graph,search_code".to_string(),
            write_operations: "index_repository".to_string(),
            side_effects: "local_read".to_string(),
            approval_requirement: "none".to_string(),
            credentials_source: Some("none".to_string()),
            notes: Some("best-effort installer profile".to_string()),
        };

        let created =
            create_mcp_profile(&connection, &input).expect("valid MCP profile should be created");
        let fetched = get_mcp_profile(&connection, &created.id)
            .expect("MCP profile get should pass")
            .expect("created MCP profile should exist");
        let listed = list_mcp_profiles(&connection).expect("MCP profile list should pass");

        assert_eq!(created.id, "codebase-memory");
        assert_eq!(created.name, "Codebase Memory MCP");
        assert_eq!(created.kind, "optional");
        assert_eq!(created.status, "missing");
        assert_eq!(created.side_effects, "local_read");
        assert_eq!(created.approval_requirement, "none");
        assert_eq!(fetched, created);
        assert_eq!(listed, vec![created]);
    }

    #[test]
    fn list_mcp_profiles_page_uses_string_keyset_cursor() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for MCP page test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let alpha = create_mcp_profile(&connection, &test_mcp_profile("alpha"))
            .expect("alpha MCP profile should create");
        let bravo = create_mcp_profile(&connection, &test_mcp_profile("bravo"))
            .expect("bravo MCP profile should create");
        let charlie = create_mcp_profile(&connection, &test_mcp_profile("charlie"))
            .expect("charlie MCP profile should create");

        let first_page = list_mcp_profiles_page(&connection, None, 2)
            .expect("first MCP profile page should list");
        let second_page =
            list_mcp_profiles_page(&connection, first_page.next_after_id.as_deref(), 2)
                .expect("second MCP profile page should list");

        assert_eq!(first_page.items, vec![alpha, bravo.clone()]);
        assert!(first_page.more_results);
        assert_eq!(first_page.next_after_id.as_deref(), Some(bravo.id.as_str()));
        assert_eq!(second_page.items, vec![charlie]);
        assert!(!second_page.more_results);
        assert_eq!(second_page.next_after_id, None);
    }

    #[test]
    fn corporate_mcp_registry_can_start_empty() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for MCP test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");

        let listed = list_mcp_profiles(&connection).expect("empty MCP profile list should pass");

        assert!(listed.is_empty());
    }

    #[test]
    fn upsert_mcp_profile_updates_existing_profile() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for MCP test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let first = NewMcpProfile {
            id: "understand-anything".to_string(),
            name: "Understand Anything".to_string(),
            kind: "optional".to_string(),
            status: "missing".to_string(),
            read_operations: "project_docs".to_string(),
            write_operations: "best_effort_index".to_string(),
            side_effects: "local_read".to_string(),
            approval_requirement: "none".to_string(),
            credentials_source: Some("none".to_string()),
            notes: Some("best-effort installer profile".to_string()),
        };
        let second = NewMcpProfile {
            status: "installed".to_string(),
            ..first.clone()
        };

        let created = upsert_mcp_profile(&connection, &first)
            .expect("initial MCP profile upsert should create");
        let updated = upsert_mcp_profile(&connection, &second)
            .expect("second MCP profile upsert should update");
        let listed = list_mcp_profiles(&connection).expect("MCP profile list should pass");

        assert_eq!(created.status, "missing");
        assert_eq!(updated.id, "understand-anything");
        assert_eq!(updated.status, "installed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, "installed");
    }

    #[test]
    fn mcp_profile_rejects_empty_required_fields() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for MCP test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = NewMcpProfile {
            id: " ".to_string(),
            name: "Codebase Memory MCP".to_string(),
            kind: "optional".to_string(),
            status: "missing".to_string(),
            read_operations: "search_graph".to_string(),
            write_operations: "index_repository".to_string(),
            side_effects: "local_read".to_string(),
            approval_requirement: "none".to_string(),
            credentials_source: None,
            notes: None,
        };

        assert!(matches!(
            create_mcp_profile(&connection, &input),
            Err(McpProfileStorageError::Validation(
                McpProfileValidationError::MissingId
            ))
        ));
    }

    #[test]
    fn mcp_profile_rejects_oversized_notes_before_writing() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for MCP size test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = NewMcpProfile {
            notes: Some("n".repeat(MAX_MCP_NOTES_BYTES + 1)),
            ..test_mcp_profile("bounded-notes")
        };

        assert!(matches!(
            create_mcp_profile(&connection, &input),
            Err(McpProfileStorageError::Validation(
                McpProfileValidationError::FieldTooLong {
                    field: "notes",
                    max_bytes: MAX_MCP_NOTES_BYTES,
                }
            ))
        ));
        assert!(list_mcp_profiles(&connection)
            .expect("MCP profiles should list")
            .is_empty());
    }

    #[test]
    fn node_source_hierarchy_parses_priority_and_path() {
        let node = Node {
            id: 1,
            node_type: "workflow".to_string(),
            status: "active".to_string(),
            title: "Priority workflow".to_string(),
            summary: None,
            body: None,
            source_ref: Some("source=mcp/corporate/github".to_string()),
            confidence: Some(0.9),
            trust_level: Some("high".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let hierarchy = node
            .source_hierarchy()
            .expect("source hierarchy should parse");

        assert_eq!(hierarchy.source_root, "mcp");
        assert_eq!(hierarchy.source_leaf, "github");
        assert_eq!(
            hierarchy.source_path,
            vec![
                "mcp".to_string(),
                "corporate".to_string(),
                "github".to_string()
            ]
        );
        assert_eq!(hierarchy.priority, 4);
    }

    #[test]
    fn tool_contract_node_extracts_least_privilege_metadata_from_body() {
        let node = Node {
            id: 2,
            node_type: "tool_contract".to_string(),
            status: "draft".to_string(),
            title: "Context export".to_string(),
            summary: None,
            body: Some(
                r#"{
                    "side_effects":"local_write_artifact",
                    "approval_requirement":"manual_review",
                    "read_operations":["memory.read"],
                    "write_operations":["artifact.write"]
                }"#
                .to_string(),
            ),
            source_ref: Some("source=tool/context-export".to_string()),
            confidence: None,
            trust_level: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let metadata = node
            .least_privilege_metadata()
            .expect("tool contract least-privilege metadata should parse");

        assert_eq!(metadata.side_effects, "local_write_artifact");
        assert_eq!(metadata.approval_requirement, "manual_review");
        assert_eq!(metadata.read_operations, vec!["memory.read".to_string()]);
        assert_eq!(
            metadata.write_operations,
            vec!["artifact.write".to_string()]
        );
        assert_eq!(metadata.privilege_rank, 2);
    }

    #[test]
    fn mcp_profile_exposes_normalized_least_privilege_metadata() {
        let profile = NewMcpProfile {
            id: "corp-github".to_string(),
            name: "Corporate GitHub".to_string(),
            kind: "corporate".to_string(),
            status: "active".to_string(),
            read_operations: " repos.read , issues.read ".to_string(),
            write_operations: "issues.write, pr.write".to_string(),
            side_effects: "external_write".to_string(),
            approval_requirement: "manual_review".to_string(),
            credentials_source: Some("keychain".to_string()),
            notes: None,
        };
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for MCP test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");

        let stored =
            create_mcp_profile(&connection, &profile).expect("MCP profile should be created");
        let metadata = stored.least_privilege_metadata();

        assert_eq!(metadata.side_effects, "external_write");
        assert_eq!(metadata.approval_requirement, "manual_review");
        assert_eq!(
            metadata.read_operations,
            vec!["repos.read".to_string(), "issues.read".to_string()]
        );
        assert_eq!(
            metadata.write_operations,
            vec!["issues.write".to_string(), "pr.write".to_string()]
        );
        assert_eq!(metadata.privilege_rank, 5);
    }

    fn draft_node(title: &str) -> NewNode {
        NewNode {
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: title.to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
        }
    }

    fn task_recall_node(node_type: &str, status: &str, title: &str, body: &str) -> NewNode {
        let active = status == "active";
        NewNode {
            node_type: node_type.to_string(),
            status: status.to_string(),
            title: title.to_string(),
            summary: None,
            body: Some(body.to_string()),
            source_ref: active.then(|| "user:test".to_string()),
            confidence: active.then_some(1.0),
            trust_level: active.then(|| "high".to_string()),
        }
    }

    fn test_mcp_profile(id: &str) -> NewMcpProfile {
        NewMcpProfile {
            id: id.to_string(),
            name: format!("{id} MCP"),
            kind: "optional".to_string(),
            status: "missing".to_string(),
            read_operations: "read".to_string(),
            write_operations: "write".to_string(),
            side_effects: "local_read".to_string(),
            approval_requirement: "none".to_string(),
            credentials_source: None,
            notes: None,
        }
    }

    fn count_fts_matches(connection: &Connection, term: &str) -> i64 {
        connection
            .query_row(
                "SELECT COUNT(*) FROM fts_nodes WHERE fts_nodes MATCH ?1;",
                [term],
                |row| row.get(0),
            )
            .expect("FTS query should pass")
    }

    #[test]
    fn search_nodes_fts_returns_bm25_order_and_excludes_old_statuses() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for FTS test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let first = create_node(
            &connection,
            &NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "needle".to_string(),
                summary: None,
                body: Some("needle needle needle".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("first node should be created");
        let second =
            create_node(&connection, &draft_node("needle")).expect("second node should be created");
        create_node(
            &connection,
            &NewNode {
                node_type: "raw_note".to_string(),
                status: "deprecated".to_string(),
                title: "needle old".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("deprecated node should be created");

        let results = search_nodes_fts(&connection, "needle", 10).expect("FTS search should pass");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].node.id, first.id);
        assert_eq!(results[1].node.id, second.id);
        assert!(results[0].rank <= results[1].rank);
    }

    #[test]
    fn task_recall_loads_typed_fts_and_direct_layers_with_complete_bodies() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for task recall");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let full_body = "complete-workflow-body-".repeat(2_000);
        let workflow = create_node(
            &connection,
            &task_recall_node("workflow", "draft", "Deploy release", &full_body),
        )
        .expect("workflow should create");
        let tool = create_node(
            &connection,
            &task_recall_node("tool_contract", "draft", "Release helper", "tool body"),
        )
        .expect("tool should create");
        let failure = create_node(
            &connection,
            &task_recall_node("failure_mode", "draft", "Old release fault", "failure body"),
        )
        .expect("failure mode should create");
        let linked_rule = create_node(
            &connection,
            &task_recall_node("rule", "draft", "Old important rule", "old rule body"),
        )
        .expect("linked rule should create");
        create_node(
            &connection,
            &task_recall_node(
                "raw_note",
                "draft",
                "Weak match",
                "deploy release deploy release deploy release",
            ),
        )
        .expect("FTS fallback should create");
        create_alias(
            &connection,
            &NewAlias {
                node_id: tool.id,
                alias: "Deploy release".to_string(),
            },
        )
        .expect("tool alias should create");
        create_tag(
            &connection,
            &NewTag {
                node_id: failure.id,
                tag: "Deploy release".to_string(),
            },
        )
        .expect("failure tag should create");
        create_link(
            &connection,
            &NewLink {
                source_node_id: workflow.id,
                target_node_id: linked_rule.id,
                link_type: "must_follow".to_string(),
            },
        )
        .expect("direct link should create");

        let candidates = load_task_recall_candidates(&connection, "Deploy release", 20)
            .expect("task candidates should load");

        assert_eq!(
            candidates
                .typed_roots
                .iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            [workflow.id, tool.id, failure.id]
        );
        assert_eq!(
            candidates.typed_roots[0].body.as_deref(),
            Some(full_body.as_str())
        );
        assert!(candidates
            .fts_results
            .windows(2)
            .all(|pair| pair[0].rank <= pair[1].rank));
        assert!(candidates
            .fts_results
            .iter()
            .all(|result| result.node.body.is_some()));
        assert!(candidates.direct_nodes.iter().any(|linked| {
            linked.root_node_id == workflow.id
                && linked.node.id == linked_rule.id
                && linked.node.body.as_deref() == Some("old rule body")
        }));
        assert!(!candidates.more_results);
    }

    #[test]
    fn task_recall_graph_finds_depth_two_node_without_query_text() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for graph recall");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let workflow = create_node(
            &connection,
            &task_recall_node("workflow", "draft", "Ship release", "workflow body"),
        )
        .expect("workflow should create");
        let step = create_node(
            &connection,
            &task_recall_node("lesson", "draft", "Historical step", "unrelated text"),
        )
        .expect("step should create");
        let old_rule = create_node(
            &connection,
            &task_recall_node(
                "rule",
                "draft",
                "Old important linked rule",
                "must preserve the old release guard",
            ),
        )
        .expect("old rule should create");
        create_link(
            &connection,
            &NewLink {
                source_node_id: workflow.id,
                target_node_id: step.id,
                link_type: "has_step".to_string(),
            },
        )
        .expect("depth-one link should create");
        create_link(
            &connection,
            &NewLink {
                source_node_id: step.id,
                target_node_id: old_rule.id,
                link_type: "must_follow".to_string(),
            },
        )
        .expect("depth-two link should create");

        let candidates = load_task_recall_candidates(&connection, "Ship release", 20)
            .expect("task candidates should load");
        let traversed = candidates
            .graph_nodes
            .iter()
            .find(|candidate| candidate.node.id == old_rule.id)
            .expect("depth-two old rule should be found through links");

        assert_eq!(traversed.root_node_id, workflow.id);
        assert_eq!(traversed.root_node_type, "workflow");
        assert_eq!(traversed.edge_source_node_id, step.id);
        assert_eq!(traversed.link.link_type, "must_follow");
        assert_eq!(traversed.depth, 2);
        assert_eq!(traversed.node.body, old_rule.body);
    }

    #[test]
    fn task_recall_graph_is_cycle_safe_and_does_not_return_root_again() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for cycle proof");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let workflow = create_node(
            &connection,
            &task_recall_node("workflow", "draft", "Cycle query", "root body"),
        )
        .expect("workflow should create");
        let lesson = create_node(
            &connection,
            &task_recall_node("lesson", "draft", "Historical lesson", "unrelated"),
        )
        .expect("lesson should create");
        create_link(
            &connection,
            &NewLink {
                source_node_id: workflow.id,
                target_node_id: lesson.id,
                link_type: "next".to_string(),
            },
        )
        .expect("forward link should create");
        create_link(
            &connection,
            &NewLink {
                source_node_id: lesson.id,
                target_node_id: workflow.id,
                link_type: "cycle".to_string(),
            },
        )
        .expect("cycle link should create");

        let candidates = load_task_recall_candidates(&connection, "Cycle query", 20)
            .expect("task candidates should load");
        let graph_ids = candidates
            .graph_nodes
            .iter()
            .map(|candidate| candidate.node.id)
            .collect::<Vec<_>>();

        assert_eq!(graph_ids, vec![lesson.id]);
        assert!(!candidates.more_results);
    }

    #[test]
    fn task_recall_graph_filters_mandatory_and_unusable_nodes_before_cap() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for graph filtering");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let workflow = create_node(
            &connection,
            &task_recall_node("workflow", "draft", "Filtered graph", "root"),
        )
        .expect("workflow should create");
        for index in 0..4 {
            let mandatory = create_node(
                &connection,
                &task_recall_node("rule", "active", &format!("Mandatory {index}"), "mandatory"),
            )
            .expect("mandatory node should create");
            create_link(
                &connection,
                &NewLink {
                    source_node_id: workflow.id,
                    target_node_id: mandatory.id,
                    link_type: "mandatory".to_string(),
                },
            )
            .expect("mandatory link should create");
        }
        for status in ["deprecated", "superseded", "broken"] {
            let excluded = create_node(
                &connection,
                &task_recall_node("lesson", status, status, "excluded"),
            )
            .expect("excluded node should create");
            create_link(
                &connection,
                &NewLink {
                    source_node_id: workflow.id,
                    target_node_id: excluded.id,
                    link_type: "excluded".to_string(),
                },
            )
            .expect("excluded link should create");
        }
        let applicable = create_node(
            &connection,
            &task_recall_node("lesson", "draft", "Applicable", "complete body"),
        )
        .expect("applicable node should create");
        create_link(
            &connection,
            &NewLink {
                source_node_id: workflow.id,
                target_node_id: applicable.id,
                link_type: "applicable".to_string(),
            },
        )
        .expect("applicable link should create");

        let candidates = load_task_recall_candidates(&connection, "Filtered graph", 1)
            .expect("task candidates should load");

        assert_eq!(candidates.graph_nodes.len(), 1);
        assert_eq!(candidates.graph_nodes[0].node.id, applicable.id);
        assert!(!candidates.graph_nodes.iter().any(|candidate| {
            candidate.node.status == "active"
                && matches!(
                    candidate.node.node_type.as_str(),
                    "kernel_contract" | "gate" | "project_profile" | "source" | "rule"
                )
        }));
    }

    #[test]
    fn task_recall_graph_probes_one_extra_candidate() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for graph cap proof");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let workflow = create_node(
            &connection,
            &task_recall_node("workflow", "draft", "Graph cap", "root"),
        )
        .expect("workflow should create");
        for index in 0..4 {
            let lesson = create_node(
                &connection,
                &task_recall_node("lesson", "draft", &format!("Lesson {index}"), "unrelated"),
            )
            .expect("lesson should create");
            create_link(
                &connection,
                &NewLink {
                    source_node_id: workflow.id,
                    target_node_id: lesson.id,
                    link_type: "step".to_string(),
                },
            )
            .expect("lesson link should create");
        }

        let candidates = load_task_recall_candidates(&connection, "Graph cap", 2)
            .expect("task candidates should load");

        assert_eq!(candidates.graph_nodes.len(), 2);
        assert!(candidates.more_results);
        assert!(candidates.graph_nodes.windows(2).all(|pair| {
            (pair[0].depth, pair[0].link.id, pair[0].node.id)
                <= (pair[1].depth, pair[1].link.id, pair[1].node.id)
        }));
    }

    #[test]
    fn task_typed_root_exact_lookup_uses_targeted_nocase_indexes() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for query-plan proof");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let mut statement = connection
            .prepare(
                "
                EXPLAIN QUERY PLAN
                WITH exact_ids(id) AS (
                    SELECT id FROM nodes WHERE title = ?1 COLLATE NOCASE
                    UNION
                    SELECT node_id FROM aliases WHERE alias = ?1 COLLATE NOCASE
                    UNION
                    SELECT node_id FROM tags WHERE tag = ?1 COLLATE NOCASE
                )
                SELECT nodes.id
                FROM exact_ids
                JOIN nodes ON nodes.id = exact_ids.id;
                ",
            )
            .expect("query plan should prepare");
        let details = statement
            .query_map(["Deploy release"], |row| row.get::<_, String>(3))
            .expect("query plan should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("query plan should collect")
            .join("\n");

        for index in [
            "idx_nodes_title_nocase",
            "idx_aliases_alias_nocase",
            "idx_tags_tag_nocase",
        ] {
            assert!(
                details.contains(index),
                "query plan did not use {index}: {details}"
            );
        }
    }

    #[test]
    fn task_recall_excludes_unusable_statuses_in_every_layer() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for exclusions");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let root = create_node(
            &connection,
            &task_recall_node("workflow", "draft", "Safe query", "safe query"),
        )
        .expect("root should create");
        let mut excluded_ids = Vec::new();
        for status in ["deprecated", "superseded", "broken"] {
            let typed = create_node(
                &connection,
                &task_recall_node("workflow", status, "Safe query", "safe query"),
            )
            .expect("excluded typed root should create");
            let linked = create_node(
                &connection,
                &task_recall_node("lesson", status, "Excluded target", "safe query"),
            )
            .expect("excluded direct target should create");
            create_link(
                &connection,
                &NewLink {
                    source_node_id: root.id,
                    target_node_id: linked.id,
                    link_type: "excluded".to_string(),
                },
            )
            .expect("excluded target link should create");
            excluded_ids.extend([typed.id, linked.id]);
        }

        let candidates = load_task_recall_candidates(&connection, "Safe query", 20)
            .expect("task candidates should load");
        let returned_ids = candidates
            .typed_roots
            .iter()
            .map(|node| node.id)
            .chain(candidates.fts_results.iter().map(|result| result.node.id))
            .chain(candidates.direct_nodes.iter().map(|linked| linked.node.id))
            .collect::<BTreeSet<_>>();

        assert!(excluded_ids
            .iter()
            .all(|node_id| !returned_ids.contains(node_id)));
    }

    #[test]
    fn mandatory_matches_do_not_consume_task_candidate_limits() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for starvation test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let mut mandatory_rules = Vec::new();
        for index in 0..8 {
            let rule = create_node(
                &connection,
                &task_recall_node(
                    "rule",
                    "active",
                    &format!("Mandatory {index}"),
                    "starvationtoken starvationtoken",
                ),
            )
            .expect("mandatory rule should create");
            mandatory_rules.push(rule);
        }
        let workflow = create_node(
            &connection,
            &task_recall_node("workflow", "draft", "Relevant workflow", "starvationtoken"),
        )
        .expect("workflow should create");
        for rule in &mandatory_rules {
            create_link(
                &connection,
                &NewLink {
                    source_node_id: workflow.id,
                    target_node_id: rule.id,
                    link_type: "mandatory".to_string(),
                },
            )
            .expect("mandatory target link should create");
        }
        let task_target = create_node(
            &connection,
            &task_recall_node("lesson", "draft", "Task target", "task target body"),
        )
        .expect("task target should create");
        create_link(
            &connection,
            &NewLink {
                source_node_id: workflow.id,
                target_node_id: task_target.id,
                link_type: "next".to_string(),
            },
        )
        .expect("task target link should create");

        let candidates = load_task_recall_candidates(&connection, "starvationtoken", 2)
            .expect("task candidates should load");

        assert!(candidates
            .fts_results
            .iter()
            .any(|result| result.node.id == workflow.id));
        assert!(candidates
            .fts_results
            .iter()
            .all(|result| result.node.node_type != "rule"));
        assert!(candidates
            .direct_nodes
            .iter()
            .any(|linked| linked.node.id == task_target.id));
        assert!(candidates
            .direct_nodes
            .iter()
            .all(|linked| linked.node.node_type != "rule"));
    }

    #[test]
    fn task_recall_candidate_layers_remain_bounded_and_report_more() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for bounded task recall");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        for index in 0..80 {
            create_node(
                &connection,
                &task_recall_node(
                    "raw_note",
                    "draft",
                    &format!("Candidate {index}"),
                    "largecandidate",
                ),
            )
            .expect("candidate should create");
        }

        let candidates = load_task_recall_candidates(&connection, "largecandidate", 50)
            .expect("bounded candidates should load");

        assert!(candidates.typed_roots.len() <= 50);
        assert_eq!(candidates.fts_results.len(), 50);
        assert!(candidates.direct_nodes.len() <= 50);
        assert!(candidates.more_results);
    }

    #[test]
    fn continuation_sql_order_matches_stage14_rust_priority_across_pages() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for priority proof");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        prepare_task_recall_connection(&connection).expect("recall scalar should register");

        let fixtures = [
            ("source=external/docs", "high", 1.0),
            ("source=user_instruction/chat", "low", 1.0),
            ("source=user_instruction/chat", "high", 0.2),
            ("source=user_instruction/chat", "high", 0.9),
        ];
        let mut inserted = Vec::new();
        for (source_ref, trust_level, confidence) in fixtures {
            let mut input = task_recall_node(
                "workflow",
                "draft",
                "priority continuation",
                "priority continuation",
            );
            input.source_ref = Some(source_ref.to_string());
            input.trust_level = Some(trust_level.to_string());
            input.confidence = Some(confidence);
            inserted.push(create_node(&connection, &input).expect("fixture should create"));
        }

        let mut paged_ids = Vec::new();
        for offset in 0..inserted.len() as u64 {
            let page = load_task_typed_roots_page(&connection, "priority continuation", offset, 1)
                .expect("priority page should load");
            assert!(page.items.len() <= 1);
            paged_ids.extend(page.items.into_iter().map(|node| node.id));
        }

        let candidates = load_task_recall_candidates(&connection, "priority continuation", 20)
            .expect("one-shot candidates should load");
        let one_shot = crate::recall::build_task_recall_context(
            candidates,
            &crate::recall::RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("one-shot ordering should build");
        let one_shot_ids = one_shot
            .section
            .nodes
            .into_iter()
            .map(|selected| selected.node.id)
            .collect::<Vec<_>>();

        assert_eq!(paged_ids, one_shot_ids);
        assert_eq!(
            paged_ids,
            vec![
                inserted[3].id,
                inserted[2].id,
                inserted[1].id,
                inserted[0].id
            ]
        );
    }

    #[test]
    fn continuation_fts_sql_order_matches_rust_bm25_tiebreak_across_pages() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for BM25 parity proof");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        prepare_task_recall_connection(&connection).expect("recall scalar should register");
        for repetitions in [1, 3, 8, 20] {
            let mut input = task_recall_node(
                "raw_note",
                "draft",
                &format!("BM25 candidate {repetitions}"),
                &format!("{} filler", "bm25parity ".repeat(repetitions)),
            );
            input.source_ref = Some("source=teach/session".to_string());
            input.trust_level = Some("high".to_string());
            input.confidence = Some(0.8);
            create_node(&connection, &input).expect("BM25 candidate should create");
        }

        let mut paged = Vec::new();
        for offset in 0..4_u64 {
            let page = load_task_fts_page(&connection, "bm25parity", offset, 1)
                .expect("FTS page should load");
            assert!(page.items.len() <= 1);
            paged.extend(page.items);
        }
        assert!(paged.windows(2).all(|items| items[0].rank <= items[1].rank));

        let one_shot = crate::recall::build_task_recall_context(
            load_task_recall_candidates(&connection, "bm25parity", 20)
                .expect("one-shot candidates should load"),
            &crate::recall::RecallSection {
                complete: true,
                nodes: Vec::new(),
            },
        )
        .expect("one-shot selection should build");
        let paged_ids = paged
            .iter()
            .map(|result| result.node.id)
            .collect::<Vec<_>>();
        let one_shot_ids = one_shot
            .section
            .nodes
            .iter()
            .map(|selected| selected.node.id)
            .collect::<Vec<_>>();

        assert_eq!(paged_ids, one_shot_ids);
        for (selected, result) in one_shot.section.nodes.iter().zip(&paged) {
            assert!(matches!(
                selected.selection_reasons.as_slice(),
                [crate::recall::RecallSelectionReason::FtsBm25 { rank }]
                    if rank.total_cmp(&result.rank).is_eq()
            ));
        }
    }

    #[test]
    fn operational_recall_revision_changes_for_canonical_memory_mutation() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for revision proof");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let before = operational_recall_revision(&connection).expect("revision should build");
        let node = create_node(
            &connection,
            &task_recall_node("workflow", "draft", "Revision", "first body"),
        )
        .expect("node should create");
        let after_create =
            operational_recall_revision(&connection).expect("revision should rebuild");
        create_alias(
            &connection,
            &NewAlias {
                node_id: node.id,
                alias: "revision alias".to_string(),
            },
        )
        .expect("alias should create");
        let after_alias =
            operational_recall_revision(&connection).expect("revision should rebuild");

        assert_ne!(before, after_create);
        assert_ne!(after_create, after_alias);
        assert_eq!(before.len(), 32);
        assert!(before.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn bounded_recall_fts_limits_results_and_omits_large_content() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for bounded recall");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let long_text = "x".repeat(1_100);
        let large_body = "body ".repeat(16_000);

        for index in 0..3 {
            create_node(
                &connection,
                &NewNode {
                    node_type: "raw_note".to_string(),
                    status: "draft".to_string(),
                    title: format!("boundedneedle {index} {long_text}"),
                    summary: Some(long_text.clone()),
                    body: Some(large_body.clone()),
                    source_ref: Some(long_text.clone()),
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("active bounded recall node should create");
        }
        let deprecated = create_node(
            &connection,
            &NewNode {
                node_type: "raw_note".to_string(),
                status: "deprecated".to_string(),
                title: "boundedneedle deprecated".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("deprecated bounded recall node should create");

        let search = search_recall_query_fts(&connection, "boundedneedle", 2)
            .expect("bounded recall query should pass");
        let punctuation = search_recall_query_fts(&connection, "!!!", 2)
            .expect("punctuation-only bounded recall query should pass");

        assert_eq!(search.results.len(), 2);
        assert!(search.more_results);
        assert!(search.content_truncated);
        assert!(search
            .results
            .iter()
            .all(|result| result.node.body.is_none()));
        assert!(search.results.iter().all(|result| {
            result.node.title.chars().count() <= BOUNDED_RECALL_FIELD_MAX_CHARS as usize
                && result.node.summary.as_ref().is_none_or(|summary| {
                    summary.chars().count() <= BOUNDED_RECALL_FIELD_MAX_CHARS as usize
                })
                && result.node.source_ref.as_ref().is_none_or(|source_ref| {
                    source_ref.chars().count() <= BOUNDED_RECALL_FIELD_MAX_CHARS as usize
                })
        }));
        assert!(search
            .results
            .iter()
            .all(|result| result.node.id != deprecated.id));
        assert!(punctuation.results.is_empty());
        assert!(!punctuation.more_results);
        assert!(!punctuation.content_truncated);
    }

    #[test]
    fn mandatory_recall_loader_returns_only_active_exact_types_in_stable_order() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for mandatory recall");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let full_body = "mandatory-body-".repeat(400);

        for (node_type, status, title) in [
            ("rule", "active", "Active rule"),
            ("gate", "active", "Active gate"),
            ("project_profile", "active", "Active project"),
            ("kernel_contract", "active", "Active kernel"),
            ("source", "active", "Active source"),
            ("gate", "draft", "Draft gate"),
            ("rule", "deprecated", "Deprecated rule"),
            ("project_profile", "superseded", "Superseded project"),
            ("source", "broken", "Broken source"),
            ("workflow", "active", "Active workflow"),
        ] {
            create_node(
                &connection,
                &NewNode {
                    node_type: node_type.to_string(),
                    status: status.to_string(),
                    title: title.to_string(),
                    summary: None,
                    body: Some(full_body.clone()),
                    source_ref: (status == "active").then(|| "user:test".to_string()),
                    confidence: (status == "active").then_some(1.0),
                    trust_level: (status == "active").then(|| "high".to_string()),
                },
            )
            .expect("mandatory loader fixture should create");
        }

        let nodes =
            load_active_mandatory_recall_nodes(&connection).expect("mandatory nodes should load");

        assert_eq!(
            nodes
                .iter()
                .map(|node| node.node_type.as_str())
                .collect::<Vec<_>>(),
            [
                "kernel_contract",
                "gate",
                "project_profile",
                "source",
                "rule"
            ]
        );
        assert!(nodes.iter().all(|node| node.status == "active"));
        assert!(nodes
            .iter()
            .all(|node| node.body.as_deref() == Some(full_body.as_str())));
    }

    #[test]
    fn bounded_legacy_recall_caps_roots_links_and_node_content() {
        let mut connection = Connection::open_in_memory()
            .expect("in-memory DB should open for bounded legacy recall");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let long_text = "x".repeat(1_100);
        let mut workflow_ids = Vec::new();

        for index in 0..=LEGACY_RECALL_ROOT_LIMIT_PER_TYPE {
            let node = create_node(
                &connection,
                &NewNode {
                    node_type: "workflow".to_string(),
                    status: "draft".to_string(),
                    title: format!("Workflow {index} {long_text}"),
                    summary: Some(long_text.clone()),
                    body: Some(long_text.clone()),
                    source_ref: Some(long_text.clone()),
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("workflow should create");
            workflow_ids.push(node.id);
        }

        create_node(
            &connection,
            &NewNode {
                node_type: "workflow".to_string(),
                status: "deprecated".to_string(),
                ..draft_node("Deprecated root")
            },
        )
        .expect("deprecated root should create");
        let deprecated_target = create_node(
            &connection,
            &NewNode {
                status: "deprecated".to_string(),
                ..draft_node("Deprecated target")
            },
        )
        .expect("deprecated target should create");
        create_link(
            &connection,
            &NewLink {
                source_node_id: workflow_ids[0],
                target_node_id: deprecated_target.id,
                link_type: "supports".to_string(),
            },
        )
        .expect("deprecated target link should create");

        for index in 0..=LEGACY_RECALL_LINK_LIMIT {
            let target = create_node(
                &connection,
                &NewNode {
                    node_type: "raw_note".to_string(),
                    status: "draft".to_string(),
                    title: format!("Target {index}"),
                    summary: None,
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("target should create");
            create_link(
                &connection,
                &NewLink {
                    source_node_id: workflow_ids[0],
                    target_node_id: target.id,
                    link_type: "supports".to_string(),
                },
            )
            .expect("bounded recall link should create");
        }

        let recall =
            load_bounded_legacy_recall(&connection).expect("bounded legacy recall should load");
        let workflow_count = recall
            .nodes
            .iter()
            .filter(|node| node.node_type == "workflow")
            .count();

        assert_eq!(workflow_count, LEGACY_RECALL_ROOT_LIMIT_PER_TYPE);
        assert_eq!(recall.links.len(), LEGACY_RECALL_LINK_LIMIT);
        assert!(recall.more_results);
        assert!(recall.content_truncated);
        assert!(recall
            .nodes
            .iter()
            .all(|node| { !matches!(node.status.as_str(), "deprecated" | "superseded") }));
        assert!(recall
            .links
            .iter()
            .all(|link| link.target_node_id != deprecated_target.id));
        assert!(!recall
            .nodes
            .iter()
            .any(|node| node.id == workflow_ids[LEGACY_RECALL_ROOT_LIMIT_PER_TYPE]));
        assert!(
            recall.nodes.len()
                <= LEGACY_RECALL_ROOT_TYPES.len() * LEGACY_RECALL_ROOT_LIMIT_PER_TYPE
                    + LEGACY_RECALL_LINK_LIMIT
        );
        assert!(recall.nodes.iter().all(|node| {
            node.title.chars().count() <= BOUNDED_RECALL_FIELD_MAX_CHARS as usize
                && node.summary.as_ref().is_none_or(|summary| {
                    summary.chars().count() <= BOUNDED_RECALL_FIELD_MAX_CHARS as usize
                })
                && node.body.as_ref().is_none_or(|body| {
                    body.chars().count() <= BOUNDED_RECALL_FIELD_MAX_CHARS as usize
                })
                && node.source_ref.as_ref().is_none_or(|source_ref| {
                    source_ref.chars().count() <= BOUNDED_RECALL_FIELD_MAX_CHARS as usize
                })
        }));
    }

    #[test]
    fn bounded_legacy_recall_loads_only_two_link_hops() {
        let mut connection = Connection::open_in_memory()
            .expect("in-memory DB should open for bounded legacy traversal");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let root = create_node(
            &connection,
            &NewNode {
                node_type: "workflow".to_string(),
                ..draft_node("Root")
            },
        )
        .expect("root should create");
        let depth_one = create_node(
            &connection,
            &NewNode {
                node_type: "decision".to_string(),
                ..draft_node("Depth one")
            },
        )
        .expect("depth-one node should create");
        let depth_two = create_node(
            &connection,
            &NewNode {
                node_type: "lesson".to_string(),
                ..draft_node("Depth two")
            },
        )
        .expect("depth-two node should create");
        let too_deep = create_node(
            &connection,
            &NewNode {
                node_type: "project_fact".to_string(),
                ..draft_node("Too deep")
            },
        )
        .expect("too-deep node should create");

        for (source_node_id, target_node_id) in [
            (root.id, depth_one.id),
            (depth_one.id, depth_two.id),
            (depth_two.id, too_deep.id),
        ] {
            create_link(
                &connection,
                &NewLink {
                    source_node_id,
                    target_node_id,
                    link_type: "supports".to_string(),
                },
            )
            .expect("traversal link should create");
        }

        let recall =
            load_bounded_legacy_recall(&connection).expect("bounded legacy recall should load");

        assert_eq!(recall.links.len(), 2);
        assert!(recall.nodes.iter().any(|node| node.id == root.id));
        assert!(recall.nodes.iter().any(|node| node.id == depth_one.id));
        assert!(recall.nodes.iter().any(|node| node.id == depth_two.id));
        assert!(!recall.nodes.iter().any(|node| node.id == too_deep.id));
    }
}
