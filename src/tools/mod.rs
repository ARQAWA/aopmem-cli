//! Tool registry and `tool.json` contract helpers.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::storage;

/// Canonical `tool.json` file name.
pub const TOOL_JSON_FILE_NAME: &str = "tool.json";
/// Draft tool status created by `aopmem tool create-draft`.
pub const DRAFT_TOOL_STATUS: &str = "draft";
/// Draft-local executable directory under `tools/<tool-id>/`.
pub const TOOL_BIN_DIR_NAME: &str = "bin";
/// Draft-local runtime directory under `tools/<tool-id>/`.
pub const TOOL_RUNTIME_DIR_NAME: &str = "runtime";

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
    pub supports_dry_run: bool,
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
}

/// SQLite-backed registered tool contract with timestamps.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolContractRecord {
    #[serde(flatten)]
    pub contract: ToolContract,
    pub created_at: String,
    pub updated_at: String,
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

/// Result of a validated tool manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolValidationRecord {
    pub tool_id: String,
    pub tool_json_path: String,
    pub executable_path: String,
    pub side_effects: String,
    pub approval_requirement: String,
    pub runner_dry_run_supported: bool,
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
    #[error("missing required field: name")]
    MissingName,
    #[error("missing required field: status")]
    MissingStatus,
    #[error("owner_workflow must not be blank when present")]
    BlankOwnerWorkflow,
    #[error("missing required field: command.entrypoint")]
    MissingCommandEntrypoint,
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
    #[error("example name must not be blank")]
    BlankExampleName,
    #[error("missing required field: runtime.executable_path")]
    MissingRuntimeExecutablePath,
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
    Io(#[from] io::Error),
    #[error("tool process exited with non-zero status: {0}")]
    ProcessFailed(i32),
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

/// Writes `tool.json` under `tools/<tool-id>/`.
pub fn write_tool_json(
    workspace_paths: &storage::WorkspacePaths,
    contract: &ToolContract,
) -> Result<PathBuf, ToolJsonError> {
    validate_tool_contract(contract)?;

    let tool_dir = tool_dir(workspace_paths, &contract.tool_id);
    fs::create_dir_all(&tool_dir)?;

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
    let contract = draft_tool_contract(input);
    let record = create_tool_contract(connection, &contract)?;
    let tool_root = tool_dir(workspace_paths, &contract.tool_id);
    let bin_dir = tool_root.join(TOOL_BIN_DIR_NAME);
    let runtime_dir = tool_root.join(TOOL_RUNTIME_DIR_NAME);

    let create_result = (|| -> Result<PathBuf, CreateDraftToolError> {
        fs::create_dir_all(&bin_dir)?;
        fs::create_dir_all(&runtime_dir)?;
        write_tool_json(workspace_paths, &contract).map_err(CreateDraftToolError::from)
    })();

    let tool_json_path = match create_result {
        Ok(path) => path,
        Err(error) => {
            let _deleted = connection.execute(
                "DELETE FROM tool_contracts WHERE tool_id = ?1;",
                [&contract.tool_id],
            );
            return Err(error);
        }
    };

    Ok(DraftToolRecord {
        record,
        tool_dir: tool_root.display().to_string(),
        tool_json_path: tool_json_path.display().to_string(),
        bin_dir: bin_dir.display().to_string(),
        runtime_dir: runtime_dir.display().to_string(),
    })
}

/// Validates one registered tool and its local executable reference.
pub fn validate_tool(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
) -> Result<ToolValidationRecord, ValidateToolError> {
    let contract = load_canonical_tool_contract(workspace_paths, connection, tool_id)?;
    let executable_path = resolve_executable_path(
        &tool_dir(workspace_paths, tool_id),
        &contract.runtime.executable_path,
    );
    if !executable_path.is_file() {
        return Err(ValidateToolError::MissingExecutablePath(
            executable_path.display().to_string(),
        ));
    }

    Ok(ToolValidationRecord {
        tool_id: contract.tool_id,
        tool_json_path: tool_json_path(workspace_paths, tool_id)
            .display()
            .to_string(),
        executable_path: executable_path.display().to_string(),
        side_effects: contract.side_effects,
        approval_requirement: contract.approval_requirement,
        runner_dry_run_supported: true,
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
pub fn run_tool(
    workspace_paths: &storage::WorkspacePaths,
    connection: &Connection,
    tool_id: &str,
    args: &[String],
    approved: Option<&str>,
) -> Result<ToolRunRecord, RunToolError> {
    let contract = load_canonical_tool_contract(workspace_paths, connection, tool_id)?;
    if !can_run_tool(&contract, approved) {
        return Err(RunToolError::UnsafeActionBlocked {
            tool_id: contract.tool_id,
            side_effects: contract.side_effects,
            approval_requirement: contract.approval_requirement,
        });
    }

    let tool_root = tool_dir(workspace_paths, tool_id);
    let executable_path = resolve_executable_path(&tool_root, &contract.runtime.executable_path);
    if !executable_path.is_file() {
        return Err(RunToolError::MissingExecutablePath(
            executable_path.display().to_string(),
        ));
    }

    let output = Command::new(&executable_path)
        .current_dir(&tool_root)
        .args(args)
        .output()?;
    let exit_code = output.status.code().unwrap_or(-1);
    if !output.status.success() {
        return Err(RunToolError::ProcessFailed(exit_code));
    }

    Ok(ToolRunRecord {
        tool_id: contract.tool_id,
        tool_json_path: tool_json_path(workspace_paths, tool_id)
            .display()
            .to_string(),
        executable_path: executable_path.display().to_string(),
        args: args.to_vec(),
        exit_code,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn draft_tool_contract(input: &DraftToolInput) -> ToolContract {
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
            supports_dry_run: false,
        },
    }
}

fn validate_tool_contract(contract: &ToolContract) -> Result<(), ToolContractValidationError> {
    if contract.tool_id.trim().is_empty() {
        return Err(ToolContractValidationError::MissingToolId);
    }
    if contract.name.trim().is_empty() {
        return Err(ToolContractValidationError::MissingName);
    }
    if contract.status.trim().is_empty() {
        return Err(ToolContractValidationError::MissingStatus);
    }
    if contract
        .owner_workflow
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(ToolContractValidationError::BlankOwnerWorkflow);
    }
    if contract.command.entrypoint.trim().is_empty() {
        return Err(ToolContractValidationError::MissingCommandEntrypoint);
    }
    if !contract.args_schema.is_object() {
        return Err(ToolContractValidationError::ArgsSchemaMustBeObject);
    }
    if !contract.output_schema.is_object() {
        return Err(ToolContractValidationError::OutputSchemaMustBeObject);
    }
    if !ALLOWED_TOOL_SIDE_EFFECTS.contains(&contract.side_effects.as_str()) {
        return Err(ToolContractValidationError::InvalidSideEffects(
            contract.side_effects.clone(),
        ));
    }
    if contract.approval_requirement.trim().is_empty() {
        return Err(ToolContractValidationError::MissingApprovalRequirement);
    }
    if contract.examples.is_empty() {
        return Err(ToolContractValidationError::MissingExamples);
    }
    if contract
        .examples
        .iter()
        .any(|example| example.name.trim().is_empty())
    {
        return Err(ToolContractValidationError::BlankExampleName);
    }
    if contract.runtime.executable_path.trim().is_empty() {
        return Err(ToolContractValidationError::MissingRuntimeExecutablePath);
    }

    Ok(())
}

fn resolve_executable_path(tool_root: &Path, executable_path: &str) -> PathBuf {
    let path = PathBuf::from(executable_path);
    if path.is_absolute() {
        path
    } else {
        tool_root.join(path)
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
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

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
                supports_dry_run: true,
            },
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
        assert_eq!(PathBuf::from(&validated.executable_path), executable_path);

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
    fn run_tool_executes_safe_registered_tool() {
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
        let mut permissions = fs::metadata(&executable_path)
            .expect("tool script metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, permissions)
            .expect("tool script should be executable");

        let ran = run_tool(
            &workspace_paths,
            &connection,
            "run-safe-tool",
            &["--json".to_string(), "value".to_string()],
            None,
        )
        .expect("tool run should pass");

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
        let mut permissions = fs::metadata(&executable_path)
            .expect("drift tool metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, permissions)
            .expect("drift tool should be executable");
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

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn run_tool_blocks_approval_required_or_side_effectful_tool() {
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
            approval_requirement: "manual_review".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before blocked run");
        let executable_path =
            tool_dir(&workspace_paths, "run-blocked-tool").join("bin/run-blocked-tool");
        fs::write(&executable_path, "#!/bin/sh\nexit 0\n")
            .expect("blocked tool script should still be created");
        let mut permissions = fs::metadata(&executable_path)
            .expect("blocked tool script metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, permissions)
            .expect("blocked tool script should be executable");

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
                assert_eq!(approval_requirement, "manual_review");
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
        fs::write(
            &executable_path,
            format!(
                "#!/bin/sh\nprintf side-effect > {}\n",
                side_effect_path.display()
            ),
        )
        .expect("dry-run script should be created");

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
    fn run_tool_allows_external_read_when_approval_requirement_is_none() {
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
        let mut permissions = fs::metadata(&executable_path)
            .expect("external read script metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, permissions)
            .expect("external read script should be executable");

        let record = run_tool(
            &workspace_paths,
            &connection,
            "external-read-tool",
            &[],
            None,
        )
        .expect("external_read with no approval requirement should run");

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
    fn run_tool_accepts_approval_text_with_triple_plus() {
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
            approval_requirement: "manual_review".to_string(),
        };

        create_draft_tool(&workspace_paths, &connection, &input)
            .expect("draft should create before approved run");
        let executable_path =
            tool_dir(&workspace_paths, "run-approved-tool").join("bin/run-approved-tool");
        fs::write(&executable_path, "#!/bin/sh\necho approved-run\n")
            .expect("approved tool script should be created");
        let mut permissions = fs::metadata(&executable_path)
            .expect("approved tool script metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, permissions)
            .expect("approved tool script should be executable");

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
}
