use clap::{error::ErrorKind, Args, Parser, Subcommand};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use crate::adapter;
use crate::artifacts;
use crate::audit;
use crate::install;
use crate::mutation;
use crate::observability::export as observability_export;
use crate::observability::report::{
    self as observability_report, CollectionStatus, ObserveReadError,
};
use crate::observability::{
    ArtifactPayload, CollectorEvent, CollectorInputError, CountItem, CountsPayload, EventOutcome,
    EventPayload, EventType, FeedbackOutcome, FeedbackReceipt, FeedbackRecordInput,
    FeedbackWriteError, LinkPayload, LocalCollector, McpPayload, McpStatus, NodePayload,
    RecallBundleNode, RecallBundleRecord, RecallPayload, RecallScore, SelectionReason, ToolPayload,
};
use crate::output::{OutputWarning, OBSERVABILITY_WRITE_FAILED};
use crate::recall;
use crate::reflection;
use crate::storage;
use crate::tools;
use crate::ui;
use crate::upgrade;
use crate::verify;

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_GENERIC_ERROR: u8 = 1;
pub const EXIT_INVALID_ARGS: u8 = 2;
pub const EXIT_WORKSPACE_NOT_FOUND: u8 = 3;
pub const EXIT_DB_SCHEMA_ERROR: u8 = 4;
pub const EXIT_VALIDATION_FAILED: u8 = 5;
pub const EXIT_UNSAFE_ACTION_BLOCKED: u8 = 6;
pub const EXIT_NOT_IMPLEMENTED: u8 = 7;
pub const EXIT_DRIFT_DETECTED: u8 = 8;
pub const EXIT_IO_ERROR: u8 = 9;

const DEFAULT_BOUNDED_RECALL_LIMIT: usize = 12;
const MAX_BOUNDED_RECALL_LIMIT: usize = 50;
const MAX_STRUCTURED_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_LIST_LIMIT: usize = 100;
const MAX_LIST_LIMIT: usize = 500;
const MAX_CURSOR_BYTES: usize = 1024;
const MAX_TOOL_RUN_ARGS: usize = 128;
const MAX_TOOL_RUN_ARG_BYTES: usize = 8 * 1024;
const MAX_TOOL_RUN_TOTAL_ARG_BYTES: usize = 64 * 1024;
const AUDIT_SNAPSHOT_FAILED_ERROR_CODE: &str = "AUDIT_SNAPSHOT_FAILED";

/// Invocation-scoped, best-effort Local Observability state.
///
/// A collector is attached only after the command has resolved a safe
/// workspace. Any collector failure is latched into one output warning and
/// never changes the core command result.
struct CommandObservation {
    command_id: &'static str,
    started_at: Instant,
    terminal_duration_ms: Option<u64>,
    bundle_id: Option<recall::RecallBundleId>,
    collector: Option<LocalCollector>,
    attach_attempted: bool,
    warning: Option<OutputWarning>,
}

impl CommandObservation {
    fn new(command_id: &'static str, bundle_id: Option<recall::RecallBundleId>) -> Self {
        Self {
            command_id,
            started_at: Instant::now(),
            terminal_duration_ms: None,
            bundle_id,
            collector: None,
            attach_attempted: false,
            warning: None,
        }
    }

    fn attach_workspace(&mut self, workspace_paths: &storage::WorkspacePaths) {
        if self.attach_attempted {
            return;
        }
        self.attach_attempted = true;
        match LocalCollector::new(workspace_paths, self.command_id) {
            Ok(collector) => self.collector = Some(collector),
            Err(_) => self.latch_write_warning(),
        }
    }

    fn record(&mut self, event: Result<CollectorEvent, crate::observability::CollectorInputError>) {
        let event = event.and_then(|event| match &self.bundle_id {
            Some(bundle_id) => event.with_bundle_id(bundle_id.as_str()),
            None => Ok(event),
        });
        let warning = self
            .collector
            .as_mut()
            .and_then(|collector| collector.record_result(event));
        if let Some(warning) = warning {
            self.latch_warning(warning);
        }
    }

    fn bind_recall_bundle(&mut self, bundle_id: &recall::RecallBundleId) -> Result<(), CliError> {
        if self
            .bundle_id
            .as_ref()
            .is_some_and(|current| current != bundle_id)
        {
            return Err(CliError::recall_bundle_mismatch());
        }
        self.bundle_id = Some(bundle_id.clone());
        Ok(())
    }

    fn record_recall_bundle(
        &mut self,
        record: Result<RecallBundleRecord, CollectorInputError>,
        events: Vec<Result<CollectorEvent, CollectorInputError>>,
    ) {
        let record = match record {
            Ok(record) => record,
            Err(_) => {
                self.latch_write_warning();
                return;
            }
        };
        let events = match events.into_iter().collect::<Result<Vec<_>, _>>() {
            Ok(events) => events,
            Err(_) => {
                self.latch_write_warning();
                return;
            }
        };
        let warning = self
            .collector
            .as_mut()
            .and_then(|collector| collector.record_recall_bundle(&record, &events));
        if let Some(warning) = warning {
            self.latch_warning(warning);
        }
    }

    fn record_feedback(
        &mut self,
        input: FeedbackRecordInput,
    ) -> Result<FeedbackReceipt, FeedbackWriteError> {
        let Some(collector) = self.collector.as_mut() else {
            return Err(FeedbackWriteError::StoreUnavailable);
        };
        let mut outcome = collector.record_feedback(input)?;
        if let Some(warning) = outcome.warning.take() {
            self.latch_warning(warning);
        }
        Ok(outcome.receipt)
    }

    fn record_terminal(&mut self, event: Result<CollectorEvent, CollectorInputError>) {
        let duration_ms = self.freeze_terminal_duration_ms();
        self.record(event.and_then(|event| event.with_duration_ms(duration_ms)));
    }

    fn warnings_after(&self, mut core_warnings: Vec<OutputWarning>) -> Vec<OutputWarning> {
        if let Some(warning) = &self.warning {
            core_warnings.push(warning.clone());
        }
        core_warnings
    }

    fn latch_write_warning(&mut self) {
        self.latch_warning(OutputWarning {
            code: OBSERVABILITY_WRITE_FAILED,
            message: "local observability write failed; core command result is unchanged"
                .to_string(),
        });
    }

    fn latch_warning(&mut self, warning: OutputWarning) {
        if self.warning.is_none() {
            self.warning = Some(warning);
        }
    }

    fn freeze_terminal_duration_ms(&mut self) -> u64 {
        if let Some(duration_ms) = self.terminal_duration_ms {
            return duration_ms;
        }
        let duration_ms = u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.terminal_duration_ms = Some(duration_ms);
        duration_ms
    }

    #[cfg(test)]
    fn correlation_id(&self) -> Option<&str> {
        self.collector.as_ref().map(LocalCollector::correlation_id)
    }
}

fn command_observation_event(
    event_type: EventType,
    outcome: EventOutcome,
    payload: EventPayload,
    error_code: Option<&str>,
) -> Result<CollectorEvent, CollectorInputError> {
    let event = CollectorEvent::new(event_type, outcome, payload)?;
    match error_code {
        Some(error_code) => event.with_error_code(error_code),
        None => Ok(event),
    }
}

fn node_observation_event(
    event_type: EventType,
    outcome: EventOutcome,
    node: &storage::Node,
) -> Result<CollectorEvent, CollectorInputError> {
    let payload = NodePayload::new(
        node.id,
        &node.node_type,
        &node.title,
        node.summary.as_deref(),
        node.source_ref.as_deref(),
    )?;
    command_observation_event(event_type, outcome, EventPayload::Node(payload), None)
}

fn link_observation_event(link: &storage::Link) -> Result<CollectorEvent, CollectorInputError> {
    let payload = LinkPayload::new(link.source_node_id, link.target_node_id, &link.link_type)?;
    command_observation_event(
        EventType::LinkCreated,
        EventOutcome::Recorded,
        EventPayload::Link(payload),
        None,
    )
}

fn items_observation_event(
    event_type: EventType,
    outcome: EventOutcome,
    item_count: usize,
) -> Result<CollectorEvent, CollectorInputError> {
    let item_count = u64::try_from(item_count).map_err(|_| CollectorInputError::Serialization)?;
    let payload = CountsPayload::new(vec![CountItem::new("items", item_count)?])?;
    command_observation_event(event_type, outcome, EventPayload::Counts(payload), None)
}

fn counts_observation_event(
    event_type: EventType,
    outcome: EventOutcome,
    items: &[(&str, u64)],
    error_code: Option<&str>,
) -> Result<CollectorEvent, CollectorInputError> {
    let items = items
        .iter()
        .map(|(name, count)| CountItem::new(name, *count))
        .collect::<Result<Vec<_>, _>>()?;
    let payload = CountsPayload::new(items)?;
    command_observation_event(
        event_type,
        outcome,
        EventPayload::Counts(payload),
        error_code,
    )
}

fn record_snapshot_observation(
    observation: &mut CommandObservation,
    snapshot: mutation::SnapshotObservation,
) {
    match snapshot {
        mutation::SnapshotObservation::Completed {
            duration_ms,
            bytes_written,
        } => observation.record_terminal(counts_observation_event(
            EventType::AuditSnapshotCompleted,
            EventOutcome::Success,
            &[
                ("duration_ms", duration_ms),
                ("bytes_written", bytes_written),
            ],
            None,
        )),
        mutation::SnapshotObservation::Pending { duration_ms } => {
            observation.record_terminal(counts_observation_event(
                EventType::AuditSnapshotFailed,
                EventOutcome::Failure,
                &[("duration_ms", duration_ms)],
                Some(AUDIT_SNAPSHOT_FAILED_ERROR_CODE),
            ));
            observation.record_terminal(counts_observation_event(
                EventType::AuditSnapshotPending,
                EventOutcome::Pending,
                &[("duration_ms", duration_ms)],
                Some(mutation::AUDIT_SNAPSHOT_PENDING),
            ));
        }
    }
}

fn tool_observation_event(
    event_type: EventType,
    outcome: EventOutcome,
    tool_id: &str,
    approval_present: bool,
    error_code: Option<&str>,
) -> Result<CollectorEvent, CollectorInputError> {
    let payload = ToolPayload::new(tool_id, approval_present)?;
    command_observation_event(event_type, outcome, EventPayload::Tool(payload), error_code)
}

fn mcp_observation_event(
    profile: &storage::McpProfile,
) -> Option<Result<CollectorEvent, CollectorInputError>> {
    let (status, outcome) = match profile.status.as_str() {
        "installed" => (McpStatus::Installed, EventOutcome::Configured),
        "missing" => (McpStatus::Missing, EventOutcome::Missing),
        "configured_unverified" => (
            McpStatus::ConfiguredUnverified,
            EventOutcome::ConfiguredUnverified,
        ),
        _ => return None,
    };
    Some(McpPayload::new(&profile.id, status).and_then(|payload| {
        command_observation_event(
            EventType::McpStatus,
            outcome,
            EventPayload::Mcp(payload),
            None,
        )
    }))
}

fn record_mcp_status(observation: &mut CommandObservation, profile: &storage::McpProfile) {
    if let Some(event) = mcp_observation_event(profile) {
        observation.freeze_terminal_duration_ms();
        observation.record_terminal(event);
    }
}

fn mcp_status_aggregate_event(
    page: &storage::Page<storage::McpProfile, String>,
) -> Result<CollectorEvent, CollectorInputError> {
    let mut installed = 0_u64;
    let mut missing = 0_u64;
    let mut configured_unverified = 0_u64;
    let mut unrecognized = 0_u64;
    for profile in &page.items {
        match profile.status.as_str() {
            "installed" => installed = installed.saturating_add(1),
            "missing" => missing = missing.saturating_add(1),
            "configured_unverified" => {
                configured_unverified = configured_unverified.saturating_add(1);
            }
            _ => unrecognized = unrecognized.saturating_add(1),
        }
    }
    let profiles =
        u64::try_from(page.items.len()).map_err(|_| CollectorInputError::Serialization)?;
    counts_observation_event(
        EventType::McpStatus,
        if page.more_results {
            EventOutcome::Truncated
        } else {
            EventOutcome::Success
        },
        &[
            ("profiles", profiles),
            ("installed", installed),
            ("missing", missing),
            ("configured_unverified", configured_unverified),
            ("unrecognized", unrecognized),
            ("more_results", u64::from(page.more_results)),
        ],
        None,
    )
}

fn artifact_cleanup_counts(
    report: Option<&artifacts::CleanupReport>,
    deleted_paths: Option<&[String]>,
) -> Result<Vec<(&'static str, u64)>, CollectorInputError> {
    let to_u64 =
        |value: usize| u64::try_from(value).map_err(|_| CollectorInputError::Serialization);
    match report {
        Some(report) => Ok(vec![
            ("bytes_before", report.bytes_before),
            ("bytes_after", report.bytes_after),
            ("deleted_dirs", to_u64(report.deleted_dirs.len())?),
            ("deleted_files", to_u64(report.deleted_files.len())?),
            ("deleted_paths", to_u64(report.deleted_paths.len())?),
            ("kept_dirs", to_u64(report.kept_dirs.len())?),
            ("complete", u64::from(report.complete)),
        ]),
        None => Ok(vec![(
            "deleted_paths",
            to_u64(deleted_paths.map_or(0, <[String]>::len))?,
        )]),
    }
}

fn artifact_cleanup_observation_event(
    result: &Result<artifacts::CleanupReport, artifacts::ArtifactError>,
) -> Result<CollectorEvent, CollectorInputError> {
    match result {
        Ok(report) => {
            let counts = artifact_cleanup_counts(Some(report), None)?;
            counts_observation_event(
                EventType::ArtifactsCleanup,
                EventOutcome::Success,
                &counts,
                None,
            )
        }
        Err(error) => {
            let cli_error = CliError::artifacts(error);
            let counts = artifact_cleanup_counts(error.cleanup_report(), error.deleted_paths())?;
            let outcome = match error {
                artifacts::ArtifactError::CleanupPartial { .. }
                | artifacts::ArtifactError::RetentionLimitNotMet { .. } => EventOutcome::Warning,
                artifacts::ArtifactError::CleanupStateUnknown { .. }
                | artifacts::ArtifactError::Io(_)
                | artifacts::ArtifactError::Db(_)
                | artifacts::ArtifactError::InvalidDay(_)
                | artifacts::ArtifactError::LockTimeout { .. } => EventOutcome::Failure,
            };
            counts_observation_event(
                EventType::ArtifactsCleanup,
                outcome,
                &counts,
                Some(cli_error.code),
            )
        }
    }
}

fn tool_run_error_code(error: &tools::RunToolError) -> &'static str {
    match error {
        tools::RunToolError::NotFound(_) => "NOT_FOUND",
        tools::RunToolError::Db(_) => "DB_SCHEMA_ERROR",
        tools::RunToolError::Json(tools::ToolJsonError::Validation(_))
        | tools::RunToolError::MissingExecutablePath(_)
        | tools::RunToolError::Limit(tools::ToolRunLimitError::InvalidLimits { .. }) => {
            "VALIDATION_ERROR"
        }
        tools::RunToolError::Json(tools::ToolJsonError::Json(_)) => "TOOL_JSON_ERROR",
        tools::RunToolError::Json(tools::ToolJsonError::Io(_)) | tools::RunToolError::Io(_) => {
            "IO_ERROR"
        }
        tools::RunToolError::ContractDrift(_) => "DRIFT_DETECTED",
        tools::RunToolError::UnsafeActionBlocked { .. } => "UNSAFE_ACTION_BLOCKED",
        tools::RunToolError::Limit(tools::ToolRunLimitError::TimedOut { .. }) => "TOOL_TIMEOUT",
        tools::RunToolError::Limit(
            tools::ToolRunLimitError::OutputOverflow { .. }
            | tools::ToolRunLimitError::ArtifactHardOverflow { .. },
        ) => "TOOL_OUTPUT_OVERFLOW",
        tools::RunToolError::ProcessFailed(_) => "TOOL_PROCESS_FAILED",
    }
}

fn record_tool_run_observation(
    observation: &mut CommandObservation,
    tool_id: &str,
    approval_present: bool,
    trace: tools::ToolRunTrace,
    result: &Result<tools::ToolInvocationRecord, tools::RunToolError>,
    dry_run: bool,
) {
    observation.freeze_terminal_duration_ms();
    let (validation_outcome, validation_error_code) = match result {
        Ok(_) => (EventOutcome::Success, None),
        Err(tools::RunToolError::UnsafeActionBlocked { .. }) => {
            (EventOutcome::Blocked, Some("UNSAFE_ACTION_BLOCKED"))
        }
        Err(_error) if trace.validation_succeeded() => (EventOutcome::Success, None),
        Err(error) => (EventOutcome::Failure, Some(tool_run_error_code(error))),
    };
    observation.record_terminal(tool_observation_event(
        EventType::ToolValidation,
        validation_outcome,
        tool_id,
        approval_present,
        validation_error_code,
    ));

    if dry_run || !trace.validation_succeeded() {
        return;
    }
    if trace.process_spawned() {
        observation.record(tool_observation_event(
            EventType::ToolRunStarted,
            EventOutcome::Started,
            tool_id,
            approval_present,
            None,
        ));
    }

    match result {
        Ok(tools::ToolInvocationRecord::Run(record)) => {
            observation.record_terminal(tool_observation_event(
                EventType::ToolRunCompleted,
                EventOutcome::Success,
                tool_id,
                approval_present,
                None,
            ));
            if let Some(artifacts) = &record.artifacts {
                for stream in [&artifacts.stdout, &artifacts.stderr] {
                    observation.record_terminal(
                        ArtifactPayload::new(&stream.path, stream.bytes).and_then(|payload| {
                            command_observation_event(
                                EventType::ToolOutputArtifact,
                                EventOutcome::Recorded,
                                EventPayload::Artifact(payload),
                                None,
                            )
                        }),
                    );
                }
            }
        }
        Ok(tools::ToolInvocationRecord::DryRun(_)) => {}
        Err(tools::RunToolError::Limit(tools::ToolRunLimitError::TimedOut { .. })) => {
            observation.record_terminal(tool_observation_event(
                EventType::ToolRunTimeout,
                EventOutcome::Timeout,
                tool_id,
                approval_present,
                Some("TOOL_TIMEOUT"),
            ));
        }
        Err(error) => observation.record_terminal(tool_observation_event(
            EventType::ToolRunFailed,
            EventOutcome::Failure,
            tool_id,
            approval_present,
            Some(tool_run_error_code(error)),
        )),
    }
}

fn workspace_init_observation_event(
    status: &install::WorkspaceInitStatus,
) -> Result<CollectorEvent, CollectorInputError> {
    let counts = [
        ("seeded_nodes_created", status.seeded_nodes_created),
        ("seeded_nodes_existing", status.seeded_nodes_existing),
        ("semantic_nodes_created", status.semantic_nodes_created),
        ("semantic_nodes_existing", status.semantic_nodes_existing),
    ]
    .into_iter()
    .map(|(name, count)| {
        u64::try_from(count)
            .map_err(|_| CollectorInputError::Serialization)
            .and_then(|count| CountItem::new(name, count))
    })
    .collect::<Result<Vec<_>, _>>()?;
    let payload = CountsPayload::new(counts)?;
    command_observation_event(
        EventType::WorkspaceInit,
        EventOutcome::Success,
        EventPayload::Counts(payload),
        None,
    )
}

fn record_failed_observation(
    observation: &mut CommandObservation,
    event_type: EventType,
    error: &CliError,
) {
    observation.record_terminal(command_observation_event(
        event_type,
        EventOutcome::Failure,
        EventPayload::Empty,
        Some(error.code),
    ));
}

#[derive(Debug, Parser)]
#[command(name = "aopmem")]
#[command(version)]
#[command(about = "AOPMem command line interface")]
pub struct Cli {
    #[arg(long, global = true, help = "Print stable machine-readable JSON")]
    json: bool,

    #[arg(
        long,
        global = true,
        help = "Approval text; any value containing +++ is accepted"
    )]
    approved: Option<String>,

    #[arg(
        long,
        global = true,
        value_parser = parse_global_bundle_id,
        help = "Correlate this operation with a canonical lowercase UUID v4 recall bundle"
    )]
    bundle_id: Option<recall::RecallBundleId>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum Command {
    Init,
    Status,
    Doctor,
    Verify,
    Node {
        #[command(subcommand)]
        command: NodeCommand,
    },
    Link {
        #[command(subcommand)]
        command: LinkCommand,
    },
    Alias {
        #[command(subcommand)]
        command: AliasCommand,
    },
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    Recall(RecallArgs),
    Remember(RememberArgs),
    Teach {
        #[command(subcommand)]
        command: TeachCommand,
    },
    Reflect {
        #[command(subcommand)]
        command: ReflectCommand,
    },
    Tool {
        #[command(subcommand)]
        command: ToolCommand,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
    Adapter {
        #[command(subcommand)]
        command: AdapterCommand,
    },
    Artifacts {
        #[command(subcommand)]
        command: ArtifactsCommand,
    },
    Feedback {
        #[command(subcommand)]
        command: FeedbackCommand,
    },
    Observe {
        #[command(subcommand)]
        command: ObserveCommand,
    },
    Upgrade {
        #[command(subcommand)]
        command: UpgradeCommand,
    },
    Ui(UiArgs),
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum NodeCommand {
    Create(NodeCreateArgs),
    Get(NodeGetArgs),
    List(NodeListArgs),
    Update(NodeUpdateArgs),
}

#[derive(Debug, Args)]
struct NodeCreateArgs {
    #[arg(long = "type")]
    node_type: String,

    #[arg(long, default_value = "draft")]
    status: String,

    #[arg(long)]
    title: String,

    #[arg(long)]
    summary: Option<String>,

    #[arg(long)]
    body: Option<String>,

    #[arg(long)]
    source_ref: Option<String>,

    #[arg(long)]
    confidence: Option<f64>,

    #[arg(long)]
    trust_level: Option<String>,
}

#[derive(Debug, Args)]
struct NodeGetArgs {
    #[arg(long)]
    id: i64,
}

#[derive(Debug, Args)]
struct NodeListArgs {
    #[arg(long, default_value_t = DEFAULT_LIST_LIMIT, value_parser = parse_list_limit)]
    limit: usize,

    #[arg(long, conflicts_with_all = ["all", "after_id"])]
    cursor: Option<String>,

    #[arg(long, conflicts_with_all = ["cursor", "after_id"])]
    all: bool,

    #[arg(
        long,
        hide = true,
        conflicts_with_all = ["cursor", "all"],
        value_parser = parse_positive_id
    )]
    after_id: Option<i64>,

    #[arg(long)]
    include_body: bool,
}

#[derive(Debug, Args)]
struct NodeUpdateArgs {
    #[arg(long, value_parser = parse_positive_id)]
    id: i64,

    #[arg(long)]
    status: String,

    #[arg(long)]
    title: String,

    #[arg(long)]
    summary: Option<String>,

    #[arg(long)]
    body: Option<String>,

    #[arg(long)]
    source_ref: Option<String>,

    #[arg(long)]
    confidence: Option<f64>,

    #[arg(long)]
    trust_level: Option<String>,
}

#[derive(Debug, Args)]
struct RecallArgs {
    #[arg(long, conflicts_with = "full")]
    query: Option<String>,

    #[arg(
        long,
        requires = "query",
        conflicts_with = "full",
        value_parser = parse_bounded_recall_limit
    )]
    limit: Option<usize>,

    #[arg(
        long,
        conflicts_with_all = ["query", "limit", "continuation_cursor"]
    )]
    full: bool,

    #[arg(
        long,
        requires = "query",
        conflicts_with = "full",
        value_parser = parse_recall_continuation_cursor
    )]
    continuation_cursor: Option<String>,
}

fn parse_bounded_recall_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| "limit must be a positive integer".to_string())?;
    if (1..=MAX_BOUNDED_RECALL_LIMIT).contains(&limit) {
        Ok(limit)
    } else {
        Err(format!(
            "limit must be between 1 and {MAX_BOUNDED_RECALL_LIMIT}"
        ))
    }
}

fn parse_recall_continuation_cursor(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err("continuation cursor must not be empty".to_string());
    }
    if value.len() > recall::MAX_RECALL_CONTINUATION_CURSOR_BYTES {
        return Err(format!(
            "continuation cursor must be at most {} bytes",
            recall::MAX_RECALL_CONTINUATION_CURSOR_BYTES
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'~' | b'-'))
    {
        return Err("continuation cursor must contain only URL-safe ASCII characters".to_string());
    }
    Ok(value.to_string())
}

fn parse_global_bundle_id(value: &str) -> Result<recall::RecallBundleId, String> {
    recall::RecallBundleId::parse(value).map_err(|error| error.to_string())
}

#[derive(Debug, Args)]
struct RememberArgs {
    #[arg(value_name = "NOTE")]
    note: Option<String>,

    #[arg(long = "type")]
    node_type: Option<String>,

    #[arg(long)]
    status: Option<String>,

    #[arg(long)]
    title: Option<String>,

    #[arg(long)]
    summary: Option<String>,

    #[arg(long)]
    body: Option<String>,

    #[arg(long)]
    source_ref: Option<String>,

    #[arg(long)]
    confidence: Option<f64>,

    #[arg(long)]
    trust_level: Option<String>,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum LinkCommand {
    Add(LinkAddArgs),
    List(NumericListArgs),
}

#[derive(Debug, Args)]
struct LinkAddArgs {
    #[arg(long, value_parser = parse_positive_id)]
    source_id: i64,

    #[arg(long, value_parser = parse_positive_id)]
    target_id: i64,

    #[arg(long = "type")]
    link_type: String,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum AliasCommand {
    Add(AliasAddArgs),
    List(NodeMetadataListArgs),
}

#[derive(Debug, Args)]
struct AliasAddArgs {
    #[arg(long)]
    node_id: i64,

    #[arg(long)]
    alias: String,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum TagCommand {
    Add(TagAddArgs),
    List(NodeMetadataListArgs),
}

#[derive(Debug, Args)]
struct TagAddArgs {
    #[arg(long)]
    node_id: i64,

    #[arg(long)]
    tag: String,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum SourceCommand {
    Add(SourceAddArgs),
    List(NodeMetadataListArgs),
}

#[derive(Debug, Args)]
struct SourceAddArgs {
    #[arg(long)]
    node_id: i64,

    #[arg(long)]
    source_ref: String,
}

#[derive(Debug, Args)]
struct NodeMetadataListArgs {
    #[arg(long)]
    node_id: Option<i64>,

    #[arg(long, default_value_t = DEFAULT_LIST_LIMIT, value_parser = parse_list_limit)]
    limit: usize,

    #[arg(long, conflicts_with_all = ["all", "after_id"])]
    cursor: Option<String>,

    #[arg(long, conflicts_with_all = ["cursor", "after_id"])]
    all: bool,

    #[arg(
        long,
        hide = true,
        conflicts_with_all = ["cursor", "all"],
        value_parser = parse_positive_id
    )]
    after_id: Option<i64>,
}

#[derive(Debug, Args)]
struct NumericListArgs {
    #[arg(long, default_value_t = DEFAULT_LIST_LIMIT, value_parser = parse_list_limit)]
    limit: usize,

    #[arg(long, conflicts_with_all = ["all", "after_id"])]
    cursor: Option<String>,

    #[arg(long, conflicts_with_all = ["cursor", "after_id"])]
    all: bool,

    #[arg(
        long,
        hide = true,
        conflicts_with_all = ["cursor", "all"],
        value_parser = parse_positive_id
    )]
    after_id: Option<i64>,
}

#[derive(Debug, Args)]
struct StringListArgs {
    #[arg(long, default_value_t = DEFAULT_LIST_LIMIT, value_parser = parse_list_limit)]
    limit: usize,

    #[arg(long, conflicts_with_all = ["all", "after_id"])]
    cursor: Option<String>,

    #[arg(long, conflicts_with_all = ["cursor", "after_id"])]
    all: bool,

    #[arg(
        long,
        hide = true,
        conflicts_with_all = ["cursor", "all"],
        value_parser = parse_non_empty_cursor
    )]
    after_id: Option<String>,
}

fn parse_list_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| "limit must be a positive integer".to_string())?;
    if (1..=MAX_LIST_LIMIT).contains(&limit) {
        Ok(limit)
    } else {
        Err(format!("limit must be between 1 and {MAX_LIST_LIMIT}"))
    }
}

fn parse_positive_id(value: &str) -> Result<i64, String> {
    let id = value
        .parse::<i64>()
        .map_err(|_| "id must be a positive integer".to_string())?;
    if id > 0 {
        Ok(id)
    } else {
        Err("id must be a positive integer".to_string())
    }
}

fn parse_non_empty_cursor(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err("after-id must not be empty".to_string())
    } else {
        Ok(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorKind {
    Node,
    Link,
    Alias,
    Tag,
    Source,
    Tool,
    Mcp,
}

impl CursorKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Link => "link",
            Self::Alias => "alias",
            Self::Tag => "tag",
            Self::Source => "source",
            Self::Tool => "tool",
            Self::Mcp => "mcp",
        }
    }
}

fn metadata_cursor_scope(node_id: Option<i64>) -> String {
    node_id.map_or_else(|| "all".to_string(), |id| format!("node-{id}"))
}

fn encode_list_cursor(kind: CursorKind, scope: &str, key: &str) -> Result<String, &'static str> {
    if key.is_empty() {
        return Err("cursor key must not be empty");
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let prefix = format!("v1.{}.{}.", kind.as_str(), scope);
    let mut cursor = String::with_capacity(prefix.len() + key.len() * 2);
    cursor.push_str(&prefix);
    for byte in key.bytes() {
        cursor.push(HEX[(byte >> 4) as usize] as char);
        cursor.push(HEX[(byte & 0x0f) as usize] as char);
    }
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err("cursor exceeds the 1024-byte limit");
    }
    Ok(cursor)
}

fn decode_list_cursor(cursor: &str, kind: CursorKind, scope: &str) -> Result<String, &'static str> {
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err("cursor exceeds the 1024-byte limit");
    }
    let prefix = format!("v1.{}.{}.", kind.as_str(), scope);
    let payload = cursor
        .strip_prefix(&prefix)
        .ok_or("cursor kind or scope does not match this list")?;
    if payload.is_empty() || !payload.len().is_multiple_of(2) {
        return Err("cursor payload must be non-empty, even-length lowercase hex");
    }

    let mut key_bytes = Vec::with_capacity(payload.len() / 2);
    for pair in payload.as_bytes().chunks_exact(2) {
        let high = lowercase_hex_nibble(pair[0])?;
        let low = lowercase_hex_nibble(pair[1])?;
        key_bytes.push((high << 4) | low);
    }
    let key = std::str::from_utf8(&key_bytes).map_err(|_| "cursor key must be valid UTF-8")?;
    if key.is_empty() {
        return Err("cursor key must not be empty");
    }
    let canonical = encode_list_cursor(kind, scope, key)?;
    if canonical != cursor {
        return Err("cursor is not canonical");
    }
    Ok(key.to_string())
}

fn encode_numeric_cursor(kind: CursorKind, scope: &str, id: i64) -> Result<String, &'static str> {
    if id <= 0 {
        return Err("numeric cursor key must be a positive integer");
    }
    encode_list_cursor(kind, scope, &id.to_string())
}

fn decode_numeric_cursor(cursor: &str, kind: CursorKind, scope: &str) -> Result<i64, &'static str> {
    let key = decode_list_cursor(cursor, kind, scope)?;
    let id = key
        .parse::<i64>()
        .map_err(|_| "numeric cursor key must be a positive integer")?;
    let canonical = encode_numeric_cursor(kind, scope, id)?;
    if canonical != cursor {
        return Err("numeric cursor key is not canonical");
    }
    Ok(id)
}

fn encode_node_cursor(id: i64) -> Result<String, &'static str> {
    encode_numeric_cursor(CursorKind::Node, "all", id)
}

fn decode_node_cursor(cursor: &str) -> Result<i64, &'static str> {
    decode_numeric_cursor(cursor, CursorKind::Node, "all")
}

fn lowercase_hex_nibble(byte: u8) -> Result<u8, &'static str> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("cursor payload must use lowercase hex"),
    }
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum TeachCommand {
    Start(TeachStartArgs),
    Add(TeachPayloadArgs),
    Propose(TeachPayloadArgs),
    Apply(TeachApplyArgs),
}

#[derive(Debug, Args)]
struct TeachStartArgs {
    #[arg(long)]
    title: String,

    #[arg(long)]
    summary: Option<String>,
}

#[derive(Debug, Args)]
struct TeachPayloadArgs {
    #[arg(long, value_parser = parse_positive_id)]
    session_id: i64,

    #[arg(long)]
    payload: String,
}

#[derive(Debug, Args)]
struct TeachApplyArgs {
    #[arg(long, value_parser = parse_positive_id)]
    session_id: i64,

    #[arg(long, value_parser = parse_positive_id)]
    proposal_id: i64,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum ReflectCommand {
    Inventory,
    Proposal {
        #[command(subcommand)]
        command: ReflectProposalCommand,
    },
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum ReflectProposalCommand {
    Create(ReflectProposalCreateArgs),
    Apply(ReflectProposalApplyArgs),
}

#[derive(Debug, Args)]
struct ReflectProposalCreateArgs {
    #[arg(long)]
    session_id: String,

    #[arg(long = "proposal-file")]
    proposal_file: PathBuf,
}

#[derive(Debug, Args)]
struct ReflectProposalApplyArgs {
    #[arg(long, value_parser = parse_positive_id)]
    proposal_id: i64,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum ToolCommand {
    CreateDraft(ToolCreateDraftArgs),
    List(StringListArgs),
    Get(ToolGetArgs),
    Run(ToolRunArgs),
    Validate(ToolValidateArgs),
}

#[derive(Debug, Args)]
struct ToolCreateDraftArgs {
    #[arg(long)]
    id: String,

    #[arg(long)]
    name: String,

    #[arg(long)]
    entrypoint: Option<String>,

    #[arg(long)]
    owner_workflow: Option<String>,

    #[arg(long, default_value = "none")]
    side_effects: String,

    #[arg(long, default_value = "none")]
    approval_requirement: String,

    #[arg(long, default_value_t = tools::DEFAULT_TOOL_TIMEOUT_MS)]
    timeout_ms: u64,

    #[arg(long, default_value_t = tools::DEFAULT_TOOL_OUTPUT_LIMIT_BYTES)]
    stdout_limit_bytes: u64,

    #[arg(long, default_value_t = tools::DEFAULT_TOOL_OUTPUT_LIMIT_BYTES)]
    stderr_limit_bytes: u64,

    #[arg(long)]
    supports_dry_run: bool,

    #[arg(long, default_value = "inline")]
    output_mode: tools::ToolOutputMode,
}

#[derive(Debug, Args)]
struct ToolValidateArgs {
    tool_id: String,
}

#[derive(Debug, Args)]
struct ToolGetArgs {
    tool_id: String,
}

#[derive(Debug, Args)]
struct ToolRunArgs {
    tool_id: String,

    #[arg(long)]
    dry_run: bool,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum McpCommand {
    List(StringListArgs),
    Add(Box<McpAddArgs>),
    Get(McpGetArgs),
}

#[derive(Debug, Args)]
struct McpAddArgs {
    #[arg(long)]
    id: String,

    #[arg(long)]
    name: String,

    #[arg(long)]
    kind: String,

    #[arg(long)]
    status: String,

    #[arg(long)]
    read_operations: String,

    #[arg(long)]
    write_operations: String,

    #[arg(long)]
    side_effects: String,

    #[arg(long)]
    approval_requirement: String,

    #[arg(long)]
    credentials_source: Option<String>,

    #[arg(long)]
    notes: Option<String>,
}

#[derive(Debug, Args)]
struct McpGetArgs {
    #[arg(long)]
    id: String,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum AdapterCommand {
    Seed(AdapterSeedArgs),
    Sync(AdapterTargetArgs),
    Status(AdapterTargetArgs),
}

#[derive(Debug, Args)]
struct AdapterSeedArgs {
    #[arg(long)]
    file: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct AdapterTargetArgs {
    #[arg(long)]
    file: Option<PathBuf>,
}

struct AdapterResolvedTarget {
    instruction_file: PathBuf,
    workspace_key: String,
    observation_workspace: Option<storage::WorkspacePaths>,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum ArtifactsCommand {
    Cleanup,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum FeedbackCommand {
    Record(FeedbackRecordArgs),
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum ObserveCommand {
    Status,
    Report,
    Export(ObserveExportArgs),
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum UpgradeCommand {
    Plan(UpgradePlanArgs),
    Apply(UpgradePlanArgs),
}

#[derive(Debug, Args)]
struct UpgradePlanArgs {
    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
    all_workspaces: bool,
}

#[derive(Debug, Args)]
struct ObserveExportArgs {
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct UiArgs {
    #[arg(long, default_value_t = 0)]
    port: u16,

    #[arg(long)]
    no_open: bool,
}

#[derive(Debug, Args)]
struct FeedbackRecordArgs {
    #[arg(long, value_parser = parse_feedback_outcome)]
    outcome: FeedbackOutcome,

    #[arg(long)]
    reason: Option<String>,
}

fn parse_feedback_outcome(value: &str) -> Result<FeedbackOutcome, String> {
    value
        .parse::<FeedbackOutcome>()
        .map_err(|error| error.to_string())
}

pub fn run() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => run_parsed(&cli),
        Err(error) => handle_parse_error(error, json_flag_present(env::args())),
    }
}

fn run_parsed(cli: &Cli) -> ExitCode {
    run_command_with_context(
        &cli.command,
        cli.json,
        cli.approved.as_deref(),
        cli.bundle_id.clone(),
    )
}

#[cfg(test)]
fn run_command(command: &Command, json: bool) -> ExitCode {
    run_command_with_approval(command, json, None)
}

#[cfg(test)]
fn run_command_with_approval(command: &Command, json: bool, approved: Option<&str>) -> ExitCode {
    run_command_with_context(command, json, approved, None)
}

fn run_command_with_context(
    command: &Command,
    json: bool,
    approved: Option<&str>,
    bundle_id: Option<recall::RecallBundleId>,
) -> ExitCode {
    let command_id = command_id(command);
    if let Command::Ui(args) = command {
        return run_ui(command_id, args, json);
    }
    if let Command::Observe { command } = command {
        return match command {
            ObserveCommand::Status => run_observe_status(command_id, json),
            ObserveCommand::Report => run_observe_report(command_id, json),
            ObserveCommand::Export(args) => run_observe_export(command_id, args, json),
        };
    }
    if let Command::Upgrade { command } = command {
        return match command {
            UpgradeCommand::Plan(args) => run_upgrade_plan(command_id, args, json),
            UpgradeCommand::Apply(args) => run_upgrade_apply(command_id, args, json),
        };
    }
    let mut observation = CommandObservation::new(command_id, bundle_id);
    match command {
        Command::Init => run_init(command_id, json, &mut observation),
        Command::Status => run_status(command_id, json),
        Command::Doctor => run_doctor(command_id, json, &mut observation),
        Command::Verify => run_verify(command_id, json, &mut observation),
        Command::Node {
            command: NodeCommand::Create(args),
        } => run_node_create(command_id, args, json, &mut observation),
        Command::Node {
            command: NodeCommand::Get(args),
        } => run_node_get(command_id, args, json),
        Command::Node {
            command: NodeCommand::List(args),
        } => run_node_list(command_id, args, json),
        Command::Node {
            command: NodeCommand::Update(args),
        } => run_node_update(command_id, args, json, &mut observation),
        Command::Link {
            command: LinkCommand::Add(args),
        } => run_link_add(command_id, args, json, &mut observation),
        Command::Link {
            command: LinkCommand::List(args),
        } => run_link_list(command_id, args, json),
        Command::Alias {
            command: AliasCommand::Add(args),
        } => run_alias_add(command_id, args, json, &mut observation),
        Command::Alias {
            command: AliasCommand::List(args),
        } => run_alias_list(command_id, args, json),
        Command::Tag {
            command: TagCommand::Add(args),
        } => run_tag_add(command_id, args, json, &mut observation),
        Command::Tag {
            command: TagCommand::List(args),
        } => run_tag_list(command_id, args, json),
        Command::Source {
            command: SourceCommand::Add(args),
        } => run_source_add(command_id, args, json, &mut observation),
        Command::Source {
            command: SourceCommand::List(args),
        } => run_source_list(command_id, args, json),
        Command::Mcp {
            command: McpCommand::List(args),
        } => run_mcp_list(command_id, args, json, &mut observation),
        Command::Mcp {
            command: McpCommand::Add(args),
        } => run_mcp_add(command_id, args, json, &mut observation),
        Command::Mcp {
            command: McpCommand::Get(args),
        } => run_mcp_get(command_id, args, json, &mut observation),
        Command::Recall(args) => run_recall(command_id, args, json, &mut observation),
        Command::Remember(args) => run_remember(command_id, args, json, &mut observation),
        Command::Teach {
            command: TeachCommand::Start(args),
        } => run_teach_start(command_id, args, json, &mut observation),
        Command::Teach {
            command: TeachCommand::Add(args),
        } => run_teach_add(command_id, args, json, &mut observation),
        Command::Teach {
            command: TeachCommand::Propose(args),
        } => run_teach_propose(command_id, args, json, &mut observation),
        Command::Teach {
            command: TeachCommand::Apply(args),
        } => run_teach_apply(command_id, args, json, &mut observation),
        Command::Reflect {
            command: ReflectCommand::Inventory,
        } => run_reflect_inventory(command_id, json, &mut observation),
        Command::Reflect {
            command:
                ReflectCommand::Proposal {
                    command: ReflectProposalCommand::Create(args),
                },
        } => run_reflect_proposal_create(command_id, args, json, &mut observation),
        Command::Reflect {
            command:
                ReflectCommand::Proposal {
                    command: ReflectProposalCommand::Apply(args),
                },
        } => run_reflect_proposal_apply(command_id, args, json, &mut observation),
        Command::Adapter {
            command: AdapterCommand::Seed(args),
        } => run_adapter_seed(command_id, args, json, &mut observation),
        Command::Adapter {
            command: AdapterCommand::Sync(args),
        } => run_adapter_sync(command_id, args, json, &mut observation),
        Command::Adapter {
            command: AdapterCommand::Status(args),
        } => run_adapter_status(command_id, args, json, &mut observation),
        Command::Tool {
            command: ToolCommand::CreateDraft(args),
        } => run_tool_create_draft(command_id, args, json, &mut observation),
        Command::Tool {
            command: ToolCommand::List(args),
        } => run_tool_list(command_id, args, json),
        Command::Tool {
            command: ToolCommand::Get(args),
        } => run_tool_get(command_id, args, json),
        Command::Tool {
            command: ToolCommand::Run(args),
        } => run_tool_run(command_id, args, json, approved, &mut observation),
        Command::Tool {
            command: ToolCommand::Validate(args),
        } => run_tool_validate(command_id, args, json, &mut observation),
        Command::Artifacts {
            command: ArtifactsCommand::Cleanup,
        } => run_artifacts_cleanup(command_id, json, &mut observation),
        Command::Feedback {
            command: FeedbackCommand::Record(args),
        } => run_feedback_record(command_id, args, json, &mut observation),
        Command::Observe { .. } => unreachable!("observe commands return before collector state"),
        Command::Upgrade { .. } => unreachable!("upgrade plan returns before collector state"),
        Command::Ui(_) => unreachable!("UI returns before collector state"),
    }
}

fn run_upgrade_plan(
    command_id: &'static str,
    args: &UpgradePlanArgs,
    json_output: bool,
) -> ExitCode {
    if !args.all_workspaces {
        return print_error(command_id, &CliError::invalid_args(), json_output);
    }
    match upgrade::plan_all_workspaces() {
        Ok(report) => {
            if json_output {
                println!(
                    "{}",
                    success_envelope(
                        command_id,
                        serde_json::to_value(&report)
                            .expect("upgrade plan serialization should not fail"),
                    )
                );
            } else {
                println!(
                    "upgrade plan only: ready={} workspaces={} disk_sufficient={} writes_performed={}",
                    report.ready,
                    report.workspace_count,
                    report.disk_space.sufficient,
                    report.writes_performed,
                );
                for workspace in &report.workspaces {
                    println!("{}: {:?}", workspace.workspace_key, workspace.status);
                    if let Some(error) = &workspace.error {
                        println!("  {}: {}", error.code, error.message);
                    }
                }
            }
            ExitCode::from(EXIT_SUCCESS)
        }
        Err(error) => print_error(command_id, &CliError::upgrade_plan(error), json_output),
    }
}

fn run_upgrade_apply(
    command_id: &'static str,
    args: &UpgradePlanArgs,
    json_output: bool,
) -> ExitCode {
    if !args.all_workspaces {
        return print_error(command_id, &CliError::invalid_args(), json_output);
    }
    let execution = match upgrade::apply_all_workspaces() {
        Ok(execution) => execution,
        Err(error) => return print_error(command_id, &CliError::upgrade_apply(error), json_output),
    };
    let data = serde_json::to_value(&execution.report)
        .expect("upgrade apply report serialization should not fail");
    match execution.failure {
        None => {
            if json_output {
                println!(
                    "{}",
                    success_envelope_with_meta_and_warnings(
                        command_id,
                        data,
                        OutputMeta::default(),
                        execution.warnings,
                    )
                );
            } else {
                println!(
                    "upgrade apply: success workspaces={} backup_root={}",
                    execution.report.workspaces.len(),
                    execution.report.backup_root.as_deref().unwrap_or("none"),
                );
                print_text_warnings(execution.warnings);
            }
            ExitCode::from(EXIT_SUCCESS)
        }
        Some(failure) => {
            if json_output {
                println!(
                    "{}",
                    serialize_envelope(&OutputEnvelope {
                        ok: false,
                        command: command_id,
                        data: Some(data),
                        warnings: execution.warnings,
                        errors: vec![OutputError {
                            code: failure.code,
                            message: failure.message,
                            fix_hint: "keep every backup, fix the exact reported workspace error, then rerun upgrade plan before apply".to_string(),
                            details: None,
                        }],
                        meta: OutputMeta::default(),
                    })
                );
            } else {
                eprintln!("{}: {}", failure.code, failure.message);
                if let Some(workspace_key) = failure.workspace_key {
                    eprintln!("workspace: {workspace_key}");
                }
                print_text_warnings(execution.warnings);
            }
            ExitCode::from(EXIT_GENERIC_ERROR)
        }
    }
}

fn run_tool_create_draft(
    command_id: &'static str,
    args: &ToolCreateDraftArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let entrypoint = args
        .entrypoint
        .clone()
        .unwrap_or_else(|| format!("bin/{}", args.id));
    let input = tools::DraftToolInput {
        tool_id: args.id.clone(),
        name: args.name.clone(),
        entrypoint,
        owner_workflow: args.owner_workflow.clone(),
        side_effects: args.side_effects.clone(),
        approval_requirement: args.approval_requirement.clone(),
    };
    let runtime = tools::DraftToolRuntimeInput {
        timeout_ms: args.timeout_ms,
        stdout_limit_bytes: args.stdout_limit_bytes,
        stderr_limit_bytes: args.stderr_limit_bytes,
        supports_dry_run: args.supports_dry_run,
        output_mode: args.output_mode,
    };
    if let Err(error) = tools::validate_draft_tool_input_with_runtime(&input, &runtime) {
        return print_error(command_id, &CliError::tool_validation(error), json_output);
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, effects| {
        tools::create_draft_tool_in_mutation_with_runtime(
            &workspace_paths,
            database,
            &input,
            &runtime,
            effects,
        )
    }) {
        Ok(outcome) => print_observed_mutation_success(
            command_id,
            outcome,
            workspace_key,
            observation,
            json_output,
        ),
        Err(mutation::MutationError::Operation(tools::CreateDraftToolError::Storage(
            tools::ToolContractStorageError::Validation(error),
        )))
        | Err(mutation::MutationError::Operation(tools::CreateDraftToolError::Json(
            tools::ToolJsonError::Validation(error),
        ))) => print_error(command_id, &CliError::tool_validation(error), json_output),
        Err(mutation::MutationError::Operation(tools::CreateDraftToolError::Storage(
            tools::ToolContractStorageError::Db(error),
        ))) => print_error(command_id, &CliError::db_schema(error), json_output),
        Err(mutation::MutationError::Operation(tools::CreateDraftToolError::Storage(
            tools::ToolContractStorageError::Json(error),
        )))
        | Err(mutation::MutationError::Operation(tools::CreateDraftToolError::Json(
            tools::ToolJsonError::Json(error),
        ))) => print_error(
            command_id,
            &CliError::tool_contract_json(error),
            json_output,
        ),
        Err(mutation::MutationError::Operation(tools::CreateDraftToolError::Json(
            tools::ToolJsonError::Io(error),
        )))
        | Err(mutation::MutationError::Operation(tools::CreateDraftToolError::Io(error))) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
        Err(error) => print_error(
            command_id,
            &mutation_infrastructure_error(error),
            json_output,
        ),
    }
}

fn run_tool_validate(
    command_id: &'static str,
    args: &ToolValidateArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_read_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    let result = tools::validate_tool(&workspace_paths, &connection, &args.tool_id);
    drop(connection);
    observation.freeze_terminal_duration_ms();
    match result {
        Ok(record) => {
            observation.record_terminal(tool_observation_event(
                EventType::ToolValidation,
                EventOutcome::Success,
                &args.tool_id,
                false,
                None,
            ));
            print_success_with_warnings(
                command_id,
                json!(record),
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(error) => {
            let error = validate_tool_cli_error(error);
            observation.record_terminal(tool_observation_event(
                EventType::ToolValidation,
                EventOutcome::Failure,
                &args.tool_id,
                false,
                Some(error.code),
            ));
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn validate_tool_cli_error(error: tools::ValidateToolError) -> CliError {
    match error {
        tools::ValidateToolError::NotFound(tool_id) => CliError::tool_not_found(&tool_id),
        tools::ValidateToolError::Db(error) => CliError::db_schema(error),
        tools::ValidateToolError::Json(tools::ToolJsonError::Validation(error)) => {
            CliError::tool_validation(error)
        }
        tools::ValidateToolError::Json(tools::ToolJsonError::Json(error)) => {
            CliError::tool_contract_json(error)
        }
        tools::ValidateToolError::Json(tools::ToolJsonError::Io(error)) => CliError::io(error),
        tools::ValidateToolError::ContractDrift(tool_id) => CliError::tool_contract_drift(&tool_id),
        tools::ValidateToolError::MissingExecutablePath(path) => {
            CliError::tool_executable_missing(&path)
        }
    }
}

fn run_tool_list(command_id: &'static str, args: &StringListArgs, json_output: bool) -> ExitCode {
    let after_id = match string_cursor_or_legacy(
        args.cursor.as_deref(),
        args.after_id.as_deref(),
        CursorKind::Tool,
        "all",
    ) {
        Ok(after_id) => after_id,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, _workspace_paths, mut connection) =
        match open_current_workspace_read_context() {
            Ok(workspace) => workspace,
            Err(error) => return print_error(command_id, &error, json_output),
        };

    let page = if args.all {
        list_all_pages_in_read_transaction(
            &mut connection,
            args.limit,
            |connection, after_id: Option<&String>, limit| {
                tools::list_tool_contracts_page(connection, after_id.map(String::as_str), limit)
                    .map(tool_contracts_page)
            },
            |record| record.contract.tool_id.clone(),
        )
    } else {
        tools::list_tool_contracts_page(&connection, after_id.as_deref(), args.limit)
            .map(tool_contracts_page)
            .map_err(ListError::Db)
            .and_then(|page| {
                validate_page(&page, after_id.as_ref(), args.limit, |record| {
                    record.contract.tool_id.clone()
                })?;
                Ok(page)
            })
    };

    print_list_outcome(
        command_id,
        page.and_then(|page| string_list_data("tools", page, CursorKind::Tool, "all")),
        workspace_key,
        json_output,
    )
}

fn run_tool_get(command_id: &'static str, args: &ToolGetArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_read_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match tools::get_tool_contract(&connection, &args.tool_id) {
        Ok(Some(record)) => {
            let tool_json_path = tools::tool_json_path(&workspace_paths, &args.tool_id);
            let tool_json_path = if tool_json_path.is_file() {
                Some(tool_json_path)
            } else {
                None
            };
            print_success(
                command_id,
                json!({
                    "tool": record,
                    "tool_json_path": tool_json_path,
                }),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Ok(None) => print_error(
            command_id,
            &CliError::tool_not_found(&args.tool_id),
            json_output,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_tool_run(
    command_id: &'static str,
    args: &ToolRunArgs,
    json_output: bool,
    approved: Option<&str>,
    observation: &mut CommandObservation,
) -> ExitCode {
    if let Err(error) = validate_tool_run_args(&args.args) {
        return print_error(command_id, &error, json_output);
    }
    // A run reads the canonical contract and files, but does not write its own
    // record. Keep both normal and dry-run execution from creating a workspace
    // or taking a SQLite write lock.
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_read_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    let mut trace = tools::ToolRunTrace::default();
    let result = if args.dry_run {
        tools::dry_run_tool(&workspace_paths, &connection, &args.tool_id, &args.args)
            .map(tools::ToolInvocationRecord::DryRun)
    } else {
        tools::run_tool_with_trace(
            &workspace_paths,
            &connection,
            &args.tool_id,
            &args.args,
            approved,
            &mut trace,
        )
        .map(tools::ToolInvocationRecord::Run)
    };
    drop(connection);
    record_tool_run_observation(
        observation,
        &args.tool_id,
        approved.is_some(),
        trace,
        &result,
        args.dry_run,
    );

    match result {
        Ok(record) => print_success_with_warnings(
            command_id,
            json!(record),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
            EXIT_SUCCESS,
        ),
        Err(tools::RunToolError::NotFound(tool_id)) => print_error_with_warnings(
            command_id,
            &CliError::tool_not_found(&tool_id),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(tools::RunToolError::Db(error)) => print_error_with_warnings(
            command_id,
            &CliError::db_schema(error),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(tools::RunToolError::Json(tools::ToolJsonError::Validation(error))) => {
            print_error_with_warnings(
                command_id,
                &CliError::tool_validation(error),
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(tools::RunToolError::Json(tools::ToolJsonError::Json(error))) => {
            print_error_with_warnings(
                command_id,
                &CliError::tool_contract_json(error),
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(tools::RunToolError::Json(tools::ToolJsonError::Io(error)))
        | Err(tools::RunToolError::Io(error)) => print_error_with_warnings(
            command_id,
            &CliError::io(error),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(tools::RunToolError::Limit(tools::ToolRunLimitError::InvalidLimits { .. })) => {
            print_error_with_warnings(
                command_id,
                &CliError::tool_run_limits_invalid(),
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(tools::RunToolError::Limit(error)) => print_tool_limit_error_with_warnings(
            command_id,
            &error,
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(tools::RunToolError::ContractDrift(tool_id)) => print_error_with_warnings(
            command_id,
            &CliError::tool_contract_drift(&tool_id),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(tools::RunToolError::MissingExecutablePath(path)) => print_error_with_warnings(
            command_id,
            &CliError::tool_executable_missing(&path),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(tools::RunToolError::UnsafeActionBlocked {
            tool_id,
            side_effects,
            approval_requirement,
        }) => print_error_with_warnings(
            command_id,
            &CliError::unsafe_tool_run_blocked(&tool_id, &side_effects, &approval_requirement),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(tools::RunToolError::ProcessFailed(exit_code)) => print_error_with_warnings(
            command_id,
            &CliError::tool_process_failed(exit_code),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
    }
}

fn validate_tool_run_args(args: &[String]) -> Result<(), CliError> {
    if args.len() > MAX_TOOL_RUN_ARGS {
        return Err(CliError::tool_run_args_too_large(format!(
            "tool arguments exceed {MAX_TOOL_RUN_ARGS} items"
        )));
    }
    if args.iter().any(|arg| arg.len() > MAX_TOOL_RUN_ARG_BYTES) {
        return Err(CliError::tool_run_args_too_large(format!(
            "a tool argument exceeds {MAX_TOOL_RUN_ARG_BYTES} bytes"
        )));
    }
    if args.iter().map(String::len).sum::<usize>() > MAX_TOOL_RUN_TOTAL_ARG_BYTES {
        return Err(CliError::tool_run_args_too_large(format!(
            "tool arguments exceed {MAX_TOOL_RUN_TOTAL_ARG_BYTES} bytes in total"
        )));
    }

    Ok(())
}

fn run_artifacts_cleanup(
    command_id: &'static str,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_read_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    let result = artifacts::cleanup_workspace_artifacts(&workspace_paths, &connection);
    drop(connection);
    observation.freeze_terminal_duration_ms();
    observation.record_terminal(artifact_cleanup_observation_event(&result));

    match result {
        Ok(report) => print_success_with_warnings(
            command_id,
            json!(report),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
            EXIT_SUCCESS,
        ),
        Err(artifacts::ArtifactError::Io(error)) => print_error_with_warnings(
            command_id,
            &CliError::io(error),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(artifacts::ArtifactError::Db(error)) => print_error_with_warnings(
            command_id,
            &CliError::db_schema(error),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(error @ artifacts::ArtifactError::LockTimeout { .. }) => print_error_with_warnings(
            command_id,
            &CliError::artifacts(&error),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(
            error @ (artifacts::ArtifactError::CleanupPartial { .. }
            | artifacts::ArtifactError::CleanupStateUnknown { .. }
            | artifacts::ArtifactError::RetentionLimitNotMet { .. }),
        ) => print_artifact_cleanup_error_with_warnings(
            command_id,
            &error,
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(error) => print_error_with_warnings(
            command_id,
            &CliError::artifacts(&error),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
    }
}

fn handle_parse_error(error: clap::Error, json: bool) -> ExitCode {
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => error.exit(),
        _ if json => {
            let cli_error = CliError::invalid_args();
            println!("{}", error_envelope("parse", &cli_error));
            ExitCode::from(EXIT_INVALID_ARGS)
        }
        _ => error.exit(),
    }
}

fn json_flag_present<I, T>(args: I) -> bool
where
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == "--json")
}

pub fn success_envelope(command: &'static str, data: Value) -> String {
    success_envelope_with_meta(command, data, OutputMeta::default())
}

#[cfg(test)]
fn success_envelope_with_workspace_key(
    command: &'static str,
    data: Value,
    workspace_key: String,
) -> String {
    success_envelope_with_meta_and_warnings(
        command,
        data,
        OutputMeta {
            version: env!("CARGO_PKG_VERSION"),
            workspace_key: Some(workspace_key),
        },
        Vec::new(),
    )
}

fn success_envelope_with_meta(command: &'static str, data: Value, meta: OutputMeta) -> String {
    success_envelope_with_meta_and_warnings(command, data, meta, Vec::new())
}

fn success_envelope_with_meta_and_warnings(
    command: &'static str,
    data: Value,
    meta: OutputMeta,
    warnings: Vec<mutation::MutationWarning>,
) -> String {
    let envelope = OutputEnvelope {
        ok: true,
        command,
        data: Some(data),
        warnings,
        errors: Vec::new(),
        meta,
    };

    serialize_envelope(&envelope)
}

fn error_envelope(command: &'static str, error: &CliError) -> String {
    error_envelope_with_meta_and_warnings(command, error, OutputMeta::default(), Vec::new())
}

fn error_envelope_with_meta_and_warnings(
    command: &'static str,
    error: &CliError,
    meta: OutputMeta,
    warnings: Vec<mutation::MutationWarning>,
) -> String {
    let envelope = OutputEnvelope {
        ok: false,
        command,
        data: None,
        warnings,
        errors: vec![OutputError {
            code: error.code,
            message: error.message.clone(),
            fix_hint: error.fix_hint.clone(),
            details: None,
        }],
        meta,
    };

    serialize_envelope(&envelope)
}

fn run_node_create(
    command_id: &'static str,
    args: &NodeCreateArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let input = storage::NewNode {
        node_type: args.node_type.clone(),
        status: args.status.clone(),
        title: args.title.clone(),
        summary: args.summary.clone(),
        body: args.body.clone(),
        source_ref: args.source_ref.clone(),
        confidence: args.confidence,
        trust_level: args.trust_level.clone(),
    };

    run_node_create_input(
        command_id,
        &input,
        json_output,
        observation,
        EventType::NodeCreated,
    )
}

fn run_remember(
    command_id: &'static str,
    args: &RememberArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let input = match remember_to_new_node(args) {
        Ok(input) => input,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    run_node_create_input(
        command_id,
        &input,
        json_output,
        observation,
        EventType::Remember,
    )
}

fn run_teach_start(
    command_id: &'static str,
    args: &TeachStartArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let session = storage::NewTeachSession {
        title: args.title.clone(),
        summary: args.summary.clone(),
    };
    if let Err(error) = storage::validate_new_teach_session_input(&session) {
        return print_error(command_id, &CliError::teach(error), json_output);
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::create_teach_session(database, &session)
    }) {
        Ok(outcome) => {
            observation.record_terminal(command_observation_event(
                EventType::TeachStarted,
                EventOutcome::Started,
                EventPayload::Empty,
                None,
            ));
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(error)) => {
            let error = CliError::teach(error);
            record_failed_observation(observation, EventType::TeachStarted, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, EventType::TeachStarted, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_teach_add(
    command_id: &'static str,
    args: &TeachPayloadArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let payload = match parse_teach_payload(&args.payload) {
        Ok(payload) => payload,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::add_teach_material(database, args.session_id, &payload)
    }) {
        Ok(outcome) => print_observed_mutation_success(
            command_id,
            outcome,
            workspace_key,
            observation,
            json_output,
        ),
        Err(mutation::MutationError::Operation(error)) => {
            print_error(command_id, &CliError::teach(error), json_output)
        }
        Err(error) => print_error(
            command_id,
            &mutation_infrastructure_error(error),
            json_output,
        ),
    }
}

fn run_teach_propose(
    command_id: &'static str,
    args: &TeachPayloadArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let proposal = match parse_teach_proposal(&args.payload) {
        Ok(proposal) => proposal,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    if let Err(error) = storage::validate_teach_proposal_input(args.session_id, &proposal) {
        return print_error(command_id, &CliError::teach(error), json_output);
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::store_teach_proposal(database, args.session_id, &proposal)
    }) {
        Ok(outcome) => {
            observation.record_terminal(command_observation_event(
                EventType::TeachProposed,
                EventOutcome::Proposed,
                EventPayload::Empty,
                None,
            ));
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(error)) => {
            let error = CliError::teach(error);
            record_failed_observation(observation, EventType::TeachProposed, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, EventType::TeachProposed, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_teach_apply(
    command_id: &'static str,
    args: &TeachApplyArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::apply_teach_proposal(database, args.session_id, args.proposal_id)
    }) {
        Ok(outcome) => {
            observation.record_terminal(command_observation_event(
                EventType::TeachApplied,
                EventOutcome::Applied,
                EventPayload::Empty,
                None,
            ));
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(error)) => {
            let error = CliError::teach(error);
            record_failed_observation(observation, EventType::TeachApplied, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, EventType::TeachApplied, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_reflect_inventory(
    command_id: &'static str,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        reflection::inventory_sessions(database)
    }) {
        Ok(outcome) => {
            observation.record_terminal(command_observation_event(
                EventType::ReflectionInventory,
                EventOutcome::Success,
                EventPayload::Empty,
                None,
            ));
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(error)) => {
            let error = CliError::reflection(error);
            record_failed_observation(observation, EventType::ReflectionInventory, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, EventType::ReflectionInventory, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_reflect_proposal_create(
    command_id: &'static str,
    args: &ReflectProposalCreateArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let proposal = match parse_reflect_proposal_file(&args.proposal_file) {
        Ok(proposal) => proposal,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    if let Err(error) = reflection::validate_proposal_input(&args.session_id, &proposal) {
        return print_error(command_id, &CliError::reflection(error), json_output);
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        reflection::store_proposal(database, &args.session_id, &proposal)
    }) {
        Ok(outcome) => {
            observation.record_terminal(items_observation_event(
                EventType::ReflectionProposal,
                EventOutcome::Proposed,
                proposal.items.len(),
            ));
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(error)) => {
            let error = CliError::reflection(error);
            record_failed_observation(observation, EventType::ReflectionProposal, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, EventType::ReflectionProposal, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_reflect_proposal_apply(
    command_id: &'static str,
    args: &ReflectProposalApplyArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        reflection::attempt_apply_proposal(database, args.proposal_id)
    }) {
        Ok(mutation::MutationOutcome {
            value: reflection::ReflectionApplyAttempt::Applied(report),
            warning,
            snapshot_report,
            snapshot_observation,
        }) => {
            if !report.applied_item_indexes.is_empty() {
                observation.record_terminal(items_observation_event(
                    EventType::ReflectionApplied,
                    EventOutcome::Applied,
                    report.applied_item_indexes.len(),
                ));
            }
            if !report.draft_items.is_empty() {
                observation.record_terminal(items_observation_event(
                    EventType::ReflectionApplied,
                    EventOutcome::Drafted,
                    report.draft_items.len(),
                ));
            }
            print_observed_mutation_success(
                command_id,
                mutation::MutationOutcome {
                    value: report,
                    warning,
                    snapshot_report,
                    snapshot_observation,
                },
                workspace_key,
                observation,
                json_output,
            )
        }
        Ok(mutation::MutationOutcome {
            value: reflection::ReflectionApplyAttempt::Failed { error },
            warning,
            snapshot_observation,
            ..
        }) => {
            record_snapshot_observation(observation, snapshot_observation);
            let error = CliError::reflection(error);
            record_failed_observation(observation, EventType::ReflectionApplied, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(warning.into_iter().collect()),
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(error)) => {
            let error = CliError::reflection(error);
            record_failed_observation(observation, EventType::ReflectionApplied, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, EventType::ReflectionApplied, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn remember_to_new_node(args: &RememberArgs) -> Result<storage::NewNode, CliError> {
    if args.note.is_some() && args.body.is_some() {
        return Err(CliError::remember_note_conflict());
    }

    let title = args
        .title
        .clone()
        .or_else(|| args.note.clone())
        .ok_or_else(|| CliError::validation(storage::NodeValidationError::MissingTitle))?;
    let body = args.body.clone().or_else(|| {
        if args.title.is_some() {
            args.note.clone()
        } else {
            None
        }
    });

    Ok(storage::NewNode {
        node_type: args
            .node_type
            .clone()
            .unwrap_or_else(|| "raw_note".to_string()),
        status: args.status.clone().unwrap_or_else(|| "draft".to_string()),
        title,
        summary: args.summary.clone(),
        body,
        source_ref: args.source_ref.clone(),
        confidence: args.confidence,
        trust_level: args.trust_level.clone(),
    })
}

fn parse_teach_payload(payload: &str) -> Result<Value, CliError> {
    validate_structured_payload_size("teach payload", payload.len())?;
    serde_json::from_str(payload).map_err(CliError::teach_payload_json)
}

fn parse_teach_proposal(payload: &str) -> Result<storage::TeachProposalInput, CliError> {
    validate_structured_payload_size("teach payload", payload.len())?;
    serde_json::from_str(payload).map_err(CliError::teach_payload_json)
}

fn parse_reflect_proposal_file(
    proposal_file: &PathBuf,
) -> Result<reflection::ReflectionProposalInput, CliError> {
    let payload = read_bounded_utf8_file(proposal_file, "reflection proposal file")?;
    serde_json::from_str(&payload).map_err(CliError::reflection_proposal_json)
}

fn validate_structured_payload_size(input: &'static str, bytes: usize) -> Result<(), CliError> {
    if bytes > MAX_STRUCTURED_PAYLOAD_BYTES {
        return Err(CliError::input_too_large(
            input,
            MAX_STRUCTURED_PAYLOAD_BYTES,
        ));
    }

    Ok(())
}

fn read_bounded_utf8_file(path: &PathBuf, input: &'static str) -> Result<String, CliError> {
    let file = fs::File::open(path).map_err(CliError::io)?;
    let mut bytes = Vec::with_capacity(MAX_STRUCTURED_PAYLOAD_BYTES.saturating_add(1));
    file.take((MAX_STRUCTURED_PAYLOAD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(CliError::io)?;
    validate_structured_payload_size(input, bytes.len())?;

    String::from_utf8(bytes).map_err(|error| {
        CliError::io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("input is not valid UTF-8: {error}"),
        ))
    })
}

fn run_node_create_input(
    command_id: &'static str,
    input: &storage::NewNode,
    json_output: bool,
    observation: &mut CommandObservation,
    event_type: EventType,
) -> ExitCode {
    if let Err(error) = storage::validate_new_node_input(input) {
        return print_error(command_id, &CliError::validation(error), json_output);
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::create_node(database, input)
    }) {
        Ok(outcome) => {
            observation.record_terminal(node_observation_event(
                event_type,
                EventOutcome::Recorded,
                &outcome.value,
            ));
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(storage::NodeStorageError::Validation(error))) => {
            let error = CliError::validation(error);
            record_failed_observation(observation, event_type, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(storage::NodeStorageError::Db(error))) => {
            let error = CliError::db_schema(error);
            record_failed_observation(observation, event_type, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, event_type, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_init(
    command_id: &'static str,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let repo_root = match storage::resolve_current_workspace_root() {
        Ok(path) => path,
        Err(error) => return print_error(command_id, &CliError::io(error), json_output),
    };
    let stdin = io::stdin();
    let mut input = stdin.lock();

    if json_output {
        let stderr = io::stderr();
        let mut prompt_output = stderr.lock();
        run_init_with_io(
            command_id,
            json_output,
            &repo_root,
            &mut input,
            &mut prompt_output,
            observation,
        )
    } else {
        let stdout = io::stdout();
        let mut prompt_output = stdout.lock();
        run_init_with_io(
            command_id,
            json_output,
            &repo_root,
            &mut input,
            &mut prompt_output,
            observation,
        )
    }
}

fn run_init_with_io<R, W>(
    command_id: &'static str,
    json_output: bool,
    repo_root: &std::path::Path,
    input: &mut R,
    prompt_output: &mut W,
    observation: &mut CommandObservation,
) -> ExitCode
where
    R: BufRead,
    W: Write,
{
    let mut progress = None;
    match install::run_install_flow_with_progress(repo_root, input, prompt_output, &mut progress) {
        Ok(mut status) => {
            attach_install_observation(observation, &status.workspace_key);
            observation.freeze_terminal_duration_ms();
            observation.record(command_observation_event(
                EventType::InstallStarted,
                EventOutcome::Started,
                EventPayload::Empty,
                None,
            ));
            observation.record_terminal(workspace_init_observation_event(&status));
            record_snapshot_observation(observation, status.snapshot_observation);
            observation.record_terminal(command_observation_event(
                EventType::InstallCompleted,
                EventOutcome::Success,
                EventPayload::Empty,
                None,
            ));
            let warnings = observation
                .warnings_after(status.audit_warning.take().into_iter().collect::<Vec<_>>());
            if json_output {
                println!(
                    "{}",
                    success_envelope_with_meta_and_warnings(
                        command_id,
                        json!({
                            "initialized": true,
                            "db_created": status.db_created,
                            "seeded_nodes_created": status.seeded_nodes_created,
                            "seeded_nodes_existing": status.seeded_nodes_existing,
                            "semantic_nodes_created": status.semantic_nodes_created,
                            "semantic_nodes_existing": status.semantic_nodes_existing,
                            "understand_anything_enabled": status.understand_anything_enabled,
                            "codebase_memory_enabled": status.codebase_memory_enabled,
                            "style": "default",
                        }),
                        OutputMeta {
                            version: env!("CARGO_PKG_VERSION"),
                            workspace_key: Some(status.workspace_key),
                        },
                        warnings,
                    )
                );
            } else {
                println!("AOPMem готов.");
                print_text_warnings(warnings);
            }

            ExitCode::from(EXIT_SUCCESS)
        }
        Err(error) => {
            let error = workspace_init_cli_error(error);
            if let Some(mut status) = progress {
                attach_install_observation(observation, &status.workspace_key);
                observation.freeze_terminal_duration_ms();
                observation.record(command_observation_event(
                    EventType::InstallStarted,
                    EventOutcome::Started,
                    EventPayload::Empty,
                    None,
                ));
                observation.record_terminal(workspace_init_observation_event(&status));
                record_snapshot_observation(observation, status.snapshot_observation);
                record_failed_observation(observation, EventType::InstallFailed, &error);
                return print_error_with_warnings(
                    command_id,
                    &error,
                    status.workspace_key,
                    observation.warnings_after(status.audit_warning.take().into_iter().collect()),
                    json_output,
                );
            }
            print_error(command_id, &error, json_output)
        }
    }
}

fn attach_install_observation(observation: &mut CommandObservation, workspace_key: &str) {
    match storage::resolve_paths() {
        Ok(paths) => {
            let workspace_paths = storage::workspace_paths_for_key(&paths, workspace_key);
            observation.attach_workspace(&workspace_paths);
        }
        Err(_) => observation.latch_write_warning(),
    }
}

fn workspace_init_cli_error(error: install::WorkspaceInitError) -> CliError {
    match error {
        install::WorkspaceInitError::Path(error) => CliError::path(error),
        install::WorkspaceInitError::WorkspaceKey(error) => CliError::workspace_key(error),
        install::WorkspaceInitError::WorkspaceResolve(error) => CliError::workspace_resolve(error),
        install::WorkspaceInitError::InvalidUtf8Input => CliError::invalid_utf8_input(),
        install::WorkspaceInitError::SuspiciousMojibakeInput => {
            CliError::suspicious_mojibake_input()
        }
        install::WorkspaceInitError::InputTooLarge { max_bytes } => {
            CliError::input_too_large("install answer", max_bytes)
        }
        install::WorkspaceInitError::Io(error) => CliError::io(error),
        install::WorkspaceInitError::Db(error) => CliError::db_schema(error),
        install::WorkspaceInitError::Seed(storage::NodeStorageError::Validation(error)) => {
            CliError::validation(error)
        }
        install::WorkspaceInitError::Seed(storage::NodeStorageError::Db(error)) => {
            CliError::db_schema(error)
        }
    }
}

fn run_node_get(command_id: &'static str, args: &NodeGetArgs, json_output: bool) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::get_node(&connection, args.id) {
        Ok(Some(node)) => print_success(
            command_id,
            json!(node),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Ok(None) => print_error(command_id, &CliError::node_not_found(args.id), json_output),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

#[derive(Debug)]
enum ListError {
    Db(rusqlite::Error),
    Pagination(&'static str),
}

impl From<rusqlite::Error> for ListError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

fn run_node_list(command_id: &'static str, args: &NodeListArgs, json_output: bool) -> ExitCode {
    let after_id = match args.cursor.as_deref() {
        Some(cursor) => match decode_node_cursor(cursor) {
            Ok(id) => Some(id),
            Err(reason) => {
                return print_error(command_id, &CliError::invalid_cursor(reason), json_output)
            }
        },
        None => args.after_id,
    };

    let (workspace_key, mut connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    let node_page = if args.all {
        list_all_nodes(&mut connection, args.limit, args.include_body)
    } else {
        storage::list_nodes_page(&connection, after_id, args.limit, args.include_body)
            .map_err(ListError::Db)
            .and_then(|page| {
                validate_node_page(&page, after_id, args.limit)?;
                Ok(page)
            })
    };

    match node_page.and_then(|page| node_list_data(page, args.include_body)) {
        Ok(data) => print_success(command_id, data, workspace_key, json_output, EXIT_SUCCESS),
        Err(ListError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(ListError::Pagination(reason)) => {
            print_error(command_id, &CliError::pagination(reason), json_output)
        }
    }
}

fn list_all_nodes(
    connection: &mut rusqlite::Connection,
    page_size: usize,
    include_body: bool,
) -> Result<storage::NodePage, ListError> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)?;
    let result = collect_all_node_pages(page_size, |after_id, limit| {
        storage::list_nodes_page(&transaction, after_id, limit, include_body).map_err(ListError::Db)
    });

    match result {
        Ok(page) => {
            transaction.commit()?;
            Ok(page)
        }
        Err(error) => Err(error),
    }
}

fn collect_all_node_pages(
    page_size: usize,
    mut fetch: impl FnMut(Option<i64>, usize) -> Result<storage::NodePage, ListError>,
) -> Result<storage::NodePage, ListError> {
    let mut nodes = Vec::new();
    let mut after_id = None;
    let mut body_omitted = false;
    let mut content_truncated = false;

    loop {
        let page = fetch(after_id, page_size)?;
        let next_after_id = validate_node_page(&page, after_id, page_size)?;
        let more_results = page.page.more_results;
        body_omitted |= page.body_omitted;
        content_truncated |= page.content_truncated;
        nodes.extend(page.page.items);

        if !more_results {
            return Ok(storage::NodePage {
                page: storage::Page {
                    items: nodes,
                    next_after_id: None,
                    more_results: false,
                },
                body_omitted,
                content_truncated,
            });
        }
        after_id = next_after_id;
    }
}

fn validate_node_page(
    page: &storage::NodePage,
    after_id: Option<i64>,
    page_size: usize,
) -> Result<Option<i64>, ListError> {
    validate_page(&page.page, after_id.as_ref(), page_size, |node| node.id)
}

fn validate_page<T, Key>(
    page: &storage::Page<T, Key>,
    after_key: Option<&Key>,
    page_size: usize,
    key_for: impl Fn(&T) -> Key,
) -> Result<Option<Key>, ListError>
where
    Key: Clone + Ord,
{
    if page.items.len() > page_size {
        return Err(ListError::Pagination(
            "list page exceeded the requested page size",
        ));
    }

    let mut last_key = after_key.cloned();
    for item in &page.items {
        let key = key_for(item);
        if last_key.as_ref().is_some_and(|last| key <= *last) {
            return Err(ListError::Pagination(
                "list page contained a duplicate or non-progressing key",
            ));
        }
        last_key = Some(key);
    }

    match (&page.more_results, &page.next_after_id) {
        (true, Some(next_key)) if !page.items.is_empty() && last_key.as_ref() == Some(next_key) => {
            Ok(Some(next_key.clone()))
        }
        (true, _) => Err(ListError::Pagination(
            "incomplete list page did not provide a progressing cursor",
        )),
        (false, None) => Ok(None),
        (false, Some(_)) => Err(ListError::Pagination(
            "complete list page unexpectedly provided a continuation cursor",
        )),
    }
}

fn collect_all_pages<T, Key>(
    page_size: usize,
    mut fetch: impl FnMut(Option<&Key>, usize) -> Result<storage::Page<T, Key>, ListError>,
    key_for: impl Fn(&T) -> Key + Copy,
) -> Result<storage::Page<T, Key>, ListError>
where
    Key: Clone + Ord,
{
    let mut items = Vec::new();
    let mut after_key = None;

    loop {
        let page = fetch(after_key.as_ref(), page_size)?;
        let next_key = validate_page(&page, after_key.as_ref(), page_size, key_for)?;
        let more_results = page.more_results;
        items.extend(page.items);

        if !more_results {
            return Ok(storage::Page {
                items,
                next_after_id: None,
                more_results: false,
            });
        }
        after_key = next_key;
    }
}

fn list_all_pages_in_read_transaction<T, Key>(
    connection: &mut rusqlite::Connection,
    page_size: usize,
    mut fetch: impl FnMut(
        &rusqlite::Connection,
        Option<&Key>,
        usize,
    ) -> rusqlite::Result<storage::Page<T, Key>>,
    key_for: impl Fn(&T) -> Key + Copy,
) -> Result<storage::Page<T, Key>, ListError>
where
    Key: Clone + Ord,
{
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)?;
    let result = collect_all_pages(
        page_size,
        |after_key, limit| fetch(&transaction, after_key, limit).map_err(ListError::Db),
        key_for,
    );

    match result {
        Ok(page) => {
            transaction.commit()?;
            Ok(page)
        }
        Err(error) => Err(error),
    }
}

fn numeric_list_data<T: Serialize>(
    field: &str,
    page: storage::Page<T>,
    kind: CursorKind,
    scope: &str,
) -> Result<Value, ListError> {
    let next_cursor = page
        .next_after_id
        .map(|id| encode_numeric_cursor(kind, scope, id))
        .transpose()
        .map_err(ListError::Pagination)?;
    Ok(list_data(field, page.items, next_cursor, page.more_results))
}

fn string_list_data<T: Serialize>(
    field: &str,
    page: storage::Page<T, String>,
    kind: CursorKind,
    scope: &str,
) -> Result<Value, ListError> {
    let next_cursor = page
        .next_after_id
        .as_deref()
        .map(|key| encode_list_cursor(kind, scope, key))
        .transpose()
        .map_err(ListError::Pagination)?;
    Ok(list_data(field, page.items, next_cursor, page.more_results))
}

fn list_data<T: Serialize>(
    field: &str,
    items: Vec<T>,
    next_cursor: Option<String>,
    more_results: bool,
) -> Value {
    let mut data = serde_json::Map::new();
    data.insert(
        field.to_string(),
        serde_json::to_value(items).expect("list items must serialize"),
    );
    data.insert("next_cursor".to_string(), json!(next_cursor));
    data.insert("more_results".to_string(), Value::Bool(more_results));
    Value::Object(data)
}

fn numeric_cursor_or_legacy(
    cursor: Option<&str>,
    legacy_after_id: Option<i64>,
    kind: CursorKind,
    scope: &str,
) -> Result<Option<i64>, CliError> {
    cursor.map_or(Ok(legacy_after_id), |cursor| {
        decode_numeric_cursor(cursor, kind, scope)
            .map(Some)
            .map_err(CliError::invalid_cursor)
    })
}

fn string_cursor_or_legacy(
    cursor: Option<&str>,
    legacy_after_id: Option<&str>,
    kind: CursorKind,
    scope: &str,
) -> Result<Option<String>, CliError> {
    cursor.map_or_else(
        || Ok(legacy_after_id.map(str::to_string)),
        |cursor| {
            decode_list_cursor(cursor, kind, scope)
                .map(Some)
                .map_err(CliError::invalid_cursor)
        },
    )
}

fn print_list_outcome(
    command_id: &'static str,
    outcome: Result<Value, ListError>,
    workspace_key: String,
    json_output: bool,
) -> ExitCode {
    match outcome {
        Ok(data) => print_success(command_id, data, workspace_key, json_output, EXIT_SUCCESS),
        Err(ListError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(ListError::Pagination(reason)) => {
            print_error(command_id, &CliError::pagination(reason), json_output)
        }
    }
}

fn tool_contracts_page(
    page: tools::ToolContractsPage,
) -> storage::Page<tools::ToolContractRecord, String> {
    storage::Page {
        items: page.items,
        next_after_id: page.next_after_id,
        more_results: page.more_results,
    }
}

fn node_list_data(node_page: storage::NodePage, include_body: bool) -> Result<Value, ListError> {
    let storage::NodePage {
        page,
        body_omitted,
        content_truncated,
    } = node_page;
    let next_cursor = page
        .next_after_id
        .map(encode_node_cursor)
        .transpose()
        .map_err(ListError::Pagination)?;
    let nodes = page
        .items
        .into_iter()
        .map(|node| {
            let mut value = json!(node);
            if !include_body {
                value
                    .as_object_mut()
                    .expect("serialized node must be a JSON object")
                    .remove("body");
            }
            value
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "nodes": nodes,
        "next_cursor": next_cursor,
        "more_results": page.more_results,
        "body_omitted": body_omitted,
        "content_truncated": content_truncated,
    }))
}

fn run_node_update(
    command_id: &'static str,
    args: &NodeUpdateArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let update = storage::NodeUpdate {
        id: args.id,
        status: args.status.clone(),
        title: args.title.clone(),
        summary: args.summary.clone(),
        body: args.body.clone(),
        source_ref: args.source_ref.clone(),
        confidence: args.confidence,
        trust_level: args.trust_level.clone(),
    };
    if let Err(error) = storage::validate_node_update_input(&update) {
        return print_error(command_id, &CliError::validation(error), json_output);
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);
    let requested_event_type = if update.status == "deprecated" {
        EventType::NodeDeprecated
    } else {
        EventType::NodeUpdated
    };

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::update_node(database, &update)
    }) {
        Ok(outcome) if outcome.value.is_some() => {
            let node = outcome.value.expect("checked node option");
            let event_type = if node.status == "deprecated" {
                EventType::NodeDeprecated
            } else {
                EventType::NodeUpdated
            };
            observation.record_terminal(node_observation_event(
                event_type,
                EventOutcome::Success,
                &node,
            ));
            let outcome = mutation::MutationOutcome {
                value: node,
                warning: outcome.warning,
                snapshot_report: outcome.snapshot_report,
                snapshot_observation: outcome.snapshot_observation,
            };
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Ok(outcome) => {
            record_snapshot_observation(observation, outcome.snapshot_observation);
            let error = CliError::node_not_found(args.id);
            record_failed_observation(observation, requested_event_type, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(outcome.warning.into_iter().collect()),
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(storage::NodeStorageError::Validation(error))) => {
            let error = CliError::validation(error);
            record_failed_observation(observation, requested_event_type, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(storage::NodeStorageError::Db(error))) => {
            let error = CliError::db_schema(error);
            record_failed_observation(observation, requested_event_type, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, requested_event_type, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_status(command_id: &'static str, json_output: bool) -> ExitCode {
    match install::global_install_status() {
        Ok(status) => {
            if json_output {
                println!(
                    "{}",
                    success_envelope(command_id, json!({ "install": status }))
                );
            } else {
                println!(
                    "global install: {}\ndirs: {}\nbin: {}\ntemplates: {}",
                    status.status.as_str(),
                    status.dirs.as_str(),
                    status.bin.as_str(),
                    status.templates.as_str(),
                );
            }

            ExitCode::from(EXIT_SUCCESS)
        }
        Err(error) => print_error(command_id, &CliError::path(error), json_output),
    }
}

fn run_observe_status(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths) = match current_workspace_observability_target() {
        Ok(target) => target,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    match observability_report::observe_status(&workspace_paths, &workspace_key) {
        Ok(status) => {
            if json_output {
                print_success(command_id, json!(status), workspace_key, true, EXIT_SUCCESS)
            } else {
                match (&status.collection_status, &status.facts) {
                    (CollectionStatus::NotCollected, _) => {
                        println!("Local Observability: not_collected");
                    }
                    (CollectionStatus::Ready, Some(facts)) => {
                        println!(
                            "Local Observability: ready\nevents: {}\nrecall_bundles: {}\nbundle_nodes: {}\nfeedback: {}",
                            facts.observability_events,
                            facts.recall_bundles,
                            facts.bundle_nodes,
                            facts.feedback,
                        );
                    }
                    (CollectionStatus::Ready, None) => {
                        return print_error(
                            command_id,
                            &CliError::observability_read(ObserveReadError::InvalidStore),
                            false,
                        );
                    }
                }
                ExitCode::from(EXIT_SUCCESS)
            }
        }
        Err(error) => print_error(
            command_id,
            &CliError::observability_read(error),
            json_output,
        ),
    }
}

fn run_observe_report(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths) = match current_workspace_observability_target() {
        Ok(target) => target,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    match observability_report::effectiveness_report(&workspace_paths, &workspace_key) {
        Ok(report) => {
            if json_output {
                print_success(command_id, json!(report), workspace_key, true, EXIT_SUCCESS)
            } else {
                match (&report.collection_status, &report.facts) {
                    (CollectionStatus::NotCollected, _) => {
                        println!(
                            "Local Observability: not_collected\nperiod: {} .. {}",
                            report.period.start_at, report.period.end_at,
                        );
                    }
                    (CollectionStatus::Ready, Some(facts)) => {
                        println!(
                            "Local Observability report\nperiod: {} .. {}\nrecalls: {}\nfailed: {}\nempty: {}\ntool_success: {}\ntool_failure: {}\ntool_timeout: {}",
                            report.period.start_at,
                            report.period.end_at,
                            facts.recall.count,
                            facts.recall.failed,
                            facts.recall.empty,
                            facts.tools.success,
                            facts.tools.failure,
                            facts.tools.timeout,
                        );
                    }
                    (CollectionStatus::Ready, None) => {
                        return print_error(
                            command_id,
                            &CliError::observability_read(ObserveReadError::InvalidStore),
                            false,
                        );
                    }
                }
                ExitCode::from(EXIT_SUCCESS)
            }
        }
        Err(error) => print_error(
            command_id,
            &CliError::observability_read(error),
            json_output,
        ),
    }
}

fn run_observe_export(
    command_id: &'static str,
    args: &ObserveExportArgs,
    json_output: bool,
) -> ExitCode {
    let (workspace_key, workspace_paths) = match current_workspace_observability_target() {
        Ok(target) => target,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    match observability_export::export_debug_capsule(&workspace_key, &workspace_paths, &args.output)
    {
        Ok(result) => {
            let warnings = result.warning.clone().into_iter().collect::<Vec<_>>();
            if json_output {
                print_success_with_warnings(
                    command_id,
                    json!(result),
                    workspace_key,
                    warnings,
                    true,
                    EXIT_SUCCESS,
                )
            } else {
                let collection_status = match result.collection_status {
                    CollectionStatus::NotCollected => "not_collected",
                    CollectionStatus::Ready => "ready",
                };
                println!(
                    "debug capsule: {}\nentries: {}\nbytes: {}\nobservability: {}\npublication: {}\ntemporary_cleanup_confirmed: {}",
                    result.output,
                    result.entries,
                    result.bytes,
                    collection_status,
                    result.publication_status.as_str(),
                    result.temporary_cleanup_confirmed,
                );
                print_text_warnings(warnings);
                ExitCode::from(EXIT_SUCCESS)
            }
        }
        Err(error) => print_error(command_id, &CliError::debug_capsule(error), json_output),
    }
}

fn run_ui(command_id: &'static str, args: &UiArgs, json_output: bool) -> ExitCode {
    let (workspace_key, started) = match prepare_ui_with_launcher(args, &ui::SystemBrowserLauncher)
    {
        Ok(prepared) => prepared,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let warnings = started.warning().cloned().into_iter().collect::<Vec<_>>();
    if json_output {
        println!(
            "{}",
            success_envelope_with_meta_and_warnings(
                command_id,
                json!({
                    "url": started.url(),
                    "bind": "127.0.0.1",
                    "port": started.port(),
                    "read_only": true,
                }),
                OutputMeta {
                    version: env!("CARGO_PKG_VERSION"),
                    workspace_key: Some(workspace_key),
                },
                warnings,
            )
        );
    } else {
        println!("AOPMem UI: {}", started.url());
        print_text_warnings(warnings);
    }
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
    match started.serve() {
        Ok(()) => ExitCode::from(EXIT_SUCCESS),
        Err(error) => print_error(command_id, &CliError::ui(error), json_output),
    }
}

fn prepare_ui_with_launcher(
    args: &UiArgs,
    launcher: &dyn ui::BrowserLauncher,
) -> Result<(String, ui::StartedUi), CliError> {
    let (workspace_key, workspace_paths, connection) = open_current_workspace_read_context()?;
    drop(connection);
    let context = ui::data::UiDataContext::new(workspace_key.clone(), workspace_paths);
    let started = ui::start_with_launcher(
        ui::UiOptions::new(args.port, args.no_open),
        context,
        launcher,
    )
    .map_err(CliError::ui)?;
    Ok((workspace_key, started))
}

#[cfg(test)]
fn doctor_success_envelope(report: &verify::DoctorReport) -> String {
    success_envelope_with_workspace_key("doctor", json!(report), report.workspace_key.clone())
}

#[cfg(test)]
fn verify_success_envelope(report: &verify::LintReport) -> String {
    success_envelope_with_workspace_key("verify", json!(report), report.workspace_key.clone())
}

fn existing_observation_workspace(
    repo_root: &std::path::Path,
) -> Option<(String, storage::WorkspacePaths)> {
    let paths = storage::resolve_paths().ok()?;
    let workspace_key = storage::resolve_workspace_key(&paths, repo_root).ok()?;
    let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
    storage::validate_workspace_read_paths(&workspace_paths).ok()?;
    Some((workspace_key, workspace_paths))
}

fn doctor_observation_event(
    report: &verify::DoctorReport,
) -> Result<CollectorEvent, CollectorInputError> {
    let statuses = [
        report.checks.global_dirs.status,
        report.checks.workspace.status,
        report.checks.db.status,
        report.checks.schema.status,
        report.checks.fts.status,
        report.checks.adapter_block.status,
        report.checks.artifacts_dirs.status,
        report.checks.audit_snapshot.status,
        report.checks.tools_dirs.status,
    ];
    let ready = statuses
        .iter()
        .filter(|status| **status == verify::DoctorStatus::Ready)
        .count();
    let missing = statuses
        .iter()
        .filter(|status| **status == verify::DoctorStatus::Missing)
        .count();
    let error = statuses
        .iter()
        .filter(|status| **status == verify::DoctorStatus::Error)
        .count();
    let counts = [
        ("checks", u64::try_from(statuses.len())),
        ("ready", u64::try_from(ready)),
        ("missing", u64::try_from(missing)),
        ("error", u64::try_from(error)),
    ];
    let counts = counts
        .into_iter()
        .map(|(name, count)| {
            count
                .map(|count| (name, count))
                .map_err(|_| CollectorInputError::Serialization)
        })
        .collect::<Result<Vec<_>, _>>()?;
    counts_observation_event(
        EventType::Doctor,
        if report.healthy {
            EventOutcome::Success
        } else {
            EventOutcome::Warning
        },
        &counts,
        None,
    )
}

fn verify_observation_event(
    report: &verify::LintReport,
) -> Result<CollectorEvent, CollectorInputError> {
    let counts = [
        ("total", report.summary.total),
        ("duplicate_ids", report.summary.duplicate_ids),
        ("broken_links", report.summary.broken_links),
        (
            "deprecated_active_links",
            report.summary.deprecated_active_links,
        ),
        ("missing_source", report.summary.missing_source),
        ("missing_summary", report.summary.missing_summary),
        ("missing_gates", report.summary.missing_gates),
        ("adapter_block_drift", report.summary.adapter_block_drift),
        ("schema_drift", report.summary.schema_drift),
        (
            "forbidden_feature_terms",
            report.summary.forbidden_feature_terms,
        ),
        (
            "pending_audit_snapshot",
            report.summary.pending_audit_snapshot,
        ),
    ]
    .into_iter()
    .map(|(name, count)| {
        u64::try_from(count)
            .map(|count| (name, count))
            .map_err(|_| CollectorInputError::Serialization)
    })
    .collect::<Result<Vec<_>, _>>()?;
    counts_observation_event(
        EventType::Verify,
        if report.clean {
            EventOutcome::Success
        } else {
            EventOutcome::Warning
        },
        &counts,
        None,
    )
}

fn run_doctor(
    command_id: &'static str,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let repo_root = match storage::resolve_current_workspace_root() {
        Ok(path) => path,
        Err(error) => return print_error(command_id, &CliError::io(error), json_output),
    };
    let result = verify::run_doctor(&repo_root);
    let observation_workspace = existing_observation_workspace(&repo_root);
    if let Some((_, workspace_paths)) = &observation_workspace {
        observation.attach_workspace(workspace_paths);
        observation.freeze_terminal_duration_ms();
    }

    match result {
        Ok(report) => {
            observation.record_terminal(doctor_observation_event(&report));
            let warnings = observation.warnings_after(Vec::new());
            if json_output {
                println!(
                    "{}",
                    success_envelope_with_meta_and_warnings(
                        command_id,
                        json!(report),
                        OutputMeta {
                            version: env!("CARGO_PKG_VERSION"),
                            workspace_key: Some(report.workspace_key.clone()),
                        },
                        warnings,
                    )
                );
                ExitCode::from(EXIT_SUCCESS)
            } else {
                println!(
                    "doctor: {}\nglobal_dirs: {}\nworkspace: {}\ndb: {}\nschema: {}\nfts: {}\nadapter_block: {}\nartifacts_dirs: {}\naudit_snapshot: {}\ntools_dirs: {}",
                    if report.healthy { "ready" } else { "issues" },
                    report.checks.global_dirs.status.as_str(),
                    report.checks.workspace.status.as_str(),
                    report.checks.db.status.as_str(),
                    report.checks.schema.status.as_str(),
                    report.checks.fts.status.as_str(),
                    report.checks.adapter_block.status.as_str(),
                    report.checks.artifacts_dirs.status.as_str(),
                    report.checks.audit_snapshot.status.as_str(),
                    report.checks.tools_dirs.status.as_str(),
                );
                print_text_warnings(warnings);

                ExitCode::from(EXIT_SUCCESS)
            }
        }
        Err(error) => {
            let error = match error {
                verify::DoctorError::Path(error) => CliError::path(error),
                verify::DoctorError::WorkspaceResolve(error) => CliError::workspace_resolve(error),
                verify::DoctorError::Io(error) => CliError::io(error),
            };
            observation.record_terminal(counts_observation_event(
                EventType::Doctor,
                EventOutcome::Failure,
                &[],
                Some(error.code),
            ));
            match observation_workspace {
                Some((workspace_key, _)) => print_error_with_warnings(
                    command_id,
                    &error,
                    workspace_key,
                    observation.warnings_after(Vec::new()),
                    json_output,
                ),
                None => print_error(command_id, &error, json_output),
            }
        }
    }
}

fn run_verify(
    command_id: &'static str,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let repo_root = match storage::resolve_current_workspace_root() {
        Ok(path) => path,
        Err(error) => return print_error(command_id, &CliError::io(error), json_output),
    };

    let result = verify::run_lint(&repo_root);
    let observation_workspace = existing_observation_workspace(&repo_root);
    if let Some((_, workspace_paths)) = &observation_workspace {
        observation.attach_workspace(workspace_paths);
        observation.freeze_terminal_duration_ms();
    }

    match result {
        Ok(report) => {
            let exit_code = if report.clean {
                EXIT_SUCCESS
            } else {
                EXIT_DRIFT_DETECTED
            };

            observation.record_terminal(verify_observation_event(&report));
            let warnings = observation.warnings_after(Vec::new());
            if json_output {
                println!(
                    "{}",
                    success_envelope_with_meta_and_warnings(
                        command_id,
                        json!(report),
                        OutputMeta {
                            version: env!("CARGO_PKG_VERSION"),
                            workspace_key: Some(report.workspace_key.clone()),
                        },
                        warnings,
                    )
                );
            } else {
                println!(
                    "verify: {}\nissues: {}\nduplicate_ids: {}\nbroken_links: {}\ndeprecated_active_links: {}\nmissing_source: {}\nmissing_summary: {}\nmissing_gates: {}\npending_audit_snapshot: {}",
                    if report.clean { "clean" } else { "issues" },
                    report.summary.total,
                    report.summary.duplicate_ids,
                    report.summary.broken_links,
                    report.summary.deprecated_active_links,
                    report.summary.missing_source,
                    report.summary.missing_summary,
                    report.summary.missing_gates,
                    report.summary.pending_audit_snapshot,
                );
                print_text_warnings(warnings);
            }

            ExitCode::from(exit_code)
        }
        Err(error) => {
            let error = match error {
                verify::LintError::Path(error) => CliError::path(error),
                verify::LintError::WorkspaceResolve(error) => CliError::workspace_resolve(error),
                verify::LintError::WorkspaceDbMissing(path) => {
                    CliError::workspace_db_missing(&path)
                }
                verify::LintError::Db(error) => CliError::db_schema(error),
                verify::LintError::Io(error) => CliError::io(error),
            };
            observation.record_terminal(counts_observation_event(
                EventType::Verify,
                EventOutcome::Failure,
                &[],
                Some(error.code),
            ));
            match observation_workspace {
                Some((workspace_key, _)) => print_error_with_warnings(
                    command_id,
                    &error,
                    workspace_key,
                    observation.warnings_after(Vec::new()),
                    json_output,
                ),
                None => print_error(command_id, &error, json_output),
            }
        }
    }
}

fn run_link_add(
    command_id: &'static str,
    args: &LinkAddArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let input = storage::NewLink {
        source_node_id: args.source_id,
        target_node_id: args.target_id,
        link_type: args.link_type.clone(),
    };
    if let Err(error) = storage::validate_new_link_input(&input) {
        return print_error(command_id, &CliError::link_validation(error), json_output);
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::create_link(database, &input)
    }) {
        Ok(outcome) => {
            observation.record_terminal(link_observation_event(&outcome.value));
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(storage::LinkStorageError::Validation(error))) => {
            let error = CliError::link_validation(error);
            record_failed_observation(observation, EventType::LinkCreated, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(storage::LinkStorageError::Db(error))) => {
            let error = CliError::db_schema(error);
            record_failed_observation(observation, EventType::LinkCreated, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(error) => {
            let error = mutation_infrastructure_error(error);
            record_failed_observation(observation, EventType::LinkCreated, &error);
            print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_link_list(command_id: &'static str, args: &NumericListArgs, json_output: bool) -> ExitCode {
    let after_id = match numeric_cursor_or_legacy(
        args.cursor.as_deref(),
        args.after_id,
        CursorKind::Link,
        "all",
    ) {
        Ok(after_id) => after_id,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, mut connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    let page = if args.all {
        list_all_pages_in_read_transaction(
            &mut connection,
            args.limit,
            |connection, after_id, limit| {
                storage::list_links_page(connection, after_id.copied(), limit)
            },
            |link| link.id,
        )
    } else {
        storage::list_links_page(&connection, after_id, args.limit)
            .map_err(ListError::Db)
            .and_then(|page| {
                validate_page(&page, after_id.as_ref(), args.limit, |link| link.id)?;
                Ok(page)
            })
    };

    print_list_outcome(
        command_id,
        page.and_then(|page| numeric_list_data("links", page, CursorKind::Link, "all")),
        workspace_key,
        json_output,
    )
}

fn run_alias_add(
    command_id: &'static str,
    args: &AliasAddArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let input = storage::NewAlias {
        node_id: args.node_id,
        alias: args.alias.clone(),
    };
    if let Err(error) = storage::validate_new_alias_input(&input) {
        return print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        );
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::create_alias(database, &input)
    }) {
        Ok(outcome) => print_observed_mutation_success(
            command_id,
            outcome,
            workspace_key,
            observation,
            json_output,
        ),
        Err(mutation::MutationError::Operation(storage::MetadataStorageError::Validation(
            error,
        ))) => print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        ),
        Err(mutation::MutationError::Operation(storage::MetadataStorageError::Db(error))) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(error) => print_error(
            command_id,
            &mutation_infrastructure_error(error),
            json_output,
        ),
    }
}

fn run_alias_list(
    command_id: &'static str,
    args: &NodeMetadataListArgs,
    json_output: bool,
) -> ExitCode {
    let scope = metadata_cursor_scope(args.node_id);
    let after_id = match numeric_cursor_or_legacy(
        args.cursor.as_deref(),
        args.after_id,
        CursorKind::Alias,
        &scope,
    ) {
        Ok(after_id) => after_id,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, mut connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    let page = if args.all {
        list_all_pages_in_read_transaction(
            &mut connection,
            args.limit,
            |connection, after_id, limit| {
                storage::list_aliases_page(connection, args.node_id, after_id.copied(), limit)
            },
            |alias| alias.id,
        )
    } else {
        storage::list_aliases_page(&connection, args.node_id, after_id, args.limit)
            .map_err(ListError::Db)
            .and_then(|page| {
                validate_page(&page, after_id.as_ref(), args.limit, |alias| alias.id)?;
                Ok(page)
            })
    };

    print_list_outcome(
        command_id,
        page.and_then(|page| numeric_list_data("aliases", page, CursorKind::Alias, &scope)),
        workspace_key,
        json_output,
    )
}

fn run_tag_add(
    command_id: &'static str,
    args: &TagAddArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let input = storage::NewTag {
        node_id: args.node_id,
        tag: args.tag.clone(),
    };
    if let Err(error) = storage::validate_new_tag_input(&input) {
        return print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        );
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::create_tag(database, &input)
    }) {
        Ok(outcome) => print_observed_mutation_success(
            command_id,
            outcome,
            workspace_key,
            observation,
            json_output,
        ),
        Err(mutation::MutationError::Operation(storage::MetadataStorageError::Validation(
            error,
        ))) => print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        ),
        Err(mutation::MutationError::Operation(storage::MetadataStorageError::Db(error))) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(error) => print_error(
            command_id,
            &mutation_infrastructure_error(error),
            json_output,
        ),
    }
}

fn run_tag_list(
    command_id: &'static str,
    args: &NodeMetadataListArgs,
    json_output: bool,
) -> ExitCode {
    let scope = metadata_cursor_scope(args.node_id);
    let after_id = match numeric_cursor_or_legacy(
        args.cursor.as_deref(),
        args.after_id,
        CursorKind::Tag,
        &scope,
    ) {
        Ok(after_id) => after_id,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, mut connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    let page = if args.all {
        list_all_pages_in_read_transaction(
            &mut connection,
            args.limit,
            |connection, after_id, limit| {
                storage::list_tags_page(connection, args.node_id, after_id.copied(), limit)
            },
            |tag| tag.id,
        )
    } else {
        storage::list_tags_page(&connection, args.node_id, after_id, args.limit)
            .map_err(ListError::Db)
            .and_then(|page| {
                validate_page(&page, after_id.as_ref(), args.limit, |tag| tag.id)?;
                Ok(page)
            })
    };

    print_list_outcome(
        command_id,
        page.and_then(|page| numeric_list_data("tags", page, CursorKind::Tag, &scope)),
        workspace_key,
        json_output,
    )
}

fn run_source_add(
    command_id: &'static str,
    args: &SourceAddArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let input = storage::NewSource {
        node_id: args.node_id,
        source_ref: args.source_ref.clone(),
    };
    if let Err(error) = storage::validate_new_source_input(&input) {
        return print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        );
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::create_source(database, &input)
    }) {
        Ok(outcome) => print_observed_mutation_success(
            command_id,
            outcome,
            workspace_key,
            observation,
            json_output,
        ),
        Err(mutation::MutationError::Operation(storage::MetadataStorageError::Validation(
            error,
        ))) => print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        ),
        Err(mutation::MutationError::Operation(storage::MetadataStorageError::Db(error))) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(error) => print_error(
            command_id,
            &mutation_infrastructure_error(error),
            json_output,
        ),
    }
}

fn run_source_list(
    command_id: &'static str,
    args: &NodeMetadataListArgs,
    json_output: bool,
) -> ExitCode {
    let scope = metadata_cursor_scope(args.node_id);
    let after_id = match numeric_cursor_or_legacy(
        args.cursor.as_deref(),
        args.after_id,
        CursorKind::Source,
        &scope,
    ) {
        Ok(after_id) => after_id,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, mut connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    let page = if args.all {
        list_all_pages_in_read_transaction(
            &mut connection,
            args.limit,
            |connection, after_id, limit| {
                storage::list_sources_page(connection, args.node_id, after_id.copied(), limit)
            },
            |source| source.id,
        )
    } else {
        storage::list_sources_page(&connection, args.node_id, after_id, args.limit)
            .map_err(ListError::Db)
            .and_then(|page| {
                validate_page(&page, after_id.as_ref(), args.limit, |source| source.id)?;
                Ok(page)
            })
    };

    print_list_outcome(
        command_id,
        page.and_then(|page| numeric_list_data("sources", page, CursorKind::Source, &scope)),
        workspace_key,
        json_output,
    )
}

fn run_mcp_list(
    command_id: &'static str,
    args: &StringListArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let after_id = match string_cursor_or_legacy(
        args.cursor.as_deref(),
        args.after_id.as_deref(),
        CursorKind::Mcp,
        "all",
    ) {
        Ok(after_id) => after_id,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, workspace_paths, mut connection) =
        match open_current_workspace_read_context() {
            Ok(workspace) => workspace,
            Err(error) => return print_error(command_id, &error, json_output),
        };
    observation.attach_workspace(&workspace_paths);

    let page = if args.all {
        list_all_pages_in_read_transaction(
            &mut connection,
            args.limit,
            |connection, after_id: Option<&String>, limit| {
                storage::list_mcp_profiles_page(connection, after_id.map(String::as_str), limit)
            },
            |profile| profile.id.clone(),
        )
    } else {
        storage::list_mcp_profiles_page(&connection, after_id.as_deref(), args.limit)
            .map_err(ListError::Db)
            .and_then(|page| {
                validate_page(&page, after_id.as_ref(), args.limit, |profile| {
                    profile.id.clone()
                })?;
                Ok(page)
            })
    };

    drop(connection);
    if let Ok(page) = &page {
        observation.freeze_terminal_duration_ms();
        observation.record_terminal(mcp_status_aggregate_event(page));
    }
    match page.and_then(|page| string_list_data("mcp_profiles", page, CursorKind::Mcp, "all")) {
        Ok(data) => print_success_with_warnings(
            command_id,
            data,
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
            EXIT_SUCCESS,
        ),
        Err(ListError::Db(error)) => print_error_with_warnings(
            command_id,
            &CliError::db_schema(error),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(ListError::Pagination(reason)) => print_error_with_warnings(
            command_id,
            &CliError::pagination(reason),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
    }
}

fn run_mcp_add(
    command_id: &'static str,
    args: &McpAddArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let input = storage::NewMcpProfile {
        id: args.id.clone(),
        name: args.name.clone(),
        kind: args.kind.clone(),
        status: args.status.clone(),
        read_operations: args.read_operations.clone(),
        write_operations: args.write_operations.clone(),
        side_effects: args.side_effects.clone(),
        approval_requirement: args.approval_requirement.clone(),
        credentials_source: args.credentials_source.clone(),
        notes: args.notes.clone(),
    };
    if let Err(error) = storage::validate_new_mcp_profile_input(&input) {
        return print_error(
            command_id,
            &CliError::mcp_profile_validation(error),
            json_output,
        );
    }
    let (workspace_key, workspace_paths) = match current_workspace_mutation_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match mutation::mutate_workspace(&workspace_paths, |database, _effects| {
        storage::create_mcp_profile(database, &input)
    }) {
        Ok(outcome) => {
            record_mcp_status(observation, &outcome.value);
            print_observed_mutation_success(
                command_id,
                outcome,
                workspace_key,
                observation,
                json_output,
            )
        }
        Err(mutation::MutationError::Operation(storage::McpProfileStorageError::Validation(
            error,
        ))) => print_error(
            command_id,
            &CliError::mcp_profile_validation(error),
            json_output,
        ),
        Err(mutation::MutationError::Operation(storage::McpProfileStorageError::Db(error))) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(error) => print_error(
            command_id,
            &mutation_infrastructure_error(error),
            json_output,
        ),
    }
}

fn run_mcp_get(
    command_id: &'static str,
    args: &McpGetArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_read_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    let result = storage::get_mcp_profile(&connection, &args.id);
    drop(connection);
    match result {
        Ok(Some(profile)) => {
            record_mcp_status(observation, &profile);
            print_success_with_warnings(
                command_id,
                json!(profile),
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
                EXIT_SUCCESS,
            )
        }
        Ok(None) => print_error_with_warnings(
            command_id,
            &CliError::mcp_profile_not_found(&args.id),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(error) => print_error_with_warnings(
            command_id,
            &CliError::db_schema(error),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
    }
}

struct RecallObservationFacts {
    payload: Result<RecallPayload, CollectorInputError>,
    bundle_nodes: Result<Vec<RecallBundleNode>, CollectorInputError>,
    empty: bool,
    truncated: bool,
    more_results: bool,
}

impl RecallObservationFacts {
    fn completed_event(&self) -> Result<CollectorEvent, CollectorInputError> {
        self.payload.clone().and_then(|payload| {
            command_observation_event(
                EventType::RecallCompleted,
                EventOutcome::Success,
                EventPayload::Recall(payload),
                None,
            )
        })
    }
}

struct RecallCommandSuccess {
    data: Value,
    facts: RecallObservationFacts,
}

enum RecallCommandError {
    Cli(CliError),
    CursorBinding(CliError),
    MandatoryOverflow {
        hard_limit_bytes: usize,
        used_bytes_before_overflow: usize,
        offending_node_ids: Vec<i64>,
    },
}

fn run_recall(
    command_id: &'static str,
    args: &RecallArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    if args
        .query
        .as_deref()
        .is_some_and(|query| query.trim().is_empty())
    {
        return print_error(command_id, &CliError::invalid_recall_query(), json_output);
    }
    let decoded_cursor = match (args.continuation_cursor.as_deref(), args.query.as_deref()) {
        (Some(cursor), Some(query)) => {
            match recall::decode_recall_continuation_cursor(cursor, query) {
                Ok(state) => Some(state),
                Err(recall::RecallCursorError::Exhausted) => {
                    return print_error(
                        command_id,
                        &CliError::recall_budget_exhausted(),
                        json_output,
                    )
                }
                Err(error) => {
                    return print_error(
                        command_id,
                        &CliError::invalid_recall_cursor(error),
                        json_output,
                    )
                }
            }
        }
        _ => None,
    };

    let incoming_continuation = args.continuation_cursor.is_some();
    let bundle_id = match decoded_cursor.as_ref() {
        Some(state) => {
            let cursor_bundle = match state.bundle_id() {
                Ok(bundle_id) => bundle_id,
                Err(error) => {
                    return print_error(
                        command_id,
                        &CliError::invalid_recall_cursor(error),
                        json_output,
                    )
                }
            };
            if observation
                .bundle_id
                .as_ref()
                .is_some_and(|supplied| supplied != &cursor_bundle)
            {
                return print_error(command_id, &CliError::recall_bundle_mismatch(), json_output);
            }
            cursor_bundle
        }
        None => {
            if observation.bundle_id.is_some() {
                return print_error(
                    command_id,
                    &CliError::recall_bundle_on_new_recall(),
                    json_output,
                );
            }
            recall::RecallBundleId::generate()
        }
    };
    if let Err(error) = observation.bind_recall_bundle(&bundle_id) {
        return print_error(command_id, &error, json_output);
    }
    let (workspace_key, workspace_paths) = match current_workspace_read_target() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);
    let connection = match open_workspace_read_connection(&workspace_paths) {
        Ok(connection) => connection,
        Err(WorkspaceReadOpenError::Missing(error)) => {
            return print_error(command_id, &error, json_output)
        }
        Err(WorkspaceReadOpenError::Existing(error)) => {
            let duration_ms = observation.freeze_terminal_duration_ms();
            let events = vec![
                command_observation_event(
                    EventType::RecallStarted,
                    EventOutcome::Started,
                    EventPayload::Empty,
                    None,
                ),
                command_observation_event(
                    EventType::RecallFailed,
                    EventOutcome::Failure,
                    EventPayload::Empty,
                    Some(error.code),
                )
                .and_then(|event| event.with_duration_ms(duration_ms)),
            ];
            observation.record_recall_bundle(
                RecallBundleRecord::failure(
                    bundle_id.as_str(),
                    duration_ms,
                    error.code,
                    incoming_continuation,
                ),
                events,
            );
            return print_recall_error_with_warnings(
                command_id,
                &error,
                &bundle_id,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            );
        }
    };
    let result = execute_recall_command(
        &connection,
        &workspace_key,
        args,
        decoded_cursor,
        bundle_id.clone(),
    );
    drop(connection);

    let duration_ms = observation.freeze_terminal_duration_ms();
    let mut events = vec![command_observation_event(
        EventType::RecallStarted,
        EventOutcome::Started,
        EventPayload::Empty,
        None,
    )];
    if incoming_continuation {
        events.push(
            command_observation_event(
                EventType::RecallContinuation,
                EventOutcome::Recorded,
                EventPayload::Empty,
                None,
            )
            .and_then(|event| event.with_duration_ms(duration_ms)),
        );
    }

    match result {
        Ok(success) => {
            if success.facts.empty {
                events.push(
                    command_observation_event(
                        EventType::RecallEmpty,
                        EventOutcome::Empty,
                        EventPayload::Empty,
                        None,
                    )
                    .and_then(|event| event.with_duration_ms(duration_ms)),
                );
            }
            if success.facts.truncated {
                events.push(
                    command_observation_event(
                        EventType::RecallTruncated,
                        EventOutcome::Truncated,
                        EventPayload::Empty,
                        None,
                    )
                    .and_then(|event| event.with_duration_ms(duration_ms)),
                );
            }
            events.push(
                success
                    .facts
                    .completed_event()
                    .and_then(|event| event.with_duration_ms(duration_ms)),
            );
            let record = success.facts.bundle_nodes.and_then(|nodes| {
                RecallBundleRecord::success(
                    bundle_id.as_str(),
                    duration_ms,
                    success.facts.more_results,
                    incoming_continuation,
                    nodes,
                )
            });
            observation.record_recall_bundle(record, events);
            print_success_with_warnings(
                command_id,
                success.data,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(RecallCommandError::Cli(error)) => {
            events.push(
                command_observation_event(
                    EventType::RecallFailed,
                    EventOutcome::Failure,
                    EventPayload::Empty,
                    Some(error.code),
                )
                .and_then(|event| event.with_duration_ms(duration_ms)),
            );
            observation.record_recall_bundle(
                RecallBundleRecord::failure(
                    bundle_id.as_str(),
                    duration_ms,
                    error.code,
                    incoming_continuation,
                ),
                events,
            );
            print_recall_error_with_warnings(
                command_id,
                &error,
                &bundle_id,
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(RecallCommandError::CursorBinding(error)) => print_recall_error_with_warnings(
            command_id,
            &error,
            &bundle_id,
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
        ),
        Err(RecallCommandError::MandatoryOverflow {
            hard_limit_bytes,
            used_bytes_before_overflow,
            offending_node_ids,
        }) => {
            let error =
                CliError::mandatory_context_overflow(hard_limit_bytes, offending_node_ids.len());
            events.push(
                mandatory_overflow_observation_event(offending_node_ids.len())
                    .and_then(|event| event.with_duration_ms(duration_ms)),
            );
            events.push(
                command_observation_event(
                    EventType::RecallFailed,
                    EventOutcome::Failure,
                    EventPayload::Empty,
                    Some(error.code),
                )
                .and_then(|event| event.with_duration_ms(duration_ms)),
            );
            observation.record_recall_bundle(
                RecallBundleRecord::failure(
                    bundle_id.as_str(),
                    duration_ms,
                    error.code,
                    incoming_continuation,
                ),
                events,
            );
            print_mandatory_context_overflow_with_warnings(
                command_id,
                &error,
                MandatoryContextOverflowDetails {
                    bundle_id: bundle_id.as_str().to_string(),
                    hard_limit_bytes,
                    used_bytes_before_overflow,
                    offending_node_ids,
                },
                workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn execute_recall_command(
    connection: &rusqlite::Connection,
    workspace_key: &str,
    args: &RecallArgs,
    decoded_cursor: Option<recall::RecallContinuationState>,
    bundle_id: recall::RecallBundleId,
) -> Result<RecallCommandSuccess, RecallCommandError> {
    connection
        .execute_batch("BEGIN DEFERRED TRANSACTION;")
        .map_err(CliError::db_schema)
        .map_err(RecallCommandError::Cli)?;

    if args.full {
        let response =
            build_full_recall_response(connection, bundle_id).map_err(RecallCommandError::Cli)?;
        let payload = u64::try_from(response.nodes.len())
            .map(|node_count| RecallPayload::new(node_count, false, 0, false, false))
            .map_err(|_| CollectorInputError::Serialization);
        let empty = response.nodes.is_empty();
        return Ok(RecallCommandSuccess {
            data: json!(response),
            facts: RecallObservationFacts {
                payload,
                bundle_nodes: Ok(Vec::new()),
                empty,
                truncated: false,
                more_results: false,
            },
        });
    }

    let task_recall = if let Some(query) = args.query.as_deref() {
        let operational_revision = storage::operational_recall_revision(connection)
            .map_err(CliError::db_schema)
            .map_err(RecallCommandError::Cli)?;
        let database_revision =
            recall::bind_recall_revision_to_workspace(workspace_key, &operational_revision)
                .map_err(CliError::invalid_recall_cursor)
                .map_err(RecallCommandError::Cli)?;
        if let Some(state) = decoded_cursor.as_ref() {
            validate_recall_cursor_revision(state, &database_revision)
                .map_err(RecallCommandError::CursorBinding)?;
        }
        Some((query.trim(), database_revision))
    } else {
        None
    };

    let mandatory_nodes = storage::load_active_mandatory_recall_nodes(connection)
        .map_err(CliError::db_schema)
        .map_err(RecallCommandError::Cli)?;
    let mandatory_context = match recall::build_mandatory_recall_context(mandatory_nodes) {
        Ok(context) => context,
        Err(recall::MandatoryContextBuildError::Overflow {
            hard_limit_bytes,
            used_bytes_before_overflow,
            offending_node_ids,
        }) => {
            return Err(RecallCommandError::MandatoryOverflow {
                hard_limit_bytes,
                used_bytes_before_overflow,
                offending_node_ids,
            })
        }
        Err(error) => return Err(RecallCommandError::Cli(CliError::recall_contract(error))),
    };

    if let Some((query, database_revision)) = task_recall {
        let limit = args.limit.unwrap_or(DEFAULT_BOUNDED_RECALL_LIMIT);
        let response = build_task_recall_response(
            connection,
            query,
            limit,
            mandatory_context,
            decoded_cursor,
            database_revision,
            bundle_id,
        )
        .map_err(RecallCommandError::Cli)?;
        let facts = task_recall_observation_facts(&response, args.continuation_cursor.is_some());
        return Ok(RecallCommandSuccess {
            data: json!(response),
            facts,
        });
    }

    let legacy_recall = storage::load_bounded_legacy_recall(connection)
        .map_err(CliError::db_schema)
        .map_err(RecallCommandError::Cli)?;
    let storage::BoundedLegacyRecall {
        nodes,
        links,
        mut more_results,
        mut content_truncated,
    } = legacy_recall;
    let mut selected_node_ids = mandatory_context
        .section
        .nodes
        .iter()
        .map(|selected| selected.node.id)
        .chain(nodes.iter().map(|node| node.id))
        .collect::<std::collections::BTreeSet<_>>();
    let mut task_empty = nodes.is_empty();
    let mut fts_fallback_used = false;

    let mut bundle = recall::build_structured_bundle_with_links(nodes, links);
    if recall::needs_fts_fallback(&bundle) {
        if let Some(query) = recall::derive_fts_fallback_query(&bundle) {
            let search = storage::search_recall_query_fts(connection, &query, 5)
                .map_err(CliError::db_schema)
                .map_err(RecallCommandError::Cli)?;
            fts_fallback_used = true;
            task_empty &= search.results.is_empty();
            selected_node_ids.extend(search.results.iter().map(|result| result.node.id));
            more_results |= search.more_results;
            content_truncated |= search.content_truncated;
            bundle = recall::add_fts_fallback(bundle, search.results);
        }
    }

    selected_node_ids.extend(bundle.linked_nodes.iter().map(|linked| linked.node.id));
    let graph_traversal_used = bundle.linked_nodes.iter().any(|linked| linked.depth > 1);
    let data = recall_data_with_mandatory(
        legacy_recall_data(bundle, more_results, content_truncated),
        mandatory_context,
        bundle_id,
    )
    .map_err(CliError::recall_model)
    .map_err(RecallCommandError::Cli)?;
    let payload = u64::try_from(selected_node_ids.len())
        .map(|node_count| {
            RecallPayload::new(
                node_count,
                more_results,
                0,
                fts_fallback_used,
                graph_traversal_used,
            )
        })
        .map_err(|_| CollectorInputError::Serialization);

    Ok(RecallCommandSuccess {
        data,
        facts: RecallObservationFacts {
            payload,
            bundle_nodes: Ok(Vec::new()),
            empty: task_empty,
            truncated: more_results || content_truncated,
            more_results,
        },
    })
}

fn task_recall_observation_facts(
    response: &recall::RecallResponseV2,
    incoming_continuation: bool,
) -> RecallObservationFacts {
    let selected = response
        .mandatory
        .nodes
        .iter()
        .chain(response.task.nodes.iter())
        .collect::<Vec<_>>();
    let fts_fallback_used = selected.iter().any(|selected| {
        selected
            .selection_reasons
            .iter()
            .any(|reason| matches!(reason, recall::RecallSelectionReason::FtsBm25 { .. }))
    });
    let graph_traversal_used = selected.iter().any(|selected| {
        selected
            .selection_reasons
            .iter()
            .any(|reason| matches!(reason, recall::RecallSelectionReason::GraphTraversal { .. }))
    });
    let payload = build_task_recall_observation_payload(
        &selected,
        response.more_results,
        incoming_continuation,
        fts_fallback_used,
        graph_traversal_used,
    );
    let bundle_nodes = build_task_recall_bundle_nodes(&selected);

    RecallObservationFacts {
        payload,
        bundle_nodes,
        empty: response.task.nodes.is_empty(),
        truncated: response.more_results || response.budget.task.exhausted,
        more_results: response.more_results,
    }
}

fn build_task_recall_bundle_nodes(
    selected: &[&recall::SelectedRecallNode],
) -> Result<Vec<RecallBundleNode>, CollectorInputError> {
    selected
        .iter()
        .map(|selected| {
            let score = selected
                .selection_reasons
                .iter()
                .find_map(|reason| match reason {
                    recall::RecallSelectionReason::FtsBm25 { rank } => Some(*rank),
                    _ => None,
                });
            let reasons = selected
                .selection_reasons
                .iter()
                .map(recall_selection_reason)
                .collect::<Vec<_>>();
            RecallBundleNode::new(
                selected.node.id,
                &selected.node.node_type,
                &selected.node.title,
                selected.node.summary.as_deref(),
                selected.node.source_ref.as_deref(),
                selected.node.trust_level.as_deref(),
                selected.node.confidence,
                score,
                reasons,
            )
        })
        .collect()
}

fn build_task_recall_observation_payload(
    selected: &[&recall::SelectedRecallNode],
    more_results: bool,
    incoming_continuation: bool,
    fts_fallback_used: bool,
    graph_traversal_used: bool,
) -> Result<RecallPayload, CollectorInputError> {
    let node_count =
        u64::try_from(selected.len()).map_err(|_| CollectorInputError::Serialization)?;
    let payload = RecallPayload::new(
        node_count,
        more_results,
        u64::from(incoming_continuation),
        fts_fallback_used,
        graph_traversal_used,
    );
    if selected.len() > 128 {
        return Ok(payload);
    }

    let node_ids = selected
        .iter()
        .map(|selected| selected.node.id)
        .collect::<Vec<_>>();
    let reasons = selected
        .iter()
        .map(|selected| {
            selected
                .selection_reasons
                .first()
                .map(recall_selection_reason)
                .ok_or(CollectorInputError::Serialization)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scores = selected
        .iter()
        .filter_map(|selected| {
            selected
                .selection_reasons
                .iter()
                .find_map(|reason| match reason {
                    recall::RecallSelectionReason::FtsBm25 { rank } => Some(*rank),
                    _ => None,
                })
                .map(|rank| RecallScore::new(selected.node.id, rank))
        })
        .collect::<Result<Vec<_>, _>>()?;

    payload
        .with_selected_node_ids(node_ids)?
        .with_selection_reasons(reasons)?
        .with_scores(scores)
}

fn recall_selection_reason(reason: &recall::RecallSelectionReason) -> SelectionReason {
    match reason {
        recall::RecallSelectionReason::MandatoryContext { .. } => SelectionReason::Mandatory,
        recall::RecallSelectionReason::TypedRoot { .. } => SelectionReason::TypedRoot,
        recall::RecallSelectionReason::FtsBm25 { .. } => SelectionReason::FtsBm25,
        recall::RecallSelectionReason::DirectLink { .. } => SelectionReason::DirectLink,
        recall::RecallSelectionReason::GraphTraversal { .. } => SelectionReason::GraphTraversal,
        recall::RecallSelectionReason::Expansion { expansion_type, .. } => match expansion_type {
            recall::RecallExpansionType::Workflow => SelectionReason::Workflow,
            recall::RecallExpansionType::Tool => SelectionReason::Tool,
            recall::RecallExpansionType::FailureMode => SelectionReason::FailureMode,
        },
    }
}

fn mandatory_overflow_observation_event(
    offending_node_count: usize,
) -> Result<CollectorEvent, CollectorInputError> {
    let offending_node_count =
        u64::try_from(offending_node_count).map_err(|_| CollectorInputError::Serialization)?;
    let payload = CountsPayload::new(vec![CountItem::new(
        "offending_nodes",
        offending_node_count,
    )?])?;
    command_observation_event(
        EventType::RecallMandatoryOverflow,
        EventOutcome::Overflow,
        EventPayload::Counts(payload),
        None,
    )
}

fn validate_recall_cursor_revision(
    state: &recall::RecallContinuationState,
    database_revision: &str,
) -> Result<(), CliError> {
    if state.database_revision == database_revision {
        Ok(())
    } else {
        Err(CliError::stale_recall_cursor())
    }
}

fn insert_continuation_seen_node(
    state: &mut recall::RecallContinuationState,
    seen_node_ids: &mut HashSet<i64>,
    node_id: i64,
) -> Result<(), recall::RecallCursorError> {
    if node_id <= 0 {
        return Err(recall::RecallCursorError::InvalidNodeId);
    }
    if seen_node_ids.insert(node_id) {
        state.seen_node_ids.push(node_id);
    }
    Ok(())
}

fn canonicalize_continuation_seen_nodes(state: &mut recall::RecallContinuationState) {
    state.seen_node_ids.sort_unstable();
}

fn build_task_recall_response(
    connection: &rusqlite::Connection,
    query: &str,
    limit: usize,
    mandatory_context: recall::MandatoryRecallContext,
    continuation: Option<recall::RecallContinuationState>,
    database_revision: String,
    bundle_id: recall::RecallBundleId,
) -> Result<recall::RecallResponseV2, CliError> {
    storage::prepare_task_recall_connection(connection).map_err(CliError::db_schema)?;
    let mut state = match continuation {
        Some(state) => state,
        None => {
            recall::RecallContinuationState::new_with_bundle_id(query, database_revision, bundle_id)
                .map_err(CliError::invalid_recall_cursor)?
        }
    };
    let bundle_id = state.bundle_id().map_err(CliError::invalid_recall_cursor)?;
    let mut root_ids = state
        .roots
        .iter()
        .map(|root| root.node_id)
        .collect::<HashSet<_>>();
    let mut seen_node_ids = state.seen_node_ids.iter().copied().collect::<HashSet<_>>();
    let candidate_selector = recall::TaskRecallCandidateSelector::new(&mandatory_context.section);
    let mut page_nodes = Vec::new();
    let mut retrieval_complete = false;

    loop {
        let remaining_items = limit.saturating_sub(page_nodes.len());
        if remaining_items == 0 {
            break;
        }
        let (candidates, candidate_count, layer_more_results) = load_continuation_candidate_page(
            connection,
            query,
            &mut state,
            &mut root_ids,
            remaining_items,
        )?;
        state.offset = state
            .offset
            .checked_add(candidate_count as u64)
            .ok_or_else(|| {
                CliError::invalid_recall_cursor(recall::RecallCursorError::InvalidShape)
            })?;
        let selected = candidate_selector.select(candidates);
        for selected_node in selected {
            if seen_node_ids.contains(&selected_node.node.id) {
                continue;
            }
            let selected_bytes =
                recall::canonical_json_byte_len(&selected_node).map_err(CliError::recall_model)?;
            let separator_bytes = usize::from(state.emitted_count > 0);
            let increment = selected_bytes.checked_add(separator_bytes).ok_or_else(|| {
                CliError::recall_model(recall::RecallModelError::ByteCountOverflow)
            })?;
            let next_node_bytes = usize::try_from(state.task_node_bytes)
                .ok()
                .and_then(|bytes| bytes.checked_add(increment))
                .ok_or_else(|| {
                    CliError::recall_model(recall::RecallModelError::ByteCountOverflow)
                })?;
            let prospective = recall::empty_task_section_byte_len(false)
                .map_err(CliError::recall_model)?
                .checked_add(next_node_bytes)
                .ok_or_else(|| {
                    CliError::recall_model(recall::RecallModelError::ByteCountOverflow)
                })?;
            if prospective > recall::TASK_RECALL_SOFT_BUDGET_BYTES {
                state.exhausted = true;
                break;
            }
            state.task_node_bytes = next_node_bytes as u64;
            state.emitted_count = state.emitted_count.checked_add(1).ok_or_else(|| {
                CliError::invalid_recall_cursor(recall::RecallCursorError::InvalidShape)
            })?;
            insert_continuation_seen_node(&mut state, &mut seen_node_ids, selected_node.node.id)
                .map_err(CliError::invalid_recall_cursor)?;
            page_nodes.push(selected_node);
        }

        if state.exhausted {
            break;
        }
        if layer_more_results {
            if page_nodes.len() >= limit {
                break;
            }
            continue;
        }
        state.offset = 0;
        state.phase = match state.phase {
            recall::RecallContinuationPhase::TypedRoots => recall::RecallContinuationPhase::Fts,
            recall::RecallContinuationPhase::Fts => recall::RecallContinuationPhase::DirectLinks,
            recall::RecallContinuationPhase::DirectLinks => recall::RecallContinuationPhase::Graph,
            recall::RecallContinuationPhase::Graph => {
                retrieval_complete = true;
                break;
            }
        };
    }

    if !state.exhausted && !retrieval_complete {
        retrieval_complete = !probe_more_relevant_task_memory(
            connection,
            query,
            &candidate_selector,
            &seen_node_ids,
            &mut state,
            &mut root_ids,
        )?;
    }

    canonicalize_continuation_seen_nodes(&mut state);

    let more_results = state.exhausted || !retrieval_complete;
    let task_used_bytes = state
        .task_used_bytes(retrieval_complete)
        .map_err(CliError::recall_model)?;
    let budget = recall::RecallBudgetMetadata::with_task_state(
        mandatory_context.used_bytes,
        task_used_bytes,
        state.exhausted,
    )
    .map_err(CliError::recall_model)?;
    let continuation_cursor = if more_results {
        Some(
            recall::encode_recall_continuation_cursor(&state)
                .map_err(CliError::invalid_recall_cursor)?,
        )
    } else {
        None
    };

    Ok(recall::RecallResponseV2 {
        bundle_id,
        mode: recall::RecallMode::Task,
        mandatory: mandatory_context.section,
        task: recall::RecallSection {
            complete: retrieval_complete,
            nodes: page_nodes,
        },
        more_results,
        continuation_cursor,
        budget,
    })
}

fn continuation_roots(state: &recall::RecallContinuationState) -> Vec<(i64, String)> {
    state
        .roots
        .iter()
        .map(|root| (root.node_id, root.node_type.clone()))
        .collect()
}

fn load_continuation_candidate_page(
    connection: &rusqlite::Connection,
    query: &str,
    state: &mut recall::RecallContinuationState,
    root_ids: &mut HashSet<i64>,
    limit: usize,
) -> Result<(storage::TaskRecallCandidates, usize, bool), CliError> {
    let mut candidates = storage::TaskRecallCandidates::default();
    let (candidate_count, more_results) = match state.phase {
        recall::RecallContinuationPhase::TypedRoots => {
            let page = storage::load_task_typed_roots_page(connection, query, state.offset, limit)
                .map_err(CliError::db_schema)?;
            for node in &page.items {
                state
                    .insert_root_indexed(node, root_ids)
                    .map_err(CliError::invalid_recall_cursor)?;
            }
            let count = page.items.len();
            candidates.typed_roots = page.items;
            (count, page.more_results)
        }
        recall::RecallContinuationPhase::Fts => {
            let page = storage::load_task_fts_page(connection, query, state.offset, limit)
                .map_err(CliError::db_schema)?;
            for result in &page.items {
                state
                    .insert_root_indexed(&result.node, root_ids)
                    .map_err(CliError::invalid_recall_cursor)?;
            }
            let count = page.items.len();
            candidates.fts_results = page.items;
            (count, page.more_results)
        }
        recall::RecallContinuationPhase::DirectLinks => {
            let roots = continuation_roots(state);
            let page = storage::load_task_direct_page(connection, &roots, state.offset, limit)
                .map_err(CliError::db_schema)?;
            let count = page.items.len();
            candidates.direct_nodes = page.items;
            (count, page.more_results)
        }
        recall::RecallContinuationPhase::Graph => {
            let roots = continuation_roots(state);
            let page = storage::load_task_graph_page(connection, &roots, state.offset, limit)
                .map_err(CliError::db_schema)?;
            let count = page.items.len();
            candidates.graph_nodes = page.items;
            (count, page.more_results)
        }
    };
    Ok((candidates, candidate_count, more_results))
}

struct RecallProbeCheckpoint {
    phase: recall::RecallContinuationPhase,
    offset: u64,
    roots_len: usize,
}

impl RecallProbeCheckpoint {
    fn capture(state: &recall::RecallContinuationState) -> Self {
        Self {
            phase: state.phase,
            offset: state.offset,
            roots_len: state.roots.len(),
        }
    }

    fn restore(self, state: &mut recall::RecallContinuationState, root_ids: &mut HashSet<i64>) {
        state.phase = self.phase;
        state.offset = self.offset;
        for root in &state.roots[self.roots_len..] {
            root_ids.remove(&root.node_id);
        }
        state.roots.truncate(self.roots_len);
    }
}

/// Skips candidates already emitted by earlier layers. On success with more
/// data, state points immediately before the next new task node.
fn probe_more_relevant_task_memory(
    connection: &rusqlite::Connection,
    query: &str,
    candidate_selector: &recall::TaskRecallCandidateSelector,
    seen_node_ids: &HashSet<i64>,
    state: &mut recall::RecallContinuationState,
    root_ids: &mut HashSet<i64>,
) -> Result<bool, CliError> {
    loop {
        let checkpoint = RecallProbeCheckpoint::capture(state);
        let (candidates, candidate_count, layer_more_results) =
            load_continuation_candidate_page(connection, query, state, root_ids, 1)?;
        let selected = candidate_selector.select(candidates);
        if let Some(next) = selected
            .iter()
            .find(|selected| !seen_node_ids.contains(&selected.node.id))
        {
            checkpoint.restore(state, root_ids);
            let selected_bytes =
                recall::canonical_json_byte_len(next).map_err(CliError::recall_model)?;
            let separator_bytes = usize::from(state.emitted_count > 0);
            let prospective = state
                .task_used_bytes(false)
                .map_err(CliError::recall_model)?
                .checked_add(separator_bytes)
                .and_then(|bytes| bytes.checked_add(selected_bytes))
                .ok_or_else(|| {
                    CliError::recall_model(recall::RecallModelError::ByteCountOverflow)
                })?;
            if prospective > recall::TASK_RECALL_SOFT_BUDGET_BYTES {
                state.exhausted = true;
            }
            return Ok(true);
        }

        state.offset = state
            .offset
            .checked_add(candidate_count as u64)
            .ok_or_else(|| {
                CliError::invalid_recall_cursor(recall::RecallCursorError::InvalidShape)
            })?;
        if layer_more_results {
            continue;
        }
        state.offset = 0;
        state.phase = match state.phase {
            recall::RecallContinuationPhase::TypedRoots => recall::RecallContinuationPhase::Fts,
            recall::RecallContinuationPhase::Fts => recall::RecallContinuationPhase::DirectLinks,
            recall::RecallContinuationPhase::DirectLinks => recall::RecallContinuationPhase::Graph,
            recall::RecallContinuationPhase::Graph => return Ok(false),
        };
    }
}

fn build_full_recall_response(
    connection: &rusqlite::Connection,
    bundle_id: recall::RecallBundleId,
) -> Result<recall::FullRecallResponse, CliError> {
    Ok(recall::FullRecallResponse {
        bundle_id,
        mode: recall::RecallMode::Full,
        debug_only: true,
        nodes: storage::list_nodes(connection).map_err(CliError::db_schema)?,
        links: storage::list_links(connection).map_err(CliError::db_schema)?,
        aliases: storage::list_aliases(connection, None).map_err(CliError::db_schema)?,
        tags: storage::list_tags(connection, None).map_err(CliError::db_schema)?,
        sources: storage::list_sources(connection, None).map_err(CliError::db_schema)?,
        events: crate::audit::list_events(connection).map_err(CliError::db_schema)?,
        tool_contracts: tools::list_tool_contracts(connection).map_err(CliError::db_schema)?,
        mcp_profiles: storage::list_mcp_profiles(connection).map_err(CliError::db_schema)?,
        more_results: false,
        continuation_cursor: None,
    })
}

fn legacy_recall_data(
    bundle: recall::StructuredRecallBundle,
    more_results: bool,
    content_truncated: bool,
) -> Value {
    let mut data = json!(bundle);
    if let Value::Object(object) = &mut data {
        object.insert("more_results".to_string(), Value::Bool(more_results));
        object.insert(
            "content_truncated".to_string(),
            Value::Bool(content_truncated),
        );
    }
    data
}

fn recall_data_with_mandatory(
    mut task_data: Value,
    mandatory_context: recall::MandatoryRecallContext,
    bundle_id: recall::RecallBundleId,
) -> Result<Value, recall::RecallModelError> {
    let task_used_bytes = recall::canonical_json_byte_len(&task_data)?;
    let budget = recall::RecallBudgetMetadata::new(mandatory_context.used_bytes, task_used_bytes)?;
    let Value::Object(object) = &mut task_data else {
        return Err(recall::RecallModelError::InvalidTaskPayload);
    };
    object.insert("bundle_id".to_string(), json!(bundle_id));
    object.insert(
        "mandatory".to_string(),
        serde_json::to_value(mandatory_context.section)?,
    );
    object.insert("budget".to_string(), serde_json::to_value(budget)?);

    Ok(task_data)
}

fn run_feedback_record(
    command_id: &'static str,
    args: &FeedbackRecordArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let Some(bundle_id) = observation.bundle_id.clone() else {
        return print_error(
            command_id,
            &CliError::feedback_bundle_required(),
            json_output,
        );
    };
    let input =
        match FeedbackRecordInput::new(bundle_id.as_str(), args.outcome, args.reason.as_deref()) {
            Ok(input) => input,
            Err(error) => {
                return print_error(command_id, &CliError::feedback_input(error), json_output)
            }
        };
    let (workspace_key, workspace_paths) = match current_workspace_observability_target() {
        Ok(target) => target,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    observation.attach_workspace(&workspace_paths);

    match observation.record_feedback(input) {
        Ok(receipt) => print_success_with_warnings(
            command_id,
            json!(receipt),
            workspace_key,
            observation.warnings_after(Vec::new()),
            json_output,
            EXIT_SUCCESS,
        ),
        Err(FeedbackWriteError::BundleNotFound) => print_error_with_warnings(
            command_id,
            &CliError::feedback_bundle_not_found(&bundle_id),
            workspace_key,
            Vec::new(),
            json_output,
        ),
        Err(FeedbackWriteError::StoreUnavailable) => print_error_with_warnings(
            command_id,
            &CliError::feedback_store_unavailable(),
            workspace_key,
            Vec::new(),
            json_output,
        ),
    }
}

fn run_adapter_seed(
    command_id: &'static str,
    args: &AdapterSeedArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let target = match resolve_adapter_instruction_file(&args.file, json_output, command_id) {
        Ok(target) => target,
        Err(exit_code) => return exit_code,
    };

    let result = adapter::seed_instruction_file(&target.instruction_file);
    attach_adapter_observation(observation, &target);
    match result {
        Ok(outcome) => {
            observation.record_terminal(command_observation_event(
                EventType::AdapterSeed,
                EventOutcome::Success,
                EventPayload::Empty,
                None,
            ));
            print_success_with_warnings(
                command_id,
                json!({
                    "instruction_file": outcome.instruction_file.display().to_string(),
                    "file_created": outcome.file_created,
                    "block_updated": outcome.block_updated,
                }),
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(adapter::SeedError::Io(error)) => {
            let error = CliError::io(error);
            record_failed_observation(observation, EventType::AdapterSeed, &error);
            print_error_with_warnings(
                command_id,
                &error,
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(adapter::SeedError::DamagedManagedBlock) => {
            let error = CliError::managed_block();
            record_failed_observation(observation, EventType::AdapterSeed, &error);
            print_error_with_warnings(
                command_id,
                &error,
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_adapter_sync(
    command_id: &'static str,
    args: &AdapterTargetArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let target = match resolve_adapter_instruction_file(&args.file, json_output, command_id) {
        Ok(target) => target,
        Err(exit_code) => return exit_code,
    };

    let result = adapter::sync_instruction_file(&target.instruction_file);
    attach_adapter_observation(observation, &target);
    match result {
        Ok(outcome) => {
            observation.record_terminal(command_observation_event(
                EventType::AdapterSync,
                EventOutcome::Success,
                EventPayload::Empty,
                None,
            ));
            print_success_with_warnings(
                command_id,
                json!({
                    "instruction_file": outcome.instruction_file.display().to_string(),
                    "file_created": outcome.file_created,
                    "block_present": outcome.block_present,
                    "block_inserted": outcome.block_inserted,
                    "block_updated": outcome.block_updated,
                }),
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(adapter::SeedError::Io(error)) => {
            let error = CliError::io(error);
            record_failed_observation(observation, EventType::AdapterSync, &error);
            print_error_with_warnings(
                command_id,
                &error,
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(adapter::SeedError::DamagedManagedBlock) => {
            let error = CliError::managed_block_drift();
            record_failed_observation(observation, EventType::AdapterSync, &error);
            print_error_with_warnings(
                command_id,
                &error,
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn run_adapter_status(
    command_id: &'static str,
    args: &AdapterTargetArgs,
    json_output: bool,
    observation: &mut CommandObservation,
) -> ExitCode {
    let target = match resolve_adapter_instruction_file(&args.file, json_output, command_id) {
        Ok(target) => target,
        Err(exit_code) => return exit_code,
    };

    let result = adapter::instruction_file_status(&target.instruction_file);
    attach_adapter_observation(observation, &target);
    match result {
        Ok(outcome) => {
            match outcome.managed_block {
                adapter::ManagedBlockStatus::Missing => {
                    observation.record_terminal(command_observation_event(
                        EventType::AdapterDrift,
                        EventOutcome::Missing,
                        EventPayload::Empty,
                        None,
                    ));
                }
                adapter::ManagedBlockStatus::Drifted => {
                    observation.record_terminal(command_observation_event(
                        EventType::AdapterDrift,
                        EventOutcome::Warning,
                        EventPayload::Empty,
                        None,
                    ));
                }
                adapter::ManagedBlockStatus::InSync => {}
            }
            print_success_with_warnings(
                command_id,
                json!({
                    "instruction_file": outcome.instruction_file.display().to_string(),
                    "file_exists": outcome.file_exists,
                    "managed_block": outcome.managed_block.as_str(),
                }),
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(adapter::SeedError::Io(error)) => {
            let error = CliError::io(error);
            record_failed_observation(observation, EventType::AdapterDrift, &error);
            print_error_with_warnings(
                command_id,
                &error,
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
        Err(adapter::SeedError::DamagedManagedBlock) => {
            let error = CliError::managed_block_drift();
            record_failed_observation(observation, EventType::AdapterDrift, &error);
            print_error_with_warnings(
                command_id,
                &error,
                target.workspace_key,
                observation.warnings_after(Vec::new()),
                json_output,
            )
        }
    }
}

fn attach_adapter_observation(
    observation: &mut CommandObservation,
    target: &AdapterResolvedTarget,
) {
    if let Some(workspace_paths) = &target.observation_workspace {
        observation.attach_workspace(workspace_paths);
    }
}

fn resolve_adapter_instruction_file(
    file: &Option<PathBuf>,
    json_output: bool,
    command_id: &'static str,
) -> Result<AdapterResolvedTarget, ExitCode> {
    let repo_root = match storage::resolve_current_workspace_root() {
        Ok(path) => path,
        Err(error) => return Err(print_error(command_id, &CliError::io(error), json_output)),
    };
    let paths = match storage::resolve_paths() {
        Ok(paths) => paths,
        Err(error) => return Err(print_error(command_id, &CliError::path(error), json_output)),
    };
    let workspace_key = match storage::resolve_workspace_key(&paths, &repo_root) {
        Ok(key) => key,
        Err(error) => {
            return Err(print_error(
                command_id,
                &CliError::workspace_resolve(error),
                json_output,
            ))
        }
    };

    let instruction_file = file
        .clone()
        .unwrap_or_else(|| adapter::default_instruction_file(&repo_root));
    let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
    let observation_workspace = storage::open_workspace_db_read_only(&workspace_paths)
        .ok()
        .map(|connection| {
            drop(connection);
            workspace_paths
        });

    Ok(AdapterResolvedTarget {
        instruction_file,
        workspace_key,
        observation_workspace,
    })
}

fn open_current_workspace() -> Result<(String, rusqlite::Connection), CliError> {
    let (key, _workspace_paths, connection) = open_current_workspace_read_context()?;
    Ok((key, connection))
}

enum WorkspaceReadOpenError {
    Missing(CliError),
    Existing(CliError),
}

impl WorkspaceReadOpenError {
    fn into_cli_error(self) -> CliError {
        match self {
            Self::Missing(error) | Self::Existing(error) => error,
        }
    }
}

fn current_workspace_read_target() -> Result<(String, storage::WorkspacePaths), CliError> {
    let repo_root = storage::resolve_current_workspace_root().map_err(CliError::io)?;
    let paths = storage::resolve_paths().map_err(CliError::path)?;
    let key =
        storage::resolve_workspace_key(&paths, &repo_root).map_err(CliError::workspace_resolve)?;
    let workspace_paths = storage::workspace_paths_for_key(&paths, &key);
    Ok((key, workspace_paths))
}

fn open_workspace_read_connection(
    workspace_paths: &storage::WorkspacePaths,
) -> Result<rusqlite::Connection, WorkspaceReadOpenError> {
    match storage::open_workspace_db_read_only(workspace_paths) {
        Ok(connection) => Ok(connection),
        Err(storage::OpenWorkspaceReadOnlyError::Missing(path)) => {
            let path = path.display().to_string();
            Err(WorkspaceReadOpenError::Missing(
                CliError::workspace_db_missing(&path),
            ))
        }
        Err(storage::OpenWorkspaceReadOnlyError::UnsafePath(error)) => {
            Err(WorkspaceReadOpenError::Existing(CliError::io(error)))
        }
        Err(storage::OpenWorkspaceReadOnlyError::Db(error)) => {
            Err(WorkspaceReadOpenError::Existing(CliError::db_schema(error)))
        }
    }
}

fn open_current_workspace_read_context(
) -> Result<(String, storage::WorkspacePaths, rusqlite::Connection), CliError> {
    let (key, workspace_paths) = current_workspace_read_target()?;
    let connection = open_workspace_read_connection(&workspace_paths)
        .map_err(WorkspaceReadOpenError::into_cli_error)?;

    Ok((key, workspace_paths, connection))
}

fn current_workspace_mutation_target() -> Result<(String, storage::WorkspacePaths), CliError> {
    let repo_root = storage::resolve_current_workspace_root().map_err(CliError::io)?;
    let paths = storage::resolve_paths().map_err(CliError::path)?;
    let key =
        storage::resolve_workspace_key(&paths, &repo_root).map_err(CliError::workspace_resolve)?;

    storage::ensure_global_dirs(&paths).map_err(CliError::io)?;
    let workspace_paths = storage::ensure_workspace_dirs(&paths, &key).map_err(CliError::io)?;
    Ok((key, workspace_paths))
}

fn current_workspace_observability_target() -> Result<(String, storage::WorkspacePaths), CliError> {
    current_workspace_read_target()
}

fn mutation_infrastructure_error<E>(error: mutation::MutationError<E>) -> CliError {
    match error {
        mutation::MutationError::Io(error)
        | mutation::MutationError::FilesystemRollback { source: error } => CliError::io(error),
        mutation::MutationError::Db(error)
        | mutation::MutationError::Rollback { source: error, .. } => CliError::db_schema(error),
        mutation::MutationError::Operation(_) => {
            unreachable!("operation errors must be handled by their command")
        }
    }
}

#[cfg(test)]
fn open_current_workspace_context(
) -> Result<(String, storage::WorkspacePaths, rusqlite::Connection), CliError> {
    let (key, workspace_paths) = current_workspace_mutation_target()?;
    let connection = storage::open_workspace_db(&workspace_paths).map_err(CliError::db_schema)?;
    Ok((key, workspace_paths, connection))
}

fn print_success(
    command_id: &'static str,
    data: Value,
    workspace_key: String,
    json_output: bool,
    exit_code: u8,
) -> ExitCode {
    print_success_with_warnings(
        command_id,
        data,
        workspace_key,
        Vec::new(),
        json_output,
        exit_code,
    )
}

fn print_success_with_warnings(
    command_id: &'static str,
    data: Value,
    workspace_key: String,
    warnings: Vec<OutputWarning>,
    json_output: bool,
    exit_code: u8,
) -> ExitCode {
    if json_output {
        println!(
            "{}",
            success_envelope_with_meta_and_warnings(
                command_id,
                data,
                OutputMeta {
                    version: env!("CARGO_PKG_VERSION"),
                    workspace_key: Some(workspace_key),
                },
                warnings,
            )
        );
    } else {
        println!("{data}");
        print_text_warnings(warnings);
    }

    ExitCode::from(exit_code)
}

fn print_observed_mutation_success<T: Serialize>(
    command_id: &'static str,
    outcome: mutation::MutationOutcome<T>,
    workspace_key: String,
    observation: &mut CommandObservation,
    json_output: bool,
) -> ExitCode {
    record_snapshot_observation(observation, outcome.snapshot_observation);
    print_mutation_success_with_warnings(
        command_id,
        outcome,
        workspace_key,
        observation.warnings_after(Vec::new()),
        json_output,
    )
}

fn print_mutation_success_with_warnings<T: Serialize>(
    command_id: &'static str,
    outcome: mutation::MutationOutcome<T>,
    workspace_key: String,
    trailing_warnings: Vec<OutputWarning>,
    json_output: bool,
) -> ExitCode {
    let mut warnings = outcome.warning.into_iter().collect::<Vec<_>>();
    warnings.extend(trailing_warnings);
    let data =
        serde_json::to_value(outcome.value).expect("mutation result serialization should not fail");
    print_success_with_warnings(
        command_id,
        data,
        workspace_key,
        warnings,
        json_output,
        EXIT_SUCCESS,
    )
}

fn print_error(command_id: &'static str, error: &CliError, json_output: bool) -> ExitCode {
    if json_output {
        println!("{}", error_envelope(command_id, error));
    } else {
        eprintln!("{}: {}", error.code, error.message);
    }

    ExitCode::from(error.exit_code)
}

fn print_error_with_warnings(
    command_id: &'static str,
    error: &CliError,
    workspace_key: String,
    warnings: Vec<mutation::MutationWarning>,
    json_output: bool,
) -> ExitCode {
    if json_output {
        println!(
            "{}",
            error_envelope_with_meta_and_warnings(
                command_id,
                error,
                OutputMeta {
                    version: env!("CARGO_PKG_VERSION"),
                    workspace_key: Some(workspace_key),
                },
                warnings,
            )
        );
    } else {
        eprintln!("{}: {}", error.code, error.message);
        print_text_warnings(warnings);
    }

    ExitCode::from(error.exit_code)
}

fn print_recall_error_with_warnings(
    command_id: &'static str,
    error: &CliError,
    bundle_id: &recall::RecallBundleId,
    workspace_key: String,
    warnings: Vec<OutputWarning>,
    json_output: bool,
) -> ExitCode {
    if json_output {
        println!(
            "{}",
            recall_error_envelope_with_meta_and_warnings(
                command_id,
                error,
                bundle_id,
                OutputMeta {
                    version: env!("CARGO_PKG_VERSION"),
                    workspace_key: Some(workspace_key),
                },
                warnings,
            )
        );
    } else {
        eprintln!("{}: {}", error.code, error.message);
        eprintln!("bundle_id: {}", bundle_id.as_str());
        print_text_warnings(warnings);
    }
    ExitCode::from(error.exit_code)
}

fn recall_error_envelope_with_meta_and_warnings(
    command_id: &'static str,
    error: &CliError,
    bundle_id: &recall::RecallBundleId,
    meta: OutputMeta,
    warnings: Vec<OutputWarning>,
) -> String {
    serialize_envelope(&OutputEnvelope {
        ok: false,
        command: command_id,
        data: None,
        warnings,
        errors: vec![OutputError {
            code: error.code,
            message: error.message.clone(),
            fix_hint: error.fix_hint.clone(),
            details: Some(OutputErrorDetails::RecallFailure(RecallFailureDetails {
                bundle_id: bundle_id.as_str().to_string(),
            })),
        }],
        meta,
    })
}

fn print_text_warnings(warnings: Vec<OutputWarning>) {
    for warning in warnings {
        eprintln!("WARNING {}: {}", warning.code, warning.message);
    }
}

fn print_mandatory_context_overflow_with_warnings(
    command_id: &'static str,
    error: &CliError,
    details: MandatoryContextOverflowDetails,
    workspace_key: String,
    warnings: Vec<OutputWarning>,
    json_output: bool,
) -> ExitCode {
    let text_bundle_id = details.bundle_id.clone();
    if json_output {
        println!(
            "{}",
            mandatory_context_overflow_envelope_with_meta_and_warnings(
                command_id,
                error,
                details,
                OutputMeta {
                    version: env!("CARGO_PKG_VERSION"),
                    workspace_key: Some(workspace_key),
                },
                warnings,
            )
        );
    } else {
        eprintln!("{}: {}", error.code, error.message);
        eprintln!("bundle_id: {text_bundle_id}");
        print_text_warnings(warnings);
    }

    ExitCode::from(error.exit_code)
}

fn print_tool_limit_error_with_warnings(
    command_id: &'static str,
    limit_error: &tools::ToolRunLimitError,
    workspace_key: String,
    warnings: Vec<OutputWarning>,
    json_output: bool,
) -> ExitCode {
    let (error, details) = match tool_limit_error_parts(limit_error) {
        Ok(parts) => parts,
        Err(error) => {
            return print_error_with_warnings(
                command_id,
                &error,
                workspace_key,
                warnings,
                json_output,
            )
        }
    };

    if json_output {
        println!(
            "{}",
            serialize_envelope(&OutputEnvelope {
                ok: false,
                command: command_id,
                data: None,
                warnings,
                errors: vec![OutputError {
                    code: error.code,
                    message: error.message.clone(),
                    fix_hint: error.fix_hint.clone(),
                    details: Some(details),
                }],
                meta: OutputMeta {
                    version: env!("CARGO_PKG_VERSION"),
                    workspace_key: Some(workspace_key),
                },
            })
        );
    } else {
        eprintln!("{}: {}", error.code, error.message);
        print_text_warnings(warnings);
    }
    ExitCode::from(error.exit_code)
}

fn print_artifact_cleanup_error_with_warnings(
    command_id: &'static str,
    artifact_error: &artifacts::ArtifactError,
    workspace_key: String,
    warnings: Vec<OutputWarning>,
    json_output: bool,
) -> ExitCode {
    let error = CliError::artifacts(artifact_error);
    if json_output {
        let details = artifact_error.deleted_paths().map(|deleted_paths| {
            OutputErrorDetails::ArtifactCleanup(ArtifactCleanupErrorDetails {
                report: artifact_error.cleanup_report().cloned(),
                deleted_paths: deleted_paths.to_vec(),
            })
        });
        println!(
            "{}",
            serialize_envelope(&OutputEnvelope {
                ok: false,
                command: command_id,
                data: None,
                warnings,
                errors: vec![OutputError {
                    code: error.code,
                    message: error.message.clone(),
                    fix_hint: error.fix_hint.clone(),
                    details,
                }],
                meta: OutputMeta {
                    version: env!("CARGO_PKG_VERSION"),
                    workspace_key: Some(workspace_key),
                },
            })
        );
    } else {
        eprintln!("{}", artifact_cleanup_error_text(&error, artifact_error));
        print_text_warnings(warnings);
    }
    ExitCode::from(error.exit_code)
}

fn artifact_cleanup_error_text(
    error: &CliError,
    artifact_error: &artifacts::ArtifactError,
) -> String {
    let mut output = format!("{}: {}", error.code, error.message);
    if let Some(report) = artifact_error.cleanup_report() {
        write!(
            output,
            "\nartifact_root: {}\ntoday_dir: {}\nbytes_before: {}\nbytes_after: {}\ncomplete: {}\ndeleted_dirs: {}\ndeleted_files: {}\nkept_dirs: {}",
            report.artifact_root,
            report.today_dir,
            report.bytes_before,
            report.bytes_after,
            report.complete,
            report.deleted_dirs.len(),
            report.deleted_files.len(),
            report.kept_dirs.len(),
        )
        .expect("writing an artifact error to a String cannot fail");
    }
    if let Some(deleted_paths) = artifact_error.deleted_paths() {
        output.push_str("\ndeleted_paths:");
        if deleted_paths.is_empty() {
            output.push_str(" []");
        } else {
            for path in deleted_paths {
                write!(output, "\n- {path}")
                    .expect("writing an artifact path to a String cannot fail");
            }
        }
    }
    output
}

#[cfg(test)]
fn artifact_cleanup_error_envelope(
    command_id: &'static str,
    error: &CliError,
    artifact_error: &artifacts::ArtifactError,
) -> String {
    let details = artifact_error.deleted_paths().map(|deleted_paths| {
        OutputErrorDetails::ArtifactCleanup(ArtifactCleanupErrorDetails {
            report: artifact_error.cleanup_report().cloned(),
            deleted_paths: deleted_paths.to_vec(),
        })
    });
    serialize_envelope(&OutputEnvelope {
        ok: false,
        command: command_id,
        data: None,
        warnings: Vec::new(),
        errors: vec![OutputError {
            code: error.code,
            message: error.message.clone(),
            fix_hint: error.fix_hint.clone(),
            details,
        }],
        meta: OutputMeta::default(),
    })
}

fn tool_limit_error_parts(
    limit_error: &tools::ToolRunLimitError,
) -> Result<(CliError, OutputErrorDetails), CliError> {
    match limit_error {
        tools::ToolRunLimitError::TimedOut {
            timeout_ms,
            stdout_limit_bytes,
            stderr_limit_bytes,
            stdout_truncated,
            stderr_truncated,
        } => Ok((
            CliError::tool_timeout(*timeout_ms),
            OutputErrorDetails::ToolRunLimit(ToolRunLimitDetails {
                timeout_ms: *timeout_ms,
                stdout_limit_bytes: *stdout_limit_bytes,
                stderr_limit_bytes: *stderr_limit_bytes,
                stdout_truncated: *stdout_truncated,
                stderr_truncated: *stderr_truncated,
            }),
        )),
        tools::ToolRunLimitError::OutputOverflow {
            timeout_ms,
            stdout_limit_bytes,
            stderr_limit_bytes,
            stdout_truncated,
            stderr_truncated,
        } => Ok((
            CliError::tool_output_overflow(),
            OutputErrorDetails::ToolRunLimit(ToolRunLimitDetails {
                timeout_ms: *timeout_ms,
                stdout_limit_bytes: *stdout_limit_bytes,
                stderr_limit_bytes: *stderr_limit_bytes,
                stdout_truncated: *stdout_truncated,
                stderr_truncated: *stderr_truncated,
            }),
        )),
        tools::ToolRunLimitError::ArtifactHardOverflow {
            timeout_ms,
            stdout_limit_bytes,
            stderr_limit_bytes,
            hard_limit_bytes,
            stdout_truncated,
            stderr_truncated,
            stdout_hard_limit_exceeded,
            stderr_hard_limit_exceeded,
        } => Ok((
            CliError::tool_artifact_hard_overflow(*hard_limit_bytes),
            OutputErrorDetails::ToolRunHardOverflow(ToolRunHardOverflowDetails {
                timeout_ms: *timeout_ms,
                stdout_limit_bytes: *stdout_limit_bytes,
                stderr_limit_bytes: *stderr_limit_bytes,
                hard_limit_bytes: *hard_limit_bytes,
                stdout_truncated: *stdout_truncated,
                stderr_truncated: *stderr_truncated,
                stdout_hard_limit_exceeded: *stdout_hard_limit_exceeded,
                stderr_hard_limit_exceeded: *stderr_hard_limit_exceeded,
            }),
        )),
        tools::ToolRunLimitError::InvalidLimits { .. } => Err(CliError::tool_run_limits_invalid()),
    }
}

#[cfg(test)]
fn tool_limit_error_envelope(
    command_id: &'static str,
    error: &CliError,
    details: OutputErrorDetails,
) -> String {
    serialize_envelope(&OutputEnvelope {
        ok: false,
        command: command_id,
        data: None,
        warnings: Vec::new(),
        errors: vec![OutputError {
            code: error.code,
            message: error.message.clone(),
            fix_hint: error.fix_hint.clone(),
            details: Some(details),
        }],
        meta: OutputMeta::default(),
    })
}

#[cfg(test)]
fn mandatory_context_overflow_envelope(
    command_id: &'static str,
    error: &CliError,
    hard_limit_bytes: usize,
    used_bytes_before_overflow: usize,
    offending_node_ids: Vec<i64>,
) -> String {
    mandatory_context_overflow_envelope_with_meta_and_warnings(
        command_id,
        error,
        MandatoryContextOverflowDetails {
            bundle_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            hard_limit_bytes,
            used_bytes_before_overflow,
            offending_node_ids,
        },
        OutputMeta::default(),
        Vec::new(),
    )
}

fn mandatory_context_overflow_envelope_with_meta_and_warnings(
    command_id: &'static str,
    error: &CliError,
    details: MandatoryContextOverflowDetails,
    meta: OutputMeta,
    warnings: Vec<OutputWarning>,
) -> String {
    serialize_envelope(&OutputEnvelope {
        ok: false,
        command: command_id,
        data: None,
        warnings,
        errors: vec![OutputError {
            code: error.code,
            message: error.message.clone(),
            fix_hint: error.fix_hint.clone(),
            details: Some(OutputErrorDetails::MandatoryContextOverflow(details)),
        }],
        meta,
    })
}

fn serialize_envelope(envelope: &OutputEnvelope) -> String {
    serde_json::to_string(envelope).expect("CLI envelope serialization should not fail")
}

fn command_id(command: &Command) -> &'static str {
    match command {
        Command::Init => "init",
        Command::Status => "status",
        Command::Doctor => "doctor",
        Command::Verify => "verify",
        Command::Node { command } => match command {
            NodeCommand::Create(_) => "node_create",
            NodeCommand::Get(_) => "node_get",
            NodeCommand::List(_) => "node_list",
            NodeCommand::Update(_) => "node_update",
        },
        Command::Link { command } => match command {
            LinkCommand::Add(_) => "link_add",
            LinkCommand::List(_) => "link_list",
        },
        Command::Alias { command } => match command {
            AliasCommand::Add(_) => "alias_add",
            AliasCommand::List(_) => "alias_list",
        },
        Command::Tag { command } => match command {
            TagCommand::Add(_) => "tag_add",
            TagCommand::List(_) => "tag_list",
        },
        Command::Source { command } => match command {
            SourceCommand::Add(_) => "source_add",
            SourceCommand::List(_) => "source_list",
        },
        Command::Recall(_) => "recall",
        Command::Remember(_) => "remember",
        Command::Teach { command } => match command {
            TeachCommand::Start(_) => "teach_start",
            TeachCommand::Add(_) => "teach_add",
            TeachCommand::Propose(_) => "teach_propose",
            TeachCommand::Apply(_) => "teach_apply",
        },
        Command::Reflect { command } => match command {
            ReflectCommand::Inventory => "reflect_inventory",
            ReflectCommand::Proposal { command } => match command {
                ReflectProposalCommand::Create(_) => "reflect_proposal_create",
                ReflectProposalCommand::Apply(_) => "reflect_proposal_apply",
            },
        },
        Command::Tool { command } => match command {
            ToolCommand::CreateDraft(_) => "tool_create_draft",
            ToolCommand::List(_) => "tool_list",
            ToolCommand::Get(_) => "tool_get",
            ToolCommand::Run(_) => "tool_run",
            ToolCommand::Validate(_) => "tool_validate",
        },
        Command::Mcp { command } => match command {
            McpCommand::List(_) => "mcp_list",
            McpCommand::Add(_) => "mcp_add",
            McpCommand::Get(_) => "mcp_get",
        },
        Command::Adapter { command } => match command {
            AdapterCommand::Seed(_) => "adapter_seed",
            AdapterCommand::Sync(_) => "adapter_sync",
            AdapterCommand::Status(_) => "adapter_status",
        },
        Command::Artifacts { command } => match command {
            ArtifactsCommand::Cleanup => "artifacts_cleanup",
        },
        Command::Feedback { command } => match command {
            FeedbackCommand::Record(_) => "feedback_record",
        },
        Command::Observe { command } => match command {
            ObserveCommand::Status => "observe_status",
            ObserveCommand::Report => "observe_report",
            ObserveCommand::Export(_) => "observe_export",
        },
        Command::Upgrade { command } => match command {
            UpgradeCommand::Plan(_) => "upgrade_plan",
            UpgradeCommand::Apply(_) => "upgrade_apply",
        },
        Command::Ui(_) => "ui",
    }
}

#[derive(Debug)]
struct CliError {
    exit_code: u8,
    code: &'static str,
    message: String,
    fix_hint: String,
}

impl CliError {
    fn invalid_args() -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: "invalid command line arguments".to_string(),
            fix_hint: "run `aopmem --help` to see supported commands".to_string(),
        }
    }

    fn ui(error: ui::UiError) -> Self {
        Self {
            exit_code: EXIT_IO_ERROR,
            code: "UI_SERVER_FAILED",
            message: error.to_string(),
            fix_hint: "check the local workspace and loopback port, then retry `aopmem ui`"
                .to_string(),
        }
    }

    fn upgrade_plan(error: upgrade::UpgradePlanError) -> Self {
        Self {
            exit_code: EXIT_IO_ERROR,
            code: "UPGRADE_PLAN_FAILED",
            message: error.to_string(),
            fix_hint: "preserve all files, fix the reported local path or disk error, then rerun `aopmem upgrade plan --all-workspaces --json`".to_string(),
        }
    }

    fn upgrade_apply(error: upgrade::UpgradeApplyError) -> Self {
        Self {
            exit_code: EXIT_IO_ERROR,
            code: "UPGRADE_APPLY_FAILED",
            message: error.to_string(),
            fix_hint: "preserve all backups and rerun `aopmem upgrade plan --all-workspaces --json` after fixing the reported local error".to_string(),
        }
    }

    fn invalid_recall_query() -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: "recall query must not be blank".to_string(),
            fix_hint: "provide a non-empty task description after `--query`".to_string(),
        }
    }

    fn recall_bundle_mismatch() -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: "--bundle-id does not match the recall continuation cursor".to_string(),
            fix_hint: "pass the exact bundle_id returned by the recall page, or omit --bundle-id"
                .to_string(),
        }
    }

    fn recall_bundle_on_new_recall() -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: "--bundle-id is not accepted when starting a new recall bundle".to_string(),
            fix_hint: "omit --bundle-id; the first recall command creates and returns bundle_id"
                .to_string(),
        }
    }

    fn feedback_bundle_required() -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: "feedback record requires global --bundle-id".to_string(),
            fix_hint: "pass the bundle_id returned by the recall used for this work".to_string(),
        }
    }

    fn feedback_input(error: CollectorInputError) -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: format!("invalid feedback: {error}"),
            fix_hint: "use useful|partial|wrong and an optional non-blank reason up to 1024 bytes"
                .to_string(),
        }
    }

    fn feedback_bundle_not_found(bundle_id: &recall::RecallBundleId) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "NOT_FOUND",
            message: format!(
                "recall bundle not found in this workspace observability store: {}",
                bundle_id.as_str()
            ),
            fix_hint: "use a bundle_id returned by a successful recall in this workspace"
                .to_string(),
        }
    }

    fn feedback_store_unavailable() -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: OBSERVABILITY_WRITE_FAILED,
            message: "local observability feedback write failed".to_string(),
            fix_hint: "check the workspace observability store and retry feedback record"
                .to_string(),
        }
    }

    fn observability_read(error: ObserveReadError) -> Self {
        match error {
            ObserveReadError::UnsafePath => Self {
                exit_code: EXIT_IO_ERROR,
                code: "OBSERVABILITY_UNSAFE_PATH",
                message: "Local Observability path is unsafe".to_string(),
                fix_hint: "remove managed-path links or reparse points, then run doctor"
                    .to_string(),
            },
            ObserveReadError::InvalidStore => Self {
                exit_code: EXIT_DB_SCHEMA_ERROR,
                code: "OBSERVABILITY_INVALID_STORE",
                message: "Local Observability store is invalid or incompatible".to_string(),
                fix_hint: "preserve the store, keep a backup, and run doctor for diagnosis"
                    .to_string(),
            },
            ObserveReadError::ReadFailed => Self {
                exit_code: EXIT_DB_SCHEMA_ERROR,
                code: "OBSERVABILITY_READ_FAILED",
                message: "Local Observability store could not be read".to_string(),
                fix_hint: "check local file access and retry the read-only command".to_string(),
            },
            ObserveReadError::ClockFailed => Self {
                exit_code: EXIT_GENERIC_ERROR,
                code: "OBSERVABILITY_READ_FAILED",
                message: "Local Observability report clock failed".to_string(),
                fix_hint: "retry the read-only report command".to_string(),
            },
        }
    }

    fn debug_capsule(error: observability_export::ExportError) -> Self {
        match error {
            observability_export::ExportError::InvalidOutput => Self {
                exit_code: EXIT_INVALID_ARGS,
                code: "INVALID_ARGS",
                message: "debug capsule output must name a file".to_string(),
                fix_hint: "pass `--output <existing-parent>/<new-file>.zip`".to_string(),
            },
            observability_export::ExportError::OutputExists => Self {
                exit_code: EXIT_IO_ERROR,
                code: "OUTPUT_EXISTS",
                message: "debug capsule output already exists".to_string(),
                fix_hint: "choose a new output path; export never overwrites files".to_string(),
            },
            observability_export::ExportError::UnsafeOutput => Self {
                exit_code: EXIT_IO_ERROR,
                code: "EXPORT_UNSAFE_PATH",
                message: "debug capsule output path is unsafe".to_string(),
                fix_hint: "use a new file under an existing real directory without links"
                    .to_string(),
            },
            observability_export::ExportError::WorkspaceMissing => Self {
                exit_code: EXIT_WORKSPACE_NOT_FOUND,
                code: "WORKSPACE_NOT_FOUND",
                message: "workspace database is required for debug capsule export".to_string(),
                fix_hint: "run `aopmem init` before exporting this workspace".to_string(),
            },
            observability_export::ExportError::WorkspaceUnsafe => Self {
                exit_code: EXIT_IO_ERROR,
                code: "WORKSPACE_UNSAFE_PATH",
                message: "workspace database path is unsafe".to_string(),
                fix_hint: "remove managed-path links or reparse points, then run doctor"
                    .to_string(),
            },
            observability_export::ExportError::WorkspaceInvalid => Self {
                exit_code: EXIT_DB_SCHEMA_ERROR,
                code: "DB_SCHEMA_ERROR",
                message: "workspace database is invalid or incompatible".to_string(),
                fix_hint: "preserve the database, keep a backup, and run doctor".to_string(),
            },
            observability_export::ExportError::Observability(error) => {
                Self::observability_read(error)
            }
            observability_export::ExportError::TemporaryFile
            | observability_export::ExportError::RandomFailed
            | observability_export::ExportError::Serialization
            | observability_export::ExportError::Zip
            | observability_export::ExportError::Sync
            | observability_export::ExportError::Publish => Self {
                exit_code: EXIT_IO_ERROR,
                code: "EXPORT_FAILED",
                message: "debug capsule export failed before publication".to_string(),
                fix_hint: "check free space and output permissions, then retry with a new path"
                    .to_string(),
            },
        }
    }

    fn invalid_recall_cursor(error: recall::RecallCursorError) -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_CURSOR",
            message: format!("invalid recall continuation cursor: {error}"),
            fix_hint: "restart recall with the same `--query` and no continuation cursor"
                .to_string(),
        }
    }

    fn stale_recall_cursor() -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "STALE_RECALL_CURSOR",
            message: "operational memory changed after this recall bundle started".to_string(),
            fix_hint: "restart recall with the same `--query` and no continuation cursor"
                .to_string(),
        }
    }

    fn recall_budget_exhausted() -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "RECALL_BUDGET_EXHAUSTED",
            message: "this recall bundle already exhausted its cumulative task budget".to_string(),
            fix_hint: "stop continuation and use the bounded context already returned".to_string(),
        }
    }

    fn invalid_cursor(reason: &str) -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_CURSOR",
            message: format!("invalid list cursor: {reason}"),
            fix_hint: "use the opaque `next_cursor` returned by the previous page of the same list and filter scope".to_string(),
        }
    }

    fn pagination(reason: &str) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "PAGINATION_ERROR",
            message: reason.to_string(),
            fix_hint: "retry the node list from the first page; run doctor if the error repeats"
                .to_string(),
        }
    }

    fn input_too_large(input: &'static str, max_bytes: usize) -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: format!("{input} exceeds {max_bytes} bytes"),
            fix_hint: format!("reduce {input} to at most {max_bytes} bytes"),
        }
    }

    fn tool_run_args_too_large(message: String) -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message,
            fix_hint: "reduce `aopmem tool run` arguments before retrying".to_string(),
        }
    }

    #[cfg(test)]
    fn not_implemented(command: &'static str) -> Self {
        Self {
            exit_code: EXIT_NOT_IMPLEMENTED,
            code: "NOT_IMPLEMENTED",
            message: format!("command is not implemented yet: {command}"),
            fix_hint: "wait for a later AOPMem stage to implement this command".to_string(),
        }
    }

    fn mandatory_context_overflow(hard_limit_bytes: usize, offending_count: usize) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "MANDATORY_CONTEXT_OVERFLOW",
            message: format!(
                "mandatory recall context exceeds {hard_limit_bytes} canonical JSON UTF-8 bytes; {offending_count} node(s) do not fit"
            ),
            fix_hint: "reduce or supersede mandatory memory nodes before retrying recall"
                .to_string(),
        }
    }

    fn recall_contract(error: recall::MandatoryContextBuildError) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "RECALL_CONTRACT_ERROR",
            message: error.to_string(),
            fix_hint:
                "run doctor and repair invalid mandatory memory records before retrying recall"
                    .to_string(),
        }
    }

    fn recall_model(error: recall::RecallModelError) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "RECALL_CONTRACT_ERROR",
            message: error.to_string(),
            fix_hint: "retry recall; run doctor if deterministic JSON accounting keeps failing"
                .to_string(),
        }
    }

    fn validation(error: storage::NodeValidationError) -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "VALIDATION_ERROR",
            message: error.to_string(),
            fix_hint: "provide allowed node fields from the AOPMem storage spec".to_string(),
        }
    }

    fn link_validation(error: storage::LinkValidationError) -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "VALIDATION_ERROR",
            message: error.to_string(),
            fix_hint: "provide existing source and target node ids and a non-empty link type"
                .to_string(),
        }
    }

    fn metadata_validation(error: storage::MetadataValidationError) -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "VALIDATION_ERROR",
            message: error.to_string(),
            fix_hint: "provide an existing node id and a non-empty metadata value".to_string(),
        }
    }

    fn mcp_profile_validation(error: storage::McpProfileValidationError) -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "VALIDATION_ERROR",
            message: error.to_string(),
            fix_hint: "provide all required MCP profile fields from the MCP registry spec"
                .to_string(),
        }
    }

    fn tool_validation(error: tools::ToolContractValidationError) -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "VALIDATION_ERROR",
            message: error.to_string(),
            fix_hint: "provide tool id/name and a valid draft tool contract shape".to_string(),
        }
    }

    fn tool_not_found(id: &str) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "NOT_FOUND",
            message: format!("tool not found: {id}"),
            fix_hint: "run `aopmem tool create-draft --id <tool-id> --name <name>` first"
                .to_string(),
        }
    }

    fn tool_executable_missing(path: &str) -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "VALIDATION_ERROR",
            message: format!("tool executable path does not exist: {path}"),
            fix_hint: "create the referenced executable file before running validate".to_string(),
        }
    }

    fn unsafe_tool_run_blocked(
        tool_id: &str,
        side_effects: &str,
        approval_requirement: &str,
    ) -> Self {
        Self {
            exit_code: EXIT_UNSAFE_ACTION_BLOCKED,
            code: "UNSAFE_ACTION_BLOCKED",
            message: format!(
                "tool run blocked without approval: tool_id={tool_id}, side_effects={side_effects}, approval_requirement={approval_requirement}"
            ),
            fix_hint: "rerun with `--approved '+++'` for approved external or high-risk tool actions"
                .to_string(),
        }
    }

    fn node_not_found(id: i64) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "NOT_FOUND",
            message: format!("node not found: {id}"),
            fix_hint: "run `aopmem node list --json` to see existing nodes".to_string(),
        }
    }

    fn mcp_profile_not_found(id: &str) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "NOT_FOUND",
            message: format!("mcp profile not found: {id}"),
            fix_hint: "run `aopmem mcp list --json` to see existing MCP profiles".to_string(),
        }
    }

    fn teach(error: storage::TeachStorageError) -> Self {
        match error {
            storage::TeachStorageError::Validation(error) => Self {
                exit_code: EXIT_VALIDATION_FAILED,
                code: "VALIDATION_ERROR",
                message: error.to_string(),
                fix_hint: "provide a valid teach session id and deterministic structured payload"
                    .to_string(),
            },
            storage::TeachStorageError::Node(storage::NodeStorageError::Validation(error)) => {
                Self::validation(error)
            }
            storage::TeachStorageError::Node(storage::NodeStorageError::Db(error))
            | storage::TeachStorageError::Link(storage::LinkStorageError::Db(error))
            | storage::TeachStorageError::Metadata(storage::MetadataStorageError::Db(error))
            | storage::TeachStorageError::Db(error) => Self::db_schema(error),
            storage::TeachStorageError::Link(storage::LinkStorageError::Validation(error)) => {
                Self::link_validation(error)
            }
            storage::TeachStorageError::Metadata(storage::MetadataStorageError::Validation(
                error,
            )) => Self::metadata_validation(error),
            storage::TeachStorageError::Json(error) => Self::teach_payload_json(error),
        }
    }

    fn reflection(error: reflection::ReflectionError) -> Self {
        match error {
            reflection::ReflectionError::EmptySessionId => Self {
                exit_code: EXIT_VALIDATION_FAILED,
                code: "VALIDATION_ERROR",
                message: error.to_string(),
                fix_hint: "store only non-empty reflection session ids in AOPMem reflection records"
                    .to_string(),
            },
            reflection::ReflectionError::ApplyAttemptRequiresTransaction => Self {
                exit_code: EXIT_DB_SCHEMA_ERROR,
                code: "DB_SCHEMA_ERROR",
                message: error.to_string(),
                fix_hint: "run reflection apply through the coordinated workspace mutation path"
                    .to_string(),
            },
            reflection::ReflectionError::InventoryNodeDisappeared(node_id) => Self {
                exit_code: EXIT_DB_SCHEMA_ERROR,
                code: "DB_SCHEMA_ERROR",
                message: format!(
                    "current reflection inventory node disappeared during update: {node_id}"
                ),
                fix_hint: "run doctor and verify before retrying reflection inventory"
                    .to_string(),
            },
            reflection::ReflectionError::Validation(error) => Self {
                exit_code: EXIT_VALIDATION_FAILED,
                code: "VALIDATION_ERROR",
                message: error.to_string(),
                fix_hint: "pass a valid reflection proposal JSON file with supported low/high risk item types"
                    .to_string(),
            },
            reflection::ReflectionError::ProposalNotFound(proposal_id) => Self {
                exit_code: EXIT_GENERIC_ERROR,
                code: "NOT_FOUND",
                message: format!("reflection proposal not found: {proposal_id}"),
                fix_hint: "run `aopmem node list --json` and use a proposal raw_note id with summary `reflection_proposal_v1`".to_string(),
            },
            reflection::ReflectionError::InvalidProposalRecord(proposal_id) => Self {
                exit_code: EXIT_DB_SCHEMA_ERROR,
                code: "DB_SCHEMA_ERROR",
                message: format!("invalid reflection proposal record: {proposal_id}"),
                fix_hint: "repair the stored reflection proposal raw_note body before applying it".to_string(),
            },
            reflection::ReflectionError::DuplicateNodeRef(node_ref) => Self {
                exit_code: EXIT_VALIDATION_FAILED,
                code: "VALIDATION_ERROR",
                message: format!("reflection proposal has duplicate node_ref: {node_ref}"),
                fix_hint: "use each proposal-local node_ref only once inside one reflection proposal".to_string(),
            },
            reflection::ReflectionError::Node(storage::NodeStorageError::Validation(error)) => {
                Self::validation(error)
            }
            reflection::ReflectionError::Link(storage::LinkStorageError::Validation(error)) => {
                Self::link_validation(error)
            }
            reflection::ReflectionError::Metadata(
                storage::MetadataStorageError::Validation(error),
            ) => Self::metadata_validation(error),
            reflection::ReflectionError::Audit(audit::AuditError::Validation(error)) => Self {
                exit_code: EXIT_DB_SCHEMA_ERROR,
                code: "DB_SCHEMA_ERROR",
                message: format!("invalid reflection event: {error}"),
                fix_hint: "run doctor and verify before retrying reflection".to_string(),
            },
            reflection::ReflectionError::Node(storage::NodeStorageError::Db(error))
            | reflection::ReflectionError::Link(storage::LinkStorageError::Db(error))
            | reflection::ReflectionError::Metadata(storage::MetadataStorageError::Db(error))
            | reflection::ReflectionError::Audit(audit::AuditError::Db(error))
            | reflection::ReflectionError::Db(error) => Self::db_schema(error),
            reflection::ReflectionError::Json(error) => Self {
                exit_code: EXIT_DB_SCHEMA_ERROR,
                code: "DB_SCHEMA_ERROR",
                message: format!("invalid stored reflection record json: {error}"),
                fix_hint: "repair malformed reflection raw_note records in the workspace database"
                    .to_string(),
            },
        }
    }

    fn db_schema(error: rusqlite::Error) -> Self {
        Self {
            exit_code: EXIT_DB_SCHEMA_ERROR,
            code: "DB_SCHEMA_ERROR",
            message: error.to_string(),
            fix_hint: "check that the workspace database can be opened and migrated".to_string(),
        }
    }

    fn tool_contract_json(error: serde_json::Error) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "TOOL_JSON_ERROR",
            message: error.to_string(),
            fix_hint: "check the generated tool contract JSON fields".to_string(),
        }
    }

    fn tool_contract_drift(tool_id: &str) -> Self {
        Self {
            exit_code: EXIT_DRIFT_DETECTED,
            code: "DRIFT_DETECTED",
            message: format!(
                "tool contract drift detected between SQLite and tool.json: {tool_id}"
            ),
            fix_hint: "sync the local tool.json export back to the canonical SQLite tool contract"
                .to_string(),
        }
    }

    fn io(error: std::io::Error) -> Self {
        Self {
            exit_code: EXIT_IO_ERROR,
            code: "IO_ERROR",
            message: error.to_string(),
            fix_hint: "check filesystem permissions for AOPMEM_HOME".to_string(),
        }
    }

    fn invalid_utf8_input() -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "INVALID_UTF8_INPUT",
            message: "stdin contains invalid UTF-8 input".to_string(),
            fix_hint: "Set PowerShell UTF-8 encoding before piping answers".to_string(),
        }
    }

    fn suspicious_mojibake_input() -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "SUSPICIOUS_MOJIBAKE_INPUT",
            message: "semantic answer looks like mojibake input".to_string(),
            fix_hint: "Set PowerShell UTF-8 encoding before piping answers".to_string(),
        }
    }

    fn artifacts(error: &artifacts::ArtifactError) -> Self {
        let (exit_code, code, fix_hint) = match error {
            artifacts::ArtifactError::LockTimeout { .. } => (
                EXIT_IO_ERROR,
                "ARTIFACT_LOCK_TIMEOUT",
                "wait for active tool artifact capture to finish, then retry",
            ),
            artifacts::ArtifactError::CleanupPartial { .. } => (
                EXIT_IO_ERROR,
                "ARTIFACT_CLEANUP_PARTIAL",
                "inspect deleted_paths and fix the reported filesystem entry before retrying",
            ),
            artifacts::ArtifactError::CleanupStateUnknown { .. } => (
                EXIT_IO_ERROR,
                "ARTIFACT_CLEANUP_STATE_UNKNOWN",
                "inspect deleted_paths and the artifact tree before retrying cleanup",
            ),
            artifacts::ArtifactError::RetentionLimitNotMet { .. } => (
                EXIT_GENERIC_ERROR,
                "ARTIFACT_RETENTION_NOT_MET",
                "inspect the cleanup report and the workspace artifacts directory",
            ),
            artifacts::ArtifactError::Io(_)
            | artifacts::ArtifactError::Db(_)
            | artifacts::ArtifactError::InvalidDay(_) => (
                EXIT_GENERIC_ERROR,
                "ARTIFACTS_ERROR",
                "check artifact day folders under the workspace artifacts directory",
            ),
        };
        Self {
            exit_code,
            code,
            message: error.to_string(),
            fix_hint: fix_hint.to_string(),
        }
    }

    fn path(error: storage::PathResolveError) -> Self {
        Self {
            exit_code: EXIT_IO_ERROR,
            code: "PATH_ERROR",
            message: error.to_string(),
            fix_hint: "set AOPMEM_HOME, or USERPROFILE on Windows, or HOME elsewhere".to_string(),
        }
    }

    fn workspace_key(error: storage::WorkspaceKeyError) -> Self {
        Self {
            exit_code: EXIT_WORKSPACE_NOT_FOUND,
            code: "WORKSPACE_KEY_ERROR",
            message: error.to_string(),
            fix_hint: "run aopmem from an absolute workspace path".to_string(),
        }
    }

    fn workspace_resolve(error: storage::WorkspaceResolveError) -> Self {
        let fix_hint = match &error {
            storage::WorkspaceResolveError::Ambiguous { .. } => {
                "preserve both workspace roots and run `aopmem upgrade plan --all-workspaces --json` before choosing one"
            }
            _ => "check AOPMEM_HOME permissions and run aopmem from the repository root",
        };
        Self {
            exit_code: EXIT_WORKSPACE_NOT_FOUND,
            code: "WORKSPACE_RESOLVE_ERROR",
            message: error.to_string(),
            fix_hint: fix_hint.to_string(),
        }
    }

    fn workspace_db_missing(path: &str) -> Self {
        Self {
            exit_code: EXIT_WORKSPACE_NOT_FOUND,
            code: "WORKSPACE_NOT_FOUND",
            message: format!("workspace database not found: {path}"),
            fix_hint: "run `aopmem init` first for this repository".to_string(),
        }
    }

    fn managed_block() -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "MANAGED_BLOCK_ERROR",
            message: "managed block markers are damaged or duplicated".to_string(),
            fix_hint: "repair the instruction file so it has one complete managed block or no block at all".to_string(),
        }
    }

    fn managed_block_drift() -> Self {
        Self {
            exit_code: EXIT_DRIFT_DETECTED,
            code: "DRIFT_DETECTED",
            message: "managed block markers are damaged or duplicated".to_string(),
            fix_hint: "repair the instruction file so it has one complete managed block or no block at all".to_string(),
        }
    }

    fn tool_process_failed(exit_code: i32) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "TOOL_PROCESS_FAILED",
            message: format!("tool process exited with non-zero status: {exit_code}"),
            fix_hint: "check the generated tool stderr/stdout and executable behavior".to_string(),
        }
    }

    fn tool_run_limits_invalid() -> Self {
        Self {
            exit_code: EXIT_VALIDATION_FAILED,
            code: "VALIDATION_ERROR",
            message: "tool runtime limits are outside the global hard ceilings".to_string(),
            fix_hint: "validate the persisted tool.json runtime limits before running the tool"
                .to_string(),
        }
    }

    fn tool_timeout(timeout_ms: u128) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "TOOL_TIMEOUT",
            message: format!("tool run timed out after {timeout_ms} ms"),
            fix_hint: "adjust the validated tool timeout or make the tool finish sooner"
                .to_string(),
        }
    }

    fn tool_output_overflow() -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "TOOL_OUTPUT_OVERFLOW",
            message: "tool output exceeded its inline capture limit".to_string(),
            fix_hint: "reduce inline output or set output_mode=artifact in the tool contract"
                .to_string(),
        }
    }

    fn tool_artifact_hard_overflow(hard_limit_bytes: usize) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "TOOL_OUTPUT_OVERFLOW",
            message: format!(
                "tool artifact output exceeded the global {hard_limit_bytes}-byte per-stream capture ceiling"
            ),
            fix_hint:
                "reduce tool output below the global ceiling; no artifact was published for this run"
                    .to_string(),
        }
    }

    fn remember_note_conflict() -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: "remember note text cannot be combined with --body".to_string(),
            fix_hint: "pass note text as the positional argument or use --body, but not both"
                .to_string(),
        }
    }

    fn teach_payload_json(error: serde_json::Error) -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: format!("invalid teach payload json: {error}"),
            fix_hint: "pass deterministic JSON in `--payload`, for example {\"items\":[...]}"
                .to_string(),
        }
    }

    fn reflection_proposal_json(error: serde_json::Error) -> Self {
        Self {
            exit_code: EXIT_INVALID_ARGS,
            code: "INVALID_ARGS",
            message: format!("invalid reflection proposal json: {error}"),
            fix_hint: "pass a JSON file with {\"items\":[...]} and explicit low/high risk values"
                .to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct OutputEnvelope {
    ok: bool,
    command: &'static str,
    data: Option<Value>,
    warnings: Vec<mutation::MutationWarning>,
    errors: Vec<OutputError>,
    meta: OutputMeta,
}

#[derive(Debug, Serialize)]
struct OutputError {
    code: &'static str,
    message: String,
    fix_hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<OutputErrorDetails>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OutputErrorDetails {
    MandatoryContextOverflow(MandatoryContextOverflowDetails),
    RecallFailure(RecallFailureDetails),
    ToolRunLimit(ToolRunLimitDetails),
    ToolRunHardOverflow(ToolRunHardOverflowDetails),
    ArtifactCleanup(ArtifactCleanupErrorDetails),
}

#[derive(Debug, Serialize)]
struct ArtifactCleanupErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<artifacts::CleanupReport>,
    deleted_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MandatoryContextOverflowDetails {
    bundle_id: String,
    hard_limit_bytes: usize,
    used_bytes_before_overflow: usize,
    offending_node_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
struct RecallFailureDetails {
    bundle_id: String,
}

#[derive(Debug, Serialize)]
struct ToolRunLimitDetails {
    timeout_ms: u128,
    stdout_limit_bytes: usize,
    stderr_limit_bytes: usize,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

#[derive(Debug, Serialize)]
struct ToolRunHardOverflowDetails {
    timeout_ms: u128,
    stdout_limit_bytes: usize,
    stderr_limit_bytes: usize,
    hard_limit_bytes: usize,
    stdout_truncated: bool,
    stderr_truncated: bool,
    stdout_hard_limit_exceeded: bool,
    stderr_hard_limit_exceeded: bool,
}

#[derive(Debug, Serialize)]
struct OutputMeta {
    version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_key: Option<String>,
}

impl Default for OutputMeta {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            workspace_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::collections::hash_map::DefaultHasher;
    use std::env;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";
    const HOME_ENV: &str = "HOME";

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        env::temp_dir().join(format!("aopmem-stage-026-cli-{name}-{nanos}"))
    }

    fn file_fingerprint(path: &Path) -> (u64, u64, SystemTime) {
        let bytes = fs::read(path).expect("fingerprinted file should read");
        let metadata = fs::metadata(path).expect("fingerprinted metadata should read");
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        (
            hasher.finish(),
            metadata.len(),
            metadata
                .modified()
                .expect("fingerprinted mtime should read"),
        )
    }

    #[derive(Debug)]
    struct ObservedEventRow {
        event_type: String,
        outcome: String,
        error_code: Option<String>,
        payload_json: String,
        correlation_id: String,
        duration_ms: Option<i64>,
        bundle_id: Option<String>,
    }

    fn observed_command_events(
        workspace_paths: &storage::WorkspacePaths,
        command: &str,
    ) -> Vec<ObservedEventRow> {
        let connection = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let mut statement = connection
            .prepare(
                "SELECT event_type, outcome, error_code, payload_json, correlation_id, \
                        duration_ms, bundle_id \
                 FROM observability_events WHERE command = ?1 ORDER BY rowid",
            )
            .expect("observability event query should prepare");
        statement
            .query_map([command], |row| {
                Ok(ObservedEventRow {
                    event_type: row.get(0)?,
                    outcome: row.get(1)?,
                    error_code: row.get(2)?,
                    payload_json: row.get(3)?,
                    correlation_id: row.get(4)?,
                    duration_ms: row.get(5)?,
                    bundle_id: row.get(6)?,
                })
            })
            .expect("observability events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("observability events should collect")
    }

    fn seed_recall_parent(
        workspace_paths: &storage::WorkspacePaths,
        bundle_id: &recall::RecallBundleId,
    ) {
        let mut collector = LocalCollector::new(workspace_paths, "recall")
            .expect("recall collector should construct");
        let record = RecallBundleRecord::success(bundle_id.as_str(), 3, false, false, Vec::new())
            .expect("recall parent should validate");
        let events = [
            CollectorEvent::new(
                EventType::RecallStarted,
                EventOutcome::Started,
                EventPayload::Empty,
            )
            .expect("recall started event should validate"),
            CollectorEvent::new(
                EventType::RecallCompleted,
                EventOutcome::Success,
                EventPayload::Empty,
            )
            .and_then(|event| event.with_duration_ms(3))
            .expect("recall completed event should validate"),
        ];
        assert!(collector.record_recall_bundle(&record, &events).is_none());
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, script: &str) {
        use std::os::unix::fs::PermissionsExt;

        fs::write(path, script).expect("tool executable should write");
        let mut permissions = fs::metadata(path)
            .expect("tool executable metadata should read")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("tool executable should become executable");
    }

    #[test]
    fn structured_payload_readers_reject_oversized_input_before_json_parsing() {
        let inline = "x".repeat(MAX_STRUCTURED_PAYLOAD_BYTES + 1);
        let inline_error = parse_teach_payload(&inline).expect_err("payload should be bounded");

        assert_eq!(inline_error.code, "INVALID_ARGS");
        assert!(inline_error.message.contains("teach payload exceeds"));

        let file = temp_path("oversized-reflection-proposal.json");
        fs::write(&file, "x".repeat(MAX_STRUCTURED_PAYLOAD_BYTES + 1))
            .expect("oversized proposal fixture should write");
        let file_error =
            parse_reflect_proposal_file(&file).expect_err("proposal file should be bounded");

        assert_eq!(file_error.code, "INVALID_ARGS");
        assert!(file_error
            .message
            .contains("reflection proposal file exceeds"));
        fs::remove_file(file).expect("proposal fixture should be removed");
    }

    #[test]
    fn global_bundle_id_parser_accepts_only_canonical_lowercase_uuid_v4() {
        const CANONICAL: &str = "550e8400-e29b-41d4-a716-446655440000";
        let parsed = Cli::try_parse_from(["aopmem", "--bundle-id", CANONICAL, "status"])
            .expect("canonical UUID v4 should parse");
        assert_eq!(
            parsed
                .bundle_id
                .as_ref()
                .map(recall::RecallBundleId::as_str),
            Some(CANONICAL)
        );

        for invalid in [
            "550E8400-E29B-41D4-A716-446655440000",
            "550e8400-e29b-11d4-a716-446655440000",
            "550e8400e29b41d4a716446655440000",
            "00000000-0000-0000-0000-000000000000",
            "not-a-uuid",
        ] {
            let error = Cli::try_parse_from(["aopmem", "--bundle-id", invalid, "status"])
                .expect_err("non-canonical or non-v4 bundle ids must fail parsing");
            assert!(error.to_string().contains("lowercase hyphenated UUID v4"));
        }

        let after_subcommand = Cli::try_parse_from([
            "aopmem",
            "feedback",
            "record",
            "--bundle-id",
            CANONICAL,
            "--outcome",
            "useful",
        ])
        .expect("global bundle id should parse after the subcommand");
        assert_eq!(
            after_subcommand
                .bundle_id
                .as_ref()
                .map(recall::RecallBundleId::as_str),
            Some(CANONICAL)
        );
    }

    #[test]
    fn new_bare_and_full_recall_reject_global_bundle_before_workspace_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("new-recall-global-bundle-home");
        let repo_root = temp_path("new-recall-global-bundle-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let bundle_id = "550e8400-e29b-41d4-a716-446655440000";
        let cases = [
            vec![
                "aopmem",
                "--bundle-id",
                bundle_id,
                "recall",
                "--query",
                "task",
            ],
            vec!["aopmem", "--bundle-id", bundle_id, "recall"],
            vec!["aopmem", "--bundle-id", bundle_id, "recall", "--full"],
        ];

        for args in cases {
            let cli = Cli::try_parse_from(args).expect("recall case should parse");
            assert_eq!(
                run_parsed(&cli),
                ExitCode::from(EXIT_INVALID_ARGS),
                "new recall must allocate its own bundle id"
            );
        }
        assert!(
            !override_home.exists(),
            "bundle validation must happen before workspace access"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn continuation_requires_exact_global_bundle_match_before_workspace_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("continuation-bundle-match-home");
        let repo_root = temp_path("continuation-bundle-match-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let bundle_id = recall::RecallBundleId::parse("550e8400-e29b-41d4-a716-446655440000")
            .expect("fixture bundle should parse");
        let state = recall::RecallContinuationState::new_with_bundle_id(
            "exact bundle",
            "0".repeat(32),
            bundle_id.clone(),
        )
        .expect("continuation state should build");
        let cursor = recall::encode_recall_continuation_cursor(&state)
            .expect("continuation cursor should encode");
        let mismatch = Cli::try_parse_from([
            "aopmem",
            "--bundle-id",
            "9b2de03e-3bb2-4a82-a772-50dc0b8887a7",
            "recall",
            "--query",
            "exact bundle",
            "--continuation-cursor",
            cursor.as_str(),
        ])
        .expect("mismatch invocation should parse");
        assert_eq!(run_parsed(&mismatch), ExitCode::from(EXIT_INVALID_ARGS));
        assert!(
            !override_home.exists(),
            "mismatch must fail before workspace access"
        );

        let exact = Cli::try_parse_from([
            "aopmem",
            "--bundle-id",
            bundle_id.as_str(),
            "recall",
            "--query",
            "exact bundle",
            "--continuation-cursor",
            cursor.as_str(),
        ])
        .expect("exact invocation should parse");
        assert_eq!(
            run_parsed(&exact),
            ExitCode::from(EXIT_WORKSPACE_NOT_FOUND),
            "matching global id must pass correlation validation and reach core workspace lookup"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn bare_and_full_recall_store_parent_only_bundles() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("parent-only-recall-home");
        let home = temp_path("parent-only-recall-fallback-home");
        let repo_root = temp_path("parent-only-recall-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Parent-only recall canary".to_string(),
                summary: Some("legacy and full data".to_string()),
                body: Some("must not enter bundle_nodes".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("recall canary should create");
        drop(connection);

        for args in [vec!["aopmem", "recall"], vec!["aopmem", "recall", "--full"]] {
            let cli = Cli::try_parse_from(args).expect("recall invocation should parse");
            assert_eq!(run_parsed(&cli), ExitCode::from(EXIT_SUCCESS));
        }

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let counts: (i64, i64) = observability
            .query_row(
                "SELECT (SELECT COUNT(*) FROM recall_bundles), \
                        (SELECT COUNT(*) FROM bundle_nodes)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("parent-only counts should query");
        assert_eq!(counts, (2, 0));
        let missing_bundle_events: i64 = observability
            .query_row(
                "SELECT COUNT(*) FROM observability_events \
                 WHERE event_type LIKE 'recall.%' AND bundle_id IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("recall event bundle coverage should query");
        assert_eq!(missing_bundle_events, 0);

        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn feedback_input_validation_happens_before_workspace_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("feedback-input-home");
        let repo_root = temp_path("feedback-input-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let bundle_id = "550e8400-e29b-41d4-a716-446655440000";

        let missing_bundle =
            Cli::try_parse_from(["aopmem", "feedback", "record", "--outcome", "useful"])
                .expect("feedback without bundle should parse");
        assert_eq!(
            run_parsed(&missing_bundle),
            ExitCode::from(EXIT_INVALID_ARGS)
        );

        for reason in ["   ".to_string(), "x".repeat(1_025)] {
            let invalid_reason = Cli::try_parse_from([
                "aopmem",
                "feedback",
                "record",
                "--bundle-id",
                bundle_id,
                "--outcome",
                "partial",
                "--reason",
                reason.as_str(),
            ])
            .expect("invalid feedback reason should reach typed validation");
            assert_eq!(
                run_parsed(&invalid_reason),
                ExitCode::from(EXIT_INVALID_ARGS)
            );
        }
        assert!(
            !override_home.exists(),
            "feedback input errors must not create AOPMEM_HOME"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn feedback_missing_store_returns_not_found_without_creating_observability() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("feedback-missing-store-home");
        let home = temp_path("feedback-missing-store-fallback-home");
        let repo_root = temp_path("feedback-missing-store-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        drop(connection);
        assert!(!workspace_paths.observability_db().exists());

        let cli = Cli::try_parse_from([
            "aopmem",
            "feedback",
            "record",
            "--bundle-id",
            "550e8400-e29b-41d4-a716-446655440000",
            "--outcome",
            "wrong",
        ])
        .expect("feedback should parse");
        assert_eq!(run_parsed(&cli), ExitCode::from(EXIT_GENERIC_ERROR));
        assert!(
            !workspace_paths.observability_db().exists(),
            "feedback must not create a missing observability store"
        );

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn feedback_is_observability_only_atomic_redacted_and_preserves_memory_snapshot() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("feedback-success-home");
        let home = temp_path("feedback-success-fallback-home");
        let repo_root = temp_path("feedback-success-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let remember = Cli::try_parse_from([
            "aopmem",
            "remember",
            "--title",
            "Feedback operational canary",
            "--body",
            "must remain unchanged",
        ])
        .expect("remember should parse");
        assert_eq!(run_parsed(&remember), ExitCode::from(EXIT_SUCCESS));
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_key = storage::workspace_key(&repo_root).expect("workspace key should build");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
        let bundle_id = recall::RecallBundleId::parse("550e8400-e29b-41d4-a716-446655440000")
            .expect("fixture bundle should parse");
        seed_recall_parent(&workspace_paths, &bundle_id);
        let snapshot = workspace_paths.audit_git().join("memory.sql");
        assert!(
            snapshot.is_file(),
            "remember should publish an audit snapshot"
        );
        let db_before = file_fingerprint(workspace_paths.db());
        let snapshot_before = file_fingerprint(&snapshot);

        let feedback = Cli::try_parse_from([
            "aopmem",
            "feedback",
            "record",
            "--bundle-id",
            bundle_id.as_str(),
            "--outcome",
            "useful",
            "--reason",
            "  Authorization: Bearer feedback-secret-value  ",
        ])
        .expect("feedback should parse");
        assert_eq!(run_parsed(&feedback), ExitCode::from(EXIT_SUCCESS));
        assert_eq!(file_fingerprint(workspace_paths.db()), db_before);
        assert_eq!(file_fingerprint(&snapshot), snapshot_before);

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let (feedback_count, event_count): (i64, i64) = observability
            .query_row(
                "SELECT (SELECT COUNT(*) FROM feedback WHERE bundle_id = ?1), \
                        (SELECT COUNT(*) FROM observability_events \
                         WHERE bundle_id = ?1 AND event_type = 'feedback.recorded')",
                [bundle_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("feedback and event counts should query");
        assert_eq!((feedback_count, event_count), (1, 1));
        let (reason, feedback_timestamp, event_timestamp, command): (
            String,
            String,
            String,
            String,
        ) = observability
            .query_row(
                "SELECT feedback.reason, feedback.timestamp, observability_events.timestamp, \
                        observability_events.command \
                 FROM feedback JOIN observability_events USING(bundle_id) \
                 WHERE feedback.bundle_id = ?1 \
                   AND observability_events.event_type = 'feedback.recorded'",
                [bundle_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("feedback receipt pair should query");
        assert!(reason.contains("[REDACTED]"));
        assert!(!reason.contains("feedback-secret-value"));
        assert_eq!(feedback_timestamp, event_timestamp);
        assert_eq!(command, "feedback_record");

        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn global_bundle_id_inherits_stage23_and_stage24_snapshot_events_and_default_is_null() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("global-bundle-inheritance-home");
        let home = temp_path("global-bundle-inheritance-fallback-home");
        let repo_root = temp_path("global-bundle-inheritance-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let without_bundle = Cli::try_parse_from([
            "aopmem",
            "remember",
            "--title",
            "No bundle",
            "--body",
            "default event correlation",
        ])
        .expect("remember without bundle should parse");
        assert_eq!(run_parsed(&without_bundle), ExitCode::from(EXIT_SUCCESS));
        let bundle_id = "550e8400-e29b-41d4-a716-446655440000";
        let with_bundle = Cli::try_parse_from([
            "aopmem",
            "remember",
            "--bundle-id",
            bundle_id,
            "--title",
            "With bundle",
            "--body",
            "propagated event correlation",
        ])
        .expect("remember with bundle should parse");
        assert_eq!(run_parsed(&with_bundle), ExitCode::from(EXIT_SUCCESS));

        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_key = storage::workspace_key(&repo_root).expect("workspace key should build");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
        let events = observed_command_events(&workspace_paths, "remember");
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec![
                "remember",
                "audit.snapshot.completed",
                "remember",
                "audit.snapshot.completed",
            ]
        );
        assert!(events[..2].iter().all(|event| event.bundle_id.is_none()));
        assert!(events[2..]
            .iter()
            .all(|event| event.bundle_id.as_deref() == Some(bundle_id)));

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn tool_run_argument_limits_reject_large_inputs_before_workspace_access() {
        let too_many = vec!["x".to_string(); MAX_TOOL_RUN_ARGS + 1];
        let too_large = vec!["x".repeat(MAX_TOOL_RUN_ARG_BYTES + 1)];
        let too_large_total = vec!["x".repeat(MAX_TOOL_RUN_ARG_BYTES); 9];

        for args in [&too_many, &too_large, &too_large_total] {
            let error = validate_tool_run_args(args).expect_err("tool args should be bounded");
            assert_eq!(error.code, "INVALID_ARGS");
        }
    }

    #[test]
    fn invalid_node_input_is_rejected_before_workspace_or_database_creation() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("invalid-node-preflight-home");
        let home = temp_path("invalid-node-preflight-fallback-home");
        let repo_root = temp_path("invalid-node-preflight-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "create",
            "--type",
            "not-a-node-type",
            "--title",
            "invalid input",
        ])
        .expect("invalid semantic input should still parse");

        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_VALIDATION_FAILED));
        assert!(
            !override_home.exists(),
            "pure validation must run before AOPMem home or DB creation"
        );
        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn success_envelope_serializes_structured_audit_warning_code() {
        let rendered = success_envelope_with_meta_and_warnings(
            "node_create",
            json!({"id": 7}),
            OutputMeta::default(),
            vec![mutation::MutationWarning {
                code: mutation::AUDIT_SNAPSHOT_PENDING,
                message: "mutation committed; audit snapshot pending".to_string(),
            }],
        );
        let value: Value = serde_json::from_str(&rendered).expect("envelope should parse");

        assert_eq!(
            value["warnings"][0]["code"],
            mutation::AUDIT_SNAPSHOT_PENDING
        );
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn command_observation_reuses_one_collector_and_correlation_id() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("command-observation-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = storage::resolve_paths().expect("AOPMem paths should resolve");
        storage::ensure_global_dirs(&paths).expect("global dirs should create");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "observation-workspace")
            .expect("workspace dirs should create");
        let mut observation = CommandObservation::new("node_create", None);

        observation.attach_workspace(&workspace_paths);
        let first_correlation_id = observation
            .correlation_id()
            .expect("collector should attach")
            .to_string();
        observation.attach_workspace(&workspace_paths);
        assert_eq!(
            observation.correlation_id(),
            Some(first_correlation_id.as_str())
        );

        for event_type in [
            crate::observability::EventType::NodeCreated,
            crate::observability::EventType::NodeUpdated,
        ] {
            observation.record_terminal(CollectorEvent::new(
                event_type,
                crate::observability::EventOutcome::Success,
                crate::observability::EventPayload::Empty,
            ));
        }
        assert!(observation.warnings_after(Vec::new()).is_empty());
        let frozen_duration_ms = observation
            .terminal_duration_ms
            .expect("terminal duration should freeze before collector I/O");
        drop(observation);

        let connection = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let (event_count, correlation_count, correlation_id, duration_count, duration_ms): (
            i64,
            i64,
            String,
            i64,
            i64,
        ) = connection
            .query_row(
                "SELECT COUNT(*), COUNT(DISTINCT correlation_id), MIN(correlation_id), \
                        COUNT(DISTINCT duration_ms), MIN(duration_ms) \
                 FROM observability_events",
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
            .expect("events should query");
        assert_eq!(event_count, 2);
        assert_eq!(correlation_count, 1);
        assert_eq!(correlation_id, first_correlation_id);
        assert_eq!(duration_count, 1);
        assert_eq!(duration_ms, frozen_duration_ms as i64);

        drop(connection);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
    }

    #[test]
    fn command_observation_latches_one_warning_after_core_warning() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("command-observation-warning-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = storage::resolve_paths().expect("AOPMem paths should resolve");
        storage::ensure_global_dirs(&paths).expect("global dirs should create");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, "observation-warning-workspace")
                .expect("workspace dirs should create");
        fs::write(workspace_paths.observability(), b"not a directory")
            .expect("blocked observability path should write");
        let mut observation = CommandObservation::new("node_create", None);
        observation.attach_workspace(&workspace_paths);
        let event = || {
            CollectorEvent::new(
                crate::observability::EventType::NodeCreated,
                crate::observability::EventOutcome::Success,
                crate::observability::EventPayload::Empty,
            )
        };

        observation.record(event());
        observation.record(event());
        let warnings = observation.warnings_after(vec![OutputWarning {
            code: mutation::AUDIT_SNAPSHOT_PENDING,
            message: "mutation committed; audit snapshot pending".to_string(),
        }]);

        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0].code, mutation::AUDIT_SNAPSHOT_PENDING);
        assert_eq!(warnings[1].code, OBSERVABILITY_WRITE_FAILED);

        drop(observation);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
    }

    #[test]
    fn observed_warnings_are_top_level_json_in_stable_order() {
        let warnings = vec![
            OutputWarning {
                code: mutation::AUDIT_SNAPSHOT_PENDING,
                message: "audit warning".to_string(),
            },
            OutputWarning {
                code: OBSERVABILITY_WRITE_FAILED,
                message: "observability warning".to_string(),
            },
        ];
        let rendered = success_envelope_with_meta_and_warnings(
            "node_create",
            json!({"id": 7}),
            OutputMeta::default(),
            warnings,
        );
        let value: Value = serde_json::from_str(&rendered).expect("envelope should parse");

        assert_eq!(
            value["warnings"][0]["code"],
            mutation::AUDIT_SNAPSHOT_PENDING
        );
        assert_eq!(value["warnings"][1]["code"], OBSERVABILITY_WRITE_FAILED);
        assert!(value["data"]["warnings"].is_null());
    }

    #[test]
    fn direct_memory_commands_emit_typed_private_observability_events() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("direct-observability-home");
        let home = temp_path("direct-observability-fallback-home");
        let repo_root = temp_path("direct-observability-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);

        for (title, body) in [
            ("Observed source", "SOURCE_BODY_CANARY"),
            ("Observed target", "TARGET_BODY_CANARY"),
        ] {
            let cli = Cli::try_parse_from([
                "aopmem",
                "--json",
                "node",
                "create",
                "--type",
                "raw_note",
                "--title",
                title,
                "--summary",
                "Authorization: Bearer summary-secret",
                "--body",
                body,
                "--source-ref",
                "token=source-secret",
            ])
            .expect("node create should parse");
            assert_eq!(
                run_command(&cli.command, cli.json),
                ExitCode::from(EXIT_SUCCESS)
            );
        }

        let remember = Cli::try_parse_from([
            "aopmem",
            "--json",
            "remember",
            "--title",
            "Observed memory",
            "--body",
            "REMEMBER_BODY_CANARY",
        ])
        .expect("remember should parse");
        assert_eq!(
            run_command(&remember.command, remember.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let source_id = nodes
            .iter()
            .find(|node| node.title == "Observed source")
            .expect("source node should exist")
            .id;
        let target_id = nodes
            .iter()
            .find(|node| node.title == "Observed target")
            .expect("target node should exist")
            .id;
        drop(connection);

        let link = Cli::try_parse_from([
            "aopmem",
            "--json",
            "link",
            "add",
            "--source-id",
            &source_id.to_string(),
            "--target-id",
            &target_id.to_string(),
            "--type",
            "supports",
        ])
        .expect("link add should parse");
        assert_eq!(
            run_command(&link.command, link.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let regular_update = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "update",
            "--id",
            &target_id.to_string(),
            "--status",
            "draft",
            "--title",
            "Observed target updated",
            "--body",
            "REGULAR_UPDATE_BODY_CANARY",
        ])
        .expect("regular node update should parse");
        assert_eq!(
            run_command(&regular_update.command, regular_update.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let update = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "update",
            "--id",
            &source_id.to_string(),
            "--status",
            "deprecated",
            "--title",
            "Observed source deprecated",
            "--body",
            "UPDATED_BODY_CANARY",
        ])
        .expect("node update should parse");
        assert_eq!(
            run_command(&update.command, update.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let observation_db = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let mut statement = observation_db
            .prepare(
                "SELECT event_type, command, correlation_id, bundle_id, outcome, payload_json \
                 FROM observability_events ORDER BY rowid",
            )
            .expect("event query should prepare");
        let events = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .expect("events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("events should collect");
        let event_types = events
            .iter()
            .map(|event| event.0.as_str())
            .collect::<Vec<_>>();
        let correlation_ids = events
            .iter()
            .map(|event| event.2.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let payloads = events
            .iter()
            .map(|event| event.5.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(
            event_types,
            vec![
                "node.created",
                "audit.snapshot.completed",
                "node.created",
                "audit.snapshot.completed",
                "remember",
                "audit.snapshot.completed",
                "link.created",
                "audit.snapshot.completed",
                "node.updated",
                "audit.snapshot.completed",
                "node.deprecated",
                "audit.snapshot.completed",
            ]
        );
        assert_eq!(correlation_ids.len(), 6);
        assert!(events.iter().all(|event| event.3.is_none()));
        assert!(events.iter().all(|event| event.4 != "failure"));
        assert!(payloads.contains("Observed source"));
        assert!(payloads.contains("[REDACTED]"));
        for forbidden in [
            "SOURCE_BODY_CANARY",
            "TARGET_BODY_CANARY",
            "REMEMBER_BODY_CANARY",
            "UPDATED_BODY_CANARY",
            "REGULAR_UPDATE_BODY_CANARY",
            "summary-secret",
            "source-secret",
            "\"body\"",
        ] {
            assert!(
                !payloads.contains(forbidden),
                "observability payload leaked {forbidden}"
            );
        }

        drop(statement);
        drop(observation_db);
        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn unavailable_collector_preserves_success_and_error_exit_codes() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("unavailable-collector-home");
        let home = temp_path("unavailable-collector-fallback-home");
        let repo_root = temp_path("unavailable-collector-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        drop(connection);
        fs::write(workspace_paths.observability(), b"blocked")
            .expect("blocked observability path should write");

        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "create",
            "--type",
            "raw_note",
            "--title",
            "Core still succeeds",
        ])
        .expect("node create should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let missing = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "update",
            "--id",
            "999999",
            "--status",
            "draft",
            "--title",
            "Missing node",
        ])
        .expect("node update should parse");
        assert_eq!(
            run_command(&missing.command, missing.json),
            ExitCode::from(EXIT_GENERIC_ERROR)
        );

        let connection = storage::open_workspace_db(&workspace_paths).expect("core DB should open");
        assert!(storage::list_nodes(&connection)
            .expect("nodes should list")
            .iter()
            .any(|node| node.title == "Core still succeeds"));
        assert!(!workspace_paths.observability_db().exists());

        drop(connection);
        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[derive(Default)]
    struct FailStyleOutput {
        bytes: Vec<u8>,
    }

    impl Write for FailStyleOutput {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if String::from_utf8_lossy(buffer).contains("Базовый стиль установлен")
            {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "forced style output failure",
                ));
            }
            self.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

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

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn set(path: &Path) -> Self {
            let original = env::current_dir().expect("current dir should resolve");
            env::set_current_dir(path).expect("current dir should be changed for test");
            Self { original }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.original).expect("current dir should be restored");
        }
    }

    fn open_test_workspace_db() -> (String, rusqlite::Connection) {
        let repo_root =
            storage::resolve_current_workspace_root().expect("workspace root should resolve");
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_key =
            storage::workspace_key(&repo_root).expect("workspace key should resolve");
        storage::ensure_global_dirs(&paths).expect("global dirs should initialize for test");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &workspace_key)
            .expect("workspace dirs should initialize for test");
        let connection = storage::open_workspace_db(&workspace_paths)
            .expect("workspace DB should initialize for test");

        (workspace_key, connection)
    }

    fn test_node(id: i64, body: Option<String>) -> storage::Node {
        storage::Node {
            id,
            node_type: "rule".to_string(),
            status: "active".to_string(),
            title: format!("Node {id}"),
            summary: Some(format!("Summary {id}")),
            body,
            source_ref: Some("source=user_instruction".to_string()),
            confidence: Some(0.9),
            trust_level: Some("high".to_string()),
            created_at: "2026-07-14 00:00:00".to_string(),
            updated_at: "2026-07-14 00:00:00".to_string(),
        }
    }

    fn test_node_page(
        nodes: Vec<storage::Node>,
        more_results: bool,
        next_after_id: Option<i64>,
    ) -> storage::NodePage {
        storage::NodePage {
            page: storage::Page {
                items: nodes,
                next_after_id,
                more_results,
            },
            body_omitted: false,
            content_truncated: false,
        }
    }

    fn seed_dirty_verify_workspace(repo_root: &Path) {
        install::init_workspace(repo_root).expect("workspace should initialize");
        let workspace_key =
            storage::workspace_key(repo_root).expect("workspace key should resolve");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        let connection = rusqlite::Connection::open(workspace_paths.db()).expect("db should open");

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
    }

    #[test]
    fn clap_contract_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn tool_list_contract_command_returns_success() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-list-contract-home");
        let home = temp_path("tool-list-contract-fallback-home");
        let repo_root = temp_path("tool-list-contract-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);
        install::init_workspace(&repo_root).expect("workspace should init");
        let cli = Cli::try_parse_from(["aopmem", "tool", "list"]).expect("valid command");

        assert_eq!(
            run_command(&cli.command, cli.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn json_success_envelope_has_contract_shape() {
        let json = success_envelope("status", serde_json::json!({}));
        let envelope: Value = serde_json::from_str(&json).expect("valid json envelope");

        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["command"], "status");
        assert_eq!(envelope["data"], serde_json::json!({}));
        assert_eq!(envelope["warnings"], serde_json::json!([]));
        assert_eq!(envelope["errors"], serde_json::json!([]));
        assert_eq!(envelope["meta"]["version"], "0.2.0-rc2");
    }

    #[test]
    fn json_error_envelope_has_contract_shape() {
        let cli = Cli::try_parse_from([
            "aopmem", "--json", "node", "create", "--type", "raw_note", "--title", "Note",
        ])
        .expect("valid command");
        let error = CliError::not_implemented("node_create");
        let json = error_envelope(command_id(&cli.command), &error);
        let envelope: Value = serde_json::from_str(&json).expect("valid json envelope");

        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["command"], "node_create");
        assert_eq!(envelope["data"], Value::Null);
        assert_eq!(envelope["warnings"], serde_json::json!([]));
        assert_eq!(envelope["errors"][0]["code"], "NOT_IMPLEMENTED");
        assert_eq!(
            envelope["errors"][0]["message"],
            "command is not implemented yet: node_create"
        );
        assert_eq!(envelope["meta"]["version"], "0.2.0-rc2");
    }

    #[test]
    fn tool_timeout_json_uses_exact_code_and_typed_bounded_details() {
        let limit_error = tools::ToolRunLimitError::TimedOut {
            timeout_ms: 12_345,
            stdout_limit_bytes: 111,
            stderr_limit_bytes: 222,
            stdout_truncated: true,
            stderr_truncated: false,
        };
        let (error, details) =
            tool_limit_error_parts(&limit_error).expect("timeout should map to CLI contract");
        let envelope: Value =
            serde_json::from_str(&tool_limit_error_envelope("tool_run", &error, details))
                .expect("tool timeout envelope should parse");

        assert_eq!(error.exit_code, EXIT_GENERIC_ERROR);
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["data"], Value::Null);
        assert_eq!(envelope["errors"][0]["code"], "TOOL_TIMEOUT");
        assert_eq!(envelope["errors"][0]["details"]["timeout_ms"], 12_345);
        assert_eq!(envelope["errors"][0]["details"]["stdout_limit_bytes"], 111);
        assert_eq!(envelope["errors"][0]["details"]["stderr_limit_bytes"], 222);
        assert_eq!(envelope["errors"][0]["details"]["stdout_truncated"], true);
        assert_eq!(envelope["errors"][0]["details"]["stderr_truncated"], false);
        assert!(envelope["errors"][0].get("stdout").is_none());
        assert!(envelope["errors"][0].get("stderr").is_none());
    }

    #[test]
    fn tool_output_overflow_json_uses_exact_code_and_no_raw_output() {
        let limit_error = tools::ToolRunLimitError::OutputOverflow {
            timeout_ms: 30_000,
            stdout_limit_bytes: 65_536,
            stderr_limit_bytes: 4_096,
            stdout_truncated: false,
            stderr_truncated: true,
        };
        let (error, details) =
            tool_limit_error_parts(&limit_error).expect("overflow should map to CLI contract");
        let serialized = tool_limit_error_envelope("tool_run", &error, details);
        let envelope: Value =
            serde_json::from_str(&serialized).expect("tool overflow envelope should parse");

        assert_eq!(error.exit_code, EXIT_GENERIC_ERROR);
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["data"], Value::Null);
        assert_eq!(envelope["errors"][0]["code"], "TOOL_OUTPUT_OVERFLOW");
        assert_eq!(
            envelope["errors"][0]["details"],
            serde_json::json!({
                "timeout_ms": 30_000,
                "stdout_limit_bytes": 65_536,
                "stderr_limit_bytes": 4_096,
                "stdout_truncated": false,
                "stderr_truncated": true
            })
        );
        assert!(!serialized.contains("raw stdout"));
        assert!(!serialized.contains("raw stderr"));
    }

    #[test]
    fn artifact_hard_overflow_json_names_global_ceiling_and_no_publish() {
        let limit_error = tools::ToolRunLimitError::ArtifactHardOverflow {
            timeout_ms: 30_000,
            stdout_limit_bytes: 64,
            stderr_limit_bytes: 128,
            hard_limit_bytes: 10_485_760,
            stdout_truncated: true,
            stderr_truncated: false,
            stdout_hard_limit_exceeded: true,
            stderr_hard_limit_exceeded: false,
        };
        let (error, details) = tool_limit_error_parts(&limit_error)
            .expect("artifact hard overflow should map to CLI contract");
        let serialized = tool_limit_error_envelope("tool_run", &error, details);
        let envelope: Value = serde_json::from_str(&serialized)
            .expect("artifact hard overflow envelope should parse");

        assert_eq!(envelope["errors"][0]["code"], "TOOL_OUTPUT_OVERFLOW");
        assert_eq!(
            envelope["errors"][0]["message"],
            "tool artifact output exceeded the global 10485760-byte per-stream capture ceiling"
        );
        assert_eq!(
            envelope["errors"][0]["fix_hint"],
            "reduce tool output below the global ceiling; no artifact was published for this run"
        );
        assert_eq!(
            envelope["errors"][0]["details"],
            serde_json::json!({
                "timeout_ms": 30_000,
                "stdout_limit_bytes": 64,
                "stderr_limit_bytes": 128,
                "hard_limit_bytes": 10_485_760,
                "stdout_truncated": true,
                "stderr_truncated": false,
                "stdout_hard_limit_exceeded": true,
                "stderr_hard_limit_exceeded": false
            })
        );
        assert!(!serialized.contains("set output_mode=artifact"));
    }

    #[test]
    fn json_parse_error_uses_stable_envelope() {
        let error = Cli::try_parse_from(["aopmem", "--json", "nope"])
            .expect_err("invalid command should fail");
        let error_kind = error.kind();
        let cli_error = CliError::invalid_args();
        let json = error_envelope("parse", &cli_error);
        let envelope: Value = serde_json::from_str(&json).expect("valid json envelope");

        assert!(json_flag_present(["aopmem", "--json", "nope"]));
        assert_eq!(error_kind, ErrorKind::InvalidSubcommand);
        assert_eq!(
            handle_parse_error(error, true),
            ExitCode::from(EXIT_INVALID_ARGS)
        );
        assert_eq!(cli_error.exit_code, EXIT_INVALID_ARGS);
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["command"], "parse");
        assert_eq!(envelope["data"], Value::Null);
        assert_eq!(envelope["warnings"], serde_json::json!([]));
        assert_eq!(envelope["errors"][0]["code"], "INVALID_ARGS");
        assert_eq!(
            envelope["errors"][0]["message"],
            "invalid command line arguments"
        );
        assert_eq!(
            envelope["errors"][0]["fix_hint"],
            "run `aopmem --help` to see supported commands"
        );
        assert_eq!(envelope["meta"]["version"], "0.2.0-rc2");
    }

    #[test]
    fn contract_commands_are_routable() {
        let routable_commands = [
            [
                "aopmem", "node", "update", "--id", "1", "--status", "draft", "--title", "Updated",
            ]
            .as_slice(),
            ["aopmem", "tool", "list"].as_slice(),
            ["aopmem", "tool", "get", "context-export"].as_slice(),
        ];

        for command in routable_commands {
            let cli = Cli::try_parse_from(command).expect("contract command should parse");
            assert!(!command_id(&cli.command).is_empty());
        }
    }

    #[test]
    fn teach_commands_parse_stage_040_args() {
        let start = Cli::try_parse_from([
            "aopmem",
            "teach",
            "start",
            "--title",
            "Release flow",
            "--summary",
            "Deterministic session",
        ])
        .expect("teach start should parse");
        let add = Cli::try_parse_from([
            "aopmem",
            "teach",
            "add",
            "--session-id",
            "11",
            "--payload",
            "{\"kind\":\"note\",\"text\":\"step one\"}",
        ])
        .expect("teach add should parse");
        let propose = Cli::try_parse_from([
            "aopmem",
            "teach",
            "propose",
            "--session-id",
            "11",
            "--payload",
            "{\"items\":[{\"op\":\"create_node\",\"node_ref\":\"lesson_1\",\"node_type\":\"lesson\",\"status\":\"draft\",\"title\":\"Write it down\"}]}",
        ])
        .expect("teach propose should parse");
        let apply = Cli::try_parse_from([
            "aopmem",
            "teach",
            "apply",
            "--session-id",
            "11",
            "--proposal-id",
            "12",
        ])
        .expect("teach apply should parse");

        match start.command {
            Command::Teach {
                command: TeachCommand::Start(args),
            } => {
                assert_eq!(args.title, "Release flow");
                assert_eq!(args.summary.as_deref(), Some("Deterministic session"));
            }
            _ => panic!("expected teach start command"),
        }
        match add.command {
            Command::Teach {
                command: TeachCommand::Add(args),
            } => {
                assert_eq!(args.session_id, 11);
                assert_eq!(args.payload, "{\"kind\":\"note\",\"text\":\"step one\"}");
            }
            _ => panic!("expected teach add command"),
        }
        match propose.command {
            Command::Teach {
                command: TeachCommand::Propose(args),
            } => {
                assert_eq!(args.session_id, 11);
                assert!(args.payload.contains("\"create_node\""));
            }
            _ => panic!("expected teach propose command"),
        }
        match apply.command {
            Command::Teach {
                command: TeachCommand::Apply(args),
            } => {
                assert_eq!(args.session_id, 11);
                assert_eq!(args.proposal_id, 12);
            }
            _ => panic!("expected teach apply command"),
        }
    }

    #[test]
    fn reflect_proposal_commands_parse_stage_042_args() {
        let create = Cli::try_parse_from([
            "aopmem",
            "reflect",
            "proposal",
            "create",
            "--session-id",
            "codex-chat-42",
            "--proposal-file",
            "/tmp/reflection-proposal.json",
        ])
        .expect("reflect proposal create should parse");
        let apply = Cli::try_parse_from([
            "aopmem",
            "reflect",
            "proposal",
            "apply",
            "--proposal-id",
            "15",
        ])
        .expect("reflect proposal apply should parse");

        match create.command {
            Command::Reflect {
                command:
                    ReflectCommand::Proposal {
                        command: ReflectProposalCommand::Create(args),
                    },
            } => {
                assert_eq!(args.session_id, "codex-chat-42");
                assert_eq!(
                    args.proposal_file,
                    PathBuf::from("/tmp/reflection-proposal.json")
                );
            }
            _ => panic!("expected reflect proposal create command"),
        }
        match apply.command {
            Command::Reflect {
                command:
                    ReflectCommand::Proposal {
                        command: ReflectProposalCommand::Apply(args),
                    },
            } => {
                assert_eq!(args.proposal_id, 15);
            }
            _ => panic!("expected reflect proposal apply command"),
        }
    }

    #[test]
    fn remember_command_parses_default_note_and_explicit_fields() {
        let default_note = Cli::try_parse_from(["aopmem", "remember", "Capture this"])
            .expect("remember note should parse");
        let explicit = Cli::try_parse_from([
            "aopmem",
            "remember",
            "--type",
            "workflow",
            "--status",
            "active",
            "--title",
            "Release flow",
            "--summary",
            "Stable release steps",
            "--body",
            "1. Tag 2. Ship",
            "--source-ref",
            "source=user_instruction",
            "--confidence",
            "0.9",
            "--trust-level",
            "high",
        ])
        .expect("remember structured node should parse");

        match default_note.command {
            Command::Remember(args) => {
                assert_eq!(args.note.as_deref(), Some("Capture this"));
                assert_eq!(args.node_type, None);
                assert_eq!(args.title, None);
            }
            _ => panic!("expected remember command"),
        }

        match explicit.command {
            Command::Remember(args) => {
                assert_eq!(args.node_type.as_deref(), Some("workflow"));
                assert_eq!(args.status.as_deref(), Some("active"));
                assert_eq!(args.title.as_deref(), Some("Release flow"));
                assert_eq!(args.summary.as_deref(), Some("Stable release steps"));
                assert_eq!(args.body.as_deref(), Some("1. Tag 2. Ship"));
                assert_eq!(args.source_ref.as_deref(), Some("source=user_instruction"));
                assert_eq!(args.confidence, Some(0.9));
                assert_eq!(args.trust_level.as_deref(), Some("high"));
            }
            _ => panic!("expected remember command"),
        }
    }

    #[test]
    fn node_commands_parse_stage_011_args() {
        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "create",
            "--type",
            "decision",
            "--status",
            "active",
            "--title",
            "Use nodes",
            "--source-ref",
            "source=user_instruction",
            "--confidence",
            "0.9",
            "--trust-level",
            "high",
        ])
        .expect("node create should parse");
        let get = Cli::try_parse_from(["aopmem", "--json", "node", "get", "--id", "1"])
            .expect("node get should parse");
        let list = Cli::try_parse_from(["aopmem", "--json", "node", "list"])
            .expect("node list should parse");

        assert_eq!(command_id(&create.command), "node_create");
        assert_eq!(command_id(&get.command), "node_get");
        assert_eq!(command_id(&list.command), "node_list");
    }

    #[test]
    fn list_commands_accept_keyset_args_and_reject_invalid_limits() {
        let node = Cli::try_parse_from([
            "aopmem",
            "node",
            "list",
            "--limit",
            "2",
            "--after-id",
            "7",
            "--include-body",
        ])
        .expect("node page should parse");
        let alias = Cli::try_parse_from([
            "aopmem",
            "alias",
            "list",
            "--node-id",
            "3",
            "--limit",
            "4",
            "--after-id",
            "11",
        ])
        .expect("metadata page should parse");
        let tool = Cli::try_parse_from([
            "aopmem",
            "tool",
            "list",
            "--limit",
            "5",
            "--after-id",
            "alpha",
        ])
        .expect("tool page should parse");

        match node.command {
            Command::Node {
                command: NodeCommand::List(args),
            } => {
                assert_eq!(args.limit, 2);
                assert_eq!(args.cursor, None);
                assert!(!args.all);
                assert_eq!(args.after_id, Some(7));
                assert!(args.include_body);
            }
            _ => panic!("expected node page command"),
        }
        match alias.command {
            Command::Alias {
                command: AliasCommand::List(args),
            } => {
                assert_eq!(args.node_id, Some(3));
                assert_eq!(args.limit, 4);
                assert_eq!(args.after_id, Some(11));
            }
            _ => panic!("expected alias page command"),
        }
        match tool.command {
            Command::Tool {
                command: ToolCommand::List(args),
            } => {
                assert_eq!(args.limit, 5);
                assert_eq!(args.after_id.as_deref(), Some("alpha"));
            }
            _ => panic!("expected tool page command"),
        }

        assert!(Cli::try_parse_from(["aopmem", "node", "list", "--limit", "0"]).is_err());
        assert!(Cli::try_parse_from(["aopmem", "node", "list", "--limit", "501"]).is_err());
        assert!(Cli::try_parse_from(["aopmem", "link", "list", "--after-id", "0"]).is_err());
    }

    #[test]
    fn node_list_accepts_cursor_and_all_with_strict_conflicts() {
        let default_page = Cli::try_parse_from(["aopmem", "node", "list"])
            .expect("default node page should parse");
        let cursor_page = Cli::try_parse_from([
            "aopmem",
            "node",
            "list",
            "--cursor",
            "v1.node.all.37",
            "--limit",
            "500",
        ])
        .expect("cursor node page should parse");
        let all = Cli::try_parse_from([
            "aopmem",
            "node",
            "list",
            "--all",
            "--limit",
            "2",
            "--include-body",
        ])
        .expect("full node traversal should parse");

        match default_page.command {
            Command::Node {
                command: NodeCommand::List(args),
            } => assert_eq!(args.limit, DEFAULT_LIST_LIMIT),
            _ => panic!("expected default node list"),
        }
        match cursor_page.command {
            Command::Node {
                command: NodeCommand::List(args),
            } => {
                assert_eq!(args.limit, MAX_LIST_LIMIT);
                assert_eq!(args.cursor.as_deref(), Some("v1.node.all.37"));
                assert!(!args.all);
                assert_eq!(args.after_id, None);
            }
            _ => panic!("expected cursor node list"),
        }
        match all.command {
            Command::Node {
                command: NodeCommand::List(args),
            } => {
                assert!(args.all);
                assert_eq!(args.limit, 2);
                assert!(args.include_body);
            }
            _ => panic!("expected full node list"),
        }

        assert!(Cli::try_parse_from([
            "aopmem",
            "node",
            "list",
            "--cursor",
            "v1.node.all.37",
            "--all",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "aopmem",
            "node",
            "list",
            "--cursor",
            "v1.node.all.37",
            "--after-id",
            "7",
        ])
        .is_err());
        assert!(
            Cli::try_parse_from(["aopmem", "node", "list", "--all", "--after-id", "7",]).is_err()
        );

        let help = Cli::try_parse_from(["aopmem", "node", "list", "--help"])
            .expect_err("help should stop parsing")
            .to_string();
        assert!(!help.contains("--after-id"));
    }

    #[test]
    fn node_cursor_is_versioned_canonical_lowercase_hex() {
        assert_eq!(
            encode_node_cursor(12).expect("positive node id should encode"),
            "v1.node.all.3132"
        );

        for id in [1, 7, 12, i64::MAX] {
            let cursor = encode_node_cursor(id).expect("positive node id should encode");
            assert_eq!(decode_node_cursor(&cursor), Ok(id));
        }
    }

    #[test]
    fn node_cursor_rejects_wrong_scope_noncanonical_hex_and_invalid_keys() {
        fn raw_key_cursor(key: &str) -> String {
            let payload = key
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("v1.node.all.{payload}")
        }

        let oversized = format!("v1.node.all.{}", "30".repeat(MAX_CURSOR_BYTES));
        let invalid = [
            "",
            "v2.node.all.31",
            "v1.link.all.31",
            "v1.node.active.31",
            "v1.node.all.",
            "v1.node.all.3",
            "v1.node.all.3A",
            "v1.node.all.zz",
            "v1.node.all.ff",
            "v1.node.all.78",
            "v1.node.all.30",
            "v1.node.all.3031",
            oversized.as_str(),
        ];

        for cursor in invalid {
            assert!(
                decode_node_cursor(cursor).is_err(),
                "cursor should be rejected: {cursor}"
            );
        }
        assert!(decode_node_cursor(&raw_key_cursor("9223372036854775808")).is_err());
    }

    #[test]
    fn list_cursor_binds_kind_scope_and_round_trips_unicode_keys() {
        let tool_key = "инструмент-α";
        let mcp_key = "сервер-東京-🦀";
        let tool_cursor = encode_list_cursor(CursorKind::Tool, "all", tool_key)
            .expect("Unicode tool key should encode");
        let mcp_cursor = encode_list_cursor(CursorKind::Mcp, "all", mcp_key)
            .expect("Unicode MCP key should encode");

        assert_eq!(
            decode_list_cursor(&tool_cursor, CursorKind::Tool, "all").as_deref(),
            Ok(tool_key)
        );
        assert_eq!(
            decode_list_cursor(&mcp_cursor, CursorKind::Mcp, "all").as_deref(),
            Ok(mcp_key)
        );
        assert!(tool_cursor
            .rsplit_once('.')
            .expect("cursor should contain payload")
            .1
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        assert!(decode_list_cursor(&tool_cursor, CursorKind::Mcp, "all").is_err());

        let scoped = encode_numeric_cursor(CursorKind::Alias, "node-7", 12)
            .expect("metadata cursor should encode");
        assert_eq!(
            decode_numeric_cursor(&scoped, CursorKind::Alias, "node-7"),
            Ok(12)
        );
        assert!(decode_numeric_cursor(&scoped, CursorKind::Alias, "node-8").is_err());
        assert!(decode_numeric_cursor(&scoped, CursorKind::Tag, "node-7").is_err());
    }

    #[test]
    fn remaining_list_commands_accept_cursor_and_all_contracts() {
        let link = Cli::try_parse_from([
            "aopmem",
            "link",
            "list",
            "--cursor",
            "v1.link.all.37",
            "--limit",
            "500",
        ])
        .expect("link cursor should parse");
        let alias = Cli::try_parse_from([
            "aopmem",
            "alias",
            "list",
            "--node-id",
            "3",
            "--cursor",
            "v1.alias.node-3.3131",
        ])
        .expect("metadata cursor should parse");
        let tag = Cli::try_parse_from(["aopmem", "tag", "list", "--all", "--limit", "2"])
            .expect("tag full traversal should parse");
        let source = Cli::try_parse_from(["aopmem", "source", "list", "--all", "--node-id", "9"])
            .expect("scoped source traversal should parse");
        let tool_cursor = encode_list_cursor(CursorKind::Tool, "all", "инструмент")
            .expect("tool cursor should encode");
        let tool = Cli::try_parse_from(vec![
            "aopmem".to_string(),
            "tool".to_string(),
            "list".to_string(),
            "--cursor".to_string(),
            tool_cursor.clone(),
        ])
        .expect("Unicode tool cursor should parse");
        let mcp = Cli::try_parse_from(["aopmem", "mcp", "list", "--all"])
            .expect("MCP full traversal should parse");

        match link.command {
            Command::Link {
                command: LinkCommand::List(args),
            } => {
                assert_eq!(args.cursor.as_deref(), Some("v1.link.all.37"));
                assert_eq!(args.limit, MAX_LIST_LIMIT);
                assert!(!args.all);
            }
            _ => panic!("expected link list"),
        }
        match alias.command {
            Command::Alias {
                command: AliasCommand::List(args),
            } => {
                assert_eq!(args.node_id, Some(3));
                assert_eq!(args.cursor.as_deref(), Some("v1.alias.node-3.3131"));
            }
            _ => panic!("expected alias list"),
        }
        match tag.command {
            Command::Tag {
                command: TagCommand::List(args),
            } => assert!(args.all),
            _ => panic!("expected tag list"),
        }
        match source.command {
            Command::Source {
                command: SourceCommand::List(args),
            } => {
                assert!(args.all);
                assert_eq!(args.node_id, Some(9));
            }
            _ => panic!("expected source list"),
        }
        match tool.command {
            Command::Tool {
                command: ToolCommand::List(args),
            } => assert_eq!(args.cursor.as_deref(), Some(tool_cursor.as_str())),
            _ => panic!("expected tool list"),
        }
        match mcp.command {
            Command::Mcp {
                command: McpCommand::List(args),
            } => assert!(args.all),
            _ => panic!("expected MCP list"),
        }

        assert!(Cli::try_parse_from([
            "aopmem",
            "link",
            "list",
            "--cursor",
            "v1.link.all.37",
            "--all",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "aopmem",
            "alias",
            "list",
            "--cursor",
            "v1.alias.all.37",
            "--after-id",
            "7",
        ])
        .is_err());
        assert!(
            Cli::try_parse_from(["aopmem", "tool", "list", "--all", "--after-id", "legacy",])
                .is_err()
        );
    }

    #[test]
    fn cross_kind_and_cross_scope_cursors_fail_before_database_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("cross-list-cursor-home");
        let home = temp_path("cross-list-cursor-fallback-home");
        let repo_root = temp_path("cross-list-cursor-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let wrong_alias_scope = encode_numeric_cursor(CursorKind::Alias, "node-8", 3)
            .expect("wrong-scope cursor should encode");
        let wrong_tool_kind = encode_list_cursor(CursorKind::Mcp, "all", "инструмент")
            .expect("wrong-kind cursor should encode");
        let wrong_mcp_kind = encode_list_cursor(CursorKind::Tool, "all", "сервер")
            .expect("wrong-kind cursor should encode");
        let commands = vec![
            vec![
                "aopmem".to_string(),
                "link".to_string(),
                "list".to_string(),
                "--cursor".to_string(),
                "v1.node.all.31".to_string(),
            ],
            vec![
                "aopmem".to_string(),
                "alias".to_string(),
                "list".to_string(),
                "--node-id".to_string(),
                "7".to_string(),
                "--cursor".to_string(),
                wrong_alias_scope,
            ],
            vec![
                "aopmem".to_string(),
                "tag".to_string(),
                "list".to_string(),
                "--cursor".to_string(),
                "v1.alias.all.31".to_string(),
            ],
            vec![
                "aopmem".to_string(),
                "source".to_string(),
                "list".to_string(),
                "--node-id".to_string(),
                "7".to_string(),
                "--cursor".to_string(),
                "v1.source.all.31".to_string(),
            ],
            vec![
                "aopmem".to_string(),
                "tool".to_string(),
                "list".to_string(),
                "--cursor".to_string(),
                wrong_tool_kind,
            ],
            vec![
                "aopmem".to_string(),
                "mcp".to_string(),
                "list".to_string(),
                "--cursor".to_string(),
                wrong_mcp_kind,
            ],
        ];

        for command in commands {
            let cli = Cli::try_parse_from(command).expect("opaque cursor should parse");
            assert_eq!(
                run_command(&cli.command, true),
                ExitCode::from(EXIT_INVALID_ARGS)
            );
        }
        assert!(
            !override_home.exists(),
            "cursor binding validation must run before database access"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn invalid_node_cursor_is_rejected_before_workspace_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("invalid-node-cursor-home");
        let home = temp_path("invalid-node-cursor-fallback-home");
        let repo_root = temp_path("invalid-node-cursor-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "list",
            "--cursor",
            "v1.node.all.ff",
        ])
        .expect("opaque cursor should parse before semantic validation");

        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_INVALID_ARGS));
        assert!(
            !override_home.exists(),
            "invalid cursor validation must run before workspace access"
        );
        let envelope = error_envelope("node_list", &CliError::invalid_cursor("invalid test"));
        let parsed: Value = serde_json::from_str(&envelope).expect("error envelope should parse");
        assert_eq!(parsed["errors"][0]["code"], "INVALID_CURSOR");

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn node_list_json_omits_body_by_default_and_includes_full_body_on_request() {
        let large_body = "x".repeat(64 * 1024 + 1);
        let mut omitted_page = test_node_page(
            vec![test_node(12, Some(large_body.clone()))],
            true,
            Some(12),
        );
        omitted_page.body_omitted = true;
        let omitted = node_list_data(omitted_page, false).expect("node page should serialize");

        assert_eq!(omitted["more_results"], true);
        assert_eq!(omitted["next_cursor"], "v1.node.all.3132");
        assert!(omitted["nodes"][0].get("body").is_none());
        assert_eq!(omitted["body_omitted"], true);
        assert_eq!(omitted["content_truncated"], false);

        let included = node_list_data(
            test_node_page(vec![test_node(12, Some(large_body.clone()))], false, None),
            true,
        )
        .expect("full node page should serialize");
        assert_eq!(included["more_results"], false);
        assert!(included["next_cursor"].is_null());
        assert_eq!(included["nodes"][0]["body"], large_body);
    }

    #[test]
    fn every_remaining_list_json_declares_incomplete_and_complete_results() {
        for (field, kind, scope) in [
            ("links", CursorKind::Link, "all"),
            ("aliases", CursorKind::Alias, "node-7"),
            ("tags", CursorKind::Tag, "all"),
            ("sources", CursorKind::Source, "node-9"),
        ] {
            let incomplete = numeric_list_data(
                field,
                storage::Page {
                    items: vec![format!("{field}-item")],
                    next_after_id: Some(12),
                    more_results: true,
                },
                kind,
                scope,
            )
            .expect("numeric list page should serialize");
            assert_eq!(incomplete["more_results"], true);
            assert_eq!(
                decode_numeric_cursor(
                    incomplete["next_cursor"]
                        .as_str()
                        .expect("incomplete page should have cursor"),
                    kind,
                    scope,
                ),
                Ok(12)
            );
            assert!(incomplete.get("next_after_id").is_none());
            assert!(incomplete[field].is_array());

            let complete = numeric_list_data(
                field,
                storage::Page {
                    items: Vec::<String>::new(),
                    next_after_id: None,
                    more_results: false,
                },
                kind,
                scope,
            )
            .expect("complete numeric list should serialize");
            assert_eq!(complete["more_results"], false);
            assert!(complete["next_cursor"].is_null());
            assert!(complete.get("next_after_id").is_none());
        }

        for (field, kind, key) in [
            ("tools", CursorKind::Tool, "инструмент-α"),
            ("mcp_profiles", CursorKind::Mcp, "сервер-東京"),
        ] {
            let incomplete = string_list_data(
                field,
                storage::Page {
                    items: vec![key.to_string()],
                    next_after_id: Some(key.to_string()),
                    more_results: true,
                },
                kind,
                "all",
            )
            .expect("string list page should serialize");
            assert_eq!(incomplete["more_results"], true);
            assert_eq!(
                decode_list_cursor(
                    incomplete["next_cursor"]
                        .as_str()
                        .expect("incomplete page should have cursor"),
                    kind,
                    "all",
                )
                .as_deref(),
                Ok(key)
            );
            assert!(incomplete.get("next_after_id").is_none());

            let complete = string_list_data(
                field,
                storage::Page {
                    items: Vec::<String>::new(),
                    next_after_id: None,
                    more_results: false,
                },
                kind,
                "all",
            )
            .expect("complete string list should serialize");
            assert_eq!(complete["more_results"], false);
            assert!(complete["next_cursor"].is_null());
            assert!(complete.get("next_after_id").is_none());
        }
    }

    #[test]
    fn generic_all_traversal_crosses_more_than_two_pages_in_one_read_transaction() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for list transaction test");
        let page = list_all_pages_in_read_transaction(
            &mut connection,
            1,
            |_connection, after_id: Option<&i64>, _limit| match after_id.copied() {
                None => Ok(storage::Page {
                    items: vec![1],
                    next_after_id: Some(1),
                    more_results: true,
                }),
                Some(1) => Ok(storage::Page {
                    items: vec![2],
                    next_after_id: Some(2),
                    more_results: true,
                }),
                Some(2) => Ok(storage::Page {
                    items: vec![3],
                    next_after_id: Some(3),
                    more_results: true,
                }),
                Some(3) => Ok(storage::Page {
                    items: vec![4],
                    next_after_id: None,
                    more_results: false,
                }),
                _ => Err(rusqlite::Error::InvalidQuery),
            },
            |item| *item,
        )
        .expect("four-page traversal should succeed");

        assert_eq!(page.items, vec![1, 2, 3, 4]);
        assert!(!page.more_results);
        assert_eq!(page.next_after_id, None);
        assert!(connection.is_autocommit());
    }

    #[test]
    fn generic_all_traversal_fails_closed_on_duplicate_string_key() {
        let mut call = 0;
        let result = collect_all_pages(
            1,
            |_after_id: Option<&String>, _limit| {
                call += 1;
                if call == 1 {
                    Ok(storage::Page {
                        items: vec!["α".to_string()],
                        next_after_id: Some("α".to_string()),
                        more_results: true,
                    })
                } else {
                    Ok(storage::Page {
                        items: vec!["α".to_string()],
                        next_after_id: None,
                        more_results: false,
                    })
                }
            },
            Clone::clone,
        );

        assert!(matches!(result, Err(ListError::Pagination(_))));
    }

    #[test]
    fn all_node_traversal_reads_every_page_and_returns_complete_result() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for full node traversal");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        for id in 1..=5 {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "rule".to_string(),
                    status: "active".to_string(),
                    title: format!("Rule {id}"),
                    summary: None,
                    body: Some(format!("Body {id}")),
                    source_ref: Some("source=user_instruction".to_string()),
                    confidence: Some(0.9),
                    trust_level: Some("high".to_string()),
                },
            )
            .expect("node should create");
        }

        let page =
            list_all_nodes(&mut connection, 2, false).expect("full node traversal should succeed");

        assert_eq!(
            page.page
                .items
                .iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        assert!(page.page.items.iter().all(|node| node.body.is_none()));
        assert!(page.body_omitted);
        assert!(!page.page.more_results);
        assert_eq!(page.page.next_after_id, None);
        assert!(connection.is_autocommit());
    }

    #[test]
    fn all_node_traversal_works_on_the_production_read_only_connection_mode() {
        let db_path = temp_path("node-all-read-only.sqlite");
        let mut writable = rusqlite::Connection::open(&db_path)
            .expect("file DB should open for read-only traversal test");
        crate::schema::apply_migrations(&mut writable).expect("migrations should apply");
        storage::create_node(
            &writable,
            &storage::NewNode {
                node_type: "rule".to_string(),
                status: "active".to_string(),
                title: "Read-only traversal".to_string(),
                summary: None,
                body: Some("complete body".to_string()),
                source_ref: Some("source=user_instruction".to_string()),
                confidence: Some(0.9),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("node should create");
        drop(writable);

        let mut read_only = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("read-only DB should open");
        let page = list_all_nodes(&mut read_only, 1, true)
            .expect("read transaction should work on a read-only connection");

        assert_eq!(page.page.items.len(), 1);
        assert_eq!(page.page.items[0].body.as_deref(), Some("complete body"));
        assert!(!page.page.more_results);
        assert!(read_only.is_autocommit());
        drop(read_only);
        fs::remove_file(db_path).expect("read-only traversal DB should remove");
    }

    #[test]
    fn all_node_traversal_rejects_duplicates_and_nonprogress_without_a_result() {
        let mut duplicate_call = 0;
        let duplicate = collect_all_node_pages(1, |_after_id, _limit| {
            duplicate_call += 1;
            match duplicate_call {
                1 => Ok(test_node_page(vec![test_node(1, None)], true, Some(1))),
                _ => Ok(test_node_page(vec![test_node(1, None)], false, None)),
            }
        });
        assert!(matches!(duplicate, Err(ListError::Pagination(_))));

        let nonprogress = collect_all_node_pages(1, |_after_id, _limit| {
            Ok(test_node_page(Vec::new(), true, Some(1)))
        });
        assert!(matches!(nonprogress, Err(ListError::Pagination(_))));

        let envelope = error_envelope(
            "node_list",
            &CliError::pagination("duplicate or non-progressing node page"),
        );
        let parsed: Value = serde_json::from_str(&envelope).expect("error envelope should parse");
        assert_eq!(parsed["errors"][0]["code"], "PAGINATION_ERROR");
    }

    #[test]
    fn link_commands_parse_stage_012_args() {
        let add = Cli::try_parse_from([
            "aopmem",
            "--json",
            "link",
            "add",
            "--source-id",
            "1",
            "--target-id",
            "2",
            "--type",
            "supports",
        ])
        .expect("link add should parse");
        let list = Cli::try_parse_from(["aopmem", "--json", "link", "list"])
            .expect("link list should parse");

        assert_eq!(command_id(&add.command), "link_add");
        assert_eq!(command_id(&list.command), "link_list");
    }

    #[test]
    fn metadata_commands_parse_stage_013_args() {
        let alias_add = Cli::try_parse_from([
            "aopmem",
            "--json",
            "alias",
            "add",
            "--node-id",
            "1",
            "--alias",
            "Name",
        ])
        .expect("alias add should parse");
        let alias_list =
            Cli::try_parse_from(["aopmem", "--json", "alias", "list", "--node-id", "1"])
                .expect("alias list should parse");
        let tag_add = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tag",
            "add",
            "--node-id",
            "1",
            "--tag",
            "storage",
        ])
        .expect("tag add should parse");
        let tag_list = Cli::try_parse_from(["aopmem", "--json", "tag", "list"])
            .expect("tag list should parse");
        let source_add = Cli::try_parse_from([
            "aopmem",
            "--json",
            "source",
            "add",
            "--node-id",
            "1",
            "--source-ref",
            "source=user_instruction",
        ])
        .expect("source add should parse");
        let source_list =
            Cli::try_parse_from(["aopmem", "--json", "source", "list", "--node-id", "1"])
                .expect("source list should parse");

        assert_eq!(command_id(&alias_add.command), "alias_add");
        assert_eq!(command_id(&alias_list.command), "alias_list");
        assert_eq!(command_id(&tag_add.command), "tag_add");
        assert_eq!(command_id(&tag_list.command), "tag_list");
        assert_eq!(command_id(&source_add.command), "source_add");
        assert_eq!(command_id(&source_list.command), "source_list");
    }

    #[test]
    fn mcp_commands_parse_stage_015_args() {
        let list = Cli::try_parse_from(["aopmem", "--json", "mcp", "list"])
            .expect("mcp list should parse");
        let add = Cli::try_parse_from([
            "aopmem",
            "--json",
            "mcp",
            "add",
            "--id",
            "codebase-memory",
            "--name",
            "Codebase Memory",
            "--kind",
            "optional",
            "--status",
            "missing",
            "--read-operations",
            "search_graph",
            "--write-operations",
            "index_repository",
            "--side-effects",
            "local_read",
            "--approval-requirement",
            "none",
            "--credentials-source",
            "none",
            "--notes",
            "best-effort profile",
        ])
        .expect("mcp add should parse");
        let get =
            Cli::try_parse_from(["aopmem", "--json", "mcp", "get", "--id", "codebase-memory"])
                .expect("mcp get should parse");

        assert_eq!(command_id(&list.command), "mcp_list");
        assert_eq!(command_id(&add.command), "mcp_add");
        assert_eq!(command_id(&get.command), "mcp_get");
    }

    #[test]
    fn tool_create_draft_parses_stage_033_args() {
        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "context-export",
            "--name",
            "Context Export",
            "--entrypoint",
            "bin/context-export",
            "--owner-workflow",
            "memory_keeper",
            "--side-effects",
            "local_write_artifact",
            "--approval-requirement",
            "manual_review",
            "--timeout-ms",
            "123456",
            "--stdout-limit-bytes",
            "234567",
            "--stderr-limit-bytes",
            "345678",
            "--supports-dry-run",
            "--output-mode",
            "artifact",
        ])
        .expect("tool create-draft should parse");

        assert_eq!(command_id(&create.command), "tool_create_draft");
        match create.command {
            Command::Tool {
                command: ToolCommand::CreateDraft(args),
            } => {
                assert_eq!(args.timeout_ms, 123_456);
                assert_eq!(args.stdout_limit_bytes, 234_567);
                assert_eq!(args.stderr_limit_bytes, 345_678);
                assert!(args.supports_dry_run);
                assert_eq!(args.output_mode, tools::ToolOutputMode::Artifact);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn tool_create_draft_rejects_invalid_runtime_before_workspace_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("invalid-tool-runtime-home");
        let home = temp_path("invalid-tool-runtime-fallback-home");
        let repo_root = temp_path("invalid-tool-runtime-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let invalid_limits = [
            ("--timeout-ms", 0_u64),
            ("--timeout-ms", tools::MAX_TOOL_CONTRACT_TIMEOUT_MS + 1),
            ("--stdout-limit-bytes", 0),
            (
                "--stdout-limit-bytes",
                tools::MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES + 1,
            ),
            ("--stderr-limit-bytes", 0),
            (
                "--stderr-limit-bytes",
                tools::MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES + 1,
            ),
        ];
        for (flag, value) in invalid_limits {
            let cli = Cli::try_parse_from(vec![
                "aopmem".to_string(),
                "--json".to_string(),
                "tool".to_string(),
                "create-draft".to_string(),
                "--id".to_string(),
                "invalid-runtime".to_string(),
                "--name".to_string(),
                "Invalid Runtime".to_string(),
                flag.to_string(),
                value.to_string(),
            ])
            .expect("numeric runtime input should parse before semantic validation");

            assert_eq!(
                run_command(&cli.command, cli.json),
                ExitCode::from(EXIT_VALIDATION_FAILED)
            );
            assert!(
                !override_home.exists(),
                "invalid {flag} must not create AOPMem home or workspace DB"
            );
        }

        let unknown_mode = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "unknown-output-mode",
            "--name",
            "Unknown Output Mode",
            "--output-mode",
            "stream",
        ]);
        assert!(unknown_mode.is_err(), "unknown output mode must not parse");
        assert!(
            !override_home.exists(),
            "parse failure must not create AOPMem home or workspace DB"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn tool_validate_parses_stage_034_args() {
        let validate =
            Cli::try_parse_from(["aopmem", "--json", "tool", "validate", "context-export"])
                .expect("tool validate should parse");

        assert_eq!(command_id(&validate.command), "tool_validate");
    }

    #[test]
    fn tool_run_parses_stage_035_args() {
        let run = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "run",
            "context-export",
            "--",
            "--arg",
            "value",
        ])
        .expect("tool run should parse");

        assert_eq!(command_id(&run.command), "tool_run");
        match run.command {
            Command::Tool {
                command: ToolCommand::Run(args),
            } => {
                assert_eq!(args.tool_id, "context-export");
                assert_eq!(args.args, vec!["--arg".to_string(), "value".to_string()]);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn artifacts_cleanup_parses_stage_036_args() {
        let cleanup = Cli::try_parse_from(["aopmem", "--json", "artifacts", "cleanup"])
            .expect("artifacts cleanup should parse");

        assert_eq!(command_id(&cleanup.command), "artifacts_cleanup");
    }

    #[test]
    fn artifact_cleanup_failures_use_stable_codes_and_deleted_path_details() {
        let report = artifacts::CleanupReport {
            artifact_root: "/workspace/artifacts".to_string(),
            today_dir: "/workspace/artifacts/2026-07-15".to_string(),
            bytes_before: 10,
            bytes_after: 5,
            deleted_dirs: Vec::new(),
            deleted_files: vec!["/workspace/artifacts/2026-07-15/old.bin".to_string()],
            kept_dirs: vec!["/workspace/artifacts/2026-07-15".to_string()],
            deleted_paths: vec![
                "/workspace/artifacts/2026-07-15/old.bin".to_string(),
                "/workspace/artifacts/2026-07-15/older.bin".to_string(),
            ],
            complete: false,
        };
        let artifact_error = artifacts::ArtifactError::CleanupPartial {
            failed_path: "/workspace/artifacts/2026-07-15/next.bin".to_string(),
            report: Box::new(report),
            source: io::Error::other("forced cleanup failure"),
        };
        let cli_error = CliError::artifacts(&artifact_error);
        let envelope: Value = serde_json::from_str(&artifact_cleanup_error_envelope(
            "artifacts_cleanup",
            &cli_error,
            &artifact_error,
        ))
        .expect("artifact cleanup error envelope should parse");

        assert_eq!(cli_error.code, "ARTIFACT_CLEANUP_PARTIAL");
        assert_eq!(
            envelope["errors"][0]["details"]["deleted_paths"],
            json!([
                "/workspace/artifacts/2026-07-15/old.bin",
                "/workspace/artifacts/2026-07-15/older.bin"
            ])
        );
        assert_eq!(
            envelope["errors"][0]["details"]["report"]["complete"],
            false
        );
        let text = artifact_cleanup_error_text(&cli_error, &artifact_error);
        assert!(text.contains("artifact_root: /workspace/artifacts"));
        assert!(text.contains("bytes_before: 10"));
        assert!(text.contains("bytes_after: 5"));
        assert!(text.contains("complete: false"));
        assert!(text.contains("deleted_files: 1"));
        assert!(text.contains("- /workspace/artifacts/2026-07-15/old.bin"));
        assert!(text.contains("- /workspace/artifacts/2026-07-15/older.bin"));

        let lock_error = artifacts::ArtifactError::LockTimeout {
            mode: artifacts::ArtifactLockMode::CleanupExclusive,
            timeout_ms: 5_000,
        };
        assert_eq!(
            CliError::artifacts(&lock_error).code,
            "ARTIFACT_LOCK_TIMEOUT"
        );

        let unknown = artifacts::ArtifactError::CleanupStateUnknown {
            failed_path: "/workspace/artifacts/2026-07-15".to_string(),
            deleted_paths: vec!["first".to_string(), "second".to_string()],
            source: io::Error::other("rescan failed"),
        };
        let unknown_text = artifact_cleanup_error_text(&CliError::artifacts(&unknown), &unknown);
        assert!(unknown_text.contains("deleted_paths:\n- first\n- second"));
    }

    #[test]
    fn tool_create_draft_creates_manifest_registry_and_directories() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-draft-home");
        let home = temp_path("tool-draft-fallback-home");
        let repo_root = temp_path("tool-draft-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "context-export",
            "--name",
            "Context Export",
            "--owner-workflow",
            "memory_keeper",
            "--timeout-ms",
            "123456",
            "--stdout-limit-bytes",
            "234567",
            "--stderr-limit-bytes",
            "345678",
            "--supports-dry-run",
            "--output-mode",
            "artifact",
        ])
        .expect("tool create-draft should parse");
        let exit_code = run_command(&cli.command, cli.json);
        let (workspace_key, connection) = open_test_workspace_db();
        let stored = tools::get_tool_contract(&connection, "context-export")
            .expect("tool contract get should pass")
            .expect("tool contract should exist");
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        let manifest = tools::read_tool_json(&workspace_paths, "context-export")
            .expect("tool.json should read back");
        let tool_dir = tools::tool_dir(&workspace_paths, "context-export");

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));
        assert_eq!(stored.contract.status, tools::DRAFT_TOOL_STATUS);
        assert_eq!(stored.contract.command.entrypoint, "bin/context-export");
        assert_eq!(
            stored.contract.owner_workflow.as_deref(),
            Some("memory_keeper")
        );
        assert_eq!(stored.contract.side_effects, "none");
        assert_eq!(stored.contract.approval_requirement, "none");
        assert_eq!(stored.contract.runtime.timeout_ms, 123_456);
        assert_eq!(stored.contract.runtime.stdout_limit_bytes, 234_567);
        assert_eq!(stored.contract.runtime.stderr_limit_bytes, 345_678);
        assert!(stored.contract.runtime.supports_dry_run);
        assert_eq!(
            stored.contract.runtime.output_mode,
            tools::ToolOutputMode::Artifact
        );
        assert_eq!(manifest.status, tools::DRAFT_TOOL_STATUS);
        assert_eq!(manifest.command.entrypoint, "bin/context-export");
        assert_eq!(manifest.runtime, stored.contract.runtime);
        assert!(tool_dir.join(tools::TOOL_JSON_FILE_NAME).is_file());
        assert!(tool_dir.join(tools::TOOL_BIN_DIR_NAME).is_dir());
        assert!(tool_dir.join(tools::TOOL_RUNTIME_DIR_NAME).is_dir());

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn tool_validate_checks_manifest_and_existing_executable() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-validate-home");
        let home = temp_path("tool-validate-fallback-home");
        let repo_root = temp_path("tool-validate-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "context-export",
            "--name",
            "Context Export",
            "--owner-workflow",
            "memory_keeper",
        ])
        .expect("tool create-draft should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (workspace_key, connection) = open_test_workspace_db();
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        let executable_path =
            tools::tool_dir(&workspace_paths, "context-export").join("bin/context-export");
        fs::write(&executable_path, "#!/bin/sh\n")
            .expect("placeholder executable should be created");

        let validate =
            Cli::try_parse_from(["aopmem", "--json", "tool", "validate", "context-export"])
                .expect("tool validate should parse");
        let exit_code = run_command(&validate.command, validate.json);
        let stored = tools::get_tool_contract(&connection, "context-export")
            .expect("tool contract get should pass")
            .expect("tool contract should exist");

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));
        assert_eq!(stored.contract.tool_id, "context-export");
        assert!(executable_path.is_file());

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn tool_run_executes_safe_draft_without_approval() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-run-home");
        let home = temp_path("tool-run-fallback-home");
        let repo_root = temp_path("tool-run-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "context-export",
            "--name",
            "Context Export",
            "--owner-workflow",
            "memory_keeper",
        ])
        .expect("tool create-draft should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (workspace_key, connection) = open_test_workspace_db();
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        let executable_path =
            tools::tool_dir(&workspace_paths, "context-export").join("bin/context-export");
        fs::write(
            &executable_path,
            "#!/bin/sh\nprintf '{\"argv\": [\"%s\", \"%s\"]}\\n' \"$1\" \"$2\"\n",
        )
        .expect("tool run script should be created");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&executable_path)
                .expect("tool script metadata should be readable")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&executable_path, permissions)
                .expect("tool script should be executable");
        }

        let run = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "run",
            "context-export",
            "--",
            "--json",
            "value",
        ])
        .expect("tool run should parse");
        let exit_code = run_command(&run.command, run.json);
        let ran = tools::run_tool(
            &workspace_paths,
            &connection,
            "context-export",
            &["--json".to_string(), "value".to_string()],
            None,
        )
        .expect("safe draft tool run record should be readable");

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));
        assert_eq!(ran.stdout, "{\"argv\": [\"--json\", \"value\"]}\n");
        assert!(ran.stderr.is_empty());

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn tool_run_blocks_unsafe_tool_without_approval() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-run-blocked-home");
        let home = temp_path("tool-run-blocked-fallback-home");
        let repo_root = temp_path("tool-run-blocked-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "external-export",
            "--name",
            "External Export",
            "--owner-workflow",
            "memory_keeper",
            "--side-effects",
            "external_write",
            "--approval-requirement",
            "manual_review",
        ])
        .expect("unsafe tool create-draft should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (workspace_key, _connection) = open_test_workspace_db();
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        let executable_path =
            tools::tool_dir(&workspace_paths, "external-export").join("bin/external-export");
        fs::write(&executable_path, "#!/bin/sh\nexit 0\n")
            .expect("blocked tool script should be created");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&executable_path)
                .expect("blocked tool script metadata should be readable")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&executable_path, permissions)
                .expect("blocked tool script should be executable");
        }

        let run = Cli::try_parse_from(["aopmem", "--json", "tool", "run", "external-export"])
            .expect("blocked tool run should parse");
        let exit_code = run_command(&run.command, run.json);
        let envelope = error_envelope(
            command_id(&run.command),
            &CliError::unsafe_tool_run_blocked(
                "external-export",
                "external_write",
                "manual_review",
            ),
        );
        let parsed: Value =
            serde_json::from_str(&envelope).expect("tool run envelope should parse");

        assert_eq!(exit_code, ExitCode::from(EXIT_UNSAFE_ACTION_BLOCKED));
        assert_eq!(parsed["errors"][0]["code"], "UNSAFE_ACTION_BLOCKED");
        assert_eq!(
            parsed["errors"][0]["message"],
            "tool run blocked without approval: tool_id=external-export, side_effects=external_write, approval_requirement=manual_review"
        );

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn tool_run_accepts_approved_flag_with_triple_plus_text() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-run-approved-home");
        let home = temp_path("tool-run-approved-fallback-home");
        let repo_root = temp_path("tool-run-approved-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "approved-export",
            "--name",
            "Approved Export",
            "--owner-workflow",
            "memory_keeper",
            "--side-effects",
            "external_write",
            "--approval-requirement",
            "manual_review",
        ])
        .expect("approved tool create-draft should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (workspace_key, _connection) = open_test_workspace_db();
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        let executable_path =
            tools::tool_dir(&workspace_paths, "approved-export").join("bin/approved-export");
        fs::write(&executable_path, "#!/bin/sh\necho cli-approved\n")
            .expect("approved tool script should be created");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&executable_path)
                .expect("approved tool script metadata should be readable")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&executable_path, permissions)
                .expect("approved tool script should be executable");
        }

        let run = Cli::try_parse_from([
            "aopmem",
            "--json",
            "--approved",
            "please +++ continue",
            "tool",
            "run",
            "approved-export",
        ])
        .expect("approved tool run should parse");
        let exit_code = run_command_with_approval(&run.command, run.json, run.approved.as_deref());

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn mcp_list_command_allows_empty_corporate_registry() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("mcp-list-home");
        let home = temp_path("mcp-list-fallback-home");
        let repo_root = temp_path("mcp-list-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);
        install::init_workspace(&repo_root).expect("workspace should initialize");

        let cli =
            Cli::try_parse_from(["aopmem", "--json", "mcp", "list"]).expect("mcp list parses");
        let exit_code = run_command(&cli.command, cli.json);
        let (_workspace_key, connection) = open_test_workspace_db();
        let profiles = storage::list_mcp_profiles(&connection).expect("MCP profiles should list");

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));
        assert!(profiles.is_empty());

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn mcp_add_get_and_list_commands_store_corporate_policy_fields() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("mcp-add-home");
        let home = temp_path("mcp-add-fallback-home");
        let repo_root = temp_path("mcp-add-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let add = Cli::try_parse_from([
            "aopmem",
            "--json",
            "mcp",
            "add",
            "--id",
            "corp-github",
            "--name",
            "Corporate GitHub MCP",
            "--kind",
            "corporate",
            "--status",
            "installed",
            "--read-operations",
            "repos.read,issues.read",
            "--write-operations",
            "pull_requests.write",
            "--side-effects",
            "external_write",
            "--approval-requirement",
            "manual_review",
            "--credentials-source",
            "env:GITHUB_TOKEN",
            "--notes",
            "managed by corporate IT",
        ])
        .expect("mcp add should parse");
        let get = Cli::try_parse_from(["aopmem", "--json", "mcp", "get", "--id", "corp-github"])
            .expect("mcp get should parse");
        let list =
            Cli::try_parse_from(["aopmem", "--json", "mcp", "list"]).expect("mcp list parses");

        assert_eq!(
            run_command(&add.command, add.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert_eq!(
            run_command(&get.command, get.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert_eq!(
            run_command(&list.command, list.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (_workspace_key, connection) = open_test_workspace_db();
        let stored = storage::get_mcp_profile(&connection, "corp-github")
            .expect("MCP profile get should pass")
            .expect("stored MCP profile should exist");
        let listed = storage::list_mcp_profiles(&connection).expect("MCP profiles should list");

        assert_eq!(stored.kind, "corporate");
        assert_eq!(stored.status, "installed");
        assert_eq!(stored.side_effects, "external_write");
        assert_eq!(stored.approval_requirement, "manual_review");
        assert_eq!(
            stored.credentials_source.as_deref(),
            Some("env:GITHUB_TOKEN")
        );
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "corp-github");
        assert_eq!(listed[0].side_effects, "external_write");
        assert_eq!(listed[0].approval_requirement, "manual_review");

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn recall_command_is_routable_for_stage_017() {
        let recall =
            Cli::try_parse_from(["aopmem", "--json", "recall"]).expect("recall should parse");
        let bounded = Cli::try_parse_from([
            "aopmem",
            "--json",
            "recall",
            "--query",
            "release blocker",
            "--limit",
            "2",
        ])
        .expect("bounded recall should parse");

        assert_eq!(command_id(&recall.command), "recall");
        match recall.command {
            Command::Recall(args) => {
                assert_eq!(args.query, None);
                assert_eq!(args.limit, None);
                assert!(!args.full);
                assert_eq!(args.continuation_cursor, None);
            }
            _ => panic!("expected recall command"),
        }
        match bounded.command {
            Command::Recall(args) => {
                assert_eq!(args.query.as_deref(), Some("release blocker"));
                assert_eq!(args.limit, Some(2));
                assert!(!args.full);
                assert_eq!(args.continuation_cursor, None);
            }
            _ => panic!("expected bounded recall command"),
        }
        assert!(Cli::try_parse_from(["aopmem", "recall", "--limit", "2"]).is_err());
        assert!(Cli::try_parse_from(["aopmem", "recall", "--query", "x", "--limit", "0"]).is_err());
        assert!(
            Cli::try_parse_from(["aopmem", "recall", "--query", "x", "--limit", "51",]).is_err()
        );
    }

    #[test]
    fn recall_v2_flags_enforce_mode_and_continuation_contract() {
        let full = Cli::try_parse_from(["aopmem", "recall", "--full"])
            .expect("full debug recall should parse");
        let continued = Cli::try_parse_from([
            "aopmem",
            "recall",
            "--query",
            "release blocker",
            "--continuation-cursor",
            "v1.recall.next_1",
        ])
        .expect("query-bound continuation should parse");

        match full.command {
            Command::Recall(args) => assert!(args.full),
            _ => panic!("expected recall command"),
        }
        match continued.command {
            Command::Recall(args) => {
                assert_eq!(args.query.as_deref(), Some("release blocker"));
                assert_eq!(
                    args.continuation_cursor.as_deref(),
                    Some("v1.recall.next_1")
                );
            }
            _ => panic!("expected recall command"),
        }

        for invalid in [
            vec!["aopmem", "recall", "--continuation-cursor", "v1.next"],
            vec!["aopmem", "recall", "--full", "--query", "release blocker"],
            vec![
                "aopmem",
                "recall",
                "--full",
                "--continuation-cursor",
                "v1.next",
            ],
            vec![
                "aopmem",
                "recall",
                "--query",
                "release blocker",
                "--continuation-cursor",
                "bad cursor",
            ],
        ] {
            assert!(Cli::try_parse_from(invalid).is_err());
        }
        let oversized = "x".repeat(recall::MAX_RECALL_CONTINUATION_CURSOR_BYTES + 1);
        assert!(Cli::try_parse_from([
            "aopmem",
            "recall",
            "--query",
            "release blocker",
            "--continuation-cursor",
            oversized.as_str(),
        ])
        .is_err());
    }

    #[test]
    fn invalid_recall_cursor_does_not_create_aopmem_home() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("invalid-recall-cursor-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);

        let parsed = Cli::try_parse_from([
            "aopmem",
            "recall",
            "--query",
            "release blocker",
            "--continuation-cursor",
            "invalid cursor",
        ]);

        assert!(parsed.is_err());
        assert!(
            !override_home.exists(),
            "CLI preflight must not create AOPMEM_HOME"
        );
    }

    #[test]
    fn wrong_query_recall_cursor_fails_before_workspace_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("wrong-query-recall-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let state = recall::RecallContinuationState::new(
            "original query",
            "0123456789abcdef0123456789abcdef".to_string(),
        )
        .expect("cursor state should build");
        let cursor =
            recall::encode_recall_continuation_cursor(&state).expect("cursor should encode");
        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "recall",
            "--query",
            "different query",
            "--continuation-cursor",
            cursor.as_str(),
        ])
        .expect("URL-safe cursor should parse");

        assert_eq!(
            run_command(&cli.command, cli.json),
            ExitCode::from(EXIT_INVALID_ARGS)
        );
        assert!(
            !override_home.exists(),
            "query mismatch must fail before AOPMEM_HOME access"
        );
    }

    #[test]
    fn malformed_noncanonical_and_tampered_recall_cursors_fail_before_home_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("invalid-wire-recall-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let state = recall::RecallContinuationState::new(
            "same query",
            "0123456789abcdef0123456789abcdef".to_string(),
        )
        .expect("cursor state should build");
        let canonical =
            recall::encode_recall_continuation_cursor(&state).expect("cursor should encode");
        let mut tampered = canonical.clone().into_bytes();
        let payload_index = "v1.recall.".len();
        tampered[payload_index] = if tampered[payload_index] == b'A' {
            b'B'
        } else {
            b'A'
        };
        let tampered = String::from_utf8(tampered).expect("cursor remains ASCII");
        let mut noncanonical = canonical.clone();
        let checksum_index = noncanonical.len() - 1;
        noncanonical.replace_range(checksum_index.., "A");

        for cursor in [
            "v1.recall.invalid",
            noncanonical.as_str(),
            tampered.as_str(),
        ] {
            let cli = Cli::try_parse_from([
                "aopmem",
                "--json",
                "recall",
                "--query",
                "same query",
                "--continuation-cursor",
                cursor,
            ])
            .expect("URL-safe invalid cursor should reach semantic preflight");
            assert_eq!(
                run_command(&cli.command, cli.json),
                ExitCode::from(EXIT_INVALID_ARGS)
            );
            assert!(
                !override_home.exists(),
                "invalid cursor must fail before AOPMEM_HOME access"
            );
        }

        let overlong = "x".repeat(recall::MAX_RECALL_CONTINUATION_CURSOR_BYTES + 1);
        assert!(Cli::try_parse_from([
            "aopmem",
            "recall",
            "--query",
            "same query",
            "--continuation-cursor",
            overlong.as_str(),
        ])
        .is_err());
        assert!(!override_home.exists());
    }

    #[test]
    fn blank_recall_query_is_structured_invalid_args_before_workspace_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("blank-recall-query-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let cli = Cli::try_parse_from(["aopmem", "--json", "recall", "--query", "   "])
            .expect("blank semantic query should reach command preflight");

        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_INVALID_ARGS));
        assert_eq!(CliError::invalid_recall_query().code, "INVALID_ARGS");
        assert!(
            !override_home.exists(),
            "blank query must not create AOPMEM_HOME"
        );
    }

    #[test]
    fn missing_recall_database_does_not_create_workspace_or_observability() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("missing-recall-database-home");
        let repo_root = temp_path("missing-recall-database-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "recall",
            "--query",
            "missing workspace proof",
        ])
        .expect("recall should parse");

        assert_eq!(run_parsed(&cli), ExitCode::from(EXIT_WORKSPACE_NOT_FOUND));
        assert!(
            !override_home.exists(),
            "missing operational DB must not create workspace or observability paths"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn blank_teach_title_is_rejected_before_workspace_access() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("blank-teach-title-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let cli = Cli::try_parse_from(["aopmem", "--json", "teach", "start", "--title", "   "])
            .expect("blank title should reach pure command validation");

        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_VALIDATION_FAILED));
        assert!(
            !override_home.exists(),
            "invalid teach input must not create AOPMEM_HOME"
        );
    }

    #[test]
    fn invalid_teach_and_reflection_proposals_do_not_create_workspace() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("invalid-proposal-preflight-home");
        let repo_root = temp_path("invalid-proposal-preflight-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let teach = Cli::try_parse_from([
            "aopmem",
            "--json",
            "teach",
            "propose",
            "--session-id",
            "1",
            "--payload",
            r#"{"items":[]}"#,
        ])
        .expect("semantically invalid teach proposal should parse");
        assert_eq!(
            run_command(&teach.command, teach.json),
            ExitCode::from(EXIT_VALIDATION_FAILED)
        );
        assert!(!override_home.exists());

        let valid_proposal = repo_root.join("valid-reflection-proposal.json");
        fs::write(
            &valid_proposal,
            r#"{"items":[{"op":"create_node","risk":"low","node_ref":"lesson","node_type":"lesson","status":"draft","title":"Lesson"}]}"#,
        )
        .expect("valid proposal fixture should write");
        let blank_session = Cli::try_parse_from([
            "aopmem",
            "--json",
            "reflect",
            "proposal",
            "create",
            "--session-id",
            "",
            "--proposal-file",
            valid_proposal
                .to_str()
                .expect("fixture path should be UTF-8"),
        ])
        .expect("blank session should reach pure validation");
        assert_eq!(
            run_command(&blank_session.command, blank_session.json),
            ExitCode::from(EXIT_VALIDATION_FAILED)
        );
        assert!(!override_home.exists());

        let empty_proposal = repo_root.join("empty-reflection-proposal.json");
        fs::write(&empty_proposal, r#"{"items":[]}"#).expect("empty proposal fixture should write");
        let empty_items = Cli::try_parse_from([
            "aopmem",
            "--json",
            "reflect",
            "proposal",
            "create",
            "--session-id",
            "session-1",
            "--proposal-file",
            empty_proposal
                .to_str()
                .expect("fixture path should be UTF-8"),
        ])
        .expect("empty proposal should reach pure validation");
        assert_eq!(
            run_command(&empty_items.command, empty_items.json),
            ExitCode::from(EXIT_VALIDATION_FAILED)
        );
        assert!(
            !override_home.exists(),
            "invalid proposal inputs must not create AOPMEM_HOME"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn mutation_ids_must_be_positive_during_cli_parse() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("invalid-positive-id-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let cases: &[&[&str]] = &[
            &[
                "aopmem", "node", "update", "--id", "0", "--status", "draft", "--title", "x",
            ],
            &[
                "aopmem",
                "link",
                "add",
                "--source-id",
                "0",
                "--target-id",
                "1",
                "--type",
                "uses",
            ],
            &[
                "aopmem",
                "teach",
                "propose",
                "--session-id=-1",
                "--payload",
                r#"{"items":[]}"#,
            ],
            &[
                "aopmem",
                "teach",
                "apply",
                "--session-id",
                "1",
                "--proposal-id",
                "0",
            ],
            &[
                "aopmem",
                "reflect",
                "proposal",
                "apply",
                "--proposal-id",
                "0",
            ],
        ];

        for args in cases {
            let error = Cli::try_parse_from(args.iter().copied())
                .expect_err("zero and negative mutation ids must fail CLI parsing");
            assert!(error.to_string().contains("id must be a positive integer"));
        }
        assert!(
            !override_home.exists(),
            "CLI id validation must not create AOPMEM_HOME"
        );
    }

    #[test]
    fn existing_unopenable_recall_database_records_atomic_correlated_failure() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("unopenable-recall-database-home");
        let repo_root = temp_path("unopenable-recall-database-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        drop(connection);
        fs::remove_file(workspace_paths.db()).expect("operational DB should remove");
        fs::create_dir(workspace_paths.db()).expect("blocked DB fixture should create");

        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "recall",
            "--query",
            "unopenable database proof",
        ])
        .expect("recall should parse");
        let Command::Recall(args) = &cli.command else {
            panic!("recall command should parse as recall");
        };
        let mut observation = CommandObservation::new("recall", None);

        assert_eq!(
            run_recall("recall", args, cli.json, &mut observation),
            ExitCode::from(EXIT_IO_ERROR)
        );
        assert!(observation.warnings_after(Vec::new()).is_empty());
        let bundle_id = observation
            .bundle_id
            .clone()
            .expect("failed recall should retain its chosen bundle");

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let parent: (String, String, String, i64, String) = observability
            .query_row(
                "SELECT bundle_id, outcome, error_code, duration_ms, correlation_id \
                 FROM recall_bundles",
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
            .expect("failed recall parent should query");
        assert_eq!(parent.0, bundle_id.as_str());
        assert_eq!(parent.1, "failure");
        assert_eq!(parent.2, "IO_ERROR");
        assert!(parent.3 >= 0);
        let events = observed_command_events(&workspace_paths, "recall");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "recall.started");
        assert_eq!(events[0].outcome, "started");
        assert_eq!(events[0].bundle_id.as_deref(), Some(bundle_id.as_str()));
        assert_eq!(events[0].correlation_id, parent.4);
        assert_eq!(events[0].duration_ms, None);
        assert_eq!(events[1].event_type, "recall.failed");
        assert_eq!(events[1].outcome, "failure");
        assert_eq!(events[1].error_code.as_deref(), Some("IO_ERROR"));
        assert_eq!(events[1].bundle_id.as_deref(), Some(bundle_id.as_str()));
        assert_eq!(events[1].correlation_id, parent.4);
        assert_eq!(events[1].duration_ms, Some(parent.3));

        let rendered = recall_error_envelope_with_meta_and_warnings(
            "recall",
            &CliError::io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "blocked database fixture",
            )),
            &bundle_id,
            OutputMeta {
                version: env!("CARGO_PKG_VERSION"),
                workspace_key: Some(workspace_key),
            },
            Vec::new(),
        );
        let value: Value = serde_json::from_str(&rendered).expect("error envelope should parse");
        assert_eq!(
            value["errors"][0]["details"]["bundle_id"],
            bundle_id.as_str()
        );

        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn unopenable_recall_database_preserves_core_error_when_collector_fails() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("unopenable-recall-collector-failure-home");
        let repo_root = temp_path("unopenable-recall-collector-failure-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        drop(connection);
        fs::write(workspace_paths.observability(), b"not a directory")
            .expect("blocked observability fixture should write");
        fs::remove_file(workspace_paths.db()).expect("operational DB should remove");
        fs::create_dir(workspace_paths.db()).expect("blocked DB fixture should create");

        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "recall",
            "--query",
            "collector failure proof",
        ])
        .expect("recall should parse");
        let Command::Recall(args) = &cli.command else {
            panic!("recall command should parse as recall");
        };
        let mut observation = CommandObservation::new("recall", None);

        assert_eq!(
            run_recall("recall", args, cli.json, &mut observation),
            ExitCode::from(EXIT_IO_ERROR),
            "collector failure must preserve the core read error"
        );
        let warnings = observation.warnings_after(Vec::new());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, OBSERVABILITY_WRITE_FAILED);
        assert!(
            !workspace_paths.observability_db().exists(),
            "failed collector must not leave a partial store"
        );

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn recall_cli_records_ordered_private_terminal_facts_after_read_close() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("recall-observation-home");
        let home = temp_path("recall-observation-fallback-home");
        let repo_root = temp_path("recall-observation-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "gate".to_string(),
                status: "active".to_string(),
                title: "Mandatory observation gate".to_string(),
                summary: None,
                body: Some("mandatory body".to_string()),
                source_ref: Some("source=user_instruction".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("gate should create");
        for index in 0..3 {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "workflow".to_string(),
                    status: "draft".to_string(),
                    title: "recallprivatetoken".to_string(),
                    summary: Some(format!("workflow {index}")),
                    body: Some(format!("PRIVATE_RECALL_BODY_{index}")),
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("workflow should create");
        }
        drop(connection);

        let first = Cli::try_parse_from([
            "aopmem",
            "--json",
            "recall",
            "--query",
            "recallprivatetoken",
            "--limit",
            "1",
        ])
        .expect("initial recall should parse");
        assert_eq!(
            run_command(&first.command, first.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open after first recall");
        let (bundle_id, first_timestamp, first_parent_correlation, first_duration): (
            String,
            String,
            String,
            i64,
        ) = observability
            .query_row(
                "SELECT bundle_id, timestamp, correlation_id, duration_ms FROM recall_bundles",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("first recall parent should query");
        assert!(recall::RecallBundleId::parse(&bundle_id).is_ok());
        let first_node_count: i64 = observability
            .query_row(
                "SELECT COUNT(*) FROM bundle_nodes WHERE bundle_id = ?1",
                [&bundle_id],
                |row| row.get(0),
            )
            .expect("first recall nodes should count");
        assert_eq!(
            first_node_count, 2,
            "mandatory and first task node are stored"
        );
        drop(observability);

        let connection = storage::open_workspace_db_read_only(&workspace_paths)
            .expect("operational DB should reopen read-only");
        let mandatory = recall::build_mandatory_recall_context(
            storage::load_active_mandatory_recall_nodes(&connection)
                .expect("mandatory nodes should load"),
        )
        .expect("mandatory context should fit");
        let operational_revision =
            storage::operational_recall_revision(&connection).expect("revision should build");
        let revision =
            recall::bind_recall_revision_to_workspace(&workspace_key, &operational_revision)
                .expect("workspace-bound revision should build");
        let first_page = build_task_recall_response(
            &connection,
            "recallprivatetoken",
            1,
            mandatory,
            None,
            revision,
            recall::RecallBundleId::parse(&bundle_id).expect("bundle id should parse"),
        )
        .expect("matching first page should build");
        let continuation_cursor = first_page
            .continuation_cursor
            .expect("fixture should have continuation");
        drop(connection);

        let continuation = Cli::try_parse_from([
            "aopmem",
            "--json",
            "--bundle-id",
            bundle_id.as_str(),
            "recall",
            "--query",
            "recallprivatetoken",
            "--limit",
            "1",
            "--continuation-cursor",
            continuation_cursor.as_str(),
        ])
        .expect("continuation recall should parse");
        assert_eq!(
            run_command(&continuation.command, continuation.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let (timestamp, parent_correlation, duration, continuation_count): (
            String,
            String,
            i64,
            i64,
        ) = observability
            .query_row(
                "SELECT timestamp, correlation_id, duration_ms, continuation_count \
                 FROM recall_bundles WHERE bundle_id = ?1",
                [&bundle_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("continued recall parent should query");
        assert_eq!(timestamp, first_timestamp);
        assert_eq!(parent_correlation, first_parent_correlation);
        assert!(duration >= first_duration);
        assert_eq!(continuation_count, 1);
        let distinct_nodes: (i64, i64) = observability
            .query_row(
                "SELECT COUNT(*), COUNT(DISTINCT node_id) FROM bundle_nodes \
                 WHERE bundle_id = ?1",
                [&bundle_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("continued recall nodes should count");
        assert_eq!(distinct_nodes.0, distinct_nodes.1);
        assert!(distinct_nodes.0 > first_node_count);
        let mut statement = observability
            .prepare(
                "SELECT event_type, correlation_id, duration_ms, payload_json, bundle_id \
                 FROM observability_events ORDER BY rowid",
            )
            .expect("recall event query should prepare");
        let events = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .expect("recall events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("recall events should collect");
        let first_correlation = &events[0].1;
        let second_start = events
            .iter()
            .position(|event| event.1 != *first_correlation)
            .expect("second invocation should have a new correlation");
        assert_eq!(
            events[..second_start]
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            vec!["recall.started", "recall.truncated", "recall.completed"]
        );
        assert_eq!(
            events[second_start..]
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            vec![
                "recall.started",
                "recall.continuation",
                "recall.truncated",
                "recall.completed",
            ]
        );
        for invocation in [&events[..second_start], &events[second_start..]] {
            assert!(invocation[0].2.is_none());
            let terminal_durations = invocation[1..]
                .iter()
                .map(|event| event.2)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(terminal_durations.len(), 1);
            assert!(!terminal_durations.contains(&None));
        }
        let completed_payloads = events
            .iter()
            .filter(|event| event.0 == "recall.completed")
            .map(|event| event.3.as_str())
            .collect::<Vec<_>>();
        assert!(completed_payloads
            .iter()
            .all(|payload| payload.contains("selection_reasons")));
        for invocation in [&events[..second_start], &events[second_start..]] {
            let bundle_id = invocation[0]
                .4
                .as_deref()
                .expect("every recall event should carry bundle_id");
            assert!(recall::RecallBundleId::parse(bundle_id).is_ok());
            assert!(invocation
                .iter()
                .all(|event| event.4.as_deref() == Some(bundle_id)));
        }
        for forbidden in [
            "recallprivatetoken",
            "PRIVATE_RECALL_BODY_0",
            "PRIVATE_RECALL_BODY_1",
            "PRIVATE_RECALL_BODY_2",
        ] {
            assert!(events.iter().all(|event| !event.3.contains(forbidden)));
        }

        drop(statement);
        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn mandatory_recall_overflow_records_overflow_then_failed_without_success() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("mandatory-overflow-observation-home");
        let home = temp_path("mandatory-overflow-observation-fallback-home");
        let repo_root = temp_path("mandatory-overflow-observation-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "gate".to_string(),
                status: "active".to_string(),
                title: "Overflow gate".to_string(),
                summary: None,
                body: Some("x".repeat(recall::MANDATORY_RECALL_HARD_BUDGET_BYTES)),
                source_ref: Some("source=user_instruction".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("max-size mandatory gate should create");
        drop(connection);

        let cli = Cli::try_parse_from(["aopmem", "--json", "recall", "--query", "overflow proof"])
            .expect("recall should parse");
        assert_eq!(
            run_command(&cli.command, cli.json),
            ExitCode::from(EXIT_GENERIC_ERROR)
        );

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let mut statement = observability
            .prepare(
                "SELECT event_type, outcome, error_code, duration_ms, bundle_id, payload_json \
                 FROM observability_events ORDER BY rowid",
            )
            .expect("overflow event query should prepare");
        let events = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .expect("overflow events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("overflow events should collect");

        assert_eq!(
            events
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            vec![
                "recall.started",
                "recall.mandatory_overflow",
                "recall.failed",
            ]
        );
        assert_eq!(events[1].1, "overflow");
        assert_eq!(events[2].2.as_deref(), Some("MANDATORY_CONTEXT_OVERFLOW"));
        assert!(events[0].3.is_none());
        assert_eq!(events[1].3, events[2].3);
        assert!(events[1].3.is_some());
        let bundle_id = events[0]
            .4
            .as_deref()
            .expect("overflow events should carry bundle_id");
        assert!(recall::RecallBundleId::parse(bundle_id).is_ok());
        assert!(events
            .iter()
            .all(|event| event.4.as_deref() == Some(bundle_id)));
        let parent_only_counts: (i64, i64) = observability
            .query_row(
                "SELECT (SELECT COUNT(*) FROM recall_bundles WHERE bundle_id = ?1), \
                        (SELECT COUNT(*) FROM bundle_nodes WHERE bundle_id = ?1)",
                [bundle_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("overflow parent-only counts should query");
        assert_eq!(parent_only_counts, (1, 0));
        assert!(events
            .iter()
            .all(|event| !event.5.contains("overflow proof")));
        assert!(events.iter().all(|event| event.0 != "recall.completed"));

        drop(statement);
        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn query_recall_builds_v2_response_with_full_nodes_reasons_and_null_cursor() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for query response");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "gate".to_string(),
                status: "active".to_string(),
                title: "Always verify".to_string(),
                summary: None,
                body: Some("complete mandatory body".to_string()),
                source_ref: Some("user:test".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("mandatory gate should create");
        let workflow_body = "complete task body ".repeat(2_000);
        let workflow = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "workflow".to_string(),
                status: "draft".to_string(),
                title: "Deploy release".to_string(),
                summary: None,
                body: Some(workflow_body.clone()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("workflow should create");
        let mandatory = recall::build_mandatory_recall_context(
            storage::load_active_mandatory_recall_nodes(&connection)
                .expect("mandatory nodes should load"),
        )
        .expect("mandatory context should fit");

        let revision =
            storage::operational_recall_revision(&connection).expect("revision should build");
        let response = build_task_recall_response(
            &connection,
            "Deploy release",
            12,
            mandatory,
            None,
            revision,
            recall::RecallBundleId::generate(),
        )
        .expect("task response should build");
        let task_bytes = serde_json::to_vec(&response.task)
            .expect("task section should serialize")
            .len();
        let value = serde_json::to_value(&response).expect("response should serialize");

        assert!(recall::RecallBundleId::parse(
            value["bundle_id"]
                .as_str()
                .expect("bundle id should be a string")
        )
        .is_ok());
        assert_eq!(value["mode"], "task");
        assert_eq!(value["continuation_cursor"], Value::Null);
        assert_eq!(value["mandatory"]["complete"], true);
        assert_eq!(value["task"]["complete"], true);
        assert_eq!(value["more_results"], false);
        assert_eq!(value["task"]["nodes"][0]["node"]["id"], workflow.id);
        assert_eq!(value["task"]["nodes"][0]["node"]["body"], workflow_body);
        assert_eq!(
            value["task"]["nodes"][0]["selection_reasons"][0]["kind"],
            "typed_root"
        );
        assert_eq!(value["budget"]["task"]["used_bytes"], task_bytes);
        assert!(value.get("bundle").is_none());
        assert!(value.get("content_truncated").is_none());
    }

    #[test]
    fn continuation_seen_index_preserves_exact_canonical_cursor_bytes() {
        let mut reference_state = recall::RecallContinuationState::new_with_bundle_id(
            "seen index cursor proof",
            "0123456789abcdef0123456789abcdef".to_string(),
            recall::RecallBundleId::parse("550e8400-e29b-41d4-a716-446655440002")
                .expect("fixture should be a canonical UUID v4"),
        )
        .expect("reference state should build");
        let mut indexed_state = reference_state.clone();
        let mut seen_node_ids = HashSet::new();

        for node_id in [19, 3, 11, 3] {
            reference_state
                .insert_seen_node(node_id)
                .expect("reference insertion should succeed");
            insert_continuation_seen_node(&mut indexed_state, &mut seen_node_ids, node_id)
                .expect("indexed insertion should succeed");
        }
        reference_state.emitted_count = reference_state.seen_node_ids.len() as u64;
        indexed_state.emitted_count = indexed_state.seen_node_ids.len() as u64;
        reference_state.task_node_bytes = 321;
        indexed_state.task_node_bytes = 321;

        assert_eq!(indexed_state.seen_node_ids, vec![19, 3, 11]);
        assert_eq!(seen_node_ids, HashSet::from([3, 11, 19]));
        canonicalize_continuation_seen_nodes(&mut indexed_state);

        assert_eq!(indexed_state.seen_node_ids, vec![3, 11, 19]);
        assert_eq!(indexed_state, reference_state);
        assert_eq!(
            recall::encode_recall_continuation_cursor(&indexed_state)
                .expect("indexed cursor should encode"),
            recall::encode_recall_continuation_cursor(&reference_state)
                .expect("reference cursor should encode")
        );
    }

    #[test]
    fn continuation_probe_restores_exact_state_and_cursor_before_new_candidate() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for probe rollback proof");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "workflow".to_string(),
                status: "draft".to_string(),
                title: "probe exact cursor".to_string(),
                summary: None,
                body: Some("next continuation candidate".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("probe candidate should create");
        storage::prepare_task_recall_connection(&connection)
            .expect("recall scalar should register");
        let mandatory = recall::build_mandatory_recall_context(Vec::new())
            .expect("empty mandatory section should fit");
        let candidate_selector = recall::TaskRecallCandidateSelector::new(&mandatory.section);
        let mut state = recall::RecallContinuationState::new_with_bundle_id(
            "probe exact cursor",
            "0123456789abcdef0123456789abcdef".to_string(),
            recall::RecallBundleId::parse("550e8400-e29b-41d4-a716-446655440000")
                .expect("fixture should be a canonical UUID v4"),
        )
        .expect("continuation state should build");
        let mut root_ids = HashSet::new();
        let seen_node_ids = HashSet::new();
        let expected_state = state.clone();
        let expected_cursor = recall::encode_recall_continuation_cursor(&expected_state)
            .expect("expected cursor should encode");

        assert!(probe_more_relevant_task_memory(
            &connection,
            "probe exact cursor",
            &candidate_selector,
            &seen_node_ids,
            &mut state,
            &mut root_ids,
        )
        .expect("probe should succeed"));

        assert_eq!(state, expected_state);
        assert!(root_ids.is_empty());
        assert_eq!(
            recall::encode_recall_continuation_cursor(&state).expect("actual cursor should encode"),
            expected_cursor
        );
    }

    #[test]
    fn continuation_probe_skips_duplicates_and_preserves_order_phase_and_budget() {
        let query = "probe duplicate phase";
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for duplicate probe proof");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        for index in 0..2 {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "workflow".to_string(),
                    status: "draft".to_string(),
                    title: query.to_string(),
                    summary: Some(format!("duplicate probe {index}")),
                    body: Some(query.to_string()),
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("duplicate probe candidate should create");
        }
        storage::prepare_task_recall_connection(&connection)
            .expect("recall scalar should register");
        let ordered_roots = storage::load_task_typed_roots_page(&connection, query, 0, 10)
            .expect("typed roots should load")
            .items;
        assert_eq!(ordered_roots.len(), 2);
        let mandatory = recall::build_mandatory_recall_context(Vec::new())
            .expect("empty mandatory section should fit");
        let candidate_selector = recall::TaskRecallCandidateSelector::new(&mandatory.section);
        let mut state = recall::RecallContinuationState::new_with_bundle_id(
            query,
            "0123456789abcdef0123456789abcdef".to_string(),
            recall::RecallBundleId::parse("550e8400-e29b-41d4-a716-446655440001")
                .expect("fixture should be a canonical UUID v4"),
        )
        .expect("continuation state should build");
        for node in &ordered_roots {
            state.insert_root(node).expect("ordered root should insert");
            state
                .insert_seen_node(node.id)
                .expect("seen node should insert");
        }
        state.offset = ordered_roots.len() as u64;
        state.emitted_count = ordered_roots.len() as u64;
        state.task_node_bytes = 128;
        let seen_node_ids = state.seen_node_ids.iter().copied().collect::<HashSet<_>>();
        let mut root_ids = state
            .roots
            .iter()
            .map(|root| root.node_id)
            .collect::<HashSet<_>>();
        let root_ids_before_probe = root_ids.clone();
        let roots_before_probe = state.roots.clone();
        let mut expected_state = state.clone();
        expected_state.phase = recall::RecallContinuationPhase::Graph;
        expected_state.offset = 0;
        let expected_cursor = recall::encode_recall_continuation_cursor(&expected_state)
            .expect("expected cursor should encode");

        assert!(!probe_more_relevant_task_memory(
            &connection,
            query,
            &candidate_selector,
            &seen_node_ids,
            &mut state,
            &mut root_ids,
        )
        .expect("duplicate probe should succeed"));

        assert_eq!(state, expected_state);
        assert_eq!(state.roots, roots_before_probe);
        assert_eq!(root_ids, root_ids_before_probe);
        assert_eq!(
            recall::encode_recall_continuation_cursor(&state).expect("actual cursor should encode"),
            expected_cursor
        );
    }

    #[test]
    fn task_recall_continues_three_pages_with_same_bundle_exact_dedup_and_budget() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for continuation proof");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let gate = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "gate".to_string(),
                status: "active".to_string(),
                title: "Mandatory continuation gate".to_string(),
                summary: None,
                body: Some("never omit this gate".to_string()),
                source_ref: Some("source=user_instruction".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("mandatory gate should create");
        let mut roots = Vec::new();
        for index in 0..5 {
            roots.push(
                storage::create_node(
                    &connection,
                    &storage::NewNode {
                        node_type: "workflow".to_string(),
                        status: "draft".to_string(),
                        title: "continuation proof".to_string(),
                        summary: Some(format!("workflow {index}")),
                        body: Some(format!("continuation proof body {index}")),
                        source_ref: Some("source=user_instruction/chat".to_string()),
                        confidence: Some(1.0 - f64::from(index) / 10.0),
                        trust_level: Some("high".to_string()),
                    },
                )
                .expect("workflow should create"),
            );
        }
        let linked = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "Old linked continuation lesson".to_string(),
                summary: None,
                body: Some("linked body".to_string()),
                source_ref: Some("source=teach/session".to_string()),
                confidence: Some(0.8),
                trust_level: Some("medium".to_string()),
            },
        )
        .expect("linked lesson should create");
        for root in roots.iter().take(2) {
            storage::create_link(
                &connection,
                &storage::NewLink {
                    source_node_id: root.id,
                    target_node_id: linked.id,
                    link_type: "must_follow".to_string(),
                },
            )
            .expect("duplicate target link should create");
        }

        let revision = storage::operational_recall_revision(&connection)
            .expect("static revision should build");
        let mut continuation = None;
        let mut expected_bundle = None;
        let mut all_task_ids = Vec::new();
        let mut prior_used_bytes = 0;
        let mut pages = 0;
        loop {
            let mandatory = recall::build_mandatory_recall_context(
                storage::load_active_mandatory_recall_nodes(&connection)
                    .expect("mandatory nodes should load"),
            )
            .expect("mandatory section should fit");
            let response = build_task_recall_response(
                &connection,
                "continuation proof",
                2,
                mandatory,
                continuation,
                revision.clone(),
                recall::RecallBundleId::generate(),
            )
            .expect("continuation page should build");
            pages += 1;
            assert!(response.task.nodes.len() <= 2);
            assert_eq!(response.mandatory.nodes.len(), 1);
            assert_eq!(response.mandatory.nodes[0].node.id, gate.id);
            assert!(response
                .task
                .nodes
                .iter()
                .all(|selected| !selected.selection_reasons.is_empty()));
            assert!(response.budget.task.used_bytes >= prior_used_bytes);
            assert!(response.budget.task.used_bytes <= recall::TASK_RECALL_SOFT_BUDGET_BYTES);
            assert_eq!(
                response.budget.task.remaining_bytes,
                recall::TASK_RECALL_SOFT_BUDGET_BYTES - response.budget.task.used_bytes
            );
            prior_used_bytes = response.budget.task.used_bytes;
            all_task_ids.extend(response.task.nodes.iter().map(|selected| selected.node.id));

            match expected_bundle.as_ref() {
                Some(bundle) => assert_eq!(response.bundle_id.as_str(), bundle),
                None => expected_bundle = Some(response.bundle_id.as_str().to_string()),
            }
            if response.more_results {
                let cursor = response
                    .continuation_cursor
                    .as_deref()
                    .expect("more results must always carry a cursor");
                let next_state =
                    recall::decode_recall_continuation_cursor(cursor, "continuation proof")
                        .expect("next cursor should decode");
                assert!(next_state
                    .seen_node_ids
                    .windows(2)
                    .all(|ids| ids[0] < ids[1]));
                assert_eq!(
                    recall::encode_recall_continuation_cursor(&next_state)
                        .expect("decoded cursor state should remain canonical"),
                    cursor
                );
                continuation = Some(next_state);
            } else {
                assert!(response.continuation_cursor.is_none());
                assert!(response.task.complete);
                break;
            }
            assert!(pages < 20, "continuation must make progress");
        }

        let unique = all_task_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), all_task_ids.len());
        assert_eq!(unique.len(), 6);
        assert_eq!(pages, 3);
        assert_eq!(
            all_task_ids.iter().filter(|id| **id == linked.id).count(),
            1
        );
    }

    #[test]
    fn task_recall_budget_exhaustion_returns_terminal_cursor_without_splitting_node() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for budget proof");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        for marker in ['a', 'b'] {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "workflow".to_string(),
                    status: "draft".to_string(),
                    title: "budget continuation".to_string(),
                    summary: None,
                    body: Some(marker.to_string().repeat(150_000)),
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("large workflow should create");
        }
        let mandatory = recall::build_mandatory_recall_context(Vec::new())
            .expect("empty mandatory section should fit");
        let revision =
            storage::operational_recall_revision(&connection).expect("revision should build");

        let response = build_task_recall_response(
            &connection,
            "budget continuation",
            12,
            mandatory,
            None,
            revision,
            recall::RecallBundleId::generate(),
        )
        .expect("bounded response should build");

        assert_eq!(response.task.nodes.len(), 1);
        assert!(response.more_results);
        assert!(response.budget.task.exhausted);
        assert!(response.budget.task.used_bytes <= recall::TASK_RECALL_SOFT_BUDGET_BYTES);
        let cursor = response
            .continuation_cursor
            .as_deref()
            .expect("exhausted result still explains terminal state");
        assert!(matches!(
            recall::decode_recall_continuation_cursor(cursor, "budget continuation"),
            Err(recall::RecallCursorError::Exhausted)
        ));
    }

    #[test]
    fn large_task_recall_stays_page_bounded_until_exhaustion_or_full_completion() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for large recall proof");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        for index in 0..300 {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "raw_note".to_string(),
                    status: "draft".to_string(),
                    title: format!("Large candidate {index}"),
                    summary: None,
                    body: Some(format!("largecontinuationtoken {}", "bounded".repeat(160))),
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("large candidate should create");
        }
        let revision =
            storage::operational_recall_revision(&connection).expect("revision should build");
        let mut continuation = None;
        let mut seen = std::collections::BTreeSet::new();
        let mut pages = 0;
        let exhausted;
        loop {
            let response = build_task_recall_response(
                &connection,
                "largecontinuationtoken",
                50,
                recall::build_mandatory_recall_context(Vec::new()).expect("mandatory should fit"),
                continuation,
                revision.clone(),
                recall::RecallBundleId::generate(),
            )
            .expect("large page should build");
            pages += 1;
            assert!(response.task.nodes.len() <= 50);
            assert!(response.budget.task.used_bytes <= recall::TASK_RECALL_SOFT_BUDGET_BYTES);
            assert!(
                serde_json::to_vec(&response)
                    .expect("response should serialize")
                    .len()
                    < 512 * 1024
            );
            for selected in &response.task.nodes {
                assert!(seen.insert(selected.node.id), "duplicate continuation node");
                assert!(!selected.selection_reasons.is_empty());
            }
            if response.budget.task.exhausted {
                exhausted = true;
                assert!(response.more_results);
                break;
            }
            if !response.more_results {
                exhausted = false;
                break;
            }
            continuation = Some(
                recall::decode_recall_continuation_cursor(
                    response
                        .continuation_cursor
                        .as_deref()
                        .expect("incomplete large page needs cursor"),
                    "largecontinuationtoken",
                )
                .expect("large cursor should decode"),
            );
            assert!(pages < 20, "large recall must make bounded progress");
        }

        assert!(exhausted || seen.len() == 300);
        assert!(pages < 20);
    }

    #[test]
    fn bm25_strong_late_match_is_selected_before_weak_rows_exhaust_budget() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for BM25 starvation proof");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        for index in 0..64 {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "raw_note".to_string(),
                    status: "draft".to_string(),
                    title: format!("Weak candidate {index}"),
                    summary: None,
                    body: Some(format!("starvationbm25 {}", "weak filler ".repeat(650))),
                    source_ref: Some("source=teach/session".to_string()),
                    confidence: Some(0.8),
                    trust_level: Some("high".to_string()),
                },
            )
            .expect("weak candidate should create");
        }
        let strong = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "workflow".to_string(),
                status: "draft".to_string(),
                title: "Critical starvationbm25 workflow".to_string(),
                summary: Some("the applicable learned process".to_string()),
                body: Some("starvationbm25 ".repeat(32)),
                source_ref: Some("source=teach/session".to_string()),
                confidence: Some(0.8),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("strong late workflow should create");
        storage::prepare_task_recall_connection(&connection)
            .expect("recall scalar should register");
        let first_fts = storage::load_task_fts_page(&connection, "starvationbm25", 0, 1)
            .expect("first FTS page should load")
            .items
            .into_iter()
            .next()
            .expect("FTS should find a match");
        assert_eq!(first_fts.node.id, strong.id);

        let revision =
            storage::operational_recall_revision(&connection).expect("revision should build");
        let response = build_task_recall_response(
            &connection,
            "starvationbm25",
            50,
            recall::build_mandatory_recall_context(Vec::new()).expect("mandatory should fit"),
            None,
            revision,
            recall::RecallBundleId::generate(),
        )
        .expect("starvation recall should build");

        assert!(response.budget.task.exhausted);
        assert_eq!(response.task.nodes[0].node.id, strong.id);
        assert!(matches!(
            response.task.nodes[0].selection_reasons.as_slice(),
            [recall::RecallSelectionReason::FtsBm25 { rank }]
                if rank.total_cmp(&first_fts.rank).is_eq()
        ));
    }

    #[test]
    fn recall_cursor_revision_detects_mutation_explicitly() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for stale proof");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        for index in 0..2 {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "workflow".to_string(),
                    status: "draft".to_string(),
                    title: "stale continuation".to_string(),
                    summary: None,
                    body: Some(format!("body {index}")),
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("workflow should create");
        }
        let first_revision =
            storage::operational_recall_revision(&connection).expect("first revision should build");
        let response = build_task_recall_response(
            &connection,
            "stale continuation",
            1,
            recall::build_mandatory_recall_context(Vec::new()).expect("mandatory should fit"),
            None,
            first_revision,
            recall::RecallBundleId::generate(),
        )
        .expect("first page should build");
        let cursor = recall::decode_recall_continuation_cursor(
            response
                .continuation_cursor
                .as_deref()
                .expect("first page should continue"),
            "stale continuation",
        )
        .expect("cursor should decode");
        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "new mutation".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("mutation should create");
        let current_revision =
            storage::operational_recall_revision(&connection).expect("new revision should build");
        let error = validate_recall_cursor_revision(&cursor, &current_revision)
            .expect_err("mutation must stale cursor");

        assert_eq!(error.code, "STALE_RECALL_CURSOR");
    }

    #[test]
    fn stale_recall_cursor_fails_without_mutating_observability() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("stale-recall-observation-home");
        let repo_root = temp_path("stale-recall-observation-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        for index in 0..2 {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "workflow".to_string(),
                    status: "draft".to_string(),
                    title: "stale observation continuation".to_string(),
                    summary: None,
                    body: Some(format!("body {index}")),
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("workflow should create");
        }
        let operational_revision =
            storage::operational_recall_revision(&connection).expect("revision should build");
        let revision =
            recall::bind_recall_revision_to_workspace(&workspace_key, &operational_revision)
                .expect("workspace-bound revision should build");
        let response = build_task_recall_response(
            &connection,
            "stale observation continuation",
            1,
            recall::build_mandatory_recall_context(Vec::new()).expect("mandatory should fit"),
            None,
            revision,
            recall::RecallBundleId::generate(),
        )
        .expect("first recall page should build");
        let cursor = response
            .continuation_cursor
            .expect("first page should have a continuation cursor");
        drop(connection);

        mutation::mutate_workspace(&workspace_paths, |database, _effects| {
            storage::create_node(
                database,
                &storage::NewNode {
                    node_type: "lesson".to_string(),
                    status: "draft".to_string(),
                    title: "revision mutation".to_string(),
                    summary: None,
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
        })
        .expect("revision mutation should commit");

        assert!(
            !workspace_paths.observability().exists(),
            "direct fixture setup must not create observability state"
        );
        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "recall",
            "--query",
            "stale observation continuation",
            "--limit",
            "1",
            "--continuation-cursor",
            cursor.as_str(),
        ])
        .expect("stale continuation command should parse");
        assert_eq!(
            run_command(&cli.command, cli.json),
            ExitCode::from(EXIT_GENERIC_ERROR)
        );

        assert!(
            !workspace_paths.observability().exists(),
            "revision mismatch must fail before creating observability state"
        );

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn copied_database_cursor_precedes_mandatory_overflow_and_never_observes_target() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("cross-workspace-recall-home");
        let repo_a = temp_path("cross-workspace-recall-repo-a");
        let repo_b = temp_path("cross-workspace-recall-repo-b");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_a).expect("workspace A repo should create");
        fs::create_dir_all(&repo_b).expect("workspace B repo should create");
        let _cwd = CurrentDirGuard::set(&repo_a);
        let (workspace_a_key, workspace_a_paths, connection) =
            open_current_workspace_context().expect("workspace A should open");
        for index in 0..3 {
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "workflow".to_string(),
                    status: "draft".to_string(),
                    title: "cross workspace continuation".to_string(),
                    summary: Some(format!("workspace copy item {index}")),
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
            )
            .expect("workspace A workflow should create");
        }
        let operational_revision = storage::operational_recall_revision(&connection)
            .expect("workspace A revision should build");
        let workspace_revision =
            recall::bind_recall_revision_to_workspace(&workspace_a_key, &operational_revision)
                .expect("workspace A revision should bind");
        let response = build_task_recall_response(
            &connection,
            "cross workspace continuation",
            1,
            recall::build_mandatory_recall_context(Vec::new()).expect("mandatory should fit"),
            None,
            workspace_revision,
            recall::RecallBundleId::generate(),
        )
        .expect("workspace A first page should build");
        let bundle_id = response.bundle_id.clone();
        let cursor = response
            .continuation_cursor
            .expect("workspace A first page should continue");
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("workspace A WAL should checkpoint before copy");
        drop(connection);

        let paths = storage::resolve_paths().expect("AOPMem paths should resolve");
        let workspace_b_key =
            storage::workspace_key(&repo_b).expect("workspace B key should resolve");
        let workspace_b_paths = storage::ensure_workspace_dirs(&paths, &workspace_b_key)
            .expect("workspace B paths should create");
        fs::copy(workspace_a_paths.db(), workspace_b_paths.db())
            .expect("operational database should copy exactly to workspace B");
        assert!(
            !workspace_b_paths.observability().exists(),
            "workspace B observability must start absent"
        );

        env::set_current_dir(&repo_b).expect("current directory should switch to workspace B");
        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "--bundle-id",
            bundle_id.as_str(),
            "recall",
            "--query",
            "cross workspace continuation",
            "--limit",
            "1",
            "--continuation-cursor",
            cursor.as_str(),
        ])
        .expect("workspace B continuation should parse");

        assert_eq!(
            run_parsed(&cli),
            ExitCode::from(EXIT_GENERIC_ERROR),
            "copied operational state must not make a workspace A cursor valid in workspace B"
        );
        assert!(
            !workspace_b_paths.observability().exists(),
            "workspace-bound cursor mismatch must not create bundles, nodes, or events in B"
        );

        let workspace_b_connection = storage::open_workspace_db(&workspace_b_paths)
            .expect("workspace B should reopen for the ordering mutant");
        let overflowing_gate = storage::create_node(
            &workspace_b_connection,
            &storage::NewNode {
                node_type: "gate".to_string(),
                status: "active".to_string(),
                title: "Mandatory overflow must lose to stale cursor".to_string(),
                summary: None,
                body: Some("x".repeat(storage::MAX_NODE_BODY_BYTES)),
                source_ref: Some("source=user_instruction".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("maximum-sized active gate should create");
        let workspace_b_revision = storage::operational_recall_revision(&workspace_b_connection)
            .expect("workspace B revision should build after mutation");
        assert_ne!(
            workspace_b_revision, operational_revision,
            "workspace B mutation must change the copied operational revision"
        );
        workspace_b_connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("workspace B WAL should checkpoint");
        drop(workspace_b_connection);

        let workspace_b_reader = storage::open_workspace_db_read_only(&workspace_b_paths)
            .expect("workspace B should open read-only");
        let mandatory_nodes = storage::load_active_mandatory_recall_nodes(&workspace_b_reader)
            .expect("workspace B mandatory nodes should load");
        match recall::build_mandatory_recall_context(mandatory_nodes) {
            Err(recall::MandatoryContextBuildError::Overflow {
                offending_node_ids, ..
            }) => assert_eq!(offending_node_ids, vec![overflowing_gate.id]),
            other => panic!("fixture must overflow mandatory context, got {other:?}"),
        }
        let decoded = recall::decode_recall_continuation_cursor(
            cursor.as_str(),
            "cross workspace continuation",
        )
        .expect("workspace A cursor should decode structurally");
        let Command::Recall(recall_args) = &cli.command else {
            panic!("fixture command should remain recall");
        };
        match execute_recall_command(
            &workspace_b_reader,
            &workspace_b_key,
            recall_args,
            Some(decoded),
            recall::RecallBundleId::parse(bundle_id.as_str())
                .expect("fixture bundle should remain canonical"),
        ) {
            Err(RecallCommandError::CursorBinding(error)) => {
                assert_eq!(error.code, "STALE_RECALL_CURSOR")
            }
            _ => panic!("stale workspace cursor must be rejected before mandatory overflow"),
        }
        drop(workspace_b_reader);

        assert_eq!(
            run_parsed(&cli),
            ExitCode::from(EXIT_GENERIC_ERROR),
            "real CLI must keep stale cursor precedence after mandatory overflow mutation"
        );
        assert!(
            !workspace_b_paths.observability().exists(),
            "both cursor rejections must leave B observability entirely absent"
        );

        let workspace_b_connection = storage::open_workspace_db(&workspace_b_paths)
            .expect("workspace B should reopen for the read-error ordering fixture");
        workspace_b_connection
            .execute(
                "UPDATE nodes SET body = ?1 WHERE id = ?2",
                rusqlite::params![vec![0xff_u8], overflowing_gate.id],
            )
            .expect("schema-valid BLOB body fixture should update");
        workspace_b_connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("workspace B read-error WAL should checkpoint");
        drop(workspace_b_connection);

        let workspace_b_reader = storage::open_workspace_db_read_only(&workspace_b_paths)
            .expect("workspace B read-error fixture should open");
        storage::operational_recall_revision(&workspace_b_reader)
            .expect("revision scan must accept the schema-valid BLOB value");
        assert!(
            storage::load_active_mandatory_recall_nodes(&workspace_b_reader).is_err(),
            "mandatory row decoding must fail after revision scan"
        );
        let decoded = recall::decode_recall_continuation_cursor(
            cursor.as_str(),
            "cross workspace continuation",
        )
        .expect("workspace A cursor should decode again");
        match execute_recall_command(
            &workspace_b_reader,
            &workspace_b_key,
            recall_args,
            Some(decoded),
            recall::RecallBundleId::parse(bundle_id.as_str())
                .expect("fixture bundle should remain canonical"),
        ) {
            Err(RecallCommandError::CursorBinding(error)) => {
                assert_eq!(error.code, "STALE_RECALL_CURSOR")
            }
            _ => panic!("stale cursor must also precede mandatory row decoding failure"),
        }
        drop(workspace_b_reader);

        assert_eq!(
            run_parsed(&cli),
            ExitCode::from(EXIT_GENERIC_ERROR),
            "real CLI must keep stale cursor precedence over mandatory row decode errors"
        );
        assert!(
            !workspace_b_paths.observability().exists(),
            "read-error ordering proof must leave B observability absent"
        );

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_a).expect("workspace A repo should remove");
        fs::remove_dir_all(repo_b).expect("workspace B repo should remove");
    }

    #[test]
    fn recall_begin_holds_one_read_snapshot_across_concurrent_wal_mutation() {
        let db_path = temp_path("recall-wal-read-snapshot");
        let mut writer = rusqlite::Connection::open(&db_path).expect("writer database should open");
        writer
            .execute_batch("PRAGMA journal_mode=WAL;")
            .expect("WAL mode should enable");
        crate::schema::apply_migrations(&mut writer).expect("migrations should apply");
        storage::create_node(
            &writer,
            &storage::NewNode {
                node_type: "workflow".to_string(),
                status: "draft".to_string(),
                title: "Snapshot before".to_string(),
                summary: None,
                body: Some("before".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("initial node should create");
        let reader = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("read-only connection should open");
        reader
            .execute_batch("BEGIN DEFERRED TRANSACTION;")
            .expect("recall read transaction should begin");
        let snapshot_revision =
            storage::operational_recall_revision(&reader).expect("snapshot revision should build");
        assert_eq!(
            storage::list_nodes(&reader)
                .expect("snapshot nodes should list")
                .len(),
            1
        );

        storage::create_node(
            &writer,
            &storage::NewNode {
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "Concurrent mutation".to_string(),
                summary: None,
                body: Some("after".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("WAL writer should not be blocked by recall reader");

        assert_eq!(
            storage::operational_recall_revision(&reader)
                .expect("reader revision should remain available"),
            snapshot_revision
        );
        assert_eq!(
            storage::list_nodes(&reader)
                .expect("reader snapshot should remain stable")
                .len(),
            1
        );
        reader
            .execute_batch("ROLLBACK;")
            .expect("read snapshot should close");
        let fresh_reader = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("fresh reader should open");
        assert_ne!(
            storage::operational_recall_revision(&fresh_reader)
                .expect("fresh revision should build"),
            snapshot_revision
        );
        assert_eq!(
            storage::list_nodes(&fresh_reader)
                .expect("fresh nodes should list")
                .len(),
            2
        );

        drop(fresh_reader);
        drop(reader);
        drop(writer);
        for path in [
            db_path.clone(),
            PathBuf::from(format!("{}-wal", db_path.display())),
            PathBuf::from(format!("{}-shm", db_path.display())),
        ] {
            if path.exists() {
                fs::remove_file(path).expect("temporary SQLite file should be removed");
            }
        }
    }

    #[test]
    fn full_recall_returns_all_operational_sections_and_is_read_only() {
        let mut connection = rusqlite::Connection::open_in_memory()
            .expect("in-memory DB should open for full recall proof");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let active = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "workflow".to_string(),
                status: "active".to_string(),
                title: "Active full node".to_string(),
                summary: None,
                body: Some("full active body".to_string()),
                source_ref: Some("source=user_instruction/full".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("active node should create");
        let deprecated = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "lesson".to_string(),
                status: "deprecated".to_string(),
                title: "Deprecated full node".to_string(),
                summary: None,
                body: Some("full deprecated body".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("deprecated node should create");
        storage::create_link(
            &connection,
            &storage::NewLink {
                source_node_id: active.id,
                target_node_id: deprecated.id,
                link_type: "historical".to_string(),
            },
        )
        .expect("link should create");
        storage::create_alias(
            &connection,
            &storage::NewAlias {
                node_id: active.id,
                alias: "full alias".to_string(),
            },
        )
        .expect("alias should create");
        storage::create_tag(
            &connection,
            &storage::NewTag {
                node_id: active.id,
                tag: "full-tag".to_string(),
            },
        )
        .expect("tag should create");
        storage::create_source(
            &connection,
            &storage::NewSource {
                node_id: active.id,
                source_ref: "source=user_instruction/full".to_string(),
            },
        )
        .expect("source should create");
        let changes_before = connection.total_changes();

        let response = build_full_recall_response(&connection, recall::RecallBundleId::generate())
            .expect("full read-only response should build");
        let value = serde_json::to_value(&response).expect("full response should serialize");

        assert_eq!(connection.total_changes(), changes_before);
        assert_eq!(value["mode"], "full");
        assert_eq!(value["debug_only"], true);
        assert_eq!(value["more_results"], false);
        assert_eq!(value["continuation_cursor"], Value::Null);
        for section in [
            "nodes",
            "links",
            "aliases",
            "tags",
            "sources",
            "events",
            "tool_contracts",
            "mcp_profiles",
        ] {
            assert!(value[section].is_array(), "missing full section {section}");
        }
        assert!(value["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .any(|node| {
                node["status"] == "deprecated" && node["body"] == "full deprecated body"
            }));
        assert!(value.get("budget").is_none());
        assert!(value.get("mandatory").is_none());
    }

    #[test]
    fn recall_data_always_adds_complete_mandatory_context_and_exact_budget() {
        let mandatory_nodes = vec![
            storage::Node {
                id: 1,
                node_type: "gate".to_string(),
                status: "active".to_string(),
                title: "Never skip proof".to_string(),
                summary: None,
                body: Some("complete gate body".repeat(200)),
                source_ref: Some("user:test".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            storage::Node {
                id: 2,
                node_type: "project_profile".to_string(),
                status: "active".to_string(),
                title: "Current project".to_string(),
                summary: None,
                body: Some("complete project body".to_string()),
                source_ref: Some("user:test".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        ];
        let mandatory = recall::build_mandatory_recall_context(mandatory_nodes)
            .expect("mandatory fixture should fit");
        let mandatory_bytes = mandatory.used_bytes;
        let task_data = json!({
            "mode": "bounded_query",
            "bundle": {
                "matches": (0..2_000).map(|id| json!({"id": id})).collect::<Vec<_>>()
            }
        });
        let task_bytes =
            recall::canonical_json_byte_len(&task_data).expect("task fixture should serialize");

        let data =
            recall_data_with_mandatory(task_data, mandatory, recall::RecallBundleId::generate())
                .expect("common recall fields should attach");

        assert!(recall::RecallBundleId::parse(
            data["bundle_id"]
                .as_str()
                .expect("bundle id should be a string")
        )
        .is_ok());
        assert_eq!(data["mandatory"]["complete"], Value::Bool(true));
        assert_eq!(data["mandatory"]["nodes"][0]["node"]["id"], 1);
        assert_eq!(data["mandatory"]["nodes"][1]["node"]["id"], 2);
        assert_eq!(
            data["mandatory"]["nodes"][0]["node"]["body"],
            Value::String("complete gate body".repeat(200))
        );
        assert_eq!(data["budget"]["mandatory"]["used_bytes"], mandatory_bytes);
        assert_eq!(data["budget"]["task"]["used_bytes"], task_bytes);
        assert_eq!(
            data["budget"]["total_used_bytes"],
            mandatory_bytes + task_bytes
        );
    }

    #[test]
    fn mandatory_overflow_json_has_ids_and_no_partial_success_data() {
        let offending_node_ids = (1_i64..=1_000).collect::<Vec<_>>();
        let error = CliError::mandatory_context_overflow(
            recall::MANDATORY_RECALL_HARD_BUDGET_BYTES,
            offending_node_ids.len(),
        );
        let envelope = mandatory_context_overflow_envelope(
            "recall",
            &error,
            recall::MANDATORY_RECALL_HARD_BUDGET_BYTES,
            900_000,
            offending_node_ids.clone(),
        );
        let parsed: Value = serde_json::from_str(&envelope).expect("envelope should parse");

        assert_eq!(parsed["ok"], Value::Bool(false));
        assert_eq!(parsed["data"], Value::Null);
        assert_eq!(
            parsed["errors"][0]["code"],
            Value::String("MANDATORY_CONTEXT_OVERFLOW".to_string())
        );
        assert_eq!(
            parsed["errors"][0]["details"]["offending_node_ids"],
            json!(offending_node_ids)
        );
        assert!(parsed.get("bundle_id").is_none());
        assert!(!envelope.contains("mandatory context body"));
        assert!(!envelope.contains("\"mandatory\":{\"complete\""));
    }

    #[test]
    fn legacy_recall_data_preserves_bundle_fields_and_surfaces_bounds() {
        let bundle = recall::build_structured_bundle(Vec::new());
        let data = legacy_recall_data(bundle, true, true);

        assert!(data["project_profiles"].is_object());
        assert!(data["gates"].is_object());
        assert!(data["workflows"].is_object());
        assert!(data["linked_nodes"].is_array());
        assert!(data["fts_fallback"].is_array());
        assert!(data["compact"].is_object());
        assert_eq!(data["more_results"], Value::Bool(true));
        assert_eq!(data["content_truncated"], Value::Bool(true));
    }

    #[test]
    fn adapter_seed_command_parses_stage_023_args() {
        let default_target = Cli::try_parse_from(["aopmem", "--json", "adapter", "seed"])
            .expect("adapter seed should parse");
        let explicit_target =
            Cli::try_parse_from(["aopmem", "--json", "adapter", "seed", "--file", "CLAUDE.md"])
                .expect("adapter seed with explicit file should parse");

        assert_eq!(command_id(&default_target.command), "adapter_seed");
        assert_eq!(command_id(&explicit_target.command), "adapter_seed");
    }

    #[test]
    fn adapter_sync_and_status_commands_parse_stage_024_args() {
        let sync_default = Cli::try_parse_from(["aopmem", "--json", "adapter", "sync"])
            .expect("adapter sync should parse");
        let sync_explicit =
            Cli::try_parse_from(["aopmem", "--json", "adapter", "sync", "--file", "CLAUDE.md"])
                .expect("adapter sync with explicit file should parse");
        let status_default = Cli::try_parse_from(["aopmem", "--json", "adapter", "status"])
            .expect("adapter status should parse");

        assert_eq!(command_id(&sync_default.command), "adapter_sync");
        assert_eq!(command_id(&sync_explicit.command), "adapter_sync");
        assert_eq!(command_id(&status_default.command), "adapter_status");
    }

    #[test]
    fn adapter_commands_record_seed_sync_and_real_drift_only() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("adapter-observation-home");
        let home = temp_path("adapter-observation-fallback-home");
        let repo_root = temp_path("adapter-observation-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        install::init_workspace(&repo_root).expect("workspace should initialize");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let status = Cli::try_parse_from(["aopmem", "--json", "adapter", "status"])
            .expect("adapter status should parse");
        assert_eq!(
            run_command(&status.command, status.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        let seed = Cli::try_parse_from(["aopmem", "--json", "adapter", "seed"])
            .expect("adapter seed should parse");
        assert_eq!(
            run_command(&seed.command, seed.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        let instruction_file = repo_root.join("AGENTS.md");
        let drifted = fs::read_to_string(&instruction_file)
            .expect("seeded adapter should read")
            .replace(
                "Do not edit inside this block manually.",
                "Manual drift inside managed block.",
            );
        fs::write(&instruction_file, drifted).expect("drifted adapter should write");
        assert_eq!(
            run_command(&status.command, status.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        let sync = Cli::try_parse_from(["aopmem", "--json", "adapter", "sync"])
            .expect("adapter sync should parse");
        assert_eq!(
            run_command(&sync.command, sync.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert_eq!(
            run_command(&status.command, status.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_key = storage::workspace_key(&repo_root).expect("workspace key should build");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let mut statement = observability
            .prepare(
                "SELECT event_type, outcome, payload_json, bundle_id \
                 FROM observability_events ORDER BY rowid",
            )
            .expect("adapter events should prepare");
        let events = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .expect("adapter events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("adapter events should collect");
        assert_eq!(
            events
                .iter()
                .map(|event| (event.0.as_str(), event.1.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("adapter.drift", "missing"),
                ("adapter.seed", "success"),
                ("adapter.drift", "warning"),
                ("adapter.sync", "success"),
            ]
        );
        assert!(events.iter().all(|event| event.3.is_none()));
        assert!(events.iter().all(|event| !event.2.contains("AGENTS.md")));

        drop(statement);
        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn adapter_status_without_workspace_does_not_create_observability_paths() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("adapter-no-workspace-home");
        let home = temp_path("adapter-no-workspace-fallback-home");
        let repo_root = temp_path("adapter-no-workspace-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let status = Cli::try_parse_from(["aopmem", "--json", "adapter", "status"])
            .expect("adapter status should parse");

        assert_eq!(
            run_command(&status.command, status.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert!(
            !override_home.exists(),
            "adapter observability must not create a missing workspace"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn status_command_is_implemented_for_stage_025() {
        let cli = Cli::try_parse_from(["aopmem", "--json", "status"]).expect("status should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_ne!(exit_code, ExitCode::from(EXIT_NOT_IMPLEMENTED));
    }

    #[test]
    fn doctor_command_is_implemented_for_stage_038() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("doctor-home");
        let home = temp_path("doctor-fallback-home");
        let repo_root = temp_path("doctor-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from(["aopmem", "--json", "doctor"]).expect("doctor should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_ne!(exit_code, ExitCode::from(EXIT_NOT_IMPLEMENTED));

        drop(_cwd);
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
        if override_home.exists() {
            fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        }
    }

    #[test]
    fn doctor_json_reports_ready_health_for_prepared_workspace() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("doctor-json-home");
        let home = temp_path("doctor-json-fallback-home");
        let repo_root = temp_path("doctor-json-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        storage::ensure_global_dirs(&storage::resolve_paths().expect("paths should resolve"))
            .expect("global dirs should be created");
        let workspace_key =
            storage::workspace_key(&repo_root).expect("workspace key should resolve");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        storage::open_workspace_db(&workspace_paths).expect("workspace db should initialize");
        adapter::seed_instruction_file(&repo_root.join("AGENTS.md"))
            .expect("adapter block should be seeded");

        let report = verify::run_doctor(&repo_root).expect("doctor should succeed");
        let envelope = doctor_success_envelope(&report);
        let parsed: Value = serde_json::from_str(&envelope).expect("doctor envelope should parse");

        assert_eq!(parsed["ok"], Value::Bool(true));
        assert_eq!(parsed["command"], Value::String("doctor".to_string()));
        assert_eq!(parsed["data"]["healthy"], Value::Bool(true));
        assert_eq!(
            parsed["data"]["checks"]["global_dirs"]["status"],
            Value::String("ready".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["workspace"]["status"],
            Value::String("ready".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["db"]["status"],
            Value::String("ready".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["schema"]["status"],
            Value::String("ready".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["fts"]["status"],
            Value::String("ready".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["adapter_block"]["status"],
            Value::String("ready".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["artifacts_dirs"]["status"],
            Value::String("ready".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["tools_dirs"]["status"],
            Value::String("ready".to_string())
        );
        assert_eq!(
            parsed["meta"]["workspace_key"],
            parsed["data"]["workspace_key"]
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn doctor_json_reports_missing_health_for_uninitialized_workspace() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("doctor-json-missing-home");
        let home = temp_path("doctor-json-missing-fallback-home");
        let repo_root = temp_path("doctor-json-missing-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        storage::ensure_global_dirs(&storage::resolve_paths().expect("paths should resolve"))
            .expect("global dirs should be created");

        let report = verify::run_doctor(&repo_root).expect("doctor should succeed");
        let envelope = doctor_success_envelope(&report);
        let parsed: Value = serde_json::from_str(&envelope).expect("doctor envelope should parse");

        assert_eq!(parsed["ok"], Value::Bool(true));
        assert_eq!(parsed["command"], Value::String("doctor".to_string()));
        assert_eq!(parsed["data"]["healthy"], Value::Bool(false));
        assert_eq!(
            parsed["data"]["checks"]["workspace"]["status"],
            Value::String("missing".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["db"]["status"],
            Value::String("missing".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["schema"]["status"],
            Value::String("missing".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["fts"]["status"],
            Value::String("missing".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["artifacts_dirs"]["status"],
            Value::String("missing".to_string())
        );
        assert_eq!(
            parsed["data"]["checks"]["tools_dirs"]["status"],
            Value::String("missing".to_string())
        );
        assert_eq!(
            parsed["meta"]["workspace_key"],
            parsed["data"]["workspace_key"]
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn verify_command_is_implemented_for_stage_046() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("verify-home");
        let home = temp_path("verify-fallback-home");
        let repo_root = temp_path("verify-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        install::init_workspace(&repo_root).expect("workspace should initialize");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from(["aopmem", "--json", "verify"]).expect("verify should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_ne!(exit_code, ExitCode::from(EXIT_NOT_IMPLEMENTED));
        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn verify_json_reports_clean_initialized_workspace() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("verify-json-home");
        let home = temp_path("verify-json-fallback-home");
        let repo_root = temp_path("verify-json-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        install::init_workspace(&repo_root).expect("workspace should initialize");

        let report = verify::run_lint(&repo_root).expect("lint should succeed");
        let envelope = verify_success_envelope(&report);
        let parsed: Value = serde_json::from_str(&envelope).expect("verify envelope should parse");

        assert_eq!(parsed["ok"], Value::Bool(true));
        assert_eq!(parsed["command"], Value::String("verify".to_string()));
        assert_eq!(parsed["data"]["clean"], Value::Bool(true));
        assert_eq!(parsed["data"]["summary"]["total"], Value::Number(0.into()));
        assert_eq!(
            parsed["meta"]["workspace_key"],
            parsed["data"]["workspace_key"]
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn verify_json_reports_detected_lint_issues() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("verify-json-dirty-home");
        let home = temp_path("verify-json-dirty-fallback-home");
        let repo_root = temp_path("verify-json-dirty-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        seed_dirty_verify_workspace(&repo_root);

        let report = verify::run_lint(&repo_root).expect("lint should succeed");
        let envelope = verify_success_envelope(&report);
        let parsed: Value = serde_json::from_str(&envelope).expect("verify envelope should parse");

        assert_eq!(parsed["ok"], Value::Bool(true));
        assert_eq!(parsed["data"]["clean"], Value::Bool(false));
        assert_eq!(parsed["data"]["summary"]["total"], Value::Number(6.into()));
        assert_eq!(
            parsed["data"]["summary"]["broken_links"],
            Value::Number(1.into())
        );
        assert_eq!(
            parsed["data"]["summary"]["duplicate_ids"],
            Value::Number(1.into())
        );
        assert_eq!(
            parsed["data"]["summary"]["deprecated_active_links"],
            Value::Number(1.into())
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn verify_reports_workspace_not_found_when_db_is_missing() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("verify-missing-workspace-home");
        let home = temp_path("verify-missing-workspace-fallback-home");
        let repo_root = temp_path("verify-missing-workspace-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from(["aopmem", "--json", "verify"]).expect("verify should parse");
        let exit_code = run_command(&cli.command, cli.json);
        let db_path = storage::resolve_paths()
            .expect("paths should resolve")
            .workspaces()
            .join(storage::workspace_key(&repo_root).expect("workspace key should resolve"))
            .join("aopmem.sqlite");
        let envelope = error_envelope(
            command_id(&cli.command),
            &CliError::workspace_db_missing(db_path.to_str().expect("db path should be utf-8")),
        );
        let parsed: Value =
            serde_json::from_str(&envelope).expect("missing workspace envelope should parse");

        assert_eq!(exit_code, ExitCode::from(EXIT_WORKSPACE_NOT_FOUND));
        assert_eq!(parsed["errors"][0]["code"], "WORKSPACE_NOT_FOUND");

        drop(_cwd);
        if override_home.exists() {
            fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        }
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn node_list_missing_workspace_returns_exit_3_without_creating_paths() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("node-list-missing-workspace-home");
        let home = temp_path("node-list-missing-workspace-fallback-home");
        let repo_root = temp_path("node-list-missing-workspace-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from(["aopmem", "--json", "node", "list"])
            .expect("node list should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_WORKSPACE_NOT_FOUND));
        assert!(
            !override_home.exists(),
            "read-only node list must not create AOPMEM_HOME or a workspace DB"
        );

        drop(_cwd);
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn tool_run_dry_run_missing_workspace_does_not_create_paths() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("tool-run-read-only-home");
        let home = temp_path("tool-run-read-only-fallback-home");
        let repo_root = temp_path("tool-run-read-only-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "run",
            "missing-tool",
            "--dry-run",
        ])
        .expect("tool dry-run should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_WORKSPACE_NOT_FOUND));
        assert!(
            !override_home.exists(),
            "tool dry-run must not create AOPMEM_HOME or a workspace DB"
        );

        drop(_cwd);
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn node_create_rejects_invalid_type_via_cli_validation() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("node-invalid-type-home");
        let home = temp_path("node-invalid-type-fallback-home");
        let repo_root = temp_path("node-invalid-type-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from([
            "aopmem", "--json", "node", "create", "--type", "unknown", "--title", "Bad type",
        ])
        .expect("node create should parse");
        let exit_code = run_command(&cli.command, cli.json);
        let (_workspace_key, connection) = open_test_workspace_db();

        assert_eq!(exit_code, ExitCode::from(EXIT_VALIDATION_FAILED));
        assert!(matches!(
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "unknown".to_string(),
                    status: "draft".to_string(),
                    title: "Bad type".to_string(),
                    summary: None,
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                }
            ),
            Err(storage::NodeStorageError::Validation(
                storage::NodeValidationError::InvalidType(node_type)
            )) if node_type == "unknown"
        ));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn node_create_rejects_invalid_status_via_cli_validation() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("node-invalid-status-home");
        let home = temp_path("node-invalid-status-fallback-home");
        let repo_root = temp_path("node-invalid-status-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "create",
            "--type",
            "decision",
            "--status",
            "unknown",
            "--title",
            "Bad status",
        ])
        .expect("node create should parse");
        let exit_code = run_command(&cli.command, cli.json);
        let (_workspace_key, connection) = open_test_workspace_db();

        assert_eq!(exit_code, ExitCode::from(EXIT_VALIDATION_FAILED));
        assert!(matches!(
            storage::create_node(
                &connection,
                &storage::NewNode {
                    node_type: "decision".to_string(),
                    status: "unknown".to_string(),
                    title: "Bad status".to_string(),
                    summary: None,
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                }
            ),
            Err(storage::NodeStorageError::Validation(
                storage::NodeValidationError::InvalidStatus(status)
            )) if status == "unknown"
        ));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn verify_reports_duplicate_tool_ids_in_dirty_workspace() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("verify-duplicate-id-home");
        let home = temp_path("verify-duplicate-id-fallback-home");
        let repo_root = temp_path("verify-duplicate-id-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");

        seed_dirty_verify_workspace(&repo_root);

        let report = verify::run_lint(&repo_root).expect("lint should succeed");

        assert!(report.issues.iter().any(|issue| {
            issue.kind == verify::LintIssueKind::DuplicateId
                && issue.subject == "tool_contract:dup-tool"
        }));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn verify_reports_broken_links_in_dirty_workspace() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("verify-broken-link-home");
        let home = temp_path("verify-broken-link-fallback-home");
        let repo_root = temp_path("verify-broken-link-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");

        seed_dirty_verify_workspace(&repo_root);

        let report = verify::run_lint(&repo_root).expect("lint should succeed");

        assert!(report.issues.iter().any(|issue| {
            issue.kind == verify::LintIssueKind::BrokenLink
                && issue.subject.starts_with("link:")
                && issue
                    .message
                    .contains("source and target nodes are missing")
        }));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn recall_excludes_deprecated_nodes_from_normal_sections() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("recall-deprecated-home");
        let home = temp_path("recall-deprecated-fallback-home");
        let repo_root = temp_path("recall-deprecated-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);
        install::init_workspace(&repo_root).expect("workspace should initialize");

        let (_workspace_key, connection) = open_test_workspace_db();
        let active_workflow = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "workflow".to_string(),
                status: "active".to_string(),
                title: "Current workflow".to_string(),
                summary: Some("Active path".to_string()),
                body: None,
                source_ref: Some("source=user_instruction".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("active workflow should be created");
        let deprecated_workflow = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "workflow".to_string(),
                status: "deprecated".to_string(),
                title: "Old workflow".to_string(),
                summary: Some("Do not use".to_string()),
                body: None,
                source_ref: Some("source=user_instruction".to_string()),
                confidence: Some(1.0),
                trust_level: Some("high".to_string()),
            },
        )
        .expect("deprecated workflow should be created");
        storage::create_link(
            &connection,
            &storage::NewLink {
                source_node_id: active_workflow.id,
                target_node_id: deprecated_workflow.id,
                link_type: "depends_on".to_string(),
            },
        )
        .expect("link should be created");

        let cli = Cli::try_parse_from(["aopmem", "--json", "recall"]).expect("recall should parse");
        let exit_code = run_command(&cli.command, cli.json);
        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let links = storage::list_links(&connection).expect("links should list");
        let bundle = recall::build_structured_bundle_with_links(nodes, links);

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));
        assert_eq!(bundle.workflows.active.len(), 1);
        assert!(bundle.workflows.deprecated.is_empty());
        assert!(bundle
            .linked_nodes
            .iter()
            .all(|linked| linked.node.status != "deprecated"));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn init_command_is_implemented_for_stage_026() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("home");
        let home = temp_path("fallback-home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let cli = Cli::try_parse_from(["aopmem", "--json", "init"]).expect("init should parse");
        let input = b"no\nno\nMeaning\nRoles\nScope\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();
        let mut observation = CommandObservation::new(command_id(&cli.command), None);
        let exit_code = run_init_with_io(
            command_id(&cli.command),
            cli.json,
            &repo_root,
            &mut reader,
            &mut output,
            &mut observation,
        );

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));
        drop(observation);
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_key = storage::workspace_key(&repo_root).expect("workspace key should build");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let mut statement = observability
            .prepare(
                "SELECT event_type, duration_ms, payload_json, bundle_id \
                 FROM observability_events ORDER BY rowid",
            )
            .expect("install event query should prepare");
        let events = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .expect("install events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("install events should collect");
        assert_eq!(
            events
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.started",
                "workspace.init",
                "audit.snapshot.completed",
                "install.completed",
            ]
        );
        assert!(events[0].1.is_none());
        assert_eq!(events[1].1, events[2].1);
        assert!(events[1].1.is_some());
        assert!(events[1].2.contains("seeded_nodes_created"));
        assert!(events.iter().all(|event| event.3.is_none()));
        for private in ["Meaning", "Roles", "Scope"] {
            assert!(events.iter().all(|event| !event.2.contains(private)));
        }

        drop(statement);
        drop(observability);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn init_output_failure_keeps_workspace_and_records_failed_terminal_event() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("init-output-failure-home");
        let home = temp_path("init-output-failure-fallback-home");
        let repo_root = temp_path("init-output-failure-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let input = b"no\nno\nMeaning\nRoles\nScope\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = FailStyleOutput::default();
        let mut observation = CommandObservation::new("init", None);

        let exit_code = run_init_with_io(
            "init",
            true,
            &repo_root,
            &mut reader,
            &mut output,
            &mut observation,
        );
        assert_eq!(exit_code, ExitCode::from(EXIT_IO_ERROR));
        drop(observation);

        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_key = storage::workspace_key(&repo_root).expect("workspace key should build");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
        let operational = storage::open_workspace_db_read_only(&workspace_paths)
            .expect("committed operational DB should survive output failure");
        let node_count: i64 = operational
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .expect("committed nodes should count");
        assert!(node_count > 0);
        drop(operational);

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let mut statement = observability
            .prepare(
                "SELECT event_type, outcome, error_code, duration_ms \
                 FROM observability_events ORDER BY rowid",
            )
            .expect("install failure events should prepare");
        let events = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .expect("install failure events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("install failure events should collect");
        assert_eq!(
            events
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.started",
                "workspace.init",
                "audit.snapshot.completed",
                "install.failed",
            ]
        );
        assert_eq!(events[1].1, "success");
        assert_eq!(events[2].1, "success");
        assert_eq!(events[3].1, "failure");
        assert_eq!(events[3].2.as_deref(), Some("IO_ERROR"));
        assert!(events[0].3.is_none());
        assert_eq!(events[1].3, events[2].3);
        assert_eq!(events[2].3, events[3].3);

        drop(statement);
        drop(observability);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn init_doctor_and_recall_use_same_git_root_workspace() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("same-workspace-home");
        let home = temp_path("same-workspace-fallback-home");
        let repo_root = temp_path("same-workspace-repo");
        let nested = repo_root.join("src").join("deep");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(repo_root.join(".git")).expect("git dir should be created");
        fs::create_dir_all(&nested).expect("nested dir should be created");
        let _cwd = CurrentDirGuard::set(&nested);

        let cli = Cli::try_parse_from(["aopmem", "--json", "init"]).expect("init should parse");
        let input = b"no\nno\nMeaning\nRoles\nScope\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();
        let mut observation = CommandObservation::new(command_id(&cli.command), None);
        let exit_code = run_init_with_io(
            command_id(&cli.command),
            cli.json,
            &nested,
            &mut reader,
            &mut output,
            &mut observation,
        );
        let workspace_root =
            storage::resolve_current_workspace_root().expect("workspace root should resolve");
        let expected_key =
            storage::workspace_key(&workspace_root).expect("workspace key should resolve");
        let doctor = verify::run_doctor(&workspace_root).expect("doctor should succeed");
        let (recall_key, connection) = open_current_workspace()
            .expect("recall workspace helper should open current workspace");
        let nodes = storage::list_nodes(&connection).expect("recall nodes should list");

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));
        assert_eq!(doctor.workspace_key, expected_key);
        assert_eq!(recall_key, expected_key);
        assert!(!nodes.is_empty());

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn init_writes_sql_snapshot_under_workspace_audit_git() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("init-audit-snapshot-home");
        let home = temp_path("init-audit-snapshot-fallback-home");
        let repo_root = temp_path("init-audit-snapshot-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from(["aopmem", "--json", "init"]).expect("init should parse");
        let input = b"no\nno\nMeaning\nRoles\nScope\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();
        let mut observation = CommandObservation::new(command_id(&cli.command), None);
        let exit_code = run_init_with_io(
            command_id(&cli.command),
            cli.json,
            &repo_root,
            &mut reader,
            &mut output,
            &mut observation,
        );

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));

        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_key =
            storage::workspace_key(&repo_root).expect("workspace key should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        let snapshot_path = workspace_paths.audit_git().join("memory.sql");
        let snapshot_text =
            fs::read_to_string(&snapshot_path).expect("snapshot file should be readable");

        assert!(snapshot_path.starts_with(workspace_paths.audit_git()));
        assert!(snapshot_text.contains("INSERT INTO \"nodes\""));
        assert!(!workspace_paths.audit_git().join("aopmem.sqlite").exists());

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn invalid_utf8_error_envelope_has_required_code_and_hint() {
        let envelope = error_envelope("init", &CliError::invalid_utf8_input());
        let parsed: Value = serde_json::from_str(&envelope).expect("envelope should parse");

        assert_eq!(parsed["errors"][0]["code"], "INVALID_UTF8_INPUT");
        assert_eq!(
            parsed["errors"][0]["fix_hint"],
            "Set PowerShell UTF-8 encoding before piping answers"
        );
    }

    #[test]
    fn artifacts_cleanup_command_creates_today_dir_and_prunes_old_dirs() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("artifacts-cleanup-home");
        let home = temp_path("artifacts-cleanup-fallback-home");
        let repo_root = temp_path("artifacts-cleanup-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let (workspace_key, connection) = open_test_workspace_db();
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, &workspace_key).expect("workspace should exist");
        let old_dir = workspace_paths.artifacts().join("2000-01-01");
        let tools_file = workspace_paths.tools().join("keep.txt");
        fs::create_dir_all(&old_dir).expect("old dir should be created");
        fs::write(old_dir.join("old.txt"), b"old").expect("old artifact should be written");
        fs::write(&tools_file, b"keep").expect("tools file should be written");
        let today: String = connection
            .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))
            .expect("today should resolve");

        let cli = Cli::try_parse_from(["aopmem", "--json", "artifacts", "cleanup"])
            .expect("artifacts cleanup should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));
        assert!(!old_dir.exists());
        assert!(workspace_paths.artifacts().join(today).is_dir());
        assert!(tools_file.is_file());

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn node_create_rolls_back_node_fts_and_event_when_audit_write_fails() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("node-create-rollback-home");
        let home = temp_path("node-create-rollback-fallback-home");
        let repo_root = temp_path("node-create-rollback-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let (_workspace_key, connection) = open_test_workspace_db();
        connection
            .execute_batch(
                "
                CREATE TRIGGER fail_node_create_event
                BEFORE INSERT ON events
                BEGIN
                    SELECT RAISE(ABORT, 'forced node event failure');
                END;
                ",
            )
            .expect("failure trigger should be created");
        let baseline_counts = ["nodes", "fts_nodes", "events"]
            .into_iter()
            .map(|table_name| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table_name};"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("baseline count should query")
            })
            .collect::<Vec<_>>();
        drop(connection);

        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "create",
            "--type",
            "raw_note",
            "--title",
            "rollback-node-proof",
        ])
        .expect("node create should parse");
        assert_eq!(
            run_command(&cli.command, cli.json),
            ExitCode::from(EXIT_DB_SCHEMA_ERROR)
        );

        let (_workspace_key, connection) = open_test_workspace_db();
        let after_counts = ["nodes", "fts_nodes", "events"]
            .into_iter()
            .map(|table_name| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table_name};"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("post-failure count should query")
            })
            .collect::<Vec<_>>();

        assert_eq!(after_counts, baseline_counts);
        assert!(
            storage::list_nodes(&connection)
                .expect("nodes should list after failure")
                .is_empty(),
            "failed node creation must not leave a node behind"
        );

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn node_create_writes_sql_snapshot_under_workspace_audit_git() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("audit-snapshot-home");
        let home = temp_path("audit-snapshot-fallback-home");
        let repo_root = temp_path("audit-snapshot-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "node",
            "create",
            "--type",
            "raw_note",
            "--title",
            "Snapshot node",
        ])
        .expect("node create should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));

        let (_workspace_key, workspace_paths, _connection) =
            open_current_workspace_context().expect("workspace context should open");
        let snapshot_path = workspace_paths.audit_git().join("memory.sql");
        let snapshot_text =
            fs::read_to_string(&snapshot_path).expect("snapshot file should be readable");

        assert!(snapshot_path.starts_with(workspace_paths.audit_git()));
        assert!(snapshot_text.contains("INSERT INTO \"nodes\""));
        assert!(snapshot_text.contains("Snapshot node"));
        assert!(!workspace_paths.audit_git().join("aopmem.sqlite").exists());

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn remember_creates_raw_note_by_default() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("remember-default-home");
        let home = temp_path("remember-default-fallback-home");
        let repo_root = temp_path("remember-default-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from(["aopmem", "--json", "remember", "Keep this in mind"])
            .expect("remember should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));

        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let node = nodes.last().expect("remember node should exist");
        let snapshot_text = fs::read_to_string(workspace_paths.audit_git().join("memory.sql"))
            .expect("snapshot file should be readable");

        assert_eq!(node.node_type, "raw_note");
        assert_eq!(node.status, "draft");
        assert_eq!(node.title, "Keep this in mind");
        assert_eq!(node.body, None);
        assert!(snapshot_text.contains("Keep this in mind"));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn remember_creates_structured_node_when_explicit_fields_are_provided() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("remember-structured-home");
        let home = temp_path("remember-structured-fallback-home");
        let repo_root = temp_path("remember-structured-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let cli = Cli::try_parse_from([
            "aopmem",
            "--json",
            "remember",
            "--type",
            "workflow",
            "--status",
            "active",
            "--title",
            "Release checklist",
            "--summary",
            "Ship workflow",
            "--body",
            "tag, build, publish",
            "--source-ref",
            "source=user_instruction",
            "--confidence",
            "0.95",
            "--trust-level",
            "high",
        ])
        .expect("remember structured node should parse");
        let exit_code = run_command(&cli.command, cli.json);

        assert_eq!(exit_code, ExitCode::from(EXIT_SUCCESS));

        let (_workspace_key, _workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let node = nodes.last().expect("remember node should exist");

        assert_eq!(node.node_type, "workflow");
        assert_eq!(node.status, "active");
        assert_eq!(node.title, "Release checklist");
        assert_eq!(node.summary.as_deref(), Some("Ship workflow"));
        assert_eq!(node.body.as_deref(), Some("tag, build, publish"));
        assert_eq!(node.source_ref.as_deref(), Some("source=user_instruction"));
        assert_eq!(node.confidence, Some(0.95));
        assert_eq!(node.trust_level.as_deref(), Some("high"));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn remember_keeps_raw_note_type_without_hidden_classification() {
        let remember = remember_to_new_node(&RememberArgs {
            note: Some("workflow for deploy hotfix".to_string()),
            node_type: None,
            status: None,
            title: None,
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
        })
        .expect("remember note should map to node");

        assert_eq!(remember.node_type, "raw_note");
        assert_eq!(remember.status, "draft");
        assert_eq!(remember.title, "workflow for deploy hotfix");
    }

    #[test]
    fn teach_flow_stores_session_material_proposal_and_applies_deterministic_data() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("teach-flow-home");
        let home = temp_path("teach-flow-fallback-home");
        let repo_root = temp_path("teach-flow-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let start = Cli::try_parse_from([
            "aopmem",
            "--json",
            "teach",
            "start",
            "--title",
            "Release fixes",
            "--summary",
            "Capture reliable steps",
        ])
        .expect("teach start should parse");
        assert_eq!(
            run_command(&start.command, start.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let mut nodes = storage::list_nodes(&connection).expect("nodes should list");
        let session = nodes.last().expect("teach session should exist").clone();

        let add = Cli::try_parse_from([
            "aopmem",
            "--json",
            "teach",
            "add",
            "--session-id",
            &session.id.to_string(),
            "--payload",
            "{\"kind\":\"note\",\"text\":\"release notes must include rollback\"}",
        ])
        .expect("teach add should parse");
        assert_eq!(
            run_command(&add.command, add.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let propose = Cli::try_parse_from([
            "aopmem",
            "--json",
            "teach",
            "propose",
            "--session-id",
            &session.id.to_string(),
            "--payload",
            "{\"items\":[{\"op\":\"create_node\",\"node_ref\":\"lesson_1\",\"node_type\":\"lesson\",\"status\":\"draft\",\"title\":\"Rollback check\",\"summary\":\"Always write rollback first\"},{\"op\":\"add_alias\",\"node_ref\":\"lesson_1\",\"alias\":\"rollback-first\"},{\"op\":\"add_alias\",\"node_ref\":\"lesson_1\",\"alias\":\"rollback-plan\"},{\"op\":\"add_alias\",\"node_ref\":\"lesson_1\",\"alias\":\"rollback-ready\"},{\"op\":\"add_tag\",\"node_ref\":\"lesson_1\",\"tag\":\"release\"},{\"op\":\"add_source\",\"node_ref\":\"lesson_1\",\"source_ref\":\"source=user_instruction\"}]}",
        ])
        .expect("teach propose should parse");
        assert_eq!(
            run_command(&propose.command, propose.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        nodes = storage::list_nodes(&connection).expect("nodes should list after propose");
        let proposal = nodes.last().expect("teach proposal should exist").clone();

        let apply = Cli::try_parse_from([
            "aopmem",
            "--json",
            "teach",
            "apply",
            "--session-id",
            &session.id.to_string(),
            "--proposal-id",
            &proposal.id.to_string(),
        ])
        .expect("teach apply should parse");
        assert_eq!(
            run_command(&apply.command, apply.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let nodes = storage::list_nodes(&connection).expect("nodes should list after apply");
        let created = nodes
            .iter()
            .find(|node| node.title == "Rollback check")
            .expect("apply should create lesson node");
        let aliases =
            storage::list_aliases(&connection, Some(created.id)).expect("aliases should list");
        let tags = storage::list_tags(&connection, Some(created.id)).expect("tags should list");
        let sources =
            storage::list_sources(&connection, Some(created.id)).expect("sources should list");
        let links = storage::list_links(&connection).expect("links should list");
        let snapshot_text = fs::read_to_string(workspace_paths.audit_git().join("memory.sql"))
            .expect("snapshot file should be readable");

        assert_eq!(session.node_type, "raw_note");
        assert_eq!(session.summary.as_deref(), Some("teach_session_v1"));
        assert_eq!(created.node_type, "lesson");
        assert_eq!(created.status, "draft");
        assert_eq!(
            created.summary.as_deref(),
            Some("Always write rollback first")
        );
        assert_eq!(
            aliases
                .iter()
                .map(|alias| alias.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["rollback-first", "rollback-plan", "rollback-ready"]
        );
        for alias in ["rollback-first", "rollback-plan", "rollback-ready"] {
            let matches = storage::search_nodes_fts(&connection, alias, 5)
                .expect("alias FTS search should pass");
            assert_eq!(
                matches
                    .iter()
                    .filter(|result| result.node.id == created.id)
                    .count(),
                1,
                "alias should be searchable after batch refresh"
            );
        }
        assert_eq!(tags[0].tag, "release");
        assert_eq!(sources[0].source_ref, "source=user_instruction");
        assert!(links.iter().any(|link| link.source_node_id == session.id));
        assert!(snapshot_text.contains("teach_session_v1"));
        assert!(snapshot_text.contains("Rollback check"));

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let mut statement = observability
            .prepare(
                "SELECT event_type, correlation_id, payload_json, bundle_id \
                 FROM observability_events ORDER BY rowid",
            )
            .expect("teach event query should prepare");
        let events = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .expect("teach events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("teach events should collect");
        assert_eq!(
            events
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            vec![
                "teach.started",
                "audit.snapshot.completed",
                "audit.snapshot.completed",
                "teach.proposed",
                "audit.snapshot.completed",
                "teach.applied",
                "audit.snapshot.completed",
            ]
        );
        assert_eq!(
            events
                .iter()
                .map(|event| event.1.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            4
        );
        assert!(events.iter().all(|event| event.3.is_none()));
        assert!(events
            .iter()
            .all(|event| !event.2.contains("release notes must include rollback")));

        drop(statement);
        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn teach_apply_rolls_back_all_database_changes_after_duplicate_alias_failure() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("teach-apply-rollback-home");
        let home = temp_path("teach-apply-rollback-fallback-home");
        let repo_root = temp_path("teach-apply-rollback-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let start = Cli::try_parse_from([
            "aopmem",
            "--json",
            "teach",
            "start",
            "--title",
            "Rollback lesson",
        ])
        .expect("teach start should parse");
        assert_eq!(
            run_command(&start.command, start.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (_workspace_key, _workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let session_id = storage::list_nodes(&connection)
            .expect("nodes should list")
            .last()
            .expect("teach session should exist")
            .id;
        drop(connection);

        let propose = Cli::try_parse_from([
            "aopmem",
            "--json",
            "teach",
            "propose",
            "--session-id",
            &session_id.to_string(),
            "--payload",
            "{\"items\":[{\"op\":\"create_node\",\"node_ref\":\"lesson_rollback\",\"node_type\":\"lesson\",\"status\":\"draft\",\"title\":\"rollbackproofnode\"},{\"op\":\"add_alias\",\"node_ref\":\"lesson_rollback\",\"alias\":\"rollback-alias\"},{\"op\":\"add_alias\",\"node_ref\":\"lesson_rollback\",\"alias\":\"rollback-alias\"}]}",
        ])
        .expect("teach proposal should parse");
        assert_eq!(
            run_command(&propose.command, propose.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (_workspace_key, _workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let proposal_id = storage::list_nodes(&connection)
            .expect("nodes should list")
            .last()
            .expect("teach proposal should exist")
            .id;
        let baseline_counts = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .into_iter()
            .map(|table_name| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table_name};"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("baseline table count should query")
            })
            .collect::<Vec<_>>();
        let baseline_fts_matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM fts_nodes WHERE fts_nodes MATCH 'rollbackproofnode';",
                [],
                |row| row.get(0),
            )
            .expect("baseline FTS count should query");
        drop(connection);

        let apply = Cli::try_parse_from([
            "aopmem",
            "--json",
            "teach",
            "apply",
            "--session-id",
            &session_id.to_string(),
            "--proposal-id",
            &proposal_id.to_string(),
        ])
        .expect("teach apply should parse");
        assert_eq!(
            run_command(&apply.command, apply.json),
            ExitCode::from(EXIT_DB_SCHEMA_ERROR)
        );

        let (_workspace_key, _workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let after_counts = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .into_iter()
            .map(|table_name| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table_name};"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("post-failure table count should query")
            })
            .collect::<Vec<_>>();
        let after_fts_matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM fts_nodes WHERE fts_nodes MATCH 'rollbackproofnode';",
                [],
                |row| row.get(0),
            )
            .expect("post-failure FTS count should query");

        assert_eq!(after_counts, baseline_counts);
        assert_eq!(after_fts_matches, baseline_fts_matches);
        assert!(
            storage::list_nodes(&connection)
                .expect("nodes should list after failed apply")
                .iter()
                .all(|node| node.title != "rollbackproofnode"),
            "failed apply must not leave the created node behind"
        );
        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn reflect_apply_rolls_back_all_database_changes_after_duplicate_alias_failure() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("reflect-apply-rollback-home");
        let home = temp_path("reflect-apply-rollback-fallback-home");
        let repo_root = temp_path("reflect-apply-rollback-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let proposal_path = repo_root.join("reflection-rollback-proposal.json");
        fs::write(
            &proposal_path,
            r#"{"items":[{"op":"create_node","risk":"low","node_ref":"lesson_rollback","node_type":"lesson","status":"draft","title":"reflectionrollbackproofnode"},{"op":"add_alias","risk":"low","node_ref":"lesson_rollback","alias":"reflection-rollback-alias"},{"op":"add_alias","risk":"low","node_ref":"lesson_rollback","alias":"reflection-rollback-alias"}]}"#,
        )
        .expect("proposal file should be written");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "reflect",
            "proposal",
            "create",
            "--session-id",
            "codex-chat-rollback",
            "--proposal-file",
            proposal_path
                .to_str()
                .expect("proposal path should be utf-8"),
        ])
        .expect("reflection proposal create should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let proposal_id = storage::list_nodes(&connection)
            .expect("nodes should list")
            .last()
            .expect("reflection proposal should exist")
            .id;
        let baseline_counts = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .into_iter()
            .map(|table_name| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table_name};"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("baseline table count should query")
            })
            .collect::<Vec<_>>();
        let baseline_fts_matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM fts_nodes WHERE fts_nodes MATCH 'reflectionrollbackproofnode';",
                [],
                |row| row.get(0),
            )
            .expect("baseline FTS count should query");
        drop(connection);

        let apply = Cli::try_parse_from([
            "aopmem",
            "--json",
            "reflect",
            "proposal",
            "apply",
            "--proposal-id",
            &proposal_id.to_string(),
        ])
        .expect("reflection proposal apply should parse");
        assert_eq!(
            run_command(&apply.command, apply.json),
            ExitCode::from(EXIT_DB_SCHEMA_ERROR)
        );

        let (_workspace_key, _workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let after_counts = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .into_iter()
            .map(|table_name| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table_name};"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("post-failure table count should query")
            })
            .collect::<Vec<_>>();
        let after_fts_matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM fts_nodes WHERE fts_nodes MATCH 'reflectionrollbackproofnode';",
                [],
                |row| row.get(0),
            )
            .expect("post-failure FTS count should query");

        assert_eq!(&after_counts[..5], &baseline_counts[..5]);
        assert_eq!(after_counts[5], baseline_counts[5] + 1);
        assert_eq!(after_fts_matches, baseline_fts_matches);
        assert!(
            storage::list_nodes(&connection)
                .expect("nodes should list after failed apply")
                .iter()
                .all(|node| node.title != "reflectionrollbackproofnode"),
            "failed apply must not leave the created node behind"
        );
        let failed_events = audit::list_events(&connection)
            .expect("events should list")
            .into_iter()
            .filter(|event| event.event_type == audit::REFLECTION_APPLY_FAILED_EVENT)
            .collect::<Vec<_>>();
        assert_eq!(failed_events.len(), 1);
        assert_eq!(failed_events[0].subject_id, proposal_id);
        let snapshot = fs::read_to_string(workspace_paths.audit_git().join("memory.sql"))
            .expect("failed-attempt snapshot should exist");
        assert!(snapshot.contains(audit::REFLECTION_APPLY_FAILED_EVENT));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn reflect_inventory_stores_inventory_record_and_tracks_prior_session_ids() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("reflect-inventory-home");
        let home = temp_path("reflect-inventory-fallback-home");
        let repo_root = temp_path("reflect-inventory-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Reflection material".to_string(),
                summary: Some(reflection::REFLECTION_MATERIAL_SUMMARY.to_string()),
                body: Some("{\"session_id\":\"codex-chat-41\"}".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("reflection material should be created");

        let inventory = Cli::try_parse_from(["aopmem", "--json", "reflect", "inventory"])
            .expect("reflect inventory should parse");
        assert_eq!(
            run_command(&inventory.command, inventory.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let inventory_node = nodes.last().expect("inventory node should exist");
        let sessions = reflection::list_reflected_sessions(&connection)
            .expect("reflected sessions should list");
        let snapshot_text = fs::read_to_string(workspace_paths.audit_git().join("memory.sql"))
            .expect("snapshot file should be readable");

        assert_eq!(
            inventory_node.summary.as_deref(),
            Some(reflection::REFLECTION_INVENTORY_SUMMARY)
        );
        assert!(inventory_node
            .body
            .as_deref()
            .expect("inventory body should exist")
            .contains("codex-chat-41"));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "codex-chat-41");
        assert!(snapshot_text.contains(reflection::REFLECTION_INVENTORY_SUMMARY));
        assert!(snapshot_text.contains("codex-chat-41"));

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let (event_type, outcome, payload, bundle_id): (String, String, String, Option<String>) =
            observability
                .query_row(
                    "SELECT event_type, outcome, payload_json, bundle_id \
                 FROM observability_events",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("inventory event should query");
        assert_eq!(event_type, "reflection.inventory");
        assert_eq!(outcome, "success");
        assert!(!payload.contains("codex-chat-41"));
        assert!(bundle_id.is_none());

        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn reflect_proposal_create_stores_file_backed_schema_record() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("reflect-proposal-home");
        let home = temp_path("reflect-proposal-fallback-home");
        let repo_root = temp_path("reflect-proposal-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let proposal_path = repo_root.join("reflection-proposal.json");
        fs::write(
            &proposal_path,
            r#"{"items":[{"op":"create_node","risk":"low","node_ref":"lesson_1","node_type":"lesson","status":"draft","title":"Write recovery note","summary":"Keep fix steps"},{"op":"update_node_body","risk":"high","node_id":7,"body":"rewrite active workflow"}]}"#,
        )
        .expect("proposal file should be written");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "reflect",
            "proposal",
            "create",
            "--session-id",
            "codex-chat-42",
            "--proposal-file",
            proposal_path
                .to_str()
                .expect("proposal path should be utf-8"),
        ])
        .expect("reflect proposal create should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let proposal_node = nodes.last().expect("proposal node should exist");
        let sessions = reflection::list_reflected_sessions(&connection)
            .expect("reflected sessions should list");
        let snapshot_text = fs::read_to_string(workspace_paths.audit_git().join("memory.sql"))
            .expect("snapshot file should be readable");

        assert_eq!(
            proposal_node.summary.as_deref(),
            Some(reflection::REFLECTION_PROPOSAL_SUMMARY)
        );
        assert!(proposal_node
            .body
            .as_deref()
            .expect("proposal body should exist")
            .contains("\"risk\":\"high\""));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "codex-chat-42");
        assert!(snapshot_text.contains(reflection::REFLECTION_PROPOSAL_SUMMARY));
        assert!(snapshot_text.contains("codex-chat-42"));

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn reflect_proposal_apply_auto_applies_low_risk_and_keeps_high_risk_draft() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("reflect-apply-home");
        let home = temp_path("reflect-apply-fallback-home");
        let repo_root = temp_path("reflect-apply-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let proposal_path = repo_root.join("reflection-apply-proposal.json");
        fs::write(
            &proposal_path,
            r#"{"items":[{"op":"create_node","risk":"low","node_ref":"lesson_1","node_type":"lesson","status":"draft","title":"Apply lesson","summary":"Keep the fix path"},{"op":"add_alias","risk":"low","node_ref":"lesson_1","alias":"apply-note"},{"op":"update_node_body","risk":"high","node_id":9,"body":"rewrite active workflow"}]}"#,
        )
        .expect("proposal file should be written");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "reflect",
            "proposal",
            "create",
            "--session-id",
            "codex-chat-43",
            "--proposal-file",
            proposal_path
                .to_str()
                .expect("proposal path should be utf-8"),
        ])
        .expect("reflect proposal create should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace context should open");
        let proposal_id = storage::list_nodes(&connection)
            .expect("nodes should list")
            .last()
            .expect("proposal node should exist")
            .id;
        let apply = Cli::try_parse_from([
            "aopmem",
            "--json",
            "reflect",
            "proposal",
            "apply",
            "--proposal-id",
            &proposal_id.to_string(),
        ])
        .expect("reflect proposal apply should parse");
        assert_eq!(
            run_command(&apply.command, apply.json),
            ExitCode::from(EXIT_SUCCESS)
        );

        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let created_node = nodes
            .iter()
            .find(|node| node.title == "Apply lesson")
            .expect("low-risk node should be created");
        let aliases =
            storage::list_aliases(&connection, Some(created_node.id)).expect("aliases should list");
        let apply_record = nodes.last().expect("apply record should exist");
        let snapshot_text = fs::read_to_string(workspace_paths.audit_git().join("memory.sql"))
            .expect("snapshot file should be readable");

        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].alias, "apply-note");
        assert_eq!(
            apply_record.summary.as_deref(),
            Some(reflection::REFLECTION_APPLY_SUMMARY)
        );
        assert!(apply_record
            .body
            .as_deref()
            .expect("apply body should exist")
            .contains("\"high_risk_item\""));
        assert!(snapshot_text.contains(reflection::REFLECTION_APPLY_SUMMARY));
        assert!(snapshot_text.contains("codex-chat-43"));

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open");
        let mut statement = observability
            .prepare(
                "SELECT event_type, outcome, payload_json, correlation_id, duration_ms, bundle_id \
                 FROM observability_events \
                 WHERE event_type IN ('reflection.proposal', 'reflection.applied') \
                 ORDER BY rowid",
            )
            .expect("reflection event query should prepare");
        let events = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .expect("reflection events should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("reflection events should collect");

        assert_eq!(events.len(), 3);
        assert_eq!(
            (&events[0].0, &events[0].1),
            (&"reflection.proposal".to_string(), &"proposed".to_string())
        );
        assert_eq!(
            (&events[1].0, &events[1].1),
            (&"reflection.applied".to_string(), &"applied".to_string())
        );
        assert_eq!(
            (&events[2].0, &events[2].1),
            (&"reflection.applied".to_string(), &"drafted".to_string())
        );
        let proposal_payload: Value =
            serde_json::from_str(&events[0].2).expect("proposal payload should parse");
        let applied_payload: Value =
            serde_json::from_str(&events[1].2).expect("applied payload should parse");
        let drafted_payload: Value =
            serde_json::from_str(&events[2].2).expect("drafted payload should parse");
        assert_eq!(proposal_payload["data"]["items"][0]["count"], 3);
        assert_eq!(applied_payload["data"]["items"][0]["count"], 2);
        assert_eq!(drafted_payload["data"]["items"][0]["count"], 1);
        assert_eq!(events[1].3, events[2].3);
        assert_eq!(events[1].4, events[2].4);
        assert!(events.iter().all(|event| event.4.is_some()));
        assert!(events.iter().all(|event| event.5.is_none()));
        assert!(events
            .iter()
            .all(|event| !event.2.contains("rewrite active workflow")));

        drop(statement);
        drop(observability);
        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn snapshot_pending_records_failed_then_pending_with_one_frozen_duration() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("snapshot-pending-observation-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = storage::resolve_paths().expect("paths should resolve");
        storage::ensure_global_dirs(&paths).expect("global dirs should create");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "snapshot-observation")
            .expect("workspace dirs should create");
        let mut observation = CommandObservation::new("node_create", None);
        observation.attach_workspace(&workspace_paths);

        record_snapshot_observation(
            &mut observation,
            mutation::SnapshotObservation::Pending { duration_ms: 17 },
        );
        drop(observation);

        let events = observed_command_events(&workspace_paths, "node_create");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "audit.snapshot.failed");
        assert_eq!(events[0].outcome, "failure");
        assert_eq!(
            events[0].error_code.as_deref(),
            Some(AUDIT_SNAPSHOT_FAILED_ERROR_CODE)
        );
        assert_eq!(events[1].event_type, "audit.snapshot.pending");
        assert_eq!(events[1].outcome, "pending");
        assert_eq!(
            events[1].error_code.as_deref(),
            Some(mutation::AUDIT_SNAPSHOT_PENDING)
        );
        assert_eq!(events[0].correlation_id, events[1].correlation_id);
        assert_eq!(events[0].duration_ms, events[1].duration_ms);
        assert!(events.iter().all(|event| event.bundle_id.is_none()));
        for event in &events {
            let payload: Value =
                serde_json::from_str(&event.payload_json).expect("snapshot payload should parse");
            assert_eq!(payload["kind"], "counts");
            assert_eq!(payload["data"]["items"][0]["name"], "duration_ms");
            assert_eq!(payload["data"]["items"][0]["count"], 17);
            assert!(!event.payload_json.contains("path"));
            assert!(!event.payload_json.contains("error"));
        }

        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
    }

    #[cfg(unix)]
    #[test]
    fn tool_dry_run_emits_only_validation_and_real_spawn_has_one_terminal() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-observation-home");
        let home = temp_path("tool-observation-fallback-home");
        let repo_root = temp_path("tool-observation-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);

        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "observed-tool",
            "--name",
            "Observed Tool",
        ])
        .expect("tool create should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        drop(connection);
        let tool_root = tools::tool_dir(&workspace_paths, "observed-tool");
        let executable = tool_root.join("bin/observed-tool");
        let marker = tool_root.join("runtime/spawned");
        write_executable(
            &executable,
            "#!/bin/sh\nprintf 'RAW_TOOL_OUTPUT_CANARY\\n'\ntouch runtime/spawned\n",
        );

        let dry_run = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "run",
            "observed-tool",
            "--dry-run",
            "--",
            "--token=ARG_SECRET_CANARY",
        ])
        .expect("dry run should parse");
        assert_eq!(
            run_command(&dry_run.command, dry_run.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert!(!marker.exists(), "dry-run must not spawn the tool");

        let real_run = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "run",
            "observed-tool",
            "--",
            "ARG_SECRET_CANARY",
        ])
        .expect("real run should parse");
        assert_eq!(
            run_command(&real_run.command, real_run.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert!(marker.is_file(), "real run must execute the tool");

        let events = observed_command_events(&workspace_paths, "tool_run");
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec![
                "tool.validation",
                "tool.validation",
                "tool.run.started",
                "tool.run.completed",
            ]
        );
        assert_ne!(events[0].correlation_id, events[1].correlation_id);
        assert_eq!(events[1].correlation_id, events[2].correlation_id);
        assert_eq!(events[2].correlation_id, events[3].correlation_id);
        assert!(events[2].duration_ms.is_none());
        assert_eq!(events[1].duration_ms, events[3].duration_ms);
        assert!(events.iter().all(|event| event.bundle_id.is_none()));
        let payloads = events
            .iter()
            .map(|event| event.payload_json.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(payloads.contains("observed-tool"));
        assert!(payloads.contains("approval_present"));
        for forbidden in [
            "ARG_SECRET_CANARY",
            "RAW_TOOL_OUTPUT_CANARY",
            "stdout",
            "stderr",
            "executable",
            "runtime/spawned",
        ] {
            assert!(!payloads.contains(forbidden), "payload leaked {forbidden}");
        }

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[cfg(unix)]
    #[test]
    fn tool_validation_failure_has_no_fake_run_start() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-no-spawn-home");
        let home = temp_path("tool-no-spawn-fallback-home");
        let repo_root = temp_path("tool-no-spawn-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let create = Cli::try_parse_from([
            "aopmem",
            "--json",
            "tool",
            "create-draft",
            "--id",
            "missing-executable",
            "--name",
            "Missing Executable",
        ])
        .expect("tool create should parse");
        assert_eq!(
            run_command(&create.command, create.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        drop(connection);

        let run = Cli::try_parse_from(["aopmem", "--json", "tool", "run", "missing-executable"])
            .expect("tool run should parse");
        assert_eq!(
            run_command(&run.command, run.json),
            ExitCode::from(EXIT_VALIDATION_FAILED)
        );

        let events = observed_command_events(&workspace_paths, "tool_run");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "tool.validation");
        assert_eq!(events[0].outcome, "failure");
        assert_eq!(events[0].error_code.as_deref(), Some("VALIDATION_ERROR"));

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[cfg(unix)]
    #[test]
    fn tool_timeout_output_overflow_and_artifact_events_are_exact_and_private() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("tool-terminal-observation-home");
        let home = temp_path("tool-terminal-observation-fallback-home");
        let repo_root = temp_path("tool-terminal-observation-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);

        for args in [
            vec![
                "aopmem",
                "--json",
                "tool",
                "create-draft",
                "--id",
                "timeout-tool",
                "--name",
                "Timeout Tool",
                "--timeout-ms",
                "30",
            ],
            vec![
                "aopmem",
                "--json",
                "tool",
                "create-draft",
                "--id",
                "overflow-tool",
                "--name",
                "Overflow Tool",
                "--stdout-limit-bytes",
                "8",
            ],
            vec![
                "aopmem",
                "--json",
                "tool",
                "create-draft",
                "--id",
                "artifact-tool",
                "--name",
                "Artifact Tool",
                "--stdout-limit-bytes",
                "8",
                "--stderr-limit-bytes",
                "8",
                "--output-mode",
                "artifact",
            ],
        ] {
            let create = Cli::try_parse_from(args).expect("tool create should parse");
            assert_eq!(
                run_command(&create.command, create.json),
                ExitCode::from(EXIT_SUCCESS)
            );
        }
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        drop(connection);
        write_executable(
            &tools::tool_dir(&workspace_paths, "timeout-tool").join("bin/timeout-tool"),
            "#!/bin/sh\nsleep 1\n",
        );
        write_executable(
            &tools::tool_dir(&workspace_paths, "overflow-tool").join("bin/overflow-tool"),
            "#!/bin/sh\nprintf 'OUTPUT_SECRET_CANARY'\n",
        );
        write_executable(
            &tools::tool_dir(&workspace_paths, "artifact-tool").join("bin/artifact-tool"),
            "#!/bin/sh\nprintf 'ARTIFACT_SECRET_CANARY'\n",
        );

        for (tool_id, expected_exit) in [
            ("timeout-tool", EXIT_GENERIC_ERROR),
            ("overflow-tool", EXIT_GENERIC_ERROR),
            ("artifact-tool", EXIT_SUCCESS),
        ] {
            let run = Cli::try_parse_from(["aopmem", "--json", "tool", "run", tool_id])
                .expect("tool run should parse");
            assert_eq!(
                run_command(&run.command, run.json),
                ExitCode::from(expected_exit)
            );
        }

        let events = observed_command_events(&workspace_paths, "tool_run");
        let event_types = events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![
                "tool.validation",
                "tool.run.started",
                "tool.run.timeout",
                "tool.validation",
                "tool.run.started",
                "tool.run.failed",
                "tool.validation",
                "tool.run.started",
                "tool.run.completed",
                "tool.output.artifact",
                "tool.output.artifact",
            ]
        );
        assert_eq!(events[2].outcome, "timeout");
        assert_eq!(events[2].error_code.as_deref(), Some("TOOL_TIMEOUT"));
        assert_eq!(
            events[5].error_code.as_deref(),
            Some("TOOL_OUTPUT_OVERFLOW")
        );
        let artifact_payloads = [&events[9], &events[10]];
        assert!(artifact_payloads.iter().all(|event| {
            event.payload_json.contains("artifacts/")
                && event.payload_json.contains("bytes")
                && !event.payload_json.contains("preview")
        }));
        let payloads = events
            .iter()
            .map(|event| event.payload_json.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!payloads.contains("OUTPUT_SECRET_CANARY"));
        assert!(!payloads.contains("ARTIFACT_SECRET_CANARY"));
        assert!(events.iter().all(|event| event.bundle_id.is_none()));

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn cleanup_partial_counts_have_exact_keys_and_never_store_paths() {
        let report = artifacts::CleanupReport {
            artifact_root: "/workspace/ARTIFACT_ROOT_CANARY".to_string(),
            today_dir: "/workspace/TODAY_CANARY".to_string(),
            bytes_before: 19,
            bytes_after: 7,
            deleted_dirs: vec!["DELETED_DIR_CANARY".to_string()],
            deleted_files: vec!["DELETED_FILE_CANARY".to_string()],
            kept_dirs: vec!["KEPT_DIR_CANARY".to_string()],
            deleted_paths: vec!["DELETED_PATH_CANARY".to_string()],
            complete: false,
        };
        let counts = artifact_cleanup_counts(Some(&report), Some(&report.deleted_paths))
            .expect("cleanup counts should build");

        assert_eq!(
            counts,
            vec![
                ("bytes_before", 19),
                ("bytes_after", 7),
                ("deleted_dirs", 1),
                ("deleted_files", 1),
                ("deleted_paths", 1),
                ("kept_dirs", 1),
                ("complete", 0),
            ]
        );
        let serialized = serde_json::to_string(&counts).expect("counts should serialize");
        for forbidden in [
            "ARTIFACT_ROOT_CANARY",
            "TODAY_CANARY",
            "DELETED_DIR_CANARY",
            "DELETED_FILE_CANARY",
            "DELETED_PATH_CANARY",
            "KEPT_DIR_CANARY",
        ] {
            assert!(!serialized.contains(forbidden));
        }

        let result = Err(artifacts::ArtifactError::CleanupPartial {
            failed_path: "FAILED_PATH_CANARY".to_string(),
            report: Box::new(report),
            source: io::Error::other("CLEANUP_ERROR_CANARY"),
        });
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("cleanup-partial-observation-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let paths = storage::resolve_paths().expect("paths should resolve");
        storage::ensure_global_dirs(&paths).expect("global dirs should create");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "cleanup-partial")
            .expect("workspace dirs should create");
        let mut observation = CommandObservation::new("artifacts_cleanup", None);
        observation.attach_workspace(&workspace_paths);
        observation.record_terminal(artifact_cleanup_observation_event(&result));
        drop(observation);
        let events = observed_command_events(&workspace_paths, "artifacts_cleanup");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "artifacts.cleanup");
        assert_eq!(events[0].outcome, "warning");
        assert_eq!(
            events[0].error_code.as_deref(),
            Some("ARTIFACT_CLEANUP_PARTIAL")
        );
        for forbidden in [
            "FAILED_PATH_CANARY",
            "CLEANUP_ERROR_CANARY",
            "DELETED_PATH_CANARY",
        ] {
            assert!(!events[0].payload_json.contains(forbidden));
        }
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
    }

    #[test]
    fn mcp_list_records_exact_aggregate_and_not_found_does_not_fake_missing() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("mcp-observation-home");
        let home = temp_path("mcp-observation-fallback-home");
        let repo_root = temp_path("mcp-observation-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (_workspace_key, workspace_paths, connection) =
            open_current_workspace_context().expect("workspace should open");
        for (id, status) in [
            ("сервер-東京", "missing"),
            ("installed-server", "installed"),
            ("unverified-server", "configured_unverified"),
            ("disabled-server", "disabled"),
        ] {
            storage::create_mcp_profile(
                &connection,
                &storage::NewMcpProfile {
                    id: id.to_string(),
                    name: id.to_string(),
                    kind: "optional".to_string(),
                    status: status.to_string(),
                    read_operations: "read".to_string(),
                    write_operations: "none".to_string(),
                    side_effects: "local_read".to_string(),
                    approval_requirement: "none".to_string(),
                    credentials_source: None,
                    notes: Some("MCP_NOTES_CANARY".to_string()),
                },
            )
            .expect("MCP profile should create");
        }
        drop(connection);

        let list = Cli::try_parse_from(["aopmem", "--json", "mcp", "list"])
            .expect("MCP list should parse");
        assert_eq!(
            run_command(&list.command, list.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        let get = Cli::try_parse_from(["aopmem", "--json", "mcp", "get", "--id", "сервер-東京"])
            .expect("MCP get should parse");
        assert_eq!(
            run_command(&get.command, get.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        let not_found = Cli::try_parse_from([
            "aopmem",
            "--json",
            "mcp",
            "get",
            "--id",
            "not-found-profile",
        ])
        .expect("missing MCP get should parse");
        assert_eq!(
            run_command(&not_found.command, not_found.json),
            ExitCode::from(EXIT_GENERIC_ERROR)
        );

        let list_events = observed_command_events(&workspace_paths, "mcp_list");
        assert_eq!(list_events.len(), 1);
        assert_eq!(list_events[0].event_type, "mcp.status");
        assert_eq!(list_events[0].outcome, "success");
        let aggregate: Value =
            serde_json::from_str(&list_events[0].payload_json).expect("MCP aggregate should parse");
        let items = aggregate["data"]["items"]
            .as_array()
            .expect("MCP aggregate items should be an array");
        let counts = items
            .iter()
            .map(|item| {
                (
                    item["name"].as_str().expect("count name").to_string(),
                    item["count"].as_u64().expect("count value"),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(counts.get("profiles"), Some(&4));
        assert_eq!(counts.get("installed"), Some(&1));
        assert_eq!(counts.get("missing"), Some(&1));
        assert_eq!(counts.get("configured_unverified"), Some(&1));
        assert_eq!(counts.get("unrecognized"), Some(&1));
        assert_eq!(counts.get("more_results"), Some(&0));
        assert_eq!(counts.len(), 6);

        let get_events = observed_command_events(&workspace_paths, "mcp_get");
        assert_eq!(get_events.len(), 1, "not-found must not emit fake missing");
        assert_eq!(get_events[0].outcome, "missing");
        assert!(get_events[0].payload_json.contains("сервер-東京"));
        assert!(!get_events[0].payload_json.contains("not-found-profile"));
        assert!(!get_events[0].payload_json.contains("MCP_NOTES_CANARY"));

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn doctor_and_verify_record_typed_counts_without_issue_text_or_paths() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("health-observation-home");
        let home = temp_path("health-observation-fallback-home");
        let repo_root = temp_path("HEALTH_PATH_CANARY");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        install::init_workspace(&repo_root).expect("workspace should initialize");
        adapter::seed_instruction_file(&repo_root.join("AGENTS.md"))
            .expect("adapter block should seed");
        let _cwd = CurrentDirGuard::set(&repo_root);

        for args in [
            ["aopmem", "--json", "doctor"].as_slice(),
            ["aopmem", "--json", "verify"].as_slice(),
        ] {
            let cli = Cli::try_parse_from(args).expect("health command should parse");
            assert_eq!(
                run_command(&cli.command, cli.json),
                ExitCode::from(EXIT_SUCCESS)
            );
        }
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_key = storage::workspace_key(&repo_root).expect("workspace key should build");
        let workspace_paths = storage::workspace_paths_for_key(&paths, workspace_key);

        fs::write(repo_root.join("AGENTS.md"), "HEALTH_ISSUE_TEXT_CANARY\n")
            .expect("adapter drift fixture should write");
        let doctor_warning =
            Cli::try_parse_from(["aopmem", "--json", "doctor"]).expect("doctor should parse");
        assert_eq!(
            run_command(&doctor_warning.command, doctor_warning.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        fs::write(
            workspace_paths
                .audit_git()
                .join(audit::PENDING_SNAPSHOT_MARKER_FILE_NAME),
            b"pending\n",
        )
        .expect("pending audit fixture should write");
        let verify_warning =
            Cli::try_parse_from(["aopmem", "--json", "verify"]).expect("verify should parse");
        assert_eq!(
            run_command(&verify_warning.command, verify_warning.json),
            ExitCode::from(EXIT_DRIFT_DETECTED)
        );

        fs::write(workspace_paths.db(), b"CORRUPT_DB_CANARY")
            .expect("corrupt DB fixture should write");
        let verify_failure =
            Cli::try_parse_from(["aopmem", "--json", "verify"]).expect("verify should parse");
        assert_eq!(
            run_command(&verify_failure.command, verify_failure.json),
            ExitCode::from(EXIT_DB_SCHEMA_ERROR)
        );

        let doctor = observed_command_events(&workspace_paths, "doctor");
        let verify = observed_command_events(&workspace_paths, "verify");
        assert_eq!(doctor.len(), 2);
        assert_eq!(verify.len(), 3);
        assert_eq!(doctor[0].outcome, "success");
        assert_eq!(doctor[1].outcome, "warning");
        assert_eq!(verify[0].outcome, "success");
        assert_eq!(verify[1].outcome, "warning");
        assert_eq!(verify[2].outcome, "failure");
        assert_eq!(verify[2].error_code.as_deref(), Some("DB_SCHEMA_ERROR"));
        for (event, expected_keys) in [
            (&doctor[0], vec!["checks", "ready", "missing", "error"]),
            (
                &verify[0],
                vec![
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
                ],
            ),
        ] {
            let payload: Value =
                serde_json::from_str(&event.payload_json).expect("health payload should parse");
            let keys = payload["data"]["items"]
                .as_array()
                .expect("health counts should be an array")
                .iter()
                .map(|item| item["name"].as_str().expect("health count name"))
                .collect::<Vec<_>>();
            assert_eq!(keys, expected_keys);
            assert!(!event.payload_json.contains("HEALTH_PATH_CANARY"));
            assert!(!event.payload_json.contains("issue"));
            assert!(event.bundle_id.is_none());
        }
        for event in doctor.iter().chain(&verify) {
            for forbidden in [
                "HEALTH_PATH_CANARY",
                "HEALTH_ISSUE_TEXT_CANARY",
                "CORRUPT_DB_CANARY",
            ] {
                assert!(!event.payload_json.contains(forbidden));
            }
        }

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn fixed_exit_codes_match_cli_contract() {
        assert_eq!(EXIT_SUCCESS, 0);
        assert_eq!(EXIT_GENERIC_ERROR, 1);
        assert_eq!(EXIT_INVALID_ARGS, 2);
        assert_eq!(EXIT_WORKSPACE_NOT_FOUND, 3);
        assert_eq!(EXIT_DB_SCHEMA_ERROR, 4);
        assert_eq!(EXIT_VALIDATION_FAILED, 5);
        assert_eq!(EXIT_UNSAFE_ACTION_BLOCKED, 6);
        assert_eq!(EXIT_NOT_IMPLEMENTED, 7);
        assert_eq!(EXIT_DRIFT_DETECTED, 8);
        assert_eq!(EXIT_IO_ERROR, 9);
    }

    #[test]
    fn upgrade_plan_cli_requires_all_workspaces_and_keeps_missing_home_absent() {
        let parsed =
            Cli::try_parse_from(["aopmem", "upgrade", "plan", "--all-workspaces", "--json"])
                .expect("upgrade plan should parse");
        assert!(parsed.json);
        assert_eq!(command_id(&parsed.command), "upgrade_plan");
        assert!(matches!(
            parsed.command,
            Command::Upgrade {
                command: UpgradeCommand::Plan(UpgradePlanArgs {
                    all_workspaces: true
                })
            }
        ));
        assert!(Cli::try_parse_from(["aopmem", "upgrade", "plan", "--json"]).is_err());
        let apply =
            Cli::try_parse_from(["aopmem", "upgrade", "apply", "--all-workspaces", "--json"])
                .expect("upgrade apply should parse");
        assert!(apply.json);
        assert_eq!(command_id(&apply.command), "upgrade_apply");
        assert!(matches!(
            apply.command,
            Command::Upgrade {
                command: UpgradeCommand::Apply(UpgradePlanArgs {
                    all_workspaces: true
                })
            }
        ));
        assert!(Cli::try_parse_from(["aopmem", "upgrade", "apply", "--json"]).is_err());

        let _lock = install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let override_home = temp_path("upgrade-plan-missing-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        assert_eq!(
            run_command(&parsed.command, parsed.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert!(
            !override_home.exists(),
            "read-only upgrade plan must not create AOPMEM_HOME"
        );
    }

    #[test]
    fn observe_json_commands_parse_exactly_and_never_self_observe_or_touch_memory() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("observe-read-only-home");
        let repo_root = temp_path("observe-read-only-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (workspace_key, operational) = open_test_workspace_db();
        let operational_schema_before: i64 = operational
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("operational schema version should read");
        drop(operational);
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
        drop(
            crate::observability::open_writer(&workspace_paths)
                .expect("observability store should initialize"),
        );
        let operational_before = file_fingerprint(workspace_paths.db());
        let observability_before = file_fingerprint(workspace_paths.observability_db());

        let status = Cli::try_parse_from(["aopmem", "--json", "observe", "status"])
            .expect("observe status --json should parse");
        assert!(matches!(
            status.command,
            Command::Observe {
                command: ObserveCommand::Status
            }
        ));
        let report = Cli::try_parse_from(["aopmem", "observe", "report", "--json"])
            .expect("observe report --json should parse");
        assert!(matches!(
            report.command,
            Command::Observe {
                command: ObserveCommand::Report
            }
        ));
        let export_output = repo_root.join("debug capsule.zip");
        let export = Cli::try_parse_from([
            "aopmem",
            "--json",
            "observe",
            "export",
            "--output",
            export_output
                .to_str()
                .expect("test export path should be UTF-8"),
        ])
        .expect("observe export --json should parse");
        assert!(matches!(
            export.command,
            Command::Observe {
                command: ObserveCommand::Export(_)
            }
        ));
        assert_eq!(command_id(&export.command), "observe_export");
        assert_eq!(
            run_command(&status.command, status.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert_eq!(
            run_command(&report.command, report.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert_eq!(
            run_command(&export.command, export.json),
            ExitCode::from(EXIT_SUCCESS)
        );
        assert!(export_output.is_file(), "observe export should publish ZIP");

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should open for proof");
        let event_count: i64 = observability
            .query_row("SELECT COUNT(*) FROM observability_events", [], |row| {
                row.get(0)
            })
            .expect("event count should read");
        let observability_schema: i64 = observability
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("observability schema should read");
        drop(observability);
        let operational_after =
            rusqlite::Connection::open(workspace_paths.db()).expect("operational DB should reopen");
        let operational_schema_after: i64 = operational_after
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("operational schema should read again");
        drop(operational_after);

        assert_eq!(event_count, 0, "observe commands must not self-observe");
        assert_eq!(observability_schema, 1);
        assert_eq!(operational_schema_after, operational_schema_before);
        assert_eq!(file_fingerprint(workspace_paths.db()), operational_before);
        assert_eq!(
            file_fingerprint(workspace_paths.observability_db()),
            observability_before
        );

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn observe_json_commands_do_not_create_missing_home_or_workspace() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("observe-missing-home");
        let repo_root = temp_path("observe-missing-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);

        for args in [
            ["aopmem", "--json", "observe", "status"],
            ["aopmem", "--json", "observe", "report"],
        ] {
            let cli = Cli::try_parse_from(args).expect("observe command should parse");
            assert_eq!(
                run_command(&cli.command, cli.json),
                ExitCode::from(EXIT_SUCCESS)
            );
            assert!(
                !override_home.exists(),
                "read-only observe command created AOPMEM_HOME"
            );
        }

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn ui_cli_parses_bare_no_open_and_random_port_forms_exactly() {
        let bare = Cli::try_parse_from(["aopmem", "ui"]).expect("bare UI should parse");
        let Command::Ui(bare_args) = bare.command else {
            panic!("bare UI should select the UI command");
        };
        assert_eq!(bare_args.port, 0);
        assert!(!bare_args.no_open);

        let explicit = Cli::try_parse_from(["aopmem", "--json", "ui", "--no-open", "--port", "0"])
            .expect("UI flags should parse");
        let Command::Ui(explicit_args) = explicit.command else {
            panic!("explicit UI flags should select the UI command");
        };
        assert_eq!(explicit_args.port, 0);
        assert!(explicit_args.no_open);
        assert!(explicit.json);
        assert_eq!(command_id(&Command::Ui(explicit_args)), "ui");
    }

    #[test]
    fn ui_missing_workspace_returns_exit_3_without_creating_paths() {
        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("ui-missing-workspace-home");
        let home = temp_path("ui-missing-workspace-fallback-home");
        let repo_root = temp_path("ui-missing-workspace-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let cli = Cli::try_parse_from(["aopmem", "--json", "ui", "--no-open"])
            .expect("UI command should parse");

        assert_eq!(
            run_command(&cli.command, cli.json),
            ExitCode::from(EXIT_WORKSPACE_NOT_FOUND)
        );
        assert!(
            !override_home.exists(),
            "read-only UI must not create AOPMEM_HOME or a workspace DB"
        );

        drop(_cwd);
        fs::remove_dir_all(repo_root).expect("repo root should remove");
    }

    #[test]
    fn ui_preparation_is_read_only_and_never_self_observes() {
        struct RejectLauncher(std::cell::Cell<usize>);

        impl ui::BrowserLauncher for RejectLauncher {
            fn launch(&self, _url: &str) -> io::Result<()> {
                self.0.set(self.0.get() + 1);
                Err(io::Error::other("launcher must be skipped by --no-open"))
            }
        }

        let _lock = install::test_env_lock()
            .lock()
            .expect("test lock should not be poisoned");
        let override_home = temp_path("ui-read-only-home");
        let repo_root = temp_path("ui-read-only-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let repo_root = repo_root
            .canonicalize()
            .expect("repo root should canonicalize");
        let _cwd = CurrentDirGuard::set(&repo_root);
        let (workspace_key, operational) = open_test_workspace_db();
        drop(operational);
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &workspace_key);
        drop(
            crate::observability::open_writer(&workspace_paths)
                .expect("observability store should initialize"),
        );
        let operational_before = file_fingerprint(workspace_paths.db());
        let observability_before = file_fingerprint(workspace_paths.observability_db());
        let cli = Cli::try_parse_from(["aopmem", "ui", "--no-open", "--port", "0"])
            .expect("UI command should parse");
        let Command::Ui(args) = &cli.command else {
            panic!("UI command should select UI args");
        };
        let launcher = RejectLauncher(std::cell::Cell::new(0));

        let (prepared_workspace_key, started) = prepare_ui_with_launcher(args, &launcher)
            .expect("read-only UI preparation should succeed");
        assert_eq!(prepared_workspace_key, workspace_key);
        assert_eq!(launcher.0.get(), 0, "--no-open must skip browser launch");
        assert_ne!(started.port(), 0);
        drop(started);

        let observability = rusqlite::Connection::open(workspace_paths.observability_db())
            .expect("observability DB should reopen");
        let event_count: i64 = observability
            .query_row("SELECT COUNT(*) FROM observability_events", [], |row| {
                row.get(0)
            })
            .expect("observability event count should read");
        drop(observability);
        assert_eq!(event_count, 0, "UI must not self-observe");
        assert_eq!(file_fingerprint(workspace_paths.db()), operational_before);
        assert_eq!(
            file_fingerprint(workspace_paths.observability_db()),
            observability_before
        );

        drop(_cwd);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }
}
