//! Deterministic, privacy-bounded debug capsule export.

use super::report::{
    self, CollectionStatus, EffectivenessFacts, EffectivenessReport, ObserveReadError,
    ReportPeriod, ReportTimestamp,
};
use super::{
    open_reader, redact_sensitive_text, truncate_utf8, validate_ascii_identifier,
    validate_positive_id, validate_uuid_v4, ObservabilityOpenError, OBSERVABILITY_SCHEMA_VERSION,
};
use crate::audit::{AnchoredDir, CommittedPublishOutcome};
use crate::output::OutputWarning;
use crate::storage::{self, OpenWorkspaceReadOnlyError, WorkspacePaths};
use crate::tools;
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::Serialize;
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{self, Seek, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const PRODUCT_NAME: &str = "AOPMem";
const CAPSULE_FORMAT: &str = "aopmem-debug-capsule";
const CAPSULE_FORMAT_VERSION: u64 = 1;
const FIXED_EMPTY_REFERENCE_AT: &str = "1970-01-01T00:00:00.000Z";
const TEMP_NAME_PREFIX: &str = ".aopmem-debug-capsule-";
const TEMP_CREATE_ATTEMPTS: usize = 16;
const ZIP_ENTRY_PERMISSIONS: u32 = 0o600;
pub(crate) const EXPORT_PUBLISHED_WITH_WARNING: &str = "EXPORT_PUBLISHED_WITH_WARNING";
const OPERATIONAL_SCHEMA_VERSION: &str = "003";
const REQUIRED_MIGRATIONS: [(&str, &str); 3] = [
    ("001", "001_init"),
    ("002", "002_nodes_summary_index"),
    ("003", "003_task_recall_exact_indexes"),
];
const OPERATIONAL_TABLE_COLUMNS: &[(&str, &[&str])] = &[
    ("schema_migrations", &["version", "name", "applied_at"]),
    (
        "nodes",
        &[
            "id",
            "node_type",
            "status",
            "title",
            "summary",
            "body",
            "source_ref",
            "confidence",
            "trust_level",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "links",
        &[
            "id",
            "source_node_id",
            "target_node_id",
            "link_type",
            "created_at",
        ],
    ),
    ("aliases", &["id", "node_id", "alias", "created_at"]),
    ("tags", &["id", "node_id", "tag", "created_at"]),
    ("sources", &["id", "node_id", "source_ref", "created_at"]),
    (
        "events",
        &[
            "id",
            "type",
            "timestamp",
            "source",
            "subject_kind",
            "subject_id",
        ],
    ),
    (
        "registries",
        &[
            "id",
            "registry_type",
            "name",
            "status",
            "notes",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "tool_contracts",
        &[
            "id",
            "tool_id",
            "name",
            "status",
            "owner_workflow",
            "side_effects",
            "approval_requirement",
            "contract_json",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "mcp_profiles",
        &[
            "id",
            "name",
            "kind",
            "status",
            "read_operations",
            "write_operations",
            "side_effects",
            "approval_requirement",
            "credentials_source",
            "notes",
            "created_at",
            "updated_at",
        ],
    ),
];
const FTS_COLUMNS: &[&str] = &["title", "summary", "body", "aliases"];
const SELECTION_REASONS: &[&str] = &[
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

pub(crate) const CAPSULE_ENTRIES: [&str; 12] = [
    "manifest.json",
    "product.json",
    "workspace_summary.json",
    "memory_summary.json",
    "health.json",
    "events.jsonl",
    "recall_bundles.jsonl",
    "bundle_nodes.jsonl",
    "feedback.jsonl",
    "tools_summary.json",
    "mcp_summary.json",
    "README.md",
];

const README: &str = "# AOPMem Debug Capsule\n\n\
This ZIP contains local, read-only, privacy-bounded facts for external analysis.\n\
It excludes SQLite databases, full node bodies, raw artifacts, raw chats, raw\n\
tool output, environment variables, credentials, cookies, and tokens.\n\
All JSONL files are ordered deterministically. The reference time comes from\n\
the latest persisted Local Observability timestamp, or the documented fixed\n\
epoch when Local Observability is missing or empty.\n";

#[derive(Debug, Error)]
pub(crate) enum ExportError {
    #[error("output path has no regular file name")]
    InvalidOutput,
    #[error("output already exists")]
    OutputExists,
    #[error("output path is unsafe")]
    UnsafeOutput,
    #[error("workspace database is missing")]
    WorkspaceMissing,
    #[error("workspace database path is unsafe")]
    WorkspaceUnsafe,
    #[error("workspace database is invalid or incompatible")]
    WorkspaceInvalid,
    #[error(transparent)]
    Observability(#[from] ObserveReadError),
    #[error("could not create private export temporary file")]
    TemporaryFile,
    #[error("random temporary name generation failed")]
    RandomFailed,
    #[error("debug capsule serialization failed")]
    Serialization,
    #[error("debug capsule ZIP write failed")]
    Zip,
    #[error("debug capsule sync failed")]
    Sync,
    #[error("debug capsule atomic publish failed")]
    Publish,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ExportResult {
    pub output: String,
    pub entries: usize,
    pub bytes: u64,
    pub collection_status: CollectionStatus,
    pub reference_at: String,
    pub publication_status: PublicationStatus,
    pub temporary_cleanup_confirmed: bool,
    #[serde(skip)]
    pub warning: Option<OutputWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PublicationStatus {
    Durable,
    PublishedWithWarning,
}

impl PublicationStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Durable => "durable",
            Self::PublishedWithWarning => "published_with_warning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReferenceSource {
    ObservabilityLatestPersisted,
    FixedEpochInitializedEmpty,
    FixedEpochNotCollected,
}

#[derive(Serialize)]
struct Manifest<'a> {
    format: &'static str,
    format_version: u64,
    product_version: &'static str,
    workspace_key: &'a str,
    reference_at: &'a str,
    reference_source: ReferenceSource,
    entries: &'static [&'static str],
    deterministic: bool,
    privacy_profile: &'static str,
}

#[derive(Serialize)]
pub(crate) struct ProductSummary {
    name: &'static str,
    version: &'static str,
    operational_schema_version: &'static str,
    observability_schema_version: u64,
    local_only: bool,
}

#[derive(Serialize)]
pub(crate) struct WorkspaceSummary<'a> {
    workspace: &'a str,
    collection_status: CollectionStatus,
    complete: bool,
    observability_schema_version: Option<u64>,
    reference_at: &'a str,
    period: &'a ReportPeriod,
    effectiveness: Option<&'a EffectivenessFacts>,
}

#[derive(Serialize)]
pub(crate) struct MemorySummaryHeader {
    node_count: u64,
    link_count: u64,
    counts_by_type: Vec<NamedCount>,
    counts_by_status: Vec<NamedCount>,
    broken: u64,
    orphaned: u64,
    deprecated: u64,
    draft: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HealthObservationStatus {
    NotCollected,
    Success,
    Warning,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct HealthObservation {
    status: HealthObservationStatus,
    observed_at: Option<String>,
    error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct HealthSummary {
    collection_status: CollectionStatus,
    doctor: HealthObservation,
    verify: HealthObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct NamedCount {
    name: String,
    count: u64,
}

#[derive(Serialize)]
pub(crate) struct MemoryNodeSummary {
    id: i64,
    node_type: String,
    status: String,
    title: String,
    summary: Option<String>,
    source_ref: Option<String>,
    trust_level: Option<String>,
    confidence: Option<f64>,
    incoming_links: u64,
    outgoing_links: u64,
}

#[derive(Serialize)]
pub(crate) struct ToolSummaryItem {
    tool_id: String,
    name: String,
    status: String,
    owner_workflow: Option<String>,
    side_effects: String,
    approval_requirement: String,
}

#[derive(Serialize)]
pub(crate) struct McpSummaryItem {
    id: String,
    name: String,
    kind: String,
    status: String,
    read_operations: String,
    write_operations: String,
    side_effects: String,
    approval_requirement: String,
}

#[derive(Serialize)]
struct EventLine {
    id: String,
    timestamp: String,
    product_version: String,
    workspace_key: String,
    event_type: String,
    command: String,
    correlation_id: String,
    bundle_id: Option<String>,
    duration_ms: Option<u64>,
    outcome: String,
    error_code: Option<String>,
}

#[derive(Serialize)]
struct RecallBundleLine {
    bundle_id: String,
    timestamp: String,
    product_version: String,
    workspace_key: String,
    correlation_id: String,
    outcome: String,
    error_code: Option<String>,
    duration_ms: u64,
    more_results: bool,
    continuation_count: u64,
}

#[derive(Serialize)]
struct BundleNodeLine {
    bundle_id: String,
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

#[derive(Serialize)]
struct FeedbackLine {
    id: String,
    timestamp: String,
    bundle_id: String,
    outcome: String,
    reason: Option<String>,
}

struct OutputTarget {
    directory: AnchoredDir,
    file_name: OsString,
    display_path: PathBuf,
}

struct TemporaryArchive {
    directory: AnchoredDir,
    name: OsString,
    cleanup: bool,
}

impl TemporaryArchive {
    fn disarm(&mut self) {
        self.cleanup = false;
    }
}

impl Drop for TemporaryArchive {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = self.directory.remove_regular_os(&self.name);
        }
    }
}

pub(crate) fn export_debug_capsule(
    workspace_key: &str,
    workspace_paths: &WorkspacePaths,
    output: &Path,
) -> Result<ExportResult, ExportError> {
    validate_workspace_binding(workspace_key, workspace_paths)?;
    let target = prepare_output_target(output)?;
    let operational = open_operational_reader(workspace_paths)?;
    let operational_snapshot = operational
        .unchecked_transaction()
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    establish_operational_snapshot(&operational_snapshot)?;

    let observability_reader = open_optional_observability_reader(workspace_paths)?;
    let observability_snapshot = observability_reader
        .as_ref()
        .map(|reader| reader.connection.unchecked_transaction())
        .transpose()
        .map_err(|_| ExportError::Observability(ObserveReadError::ReadFailed))?;
    if let Some(snapshot) = observability_snapshot.as_ref() {
        validate_observability_integrity(snapshot)?;
    }
    let (reference_at, reference_source, report) = match observability_snapshot.as_ref() {
        Some(snapshot) => {
            let reference = deterministic_reference_at(snapshot)?;
            let source = if reference.as_str() == FIXED_EMPTY_REFERENCE_AT {
                ReferenceSource::FixedEpochInitializedEmpty
            } else {
                ReferenceSource::ObservabilityLatestPersisted
            };
            let report =
                report::effectiveness_report_in_snapshot(snapshot, workspace_key, &reference)?;
            (reference, source, report)
        }
        None => {
            let reference = ReportTimestamp::parse(FIXED_EMPTY_REFERENCE_AT)?;
            let report = report::not_collected_report(workspace_key, &reference)?;
            (reference, ReferenceSource::FixedEpochNotCollected, report)
        }
    };

    let (temporary_file, mut temporary) = create_temporary_archive(&target.directory)?;
    let (publish_source, bytes) = write_archive(
        temporary_file,
        workspace_key,
        reference_at.as_str(),
        reference_source,
        &report,
        &operational_snapshot,
        observability_snapshot.as_ref(),
    )?;

    operational_snapshot
        .commit()
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    if let Some(snapshot) = observability_snapshot {
        snapshot
            .commit()
            .map_err(|_| ExportError::Observability(ObserveReadError::ReadFailed))?;
    }
    let publication = match target.directory.publish_regular_no_replace_committed_os(
        &publish_source,
        &temporary.name,
        &target.file_name,
    ) {
        Ok(outcome) => outcome,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ExportError::OutputExists);
        }
        Err(_) => return Err(ExportError::Publish),
    };
    if publication.temporary_cleanup_confirmed {
        temporary.disarm();
    }
    let (publication_status, warning) = publication_result(publication);

    Ok(ExportResult {
        output: target.display_path.display().to_string(),
        entries: CAPSULE_ENTRIES.len(),
        bytes,
        collection_status: report.collection_status,
        reference_at: reference_at.as_str().to_string(),
        publication_status,
        temporary_cleanup_confirmed: publication.temporary_cleanup_confirmed,
        warning,
    })
}

fn validate_workspace_binding(
    workspace_key: &str,
    workspace_paths: &WorkspacePaths,
) -> Result<(), ExportError> {
    if workspace_key.is_empty()
        || workspace_key.as_bytes().contains(&0)
        || workspace_paths.root().file_name() != Some(OsStr::new(workspace_key))
    {
        return Err(ExportError::WorkspaceInvalid);
    }
    Ok(())
}

fn publication_result(
    outcome: CommittedPublishOutcome,
) -> (PublicationStatus, Option<OutputWarning>) {
    if outcome.durability_confirmed && outcome.temporary_cleanup_confirmed {
        (PublicationStatus::Durable, None)
    } else {
        (
            PublicationStatus::PublishedWithWarning,
            Some(OutputWarning {
                code: EXPORT_PUBLISHED_WITH_WARNING,
                message: "debug capsule was published, but directory durability or temporary cleanup could not be confirmed".to_string(),
            }),
        )
    }
}

fn prepare_output_target(output: &Path) -> Result<OutputTarget, ExportError> {
    let display_path = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| ExportError::UnsafeOutput)?
            .join(output)
    };
    let file_name = display_path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or(ExportError::InvalidOutput)?
        .to_os_string();
    let parent = display_path.parent().ok_or(ExportError::InvalidOutput)?;
    let directory =
        AnchoredDir::open_workspace(parent, None).map_err(|_| ExportError::UnsafeOutput)?;
    match directory
        .open_regular_optional_os(&file_name)
        .map_err(|_| ExportError::UnsafeOutput)?
    {
        Some(_) => Err(ExportError::OutputExists),
        None => Ok(OutputTarget {
            directory,
            file_name,
            display_path,
        }),
    }
}

fn open_operational_reader(workspace_paths: &WorkspacePaths) -> Result<Connection, ExportError> {
    match storage::open_workspace_db_read_only(workspace_paths) {
        Ok(connection) => Ok(connection),
        Err(OpenWorkspaceReadOnlyError::Missing(_)) => Err(ExportError::WorkspaceMissing),
        Err(OpenWorkspaceReadOnlyError::UnsafePath(_)) => Err(ExportError::WorkspaceUnsafe),
        Err(OpenWorkspaceReadOnlyError::Db(_)) => Err(ExportError::WorkspaceInvalid),
    }
}

fn establish_operational_snapshot(transaction: &Transaction<'_>) -> Result<(), ExportError> {
    validate_sqlite_integrity(transaction).map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut statement = transaction
        .prepare("SELECT version, name FROM schema_migrations ORDER BY version LIMIT 4")
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let migrations = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| ExportError::WorkspaceInvalid)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    if migrations.len() != REQUIRED_MIGRATIONS.len()
        || migrations
            .iter()
            .zip(REQUIRED_MIGRATIONS)
            .any(|((version, name), expected)| version != expected.0 || name != expected.1)
    {
        return Err(ExportError::WorkspaceInvalid);
    }
    for (table, columns) in OPERATIONAL_TABLE_COLUMNS {
        validate_table_columns(transaction, table, columns)?;
    }
    validate_fts_manifest(transaction)?;
    Ok(())
}

fn validate_table_columns(
    transaction: &Transaction<'_>,
    table: &str,
    expected: &[&str],
) -> Result<(), ExportError> {
    let table_type: Option<String> = transaction
        .query_row(
            "SELECT type FROM sqlite_master WHERE name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    if table_type.as_deref() != Some("table") {
        return Err(ExportError::WorkspaceInvalid);
    }
    let mut statement = transaction
        .prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let actual = statement
        .query_map([table], |row| row.get::<_, String>(0))
        .map_err(|_| ExportError::WorkspaceInvalid)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    if actual
        .iter()
        .map(String::as_str)
        .ne(expected.iter().copied())
    {
        return Err(ExportError::WorkspaceInvalid);
    }
    Ok(())
}

fn validate_fts_manifest(transaction: &Transaction<'_>) -> Result<(), ExportError> {
    validate_table_columns(transaction, "fts_nodes", FTS_COLUMNS)?;
    let sql: String = transaction
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'fts_nodes'",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let normalized = sql.to_ascii_lowercase();
    if !normalized.contains("virtual table") || !normalized.contains("using fts5") {
        return Err(ExportError::WorkspaceInvalid);
    }
    Ok(())
}

fn validate_observability_integrity(transaction: &Transaction<'_>) -> Result<(), ExportError> {
    validate_sqlite_integrity(transaction).map_err(|_| invalid_observability())
}

fn validate_sqlite_integrity(transaction: &Transaction<'_>) -> rusqlite::Result<()> {
    let result: String = transaction.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    if result != "ok" {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let mut statement = transaction.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    if rows.next()?.is_some() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

fn open_optional_observability_reader(
    workspace_paths: &WorkspacePaths,
) -> Result<Option<super::ObservabilityReader>, ExportError> {
    match open_reader(workspace_paths) {
        Ok(reader) => Ok(Some(reader)),
        Err(ObservabilityOpenError::Missing(_)) => Ok(None),
        Err(ObservabilityOpenError::UnsafePath { .. }) => {
            Err(ExportError::Observability(ObserveReadError::UnsafePath))
        }
        Err(
            ObservabilityOpenError::InvalidStore { .. }
            | ObservabilityOpenError::Sqlite(_)
            | ObservabilityOpenError::Serialization(_),
        ) => Err(ExportError::Observability(ObserveReadError::InvalidStore)),
    }
}

const DETERMINISTIC_REFERENCE_SQL: &str = r#"
SELECT MAX(timestamp) FROM (
    SELECT MAX(timestamp) AS timestamp FROM observability_events
    UNION ALL
    SELECT MAX(timestamp) FROM recall_bundles
    UNION ALL
    SELECT MAX(first_seen_at) FROM bundle_nodes
    UNION ALL
    SELECT MAX(timestamp) FROM feedback
    UNION ALL
    SELECT MAX(last_retention_at) FROM collector_state
    UNION ALL
    SELECT MAX(retention_floor_at) FROM collector_state
)
"#;

fn deterministic_reference_at(
    transaction: &Transaction<'_>,
) -> Result<ReportTimestamp, ExportError> {
    let value: Option<String> = transaction
        .query_row(DETERMINISTIC_REFERENCE_SQL, [], |row| row.get(0))
        .map_err(|_| ExportError::Observability(ObserveReadError::ReadFailed))?;
    ReportTimestamp::parse(value.as_deref().unwrap_or(FIXED_EMPTY_REFERENCE_AT))
        .map_err(ExportError::Observability)
}

fn create_temporary_archive(
    directory: &AnchoredDir,
) -> Result<(File, TemporaryArchive), ExportError> {
    for _ in 0..TEMP_CREATE_ATTEMPTS {
        let name = random_temporary_name()?;
        match directory.create_new_regular_os(&name) {
            Ok(file) => {
                return Ok((
                    file,
                    TemporaryArchive {
                        directory: directory.clone(),
                        name,
                        cleanup: true,
                    },
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(ExportError::TemporaryFile),
        }
    }
    Err(ExportError::TemporaryFile)
}

fn random_temporary_name() -> Result<OsString, ExportError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ExportError::RandomFailed)?;
    let mut name = String::with_capacity(TEMP_NAME_PREFIX.len() + bytes.len() * 2 + 4);
    name.push_str(TEMP_NAME_PREFIX);
    for byte in bytes {
        write!(&mut name, "{byte:02x}").map_err(|_| ExportError::RandomFailed)?;
    }
    name.push_str(".tmp");
    Ok(name.into())
}

fn write_archive(
    file: File,
    workspace_key: &str,
    reference_at: &str,
    reference_source: ReferenceSource,
    report: &EffectivenessReport,
    operational: &Transaction<'_>,
    observability: Option<&Transaction<'_>>,
) -> Result<(File, u64), ExportError> {
    let mut archive = ZipWriter::new(file);
    let manifest = Manifest {
        format: CAPSULE_FORMAT,
        format_version: CAPSULE_FORMAT_VERSION,
        product_version: env!("CARGO_PKG_VERSION"),
        workspace_key,
        reference_at,
        reference_source,
        entries: &CAPSULE_ENTRIES,
        deterministic: true,
        privacy_profile: "safe-facts-v1",
    };
    let product = build_product_summary()?;
    let workspace = build_workspace_summary(report, reference_at);
    let health = build_health_summary(observability, workspace_key, report.collection_status)?;
    write_json_entry(&mut archive, CAPSULE_ENTRIES[0], &manifest)?;
    write_json_entry(&mut archive, CAPSULE_ENTRIES[1], &product)?;
    write_json_entry(&mut archive, CAPSULE_ENTRIES[2], &workspace)?;
    write_memory_summary(&mut archive, operational)?;
    write_json_entry(&mut archive, CAPSULE_ENTRIES[4], &health)?;
    write_events_jsonl(&mut archive, observability, workspace_key)?;
    write_recall_bundles_jsonl(&mut archive, observability, workspace_key)?;
    write_bundle_nodes_jsonl(&mut archive, observability, workspace_key)?;
    write_feedback_jsonl(&mut archive, observability, workspace_key)?;
    write_tools_summary(&mut archive, operational)?;
    write_mcp_summary(&mut archive, operational)?;
    start_entry(&mut archive, CAPSULE_ENTRIES[11])?;
    archive
        .write_all(README.as_bytes())
        .map_err(|_| ExportError::Zip)?;
    let file = archive.finish().map_err(|_| ExportError::Zip)?;
    file.sync_all().map_err(|_| ExportError::Sync)?;
    let bytes = file
        .metadata()
        .map(|metadata| metadata.len())
        .map_err(|_| ExportError::Sync)?;
    Ok((file, bytes))
}

pub(crate) fn build_product_summary() -> Result<ProductSummary, ExportError> {
    Ok(ProductSummary {
        name: PRODUCT_NAME,
        version: env!("CARGO_PKG_VERSION"),
        operational_schema_version: OPERATIONAL_SCHEMA_VERSION,
        observability_schema_version: u64::try_from(OBSERVABILITY_SCHEMA_VERSION)
            .map_err(|_| ExportError::Serialization)?,
        local_only: true,
    })
}

pub(crate) fn build_workspace_summary<'a>(
    report: &'a EffectivenessReport,
    reference_at: &'a str,
) -> WorkspaceSummary<'a> {
    WorkspaceSummary {
        workspace: &report.workspace,
        collection_status: report.collection_status,
        complete: report.complete,
        observability_schema_version: report.observability_schema_version,
        reference_at,
        period: &report.period,
        effectiveness: report.facts.as_ref(),
    }
}

pub(crate) fn build_health_summary(
    observability: Option<&Transaction<'_>>,
    workspace_key: &str,
    collection_status: CollectionStatus,
) -> Result<HealthSummary, ExportError> {
    let not_collected = || HealthObservation {
        status: HealthObservationStatus::NotCollected,
        observed_at: None,
        error_code: None,
    };
    let Some(observability) = observability else {
        return Ok(HealthSummary {
            collection_status,
            doctor: not_collected(),
            verify: not_collected(),
        });
    };
    Ok(HealthSummary {
        collection_status,
        doctor: latest_health_observation(observability, workspace_key, "doctor")?
            .unwrap_or_else(&not_collected),
        verify: latest_health_observation(observability, workspace_key, "verify")?
            .unwrap_or_else(not_collected),
    })
}

fn latest_health_observation(
    observability: &Transaction<'_>,
    workspace_key: &str,
    event_type: &str,
) -> Result<Option<HealthObservation>, ExportError> {
    let mut statement = observability
        .prepare(
            "SELECT id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, bundle_id, duration_ms, outcome,
                    error_code, payload_json
             FROM observability_events
             WHERE event_type = ?1 AND workspace_key = ?2
             ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .map_err(|_| invalid_observability())?;
    let mut rows = statement
        .query(rusqlite::params![event_type, workspace_key])
        .map_err(|_| invalid_observability())?;
    let Some(row) = rows.next().map_err(|_| invalid_observability())? else {
        return Ok(None);
    };
    report::validate_event_row(row, workspace_key)?;
    let outcome: String = row.get(9).map_err(|_| invalid_observability())?;
    let status = match outcome.as_str() {
        "success" => HealthObservationStatus::Success,
        "warning" => HealthObservationStatus::Warning,
        "failure" => HealthObservationStatus::Failure,
        _ => return Err(invalid_observability()),
    };
    let error_code = row
        .get::<_, Option<String>>(10)
        .map_err(|_| invalid_observability())?
        .map(|value| redact_observability_text(&value, 128));
    Ok(Some(HealthObservation {
        status,
        observed_at: Some(row.get(1).map_err(|_| invalid_observability())?),
        error_code,
    }))
}

fn write_memory_summary<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    operational: &Transaction<'_>,
) -> Result<(), ExportError> {
    let header = build_memory_summary_header(operational)?;
    start_entry(archive, CAPSULE_ENTRIES[3])?;
    let header_json = serde_json::to_vec(&header).map_err(|_| ExportError::Serialization)?;
    let prefix = header_json
        .strip_suffix(b"}")
        .ok_or(ExportError::Serialization)?;
    archive.write_all(prefix).map_err(|_| ExportError::Zip)?;
    archive
        .write_all(b",\"nodes\":[")
        .map_err(|_| ExportError::Zip)?;

    let mut statement = operational
        .prepare(
            "WITH outgoing AS (
                SELECT source_node_id AS node_id, COUNT(*) AS count
                FROM links GROUP BY source_node_id
             ), incoming AS (
                SELECT target_node_id AS node_id, COUNT(*) AS count
                FROM links GROUP BY target_node_id
             )
             SELECT node.id, node.node_type, node.status, node.title,
                    node.summary, node.source_ref, node.trust_level,
                    node.confidence, COALESCE(incoming.count, 0),
                    COALESCE(outgoing.count, 0)
             FROM nodes AS node
             LEFT JOIN incoming ON incoming.node_id = node.id
             LEFT JOIN outgoing ON outgoing.node_id = node.id
             ORDER BY node.id",
        )
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut rows = statement
        .query([])
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut first = true;
    let mut streamed = 0_u64;
    while let Some(row) = rows.next().map_err(|_| ExportError::WorkspaceInvalid)? {
        let item = memory_node_summary_from_row(row)?;
        streamed = streamed
            .checked_add(1)
            .ok_or(ExportError::WorkspaceInvalid)?;
        write_json_array_item(archive, &mut first, &item)?;
    }
    if streamed != header.node_count {
        return Err(ExportError::WorkspaceInvalid);
    }
    archive.write_all(b"]}\n").map_err(|_| ExportError::Zip)
}

pub(crate) fn build_memory_summary_header(
    operational: &Transaction<'_>,
) -> Result<MemorySummaryHeader, ExportError> {
    let node_count = scalar_count(operational, "SELECT COUNT(*) FROM nodes")?;
    let link_count = scalar_count(operational, "SELECT COUNT(*) FROM links")?;
    Ok(MemorySummaryHeader {
        node_count,
        link_count,
        counts_by_type: grouped_counts(
            operational,
            "SELECT node_type, COUNT(*) FROM nodes GROUP BY node_type ORDER BY node_type",
            storage::ALLOWED_NODE_TYPES,
        )?,
        counts_by_status: grouped_counts(
            operational,
            "SELECT status, COUNT(*) FROM nodes GROUP BY status ORDER BY status",
            storage::ALLOWED_NODE_STATUSES,
        )?,
        broken: scalar_count(
            operational,
            "SELECT COUNT(*) FROM nodes WHERE status = 'broken'",
        )?,
        orphaned: scalar_count(
            operational,
            "SELECT COUNT(*) FROM nodes AS node
             WHERE NOT EXISTS (
                SELECT 1 FROM links
                WHERE source_node_id = node.id OR target_node_id = node.id
             )",
        )?,
        deprecated: scalar_count(
            operational,
            "SELECT COUNT(*) FROM nodes WHERE status = 'deprecated'",
        )?,
        draft: scalar_count(
            operational,
            "SELECT COUNT(*) FROM nodes WHERE status = 'draft'",
        )?,
    })
}

pub(crate) fn memory_node_summary_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<MemoryNodeSummary, ExportError> {
    let id: i64 = row.get(0).map_err(|_| ExportError::WorkspaceInvalid)?;
    let node_type: String = row.get(1).map_err(|_| ExportError::WorkspaceInvalid)?;
    let status: String = row.get(2).map_err(|_| ExportError::WorkspaceInvalid)?;
    let title: String = row.get(3).map_err(|_| ExportError::WorkspaceInvalid)?;
    let summary: Option<String> = row.get(4).map_err(|_| ExportError::WorkspaceInvalid)?;
    let source_ref: Option<String> = row.get(5).map_err(|_| ExportError::WorkspaceInvalid)?;
    let trust_level: Option<String> = row.get(6).map_err(|_| ExportError::WorkspaceInvalid)?;
    let confidence: Option<f64> = row.get(7).map_err(|_| ExportError::WorkspaceInvalid)?;
    let incoming_links: i64 = row.get(8).map_err(|_| ExportError::WorkspaceInvalid)?;
    let outgoing_links: i64 = row.get(9).map_err(|_| ExportError::WorkspaceInvalid)?;
    if id <= 0
        || !storage::ALLOWED_NODE_TYPES.contains(&node_type.as_str())
        || !storage::ALLOWED_NODE_STATUSES.contains(&status.as_str())
        || confidence.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        return Err(ExportError::WorkspaceInvalid);
    }
    Ok(MemoryNodeSummary {
        id,
        node_type,
        status,
        title: safe_required_text(&title, storage::MAX_NODE_TITLE_BYTES, 512)?,
        summary: safe_optional_text(summary.as_deref(), storage::MAX_NODE_SUMMARY_BYTES, 2_048)?,
        source_ref: safe_optional_text(
            source_ref.as_deref(),
            storage::MAX_NODE_SOURCE_REF_BYTES,
            2_048,
        )?,
        trust_level: safe_optional_text(
            trust_level.as_deref(),
            storage::MAX_NODE_TRUST_LEVEL_BYTES,
            storage::MAX_NODE_TRUST_LEVEL_BYTES,
        )?,
        confidence,
        incoming_links: nonnegative_count(incoming_links)?,
        outgoing_links: nonnegative_count(outgoing_links)?,
    })
}

fn write_events_jsonl<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    observability: Option<&Transaction<'_>>,
    workspace_key: &str,
) -> Result<(), ExportError> {
    start_entry(archive, CAPSULE_ENTRIES[5])?;
    let Some(observability) = observability else {
        return Ok(());
    };
    let mut statement = observability
        .prepare(
            "SELECT id, timestamp, product_version, workspace_key, event_type,
                    command, correlation_id, bundle_id, duration_ms, outcome,
                    error_code, payload_json
             FROM observability_events
             ORDER BY timestamp, id",
        )
        .map_err(|_| invalid_observability())?;
    let mut rows = statement.query([]).map_err(|_| invalid_observability())?;
    while let Some(row) = rows.next().map_err(|_| invalid_observability())? {
        report::validate_event_row(row, workspace_key)?;
        let duration_ms: Option<i64> = row.get(8).map_err(|_| invalid_observability())?;
        let line = EventLine {
            id: row.get(0).map_err(|_| invalid_observability())?,
            timestamp: row.get(1).map_err(|_| invalid_observability())?,
            product_version: redact_observability_text(
                &row.get::<_, String>(2)
                    .map_err(|_| invalid_observability())?,
                128,
            ),
            workspace_key: row.get(3).map_err(|_| invalid_observability())?,
            event_type: row.get(4).map_err(|_| invalid_observability())?,
            command: row.get(5).map_err(|_| invalid_observability())?,
            correlation_id: row.get(6).map_err(|_| invalid_observability())?,
            bundle_id: row.get(7).map_err(|_| invalid_observability())?,
            duration_ms: duration_ms.map(nonnegative_observability).transpose()?,
            outcome: row.get(9).map_err(|_| invalid_observability())?,
            error_code: row
                .get::<_, Option<String>>(10)
                .map_err(|_| invalid_observability())?
                .map(|value| redact_observability_text(&value, 128)),
        };
        write_json_line(archive, &line)?;
    }
    Ok(())
}

fn write_recall_bundles_jsonl<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    observability: Option<&Transaction<'_>>,
    workspace_key: &str,
) -> Result<(), ExportError> {
    start_entry(archive, CAPSULE_ENTRIES[6])?;
    let Some(observability) = observability else {
        return Ok(());
    };
    let mut statement = observability
        .prepare(
            "SELECT bundle_id, timestamp, product_version, workspace_key,
                    correlation_id, outcome, error_code, duration_ms,
                    more_results, continuation_count
             FROM recall_bundles ORDER BY timestamp, bundle_id",
        )
        .map_err(|_| invalid_observability())?;
    let mut rows = statement.query([]).map_err(|_| invalid_observability())?;
    while let Some(row) = rows.next().map_err(|_| invalid_observability())? {
        let bundle_id: String = row.get(0).map_err(|_| invalid_observability())?;
        let timestamp: String = row.get(1).map_err(|_| invalid_observability())?;
        let product_version: String = row.get(2).map_err(|_| invalid_observability())?;
        let stored_workspace: String = row.get(3).map_err(|_| invalid_observability())?;
        let correlation_id: String = row.get(4).map_err(|_| invalid_observability())?;
        let outcome: String = row.get(5).map_err(|_| invalid_observability())?;
        let error_code: Option<String> = row.get(6).map_err(|_| invalid_observability())?;
        let duration_ms: i64 = row.get(7).map_err(|_| invalid_observability())?;
        let more_results: i64 = row.get(8).map_err(|_| invalid_observability())?;
        let continuation_count: i64 = row.get(9).map_err(|_| invalid_observability())?;
        validate_observability_uuid(&bundle_id)?;
        ReportTimestamp::parse(&timestamp)?;
        validate_observability_uuid(&correlation_id)?;
        if stored_workspace != workspace_key
            || product_version.as_bytes().contains(&0)
            || product_version.trim().is_empty()
            || product_version.len() > 128
            || !matches!(more_results, 0 | 1)
        {
            return Err(invalid_observability());
        }
        match outcome.as_str() {
            "success" if error_code.is_none() => {}
            "failure"
                if error_code
                    .as_deref()
                    .is_some_and(valid_observability_identifier) => {}
            _ => return Err(invalid_observability()),
        }
        let line = RecallBundleLine {
            bundle_id,
            timestamp,
            product_version: redact_observability_text(&product_version, 128),
            workspace_key: stored_workspace,
            correlation_id,
            outcome,
            error_code: error_code.map(|value| redact_observability_text(&value, 128)),
            duration_ms: nonnegative_observability(duration_ms)?,
            more_results: more_results == 1,
            continuation_count: nonnegative_observability(continuation_count)?,
        };
        write_json_line(archive, &line)?;
    }
    Ok(())
}

fn write_bundle_nodes_jsonl<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    observability: Option<&Transaction<'_>>,
    workspace_key: &str,
) -> Result<(), ExportError> {
    start_entry(archive, CAPSULE_ENTRIES[7])?;
    let Some(observability) = observability else {
        return Ok(());
    };
    let mut statement = observability
        .prepare(
            "SELECT node.bundle_id, node.node_id, node.first_seen_at,
                    node.node_type, node.node_title, node.bounded_summary,
                    node.source_ref, node.trust_level, node.confidence,
                    node.score, node.selection_reasons_json,
                    bundle.workspace_key
             FROM bundle_nodes AS node
             LEFT JOIN recall_bundles AS bundle USING (bundle_id)
             ORDER BY node.bundle_id, node.node_id",
        )
        .map_err(|_| invalid_observability())?;
    let mut rows = statement.query([]).map_err(|_| invalid_observability())?;
    while let Some(row) = rows.next().map_err(|_| invalid_observability())? {
        let bundle_id: String = row.get(0).map_err(|_| invalid_observability())?;
        let node_id: i64 = row.get(1).map_err(|_| invalid_observability())?;
        let first_seen_at: String = row.get(2).map_err(|_| invalid_observability())?;
        let node_type: String = row.get(3).map_err(|_| invalid_observability())?;
        let node_title: String = row.get(4).map_err(|_| invalid_observability())?;
        let bounded_summary: Option<String> = row.get(5).map_err(|_| invalid_observability())?;
        let source_ref: Option<String> = row.get(6).map_err(|_| invalid_observability())?;
        let trust_level: Option<String> = row.get(7).map_err(|_| invalid_observability())?;
        let confidence: Option<f64> = row.get(8).map_err(|_| invalid_observability())?;
        let score: Option<f64> = row.get(9).map_err(|_| invalid_observability())?;
        let reasons_json = row
            .get_ref(10)
            .map_err(|_| invalid_observability())?
            .as_str()
            .map_err(|_| invalid_observability())?;
        let stored_workspace: Option<String> = row.get(11).map_err(|_| invalid_observability())?;
        validate_observability_uuid(&bundle_id)?;
        validate_positive_id("node_id", node_id).map_err(|_| invalid_observability())?;
        ReportTimestamp::parse(&first_seen_at)?;
        if stored_workspace.as_deref() != Some(workspace_key)
            || !storage::ALLOWED_NODE_TYPES.contains(&node_type.as_str())
            || !valid_observability_text(&node_title, 512, true)
            || !valid_optional_observability_text(bounded_summary.as_deref(), 2_048)
            || !valid_optional_observability_text(source_ref.as_deref(), 2_048)
            || !valid_optional_required_observability_text(
                trust_level.as_deref(),
                storage::MAX_NODE_TRUST_LEVEL_BYTES,
            )
            || confidence.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || score.is_some_and(|value| !value.is_finite())
        {
            return Err(invalid_observability());
        }
        let selection_reasons: Vec<String> =
            serde_json::from_str(reasons_json).map_err(|_| invalid_observability())?;
        let unique_reasons = selection_reasons.iter().collect::<BTreeSet<_>>();
        if selection_reasons.is_empty()
            || selection_reasons.len() > 64
            || unique_reasons.len() != selection_reasons.len()
            || selection_reasons
                .iter()
                .any(|reason| !SELECTION_REASONS.contains(&reason.as_str()))
        {
            return Err(invalid_observability());
        }
        let line = BundleNodeLine {
            bundle_id,
            node_id,
            first_seen_at,
            node_type,
            node_title: redact_observability_text(&node_title, 512),
            bounded_summary: bounded_summary.map(|value| redact_observability_text(&value, 2_048)),
            source_ref: source_ref.map(|value| redact_observability_text(&value, 2_048)),
            trust_level: trust_level.map(|value| {
                redact_observability_text(&value, storage::MAX_NODE_TRUST_LEVEL_BYTES)
            }),
            confidence,
            score,
            selection_reasons,
        };
        write_json_line(archive, &line)?;
    }
    Ok(())
}

fn write_feedback_jsonl<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    observability: Option<&Transaction<'_>>,
    workspace_key: &str,
) -> Result<(), ExportError> {
    start_entry(archive, CAPSULE_ENTRIES[8])?;
    let Some(observability) = observability else {
        return Ok(());
    };
    let mut statement = observability
        .prepare(
            "SELECT feedback.id, feedback.timestamp, feedback.bundle_id,
                    feedback.outcome, feedback.reason, bundle.workspace_key
             FROM feedback
             LEFT JOIN recall_bundles AS bundle USING (bundle_id)
             ORDER BY feedback.timestamp, feedback.id",
        )
        .map_err(|_| invalid_observability())?;
    let mut rows = statement.query([]).map_err(|_| invalid_observability())?;
    while let Some(row) = rows.next().map_err(|_| invalid_observability())? {
        let id: String = row.get(0).map_err(|_| invalid_observability())?;
        let timestamp: String = row.get(1).map_err(|_| invalid_observability())?;
        let bundle_id: String = row.get(2).map_err(|_| invalid_observability())?;
        let outcome: String = row.get(3).map_err(|_| invalid_observability())?;
        let reason: Option<String> = row.get(4).map_err(|_| invalid_observability())?;
        let stored_workspace: Option<String> = row.get(5).map_err(|_| invalid_observability())?;
        validate_observability_uuid(&id)?;
        validate_observability_uuid(&bundle_id)?;
        ReportTimestamp::parse(&timestamp)?;
        if stored_workspace.as_deref() != Some(workspace_key)
            || !matches!(outcome.as_str(), "useful" | "partial" | "wrong")
            || !valid_optional_required_observability_text(reason.as_deref(), 1_024)
        {
            return Err(invalid_observability());
        }
        let line = FeedbackLine {
            id,
            timestamp,
            bundle_id,
            outcome,
            reason: reason.map(|value| redact_observability_text(&value, 512)),
        };
        write_json_line(archive, &line)?;
    }
    Ok(())
}

fn write_tools_summary<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    operational: &Transaction<'_>,
) -> Result<(), ExportError> {
    let count = scalar_count(operational, "SELECT COUNT(*) FROM tool_contracts")?;
    start_counted_array_entry(archive, CAPSULE_ENTRIES[9], count)?;
    let mut statement = operational
        .prepare(
            "SELECT tool_id, name, status, owner_workflow,
                    side_effects, approval_requirement
             FROM tool_contracts ORDER BY tool_id",
        )
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut rows = statement
        .query([])
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut first = true;
    let mut streamed = 0_u64;
    while let Some(row) = rows.next().map_err(|_| ExportError::WorkspaceInvalid)? {
        let item = tool_summary_from_row(row)?;
        streamed = streamed
            .checked_add(1)
            .ok_or(ExportError::WorkspaceInvalid)?;
        write_json_array_item(archive, &mut first, &item)?;
    }
    if streamed != count {
        return Err(ExportError::WorkspaceInvalid);
    }
    archive.write_all(b"]}\n").map_err(|_| ExportError::Zip)
}

pub(crate) fn tool_summary_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<ToolSummaryItem, ExportError> {
    let tool_id: String = row.get(0).map_err(|_| ExportError::WorkspaceInvalid)?;
    if !safe_tool_id(&tool_id) {
        return Err(ExportError::WorkspaceInvalid);
    }
    let side_effects: String = row.get(4).map_err(|_| ExportError::WorkspaceInvalid)?;
    if !tools::ALLOWED_TOOL_SIDE_EFFECTS.contains(&side_effects.as_str()) {
        return Err(ExportError::WorkspaceInvalid);
    }
    Ok(ToolSummaryItem {
        tool_id: safe_required_text(&tool_id, tools::MAX_TOOL_ID_BYTES, 256)?,
        name: safe_required_text(
            &row.get::<_, String>(1)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            tools::MAX_TOOL_NAME_BYTES,
            512,
        )?,
        status: safe_required_text(
            &row.get::<_, String>(2)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            tools::MAX_TOOL_TEXT_BYTES,
            256,
        )?,
        owner_workflow: safe_optional_required_text(
            row.get::<_, Option<String>>(3)
                .map_err(|_| ExportError::WorkspaceInvalid)?
                .as_deref(),
            tools::MAX_TOOL_TEXT_BYTES,
            512,
        )?,
        side_effects,
        approval_requirement: safe_required_text(
            &row.get::<_, String>(5)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            tools::MAX_TOOL_TEXT_BYTES,
            256,
        )?,
    })
}

fn write_mcp_summary<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    operational: &Transaction<'_>,
) -> Result<(), ExportError> {
    let count = scalar_count(operational, "SELECT COUNT(*) FROM mcp_profiles")?;
    start_counted_array_entry(archive, CAPSULE_ENTRIES[10], count)?;
    let mut statement = operational
        .prepare(
            "SELECT id, name, kind, status, read_operations, write_operations,
                    side_effects, approval_requirement
             FROM mcp_profiles ORDER BY id",
        )
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut rows = statement
        .query([])
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut first = true;
    let mut streamed = 0_u64;
    while let Some(row) = rows.next().map_err(|_| ExportError::WorkspaceInvalid)? {
        let item = mcp_summary_from_row(row)?;
        streamed = streamed
            .checked_add(1)
            .ok_or(ExportError::WorkspaceInvalid)?;
        write_json_array_item(archive, &mut first, &item)?;
    }
    if streamed != count {
        return Err(ExportError::WorkspaceInvalid);
    }
    archive.write_all(b"]}\n").map_err(|_| ExportError::Zip)
}

pub(crate) fn mcp_summary_from_row(row: &rusqlite::Row<'_>) -> Result<McpSummaryItem, ExportError> {
    Ok(McpSummaryItem {
        id: safe_required_text(
            &row.get::<_, String>(0)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            storage::MAX_MCP_ID_BYTES,
            256,
        )?,
        name: safe_required_text(
            &row.get::<_, String>(1)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            storage::MAX_MCP_NAME_BYTES,
            512,
        )?,
        kind: safe_required_text(
            &row.get::<_, String>(2)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            storage::MAX_MCP_FIELD_BYTES,
            256,
        )?,
        status: safe_required_text(
            &row.get::<_, String>(3)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            storage::MAX_MCP_FIELD_BYTES,
            256,
        )?,
        read_operations: safe_required_text(
            &row.get::<_, String>(4)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            storage::MAX_MCP_FIELD_BYTES,
            2_048,
        )?,
        write_operations: safe_required_text(
            &row.get::<_, String>(5)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            storage::MAX_MCP_FIELD_BYTES,
            2_048,
        )?,
        side_effects: safe_required_text(
            &row.get::<_, String>(6)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            storage::MAX_MCP_FIELD_BYTES,
            256,
        )?,
        approval_requirement: safe_required_text(
            &row.get::<_, String>(7)
                .map_err(|_| ExportError::WorkspaceInvalid)?,
            storage::MAX_MCP_FIELD_BYTES,
            256,
        )?,
    })
}

fn scalar_count(transaction: &Transaction<'_>, sql: &str) -> Result<u64, ExportError> {
    let value: i64 = transaction
        .query_row(sql, [], |row| row.get(0))
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    nonnegative_count(value)
}

fn grouped_counts(
    transaction: &Transaction<'_>,
    sql: &str,
    allowed: &[&str],
) -> Result<Vec<NamedCount>, ExportError> {
    let mut statement = transaction
        .prepare(sql)
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut rows = statement
        .query([])
        .map_err(|_| ExportError::WorkspaceInvalid)?;
    let mut counts = Vec::with_capacity(allowed.len());
    while let Some(row) = rows.next().map_err(|_| ExportError::WorkspaceInvalid)? {
        let name: String = row.get(0).map_err(|_| ExportError::WorkspaceInvalid)?;
        let count: i64 = row.get(1).map_err(|_| ExportError::WorkspaceInvalid)?;
        if !allowed.contains(&name.as_str()) || counts.len() >= allowed.len() {
            return Err(ExportError::WorkspaceInvalid);
        }
        counts.push(NamedCount {
            name,
            count: nonnegative_count(count)?,
        });
    }
    Ok(counts)
}

fn nonnegative_count(value: i64) -> Result<u64, ExportError> {
    u64::try_from(value).map_err(|_| ExportError::WorkspaceInvalid)
}

fn invalid_observability() -> ExportError {
    ExportError::Observability(ObserveReadError::InvalidStore)
}

fn nonnegative_observability(value: i64) -> Result<u64, ExportError> {
    u64::try_from(value).map_err(|_| invalid_observability())
}

fn validate_observability_uuid(value: &str) -> Result<(), ExportError> {
    validate_uuid_v4(value)
        .map(|_| ())
        .map_err(|_| invalid_observability())
}

fn valid_observability_identifier(value: &str) -> bool {
    validate_ascii_identifier("stored_identifier", value, 128).is_ok()
}

fn valid_observability_text(value: &str, maximum_bytes: usize, required: bool) -> bool {
    !value.as_bytes().contains(&0)
        && value.len() <= maximum_bytes
        && (!required || !value.trim().is_empty())
}

fn valid_optional_observability_text(value: Option<&str>, maximum_bytes: usize) -> bool {
    value.is_none_or(|value| valid_observability_text(value, maximum_bytes, false))
}

fn valid_optional_required_observability_text(value: Option<&str>, maximum_bytes: usize) -> bool {
    value.is_none_or(|value| valid_observability_text(value, maximum_bytes, true))
}

fn redact_observability_text(value: &str, maximum_bytes: usize) -> String {
    truncate_utf8(redact_sensitive_text(value), maximum_bytes)
}

fn safe_required_text(
    value: &str,
    input_limit: usize,
    output_limit: usize,
) -> Result<String, ExportError> {
    if value.as_bytes().contains(&0) || value.trim().is_empty() || value.len() > input_limit {
        return Err(ExportError::WorkspaceInvalid);
    }
    Ok(truncate_utf8(redact_sensitive_text(value), output_limit))
}

fn safe_optional_text(
    value: Option<&str>,
    input_limit: usize,
    output_limit: usize,
) -> Result<Option<String>, ExportError> {
    value
        .map(|value| {
            if value.as_bytes().contains(&0) || value.len() > input_limit {
                return Err(ExportError::WorkspaceInvalid);
            }
            Ok(truncate_utf8(redact_sensitive_text(value), output_limit))
        })
        .transpose()
}

fn safe_optional_required_text(
    value: Option<&str>,
    input_limit: usize,
    output_limit: usize,
) -> Result<Option<String>, ExportError> {
    value
        .map(|value| safe_required_text(value, input_limit, output_limit))
        .transpose()
}

fn safe_tool_id(value: &str) -> bool {
    !value.contains(['/', '\\']) && !matches!(value, "." | "..")
}

fn start_counted_array_entry<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    name: &str,
    count: u64,
) -> Result<(), ExportError> {
    start_entry(archive, name)?;
    write!(archive, "{{\"count\":{count},\"items\":[").map_err(|_| ExportError::Zip)
}

fn write_json_array_item<W: Write + Seek, T: Serialize>(
    archive: &mut ZipWriter<W>,
    first: &mut bool,
    item: &T,
) -> Result<(), ExportError> {
    if !*first {
        archive.write_all(b",").map_err(|_| ExportError::Zip)?;
    }
    *first = false;
    serde_json::to_writer(&mut *archive, item).map_err(|_| ExportError::Serialization)
}

fn write_json_line<W: Write + Seek, T: Serialize>(
    archive: &mut ZipWriter<W>,
    value: &T,
) -> Result<(), ExportError> {
    serde_json::to_writer(&mut *archive, value).map_err(|_| ExportError::Serialization)?;
    archive.write_all(b"\n").map_err(|_| ExportError::Zip)
}

fn write_json_entry<W: Write + Seek, T: Serialize>(
    archive: &mut ZipWriter<W>,
    name: &str,
    value: &T,
) -> Result<(), ExportError> {
    start_entry(archive, name)?;
    serde_json::to_writer(&mut *archive, value).map_err(|_| ExportError::Serialization)?;
    archive.write_all(b"\n").map_err(|_| ExportError::Zip)
}

fn start_entry<W: Write + Seek>(archive: &mut ZipWriter<W>, name: &str) -> Result<(), ExportError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(ZIP_ENTRY_PERMISSIONS)
        .large_file(true);
    archive
        .start_file(name, options)
        .map_err(|_| ExportError::Zip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use serde_json::Value;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::io::Read;
    use std::sync::MutexGuard;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::ZipArchive;

    const EVENT_ID: &str = "11111111-1111-4111-8111-111111111111";
    const CORRELATION_ID: &str = "22222222-2222-4222-8222-222222222222";
    const BUNDLE_ID: &str = "33333333-3333-4333-8333-333333333333";
    const FEEDBACK_ID: &str = "44444444-4444-4444-8444-444444444444";
    const DOCTOR_ID: &str = "55555555-5555-4555-8555-555555555555";
    const LEGACY_DETERMINISTIC_REFERENCE_SQL: &str = "SELECT MAX(timestamp) FROM (
            SELECT timestamp FROM observability_events
            UNION ALL SELECT timestamp FROM recall_bundles
            UNION ALL SELECT first_seen_at AS timestamp FROM bundle_nodes
            UNION ALL SELECT timestamp FROM feedback
            UNION ALL SELECT last_retention_at AS timestamp FROM collector_state
            UNION ALL SELECT retention_floor_at AS timestamp FROM collector_state
         )";

    struct TestWorkspace {
        paths: WorkspacePaths,
        key: String,
        home: PathBuf,
        output_dir: PathBuf,
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
            let global = storage::resolve_paths().expect("test AOPMEM_HOME should resolve");
            storage::ensure_global_dirs(&global).expect("global directories should create");
            let key = format!("{name}-workspace");
            let paths = storage::ensure_workspace_dirs(&global, &key)
                .expect("workspace directories should create");
            drop(storage::open_workspace_db(&paths).expect("operational DB should initialize"));
            let output_dir = home.join("exports");
            fs::create_dir(&output_dir).expect("output directory should create");
            Self {
                paths,
                key,
                home,
                output_dir,
                original_home,
                _lock: lock,
            }
        }

        fn output(&self, name: &str) -> PathBuf {
            self.output_dir.join(name)
        }

        fn add_workspace(&self, key: &str) -> WorkspacePaths {
            let global = storage::resolve_paths().expect("test AOPMEM_HOME should resolve");
            let paths = storage::ensure_workspace_dirs(&global, key)
                .expect("second workspace should create");
            drop(storage::open_workspace_db(&paths).expect("second DB should initialize"));
            paths
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
            .expect("test clock should follow epoch")
            .as_nanos();
        env::temp_dir().join(format!(
            "aopmem-export-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn mutate_database(path: &Path, action: impl FnOnce(&Connection)) {
        let connection = Connection::open(path).expect("fixture DB should open");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign keys should enable");
        action(&connection);
        let checkpoint: (i64, i64, i64) = connection
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .expect("fixture WAL should checkpoint");
        assert_eq!(checkpoint.0, 0, "fixture checkpoint should not be busy");
    }

    fn initialize_observability(workspace: &TestWorkspace) {
        drop(
            super::super::open_writer(&workspace.paths)
                .expect("observability DB should initialize"),
        );
    }

    fn insert_privacy_fixture(workspace: &TestWorkspace) {
        mutate_database(workspace.paths.db(), |connection| {
            connection
                .execute(
                    "INSERT INTO nodes (
                        node_type, status, title, summary, body, source_ref,
                        confidence, trust_level
                     ) VALUES ('rule', 'draft', 'Draft empty optional', '',
                        'FULL_NODE_BODY_CANARY', '', NULL, '')",
                    [],
                )
                .expect("draft node should insert");
            connection
                .execute(
                    "INSERT INTO nodes (
                        node_type, status, title, summary, body, source_ref,
                        confidence, trust_level
                     ) VALUES ('workflow', 'active', 'Safe workflow', ?1,
                        'SECOND_FULL_BODY_CANARY', ?2, 0.9, 'user')",
                    params![
                        "registry=https://build-user:uri-password@packages.example/v1 token=summary-secret public=видимый",
                        "https://source-user:source-password@example.test/path?access_token=source-secret"
                    ],
                )
                .expect("active node should insert");
            connection
                .execute(
                    "INSERT INTO tool_contracts (
                        tool_id, name, status, owner_workflow, side_effects,
                        approval_requirement, contract_json
                     ) VALUES ('safe-tool', 'Safe tool', 'draft', NULL, 'none',
                        'none', ?1)",
                    [r#"{"raw":"RAW_TOOL_CONTRACT_CANARY"}"#],
                )
                .expect("tool fixture should insert");
            connection
                .execute(
                    "INSERT INTO mcp_profiles (
                        id, name, kind, status, read_operations,
                        write_operations, side_effects, approval_requirement,
                        credentials_source, notes
                     ) VALUES ('safe-mcp', 'Safe MCP', 'stdio',
                        'configured_unverified', ?1, 'none', 'none', 'none',
                        'ENV:RAW_MCP_CREDENTIAL_CANARY', 'RAW_MCP_NOTES_CANARY')",
                    ["read Authorization: Bearer mcp-operation-secret public"],
                )
                .expect("MCP fixture should insert");
        });

        initialize_observability(workspace);
        mutate_database(workspace.paths.observability_db(), |connection| {
            connection
                .execute(
                    "INSERT INTO observability_events (
                        id, timestamp, product_version, workspace_key,
                        event_type, command, correlation_id, bundle_id,
                        duration_ms, outcome, error_code, payload_json
                     ) VALUES (?1, '2026-07-15T10:00:00.000Z', ?2, ?3,
                        'node.created', 'node_create', ?4, NULL, 3,
                        'recorded', NULL, ?5)",
                    params![
                        EVENT_ID,
                        env!("CARGO_PKG_VERSION"),
                        workspace.key,
                        CORRELATION_ID,
                        r#"{"kind":"node","data":{"node_id":2,"node_type":"workflow","title":"Safe event title","summary":"RAW_EVENT_PAYLOAD_CANARY","source_ref":null}}"#,
                    ],
                )
                .expect("event fixture should insert");
            connection
                .execute(
                    "INSERT INTO recall_bundles (
                        bundle_id, timestamp, product_version, workspace_key,
                        correlation_id, outcome, error_code, duration_ms,
                        more_results, continuation_count
                     ) VALUES (?1, '2026-07-15T10:01:00.000Z', ?2, ?3,
                        ?4, 'success', NULL, 9, 0, 0)",
                    params![
                        BUNDLE_ID,
                        env!("CARGO_PKG_VERSION"),
                        workspace.key,
                        CORRELATION_ID,
                    ],
                )
                .expect("bundle fixture should insert");
            connection
                .execute(
                    "INSERT INTO bundle_nodes (
                        bundle_id, node_id, first_seen_at, node_type,
                        node_title, bounded_summary, source_ref, trust_level,
                        confidence, score, selection_reasons_json
                     ) VALUES (?1, 2, '2026-07-15T10:01:00.000Z',
                        'workflow', 'Safe workflow', '', '', 'user', 0.9,
                        1.25, '[\"workflow\"]')",
                    [BUNDLE_ID],
                )
                .expect("bundle node fixture should insert");
            connection
                .execute(
                    "INSERT INTO feedback (id, timestamp, bundle_id, outcome, reason)
                     VALUES (?1, '2026-07-15T10:02:00.000Z', ?2, 'useful',
                        'Authorization: Bearer feedback-secret; useful detail')",
                    params![FEEDBACK_ID, BUNDLE_ID],
                )
                .expect("feedback fixture should insert");
            connection
                .execute(
                    "INSERT INTO observability_events (
                        id, timestamp, product_version, workspace_key,
                        event_type, command, correlation_id, bundle_id,
                        duration_ms, outcome, error_code, payload_json
                     ) VALUES (?1, '2026-07-15T10:03:00.000Z', ?2, ?3,
                        'doctor', 'doctor', ?4, NULL, 4, 'warning', NULL, ?5)",
                    params![
                        DOCTOR_ID,
                        env!("CARGO_PKG_VERSION"),
                        workspace.key,
                        CORRELATION_ID,
                        r#"{"kind":"counts","data":{"items":[{"name":"checks","count":9},{"name":"ready","count":8},{"name":"missing","count":1},{"name":"error","count":0}]}}"#,
                    ],
                )
                .expect("doctor fixture should insert");
        });
    }

    fn archive_entries(path: &Path) -> Vec<(String, Vec<u8>)> {
        let file = File::open(path).expect("capsule should open");
        let mut archive = ZipArchive::new(file).expect("capsule should be a ZIP");
        let mut entries = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).expect("ZIP entry should open");
            let name = entry.name().to_string();
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .expect("ZIP entry should read");
            entries.push((name, bytes));
        }
        entries
    }

    fn entry_json(entries: &[(String, Vec<u8>)], name: &str) -> Value {
        let bytes = &entries
            .iter()
            .find(|(entry_name, _)| entry_name == name)
            .expect("named ZIP entry should exist")
            .1;
        serde_json::from_slice(bytes).expect("entry should contain JSON")
    }

    fn capsule_temporary_names(directory: &Path) -> Vec<OsString> {
        let mut names = fs::read_dir(directory)
            .expect("output directory should read")
            .map(|entry| entry.expect("directory entry should read").file_name())
            .filter(|name| name.to_string_lossy().starts_with(TEMP_NAME_PREFIX))
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    #[derive(Debug, PartialEq, Eq)]
    struct FileSnapshot {
        bytes: Vec<u8>,
        modified: SystemTime,
    }

    fn file_snapshot(path: &Path) -> FileSnapshot {
        let metadata = fs::metadata(path).expect("database file should stat");
        FileSnapshot {
            bytes: fs::read(path).expect("database file should read"),
            modified: metadata
                .modified()
                .expect("database file should have mtime"),
        }
    }

    fn operational_counts(path: &Path) -> (i64, i64, i64, i64, i64) {
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("operational DB should open read-only");
        connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM nodes),
                        (SELECT COUNT(*) FROM links),
                        (SELECT COUNT(*) FROM tool_contracts),
                        (SELECT COUNT(*) FROM mcp_profiles),
                        (SELECT COUNT(*) FROM schema_migrations)",
                [],
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
            .expect("operational counts should read")
    }

    fn observability_counts(path: &Path) -> (i64, i64, i64, i64, i64) {
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("observability DB should open read-only");
        connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM observability_events),
                        (SELECT COUNT(*) FROM recall_bundles),
                        (SELECT COUNT(*) FROM bundle_nodes),
                        (SELECT COUNT(*) FROM feedback),
                        (SELECT schema_version FROM collector_state WHERE singleton_id = 1)",
                [],
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
            .expect("observability counts should read")
    }

    fn reference_value(connection: &Connection, sql: &str) -> Option<String> {
        connection
            .query_row(sql, [], |row| row.get(0))
            .expect("reference timestamp query should read")
    }

    #[test]
    fn deterministic_reference_max_per_branch_matches_legacy_null_semantics() {
        let workspace = TestWorkspace::new("reference-parity");
        initialize_observability(&workspace);
        mutate_database(workspace.paths.observability_db(), |connection| {
            assert_eq!(
                reference_value(connection, DETERMINISTIC_REFERENCE_SQL),
                reference_value(connection, LEGACY_DETERMINISTIC_REFERENCE_SQL)
            );
            assert_eq!(
                reference_value(connection, DETERMINISTIC_REFERENCE_SQL),
                None
            );

            connection
                .execute(
                    "INSERT INTO observability_events (
                        id, timestamp, product_version, workspace_key, event_type,
                        command, correlation_id, bundle_id, duration_ms, outcome,
                        error_code, payload_json
                     ) VALUES (?1, '2026-01-01T00:00:00.000Z', 'test', ?2,
                               'install.completed', 'fixture', ?3, NULL, 1,
                               'success', NULL, ?4)",
                    params![
                        EVENT_ID,
                        workspace.key,
                        CORRELATION_ID,
                        r#"{"kind":"empty"}"#
                    ],
                )
                .expect("reference event should insert");
            connection
                .execute(
                    "INSERT INTO recall_bundles (
                        bundle_id, timestamp, product_version, workspace_key,
                        correlation_id, outcome, error_code, duration_ms,
                        more_results, continuation_count
                     ) VALUES (?1, '2026-01-02T00:00:00.000Z', 'test', ?2,
                               ?3, 'success', NULL, 1, 0, 0)",
                    params![BUNDLE_ID, workspace.key, CORRELATION_ID],
                )
                .expect("reference bundle should insert");
            connection
                .execute(
                    "INSERT INTO bundle_nodes (
                        bundle_id, node_id, first_seen_at, node_type, node_title,
                        bounded_summary, source_ref, trust_level, confidence,
                        score, selection_reasons_json
                     ) VALUES (?1, 1, '2026-01-03T00:00:00.000Z', 'workflow',
                               'Reference workflow', NULL, NULL, NULL, NULL,
                               NULL, ?2)",
                    params![BUNDLE_ID, r#"["typed_root"]"#],
                )
                .expect("reference bundle node should insert");
            connection
                .execute(
                    "INSERT INTO feedback (id, timestamp, bundle_id, outcome, reason)
                     VALUES (?1, '2026-01-06T00:00:00.000Z', ?2, 'useful', NULL)",
                    params![FEEDBACK_ID, BUNDLE_ID],
                )
                .expect("reference feedback should insert");
            connection
                .execute(
                    "UPDATE collector_state
                     SET last_retention_at = '2026-01-06T00:00:00.000Z',
                         retention_floor_at = '2026-01-04T00:00:00.000Z'
                     WHERE singleton_id = 1",
                    [],
                )
                .expect("reference collector state should update");

            let optimized = reference_value(connection, DETERMINISTIC_REFERENCE_SQL);
            assert_eq!(
                optimized,
                reference_value(connection, LEGACY_DETERMINISTIC_REFERENCE_SQL)
            );
            assert_eq!(optimized.as_deref(), Some("2026-01-06T00:00:00.000Z"));
            let transaction = connection
                .unchecked_transaction()
                .expect("reference transaction should start");
            let reference = deterministic_reference_at(&transaction)
                .expect("optimized deterministic reference should read");
            assert_eq!(reference.as_str(), "2026-01-06T00:00:00.000Z");
            transaction
                .commit()
                .expect("reference transaction should commit");
        });
    }

    #[test]
    fn deterministic_reference_max_per_branch_uses_timestamp_indexes() {
        let workspace = TestWorkspace::new("reference-plan");
        initialize_observability(&workspace);
        mutate_database(workspace.paths.observability_db(), |connection| {
            let explain = format!("EXPLAIN QUERY PLAN {DETERMINISTIC_REFERENCE_SQL}");
            let mut statement = connection
                .prepare(&explain)
                .expect("reference query plan should prepare");
            let plan = statement
                .query_map([], |row| row.get::<_, String>(3))
                .expect("reference query plan should run")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("reference query plan should collect")
                .join("\n");
            for index in [
                "idx_observability_events_timestamp",
                "idx_recall_bundles_timestamp",
                "idx_bundle_nodes_first_seen_at",
                "idx_feedback_timestamp",
            ] {
                assert!(
                    plan.contains(index),
                    "reference query plan did not use {index}: {plan}"
                );
            }
        });
    }

    #[test]
    fn export_is_exact_deterministic_private_and_accepts_empty_optional_text() {
        let workspace = TestWorkspace::new("exact-private");
        insert_privacy_fixture(&workspace);
        env::set_var("AOPMEM_EXPORT_ENV_CANARY", "RAW_ENV_VALUE_CANARY");
        let first = workspace.output("first capsule.zip");
        let second = workspace.output("second-capsule.zip");

        let first_result = export_debug_capsule(&workspace.key, &workspace.paths, &first)
            .expect("first export should succeed");
        let _second_result = export_debug_capsule(&workspace.key, &workspace.paths, &second)
            .expect("second export should succeed");
        env::remove_var("AOPMEM_EXPORT_ENV_CANARY");

        assert_eq!(first_result.publication_status, PublicationStatus::Durable);
        assert!(first_result.temporary_cleanup_confirmed);
        assert_eq!(first_result.collection_status, CollectionStatus::Ready);
        assert_eq!(first_result.reference_at, "2026-07-15T10:03:00.000Z");
        assert_eq!(
            fs::read(&first).expect("first capsule should read"),
            fs::read(&second).expect("second capsule should read"),
            "same snapshot must produce byte-identical capsules"
        );

        let entries = archive_entries(&first);
        assert_eq!(
            entries
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            CAPSULE_ENTRIES
        );
        let manifest = entry_json(&entries, "manifest.json");
        assert_eq!(manifest["entries"], serde_json::json!(CAPSULE_ENTRIES));
        assert_eq!(manifest["deterministic"], true);
        let health = entry_json(&entries, "health.json");
        assert_eq!(health["collection_status"], "ready");
        assert_eq!(health["doctor"]["status"], "warning");
        assert_eq!(health["verify"]["status"], "not_collected");
        let memory = entry_json(&entries, "memory_summary.json");
        assert_eq!(memory["node_count"], 2);
        assert_eq!(memory["draft"], 1);
        assert_eq!(memory["nodes"][0]["summary"], "");
        assert_eq!(memory["nodes"][0]["source_ref"], "");
        assert_eq!(memory["nodes"][0]["trust_level"], "");
        let bundle_lines = entries
            .iter()
            .find(|(name, _)| name == "bundle_nodes.jsonl")
            .expect("bundle nodes should exist");
        let bundle_node: Value =
            serde_json::from_slice(&bundle_lines.1).expect("bundle node line should parse");
        assert_eq!(bundle_node["bounded_summary"], "");
        assert_eq!(bundle_node["source_ref"], "");
        let event_line: Value = serde_json::from_slice(
            entries
                .iter()
                .find(|(name, _)| name == "events.jsonl")
                .expect("events should exist")
                .1
                .split(|byte| *byte == b'\n')
                .next()
                .expect("first event line should exist"),
        )
        .expect("event line should parse");
        assert!(event_line.get("payload_json").is_none());

        let all_bytes = fs::read(&first).expect("capsule bytes should read");
        let all_text = String::from_utf8_lossy(&all_bytes);
        for canary in [
            "FULL_NODE_BODY_CANARY",
            "SECOND_FULL_BODY_CANARY",
            "RAW_TOOL_CONTRACT_CANARY",
            "RAW_MCP_CREDENTIAL_CANARY",
            "RAW_MCP_NOTES_CANARY",
            "RAW_EVENT_PAYLOAD_CANARY",
            "RAW_ENV_VALUE_CANARY",
            "uri-password",
            "source-password",
            "summary-secret",
            "source-secret",
            "mcp-operation-secret",
            "feedback-secret",
        ] {
            assert!(!all_text.contains(canary), "secret survived: {canary}");
        }
        assert!(all_text.contains("видимый"));
    }

    #[test]
    fn missing_observability_exports_explicit_not_collected_and_empty_jsonl() {
        let workspace = TestWorkspace::new("missing-observability");
        let output = workspace.output("missing-observability.zip");

        let result = export_debug_capsule(&workspace.key, &workspace.paths, &output)
            .expect("missing observability should be a successful export");

        assert_eq!(result.collection_status, CollectionStatus::NotCollected);
        assert_eq!(result.reference_at, FIXED_EMPTY_REFERENCE_AT);
        assert!(!workspace.paths.observability().exists());
        let entries = archive_entries(&output);
        for name in [
            "events.jsonl",
            "recall_bundles.jsonl",
            "bundle_nodes.jsonl",
            "feedback.jsonl",
        ] {
            assert!(
                entries
                    .iter()
                    .find(|(entry_name, _)| entry_name == name)
                    .expect("JSONL entry should exist")
                    .1
                    .is_empty(),
                "{name} should be empty"
            );
        }
        let health = entry_json(&entries, "health.json");
        assert_eq!(health["collection_status"], "not_collected");
        assert_eq!(health["doctor"]["status"], "not_collected");
        assert_eq!(health["verify"]["status"], "not_collected");
        let workspace_summary = entry_json(&entries, "workspace_summary.json");
        assert_eq!(workspace_summary["effectiveness"], Value::Null);
    }

    #[test]
    fn workspace_key_and_paths_mismatch_fails_before_output() {
        let workspace = TestWorkspace::new("binding");
        let other_key = "other-workspace";
        let other_paths = workspace.add_workspace(other_key);
        let output = workspace.output("must-not-exist.zip");

        let error = export_debug_capsule(&workspace.key, &other_paths, &output)
            .expect_err("cross-workspace binding must fail");

        assert!(matches!(error, ExportError::WorkspaceInvalid));
        assert!(!output.exists());
        assert!(capsule_temporary_names(&workspace.output_dir).is_empty());
    }

    #[test]
    fn foreign_workspace_observability_row_fails_closed() {
        let workspace = TestWorkspace::new("foreign-observability");
        initialize_observability(&workspace);
        mutate_database(workspace.paths.observability_db(), |connection| {
            connection
                .execute(
                    "INSERT INTO recall_bundles (
                        bundle_id, timestamp, product_version, workspace_key,
                        correlation_id, outcome, error_code, duration_ms,
                        more_results, continuation_count
                     ) VALUES (?1, '2026-07-15T10:00:00.000Z', ?2,
                        'foreign-workspace', ?3, 'success', NULL, 1, 0, 0)",
                    params![BUNDLE_ID, env!("CARGO_PKG_VERSION"), CORRELATION_ID],
                )
                .expect("foreign bundle fixture should insert");
        });
        let output = workspace.output("foreign.zip");

        let error = export_debug_capsule(&workspace.key, &workspace.paths, &output)
            .expect_err("foreign workspace data must fail closed");

        assert!(matches!(
            error,
            ExportError::Observability(ObserveReadError::InvalidStore)
        ));
        assert!(!output.exists());
        assert!(capsule_temporary_names(&workspace.output_dir).is_empty());
    }

    #[test]
    fn incompatible_operational_schema_and_fts_fail_before_output() {
        let workspace = TestWorkspace::new("schema-manifest");
        mutate_database(workspace.paths.db(), |connection| {
            connection
                .execute("DROP TABLE tags", [])
                .expect("fixture table should drop");
        });
        let output = workspace.output("missing-table.zip");
        assert!(matches!(
            export_debug_capsule(&workspace.key, &workspace.paths, &output),
            Err(ExportError::WorkspaceInvalid)
        ));
        assert!(!output.exists());

        let fts_key = "missing-fts-workspace";
        let fts_paths = workspace.add_workspace(fts_key);
        mutate_database(fts_paths.db(), |connection| {
            connection
                .execute("DROP TABLE fts_nodes", [])
                .expect("fixture FTS table should drop");
        });
        let fts_output = workspace.output("missing-fts.zip");
        assert!(matches!(
            export_debug_capsule(fts_key, &fts_paths, &fts_output),
            Err(ExportError::WorkspaceInvalid)
        ));
        assert!(!fts_output.exists());
        assert!(capsule_temporary_names(&workspace.output_dir).is_empty());
    }

    #[test]
    fn operational_foreign_key_violation_fails_integrity_check() {
        let workspace = TestWorkspace::new("foreign-key");
        let connection = Connection::open(workspace.paths.db()).expect("fixture DB should open");
        connection
            .execute_batch("PRAGMA foreign_keys = OFF;")
            .expect("fixture foreign keys should disable");
        connection
            .execute(
                "INSERT INTO links (source_node_id, target_node_id, link_type)
                 VALUES (999, 1000, 'depends_on')",
                [],
            )
            .expect("broken fixture link should insert");
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("fixture WAL should checkpoint");
        drop(connection);
        let output = workspace.output("broken-foreign-key.zip");

        assert!(matches!(
            export_debug_capsule(&workspace.key, &workspace.paths, &output),
            Err(ExportError::WorkspaceInvalid)
        ));
        assert!(!output.exists());
        assert!(capsule_temporary_names(&workspace.output_dir).is_empty());
    }

    #[test]
    fn existing_output_is_never_overwritten() {
        let workspace = TestWorkspace::new("no-clobber");
        let output = workspace.output("existing.zip");
        fs::write(&output, b"existing-user-file").expect("existing output should write");

        let error = export_debug_capsule(&workspace.key, &workspace.paths, &output)
            .expect_err("existing output must not be overwritten");

        assert!(matches!(error, ExportError::OutputExists));
        assert_eq!(
            fs::read(&output).expect("existing output should read"),
            b"existing-user-file"
        );
        assert!(capsule_temporary_names(&workspace.output_dir).is_empty());
    }

    #[test]
    fn late_observability_validation_failure_removes_temporary_archive() {
        let workspace = TestWorkspace::new("late-failure-cleanup");
        insert_privacy_fixture(&workspace);
        mutate_database(workspace.paths.observability_db(), |connection| {
            connection
                .execute(
                    "INSERT INTO observability_events (
                        id, timestamp, product_version, workspace_key,
                        event_type, command, correlation_id, bundle_id,
                        duration_ms, outcome, error_code, payload_json
                     ) VALUES ('66666666-6666-4666-8666-666666666666',
                        '2020-01-01T00:00:00.000Z', ?1, ?2, 'unknown.event',
                        'unknown', ?3, NULL, 1, 'success', NULL,
                        '{\"kind\":\"empty\"}')",
                    params![env!("CARGO_PKG_VERSION"), workspace.key, CORRELATION_ID],
                )
                .expect("late-invalid event should insert");
        });
        let output = workspace.output("late-failure.zip");

        let error = export_debug_capsule(&workspace.key, &workspace.paths, &output)
            .expect_err("invalid historical row must fail closed");

        assert!(matches!(
            error,
            ExportError::Observability(ObserveReadError::InvalidStore)
        ));
        assert!(!output.exists());
        assert!(capsule_temporary_names(&workspace.output_dir).is_empty());
    }

    #[test]
    fn publication_warning_is_stable_and_keeps_core_success_typed() {
        let (status, warning) = publication_result(CommittedPublishOutcome {
            durability_confirmed: false,
            temporary_cleanup_confirmed: false,
        });

        assert_eq!(status, PublicationStatus::PublishedWithWarning);
        let warning = warning.expect("uncertain publication should warn");
        assert_eq!(warning.code, EXPORT_PUBLISHED_WITH_WARNING);
        assert!(warning.message.contains("was published"));
    }

    #[test]
    fn export_does_not_mutate_operational_or_observability_databases() {
        let workspace = TestWorkspace::new("read-only-proof");
        insert_privacy_fixture(&workspace);
        let operational_before = file_snapshot(workspace.paths.db());
        let observability_before = file_snapshot(workspace.paths.observability_db());
        let operational_counts_before = operational_counts(workspace.paths.db());
        let observability_counts_before = observability_counts(workspace.paths.observability_db());
        let output = workspace.output("read-only-proof.zip");

        export_debug_capsule(&workspace.key, &workspace.paths, &output)
            .expect("read-only export should succeed");

        assert_eq!(file_snapshot(workspace.paths.db()), operational_before);
        assert_eq!(
            file_snapshot(workspace.paths.observability_db()),
            observability_before
        );
        assert_eq!(
            operational_counts(workspace.paths.db()),
            operational_counts_before
        );
        assert_eq!(
            observability_counts(workspace.paths.observability_db()),
            observability_counts_before
        );
        for database in [workspace.paths.db(), workspace.paths.observability_db()] {
            let mut journal = database.as_os_str().to_os_string();
            journal.push("-journal");
            assert!(
                !PathBuf::from(journal).exists(),
                "export must not create a rollback journal"
            );
        }
    }

    #[test]
    fn large_workspace_streams_ten_thousand_nodes_and_thirty_thousand_links() {
        let workspace = TestWorkspace::new("large-streaming");
        mutate_database(workspace.paths.db(), |connection| {
            connection
                .execute_batch(
                    "WITH RECURSIVE sequence(value) AS (
                        SELECT 1 UNION ALL
                        SELECT value + 1 FROM sequence WHERE value < 10000
                     )
                     INSERT INTO nodes (
                        node_type, status, title, summary, body, source_ref,
                        confidence, trust_level
                     )
                     SELECT 'rule', 'draft', 'Synthetic node ' || value,
                        '', NULL, '', NULL, '' FROM sequence;
                     WITH RECURSIVE sequence(value) AS (
                        SELECT 1 UNION ALL
                        SELECT value + 1 FROM sequence WHERE value < 30000
                     )
                     INSERT INTO links (
                        source_node_id, target_node_id, link_type
                     )
                     SELECT ((value - 1) % 10000) + 1,
                        (value % 10000) + 1, 'depends_on' FROM sequence;",
                )
                .expect("large synthetic fixture should insert");
        });
        let output = workspace.output("large.zip");

        export_debug_capsule(&workspace.key, &workspace.paths, &output)
            .expect("large export should succeed");

        let entries = archive_entries(&output);
        let memory = entry_json(&entries, "memory_summary.json");
        assert_eq!(memory["node_count"], 10_000);
        assert_eq!(memory["link_count"], 30_000);
        assert_eq!(
            memory["nodes"]
                .as_array()
                .expect("nodes should be an array")
                .len(),
            10_000
        );
    }

    #[test]
    fn corrupt_operational_and_observability_databases_fail_without_output() {
        let workspace = TestWorkspace::new("corrupt-databases");
        let corrupt_operational_key = "corrupt-operational-workspace";
        let corrupt_operational = workspace.add_workspace(corrupt_operational_key);
        fs::write(corrupt_operational.db(), b"not a SQLite database")
            .expect("operational corruption fixture should write");
        let operational_output = workspace.output("corrupt-operational.zip");
        assert!(matches!(
            export_debug_capsule(
                corrupt_operational_key,
                &corrupt_operational,
                &operational_output
            ),
            Err(ExportError::WorkspaceInvalid)
        ));
        assert!(!operational_output.exists());

        initialize_observability(&workspace);
        fs::write(
            workspace.paths.observability_db(),
            b"not an observability SQLite database",
        )
        .expect("observability corruption fixture should write");
        let observability_output = workspace.output("corrupt-observability.zip");
        assert!(matches!(
            export_debug_capsule(&workspace.key, &workspace.paths, &observability_output),
            Err(ExportError::Observability(ObserveReadError::InvalidStore))
        ));
        assert!(!observability_output.exists());
        assert!(capsule_temporary_names(&workspace.output_dir).is_empty());
    }

    #[test]
    fn missing_output_parent_is_rejected_without_creating_directories() {
        let workspace = TestWorkspace::new("missing-output-parent");
        let missing_parent = workspace.home.join("missing-parent");
        let output = missing_parent.join("capsule.zip");

        assert!(matches!(
            export_debug_capsule(&workspace.key, &workspace.paths, &output),
            Err(ExportError::UnsafeOutput)
        ));
        assert!(!missing_parent.exists());
    }

    #[cfg(unix)]
    #[test]
    fn anchored_publish_rejects_replaced_temporary_name_against_original_handle() {
        let workspace = TestWorkspace::new("source-identity");
        let directory = AnchoredDir::open_workspace(&workspace.output_dir, None)
            .expect("output directory should anchor");
        let source_name = OsStr::new("source.tmp");
        let destination_name = OsStr::new("destination.zip");
        let mut original = directory
            .create_new_regular_os(source_name)
            .expect("original source should create");
        original
            .write_all(b"trusted capsule")
            .expect("original source should write");
        original.sync_all().expect("original source should sync");
        directory
            .remove_regular_os(source_name)
            .expect("original source name should remove");
        let mut replacement = directory
            .create_new_regular_os(source_name)
            .expect("replacement source should create");
        replacement
            .write_all(b"attacker replacement")
            .expect("replacement should write");
        replacement.sync_all().expect("replacement should sync");

        assert!(directory
            .publish_regular_no_replace_committed_os(&original, source_name, destination_name)
            .is_err());
        assert!(directory
            .open_regular_optional_os(destination_name)
            .expect("destination lookup should succeed")
            .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_output_and_observability_paths_are_rejected() {
        use std::os::unix::fs::symlink;

        let workspace = TestWorkspace::new("unsafe-links");
        let outside_file = workspace.home.join("outside.zip");
        fs::write(&outside_file, b"outside").expect("outside fixture should write");
        let output_link = workspace.output("linked.zip");
        symlink(&outside_file, &output_link).expect("output symlink should create");
        assert!(matches!(
            export_debug_capsule(&workspace.key, &workspace.paths, &output_link),
            Err(ExportError::UnsafeOutput)
        ));
        assert_eq!(
            fs::read(&outside_file).expect("outside file should read"),
            b"outside"
        );

        let outside_directory = workspace.home.join("outside-observability");
        fs::create_dir(&outside_directory).expect("outside directory should create");
        symlink(&outside_directory, workspace.paths.observability())
            .expect("observability symlink should create");
        let safe_output = workspace.output("unsafe-observability.zip");
        assert!(matches!(
            export_debug_capsule(&workspace.key, &workspace.paths, &safe_output),
            Err(ExportError::Observability(ObserveReadError::UnsafePath))
        ));
        assert!(!safe_output.exists());
        assert!(capsule_temporary_names(&workspace.output_dir).is_empty());
    }
}
