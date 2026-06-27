use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit;
use crate::audit::AuditError;
use crate::schema;

const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";
const HOME_ENV: &str = "HOME";
const FNV1A_32_OFFSET: u32 = 0x811c9dc5;
const FNV1A_32_PRIME: u32 = 0x01000193;
const STORAGE_AUDIT_SOURCE: &str = "aopmem_cli";
const TEACH_SESSION_SUMMARY: &str = "teach_session_v1";
const TEACH_MATERIAL_SUMMARY: &str = "teach_material_v1";
const TEACH_PROPOSAL_SUMMARY: &str = "teach_proposal_v1";
const TEACH_APPLY_SUMMARY: &str = "teach_apply_v1";
const TEACH_MATERIAL_LINK_TYPE: &str = "teach_has_material";
const TEACH_PROPOSAL_LINK_TYPE: &str = "teach_has_proposal";
const TEACH_APPLY_LINK_TYPE: &str = "teach_has_apply";
const TEACH_CREATED_LINK_TYPE: &str = "teach_created_node";

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
}

impl fmt::Display for PathResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHome => write!(formatter, "HOME is required when AOPMEM_HOME is not set"),
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

#[derive(Debug, Clone, PartialEq)]
pub enum NodeValidationError {
    InvalidType(String),
    InvalidStatus(String),
    MissingTitle,
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
    SourceNodeNotFound(i64),
    TargetNodeNotFound(i64),
}

impl fmt::Display for LinkValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingType => write!(formatter, "missing required field: link_type"),
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
}

impl fmt::Display for MetadataValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(formatter, "node not found: {id}"),
            Self::MissingAlias => write!(formatter, "missing required field: alias"),
            Self::MissingTag => write!(formatter, "missing required field: tag"),
            Self::MissingSourceRef => write!(formatter, "missing required field: source_ref"),
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
    resolve_paths_from_env(env::var_os(AOPMEM_HOME_ENV), env::var_os(HOME_ENV))
}

pub fn workspace_key(repo_root: impl AsRef<Path>) -> Result<String, WorkspaceKeyError> {
    let repo_root = repo_root.as_ref();
    if !repo_root.is_absolute() {
        return Err(WorkspaceKeyError::RelativeRepoRoot);
    }

    let folder_name = repo_root
        .file_name()
        .ok_or(WorkspaceKeyError::MissingRepoFolderName)?;
    let sanitized = sanitize_repo_folder_name(&folder_name.to_string_lossy());
    let path_hash = hash_absolute_path(repo_root);

    Ok(format!("{sanitized}-{path_hash:08x}"))
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

    fs::create_dir_all(workspace_paths.root())?;
    fs::create_dir_all(workspace_paths.tools())?;
    fs::create_dir_all(workspace_paths.artifacts())?;
    fs::create_dir_all(workspace_paths.audit_git())?;
    fs::create_dir_all(workspace_paths.runtimes())?;
    fs::create_dir_all(workspace_paths.logs())?;

    Ok(workspace_paths)
}

pub fn open_workspace_db(workspace_paths: &WorkspacePaths) -> rusqlite::Result<Connection> {
    let mut connection = Connection::open(workspace_paths.db())?;
    apply_connection_pragmas(&connection)?;
    schema::apply_migrations(&mut connection)?;

    Ok(connection)
}

pub fn create_node(connection: &Connection, node: &NewNode) -> Result<Node, NodeStorageError> {
    validate_new_node(node)?;

    connection.execute(
        "
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
            ",
        params![
            &node.node_type,
            &node.status,
            &node.title,
            &node.summary,
            &node.body,
            &node.source_ref,
            node.confidence,
            &node.trust_level
        ],
    )?;

    let id = connection.last_insert_rowid();
    let created = get_node(connection, id)?
        .ok_or(NodeStorageError::Db(rusqlite::Error::QueryReturnedNoRows))?;
    refresh_fts_node(connection, created.id)?;
    audit::record_node_created(connection, created.id, STORAGE_AUDIT_SOURCE)
        .map_err(audit_error_to_db)?;

    Ok(created)
}

pub fn update_node(
    connection: &Connection,
    update: &NodeUpdate,
) -> Result<Option<Node>, NodeStorageError> {
    let existing = match get_node(connection, update.id)? {
        Some(node) => node,
        None => return Ok(None),
    };
    validate_new_node(&NewNode {
        node_type: existing.node_type,
        status: update.status.clone(),
        title: update.title.clone(),
        summary: update.summary.clone(),
        body: update.body.clone(),
        source_ref: update.source_ref.clone(),
        confidence: update.confidence,
        trust_level: update.trust_level.clone(),
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
    connection
        .query_row(
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
            WHERE id = ?1;
            ",
            [id],
            row_to_node,
        )
        .optional()
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

pub fn create_link(connection: &Connection, link: &NewLink) -> Result<Link, LinkStorageError> {
    validate_new_link(connection, link)?;

    connection.execute(
        "
            INSERT INTO links (
                source_node_id,
                target_node_id,
                link_type
            )
            VALUES (?1, ?2, ?3);
            ",
        params![link.source_node_id, link.target_node_id, &link.link_type],
    )?;

    let id = connection.last_insert_rowid();
    let created = get_link(connection, id)?
        .ok_or(LinkStorageError::Db(rusqlite::Error::QueryReturnedNoRows))?;
    audit::record_link_created(connection, created.id, STORAGE_AUDIT_SOURCE)
        .map_err(audit_error_to_link_db)?;

    Ok(created)
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

pub fn create_alias(
    connection: &Connection,
    alias: &NewAlias,
) -> Result<Alias, MetadataStorageError> {
    validate_node_metadata(connection, alias.node_id, &alias.alias, MetadataKind::Alias)?;

    connection.execute(
        "
            INSERT INTO aliases (node_id, alias)
            VALUES (?1, ?2);
            ",
        params![alias.node_id, &alias.alias],
    )?;

    let id = connection.last_insert_rowid();
    let created = get_alias(connection, id)?.ok_or(MetadataStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))?;
    refresh_fts_node(connection, created.node_id)?;

    Ok(created)
}

pub fn list_aliases(connection: &Connection, node_id: Option<i64>) -> rusqlite::Result<Vec<Alias>> {
    let sql = "
        SELECT id, node_id, alias, created_at
        FROM aliases
        WHERE (?1 IS NULL OR node_id = ?1)
        ORDER BY id ASC;
    ";
    let mut statement = connection.prepare(sql)?;

    let aliases = statement.query_map([node_id], row_to_alias)?.collect();
    aliases
}

pub fn create_tag(connection: &Connection, tag: &NewTag) -> Result<Tag, MetadataStorageError> {
    validate_node_metadata(connection, tag.node_id, &tag.tag, MetadataKind::Tag)?;

    connection.execute(
        "
            INSERT INTO tags (node_id, tag)
            VALUES (?1, ?2);
            ",
        params![tag.node_id, &tag.tag],
    )?;

    let id = connection.last_insert_rowid();
    get_tag(connection, id)?.ok_or(MetadataStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))
}

pub fn list_tags(connection: &Connection, node_id: Option<i64>) -> rusqlite::Result<Vec<Tag>> {
    let sql = "
        SELECT id, node_id, tag, created_at
        FROM tags
        WHERE (?1 IS NULL OR node_id = ?1)
        ORDER BY id ASC;
    ";
    let mut statement = connection.prepare(sql)?;

    let tags = statement.query_map([node_id], row_to_tag)?.collect();
    tags
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

    connection.execute(
        "
            INSERT INTO sources (node_id, source_ref)
            VALUES (?1, ?2);
            ",
        params![source.node_id, &source.source_ref],
    )?;

    let id = connection.last_insert_rowid();
    get_source(connection, id)?.ok_or(MetadataStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))
}

pub fn list_sources(
    connection: &Connection,
    node_id: Option<i64>,
) -> rusqlite::Result<Vec<Source>> {
    let sql = "
        SELECT id, node_id, source_ref, created_at
        FROM sources
        WHERE (?1 IS NULL OR node_id = ?1)
        ORDER BY id ASC;
    ";
    let mut statement = connection.prepare(sql)?;

    let sources = statement.query_map([node_id], row_to_source)?.collect();
    sources
}

pub fn create_teach_session(
    connection: &Connection,
    session: &NewTeachSession,
) -> Result<TeachSession, TeachStorageError> {
    if session.title.trim().is_empty() {
        return Err(TeachValidationError::MissingSessionTitle.into());
    }

    let record = TeachSessionRecord {
        session_title: session.title.clone(),
        session_summary: session.summary.clone(),
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

pub fn add_teach_material(
    connection: &Connection,
    session_id: i64,
    payload: &Value,
) -> Result<TeachMaterial, TeachStorageError> {
    let session = require_teach_session(connection, session_id)?;
    let record = TeachMaterialRecord {
        session_id,
        payload: payload.clone(),
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
    let session = require_teach_session(connection, session_id)?;
    validate_teach_proposal(proposal)?;

    let record = TeachProposalRecord {
        session_id,
        items: proposal.items.clone(),
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

pub fn apply_teach_proposal(
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

    let mut resolved_node_refs = std::collections::BTreeMap::new();
    let mut created_node_ids = Vec::new();
    let mut created_alias_ids = Vec::new();
    let mut created_tag_ids = Vec::new();
    let mut created_source_ids = Vec::new();
    let mut created_link_ids = Vec::new();

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

                let created = create_node(
                    connection,
                    &NewNode {
                        node_type: node_type.clone(),
                        status: status.clone(),
                        title: title.clone(),
                        summary: summary.clone(),
                        body: body.clone(),
                        source_ref: source_ref.clone(),
                        confidence: *confidence,
                        trust_level: trust_level.clone(),
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
                let created = create_alias(
                    connection,
                    &NewAlias {
                        node_id: target_id,
                        alias: alias.clone(),
                    },
                )?;
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

    let receipt = TeachApplyReceiptRecord {
        session_id,
        proposal_id,
        created_node_ids: created_node_ids.clone(),
        created_alias_ids: created_alias_ids.clone(),
        created_tag_ids: created_tag_ids.clone(),
        created_source_ids: created_source_ids.clone(),
        created_link_ids: created_link_ids.clone(),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TeachSessionRecord {
    session_title: String,
    session_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TeachMaterialRecord {
    session_id: i64,
    payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TeachProposalRecord {
    session_id: i64,
    items: Vec<TeachProposalItem>,
}

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

    for item in &proposal.items {
        match item {
            TeachProposalItem::CreateNode { node_ref, .. } => {
                if let Some(node_ref) = node_ref {
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
        _ => Ok(()),
    }
}

fn resolve_teach_node_target(
    node_id: Option<i64>,
    node_ref: Option<&str>,
    resolved_node_refs: &std::collections::BTreeMap<String, i64>,
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

fn validate_new_node(node: &NewNode) -> Result<(), NodeValidationError> {
    if !ALLOWED_NODE_TYPES.contains(&node.node_type.as_str()) {
        return Err(NodeValidationError::InvalidType(node.node_type.clone()));
    }
    if !ALLOWED_NODE_STATUSES.contains(&node.status.as_str()) {
        return Err(NodeValidationError::InvalidStatus(node.status.clone()));
    }
    if node.title.trim().is_empty() {
        return Err(NodeValidationError::MissingTitle);
    }
    if node.status == "active" {
        if node.source_ref.as_deref().unwrap_or("").trim().is_empty() {
            return Err(NodeValidationError::MissingActiveSourceRef);
        }
        if node.confidence.is_none() {
            return Err(NodeValidationError::MissingActiveConfidence);
        }
        if node.trust_level.as_deref().unwrap_or("").trim().is_empty() {
            return Err(NodeValidationError::MissingActiveTrustLevel);
        }
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

    Ok(())
}

fn validate_new_link(connection: &Connection, link: &NewLink) -> Result<(), LinkStorageError> {
    if link.link_type.trim().is_empty() {
        return Err(LinkValidationError::MissingType.into());
    }
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

fn validate_node_metadata(
    connection: &Connection,
    node_id: i64,
    value: &str,
    kind: MetadataKind,
) -> Result<(), MetadataStorageError> {
    if get_node(connection, node_id)?.is_none() {
        return Err(MetadataValidationError::NodeNotFound(node_id).into());
    }
    if value.trim().is_empty() {
        let error = match kind {
            MetadataKind::Alias => MetadataValidationError::MissingAlias,
            MetadataKind::Tag => MetadataValidationError::MissingTag,
            MetadataKind::Source => MetadataValidationError::MissingSourceRef,
        };
        return Err(error.into());
    }

    Ok(())
}

fn refresh_fts_node(connection: &Connection, node_id: i64) -> rusqlite::Result<()> {
    let node = get_node(connection, node_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let aliases = aliases_for_fts(connection, node_id)?;

    connection.execute("DELETE FROM fts_nodes WHERE rowid = ?1;", [node_id])?;
    connection.execute(
        "
        INSERT INTO fts_nodes(rowid, title, summary, body, aliases)
        VALUES (?1, ?2, ?3, ?4, ?5);
        ",
        params![
            node.id,
            node.title,
            node.summary.unwrap_or_default(),
            node.body.unwrap_or_default(),
            aliases
        ],
    )?;

    Ok(())
}

fn aliases_for_fts(connection: &Connection, node_id: i64) -> rusqlite::Result<String> {
    connection.query_row(
        "
        SELECT COALESCE(group_concat(alias, ' '), '')
        FROM aliases
        WHERE node_id = ?1
        ORDER BY id ASC;
        ",
        [node_id],
        |row| row.get(0),
    )
}

fn fts_match_query(query: &str) -> Option<String> {
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

fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
    Ok(Node {
        id: row.get(0)?,
        node_type: row.get(1)?,
        status: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        body: row.get(5)?,
        source_ref: row.get(6)?,
        confidence: row.get(7)?,
        trust_level: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn get_link(connection: &Connection, id: i64) -> rusqlite::Result<Option<Link>> {
    connection
        .query_row(
            "
            SELECT
                id,
                source_node_id,
                target_node_id,
                link_type,
                created_at
            FROM links
            WHERE id = ?1;
            ",
            [id],
            row_to_link,
        )
        .optional()
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
    connection
        .query_row(
            "
            SELECT id, node_id, alias, created_at
            FROM aliases
            WHERE id = ?1;
            ",
            [id],
            row_to_alias,
        )
        .optional()
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
    connection
        .query_row(
            "
            SELECT id, node_id, tag, created_at
            FROM tags
            WHERE id = ?1;
            ",
            [id],
            row_to_tag,
        )
        .optional()
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
    connection
        .query_row(
            "
            SELECT id, node_id, source_ref, created_at
            FROM sources
            WHERE id = ?1;
            ",
            [id],
            row_to_source,
        )
        .optional()
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

fn resolve_paths_from_env(
    aopmem_home: Option<impl AsRef<OsStr>>,
    home: Option<impl AsRef<OsStr>>,
) -> Result<AopmemPaths, PathResolveError> {
    let root = match aopmem_home {
        Some(path) if !path.as_ref().is_empty() => PathBuf::from(path.as_ref()),
        _ => {
            let home = home.ok_or(PathResolveError::MissingHome)?;
            if home.as_ref().is_empty() {
                return Err(PathResolveError::MissingHome);
            }
            PathBuf::from(home.as_ref()).join(".aopmem")
        }
    };

    Ok(AopmemPaths {
        bin: root.join("bin"),
        skills: root.join("skills"),
        templates: root.join("templates"),
        workspaces: root.join("workspaces"),
        home: root,
    })
}

fn workspace_paths(paths: &AopmemPaths, workspace_key: &str) -> WorkspacePaths {
    let root = paths.workspaces().join(workspace_key);

    WorkspacePaths {
        db: root.join("aopmem.sqlite"),
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

fn hash_absolute_path(repo_root: &Path) -> u32 {
    let mut hash = FNV1A_32_OFFSET;
    for byte in repo_root.as_os_str().to_string_lossy().as_bytes() {
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
        assert!(workspace_paths.root().is_dir());
        assert!(workspace_paths.tools().is_dir());
        assert!(workspace_paths.artifacts().is_dir());
        assert!(workspace_paths.audit_git().is_dir());
        assert!(workspace_paths.runtimes().is_dir());
        assert!(workspace_paths.logs().is_dir());
        assert!(!workspace_paths.root().join("aopmem.sqlite").exists());

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
        assert_eq!(migration_count, 1);

        drop(connection);
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

        assert_eq!(migration_count, 1);

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

        assert_eq!(alias.node_id, node.id);
        assert_eq!(alias.alias, "nickname");
        assert_eq!(tag.node_id, node.id);
        assert_eq!(tag.tag, "storage");
        assert_eq!(source.node_id, node.id);
        assert_eq!(source.source_ref, "source=user_instruction");
        assert_eq!(
            list_aliases(&connection, Some(node.id)).expect("alias list should pass"),
            vec![alias]
        );
        assert_eq!(
            list_tags(&connection, Some(node.id)).expect("tag list should pass"),
            vec![tag]
        );
        assert_eq!(
            list_sources(&connection, Some(node.id)).expect("source list should pass"),
            vec![source]
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
}
