use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;
use thiserror::Error;

use crate::adapter;
use crate::audit;
use crate::storage;

const REQUIRED_SCHEMA_TABLES: &[&str] = &[
    "nodes",
    "links",
    "aliases",
    "tags",
    "sources",
    "events",
    "registries",
    "tool_contracts",
    "tool_aliases",
    "mcp_profiles",
];
const REQUIRED_SCHEMA_MIGRATION_VERSION: &str = "004";
const REQUIRED_SCHEMA_MIGRATION_NAME: &str = "004_task_protocol_and_tool_aliases";
const REQUIRED_FTS_TABLE: &str = "fts_nodes";
const REQUIRED_FTS_COLUMNS: &[&str] = &["title", "summary", "body", "aliases"];
const MIN_REQUIRED_ACTIVE_GATES: usize = 1;
const CODE_SCAN_ROOTS: &[&str] = &["src", "tests/cli"];
const CODE_SCAN_FILE_EXTENSIONS: &[&str] = &["rs"];
const FORBIDDEN_FEATURE_TERMS: &[&str] = &[
    "mem0",
    "hindsight",
    "semantic search",
    "vector search",
    "embeddings",
    "qdrant",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Ready,
    Missing,
    Error,
}

impl DoctorStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Error => "error",
        }
    }

    fn from_present(present: bool) -> Self {
        if present {
            Self::Ready
        } else {
            Self::Missing
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub healthy: bool,
    pub repo_root: String,
    pub workspace_key: String,
    pub checks: DoctorChecks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorChecks {
    pub global_dirs: GlobalDirsHealth,
    pub workspace: PathHealth,
    pub db: DbHealth,
    pub schema: SchemaHealth,
    pub fts: FtsHealth,
    pub adapter_block: AdapterBlockHealth,
    pub artifacts_dirs: ArtifactsDirsHealth,
    pub audit_snapshot: AuditSnapshotHealth,
    pub tools_dirs: PathHealth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GlobalDirsHealth {
    pub status: DoctorStatus,
    pub home: PathHealth,
    pub bin: PathHealth,
    pub skills: PathHealth,
    pub templates: PathHealth,
    pub workspaces: PathHealth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathHealth {
    pub status: DoctorStatus,
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DbHealth {
    pub status: DoctorStatus,
    pub path: String,
    pub exists: bool,
    pub open_read_only: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaHealth {
    pub status: DoctorStatus,
    pub schema_migrations: DoctorStatus,
    pub init_migration: DoctorStatus,
    pub required_tables: Vec<NamedHealth>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FtsHealth {
    pub status: DoctorStatus,
    pub table: DoctorStatus,
    pub required_columns: Vec<NamedHealth>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamedHealth {
    pub name: String,
    pub status: DoctorStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterBlockHealth {
    pub status: DoctorStatus,
    pub instruction_file: String,
    pub file_exists: bool,
    pub managed_block: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactsDirsHealth {
    pub status: DoctorStatus,
    pub artifacts: PathHealth,
    pub audit_git: PathHealth,
    pub runtimes: PathHealth,
    pub logs: PathHealth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditSnapshotHealth {
    pub status: DoctorStatus,
    pub pending: bool,
    pub marker_path: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LintReport {
    pub clean: bool,
    pub repo_root: String,
    pub workspace_key: String,
    pub summary: LintSummary,
    pub issues: Vec<LintIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct LintSummary {
    pub total: usize,
    pub duplicate_ids: usize,
    pub broken_links: usize,
    pub deprecated_active_links: usize,
    pub missing_source: usize,
    pub missing_summary: usize,
    pub missing_gates: usize,
    pub adapter_block_drift: usize,
    pub schema_drift: usize,
    pub forbidden_feature_terms: usize,
    pub pending_audit_snapshot: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LintIssue {
    pub kind: LintIssueKind,
    pub subject: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LintIssueKind {
    DuplicateId,
    BrokenLink,
    DeprecatedActiveLink,
    MissingSource,
    MissingSummary,
    MissingGates,
    AdapterBlockDrift,
    SchemaDrift,
    ForbiddenFeatureTerm,
    PendingAuditSnapshot,
}

#[derive(Debug)]
struct LintNode {
    id: i64,
    node_type: String,
    status: String,
    summary: Option<String>,
    source_ref: Option<String>,
}

#[derive(Debug, Error)]
pub enum DoctorError {
    #[error(transparent)]
    Path(#[from] storage::PathResolveError),
    #[error(transparent)]
    WorkspaceResolve(#[from] storage::WorkspaceResolveError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum LintError {
    #[error(transparent)]
    Path(#[from] storage::PathResolveError),
    #[error(transparent)]
    WorkspaceResolve(#[from] storage::WorkspaceResolveError),
    #[error("workspace database not found: {0}")]
    WorkspaceDbMissing(String),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn run_doctor(repo_root: &Path) -> Result<DoctorReport, DoctorError> {
    let repo_root = repo_root.canonicalize()?;
    let paths = storage::resolve_paths()?;
    let workspace_key = storage::resolve_workspace_key(&paths, &repo_root)?;
    let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
    let workspace_root = paths.workspaces().join(&workspace_key);
    let tools_path = workspace_root.join("tools");
    let artifacts_path = workspace_root.join("artifacts");
    let audit_git_path = workspace_root.join("audit-git");
    let runtimes_path = workspace_root.join("runtimes");
    let logs_path = workspace_root.join("logs");
    let instruction_file = adapter::default_instruction_file(&repo_root);

    let global_dirs = inspect_global_dirs(&paths);
    let workspace = inspect_path(&workspace_root);
    let tools_dirs = inspect_path(&tools_path);
    let artifacts_dirs =
        inspect_artifacts_dirs(&artifacts_path, &audit_git_path, &runtimes_path, &logs_path);
    let audit_snapshot = inspect_audit_snapshot(&audit_git_path);
    let (db, schema, fts) = inspect_database(&workspace_paths);
    let adapter_block = inspect_adapter_block(&instruction_file);

    let healthy = [
        global_dirs.status,
        workspace.status,
        db.status,
        schema.status,
        fts.status,
        adapter_block.status,
        artifacts_dirs.status,
        audit_snapshot.status,
        tools_dirs.status,
    ]
    .into_iter()
    .all(|status| status == DoctorStatus::Ready);

    Ok(DoctorReport {
        healthy,
        repo_root: path_string(&repo_root),
        workspace_key,
        checks: DoctorChecks {
            global_dirs,
            workspace,
            db,
            schema,
            fts,
            adapter_block,
            artifacts_dirs,
            audit_snapshot,
            tools_dirs,
        },
    })
}

pub fn run_lint(repo_root: &Path) -> Result<LintReport, LintError> {
    let repo_root = repo_root.canonicalize()?;
    let paths = storage::resolve_paths()?;
    let workspace_key = storage::resolve_workspace_key(&paths, &repo_root)?;
    let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
    let workspace_root = workspace_paths.root();
    let audit_git_path = workspace_root.join("audit-git");
    let connection = match storage::open_workspace_db_read_only(&workspace_paths) {
        Ok(connection) => connection,
        Err(storage::OpenWorkspaceReadOnlyError::Missing(path)) => {
            return Err(LintError::WorkspaceDbMissing(path_string(&path)));
        }
        Err(storage::OpenWorkspaceReadOnlyError::UnsafePath(error)) => {
            return Err(LintError::Io(error));
        }
        Err(storage::OpenWorkspaceReadOnlyError::Db(error)) => return Err(LintError::Db(error)),
    };
    let nodes = list_lint_nodes(&connection)?;
    let links = storage::list_links(&connection)?;
    let node_map = nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<BTreeMap<_, _>>();
    let mut issues = Vec::new();

    issues.extend(find_duplicate_id_issues(
        &connection,
        "tool_contracts",
        "tool_id",
        "tool_contract",
    )?);
    issues.extend(find_duplicate_id_issues(
        &connection,
        "mcp_profiles",
        "id",
        "mcp_profile",
    )?);
    issues.extend(find_duplicate_id_issues(
        &connection,
        "registries",
        "registry_type || ':' || name",
        "registry",
    )?);
    issues.extend(find_broken_link_issues(&links, &node_map));
    issues.extend(find_deprecated_active_link_issues(&links, &node_map));
    issues.extend(find_missing_source_issues(&nodes));
    issues.extend(find_missing_summary_issues(&nodes));
    issues.extend(find_missing_gate_issues(&nodes));
    issues.extend(find_adapter_block_drift_issues(&repo_root));
    issues.extend(find_schema_drift_issues(&connection));
    issues.extend(find_pending_audit_snapshot_issues(&audit_git_path)?);
    issues.extend(find_forbidden_feature_term_issues(&repo_root)?);

    let summary = summarize_lint_issues(&issues);

    Ok(LintReport {
        clean: issues.is_empty(),
        repo_root: path_string(&repo_root),
        workspace_key,
        summary,
        issues,
    })
}

fn list_lint_nodes(connection: &Connection) -> rusqlite::Result<Vec<LintNode>> {
    let mut statement = connection.prepare(
        "
        SELECT id, node_type, status, summary, source_ref
        FROM nodes
        ORDER BY id ASC;
        ",
    )?;
    let nodes = statement
        .query_map([], |row| {
            Ok(LintNode {
                id: row.get(0)?,
                node_type: row.get(1)?,
                status: row.get(2)?,
                summary: row.get(3)?,
                source_ref: row.get(4)?,
            })
        })?
        .collect();

    nodes
}

fn inspect_global_dirs(paths: &storage::AopmemPaths) -> GlobalDirsHealth {
    let home = inspect_path(paths.home());
    let bin = inspect_path(paths.bin());
    let skills = inspect_path(paths.skills());
    let templates = inspect_path(paths.templates());
    let workspaces = inspect_path(paths.workspaces());
    let status = combine_statuses(&[
        home.status,
        bin.status,
        skills.status,
        templates.status,
        workspaces.status,
    ]);

    GlobalDirsHealth {
        status,
        home,
        bin,
        skills,
        templates,
        workspaces,
    }
}

fn find_duplicate_id_issues(
    connection: &Connection,
    table: &str,
    column_expression: &str,
    label: &str,
) -> Result<Vec<LintIssue>, rusqlite::Error> {
    let query = format!(
        "SELECT {column_expression}, COUNT(*) \
         FROM {table} \
         GROUP BY {column_expression} \
         HAVING COUNT(*) > 1 \
         ORDER BY {column_expression};"
    );
    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut issues = Vec::new();

    for row in rows {
        let (duplicate_id, count) = row?;
        issues.push(LintIssue {
            kind: LintIssueKind::DuplicateId,
            subject: format!("{label}:{duplicate_id}"),
            message: format!("duplicate {label} id found: {duplicate_id} ({count} rows)"),
        });
    }

    Ok(issues)
}

fn find_broken_link_issues(
    links: &[storage::Link],
    node_map: &BTreeMap<i64, &LintNode>,
) -> Vec<LintIssue> {
    links
        .iter()
        .filter_map(|link| {
            let missing_source = !node_map.contains_key(&link.source_node_id);
            let missing_target = !node_map.contains_key(&link.target_node_id);
            if !missing_source && !missing_target {
                return None;
            }

            let missing = match (missing_source, missing_target) {
                (true, true) => "source and target nodes are missing",
                (true, false) => "source node is missing",
                (false, true) => "target node is missing",
                (false, false) => unreachable!(),
            };

            Some(LintIssue {
                kind: LintIssueKind::BrokenLink,
                subject: format!("link:{}", link.id),
                message: format!(
                    "broken link found: id={}, type={}, {}",
                    link.id, link.link_type, missing
                ),
            })
        })
        .collect()
}

fn find_deprecated_active_link_issues(
    links: &[storage::Link],
    node_map: &BTreeMap<i64, &LintNode>,
) -> Vec<LintIssue> {
    let mut seen = BTreeSet::new();
    let mut issues = Vec::new();

    for link in links {
        let Some(source) = node_map.get(&link.source_node_id) else {
            continue;
        };
        let Some(target) = node_map.get(&link.target_node_id) else {
            continue;
        };

        for (active, other) in [(source, target), (target, source)] {
            if active.status != "active" || !is_inactive_for_lint(&other.status) {
                continue;
            }

            let subject = format!("link:{}:{}->{}", link.id, active.id, other.id);
            if seen.insert(subject.clone()) {
                issues.push(LintIssue {
                    kind: LintIssueKind::DeprecatedActiveLink,
                    subject,
                    message: format!(
                        "active node {} links to {} node {} through {}",
                        active.id, other.status, other.id, link.link_type
                    ),
                });
            }
        }
    }

    issues
}

fn find_missing_source_issues(nodes: &[LintNode]) -> Vec<LintIssue> {
    nodes
        .iter()
        .filter(|node| node.status == "active" && is_blank(node.source_ref.as_deref()))
        .map(|node| LintIssue {
            kind: LintIssueKind::MissingSource,
            subject: format!("node:{}", node.id),
            message: format!("active node is missing source_ref: {}", node.id),
        })
        .collect()
}

fn find_missing_summary_issues(nodes: &[LintNode]) -> Vec<LintIssue> {
    nodes
        .iter()
        .filter(|node| node.status == "active" && is_blank(node.summary.as_deref()))
        .map(|node| LintIssue {
            kind: LintIssueKind::MissingSummary,
            subject: format!("node:{}", node.id),
            message: format!("active node is missing summary: {}", node.id),
        })
        .collect()
}

fn find_missing_gate_issues(nodes: &[LintNode]) -> Vec<LintIssue> {
    let active_gate_count = nodes
        .iter()
        .filter(|node| node.node_type == "gate" && node.status == "active")
        .count();

    if active_gate_count >= MIN_REQUIRED_ACTIVE_GATES {
        Vec::new()
    } else {
        vec![LintIssue {
            kind: LintIssueKind::MissingGates,
            subject: "workspace:gates".to_string(),
            message: format!(
                "workspace is missing active gates: expected at least {MIN_REQUIRED_ACTIVE_GATES}, found {active_gate_count}"
            ),
        }]
    }
}

fn summarize_lint_issues(issues: &[LintIssue]) -> LintSummary {
    let mut summary = LintSummary {
        total: issues.len(),
        ..LintSummary::default()
    };

    for issue in issues {
        match issue.kind {
            LintIssueKind::DuplicateId => summary.duplicate_ids += 1,
            LintIssueKind::BrokenLink => summary.broken_links += 1,
            LintIssueKind::DeprecatedActiveLink => summary.deprecated_active_links += 1,
            LintIssueKind::MissingSource => summary.missing_source += 1,
            LintIssueKind::MissingSummary => summary.missing_summary += 1,
            LintIssueKind::MissingGates => summary.missing_gates += 1,
            LintIssueKind::AdapterBlockDrift => summary.adapter_block_drift += 1,
            LintIssueKind::SchemaDrift => summary.schema_drift += 1,
            LintIssueKind::ForbiddenFeatureTerm => summary.forbidden_feature_terms += 1,
            LintIssueKind::PendingAuditSnapshot => summary.pending_audit_snapshot += 1,
        }
    }

    summary
}

fn find_pending_audit_snapshot_issues(
    audit_git_dir: &Path,
) -> Result<Vec<LintIssue>, std::io::Error> {
    if !audit::has_pending_snapshot(audit_git_dir)? {
        return Ok(Vec::new());
    }

    let marker_path = audit_git_dir.join(audit::PENDING_SNAPSHOT_MARKER_FILE_NAME);
    Ok(vec![LintIssue {
        kind: LintIssueKind::PendingAuditSnapshot,
        subject: format!("audit_snapshot:{}", path_string(&marker_path)),
        message: format!(
            "audit snapshot is pending: marker exists at {}; fix: aopmem audit repair --current-workspace --json",
            path_string(&marker_path)
        ),
    }])
}

fn find_adapter_block_drift_issues(repo_root: &Path) -> Vec<LintIssue> {
    let instruction_file = adapter::default_instruction_file(repo_root);
    match adapter::instruction_file_status(&instruction_file) {
        Ok(status) => match status.managed_block {
            adapter::ManagedBlockStatus::Drifted => vec![LintIssue {
                kind: LintIssueKind::AdapterBlockDrift,
                subject: format!("adapter:{}", path_string(&status.instruction_file)),
                message: format!(
                    "adapter managed block drift detected in {}",
                    path_string(&status.instruction_file)
                ),
            }],
            adapter::ManagedBlockStatus::Missing | adapter::ManagedBlockStatus::InSync => {
                Vec::new()
            }
        },
        Err(error) => vec![LintIssue {
            kind: LintIssueKind::AdapterBlockDrift,
            subject: format!("adapter:{}", path_string(&instruction_file)),
            message: format!(
                "adapter managed block is damaged in {}: {}",
                path_string(&instruction_file),
                error
            ),
        }],
    }
}

fn find_schema_drift_issues(connection: &Connection) -> Vec<LintIssue> {
    let mut issues = Vec::new();

    push_schema_status_issue(
        &mut issues,
        "schema_migrations",
        table_status(connection, "schema_migrations"),
        "schema_migrations table is missing or unreadable",
    );
    push_schema_status_issue(
        &mut issues,
        &format!("schema_migrations:{REQUIRED_SCHEMA_MIGRATION_NAME}"),
        migration_status(
            connection,
            REQUIRED_SCHEMA_MIGRATION_VERSION,
            REQUIRED_SCHEMA_MIGRATION_NAME,
        ),
        "required schema migration marker is missing or unreadable",
    );

    for table in REQUIRED_SCHEMA_TABLES {
        push_schema_status_issue(
            &mut issues,
            &format!("table:{table}"),
            table_status(connection, table),
            &format!("required table is missing or unreadable: {table}"),
        );
    }

    let fts_table_status = table_status(connection, REQUIRED_FTS_TABLE);
    push_schema_status_issue(
        &mut issues,
        &format!("table:{REQUIRED_FTS_TABLE}"),
        fts_table_status,
        &format!("required fts table is missing or unreadable: {REQUIRED_FTS_TABLE}"),
    );
    for column in REQUIRED_FTS_COLUMNS {
        push_schema_status_issue(
            &mut issues,
            &format!("fts_column:{column}"),
            fts_column_status(connection, column, fts_table_status),
            &format!("required fts column is missing or unreadable: {column}"),
        );
    }

    issues
}

fn push_schema_status_issue(
    issues: &mut Vec<LintIssue>,
    subject: &str,
    status: DoctorStatus,
    message: &str,
) {
    if status == DoctorStatus::Ready {
        return;
    }

    issues.push(LintIssue {
        kind: LintIssueKind::SchemaDrift,
        subject: format!("schema:{subject}"),
        message: message.to_string(),
    });
}

fn find_forbidden_feature_term_issues(repo_root: &Path) -> Result<Vec<LintIssue>, std::io::Error> {
    let mut issues = Vec::new();

    for root in CODE_SCAN_ROOTS {
        let scan_root = repo_root.join(root);
        if !scan_root.exists() {
            continue;
        }

        scan_code_tree(repo_root, &scan_root, &mut issues)?;
    }

    Ok(issues)
}

fn scan_code_tree(
    repo_root: &Path,
    path: &Path,
    issues: &mut Vec<LintIssue>,
) -> Result<(), std::io::Error> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            scan_code_tree(repo_root, &entry.path(), issues)?;
        }
        return Ok(());
    }

    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return Ok(());
    };
    if !CODE_SCAN_FILE_EXTENSIONS.contains(&extension) {
        return Ok(());
    }

    let content = fs::read_to_string(path)?;
    let lowercase = content.to_lowercase();
    let relative_path = path
        .strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned();

    for term in FORBIDDEN_FEATURE_TERMS {
        if lowercase.contains(term) {
            issues.push(LintIssue {
                kind: LintIssueKind::ForbiddenFeatureTerm,
                subject: format!("file:{relative_path}"),
                message: format!(
                    "forbidden feature term found in code path: term={term}, path={relative_path}"
                ),
            });
        }
    }

    Ok(())
}

fn is_blank(value: Option<&str>) -> bool {
    value.unwrap_or("").trim().is_empty()
}

fn is_inactive_for_lint(status: &str) -> bool {
    matches!(status, "deprecated" | "superseded" | "broken")
}

fn inspect_path(path: &Path) -> PathHealth {
    let exists = path.is_dir();

    PathHealth {
        status: DoctorStatus::from_present(exists),
        path: path_string(path),
        exists,
    }
}

fn inspect_database(
    workspace_paths: &storage::WorkspacePaths,
) -> (DbHealth, SchemaHealth, FtsHealth) {
    let path = workspace_paths.db();
    match storage::open_workspace_db_read_only(workspace_paths) {
        Ok(connection) => (
            DbHealth {
                status: DoctorStatus::Ready,
                path: path_string(path),
                exists: true,
                open_read_only: true,
                error: None,
            },
            inspect_schema_with_connection(&connection),
            inspect_fts_with_connection(&connection),
        ),
        Err(storage::OpenWorkspaceReadOnlyError::Missing(_)) => (
            DbHealth {
                status: DoctorStatus::Missing,
                path: path_string(path),
                exists: false,
                open_read_only: false,
                error: None,
            },
            unavailable_schema_health(None),
            unavailable_fts_health(None),
        ),
        Err(error) => {
            let error = error.to_string();
            (
                DbHealth {
                    status: DoctorStatus::Error,
                    path: path_string(path),
                    exists: true,
                    open_read_only: false,
                    error: Some(error.clone()),
                },
                unavailable_schema_health(Some(&error)),
                unavailable_fts_health(Some(&error)),
            )
        }
    }
}

fn unavailable_schema_health(db_error: Option<&str>) -> SchemaHealth {
    SchemaHealth {
        status: missing_or_error_status(db_error),
        schema_migrations: missing_or_error_status(db_error),
        init_migration: missing_or_error_status(db_error),
        required_tables: required_names(REQUIRED_SCHEMA_TABLES, missing_or_error_status(db_error)),
        error: db_error.map(str::to_string),
    }
}

fn inspect_schema_with_connection(connection: &Connection) -> SchemaHealth {
    let schema_migrations = table_status(connection, "schema_migrations");
    let init_migration = if schema_migrations == DoctorStatus::Ready {
        migration_status(
            connection,
            REQUIRED_SCHEMA_MIGRATION_VERSION,
            REQUIRED_SCHEMA_MIGRATION_NAME,
        )
    } else {
        DoctorStatus::Missing
    };
    let required_tables = REQUIRED_SCHEMA_TABLES
        .iter()
        .map(|name| NamedHealth {
            name: (*name).to_string(),
            status: table_status(connection, name),
        })
        .collect::<Vec<_>>();
    let status = combine_statuses(
        &required_tables
            .iter()
            .map(|item| item.status)
            .chain([schema_migrations, init_migration])
            .collect::<Vec<_>>(),
    );

    SchemaHealth {
        status,
        schema_migrations,
        init_migration,
        required_tables,
        error: None,
    }
}

fn unavailable_fts_health(db_error: Option<&str>) -> FtsHealth {
    FtsHealth {
        status: missing_or_error_status(db_error),
        table: missing_or_error_status(db_error),
        required_columns: required_names(REQUIRED_FTS_COLUMNS, missing_or_error_status(db_error)),
        error: db_error.map(str::to_string),
    }
}

fn inspect_fts_with_connection(connection: &Connection) -> FtsHealth {
    let table = table_status(connection, REQUIRED_FTS_TABLE);
    let required_columns = REQUIRED_FTS_COLUMNS
        .iter()
        .map(|name| NamedHealth {
            name: (*name).to_string(),
            status: fts_column_status(connection, name, table),
        })
        .collect::<Vec<_>>();
    let status = combine_statuses(
        &required_columns
            .iter()
            .map(|item| item.status)
            .chain([table])
            .collect::<Vec<_>>(),
    );

    FtsHealth {
        status,
        table,
        required_columns,
        error: None,
    }
}

fn inspect_adapter_block(path: &Path) -> AdapterBlockHealth {
    match adapter::instruction_file_status(path) {
        Ok(status) => {
            let managed_block = status.managed_block.as_str().to_string();
            let health_status = match status.managed_block {
                adapter::ManagedBlockStatus::InSync => DoctorStatus::Ready,
                adapter::ManagedBlockStatus::Missing | adapter::ManagedBlockStatus::Drifted => {
                    DoctorStatus::Missing
                }
            };

            AdapterBlockHealth {
                status: health_status,
                instruction_file: path_string(&status.instruction_file),
                file_exists: status.file_exists,
                managed_block,
                error: None,
            }
        }
        Err(error) => AdapterBlockHealth {
            status: DoctorStatus::Error,
            instruction_file: path_string(path),
            file_exists: path.is_file(),
            managed_block: "damaged".to_string(),
            error: Some(error.to_string()),
        },
    }
}

fn inspect_artifacts_dirs(
    artifacts: &Path,
    audit_git: &Path,
    runtimes: &Path,
    logs: &Path,
) -> ArtifactsDirsHealth {
    let artifacts = inspect_path(artifacts);
    let audit_git = inspect_path(audit_git);
    let runtimes = inspect_path(runtimes);
    let logs = inspect_path(logs);
    let status = combine_statuses(&[
        artifacts.status,
        audit_git.status,
        runtimes.status,
        logs.status,
    ]);

    ArtifactsDirsHealth {
        status,
        artifacts,
        audit_git,
        runtimes,
        logs,
    }
}

fn inspect_audit_snapshot(audit_git_dir: &Path) -> AuditSnapshotHealth {
    let marker_path = audit_git_dir.join(audit::PENDING_SNAPSHOT_MARKER_FILE_NAME);
    match audit::has_pending_snapshot(audit_git_dir) {
        Ok(false) => AuditSnapshotHealth {
            status: DoctorStatus::Ready,
            pending: false,
            marker_path: path_string(&marker_path),
            error: None,
        },
        Ok(true) => AuditSnapshotHealth {
            status: DoctorStatus::Error,
            pending: true,
            marker_path: path_string(&marker_path),
            error: Some(
                "pending audit snapshot marker exists; fix: aopmem audit repair --current-workspace --json"
                    .to_string(),
            ),
        },
        Err(error) => AuditSnapshotHealth {
            status: DoctorStatus::Error,
            pending: false,
            marker_path: path_string(&marker_path),
            error: Some(error.to_string()),
        },
    }
}

fn table_status(connection: &Connection, table_name: &str) -> DoctorStatus {
    match connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1;",
        [table_name],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(count) => DoctorStatus::from_present(count > 0),
        Err(_) => DoctorStatus::Error,
    }
}

fn migration_status(connection: &Connection, version: &str, name: &str) -> DoctorStatus {
    match connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1 AND name = ?2;",
        [version, name],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(count) => DoctorStatus::from_present(count > 0),
        Err(_) => DoctorStatus::Error,
    }
}

fn fts_column_status(
    connection: &Connection,
    column_name: &str,
    table_status: DoctorStatus,
) -> DoctorStatus {
    if table_status != DoctorStatus::Ready {
        return table_status;
    }

    match connection.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('fts_nodes') WHERE name = ?1;",
        [column_name],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(count) => DoctorStatus::from_present(count > 0),
        Err(_) => DoctorStatus::Error,
    }
}

fn combine_statuses(statuses: &[DoctorStatus]) -> DoctorStatus {
    if statuses.contains(&DoctorStatus::Error) {
        DoctorStatus::Error
    } else if statuses.contains(&DoctorStatus::Missing) {
        DoctorStatus::Missing
    } else {
        DoctorStatus::Ready
    }
}

fn missing_or_error_status(error: Option<&str>) -> DoctorStatus {
    if error.is_some() {
        DoctorStatus::Error
    } else {
        DoctorStatus::Missing
    }
}

fn required_names(names: &[&str], status: DoctorStatus) -> Vec<NamedHealth> {
    names
        .iter()
        .map(|name| NamedHealth {
            name: (*name).to_string(),
            status,
        })
        .collect()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    struct EnvGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let original = env::var_os(key);
            env::set_var(key, value);
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

    fn temp_path(name: &str) -> PathBuf {
        let unique = format!(
            "aopmem-verify-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        );
        env::temp_dir().join(unique)
    }

    #[test]
    fn doctor_reports_ready_for_prepared_workspace() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("ready-home");
        let home = temp_path("ready-fallback-home");
        let repo_root = temp_path("ready-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        let instruction_file = repo_root.join("AGENTS.md");
        storage::ensure_global_dirs(&storage::resolve_paths().expect("paths should resolve"))
            .expect("global dirs should be created");
        let workspace_key =
            storage::workspace_key(&repo_root).expect("workspace key should resolve");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &workspace_key)
            .expect("workspace dirs should exist");
        storage::open_workspace_db(&workspace_paths).expect("db should be initialized");
        adapter::seed_instruction_file(&instruction_file).expect("adapter block should be seeded");

        let report = run_doctor(&repo_root).expect("doctor should succeed");

        assert!(report.healthy);
        assert_eq!(report.checks.global_dirs.status, DoctorStatus::Ready);
        assert_eq!(report.checks.workspace.status, DoctorStatus::Ready);
        assert_eq!(report.checks.db.status, DoctorStatus::Ready);
        assert_eq!(report.checks.schema.status, DoctorStatus::Ready);
        assert_eq!(report.checks.fts.status, DoctorStatus::Ready);
        assert_eq!(report.checks.adapter_block.status, DoctorStatus::Ready);
        assert_eq!(report.checks.artifacts_dirs.status, DoctorStatus::Ready);
        assert_eq!(report.checks.audit_snapshot.status, DoctorStatus::Ready);
        assert_eq!(report.checks.tools_dirs.status, DoctorStatus::Ready);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn doctor_and_lint_reject_linked_external_database() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("linked-db-home");
        let home = temp_path("linked-db-fallback-home");
        let repo_root = temp_path("linked-db-repo");
        let outside_db = temp_path("linked-db-outside.sqlite");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);
        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        crate::install::init_workspace(&repo_root).expect("workspace should initialize");
        let workspace_key =
            storage::workspace_key(&repo_root).expect("workspace key should resolve");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
        fs::rename(workspace_paths.db(), &outside_db).expect("DB should move outside");
        symlink(&outside_db, workspace_paths.db()).expect("DB symlink should create");
        let outside_before = fs::read(&outside_db).expect("outside DB should read");

        let doctor = run_doctor(&repo_root).expect("doctor should return a health report");
        let lint = run_lint(&repo_root).expect_err("lint must reject linked DB");

        assert!(!doctor.healthy);
        assert_eq!(doctor.checks.db.status, DoctorStatus::Error);
        assert!(!doctor.checks.db.open_read_only);
        assert!(matches!(lint, LintError::Io(_)));
        assert_eq!(
            fs::read(&outside_db).expect("outside DB should remain readable"),
            outside_before
        );
        fs::remove_file(workspace_paths.db()).expect("DB symlink should remove");
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(&repo_root).expect("temp repo root should remove");
        fs::remove_file(outside_db).expect("outside DB should remove");
    }

    #[test]
    fn doctor_and_lint_detect_pending_audit_snapshot() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("pending-snapshot-home");
        let home = temp_path("pending-snapshot-fallback-home");
        let repo_root = temp_path("pending-snapshot-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        crate::install::init_workspace(&repo_root).expect("workspace should initialize");

        let workspace_key =
            storage::workspace_key(&repo_root).expect("workspace key should resolve");
        let marker_path = storage::resolve_paths()
            .expect("paths should resolve")
            .workspaces()
            .join(&workspace_key)
            .join("audit-git")
            .join(audit::PENDING_SNAPSHOT_MARKER_FILE_NAME);
        fs::write(&marker_path, "snapshot did not finish\n")
            .expect("pending marker should be written");

        let doctor = run_doctor(&repo_root).expect("doctor should succeed");
        assert!(!doctor.healthy);
        assert_eq!(doctor.checks.audit_snapshot.status, DoctorStatus::Error);
        assert!(doctor.checks.audit_snapshot.pending);
        assert!(doctor
            .checks
            .audit_snapshot
            .error
            .as_deref()
            .is_some_and(|error| {
                error.contains("aopmem audit repair --current-workspace --json")
            }));

        let lint = run_lint(&repo_root).expect("lint should succeed");
        assert!(!lint.clean);
        assert_eq!(lint.summary.pending_audit_snapshot, 1);
        assert!(lint.issues.iter().any(|issue| {
            issue.kind == LintIssueKind::PendingAuditSnapshot
                && issue.subject == format!("audit_snapshot:{}", marker_path.display())
                && issue
                    .message
                    .contains("aopmem audit repair --current-workspace --json")
        }));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn doctor_reports_missing_db_schema_and_fts_for_uninitialized_workspace() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("missing-home");
        let home = temp_path("missing-fallback-home");
        let repo_root = temp_path("missing-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        storage::ensure_global_dirs(&storage::resolve_paths().expect("paths should resolve"))
            .expect("global dirs should be created");

        let report = run_doctor(&repo_root).expect("doctor should succeed");

        assert!(!report.healthy);
        assert_eq!(report.checks.workspace.status, DoctorStatus::Missing);
        assert_eq!(report.checks.db.status, DoctorStatus::Missing);
        assert_eq!(report.checks.schema.status, DoctorStatus::Missing);
        assert_eq!(report.checks.fts.status, DoctorStatus::Missing);
        assert_eq!(report.checks.tools_dirs.status, DoctorStatus::Missing);
        assert_eq!(report.checks.artifacts_dirs.status, DoctorStatus::Missing);
        assert_eq!(report.checks.adapter_block.status, DoctorStatus::Missing);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn doctor_does_not_create_workspace_state_when_missing() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("missing-read-only-home");
        let home = temp_path("missing-read-only-fallback-home");
        let repo_root = temp_path("missing-read-only-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");

        let report = run_doctor(&repo_root).expect("doctor should succeed");

        assert_eq!(report.checks.db.status, DoctorStatus::Missing);
        assert!(!override_home.exists());
        assert!(!home.exists());

        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn lint_reports_clean_initialized_workspace() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("lint-clean-home");
        let home = temp_path("lint-clean-fallback-home");
        let repo_root = temp_path("lint-clean-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        crate::install::init_workspace(&repo_root).expect("workspace should initialize");

        let report = run_lint(&repo_root).expect("lint should succeed");

        assert!(report.clean);
        assert_eq!(report.summary.total, 0);
        assert!(report.issues.is_empty());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn lint_reports_duplicate_ids_broken_links_and_missing_fields() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("lint-dirty-home");
        let home = temp_path("lint-dirty-fallback-home");
        let repo_root = temp_path("lint-dirty-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        crate::install::init_workspace(&repo_root).expect("workspace should initialize");

        let workspace_key =
            storage::workspace_key(&repo_root).expect("workspace key should resolve");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &workspace_key)
            .expect("workspace dirs should exist");
        let connection = Connection::open(workspace_paths.db()).expect("db should open");

        connection
            .execute("DELETE FROM nodes WHERE node_type = 'gate';", [])
            .expect("gate nodes should delete");
        connection
            .execute(
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
                VALUES (?1, ?2, ?3, NULL, NULL, NULL, ?4, ?5);
                ",
                rusqlite::params!["workflow", "active", "Broken active workflow", 1.0, "high"],
            )
            .expect("invalid active node should insert");
        let active_node_id = connection.last_insert_rowid();
        connection
            .execute(
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
                VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7);
                ",
                rusqlite::params![
                    "workflow",
                    "deprecated",
                    "Old workflow",
                    "Deprecated workflow",
                    "source=user_instruction",
                    1.0,
                    "high"
                ],
            )
            .expect("deprecated node should insert");
        let deprecated_node_id = connection.last_insert_rowid();
        connection
            .execute(
                "
                INSERT INTO links (source_node_id, target_node_id, link_type)
                VALUES (?1, ?2, ?3);
                ",
                rusqlite::params![active_node_id, deprecated_node_id, "depends_on"],
            )
            .expect("deprecated link should insert");
        connection
            .execute_batch(
                "
                ALTER TABLE tool_contracts RENAME TO tool_contracts_original;
                CREATE TABLE tool_contracts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    tool_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    entrypoint TEXT NOT NULL,
                    owner_workflow TEXT,
                    side_effects TEXT NOT NULL,
                    approval_requirement TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO tool_contracts (
                    tool_id,
                    name,
                    entrypoint,
                    owner_workflow,
                    side_effects,
                    approval_requirement,
                    created_at,
                    updated_at
                )
                VALUES
                    (
                        'dup-tool',
                        'Duplicate tool one',
                        'bin/dup-tool-one',
                        NULL,
                        'none',
                        'none',
                        CURRENT_TIMESTAMP,
                        CURRENT_TIMESTAMP
                    ),
                    (
                        'dup-tool',
                        'Duplicate tool two',
                        'bin/dup-tool-two',
                        NULL,
                        'none',
                        'none',
                        CURRENT_TIMESTAMP,
                        CURRENT_TIMESTAMP
                    );
                PRAGMA foreign_keys = OFF;
                INSERT INTO links (source_node_id, target_node_id, link_type)
                VALUES (999999, 999998, 'broken_reference');
                PRAGMA foreign_keys = ON;
                ",
            )
            .expect("corrupted records should insert");

        let report = run_lint(&repo_root).expect("lint should succeed");

        assert!(!report.clean);
        assert_eq!(report.summary.duplicate_ids, 1);
        assert_eq!(report.summary.broken_links, 1);
        assert_eq!(report.summary.deprecated_active_links, 1);
        assert_eq!(report.summary.missing_source, 1);
        assert_eq!(report.summary.missing_summary, 1);
        assert_eq!(report.summary.missing_gates, 1);
        assert_eq!(report.summary.total, 6);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn lint_reports_adapter_block_drift() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("lint-adapter-drift-home");
        let home = temp_path("lint-adapter-drift-fallback-home");
        let repo_root = temp_path("lint-adapter-drift-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        crate::install::init_workspace(&repo_root).expect("workspace should initialize");
        let instruction_file = repo_root.join("AGENTS.md");
        adapter::seed_instruction_file(&instruction_file).expect("adapter block should be seeded");
        fs::write(
            &instruction_file,
            format!(
                "{}\nchanged by hand\n{}\n",
                adapter::BEGIN_MARKER,
                adapter::END_MARKER
            ),
        )
        .expect("drifted adapter block should be written");

        let report = run_lint(&repo_root).expect("lint should succeed");

        assert_eq!(report.summary.adapter_block_drift, 1);
        assert!(report.issues.iter().any(|issue| {
            issue.kind == LintIssueKind::AdapterBlockDrift
                && issue.subject == format!("adapter:{}", instruction_file.display())
        }));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn lint_reports_schema_drift_when_init_marker_is_missing() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("lint-schema-drift-home");
        let home = temp_path("lint-schema-drift-fallback-home");
        let repo_root = temp_path("lint-schema-drift-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(&repo_root).expect("repo root should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        crate::install::init_workspace(&repo_root).expect("workspace should initialize");

        let workspace_key =
            storage::workspace_key(&repo_root).expect("workspace key should resolve");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &workspace_key)
            .expect("workspace dirs should exist");
        let connection = Connection::open(workspace_paths.db()).expect("db should open");
        connection
            .execute(
                "DELETE FROM schema_migrations WHERE version = ?1 AND name = ?2;",
                rusqlite::params![
                    REQUIRED_SCHEMA_MIGRATION_VERSION,
                    REQUIRED_SCHEMA_MIGRATION_NAME
                ],
            )
            .expect("schema migration marker should delete");

        let report = run_lint(&repo_root).expect("lint should succeed");

        assert_eq!(report.summary.schema_drift, 1);
        assert!(report.issues.iter().any(|issue| {
            issue.kind == LintIssueKind::SchemaDrift
                && issue.subject == "schema:schema_migrations:004_task_protocol_and_tool_aliases"
        }));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn lint_reports_forbidden_feature_terms_in_code_paths() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("lint-forbidden-term-home");
        let home = temp_path("lint-forbidden-term-fallback-home");
        let repo_root = temp_path("lint-forbidden-term-repo");
        let _aopmem_home = EnvGuard::set("AOPMEM_HOME", &override_home);
        let _home = EnvGuard::set("HOME", &home);

        fs::create_dir_all(repo_root.join("src")).expect("src dir should exist");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        crate::install::init_workspace(&repo_root).expect("workspace should initialize");
        fs::write(
            repo_root.join("src").join("forbidden.rs"),
            "// Mem0 should not appear here\n",
        )
        .expect("forbidden source file should be written");

        let report = run_lint(&repo_root).expect("lint should succeed");

        assert_eq!(report.summary.forbidden_feature_terms, 1);
        assert!(report.issues.iter().any(|issue| {
            issue.kind == LintIssueKind::ForbiddenFeatureTerm
                && issue.subject == "file:src/forbidden.rs"
                && issue.message.contains("term=mem0")
        }));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }
}
