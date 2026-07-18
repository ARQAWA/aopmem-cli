use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const BEGIN_MARKER: &str = "<!-- AOPMEM:BEGIN managed block -->";
pub const END_MARKER: &str = "<!-- AOPMEM:END managed block -->";
const MANAGED_BLOCK_TEMPLATE: &str =
    include_str!("../../templates/managed-block/AGENTS.managed-block.md");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedOutcome {
    pub instruction_file: PathBuf,
    pub file_created: bool,
    pub block_updated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub instruction_file: PathBuf,
    pub file_created: bool,
    pub block_present: bool,
    pub block_inserted: bool,
    pub block_updated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedInstructionSync {
    pub bytes: Vec<u8>,
    pub file_created: bool,
    pub block_present: bool,
    pub block_inserted: bool,
    pub block_updated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedBlockStatus {
    Missing,
    InSync,
    Drifted,
}

impl ManagedBlockStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::InSync => "in_sync",
            Self::Drifted => "drifted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusOutcome {
    pub instruction_file: PathBuf,
    pub file_exists: bool,
    pub managed_block: ManagedBlockStatus,
}

#[derive(Debug)]
pub enum SeedError {
    Io(io::Error),
    DamagedManagedBlock,
}

impl fmt::Display for SeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::DamagedManagedBlock => {
                write!(formatter, "managed block markers are damaged or duplicated")
            }
        }
    }
}

impl std::error::Error for SeedError {}

impl From<io::Error> for SeedError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn default_instruction_file(repo_root: &Path) -> PathBuf {
    repo_root.join("AGENTS.md")
}

pub fn seed_instruction_file(path: &Path) -> Result<SeedOutcome, SeedError> {
    let outcome = sync_instruction_file(path)?;

    Ok(SeedOutcome {
        instruction_file: outcome.instruction_file,
        file_created: outcome.file_created,
        block_updated: outcome.block_updated,
    })
}

pub fn sync_instruction_file(path: &Path) -> Result<SyncOutcome, SeedError> {
    let existing = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let prepared = prepare_instruction_sync(existing.as_deref())?;

    if existing.as_deref() != Some(prepared.bytes.as_slice()) {
        fs::write(path, &prepared.bytes)?;
    }

    Ok(SyncOutcome {
        instruction_file: path.to_path_buf(),
        file_created: prepared.file_created,
        block_present: prepared.block_present,
        block_inserted: prepared.block_inserted,
        block_updated: prepared.block_updated,
    })
}

pub(crate) fn prepare_instruction_sync(
    existing: Option<&[u8]>,
) -> Result<PreparedInstructionSync, SeedError> {
    let file_created = existing.is_none();
    let existing = match existing {
        Some(bytes) => std::str::from_utf8(bytes).map_err(|error| {
            SeedError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("instruction file is not UTF-8: {error}"),
            ))
        })?,
        None => "",
    };
    let sync = sync_content(existing)?;
    Ok(PreparedInstructionSync {
        bytes: sync.next.into_bytes(),
        file_created,
        block_present: sync.status != ManagedBlockStatus::Missing,
        block_inserted: sync.status == ManagedBlockStatus::Missing,
        block_updated: sync.status == ManagedBlockStatus::Drifted,
    })
}

pub fn instruction_file_status(path: &Path) -> Result<StatusOutcome, SeedError> {
    let file_exists = path.exists();
    let existing = if file_exists {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    let status = inspect_content(&existing)?;

    Ok(StatusOutcome {
        instruction_file: path.to_path_buf(),
        file_exists,
        managed_block: status,
    })
}

#[cfg(test)]
fn seed_content(existing: &str) -> Result<(String, bool), SeedError> {
    let sync = sync_content(existing)?;
    Ok((sync.next, sync.status == ManagedBlockStatus::Drifted))
}

fn append_managed_block(existing: &str, block: &str) -> String {
    if existing.is_empty() {
        return block.to_string();
    }

    let mut next = String::with_capacity(existing.len() + block.len() + 2);
    next.push_str(existing);

    if !existing.ends_with('\n') {
        next.push('\n');
    }
    next.push('\n');
    next.push_str(block);

    next
}

fn marker_positions(haystack: &str, marker: &str) -> Vec<usize> {
    haystack
        .match_indices(marker)
        .map(|(index, _)| index)
        .collect()
}

fn inspect_content(existing: &str) -> Result<ManagedBlockStatus, SeedError> {
    let block = managed_block_text();
    let begin_positions = marker_positions(existing, BEGIN_MARKER);
    let end_positions = marker_positions(existing, END_MARKER);

    match (begin_positions.as_slice(), end_positions.as_slice()) {
        ([], []) => Ok(ManagedBlockStatus::Missing),
        ([begin], [end]) if begin < end => {
            let end_index = end + END_MARKER.len();
            let existing_block = &existing[*begin..end_index];

            if existing_block == block {
                Ok(ManagedBlockStatus::InSync)
            } else {
                Ok(ManagedBlockStatus::Drifted)
            }
        }
        _ => Err(SeedError::DamagedManagedBlock),
    }
}

fn sync_content(existing: &str) -> Result<ContentSync, SeedError> {
    let status = inspect_content(existing)?;

    let next = match status {
        ManagedBlockStatus::Missing => append_managed_block(existing, managed_block()),
        ManagedBlockStatus::InSync => existing.to_string(),
        ManagedBlockStatus::Drifted => replace_managed_block(existing),
    };

    Ok(ContentSync { next, status })
}

fn replace_managed_block(existing: &str) -> String {
    let begin = existing
        .find(BEGIN_MARKER)
        .expect("replace_managed_block requires a managed block");
    let end = existing
        .find(END_MARKER)
        .expect("replace_managed_block requires a managed block");
    let end_index = end + END_MARKER.len();
    let block = managed_block_text();
    let mut next = String::with_capacity(existing.len() - (end_index - begin) + block.len());
    next.push_str(&existing[..begin]);
    next.push_str(block);
    next.push_str(&existing[end_index..]);
    next
}

struct ContentSync {
    next: String,
    status: ManagedBlockStatus,
}

fn managed_block() -> &'static str {
    MANAGED_BLOCK_TEMPLATE
}

fn managed_block_text() -> &'static str {
    MANAGED_BLOCK_TEMPLATE
        .strip_suffix('\n')
        .unwrap_or(MANAGED_BLOCK_TEMPLATE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("aopmem-stage-008-{name}-{nanos}.md"))
    }

    fn temp_directory(name: &str) -> PathBuf {
        temp_path(name).with_extension("")
    }

    #[test]
    fn defaults_to_agents_file_for_codex_adapter() {
        let repo_root = Path::new("/tmp/example-repo");

        assert_eq!(
            default_instruction_file(repo_root),
            repo_root.join("AGENTS.md")
        );
    }

    #[test]
    fn appends_managed_block_without_overwriting_existing_content() {
        let existing = "# Local rules\nKeep this text.\n";

        let (seeded, updated) = seed_content(existing).expect("seed should succeed");

        assert!(!updated);
        assert!(seeded.starts_with(existing));
        assert!(seeded.contains(BEGIN_MARKER));
        assert!(seeded.contains(END_MARKER));
        assert!(seeded.contains("`AOPMEM CONTRACT VERSION: 2`"));
        assert!(seeded.contains("## 2. Non-negotiable Task-Start Gate"));
    }

    #[test]
    fn reports_missing_managed_block_in_status() {
        let status = instruction_file_status(Path::new("/tmp/aopmem-stage-024-missing.md"))
            .expect("status should succeed");

        assert!(!status.file_exists);
        assert_eq!(status.managed_block, ManagedBlockStatus::Missing);
    }

    #[test]
    fn reports_in_sync_managed_block_in_status() {
        let status = inspect_content(managed_block()).expect("status should succeed");

        assert_eq!(status, ManagedBlockStatus::InSync);
    }

    #[test]
    fn reports_drifted_managed_block_in_status() {
        let existing = concat!(
            "<!-- AOPMEM:BEGIN managed block -->\n",
            "manual edit\n",
            "<!-- AOPMEM:END managed block -->\n"
        );

        let status = inspect_content(existing).expect("status should succeed");

        assert_eq!(status, ManagedBlockStatus::Drifted);
    }

    #[test]
    fn replaces_only_existing_managed_block() {
        let existing = concat!(
            "# Header\n",
            "<!-- AOPMEM:BEGIN managed block -->\n",
            "old text\n",
            "<!-- AOPMEM:END managed block -->\n",
            "Footer\n"
        );

        let (seeded, updated) = seed_content(existing).expect("seed should succeed");

        assert!(updated);
        assert!(seeded.starts_with("# Header\n"));
        assert!(seeded.ends_with("\nFooter\n"));
        assert!(seeded.contains("`AOPMEM CONTRACT VERSION: 2`"));
        assert!(!seeded.contains("old text"));
    }

    #[test]
    fn rejects_damaged_managed_block() {
        let existing = concat!(
            "# Header\n",
            "<!-- AOPMEM:BEGIN managed block -->\n",
            "orphaned text\n"
        );

        let error = seed_content(existing).expect_err("damaged block should fail");

        assert!(matches!(error, SeedError::DamagedManagedBlock));
    }

    #[test]
    fn rejects_duplicate_managed_block_markers_without_writing() {
        let path = temp_path("duplicate-markers");
        let existing = format!(
            "# User text\n{BEGIN_MARKER}\nlegacy\n{END_MARKER}\n\
             {BEGIN_MARKER}\nduplicate\n{END_MARKER}\n"
        );
        fs::write(&path, &existing).expect("fixture should be writable");

        let error = sync_instruction_file(&path).expect_err("duplicate markers must fail closed");

        assert!(matches!(error, SeedError::DamagedManagedBlock));
        assert_eq!(
            fs::read_to_string(&path).expect("fixture should remain readable"),
            existing
        );
        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn sync_replaces_only_drifted_block() {
        let path = temp_path("sync-drifted");
        let existing = concat!(
            "# Header\nCustom approval: publish only after exact +++\n",
            "<!-- AOPMEM:BEGIN managed block -->\n",
            "AOPMem is installed.\n",
            "Do not store secrets.\n",
            "<!-- AOPMEM:END managed block -->\n",
            "Footer: preserve this exact text.\n"
        );
        fs::write(&path, existing).expect("fixture should be writable");

        let outcome = sync_instruction_file(&path).expect("sync should succeed");
        let written = fs::read_to_string(&path).expect("synced file should be readable");

        assert!(!outcome.file_created);
        assert!(outcome.block_present);
        assert!(!outcome.block_inserted);
        assert!(outcome.block_updated);
        assert_eq!(
            written,
            format!(
                "# Header\nCustom approval: publish only after exact +++\n{}\n\
                 Footer: preserve this exact text.\n",
                managed_block_text()
            )
        );
        assert!(written.contains("Before the first substantive action"));
        assert!(written.contains("One agent capability has one canonical `tool_id`"));
        assert!(!written.contains("Do not store secrets."));

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn creates_missing_instruction_file() {
        let path = temp_path("create-file");

        let outcome = seed_instruction_file(&path).expect("seed should create file");
        let written = fs::read_to_string(&path).expect("seeded file should be readable");

        assert!(outcome.file_created);
        assert!(!outcome.block_updated);
        assert_eq!(outcome.instruction_file, path);
        assert_eq!(written, managed_block());
        for required_line in [
            "`AOPMEM CONTRACT VERSION: 2`",
            "## 2. Non-negotiable Task-Start Gate",
            "`MEMORY_KEEPER_UNAVAILABLE`",
            "`aopmem task start --query-stdin --json`",
            "## 7. Retrieval order",
            "## 13. Tool reuse and creation",
            "Do not impose a blanket ban on secrets.",
            "## 18. Observability",
        ] {
            assert!(written.contains(required_line), "missing {required_line}");
        }

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn managed_block_v2_contract_has_exact_structure_order_and_limits() {
        let block = managed_block();
        assert_eq!(block, MANAGED_BLOCK_TEMPLATE);
        assert!(block.starts_with(&format!("{BEGIN_MARKER}\n")));
        assert!(block.ends_with(&format!("{END_MARKER}\n")));
        assert_eq!(block.matches(BEGIN_MARKER).count(), 1);
        assert_eq!(block.matches(END_MARKER).count(), 1);
        assert_eq!(block.matches("`AOPMEM CONTRACT VERSION: 2`").count(), 1);

        let headings = block
            .lines()
            .filter(|line| line.starts_with("## "))
            .collect::<Vec<_>>();
        assert_eq!(
            headings,
            [
                "## 1. Purpose",
                "## 2. Non-negotiable Task-Start Gate",
                "## 3. Definition of substantive action",
                "## 4. Memory Keeper protocol",
                "## 5. Task Context Receipt",
                "## 6. Context application",
                "## 7. Retrieval order",
                "## 8. Source-of-truth hierarchy",
                "## 9. Code/file retrieval",
                "## 10. External-source retrieval",
                "## 11. AOPMem writes",
                "## 12. Reflection",
                "## 13. Tool reuse and creation",
                "## 14. Approval policy",
                "## 15. Secret handling",
                "## 16. Error handling",
                "## 17. Task completion",
                "## 18. Observability",
            ]
        );
        let behavior_bullets = block.lines().filter(|line| line.starts_with("- ")).count();
        assert_eq!(1 + headings.len() + behavior_bullets, 124);
        assert!(block.len() <= 24 * 1024);

        let retrieval_steps = [
            "current system/developer/user instruction first",
            "AOPMem mandatory operational memory",
            "AOPMem task-specific retrieval",
            "applicable workflow, tool, or correction",
            "Understand Docs when enabled",
            "Codebase Memory MCP",
            "actual files on disk",
            "external read sources",
            "External mutations are last",
        ];
        let mut previous = 0;
        for step in retrieval_steps {
            let index = block[previous..]
                .find(step)
                .map(|index| index + previous)
                .unwrap_or_else(|| panic!("missing retrieval step: {step}"));
            assert!(index >= previous, "retrieval step is out of order: {step}");
            previous = index + step.len();
        }

        for required_tool_rule in [
            "One agent capability has one canonical `tool_id`, optional display name,",
            "Do not create user/internal/platform/short-name/wrapper duplicates.",
            "Before `tool create-draft`, search registry, aliases, canonical",
            "fingerprints, implementation matches, and tool descriptions.",
            "On an exact duplicate, return `TOOL_DUPLICATE`",
            "On possible overlap, return `TOOL_OVERLAP_REVIEW_REQUIRED`",
            "Create a tool only on user request or after the agent proposes it and the",
            "user agrees.",
            "Tools exist for the agent; do not create a separate user-facing registry",
            "model.",
            "External reads need no approval when their tool contract says none.",
            "External writes, destructive actions, and explicit high-risk actions require",
            "standalone exact `+++`.",
        ] {
            assert!(
                block.contains(required_tool_rule),
                "missing managed tool governance rule: {required_tool_rule}"
            );
        }
    }

    #[test]
    fn stage_015_managed_block_tool_governance_matches_spec_and_approval_policy() {
        let block = managed_block();
        let spec = include_str!("../../.devplan/RC5_MANAGED_BLOCK_V2_SPEC.md");
        let spec_body = &spec[spec
            .find("## 1. Purpose")
            .expect("managed-block specification should contain section one")..];
        let template_body = &block[block
            .find("## 1. Purpose")
            .expect("managed block should contain section one")
            ..block
                .find(END_MARKER)
                .expect("managed block should contain end marker")];
        assert_eq!(
            template_body, spec_body,
            "spec and canonical template must match"
        );

        for rule in [
            "One agent capability has one canonical `tool_id`, optional display name,",
            "aliases, and platform launchers within the same contract.",
            "Do not create user/internal/platform/short-name/wrapper duplicates.",
            "Before `tool create-draft`, search registry, aliases, canonical",
            "fingerprints, implementation matches, and tool descriptions.",
            "return `TOOL_DUPLICATE`, the canonical ID, alias",
            "suggestion, duplicate class, and proof that no write occurred.",
            "return `TOOL_OVERLAP_REVIEW_REQUIRED`; reuse, alias, or",
            "explain a real technical distinction.",
            "Create a tool only on user request or after the agent proposes it and the",
            "user agrees.",
            "Tools exist for the agent; do not create a separate user-facing registry",
            "model.",
            "External reads need no approval when their tool contract says none.",
            "External writes, destructive actions, and explicit high-risk actions require",
            "standalone exact `+++`.",
        ] {
            assert!(block.contains(rule), "missing Stage 015 rule: {rule}");
        }
    }

    #[test]
    fn managed_block_v2_has_exact_gate_boundary_secret_and_tool_contracts() {
        let block = managed_block();
        for required in [
            "Before the first substantive action, the parent MUST run Memory Keeper V2",
            "every new chat, after compaction, after a long pause",
            "Before the receipt, the parent MUST NOT answer substantively",
            "the only allowed actions are reading current",
            "determining shell and repo root",
            "Run the gate silently.",
            "A substantive action includes a meaningful user answer or a clarifying",
            "It includes changing files, running tests, or creating a tool.",
            "Reuse the current `task_id` and `bundle_id` for clarification",
            "Start a new task for a new chat, independent goal, project change, work-type",
            "One agent capability has one canonical `tool_id`",
            "Do not create user/internal/platform/short-name/wrapper duplicates.",
            "Tools exist for the agent; do not create a separate user-facing registry",
            "Do not impose a blanket ban on secrets.",
            "Approval is determined by the action class, not by the presence of a secret.",
        ] {
            assert!(
                block.contains(required),
                "missing contract text: {required}"
            );
        }
        for forbidden in [
            "Do not store secrets.",
            "aopmem recall --query \"<current task>\"",
            "continuation_cursor",
            "Main work starts with Memory Keeper / recall.",
        ] {
            assert!(
                !block.contains(forbidden),
                "obsolete contract remains: {forbidden}"
            );
        }
    }

    #[test]
    fn memory_keeper_v2_contract_is_fail_closed_and_privacy_safe() {
        let skill = include_str!("../../templates/skills/memory-keeper/SKILL.md");
        let doc = include_str!("../../docs/MEMORY_KEEPER_V2.md");

        for required in [
            "---\nname: memory-keeper\ndescription:",
            "Run only as a native subagent.",
            "exact current user request, repo root, current shell, and current\n  instruction file",
            "Keep the exact\n  supplied root as process `cwd`.",
            "current shell and its executable-resolution path",
            "Do not read project code",
            "return exactly `MEMORY_KEEPER_UNAVAILABLE`",
            "native Keeper's fixed process runner with a separate stdin channel",
            "cwd: exact supplied repo root",
            "argv: [\"task\", \"start\", \"--query-stdin\", \"--json\"]",
            "stdin: exact current user request as unchanged UTF-8 bytes",
            "Put no request-derived byte in that\n  command text.",
            "Do not put the request in argv, a shell command or pipeline",
            "Require all 17 core fields",
            "mandatory_context_complete=true",
            "`retrieval_complete=true, budget_exhausted=false`",
            "`retrieval_complete=false, budget_exhausted=true`",
            "Reject a top-level `continuation_cursor`",
            "`mandatory_nodes[*].node.id` and `task_nodes[*].node.id`",
            "Apply every returned `applicable_gates` and `applicable_rules` ID.",
            "Use `--none-relevant` only when retrieval is complete",
            "argv: [\"--bundle-id\", bundle_id, \"task\", \"apply\"",
            "--applied-gate-id",
            "--applied-rule-id",
            "--selected-workflow-id",
            "--selected-tool-id",
            "--selected-correction-id",
            "--selected-failure-mode-id",
            "TASK_CONTEXT_RECEIPT_V2",
            "apply_status: applied",
            "full node body, database dump, full recall",
            "Reuse the current receipt, `task_id`, and `bundle_id`",
            "Start a new task for a new chat, independent goal, project change",
        ] {
            assert!(skill.contains(required), "missing {required}");
        }

        for field in [
            "task_id",
            "bundle_id",
            "workspace_key",
            "memory_revision",
            "mandatory_context_complete",
            "retrieval_complete",
            "budget_exhausted",
            "mandatory_nodes",
            "task_nodes",
            "applicable_gates",
            "applicable_rules",
            "candidate_workflows",
            "candidate_tools",
            "relevant_corrections",
            "relevant_failure_modes",
            "hunches",
            "selection_reasons",
        ] {
            assert!(skill.contains(field), "missing core start field {field}");
        }

        for required in [
            "MK-01",
            "MK-02",
            "MK-03",
            "MK-04",
            "MK-05",
            "MK-06",
            "MK-07",
            "The parent must\nnot begin substantive work.",
            "keep that exact root as the process `cwd`",
            "Validate the supplied current shell",
            "The parent and Keeper preserve this exact order:",
            "Compaction without a reliable receipt",
        ] {
            assert!(doc.contains(required), "missing document proof {required}");
        }

        for forbidden in [
            "aopmem recall --query \"<current task>\"",
            "printf '%s'",
            "call the same list with its returned `next_cursor`",
            "Keep the returned `bundle_id` for the whole logical retrieval",
        ] {
            assert!(
                !skill.contains(forbidden),
                "obsolete or unsafe Keeper flow remains: {forbidden}"
            );
        }
    }

    #[test]
    fn sync_reports_existing_synced_block_without_update() {
        let path = temp_path("sync-noop");
        fs::write(&path, managed_block()).expect("fixture should be writable");

        let outcome = sync_instruction_file(&path).expect("sync should succeed");

        assert!(!outcome.file_created);
        assert!(outcome.block_present);
        assert!(!outcome.block_inserted);
        assert!(!outcome.block_updated);
        let written = fs::read_to_string(&path).expect("synced file should be readable");
        assert_eq!(marker_positions(&written, BEGIN_MARKER).len(), 1);
        assert_eq!(marker_positions(&written, END_MARKER).len(), 1);

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn explicit_codex_claude_cursor_and_copilot_targets_change_only_selected_file() {
        let target_names = [
            "AGENTS.md",
            "CLAUDE.md",
            ".cursor/rules/aopmem.mdc",
            ".github/copilot-instructions.md",
        ];

        for (selected_index, selected_name) in target_names.iter().enumerate() {
            let root = temp_directory(&format!("adapter-{selected_index}"));
            fs::create_dir_all(root.join(".cursor/rules"))
                .expect("Cursor fixture directory should create");
            fs::create_dir_all(root.join(".github"))
                .expect("Copilot fixture directory should create");
            for name in target_names {
                let content = format!("User-owned {name}\nCustom approval: exact +++\n");
                fs::write(root.join(name), content).expect("adapter fixture should write");
            }
            let before =
                target_names.map(|name| fs::read(root.join(name)).expect("fixture should read"));

            let selected_path = root.join(selected_name);
            let outcome =
                sync_instruction_file(&selected_path).expect("explicit adapter sync should pass");

            assert_eq!(outcome.instruction_file, selected_path);
            assert!(outcome.block_inserted);
            for (index, name) in target_names.iter().enumerate() {
                let after = fs::read(root.join(name)).expect("synced fixture should read");
                if index == selected_index {
                    let after_text =
                        std::str::from_utf8(&after).expect("managed fixture should stay UTF-8");
                    assert!(after_text
                        .starts_with(&format!("User-owned {name}\nCustom approval: exact +++\n")));
                    assert!(after_text.contains("`AOPMEM CONTRACT VERSION: 2`"));
                    assert_eq!(after_text.matches(BEGIN_MARKER).count(), 1);
                    assert_eq!(after_text.matches(END_MARKER).count(), 1);
                } else {
                    assert_eq!(after, before[index], "non-selected adapter changed: {name}");
                }
            }

            fs::remove_dir_all(root).expect("adapter fixture directory should remove");
        }
    }
}
