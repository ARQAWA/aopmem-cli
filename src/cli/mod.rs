use clap::{error::ErrorKind, Args, Parser, Subcommand};
use serde::Serialize;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::adapter;
use crate::artifacts;
use crate::audit;
use crate::install;
use crate::recall;
use crate::reflection;
use crate::storage;
use crate::tools;
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
    Recall,
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
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum NodeCommand {
    Create(NodeCreateArgs),
    Get(NodeGetArgs),
    List,
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
struct NodeUpdateArgs {
    #[arg(long)]
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
    List,
}

#[derive(Debug, Args)]
struct LinkAddArgs {
    #[arg(long)]
    source_id: i64,

    #[arg(long)]
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
    #[arg(long)]
    session_id: i64,

    #[arg(long)]
    payload: String,
}

#[derive(Debug, Args)]
struct TeachApplyArgs {
    #[arg(long)]
    session_id: i64,

    #[arg(long)]
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
    #[arg(long)]
    proposal_id: i64,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum ToolCommand {
    CreateDraft(ToolCreateDraftArgs),
    List,
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
    List,
    Add(McpAddArgs),
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

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
enum ArtifactsCommand {
    Cleanup,
}

pub fn run() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => run_parsed(&cli),
        Err(error) => handle_parse_error(error, json_flag_present(env::args())),
    }
}

fn run_parsed(cli: &Cli) -> ExitCode {
    run_command_with_approval(&cli.command, cli.json, cli.approved.as_deref())
}

#[cfg(test)]
fn run_command(command: &Command, json: bool) -> ExitCode {
    run_command_with_approval(command, json, None)
}

fn run_command_with_approval(command: &Command, json: bool, approved: Option<&str>) -> ExitCode {
    let command_id = command_id(command);
    match command {
        Command::Init => run_init(command_id, json),
        Command::Status => run_status(command_id, json),
        Command::Doctor => run_doctor(command_id, json),
        Command::Verify => run_verify(command_id, json),
        Command::Node {
            command: NodeCommand::Create(args),
        } => run_node_create(command_id, args, json),
        Command::Node {
            command: NodeCommand::Get(args),
        } => run_node_get(command_id, args, json),
        Command::Node {
            command: NodeCommand::List,
        } => run_node_list(command_id, json),
        Command::Node {
            command: NodeCommand::Update(args),
        } => run_node_update(command_id, args, json),
        Command::Link {
            command: LinkCommand::Add(args),
        } => run_link_add(command_id, args, json),
        Command::Link {
            command: LinkCommand::List,
        } => run_link_list(command_id, json),
        Command::Alias {
            command: AliasCommand::Add(args),
        } => run_alias_add(command_id, args, json),
        Command::Alias {
            command: AliasCommand::List(args),
        } => run_alias_list(command_id, args, json),
        Command::Tag {
            command: TagCommand::Add(args),
        } => run_tag_add(command_id, args, json),
        Command::Tag {
            command: TagCommand::List(args),
        } => run_tag_list(command_id, args, json),
        Command::Source {
            command: SourceCommand::Add(args),
        } => run_source_add(command_id, args, json),
        Command::Source {
            command: SourceCommand::List(args),
        } => run_source_list(command_id, args, json),
        Command::Mcp {
            command: McpCommand::List,
        } => run_mcp_list(command_id, json),
        Command::Mcp {
            command: McpCommand::Add(args),
        } => run_mcp_add(command_id, args, json),
        Command::Mcp {
            command: McpCommand::Get(args),
        } => run_mcp_get(command_id, args, json),
        Command::Recall => run_recall(command_id, json),
        Command::Remember(args) => run_remember(command_id, args, json),
        Command::Teach {
            command: TeachCommand::Start(args),
        } => run_teach_start(command_id, args, json),
        Command::Teach {
            command: TeachCommand::Add(args),
        } => run_teach_add(command_id, args, json),
        Command::Teach {
            command: TeachCommand::Propose(args),
        } => run_teach_propose(command_id, args, json),
        Command::Teach {
            command: TeachCommand::Apply(args),
        } => run_teach_apply(command_id, args, json),
        Command::Reflect {
            command: ReflectCommand::Inventory,
        } => run_reflect_inventory(command_id, json),
        Command::Reflect {
            command:
                ReflectCommand::Proposal {
                    command: ReflectProposalCommand::Create(args),
                },
        } => run_reflect_proposal_create(command_id, args, json),
        Command::Reflect {
            command:
                ReflectCommand::Proposal {
                    command: ReflectProposalCommand::Apply(args),
                },
        } => run_reflect_proposal_apply(command_id, args, json),
        Command::Adapter {
            command: AdapterCommand::Seed(args),
        } => run_adapter_seed(command_id, args, json),
        Command::Adapter {
            command: AdapterCommand::Sync(args),
        } => run_adapter_sync(command_id, args, json),
        Command::Adapter {
            command: AdapterCommand::Status(args),
        } => run_adapter_status(command_id, args, json),
        Command::Tool {
            command: ToolCommand::CreateDraft(args),
        } => run_tool_create_draft(command_id, args, json),
        Command::Tool {
            command: ToolCommand::List,
        } => run_tool_list(command_id, json),
        Command::Tool {
            command: ToolCommand::Get(args),
        } => run_tool_get(command_id, args, json),
        Command::Tool {
            command: ToolCommand::Run(args),
        } => run_tool_run(command_id, args, json, approved),
        Command::Tool {
            command: ToolCommand::Validate(args),
        } => run_tool_validate(command_id, args, json),
        Command::Artifacts {
            command: ArtifactsCommand::Cleanup,
        } => run_artifacts_cleanup(command_id, json),
    }
}

fn run_tool_create_draft(
    command_id: &'static str,
    args: &ToolCreateDraftArgs,
    json_output: bool,
) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
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

    match tools::create_draft_tool(&workspace_paths, &connection, &input) {
        Ok(record) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(record),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(tools::CreateDraftToolError::Storage(tools::ToolContractStorageError::Validation(
            error,
        )))
        | Err(tools::CreateDraftToolError::Json(tools::ToolJsonError::Validation(error))) => {
            print_error(command_id, &CliError::tool_validation(error), json_output)
        }
        Err(tools::CreateDraftToolError::Storage(tools::ToolContractStorageError::Db(error))) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(tools::CreateDraftToolError::Storage(tools::ToolContractStorageError::Json(error)))
        | Err(tools::CreateDraftToolError::Json(tools::ToolJsonError::Json(error))) => print_error(
            command_id,
            &CliError::tool_contract_json(error),
            json_output,
        ),
        Err(tools::CreateDraftToolError::Json(tools::ToolJsonError::Io(error)))
        | Err(tools::CreateDraftToolError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
    }
}

fn run_tool_validate(
    command_id: &'static str,
    args: &ToolValidateArgs,
    json_output: bool,
) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match tools::validate_tool(&workspace_paths, &connection, &args.tool_id) {
        Ok(record) => print_success(
            command_id,
            json!(record),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(tools::ValidateToolError::NotFound(tool_id)) => {
            print_error(command_id, &CliError::tool_not_found(&tool_id), json_output)
        }
        Err(tools::ValidateToolError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(tools::ValidateToolError::Json(tools::ToolJsonError::Validation(error))) => {
            print_error(command_id, &CliError::tool_validation(error), json_output)
        }
        Err(tools::ValidateToolError::Json(tools::ToolJsonError::Json(error))) => print_error(
            command_id,
            &CliError::tool_contract_json(error),
            json_output,
        ),
        Err(tools::ValidateToolError::Json(tools::ToolJsonError::Io(error))) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
        Err(tools::ValidateToolError::ContractDrift(tool_id)) => print_error(
            command_id,
            &CliError::tool_contract_drift(&tool_id),
            json_output,
        ),
        Err(tools::ValidateToolError::MissingExecutablePath(path)) => print_error(
            command_id,
            &CliError::tool_executable_missing(&path),
            json_output,
        ),
    }
}

fn run_tool_list(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, _workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match tools::list_tool_contracts(&connection) {
        Ok(tools) => print_success(
            command_id,
            json!({ "tools": tools }),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_tool_get(command_id: &'static str, args: &ToolGetArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
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
) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    let result = if args.dry_run {
        tools::dry_run_tool(&workspace_paths, &connection, &args.tool_id, &args.args)
            .map(tools::ToolInvocationRecord::DryRun)
    } else {
        tools::run_tool(
            &workspace_paths,
            &connection,
            &args.tool_id,
            &args.args,
            approved,
        )
        .map(tools::ToolInvocationRecord::Run)
    };

    match result {
        Ok(record) => print_success(
            command_id,
            json!(record),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(tools::RunToolError::NotFound(tool_id)) => {
            print_error(command_id, &CliError::tool_not_found(&tool_id), json_output)
        }
        Err(tools::RunToolError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(tools::RunToolError::Json(tools::ToolJsonError::Validation(error))) => {
            print_error(command_id, &CliError::tool_validation(error), json_output)
        }
        Err(tools::RunToolError::Json(tools::ToolJsonError::Json(error))) => print_error(
            command_id,
            &CliError::tool_contract_json(error),
            json_output,
        ),
        Err(tools::RunToolError::Json(tools::ToolJsonError::Io(error)))
        | Err(tools::RunToolError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
        Err(tools::RunToolError::ContractDrift(tool_id)) => print_error(
            command_id,
            &CliError::tool_contract_drift(&tool_id),
            json_output,
        ),
        Err(tools::RunToolError::MissingExecutablePath(path)) => print_error(
            command_id,
            &CliError::tool_executable_missing(&path),
            json_output,
        ),
        Err(tools::RunToolError::UnsafeActionBlocked {
            tool_id,
            side_effects,
            approval_requirement,
        }) => print_error(
            command_id,
            &CliError::unsafe_tool_run_blocked(&tool_id, &side_effects, &approval_requirement),
            json_output,
        ),
        Err(tools::RunToolError::ProcessFailed(exit_code)) => print_error(
            command_id,
            &CliError::tool_process_failed(exit_code),
            json_output,
        ),
    }
}

fn run_artifacts_cleanup(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match artifacts::cleanup_workspace_artifacts(&workspace_paths, &connection) {
        Ok(report) => print_success(
            command_id,
            json!(report),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(artifacts::ArtifactError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
        Err(artifacts::ArtifactError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(error) => print_error(command_id, &CliError::artifacts(error), json_output),
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

fn success_envelope_with_workspace_key(
    command: &'static str,
    data: Value,
    workspace_key: String,
) -> String {
    success_envelope_with_meta(
        command,
        data,
        OutputMeta {
            version: env!("CARGO_PKG_VERSION"),
            workspace_key: Some(workspace_key),
        },
    )
}

fn success_envelope_with_meta(command: &'static str, data: Value, meta: OutputMeta) -> String {
    let envelope = OutputEnvelope {
        ok: true,
        command,
        data: Some(data),
        warnings: Vec::new(),
        errors: Vec::new(),
        meta,
    };

    serialize_envelope(&envelope)
}

fn error_envelope(command: &'static str, error: &CliError) -> String {
    let envelope = OutputEnvelope {
        ok: false,
        command,
        data: None,
        warnings: Vec::new(),
        errors: vec![OutputError {
            code: error.code,
            message: error.message.clone(),
            fix_hint: error.fix_hint.clone(),
        }],
        meta: OutputMeta::default(),
    };

    serialize_envelope(&envelope)
}

fn run_node_create(command_id: &'static str, args: &NodeCreateArgs, json_output: bool) -> ExitCode {
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

    run_node_create_input(command_id, &input, json_output)
}

fn run_remember(command_id: &'static str, args: &RememberArgs, json_output: bool) -> ExitCode {
    let input = match remember_to_new_node(args) {
        Ok(input) => input,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    run_node_create_input(command_id, &input, json_output)
}

fn run_teach_start(command_id: &'static str, args: &TeachStartArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::create_teach_session(
        &connection,
        &storage::NewTeachSession {
            title: args.title.clone(),
            summary: args.summary.clone(),
        },
    ) {
        Ok(session) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(session),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(error) => print_error(command_id, &CliError::teach(error), json_output),
    }
}

fn run_teach_add(command_id: &'static str, args: &TeachPayloadArgs, json_output: bool) -> ExitCode {
    let payload = match parse_teach_payload(&args.payload) {
        Ok(payload) => payload,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::add_teach_material(&connection, args.session_id, &payload) {
        Ok(material) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(material),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(error) => print_error(command_id, &CliError::teach(error), json_output),
    }
}

fn run_teach_propose(
    command_id: &'static str,
    args: &TeachPayloadArgs,
    json_output: bool,
) -> ExitCode {
    let proposal = match parse_teach_proposal(&args.payload) {
        Ok(proposal) => proposal,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::store_teach_proposal(&connection, args.session_id, &proposal) {
        Ok(proposal) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(proposal),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(error) => print_error(command_id, &CliError::teach(error), json_output),
    }
}

fn run_teach_apply(command_id: &'static str, args: &TeachApplyArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::apply_teach_proposal(&connection, args.session_id, args.proposal_id) {
        Ok(report) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(report),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(error) => print_error(command_id, &CliError::teach(error), json_output),
    }
}

fn run_reflect_inventory(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match reflection::inventory_sessions(&connection) {
        Ok(report) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(report),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(error) => print_error(command_id, &CliError::reflection(error), json_output),
    }
}

fn run_reflect_proposal_create(
    command_id: &'static str,
    args: &ReflectProposalCreateArgs,
    json_output: bool,
) -> ExitCode {
    let proposal = match parse_reflect_proposal_file(&args.proposal_file) {
        Ok(proposal) => proposal,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match reflection::store_proposal(&connection, &args.session_id, &proposal) {
        Ok(report) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(report),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(error) => print_error(command_id, &CliError::reflection(error), json_output),
    }
}

fn run_reflect_proposal_apply(
    command_id: &'static str,
    args: &ReflectProposalApplyArgs,
    json_output: bool,
) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match reflection::apply_proposal(&connection, args.proposal_id) {
        Ok(report) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(report),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(error) => print_error(command_id, &CliError::reflection(error), json_output),
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
    serde_json::from_str(payload).map_err(CliError::teach_payload_json)
}

fn parse_teach_proposal(payload: &str) -> Result<storage::TeachProposalInput, CliError> {
    serde_json::from_str(payload).map_err(CliError::teach_payload_json)
}

fn parse_reflect_proposal_file(
    proposal_file: &PathBuf,
) -> Result<reflection::ReflectionProposalInput, CliError> {
    let payload = fs::read_to_string(proposal_file).map_err(CliError::io)?;
    serde_json::from_str(&payload).map_err(CliError::reflection_proposal_json)
}

fn run_node_create_input(
    command_id: &'static str,
    input: &storage::NewNode,
    json_output: bool,
) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::create_node(&connection, input) {
        Ok(node) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(node),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(storage::NodeStorageError::Validation(error)) => {
            print_error(command_id, &CliError::validation(error), json_output)
        }
        Err(storage::NodeStorageError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
    }
}

fn run_init(command_id: &'static str, json_output: bool) -> ExitCode {
    let repo_root = match env::current_dir() {
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
        )
    }
}

fn run_init_with_io<R, W>(
    command_id: &'static str,
    json_output: bool,
    repo_root: &std::path::Path,
    input: &mut R,
    prompt_output: &mut W,
) -> ExitCode
where
    R: BufRead,
    W: Write,
{
    match install::run_install_flow(repo_root, input, prompt_output) {
        Ok(status) => {
            let paths = match storage::resolve_paths() {
                Ok(paths) => paths,
                Err(error) => return print_error(command_id, &CliError::path(error), json_output),
            };
            let workspace_paths =
                match storage::ensure_workspace_dirs(&paths, &status.workspace_key) {
                    Ok(paths) => paths,
                    Err(error) => {
                        return print_error(command_id, &CliError::io(error), json_output)
                    }
                };
            let connection = match storage::open_workspace_db(&workspace_paths) {
                Ok(connection) => connection,
                Err(error) => {
                    return print_error(command_id, &CliError::db_schema(error), json_output);
                }
            };

            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            if json_output {
                println!(
                    "{}",
                    success_envelope_with_workspace_key(
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
                        status.workspace_key,
                    )
                );
            } else {
                println!("AOPMem готов.");
            }

            ExitCode::from(EXIT_SUCCESS)
        }
        Err(install::WorkspaceInitError::Path(error)) => {
            print_error(command_id, &CliError::path(error), json_output)
        }
        Err(install::WorkspaceInitError::WorkspaceKey(error)) => {
            print_error(command_id, &CliError::workspace_key(error), json_output)
        }
        Err(install::WorkspaceInitError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
        Err(install::WorkspaceInitError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(install::WorkspaceInitError::Seed(storage::NodeStorageError::Validation(error))) => {
            print_error(command_id, &CliError::validation(error), json_output)
        }
        Err(install::WorkspaceInitError::Seed(storage::NodeStorageError::Db(error))) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
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

fn run_node_list(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::list_nodes(&connection) {
        Ok(nodes) => print_success(
            command_id,
            json!({ "nodes": nodes }),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_node_update(command_id: &'static str, args: &NodeUpdateArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
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

    match storage::update_node(&connection, &update) {
        Ok(Some(node)) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(node),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Ok(None) => print_error(command_id, &CliError::node_not_found(args.id), json_output),
        Err(storage::NodeStorageError::Validation(error)) => {
            print_error(command_id, &CliError::validation(error), json_output)
        }
        Err(storage::NodeStorageError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
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

fn doctor_success_envelope(report: &verify::DoctorReport) -> String {
    success_envelope_with_workspace_key("doctor", json!(report), report.workspace_key.clone())
}

fn verify_success_envelope(report: &verify::LintReport) -> String {
    success_envelope_with_workspace_key("verify", json!(report), report.workspace_key.clone())
}

fn run_doctor(command_id: &'static str, json_output: bool) -> ExitCode {
    let repo_root = match env::current_dir() {
        Ok(path) => path,
        Err(error) => return print_error(command_id, &CliError::io(error), json_output),
    };

    match verify::run_doctor(&repo_root) {
        Ok(report) => {
            if json_output {
                println!("{}", doctor_success_envelope(&report));
                ExitCode::from(EXIT_SUCCESS)
            } else {
                println!(
                    "doctor: {}\nglobal_dirs: {}\nworkspace: {}\ndb: {}\nschema: {}\nfts: {}\nadapter_block: {}\nartifacts_dirs: {}\ntools_dirs: {}",
                    if report.healthy { "ready" } else { "issues" },
                    report.checks.global_dirs.status.as_str(),
                    report.checks.workspace.status.as_str(),
                    report.checks.db.status.as_str(),
                    report.checks.schema.status.as_str(),
                    report.checks.fts.status.as_str(),
                    report.checks.adapter_block.status.as_str(),
                    report.checks.artifacts_dirs.status.as_str(),
                    report.checks.tools_dirs.status.as_str(),
                );

                ExitCode::from(EXIT_SUCCESS)
            }
        }
        Err(verify::DoctorError::Path(error)) => {
            print_error(command_id, &CliError::path(error), json_output)
        }
        Err(verify::DoctorError::WorkspaceKey(error)) => {
            print_error(command_id, &CliError::workspace_key(error), json_output)
        }
        Err(verify::DoctorError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
    }
}

fn run_verify(command_id: &'static str, json_output: bool) -> ExitCode {
    let repo_root = match env::current_dir() {
        Ok(path) => path,
        Err(error) => return print_error(command_id, &CliError::io(error), json_output),
    };

    match verify::run_lint(&repo_root) {
        Ok(report) => {
            let exit_code = if report.clean {
                EXIT_SUCCESS
            } else {
                EXIT_DRIFT_DETECTED
            };

            if json_output {
                println!("{}", verify_success_envelope(&report));
            } else {
                println!(
                    "verify: {}\nissues: {}\nduplicate_ids: {}\nbroken_links: {}\ndeprecated_active_links: {}\nmissing_source: {}\nmissing_summary: {}\nmissing_gates: {}",
                    if report.clean { "clean" } else { "issues" },
                    report.summary.total,
                    report.summary.duplicate_ids,
                    report.summary.broken_links,
                    report.summary.deprecated_active_links,
                    report.summary.missing_source,
                    report.summary.missing_summary,
                    report.summary.missing_gates,
                );
            }

            ExitCode::from(exit_code)
        }
        Err(verify::LintError::Path(error)) => {
            print_error(command_id, &CliError::path(error), json_output)
        }
        Err(verify::LintError::WorkspaceKey(error)) => {
            print_error(command_id, &CliError::workspace_key(error), json_output)
        }
        Err(verify::LintError::WorkspaceDbMissing(path)) => print_error(
            command_id,
            &CliError::workspace_db_missing(&path),
            json_output,
        ),
        Err(verify::LintError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
        Err(verify::LintError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
    }
}

fn run_link_add(command_id: &'static str, args: &LinkAddArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let input = storage::NewLink {
        source_node_id: args.source_id,
        target_node_id: args.target_id,
        link_type: args.link_type.clone(),
    };

    match storage::create_link(&connection, &input) {
        Ok(link) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(link),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(storage::LinkStorageError::Validation(error)) => {
            print_error(command_id, &CliError::link_validation(error), json_output)
        }
        Err(storage::LinkStorageError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
    }
}

fn run_link_list(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::list_links(&connection) {
        Ok(links) => print_success(
            command_id,
            json!({ "links": links }),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_alias_add(command_id: &'static str, args: &AliasAddArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let input = storage::NewAlias {
        node_id: args.node_id,
        alias: args.alias.clone(),
    };

    match storage::create_alias(&connection, &input) {
        Ok(alias) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(alias),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(storage::MetadataStorageError::Validation(error)) => print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        ),
        Err(storage::MetadataStorageError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
    }
}

fn run_alias_list(
    command_id: &'static str,
    args: &NodeMetadataListArgs,
    json_output: bool,
) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::list_aliases(&connection, args.node_id) {
        Ok(aliases) => print_success(
            command_id,
            json!({ "aliases": aliases }),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_tag_add(command_id: &'static str, args: &TagAddArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let input = storage::NewTag {
        node_id: args.node_id,
        tag: args.tag.clone(),
    };

    match storage::create_tag(&connection, &input) {
        Ok(tag) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(tag),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(storage::MetadataStorageError::Validation(error)) => print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        ),
        Err(storage::MetadataStorageError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
    }
}

fn run_tag_list(
    command_id: &'static str,
    args: &NodeMetadataListArgs,
    json_output: bool,
) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::list_tags(&connection, args.node_id) {
        Ok(tags) => print_success(
            command_id,
            json!({ "tags": tags }),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_source_add(command_id: &'static str, args: &SourceAddArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
    let input = storage::NewSource {
        node_id: args.node_id,
        source_ref: args.source_ref.clone(),
    };

    match storage::create_source(&connection, &input) {
        Ok(source) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(source),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(storage::MetadataStorageError::Validation(error)) => print_error(
            command_id,
            &CliError::metadata_validation(error),
            json_output,
        ),
        Err(storage::MetadataStorageError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
    }
}

fn run_source_list(
    command_id: &'static str,
    args: &NodeMetadataListArgs,
    json_output: bool,
) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::list_sources(&connection, args.node_id) {
        Ok(sources) => print_success(
            command_id,
            json!({ "sources": sources }),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_mcp_list(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::list_mcp_profiles(&connection) {
        Ok(profiles) => print_success(
            command_id,
            json!({ "mcp_profiles": profiles }),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_mcp_add(command_id: &'static str, args: &McpAddArgs, json_output: bool) -> ExitCode {
    let (workspace_key, workspace_paths, connection) = match open_current_workspace_context() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };
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

    match storage::create_mcp_profile(&connection, &input) {
        Ok(profile) => {
            if let Err(exit_code) = write_audit_snapshot(
                command_id,
                workspace_paths.audit_git(),
                &connection,
                json_output,
            ) {
                return exit_code;
            }

            print_success(
                command_id,
                json!(profile),
                workspace_key,
                json_output,
                EXIT_SUCCESS,
            )
        }
        Err(storage::McpProfileStorageError::Validation(error)) => print_error(
            command_id,
            &CliError::mcp_profile_validation(error),
            json_output,
        ),
        Err(storage::McpProfileStorageError::Db(error)) => {
            print_error(command_id, &CliError::db_schema(error), json_output)
        }
    }
}

fn run_mcp_get(command_id: &'static str, args: &McpGetArgs, json_output: bool) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    match storage::get_mcp_profile(&connection, &args.id) {
        Ok(Some(profile)) => print_success(
            command_id,
            json!(profile),
            workspace_key,
            json_output,
            EXIT_SUCCESS,
        ),
        Ok(None) => print_error(
            command_id,
            &CliError::mcp_profile_not_found(&args.id),
            json_output,
        ),
        Err(error) => print_error(command_id, &CliError::db_schema(error), json_output),
    }
}

fn run_recall(command_id: &'static str, json_output: bool) -> ExitCode {
    let (workspace_key, connection) = match open_current_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return print_error(command_id, &error, json_output),
    };

    let nodes = match storage::list_nodes(&connection) {
        Ok(nodes) => nodes,
        Err(error) => return print_error(command_id, &CliError::db_schema(error), json_output),
    };
    let links = match storage::list_links(&connection) {
        Ok(links) => links,
        Err(error) => return print_error(command_id, &CliError::db_schema(error), json_output),
    };

    let mut bundle = recall::build_structured_bundle_with_links(nodes, links);
    if recall::needs_fts_fallback(&bundle) {
        if let Some(query) = recall::derive_fts_fallback_query(&bundle) {
            let results = match storage::search_nodes_fts(&connection, &query, 5) {
                Ok(results) => results,
                Err(error) => {
                    return print_error(command_id, &CliError::db_schema(error), json_output)
                }
            };
            bundle = recall::add_fts_fallback(bundle, results);
        }
    }

    print_success(
        command_id,
        json!(bundle),
        workspace_key,
        json_output,
        EXIT_SUCCESS,
    )
}

fn run_adapter_seed(
    command_id: &'static str,
    args: &AdapterSeedArgs,
    json_output: bool,
) -> ExitCode {
    let instruction_file =
        match resolve_adapter_instruction_file(&args.file, json_output, command_id) {
            Ok(path) => path,
            Err(exit_code) => return exit_code,
        };

    match adapter::seed_instruction_file(&instruction_file) {
        Ok(outcome) => print_success(
            command_id,
            json!({
                "instruction_file": outcome.instruction_file.display().to_string(),
                "file_created": outcome.file_created,
                "block_updated": outcome.block_updated,
            }),
            "adapter-seed".to_string(),
            json_output,
            EXIT_SUCCESS,
        ),
        Err(adapter::SeedError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
        Err(adapter::SeedError::DamagedManagedBlock) => {
            print_error(command_id, &CliError::managed_block(), json_output)
        }
    }
}

fn run_adapter_sync(
    command_id: &'static str,
    args: &AdapterTargetArgs,
    json_output: bool,
) -> ExitCode {
    let instruction_file =
        match resolve_adapter_instruction_file(&args.file, json_output, command_id) {
            Ok(path) => path,
            Err(exit_code) => return exit_code,
        };

    match adapter::sync_instruction_file(&instruction_file) {
        Ok(outcome) => print_success(
            command_id,
            json!({
                "instruction_file": outcome.instruction_file.display().to_string(),
                "file_created": outcome.file_created,
                "block_present": outcome.block_present,
                "block_inserted": outcome.block_inserted,
                "block_updated": outcome.block_updated,
            }),
            "adapter-sync".to_string(),
            json_output,
            EXIT_SUCCESS,
        ),
        Err(adapter::SeedError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
        Err(adapter::SeedError::DamagedManagedBlock) => {
            print_error(command_id, &CliError::managed_block_drift(), json_output)
        }
    }
}

fn run_adapter_status(
    command_id: &'static str,
    args: &AdapterTargetArgs,
    json_output: bool,
) -> ExitCode {
    let instruction_file =
        match resolve_adapter_instruction_file(&args.file, json_output, command_id) {
            Ok(path) => path,
            Err(exit_code) => return exit_code,
        };

    match adapter::instruction_file_status(&instruction_file) {
        Ok(outcome) => print_success(
            command_id,
            json!({
                "instruction_file": outcome.instruction_file.display().to_string(),
                "file_exists": outcome.file_exists,
                "managed_block": outcome.managed_block.as_str(),
            }),
            "adapter-status".to_string(),
            json_output,
            EXIT_SUCCESS,
        ),
        Err(adapter::SeedError::Io(error)) => {
            print_error(command_id, &CliError::io(error), json_output)
        }
        Err(adapter::SeedError::DamagedManagedBlock) => {
            print_error(command_id, &CliError::managed_block_drift(), json_output)
        }
    }
}

fn resolve_adapter_instruction_file(
    file: &Option<PathBuf>,
    json_output: bool,
    command_id: &'static str,
) -> Result<PathBuf, ExitCode> {
    let repo_root = match env::current_dir() {
        Ok(path) => path,
        Err(error) => return Err(print_error(command_id, &CliError::io(error), json_output)),
    };

    Ok(file
        .clone()
        .unwrap_or_else(|| adapter::default_instruction_file(&repo_root)))
}

fn open_current_workspace() -> Result<(String, rusqlite::Connection), CliError> {
    let (key, _workspace_paths, connection) = open_current_workspace_context()?;
    Ok((key, connection))
}

fn open_current_workspace_context(
) -> Result<(String, storage::WorkspacePaths, rusqlite::Connection), CliError> {
    let repo_root = env::current_dir().map_err(CliError::io)?;
    let paths = storage::resolve_paths().map_err(CliError::path)?;
    let key = storage::workspace_key(&repo_root).map_err(CliError::workspace_key)?;

    storage::ensure_global_dirs(&paths).map_err(CliError::io)?;
    let workspace_paths = storage::ensure_workspace_dirs(&paths, &key).map_err(CliError::io)?;
    let connection = storage::open_workspace_db(&workspace_paths).map_err(CliError::db_schema)?;

    Ok((key, workspace_paths, connection))
}

fn write_audit_snapshot(
    command_id: &'static str,
    audit_git_dir: &std::path::Path,
    connection: &rusqlite::Connection,
    json_output: bool,
) -> Result<(), ExitCode> {
    match audit::write_sql_snapshot(audit_git_dir, connection) {
        Ok(_) => Ok(()),
        Err(audit::SnapshotError::Io(error)) => {
            Err(print_error(command_id, &CliError::io(error), json_output))
        }
        Err(audit::SnapshotError::Db(error)) => Err(print_error(
            command_id,
            &CliError::db_schema(error),
            json_output,
        )),
    }
}

fn print_success(
    command_id: &'static str,
    data: Value,
    workspace_key: String,
    json_output: bool,
    exit_code: u8,
) -> ExitCode {
    if json_output {
        println!(
            "{}",
            success_envelope_with_workspace_key(command_id, data, workspace_key)
        );
    } else {
        println!("{data}");
    }

    ExitCode::from(exit_code)
}

fn print_error(command_id: &'static str, error: &CliError, json_output: bool) -> ExitCode {
    if json_output {
        println!("{}", error_envelope(command_id, error));
    } else {
        eprintln!("{}: {}", error.code, error.message);
    }

    ExitCode::from(error.exit_code)
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
            NodeCommand::List => "node_list",
            NodeCommand::Update(_) => "node_update",
        },
        Command::Link { command } => match command {
            LinkCommand::Add(_) => "link_add",
            LinkCommand::List => "link_list",
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
        Command::Recall => "recall",
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
            ToolCommand::List => "tool_list",
            ToolCommand::Get(_) => "tool_get",
            ToolCommand::Run(_) => "tool_run",
            ToolCommand::Validate(_) => "tool_validate",
        },
        Command::Mcp { command } => match command {
            McpCommand::List => "mcp_list",
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

    #[cfg(test)]
    fn not_implemented(command: &'static str) -> Self {
        Self {
            exit_code: EXIT_NOT_IMPLEMENTED,
            code: "NOT_IMPLEMENTED",
            message: format!("command is not implemented yet: {command}"),
            fix_hint: "wait for a later AOPMem stage to implement this command".to_string(),
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
            reflection::ReflectionError::Node(storage::NodeStorageError::Db(error))
            | reflection::ReflectionError::Link(storage::LinkStorageError::Db(error))
            | reflection::ReflectionError::Metadata(storage::MetadataStorageError::Db(error))
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

    fn artifacts(error: artifacts::ArtifactError) -> Self {
        Self {
            exit_code: EXIT_GENERIC_ERROR,
            code: "ARTIFACTS_ERROR",
            message: error.to_string(),
            fix_hint: "check artifact day folders under the workspace artifacts directory"
                .to_string(),
        }
    }

    fn path(error: storage::PathResolveError) -> Self {
        Self {
            exit_code: EXIT_IO_ERROR,
            code: "PATH_ERROR",
            message: error.to_string(),
            fix_hint: "set HOME or AOPMEM_HOME".to_string(),
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
    warnings: Vec<String>,
    errors: Vec<OutputError>,
    meta: OutputMeta,
}

#[derive(Debug, Serialize)]
struct OutputError {
    code: &'static str,
    message: String,
    fix_hint: String,
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
    use std::env;
    use std::fs;
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
        open_current_workspace().expect("workspace should open through CLI helper")
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
        assert_eq!(envelope["meta"]["version"], "0.1.0");
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
        assert_eq!(envelope["meta"]["version"], "0.1.0");
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
        assert_eq!(envelope["meta"]["version"], "0.1.0");
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
        ])
        .expect("tool create-draft should parse");

        assert_eq!(command_id(&create.command), "tool_create_draft");
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
        assert_eq!(manifest.status, tools::DRAFT_TOOL_STATUS);
        assert_eq!(manifest.command.entrypoint, "bin/context-export");
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
    fn tool_run_executes_safe_local_script() {
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
        .expect("tool run record should be readable");

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

        assert_eq!(command_id(&recall.command), "recall");
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
        let exit_code = run_init_with_io(
            command_id(&cli.command),
            cli.json,
            &repo_root,
            &mut reader,
            &mut output,
        );

        assert_ne!(exit_code, ExitCode::from(EXIT_NOT_IMPLEMENTED));

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
        let exit_code = run_init_with_io(
            command_id(&cli.command),
            cli.json,
            &repo_root,
            &mut reader,
            &mut output,
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
            "{\"items\":[{\"op\":\"create_node\",\"node_ref\":\"lesson_1\",\"node_type\":\"lesson\",\"status\":\"draft\",\"title\":\"Rollback check\",\"summary\":\"Always write rollback first\"},{\"op\":\"add_alias\",\"node_ref\":\"lesson_1\",\"alias\":\"rollback-first\"},{\"op\":\"add_tag\",\"node_ref\":\"lesson_1\",\"tag\":\"release\"},{\"op\":\"add_source\",\"node_ref\":\"lesson_1\",\"source_ref\":\"source=user_instruction\"}]}",
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
        assert_eq!(aliases[0].alias, "rollback-first");
        assert_eq!(tags[0].tag, "release");
        assert_eq!(sources[0].source_ref, "source=user_instruction");
        assert!(links.iter().any(|link| link.source_node_id == session.id));
        assert!(snapshot_text.contains("teach_session_v1"));
        assert!(snapshot_text.contains("Rollback check"));

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

        drop(_cwd);
        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
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
}
