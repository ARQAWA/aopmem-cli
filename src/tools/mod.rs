//! Tool registry and `tool.json` contract helpers.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
#[cfg(test)]
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::types::Type;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::artifacts::{self, ToolArtifactCaptureFiles, ToolArtifactStaging};
use crate::audit::AnchoredDir;
use crate::mutation::MutationEffects;
use crate::storage;

/// Canonical `tool.json` file name.
pub const TOOL_JSON_FILE_NAME: &str = "tool.json";
/// Draft tool status created by `aopmem tool create-draft`.
pub const DRAFT_TOOL_STATUS: &str = "draft";
/// Draft-local executable directory under `tools/<tool-id>/`.
pub const TOOL_BIN_DIR_NAME: &str = "bin";
/// Draft-local runtime directory under `tools/<tool-id>/`.
pub const TOOL_RUNTIME_DIR_NAME: &str = "runtime";

const DRAFT_TOOL_STAGING_PREFIX: &str = ".aopmem-draft-stage-";
const TOOL_RUN_POLL_INTERVAL: Duration = Duration::from_millis(10);
const TOOL_RUN_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(test)]
const ARTIFACT_FAILURE_NONE: u8 = 0;
#[cfg(test)]
const ARTIFACT_FAILURE_READ: u8 = 1;
#[cfg(test)]
const ARTIFACT_FAILURE_WRITE: u8 = 2;
#[cfg(test)]
const ARTIFACT_FAILURE_SYNC: u8 = 3;
#[cfg(test)]
const ARTIFACT_FAILURE_PUBLISH: u8 = 4;
#[cfg(test)]
static ARTIFACT_FAILURE_MODE: AtomicU8 = AtomicU8::new(ARTIFACT_FAILURE_NONE);
#[cfg(test)]
static IMPLEMENTATION_SWAP_AFTER_HASH: Mutex<Option<(PathBuf, PathBuf, PathBuf)>> =
    Mutex::new(None);
#[cfg(all(test, target_os = "macos"))]
static DARWIN_FORCE_CLONE_FALLBACK: AtomicBool = AtomicBool::new(false);
#[cfg(all(test, target_os = "macos"))]
static DARWIN_MUTATE_SOURCE_DURING_FALLBACK: AtomicBool = AtomicBool::new(false);

/// Default maximum wall-clock time for one tool process.
pub const DEFAULT_TOOL_RUN_TIMEOUT: Duration = Duration::from_secs(30);
/// Default maximum captured bytes for each tool output stream.
pub const DEFAULT_TOOL_RUN_OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
/// Default persisted wall-clock limit for one generated tool invocation.
pub const DEFAULT_TOOL_TIMEOUT_MS: u64 = 30_000;
/// Default persisted capture limit for each generated tool output stream.
pub const DEFAULT_TOOL_OUTPUT_LIMIT_BYTES: u64 = 65_536;
/// Global hard ceiling for a tool-specific wall-clock limit.
pub const MAX_TOOL_CONTRACT_TIMEOUT_MS: u64 = 15 * 60 * 1_000;
/// Global hard ceiling for each tool-specific output capture limit.
pub const MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES: u64 = 10 * 1024 * 1024;
/// Size bounds for persisted tool contracts and their list responses.
pub const MAX_TOOL_ID_BYTES: usize = 128;
pub const MAX_TOOL_NAME_BYTES: usize = 4 * 1024;
pub const MAX_TOOL_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_TOOL_SCHEMA_BYTES: usize = 128 * 1024;
pub const MAX_TOOL_EXAMPLES: usize = 100;
/// Hard bounds for direct tool aliases and their bulk APIs.
pub const MAX_TOOL_ALIAS_BYTES: usize = MAX_TOOL_ID_BYTES;
pub const MAX_TOOL_ALIAS_SOURCE_BYTES: usize = 128;
pub const MAX_TOOL_ALIAS_PAGE_SIZE: usize = 1_000;
pub const MAX_TOOL_ALIAS_BATCH_SIZE: usize = 1_000;
/// Hard bounds for one deterministic duplicate-plan operation.
pub const MAX_TOOL_DEDUPE_TOOLS: usize = 1_000;
pub const MAX_TOOL_DEDUPE_PAIRS: usize = 10_000;
pub const MAX_TOOL_IMPLEMENTATION_FILES: usize = 256;
pub const MAX_TOOL_IMPLEMENTATION_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_TOOL_IMPLEMENTATION_DEPTH: usize = 16;
pub const MAX_TOOL_IMPLEMENTATION_ENTRIES: usize = 1_024;
pub const MAX_TOOL_MANIFEST_BYTES: usize = 1024 * 1024;
/// Bounded technical explanation accepted only for a possible-overlap review.
pub const MAX_TOOL_TECHNICAL_DISTINCTION_BYTES: usize = 1024;
/// Maximum shortlisted existing tools inspected by one creation preflight.
pub const MAX_TOOL_CREATION_GUARD_CANDIDATES: usize = 64;
const MAX_TOOL_CREATION_GUARD_TOKENS_PER_DOCUMENT: usize = 512;
const MAX_TOOL_CREATION_GUARD_TOTAL_TOKENS: usize = 262_144;
/// The only persisted alias state in RC7.
pub const TOOL_ALIAS_STATUS_ACTIVE: &str = "active";
const TOOL_ALIAS_BULK_SAVEPOINT: &str = "aopmem_tool_alias_bulk";
/// Hard maximum wall-clock time accepted by the runner.
pub const MAX_TOOL_RUN_TIMEOUT: Duration = Duration::from_millis(MAX_TOOL_CONTRACT_TIMEOUT_MS);
/// Hard maximum captured bytes accepted for each tool output stream.
pub const MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES: usize = 10 * 1024 * 1024;

/// Allowed tool side effects.
pub const ALLOWED_TOOL_SIDE_EFFECTS: &[&str] = &[
    "none",
    "local_read",
    "local_write_artifact",
    "local_write_memory",
    "external_read",
    "external_write",
    "destructive",
];

const fn default_tool_timeout_ms() -> u64 {
    DEFAULT_TOOL_TIMEOUT_MS
}

const fn default_tool_output_limit_bytes() -> u64 {
    DEFAULT_TOOL_OUTPUT_LIMIT_BYTES
}

/// Minimal command contract for a generated tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCommand {
    pub entrypoint: String,
}

/// Example invocation stored in the tool contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExample {
    pub name: String,
    pub args: Vec<String>,
    pub description: Option<String>,
}

/// Runtime details stored in the tool contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRuntimeInfo {
    pub executable_path: String,
    pub runtime_dir: Option<String>,
    #[serde(default = "default_tool_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_tool_output_limit_bytes")]
    pub stdout_limit_bytes: u64,
    #[serde(default = "default_tool_output_limit_bytes")]
    pub stderr_limit_bytes: u64,
    #[serde(default)]
    pub supports_dry_run: bool,
    #[serde(default)]
    pub output_mode: ToolOutputMode,
}

/// Persisted behavior when a tool produces output beyond its inline limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolOutputMode {
    #[default]
    Inline,
    Artifact,
}

impl ToolOutputMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Artifact => "artifact",
        }
    }
}

impl FromStr for ToolOutputMode {
    type Err = ToolOutputModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "inline" => Ok(Self::Inline),
            "artifact" => Ok(Self::Artifact),
            _ => Err(ToolOutputModeParseError(value.to_string())),
        }
    }
}

/// Parse failure for a persisted tool output mode.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid tool output mode: {0}; expected inline or artifact")]
pub struct ToolOutputModeParseError(String);

/// Runtime fields accepted by `aopmem tool create-draft`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftToolRuntimeInput {
    pub timeout_ms: u64,
    pub stdout_limit_bytes: u64,
    pub stderr_limit_bytes: u64,
    pub supports_dry_run: bool,
    pub output_mode: ToolOutputMode,
}

impl Default for DraftToolRuntimeInput {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_TOOL_TIMEOUT_MS,
            stdout_limit_bytes: DEFAULT_TOOL_OUTPUT_LIMIT_BYTES,
            stderr_limit_bytes: DEFAULT_TOOL_OUTPUT_LIMIT_BYTES,
            supports_dry_run: false,
            output_mode: ToolOutputMode::Inline,
        }
    }
}

/// Exported tool contract shape for `tool.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolContract {
    pub tool_id: String,
    pub name: String,
    pub status: String,
    pub owner_workflow: Option<String>,
    pub command: ToolCommand,
    pub args_schema: Value,
    pub output_schema: Value,
    pub side_effects: String,
    pub approval_requirement: String,
    pub examples: Vec<ToolExample>,
    pub runtime: ToolRuntimeInfo,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub platform_launchers: BTreeMap<String, String>,
}

/// Stable duplicate class shown to callers. Exact-only eligibility is a
/// separate fact and is never inferred from this label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolDuplicateClass {
    ExactDuplicate,
    SameImplementationDifferentName,
    SameCapabilityDifferentWrapper,
    PossibleOverlap,
    Distinct,
}

/// One deterministic comparison from a read-only duplicate plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolDuplicateComparison {
    pub canonical_tool_id: String,
    pub candidate_tool_id: String,
    pub class: ToolDuplicateClass,
    pub exact_only_eligible: bool,
    pub reasons: Vec<String>,
}

/// Read-only, bounded duplicate plan reusable by CLI/UI and later apply logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolDedupePlan {
    pub writes_performed: bool,
    pub scanned_tools: usize,
    pub shortlisted_tools: usize,
    pub shortlisted_pairs: usize,
    pub hashed_files: usize,
    pub comparisons: Vec<ToolDuplicateComparison>,
}

/// Result of one authoritative exact-only canonicalization pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolDedupeApplyReport {
    pub writes_performed: bool,
    pub scanned_tools: usize,
    pub hashed_files: usize,
    pub canonicalized: Vec<ToolCanonicalization>,
    pub skipped_without_active_canonical: Vec<Vec<String>>,
    pub possible_overlaps: Vec<ToolDuplicateComparison>,
}

/// One old id now resolving directly to an active canonical id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolCanonicalization {
    pub canonical_tool_id: String,
    pub superseded_tool_id: String,
}

/// SQLite-backed registered tool contract with timestamps.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolContractRecord {
    #[serde(flatten)]
    pub contract: ToolContract,
    pub created_at: String,
    pub updated_at: String,
}

/// One keyset-paginated slice of tool contracts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolContractsPage {
    pub items: Vec<ToolContractRecord>,
    pub next_after_id: Option<String>,
    pub more_results: bool,
}

/// One direct alias to an active canonical tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolAlias {
    pub alias: String,
    pub canonical_tool_id: String,
    pub created_at: String,
    pub source: String,
    pub status: String,
}

/// Validated storage input for one direct tool alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewToolAlias {
    pub alias: String,
    pub canonical_tool_id: String,
    pub source: String,
    pub status: String,
}

/// One keyset-paginated slice of direct tool aliases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolAliasesPage {
    pub items: Vec<ToolAlias>,
    pub next_after_alias: Option<String>,
    pub more_results: bool,
}

/// How a requested tool id reached its canonical registry record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolIdResolutionKind {
    DirectCanonical,
    Alias,
    SupersededFallback,
}

/// Deterministic tool-id resolution without filesystem side effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolIdResolution {
    pub requested_id: String,
    pub canonical_tool_id: String,
    pub matched_alias: Option<String>,
    pub canonical_status: String,
    pub kind: ToolIdResolutionKind,
}

/// Creation-guard classification. Exact fingerprint equality is deliberately
/// absent because a not-yet-created draft has no implementation bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolCreationGuardClass {
    ActiveIdCollision,
    AliasCollision,
    PossibleOverlap,
}

/// One safe candidate returned by the creation guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolCreationGuardCandidate {
    pub canonical_tool_id: String,
    pub alias_suggestion: String,
    pub class: ToolCreationGuardClass,
    pub reasons: Vec<String>,
}

/// Read-only decision made before draft creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ToolCreationGuardDecision {
    Allowed {
        technical_distinction_provided: bool,
        reviewed_candidates: Vec<ToolCreationGuardCandidate>,
    },
    Duplicate {
        candidate: ToolCreationGuardCandidate,
        writes_performed: bool,
    },
    OverlapReviewRequired {
        candidate: ToolCreationGuardCandidate,
        writes_performed: bool,
    },
}

impl ToolCreationGuardDecision {
    /// Returns true only when draft creation may proceed.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

/// Minimal input for `aopmem tool create-draft`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftToolInput {
    pub tool_id: String,
    pub name: String,
    pub entrypoint: String,
    pub owner_workflow: Option<String>,
    pub side_effects: String,
    pub approval_requirement: String,
}

/// Result of a created draft tool.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DraftToolRecord {
    pub record: ToolContractRecord,
    pub tool_dir: String,
    pub tool_json_path: String,
    pub bin_dir: String,
    pub runtime_dir: String,
}

/// Safe CLI result from a guarded draft creation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GuardedDraftToolRecord {
    #[serde(flatten)]
    pub draft: DraftToolRecord,
    pub technical_distinction_provided: bool,
}

/// Result of a validated tool manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolValidationRecord {
    pub tool_id: String,
    pub tool_json_path: String,
    pub executable_path: String,
    pub side_effects: String,
    pub approval_requirement: String,
    pub runner_dry_run_supported: bool,
    pub runtime: ToolRuntimeInfo,
}

/// Result of a tool process execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolRunRecord {
    pub tool_id: String,
    pub tool_json_path: String,
    pub executable_path: String,
    pub args: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ToolRunArtifacts>,
}

/// Published full-output files for one artifact-mode tool run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolRunArtifacts {
    pub stdout: ToolRunArtifactStream,
    pub stderr: ToolRunArtifactStream,
}

/// One published stream and its bounded inline preview metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolRunArtifactStream {
    pub path: String,
    pub bytes: u64,
    pub preview_truncated: bool,
}

/// Bounded resources used by [`run_tool_with_limits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolRunLimits {
    pub timeout: Duration,
    pub stdout_max_bytes: usize,
    pub stderr_max_bytes: usize,
}

/// Invocation-local execution phases used by the CLI for truthful lifecycle
/// events. This trace contains no arguments, output, paths, or persisted data.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolRunTrace {
    validation_succeeded: bool,
    process_spawned: bool,
}

struct LoadedToolRun {
    contract: ToolContract,
    limits: ToolRunLimits,
}

impl ToolRunTrace {
    #[must_use]
    pub(crate) const fn validation_succeeded(self) -> bool {
        self.validation_succeeded
    }

    #[must_use]
    pub(crate) const fn process_spawned(self) -> bool {
        self.process_spawned
    }
}

impl Default for ToolRunLimits {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TOOL_RUN_TIMEOUT,
            stdout_max_bytes: DEFAULT_TOOL_RUN_OUTPUT_LIMIT_BYTES,
            stderr_max_bytes: DEFAULT_TOOL_RUN_OUTPUT_LIMIT_BYTES,
        }
    }
}

impl ToolRunLimits {
    /// Converts a validated persisted runtime contract into native runner limits.
    pub fn from_runtime(runtime: &ToolRuntimeInfo) -> Result<Self, ToolRunLimitError> {
        let stdout_max_bytes = usize::try_from(runtime.stdout_limit_bytes).map_err(|_| {
            invalid_tool_run_limits(
                runtime.timeout_ms,
                runtime.stdout_limit_bytes,
                runtime.stderr_limit_bytes,
            )
        })?;
        let stderr_max_bytes = usize::try_from(runtime.stderr_limit_bytes).map_err(|_| {
            invalid_tool_run_limits(
                runtime.timeout_ms,
                runtime.stdout_limit_bytes,
                runtime.stderr_limit_bytes,
            )
        })?;
        let limits = Self {
            timeout: Duration::from_millis(runtime.timeout_ms),
            stdout_max_bytes,
            stderr_max_bytes,
        };
        validate_tool_run_limits(limits)?;
        Ok(limits)
    }
}

/// Deterministic execution-limit failures from the local tool runner.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToolRunLimitError {
    #[error(
        "tool run limits are outside allowed bounds (timeout_ms={timeout_ms}, stdout_max_bytes={stdout_max_bytes}, stderr_max_bytes={stderr_max_bytes})"
    )]
    InvalidLimits {
        timeout_ms: u128,
        stdout_max_bytes: u128,
        stderr_max_bytes: u128,
    },
    #[error(
        "tool run timed out after {timeout_ms} ms (stdout_limit_bytes={stdout_limit_bytes}, stderr_limit_bytes={stderr_limit_bytes}, stdout_truncated={stdout_truncated}, stderr_truncated={stderr_truncated})"
    )]
    TimedOut {
        timeout_ms: u128,
        stdout_limit_bytes: usize,
        stderr_limit_bytes: usize,
        stdout_truncated: bool,
        stderr_truncated: bool,
    },
    #[error(
        "tool output exceeded configured limits (timeout_ms={timeout_ms}, stdout_limit_bytes={stdout_limit_bytes}, stderr_limit_bytes={stderr_limit_bytes}, stdout_truncated={stdout_truncated}, stderr_truncated={stderr_truncated})"
    )]
    OutputOverflow {
        timeout_ms: u128,
        stdout_limit_bytes: usize,
        stderr_limit_bytes: usize,
        stdout_truncated: bool,
        stderr_truncated: bool,
    },
    #[error(
        "tool artifact output exceeded the global capture ceiling (timeout_ms={timeout_ms}, stdout_limit_bytes={stdout_limit_bytes}, stderr_limit_bytes={stderr_limit_bytes}, hard_limit_bytes={hard_limit_bytes}, stdout_hard_limit_exceeded={stdout_hard_limit_exceeded}, stderr_hard_limit_exceeded={stderr_hard_limit_exceeded})"
    )]
    ArtifactHardOverflow {
        timeout_ms: u128,
        stdout_limit_bytes: usize,
        stderr_limit_bytes: usize,
        hard_limit_bytes: usize,
        stdout_truncated: bool,
        stderr_truncated: bool,
        stdout_hard_limit_exceeded: bool,
        stderr_hard_limit_exceeded: bool,
    },
}

/// Result of a planned tool invocation that does not execute implementation code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolDryRunRecord {
    pub mode: String,
    pub tool_id: String,
    pub tool_json_path: String,
    pub executable_path: String,
    pub args: Vec<String>,
    pub side_effects: String,
    pub approval_requirement: String,
    pub approval_required: bool,
    pub would_execute: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolInvocationRecord {
    Run(ToolRunRecord),
    DryRun(ToolDryRunRecord),
}

/// Validation errors for a tool contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToolContractValidationError {
    #[error("missing required field: tool_id")]
    MissingToolId,
    #[error("tool_id must be a single directory name: {0}")]
    InvalidToolIdPath(String),
    #[error("field {field} exceeds {max_bytes} bytes")]
    FieldTooLong {
        field: &'static str,
        max_bytes: usize,
    },
    #[error("{field} exceeds {max_bytes} bytes")]
    SchemaTooLarge {
        field: &'static str,
        max_bytes: usize,
    },
    #[error("missing required field: name")]
    MissingName,
    #[error("missing required field: status")]
    MissingStatus,
    #[error("owner_workflow must not be blank when present")]
    BlankOwnerWorkflow,
    #[error("missing required field: command.entrypoint")]
    MissingCommandEntrypoint,
    #[error("command.entrypoint must be relative and stay inside the tool directory: {0}")]
    CommandEntrypointOutsideToolDir(String),
    #[error("args_schema must be a JSON object")]
    ArgsSchemaMustBeObject,
    #[error("output_schema must be a JSON object")]
    OutputSchemaMustBeObject,
    #[error("invalid side_effects: {0}")]
    InvalidSideEffects(String),
    #[error("missing required field: approval_requirement")]
    MissingApprovalRequirement,
    #[error("examples must not be empty")]
    MissingExamples,
    #[error("examples exceed {max_examples} items")]
    TooManyExamples { max_examples: usize },
    #[error("example name must not be blank")]
    BlankExampleName,
    #[error("missing required field: runtime.executable_path")]
    MissingRuntimeExecutablePath,
    #[error("runtime.executable_path must be relative and stay inside the tool directory: {0}")]
    RuntimeExecutablePathOutsideToolDir(String),
    #[error("runtime.runtime_dir must be relative and stay inside the tool directory: {0}")]
    RuntimeDirectoryOutsideToolDir(String),
    #[error("platform launcher name is invalid: {0}")]
    InvalidPlatformLauncherName(String),
    #[error("platform launcher path must be relative and stay inside the tool directory: {0}")]
    PlatformLauncherOutsideToolDir(String),
    #[error("{field} must be between {minimum} and {maximum} inclusive; received {actual}")]
    RuntimeLimitOutOfRange {
        field: &'static str,
        minimum: u64,
        maximum: u64,
        actual: u64,
    },
    #[error("tool directory must be a real immediate child of the workspace tools directory: {0}")]
    ToolDirectoryOutsideWorkspace(String),
}

/// Combined storage errors for tool contract registry writes.
#[derive(Debug, Error)]
pub enum ToolContractStorageError {
    #[error("{0}")]
    Validation(#[from] ToolContractValidationError),
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

/// Bounded input failures for direct tool aliases.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToolAliasValidationError {
    #[error("missing required field: alias")]
    MissingAlias,
    #[error("alias must be a single tool id: {0}")]
    InvalidAlias(String),
    #[error("missing required field: canonical_tool_id")]
    MissingCanonicalToolId,
    #[error("canonical_tool_id must be a single tool id: {0}")]
    InvalidCanonicalToolId(String),
    #[error("missing required field: source")]
    MissingSource,
    #[error("{field} exceeds {max_bytes} bytes")]
    FieldTooLong {
        field: &'static str,
        max_bytes: usize,
    },
    #[error("{field} must not contain NUL")]
    ContainsNul { field: &'static str },
    #[error("invalid tool alias status: {0}; expected active")]
    InvalidStatus(String),
    #[error("tool alias page limit exceeds {max_items} items")]
    PageTooLarge { max_items: usize },
    #[error("tool alias batch exceeds {max_items} items")]
    BatchTooLarge { max_items: usize },
    #[error("tool alias batch contains duplicate alias: {0}")]
    DuplicateBatchAlias(String),
}

/// Bounded validation errors for the non-persisted overlap explanation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToolTechnicalDistinctionError {
    #[error("technical distinction must contain a non-blank UTF-8 explanation")]
    Blank,
    #[error("technical distinction exceeds {max_bytes} bytes")]
    TooLong { max_bytes: usize },
    #[error("technical distinction must not contain NUL")]
    ContainsNul,
}

/// Typed storage and invariant failures for direct tool aliases.
#[derive(Debug, Error)]
pub enum ToolAliasStorageError {
    #[error("{0}")]
    Validation(#[from] ToolAliasValidationError),
    #[error("tool alias already exists: {0}")]
    AliasAlreadyExists(String),
    #[error("canonical tool not found: {0}")]
    CanonicalToolNotFound(String),
    #[error("tool alias target is another alias: {0}")]
    AliasTargetIsAlias(String),
    #[error("canonical tool must be active: {tool_id} has status {status}")]
    CanonicalToolNotActive { tool_id: String, status: String },
    #[error("tool alias cannot shadow non-superseded tool: {tool_id} has status {status}")]
    AliasShadowsTool { tool_id: String, status: String },
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
}

/// File read/write errors for local `tool.json`.
#[derive(Debug, Error)]
pub enum ToolJsonError {
    #[error("{0}")]
    Validation(#[from] ToolContractValidationError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

/// Combined errors for draft tool creation.
#[derive(Debug, Error)]
pub enum CreateDraftToolError {
    #[error("{0}")]
    Storage(#[from] ToolContractStorageError),
    #[error("{0}")]
    Json(#[from] ToolJsonError),
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// Fail-closed errors from a bounded, read-only duplicate plan.
#[derive(Debug, Error)]
pub enum ToolDedupePlanError {
    #[error("tool registry exceeds duplicate-plan limit of {max_tools} tools")]
    TooManyTools { max_tools: usize },
    #[error("duplicate shortlist exceeds {max_pairs} candidate pairs")]
    TooManyPairs { max_pairs: usize },
    #[error("tool contract drift detected between SQLite and tool.json: {0}")]
    ContractDrift(String),
    #[error("tool implementation contains an unsafe path or file type: {0}")]
    UnsafeImplementationPath(String),
    #[error("tool implementation exceeds {max_files} files: {tool_id}")]
    TooManyImplementationFiles { tool_id: String, max_files: usize },
    #[error("tool implementation exceeds {max_entries} descendant entries")]
    TooManyImplementationEntries { max_entries: usize },
    #[error("tool implementation exceeds {max_bytes} bytes: {tool_id}")]
    ImplementationTooLarge { tool_id: String, max_bytes: u64 },
    #[error("tool implementation changed while fingerprinting: {0}")]
    ImplementationDrift(String),
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Json(#[from] ToolJsonError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("failed to serialize canonical tool fingerprint")]
    Serialization,
}

/// Errors from the in-mutation exact-only canonicalizer.
#[derive(Debug, Error)]
pub enum ToolDedupeApplyError {
    #[error(transparent)]
    Plan(#[from] ToolDedupePlanError),
    #[error(transparent)]
    Alias(#[from] ToolAliasStorageError),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] ToolJsonError),
    #[error("failed to serialize canonical tool contract")]
    Serialization,
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Fail-closed errors from tool creation preflight.
#[derive(Debug, Error)]
pub enum ToolCreationGuardError {
    #[error("{0}")]
    Validation(#[from] ToolContractValidationError),
    #[error("{0}")]
    Alias(#[from] ToolAliasStorageError),
    #[error("{0}")]
    Plan(#[from] ToolDedupePlanError),
    #[error("tool creation shortlist exceeds {max_candidates} candidates")]
    TooManyCandidates { max_candidates: usize },
    #[error("tool creation search exceeds {max_tokens} bounded description tokens")]
    TooManySearchTokens { max_tokens: usize },
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
}

/// Combined errors for the authoritative in-mutation creation-guard recheck.
#[derive(Debug, Error)]
pub enum GuardedCreateDraftError {
    #[error("{0}")]
    Guard(#[from] ToolCreationGuardError),
    #[error("tool creation blocked by registry guard")]
    Blocked(ToolCreationGuardDecision),
    #[error("{0}")]
    Create(#[from] CreateDraftToolError),
}

/// Combined errors for `aopmem tool validate`.
#[derive(Debug, Error)]
pub enum ValidateToolError {
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Json(#[from] ToolJsonError),
    #[error("tool contract drift detected between SQLite and tool.json: {0}")]
    ContractDrift(String),
    #[error("tool executable path does not exist: {0}")]
    MissingExecutablePath(String),
}

/// Combined errors for `aopmem tool run`.
#[derive(Debug, Error)]
pub enum RunToolError {
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Json(#[from] ToolJsonError),
    #[error("tool contract drift detected between SQLite and tool.json: {0}")]
    ContractDrift(String),
    #[error("tool executable path does not exist: {0}")]
    MissingExecutablePath(String),
    #[error(
        "tool run blocked without approval: tool_id={tool_id}, side_effects={side_effects}, approval_requirement={approval_requirement}"
    )]
    UnsafeActionBlocked {
        tool_id: String,
        side_effects: String,
        approval_requirement: String,
    },
    #[error("{0}")]
    Limit(#[from] ToolRunLimitError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("tool process exited with non-zero status: {0}")]
    ProcessFailed(i32),
}

impl RunToolError {
    /// Returns a structured timeout or output-limit failure, when present.
    #[must_use]
    pub fn limit_error(&self) -> Option<&ToolRunLimitError> {
        match self {
            Self::Limit(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
enum CanonicalToolContractError {
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Json(#[from] ToolJsonError),
    #[error("tool contract drift detected between SQLite and tool.json: {0}")]
    ContractDrift(String),
}

impl From<CanonicalToolContractError> for ValidateToolError {
    fn from(value: CanonicalToolContractError) -> Self {
        match value {
            CanonicalToolContractError::NotFound(tool_id) => Self::NotFound(tool_id),
            CanonicalToolContractError::Db(error) => Self::Db(error),
            CanonicalToolContractError::Json(error) => Self::Json(error),
            CanonicalToolContractError::ContractDrift(tool_id) => Self::ContractDrift(tool_id),
        }
    }
}

impl From<CanonicalToolContractError> for RunToolError {
    fn from(value: CanonicalToolContractError) -> Self {
        match value {
            CanonicalToolContractError::NotFound(tool_id) => Self::NotFound(tool_id),
            CanonicalToolContractError::Db(error) => Self::Db(error),
            CanonicalToolContractError::Json(error) => Self::Json(error),
            CanonicalToolContractError::ContractDrift(tool_id) => Self::ContractDrift(tool_id),
        }
    }
}

/// Returns the tool directory path under the workspace.
#[must_use]
pub fn tool_dir(workspace_paths: &storage::WorkspacePaths, tool_id: &str) -> PathBuf {
    workspace_paths.tools().join(tool_id)
}

/// Returns the `tool.json` path under the workspace.
#[must_use]
pub fn tool_json_path(workspace_paths: &storage::WorkspacePaths, tool_id: &str) -> PathBuf {
    tool_dir(workspace_paths, tool_id).join(TOOL_JSON_FILE_NAME)
}

/// Creates a tool contract record in the canonical SQLite registry.
pub fn create_tool_contract(
    connection: &Connection,
    contract: &ToolContract,
) -> Result<ToolContractRecord, ToolContractStorageError> {
    validate_tool_contract(contract)?;
    let contract_json = serde_json::to_string_pretty(contract)?;

    connection.execute(
        "
        INSERT INTO tool_contracts (
            tool_id,
            name,
            status,
            owner_workflow,
            side_effects,
            approval_requirement,
            contract_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);
        ",
        params![
            &contract.tool_id,
            &contract.name,
            &contract.status,
            &contract.owner_workflow,
            &contract.side_effects,
            &contract.approval_requirement,
            &contract_json
        ],
    )?;

    get_tool_contract(connection, &contract.tool_id)?.ok_or(ToolContractStorageError::Db(
        rusqlite::Error::QueryReturnedNoRows,
    ))
}

/// Looks up one tool contract by its stable tool id.
pub fn get_tool_contract(
    connection: &Connection,
    tool_id: &str,
) -> rusqlite::Result<Option<ToolContractRecord>> {
    connection
        .query_row(
            "
            SELECT
                tool_id,
                name,
                status,
                owner_workflow,
                side_effects,
                approval_requirement,
                contract_json,
                created_at,
                updated_at
            FROM tool_contracts
            WHERE tool_id = ?1;
            ",
            [tool_id],
            row_to_tool_contract_record,
        )
        .optional()
}

/// Lists all tool contracts in stable id order.
pub fn list_tool_contracts(connection: &Connection) -> rusqlite::Result<Vec<ToolContractRecord>> {
    let mut statement = connection.prepare(
        "
        SELECT
            tool_id,
            name,
            status,
            owner_workflow,
            side_effects,
            approval_requirement,
            contract_json,
            created_at,
            updated_at
        FROM tool_contracts
        ORDER BY tool_id ASC;
        ",
    )?;

    let contracts = statement
        .query_map([], row_to_tool_contract_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(contracts)
}

/// Lists one stable keyset-paginated slice of tool contracts.
///
/// A zero limit is a safe empty page. When more rows exist, use
/// `next_after_id` as `after_tool_id` for the next call.
pub fn list_tool_contracts_page(
    connection: &Connection,
    after_tool_id: Option<&str>,
    limit: usize,
) -> rusqlite::Result<ToolContractsPage> {
    if limit == 0 {
        return Ok(ToolContractsPage {
            items: Vec::new(),
            next_after_id: None,
            more_results: false,
        });
    }

    let query_limit = i64::try_from(limit)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let mut items = match after_tool_id {
        Some(after_tool_id) => {
            let mut statement = connection.prepare(
                "
                SELECT
                    tool_id,
                    name,
                    status,
                    owner_workflow,
                    side_effects,
                    approval_requirement,
                    contract_json,
                    created_at,
                    updated_at
                FROM tool_contracts
                WHERE tool_id > ?1
                ORDER BY tool_id ASC
                LIMIT ?2;
                ",
            )?;
            let items = statement
                .query_map(
                    params![after_tool_id, query_limit],
                    row_to_tool_contract_record,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            items
        }
        None => {
            let mut statement = connection.prepare(
                "
                SELECT
                    tool_id,
                    name,
                    status,
                    owner_workflow,
                    side_effects,
                    approval_requirement,
                    contract_json,
                    created_at,
                    updated_at
                FROM tool_contracts
                ORDER BY tool_id ASC
                LIMIT ?1;
                ",
            )?;
            let items = statement
                .query_map([query_limit], row_to_tool_contract_record)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            items
        }
    };

    let more_results = items.len() > limit;
    if more_results {
        items.pop();
    }
    let next_after_id = more_results
        .then(|| items.last().map(|record| record.contract.tool_id.clone()))
        .flatten();

    Ok(ToolContractsPage {
        items,
        next_after_id,
        more_results,
    })
}

/// Lists canonical (non-superseded) tool contracts with stable keyset
/// pagination. Alias rows never consume this page's limit or cursor.
pub fn list_canonical_tool_contracts_page(
    connection: &Connection,
    after_tool_id: Option<&str>,
    limit: usize,
) -> rusqlite::Result<ToolContractsPage> {
    if limit == 0 {
        return Ok(ToolContractsPage {
            items: Vec::new(),
            next_after_id: None,
            more_results: false,
        });
    }
    let query_limit = i64::try_from(limit)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let sql = match after_tool_id {
        Some(_) => {
            "
            SELECT tool_id, name, status, owner_workflow, side_effects,
                   approval_requirement, contract_json, created_at, updated_at
            FROM tool_contracts
            WHERE status <> 'superseded' AND tool_id > ?1
            ORDER BY tool_id ASC
            LIMIT ?2
            "
        }
        None => {
            "
            SELECT tool_id, name, status, owner_workflow, side_effects,
                   approval_requirement, contract_json, created_at, updated_at
            FROM tool_contracts
            WHERE status <> 'superseded'
            ORDER BY tool_id ASC
            LIMIT ?1
            "
        }
    };
    let mut statement = connection.prepare(sql)?;
    let mut items = match after_tool_id {
        Some(after_tool_id) => statement
            .query_map(
                params![after_tool_id, query_limit],
                row_to_tool_contract_record,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        None => statement
            .query_map([query_limit], row_to_tool_contract_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    };
    let more_results = items.len() > limit;
    if more_results {
        items.pop();
    }
    let next_after_id = more_results
        .then(|| items.last().map(|record| record.contract.tool_id.clone()))
        .flatten();
    Ok(ToolContractsPage {
        items,
        next_after_id,
        more_results,
    })
}

#[derive(Serialize)]
struct CapabilityFingerprintInput<'a> {
    args_schema: Value,
    output_schema: Value,
    side_effects: &'a str,
    approval_requirement: &'a str,
    timeout_ms: u64,
    stdout_limit_bytes: u64,
    stderr_limit_bytes: u64,
    supports_dry_run: bool,
    output_mode: &'a str,
}

#[derive(Serialize)]
struct CanonicalFingerprintInput<'a> {
    capability: CapabilityFingerprintInput<'a>,
    command_entrypoint: String,
    executable_path: String,
    runtime_dir: Option<String>,
    platform_launchers: BTreeMap<String, String>,
    implementation_files: &'a BTreeMap<String, String>,
}

struct FingerprintedTool {
    record: ToolContractRecord,
    capability_fingerprint: String,
    canonical_fingerprint: String,
    implementation_fingerprint: String,
    normalized_label: String,
}

struct ImplementationFingerprint {
    files: BTreeMap<String, String>,
    content_fingerprint: String,
    hashed_files: usize,
}

/// Builds a deterministic duplicate plan without writing SQLite,
/// observability, manifests, executables, or any other workspace path.
pub fn plan_tool_deduplication(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
) -> Result<ToolDedupePlan, ToolDedupePlanError> {
    let page = list_tool_contracts_page(connection, None, MAX_TOOL_DEDUPE_TOOLS)?;
    if page.more_results {
        return Err(ToolDedupePlanError::TooManyTools {
            max_tools: MAX_TOOL_DEDUPE_TOOLS,
        });
    }
    let records = page.items;
    let scanned_tools = records.len();

    let mut capability_fingerprints = Vec::with_capacity(records.len());
    let mut normalized_labels = Vec::with_capacity(records.len());
    let mut capability_buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut label_buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut token_buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();

    for (index, record) in records.iter().enumerate() {
        let capability = capability_fingerprint(&record.contract)?;
        let label = normalized_tool_label(&record.contract);
        capability_buckets
            .entry(capability.clone())
            .or_default()
            .push(index);
        if !label.is_empty() {
            label_buckets.entry(label.clone()).or_default().push(index);
            for token in label.split('-') {
                token_buckets
                    .entry(token.to_string())
                    .or_default()
                    .push(index);
            }
        }
        capability_fingerprints.push(capability);
        normalized_labels.push(label);
    }

    let mut pairs = BTreeSet::new();
    add_bucket_pairs(&capability_buckets, &mut pairs)?;
    add_bucket_pairs(&label_buckets, &mut pairs)?;
    add_bucket_pairs(&token_buckets, &mut pairs)?;

    let shortlisted_indices = pairs
        .iter()
        .flat_map(|(left, right)| [*left, *right])
        .collect::<BTreeSet<_>>();
    let mut fingerprinted = BTreeMap::new();
    let mut hashed_files = 0_usize;
    for index in shortlisted_indices.iter().copied() {
        let (tool, tool_hashed_files) = fingerprint_registered_tool(
            workspace_paths,
            records[index].clone(),
            capability_fingerprints[index].clone(),
            normalized_labels[index].clone(),
        )?;
        hashed_files = hashed_files
            .checked_add(tool_hashed_files)
            .ok_or(ToolDedupePlanError::Serialization)?;
        fingerprinted.insert(index, tool);
    }

    let mut comparisons = Vec::with_capacity(pairs.len());
    for (left, right) in pairs {
        let left = fingerprinted
            .get(&left)
            .ok_or(ToolDedupePlanError::Serialization)?;
        let right = fingerprinted
            .get(&right)
            .ok_or(ToolDedupePlanError::Serialization)?;
        comparisons.push(compare_fingerprinted_tools(left, right));
    }
    comparisons.sort_by(|left, right| {
        left.canonical_tool_id
            .cmp(&right.canonical_tool_id)
            .then_with(|| left.candidate_tool_id.cmp(&right.candidate_tool_id))
            .then_with(|| left.class.cmp(&right.class))
    });

    Ok(ToolDedupePlan {
        writes_performed: false,
        scanned_tools,
        shortlisted_tools: shortlisted_indices.len(),
        shortlisted_pairs: comparisons.len(),
        hashed_files,
        comparisons,
    })
}

/// Rebuilds the Stage 012 fingerprint state under the caller's active
/// mutation transaction, then canonicalizes only equal full fingerprints.
///
/// This deliberately scans each bounded implementation once and groups by
/// fingerprint. It never expands an equal-fingerprint bucket into pairs.
pub fn apply_exact_only_deduplication_in_mutation(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    effects: &mut MutationEffects,
) -> Result<ToolDedupeApplyReport, ToolDedupeApplyError> {
    let page = list_tool_contracts_page(connection, None, MAX_TOOL_DEDUPE_TOOLS)?;
    if page.more_results {
        return Err(ToolDedupePlanError::TooManyTools {
            max_tools: MAX_TOOL_DEDUPE_TOOLS,
        }
        .into());
    }
    let records = page.items;
    let mut capabilities = Vec::with_capacity(records.len());
    let mut labels = Vec::with_capacity(records.len());
    let mut signal_buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, record) in records.iter().enumerate() {
        let capability = capability_fingerprint(&record.contract)?;
        let label = normalized_tool_label(&record.contract);
        signal_buckets
            .entry(format!("c:{capability}"))
            .or_default()
            .push(index);
        if !label.is_empty() {
            signal_buckets
                .entry(format!("l:{label}"))
                .or_default()
                .push(index);
            for token in label.split('-') {
                signal_buckets
                    .entry(format!("t:{token}"))
                    .or_default()
                    .push(index);
            }
        }
        capabilities.push(capability);
        labels.push(label);
    }
    let mut shortlisted = BTreeSet::new();
    for bucket in signal_buckets.values() {
        if bucket.len() > 1 {
            shortlisted.extend(bucket.iter().copied());
        }
    }
    let mut fingerprinted = BTreeMap::new();
    let mut fingerprint_buckets: BTreeMap<String, Vec<ToolContractRecord>> = BTreeMap::new();
    let mut hashed_files = 0_usize;
    for index in shortlisted.iter().copied() {
        let (tool, count) = fingerprint_registered_tool(
            workspace_paths,
            records[index].clone(),
            capabilities[index].clone(),
            labels[index].clone(),
        )?;
        hashed_files = hashed_files
            .checked_add(count)
            .ok_or(ToolDedupeApplyError::Serialization)?;
        fingerprint_buckets
            .entry(tool.canonical_fingerprint.clone())
            .or_default()
            .push(tool.record.clone());
        fingerprinted.insert(index, tool);
    }

    let mut canonicalized = Vec::new();
    let mut skipped_without_active_canonical = Vec::new();
    for records in fingerprint_buckets.values_mut() {
        if records.len() < 2 {
            continue;
        }
        records.sort_by(|left, right| canonical_record_key(left).cmp(&canonical_record_key(right)));
        let Some(canonical) = records
            .iter()
            .find(|record| record.contract.status == "active")
            .cloned()
        else {
            skipped_without_active_canonical.push(
                records
                    .iter()
                    .map(|record| record.contract.tool_id.clone())
                    .collect(),
            );
            continue;
        };
        for duplicate in records
            .iter()
            .filter(|record| record.contract.tool_id != canonical.contract.tool_id)
        {
            if canonicalize_exact_duplicate(
                workspace_paths,
                connection,
                effects,
                &canonical,
                duplicate,
            )? {
                canonicalized.push(ToolCanonicalization {
                    canonical_tool_id: canonical.contract.tool_id.clone(),
                    superseded_tool_id: duplicate.contract.tool_id.clone(),
                });
            }
        }
    }
    canonicalized.sort_by(|left, right| {
        left.canonical_tool_id
            .cmp(&right.canonical_tool_id)
            .then_with(|| left.superseded_tool_id.cmp(&right.superseded_tool_id))
    });
    skipped_without_active_canonical.sort();
    Ok(ToolDedupeApplyReport {
        writes_performed: !canonicalized.is_empty(),
        scanned_tools: records.len(),
        hashed_files,
        canonicalized,
        skipped_without_active_canonical,
        possible_overlaps: authoritative_overlap_reports(&signal_buckets, &fingerprinted)?,
    })
}

fn canonicalize_exact_duplicate(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    effects: &mut MutationEffects,
    canonical: &ToolContractRecord,
    duplicate: &ToolContractRecord,
) -> Result<bool, ToolDedupeApplyError> {
    let duplicate_id = &duplicate.contract.tool_id;
    let canonical_id = &canonical.contract.tool_id;
    let alias_correct = get_tool_alias(connection, duplicate_id)?
        .is_some_and(|alias| alias.canonical_tool_id == *canonical_id);
    if duplicate.contract.status == "superseded" && alias_correct {
        return Ok(false);
    }
    if duplicate.contract.status == "superseded" {
        if get_tool_alias(connection, duplicate_id)?.is_some() {
            remove_tool_alias(connection, duplicate_id)?;
        }
        add_tool_alias(
            connection,
            &NewToolAlias {
                alias: duplicate_id.clone(),
                canonical_tool_id: canonical_id.clone(),
                source: "exact_only_dedupe".to_string(),
                status: TOOL_ALIAS_STATUS_ACTIVE.to_string(),
            },
        )?;
        return Ok(true);
    }

    let root = anchored_tool_directory(workspace_paths, duplicate_id)?;
    let mut manifest =
        root.open_regular_for_update_os(std::ffi::OsStr::new(TOOL_JSON_FILE_NAME))?;
    let mut original = Vec::new();
    manifest.read_to_end(&mut original)?;
    let mut contract: ToolContract =
        serde_json::from_slice(&original).map_err(ToolJsonError::Json)?;
    validate_tool_contract(&contract).map_err(ToolJsonError::Validation)?;
    if contract != duplicate.contract {
        return Err(ToolDedupePlanError::ContractDrift(duplicate_id.clone()).into());
    }
    contract.status = "superseded".to_string();
    let replacement =
        serde_json::to_vec_pretty(&contract).map_err(|_| ToolDedupeApplyError::Serialization)?;
    effects.register_anchored_file_restore(
        root.clone(),
        std::ffi::OsString::from(TOOL_JSON_FILE_NAME),
        original,
    );
    manifest.seek(SeekFrom::Start(0))?;
    manifest.set_len(0)?;
    manifest.write_all(&replacement)?;
    manifest.sync_all()?;
    root.verify_logical_identity()?;

    let contract_json =
        serde_json::to_string_pretty(&contract).map_err(|_| ToolDedupeApplyError::Serialization)?;
    // A duplicate can itself be a current alias target. Retarget those direct
    // aliases before its status changes; SQLite forbids orphaning them.
    connection.execute(
        "UPDATE tool_aliases SET canonical_tool_id = ?1 WHERE canonical_tool_id = ?2 AND status = 'active'",
        params![canonical_id, duplicate_id],
    )?;
    let changed = connection.execute(
        "UPDATE tool_contracts SET status = 'superseded', contract_json = ?1, updated_at = CURRENT_TIMESTAMP WHERE tool_id = ?2",
        params![contract_json, duplicate_id],
    )?;
    if changed != 1 {
        return Err(ToolDedupeApplyError::Db(
            rusqlite::Error::QueryReturnedNoRows,
        ));
    }
    if let Some(alias) = get_tool_alias(connection, duplicate_id)? {
        if alias.canonical_tool_id != *canonical_id {
            remove_tool_alias(connection, duplicate_id)?;
        }
    }
    if !alias_correct {
        add_tool_alias(
            connection,
            &NewToolAlias {
                alias: duplicate_id.clone(),
                canonical_tool_id: canonical_id.clone(),
                source: "exact_only_dedupe".to_string(),
                status: TOOL_ALIAS_STATUS_ACTIVE.to_string(),
            },
        )?;
    }
    Ok(true)
}

fn authoritative_overlap_reports(
    buckets: &BTreeMap<String, Vec<usize>>,
    fingerprinted: &BTreeMap<usize, FingerprintedTool>,
) -> Result<Vec<ToolDuplicateComparison>, ToolDedupeApplyError> {
    let mut pairs = BTreeSet::new();
    for bucket in buckets.values() {
        let mut representatives = BTreeMap::new();
        for index in bucket {
            if let Some(item) = fingerprinted.get(index) {
                representatives
                    .entry(item.canonical_fingerprint.as_str())
                    .or_insert(item);
            }
        }
        let representatives = representatives.into_values().collect::<Vec<_>>();
        for (offset, left) in representatives.iter().enumerate() {
            for right in &representatives[offset + 1..] {
                pairs.insert((
                    left.record.contract.tool_id.clone(),
                    right.record.contract.tool_id.clone(),
                ));
                if pairs.len() > MAX_TOOL_DEDUPE_PAIRS {
                    return Err(ToolDedupePlanError::TooManyPairs {
                        max_pairs: MAX_TOOL_DEDUPE_PAIRS,
                    }
                    .into());
                }
            }
        }
    }
    let by_id = fingerprinted
        .values()
        .map(|item| (item.record.contract.tool_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    Ok(pairs
        .into_iter()
        .filter_map(|(left, right)| {
            Some(compare_fingerprinted_tools(
                by_id.get(left.as_str())?,
                by_id.get(right.as_str())?,
            ))
        })
        .collect())
}

fn fingerprint_registered_tool(
    workspace_paths: &storage::WorkspacePaths,
    record: ToolContractRecord,
    capability_fingerprint: String,
    normalized_label: String,
) -> Result<(FingerprintedTool, usize), ToolDedupePlanError> {
    let anchor = anchored_tool_directory(workspace_paths, &record.contract.tool_id)?;
    let mut implementation_paths = Vec::new();
    let mut entries_seen = 0_usize;
    collect_implementation_paths(&anchor, "", 0, &mut entries_seen, &mut implementation_paths)?;
    let manifest = read_anchored_tool_json(&anchor)?;
    if manifest != record.contract {
        return Err(ToolDedupePlanError::ContractDrift(
            record.contract.tool_id.clone(),
        ));
    }
    let implementation =
        fingerprint_tool_implementation(&anchor, &record.contract.tool_id, implementation_paths)?;
    validate_declared_implementation_paths(&record.contract, &implementation.files)?;
    let canonical_fingerprint =
        canonical_tool_fingerprint(&record.contract, &implementation.files)?;
    let manifest_after = read_anchored_tool_json(&anchor)?;
    if manifest_after != record.contract {
        return Err(ToolDedupePlanError::ImplementationDrift(
            record.contract.tool_id.clone(),
        ));
    }
    anchor.verify_logical_identity()?;
    let hashed_files = implementation.hashed_files;
    Ok((
        FingerprintedTool {
            record,
            capability_fingerprint,
            canonical_fingerprint,
            implementation_fingerprint: implementation.content_fingerprint,
            normalized_label,
        },
        hashed_files,
    ))
}

/// Validates an explicit technical distinction without retaining its content.
pub fn validate_technical_distinction(value: &str) -> Result<(), ToolTechnicalDistinctionError> {
    if value.is_empty() || value.trim().is_empty() {
        return Err(ToolTechnicalDistinctionError::Blank);
    }
    if value.len() > MAX_TOOL_TECHNICAL_DISTINCTION_BYTES {
        return Err(ToolTechnicalDistinctionError::TooLong {
            max_bytes: MAX_TOOL_TECHNICAL_DISTINCTION_BYTES,
        });
    }
    if value.as_bytes().contains(&0) {
        return Err(ToolTechnicalDistinctionError::ContainsNul);
    }
    Ok(())
}

/// Runs the bounded, read-only creation guard.
///
/// Existing candidates are validated with the same anchored Stage 012
/// fingerprint path. The proposed draft has no implementation bytes, so this
/// function never claims full-fingerprint equality for it.
pub fn preflight_tool_creation(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    input: &DraftToolInput,
    runtime: &DraftToolRuntimeInput,
    technical_distinction_provided: bool,
) -> Result<ToolCreationGuardDecision, ToolCreationGuardError> {
    let (decision, records) = creation_guard_registry_decision(
        connection,
        input,
        runtime,
        technical_distinction_provided,
    )?;
    if matches!(decision, ToolCreationGuardDecision::Duplicate { .. }) {
        return Ok(decision);
    }

    for record in records {
        let capability = capability_fingerprint(&record.contract)?;
        let label = normalized_tool_label(&record.contract);
        let _ = fingerprint_registered_tool(workspace_paths, record, capability, label)?;
    }
    Ok(decision)
}

/// Rechecks registry-only guard facts inside the coordinated mutation.
///
/// Filesystem candidates were already anchored and hashed during immutable
/// preflight. This second pass closes the registry race without hashing twice.
pub fn recheck_tool_creation_registry(
    connection: &Connection,
    input: &DraftToolInput,
    runtime: &DraftToolRuntimeInput,
    technical_distinction_provided: bool,
) -> Result<ToolCreationGuardDecision, ToolCreationGuardError> {
    creation_guard_registry_decision(connection, input, runtime, technical_distinction_provided)
        .map(|(decision, _)| decision)
}

fn creation_guard_registry_decision(
    connection: &Connection,
    input: &DraftToolInput,
    runtime: &DraftToolRuntimeInput,
    technical_distinction_provided: bool,
) -> Result<(ToolCreationGuardDecision, Vec<ToolContractRecord>), ToolCreationGuardError> {
    validate_draft_tool_input_with_runtime(input, runtime)?;
    if let Some(record) = get_tool_contract(connection, &input.tool_id)? {
        let candidate = ToolCreationGuardCandidate {
            canonical_tool_id: record.contract.tool_id,
            alias_suggestion: input.tool_id.clone(),
            class: ToolCreationGuardClass::ActiveIdCollision,
            reasons: vec!["registry_id_collision".to_string()],
        };
        return Ok((
            ToolCreationGuardDecision::Duplicate {
                candidate,
                writes_performed: false,
            },
            Vec::new(),
        ));
    }
    if let Some(alias) = get_tool_alias(connection, &input.tool_id)? {
        let candidate = ToolCreationGuardCandidate {
            canonical_tool_id: alias.canonical_tool_id,
            alias_suggestion: input.tool_id.clone(),
            class: ToolCreationGuardClass::AliasCollision,
            reasons: vec!["registry_alias_collision".to_string()],
        };
        return Ok((
            ToolCreationGuardDecision::Duplicate {
                candidate,
                writes_performed: false,
            },
            Vec::new(),
        ));
    }

    let page = list_tool_contracts_page(connection, None, MAX_TOOL_DEDUPE_TOOLS)?;
    if page.more_results {
        return Err(ToolDedupePlanError::TooManyTools {
            max_tools: MAX_TOOL_DEDUPE_TOOLS,
        }
        .into());
    }
    let proposed = draft_tool_contract_with_runtime(input, runtime);
    let proposed_capability = capability_fingerprint(&proposed)?;
    let proposed_label = normalized_tool_label(&proposed);
    let records = page.items;
    let bm25_scores = creation_bm25_scores(&proposed, &records)?;
    let mut candidates = Vec::new();
    for record in records {
        let score = bm25_scores
            .get(&record.contract.tool_id)
            .copied()
            .unwrap_or(0.0);
        let reasons = creation_overlap_reasons(
            &proposed_capability,
            &proposed_label,
            score,
            &record.contract,
        )?;
        if !reasons.is_empty() {
            candidates.push((record, reasons, score));
        }
    }
    candidates.sort_by(
        |(left_record, left_reasons, left_score), (right_record, right_reasons, right_score)| {
            creation_reason_rank(left_reasons)
                .cmp(&creation_reason_rank(right_reasons))
                .then_with(|| right_score.total_cmp(left_score))
                .then_with(|| {
                    left_record
                        .contract
                        .tool_id
                        .cmp(&right_record.contract.tool_id)
                })
        },
    );
    if candidates.len() > MAX_TOOL_CREATION_GUARD_CANDIDATES {
        return Err(ToolCreationGuardError::TooManyCandidates {
            max_candidates: MAX_TOOL_CREATION_GUARD_CANDIDATES,
        });
    }
    let reviewed_candidates = candidates
        .iter()
        .map(|(record, reasons, _)| ToolCreationGuardCandidate {
            canonical_tool_id: record.contract.tool_id.clone(),
            alias_suggestion: input.tool_id.clone(),
            class: ToolCreationGuardClass::PossibleOverlap,
            reasons: reasons.clone(),
        })
        .collect::<Vec<_>>();
    let records = candidates
        .into_iter()
        .map(|(record, _, _)| record)
        .collect::<Vec<_>>();
    let decision = match reviewed_candidates.first().cloned() {
        Some(candidate) if !technical_distinction_provided => {
            ToolCreationGuardDecision::OverlapReviewRequired {
                candidate,
                writes_performed: false,
            }
        }
        _ => ToolCreationGuardDecision::Allowed {
            technical_distinction_provided,
            reviewed_candidates,
        },
    };
    Ok((decision, records))
}

fn creation_overlap_reasons(
    proposed_capability: &str,
    proposed_label: &str,
    bm25_score: f64,
    existing: &ToolContract,
) -> Result<Vec<String>, ToolDedupePlanError> {
    let mut reasons = Vec::new();
    let label_equal =
        !proposed_label.is_empty() && normalized_tool_label(existing) == proposed_label;
    if label_equal {
        reasons.push("normalized_capability_label_equal".to_string());
    }
    if bm25_score > 0.0 {
        reasons.push("bm25_description_candidate".to_string());
    }
    if reasons.is_empty() {
        return Ok(reasons);
    }
    if capability_fingerprint(existing)? == proposed_capability {
        reasons.push("capability_signature_equal".to_string());
    }
    reasons.sort();
    reasons.dedup();
    Ok(reasons)
}

fn creation_reason_rank(reasons: &[String]) -> u8 {
    if reasons
        .iter()
        .any(|reason| reason == "capability_signature_equal")
    {
        0
    } else if reasons
        .iter()
        .any(|reason| reason == "normalized_capability_label_equal")
    {
        1
    } else {
        2
    }
}

fn creation_bm25_scores(
    proposed: &ToolContract,
    records: &[ToolContractRecord],
) -> Result<BTreeMap<String, f64>, ToolCreationGuardError> {
    const K1: f64 = 1.2;
    const B: f64 = 0.75;
    let query_terms = searchable_tool_terms(proposed)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if query_terms.is_empty() || records.is_empty() {
        return Ok(BTreeMap::new());
    }
    let documents = records
        .iter()
        .map(|record| {
            let terms = searchable_tool_terms(&record.contract);
            let mut frequencies = BTreeMap::<String, usize>::new();
            for term in &terms {
                *frequencies.entry(term.clone()).or_default() += 1;
            }
            (record.contract.tool_id.clone(), terms.len(), frequencies)
        })
        .collect::<Vec<_>>();
    let total_tokens = documents
        .iter()
        .try_fold(query_terms.len(), |total, (_, length, _)| {
            total.checked_add(*length)
        });
    if total_tokens.is_none_or(|total| total > MAX_TOOL_CREATION_GUARD_TOTAL_TOKENS) {
        return Err(ToolCreationGuardError::TooManySearchTokens {
            max_tokens: MAX_TOOL_CREATION_GUARD_TOTAL_TOKENS,
        });
    }
    let mut document_frequency = BTreeMap::<String, usize>::new();
    for term in &query_terms {
        let count = documents
            .iter()
            .filter(|(_, _, frequencies)| frequencies.contains_key(term))
            .count();
        document_frequency.insert(term.clone(), count);
    }
    let document_count = documents.len() as f64;
    let average_length = documents
        .iter()
        .map(|(_, length, _)| *length as f64)
        .sum::<f64>()
        / document_count;
    Ok(documents
        .into_iter()
        .map(|(tool_id, length, frequencies)| {
            let length = length as f64;
            let score = query_terms
                .iter()
                .filter_map(|term| {
                    let frequency = frequencies.get(term).copied()? as f64;
                    let matching_documents =
                        document_frequency.get(term).copied().unwrap_or_default() as f64;
                    let inverse_document_frequency = ((document_count - matching_documents + 0.5)
                        / (matching_documents + 0.5)
                        + 1.0)
                        .ln();
                    let normalization = if average_length > 0.0 {
                        1.0 - B + B * length / average_length
                    } else {
                        1.0
                    };
                    Some(
                        inverse_document_frequency * frequency * (K1 + 1.0)
                            / (frequency + K1 * normalization),
                    )
                })
                .sum();
            (tool_id, score)
        })
        .collect())
}

fn searchable_tool_terms(contract: &ToolContract) -> Vec<String> {
    contract
        .tool_id
        .chars()
        .chain(std::iter::once(' '))
        .chain(contract.name.chars())
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .filter(|token| {
            token.len() >= 3
                && !token.bytes().all(|byte| byte.is_ascii_digit())
                && !matches!(*token, "tool" | "stage" | "test" | "aopmem")
        })
        .take(MAX_TOOL_CREATION_GUARD_TOKENS_PER_DOCUMENT)
        .map(str::to_string)
        .collect()
}

fn add_bucket_pairs(
    buckets: &BTreeMap<String, Vec<usize>>,
    pairs: &mut BTreeSet<(usize, usize)>,
) -> Result<(), ToolDedupePlanError> {
    for indices in buckets.values() {
        let possible_pairs = indices
            .len()
            .checked_mul(indices.len().saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or(ToolDedupePlanError::TooManyPairs {
                max_pairs: MAX_TOOL_DEDUPE_PAIRS,
            })?;
        if possible_pairs > MAX_TOOL_DEDUPE_PAIRS {
            return Err(ToolDedupePlanError::TooManyPairs {
                max_pairs: MAX_TOOL_DEDUPE_PAIRS,
            });
        }
        for (offset, left) in indices.iter().enumerate() {
            for right in &indices[offset + 1..] {
                pairs.insert((*left, *right));
                if pairs.len() > MAX_TOOL_DEDUPE_PAIRS {
                    return Err(ToolDedupePlanError::TooManyPairs {
                        max_pairs: MAX_TOOL_DEDUPE_PAIRS,
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_declared_implementation_paths(
    contract: &ToolContract,
    implementation_files: &BTreeMap<String, String>,
) -> Result<(), ToolDedupePlanError> {
    let required = std::iter::once(contract.command.entrypoint.as_str())
        .chain(std::iter::once(contract.runtime.executable_path.as_str()))
        .chain(contract.platform_launchers.values().map(String::as_str));
    for path in required {
        let normalized = normalized_relative_path(path);
        if !implementation_files.contains_key(&normalized) {
            return Err(ToolDedupePlanError::UnsafeImplementationPath(format!(
                "{}:{normalized}",
                contract.tool_id
            )));
        }
    }
    Ok(())
}

fn compare_fingerprinted_tools(
    left: &FingerprintedTool,
    right: &FingerprintedTool,
) -> ToolDuplicateComparison {
    let full_equal = left.canonical_fingerprint == right.canonical_fingerprint;
    let implementation_equal = left.implementation_fingerprint == right.implementation_fingerprint;
    let capability_equal = left.capability_fingerprint == right.capability_fingerprint;
    let label_equal =
        !left.normalized_label.is_empty() && left.normalized_label == right.normalized_label;

    let (class, reasons) = if full_equal && left.record.contract.name != right.record.contract.name
    {
        (
            ToolDuplicateClass::SameImplementationDifferentName,
            vec![
                "canonical_fingerprint_equal".to_string(),
                "display_name_differs".to_string(),
            ],
        )
    } else if full_equal {
        (
            ToolDuplicateClass::ExactDuplicate,
            vec!["canonical_fingerprint_equal".to_string()],
        )
    } else if implementation_equal {
        (
            ToolDuplicateClass::SameImplementationDifferentName,
            vec![
                "implementation_fingerprint_equal".to_string(),
                "canonical_fingerprint_differs".to_string(),
            ],
        )
    } else if capability_equal {
        (
            ToolDuplicateClass::SameCapabilityDifferentWrapper,
            vec![
                "capability_fingerprint_equal".to_string(),
                "implementation_fingerprint_differs".to_string(),
            ],
        )
    } else if label_equal {
        (
            ToolDuplicateClass::PossibleOverlap,
            vec!["normalized_capability_label_equal".to_string()],
        )
    } else {
        (
            ToolDuplicateClass::Distinct,
            vec!["shortlist_signal_only".to_string()],
        )
    };

    let (canonical, candidate) = select_canonical_record(&left.record, &right.record);
    ToolDuplicateComparison {
        canonical_tool_id: canonical.contract.tool_id.clone(),
        candidate_tool_id: candidate.contract.tool_id.clone(),
        class,
        exact_only_eligible: full_equal,
        reasons,
    }
}

fn select_canonical_record<'a>(
    left: &'a ToolContractRecord,
    right: &'a ToolContractRecord,
) -> (&'a ToolContractRecord, &'a ToolContractRecord) {
    if canonical_record_key(left) <= canonical_record_key(right) {
        (left, right)
    } else {
        (right, left)
    }
}

fn canonical_record_key(record: &ToolContractRecord) -> (u8, u8, usize, &str, &str) {
    (
        if record.contract.status == "active" {
            0
        } else {
            1
        },
        if has_non_neutral_tool_suffix(&record.contract.tool_id) {
            1
        } else {
            0
        },
        record.contract.tool_id.len(),
        record.contract.tool_id.as_str(),
        record.created_at.as_str(),
    )
}

fn has_non_neutral_tool_suffix(tool_id: &str) -> bool {
    const SUFFIXES: &[&str] = &[
        "internal", "user", "windows", "win", "macos", "linux", "wrapper",
    ];
    tool_id
        .split(['-', '_'])
        .next_back()
        .is_some_and(|suffix| SUFFIXES.contains(&suffix))
}

fn normalized_tool_label(contract: &ToolContract) -> String {
    const COSMETIC_TOKENS: &[&str] = &[
        "internal", "user", "windows", "win", "macos", "linux", "wrapper", "tool",
    ];
    let mut tokens = contract
        .name
        .chars()
        .chain(std::iter::once(' '))
        .chain(contract.tool_id.chars())
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .filter(|token| !COSMETIC_TOKENS.contains(token))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    tokens.remove("");
    tokens.into_iter().collect::<Vec<_>>().join("-")
}

fn capability_fingerprint(contract: &ToolContract) -> Result<String, ToolDedupePlanError> {
    let input = CapabilityFingerprintInput {
        args_schema: canonical_json_value(&contract.args_schema),
        output_schema: canonical_json_value(&contract.output_schema),
        side_effects: &contract.side_effects,
        approval_requirement: &contract.approval_requirement,
        timeout_ms: contract.runtime.timeout_ms,
        stdout_limit_bytes: contract.runtime.stdout_limit_bytes,
        stderr_limit_bytes: contract.runtime.stderr_limit_bytes,
        supports_dry_run: contract.runtime.supports_dry_run,
        output_mode: contract.runtime.output_mode.as_str(),
    };
    hash_serializable("aopmem.tool.capability.v1", &input)
}

fn canonical_tool_fingerprint(
    contract: &ToolContract,
    implementation_files: &BTreeMap<String, String>,
) -> Result<String, ToolDedupePlanError> {
    let platform_launchers = contract
        .platform_launchers
        .iter()
        .map(|(platform, path)| (platform.clone(), normalized_relative_path(path)))
        .collect();
    let input = CanonicalFingerprintInput {
        capability: CapabilityFingerprintInput {
            args_schema: canonical_json_value(&contract.args_schema),
            output_schema: canonical_json_value(&contract.output_schema),
            side_effects: &contract.side_effects,
            approval_requirement: &contract.approval_requirement,
            timeout_ms: contract.runtime.timeout_ms,
            stdout_limit_bytes: contract.runtime.stdout_limit_bytes,
            stderr_limit_bytes: contract.runtime.stderr_limit_bytes,
            supports_dry_run: contract.runtime.supports_dry_run,
            output_mode: contract.runtime.output_mode.as_str(),
        },
        command_entrypoint: normalized_relative_path(&contract.command.entrypoint),
        executable_path: normalized_relative_path(&contract.runtime.executable_path),
        runtime_dir: contract
            .runtime
            .runtime_dir
            .as_deref()
            .map(normalized_relative_path),
        platform_launchers,
        implementation_files,
    };
    hash_serializable("aopmem.tool.canonical.v1", &input)
}

fn normalized_relative_path(value: &str) -> String {
    Path::new(value)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn canonical_json_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json_value(value)))
                .collect();
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json_value).collect()),
        other => other.clone(),
    }
}

fn hash_serializable(domain: &str, value: &impl Serialize) -> Result<String, ToolDedupePlanError> {
    let bytes = serde_json::to_vec(value).map_err(|_| ToolDedupePlanError::Serialization)?;
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, domain.as_bytes());
    hash_component(&mut hasher, &bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_component(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn fingerprint_tool_implementation(
    root: &AnchoredDir,
    tool_id: &str,
    mut paths: Vec<String>,
) -> Result<ImplementationFingerprint, ToolDedupePlanError> {
    paths.sort();
    if paths.len() > MAX_TOOL_IMPLEMENTATION_FILES {
        return Err(ToolDedupePlanError::TooManyImplementationFiles {
            tool_id: tool_id.to_string(),
            max_files: MAX_TOOL_IMPLEMENTATION_FILES,
        });
    }

    let mut total_bytes = 0_u64;
    let mut files = BTreeMap::new();
    let mut content_hashes = Vec::with_capacity(paths.len());
    for relative in paths {
        let (directory, name, mut file) = open_anchored_relative_file(root, &relative)?;
        let before = file.metadata()?;
        total_bytes = total_bytes.checked_add(before.len()).ok_or_else(|| {
            ToolDedupePlanError::ImplementationTooLarge {
                tool_id: tool_id.to_string(),
                max_bytes: MAX_TOOL_IMPLEMENTATION_BYTES,
            }
        })?;
        if total_bytes > MAX_TOOL_IMPLEMENTATION_BYTES {
            return Err(ToolDedupePlanError::ImplementationTooLarge {
                tool_id: tool_id.to_string(),
                max_bytes: MAX_TOOL_IMPLEMENTATION_BYTES,
            });
        }
        let digest = hash_open_file_once(&mut file)?;
        #[cfg(test)]
        replace_implementation_after_hash_for_test(&directory.logical_path().join(&name))?;
        let after = file.metadata()?;
        if before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
            || !after.is_file()
        {
            return Err(ToolDedupePlanError::ImplementationDrift(format!(
                "{tool_id}:{relative}"
            )));
        }
        if !directory.regular_child_matches_open_file(&name, &file)? {
            return Err(ToolDedupePlanError::ImplementationDrift(format!(
                "{tool_id}:{relative}"
            )));
        }
        content_hashes.push(digest.clone());
        files.insert(relative, digest);
    }
    content_hashes.sort();
    let content_fingerprint =
        hash_serializable("aopmem.tool.implementation-content.v1", &content_hashes)?;
    Ok(ImplementationFingerprint {
        hashed_files: files.len(),
        files,
        content_fingerprint,
    })
}

fn collect_implementation_paths(
    directory: &AnchoredDir,
    relative_directory: &str,
    depth: usize,
    entries_seen: &mut usize,
    paths: &mut Vec<String>,
) -> Result<(), ToolDedupePlanError> {
    if depth > MAX_TOOL_IMPLEMENTATION_DEPTH {
        return Err(ToolDedupePlanError::UnsafeImplementationPath(
            directory.logical_path().display().to_string(),
        ));
    }
    directory.verify_logical_identity()?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory.logical_path())? {
        *entries_seen = entries_seen.checked_add(1).ok_or(
            ToolDedupePlanError::TooManyImplementationEntries {
                max_entries: MAX_TOOL_IMPLEMENTATION_ENTRIES,
            },
        )?;
        if *entries_seen > MAX_TOOL_IMPLEMENTATION_ENTRIES {
            return Err(ToolDedupePlanError::TooManyImplementationEntries {
                max_entries: MAX_TOOL_IMPLEMENTATION_ENTRIES,
            });
        }
        entries.push(entry?);
    }
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let path = directory.logical_path().join(&name);
        let name = name.to_str().ok_or_else(|| {
            ToolDedupePlanError::UnsafeImplementationPath(path.display().to_string())
        })?;
        let metadata = fs::symlink_metadata(&path)?;
        if tool_root_is_link_or_reparse_point(&metadata) {
            return Err(ToolDedupePlanError::UnsafeImplementationPath(
                path.display().to_string(),
            ));
        }
        let relative = if relative_directory.is_empty() {
            name.to_string()
        } else {
            format!("{relative_directory}/{name}")
        };
        if relative == TOOL_JSON_FILE_NAME {
            directory.open_regular_os(entry.file_name().as_os_str())?;
            continue;
        }
        if metadata.is_dir() {
            let child = directory.child_dir_os(entry.file_name().as_os_str(), false)?;
            collect_implementation_paths(&child, &relative, depth + 1, entries_seen, paths)?;
        } else if metadata.is_file() {
            directory.open_regular_os(entry.file_name().as_os_str())?;
            paths.push(relative);
            if paths.len() > MAX_TOOL_IMPLEMENTATION_FILES {
                return Err(ToolDedupePlanError::TooManyImplementationFiles {
                    tool_id: directory
                        .logical_path()
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("<invalid>")
                        .to_string(),
                    max_files: MAX_TOOL_IMPLEMENTATION_FILES,
                });
            }
        } else {
            return Err(ToolDedupePlanError::UnsafeImplementationPath(
                path.display().to_string(),
            ));
        }
    }
    directory.verify_logical_identity()?;
    Ok(())
}

fn anchored_tool_directory(
    workspace_paths: &storage::WorkspacePaths,
    tool_id: &str,
) -> Result<AnchoredDir, ToolDedupePlanError> {
    if !is_single_tool_directory_name(tool_id) {
        return Err(ToolDedupePlanError::UnsafeImplementationPath(
            tool_id.to_string(),
        ));
    }
    let workspace = AnchoredDir::open_workspace(workspace_paths.root(), None)?;
    let tools_name = workspace_paths.tools().file_name().ok_or_else(|| {
        ToolDedupePlanError::UnsafeImplementationPath(workspace_paths.tools().display().to_string())
    })?;
    let tools = workspace.child_dir_os(tools_name, false)?;
    Ok(tools.child_dir_os(std::ffi::OsStr::new(tool_id), false)?)
}

fn read_anchored_tool_json(root: &AnchoredDir) -> Result<ToolContract, ToolDedupePlanError> {
    let name = std::ffi::OsStr::new(TOOL_JSON_FILE_NAME);
    let mut file = root.open_regular_os(name)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((MAX_TOOL_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if !root.regular_child_matches_open_file(name, &file)? {
        return Err(ToolDedupePlanError::ImplementationDrift(
            "manifest".to_string(),
        ));
    }
    if bytes.len() > MAX_TOOL_MANIFEST_BYTES {
        return Err(ToolDedupePlanError::UnsafeImplementationPath(format!(
            "{}:{TOOL_JSON_FILE_NAME}",
            root.logical_path().display()
        )));
    }
    let contract: ToolContract = serde_json::from_slice(&bytes)
        .map_err(ToolJsonError::Json)
        .map_err(ToolDedupePlanError::Json)?;
    validate_tool_contract(&contract)
        .map_err(ToolJsonError::Validation)
        .map_err(ToolDedupePlanError::Json)?;
    Ok(contract)
}

fn open_anchored_relative_file(
    root: &AnchoredDir,
    relative: &str,
) -> Result<(AnchoredDir, std::ffi::OsString, File), ToolDedupePlanError> {
    let components = Path::new(relative).components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(ToolDedupePlanError::UnsafeImplementationPath(
            relative.to_string(),
        ));
    }
    let mut directory = root.clone();
    for component in &components[..components.len() - 1] {
        let Component::Normal(name) = component else {
            return Err(ToolDedupePlanError::UnsafeImplementationPath(
                relative.to_string(),
            ));
        };
        directory = directory.child_dir_os(name, false)?;
    }
    let Component::Normal(name) = components[components.len() - 1] else {
        return Err(ToolDedupePlanError::UnsafeImplementationPath(
            relative.to_string(),
        ));
    };
    let file = directory.open_regular_os(name)?;
    Ok((directory, name.to_os_string(), file))
}

#[cfg(test)]
fn replace_implementation_after_hash_for_test(current: &Path) -> io::Result<()> {
    let mut hook = IMPLEMENTATION_SWAP_AFTER_HASH
        .lock()
        .map_err(|_| io::Error::other("implementation swap hook lock poisoned"))?;
    let replacement = hook.take();
    if let Some((target, replacement, destination)) = replacement {
        if target != current {
            *hook = Some((target, replacement, destination));
            return Ok(());
        }
        fs::rename(replacement, destination)?;
    }
    Ok(())
}

fn hash_open_file_once(file: &mut File) -> Result<String, ToolDedupePlanError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Adds one direct alias to an active canonical tool.
///
/// This function changes SQLite only. It never creates a directory, manifest,
/// executable, or artifact namespace.
pub fn add_tool_alias(
    connection: &Connection,
    input: &NewToolAlias,
) -> Result<ToolAlias, ToolAliasStorageError> {
    validate_new_tool_alias(input)?;
    if get_tool_alias(connection, &input.alias)?.is_some() {
        return Err(ToolAliasStorageError::AliasAlreadyExists(
            input.alias.clone(),
        ));
    }
    if get_tool_alias(connection, &input.canonical_tool_id)?.is_some() {
        return Err(ToolAliasStorageError::AliasTargetIsAlias(
            input.canonical_tool_id.clone(),
        ));
    }

    let target = get_tool_contract(connection, &input.canonical_tool_id)?.ok_or_else(|| {
        ToolAliasStorageError::CanonicalToolNotFound(input.canonical_tool_id.clone())
    })?;
    if target.contract.status != "active" {
        return Err(ToolAliasStorageError::CanonicalToolNotActive {
            tool_id: input.canonical_tool_id.clone(),
            status: target.contract.status,
        });
    }

    if let Some(direct) = get_tool_contract(connection, &input.alias)? {
        if direct.contract.status != "superseded" {
            return Err(ToolAliasStorageError::AliasShadowsTool {
                tool_id: input.alias.clone(),
                status: direct.contract.status,
            });
        }
    }

    connection.execute(
        "
        INSERT INTO tool_aliases (
            alias, canonical_tool_id, source, status
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            &input.alias,
            &input.canonical_tool_id,
            &input.source,
            &input.status
        ],
    )?;

    get_tool_alias(connection, &input.alias)?
        .ok_or_else(|| ToolAliasStorageError::Db(rusqlite::Error::QueryReturnedNoRows))
}

/// Adds a bounded batch atomically under a nested SQLite savepoint.
pub fn add_tool_aliases(
    connection: &Connection,
    inputs: &[NewToolAlias],
) -> Result<Vec<ToolAlias>, ToolAliasStorageError> {
    if inputs.len() > MAX_TOOL_ALIAS_BATCH_SIZE {
        return Err(ToolAliasValidationError::BatchTooLarge {
            max_items: MAX_TOOL_ALIAS_BATCH_SIZE,
        }
        .into());
    }

    let mut aliases = BTreeSet::new();
    for input in inputs {
        validate_new_tool_alias(input)?;
        if !aliases.insert(input.alias.as_str()) {
            return Err(ToolAliasValidationError::DuplicateBatchAlias(input.alias.clone()).into());
        }
    }

    connection.execute_batch(&format!("SAVEPOINT {TOOL_ALIAS_BULK_SAVEPOINT}"))?;
    let result = inputs
        .iter()
        .map(|input| add_tool_alias(connection, input))
        .collect::<Result<Vec<_>, _>>();
    match result {
        Ok(records) => {
            connection.execute_batch(&format!("RELEASE SAVEPOINT {TOOL_ALIAS_BULK_SAVEPOINT}"))?;
            Ok(records)
        }
        Err(error) => {
            if let Err(rollback_error) = connection.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {TOOL_ALIAS_BULK_SAVEPOINT};\
                 RELEASE SAVEPOINT {TOOL_ALIAS_BULK_SAVEPOINT}"
            )) {
                return Err(ToolAliasStorageError::Db(rollback_error));
            }
            Err(error)
        }
    }
}

/// Looks up one direct tool alias by exact id.
pub fn get_tool_alias(
    connection: &Connection,
    alias: &str,
) -> Result<Option<ToolAlias>, ToolAliasStorageError> {
    validate_tool_alias_id("alias", alias)?;
    connection
        .query_row(
            "
            SELECT alias, canonical_tool_id, created_at, source, status
            FROM tool_aliases
            WHERE alias = ?1
            ",
            [alias],
            row_to_tool_alias,
        )
        .optional()
        .map_err(ToolAliasStorageError::Db)
}

/// Lists every direct tool alias in deterministic exact-id order.
pub fn list_tool_aliases(connection: &Connection) -> Result<Vec<ToolAlias>, ToolAliasStorageError> {
    let mut statement = connection.prepare(
        "
        SELECT alias, canonical_tool_id, created_at, source, status
        FROM tool_aliases
        ORDER BY alias ASC
        ",
    )?;
    let aliases = statement
        .query_map([], row_to_tool_alias)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(aliases)
}

/// Loads aliases for a bounded canonical page in one indexed SQL query.
pub fn list_tool_aliases_for_canonical_ids(
    connection: &Connection,
    canonical_tool_ids: &[String],
) -> Result<BTreeMap<String, Vec<ToolAlias>>, ToolAliasStorageError> {
    if canonical_tool_ids.len() > MAX_TOOL_ALIAS_PAGE_SIZE {
        return Err(ToolAliasValidationError::PageTooLarge {
            max_items: MAX_TOOL_ALIAS_PAGE_SIZE,
        }
        .into());
    }
    let mut grouped = canonical_tool_ids
        .iter()
        .map(|tool_id| (tool_id.clone(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    if canonical_tool_ids.is_empty() {
        return Ok(grouped);
    }
    for tool_id in canonical_tool_ids {
        validate_tool_alias_id("canonical_tool_id", tool_id)?;
    }
    let placeholders = (1..=canonical_tool_ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "
        SELECT alias, canonical_tool_id, created_at, source, status
        FROM tool_aliases
        WHERE canonical_tool_id IN ({placeholders})
          AND status = 'active'
        ORDER BY canonical_tool_id ASC, alias ASC
        "
    );
    let mut statement = connection.prepare(&sql)?;
    let aliases = statement
        .query_map(
            params_from_iter(canonical_tool_ids.iter()),
            row_to_tool_alias,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for alias in aliases {
        grouped
            .entry(alias.canonical_tool_id.clone())
            .or_default()
            .push(alias);
    }
    Ok(grouped)
}

/// Lists one bounded, stable keyset page of direct tool aliases.
pub fn list_tool_aliases_page(
    connection: &Connection,
    after_alias: Option<&str>,
    limit: usize,
) -> Result<ToolAliasesPage, ToolAliasStorageError> {
    if limit > MAX_TOOL_ALIAS_PAGE_SIZE {
        return Err(ToolAliasValidationError::PageTooLarge {
            max_items: MAX_TOOL_ALIAS_PAGE_SIZE,
        }
        .into());
    }
    if let Some(after_alias) = after_alias {
        validate_tool_alias_id("after_alias", after_alias)?;
    }
    if limit == 0 {
        return Ok(ToolAliasesPage {
            items: Vec::new(),
            next_after_alias: None,
            more_results: false,
        });
    }

    let query_limit = i64::try_from(limit)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let mut items = match after_alias {
        Some(after_alias) => {
            let mut statement = connection.prepare(
                "
                SELECT alias, canonical_tool_id, created_at, source, status
                FROM tool_aliases
                WHERE alias > ?1
                ORDER BY alias ASC
                LIMIT ?2
                ",
            )?;
            let page = statement
                .query_map(params![after_alias, query_limit], row_to_tool_alias)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            page
        }
        None => {
            let mut statement = connection.prepare(
                "
                SELECT alias, canonical_tool_id, created_at, source, status
                FROM tool_aliases
                ORDER BY alias ASC
                LIMIT ?1
                ",
            )?;
            let page = statement
                .query_map([query_limit], row_to_tool_alias)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            page
        }
    };

    let more_results = items.len() > limit;
    if more_results {
        items.pop();
    }
    let next_after_alias = more_results
        .then(|| items.last().map(|record| record.alias.clone()))
        .flatten();
    Ok(ToolAliasesPage {
        items,
        next_after_alias,
        more_results,
    })
}

/// Removes one exact alias without touching its canonical tool files.
pub fn remove_tool_alias(
    connection: &Connection,
    alias: &str,
) -> Result<Option<ToolAlias>, ToolAliasStorageError> {
    let existing = get_tool_alias(connection, alias)?;
    if existing.is_some() {
        connection.execute("DELETE FROM tool_aliases WHERE alias = ?1", [alias])?;
    }
    Ok(existing)
}

/// Resolves one requested id with stable precedence.
///
/// Direct non-superseded tools win first. An active alias to an active target
/// wins over a direct superseded record. A superseded direct record remains a
/// final compatibility fallback.
pub fn resolve_tool_id(
    connection: &Connection,
    requested_id: &str,
) -> Result<Option<ToolIdResolution>, ToolAliasStorageError> {
    validate_tool_alias_id("requested_id", requested_id)?;
    connection
        .query_row(
            "
            WITH candidates (
                precedence, requested_id, canonical_tool_id,
                matched_alias, canonical_status, resolution_kind
            ) AS (
                SELECT
                    0, ?1, tool_id, NULL, status, 'direct_canonical'
                FROM tool_contracts
                WHERE tool_id = ?1 AND status <> 'superseded'
                UNION ALL
                SELECT
                    1, ?1, aliases.canonical_tool_id, aliases.alias,
                    canonical.status, 'alias'
                FROM tool_aliases AS aliases
                JOIN tool_contracts AS canonical
                  ON canonical.tool_id = aliases.canonical_tool_id
                WHERE aliases.alias = ?1
                  AND aliases.status = 'active'
                  AND canonical.status = 'active'
                UNION ALL
                SELECT
                    2, ?1, tool_id, NULL, status, 'superseded_fallback'
                FROM tool_contracts
                WHERE tool_id = ?1 AND status = 'superseded'
            )
            SELECT
                requested_id, canonical_tool_id, matched_alias,
                canonical_status, resolution_kind
            FROM candidates
            ORDER BY precedence ASC
            LIMIT 1
            ",
            [requested_id],
            row_to_tool_id_resolution,
        )
        .optional()
        .map_err(ToolAliasStorageError::Db)
}

/// Writes `tool.json` under `tools/<tool-id>/`.
pub fn write_tool_json(
    workspace_paths: &storage::WorkspacePaths,
    contract: &ToolContract,
) -> Result<PathBuf, ToolJsonError> {
    validate_tool_contract(contract)?;
    ensure_tools_root_stays_in_workspace(workspace_paths)?;

    let tool_dir = tool_dir(workspace_paths, &contract.tool_id);
    fs::create_dir_all(&tool_dir)?;
    ensure_tool_root_stays_in_workspace_tools_dir(workspace_paths, &contract.tool_id)?;

    let manifest_path = tool_dir.join(TOOL_JSON_FILE_NAME);
    let manifest_json = serde_json::to_vec_pretty(contract)?;
    fs::write(&manifest_path, manifest_json)?;

    Ok(manifest_path)
}

/// Reads and validates `tool.json` from `tools/<tool-id>/`.
pub fn read_tool_json(
    workspace_paths: &storage::WorkspacePaths,
    tool_id: &str,
) -> Result<ToolContract, ToolJsonError> {
    ensure_tool_root_stays_in_workspace_tools_dir(workspace_paths, tool_id)?;
    let manifest_path = tool_json_path(workspace_paths, tool_id);
    let manifest_json = fs::read(&manifest_path)?;
    let contract: ToolContract = serde_json::from_slice(&manifest_json)?;
    validate_tool_contract(&contract)?;
    Ok(contract)
}

/// Creates a draft tool directory, writes `tool.json`, and registers it in SQLite.
pub fn create_draft_tool(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    input: &DraftToolInput,
) -> Result<DraftToolRecord, CreateDraftToolError> {
    create_draft_tool_with_publish(
        workspace_paths,
        connection,
        input,
        |staging_root, tool_root| fs::rename(staging_root, tool_root),
    )
}

/// Creates a draft while the workspace mutation coordinator owns the active
/// SQLite transaction. The final directory is registered for rollback if the
/// outer transaction cannot commit.
pub fn create_draft_tool_in_mutation(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    input: &DraftToolInput,
    effects: &mut MutationEffects,
) -> Result<DraftToolRecord, CreateDraftToolError> {
    create_draft_tool_in_mutation_with_runtime(
        workspace_paths,
        connection,
        input,
        &DraftToolRuntimeInput::default(),
        effects,
    )
}

/// Creates a draft with an explicit persisted runtime contract while the
/// workspace mutation coordinator owns the active SQLite transaction.
pub fn create_draft_tool_in_mutation_with_runtime(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    input: &DraftToolInput,
    runtime: &DraftToolRuntimeInput,
    effects: &mut MutationEffects,
) -> Result<DraftToolRecord, CreateDraftToolError> {
    ensure_tools_root_stays_in_workspace(workspace_paths)?;
    create_draft_tool_without_transaction(
        workspace_paths,
        connection,
        input,
        runtime,
        |staging_root, tool_root| fs::rename(staging_root, tool_root),
        effects,
    )
}

/// Rechecks the creation guard before the first write and creates the draft
/// only when the registry decision still permits it.
pub fn create_guarded_draft_tool_in_mutation(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    input: &DraftToolInput,
    runtime: &DraftToolRuntimeInput,
    technical_distinction_provided: bool,
    effects: &mut MutationEffects,
) -> Result<GuardedDraftToolRecord, GuardedCreateDraftError> {
    let decision =
        recheck_tool_creation_registry(connection, input, runtime, technical_distinction_provided)?;
    if !decision.is_allowed() {
        return Err(GuardedCreateDraftError::Blocked(decision));
    }
    let draft = create_draft_tool_in_mutation_with_runtime(
        workspace_paths,
        connection,
        input,
        runtime,
        effects,
    )?;
    Ok(GuardedDraftToolRecord {
        draft,
        technical_distinction_provided,
    })
}

fn create_draft_tool_with_publish(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    input: &DraftToolInput,
    publish: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<DraftToolRecord, CreateDraftToolError> {
    ensure_tools_root_stays_in_workspace(workspace_paths)?;
    let transaction =
        rusqlite::Transaction::new_unchecked(connection, rusqlite::TransactionBehavior::Immediate)
            .map_err(ToolContractStorageError::Db)?;
    let mut effects = MutationEffects::default();
    let result = create_draft_tool_without_transaction(
        workspace_paths,
        &transaction,
        input,
        &DraftToolRuntimeInput::default(),
        publish,
        &mut effects,
    );
    let record = match result {
        Ok(record) => record,
        Err(error) => {
            let _ = transaction.rollback();
            effects.rollback_created_directories_best_effort();
            return Err(error);
        }
    };
    if let Err(error) = transaction.commit() {
        effects.rollback_created_directories_best_effort();
        return Err(ToolContractStorageError::Db(error).into());
    }
    effects.disarm();
    Ok(record)
}

fn create_draft_tool_without_transaction(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    input: &DraftToolInput,
    runtime: &DraftToolRuntimeInput,
    publish: impl FnOnce(&Path, &Path) -> io::Result<()>,
    effects: &mut MutationEffects,
) -> Result<DraftToolRecord, CreateDraftToolError> {
    let contract = draft_tool_contract_with_runtime(input, runtime);
    validate_tool_contract(&contract).map_err(ToolContractStorageError::Validation)?;
    let manifest = serde_json::to_vec_pretty(&contract).map_err(ToolContractStorageError::Json)?;
    let tool_root = tool_dir(workspace_paths, &contract.tool_id);
    if tool_root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("tool directory already exists: {}", tool_root.display()),
        )
        .into());
    }

    let staging_root = stage_draft_tool_layout(workspace_paths.tools(), &manifest)?;
    let record = match create_tool_contract(connection, &contract) {
        Ok(record) => record,
        Err(error) => {
            remove_staged_tool_dir(&staging_root);
            return Err(error.into());
        }
    };

    if let Err(error) = publish(&staging_root, &tool_root) {
        remove_staged_tool_dir(&staging_root);
        return Err(error.into());
    }
    effects.register_created_directory(tool_root.clone());

    let tool_json_path = tool_root.join(TOOL_JSON_FILE_NAME);
    let bin_dir = tool_root.join(TOOL_BIN_DIR_NAME);
    let runtime_dir = tool_root.join(TOOL_RUNTIME_DIR_NAME);

    Ok(DraftToolRecord {
        record,
        tool_dir: tool_root.display().to_string(),
        tool_json_path: tool_json_path.display().to_string(),
        bin_dir: bin_dir.display().to_string(),
        runtime_dir: runtime_dir.display().to_string(),
    })
}

fn stage_draft_tool_layout(tools_root: &Path, manifest: &[u8]) -> io::Result<PathBuf> {
    let staging_root = create_draft_tool_staging_dir(tools_root)?;
    let stage_result = (|| {
        fs::create_dir(staging_root.join(TOOL_BIN_DIR_NAME))?;
        fs::create_dir(staging_root.join(TOOL_RUNTIME_DIR_NAME))?;
        fs::write(staging_root.join(TOOL_JSON_FILE_NAME), manifest)?;
        Ok(())
    })();

    if let Err(error) = stage_result {
        remove_staged_tool_dir(&staging_root);
        return Err(error);
    }

    Ok(staging_root)
}

fn create_draft_tool_staging_dir(tools_root: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(tools_root)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..16 {
        let staging_root = tools_root.join(format!(
            "{DRAFT_TOOL_STAGING_PREFIX}{}-{timestamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&staging_root) {
            Ok(()) => return Ok(staging_root),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique draft tool staging directory",
    ))
}

fn remove_staged_tool_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

/// Validates one registered tool and its local executable reference.
pub fn validate_tool(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
) -> Result<ToolValidationRecord, ValidateToolError> {
    let contract = load_canonical_tool_contract(workspace_paths, connection, tool_id)?;
    let tool_root = ensure_tool_root_stays_in_workspace_tools_dir(workspace_paths, tool_id)?;
    let executable_path = resolve_executable_path(&tool_root, &contract.runtime.executable_path);
    if !executable_path.is_file() {
        return Err(ValidateToolError::MissingExecutablePath(
            executable_path.display().to_string(),
        ));
    }
    ensure_executable_stays_in_tool_dir(
        &tool_root,
        &executable_path,
        &contract.runtime.executable_path,
    )
    .map_err(ToolJsonError::Validation)?;
    let runtime = contract.runtime.clone();

    Ok(ToolValidationRecord {
        tool_id: contract.tool_id,
        tool_json_path: tool_json_path(workspace_paths, tool_id)
            .display()
            .to_string(),
        executable_path: executable_path.display().to_string(),
        side_effects: contract.side_effects,
        approval_requirement: contract.approval_requirement,
        runner_dry_run_supported: true,
        runtime,
    })
}

/// Plans one registered tool invocation without executing implementation code.
pub fn dry_run_tool(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
    args: &[String],
) -> Result<ToolDryRunRecord, RunToolError> {
    let contract = load_canonical_tool_contract(workspace_paths, connection, tool_id)?;
    let tool_root = tool_dir(workspace_paths, tool_id);
    let executable_path = resolve_executable_path(&tool_root, &contract.runtime.executable_path);

    Ok(ToolDryRunRecord {
        mode: "dry_run".to_string(),
        tool_id: contract.tool_id.clone(),
        tool_json_path: tool_json_path(workspace_paths, tool_id)
            .display()
            .to_string(),
        executable_path: executable_path.display().to_string(),
        args: args.to_vec(),
        side_effects: contract.side_effects.clone(),
        approval_requirement: contract.approval_requirement.clone(),
        approval_required: requires_approval(&contract),
        would_execute: false,
    })
}

/// Runs one registered tool through its validated `tool.json` runtime metadata.
///
/// The process cwd is the tool root. Runtime resources must be addressed through
/// the validated relative `runtime.runtime_dir`; a shebang process must not use
/// its concrete `$0` launch pathname for resource discovery.
pub fn run_tool(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
    args: &[String],
    approved: Option<&str>,
) -> Result<ToolRunRecord, RunToolError> {
    let mut trace = ToolRunTrace::default();
    run_tool_with_trace(
        workspace_paths,
        connection,
        tool_id,
        args,
        approved,
        &mut trace,
    )
}

pub(crate) fn run_tool_with_trace(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
    args: &[String],
    approved: Option<&str>,
    trace: &mut ToolRunTrace,
) -> Result<ToolRunRecord, RunToolError> {
    let contract = load_canonical_tool_contract(workspace_paths, connection, tool_id)?;
    let limits = ToolRunLimits::from_runtime(&contract.runtime)?;
    run_loaded_tool_contract(
        workspace_paths,
        connection,
        tool_id,
        args,
        approved,
        LoadedToolRun { contract, limits },
        trace,
    )
}

/// Runs one registered tool with explicit bounded process resources.
pub fn run_tool_with_limits(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
    args: &[String],
    approved: Option<&str>,
    limits: ToolRunLimits,
) -> Result<ToolRunRecord, RunToolError> {
    let mut trace = ToolRunTrace::default();
    let contract = load_canonical_tool_contract(workspace_paths, connection, tool_id)?;
    validate_tool_run_limits(limits)?;
    run_loaded_tool_contract(
        workspace_paths,
        connection,
        tool_id,
        args,
        approved,
        LoadedToolRun { contract, limits },
        &mut trace,
    )
}

fn run_loaded_tool_contract(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
    args: &[String],
    approved: Option<&str>,
    loaded: LoadedToolRun,
    trace: &mut ToolRunTrace,
) -> Result<ToolRunRecord, RunToolError> {
    let LoadedToolRun { contract, limits } = loaded;
    if !can_run_tool(&contract, approved) {
        return Err(RunToolError::UnsafeActionBlocked {
            tool_id: contract.tool_id,
            side_effects: contract.side_effects,
            approval_requirement: contract.approval_requirement,
        });
    }

    let tool_root = tool_dir(workspace_paths, tool_id);
    ensure_tool_root_stays_in_workspace_tools_dir(workspace_paths, tool_id)?;
    let executable_path = resolve_executable_path(&tool_root, &contract.runtime.executable_path);
    if !executable_path.is_file() {
        return Err(RunToolError::MissingExecutablePath(
            executable_path.display().to_string(),
        ));
    }
    ensure_executable_stays_in_tool_dir(
        &tool_root,
        &executable_path,
        &contract.runtime.executable_path,
    )
    .map_err(ToolJsonError::Validation)?;
    trace.validation_succeeded = true;

    let output_mode = contract.runtime.output_mode;
    let (output, staging) = match output_mode {
        ToolOutputMode::Inline => (
            run_bounded_tool_process(&executable_path, &tool_root, args, limits, trace)?,
            None,
        ),
        ToolOutputMode::Artifact => {
            let (staging, files) = ToolArtifactStaging::create(workspace_paths, connection)
                .map_err(run_tool_artifact_error)?;
            let output = run_artifact_tool_process(
                &executable_path,
                &tool_root,
                args,
                limits,
                files,
                trace,
            )?;
            (output, Some(staging))
        }
    };
    let exit_code = output.status.code().unwrap_or(-1);
    if !output.status.success() {
        return Err(RunToolError::ProcessFailed(exit_code));
    }

    let artifacts = if output_mode == ToolOutputMode::Artifact
        && (output.stdout.truncated || output.stderr.truncated)
    {
        maybe_fail_artifact_publish()?;
        let paths = staging
            .ok_or_else(|| io::Error::other("artifact output staging was not available"))?
            .publish()
            .map_err(run_tool_artifact_error)?;
        Some(ToolRunArtifacts {
            stdout: ToolRunArtifactStream {
                path: paths.stdout,
                bytes: output.stdout.total_bytes,
                preview_truncated: output.stdout.truncated,
            },
            stderr: ToolRunArtifactStream {
                path: paths.stderr,
                bytes: output.stderr.total_bytes,
                preview_truncated: output.stderr.truncated,
            },
        })
    } else {
        None
    };

    Ok(ToolRunRecord {
        tool_id: contract.tool_id,
        tool_json_path: tool_json_path(workspace_paths, tool_id)
            .display()
            .to_string(),
        executable_path: executable_path.display().to_string(),
        args: args.to_vec(),
        exit_code,
        stdout: String::from_utf8_lossy(&output.stdout.bytes).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr.bytes).into_owned(),
        artifacts,
    })
}

fn run_tool_artifact_error(error: artifacts::ArtifactError) -> RunToolError {
    match error {
        artifacts::ArtifactError::Io(error) => RunToolError::Io(error),
        artifacts::ArtifactError::Db(error) => RunToolError::Db(error),
        artifacts::ArtifactError::InvalidDay(day) => RunToolError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid SQLite local artifact day: {day}"),
        )),
        error @ artifacts::ArtifactError::LockTimeout { .. } => {
            RunToolError::Io(io::Error::new(io::ErrorKind::TimedOut, error.to_string()))
        }
        error @ (artifacts::ArtifactError::CleanupPartial { .. }
        | artifacts::ArtifactError::CleanupStateUnknown { .. }
        | artifacts::ArtifactError::RetentionLimitNotMet { .. }) => {
            RunToolError::Io(io::Error::other(error.to_string()))
        }
    }
}

#[cfg(not(test))]
fn maybe_fail_artifact_read() -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
fn maybe_fail_artifact_read() -> io::Result<()> {
    if ARTIFACT_FAILURE_MODE.load(Ordering::SeqCst) == ARTIFACT_FAILURE_READ {
        Err(io::Error::other("forced tool artifact read failure"))
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
fn maybe_fail_artifact_write() -> io::Result<()> {
    Ok(())
}

#[cfg(not(test))]
fn maybe_fail_artifact_sync() -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
fn maybe_fail_artifact_sync() -> io::Result<()> {
    if ARTIFACT_FAILURE_MODE.load(Ordering::SeqCst) == ARTIFACT_FAILURE_SYNC {
        Err(io::Error::other("forced tool artifact sync failure"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
fn maybe_fail_artifact_write() -> io::Result<()> {
    if ARTIFACT_FAILURE_MODE.load(Ordering::SeqCst) == ARTIFACT_FAILURE_WRITE {
        Err(io::Error::other("forced tool artifact write failure"))
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
fn maybe_fail_artifact_publish() -> Result<(), RunToolError> {
    Ok(())
}

#[cfg(test)]
fn maybe_fail_artifact_publish() -> Result<(), RunToolError> {
    if ARTIFACT_FAILURE_MODE.load(Ordering::SeqCst) == ARTIFACT_FAILURE_PUBLISH {
        Err(RunToolError::Io(io::Error::other(
            "forced tool artifact publish failure",
        )))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
struct BoundedToolProcessOutput {
    status: std::process::ExitStatus,
    stdout: BoundedToolStream,
    stderr: BoundedToolStream,
}

#[derive(Debug)]
struct BoundedToolStream {
    bytes: Vec<u8>,
    truncated: bool,
    total_bytes: u64,
    hard_overflowed: bool,
}

#[cfg(target_os = "macos")]
struct DarwinExecutableAnchor {
    root_fd: std::os::fd::OwnedFd,
    name: std::ffi::CString,
}

#[cfg(target_os = "macos")]
impl Drop for DarwinExecutableAnchor {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;

        // Remove only this invocation's UUID anchor through its held root fd.
        // A prefix sweep could unlink another concurrent invocation, so stale
        // crash residue is left for a future ownership-safe cleanup design.
        let _unlink_result =
            unsafe { libc::unlinkat(self.root_fd.as_raw_fd(), self.name.as_ptr(), 0) };
    }
}

struct PreparedToolCommand {
    command: Command,
    #[cfg(target_os = "macos")]
    _root_fd: std::os::fd::OwnedFd,
    #[cfg(target_os = "macos")]
    _executable_fd: std::os::fd::OwnedFd,
    #[cfg(target_os = "macos")]
    _executable_anchor: DarwinExecutableAnchor,
    #[cfg(windows)]
    _path_handles: Vec<WindowsOwnedHandle>,
}

#[cfg(not(any(target_os = "macos", windows)))]
fn prepare_tool_command(
    executable_path: &Path,
    tool_root: &Path,
    args: &[String],
) -> io::Result<PreparedToolCommand> {
    let mut command = Command::new(executable_path);
    command
        .current_dir(tool_root)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(PreparedToolCommand { command })
}

#[cfg(windows)]
fn prepare_tool_command(
    executable_path: &Path,
    tool_root: &Path,
    args: &[String],
) -> io::Result<PreparedToolCommand> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetFileInformationByHandle, GetFinalPathNameByHandleW,
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING, VOLUME_NAME_DOS,
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct WindowsFileIdentity {
        volume_serial: u32,
        file_index: u64,
    }

    fn normalize_windows_path(path: &Path) -> String {
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase()
    }

    fn opened_path(handle: &WindowsOwnedHandle) -> io::Result<PathBuf> {
        const WINDOWS_MAX_PATH_CHARS: usize = 32_768;
        let mut path = vec![0u16; WINDOWS_MAX_PATH_CHARS];
        let length = unsafe {
            GetFinalPathNameByHandleW(
                handle.0,
                path.as_mut_ptr(),
                u32::try_from(path.len())
                    .map_err(|_| io::Error::other("Windows path buffer is too large"))?,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        };
        let length = usize::try_from(length)
            .map_err(|_| io::Error::other("Windows opened path length is invalid"))?;
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if length >= path.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows opened path exceeded the bounded path buffer",
            ));
        }
        path.truncate(length);
        Ok(PathBuf::from(std::ffi::OsString::from_wide(&path)))
    }

    fn open_stable_path(
        path: &Path,
        directory: bool,
    ) -> io::Result<(WindowsOwnedHandle, WindowsFileIdentity)> {
        let mut path = path.as_os_str().encode_wide().collect::<Vec<_>>();
        path.push(0);
        let mut flags = FILE_FLAG_OPEN_REPARSE_POINT;
        if directory {
            flags |= FILE_FLAG_BACKUP_SEMANTICS;
        }
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                flags,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let handle = WindowsOwnedHandle(handle);
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        if unsafe { GetFileInformationByHandle(handle.0, &mut information) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "validated Windows tool path became a reparse point before spawn",
            ));
        }
        let identity = WindowsFileIdentity {
            volume_serial: information.dwVolumeSerialNumber,
            file_index: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        };
        Ok((handle, identity))
    }

    let canonical_root = fs::canonicalize(tool_root)?;
    let canonical_executable = fs::canonicalize(executable_path)?;
    let relative_executable = canonical_executable
        .strip_prefix(&canonical_root)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "validated Windows tool executable is outside its tool root",
            )
        })?;
    let (root_handle, root_identity) = open_stable_path(&canonical_root, true)?;
    let (_root_verification_handle, root_verification_identity) =
        open_stable_path(&canonical_root, true)?;
    if root_identity != root_verification_identity
        || normalize_windows_path(&opened_path(&root_handle)?)
            != normalize_windows_path(&canonical_root)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "opened Windows tool root did not match the validated path",
        ));
    }
    let mut path_handles = vec![root_handle];
    let mut current = canonical_root.clone();
    let components = relative_executable.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validated Windows tool executable path is empty",
        ));
    }
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "validated Windows tool executable contains a non-normal component",
            ));
        };
        current.push(name);
        let is_directory = index + 1 != components.len();
        let (handle, identity) = open_stable_path(&current, is_directory)?;
        let (_verification_handle, verification_identity) =
            open_stable_path(&current, is_directory)?;
        if identity != verification_identity
            || identity.volume_serial != root_identity.volume_serial
            || normalize_windows_path(&opened_path(&handle)?)
                != normalize_windows_path(&fs::canonicalize(&current)?)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "opened Windows tool component did not match the validated path identity",
            ));
        }
        path_handles.push(handle);
    }
    let mut command = Command::new(executable_path);
    command
        .current_dir(tool_root)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(PreparedToolCommand {
        command,
        _path_handles: path_handles,
    })
}

#[cfg(target_os = "macos")]
fn darwin_clone_fallback_forced() -> bool {
    #[cfg(test)]
    {
        DARWIN_FORCE_CLONE_FALLBACK.load(Ordering::SeqCst)
    }
    #[cfg(not(test))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
fn darwin_mutate_source_during_fallback() -> bool {
    #[cfg(test)]
    {
        DARWIN_MUTATE_SOURCE_DURING_FALLBACK.load(Ordering::SeqCst)
    }
    #[cfg(not(test))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
fn prepare_tool_command(
    executable_path: &Path,
    tool_root: &Path,
    args: &[String],
) -> io::Result<PreparedToolCommand> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::process::CommandExt;

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct FileIdentity {
        device: u64,
        inode: u64,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct FileState {
        identity: FileIdentity,
        len: u64,
        mode: u32,
        modified_seconds: i64,
        modified_nanoseconds: i64,
        changed_seconds: i64,
        changed_nanoseconds: i64,
    }

    fn metadata_identity(metadata: &fs::Metadata) -> FileIdentity {
        FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    fn metadata_state(metadata: &fs::Metadata) -> FileState {
        FileState {
            identity: metadata_identity(metadata),
            len: metadata.len(),
            mode: metadata.mode(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    fn fd_identity(fd: libc::c_int) -> io::Result<FileIdentity> {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let stat = unsafe { stat.assume_init() };
        Ok(FileIdentity {
            device: stat.st_dev as u64,
            inode: stat.st_ino,
        })
    }

    fn open_path(path: &Path, flags: libc::c_int) -> io::Result<OwnedFd> {
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "validated tool path contains an interior NUL byte",
            )
        })?;
        let fd = unsafe { libc::open(path.as_ptr(), flags) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }

    fn openat_component(
        directory_fd: libc::c_int,
        component: &std::ffi::OsStr,
        flags: libc::c_int,
    ) -> io::Result<OwnedFd> {
        let component = CString::new(component.as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "validated tool path contains an interior NUL byte",
            )
        })?;
        let fd = unsafe { libc::openat(directory_fd, component.as_ptr(), flags) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }

    fn createat_component(
        directory_fd: libc::c_int,
        component: &CString,
        mode: u32,
    ) -> io::Result<OwnedFd> {
        let fd = unsafe {
            libc::openat(
                directory_fd,
                component.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                mode as libc::c_uint,
            )
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }

    let expected_root = fs::metadata(tool_root)?;
    let expected_executable = fs::metadata(executable_path)?;
    let root_fd = open_path(
        tool_root,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
    )?;
    if fd_identity(root_fd.as_raw_fd())? != metadata_identity(&expected_root) {
        return Err(io::Error::other(
            "validated tool root changed before process spawn",
        ));
    }

    let relative_executable = executable_path.strip_prefix(tool_root).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "validated tool executable is outside its stable tool root",
        )
    })?;
    let components = relative_executable.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validated tool executable path is empty",
        ));
    }

    let mut current_directory = None;
    let mut directory_fd = root_fd.as_raw_fd();
    for component in &components[..components.len() - 1] {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "validated tool executable contains a non-normal path component",
            ));
        };
        let next = openat_component(
            directory_fd,
            name,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )?;
        directory_fd = next.as_raw_fd();
        current_directory = Some(next);
    }
    let Component::Normal(executable_name) = components[components.len() - 1] else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validated tool executable contains a non-normal final component",
        ));
    };
    let executable_fd = openat_component(
        directory_fd,
        executable_name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
    )?;
    let opened_executable = File::from(executable_fd.try_clone()?);
    let expected_executable_state = metadata_state(&expected_executable);
    let opened_executable_state = metadata_state(&opened_executable.metadata()?);
    if opened_executable_state != expected_executable_state
        || fd_identity(executable_fd.as_raw_fd())? != expected_executable_state.identity
    {
        return Err(io::Error::other(
            "validated tool executable changed before process spawn",
        ));
    }
    if expected_executable.len() > MAX_TOOL_IMPLEMENTATION_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "validated tool executable exceeds the snapshot byte ceiling",
        ));
    }

    let anchor_name = format!(".aopmem-exec-{}", uuid::Uuid::new_v4());
    let anchor_name_c = CString::new(anchor_name.as_bytes())
        .map_err(|_| io::Error::other("generated executable anchor name was invalid"))?;
    drop(current_directory);
    let anchor_root_fd = unsafe { libc::fcntl(root_fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
    if anchor_root_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let executable_anchor = DarwinExecutableAnchor {
        root_fd: unsafe { OwnedFd::from_raw_fd(anchor_root_fd) },
        name: anchor_name_c,
    };
    let executable_mode = expected_executable.permissions().mode() & 0o777;
    const CLONE_NOOWNERCOPY: u32 = 0x0002;
    unsafe extern "C" {
        fn fclonefileat(
            source_fd: libc::c_int,
            destination_directory_fd: libc::c_int,
            destination_name: *const libc::c_char,
            flags: u32,
        ) -> libc::c_int;
    }
    let clone_result = if darwin_clone_fallback_forced() {
        -1
    } else {
        unsafe {
            fclonefileat(
                executable_fd.as_raw_fd(),
                root_fd.as_raw_fd(),
                executable_anchor.name.as_ptr(),
                CLONE_NOOWNERCOPY,
            )
        }
    };
    if clone_result != 0 {
        let clone_error = if darwin_clone_fallback_forced() {
            io::Error::from_raw_os_error(libc::ENOTSUP)
        } else {
            io::Error::last_os_error()
        };
        if !matches!(
            clone_error.raw_os_error(),
            Some(libc::ENOTSUP) | Some(libc::EXDEV)
        ) {
            return Err(clone_error);
        }
        let anchor_fd = createat_component(
            root_fd.as_raw_fd(),
            &executable_anchor.name,
            executable_mode,
        )?;
        let mut source = File::from(executable_fd.try_clone()?);
        source.seek(SeekFrom::Start(0))?;
        let mut snapshot = File::from(anchor_fd);
        let copied = io::copy(
            &mut source.take(MAX_TOOL_IMPLEMENTATION_BYTES.saturating_add(1)),
            &mut snapshot,
        )?;
        if copied != expected_executable.len() {
            return Err(io::Error::other(
                "validated tool executable snapshot was incomplete",
            ));
        }
        snapshot.set_permissions(fs::Permissions::from_mode(executable_mode))?;
        snapshot.sync_all()?;
        if darwin_mutate_source_during_fallback() {
            let mut source_path = fs::OpenOptions::new().write(true).open(executable_path)?;
            source_path.write_all(b"\n# mutated during snapshot\n")?;
            source_path.sync_all()?;
        }
    }
    let final_executable = File::from(executable_fd.try_clone()?);
    if metadata_state(&final_executable.metadata()?) != opened_executable_state {
        return Err(io::Error::other(
            "validated tool executable changed while creating its stable snapshot",
        ));
    }
    // Darwin has no fd-relative exec. An APFS clone in the already-opened root
    // gives execve an O(1), metadata-preserving snapshot even if an executable
    // ancestor is swapped. Unsupported filesystems use a bounded fd-to-fd copy.
    // A separate inode also avoids endpoint-security rejection of new hardlinks.
    let stable_executable_path = PathBuf::from(format!("./{anchor_name}"));
    let root_raw_fd = root_fd.as_raw_fd();
    let mut command = Command::new(stable_executable_path);
    command
        .arg0(executable_path.as_os_str())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(move || {
            if libc::fchdir(root_raw_fd) == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        });
    }

    Ok(PreparedToolCommand {
        command,
        _root_fd: root_fd,
        _executable_fd: executable_fd,
        _executable_anchor: executable_anchor,
    })
}

fn run_bounded_tool_process(
    executable_path: &Path,
    tool_root: &Path,
    args: &[String],
    limits: ToolRunLimits,
    trace: &mut ToolRunTrace,
) -> Result<BoundedToolProcessOutput, RunToolError> {
    validate_tool_run_limits(limits)?;

    let mut prepared = prepare_tool_command(executable_path, tool_root, args)?;
    let (mut child, mut process_tree) =
        spawn_tool_process(&mut prepared.command, executable_path, trace)?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            best_effort_cleanup_tool_process(&mut process_tree, &mut child);
            return Err(io::Error::other("tool process stdout pipe was not available").into());
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            best_effort_cleanup_tool_process(&mut process_tree, &mut child);
            return Err(io::Error::other("tool process stderr pipe was not available").into());
        }
    };
    let (stream_sender, stream_receiver) = mpsc::channel();
    spawn_bounded_output_reader(
        stdout,
        limits.stdout_max_bytes,
        ToolStreamKind::Stdout,
        stream_sender.clone(),
    );
    spawn_bounded_output_reader(
        stderr,
        limits.stderr_max_bytes,
        ToolStreamKind::Stderr,
        stream_sender.clone(),
    );
    drop(stream_sender);

    let started = Instant::now();
    let mut stdout = ToolStreamState::default();
    let mut stderr = ToolStreamState::default();
    let termination = loop {
        if let Err(error) = refresh_tool_process_tree(&mut process_tree, &mut child) {
            best_effort_cleanup_tool_process(&mut process_tree, &mut child);
            receive_streams_bounded(
                &stream_receiver,
                &mut stdout,
                &mut stderr,
                TOOL_RUN_CLEANUP_TIMEOUT,
            );
            return Err(error.into());
        }
        receive_ready_streams(&stream_receiver, &mut stdout, &mut stderr);
        if stdout.failed() || stderr.failed() {
            break ToolProcessTermination::StreamFailure;
        }
        if stdout.overflowed || stderr.overflowed {
            break ToolProcessTermination::OutputOverflow;
        }

        match child.try_wait() {
            Ok(Some(status)) => break ToolProcessTermination::Completed(status),
            Ok(None) => {}
            Err(error) => {
                best_effort_cleanup_tool_process(&mut process_tree, &mut child);
                receive_streams_bounded(
                    &stream_receiver,
                    &mut stdout,
                    &mut stderr,
                    TOOL_RUN_CLEANUP_TIMEOUT,
                );
                return Err(error.into());
            }
        }

        if started.elapsed() >= limits.timeout {
            break ToolProcessTermination::TimedOut;
        }

        let remaining = limits.timeout.saturating_sub(started.elapsed());
        thread::sleep(TOOL_RUN_POLL_INTERVAL.min(remaining));
    };
    // A direct parent may exit while a descendant still owns inherited pipes.
    // Always close the whole isolated tree before collecting reader results.
    if let Err(error) = terminate_tool_process_tree(&mut process_tree, &mut child) {
        best_effort_cleanup_tool_process(&mut process_tree, &mut child);
        receive_streams_bounded(
            &stream_receiver,
            &mut stdout,
            &mut stderr,
            TOOL_RUN_CLEANUP_TIMEOUT,
        );
        return Err(error.into());
    }
    if let Err(error) = reap_tool_process_bounded(
        &mut child,
        matches!(termination, ToolProcessTermination::Completed(_)),
    ) {
        best_effort_cleanup_tool_process(&mut process_tree, &mut child);
        receive_streams_bounded(
            &stream_receiver,
            &mut stdout,
            &mut stderr,
            TOOL_RUN_CLEANUP_TIMEOUT,
        );
        return Err(error.into());
    }
    receive_streams_bounded(
        &stream_receiver,
        &mut stdout,
        &mut stderr,
        TOOL_RUN_CLEANUP_TIMEOUT,
    );

    let stdout_truncated = stdout.truncated();
    let stderr_truncated = stderr.truncated();
    match termination {
        ToolProcessTermination::TimedOut => Err(ToolRunLimitError::TimedOut {
            timeout_ms: limits.timeout.as_millis(),
            stdout_limit_bytes: limits.stdout_max_bytes,
            stderr_limit_bytes: limits.stderr_max_bytes,
            stdout_truncated,
            stderr_truncated,
        }
        .into()),
        ToolProcessTermination::OutputOverflow => Err(ToolRunLimitError::OutputOverflow {
            timeout_ms: limits.timeout.as_millis(),
            stdout_limit_bytes: limits.stdout_max_bytes,
            stderr_limit_bytes: limits.stderr_max_bytes,
            stdout_truncated,
            stderr_truncated,
        }
        .into()),
        ToolProcessTermination::Completed(status) => {
            let stdout = stdout.take_completed("stdout")?;
            let stderr = stderr.take_completed("stderr")?;
            if stdout.truncated || stderr.truncated {
                return Err(ToolRunLimitError::OutputOverflow {
                    timeout_ms: limits.timeout.as_millis(),
                    stdout_limit_bytes: limits.stdout_max_bytes,
                    stderr_limit_bytes: limits.stderr_max_bytes,
                    stdout_truncated: stdout.truncated,
                    stderr_truncated: stderr.truncated,
                }
                .into());
            }

            Ok(BoundedToolProcessOutput {
                status,
                stdout,
                stderr,
            })
        }
        ToolProcessTermination::StreamFailure => {
            stdout.take_completed("stdout")?;
            stderr.take_completed("stderr")?;
            Err(io::Error::other("tool output reader failed without an error").into())
        }
    }
}

fn run_artifact_tool_process(
    executable_path: &Path,
    tool_root: &Path,
    args: &[String],
    limits: ToolRunLimits,
    files: ToolArtifactCaptureFiles,
    trace: &mut ToolRunTrace,
) -> Result<BoundedToolProcessOutput, RunToolError> {
    validate_tool_run_limits(limits)?;

    let mut prepared = prepare_tool_command(executable_path, tool_root, args)?;
    let (mut child, mut process_tree) =
        spawn_tool_process(&mut prepared.command, executable_path, trace)?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            best_effort_cleanup_tool_process(&mut process_tree, &mut child);
            return Err(io::Error::other("tool process stdout pipe was not available").into());
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            best_effort_cleanup_tool_process(&mut process_tree, &mut child);
            return Err(io::Error::other("tool process stderr pipe was not available").into());
        }
    };
    let (stream_sender, stream_receiver) = mpsc::channel();
    spawn_artifact_output_reader(
        stdout,
        files.stdout,
        limits.stdout_max_bytes,
        ToolStreamKind::Stdout,
        stream_sender.clone(),
    );
    spawn_artifact_output_reader(
        stderr,
        files.stderr,
        limits.stderr_max_bytes,
        ToolStreamKind::Stderr,
        stream_sender.clone(),
    );
    drop(stream_sender);

    let started = Instant::now();
    let mut stdout = ArtifactToolStreamState::default();
    let mut stderr = ArtifactToolStreamState::default();
    let termination = loop {
        if let Err(error) = refresh_tool_process_tree(&mut process_tree, &mut child) {
            best_effort_cleanup_tool_process(&mut process_tree, &mut child);
            receive_artifact_streams_bounded(
                &stream_receiver,
                &mut stdout,
                &mut stderr,
                TOOL_RUN_CLEANUP_TIMEOUT,
            );
            return Err(error.into());
        }
        receive_ready_artifact_streams(&stream_receiver, &mut stdout, &mut stderr);
        if stdout.failed() || stderr.failed() {
            break ArtifactProcessTermination::StreamFailure;
        }
        if stdout.hard_overflowed || stderr.hard_overflowed {
            break ArtifactProcessTermination::HardOutputOverflow;
        }

        match child.try_wait() {
            Ok(Some(status)) => break ArtifactProcessTermination::Completed(status),
            Ok(None) => {}
            Err(error) => {
                best_effort_cleanup_tool_process(&mut process_tree, &mut child);
                receive_artifact_streams_bounded(
                    &stream_receiver,
                    &mut stdout,
                    &mut stderr,
                    TOOL_RUN_CLEANUP_TIMEOUT,
                );
                return Err(error.into());
            }
        }

        if started.elapsed() >= limits.timeout {
            break ArtifactProcessTermination::TimedOut;
        }

        let remaining = limits.timeout.saturating_sub(started.elapsed());
        thread::sleep(TOOL_RUN_POLL_INTERVAL.min(remaining));
    };

    if let Err(error) = terminate_tool_process_tree(&mut process_tree, &mut child) {
        best_effort_cleanup_tool_process(&mut process_tree, &mut child);
        receive_artifact_streams_bounded(
            &stream_receiver,
            &mut stdout,
            &mut stderr,
            TOOL_RUN_CLEANUP_TIMEOUT,
        );
        return Err(error.into());
    }
    if let Err(error) = reap_tool_process_bounded(
        &mut child,
        matches!(termination, ArtifactProcessTermination::Completed(_)),
    ) {
        best_effort_cleanup_tool_process(&mut process_tree, &mut child);
        receive_artifact_streams_bounded(
            &stream_receiver,
            &mut stdout,
            &mut stderr,
            TOOL_RUN_CLEANUP_TIMEOUT,
        );
        return Err(error.into());
    }
    receive_artifact_streams_bounded(
        &stream_receiver,
        &mut stdout,
        &mut stderr,
        TOOL_RUN_CLEANUP_TIMEOUT,
    );

    let stdout_truncated = stdout.truncated();
    let stderr_truncated = stderr.truncated();
    let stdout_hard_limit_exceeded = stdout.hard_overflowed;
    let stderr_hard_limit_exceeded = stderr.hard_overflowed;
    match termination {
        ArtifactProcessTermination::TimedOut => Err(ToolRunLimitError::TimedOut {
            timeout_ms: limits.timeout.as_millis(),
            stdout_limit_bytes: limits.stdout_max_bytes,
            stderr_limit_bytes: limits.stderr_max_bytes,
            stdout_truncated,
            stderr_truncated,
        }
        .into()),
        ArtifactProcessTermination::HardOutputOverflow => {
            Err(ToolRunLimitError::ArtifactHardOverflow {
                timeout_ms: limits.timeout.as_millis(),
                stdout_limit_bytes: limits.stdout_max_bytes,
                stderr_limit_bytes: limits.stderr_max_bytes,
                hard_limit_bytes: MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES,
                stdout_truncated,
                stderr_truncated,
                stdout_hard_limit_exceeded,
                stderr_hard_limit_exceeded,
            }
            .into())
        }
        ArtifactProcessTermination::Completed(status) => {
            let stdout = stdout.take_completed("stdout")?;
            let stderr = stderr.take_completed("stderr")?;
            if stdout.hard_overflowed || stderr.hard_overflowed {
                return Err(ToolRunLimitError::ArtifactHardOverflow {
                    timeout_ms: limits.timeout.as_millis(),
                    stdout_limit_bytes: limits.stdout_max_bytes,
                    stderr_limit_bytes: limits.stderr_max_bytes,
                    hard_limit_bytes: MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES,
                    stdout_truncated: stdout.truncated,
                    stderr_truncated: stderr.truncated,
                    stdout_hard_limit_exceeded: stdout.hard_overflowed,
                    stderr_hard_limit_exceeded: stderr.hard_overflowed,
                }
                .into());
            }
            Ok(BoundedToolProcessOutput {
                status,
                stdout,
                stderr,
            })
        }
        ArtifactProcessTermination::StreamFailure => {
            stdout.take_completed("stdout")?;
            stderr.take_completed("stderr")?;
            Err(io::Error::other("tool artifact output reader failed without an error").into())
        }
    }
}

#[derive(Debug)]
enum ArtifactProcessTermination {
    Completed(std::process::ExitStatus),
    TimedOut,
    HardOutputOverflow,
    StreamFailure,
}

#[derive(Debug)]
enum ArtifactToolStreamEvent {
    PreviewOverflow(ToolStreamKind),
    HardOverflow(ToolStreamKind),
    Finished {
        kind: ToolStreamKind,
        result: io::Result<BoundedToolStream>,
    },
}

#[derive(Debug, Default)]
struct ArtifactToolStreamState {
    result: Option<io::Result<BoundedToolStream>>,
    preview_overflowed: bool,
    hard_overflowed: bool,
}

impl ArtifactToolStreamState {
    fn failed(&self) -> bool {
        matches!(self.result, Some(Err(_)))
    }

    fn truncated(&self) -> bool {
        self.preview_overflowed
            || self
                .result
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .is_some_and(|stream| stream.truncated)
    }

    fn take_completed(&mut self, stream_name: &str) -> io::Result<BoundedToolStream> {
        self.result.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "tool artifact {stream_name} pipe did not close after process-tree termination"
                ),
            )
        })?
    }
}

fn spawn_artifact_output_reader<R>(
    reader: R,
    file: fs::File,
    preview_limit: usize,
    kind: ToolStreamKind,
    sender: Sender<ArtifactToolStreamEvent>,
) where
    R: Read + Send + 'static,
{
    let _reader = thread::spawn(move || {
        let preview_sender = sender.clone();
        let hard_sender = sender.clone();
        let result = read_artifact_output(
            reader,
            file,
            preview_limit,
            MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES,
            || {
                let _send_result =
                    preview_sender.send(ArtifactToolStreamEvent::PreviewOverflow(kind));
            },
            || {
                let _send_result = hard_sender.send(ArtifactToolStreamEvent::HardOverflow(kind));
            },
        );
        let _send_result = sender.send(ArtifactToolStreamEvent::Finished { kind, result });
    });
}

fn read_artifact_output<R: Read>(
    mut reader: R,
    mut file: fs::File,
    preview_limit: usize,
    hard_limit: usize,
    on_preview_overflow: impl FnOnce(),
    on_hard_overflow: impl FnOnce(),
) -> io::Result<BoundedToolStream> {
    let mut preview = Vec::new();
    let mut preview_truncated = false;
    let mut hard_overflowed = false;
    let mut total_bytes = 0_u64;
    let mut captured_bytes = 0_usize;
    let mut buffer = [0_u8; 8192];
    let mut on_preview_overflow = Some(on_preview_overflow);
    let mut on_hard_overflow = Some(on_hard_overflow);

    loop {
        maybe_fail_artifact_read()?;
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(read as u64);

        let preview_remaining = preview_limit.saturating_sub(preview.len());
        let preview_kept = read.min(preview_remaining);
        preview.extend_from_slice(&buffer[..preview_kept]);
        if preview_kept < read {
            preview_truncated = true;
            if let Some(notify) = on_preview_overflow.take() {
                notify();
            }
        }

        let capture_remaining = hard_limit.saturating_sub(captured_bytes);
        let captured = read.min(capture_remaining);
        if captured > 0 {
            maybe_fail_artifact_write()?;
            file.write_all(&buffer[..captured])?;
            captured_bytes += captured;
        }
        if captured < read {
            hard_overflowed = true;
            if let Some(notify) = on_hard_overflow.take() {
                notify();
            }
        }
    }

    if !hard_overflowed {
        file.flush()?;
        maybe_fail_artifact_sync()?;
        file.sync_all()?;
    }

    Ok(BoundedToolStream {
        bytes: preview,
        truncated: preview_truncated,
        total_bytes,
        hard_overflowed,
    })
}

fn receive_ready_artifact_streams(
    receiver: &Receiver<ArtifactToolStreamEvent>,
    stdout: &mut ArtifactToolStreamState,
    stderr: &mut ArtifactToolStreamState,
) {
    while let Ok(event) = receiver.try_recv() {
        store_artifact_stream_event(event, stdout, stderr);
    }
}

fn receive_artifact_streams_bounded(
    receiver: &Receiver<ArtifactToolStreamEvent>,
    stdout: &mut ArtifactToolStreamState,
    stderr: &mut ArtifactToolStreamState,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    while (stdout.result.is_none() || stderr.result.is_none()) && Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(TOOL_RUN_POLL_INTERVAL.min(remaining)) {
            Ok(event) => store_artifact_stream_event(event, stdout, stderr),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn store_artifact_stream_event(
    event: ArtifactToolStreamEvent,
    stdout: &mut ArtifactToolStreamState,
    stderr: &mut ArtifactToolStreamState,
) {
    match event {
        ArtifactToolStreamEvent::PreviewOverflow(ToolStreamKind::Stdout) => {
            stdout.preview_overflowed = true;
        }
        ArtifactToolStreamEvent::PreviewOverflow(ToolStreamKind::Stderr) => {
            stderr.preview_overflowed = true;
        }
        ArtifactToolStreamEvent::HardOverflow(ToolStreamKind::Stdout) => {
            stdout.hard_overflowed = true;
        }
        ArtifactToolStreamEvent::HardOverflow(ToolStreamKind::Stderr) => {
            stderr.hard_overflowed = true;
        }
        ArtifactToolStreamEvent::Finished {
            kind: ToolStreamKind::Stdout,
            result,
        } => {
            stdout.hard_overflowed |= result
                .as_ref()
                .is_ok_and(|stream| stream.total_bytes > MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES as u64);
            stdout.result = Some(result);
        }
        ArtifactToolStreamEvent::Finished {
            kind: ToolStreamKind::Stderr,
            result,
        } => {
            stderr.hard_overflowed |= result
                .as_ref()
                .is_ok_and(|stream| stream.total_bytes > MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES as u64);
            stderr.result = Some(result);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolStreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug)]
enum ToolStreamEvent {
    Overflow(ToolStreamKind),
    Finished {
        kind: ToolStreamKind,
        result: io::Result<BoundedToolStream>,
    },
}

#[derive(Debug, Default)]
struct ToolStreamState {
    result: Option<io::Result<BoundedToolStream>>,
    overflowed: bool,
}

impl ToolStreamState {
    fn failed(&self) -> bool {
        matches!(self.result, Some(Err(_)))
    }

    fn truncated(&self) -> bool {
        self.overflowed
            || self
                .result
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .is_some_and(|stream| stream.truncated)
    }

    fn take_completed(&mut self, stream_name: &str) -> io::Result<BoundedToolStream> {
        self.result.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "tool process {stream_name} pipe did not close after process-tree termination"
                ),
            )
        })?
    }
}

#[derive(Debug)]
enum ToolProcessTermination {
    Completed(std::process::ExitStatus),
    TimedOut,
    OutputOverflow,
    StreamFailure,
}

fn spawn_bounded_output_reader<R>(
    reader: R,
    max_bytes: usize,
    kind: ToolStreamKind,
    sender: Sender<ToolStreamEvent>,
) where
    R: Read + Send + 'static,
{
    let _reader = thread::spawn(move || {
        let overflow_sender = sender.clone();
        let result = read_bounded_output_with_overflow(reader, max_bytes, || {
            let _send_result = overflow_sender.send(ToolStreamEvent::Overflow(kind));
        });
        let _send_result = sender.send(ToolStreamEvent::Finished { kind, result });
    });
}

#[cfg(test)]
fn read_bounded_output<R: Read>(mut reader: R, max_bytes: usize) -> io::Result<BoundedToolStream> {
    read_bounded_output_with_overflow(&mut reader, max_bytes, || {})
}

fn read_bounded_output_with_overflow<R: Read>(
    mut reader: R,
    max_bytes: usize,
    on_overflow: impl FnOnce(),
) -> io::Result<BoundedToolStream> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut total_bytes = 0_u64;
    let mut buffer = [0_u8; 8192];
    let mut on_overflow = Some(on_overflow);

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(read as u64);

        let remaining = max_bytes.saturating_sub(bytes.len());
        let kept = read.min(remaining);
        bytes.extend_from_slice(&buffer[..kept]);
        truncated |= kept < read;
        if truncated {
            if let Some(notify) = on_overflow.take() {
                notify();
            }
        }
    }

    Ok(BoundedToolStream {
        bytes,
        truncated,
        total_bytes,
        hard_overflowed: false,
    })
}

fn receive_ready_streams(
    receiver: &Receiver<ToolStreamEvent>,
    stdout: &mut ToolStreamState,
    stderr: &mut ToolStreamState,
) {
    while let Ok(event) = receiver.try_recv() {
        store_stream_event(event, stdout, stderr);
    }
}

fn receive_streams_bounded(
    receiver: &Receiver<ToolStreamEvent>,
    stdout: &mut ToolStreamState,
    stderr: &mut ToolStreamState,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    while (stdout.result.is_none() || stderr.result.is_none()) && Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(TOOL_RUN_POLL_INTERVAL.min(remaining)) {
            Ok(event) => store_stream_event(event, stdout, stderr),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn store_stream_event(
    event: ToolStreamEvent,
    stdout: &mut ToolStreamState,
    stderr: &mut ToolStreamState,
) {
    match event {
        ToolStreamEvent::Overflow(ToolStreamKind::Stdout) => stdout.overflowed = true,
        ToolStreamEvent::Overflow(ToolStreamKind::Stderr) => stderr.overflowed = true,
        ToolStreamEvent::Finished {
            kind: ToolStreamKind::Stdout,
            result,
        } => stdout.result = Some(result),
        ToolStreamEvent::Finished {
            kind: ToolStreamKind::Stderr,
            result,
        } => stderr.result = Some(result),
    }
}

fn reap_tool_process_bounded(
    child: &mut std::process::Child,
    direct_process_completed: bool,
) -> io::Result<()> {
    if direct_process_completed {
        return Ok(());
    }

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(error) => return Err(error),
        }

        if started.elapsed() >= TOOL_RUN_CLEANUP_TIMEOUT {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "tool process did not exit after process-tree termination",
            ));
        }
        thread::sleep(TOOL_RUN_POLL_INTERVAL);
    }
}

fn best_effort_cleanup_tool_process(
    process_tree: &mut ToolProcessTree,
    child: &mut std::process::Child,
) {
    let _terminate_result = terminate_tool_process_tree(process_tree, child);
    let _kill_result = child.kill();
    let _reap_result = reap_tool_process_bounded(child, false);
}

fn validate_tool_run_limits(limits: ToolRunLimits) -> Result<(), ToolRunLimitError> {
    if limits.timeout == Duration::ZERO
        || limits.timeout > MAX_TOOL_RUN_TIMEOUT
        || limits.stdout_max_bytes == 0
        || limits.stdout_max_bytes > MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES
        || limits.stderr_max_bytes == 0
        || limits.stderr_max_bytes > MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES
    {
        return Err(ToolRunLimitError::InvalidLimits {
            timeout_ms: limits.timeout.as_millis(),
            stdout_max_bytes: limits.stdout_max_bytes as u128,
            stderr_max_bytes: limits.stderr_max_bytes as u128,
        });
    }
    Ok(())
}

fn invalid_tool_run_limits(
    timeout_ms: u64,
    stdout_max_bytes: u64,
    stderr_max_bytes: u64,
) -> ToolRunLimitError {
    ToolRunLimitError::InvalidLimits {
        timeout_ms: u128::from(timeout_ms),
        stdout_max_bytes: u128::from(stdout_max_bytes),
        stderr_max_bytes: u128::from(stderr_max_bytes),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
struct ToolProcessTree;

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_tool_process(
    command: &mut Command,
    _expected_executable: &Path,
    trace: &mut ToolRunTrace,
) -> io::Result<(std::process::Child, ToolProcessTree)> {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
    let child = command.spawn()?;
    trace.process_spawned = true;
    Ok((child, ToolProcessTree))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn refresh_tool_process_tree(
    _process_tree: &mut ToolProcessTree,
    _child: &mut std::process::Child,
) -> io::Result<()> {
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn terminate_tool_process_tree(
    _process_tree: &mut ToolProcessTree,
    child: &mut std::process::Child,
) -> io::Result<()> {
    use std::os::raw::c_int;

    unsafe extern "C" {
        fn kill(pid: c_int, signal: c_int) -> c_int;
    }

    const SIGKILL: c_int = 9;
    let process_group = c_int::try_from(child.id()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "tool process id does not fit the platform process-id type",
        )
    })?;
    let result = unsafe { kill(-process_group, SIGKILL) };
    if result == 0 {
        return Ok(());
    }

    let error = io::Error::last_os_error();
    const ESRCH: i32 = 3;
    if error.kind() == io::ErrorKind::NotFound || error.raw_os_error() == Some(ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(target_os = "macos")]
const DARWIN_MAX_TRACKED_TOOL_PROCESSES: usize = 32_768;
#[cfg(target_os = "macos")]
const DARWIN_ZOMBIE_PROCESS_STATUS: u32 = 5;

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DarwinProcessIdentity {
    pid: libc::pid_t,
    started_seconds: u64,
    started_microseconds: u64,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct DarwinProcessDetails {
    identity: DarwinProcessIdentity,
    parent_pid: libc::pid_t,
    process_group: libc::pid_t,
    status: u32,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct DarwinProcBsdInfo {
    flags: u32,
    status: u32,
    exit_status: u32,
    pid: u32,
    parent_pid: u32,
    uid: libc::uid_t,
    gid: libc::gid_t,
    real_uid: libc::uid_t,
    real_gid: libc::gid_t,
    saved_uid: libc::uid_t,
    saved_gid: libc::gid_t,
    reserved: u32,
    command: [libc::c_char; 16],
    name: [libc::c_char; 32],
    open_file_count: u32,
    process_group: u32,
    job_control_count: u32,
    controlling_device: u32,
    terminal_process_group: u32,
    nice: i32,
    started_seconds: u64,
    started_microseconds: u64,
}

#[cfg(target_os = "macos")]
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_listchildpids(
        parent_pid: libc::pid_t,
        buffer: *mut libc::c_void,
        buffer_size: libc::c_int,
    ) -> libc::c_int;
    fn proc_listpgrppids(
        process_group: libc::pid_t,
        buffer: *mut libc::c_void,
        buffer_size: libc::c_int,
    ) -> libc::c_int;
    fn proc_pidinfo(
        pid: libc::c_int,
        flavor: libc::c_int,
        argument: u64,
        buffer: *mut libc::c_void,
        buffer_size: libc::c_int,
    ) -> libc::c_int;
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct DarwinDescendantTracker {
    root: DarwinProcessIdentity,
    tracked: std::collections::BTreeMap<libc::pid_t, DarwinProcessIdentity>,
}

#[cfg(target_os = "macos")]
impl DarwinDescendantTracker {
    fn new(root: DarwinProcessIdentity) -> Self {
        Self {
            root,
            tracked: std::iter::once((root.pid, root)).collect(),
        }
    }

    fn insert(&mut self, identity: DarwinProcessIdentity) -> io::Result<bool> {
        self.insert_with_ceiling(identity, DARWIN_MAX_TRACKED_TOOL_PROCESSES)
    }

    fn insert_with_ceiling(
        &mut self,
        identity: DarwinProcessIdentity,
        ceiling: usize,
    ) -> io::Result<bool> {
        if let Some(existing) = self.tracked.get(&identity.pid) {
            if existing == &identity {
                return Ok(false);
            }
            self.tracked.insert(identity.pid, identity);
            return Ok(true);
        }
        if self.tracked.len() >= ceiling {
            return Err(io::Error::other(
                "macOS tool process tree exceeded the tracked-process ceiling",
            ));
        }
        self.tracked.insert(identity.pid, identity);
        Ok(true)
    }

    fn refresh(&mut self, child: &mut std::process::Child) -> io::Result<usize> {
        let mut queue = std::collections::VecDeque::new();
        for identity in self.tracked.values().copied().collect::<Vec<_>>() {
            match darwin_tracked_process_details(identity, self.root, child)? {
                Some(details) if details.identity == identity => queue.push_back(identity),
                _ => {
                    self.tracked.remove(&identity.pid);
                }
            }
        }

        let mut discovered = 0usize;
        let mut scanned = std::collections::BTreeSet::new();
        while let Some(parent) = queue.pop_front() {
            if !scanned.insert(parent.pid) {
                continue;
            }
            let remaining = DARWIN_MAX_TRACKED_TOOL_PROCESSES.saturating_sub(self.tracked.len());
            for child_pid in darwin_child_pids(parent.pid, remaining)? {
                let Some(details) = darwin_process_details(child_pid)? else {
                    continue;
                };
                if details.parent_pid != parent.pid {
                    continue;
                }
                if self.insert(details.identity)? {
                    discovered += 1;
                }
                queue.push_back(details.identity);
            }
        }
        Ok(discovered)
    }

    fn live_identities(
        &self,
        child: &mut std::process::Child,
    ) -> io::Result<Vec<DarwinProcessIdentity>> {
        let mut live = Vec::with_capacity(self.tracked.len());
        for identity in self.tracked.values().copied() {
            if matches!(
                darwin_tracked_process_details(identity, self.root, child)?,
                Some(details)
                    if details.identity == identity
                        && details.status != DARWIN_ZOMBIE_PROCESS_STATUS
            ) {
                live.push(identity);
            }
        }
        Ok(live)
    }
}

#[cfg(target_os = "macos")]
fn darwin_process_details(pid: libc::pid_t) -> io::Result<Option<DarwinProcessDetails>> {
    darwin_process_details_with(pid, |pid, flavor, argument, buffer, buffer_size| {
        let read = unsafe { proc_pidinfo(pid, flavor, argument, buffer, buffer_size) };
        if read <= 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(read)
        }
    })
}

#[cfg(target_os = "macos")]
fn darwin_process_details_with<F>(
    pid: libc::pid_t,
    mut query: F,
) -> io::Result<Option<DarwinProcessDetails>>
where
    F: FnMut(
        libc::c_int,
        libc::c_int,
        u64,
        *mut libc::c_void,
        libc::c_int,
    ) -> io::Result<libc::c_int>,
{
    const PROC_PIDTBSDINFO: libc::c_int = 3;
    let buffer_size = libc::c_int::try_from(std::mem::size_of::<DarwinProcBsdInfo>())
        .map_err(|_| io::Error::other("macOS process-info structure is too large"))?;
    let mut info = std::mem::MaybeUninit::<DarwinProcBsdInfo>::uninit();
    let read = match query(
        pid,
        PROC_PIDTBSDINFO,
        0,
        info.as_mut_ptr().cast(),
        buffer_size,
    ) {
        Ok(read) => read,
        Err(error) if darwin_process_is_absent(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    if read == buffer_size {
        let info = unsafe { info.assume_init() };
        return Ok(Some(DarwinProcessDetails {
            identity: DarwinProcessIdentity {
                pid: libc::pid_t::try_from(info.pid)
                    .map_err(|_| io::Error::other("macOS process id is outside pid_t"))?,
                started_seconds: info.started_seconds,
                started_microseconds: info.started_microseconds,
            },
            parent_pid: libc::pid_t::try_from(info.parent_pid)
                .map_err(|_| io::Error::other("macOS parent process id is outside pid_t"))?,
            process_group: libc::pid_t::try_from(info.process_group)
                .map_err(|_| io::Error::other("macOS process group is outside pid_t"))?,
            status: info.status,
        }));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "macOS returned an incomplete process identity",
    ))
}

#[cfg(target_os = "macos")]
fn darwin_tracked_process_details(
    identity: DarwinProcessIdentity,
    root: DarwinProcessIdentity,
    child: &mut std::process::Child,
) -> io::Result<Option<DarwinProcessDetails>> {
    darwin_tracked_process_details_with(
        identity,
        root,
        || darwin_process_details(identity.pid),
        || child.try_wait().map(|status| status.is_some()),
    )
}

#[cfg(target_os = "macos")]
fn darwin_tracked_process_details_with<Q, C>(
    identity: DarwinProcessIdentity,
    root: DarwinProcessIdentity,
    query: Q,
    direct_process_completed: C,
) -> io::Result<Option<DarwinProcessDetails>>
where
    Q: FnOnce() -> io::Result<Option<DarwinProcessDetails>>,
    C: FnOnce() -> io::Result<bool>,
{
    match query() {
        Err(error)
            if error.raw_os_error() == Some(libc::EPERM)
                && identity == root
                && direct_process_completed()? =>
        {
            Ok(None)
        }
        result => result,
    }
}

#[cfg(target_os = "macos")]
fn darwin_child_pids(
    parent_pid: libc::pid_t,
    remaining_capacity: usize,
) -> io::Result<Vec<libc::pid_t>> {
    darwin_child_pids_with(
        parent_pid,
        remaining_capacity,
        |parent_pid, buffer, buffer_size| {
            let returned = unsafe { proc_listchildpids(parent_pid, buffer, buffer_size) };
            if returned < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(returned)
            }
        },
    )
}

#[cfg(target_os = "macos")]
fn darwin_child_pids_with<F>(
    parent_pid: libc::pid_t,
    remaining_capacity: usize,
    mut list_children: F,
) -> io::Result<Vec<libc::pid_t>>
where
    F: FnMut(libc::pid_t, *mut libc::c_void, libc::c_int) -> io::Result<libc::c_int>,
{
    let estimated_count = match list_children(parent_pid, std::ptr::null_mut(), 0) {
        Ok(estimated_count) => estimated_count,
        Err(error) if darwin_process_is_absent(&error) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    if estimated_count == 0 {
        return Ok(Vec::new());
    }
    if remaining_capacity == 0 {
        return Err(io::Error::other(
            "macOS tool process tree exceeded the tracked-process ceiling",
        ));
    }

    let maximum_allocation = remaining_capacity.saturating_add(1);
    let mut allocation_count = usize::try_from(estimated_count)
        .map_err(|_| io::Error::other("macOS child-process list size is invalid"))?
        .saturating_add(16)
        .min(maximum_allocation)
        .max(1);
    loop {
        let mut pids = vec![0; allocation_count];
        let buffer_bytes = libc::c_int::try_from(
            pids.len()
                .saturating_mul(std::mem::size_of::<libc::pid_t>()),
        )
        .map_err(|_| io::Error::other("macOS child-process buffer is too large"))?;
        let returned_count = match list_children(parent_pid, pids.as_mut_ptr().cast(), buffer_bytes)
        {
            Ok(returned_count) => returned_count,
            Err(error) if darwin_process_is_absent(&error) => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let returned_count = usize::try_from(returned_count)
            .map_err(|_| io::Error::other("macOS child-process result size is invalid"))?;
        if returned_count < allocation_count {
            pids.truncate(returned_count);
            pids.retain(|pid| *pid > 0);
            return Ok(pids);
        }
        if allocation_count >= maximum_allocation {
            return Err(io::Error::other(
                "macOS tool process tree exceeded the tracked-process ceiling",
            ));
        }
        allocation_count = allocation_count.saturating_mul(2).min(maximum_allocation);
    }
}

#[cfg(target_os = "macos")]
fn darwin_process_is_absent(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::ESRCH | libc::ENOENT | libc::EINVAL)
    )
}

#[cfg(target_os = "macos")]
fn darwin_remove_completed_exact_root(
    live: &mut Vec<DarwinProcessIdentity>,
    root: DarwinProcessIdentity,
    direct_process_completed: bool,
) {
    if direct_process_completed {
        live.retain(|identity| *identity != root);
    }
}

#[cfg(target_os = "macos")]
fn darwin_process_group_pids(process_group: libc::pid_t) -> io::Result<Vec<libc::pid_t>> {
    let estimated = unsafe { proc_listpgrppids(process_group, std::ptr::null_mut(), 0) };
    if estimated < 0 {
        return Err(io::Error::last_os_error());
    }
    let allocation = usize::try_from(estimated)
        .map_err(|_| io::Error::other("macOS process-group list size is invalid"))?
        .saturating_add(16)
        .min(DARWIN_MAX_TRACKED_TOOL_PROCESSES);
    if allocation == 0 {
        return Ok(Vec::new());
    }
    let mut pids = vec![0; allocation];
    let buffer_size = libc::c_int::try_from(
        pids.len()
            .saturating_mul(std::mem::size_of::<libc::pid_t>()),
    )
    .map_err(|_| io::Error::other("macOS process-group list buffer is too large"))?;
    let returned =
        unsafe { proc_listpgrppids(process_group, pids.as_mut_ptr().cast(), buffer_size) };
    if returned < 0 {
        return Err(io::Error::last_os_error());
    }
    let returned = usize::try_from(returned)
        .map_err(|_| io::Error::other("macOS process-group list result is invalid"))?;
    if returned >= pids.len() {
        return Err(io::Error::other(
            "macOS process-group list exceeded the tracked-process ceiling",
        ));
    }
    pids.truncate(returned);
    pids.retain(|pid| *pid > 0);
    Ok(pids)
}

#[cfg(target_os = "macos")]
fn darwin_process_group_identities(
    process_group: libc::pid_t,
    root: DarwinProcessIdentity,
) -> io::Result<Vec<DarwinProcessIdentity>> {
    let mut identities = Vec::new();
    for pid in darwin_process_group_pids(process_group)? {
        let Some(details) = darwin_process_details(pid)? else {
            continue;
        };
        if details.process_group != process_group || details.status == DARWIN_ZOMBIE_PROCESS_STATUS
        {
            continue;
        }
        if details.identity.pid == root.pid && details.identity != root {
            // The original group is gone and its leader pid was reused. Never
            // signal identities from the unrelated replacement group.
            return Ok(Vec::new());
        }
        identities.push(details.identity);
    }
    Ok(identities)
}

#[cfg(target_os = "macos")]
fn darwin_completed_root_error_is_benign(
    raw_os_error: Option<i32>,
    identity: DarwinProcessIdentity,
    root: DarwinProcessIdentity,
    direct_process_completed: bool,
) -> bool {
    raw_os_error == Some(libc::EPERM) && direct_process_completed && identity == root
}

#[cfg(target_os = "macos")]
fn darwin_signal_identity(
    identity: DarwinProcessIdentity,
    root: DarwinProcessIdentity,
    child: &mut std::process::Child,
    signal: libc::c_int,
) -> io::Result<()> {
    let Some(details) = darwin_tracked_process_details(identity, root, child)? else {
        return Ok(());
    };
    if details.identity != identity || details.status == DARWIN_ZOMBIE_PROCESS_STATUS {
        return Ok(());
    }
    if unsafe { libc::kill(identity.pid, signal) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH)
        || darwin_completed_root_error_is_benign(
            error.raw_os_error(),
            identity,
            root,
            child.try_wait()?.is_some(),
        )
    {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(target_os = "macos")]
struct ToolProcessTree {
    process_group: libc::pid_t,
    tracker: DarwinDescendantTracker,
}

#[cfg(target_os = "macos")]
fn spawn_tool_process(
    command: &mut Command,
    _expected_executable: &Path,
    trace: &mut ToolRunTrace,
) -> io::Result<(std::process::Child, ToolProcessTree)> {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
    let mut child = command.spawn()?;
    trace.process_spawned = true;
    let process_group = libc::pid_t::try_from(child.id()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "tool process id does not fit the macOS process-id type",
        )
    })?;
    let root = match darwin_process_details(process_group) {
        Ok(Some(details)) => details.identity,
        Ok(None) => {
            if child.try_wait()?.is_some() {
                DarwinProcessIdentity {
                    pid: process_group,
                    started_seconds: 0,
                    started_microseconds: 0,
                }
            } else {
                let _kill_result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
                let _wait_result = child.wait();
                return Err(io::Error::other(
                    "macOS could not identify a live tool process",
                ));
            }
        }
        Err(error) => {
            if error.raw_os_error() == Some(libc::EPERM) && child.try_wait()?.is_some() {
                DarwinProcessIdentity {
                    pid: process_group,
                    started_seconds: 0,
                    started_microseconds: 0,
                }
            } else {
                let _kill_result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
                let _wait_result = child.wait();
                return Err(error);
            }
        }
    };
    Ok((
        child,
        ToolProcessTree {
            process_group,
            tracker: DarwinDescendantTracker::new(root),
        },
    ))
}

#[cfg(target_os = "macos")]
fn refresh_tool_process_tree(
    process_tree: &mut ToolProcessTree,
    child: &mut std::process::Child,
) -> io::Result<()> {
    process_tree.tracker.refresh(child).map(|_| ())
}

#[cfg(target_os = "macos")]
fn terminate_tool_process_tree(
    process_tree: &mut ToolProcessTree,
    child: &mut std::process::Child,
) -> io::Result<()> {
    let deadline = Instant::now() + TOOL_RUN_CLEANUP_TIMEOUT;
    let mut stopped = std::collections::BTreeMap::new();
    loop {
        let discovered = process_tree.tracker.refresh(child)?;
        let mut live = process_tree.tracker.live_identities(child)?;
        live.extend(darwin_process_group_identities(
            process_tree.process_group,
            process_tree.tracker.root,
        )?);
        darwin_remove_completed_exact_root(
            &mut live,
            process_tree.tracker.root,
            child.try_wait()?.is_some(),
        );
        let mut newly_stopped = 0usize;
        for identity in live {
            if stopped.get(&identity.pid) == Some(&identity) {
                continue;
            }
            darwin_signal_identity(identity, process_tree.tracker.root, child, libc::SIGSTOP)?;
            stopped.insert(identity.pid, identity);
            newly_stopped += 1;
        }
        if discovered == 0 && newly_stopped == 0 {
            break;
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "macOS tool descendants did not stabilize before termination",
            ));
        }
    }

    let mut tracked = process_tree.tracker.live_identities(child)?;
    tracked.extend(darwin_process_group_identities(
        process_tree.process_group,
        process_tree.tracker.root,
    )?);
    darwin_remove_completed_exact_root(
        &mut tracked,
        process_tree.tracker.root,
        child.try_wait()?.is_some(),
    );
    for identity in tracked {
        darwin_signal_identity(identity, process_tree.tracker.root, child, libc::SIGKILL)?;
    }

    while Instant::now() < deadline {
        process_tree.tracker.refresh(child)?;
        let mut live = process_tree.tracker.live_identities(child)?;
        live.extend(darwin_process_group_identities(
            process_tree.process_group,
            process_tree.tracker.root,
        )?);
        darwin_remove_completed_exact_root(
            &mut live,
            process_tree.tracker.root,
            child.try_wait()?.is_some(),
        );
        if live.is_empty() {
            return Ok(());
        }
        for identity in live {
            darwin_signal_identity(identity, process_tree.tracker.root, child, libc::SIGKILL)?;
        }
        thread::sleep(TOOL_RUN_POLL_INTERVAL);
    }
    let tracker_empty = process_tree.tracker.live_identities(child)?.is_empty();
    let group_empty =
        darwin_process_group_identities(process_tree.process_group, process_tree.tracker.root)?
            .is_empty();
    if tracker_empty && group_empty {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "macOS tool descendants survived bounded process-tree termination",
        ))
    }
}

#[cfg(windows)]
struct ToolProcessTree {
    job: WindowsOwnedHandle,
}

#[cfg(windows)]
struct WindowsOwnedHandle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl WindowsOwnedHandle {
    fn from_nullable(handle: windows_sys::Win32::Foundation::HANDLE) -> io::Result<Self> {
        if handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle))
        }
    }

    fn from_snapshot(handle: windows_sys::Win32::Foundation::HANDLE) -> io::Result<Self> {
        if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle))
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsOwnedHandle {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn spawn_tool_process(
    command: &mut Command,
    expected_executable: &Path,
    trace: &mut ToolRunTrace,
) -> io::Result<(std::process::Child, ToolProcessTree)> {
    use std::mem::size_of;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

    let expected_executable = fs::canonicalize(expected_executable)?;

    let job = WindowsOwnedHandle::from_nullable(unsafe {
        CreateJobObjectW(std::ptr::null(), std::ptr::null())
    })?;
    let mut process_tree = ToolProcessTree { job };
    let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let information_size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(|_| io::Error::other("Windows Job Object limit structure is too large"))?;
    let configured = unsafe {
        SetInformationJobObject(
            process_tree.job.0,
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&information).cast(),
            information_size,
        )
    };
    if configured == 0 {
        return Err(io::Error::last_os_error());
    }

    // Start suspended, attach to the kill-on-close Job Object, then resume.
    // No tool code can create an untracked descendant between spawn and assign.
    command.creation_flags(CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    trace.process_spawned = true;
    let assigned = unsafe {
        AssignProcessToJobObject(
            process_tree.job.0,
            child.as_raw_handle() as *mut std::ffi::c_void,
        )
    };
    if assigned == 0 {
        let error = io::Error::last_os_error();
        let _kill_result = child.kill();
        let _wait_result = child.wait();
        return Err(error);
    }
    if let Err(error) = verify_suspended_windows_process_image(&child, &expected_executable) {
        best_effort_cleanup_tool_process(&mut process_tree, &mut child);
        return Err(error);
    }
    if let Err(error) = resume_suspended_process(child.id()) {
        best_effort_cleanup_tool_process(&mut process_tree, &mut child);
        return Err(error);
    }
    Ok((child, process_tree))
}

#[cfg(windows)]
fn verify_suspended_windows_process_image(
    child: &std::process::Child,
    expected_executable: &Path,
) -> io::Result<()> {
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Threading::QueryFullProcessImageNameW;

    const WINDOWS_MAX_PATH_CHARS: usize = 32_768;
    let mut path = vec![0u16; WINDOWS_MAX_PATH_CHARS];
    let mut path_len = u32::try_from(path.len())
        .map_err(|_| io::Error::other("Windows process image buffer is too large"))?;
    let queried = unsafe {
        QueryFullProcessImageNameW(
            child.as_raw_handle() as *mut std::ffi::c_void,
            0,
            path.as_mut_ptr(),
            &mut path_len,
        )
    };
    if queried == 0 {
        return Err(io::Error::last_os_error());
    }
    path.truncate(
        usize::try_from(path_len)
            .map_err(|_| io::Error::other("Windows process image length is invalid"))?,
    );
    let actual = PathBuf::from(std::ffi::OsString::from_wide(&path));
    let actual = fs::canonicalize(actual)?;
    let normalize = |path: &Path| {
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase()
    };
    if normalize(&actual) == normalize(expected_executable) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "spawned Windows tool image did not match the validated executable",
        ))
    }
}

#[cfg(windows)]
fn refresh_tool_process_tree(
    _process_tree: &mut ToolProcessTree,
    _child: &mut std::process::Child,
) -> io::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn resume_suspended_process(process_id: u32) -> io::Result<()> {
    use std::mem::size_of;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    let snapshot = WindowsOwnedHandle::from_snapshot(unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
    })?;
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>())
            .map_err(|_| io::Error::other("Windows thread entry structure is too large"))?,
        ..THREADENTRY32::default()
    };
    if unsafe { Thread32First(snapshot.0, &mut entry) } == 0 {
        return Err(io::Error::last_os_error());
    }

    loop {
        if entry.th32OwnerProcessID == process_id {
            let thread = WindowsOwnedHandle::from_nullable(unsafe {
                OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID)
            })?;
            if unsafe { ResumeThread(thread.0) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            return Ok(());
        }
        if unsafe { Thread32Next(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "suspended Windows tool process thread was not found",
    ))
}

#[cfg(windows)]
fn terminate_tool_process_tree(
    process_tree: &mut ToolProcessTree,
    _child: &mut std::process::Child,
) -> io::Result<()> {
    use windows_sys::Win32::System::JobObjects::TerminateJobObject;

    let terminated = unsafe { TerminateJobObject(process_tree.job.0, 1) };
    if terminated == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
struct ToolProcessTree;

#[cfg(not(any(unix, windows)))]
fn spawn_tool_process(
    command: &mut Command,
    _expected_executable: &Path,
    trace: &mut ToolRunTrace,
) -> io::Result<(std::process::Child, ToolProcessTree)> {
    let child = command.spawn()?;
    trace.process_spawned = true;
    Ok((child, ToolProcessTree))
}

#[cfg(not(any(unix, windows)))]
fn refresh_tool_process_tree(
    _process_tree: &mut ToolProcessTree,
    _child: &mut std::process::Child,
) -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn terminate_tool_process_tree(
    _process_tree: &mut ToolProcessTree,
    child: &mut std::process::Child,
) -> io::Result<()> {
    match child.kill() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
        Err(error) => Err(error),
    }
}

fn draft_tool_contract(input: &DraftToolInput) -> ToolContract {
    draft_tool_contract_with_runtime(input, &DraftToolRuntimeInput::default())
}

fn draft_tool_contract_with_runtime(
    input: &DraftToolInput,
    runtime: &DraftToolRuntimeInput,
) -> ToolContract {
    ToolContract {
        tool_id: input.tool_id.clone(),
        name: input.name.clone(),
        status: DRAFT_TOOL_STATUS.to_string(),
        owner_workflow: input.owner_workflow.clone(),
        command: ToolCommand {
            entrypoint: input.entrypoint.clone(),
        },
        args_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        output_schema: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        side_effects: input.side_effects.clone(),
        approval_requirement: input.approval_requirement.clone(),
        examples: vec![ToolExample {
            name: "draft invocation".to_string(),
            args: Vec::new(),
            description: Some("fill tool args before validate or run".to_string()),
        }],
        runtime: ToolRuntimeInfo {
            executable_path: input.entrypoint.clone(),
            runtime_dir: Some(TOOL_RUNTIME_DIR_NAME.to_string()),
            timeout_ms: runtime.timeout_ms,
            stdout_limit_bytes: runtime.stdout_limit_bytes,
            stderr_limit_bytes: runtime.stderr_limit_bytes,
            supports_dry_run: runtime.supports_dry_run,
            output_mode: runtime.output_mode,
        },
        platform_launchers: BTreeMap::new(),
    }
}

pub fn validate_draft_tool_input(
    input: &DraftToolInput,
) -> Result<(), ToolContractValidationError> {
    validate_tool_contract(&draft_tool_contract(input))
}

/// Validates draft metadata and an explicit persisted runtime contract.
pub fn validate_draft_tool_input_with_runtime(
    input: &DraftToolInput,
    runtime: &DraftToolRuntimeInput,
) -> Result<(), ToolContractValidationError> {
    validate_tool_contract(&draft_tool_contract_with_runtime(input, runtime))
}

fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractValidationError> {
    if contract.tool_id.trim().is_empty() {
        return Err(ToolContractValidationError::MissingToolId);
    }
    validate_tool_text_size("tool_id", &contract.tool_id, MAX_TOOL_ID_BYTES)?;
    if !is_single_tool_directory_name(&contract.tool_id) {
        return Err(ToolContractValidationError::InvalidToolIdPath(
            contract.tool_id.clone(),
        ));
    }
    if contract.name.trim().is_empty() {
        return Err(ToolContractValidationError::MissingName);
    }
    validate_tool_text_size("name", &contract.name, MAX_TOOL_NAME_BYTES)?;
    if contract.status.trim().is_empty() {
        return Err(ToolContractValidationError::MissingStatus);
    }
    validate_tool_text_size("status", &contract.status, MAX_TOOL_TEXT_BYTES)?;
    if contract
        .owner_workflow
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(ToolContractValidationError::BlankOwnerWorkflow);
    }
    if let Some(owner_workflow) = contract.owner_workflow.as_deref() {
        validate_tool_text_size("owner_workflow", owner_workflow, MAX_TOOL_TEXT_BYTES)?;
    }
    if contract.command.entrypoint.trim().is_empty() {
        return Err(ToolContractValidationError::MissingCommandEntrypoint);
    }
    validate_tool_text_size(
        "command.entrypoint",
        &contract.command.entrypoint,
        MAX_TOOL_TEXT_BYTES,
    )?;
    if !is_relative_path_inside_tool_dir(&contract.command.entrypoint) {
        return Err(
            ToolContractValidationError::CommandEntrypointOutsideToolDir(
                contract.command.entrypoint.clone(),
            ),
        );
    }
    if !contract.args_schema.is_object() {
        return Err(ToolContractValidationError::ArgsSchemaMustBeObject);
    }
    validate_tool_schema_size("args_schema", &contract.args_schema)?;
    if !contract.output_schema.is_object() {
        return Err(ToolContractValidationError::OutputSchemaMustBeObject);
    }
    validate_tool_schema_size("output_schema", &contract.output_schema)?;
    if !ALLOWED_TOOL_SIDE_EFFECTS.contains(&contract.side_effects.as_str()) {
        return Err(ToolContractValidationError::InvalidSideEffects(
            contract.side_effects.clone(),
        ));
    }
    if contract.approval_requirement.trim().is_empty() {
        return Err(ToolContractValidationError::MissingApprovalRequirement);
    }
    validate_tool_text_size(
        "approval_requirement",
        &contract.approval_requirement,
        MAX_TOOL_TEXT_BYTES,
    )?;
    if contract.examples.is_empty() {
        return Err(ToolContractValidationError::MissingExamples);
    }
    if contract.examples.len() > MAX_TOOL_EXAMPLES {
        return Err(ToolContractValidationError::TooManyExamples {
            max_examples: MAX_TOOL_EXAMPLES,
        });
    }
    if contract
        .examples
        .iter()
        .any(|example| example.name.trim().is_empty())
    {
        return Err(ToolContractValidationError::BlankExampleName);
    }
    for example in &contract.examples {
        validate_tool_text_size("examples[].name", &example.name, MAX_TOOL_TEXT_BYTES)?;
        if let Some(description) = example.description.as_deref() {
            validate_tool_text_size("examples[].description", description, MAX_TOOL_TEXT_BYTES)?;
        }
        for arg in &example.args {
            validate_tool_text_size("examples[].args[]", arg, MAX_TOOL_TEXT_BYTES)?;
        }
    }
    if contract.runtime.executable_path.trim().is_empty() {
        return Err(ToolContractValidationError::MissingRuntimeExecutablePath);
    }
    validate_tool_text_size(
        "runtime.executable_path",
        &contract.runtime.executable_path,
        MAX_TOOL_TEXT_BYTES,
    )?;
    if let Some(runtime_dir) = contract.runtime.runtime_dir.as_deref() {
        validate_tool_text_size("runtime.runtime_dir", runtime_dir, MAX_TOOL_TEXT_BYTES)?;
        if !is_relative_path_inside_tool_dir(runtime_dir) {
            return Err(ToolContractValidationError::RuntimeDirectoryOutsideToolDir(
                runtime_dir.to_string(),
            ));
        }
    }
    validate_runtime_limit(
        "runtime.timeout_ms",
        contract.runtime.timeout_ms,
        MAX_TOOL_CONTRACT_TIMEOUT_MS,
    )?;
    validate_runtime_limit(
        "runtime.stdout_limit_bytes",
        contract.runtime.stdout_limit_bytes,
        MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES,
    )?;
    validate_runtime_limit(
        "runtime.stderr_limit_bytes",
        contract.runtime.stderr_limit_bytes,
        MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES,
    )?;
    if !is_relative_path_inside_tool_dir(&contract.runtime.executable_path) {
        return Err(
            ToolContractValidationError::RuntimeExecutablePathOutsideToolDir(
                contract.runtime.executable_path.clone(),
            ),
        );
    }
    for (platform, launcher) in &contract.platform_launchers {
        if platform.is_empty()
            || platform.len() > 64
            || !platform
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(ToolContractValidationError::InvalidPlatformLauncherName(
                platform.clone(),
            ));
        }
        validate_tool_text_size("platform_launchers[]", launcher, MAX_TOOL_TEXT_BYTES)?;
        if !is_relative_path_inside_tool_dir(launcher) {
            return Err(ToolContractValidationError::PlatformLauncherOutsideToolDir(
                launcher.clone(),
            ));
        }
    }

    Ok(())
}

fn validate_runtime_limit(
    field: &'static str,
    actual: u64,
    maximum: u64,
) -> Result<(), ToolContractValidationError> {
    const MINIMUM: u64 = 1;
    if !(MINIMUM..=maximum).contains(&actual) {
        return Err(ToolContractValidationError::RuntimeLimitOutOfRange {
            field,
            minimum: MINIMUM,
            maximum,
            actual,
        });
    }

    Ok(())
}

fn validate_tool_text_size(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), ToolContractValidationError> {
    if value.len() > max_bytes {
        return Err(ToolContractValidationError::FieldTooLong { field, max_bytes });
    }

    Ok(())
}

fn validate_tool_schema_size(
    field: &'static str,
    value: &Value,
) -> Result<(), ToolContractValidationError> {
    if value.to_string().len() > MAX_TOOL_SCHEMA_BYTES {
        return Err(ToolContractValidationError::SchemaTooLarge {
            field,
            max_bytes: MAX_TOOL_SCHEMA_BYTES,
        });
    }

    Ok(())
}

fn is_single_tool_directory_name(value: &str) -> bool {
    if value.contains('\\') {
        return false;
    }

    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn resolve_executable_path(tool_root: &Path, executable_path: &str) -> PathBuf {
    tool_root.join(executable_path)
}

fn is_relative_path_inside_tool_dir(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::CurDir | Component::Normal(_)))
}

fn ensure_executable_stays_in_tool_dir(
    tool_root: &Path,
    executable_path: &Path,
    configured_path: &str,
) -> Result<(), ToolContractValidationError> {
    let canonical_tool_root = tool_root.canonicalize().map_err(|_| {
        ToolContractValidationError::RuntimeExecutablePathOutsideToolDir(
            configured_path.to_string(),
        )
    })?;
    let canonical_executable = executable_path.canonicalize().map_err(|_| {
        ToolContractValidationError::RuntimeExecutablePathOutsideToolDir(
            configured_path.to_string(),
        )
    })?;

    if canonical_executable.starts_with(canonical_tool_root) {
        Ok(())
    } else {
        Err(
            ToolContractValidationError::RuntimeExecutablePathOutsideToolDir(
                configured_path.to_string(),
            ),
        )
    }
}

fn ensure_tool_root_stays_in_workspace_tools_dir(
    workspace_paths: &storage::WorkspacePaths,
    tool_id: &str,
) -> Result<PathBuf, ToolJsonError> {
    if !is_single_tool_directory_name(tool_id) {
        return Err(ToolContractValidationError::InvalidToolIdPath(tool_id.to_string()).into());
    }

    let canonical_tools_root = ensure_tools_root_stays_in_workspace(workspace_paths)?;
    let tool_root = tool_dir(workspace_paths, tool_id);
    let metadata = fs::symlink_metadata(&tool_root)?;
    if tool_root_is_link_or_reparse_point(&metadata) {
        return Err(ToolContractValidationError::ToolDirectoryOutsideWorkspace(
            tool_id.to_string(),
        )
        .into());
    }

    let canonical_tool_root = tool_root.canonicalize()?;
    if !metadata.is_dir() || canonical_tool_root.parent() != Some(canonical_tools_root.as_path()) {
        return Err(ToolContractValidationError::ToolDirectoryOutsideWorkspace(
            tool_id.to_string(),
        )
        .into());
    }

    Ok(canonical_tool_root)
}

fn ensure_tools_root_stays_in_workspace(
    workspace_paths: &storage::WorkspacePaths,
) -> Result<PathBuf, ToolJsonError> {
    let tools_root = workspace_paths.tools();
    let metadata = fs::symlink_metadata(tools_root)?;
    if tool_root_is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(ToolContractValidationError::ToolDirectoryOutsideWorkspace(
            tools_root.display().to_string(),
        )
        .into());
    }

    let canonical_workspace_root = workspace_paths.root().canonicalize()?;
    let canonical_tools_root = tools_root.canonicalize()?;
    if canonical_tools_root.parent() != Some(canonical_workspace_root.as_path()) {
        return Err(ToolContractValidationError::ToolDirectoryOutsideWorkspace(
            tools_root.display().to_string(),
        )
        .into());
    }

    Ok(canonical_tools_root)
}

fn tool_root_is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
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

fn load_canonical_tool_contract(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
) -> Result<ToolContract, CanonicalToolContractError> {
    let stored = get_tool_contract(connection, tool_id)?
        .ok_or_else(|| CanonicalToolContractError::NotFound(tool_id.to_string()))?;
    let local = read_tool_json(workspace_paths, tool_id)?;
    if stored.contract != local {
        return Err(CanonicalToolContractError::ContractDrift(
            tool_id.to_string(),
        ));
    }

    Ok(stored.contract)
}

fn can_run_tool(contract: &ToolContract, approved: Option<&str>) -> bool {
    if requires_approval(contract) {
        return approval_granted(approved);
    }

    true
}

fn requires_approval(contract: &ToolContract) -> bool {
    contract.approval_requirement != "none"
        || matches!(
            contract.side_effects.as_str(),
            "external_write" | "destructive"
        )
}

fn approval_granted(approved: Option<&str>) -> bool {
    approved.is_some_and(|value| value.contains("+++"))
}

/// Validates one alias input without reading or writing SQLite.
pub fn validate_new_tool_alias(input: &NewToolAlias) -> Result<(), ToolAliasValidationError> {
    validate_tool_alias_id("alias", &input.alias)?;
    validate_tool_alias_id("canonical_tool_id", &input.canonical_tool_id)?;
    validate_tool_alias_text(
        "source",
        &input.source,
        MAX_TOOL_ALIAS_SOURCE_BYTES,
        ToolAliasValidationError::MissingSource,
    )?;
    if input.status != TOOL_ALIAS_STATUS_ACTIVE {
        return Err(ToolAliasValidationError::InvalidStatus(
            input.status.clone(),
        ));
    }
    Ok(())
}

fn validate_tool_alias_id(
    field: &'static str,
    value: &str,
) -> Result<(), ToolAliasValidationError> {
    let missing = if field == "canonical_tool_id" {
        ToolAliasValidationError::MissingCanonicalToolId
    } else {
        ToolAliasValidationError::MissingAlias
    };
    validate_tool_alias_text(field, value, MAX_TOOL_ALIAS_BYTES, missing)?;
    if !is_single_tool_directory_name(value) {
        return Err(if field == "canonical_tool_id" {
            ToolAliasValidationError::InvalidCanonicalToolId(value.to_string())
        } else {
            ToolAliasValidationError::InvalidAlias(value.to_string())
        });
    }
    Ok(())
}

fn validate_tool_alias_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
    missing: ToolAliasValidationError,
) -> Result<(), ToolAliasValidationError> {
    if value.trim().is_empty() {
        return Err(missing);
    }
    if value.len() > max_bytes {
        return Err(ToolAliasValidationError::FieldTooLong { field, max_bytes });
    }
    if value.contains('\0') {
        return Err(ToolAliasValidationError::ContainsNul { field });
    }
    Ok(())
}

fn row_to_tool_alias(row: &Row<'_>) -> rusqlite::Result<ToolAlias> {
    Ok(ToolAlias {
        alias: row.get(0)?,
        canonical_tool_id: row.get(1)?,
        created_at: row.get(2)?,
        source: row.get(3)?,
        status: row.get(4)?,
    })
}

fn row_to_tool_id_resolution(row: &Row<'_>) -> rusqlite::Result<ToolIdResolution> {
    let kind_value: String = row.get(4)?;
    let kind = match kind_value.as_str() {
        "direct_canonical" => ToolIdResolutionKind::DirectCanonical,
        "alias" => ToolIdResolutionKind::Alias,
        "superseded_fallback" => ToolIdResolutionKind::SupersededFallback,
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                4,
                Type::Text,
                Box::new(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid tool resolution kind",
                )),
            ));
        }
    };
    Ok(ToolIdResolution {
        requested_id: row.get(0)?,
        canonical_tool_id: row.get(1)?,
        matched_alias: row.get(2)?,
        canonical_status: row.get(3)?,
        kind,
    })
}

fn row_to_tool_contract_record(row: &Row<'_>) -> rusqlite::Result<ToolContractRecord> {
    let contract_json: String = row.get(6)?;
    let mut contract: ToolContract = serde_json::from_str(&contract_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, Type::Text, Box::new(error))
    })?;

    contract.tool_id = row.get(0)?;
    contract.name = row.get(1)?;
    contract.status = row.get(2)?;
    contract.owner_workflow = row.get(3)?;
    contract.side_effects = row.get(4)?;
    contract.approval_requirement = row.get(5)?;

    Ok(ToolContractRecord {
        contract,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use std::env;
    use std::ffi::OsString;
    use std::fs::OpenOptions;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";
    const HOME_ENV: &str = "HOME";

    fn sample_tool_contract(tool_id: &str) -> ToolContract {
        ToolContract {
            tool_id: tool_id.to_string(),
            name: "Context Export".to_string(),
            status: "draft".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            command: ToolCommand {
                entrypoint: "bin/context-export".to_string(),
            },
            args_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "artifacts": { "type": "array" }
                }
            }),
            side_effects: "local_write_artifact".to_string(),
            approval_requirement: "manual_review".to_string(),
            examples: vec![ToolExample {
                name: "basic export".to_string(),
                args: vec!["--query".to_string(), "incident".to_string()],
                description: Some("exports a context bundle".to_string()),
            }],
            runtime: ToolRuntimeInfo {
                executable_path: "bin/context-export".to_string(),
                runtime_dir: Some("runtime".to_string()),
                timeout_ms: DEFAULT_TOOL_TIMEOUT_MS,
                stdout_limit_bytes: DEFAULT_TOOL_OUTPUT_LIMIT_BYTES,
                stderr_limit_bytes: DEFAULT_TOOL_OUTPUT_LIMIT_BYTES,
                supports_dry_run: true,
                output_mode: ToolOutputMode::Inline,
            },
            platform_launchers: BTreeMap::new(),
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        env::temp_dir().join(format!("aopmem-stage-032-{name}-{nanos}"))
    }

    struct EnvGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &PathBuf) -> Self {
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

    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("tool script should be written");
        mark_executable(path);
    }

    fn mark_executable(path: &Path) {
        let mut permissions = fs::metadata(path)
            .expect("tool script metadata should be readable")
            .permissions();
        #[cfg(unix)]
        permissions.set_mode(0o755);
        #[cfg(windows)]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).expect("tool script should be executable");
    }

    fn create_runnable_test_tool(
        workspace_paths: &storage::WorkspacePaths,
        connection: &Connection,
        tool_id: &str,
        script: &str,
    ) -> PathBuf {
        let input = DraftToolInput {
            tool_id: tool_id.to_string(),
            name: format!("{tool_id} runner"),
            entrypoint: format!("bin/{tool_id}"),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };
        create_draft_tool(workspace_paths, connection, &input)
            .expect("draft should create before run");

        let executable_path = tool_dir(workspace_paths, tool_id).join("bin").join(tool_id);
        write_executable(&executable_path, script);
        executable_path
    }

    fn set_test_tool_runtime(
        workspace_paths: &storage::WorkspacePaths,
        connection: &Connection,
        tool_id: &str,
        runtime: ToolRuntimeInfo,
    ) {
        let mut contract = read_tool_json(workspace_paths, tool_id)
            .expect("test tool contract should be readable");
        contract.runtime = runtime;
        persist_test_tool_contract(workspace_paths, connection, contract);
    }

    fn persist_test_tool_contract(
        workspace_paths: &storage::WorkspacePaths,
        connection: &Connection,
        contract: ToolContract,
    ) {
        let tool_id = contract.tool_id.clone();
        write_tool_json(workspace_paths, &contract)
            .expect("updated test tool contract should be written");
        let contract_json =
            serde_json::to_string_pretty(&contract).expect("test tool contract should serialize");
        connection
            .execute(
                "
                UPDATE tool_contracts
                SET name = ?1,
                    status = ?2,
                    owner_workflow = ?3,
                    side_effects = ?4,
                    approval_requirement = ?5,
                    contract_json = ?6
                WHERE tool_id = ?7
                ",
                params![
                    contract.name,
                    contract.status,
                    contract.owner_workflow,
                    contract.side_effects,
                    contract.approval_requirement,
                    contract_json,
                    tool_id
                ],
            )
            .expect("canonical test tool contract should update");
    }

    fn set_test_tool_artifact_runtime(
        workspace_paths: &storage::WorkspacePaths,
        connection: &Connection,
        tool_id: &str,
        stdout_limit_bytes: u64,
        stderr_limit_bytes: u64,
    ) {
        let mut runtime = read_tool_json(workspace_paths, tool_id)
            .expect("test tool runtime should be readable")
            .runtime;
        runtime.timeout_ms = 3_000;
        runtime.stdout_limit_bytes = stdout_limit_bytes;
        runtime.stderr_limit_bytes = stderr_limit_bytes;
        runtime.output_mode = ToolOutputMode::Artifact;
        set_test_tool_runtime(workspace_paths, connection, tool_id, runtime);
    }

    fn artifact_run_entries(artifacts_root: &Path) -> Vec<PathBuf> {
        fn visit(path: &Path, entries: &mut Vec<PathBuf>) {
            let Ok(children) = fs::read_dir(path) else {
                return;
            };
            for child in children {
                let child = child.expect("artifact fixture entry should be readable");
                let child_path = child.path();
                let metadata = fs::symlink_metadata(&child_path)
                    .expect("artifact fixture metadata should be readable");
                let name = child.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("tool-run-")
                    || (name.starts_with(".tool-run-") && name.ends_with(".tmp"))
                {
                    entries.push(child_path.clone());
                }
                if metadata.is_dir() && !metadata.file_type().is_symlink() {
                    visit(&child_path, entries);
                }
            }
        }

        let mut entries = Vec::new();
        visit(artifacts_root, &mut entries);
        entries.sort();
        entries
    }

    struct ArtifactFailureGuard;

    impl ArtifactFailureGuard {
        fn set(mode: u8) -> Self {
            ARTIFACT_FAILURE_MODE.store(mode, Ordering::SeqCst);
            Self
        }
    }

    impl Drop for ArtifactFailureGuard {
        fn drop(&mut self) {
            ARTIFACT_FAILURE_MODE.store(ARTIFACT_FAILURE_NONE, Ordering::SeqCst);
        }
    }

    #[cfg(target_os = "macos")]
    struct DarwinSnapshotHookGuard;

    #[cfg(target_os = "macos")]
    impl DarwinSnapshotHookGuard {
        fn set(force_fallback: bool, mutate_source: bool) -> Self {
            DARWIN_FORCE_CLONE_FALLBACK.store(force_fallback, Ordering::SeqCst);
            DARWIN_MUTATE_SOURCE_DURING_FALLBACK.store(mutate_source, Ordering::SeqCst);
            Self
        }
    }

    #[cfg(target_os = "macos")]
    impl Drop for DarwinSnapshotHookGuard {
        fn drop(&mut self) {
            DARWIN_FORCE_CLONE_FALLBACK.store(false, Ordering::SeqCst);
            DARWIN_MUTATE_SOURCE_DURING_FALLBACK.store(false, Ordering::SeqCst);
        }
    }

    fn setup_test_workspace(
        name: &str,
    ) -> (
        PathBuf,
        EnvGuard,
        EnvGuard,
        storage::WorkspacePaths,
        Connection,
    ) {
        let override_home = temp_path(&format!("{name}-aopmem-home"));
        let home = temp_path(&format!("{name}-user-home"));
        let aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let user_home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, name)
            .expect("test workspace directories should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        (
            override_home,
            aopmem_home,
            user_home,
            workspace_paths,
            connection,
        )
    }

    #[cfg(unix)]
    fn process_exists(pid: i32) -> bool {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }

        let result = unsafe { kill(pid, 0) };
        if result == 0 {
            return true;
        }
        io::Error::last_os_error().raw_os_error() != Some(3)
    }

    #[cfg(windows)]
    fn process_exists(pid: i32) -> bool {
        let filter = format!("PID eq {pid}");
        let output = Command::new("tasklist")
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
            .expect("tasklist should inspect the descendant process");
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
    }

    fn assert_process_stops(pid: i32) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while process_exists(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!process_exists(pid), "descendant process {pid} survived");
    }

    fn register_tool_with_status(connection: &Connection, tool_id: &str, status: &str) {
        let mut contract = sample_tool_contract(tool_id);
        contract.status = status.to_string();
        create_tool_contract(connection, &contract).expect("fixture tool should register");
    }

    fn alias_input(alias: &str, canonical_tool_id: &str) -> NewToolAlias {
        NewToolAlias {
            alias: alias.to_string(),
            canonical_tool_id: canonical_tool_id.to_string(),
            source: "stage_011_test".to_string(),
            status: TOOL_ALIAS_STATUS_ACTIVE.to_string(),
        }
    }

    fn stage_026_nearest_rank_p95(values: &[u128]) -> u128 {
        let mut ordered = values.to_vec();
        ordered.sort_unstable();
        ordered[(95 * ordered.len()).div_ceil(100) - 1]
    }

    fn stage_026_write_alias_benchmark_evidence(rows: &[(String, usize, usize, u128)]) {
        let Some(output) = env::var_os("AOPMEM_STAGE26_ALIAS_BENCH_OUTPUT") else {
            return;
        };
        let output = PathBuf::from(output);
        fs::create_dir_all(&output).expect("Stage 026 benchmark output should create");
        let raw_path = output.join("alias_raw_samples.csv");
        let mut raw = String::from("corpus,active_tools,phase,sample,elapsed_ns\n");
        for (corpus, active_tools, sample, elapsed_ns) in rows {
            raw.push_str(&format!(
                "{corpus},{active_tools},sample,{sample},{elapsed_ns}\n"
            ));
        }
        fs::write(&raw_path, raw).expect("Stage 026 alias raw evidence should write");

        let mut results = Vec::new();
        for (corpus, active_tools) in [("small", 16_usize), ("medium", 64), ("large", 256)] {
            let values = rows
                .iter()
                .filter(|(row_corpus, _, _, _)| row_corpus == corpus)
                .map(|(_, _, _, elapsed_ns)| *elapsed_ns)
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 15, "Stage 026 needs exactly 15 samples");
            let median = {
                let mut ordered = values.clone();
                ordered.sort_unstable();
                ordered[ordered.len() / 2]
            };
            results.push(serde_json::json!({
                "corpus": corpus,
                "active_tools": active_tools,
                "samples": 15,
                "warmups": 3,
                "method": "in-process resolve_tool_id duration; median; nearest-rank p95",
                "median_ns": median,
                "p95_nearest_rank_ns": stage_026_nearest_rank_p95(&values),
                "resolved_via_alias": true,
            }));
        }
        let summary_path = output.join("alias_summary.json");
        fs::write(
            &summary_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "harness": "src/tools/mod.rs::tests::stage_026_alias_resolution_benchmark",
                "method_scope": "in-process; do not compare with CLI wall-clock rows",
                "results": results,
            }))
            .expect("Stage 026 alias summary should serialize")
                + "\n",
        )
        .expect("Stage 026 alias summary should write");

        let digest = |path: &Path| {
            let bytes = fs::read(path).expect("Stage 026 evidence should read for hash");
            format!("{:x}", Sha256::digest(bytes))
        };
        fs::write(
            output.join("alias_SHA256SUMS"),
            format!(
                "{}  alias_raw_samples.csv\n{}  alias_summary.json\n",
                digest(&raw_path),
                digest(&summary_path)
            ),
        )
        .expect("Stage 026 alias hashes should write");
    }

    #[test]
    fn stage_026_alias_resolution_benchmark() {
        let mut rows = Vec::new();
        for (corpus, active_tools) in [("small", 16_usize), ("medium", 64), ("large", 256)] {
            let mut connection =
                Connection::open_in_memory().expect("Stage 026 in-memory registry should open");
            schema::apply_migrations(&mut connection).expect("Stage 026 migrations should apply");
            for index in 0..active_tools {
                register_tool_with_status(
                    &connection,
                    &format!("stage26-tool-{index:04}"),
                    "active",
                );
            }
            add_tool_alias(
                &connection,
                &alias_input("stage26-reader", "stage26-tool-0000"),
            )
            .expect("Stage 026 alias add should succeed");
            for _ in 0..3 {
                let resolved = resolve_tool_id(&connection, "stage26-reader")
                    .expect("Stage 026 alias resolve should query")
                    .expect("Stage 026 alias should resolve");
                assert_eq!(resolved.kind, ToolIdResolutionKind::Alias);
                assert_eq!(resolved.matched_alias.as_deref(), Some("stage26-reader"));
            }
            for sample in 1..=15 {
                let started = Instant::now();
                let resolved = resolve_tool_id(&connection, "stage26-reader")
                    .expect("Stage 026 alias resolve should query")
                    .expect("Stage 026 alias should resolve");
                let elapsed_ns = started.elapsed().as_nanos();
                assert_eq!(resolved.kind, ToolIdResolutionKind::Alias);
                assert_eq!(resolved.matched_alias.as_deref(), Some("stage26-reader"));
                rows.push((corpus.to_string(), active_tools, sample, elapsed_ns));
            }
        }
        stage_026_write_alias_benchmark_evidence(&rows);
    }

    fn create_stage_012_tool(
        workspace_paths: &storage::WorkspacePaths,
        connection: &Connection,
        tool_id: &str,
        name: &str,
        implementation: &[u8],
    ) {
        let input = DraftToolInput {
            tool_id: tool_id.to_string(),
            name: name.to_string(),
            entrypoint: "bin/runner".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };
        create_draft_tool(workspace_paths, connection, &input)
            .expect("Stage 012 tool fixture should create");
        fs::write(
            tool_dir(workspace_paths, tool_id)
                .join("bin")
                .join("runner"),
            implementation,
        )
        .expect("Stage 012 implementation should write");
    }

    fn install_stage_015_fixture_tool(
        workspace_paths: &storage::WorkspacePaths,
        connection: &Connection,
        tool_id: &str,
    ) -> ToolContract {
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/stage_015/confluence_tools")
            .join(tool_id);
        let destination = tool_dir(workspace_paths, tool_id);
        fs::create_dir_all(destination.join("bin"))
            .expect("Stage 015 fixture tool directory should create");
        fs::copy(
            fixture_root.join(TOOL_JSON_FILE_NAME),
            destination.join(TOOL_JSON_FILE_NAME),
        )
        .expect("Stage 015 fixture manifest should copy");
        let runner = destination.join("bin/runner");
        fs::copy(fixture_root.join("bin/runner"), &runner)
            .expect("Stage 015 fixture executable should copy");
        #[cfg(unix)]
        {
            let mut permissions = fs::metadata(&runner)
                .expect("Stage 015 fixture runner metadata should read")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&runner, permissions)
                .expect("Stage 015 fixture runner should become executable");
        }
        let contract: ToolContract = serde_json::from_slice(
            &fs::read(destination.join(TOOL_JSON_FILE_NAME))
                .expect("Stage 015 fixture manifest should read"),
        )
        .expect("Stage 015 fixture manifest must be valid JSON");
        validate_tool_contract(&contract).expect("Stage 015 fixture contract must validate");
        create_tool_contract(connection, &contract)
            .expect("Stage 015 fixture contract should register");
        contract
    }

    fn stage_013_draft_input(tool_id: &str, name: &str) -> DraftToolInput {
        DraftToolInput {
            tool_id: tool_id.to_string(),
            name: name.to_string(),
            entrypoint: "bin/runner".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        }
    }

    #[test]
    fn stage_013_creation_guard_blocks_collisions_and_overlap_without_writes() {
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-013-guard");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "canonical-reader",
            "Canonical Reader",
            b"reader implementation",
        );
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("guard fixture should checkpoint");
        drop(connection);
        let connection = storage::open_workspace_db_immutable(&workspace_paths)
            .expect("guard fixture should open immutable");
        let before = stage_012_tree_snapshot(&home);

        let collision = preflight_tool_creation(
            &workspace_paths,
            &connection,
            &stage_013_draft_input("canonical-reader", "Anything"),
            &DraftToolRuntimeInput::default(),
            true,
        )
        .expect("id collision should be a safe decision");
        assert!(matches!(
            collision,
            ToolCreationGuardDecision::Duplicate {
                candidate: ToolCreationGuardCandidate {
                    class: ToolCreationGuardClass::ActiveIdCollision,
                    ..
                },
                writes_performed: false,
            }
        ));

        let overlap_input = stage_013_draft_input("new-reader", "Canonical Reader");
        let overlap = preflight_tool_creation(
            &workspace_paths,
            &connection,
            &overlap_input,
            &DraftToolRuntimeInput::default(),
            false,
        )
        .expect("overlap should be detected");
        assert!(matches!(
            overlap,
            ToolCreationGuardDecision::OverlapReviewRequired {
                writes_performed: false,
                ..
            }
        ));
        let allowed = preflight_tool_creation(
            &workspace_paths,
            &connection,
            &overlap_input,
            &DraftToolRuntimeInput::default(),
            true,
        )
        .expect("bounded distinction may bypass only overlap");
        assert!(matches!(
            allowed,
            ToolCreationGuardDecision::Allowed {
                technical_distinction_provided: true,
                ref reviewed_candidates,
            } if !reviewed_candidates.is_empty()
        ));
        assert_eq!(before, stage_012_tree_snapshot(&home));
        drop(connection);
        fs::remove_dir_all(home).expect("Stage 013 guard home should remove");
    }

    #[test]
    fn stage_013_registry_recheck_blocks_race_before_first_write() {
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-013-race");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "existing",
            "Existing",
            b"existing",
        );
        let input = stage_013_draft_input("new-id", "Existing");
        let mut effects = MutationEffects::default();
        let changes_before = connection.total_changes();
        let error = create_guarded_draft_tool_in_mutation(
            &workspace_paths,
            &connection,
            &input,
            &DraftToolRuntimeInput::default(),
            false,
            &mut effects,
        )
        .expect_err("authoritative registry recheck must block overlap");
        assert!(matches!(
            error,
            GuardedCreateDraftError::Blocked(
                ToolCreationGuardDecision::OverlapReviewRequired { .. }
            )
        ));
        assert!(get_tool_contract(&connection, "new-id")
            .expect("registry should remain readable")
            .is_none());
        assert_eq!(connection.total_changes(), changes_before);
        assert!(!tool_dir(&workspace_paths, "new-id").exists());
        fs::remove_dir_all(home).expect("Stage 013 race home should remove");
    }

    #[test]
    fn stage_013_bm25_ranks_multi_term_description_above_repeated_single_term() {
        let proposed = draft_tool_contract(&stage_013_draft_input(
            "alpha-beta-client",
            "Alpha Beta Client",
        ));
        let mut repeated = sample_tool_contract("repeated");
        repeated.name = "Alpha Alpha Alpha Client".to_string();
        let mut covered = sample_tool_contract("covered");
        covered.name = "Alpha Beta Client".to_string();
        let records = vec![
            ToolContractRecord {
                contract: repeated,
                created_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
            },
            ToolContractRecord {
                contract: covered,
                created_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
            },
        ];
        let scores = creation_bm25_scores(&proposed, &records).expect("bounded BM25 should score");
        assert!(scores["covered"] > scores["repeated"]);
    }

    #[test]
    fn stage_013_bm25_total_term_bound_fails_closed() {
        let proposed = draft_tool_contract(&stage_013_draft_input("query", "Token Query"));
        let records = (0..=MAX_TOOL_CREATION_GUARD_TOTAL_TOKENS
            / MAX_TOOL_CREATION_GUARD_TOKENS_PER_DOCUMENT)
            .map(|index| {
                let mut contract = sample_tool_contract(&format!("bounded-{index}"));
                contract.name = "token ".repeat(600);
                ToolContractRecord {
                    contract,
                    created_at: "2026-01-01".to_string(),
                    updated_at: "2026-01-01".to_string(),
                }
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            creation_bm25_scores(&proposed, &records),
            Err(ToolCreationGuardError::TooManySearchTokens { .. })
        ));
    }

    #[test]
    fn stage_013_technical_distinction_is_bounded_without_echo_contract() {
        assert!(validate_technical_distinction("different protocol").is_ok());
        assert!(matches!(
            validate_technical_distinction(" "),
            Err(ToolTechnicalDistinctionError::Blank)
        ));
        assert!(matches!(
            validate_technical_distinction(&"x".repeat(MAX_TOOL_TECHNICAL_DISTINCTION_BYTES + 1)),
            Err(ToolTechnicalDistinctionError::TooLong { .. })
        ));
        assert!(matches!(
            validate_technical_distinction("canary\0secret"),
            Err(ToolTechnicalDistinctionError::ContainsNul)
        ));
    }

    #[test]
    fn stage_013_alias_resolution_uses_only_canonical_paths_and_keeps_approval() {
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-013-alias-run");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "canonical-writer",
            "#!/bin/sh\nprintf canonical\n",
        );
        let mut contract = read_tool_json(&workspace_paths, "canonical-writer")
            .expect("canonical contract should read");
        contract.status = "active".to_string();
        contract.side_effects = "external_write".to_string();
        contract.approval_requirement = "explicit".to_string();
        persist_test_tool_contract(&workspace_paths, &connection, contract);
        add_tool_alias(
            &connection,
            &alias_input("writer-short", "canonical-writer"),
        )
        .expect("direct alias should create");

        let resolution = resolve_tool_id(&connection, "writer-short")
            .expect("alias should resolve")
            .expect("alias should exist");
        assert_eq!(resolution.canonical_tool_id, "canonical-writer");
        assert_eq!(resolution.matched_alias.as_deref(), Some("writer-short"));

        let validation =
            validate_tool(&workspace_paths, &connection, &resolution.canonical_tool_id)
                .expect("canonical validation should pass");
        assert!(validation.tool_json_path.contains("canonical-writer"));
        assert!(!validation.tool_json_path.contains("writer-short"));
        let dry_run = dry_run_tool(
            &workspace_paths,
            &connection,
            &resolution.canonical_tool_id,
            &[],
        )
        .expect("canonical dry-run should pass");
        assert!(dry_run.tool_json_path.contains("canonical-writer"));
        assert!(!dry_run.tool_json_path.contains("writer-short"));
        assert!(dry_run.approval_required);
        assert!(matches!(
            run_tool(
                &workspace_paths,
                &connection,
                &resolution.canonical_tool_id,
                &[],
                None,
            ),
            Err(RunToolError::UnsafeActionBlocked { .. })
        ));
        let run = run_tool(
            &workspace_paths,
            &connection,
            &resolution.canonical_tool_id,
            &[],
            Some("+++"),
        )
        .expect("same approval token should run canonical tool");
        assert_eq!(run.stdout, "canonical");
        assert!(run.tool_json_path.contains("canonical-writer"));
        assert!(!tool_dir(&workspace_paths, "writer-short").exists());
        fs::remove_dir_all(home).expect("Stage 013 alias run home should remove");
    }

    #[test]
    fn stage_013_alias_batch_listing_is_one_deterministic_grouped_query() {
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-013-alias-list");
        for tool_id in ["alpha", "beta"] {
            create_stage_012_tool(
                &workspace_paths,
                &connection,
                tool_id,
                tool_id,
                tool_id.as_bytes(),
            );
            let mut contract =
                read_tool_json(&workspace_paths, tool_id).expect("contract should read");
            contract.status = "active".to_string();
            persist_test_tool_contract(&workspace_paths, &connection, contract);
        }
        add_tool_alias(&connection, &alias_input("a-two", "alpha")).expect("alias should add");
        add_tool_alias(&connection, &alias_input("a-one", "alpha")).expect("alias should add");
        add_tool_alias(&connection, &alias_input("b-one", "beta")).expect("alias should add");
        let grouped = list_tool_aliases_for_canonical_ids(
            &connection,
            &["beta".to_string(), "alpha".to_string()],
        )
        .expect("batch aliases should list");
        assert_eq!(
            grouped["alpha"]
                .iter()
                .map(|alias| alias.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["a-one", "a-two"]
        );
        assert_eq!(grouped["beta"][0].alias, "b-one");
        fs::remove_dir_all(home).expect("Stage 013 alias list home should remove");
    }

    #[cfg(unix)]
    #[test]
    fn stage_013_creation_guard_rejects_symlinked_candidate_and_contract_drift() {
        use std::os::unix::fs::symlink;

        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-013-unsafe");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "unsafe-reader",
            "Unsafe Reader",
            b"reader",
        );
        let input = stage_013_draft_input("new-unsafe-reader", "Unsafe Reader");
        let runner = tool_dir(&workspace_paths, "unsafe-reader")
            .join("bin")
            .join("runner");
        fs::remove_file(&runner).expect("fixture runner should remove");
        symlink("/tmp", &runner).expect("unsafe runner symlink should create");
        assert!(matches!(
            preflight_tool_creation(
                &workspace_paths,
                &connection,
                &input,
                &DraftToolRuntimeInput::default(),
                true,
            ),
            Err(ToolCreationGuardError::Plan(
                ToolDedupePlanError::UnsafeImplementationPath(_)
            ))
        ));

        fs::remove_file(&runner).expect("unsafe symlink should remove");
        fs::write(&runner, b"reader").expect("runner should restore");
        let mut drifted =
            read_tool_json(&workspace_paths, "unsafe-reader").expect("manifest should read");
        drifted.name = "drifted local name".to_string();
        fs::write(
            tool_json_path(&workspace_paths, "unsafe-reader"),
            serde_json::to_vec_pretty(&drifted).expect("drift manifest should serialize"),
        )
        .expect("drift manifest should write");
        assert!(matches!(
            preflight_tool_creation(
                &workspace_paths,
                &connection,
                &input,
                &DraftToolRuntimeInput::default(),
                true,
            ),
            Err(ToolCreationGuardError::Plan(
                ToolDedupePlanError::ContractDrift(_)
            ))
        ));
        fs::remove_dir_all(home).expect("Stage 013 unsafe home should remove");
    }

    fn stage_012_tree_snapshot(root: &Path) -> BTreeMap<String, (bool, u64, u128, String)> {
        fn visit(
            root: &Path,
            path: &Path,
            snapshot: &mut BTreeMap<String, (bool, u64, u128, String)>,
        ) {
            let mut entries = fs::read_dir(path)
                .expect("snapshot directory should read")
                .collect::<Result<Vec<_>, _>>()
                .expect("snapshot entries should read");
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path).expect("snapshot metadata should read");
                let relative = path
                    .strip_prefix(root)
                    .expect("snapshot path should stay under root")
                    .to_string_lossy()
                    .replace('\\', "/");
                let modified = metadata
                    .modified()
                    .expect("snapshot mtime should read")
                    .duration_since(UNIX_EPOCH)
                    .expect("snapshot mtime should be after epoch")
                    .as_nanos();
                let digest = if metadata.is_file() {
                    let bytes = fs::read(&path).expect("snapshot file should read");
                    format!("{:x}", Sha256::digest(bytes))
                } else {
                    String::new()
                };
                snapshot.insert(
                    relative,
                    (metadata.is_dir(), metadata.len(), modified, digest),
                );
                if metadata.is_dir() {
                    visit(root, &path, snapshot);
                }
            }
        }

        let mut snapshot = BTreeMap::new();
        visit(root, root, &mut snapshot);
        snapshot
    }

    #[test]
    fn create_get_and_list_tool_contracts() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for tool test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let contract = sample_tool_contract("context-export");

        let created =
            create_tool_contract(&connection, &contract).expect("tool contract should be created");
        let fetched = get_tool_contract(&connection, &contract.tool_id)
            .expect("tool contract lookup should pass")
            .expect("tool contract should exist");
        let listed = list_tool_contracts(&connection).expect("tool contract list should pass");

        assert_eq!(created.contract, contract);
        assert_eq!(fetched, created);
        assert_eq!(listed, vec![created.clone()]);
        assert!(!created.created_at.is_empty());
        assert!(!created.updated_at.is_empty());
    }

    #[test]
    fn stage_012_dedupe_plan_is_deterministic_exact_only_and_zero_write() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-012-zero-write");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "reader",
            "Reader",
            b"same implementation",
        );
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "reader-internal",
            "Internal Reader",
            b"same implementation",
        );
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "reader-wrapper",
            "Reader wrapper",
            b"different wrapper",
        );
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("fixture WAL should checkpoint");
        drop(connection);

        let read = storage::open_workspace_db_immutable(&workspace_paths)
            .expect("immutable DB should open");
        let before = stage_012_tree_snapshot(&home);
        let first = plan_tool_deduplication(&workspace_paths, &read)
            .expect("first duplicate plan should pass");
        let second = plan_tool_deduplication(&workspace_paths, &read)
            .expect("second duplicate plan should pass");
        let after = stage_012_tree_snapshot(&home);

        assert_eq!(first, second);
        assert_eq!(
            after, before,
            "plan changed bytes, mtimes, or directory tree"
        );
        assert!(!first.writes_performed);
        assert_eq!(first.scanned_tools, 3);
        assert_eq!(first.shortlisted_tools, 3);
        assert_eq!(first.hashed_files, 3, "each implementation hashes once");
        assert!(!workspace_paths.observability_db().exists());
        let json = serde_json::to_value(&first).expect("plan should serialize");
        assert!(json.get("writes_performed").is_some());
        assert!(json.get("comparisons").is_some());
        for comparison in json["comparisons"]
            .as_array()
            .expect("comparisons should be an array")
        {
            let object = comparison
                .as_object()
                .expect("comparison should be an object");
            assert!(!object.contains_key("canonical_fingerprint"));
            assert!(!object.contains_key("capability_fingerprint"));
            assert!(!object.contains_key("implementation_fingerprint"));
        }
        let exact = first
            .comparisons
            .iter()
            .find(|item| {
                item.canonical_tool_id == "reader" && item.candidate_tool_id == "reader-internal"
            })
            .expect("exact eligible pair should be present");
        assert_eq!(
            exact.class,
            ToolDuplicateClass::SameImplementationDifferentName
        );
        assert!(exact.exact_only_eligible);
        let wrapper = first
            .comparisons
            .iter()
            .find(|item| item.candidate_tool_id == "reader-wrapper")
            .expect("wrapper pair should be present");
        assert_eq!(
            wrapper.class,
            ToolDuplicateClass::SameCapabilityDifferentWrapper
        );
        assert!(!wrapper.exact_only_eligible);

        drop(read);
        fs::remove_dir_all(home).expect("Stage 012 fixture home should remove");
    }

    #[test]
    fn stage_014_exact_apply_supersedes_aliases_and_replays_without_deleting_files() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-014-exact-apply");
        create_stage_012_tool(&workspace_paths, &connection, "reader", "Reader", b"same");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "reader-user",
            "Reader for user",
            b"same",
        );
        let mut canonical =
            read_tool_json(&workspace_paths, "reader").expect("canonical manifest should read");
        canonical.status = "active".to_string();
        persist_test_tool_contract(&workspace_paths, &connection, canonical);
        let reader_executable = tool_dir(&workspace_paths, "reader-user").join("bin/runner");
        let reader_directory = tool_dir(&workspace_paths, "reader-user");
        let first = crate::mutation::mutate_workspace(&workspace_paths, |database, effects| {
            apply_exact_only_deduplication_in_mutation(&workspace_paths, database, effects)
        })
        .expect("exact-only apply should pass");
        assert_eq!(first.value.canonicalized.len(), 1);
        assert!(reader_directory.is_dir());
        assert!(reader_executable.is_file());
        let superseded = get_tool_contract(&connection, "reader-user")
            .expect("registry should read")
            .expect("duplicate should remain");
        assert_eq!(superseded.contract.status, "superseded");
        assert!(!superseded.updated_at.is_empty());
        assert_eq!(
            read_tool_json(&workspace_paths, "reader-user").expect("manifest"),
            superseded.contract
        );
        assert_eq!(
            resolve_tool_id(&connection, "reader-user")
                .expect("alias should resolve")
                .expect("resolution should exist")
                .canonical_tool_id,
            "reader"
        );
        let replay = crate::mutation::mutate_workspace(&workspace_paths, |database, effects| {
            apply_exact_only_deduplication_in_mutation(&workspace_paths, database, effects)
        })
        .expect("idempotent replay should pass");
        assert!(!replay.value.writes_performed);
        fs::remove_dir_all(home).expect("Stage 014 fixture home should remove");
    }

    #[cfg(unix)]
    #[test]
    fn stage_015_confluence_fixture_proves_generic_exact_canonicalization() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, paths, connection) =
            setup_test_workspace("stage-015-confluence-fixture");
        let canonical = install_stage_015_fixture_tool(&paths, &connection, "confluence_reader");
        let duplicate =
            install_stage_015_fixture_tool(&paths, &connection, "confluence_reader_internal");
        assert_eq!(canonical.status, "active");
        assert_eq!(duplicate.status, "active");
        connection
            .execute(
                "UPDATE tool_contracts SET created_at = '2020-01-01 00:00:00' \
                 WHERE tool_id = 'confluence_reader_internal'",
                [],
            )
            .expect("fixture duplicate should be older in SQLite");
        let canonical_runner = tool_dir(&paths, "confluence_reader").join("bin/runner");
        let duplicate_runner = tool_dir(&paths, "confluence_reader_internal").join("bin/runner");
        let canonical_bytes = fs::read(&canonical_runner).expect("canonical runner should read");
        let duplicate_bytes = fs::read(&duplicate_runner).expect("duplicate runner should read");
        assert_eq!(canonical_bytes, duplicate_bytes);

        let plan = plan_tool_deduplication(&paths, &connection)
            .expect("fixture dedupe plan should succeed");
        let pair = plan
            .comparisons
            .iter()
            .find(|item| {
                item.canonical_tool_id == "confluence_reader"
                    && item.candidate_tool_id == "confluence_reader_internal"
            })
            .expect("neutral suffix must outrank the older final tie-break");
        assert_eq!(
            pair.class,
            ToolDuplicateClass::SameImplementationDifferentName
        );
        assert!(pair.exact_only_eligible);

        let applied = crate::mutation::mutate_workspace(&paths, |database, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, database, effects)
        })
        .expect("fixture exact-only apply should succeed");
        assert_eq!(applied.value.canonicalized.len(), 1);
        assert_eq!(
            applied.value.canonicalized[0],
            ToolCanonicalization {
                canonical_tool_id: "confluence_reader".to_string(),
                superseded_tool_id: "confluence_reader_internal".to_string(),
            }
        );
        assert_eq!(
            get_tool_contract(&connection, "confluence_reader_internal")
                .expect("duplicate contract should read")
                .expect("duplicate contract should remain")
                .contract
                .status,
            "superseded"
        );
        let superseded_manifest = read_tool_json(&paths, "confluence_reader_internal")
            .expect("superseded fixture manifest should read");
        assert_eq!(superseded_manifest.status, "superseded");
        assert_eq!(
            superseded_manifest,
            get_tool_contract(&connection, "confluence_reader_internal")
                .expect("superseded registry contract should read")
                .expect("superseded registry contract should remain")
                .contract
        );
        assert_eq!(
            resolve_tool_id(&connection, "confluence_reader_internal")
                .expect("old ID should resolve")
                .expect("old ID should have an alias")
                .canonical_tool_id,
            "confluence_reader"
        );
        let listed = list_canonical_tool_contracts_page(&connection, None, 10)
            .expect("canonical list should read");
        assert_eq!(listed.items.len(), 1);
        assert_eq!(listed.items[0].contract.tool_id, "confluence_reader");
        let all_contracts = list_tool_contracts(&connection).expect("all contracts should read");
        assert_eq!(all_contracts.len(), 2);
        assert_eq!(
            all_contracts
                .iter()
                .map(|record| record.contract.status.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["active", "superseded"])
        );
        let alias_resolution = resolve_tool_id(&connection, "confluence_reader_internal")
            .expect("old ID should resolve")
            .expect("old ID should resolve to a canonical contract");
        let validation = validate_tool(&paths, &connection, &alias_resolution.canonical_tool_id)
            .expect("canonical validation should pass");
        assert!(validation.tool_json_path.contains("confluence_reader"));
        assert!(!validation
            .tool_json_path
            .contains("confluence_reader_internal"));
        let run = run_tool(
            &paths,
            &connection,
            &alias_resolution.canonical_tool_id,
            &[],
            None,
        )
        .expect("canonical runner should run");
        assert_eq!(run.stdout, "fixture-confluence-reader\n");
        assert!(tool_dir(&paths, "confluence_reader").is_dir());
        assert!(tool_dir(&paths, "confluence_reader_internal").is_dir());
        assert_eq!(
            fs::read(canonical_runner).expect("canonical runner remains"),
            canonical_bytes
        );
        assert_eq!(
            fs::read(duplicate_runner).expect("duplicate runner remains"),
            duplicate_bytes
        );

        let replay = crate::mutation::mutate_workspace(&paths, |database, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, database, effects)
        })
        .expect("fixture replay should succeed");
        assert!(!replay.value.writes_performed);
        assert!(replay.value.canonicalized.is_empty());

        create_stage_012_tool(&paths, &connection, "reader", "Reader", b"same-control");
        create_stage_012_tool(
            &paths,
            &connection,
            "reader_internal",
            "Reader Internal",
            b"same-control",
        );
        for tool_id in ["reader", "reader_internal"] {
            let mut contract =
                read_tool_json(&paths, tool_id).expect("control manifest should read");
            contract.status = "active".to_string();
            persist_test_tool_contract(&paths, &connection, contract);
        }
        let control = crate::mutation::mutate_workspace(&paths, |database, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, database, effects)
        })
        .expect("generic non-Confluence control should succeed");
        assert!(control.value.canonicalized.iter().any(|item| {
            item.canonical_tool_id == "reader" && item.superseded_tool_id == "reader_internal"
        }));
        fs::remove_dir_all(home).expect("Stage 015 fixture home should remove");
    }

    #[test]
    fn stage_014_late_failure_restores_every_manifest_and_registry() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-014-rollback");
        create_stage_012_tool(&workspace_paths, &connection, "reader", "Reader", b"same");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "reader-user",
            "Reader user",
            b"same",
        );
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "reader-internal",
            "Reader internal",
            b"same",
        );
        let mut canonical =
            read_tool_json(&workspace_paths, "reader").expect("manifest should read");
        canonical.status = "active".to_string();
        persist_test_tool_contract(&workspace_paths, &connection, canonical);
        let ids = ["reader-user", "reader-internal"];
        let before = ids.map(|id| {
            fs::read(tool_json_path(&workspace_paths, id)).expect("manifest should read")
        });
        let executables = ids.map(|id| tool_dir(&workspace_paths, id).join("bin/runner"));
        let result = crate::mutation::mutate_workspace(&workspace_paths, |database, effects| {
            apply_exact_only_deduplication_in_mutation(&workspace_paths, database, effects)?;
            Err::<(), _>(ToolDedupeApplyError::Serialization)
        });
        assert!(matches!(
            result,
            Err(crate::mutation::MutationError::Operation(
                ToolDedupeApplyError::Serialization
            ))
        ));
        for ((id, before), executable) in ids.into_iter().zip(before).zip(executables) {
            assert_eq!(
                fs::read(tool_json_path(&workspace_paths, id)).expect("manifest should restore"),
                before
            );
            assert_eq!(
                get_tool_contract(&connection, id)
                    .expect("registry should read")
                    .expect("record should remain")
                    .contract
                    .status,
                "draft"
            );
            assert!(get_tool_alias(&connection, id)
                .expect("aliases should read")
                .is_none());
            assert!(tool_dir(&workspace_paths, id).is_dir());
            assert!(executable.is_file());
        }
        fs::remove_dir_all(home).expect("fixture should remove");
    }

    #[test]
    fn stage_014_skips_exact_group_without_active_canonical() {
        let _lock = crate::install::test_env_lock().lock().expect("env lock");
        let (home, _, _, paths, connection) = setup_test_workspace("stage-014-no-active");
        create_stage_012_tool(&paths, &connection, "reader", "Reader", b"same");
        create_stage_012_tool(&paths, &connection, "reader-user", "Reader user", b"same");
        let before = fs::read(tool_json_path(&paths, "reader-user")).expect("manifest");
        let report = crate::mutation::mutate_workspace(&paths, |db, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, db, effects)
        })
        .expect("apply");
        assert!(!report.value.writes_performed);
        assert_eq!(
            report.value.skipped_without_active_canonical,
            vec![vec!["reader".to_string(), "reader-user".to_string()]]
        );
        assert_eq!(
            fs::read(tool_json_path(&paths, "reader-user")).expect("manifest"),
            before
        );
        assert!(get_tool_alias(&connection, "reader-user")
            .expect("alias")
            .is_none());
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn stage_014_canonical_key_uses_frozen_five_rules() {
        let record = |id: &str, status: &str, created_at: &str| ToolContractRecord {
            contract: ToolContract {
                tool_id: id.to_string(),
                status: status.to_string(),
                ..sample_tool_contract("x")
            },
            created_at: created_at.to_string(),
            updated_at: created_at.to_string(),
        };
        assert!(
            canonical_record_key(&record("reader", "active", "2026-02-01"))
                < canonical_record_key(&record("reader", "draft", "2026-01-01"))
        );
        assert!(
            canonical_record_key(&record("reader", "active", "2026-02-01"))
                < canonical_record_key(&record("reader-user", "active", "2026-01-01"))
        );
        assert!(
            canonical_record_key(&record("read", "active", "2026-02-01"))
                < canonical_record_key(&record("reader", "active", "2026-01-01"))
        );
        assert!(
            canonical_record_key(&record("alpha", "active", "2026-02-01"))
                < canonical_record_key(&record("bravo", "active", "2026-01-01"))
        );
        assert!(
            canonical_record_key(&record("alpha", "active", "2026-01-01"))
                < canonical_record_key(&record("alpha", "active", "2026-02-01"))
        );
    }

    #[test]
    fn stage_014_reports_nonexact_overlap_without_mutation() {
        let _lock = crate::install::test_env_lock().lock().expect("env lock");
        let (home, _, _, paths, connection) = setup_test_workspace("stage-014-overlap");
        create_stage_012_tool(&paths, &connection, "reader", "Reader", b"one");
        create_stage_012_tool(&paths, &connection, "reader-user", "Reader user", b"two");
        let mut canonical = read_tool_json(&paths, "reader").expect("manifest");
        canonical.status = "active".to_string();
        persist_test_tool_contract(&paths, &connection, canonical);
        let report = crate::mutation::mutate_workspace(&paths, |db, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, db, effects)
        })
        .expect("apply");
        assert!(!report.value.writes_performed);
        assert!(!report.value.possible_overlaps.is_empty());
        assert_eq!(
            get_tool_contract(&connection, "reader-user")
                .expect("db")
                .expect("tool")
                .contract
                .status,
            "draft"
        );
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn stage_014_retargets_existing_alias_target_before_supersede() {
        let _lock = crate::install::test_env_lock().lock().expect("env lock");
        let (home, _, _, paths, connection) = setup_test_workspace("stage-014-retarget");
        create_stage_012_tool(&paths, &connection, "a", "Reader", b"same");
        create_stage_012_tool(&paths, &connection, "b", "Reader", b"same");
        let mut a = read_tool_json(&paths, "a").expect("manifest");
        a.status = "active".to_string();
        persist_test_tool_contract(&paths, &connection, a);
        let mut b = read_tool_json(&paths, "b").expect("manifest");
        b.status = "active".to_string();
        persist_test_tool_contract(&paths, &connection, b);
        add_tool_alias(
            &connection,
            &NewToolAlias {
                alias: "legacy-b".to_string(),
                canonical_tool_id: "b".to_string(),
                source: "test".to_string(),
                status: TOOL_ALIAS_STATUS_ACTIVE.to_string(),
            },
        )
        .expect("alias");
        let first = crate::mutation::mutate_workspace(&paths, |db, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, db, effects)
        })
        .expect("apply");
        assert_eq!(first.value.canonicalized.len(), 1);
        for id in ["b", "legacy-b"] {
            assert_eq!(
                resolve_tool_id(&connection, id)
                    .expect("resolve")
                    .expect("resolved")
                    .canonical_tool_id,
                "a"
            );
        }
        let replay = crate::mutation::mutate_workspace(&paths, |db, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, db, effects)
        })
        .expect("replay");
        assert!(replay.value.canonicalized.is_empty());
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn stage_014_large_exact_bucket_hashes_once_without_pair_expansion() {
        let _lock = crate::install::test_env_lock().lock().expect("env lock");
        let (home, _, _, paths, connection) = setup_test_workspace("stage-014-large-exact");
        for index in 0..143 {
            create_stage_012_tool(
                &paths,
                &connection,
                &format!("reader-{index:03}"),
                "Reader",
                b"same",
            );
        }
        let mut canonical = read_tool_json(&paths, "reader-000").expect("manifest");
        canonical.status = "active".to_string();
        persist_test_tool_contract(&paths, &connection, canonical);
        let report = crate::mutation::mutate_workspace(&paths, |db, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, db, effects)
        })
        .expect("exact bucket must not hit pair cap");
        assert_eq!(report.value.hashed_files, 143);
        assert_eq!(report.value.canonicalized.len(), 142);
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn stage_014_authoritative_apply_ignores_stale_read_only_preflight() {
        let _lock = crate::install::test_env_lock().lock().expect("env lock");
        let (home, _, _, paths, connection) = setup_test_workspace("stage-014-rescan");
        create_stage_012_tool(&paths, &connection, "reader", "Reader", b"same");
        let mut canonical = read_tool_json(&paths, "reader").expect("manifest");
        canonical.status = "active".to_string();
        persist_test_tool_contract(&paths, &connection, canonical);
        let preflight = plan_tool_deduplication(&paths, &connection).expect("read-only preflight");
        assert!(preflight.comparisons.is_empty());
        create_stage_012_tool(&paths, &connection, "reader-user", "Reader user", b"same");
        let report = crate::mutation::mutate_workspace(&paths, |db, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, db, effects)
        })
        .expect("fresh apply");
        assert_eq!(report.value.canonicalized.len(), 1);
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn stage_014_snapshot_failure_keeps_committed_canonical_state() {
        let _lock = crate::install::test_env_lock().lock().expect("env lock");
        let (home, _, _, paths, connection) = setup_test_workspace("stage-014-snapshot-pending");
        create_stage_012_tool(&paths, &connection, "reader", "Reader", b"same");
        create_stage_012_tool(&paths, &connection, "reader-user", "Reader user", b"same");
        let mut canonical = read_tool_json(&paths, "reader").expect("manifest");
        canonical.status = "active".to_string();
        persist_test_tool_contract(&paths, &connection, canonical);
        fs::write(paths.audit_git().join(".git"), b"not a git directory")
            .expect("force post-commit snapshot failure");
        let outcome = crate::mutation::mutate_workspace(&paths, |db, effects| {
            apply_exact_only_deduplication_in_mutation(&paths, db, effects)
        })
        .expect("commit remains successful");
        assert_eq!(
            outcome.warning.as_ref().map(|warning| warning.code),
            Some(crate::mutation::AUDIT_SNAPSHOT_PENDING)
        );
        assert_eq!(
            get_tool_contract(&connection, "reader-user")
                .expect("db")
                .expect("tool")
                .contract
                .status,
            "superseded"
        );
        assert_eq!(
            resolve_tool_id(&connection, "reader-user")
                .expect("resolve")
                .expect("resolved")
                .canonical_tool_id,
            "reader"
        );
        assert!(crate::audit::has_pending_snapshot(paths.audit_git()).expect("pending marker"));
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn stage_012_013_014_public_operations_fail_closed_on_targeted_file_swap() {
        let _lock = crate::install::test_env_lock().lock().expect("env lock");
        for operation in ["plan", "preflight", "apply"] {
            let (home, _aopmem_home, _user_home, paths, connection) =
                setup_test_workspace(&format!("stage-swap-{operation}"));
            create_stage_012_tool(&paths, &connection, "reader", "Reader", b"same");
            create_stage_012_tool(&paths, &connection, "reader-user", "Reader user", b"same");
            let target = tool_dir(&paths, "reader-user").join("bin/runner");
            let replacement = tool_dir(&paths, "reader-user").join("bin/replacement");
            fs::write(
                &replacement,
                fs::read(&target).expect("target bytes should read"),
            )
            .expect("same-byte replacement should write");
            *IMPLEMENTATION_SWAP_AFTER_HASH.lock().expect("hook") =
                Some((target.clone(), replacement, target));
            match operation {
                "plan" => assert!(matches!(
                    plan_tool_deduplication(&paths, &connection),
                    Err(ToolDedupePlanError::ImplementationDrift(_))
                )),
                "preflight" => assert!(matches!(
                    preflight_tool_creation(
                        &paths,
                        &connection,
                        &stage_013_draft_input("new-reader", "New Reader"),
                        &DraftToolRuntimeInput::default(),
                        false
                    ),
                    Err(ToolCreationGuardError::Plan(
                        ToolDedupePlanError::ImplementationDrift(_)
                    ))
                )),
                "apply" => {
                    let ids = ["reader", "reader-user"];
                    let manifests_before =
                        ids.map(|id| fs::read(tool_json_path(&paths, id)).expect("manifest"));
                    let records_before = ids.map(|id| {
                        get_tool_contract(&connection, id)
                            .expect("registry should read")
                            .expect("tool should remain registered")
                    });
                    let runners = ids.map(|id| tool_dir(&paths, id).join("bin/runner"));
                    let runner_bytes_before = runners
                        .each_ref()
                        .map(|path| fs::read(path).expect("runner should read"));
                    let directories_before = ids.map(|id| tool_dir(&paths, id).is_dir());
                    let result = crate::mutation::mutate_workspace(&paths, |db, effects| {
                        apply_exact_only_deduplication_in_mutation(&paths, db, effects)
                    });
                    assert!(matches!(
                        result,
                        Err(crate::mutation::MutationError::Operation(
                            ToolDedupeApplyError::Plan(ToolDedupePlanError::ImplementationDrift(_))
                        ))
                    ));
                    for (index, id) in ids.into_iter().enumerate() {
                        assert_eq!(
                            fs::read(tool_json_path(&paths, id)).expect("manifest"),
                            manifests_before[index]
                        );
                        assert_eq!(
                            get_tool_contract(&connection, id)
                                .expect("registry should read")
                                .expect("tool should remain registered"),
                            records_before[index]
                        );
                        assert!(get_tool_alias(&connection, id).expect("alias").is_none());
                        assert_eq!(
                            fs::read(&runners[index]).expect("runner should remain"),
                            runner_bytes_before[index]
                        );
                        assert_eq!(tool_dir(&paths, id).is_dir(), directories_before[index]);
                    }
                }
                _ => unreachable!(),
            }
            fs::remove_dir_all(home).expect("cleanup");
        }
    }

    #[test]
    fn stage_012_fingerprint_canonicalizes_schema_keys_and_excludes_identity_state_and_cosmetics() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-012-canonical");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "neutral-reader",
            "Neutral Reader",
            b"same bytes",
        );
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "reader-user",
            "Reader for user",
            b"same bytes",
        );
        let mut left =
            read_tool_json(&workspace_paths, "neutral-reader").expect("left manifest should read");
        left.status = "active".to_string();
        left.args_schema =
            serde_json::from_str(r#"{"b":2,"a":{"y":2,"x":1}}"#).expect("left schema should parse");
        left.examples[0].description = Some("cosmetic left".to_string());
        persist_test_tool_contract(&workspace_paths, &connection, left);
        let mut right =
            read_tool_json(&workspace_paths, "reader-user").expect("right manifest should read");
        right.args_schema = serde_json::from_str(r#"{"a":{"x":1,"y":2},"b":2}"#)
            .expect("right schema should parse");
        right.examples[0].description = Some("cosmetic right".to_string());
        persist_test_tool_contract(&workspace_paths, &connection, right);

        let plan = plan_tool_deduplication(&workspace_paths, &connection)
            .expect("canonical duplicate plan should pass");
        let comparison = plan
            .comparisons
            .iter()
            .find(|item| {
                item.canonical_tool_id == "neutral-reader"
                    && item.candidate_tool_id == "reader-user"
            })
            .expect("canonical pair should be present");
        assert!(comparison.exact_only_eligible);
        assert_eq!(
            comparison.class,
            ToolDuplicateClass::SameImplementationDifferentName
        );

        let mut changed =
            read_tool_json(&workspace_paths, "reader-user").expect("changed manifest should read");
        changed
            .platform_launchers
            .insert("windows".to_string(), "bin/windows-runner.exe".to_string());
        fs::write(
            tool_dir(&workspace_paths, "reader-user")
                .join("bin")
                .join("windows-runner.exe"),
            b"windows launcher",
        )
        .expect("platform launcher fixture should write");
        persist_test_tool_contract(&workspace_paths, &connection, changed);
        let changed_plan = plan_tool_deduplication(&workspace_paths, &connection)
            .expect("changed launcher plan should pass");
        let changed_comparison = changed_plan
            .comparisons
            .iter()
            .find(|item| item.candidate_tool_id == "reader-user")
            .expect("changed pair should be present");
        assert!(!changed_comparison.exact_only_eligible);

        drop(connection);
        fs::remove_dir_all(home).expect("Stage 012 fixture home should remove");
    }

    #[test]
    fn stage_012_shortlist_is_bounded_before_any_implementation_hash() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-012-pair-bound");
        for index in 0..143 {
            let contract = ToolContract {
                tool_id: format!("same-capability-{index:03}"),
                name: "Same capability".to_string(),
                ..sample_tool_contract("template")
            };
            create_tool_contract(&connection, &contract).expect("bounded fixture should register");
        }
        assert!(matches!(
            plan_tool_deduplication(&workspace_paths, &connection),
            Err(ToolDedupePlanError::TooManyPairs {
                max_pairs: MAX_TOOL_DEDUPE_PAIRS
            })
        ));
        drop(connection);
        fs::remove_dir_all(home).expect("Stage 012 fixture home should remove");
    }

    #[test]
    fn stage_012_legacy_contract_defaults_platform_launchers() {
        let value = serde_json::to_value(sample_tool_contract("legacy"))
            .expect("sample contract should serialize");
        let mut object = value
            .as_object()
            .expect("contract should be object")
            .clone();
        object.remove("platform_launchers");
        let decoded: ToolContract =
            serde_json::from_value(Value::Object(object)).expect("legacy contract should decode");
        assert!(decoded.platform_launchers.is_empty());

        let mut unsafe_launcher = sample_tool_contract("unsafe-launcher");
        unsafe_launcher
            .platform_launchers
            .insert("windows".to_string(), "../outside.exe".to_string());
        assert!(matches!(
            validate_tool_contract(&unsafe_launcher),
            Err(ToolContractValidationError::PlatformLauncherOutsideToolDir(
                _
            ))
        ));
    }

    #[test]
    fn stage_012_all_five_classes_keep_exact_eligibility_separate() {
        fn candidate(
            tool_id: &str,
            name: &str,
            full: &str,
            implementation: &str,
            capability: &str,
            label: &str,
        ) -> FingerprintedTool {
            let mut contract = sample_tool_contract(tool_id);
            contract.name = name.to_string();
            FingerprintedTool {
                record: ToolContractRecord {
                    contract,
                    created_at: "2026-07-18T00:00:00Z".to_string(),
                    updated_at: "2026-07-18T00:00:00Z".to_string(),
                },
                capability_fingerprint: capability.to_string(),
                canonical_fingerprint: full.to_string(),
                implementation_fingerprint: implementation.to_string(),
                normalized_label: label.to_string(),
            }
        }

        let base = candidate("reader", "Reader", "full", "impl", "cap", "reader");
        let exact = candidate("reader-v2", "Reader", "full", "impl", "cap", "reader");
        let renamed = candidate(
            "reader-internal",
            "Internal Reader",
            "full",
            "impl",
            "cap",
            "reader",
        );
        let same_impl = candidate("reader-copy", "Copy", "other", "impl", "other", "copy");
        let wrapper = candidate(
            "reader-wrapper",
            "Reader wrapper",
            "other",
            "other",
            "cap",
            "reader",
        );
        let overlap = candidate(
            "reader-overlap",
            "Reader overlap",
            "other",
            "other",
            "other",
            "reader",
        );
        let distinct = candidate("writer", "Writer", "other", "other", "other", "writer");

        let cases = [
            (&exact, ToolDuplicateClass::ExactDuplicate, true),
            (
                &renamed,
                ToolDuplicateClass::SameImplementationDifferentName,
                true,
            ),
            (
                &same_impl,
                ToolDuplicateClass::SameImplementationDifferentName,
                false,
            ),
            (
                &wrapper,
                ToolDuplicateClass::SameCapabilityDifferentWrapper,
                false,
            ),
            (&overlap, ToolDuplicateClass::PossibleOverlap, false),
            (&distinct, ToolDuplicateClass::Distinct, false),
        ];
        for (candidate, expected_class, expected_exact) in cases {
            let comparison = compare_fingerprinted_tools(&base, candidate);
            assert_eq!(comparison.class, expected_class);
            assert_eq!(comparison.exact_only_eligible, expected_exact);
        }
    }

    #[cfg(unix)]
    #[test]
    fn stage_012_rejects_symlinked_implementation_before_hashing() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-012-symlink");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "linked-reader",
            "Reader",
            b"reader",
        );
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "other-reader",
            "Reader",
            b"reader",
        );
        let outside = home.join("outside");
        fs::write(&outside, b"outside secret").expect("outside file should write");
        symlink(
            &outside,
            tool_dir(&workspace_paths, "linked-reader")
                .join("runtime")
                .join("escape"),
        )
        .expect("unsafe fixture symlink should create");

        assert!(matches!(
            plan_tool_deduplication(&workspace_paths, &connection),
            Err(ToolDedupePlanError::UnsafeImplementationPath(_))
        ));
        fs::remove_dir_all(home).expect("Stage 012 fixture home should remove");
    }

    #[cfg(unix)]
    #[test]
    fn stage_012_rejects_symlinked_manifest_before_reading_it() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-012-manifest-link");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "linked-reader",
            "Reader",
            b"reader",
        );
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "other-reader",
            "Reader",
            b"reader",
        );
        let manifest = tool_json_path(&workspace_paths, "linked-reader");
        let outside = home.join("outside-tool.json");
        fs::rename(&manifest, &outside).expect("manifest should move outside");
        symlink(&outside, &manifest).expect("unsafe manifest symlink should create");

        assert!(matches!(
            plan_tool_deduplication(&workspace_paths, &connection),
            Err(ToolDedupePlanError::UnsafeImplementationPath(_))
        ));
        fs::remove_dir_all(home).expect("Stage 012 fixture home should remove");
    }

    #[test]
    fn stage_012_anchored_tool_identity_rejects_directory_swap() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-012-directory-swap");
        create_stage_012_tool(&workspace_paths, &connection, "reader", "Reader", b"reader");
        let anchor = anchored_tool_directory(&workspace_paths, "reader")
            .expect("tool directory should anchor");
        let original = tool_dir(&workspace_paths, "reader");
        let moved = workspace_paths.tools().join("reader-moved");
        fs::rename(&original, &moved).expect("original directory should move");
        fs::create_dir(&original).expect("replacement directory should create");

        assert!(
            anchor.verify_logical_identity().is_err(),
            "anchored identity must reject a same-path replacement"
        );
        drop(anchor);
        drop(connection);
        fs::remove_dir_all(home).expect("Stage 012 fixture home should remove");
    }

    #[test]
    fn stage_012_rejects_same_path_regular_file_swap_after_hash() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-012-file-swap");
        create_stage_012_tool(&workspace_paths, &connection, "reader", "Reader", b"same");
        let root = anchored_tool_directory(&workspace_paths, "reader").expect("tool should anchor");
        let destination = tool_dir(&workspace_paths, "reader").join("bin/runner");
        let replacement = tool_dir(&workspace_paths, "reader").join("bin/replacement");
        fs::write(&replacement, b"swap").expect("same-size replacement should write");
        *IMPLEMENTATION_SWAP_AFTER_HASH.lock().expect("hook lock") =
            Some((destination.clone(), replacement, destination));
        assert!(matches!(
            fingerprint_tool_implementation(&root, "reader", vec!["bin/runner".to_string()]),
            Err(ToolDedupePlanError::ImplementationDrift(_))
        ));
        fs::remove_dir_all(home).expect("fixture home should remove");
    }

    #[test]
    fn stage_012_rejects_over_limit_empty_descendant_tree_before_hashing() {
        let _lock = crate::install::test_env_lock().lock().expect("env lock");
        let (home, _aopmem_home, _user_home, workspace_paths, connection) =
            setup_test_workspace("stage-012-entry-limit");
        create_stage_012_tool(&workspace_paths, &connection, "reader", "Reader", b"same");
        create_stage_012_tool(
            &workspace_paths,
            &connection,
            "reader-user",
            "Reader",
            b"same",
        );
        for index in 0..=MAX_TOOL_IMPLEMENTATION_ENTRIES {
            fs::create_dir(tool_dir(&workspace_paths, "reader").join(format!("empty-{index}")))
                .expect("empty descendant should create");
        }
        assert!(matches!(
            plan_tool_deduplication(&workspace_paths, &connection),
            Err(ToolDedupePlanError::TooManyImplementationEntries { .. })
        ));
        fs::remove_dir_all(home).expect("fixture home should remove");
    }

    #[test]
    fn stage_011_alias_crud_keyset_bulk_and_resolver_precedence() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for alias test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        register_tool_with_status(&connection, "canonical", "active");
        register_tool_with_status(&connection, "old-id", "superseded");
        register_tool_with_status(&connection, "fallback-id", "superseded");

        let created = add_tool_aliases(
            &connection,
            &[
                alias_input("zulu", "canonical"),
                alias_input("alpha", "canonical"),
                alias_input("old-id", "canonical"),
            ],
        )
        .expect("bounded alias batch should insert");
        assert_eq!(created.len(), 3);
        assert_eq!(
            get_tool_alias(&connection, "alpha")
                .expect("alias lookup should pass")
                .expect("alias should exist")
                .canonical_tool_id,
            "canonical"
        );
        assert_eq!(
            list_tool_aliases(&connection)
                .expect("alias list should pass")
                .into_iter()
                .map(|record| record.alias)
                .collect::<Vec<_>>(),
            ["alpha", "old-id", "zulu"]
        );

        let first =
            list_tool_aliases_page(&connection, None, 2).expect("first alias page should pass");
        assert_eq!(
            first
                .items
                .iter()
                .map(|record| record.alias.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "old-id"]
        );
        assert!(first.more_results);
        assert_eq!(first.next_after_alias.as_deref(), Some("old-id"));
        let second = list_tool_aliases_page(&connection, first.next_after_alias.as_deref(), 2)
            .expect("second alias page should pass");
        assert_eq!(
            second
                .items
                .iter()
                .map(|record| record.alias.as_str())
                .collect::<Vec<_>>(),
            ["zulu"]
        );
        assert!(!second.more_results);

        let direct = resolve_tool_id(&connection, "canonical")
            .expect("direct resolution should pass")
            .expect("direct tool should resolve");
        assert_eq!(direct.kind, ToolIdResolutionKind::DirectCanonical);
        assert_eq!(direct.canonical_tool_id, "canonical");
        assert_eq!(direct.matched_alias, None);

        let alias = resolve_tool_id(&connection, "alpha")
            .expect("alias resolution should pass")
            .expect("alias should resolve");
        assert_eq!(alias.kind, ToolIdResolutionKind::Alias);
        assert_eq!(alias.canonical_tool_id, "canonical");
        assert_eq!(alias.matched_alias.as_deref(), Some("alpha"));

        let superseded_alias = resolve_tool_id(&connection, "old-id")
            .expect("superseded alias resolution should pass")
            .expect("superseded old id should resolve");
        assert_eq!(superseded_alias.kind, ToolIdResolutionKind::Alias);
        assert_eq!(superseded_alias.canonical_tool_id, "canonical");

        let fallback = resolve_tool_id(&connection, "fallback-id")
            .expect("superseded fallback should pass")
            .expect("superseded direct tool should remain discoverable");
        assert_eq!(fallback.kind, ToolIdResolutionKind::SupersededFallback);
        assert_eq!(fallback.canonical_tool_id, "fallback-id");
        assert!(resolve_tool_id(&connection, "missing")
            .expect("missing resolution should pass")
            .is_none());

        let removed = remove_tool_alias(&connection, "alpha")
            .expect("alias removal should pass")
            .expect("alias should be removed");
        assert_eq!(removed.alias, "alpha");
        assert!(remove_tool_alias(&connection, "alpha")
            .expect("repeated removal should be idempotent")
            .is_none());
    }

    #[test]
    fn stage_011_alias_invariants_reject_shadow_chain_cycle_and_inactive_target() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for alias test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        register_tool_with_status(&connection, "canonical", "active");
        register_tool_with_status(&connection, "draft-target", "draft");
        register_tool_with_status(&connection, "occupied", "active");

        let inactive = add_tool_alias(&connection, &alias_input("inactive-alias", "draft-target"))
            .expect_err("inactive targets must be rejected");
        assert!(matches!(
            inactive,
            ToolAliasStorageError::CanonicalToolNotActive {
                tool_id,
                status
            } if tool_id == "draft-target" && status == "draft"
        ));

        let shadow = add_tool_alias(&connection, &alias_input("occupied", "canonical"))
            .expect_err("non-superseded tool ids must not be shadowed");
        assert!(matches!(
            shadow,
            ToolAliasStorageError::AliasShadowsTool {
                tool_id,
                status
            } if tool_id == "occupied" && status == "active"
        ));

        add_tool_alias(&connection, &alias_input("first-alias", "canonical"))
            .expect("first direct alias should insert");
        let chained = add_tool_alias(&connection, &alias_input("second-alias", "first-alias"))
            .expect_err("alias-to-alias target must be rejected");
        assert!(matches!(
            chained,
            ToolAliasStorageError::AliasTargetIsAlias(target)
                if target == "first-alias"
        ));
        let self_cycle = add_tool_alias(&connection, &alias_input("canonical", "canonical"))
            .expect_err("self cycle must be rejected");
        assert!(matches!(
            self_cycle,
            ToolAliasStorageError::AliasShadowsTool { .. }
        ));
        assert_eq!(
            list_tool_aliases(&connection)
                .expect("aliases should list")
                .len(),
            1
        );
    }

    #[test]
    fn stage_011_alias_bulk_rolls_back_and_validates_bounds_before_write() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for alias test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        register_tool_with_status(&connection, "canonical", "active");

        let error = add_tool_aliases(
            &connection,
            &[
                alias_input("kept-only-on-success", "canonical"),
                alias_input("invalid-target", "missing"),
            ],
        )
        .expect_err("late batch failure must roll back earlier aliases");
        assert!(matches!(
            error,
            ToolAliasStorageError::CanonicalToolNotFound(target) if target == "missing"
        ));
        assert!(list_tool_aliases(&connection)
            .expect("aliases should list")
            .is_empty());

        let duplicate = add_tool_aliases(
            &connection,
            &[
                alias_input("duplicate", "canonical"),
                alias_input("duplicate", "canonical"),
            ],
        )
        .expect_err("duplicate batch input must fail before writing");
        assert!(matches!(
            duplicate,
            ToolAliasStorageError::Validation(
                ToolAliasValidationError::DuplicateBatchAlias(alias)
            ) if alias == "duplicate"
        ));
        assert!(list_tool_aliases(&connection)
            .expect("aliases should remain empty")
            .is_empty());

        let mut oversized = alias_input("oversized", "canonical");
        oversized.source = "s".repeat(MAX_TOOL_ALIAS_SOURCE_BYTES + 1);
        assert!(matches!(
            add_tool_alias(&connection, &oversized),
            Err(ToolAliasStorageError::Validation(
                ToolAliasValidationError::FieldTooLong {
                    field: "source",
                    max_bytes: MAX_TOOL_ALIAS_SOURCE_BYTES
                }
            ))
        ));
        assert!(matches!(
            list_tool_aliases_page(&connection, None, MAX_TOOL_ALIAS_PAGE_SIZE + 1),
            Err(ToolAliasStorageError::Validation(
                ToolAliasValidationError::PageTooLarge {
                    max_items: MAX_TOOL_ALIAS_PAGE_SIZE
                }
            ))
        ));
    }

    #[test]
    fn stage_011_database_constraints_preserve_direct_active_targets() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for alias test");
        connection
            .execute_batch("PRAGMA foreign_keys = ON")
            .expect("foreign keys should enable");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        register_tool_with_status(&connection, "canonical", "active");
        add_tool_alias(&connection, &alias_input("old-id", "canonical"))
            .expect("valid direct alias should insert");

        assert!(connection
            .execute(
                "INSERT INTO tool_aliases (
                    alias, canonical_tool_id, source, status
                 ) VALUES ('missing-target', 'missing', 'test', 'active')",
                [],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO tool_aliases (
                    alias, canonical_tool_id, source, status
                 ) VALUES ('bad-status', 'canonical', 'test', 'disabled')",
                [],
            )
            .is_err());
        assert!(connection
            .execute(
                "UPDATE tool_contracts SET status = 'superseded'
                 WHERE tool_id = 'canonical'",
                [],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO tool_contracts (
                    tool_id, name, status, side_effects,
                    approval_requirement, contract_json
                 ) VALUES ('old-id', 'Shadow', 'active', 'none', 'none', '{}')",
                [],
            )
            .is_err());

        let violations: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("foreign key check should run");
        assert_eq!(violations, 0);
    }

    #[test]
    fn stage_011_alias_mutations_change_operational_revision_without_files() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("stage-011-alias-revision");
        register_tool_with_status(&connection, "canonical", "active");
        let tools_before = fs::read_dir(workspace_paths.tools())
            .expect("tools directory should list")
            .count();
        let before = storage::operational_recall_revision(&connection)
            .expect("baseline revision should build");

        add_tool_alias(&connection, &alias_input("old-id", "canonical"))
            .expect("alias should insert");
        let after_add =
            storage::operational_recall_revision(&connection).expect("alias revision should build");
        remove_tool_alias(&connection, "old-id")
            .expect("alias should remove")
            .expect("alias should exist");
        let after_remove = storage::operational_recall_revision(&connection)
            .expect("removed alias revision should build");

        assert_ne!(before, after_add);
        assert_eq!(before, after_remove);
        assert_eq!(
            fs::read_dir(workspace_paths.tools())
                .expect("tools directory should relist")
                .count(),
            tools_before
        );
        fs::remove_dir_all(override_home).expect("alias fixture should remove");
    }

    #[test]
    fn stage_011_alias_keyset_query_uses_primary_key_index() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for alias test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        let detail: String = connection
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT alias, canonical_tool_id, created_at, source, status
                 FROM tool_aliases
                 WHERE alias > 'cursor'
                 ORDER BY alias ASC
                 LIMIT 101",
                [],
                |row| row.get(3),
            )
            .expect("keyset query plan should read");
        assert!(detail.contains("SEARCH tool_aliases USING INDEX"));
        assert!(detail.contains("alias>?"));
    }

    #[test]
    fn list_tool_contracts_page_uses_stable_keyset_cursor() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for tool test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        for tool_id in ["alpha", "bravo", "charlie", "delta", "echo"] {
            create_tool_contract(&connection, &sample_tool_contract(tool_id))
                .expect("tool contract should be created");
        }

        let first =
            list_tool_contracts_page(&connection, None, 2).expect("first page should query");
        let second = list_tool_contracts_page(&connection, first.next_after_id.as_deref(), 2)
            .expect("second page should query");
        let third = list_tool_contracts_page(&connection, second.next_after_id.as_deref(), 2)
            .expect("third page should query");

        assert_eq!(
            first
                .items
                .iter()
                .map(|record| record.contract.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "bravo"]
        );
        assert_eq!(first.next_after_id.as_deref(), Some("bravo"));
        assert!(first.more_results);
        assert_eq!(
            second
                .items
                .iter()
                .map(|record| record.contract.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["charlie", "delta"]
        );
        assert_eq!(second.next_after_id.as_deref(), Some("delta"));
        assert!(second.more_results);
        assert_eq!(
            third
                .items
                .iter()
                .map(|record| record.contract.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["echo"]
        );
        assert_eq!(third.next_after_id, None);
        assert!(!third.more_results);
    }

    #[test]
    fn list_tool_contracts_page_returns_empty_page_for_zero_limit() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory DB should open for tool test");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        create_tool_contract(&connection, &sample_tool_contract("alpha"))
            .expect("tool contract should be created");

        let page = list_tool_contracts_page(&connection, Some("alpha"), 0)
            .expect("zero limit should be safe");

        assert!(page.items.is_empty());
        assert_eq!(page.next_after_id, None);
        assert!(!page.more_results);
    }

    #[test]
    fn write_and_read_tool_json_round_trip() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("home");
        let home = temp_path("user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-032-workspace")
            .expect("workspace dirs should be created");
        let contract = sample_tool_contract("context-export");

        let manifest_path =
            write_tool_json(&workspace_paths, &contract).expect("tool.json should be written");
        let read_back = read_tool_json(&workspace_paths, &contract.tool_id)
            .expect("tool.json should round-trip");

        assert_eq!(
            manifest_path,
            workspace_paths
                .tools()
                .join("context-export")
                .join(TOOL_JSON_FILE_NAME)
        );
        assert!(manifest_path.is_file());
        assert_eq!(read_back, contract);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn legacy_file_and_sqlite_runtime_fields_default_without_contract_drift() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("legacy-runtime-home");
        let home = temp_path("legacy-runtime-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "legacy-runtime-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        schema::apply_migrations(&mut connection).expect("migrations should apply");

        let contract = sample_tool_contract("legacy-runtime-tool");
        let mut legacy_value = serde_json::to_value(&contract).expect("contract should serialize");
        let legacy_runtime = legacy_value["runtime"]
            .as_object_mut()
            .expect("runtime should be an object");
        legacy_runtime.remove("timeout_ms");
        legacy_runtime.remove("stdout_limit_bytes");
        legacy_runtime.remove("stderr_limit_bytes");
        legacy_runtime.remove("output_mode");
        let legacy_json =
            serde_json::to_vec_pretty(&legacy_value).expect("legacy contract should serialize");

        let tool_root = tool_dir(&workspace_paths, &contract.tool_id);
        fs::create_dir_all(&tool_root).expect("legacy tool root should create");
        fs::write(tool_root.join(TOOL_JSON_FILE_NAME), &legacy_json)
            .expect("legacy tool.json should write");
        connection
            .execute(
                "
                INSERT INTO tool_contracts (
                    tool_id, name, status, owner_workflow, side_effects,
                    approval_requirement, contract_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);
                ",
                params![
                    &contract.tool_id,
                    &contract.name,
                    &contract.status,
                    &contract.owner_workflow,
                    &contract.side_effects,
                    &contract.approval_requirement,
                    String::from_utf8(legacy_json).expect("legacy JSON should be UTF-8")
                ],
            )
            .expect("legacy SQLite contract should insert");

        let canonical =
            load_canonical_tool_contract(&workspace_paths, &connection, &contract.tool_id)
                .expect("legacy file and SQLite JSON should not drift");

        assert_eq!(canonical.runtime.timeout_ms, DEFAULT_TOOL_TIMEOUT_MS);
        assert_eq!(
            canonical.runtime.stdout_limit_bytes,
            DEFAULT_TOOL_OUTPUT_LIMIT_BYTES
        );
        assert_eq!(
            canonical.runtime.stderr_limit_bytes,
            DEFAULT_TOOL_OUTPUT_LIMIT_BYTES
        );
        assert_eq!(canonical.runtime.output_mode, ToolOutputMode::Inline);
        assert!(canonical.runtime.supports_dry_run);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn runtime_contract_serializes_explicit_defaults_and_custom_values_round_trip() {
        let default_contract = sample_tool_contract("explicit-runtime-defaults");
        let default_json =
            serde_json::to_value(&default_contract).expect("default contract should serialize");

        assert_eq!(default_json["runtime"]["timeout_ms"], 30_000);
        assert_eq!(default_json["runtime"]["stdout_limit_bytes"], 65_536);
        assert_eq!(default_json["runtime"]["stderr_limit_bytes"], 65_536);
        assert_eq!(default_json["runtime"]["supports_dry_run"], true);
        assert_eq!(default_json["runtime"]["output_mode"], "inline");

        let mut custom_contract = sample_tool_contract("custom-runtime");
        custom_contract.runtime.timeout_ms = 123_456;
        custom_contract.runtime.stdout_limit_bytes = 234_567;
        custom_contract.runtime.stderr_limit_bytes = 345_678;
        custom_contract.runtime.supports_dry_run = false;
        custom_contract.runtime.output_mode = ToolOutputMode::Artifact;
        let encoded =
            serde_json::to_vec(&custom_contract).expect("custom contract should serialize");
        let decoded: ToolContract =
            serde_json::from_slice(&encoded).expect("custom contract should deserialize");

        assert_eq!(decoded, custom_contract);
        validate_tool_contract(&decoded).expect("custom runtime should validate");
    }

    #[test]
    fn runtime_contract_accepts_exact_ceilings_and_rejects_out_of_range_values() {
        let mut exact = sample_tool_contract("runtime-exact-ceilings");
        exact.runtime.timeout_ms = MAX_TOOL_CONTRACT_TIMEOUT_MS;
        exact.runtime.stdout_limit_bytes = MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES;
        exact.runtime.stderr_limit_bytes = MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES;
        validate_tool_contract(&exact).expect("exact runtime ceilings should validate");

        let cases = [
            ("runtime.timeout_ms", 0, MAX_TOOL_CONTRACT_TIMEOUT_MS),
            (
                "runtime.timeout_ms",
                MAX_TOOL_CONTRACT_TIMEOUT_MS + 1,
                MAX_TOOL_CONTRACT_TIMEOUT_MS,
            ),
            (
                "runtime.stdout_limit_bytes",
                0,
                MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES,
            ),
            (
                "runtime.stdout_limit_bytes",
                MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES + 1,
                MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES,
            ),
            (
                "runtime.stderr_limit_bytes",
                0,
                MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES,
            ),
            (
                "runtime.stderr_limit_bytes",
                MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES + 1,
                MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES,
            ),
        ];

        for (field, actual, maximum) in cases {
            let mut invalid = sample_tool_contract("runtime-invalid-limit");
            match field {
                "runtime.timeout_ms" => invalid.runtime.timeout_ms = actual,
                "runtime.stdout_limit_bytes" => invalid.runtime.stdout_limit_bytes = actual,
                "runtime.stderr_limit_bytes" => invalid.runtime.stderr_limit_bytes = actual,
                other => panic!("unexpected test field: {other}"),
            }
            assert_eq!(
                validate_tool_contract(&invalid),
                Err(ToolContractValidationError::RuntimeLimitOutOfRange {
                    field,
                    minimum: 1,
                    maximum,
                    actual,
                })
            );
        }
    }

    #[test]
    fn runtime_contract_rejects_unknown_output_mode() {
        let contract = sample_tool_contract("unknown-output-mode");
        let mut value = serde_json::to_value(contract).expect("contract should serialize");
        value["runtime"]["output_mode"] = Value::String("stream".to_string());

        let error = serde_json::from_value::<ToolContract>(value)
            .expect_err("unknown output mode should be rejected");

        assert!(error.to_string().contains("unknown variant `stream`"));
    }

    #[test]
    fn stage_17_runner_uses_contract_global_ceilings() {
        assert_eq!(MAX_TOOL_RUN_TIMEOUT, Duration::from_secs(15 * 60));
        assert_eq!(MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES, 10_485_760);
        assert_eq!(MAX_TOOL_CONTRACT_TIMEOUT_MS, 900_000);
        assert_eq!(MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES, 10_485_760);
    }

    #[test]
    fn rejects_invalid_side_effects() {
        let contract = ToolContract {
            side_effects: "network_write".to_string(),
            ..sample_tool_contract("invalid-side-effects")
        };

        let connection =
            Connection::open_in_memory().expect("in-memory DB should open for validation test");
        let error = create_tool_contract(&connection, &contract).unwrap_err();

        match error {
            ToolContractStorageError::Validation(
                ToolContractValidationError::InvalidSideEffects(value),
            ) => assert_eq!(value, "network_write"),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn rejects_oversized_tool_contract_text_and_schema() {
        let oversized_name = ToolContract {
            name: "n".repeat(MAX_TOOL_NAME_BYTES + 1),
            ..sample_tool_contract("bounded-name")
        };
        let oversized_schema = ToolContract {
            args_schema: serde_json::json!({ "payload": "x".repeat(MAX_TOOL_SCHEMA_BYTES) }),
            ..sample_tool_contract("bounded-schema")
        };

        assert_eq!(
            validate_tool_contract(&oversized_name),
            Err(ToolContractValidationError::FieldTooLong {
                field: "name",
                max_bytes: MAX_TOOL_NAME_BYTES,
            })
        );
        assert!(matches!(
            validate_tool_contract(&oversized_schema),
            Err(ToolContractValidationError::SchemaTooLarge {
                field: "args_schema",
                max_bytes: MAX_TOOL_SCHEMA_BYTES,
            })
        ));
    }

    #[test]
    fn rejects_runtime_executable_path_outside_tool_dir() {
        for executable_path in ["/tmp/aopmem-tool", "bin/../outside-tool"] {
            let mut contract = sample_tool_contract("unsafe-runtime-path");
            contract.runtime.executable_path = executable_path.to_string();

            assert_eq!(
                validate_tool_contract(&contract),
                Err(
                    ToolContractValidationError::RuntimeExecutablePathOutsideToolDir(
                        executable_path.to_string(),
                    ),
                ),
            );
        }
    }

    #[test]
    fn rejects_runtime_resource_directory_outside_tool_root() {
        for runtime_dir in ["/tmp/aopmem-runtime", "runtime/../outside-runtime"] {
            let mut contract = sample_tool_contract("unsafe-runtime-directory");
            contract.runtime.runtime_dir = Some(runtime_dir.to_string());

            assert_eq!(
                validate_tool_contract(&contract),
                Err(ToolContractValidationError::RuntimeDirectoryOutsideToolDir(
                    runtime_dir.to_string(),
                )),
            );
        }
    }

    #[test]
    fn rejects_tool_id_outside_workspace_tools_dir() {
        let contract = ToolContract {
            tool_id: "../outside-workspace".to_string(),
            ..sample_tool_contract("safe-tool-id")
        };
        let connection =
            Connection::open_in_memory().expect("in-memory DB should open for validation test");

        let error = create_tool_contract(&connection, &contract).unwrap_err();

        assert!(matches!(
            error,
            ToolContractStorageError::Validation(ToolContractValidationError::InvalidToolIdPath(
                tool_id
            )) if tool_id == "../outside-workspace"
        ));
    }

    #[test]
    fn create_draft_tool_creates_layout_manifest_and_registry_record() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("draft-home");
        let home = temp_path("draft-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-033-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "draft-tool".to_string(),
            name: "Draft Tool".to_string(),
            entrypoint: "bin/draft-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };

        let created =
            create_draft_tool(&workspace_paths, &connection, &input).expect("draft should create");
        let stored = get_tool_contract(&connection, "draft-tool")
            .expect("tool contract get should pass")
            .expect("tool contract should exist");
        let manifest = read_tool_json(&workspace_paths, "draft-tool")
            .expect("tool manifest should round-trip");

        assert_eq!(created.record.contract.status, DRAFT_TOOL_STATUS);
        assert_eq!(stored.contract.status, DRAFT_TOOL_STATUS);
        assert_eq!(manifest.status, DRAFT_TOOL_STATUS);
        assert!(PathBuf::from(&created.tool_dir).is_dir());
        assert!(PathBuf::from(&created.tool_json_path).is_file());
        assert!(PathBuf::from(&created.bin_dir).is_dir());
        assert!(PathBuf::from(&created.runtime_dir).is_dir());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn create_draft_tool_rolls_back_registry_and_staging_when_publish_fails() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("draft-publish-failure-home");
        let home = temp_path("draft-publish-failure-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "draft-publish-failure")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "publish-failure-tool".to_string(),
            name: "Publish Failure Tool".to_string(),
            entrypoint: "bin/publish-failure-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };

        let error =
            create_draft_tool_with_publish(&workspace_paths, &connection, &input, |_, _| {
                Err(io::Error::other("forced publish failure"))
            })
            .expect_err("forced publish failure should be returned");

        assert!(matches!(error, CreateDraftToolError::Io(_)));
        assert!(
            get_tool_contract(&connection, &input.tool_id)
                .expect("tool lookup should pass")
                .is_none(),
            "failed publication must roll back the SQLite registry row"
        );
        assert!(
            !tool_dir(&workspace_paths, &input.tool_id).exists(),
            "failed publication must not leave a final tool directory"
        );
        let staging_entries = fs::read_dir(workspace_paths.tools())
            .expect("tools dir should list")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(DRAFT_TOOL_STAGING_PREFIX)
            })
            .count();
        assert_eq!(staging_entries, 0, "failed publication must clean staging");

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn create_draft_rejects_symlinked_tools_root_before_any_write() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("draft-tools-root-link-home");
        let home = temp_path("draft-tools-root-link-user-home");
        let outside = temp_path("draft-tools-root-link-outside");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "linked-tools-root")
            .expect("workspace dirs should create");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        fs::remove_dir(workspace_paths.tools()).expect("real tools dir should remove");
        fs::create_dir_all(&outside).expect("outside dir should create");
        symlink(&outside, workspace_paths.tools()).expect("tools symlink should create");
        let input = DraftToolInput {
            tool_id: "escaped-draft".to_string(),
            name: "Escaped Draft".to_string(),
            entrypoint: "bin/escaped-draft".to_string(),
            owner_workflow: None,
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };

        let error = create_draft_tool(&workspace_paths, &connection, &input)
            .expect_err("symlinked tools root must be rejected");

        assert!(matches!(
            error,
            CreateDraftToolError::Json(ToolJsonError::Validation(
                ToolContractValidationError::ToolDirectoryOutsideWorkspace(_)
            ))
        ));
        assert_eq!(
            fs::read_dir(&outside)
                .expect("outside dir should list")
                .count(),
            0,
            "validation must happen before staging outside the workspace"
        );
        assert!(get_tool_contract(&connection, &input.tool_id)
            .expect("registry should read")
            .is_none());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(&outside).expect("outside dir should remove");
    }

    #[test]
    fn coordinated_draft_rolls_back_registry_and_published_directory() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("draft-coordinator-rollback-home");
        let home = temp_path("draft-coordinator-rollback-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "draft-coordinator-rollback")
            .expect("workspace dirs should create");
        crate::mutation::mutate_workspace(&workspace_paths, |_connection, _effects| {
            Ok::<_, rusqlite::Error>(())
        })
        .expect("workspace schema should initialize through coordinator");
        let input = DraftToolInput {
            tool_id: "rollback-draft".to_string(),
            name: "Rollback Draft".to_string(),
            entrypoint: "bin/rollback-draft".to_string(),
            owner_workflow: None,
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };

        let error = crate::mutation::mutate_workspace(&workspace_paths, |connection, effects| {
            create_draft_tool_in_mutation(&workspace_paths, connection, &input, effects)?;
            Err::<DraftToolRecord, _>(CreateDraftToolError::Io(io::Error::other(
                "forced post-publish failure",
            )))
        })
        .expect_err("forced post-publish failure should roll back");

        assert!(matches!(
            error,
            crate::mutation::MutationError::Operation(CreateDraftToolError::Io(_))
        ));
        assert!(!tool_dir(&workspace_paths, &input.tool_id).exists());
        let connection = storage::open_workspace_db_read_only(&workspace_paths)
            .expect("rolled-back DB should open");
        assert!(get_tool_contract(&connection, &input.tool_id)
            .expect("registry should read")
            .is_none());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should remove");
    }

    #[test]
    fn validate_tool_accepts_existing_local_executable() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("validate-home");
        let home = temp_path("validate-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-034-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "validated-tool".to_string(),
            name: "Validated Tool".to_string(),
            entrypoint: "bin/validated-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before validate");
        let executable_path =
            tool_dir(&workspace_paths, "validated-tool").join("bin/validated-tool");
        fs::write(&executable_path, "#!/bin/sh\n")
            .expect("placeholder executable should be written");

        let validated = validate_tool(&workspace_paths, &connection, "validated-tool")
            .expect("tool validate should pass");

        assert_eq!(validated.tool_id, "validated-tool");
        assert!(PathBuf::from(&validated.tool_json_path).is_file());
        assert_eq!(
            PathBuf::from(&validated.executable_path),
            executable_path
                .canonicalize()
                .expect("expected executable should canonicalize")
        );
        assert_eq!(validated.runtime.timeout_ms, DEFAULT_TOOL_TIMEOUT_MS);
        assert_eq!(
            validated.runtime.stdout_limit_bytes,
            DEFAULT_TOOL_OUTPUT_LIMIT_BYTES
        );
        assert_eq!(
            validated.runtime.stderr_limit_bytes,
            DEFAULT_TOOL_OUTPUT_LIMIT_BYTES
        );
        assert!(!validated.runtime.supports_dry_run);
        assert_eq!(validated.runtime.output_mode, ToolOutputMode::Inline);
        let validation_json =
            serde_json::to_value(&validated).expect("validation record should serialize");
        assert_eq!(validation_json["runtime"]["timeout_ms"], 30_000);
        assert_eq!(validation_json["runtime"]["output_mode"], "inline");

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn validate_tool_rejects_executable_symlink_outside_tool_root() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("validate-executable-symlink-home");
        let home = temp_path("validate-executable-symlink-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "validate-executable-symlink")
            .expect("workspace dirs should be created");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        let input = DraftToolInput {
            tool_id: "executable-symlink-tool".to_string(),
            name: "Executable Symlink Tool".to_string(),
            entrypoint: "bin/executable-symlink-tool".to_string(),
            owner_workflow: None,
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };
        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before containment check");
        let outside_executable = override_home.join("outside-executable");
        write_executable(&outside_executable, "#!/bin/sh\nexit 0\n");
        let executable_path = tool_dir(&workspace_paths, &input.tool_id).join(&input.entrypoint);
        symlink(&outside_executable, &executable_path)
            .expect("outside executable symlink should create");

        let error = validate_tool(&workspace_paths, &connection, &input.tool_id)
            .expect_err("executable symlink escape must be rejected");

        assert!(matches!(
            error,
            ValidateToolError::Json(ToolJsonError::Validation(
                ToolContractValidationError::RuntimeExecutablePathOutsideToolDir(path)
            )) if path == input.entrypoint
        ));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn validate_tool_rejects_symlinked_tool_root_outside_workspace() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("validate-tool-root-symlink-home");
        let home = temp_path("validate-tool-root-symlink-user-home");
        let outside_root = temp_path("validate-tool-root-symlink-outside");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "validate-tool-root-symlink")
            .expect("workspace dirs should be created");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        let input = DraftToolInput {
            tool_id: "root-symlink-tool".to_string(),
            name: "Root Symlink Tool".to_string(),
            entrypoint: "bin/root-symlink-tool".to_string(),
            owner_workflow: None,
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };
        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before containment check");
        let tool_root = tool_dir(&workspace_paths, &input.tool_id);
        fs::rename(&tool_root, &outside_root).expect("tool root should move outside workspace");
        symlink(&outside_root, &tool_root).expect("outside tool-root symlink should create");

        let error = validate_tool(&workspace_paths, &connection, &input.tool_id)
            .expect_err("symlinked tool root must be rejected");

        assert!(matches!(
            error,
            ValidateToolError::Json(ToolJsonError::Validation(
                ToolContractValidationError::ToolDirectoryOutsideWorkspace(tool_id)
            )) if tool_id == input.tool_id
        ));

        fs::remove_file(&tool_root).expect("tool-root symlink should be removed");
        fs::remove_dir_all(&outside_root).expect("outside tool root should be removed");
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn validate_tool_rejects_missing_local_executable() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("validate-missing-home");
        let home = temp_path("validate-missing-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, "stage-034-missing-executable")
                .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "missing-executable".to_string(),
            name: "Missing Executable".to_string(),
            entrypoint: "bin/missing-executable".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before validate");

        let error = validate_tool(&workspace_paths, &connection, "missing-executable")
            .expect_err("tool validate should fail when executable is absent");

        match error {
            ValidateToolError::MissingExecutablePath(path) => {
                assert!(path.ends_with("/missing-executable/bin/missing-executable"));
            }
            other => panic!("unexpected error: {other}"),
        }

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn validate_tool_rejects_sqlite_and_tool_json_drift() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("validate-drift-home");
        let home = temp_path("validate-drift-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-035-validate-drift")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "validate-drift-tool".to_string(),
            name: "Validate Drift Tool".to_string(),
            entrypoint: "bin/validate-drift-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before drift check");
        let mut drifted = read_tool_json(&workspace_paths, "validate-drift-tool")
            .expect("tool manifest should be readable");
        drifted.runtime.executable_path = "bin/other-path".to_string();
        write_tool_json(&workspace_paths, &drifted).expect("drifted tool.json should be written");

        let error = validate_tool(&workspace_paths, &connection, "validate-drift-tool")
            .expect_err("tool validate should fail on contract drift");

        match error {
            ValidateToolError::ContractDrift(tool_id) => {
                assert_eq!(tool_id, "validate-drift-tool");
            }
            other => panic!("unexpected error: {other}"),
        }

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_executes_safe_draft_without_approval() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("run-home");
        let home = temp_path("run-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-035-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "run-safe-tool".to_string(),
            name: "Run Safe Tool".to_string(),
            entrypoint: "bin/run-safe-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "none".to_string(),
            approval_requirement: "none".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before run");
        let executable_path = tool_dir(&workspace_paths, "run-safe-tool").join("bin/run-safe-tool");
        fs::write(
            &executable_path,
            "#!/bin/sh\nprintf '{\"argv\": [\"%s\", \"%s\"]}\\n' \"$1\" \"$2\"\n",
        )
        .expect("tool script should be written");
        mark_executable(&executable_path);

        let ran = run_tool(
            &workspace_paths,
            &connection,
            "run-safe-tool",
            &["--json".to_string(), "value".to_string()],
            None,
        )
        .expect("safe draft tool should run without approval");

        assert_eq!(ran.tool_id, "run-safe-tool");
        assert_eq!(ran.exit_code, 0);
        assert_eq!(ran.args, vec!["--json".to_string(), "value".to_string()]);
        assert_eq!(ran.stdout, "{\"argv\": [\"--json\", \"value\"]}\n");
        assert!(ran.stderr.is_empty());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_rejects_sqlite_and_tool_json_drift_before_local_policy_override() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("run-drift-home");
        let home = temp_path("run-drift-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-035-run-drift")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "run-drift-tool".to_string(),
            name: "Run Drift Tool".to_string(),
            entrypoint: "bin/run-drift-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "external_write".to_string(),
            approval_requirement: "manual_review".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before drift check");
        let executable_path =
            tool_dir(&workspace_paths, "run-drift-tool").join("bin/run-drift-tool");
        fs::write(&executable_path, "#!/bin/sh\nexit 0\n")
            .expect("drift tool script should be written");
        mark_executable(&executable_path);
        let mut canonical = read_tool_json(&workspace_paths, "run-drift-tool")
            .expect("canonical tool manifest should be readable");
        canonical.runtime.output_mode = ToolOutputMode::Artifact;
        canonical.runtime.stdout_limit_bytes = 8;
        canonical.runtime.stderr_limit_bytes = 8;
        persist_test_tool_contract(&workspace_paths, &connection, canonical);
        let mut drifted = read_tool_json(&workspace_paths, "run-drift-tool")
            .expect("tool manifest should be readable");
        drifted.side_effects = "none".to_string();
        drifted.approval_requirement = "none".to_string();
        write_tool_json(&workspace_paths, &drifted).expect("drifted tool.json should be written");

        let error = run_tool(&workspace_paths, &connection, "run-drift-tool", &[], None)
            .expect_err("tool run should fail on contract drift");

        match error {
            RunToolError::ContractDrift(tool_id) => assert_eq!(tool_id, "run-drift-tool"),
            other => panic!("unexpected error: {other}"),
        }
        assert!(fs::read_dir(workspace_paths.artifacts())
            .expect("artifacts root should remain readable")
            .next()
            .is_none());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_blocks_external_write_without_approval() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("run-blocked-home");
        let home = temp_path("run-blocked-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-035-blocked-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "run-blocked-tool".to_string(),
            name: "Run Blocked Tool".to_string(),
            entrypoint: "bin/run-blocked-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "external_write".to_string(),
            approval_requirement: "none".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before blocked run");
        let executable_path =
            tool_dir(&workspace_paths, "run-blocked-tool").join("bin/run-blocked-tool");
        fs::write(&executable_path, "#!/bin/sh\nexit 0\n")
            .expect("blocked tool script should still be created");
        mark_executable(&executable_path);

        let error = run_tool(&workspace_paths, &connection, "run-blocked-tool", &[], None)
            .expect_err("tool run should block unsafe tool");

        match error {
            RunToolError::UnsafeActionBlocked {
                tool_id,
                side_effects,
                approval_requirement,
            } => {
                assert_eq!(tool_id, "run-blocked-tool");
                assert_eq!(side_effects, "external_write");
                assert_eq!(approval_requirement, "none");
            }
            other => panic!("unexpected error: {other}"),
        }

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn dry_run_external_write_plans_without_executing_tool() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("dry-run-home");
        let home = temp_path("dry-run-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-ga-002-dry-run")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "dry-run-tool".to_string(),
            name: "Dry Run Tool".to_string(),
            entrypoint: "bin/dry-run-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "external_write".to_string(),
            approval_requirement: "manual_review".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before dry-run");
        let executable_path = tool_dir(&workspace_paths, "dry-run-tool").join("bin/dry-run-tool");
        let side_effect_path = workspace_paths.artifacts().join("dry-run-side-effect.txt");
        write_executable(
            &executable_path,
            &format!(
                "#!/bin/sh\nprintf side-effect > {}\n",
                side_effect_path.display()
            ),
        );

        let planned = dry_run_tool(
            &workspace_paths,
            &connection,
            "dry-run-tool",
            &["--flag".to_string()],
        )
        .expect("dry-run should plan without approval");

        assert_eq!(planned.tool_id, "dry-run-tool");
        assert_eq!(planned.side_effects, "external_write");
        assert_eq!(planned.approval_requirement, "manual_review");
        assert!(planned.approval_required);
        assert!(!planned.would_execute);
        assert!(!side_effect_path.exists());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_allows_external_read_without_approval() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("external-read-home");
        let home = temp_path("external-read-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-ga-008-read")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "external-read-tool".to_string(),
            name: "External Read Tool".to_string(),
            entrypoint: "bin/external-read-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "external_read".to_string(),
            approval_requirement: "none".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before external read run");
        let executable_path =
            tool_dir(&workspace_paths, "external-read-tool").join("bin/external-read-tool");
        fs::write(&executable_path, "#!/bin/sh\necho external-read\n")
            .expect("external read script should be created");
        mark_executable(&executable_path);

        let record = run_tool(
            &workspace_paths,
            &connection,
            "external-read-tool",
            &[],
            None,
        )
        .expect("external_read without an approval requirement should run");

        assert_eq!(record.stdout, "external-read\n");

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_blocks_external_read_when_manual_review_is_required() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("external-read-review-home");
        let home = temp_path("external-read-review-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "stage-ga-008-read-review")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "external-read-review-tool".to_string(),
            name: "External Read Review Tool".to_string(),
            entrypoint: "bin/external-read-review-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "external_read".to_string(),
            approval_requirement: "manual_review".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before external read review run");
        let error = run_tool(
            &workspace_paths,
            &connection,
            "external-read-review-tool",
            &[],
            None,
        )
        .expect_err("manual review should block external_read");

        match error {
            RunToolError::UnsafeActionBlocked {
                tool_id,
                side_effects,
                approval_requirement,
            } => {
                assert_eq!(tool_id, "external-read-review-tool");
                assert_eq!(side_effects, "external_read");
                assert_eq!(approval_requirement, "manual_review");
            }
            other => panic!("unexpected error: {other}"),
        }

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_runs_external_write_with_approval() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("run-approved-home");
        let home = temp_path("run-approved-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths =
            storage::ensure_workspace_dirs(&paths, "stage-044-approved-workspace")
                .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let input = DraftToolInput {
            tool_id: "run-approved-tool".to_string(),
            name: "Run Approved Tool".to_string(),
            entrypoint: "bin/run-approved-tool".to_string(),
            owner_workflow: Some("memory_keeper".to_string()),
            side_effects: "external_write".to_string(),
            approval_requirement: "none".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before approved run");
        let executable_path =
            tool_dir(&workspace_paths, "run-approved-tool").join("bin/run-approved-tool");
        fs::write(&executable_path, "#!/bin/sh\necho approved-run\n")
            .expect("approved tool script should be created");
        mark_executable(&executable_path);

        let record = run_tool(
            &workspace_paths,
            &connection,
            "run-approved-tool",
            &[],
            Some("operator said +++ continue"),
        )
        .expect("tool run should accept approval");

        assert_eq!(record.exit_code, 0);
        assert_eq!(record.stdout, "approved-run\n");
        assert!(record.stderr.is_empty());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn destructive_side_effect_requires_approval_even_when_contract_says_none() {
        let mut contract = sample_tool_contract("destructive-policy");
        contract.side_effects = "destructive".to_string();
        contract.approval_requirement = "none".to_string();

        assert!(requires_approval(&contract));
        assert!(!can_run_tool(&contract, None));
        assert!(can_run_tool(&contract, Some("approved +++")));
    }

    #[test]
    fn bounded_output_keeps_only_configured_prefix_while_draining() {
        let output =
            read_bounded_output(&b"0123456789"[..], 4).expect("bounded reader should read stream");

        assert_eq!(output.bytes, b"0123");
        assert!(output.truncated);
    }

    #[test]
    fn legacy_runtime_defaults_convert_to_runner_defaults() {
        let runtime: ToolRuntimeInfo = serde_json::from_value(serde_json::json!({
            "executable_path": "bin/legacy",
            "runtime_dir": null
        }))
        .expect("legacy runtime should deserialize with defaults");

        assert_eq!(
            ToolRunLimits::from_runtime(&runtime).expect("legacy limits should convert"),
            ToolRunLimits::default()
        );
    }

    #[test]
    fn runtime_exact_global_ceilings_convert_without_rounding() {
        let runtime = ToolRuntimeInfo {
            executable_path: "bin/ceiling".to_string(),
            runtime_dir: None,
            timeout_ms: MAX_TOOL_CONTRACT_TIMEOUT_MS,
            stdout_limit_bytes: MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES,
            stderr_limit_bytes: MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES,
            supports_dry_run: false,
            output_mode: ToolOutputMode::Inline,
        };

        let limits = ToolRunLimits::from_runtime(&runtime).expect("exact ceilings should convert");
        assert_eq!(limits.timeout, Duration::from_millis(900_000));
        assert_eq!(limits.stdout_max_bytes, 10_485_760);
        assert_eq!(limits.stderr_max_bytes, 10_485_760);
    }

    #[test]
    fn runtime_conversion_rejects_values_beyond_global_ceilings() {
        for runtime in [
            ToolRuntimeInfo {
                executable_path: "bin/invalid-timeout".to_string(),
                runtime_dir: None,
                timeout_ms: MAX_TOOL_CONTRACT_TIMEOUT_MS + 1,
                stdout_limit_bytes: DEFAULT_TOOL_OUTPUT_LIMIT_BYTES,
                stderr_limit_bytes: DEFAULT_TOOL_OUTPUT_LIMIT_BYTES,
                supports_dry_run: false,
                output_mode: ToolOutputMode::Inline,
            },
            ToolRuntimeInfo {
                executable_path: "bin/invalid-stdout".to_string(),
                runtime_dir: None,
                timeout_ms: DEFAULT_TOOL_TIMEOUT_MS,
                stdout_limit_bytes: MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES + 1,
                stderr_limit_bytes: DEFAULT_TOOL_OUTPUT_LIMIT_BYTES,
                supports_dry_run: false,
                output_mode: ToolOutputMode::Inline,
            },
            ToolRuntimeInfo {
                executable_path: "bin/invalid-stderr".to_string(),
                runtime_dir: None,
                timeout_ms: DEFAULT_TOOL_TIMEOUT_MS,
                stdout_limit_bytes: DEFAULT_TOOL_OUTPUT_LIMIT_BYTES,
                stderr_limit_bytes: MAX_TOOL_CONTRACT_OUTPUT_LIMIT_BYTES + 1,
                supports_dry_run: false,
                output_mode: ToolOutputMode::Inline,
            },
        ] {
            assert!(matches!(
                ToolRunLimits::from_runtime(&runtime),
                Err(ToolRunLimitError::InvalidLimits { .. })
            ));
        }
    }

    #[test]
    fn persisted_timeout_controls_production_run_tool() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("persisted-timeout-home");
        let home = temp_path("persisted-timeout-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "persisted-timeout-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "persisted-timeout-tool",
            "#!/bin/sh\nsleep 5\n",
        );
        let mut runtime = read_tool_json(&workspace_paths, "persisted-timeout-tool")
            .expect("runtime should be readable")
            .runtime;
        runtime.timeout_ms = 40;
        set_test_tool_runtime(
            &workspace_paths,
            &connection,
            "persisted-timeout-tool",
            runtime,
        );

        let error = run_tool(
            &workspace_paths,
            &connection,
            "persisted-timeout-tool",
            &[],
            None,
        )
        .expect_err("persisted timeout should stop production run");

        assert!(matches!(
            error.limit_error(),
            Some(ToolRunLimitError::TimedOut { timeout_ms: 40, .. })
        ));
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn persisted_stdout_limit_controls_production_run_tool_independently() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("persisted-stdout-home");
        let home = temp_path("persisted-stdout-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "persisted-stdout-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "persisted-stdout-tool",
            "#!/bin/sh\nprintf '123456789'\nprintf 'ok' >&2\n",
        );
        let mut runtime = read_tool_json(&workspace_paths, "persisted-stdout-tool")
            .expect("runtime should be readable")
            .runtime;
        runtime.stdout_limit_bytes = 8;
        runtime.stderr_limit_bytes = 1024;
        set_test_tool_runtime(
            &workspace_paths,
            &connection,
            "persisted-stdout-tool",
            runtime,
        );

        let error = run_tool(
            &workspace_paths,
            &connection,
            "persisted-stdout-tool",
            &[],
            None,
        )
        .expect_err("persisted stdout limit should stop production run");

        assert!(
            matches!(
                error.limit_error(),
                Some(ToolRunLimitError::OutputOverflow {
                    stdout_limit_bytes: 8,
                    stderr_limit_bytes: 1024,
                    stdout_truncated: true,
                    stderr_truncated: false,
                    ..
                })
            ),
            "{error:?}"
        );
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn persisted_stderr_limit_controls_production_run_tool_independently() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("persisted-stderr-home");
        let home = temp_path("persisted-stderr-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "persisted-stderr-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "persisted-stderr-tool",
            "#!/bin/sh\nprintf 'ok'\nprintf '123456789' >&2\n",
        );
        let mut runtime = read_tool_json(&workspace_paths, "persisted-stderr-tool")
            .expect("runtime should be readable")
            .runtime;
        runtime.stdout_limit_bytes = 1024;
        runtime.stderr_limit_bytes = 8;
        set_test_tool_runtime(
            &workspace_paths,
            &connection,
            "persisted-stderr-tool",
            runtime,
        );

        let error = run_tool(
            &workspace_paths,
            &connection,
            "persisted-stderr-tool",
            &[],
            None,
        )
        .expect_err("persisted stderr limit should stop production run");

        assert!(
            matches!(
                error.limit_error(),
                Some(ToolRunLimitError::OutputOverflow {
                    stdout_limit_bytes: 1024,
                    stderr_limit_bytes: 8,
                    stdout_truncated: false,
                    stderr_truncated: true,
                    ..
                })
            ),
            "{error:?}"
        );
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_times_out_and_terminates_its_process_group() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("run-timeout-home");
        let home = temp_path("run-timeout-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "run-timeout-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "run-timeout-tool",
            "#!/bin/sh\n(sleep 5; printf 'escaped' > \"$1\") &\nprintf '%s' \"$!\" > \"$2\"\nwait\n",
        );
        let marker = tool_dir(&workspace_paths, "run-timeout-tool").join("runtime/escaped");
        let child_pid_file =
            tool_dir(&workspace_paths, "run-timeout-tool").join("runtime/child.pid");

        let limits = ToolRunLimits {
            timeout: Duration::from_secs(1),
            stdout_max_bytes: 1024,
            stderr_max_bytes: 1024,
        };
        let started = Instant::now();
        let error = run_tool_with_limits(
            &workspace_paths,
            &connection,
            "run-timeout-tool",
            &[
                marker.display().to_string(),
                child_pid_file.display().to_string(),
            ],
            Some("+++ exercise timeout limit"),
            limits,
        )
        .expect_err("sleeping tool should time out");

        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(matches!(&error, RunToolError::Limit(_)), "{error:?}");
        assert_eq!(
            error.limit_error(),
            Some(&ToolRunLimitError::TimedOut {
                timeout_ms: 1_000,
                stdout_limit_bytes: 1024,
                stderr_limit_bytes: 1024,
                stdout_truncated: false,
                stderr_truncated: false,
            })
        );
        let child_pid: i32 = fs::read_to_string(&child_pid_file)
            .expect("child pid should be recorded before timeout")
            .parse()
            .expect("child pid should parse");
        assert_process_stops(child_pid);
        assert!(
            !marker.exists(),
            "terminated descendant must not write marker"
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_drains_and_reports_truncated_stdout_and_stderr() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("run-truncated-home");
        let home = temp_path("run-truncated-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "run-truncated-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "run-truncated-tool",
            "#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 2048 ]; do\n  printf '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'\n  printf 'fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210' >&2\n  i=$((i + 1))\ndone\n",
        );

        let limits = ToolRunLimits {
            timeout: Duration::from_secs(3),
            stdout_max_bytes: 32,
            stderr_max_bytes: 48,
        };
        let started = Instant::now();
        let error = run_tool_with_limits(
            &workspace_paths,
            &connection,
            "run-truncated-tool",
            &[],
            Some("+++ exercise output limit"),
            limits,
        )
        .expect_err("oversized tool output should be rejected");

        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(matches!(&error, RunToolError::Limit(_)), "{error:?}");
        match error.limit_error() {
            Some(ToolRunLimitError::OutputOverflow {
                timeout_ms,
                stdout_limit_bytes,
                stderr_limit_bytes,
                stdout_truncated,
                stderr_truncated,
            }) => {
                assert_eq!(*timeout_ms, 3_000);
                assert_eq!(*stdout_limit_bytes, 32);
                assert_eq!(*stderr_limit_bytes, 48);
                assert!(*stdout_truncated || *stderr_truncated);
            }
            other => panic!("unexpected output-limit error: {other:?}"),
        }

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn invalid_explicit_limits_fail_before_process_spawn() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("invalid-pre-spawn-home");
        let home = temp_path("invalid-pre-spawn-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "invalid-pre-spawn-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "invalid-pre-spawn-tool",
            "#!/bin/sh\nprintf 'spawned' > \"$1\"\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "invalid-pre-spawn-tool",
            8,
            8,
        );
        let marker = tool_dir(&workspace_paths, "invalid-pre-spawn-tool").join("runtime/spawned");

        let error = run_tool_with_limits(
            &workspace_paths,
            &connection,
            "invalid-pre-spawn-tool",
            &[marker.display().to_string()],
            None,
            ToolRunLimits {
                timeout: MAX_TOOL_RUN_TIMEOUT + Duration::from_millis(1),
                stdout_max_bytes: 1024,
                stderr_max_bytes: 1024,
            },
        )
        .expect_err("invalid limit must fail before spawn");

        assert!(matches!(
            error.limit_error(),
            Some(ToolRunLimitError::InvalidLimits { .. })
        ));
        assert!(
            !marker.exists(),
            "invalid limits must not execute tool code"
        );
        assert!(fs::read_dir(workspace_paths.artifacts())
            .expect("artifacts root should remain readable")
            .next()
            .is_none());
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn stdout_and_stderr_overflow_each_terminate_descendant_tree() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("overflow-tree-home");
        let home = temp_path("overflow-tree-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "overflow-tree-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        schema::apply_migrations(&mut connection).expect("migrations should apply");

        for (tool_id, stream_redirect, expect_stdout, expect_stderr) in [
            ("stdout-tree-tool", "", true, false),
            ("stderr-tree-tool", " >&2", false, true),
        ] {
            let script = format!(
                "#!/bin/sh\n(sleep 5; printf 'escaped' > \"$1\") &\nprintf '%s' \"$!\" > \"$2\"\ni=0\nwhile [ \"$i\" -lt 4096 ]; do\n  printf '0123456789abcdef0123456789abcdef'{stream_redirect}\n  i=$((i + 1))\ndone\nwait\n"
            );
            create_runnable_test_tool(&workspace_paths, &connection, tool_id, &script);
            let marker = tool_dir(&workspace_paths, tool_id).join("runtime/escaped");
            let child_pid_file = tool_dir(&workspace_paths, tool_id).join("runtime/child.pid");
            let error = run_tool_with_limits(
                &workspace_paths,
                &connection,
                tool_id,
                &[
                    marker.display().to_string(),
                    child_pid_file.display().to_string(),
                ],
                None,
                ToolRunLimits {
                    timeout: Duration::from_secs(3),
                    stdout_max_bytes: if expect_stdout { 32 } else { 1024 },
                    stderr_max_bytes: if expect_stderr { 32 } else { 1024 },
                },
            )
            .expect_err("selected output stream should overflow");

            match error.limit_error() {
                Some(ToolRunLimitError::OutputOverflow {
                    stdout_truncated,
                    stderr_truncated,
                    ..
                }) => {
                    assert_eq!(*stdout_truncated, expect_stdout);
                    assert_eq!(*stderr_truncated, expect_stderr);
                }
                other => panic!("unexpected output-limit error: {other:?}"),
            }
            let child_pid: i32 = fs::read_to_string(&child_pid_file)
                .expect("descendant pid should be written before output")
                .parse()
                .expect("descendant pid should parse");
            assert_process_stops(child_pid);
            assert!(!marker.exists(), "overflow must terminate descendant tree");
        }

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn direct_parent_exit_with_live_pipe_inheriting_child_does_not_hang() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("parent-exit-home");
        let home = temp_path("parent-exit-user-home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let paths = storage::resolve_paths().expect("AOPMEM_HOME should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, "parent-exit-workspace")
            .expect("workspace dirs should be created");
        let mut connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        schema::apply_migrations(&mut connection).expect("migrations should apply");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "parent-exit-tool",
            "#!/bin/sh\nsleep 5 &\nprintf '%s' \"$!\" > \"$1\"\nprintf 'done'\nexit 0\n",
        );
        let child_pid_file =
            tool_dir(&workspace_paths, "parent-exit-tool").join("runtime/child.pid");

        let started = Instant::now();
        let record = run_tool_with_limits(
            &workspace_paths,
            &connection,
            "parent-exit-tool",
            &[child_pid_file.display().to_string()],
            None,
            ToolRunLimits {
                timeout: Duration::from_secs(3),
                stdout_max_bytes: 1024,
                stderr_max_bytes: 1024,
            },
        )
        .expect("direct parent completion should return safely");

        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(record.stdout, "done");
        let child_pid: i32 = fs::read_to_string(&child_pid_file)
            .expect("live child pid should be recorded")
            .parse()
            .expect("live child pid should parse");
        assert_process_stops(child_pid);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn artifact_mode_publishes_exact_large_stdout_and_stderr_with_bounded_previews() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("artifact-both-streams");
        let stdout_chunk = "0123456789abcdef".repeat(4);
        let stderr_chunk = "fedcba9876543210".repeat(4);
        let script = format!(
            "#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 64 ]; do\n  printf '%s' '{stdout_chunk}'\n  printf '%s' '{stderr_chunk}' >&2\n  i=$((i + 1))\ndone\n"
        );
        create_runnable_test_tool(&workspace_paths, &connection, "artifact-both-tool", &script);
        set_test_tool_artifact_runtime(&workspace_paths, &connection, "artifact-both-tool", 32, 48);

        let record = run_tool(
            &workspace_paths,
            &connection,
            "artifact-both-tool",
            &[],
            None,
        )
        .expect("artifact-mode tool should publish oversized output");
        let artifacts = record
            .artifacts
            .as_ref()
            .expect("oversized artifact output should be published");
        let expected_stdout = stdout_chunk.repeat(64).into_bytes();
        let expected_stderr = stderr_chunk.repeat(64).into_bytes();

        assert_eq!(record.stdout.as_bytes(), &expected_stdout[..32]);
        assert_eq!(record.stderr.as_bytes(), &expected_stderr[..48]);
        assert_eq!(artifacts.stdout.bytes, expected_stdout.len() as u64);
        assert_eq!(artifacts.stderr.bytes, expected_stderr.len() as u64);
        assert!(artifacts.stdout.preview_truncated);
        assert!(artifacts.stderr.preview_truncated);
        assert!(!Path::new(&artifacts.stdout.path).is_absolute());
        assert!(!Path::new(&artifacts.stderr.path).is_absolute());
        assert_eq!(
            fs::read(workspace_paths.root().join(&artifacts.stdout.path))
                .expect("published stdout should be readable"),
            expected_stdout
        );
        assert_eq!(
            fs::read(workspace_paths.root().join(&artifacts.stderr.path))
                .expect("published stderr should be readable"),
            expected_stderr
        );
        let entries = artifact_run_entries(workspace_paths.artifacts());
        assert_eq!(entries.len(), 1, "only one final run directory may remain");
        assert!(!entries[0]
            .file_name()
            .expect("run directory should have a name")
            .to_string_lossy()
            .starts_with('.'));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn artifact_mode_exact_preview_limit_stays_inline_and_limit_plus_one_publishes() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("artifact-boundary");

        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-exact-tool",
            "#!/bin/sh\nprintf '12345678'\n",
        );
        set_test_tool_artifact_runtime(&workspace_paths, &connection, "artifact-exact-tool", 8, 8);
        let exact = run_tool(
            &workspace_paths,
            &connection,
            "artifact-exact-tool",
            &[],
            None,
        )
        .expect("output at the exact preview limit should stay inline");
        assert_eq!(exact.stdout, "12345678");
        assert!(exact.artifacts.is_none());
        assert!(serde_json::to_value(&exact)
            .expect("inline result should serialize")
            .get("artifacts")
            .is_none());
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());

        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-plus-one-tool",
            "#!/bin/sh\nprintf '123456789'\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-plus-one-tool",
            8,
            8,
        );
        let plus_one = run_tool(
            &workspace_paths,
            &connection,
            "artifact-plus-one-tool",
            &[],
            None,
        )
        .expect("output above the preview limit should publish");
        let artifacts = plus_one
            .artifacts
            .expect("limit plus one should produce artifact metadata");
        assert_eq!(plus_one.stdout, "12345678");
        assert_eq!(artifacts.stdout.bytes, 9);
        assert!(artifacts.stdout.preview_truncated);
        assert_eq!(
            fs::read(workspace_paths.root().join(artifacts.stdout.path))
                .expect("published plus-one stdout should be readable"),
            b"123456789"
        );
        assert_eq!(artifact_run_entries(workspace_paths.artifacts()).len(), 1);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn artifact_mode_preserves_invalid_utf8_and_publishes_both_streams() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("artifact-invalid-utf8");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-invalid-utf8-tool",
            "#!/bin/sh\nprintf '\\377\\376ABC'\nprintf 'ok' >&2\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-invalid-utf8-tool",
            2,
            8,
        );

        let record = run_tool(
            &workspace_paths,
            &connection,
            "artifact-invalid-utf8-tool",
            &[],
            None,
        )
        .expect("invalid UTF-8 should remain valid raw artifact data");
        let artifacts = record
            .artifacts
            .expect("one oversized stream should publish both streams");
        assert_eq!(record.stdout, String::from_utf8_lossy(&[0xff, 0xfe]));
        assert_eq!(record.stderr, "ok");
        assert_eq!(artifacts.stdout.bytes, 5);
        assert_eq!(artifacts.stderr.bytes, 2);
        assert!(artifacts.stdout.preview_truncated);
        assert!(!artifacts.stderr.preview_truncated);
        assert_eq!(
            fs::read(workspace_paths.root().join(artifacts.stdout.path))
                .expect("raw invalid UTF-8 stdout should be readable"),
            vec![0xff, 0xfe, b'A', b'B', b'C']
        );
        assert_eq!(
            fs::read(workspace_paths.root().join(artifacts.stderr.path))
                .expect("small stderr should be published with stdout"),
            b"ok"
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn artifact_mode_all_runtime_failures_leave_no_run_tree() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("artifact-failure-cleanup");

        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-timeout-tool",
            "#!/bin/sh\nsleep 5\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-timeout-tool",
            8,
            8,
        );
        let mut timeout_runtime = read_tool_json(&workspace_paths, "artifact-timeout-tool")
            .expect("timeout runtime should be readable")
            .runtime;
        timeout_runtime.timeout_ms = 40;
        set_test_tool_runtime(
            &workspace_paths,
            &connection,
            "artifact-timeout-tool",
            timeout_runtime,
        );
        let timeout = run_tool(
            &workspace_paths,
            &connection,
            "artifact-timeout-tool",
            &[],
            None,
        )
        .expect_err("artifact timeout should fail");
        assert!(matches!(
            timeout.limit_error(),
            Some(ToolRunLimitError::TimedOut { timeout_ms: 40, .. })
        ));
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());

        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-nonzero-tool",
            "#!/bin/sh\nprintf '123456789'\nexit 7\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-nonzero-tool",
            8,
            8,
        );
        let nonzero = run_tool(
            &workspace_paths,
            &connection,
            "artifact-nonzero-tool",
            &[],
            None,
        )
        .expect_err("nonzero artifact tool must not publish");
        assert!(matches!(nonzero, RunToolError::ProcessFailed(7)));
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());

        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-read-failure-tool",
            "#!/bin/sh\nprintf '123456789'\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-read-failure-tool",
            8,
            8,
        );
        let read_failure = {
            let _failure = ArtifactFailureGuard::set(ARTIFACT_FAILURE_READ);
            run_tool(
                &workspace_paths,
                &connection,
                "artifact-read-failure-tool",
                &[],
                None,
            )
        }
        .expect_err("forced artifact read failure should fail");
        assert!(matches!(read_failure, RunToolError::Io(_)));
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());

        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-write-failure-tool",
            "#!/bin/sh\nprintf '123456789'\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-write-failure-tool",
            8,
            8,
        );
        let write_failure = {
            let _failure = ArtifactFailureGuard::set(ARTIFACT_FAILURE_WRITE);
            run_tool(
                &workspace_paths,
                &connection,
                "artifact-write-failure-tool",
                &[],
                None,
            )
        }
        .expect_err("forced artifact write failure should fail");
        assert!(matches!(write_failure, RunToolError::Io(_)));
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());

        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-sync-failure-tool",
            "#!/bin/sh\nprintf '123456789'\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-sync-failure-tool",
            8,
            8,
        );
        let sync_failure = {
            let _failure = ArtifactFailureGuard::set(ARTIFACT_FAILURE_SYNC);
            run_tool(
                &workspace_paths,
                &connection,
                "artifact-sync-failure-tool",
                &[],
                None,
            )
        }
        .expect_err("forced artifact sync failure should fail");
        assert!(matches!(sync_failure, RunToolError::Io(_)));
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());

        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-publish-failure-tool",
            "#!/bin/sh\nprintf '123456789'\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-publish-failure-tool",
            8,
            8,
        );
        let publish_failure = {
            let _failure = ArtifactFailureGuard::set(ARTIFACT_FAILURE_PUBLISH);
            run_tool(
                &workspace_paths,
                &connection,
                "artifact-publish-failure-tool",
                &[],
                None,
            )
        }
        .expect_err("forced artifact publish failure should fail");
        assert!(matches!(publish_failure, RunToolError::Io(_)));
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn artifact_mode_approval_and_dry_run_create_nothing_until_approved_execution() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("artifact-approval");
        let marker = tool_dir(&workspace_paths, "artifact-approval-tool").join("runtime/ran");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-approval-tool",
            "#!/bin/sh\nprintf ran > \"$1\"\nprintf '123456789'\n",
        );
        let mut contract = read_tool_json(&workspace_paths, "artifact-approval-tool")
            .expect("approval contract should be readable");
        contract.side_effects = "external_write".to_string();
        contract.approval_requirement = "none".to_string();
        contract.runtime.output_mode = ToolOutputMode::Artifact;
        contract.runtime.stdout_limit_bytes = 8;
        contract.runtime.stderr_limit_bytes = 8;
        persist_test_tool_contract(&workspace_paths, &connection, contract);

        let blocked = run_tool(
            &workspace_paths,
            &connection,
            "artifact-approval-tool",
            &[marker.display().to_string()],
            None,
        )
        .expect_err("external write must be blocked without approval");
        assert!(
            matches!(blocked, RunToolError::UnsafeActionBlocked { .. }),
            "unexpected approval result: {blocked:?}"
        );
        assert!(!marker.exists());
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());
        assert!(fs::read_dir(workspace_paths.artifacts())
            .expect("artifacts root should be readable")
            .next()
            .is_none());

        let plan = dry_run_tool(
            &workspace_paths,
            &connection,
            "artifact-approval-tool",
            &[marker.display().to_string()],
        )
        .expect("dry-run should plan without execution");
        assert!(!plan.would_execute);
        assert!(!marker.exists());
        assert!(fs::read_dir(workspace_paths.artifacts())
            .expect("artifacts root should remain readable")
            .next()
            .is_none());

        let approved = run_tool(
            &workspace_paths,
            &connection,
            "artifact-approval-tool",
            &[marker.display().to_string()],
            Some("+++ approved external write"),
        )
        .expect("approved external write should execute artifact mode");
        assert!(marker.is_file());
        assert!(approved.artifacts.is_some());
        assert_eq!(artifact_run_entries(workspace_paths.artifacts()).len(), 1);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn artifact_hard_ceiling_plus_one_kills_tree_and_publishes_nothing() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("artifact-hard-ceiling");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "artifact-hard-ceiling-tool",
            "#!/bin/sh\n(sleep 5; printf escaped > \"$1\") &\nprintf '%s' \"$!\" > \"$2\"\ndd if=/dev/zero bs=10485761 count=1 2>/dev/null\nwait\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "artifact-hard-ceiling-tool",
            16,
            16,
        );
        let marker =
            tool_dir(&workspace_paths, "artifact-hard-ceiling-tool").join("runtime/escaped");
        let child_pid_file =
            tool_dir(&workspace_paths, "artifact-hard-ceiling-tool").join("runtime/child.pid");

        let error = run_tool(
            &workspace_paths,
            &connection,
            "artifact-hard-ceiling-tool",
            &[
                marker.display().to_string(),
                child_pid_file.display().to_string(),
            ],
            None,
        )
        .expect_err("hard ceiling plus one must fail");
        match error.limit_error() {
            Some(ToolRunLimitError::ArtifactHardOverflow {
                hard_limit_bytes,
                stdout_hard_limit_exceeded,
                stderr_hard_limit_exceeded,
                ..
            }) => {
                assert_eq!(*hard_limit_bytes, MAX_TOOL_RUN_OUTPUT_LIMIT_BYTES);
                assert!(*stdout_hard_limit_exceeded);
                assert!(!*stderr_hard_limit_exceeded);
            }
            other => panic!("unexpected hard-ceiling error: {other:?}"),
        }
        let child_pid: i32 = fs::read_to_string(&child_pid_file)
            .expect("descendant pid should be recorded before overflow")
            .parse()
            .expect("descendant pid should parse");
        assert_process_stops(child_pid);
        assert!(!marker.exists());
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn artifact_reader_retains_only_preview_in_memory_while_persisting_full_stream() {
        let path = temp_path("artifact-reader-bounded.bin");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("artifact reader fixture should open");
        let full = vec![0x5a; 2 * 1024 * 1024];
        let output = read_artifact_output(
            io::Cursor::new(full.clone()),
            file,
            17,
            full.len(),
            || {},
            || {},
        )
        .expect("artifact reader should stream fixture");

        assert_eq!(output.bytes, vec![0x5a; 17]);
        assert!(output.truncated);
        assert_eq!(output.total_bytes, full.len() as u64);
        assert!(!output.hard_overflowed);
        assert_eq!(
            fs::read(&path).expect("streamed artifact should be readable"),
            full
        );
        fs::remove_file(path).expect("artifact reader fixture should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn artifact_root_and_day_symlinks_are_rejected_before_tool_spawn() {
        use std::os::unix::fs::symlink;

        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");

        for case in ["root", "day"] {
            let (override_home, _aopmem_home, _home, workspace_paths, connection) =
                setup_test_workspace(&format!("artifact-{case}-symlink"));
            create_runnable_test_tool(
                &workspace_paths,
                &connection,
                "artifact-symlink-tool",
                "#!/bin/sh\nprintf spawned > \"$1\"\nprintf '123456789'\n",
            );
            set_test_tool_artifact_runtime(
                &workspace_paths,
                &connection,
                "artifact-symlink-tool",
                8,
                8,
            );
            let marker =
                tool_dir(&workspace_paths, "artifact-symlink-tool").join("runtime/spawned");
            let outside = temp_path(&format!("artifact-{case}-outside"));
            fs::create_dir_all(&outside).expect("outside directory should be created");
            let sentinel = outside.join("sentinel");
            fs::write(&sentinel, b"safe").expect("outside sentinel should be written");

            if case == "root" {
                fs::remove_dir(workspace_paths.artifacts())
                    .expect("empty artifacts root should be removable");
                symlink(&outside, workspace_paths.artifacts())
                    .expect("artifacts root symlink should be created");
            } else {
                let day: String = connection
                    .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))
                    .expect("local artifact day should resolve");
                symlink(&outside, workspace_paths.artifacts().join(day))
                    .expect("artifact day symlink should be created");
            }

            let error = run_tool(
                &workspace_paths,
                &connection,
                "artifact-symlink-tool",
                &[marker.display().to_string()],
                None,
            )
            .expect_err("artifact symlink must be rejected before spawn");
            assert!(matches!(error, RunToolError::Io(_)));
            assert!(!marker.exists());
            assert_eq!(
                fs::read(&sentinel).expect("sentinel should remain"),
                b"safe"
            );

            if case == "root" {
                fs::remove_file(workspace_paths.artifacts())
                    .expect("artifacts root symlink should be removed");
            } else {
                let day: String = connection
                    .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))
                    .expect("local artifact day should resolve");
                fs::remove_file(workspace_paths.artifacts().join(day))
                    .expect("artifact day symlink should be removed");
            }
            fs::remove_dir_all(&outside).expect("outside directory should be removed");
            fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_prepared_command_anchors_executable_cwd_args_and_sibling_resources() {
        let root = temp_path("macos-stable-tool-root");
        let bin = root.join("bin");
        let runtime = root.join("runtime");
        fs::create_dir_all(&bin).expect("stable command bin should be created");
        fs::create_dir_all(&runtime).expect("stable command runtime should be created");
        fs::write(runtime.join("sibling.txt"), b"sibling")
            .expect("sibling resource should be written");
        let executable = bin.join("runner");
        write_executable(
            &executable,
            "#!/bin/sh\nprintf 'original|%s|%s|%s' \"$1\" \"$PWD\" \"$(cat runtime/sibling.txt)\"\n",
        );
        let canonical_root = root
            .canonicalize()
            .expect("stable tool root should canonicalize");
        let canonical_executable = canonical_root.join("bin/runner");
        let args = vec!["argument with spaces".to_string()];
        let mut prepared = prepare_tool_command(&canonical_executable, &canonical_root, &args)
            .expect("stable macOS command should prepare");

        let original_bin = root.join("bin.original");
        fs::rename(&bin, &original_bin)
            .expect("opened executable ancestor should remain renameable");
        fs::create_dir(&bin).expect("replacement bin should be created");
        write_executable(&bin.join("runner"), "#!/bin/sh\nprintf replacement\n");
        let output = prepared
            .command
            .output()
            .expect("stable macOS command should execute");

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).expect("stable output should be UTF-8"),
            format!(
                "original|argument with spaces|{}|sibling",
                canonical_root.display()
            )
        );
        drop(prepared);
        assert!(fs::read_dir(&root)
            .expect("tool root should remain readable")
            .all(|entry| !entry
                .expect("tool root entry should be readable")
                .file_name()
                .to_string_lossy()
                .starts_with(".aopmem-exec-")));
        fs::remove_dir_all(root).expect("stable command fixture should be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_tracker_identity_reuse_cap_and_completed_eperm_are_deterministic() {
        let root = DarwinProcessIdentity {
            pid: 101,
            started_seconds: 10,
            started_microseconds: 20,
        };
        let mut tracker = DarwinDescendantTracker::new(root);
        assert!(!tracker
            .insert_with_ceiling(root, 1)
            .expect("same identity should be a no-op"));
        let reused = DarwinProcessIdentity {
            started_microseconds: 21,
            ..root
        };
        assert!(tracker
            .insert_with_ceiling(reused, 1)
            .expect("reused pid with a new start time should replace identity"));
        let second = DarwinProcessIdentity { pid: 102, ..root };
        assert!(tracker.insert_with_ceiling(second, 1).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_only_completed_exact_root_can_suppress_contextual_eperm() {
        let root = DarwinProcessIdentity {
            pid: 201,
            started_seconds: 10,
            started_microseconds: 20,
        };
        let descendant = DarwinProcessIdentity { pid: 202, ..root };
        assert!(darwin_completed_root_error_is_benign(
            Some(libc::EPERM),
            root,
            root,
            true
        ));
        assert!(!darwin_completed_root_error_is_benign(
            Some(libc::EPERM),
            root,
            root,
            false
        ));
        assert!(!darwin_completed_root_error_is_benign(
            Some(libc::EPERM),
            descendant,
            root,
            true
        ));
        let completed = darwin_tracked_process_details_with(
            root,
            root,
            || Err(io::Error::from_raw_os_error(libc::EPERM)),
            || Ok(true),
        )
        .expect("completed exact root EPERM should be benign");
        assert!(completed.is_none());

        let live_error = darwin_tracked_process_details_with(
            root,
            root,
            || Err(io::Error::from_raw_os_error(libc::EPERM)),
            || Ok(false),
        )
        .expect_err("live root EPERM must remain fail-closed");
        assert_eq!(live_error.raw_os_error(), Some(libc::EPERM));

        for strict_identity in [
            descendant,
            DarwinProcessIdentity {
                started_microseconds: root.started_microseconds + 1,
                ..root
            },
        ] {
            let completion_checked = std::cell::Cell::new(false);
            let error = darwin_tracked_process_details_with(
                strict_identity,
                root,
                || Err(io::Error::from_raw_os_error(libc::EPERM)),
                || {
                    completion_checked.set(true);
                    Ok(true)
                },
            )
            .expect_err("descendant and reused-root EPERM must remain fail-closed");
            assert_eq!(error.raw_os_error(), Some(libc::EPERM));
            assert!(!completion_checked.get());
        }

        let reused_root_pid = DarwinProcessIdentity {
            started_microseconds: root.started_microseconds + 1,
            ..root
        };
        let mut live = vec![root, descendant, reused_root_pid];
        darwin_remove_completed_exact_root(&mut live, root, true);
        assert_eq!(live, vec![descendant, reused_root_pid]);
        darwin_remove_completed_exact_root(&mut live, root, false);
        assert_eq!(live, vec![descendant, reused_root_pid]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_libproc_identity_eperm_is_fail_closed() {
        let error = darwin_process_details_with(101, |_, _, _, _, _| {
            Err(io::Error::from_raw_os_error(libc::EPERM))
        })
        .expect_err("EPERM must not make a tracked process look absent");

        assert_eq!(error.raw_os_error(), Some(libc::EPERM));
        assert!(!darwin_process_is_absent(&error));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_libproc_child_list_eperm_is_fail_closed() {
        let estimate_error = darwin_child_pids_with(101, 4, |_, _, _| {
            Err(io::Error::from_raw_os_error(libc::EPERM))
        })
        .expect_err("EPERM from the child-count query must propagate");
        assert_eq!(estimate_error.raw_os_error(), Some(libc::EPERM));

        let mut calls = 0;
        let list_error = darwin_child_pids_with(101, 4, |_, _, _| {
            calls += 1;
            if calls == 1 {
                Ok(1)
            } else {
                Err(io::Error::from_raw_os_error(libc::EPERM))
            }
        })
        .expect_err("EPERM from the child-list query must propagate");
        assert_eq!(list_error.raw_os_error(), Some(libc::EPERM));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_libproc_absence_errors_remain_benign() {
        for error_code in [libc::ESRCH, libc::ENOENT, libc::EINVAL] {
            let details = darwin_process_details_with(101, |_, _, _, _, _| {
                Err(io::Error::from_raw_os_error(error_code))
            })
            .expect("an absent process identity should be benign");
            assert!(details.is_none());

            let children = darwin_child_pids_with(101, 4, |_, _, _| {
                Err(io::Error::from_raw_os_error(error_code))
            })
            .expect("an absent parent child-list should be benign");
            assert!(children.is_empty());
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_fast_same_group_orphan_is_killed_after_parent_success() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("macos-fast-same-group-orphan");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "macos-fast-same-group-orphan-tool",
            "#!/bin/sh\n(trap '' HUP; exec >/dev/null 2>&1; sleep 2; printf escaped > \"$1\") &\nprintf '%s' \"$!\" > \"$2\"\nexit 0\n",
        );
        let tool_root = tool_dir(&workspace_paths, "macos-fast-same-group-orphan-tool");
        let marker = tool_root.join("runtime/escaped");
        let pid_path = tool_root.join("runtime/child.pid");
        let record = run_tool(
            &workspace_paths,
            &connection,
            "macos-fast-same-group-orphan-tool",
            &[marker.display().to_string(), pid_path.display().to_string()],
            None,
        )
        .expect("successful parent should still clean up its same-group orphan");
        assert_eq!(record.exit_code, 0);
        let pid = fs::read_to_string(pid_path)
            .expect("orphan pid should be recorded")
            .parse()
            .expect("orphan pid should parse");
        assert_process_stops(pid);
        thread::sleep(Duration::from_millis(50));
        assert!(!marker.exists());
        fs::remove_dir_all(override_home).expect("same-group fixture should be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_repeated_short_tool_runs_do_not_receive_stale_cleanup_signal() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("macos-repeated-short-runs");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "macos-repeated-short-runs-tool",
            "#!/bin/sh\nprintf '%s\\n' \"$1\"\n",
        );

        for invocation in 0..100 {
            let expected = invocation.to_string();
            let record = run_tool(
                &workspace_paths,
                &connection,
                "macos-repeated-short-runs-tool",
                std::slice::from_ref(&expected),
                None,
            )
            .unwrap_or_else(|error| {
                panic!("short tool invocation {invocation} should succeed: {error:?}")
            });
            assert_eq!(record.exit_code, 0);
            assert_eq!(record.stdout, format!("{expected}\n"));
        }

        fs::remove_dir_all(override_home).expect("repeated-run fixture should be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_executable_snapshot_fallback_is_bounded_and_runnable() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let _snapshot_hook = DarwinSnapshotHookGuard::set(true, false);
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("macos-snapshot-fallback");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "macos-snapshot-fallback-tool",
            "#!/bin/sh\nprintf fallback\n",
        );
        let record = run_tool(
            &workspace_paths,
            &connection,
            "macos-snapshot-fallback-tool",
            &[],
            None,
        )
        .expect("bounded fd-copy fallback should remain runnable");
        assert_eq!(record.stdout, "fallback");
        fs::remove_dir_all(override_home).expect("fallback fixture should be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_executable_snapshot_fails_closed_on_in_place_source_mutation() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let _snapshot_hook = DarwinSnapshotHookGuard::set(true, true);
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("macos-snapshot-mutation");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "macos-snapshot-mutation-tool",
            "#!/bin/sh\nprintf original\n",
        );
        let error = run_tool(
            &workspace_paths,
            &connection,
            "macos-snapshot-mutation-tool",
            &[],
            None,
        )
        .expect_err("in-place source mutation must fail closed");
        assert!(matches!(error, RunToolError::Io(_)));
        let tool_root = tool_dir(&workspace_paths, "macos-snapshot-mutation-tool");
        assert!(fs::read_dir(tool_root)
            .expect("tool root should remain readable")
            .all(|entry| !entry
                .expect("tool root entry should be readable")
                .file_name()
                .to_string_lossy()
                .starts_with(".aopmem-exec-")));
        fs::remove_dir_all(override_home).expect("mutation fixture should be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_setsid_descendant_is_killed_on_inline_timeout() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("macos-setsid-timeout");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "macos-setsid-timeout-tool",
            "#!/bin/sh\nperl -MPOSIX=setsid -e 'setsid(); sleep 5; open(my $f, q(>), $ARGV[0]) or die; print $f q(escaped)' \"$1\" &\nprintf '%s' \"$!\" > \"$2\"\nwait\n",
        );
        let marker =
            tool_dir(&workspace_paths, "macos-setsid-timeout-tool").join("runtime/escaped");
        let pid_path =
            tool_dir(&workspace_paths, "macos-setsid-timeout-tool").join("runtime/child.pid");
        let error = run_tool_with_limits(
            &workspace_paths,
            &connection,
            "macos-setsid-timeout-tool",
            &[marker.display().to_string(), pid_path.display().to_string()],
            None,
            ToolRunLimits {
                timeout: Duration::from_secs(2),
                stdout_max_bytes: 1024,
                stderr_max_bytes: 1024,
            },
        )
        .expect_err("setsid descendant should be terminated on timeout");
        assert!(matches!(
            error.limit_error(),
            Some(ToolRunLimitError::TimedOut { .. })
        ));
        let pid = fs::read_to_string(pid_path)
            .expect("detached pid should be recorded")
            .parse()
            .expect("detached pid should parse");
        assert_process_stops(pid);
        assert!(!marker.exists());
        fs::remove_dir_all(override_home).expect("timeout fixture should be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_setsid_descendant_is_killed_on_inline_output_overflow() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("macos-setsid-inline-overflow");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "macos-setsid-inline-overflow-tool",
            "#!/bin/sh\nperl -MPOSIX=setsid -e 'setsid(); sleep 5; open(my $f, q(>), $ARGV[0]) or die; print $f q(escaped)' \"$1\" &\nprintf '%s' \"$!\" > \"$2\"\nsleep 0.1\ni=0\nwhile [ \"$i\" -lt 4096 ]; do printf 0123456789abcdef; i=$((i + 1)); done\nwait\n",
        );
        let marker =
            tool_dir(&workspace_paths, "macos-setsid-inline-overflow-tool").join("runtime/escaped");
        let pid_path = tool_dir(&workspace_paths, "macos-setsid-inline-overflow-tool")
            .join("runtime/child.pid");
        let error = run_tool_with_limits(
            &workspace_paths,
            &connection,
            "macos-setsid-inline-overflow-tool",
            &[marker.display().to_string(), pid_path.display().to_string()],
            None,
            ToolRunLimits {
                timeout: Duration::from_secs(3),
                stdout_max_bytes: 32,
                stderr_max_bytes: 1024,
            },
        )
        .expect_err("setsid descendant should be terminated on inline overflow");
        assert!(matches!(
            error.limit_error(),
            Some(ToolRunLimitError::OutputOverflow { .. })
        ));
        let pid = fs::read_to_string(pid_path)
            .expect("detached pid should be recorded")
            .parse()
            .expect("detached pid should parse");
        assert_process_stops(pid);
        assert!(!marker.exists());
        fs::remove_dir_all(override_home).expect("inline overflow fixture should be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_setsid_descendant_is_killed_on_artifact_hard_overflow() {
        let _lock = crate::install::test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let (override_home, _aopmem_home, _home, workspace_paths, connection) =
            setup_test_workspace("macos-setsid-artifact-overflow");
        create_runnable_test_tool(
            &workspace_paths,
            &connection,
            "macos-setsid-artifact-overflow-tool",
            "#!/bin/sh\nperl -MPOSIX=setsid -e 'setsid(); sleep 5; open(my $f, q(>), $ARGV[0]) or die; print $f q(escaped)' \"$1\" &\nprintf '%s' \"$!\" > \"$2\"\nsleep 0.1\ndd if=/dev/zero bs=10485761 count=1 2>/dev/null\nwait\n",
        );
        set_test_tool_artifact_runtime(
            &workspace_paths,
            &connection,
            "macos-setsid-artifact-overflow-tool",
            16,
            16,
        );
        let marker = tool_dir(&workspace_paths, "macos-setsid-artifact-overflow-tool")
            .join("runtime/escaped");
        let pid_path = tool_dir(&workspace_paths, "macos-setsid-artifact-overflow-tool")
            .join("runtime/child.pid");
        let error = run_tool(
            &workspace_paths,
            &connection,
            "macos-setsid-artifact-overflow-tool",
            &[marker.display().to_string(), pid_path.display().to_string()],
            None,
        )
        .expect_err("setsid descendant should be terminated on artifact hard overflow");
        assert!(matches!(
            error.limit_error(),
            Some(ToolRunLimitError::ArtifactHardOverflow { .. })
        ));
        let pid = fs::read_to_string(pid_path)
            .expect("detached pid should be recorded")
            .parse()
            .expect("detached pid should parse");
        assert_process_stops(pid);
        assert!(!marker.exists());
        assert!(artifact_run_entries(workspace_paths.artifacts()).is_empty());
        fs::remove_dir_all(override_home).expect("artifact overflow fixture should be removed");
    }

    #[test]
    fn windows_runner_uses_kill_on_close_job_object_contract() {
        let source = include_str!("mod.rs");
        let artifact_source = include_str!("../artifacts/mod.rs");
        assert!(source.contains("JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE"));
        assert!(source.contains("AssignProcessToJobObject"));
        assert!(source.contains("TerminateJobObject"));
        assert!(source.contains("CREATE_SUSPENDED"));
        assert!(source.contains("QueryFullProcessImageNameW"));
        assert!(source.contains("verify_suspended_windows_process_image"));
        assert!(source.contains("FILE_SHARE_READ | FILE_SHARE_WRITE"));
        assert!(!source
            .contains(&["FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_", "DELETE"].concat()));
        assert!(source.contains("relative_executable.components()"));
        assert!(source.contains("GetFileInformationByHandle"));
        assert!(source.contains("GetFinalPathNameByHandleW"));
        assert!(source.contains("opened Windows tool component did not match"));
        assert!(source.contains("resume_suspended_process"));
        assert!(source.contains("#[cfg(windows)]\nstruct ToolProcessTree"));
        assert!(artifact_source.contains("FILE_ATTRIBUTE_REPARSE_POINT"));
        assert!(artifact_source.contains("MOVEFILE_WRITE_THROUGH"));
        assert!(!artifact_source.contains(&["MOVEFILE_REPLACE", "_EXISTING"].concat()));
    }
}
