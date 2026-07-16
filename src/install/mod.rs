use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use rusqlite::Connection;
use serde::Serialize;

use crate::mutation;
use crate::storage;

const AOPMEM_BINARY_NAME: &str = "aopmem";
const UNDERSTAND_DOCS_DIR: &str = ".understand.docs";
const UNDERSTAND_DOCS_SCHEMA: &str = "SCHEMA.md";
const UNDERSTAND_DOCS_EXCLUDE_ENTRY: &str = "/.understand.docs/";
const UNDERSTAND_PROFILE_ID: &str = "understand-anything";
const UNDERSTAND_PROFILE_NAME: &str = "Understand Anything";
const CODEBASE_MEMORY_PROFILE_ID: &str = "codebase-memory-mcp";
const CODEBASE_MEMORY_PROFILE_NAME: &str = "Codebase Memory MCP";
const MCP_PROFILE_KIND_OPTIONAL: &str = "optional";
const MCP_PROFILE_STATUS_INSTALLED: &str = "installed";
const MCP_PROFILE_STATUS_MISSING: &str = "missing";
const MCP_PROFILE_STATUS_DISABLED: &str = "disabled";
const MCP_PROFILE_STATUS_CONFIGURED_UNVERIFIED: &str = "configured_unverified";
const UNDERSTAND_PROFILE_READ_OPERATIONS: &str = "project_docs";
const UNDERSTAND_PROFILE_WRITE_OPERATIONS: &str = "best_effort_index";
const UNDERSTAND_PROFILE_SIDE_EFFECTS: &str = "local_read";
const CODEBASE_MEMORY_PROFILE_READ_OPERATIONS: &str = "code_navigation";
const CODEBASE_MEMORY_PROFILE_WRITE_OPERATIONS: &str = "none";
const CODEBASE_MEMORY_PROFILE_SIDE_EFFECTS: &str = "local_read";
const MCP_PROFILE_APPROVAL_NONE: &str = "none";
const MCP_PROFILE_CREDENTIALS_NONE: &str = "none";
const UNDERSTAND_PROFILE_NOTES: &str = "best-effort installer profile";
const CODEBASE_MEMORY_PROFILE_NOTES: &str = "best-effort installer profile";
const UNDERSTAND_DOCS_DIRECTORIES: &[&str] = &[
    "index",
    "log",
    "raw",
    "concepts",
    "entities",
    "architecture",
    "domain",
    "adr",
    "module-notes",
    "testing-model",
    "maps",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallCheckStatus {
    Ready,
    Missing,
}

impl InstallCheckStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Missing => "missing",
        }
    }

    fn from_present(present: bool) -> Self {
        if present {
            Self::Ready
        } else {
            Self::Missing
        }
    }

    fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GlobalInstallStatus {
    pub status: InstallCheckStatus,
    pub dirs: InstallCheckStatus,
    pub bin: InstallCheckStatus,
    pub templates: InstallCheckStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceInitStatus {
    pub workspace_key: String,
    pub seeded_nodes_created: usize,
    pub seeded_nodes_existing: usize,
    pub db_created: bool,
    pub semantic_nodes_created: usize,
    pub semantic_nodes_existing: usize,
    pub understand_anything_enabled: bool,
    pub codebase_memory_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_warning: Option<mutation::MutationWarning>,
    #[serde(skip)]
    pub(crate) snapshot_observation: mutation::SnapshotObservation,
}

#[derive(Debug)]
pub enum WorkspaceInitError {
    Path(storage::PathResolveError),
    WorkspaceKey(storage::WorkspaceKeyError),
    WorkspaceResolve(storage::WorkspaceResolveError),
    InvalidUtf8Input,
    SuspiciousMojibakeInput,
    InputTooLarge { max_bytes: usize },
    Io(io::Error),
    Db(rusqlite::Error),
    Seed(storage::NodeStorageError),
}

impl From<storage::PathResolveError> for WorkspaceInitError {
    fn from(error: storage::PathResolveError) -> Self {
        Self::Path(error)
    }
}

impl From<storage::WorkspaceKeyError> for WorkspaceInitError {
    fn from(error: storage::WorkspaceKeyError) -> Self {
        Self::WorkspaceKey(error)
    }
}

impl From<storage::WorkspaceResolveError> for WorkspaceInitError {
    fn from(error: storage::WorkspaceResolveError) -> Self {
        Self::WorkspaceResolve(error)
    }
}

impl From<io::Error> for WorkspaceInitError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for WorkspaceInitError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

impl From<storage::NodeStorageError> for WorkspaceInitError {
    fn from(error: storage::NodeStorageError) -> Self {
        Self::Seed(error)
    }
}

struct SeedNode {
    node_type: &'static str,
    title: &'static str,
    summary: &'static str,
}

struct SemanticSeedNode<'a> {
    node_type: &'static str,
    title: &'static str,
    summary: &'static str,
    body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallAnswers {
    pub understand_anything_enabled: bool,
    pub codebase_memory_enabled: bool,
    pub project_meaning: String,
    pub roles: String,
    pub scope: String,
}

const DEFAULT_SOURCE_REF: &str = "source=user_instruction";
const DEFAULT_TRUST_LEVEL: &str = "high";
const DEFAULT_CONFIDENCE: f64 = 1.0;
const STYLE_NOTE: &str = "Базовый стиль установлен. Его можно изменить позже через AOPMem.";
const UNDERSTAND_ANYTHING_QUESTION: &str =
    "Включаем Understand Anything для локального понимания проекта и .understand.docs?";
const CODEBASE_MEMORY_QUESTION: &str = "Включаем Codebase Memory MCP для навигации по коду?";
const PROJECT_MEANING_QUESTION: &str =
    "Объясни, что это за проект, зачем он нужен и чем мы тут занимаемся.";
const ROLES_QUESTION: &str = "Какая твоя роль в этом проекте и какая роль у агента?";
const SCOPE_QUESTION: &str =
    "Какие части проекта рабочие, какие вспомогательные, какие нельзя трогать?";
const UNDERSTAND_ANYTHING_TITLE: &str = "Understand Anything install choice";
const CODEBASE_MEMORY_TITLE: &str = "Codebase Memory MCP install choice";
const PROJECT_MEANING_TITLE: &str = "Project meaning";
const PROJECT_ROLES_TITLE: &str = "Project roles";
const PROJECT_SCOPE_TITLE: &str = "Project scope boundaries";
const BASE_WORKSPACE_NODES: &[SeedNode] = &[
    SeedNode {
        node_type: "kernel_contract",
        title: "Use AOPMem as canonical operational memory",
        summary: "Use AOPMem CLI for workspace memory operations before raw file digging.",
    },
    SeedNode {
        node_type: "gate",
        title: "Memory writes stay user-triggered",
        summary:
            "Write memory only in explicit user-triggered remember, teach, or reflection flows.",
    },
    SeedNode {
        node_type: "gate",
        title: "Agents do not use direct SQL",
        summary: "Agents use AOPMem CLI commands instead of direct workspace database queries.",
    },
    SeedNode {
        node_type: "preference",
        title: "Default communication style",
        summary: "Base style is installed by default and can be adjusted later through AOPMem.",
    },
];

pub fn global_install_status() -> Result<GlobalInstallStatus, storage::PathResolveError> {
    let paths = storage::resolve_paths()?;
    let dirs = InstallCheckStatus::from_present(
        paths.home().is_dir()
            && paths.bin().is_dir()
            && paths.skills().is_dir()
            && paths.workspaces().is_dir(),
    );
    let bin = InstallCheckStatus::from_present(paths.bin().join(AOPMEM_BINARY_NAME).is_file());
    let templates = InstallCheckStatus::from_present(paths.templates().is_dir());
    let status =
        InstallCheckStatus::from_present(dirs.is_ready() && bin.is_ready() && templates.is_ready());

    Ok(GlobalInstallStatus {
        status,
        dirs,
        bin,
        templates,
    })
}

#[cfg(test)]
pub(crate) fn test_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

pub fn init_workspace(
    repo_root: impl AsRef<Path>,
) -> Result<WorkspaceInitStatus, WorkspaceInitError> {
    let repo_root = storage::resolve_workspace_root_from(repo_root.as_ref())?;
    let paths = storage::resolve_paths()?;
    let workspace_key = storage::resolve_workspace_key(&paths, &repo_root)?;

    storage::ensure_global_dirs(&paths)?;
    let workspace_paths = storage::ensure_workspace_dirs(&paths, &workspace_key)?;
    let db_created = !workspace_paths.db().is_file();
    let outcome = mutation::mutate_workspace(&workspace_paths, |connection, _effects| {
        seed_base_workspace_nodes(connection)
    })
    .map_err(workspace_mutation_error)?;
    let (seeded_nodes_created, seeded_nodes_existing) = outcome.value;

    Ok(WorkspaceInitStatus {
        workspace_key,
        seeded_nodes_created,
        seeded_nodes_existing,
        db_created,
        semantic_nodes_created: 0,
        semantic_nodes_existing: 0,
        understand_anything_enabled: false,
        codebase_memory_enabled: false,
        audit_warning: outcome.warning,
        snapshot_observation: outcome.snapshot_observation,
    })
}

pub fn run_install_flow<R, W>(
    repo_root: impl AsRef<Path>,
    reader: &mut R,
    writer: &mut W,
) -> Result<WorkspaceInitStatus, WorkspaceInitError>
where
    R: BufRead,
    W: Write,
{
    let mut progress = None;
    run_install_flow_with_progress(repo_root, reader, writer, &mut progress)
}

pub fn run_install_flow_with_progress<R, W>(
    repo_root: impl AsRef<Path>,
    reader: &mut R,
    writer: &mut W,
    progress: &mut Option<WorkspaceInitStatus>,
) -> Result<WorkspaceInitStatus, WorkspaceInitError>
where
    R: BufRead,
    W: Write,
{
    *progress = None;
    let repo_root = storage::resolve_workspace_root_from(repo_root.as_ref())?;
    let answers = collect_install_answers(reader, writer)?;
    let paths = storage::resolve_paths()?;
    let workspace_key = storage::resolve_workspace_key(&paths, &repo_root)?;
    storage::ensure_global_dirs(&paths)?;
    let workspace_paths = storage::ensure_workspace_dirs(&paths, &workspace_key)?;
    let db_created = !workspace_paths.db().is_file();

    let outcome = mutation::mutate_workspace(&workspace_paths, |connection, effects| {
        if answers.understand_anything_enabled {
            ensure_understand_docs(&repo_root, effects)?;
        }
        let (seeded_nodes_created, seeded_nodes_existing) = seed_base_workspace_nodes(connection)?;
        register_understand_profile_best_effort(connection, answers.understand_anything_enabled);
        register_codebase_memory_profile_best_effort(connection, answers.codebase_memory_enabled);
        let (semantic_nodes_created, semantic_nodes_existing) =
            seed_install_answers(connection, &answers)?;
        Ok::<_, WorkspaceInitError>((
            seeded_nodes_created,
            seeded_nodes_existing,
            semantic_nodes_created,
            semantic_nodes_existing,
        ))
    })
    .map_err(workspace_mutation_error)?;
    let (
        seeded_nodes_created,
        seeded_nodes_existing,
        semantic_nodes_created,
        semantic_nodes_existing,
    ) = outcome.value;

    let status = WorkspaceInitStatus {
        workspace_key,
        seeded_nodes_created,
        seeded_nodes_existing,
        db_created,
        semantic_nodes_created,
        semantic_nodes_existing,
        understand_anything_enabled: answers.understand_anything_enabled,
        codebase_memory_enabled: answers.codebase_memory_enabled,
        audit_warning: outcome.warning,
        snapshot_observation: outcome.snapshot_observation,
    };
    *progress = Some(status.clone());

    writer.write_all(STYLE_NOTE.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    Ok(status)
}

fn workspace_mutation_error(
    error: mutation::MutationError<WorkspaceInitError>,
) -> WorkspaceInitError {
    match error {
        mutation::MutationError::Operation(error) => error,
        mutation::MutationError::Io(error)
        | mutation::MutationError::FilesystemRollback { source: error } => {
            WorkspaceInitError::Io(error)
        }
        mutation::MutationError::Db(error)
        | mutation::MutationError::Rollback { source: error, .. } => WorkspaceInitError::Db(error),
    }
}

fn register_understand_profile_best_effort(
    connection: &Connection,
    understand_anything_enabled: bool,
) -> bool {
    let status = optional_mcp_status(understand_anything_enabled, None);
    let profile = storage::NewMcpProfile {
        id: UNDERSTAND_PROFILE_ID.to_string(),
        name: UNDERSTAND_PROFILE_NAME.to_string(),
        kind: MCP_PROFILE_KIND_OPTIONAL.to_string(),
        status: status.to_string(),
        read_operations: UNDERSTAND_PROFILE_READ_OPERATIONS.to_string(),
        write_operations: UNDERSTAND_PROFILE_WRITE_OPERATIONS.to_string(),
        side_effects: UNDERSTAND_PROFILE_SIDE_EFFECTS.to_string(),
        approval_requirement: MCP_PROFILE_APPROVAL_NONE.to_string(),
        credentials_source: Some(MCP_PROFILE_CREDENTIALS_NONE.to_string()),
        notes: Some(UNDERSTAND_PROFILE_NOTES.to_string()),
    };

    storage::upsert_mcp_profile(connection, &profile).is_ok()
}

fn register_codebase_memory_profile_best_effort(
    connection: &Connection,
    codebase_memory_enabled: bool,
) -> bool {
    let status = optional_mcp_status(codebase_memory_enabled, None);
    let profile = storage::NewMcpProfile {
        id: CODEBASE_MEMORY_PROFILE_ID.to_string(),
        name: CODEBASE_MEMORY_PROFILE_NAME.to_string(),
        kind: MCP_PROFILE_KIND_OPTIONAL.to_string(),
        status: status.to_string(),
        read_operations: CODEBASE_MEMORY_PROFILE_READ_OPERATIONS.to_string(),
        write_operations: CODEBASE_MEMORY_PROFILE_WRITE_OPERATIONS.to_string(),
        side_effects: CODEBASE_MEMORY_PROFILE_SIDE_EFFECTS.to_string(),
        approval_requirement: MCP_PROFILE_APPROVAL_NONE.to_string(),
        credentials_source: Some(MCP_PROFILE_CREDENTIALS_NONE.to_string()),
        notes: Some(CODEBASE_MEMORY_PROFILE_NOTES.to_string()),
    };

    storage::upsert_mcp_profile(connection, &profile).is_ok()
}

fn optional_mcp_status(enabled: bool, detector_passed: Option<bool>) -> &'static str {
    if !enabled {
        return MCP_PROFILE_STATUS_DISABLED;
    }

    match detector_passed {
        Some(true) => MCP_PROFILE_STATUS_INSTALLED,
        Some(false) => MCP_PROFILE_STATUS_MISSING,
        None => MCP_PROFILE_STATUS_CONFIGURED_UNVERIFIED,
    }
}

fn ensure_understand_docs(
    repo_root: &Path,
    effects: &mut mutation::MutationEffects,
) -> Result<(), WorkspaceInitError> {
    let docs_root = repo_root.join(UNDERSTAND_DOCS_DIR);
    ensure_tracked_directory(&docs_root, effects)?;
    let canonical_repo_root = repo_root.canonicalize()?;
    let canonical_docs_root = docs_root.canonicalize()?;
    if canonical_docs_root.parent() != Some(canonical_repo_root.as_path()) {
        return Err(unsafe_understand_docs_path(&docs_root));
    }
    ensure_understand_docs_schema(&docs_root, effects)?;

    for directory in UNDERSTAND_DOCS_DIRECTORIES {
        let directory = docs_root.join(directory);
        ensure_tracked_directory(&directory, effects)?;
        if directory.canonicalize()?.parent() != Some(canonical_docs_root.as_path()) {
            return Err(unsafe_understand_docs_path(&directory));
        }
    }

    ensure_understand_docs_exclude(repo_root, effects)?;
    Ok(())
}

fn ensure_understand_docs_schema(
    docs_root: &Path,
    effects: &mut mutation::MutationEffects,
) -> Result<(), WorkspaceInitError> {
    let schema_path = docs_root.join(UNDERSTAND_DOCS_SCHEMA);
    match fs::symlink_metadata(&schema_path) {
        Ok(metadata) => {
            if path_is_link_or_reparse_point(&metadata) || !metadata.is_file() {
                return Err(unsafe_understand_docs_path(&schema_path));
            }
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let schema = concat!(
        "# Understand Docs Schema\n\n",
        "Local-only project knowledge workspace.\n\n",
        "Structure:\n",
        "- index/\n",
        "- log/\n",
        "- raw/\n",
        "- concepts/\n",
        "- entities/\n",
        "- architecture/\n",
        "- domain/\n",
        "- adr/\n",
        "- module-notes/\n",
        "- testing-model/\n",
        "- maps/\n",
    );
    effects.register_created_file(schema_path.clone());
    fs::write(schema_path, schema)?;
    Ok(())
}

fn ensure_understand_docs_exclude(
    repo_root: &Path,
    effects: &mut mutation::MutationEffects,
) -> Result<(), WorkspaceInitError> {
    let Some(git_dir) = resolve_git_dir(repo_root)? else {
        return Ok(());
    };
    let exclude_path = git_dir.join("info").join("exclude");
    let existing = match fs::symlink_metadata(&exclude_path) {
        Ok(metadata) => {
            if path_is_link_or_reparse_point(&metadata) || !metadata.is_file() {
                return Err(unsafe_understand_docs_path(&exclude_path));
            }
            Some(fs::read(&exclude_path)?)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let existing_text = match existing.as_deref() {
        Some(bytes) => std::str::from_utf8(bytes).map_err(|error| {
            WorkspaceInitError::Io(io::Error::new(io::ErrorKind::InvalidData, error))
        })?,
        None => "",
    };
    if existing_text
        .lines()
        .any(|line| line.trim() == UNDERSTAND_DOCS_EXCLUDE_ENTRY)
    {
        return Ok(());
    }

    if let Some(parent) = exclude_path.parent() {
        ensure_tracked_directory(parent, effects)?;
    }

    let mut updated = existing_text.to_string();
    match existing {
        Some(bytes) => effects.register_file_restore(exclude_path.clone(), bytes),
        None => effects.register_created_file(exclude_path.clone()),
    }
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(UNDERSTAND_DOCS_EXCLUDE_ENTRY);
    updated.push('\n');
    fs::write(exclude_path, updated)?;
    Ok(())
}

fn ensure_tracked_directory(
    path: &Path,
    effects: &mut mutation::MutationEffects,
) -> Result<(), WorkspaceInitError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if path_is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
                return Err(unsafe_understand_docs_path(path));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
            effects.register_created_empty_directory(path.to_path_buf());
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn unsafe_understand_docs_path(path: &Path) -> WorkspaceInitError {
    WorkspaceInitError::Io(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "understand docs path is not a direct real file or directory: {}",
            path.display()
        ),
    ))
}

fn path_is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
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

fn resolve_git_dir(repo_root: &Path) -> Result<Option<PathBuf>, WorkspaceInitError> {
    let git_path = repo_root.join(".git");
    if git_path.is_dir() {
        return Ok(Some(git_path));
    }
    if !git_path.is_file() {
        return Ok(None);
    }

    let git_dir_ref = fs::read_to_string(&git_path)?;
    let trimmed = git_dir_ref.trim();
    let Some(relative_git_dir) = trimmed.strip_prefix("gitdir:") else {
        return Err(WorkspaceInitError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported .git file format",
        )));
    };

    let git_dir = relative_git_dir.trim();
    if git_dir.is_empty() {
        return Err(WorkspaceInitError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "gitdir path is empty",
        )));
    }

    Ok(Some(repo_root.join(git_dir)))
}

fn seed_base_workspace_nodes(
    connection: &Connection,
) -> Result<(usize, usize), WorkspaceInitError> {
    let mut existing_nodes = storage::list_nodes(connection)?;
    let mut created = 0;
    let mut existing = 0;

    for seed in BASE_WORKSPACE_NODES {
        if has_seed_node(&existing_nodes, seed) {
            existing += 1;
            continue;
        }

        let node = storage::NewNode {
            node_type: seed.node_type.to_string(),
            status: "active".to_string(),
            title: seed.title.to_string(),
            summary: Some(seed.summary.to_string()),
            body: None,
            source_ref: Some(DEFAULT_SOURCE_REF.to_string()),
            confidence: Some(DEFAULT_CONFIDENCE),
            trust_level: Some(DEFAULT_TRUST_LEVEL.to_string()),
        };
        let created_node = storage::create_node(connection, &node)?;
        existing_nodes.push(created_node);
        created += 1;
    }

    Ok((created, existing))
}

fn seed_install_answers(
    connection: &Connection,
    answers: &InstallAnswers,
) -> Result<(usize, usize), WorkspaceInitError> {
    let mut existing_nodes = storage::list_nodes(connection)?;
    let mut created = 0;
    let mut existing = 0;
    let understand_anything_body = if answers.understand_anything_enabled {
        "enabled"
    } else {
        "disabled"
    };
    let codebase_memory_body = if answers.codebase_memory_enabled {
        "enabled"
    } else {
        "disabled"
    };
    let seed_nodes = [
        SemanticSeedNode {
            node_type: "preference",
            title: UNDERSTAND_ANYTHING_TITLE,
            summary: "Installer choice for local project understanding and .understand.docs.",
            body: understand_anything_body,
        },
        SemanticSeedNode {
            node_type: "preference",
            title: CODEBASE_MEMORY_TITLE,
            summary: "Installer choice for code navigation with Codebase Memory MCP.",
            body: codebase_memory_body,
        },
        SemanticSeedNode {
            node_type: "project_profile",
            title: PROJECT_MEANING_TITLE,
            summary: "Why the project exists and what work happens here.",
            body: answers.project_meaning.as_str(),
        },
        SemanticSeedNode {
            node_type: "project_profile",
            title: PROJECT_ROLES_TITLE,
            summary: "User role and agent role in the project.",
            body: answers.roles.as_str(),
        },
        SemanticSeedNode {
            node_type: "project_profile",
            title: PROJECT_SCOPE_TITLE,
            summary: "Working, helper, and protected parts of the project.",
            body: answers.scope.as_str(),
        },
    ];

    for seed in seed_nodes {
        if has_node_with_title(&existing_nodes, seed.node_type, seed.title) {
            existing += 1;
            continue;
        }

        let node = storage::NewNode {
            node_type: seed.node_type.to_string(),
            status: "active".to_string(),
            title: seed.title.to_string(),
            summary: Some(seed.summary.to_string()),
            body: Some(seed.body.to_string()),
            source_ref: Some(DEFAULT_SOURCE_REF.to_string()),
            confidence: Some(DEFAULT_CONFIDENCE),
            trust_level: Some(DEFAULT_TRUST_LEVEL.to_string()),
        };
        let created_node = storage::create_node(connection, &node)?;
        existing_nodes.push(created_node);
        created += 1;
    }

    Ok((created, existing))
}

fn has_seed_node(nodes: &[storage::Node], seed: &SeedNode) -> bool {
    nodes.iter().any(|node| {
        node.node_type == seed.node_type && node.status == "active" && node.title == seed.title
    })
}

fn has_node_with_title(nodes: &[storage::Node], node_type: &str, title: &str) -> bool {
    nodes
        .iter()
        .any(|node| node.node_type == node_type && node.status == "active" && node.title == title)
}

fn collect_install_answers<R, W>(
    reader: &mut R,
    writer: &mut W,
) -> Result<InstallAnswers, WorkspaceInitError>
where
    R: BufRead,
    W: Write,
{
    Ok(InstallAnswers {
        understand_anything_enabled: ask_yes_no(reader, writer, UNDERSTAND_ANYTHING_QUESTION)?,
        codebase_memory_enabled: ask_yes_no(reader, writer, CODEBASE_MEMORY_QUESTION)?,
        project_meaning: ask_text(reader, writer, PROJECT_MEANING_QUESTION)?,
        roles: ask_text(reader, writer, ROLES_QUESTION)?,
        scope: ask_text(reader, writer, SCOPE_QUESTION)?,
    })
}

fn ask_yes_no<R, W>(
    reader: &mut R,
    writer: &mut W,
    question: &str,
) -> Result<bool, WorkspaceInitError>
where
    R: BufRead,
    W: Write,
{
    loop {
        write_question(writer, question)?;
        let answer = read_answer(reader)?;
        match answer.to_ascii_lowercase().as_str() {
            "y" | "yes" | "да" => return Ok(true),
            "n" | "no" | "нет" => return Ok(false),
            _ => {
                writer.write_all("Ответ: yes/no.\n".as_bytes())?;
                writer.flush()?;
            }
        }
    }
}

fn ask_text<R, W>(
    reader: &mut R,
    writer: &mut W,
    question: &str,
) -> Result<String, WorkspaceInitError>
where
    R: BufRead,
    W: Write,
{
    loop {
        write_question(writer, question)?;
        let answer = read_answer(reader)?;
        if is_suspicious_mojibake_input(&answer) {
            return Err(WorkspaceInitError::SuspiciousMojibakeInput);
        }
        if !answer.is_empty() {
            return Ok(answer);
        }

        writer.write_all("Ответ не должен быть пустым.\n".as_bytes())?;
        writer.flush()?;
    }
}

fn write_question<W>(writer: &mut W, question: &str) -> Result<(), WorkspaceInitError>
where
    W: Write,
{
    writer.write_all(question.as_bytes())?;
    writer.write_all(b"\n> ")?;
    writer.flush()?;
    Ok(())
}

fn read_answer<R>(reader: &mut R) -> Result<String, WorkspaceInitError>
where
    R: BufRead,
{
    const MAX_LINE_BYTES: usize = storage::MAX_NODE_BODY_BYTES + 2;
    let mut answer = Vec::new();
    let mut bounded_reader = <&mut R as io::Read>::take(reader, (MAX_LINE_BYTES + 1) as u64);
    let bytes_read = bounded_reader.read_until(b'\n', &mut answer)?;
    if bytes_read == 0 {
        return Err(WorkspaceInitError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "stdin closed during install flow",
        )));
    }
    if answer.len() > MAX_LINE_BYTES {
        return Err(WorkspaceInitError::InputTooLarge {
            max_bytes: storage::MAX_NODE_BODY_BYTES,
        });
    }

    let answer = String::from_utf8(answer).map_err(|_| WorkspaceInitError::InvalidUtf8Input)?;
    let answer = answer.trim().to_string();
    if answer.len() > storage::MAX_NODE_BODY_BYTES {
        return Err(WorkspaceInitError::InputTooLarge {
            max_bytes: storage::MAX_NODE_BODY_BYTES,
        });
    }
    Ok(answer)
}

fn is_suspicious_mojibake_input(answer: &str) -> bool {
    let meaningful_chars = answer.chars().filter(|char| !char.is_whitespace()).count();
    if meaningful_chars == 0 {
        return false;
    }

    let question_marks = answer.chars().filter(|char| *char == '?').count();
    question_marks >= 4 && question_marks * 2 >= meaningful_chars
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    const AOPMEM_HOME_ENV: &str = "AOPMEM_HOME";
    const HOME_ENV: &str = "HOME";

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        env::temp_dir().join(format!("aopmem-stage-025-{name}-{nanos}"))
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
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }

    fn prepare_second_seed_failure(repo_root: &Path) -> storage::WorkspacePaths {
        let paths = storage::resolve_paths().expect("test AOPMem paths should resolve");
        let workspace_key = storage::resolve_workspace_key(&paths, repo_root)
            .expect("workspace key should resolve");
        storage::ensure_global_dirs(&paths).expect("global dirs should create");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, workspace_key)
            .expect("workspace dirs should create");
        mutation::mutate_workspace(&workspace_paths, |_connection, _effects| {
            Ok::<_, rusqlite::Error>(())
        })
        .expect("empty schema should initialize through coordinator");
        let connection = storage::open_workspace_db(&workspace_paths)
            .expect("test DB should open for trigger fixture");
        connection
            .execute_batch(
                "
                CREATE TRIGGER fail_second_seed
                BEFORE INSERT ON nodes
                WHEN (SELECT COUNT(*) FROM nodes) = 1
                BEGIN
                    SELECT RAISE(FAIL, 'forced second seed failure');
                END;
                ",
            )
            .expect("second seed failure trigger should create");
        workspace_paths
    }

    fn tree_snapshot(root: &Path) -> Vec<(String, Option<Vec<u8>>)> {
        fn collect(root: &Path, directory: &Path, rows: &mut Vec<(String, Option<Vec<u8>>)>) {
            let mut entries = fs::read_dir(directory)
                .expect("snapshot directory should list")
                .collect::<Result<Vec<_>, _>>()
                .expect("snapshot entries should read");
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .expect("snapshot path should stay under root")
                    .to_string_lossy()
                    .to_string();
                let metadata =
                    fs::symlink_metadata(&path).expect("snapshot metadata should be readable");
                if metadata.is_dir() {
                    rows.push((relative, None));
                    collect(root, &path, rows);
                } else {
                    rows.push((
                        relative,
                        Some(fs::read(&path).expect("snapshot file should read")),
                    ));
                }
            }
        }

        let mut rows = Vec::new();
        collect(root, root, &mut rows);
        rows
    }

    #[derive(Default)]
    struct FailOnStyleWriter {
        bytes: Vec<u8>,
    }

    impl Write for FailOnStyleWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if buffer == STYLE_NOTE.as_bytes() {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "forced style-note write failure",
                ));
            }
            self.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn global_install_status_reports_missing_when_install_is_absent() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("missing");
        let home = temp_path("home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);

        let status = global_install_status().expect("path resolution should succeed");

        assert_eq!(status.status, InstallCheckStatus::Missing);
        assert_eq!(status.dirs, InstallCheckStatus::Missing);
        assert_eq!(status.bin, InstallCheckStatus::Missing);
        assert_eq!(status.templates, InstallCheckStatus::Missing);
        assert!(!override_home.exists());
        assert!(!home.exists());
    }

    #[test]
    fn oversized_install_answer_is_bounded_before_workspace_creation() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("oversized-answer-home");
        let home = temp_path("oversized-answer-fallback-home");
        let repo_root = temp_path("oversized-answer-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let mut input = vec![b'x'; storage::MAX_NODE_BODY_BYTES + 3];
        input.push(b'\n');
        let mut reader = Cursor::new(input);
        let mut output = Vec::new();

        let error = run_install_flow(&repo_root, &mut reader, &mut output)
            .expect_err("oversized install answer must fail");

        assert!(matches!(
            error,
            WorkspaceInitError::InputTooLarge { max_bytes }
                if max_bytes == storage::MAX_NODE_BODY_BYTES
        ));
        assert!(
            !override_home.exists(),
            "oversized install input must not create AOPMEM_HOME"
        );

        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn global_install_status_reports_ready_when_required_items_exist() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("ready");
        let home = temp_path("home");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);

        fs::create_dir_all(override_home.join("bin")).expect("bin dir should be created");
        fs::create_dir_all(override_home.join("skills")).expect("skills dir should be created");
        fs::create_dir_all(override_home.join("templates"))
            .expect("templates dir should be created");
        fs::create_dir_all(override_home.join("workspaces"))
            .expect("workspaces dir should be created");
        fs::write(
            override_home.join("bin").join(AOPMEM_BINARY_NAME),
            b"binary",
        )
        .expect("binary should be created");

        let status = global_install_status().expect("path resolution should succeed");

        assert_eq!(status.status, InstallCheckStatus::Ready);
        assert_eq!(status.dirs, InstallCheckStatus::Ready);
        assert_eq!(status.bin, InstallCheckStatus::Ready);
        assert_eq!(status.templates, InstallCheckStatus::Ready);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
    }

    #[test]
    fn init_workspace_creates_dirs_db_and_base_seed_nodes() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("init-home");
        let home = temp_path("home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");

        let outcome = init_workspace(&repo_root).expect("workspace init should succeed");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &outcome.workspace_key)
            .expect("workspace dirs should resolve");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        let nodes = storage::list_nodes(&connection).expect("seeded nodes should list");

        assert!(paths.home().is_dir());
        assert!(paths.workspaces().is_dir());
        assert!(workspace_paths.root().is_dir());
        assert!(workspace_paths.db().is_file());
        assert_eq!(outcome.seeded_nodes_created, BASE_WORKSPACE_NODES.len());
        assert_eq!(outcome.seeded_nodes_existing, 0);
        assert!(outcome.db_created);
        assert_eq!(outcome.semantic_nodes_created, 0);
        assert_eq!(outcome.semantic_nodes_existing, 0);
        assert!(!outcome.understand_anything_enabled);
        assert!(!outcome.codebase_memory_enabled);
        assert_eq!(nodes.len(), BASE_WORKSPACE_NODES.len());
        assert!(has_seed_node(&nodes, &BASE_WORKSPACE_NODES[0]));
        assert!(has_seed_node(&nodes, &BASE_WORKSPACE_NODES[1]));
        assert!(has_seed_node(&nodes, &BASE_WORKSPACE_NODES[2]));
        assert!(has_seed_node(&nodes, &BASE_WORKSPACE_NODES[3]));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn init_workspace_is_idempotent() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("idempotent-home");
        let home = temp_path("home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");

        let first = init_workspace(&repo_root).expect("first init should succeed");
        let second = init_workspace(&repo_root).expect("second init should succeed");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &first.workspace_key)
            .expect("workspace dirs should resolve");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        let nodes = storage::list_nodes(&connection).expect("seeded nodes should list");

        assert_eq!(first.seeded_nodes_created, BASE_WORKSPACE_NODES.len());
        assert_eq!(first.seeded_nodes_existing, 0);
        assert!(first.db_created);
        assert_eq!(second.workspace_key, first.workspace_key);
        assert_eq!(second.seeded_nodes_created, 0);
        assert_eq!(second.seeded_nodes_existing, BASE_WORKSPACE_NODES.len());
        assert!(!second.db_created);
        assert_eq!(second.semantic_nodes_created, 0);
        assert_eq!(second.semantic_nodes_existing, 0);
        assert_eq!(nodes.len(), BASE_WORKSPACE_NODES.len());

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn run_install_flow_collects_answers_and_seeds_semantic_nodes() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("flow-home");
        let home = temp_path("home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        fs::create_dir_all(repo_root.join(".git").join("info"))
            .expect("git info dir should be created");
        let input = b"yes\nyes\nProject meaning\nUser and agent roles\nCore and no-touch areas\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();

        let outcome =
            run_install_flow(&repo_root, &mut reader, &mut output).expect("flow should succeed");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &outcome.workspace_key)
            .expect("workspace dirs should resolve");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        let nodes = storage::list_nodes(&connection).expect("seeded nodes should list");
        let understand_profile = storage::get_mcp_profile(&connection, UNDERSTAND_PROFILE_ID)
            .expect("MCP profile get should pass")
            .expect("Understand profile should exist");
        let codebase_memory_profile =
            storage::get_mcp_profile(&connection, CODEBASE_MEMORY_PROFILE_ID)
                .expect("MCP profile get should pass")
                .expect("Codebase Memory profile should exist");
        let rendered = String::from_utf8(output).expect("prompt output should be valid utf-8");
        let docs_root = repo_root.join(UNDERSTAND_DOCS_DIR);
        let exclude_path = repo_root.join(".git").join("info").join("exclude");

        assert!(rendered.contains(UNDERSTAND_ANYTHING_QUESTION));
        assert!(rendered.contains(CODEBASE_MEMORY_QUESTION));
        assert!(rendered.contains(PROJECT_MEANING_QUESTION));
        assert!(rendered.contains(ROLES_QUESTION));
        assert!(rendered.contains(SCOPE_QUESTION));
        assert!(rendered.contains(STYLE_NOTE));
        assert_eq!(outcome.semantic_nodes_created, 5);
        assert_eq!(outcome.semantic_nodes_existing, 0);
        assert!(outcome.understand_anything_enabled);
        assert!(outcome.codebase_memory_enabled);
        assert_eq!(
            understand_profile.status,
            MCP_PROFILE_STATUS_CONFIGURED_UNVERIFIED
        );
        assert_eq!(
            codebase_memory_profile.status,
            MCP_PROFILE_STATUS_CONFIGURED_UNVERIFIED
        );
        assert!(docs_root.is_dir());
        assert!(docs_root.join(UNDERSTAND_DOCS_SCHEMA).is_file());
        for directory in UNDERSTAND_DOCS_DIRECTORIES {
            assert!(docs_root.join(directory).is_dir());
        }
        assert!(fs::read_to_string(exclude_path)
            .expect("exclude file should exist")
            .contains(UNDERSTAND_DOCS_EXCLUDE_ENTRY));
        assert!(has_node_with_title(
            &nodes,
            "preference",
            UNDERSTAND_ANYTHING_TITLE
        ));
        assert!(has_node_with_title(
            &nodes,
            "preference",
            CODEBASE_MEMORY_TITLE
        ));
        assert!(has_node_with_title(
            &nodes,
            "project_profile",
            PROJECT_MEANING_TITLE
        ));
        assert!(has_node_with_title(
            &nodes,
            "project_profile",
            PROJECT_ROLES_TITLE
        ));
        assert!(has_node_with_title(
            &nodes,
            "project_profile",
            PROJECT_SCOPE_TITLE
        ));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn install_progress_retains_committed_workspace_when_style_note_write_fails() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("style-note-failure-home");
        let home = temp_path("style-note-failure-user-home");
        let repo_root = temp_path("style-note-failure-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        let input = b"no\nno\nProject meaning\nUser and agent roles\nCore scope\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut writer = FailOnStyleWriter::default();
        let mut progress = None;

        let error =
            run_install_flow_with_progress(&repo_root, &mut reader, &mut writer, &mut progress)
                .expect_err("style-note write should fail after commit");
        let status = progress.expect("committed workspace status must survive output failure");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::workspace_paths_for_key(&paths, &status.workspace_key);
        let connection = storage::open_workspace_db_read_only(&workspace_paths)
            .expect("committed workspace DB should remain readable");
        let node_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .expect("committed nodes should count");

        assert!(matches!(error, WorkspaceInitError::Io(_)));
        assert!(node_count > 0);
        assert_eq!(status.seeded_nodes_created, BASE_WORKSPACE_NODES.len());
        assert_eq!(status.semantic_nodes_created, 5);

        drop(connection);
        fs::remove_dir_all(override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn failed_second_seed_rolls_back_new_docs_exclude_and_database_rows() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("docs-new-rollback-home");
        let home = temp_path("docs-new-rollback-user-home");
        let repo_root = temp_path("docs-new-rollback-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let exclude_path = repo_root.join(".git").join("info").join("exclude");
        let original_exclude = b"# user rule\r\n/local-only-without-newline".to_vec();
        fs::create_dir_all(exclude_path.parent().expect("exclude parent should exist"))
            .expect("git info should create");
        fs::write(&exclude_path, &original_exclude).expect("exclude fixture should write");
        let workspace_paths = prepare_second_seed_failure(&repo_root);
        let input = b"yes\nno\nProject meaning\nUser and agent roles\nCore and no-touch areas\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();

        let error = run_install_flow(&repo_root, &mut reader, &mut output)
            .expect_err("forced second seed must fail install");

        assert!(matches!(error, WorkspaceInitError::Seed(_)));
        let connection = storage::open_workspace_db_read_only(&workspace_paths)
            .expect("rolled-back DB should open");
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM nodes;", [], |row| row.get(0))
            .expect("node count should read");
        assert_eq!(rows, 0, "first seed insert must roll back with second");
        assert!(!repo_root.join(UNDERSTAND_DOCS_DIR).exists());
        assert_eq!(
            fs::read(&exclude_path).expect("exclude should read"),
            original_exclude
        );
        assert!(
            !crate::audit::has_pending_snapshot(workspace_paths.audit_git())
                .expect("pending marker should read")
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(&repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn failed_seed_preserves_preexisting_docs_tree_and_exclude_exactly() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("docs-existing-rollback-home");
        let home = temp_path("docs-existing-rollback-user-home");
        let repo_root = temp_path("docs-existing-rollback-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let docs_root = repo_root.join(UNDERSTAND_DOCS_DIR);
        let exclude_path = repo_root.join(".git").join("info").join("exclude");
        let original_exclude = b"# exact bytes\r\nkeep-me".to_vec();
        fs::create_dir_all(docs_root.join("raw")).expect("existing raw dir should create");
        fs::create_dir_all(docs_root.join("index")).expect("existing index dir should create");
        fs::write(
            docs_root.join(UNDERSTAND_DOCS_SCHEMA),
            b"user schema\0bytes",
        )
        .expect("existing schema should write");
        fs::write(docs_root.join("raw").join("sentinel.bin"), b"raw\0sentinel")
            .expect("raw sentinel should write");
        fs::write(docs_root.join("custom.bin"), b"custom\xffbytes")
            .expect("custom sentinel should write");
        fs::create_dir_all(exclude_path.parent().expect("exclude parent should exist"))
            .expect("git info should create");
        fs::write(&exclude_path, &original_exclude).expect("exclude fixture should write");
        let original_docs = tree_snapshot(&docs_root);
        let workspace_paths = prepare_second_seed_failure(&repo_root);
        let input = b"yes\nno\nProject meaning\nUser and agent roles\nCore and no-touch areas\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();

        run_install_flow(&repo_root, &mut reader, &mut output)
            .expect_err("forced second seed must fail install");

        assert_eq!(tree_snapshot(&docs_root), original_docs);
        assert_eq!(
            fs::read(&exclude_path).expect("exclude should read"),
            original_exclude
        );
        let connection = storage::open_workspace_db_read_only(&workspace_paths)
            .expect("rolled-back DB should open");
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM nodes;", [], |row| row.get(0))
            .expect("node count should read");
        assert_eq!(rows, 0);
        assert!(
            !crate::audit::has_pending_snapshot(workspace_paths.audit_git())
                .expect("pending marker should read")
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(&repo_root).expect("temp repo root should remove");
    }

    #[test]
    fn partial_docs_setup_failure_removes_only_new_entries() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("docs-partial-rollback-home");
        let home = temp_path("docs-partial-rollback-user-home");
        let repo_root = temp_path("docs-partial-rollback-repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        let docs_root = repo_root.join(UNDERSTAND_DOCS_DIR);
        fs::create_dir_all(&docs_root).expect("existing docs root should create");
        fs::write(docs_root.join("raw"), b"user blocker must survive")
            .expect("blocking user file should write");
        let original_docs = tree_snapshot(&docs_root);
        let workspace_paths = prepare_second_seed_failure(&repo_root);
        let input = b"yes\nno\nProject meaning\nUser and agent roles\nCore and no-touch areas\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();

        let error = run_install_flow(&repo_root, &mut reader, &mut output)
            .expect_err("non-directory docs entry must fail safely");

        assert!(matches!(error, WorkspaceInitError::Io(_)));
        assert_eq!(tree_snapshot(&docs_root), original_docs);
        assert!(
            !crate::audit::has_pending_snapshot(workspace_paths.audit_git())
                .expect("pending marker should read")
        );
        let connection = storage::open_workspace_db_read_only(&workspace_paths)
            .expect("rolled-back DB should open");
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM nodes;", [], |row| row.get(0))
            .expect("node count should read");
        assert_eq!(rows, 0);

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(&repo_root).expect("temp repo root should remove");
    }

    #[cfg(unix)]
    #[test]
    fn understand_docs_symlink_escape_is_rejected_before_repo_writes() {
        use std::os::unix::fs::symlink;

        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("docs-symlink-home");
        let home = temp_path("docs-symlink-user-home");
        let repo_root = temp_path("docs-symlink-repo");
        let outside = temp_path("docs-symlink-outside");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should create");
        fs::create_dir_all(&outside).expect("outside dir should create");
        fs::write(outside.join("sentinel.bin"), b"outside must stay exact")
            .expect("outside sentinel should write");
        let original_outside = tree_snapshot(&outside);
        symlink(&outside, repo_root.join(UNDERSTAND_DOCS_DIR))
            .expect("docs symlink fixture should create");
        let workspace_paths = prepare_second_seed_failure(&repo_root);
        let input = b"yes\nno\nProject meaning\nUser and agent roles\nCore and no-touch areas\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();

        let error = run_install_flow(&repo_root, &mut reader, &mut output)
            .expect_err("docs symlink must be rejected");

        assert!(matches!(error, WorkspaceInitError::Io(_)));
        assert_eq!(tree_snapshot(&outside), original_outside);
        assert!(fs::symlink_metadata(repo_root.join(UNDERSTAND_DOCS_DIR))
            .expect("docs symlink should remain")
            .file_type()
            .is_symlink());
        assert!(
            !crate::audit::has_pending_snapshot(workspace_paths.audit_git())
                .expect("pending marker should read")
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should remove");
        fs::remove_dir_all(&repo_root).expect("temp repo root should remove");
        fs::remove_dir_all(&outside).expect("outside dir should remove");
    }

    #[test]
    fn run_install_flow_stores_valid_cyrillic_answers() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("flow-cyrillic-home");
        let home = temp_path("home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let input = concat!(
            "no\n",
            "no\n",
            "Тестовый Windows workspace для AOPMem rc3.\n",
            "Пользователь проверяет установку; агент ведет operational memory.\n",
            "Вся папка рабочая; ничего запрещенного нет.\n",
        );
        let mut reader = Cursor::new(input.as_bytes());
        let mut output = Vec::new();

        let outcome =
            run_install_flow(&repo_root, &mut reader, &mut output).expect("flow should succeed");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &outcome.workspace_key)
            .expect("workspace dirs should resolve");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        let nodes = storage::list_nodes(&connection).expect("seeded nodes should list");
        let profile = nodes
            .iter()
            .find(|node| node.node_type == "project_profile" && node.title == PROJECT_MEANING_TITLE)
            .expect("project profile should be stored");

        assert_eq!(
            profile.body.as_deref(),
            Some("Тестовый Windows workspace для AOPMem rc3.")
        );
        assert!(!profile.body.as_deref().unwrap_or_default().contains("????"));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn run_install_flow_rejects_invalid_utf8_input() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("flow-invalid-utf8-home");
        let home = temp_path("home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let input = b"no\nno\n\xff\xff\nRoles\nScope\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();

        let error = run_install_flow(&repo_root, &mut reader, &mut output)
            .expect_err("invalid UTF-8 should fail");

        assert!(matches!(error, WorkspaceInitError::InvalidUtf8Input));
        assert!(!override_home.exists());

        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn run_install_flow_rejects_mojibake_question_marks() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("flow-mojibake-home");
        let home = temp_path("home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        let input = b"no\nno\n????\nRoles\nScope\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();

        let error = run_install_flow(&repo_root, &mut reader, &mut output)
            .expect_err("mojibake-like input should fail");

        assert!(matches!(error, WorkspaceInitError::SuspiciousMojibakeInput));
        assert!(!override_home.exists());

        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn optional_mcp_status_model_matches_detector_state() {
        assert_eq!(
            optional_mcp_status(false, None),
            MCP_PROFILE_STATUS_DISABLED
        );
        assert_eq!(
            optional_mcp_status(true, Some(false)),
            MCP_PROFILE_STATUS_MISSING
        );
        assert_eq!(
            optional_mcp_status(true, None),
            MCP_PROFILE_STATUS_CONFIGURED_UNVERIFIED
        );
        assert_eq!(
            optional_mcp_status(true, Some(true)),
            MCP_PROFILE_STATUS_INSTALLED
        );
    }

    #[test]
    fn run_install_flow_is_idempotent_for_semantic_nodes() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("flow-idempotent-home");
        let home = temp_path("home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        fs::create_dir_all(repo_root.join(".git").join("info"))
            .expect("git info dir should be created");
        fs::write(repo_root.join(".git").join("info").join("exclude"), b"")
            .expect("exclude file should be created");

        let first_input =
            b"yes\nyes\nProject meaning\nUser and agent roles\nCore and no-touch areas\n";
        let second_input = b"no\nno\nAnother meaning\nDifferent roles\nDifferent scope\n";
        let mut first_reader = Cursor::new(first_input.as_slice());
        let mut second_reader = Cursor::new(second_input.as_slice());
        let mut first_output = Vec::new();
        let mut second_output = Vec::new();

        let first = run_install_flow(&repo_root, &mut first_reader, &mut first_output)
            .expect("first flow should succeed");
        let second = run_install_flow(&repo_root, &mut second_reader, &mut second_output)
            .expect("second flow should succeed");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &first.workspace_key)
            .expect("workspace dirs should resolve");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        let codebase_memory_profile =
            storage::get_mcp_profile(&connection, CODEBASE_MEMORY_PROFILE_ID)
                .expect("MCP profile get should pass")
                .expect("Codebase Memory profile should exist");
        let exclude = fs::read_to_string(repo_root.join(".git").join("info").join("exclude"))
            .expect("exclude file should exist");

        assert_eq!(first.semantic_nodes_created, 5);
        assert_eq!(first.semantic_nodes_existing, 0);
        assert_eq!(second.semantic_nodes_created, 0);
        assert_eq!(second.semantic_nodes_existing, 5);
        assert!(first.codebase_memory_enabled);
        assert!(!second.codebase_memory_enabled);
        assert_eq!(codebase_memory_profile.status, MCP_PROFILE_STATUS_DISABLED);
        assert_eq!(
            exclude
                .lines()
                .filter(|line| line.trim() == UNDERSTAND_DOCS_EXCLUDE_ENTRY)
                .count(),
            1
        );

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn run_install_flow_skips_understand_docs_when_understand_is_disabled() {
        let _lock = test_env_lock()
            .lock()
            .expect("env lock should not be poisoned");
        let override_home = temp_path("flow-no-understand-home");
        let home = temp_path("home");
        let repo_root = temp_path("repo");
        let _aopmem_home = EnvGuard::set(AOPMEM_HOME_ENV, &override_home);
        let _home = EnvGuard::set(HOME_ENV, &home);
        fs::create_dir_all(&repo_root).expect("repo root should be created");
        fs::create_dir_all(repo_root.join(".git").join("info"))
            .expect("git info dir should be created");
        fs::write(repo_root.join(".git").join("info").join("exclude"), b"")
            .expect("exclude file should be created");
        let input = b"no\nno\nProject meaning\nUser and agent roles\nCore and no-touch areas\n";
        let mut reader = Cursor::new(input.as_slice());
        let mut output = Vec::new();

        let outcome =
            run_install_flow(&repo_root, &mut reader, &mut output).expect("flow should succeed");
        let paths = storage::resolve_paths().expect("paths should resolve");
        let workspace_paths = storage::ensure_workspace_dirs(&paths, &outcome.workspace_key)
            .expect("workspace dirs should resolve");
        let connection =
            storage::open_workspace_db(&workspace_paths).expect("workspace DB should open");
        let understand_profile = storage::get_mcp_profile(&connection, UNDERSTAND_PROFILE_ID)
            .expect("MCP profile get should pass")
            .expect("Understand profile should exist");
        let codebase_memory_profile =
            storage::get_mcp_profile(&connection, CODEBASE_MEMORY_PROFILE_ID)
                .expect("MCP profile get should pass")
                .expect("Codebase Memory profile should exist");
        let exclude = fs::read_to_string(repo_root.join(".git").join("info").join("exclude"))
            .expect("exclude file should exist");

        assert!(!outcome.understand_anything_enabled);
        assert!(!outcome.codebase_memory_enabled);
        assert_eq!(understand_profile.status, MCP_PROFILE_STATUS_DISABLED);
        assert_eq!(codebase_memory_profile.status, MCP_PROFILE_STATUS_DISABLED);
        assert!(!repo_root.join(UNDERSTAND_DOCS_DIR).exists());
        assert!(!exclude
            .lines()
            .any(|line| line.trim() == UNDERSTAND_DOCS_EXCLUDE_ENTRY));

        fs::remove_dir_all(&override_home).expect("temp AOPMEM_HOME should be removed");
        fs::remove_dir_all(&repo_root).expect("temp repo root should be removed");
    }

    #[test]
    fn register_understand_profile_best_effort_does_not_fail_on_storage_error() {
        let connection =
            Connection::open_in_memory().expect("in-memory DB should open for best-effort test");

        let stored = register_understand_profile_best_effort(&connection, true);

        assert!(!stored);
    }

    #[test]
    fn register_codebase_memory_profile_best_effort_does_not_fail_on_storage_error() {
        let connection =
            Connection::open_in_memory().expect("in-memory DB should open for best-effort test");

        let stored = register_codebase_memory_profile_best_effort(&connection, true);

        assert!(!stored);
    }
}
