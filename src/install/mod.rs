use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use rusqlite::Connection;
use serde::Serialize;

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
}

#[derive(Debug)]
pub enum WorkspaceInitError {
    Path(storage::PathResolveError),
    WorkspaceKey(storage::WorkspaceKeyError),
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
    let repo_root = repo_root.as_ref();
    let paths = storage::resolve_paths()?;
    let workspace_key = storage::workspace_key(repo_root)?;

    storage::ensure_global_dirs(&paths)?;
    let workspace_paths = storage::ensure_workspace_dirs(&paths, &workspace_key)?;
    let db_created = !workspace_paths.db().is_file();
    let connection = storage::open_workspace_db(&workspace_paths)?;
    let (seeded_nodes_created, seeded_nodes_existing) = seed_base_workspace_nodes(&connection)?;

    Ok(WorkspaceInitStatus {
        workspace_key,
        seeded_nodes_created,
        seeded_nodes_existing,
        db_created,
        semantic_nodes_created: 0,
        semantic_nodes_existing: 0,
        understand_anything_enabled: false,
        codebase_memory_enabled: false,
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
    let repo_root = repo_root.as_ref();
    let mut status = init_workspace(repo_root)?;
    let answers = collect_install_answers(reader, writer)?;
    let paths = storage::resolve_paths()?;
    let workspace_paths = storage::ensure_workspace_dirs(&paths, &status.workspace_key)?;
    let connection = storage::open_workspace_db(&workspace_paths)?;

    if answers.understand_anything_enabled {
        ensure_understand_docs(repo_root)?;
    }

    register_understand_profile_best_effort(&connection, answers.understand_anything_enabled);
    register_codebase_memory_profile_best_effort(&connection, answers.codebase_memory_enabled);

    let (semantic_nodes_created, semantic_nodes_existing) =
        seed_install_answers(&connection, &answers)?;

    writer.write_all(STYLE_NOTE.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    status.semantic_nodes_created = semantic_nodes_created;
    status.semantic_nodes_existing = semantic_nodes_existing;
    status.understand_anything_enabled = answers.understand_anything_enabled;
    status.codebase_memory_enabled = answers.codebase_memory_enabled;

    Ok(status)
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

fn ensure_understand_docs(repo_root: &Path) -> Result<(), WorkspaceInitError> {
    let docs_root = repo_root.join(UNDERSTAND_DOCS_DIR);
    fs::create_dir_all(&docs_root)?;
    ensure_understand_docs_schema(&docs_root)?;

    for directory in UNDERSTAND_DOCS_DIRECTORIES {
        fs::create_dir_all(docs_root.join(directory))?;
    }

    ensure_understand_docs_exclude(repo_root)?;
    Ok(())
}

fn ensure_understand_docs_schema(docs_root: &Path) -> Result<(), WorkspaceInitError> {
    let schema_path = docs_root.join(UNDERSTAND_DOCS_SCHEMA);
    if schema_path.is_file() {
        return Ok(());
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
    fs::write(schema_path, schema)?;
    Ok(())
}

fn ensure_understand_docs_exclude(repo_root: &Path) -> Result<(), WorkspaceInitError> {
    let Some(git_dir) = resolve_git_dir(repo_root)? else {
        return Ok(());
    };
    let exclude_path = git_dir.join("info").join("exclude");
    let existing = read_text_if_exists(&exclude_path)?;
    if existing
        .lines()
        .any(|line| line.trim() == UNDERSTAND_DOCS_EXCLUDE_ENTRY)
    {
        return Ok(());
    }

    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(UNDERSTAND_DOCS_EXCLUDE_ENTRY);
    updated.push('\n');
    fs::write(exclude_path, updated)?;
    Ok(())
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

fn read_text_if_exists(path: &Path) -> Result<String, WorkspaceInitError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(WorkspaceInitError::Io(error)),
    }
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
    let mut answer = String::new();
    let bytes_read = reader.read_line(&mut answer)?;
    if bytes_read == 0 {
        return Err(WorkspaceInitError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "stdin closed during install flow",
        )));
    }

    Ok(answer.trim().to_string())
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
        assert_eq!(first.codebase_memory_enabled, true);
        assert_eq!(second.codebase_memory_enabled, false);
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
