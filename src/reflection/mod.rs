use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{audit, storage};

pub const REFLECTION_INVENTORY_SUMMARY: &str = "reflection_inventory_v1";
pub const REFLECTION_MATERIAL_SUMMARY: &str = "reflection_material_v1";
pub const REFLECTION_SANITIZED_MATERIAL_SUMMARY: &str = "reflection_sanitized_material_v1";
pub const REFLECTION_PROPOSAL_SUMMARY: &str = "reflection_proposal_v1";
pub const REFLECTION_APPLY_SUMMARY: &str = "reflection_apply_v1";

const REFLECTION_APPLY_SAVEPOINT: &str = "aopmem_reflection_apply_attempt";
const REFLECTION_INVENTORY_SAVEPOINT: &str = "aopmem_reflection_inventory";
const REFLECTION_PROPOSAL_SAVEPOINT: &str = "aopmem_reflection_proposal";

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReflectionSavepointFailure {
    Rollback,
    Release,
}

#[cfg(test)]
std::thread_local! {
    static REFLECTION_SAVEPOINT_FAILURE: std::cell::Cell<Option<ReflectionSavepointFailure>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn inject_reflection_savepoint_failure(failure: ReflectionSavepointFailure) {
    REFLECTION_SAVEPOINT_FAILURE.with(|current| current.set(Some(failure)));
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryStatus {
    Empty,
    Tracked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReflectionInventorySession {
    pub session_id: String,
    pub source_node_ids: Vec<i64>,
    pub inventory_status: InventoryStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReflectionInventoryReport {
    pub inventory_id: i64,
    pub inventory_status: InventoryStatus,
    pub reflected_session_ids: Vec<String>,
    pub sessions: Vec<ReflectionInventorySession>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionInventoryRecord {
    inventory_status: InventoryStatus,
    reflected_session_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionSessionRecord {
    session_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflectionRisk {
    Low,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionProposalInput {
    pub items: Vec<ReflectionProposalItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReflectionProposal {
    pub proposal_id: i64,
    pub session_id: String,
    pub items: Vec<ReflectionProposalItem>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectionDraftItem {
    pub index: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReflectionApplyReport {
    pub apply_id: i64,
    pub proposal_id: i64,
    pub session_id: String,
    pub applied_item_indexes: Vec<usize>,
    pub draft_items: Vec<ReflectionDraftItem>,
    pub created_node_ids: Vec<i64>,
    pub created_alias_ids: Vec<i64>,
    pub created_tag_ids: Vec<i64>,
    pub created_source_ids: Vec<i64>,
    pub created_link_ids: Vec<i64>,
    pub created_at: String,
}

/// Result committed by the mutation coordinator for one apply attempt.
///
/// A normal apply failure is a durable outcome because its failure event must
/// survive while all proposal effects are rolled back to the savepoint.
#[derive(Debug)]
pub(crate) enum ReflectionApplyAttempt {
    Applied(ReflectionApplyReport),
    Failed { error: ReflectionError },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ReflectionProposalItem {
    CreateNode {
        risk: ReflectionRisk,
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
        risk: ReflectionRisk,
        #[serde(default)]
        node_id: Option<i64>,
        #[serde(default)]
        node_ref: Option<String>,
        alias: String,
    },
    AddTag {
        risk: ReflectionRisk,
        #[serde(default)]
        node_id: Option<i64>,
        #[serde(default)]
        node_ref: Option<String>,
        tag: String,
    },
    AddSource {
        risk: ReflectionRisk,
        #[serde(default)]
        node_id: Option<i64>,
        #[serde(default)]
        node_ref: Option<String>,
        source_ref: String,
    },
    AddLink {
        risk: ReflectionRisk,
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
    UpdateNodeBody {
        risk: ReflectionRisk,
        node_id: i64,
        body: String,
    },
    UpdateNodeStatus {
        risk: ReflectionRisk,
        node_id: i64,
        status: String,
    },
    DeleteNode {
        risk: ReflectionRisk,
        node_id: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ReflectionProposalRecord {
    session_id: String,
    items: Vec<ReflectionProposalItem>,
}

#[derive(Serialize)]
struct ReflectionProposalRecordRef<'a> {
    session_id: &'a str,
    items: &'a [ReflectionProposalItem],
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionApplyRecord {
    session_id: String,
    proposal_id: i64,
    applied_item_indexes: Vec<usize>,
    draft_items: Vec<ReflectionDraftItem>,
    created_node_ids: Vec<i64>,
    created_alias_ids: Vec<i64>,
    created_tag_ids: Vec<i64>,
    created_source_ids: Vec<i64>,
    created_link_ids: Vec<i64>,
}

#[derive(Serialize)]
struct ReflectionApplyRecordRef<'a> {
    session_id: &'a str,
    proposal_id: i64,
    applied_item_indexes: &'a [usize],
    draft_items: &'a [ReflectionDraftItem],
    created_node_ids: &'a [i64],
    created_alias_ids: &'a [i64],
    created_tag_ids: &'a [i64],
    created_source_ids: &'a [i64],
    created_link_ids: &'a [i64],
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReflectionValidationError {
    #[error("reflection proposal must contain at least one item")]
    EmptyProposal,
    #[error("reflection proposal has {actual} items; maximum is {max_items}")]
    TooManyProposalItems { max_items: usize, actual: usize },
    #[error(
        "reflection proposal item {index} has invalid risk: expected {expected}, got {actual}"
    )]
    InvalidRisk {
        index: usize,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("reflection proposal item {index} has an empty node_ref")]
    EmptyNodeRef { index: usize },
    #[error("reflection proposal item {index} node_ref exceeds {max_bytes} bytes")]
    NodeRefTooLong { index: usize, max_bytes: usize },
    #[error("reflection proposal item {index} must set exactly one of node_id or node_ref")]
    InvalidNodeSelector { index: usize },
    #[error(
        "reflection proposal item {index} must set exactly one source and one target selector"
    )]
    InvalidLinkSelector { index: usize },
    #[error("reflection proposal item {index} has an empty title")]
    EmptyTitle { index: usize },
    #[error("reflection proposal item {index} has an empty status")]
    EmptyStatus { index: usize },
    #[error("reflection proposal item {index} has an empty body")]
    EmptyBody { index: usize },
    #[error("reflection proposal item {index} has an empty value")]
    EmptyValue { index: usize },
}

#[derive(Debug, Error)]
pub enum ReflectionError {
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error(transparent)]
    Node(#[from] storage::NodeStorageError),
    #[error(transparent)]
    Link(#[from] storage::LinkStorageError),
    #[error(transparent)]
    Metadata(#[from] storage::MetadataStorageError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Validation(#[from] ReflectionValidationError),
    #[error(transparent)]
    Audit(#[from] audit::AuditError),
    #[error("reflection session id cannot be empty")]
    EmptySessionId,
    #[error("reflection apply attempts require an existing outer transaction")]
    ApplyAttemptRequiresTransaction,
    #[error("current reflection inventory node disappeared during update: {0}")]
    InventoryNodeDisappeared(i64),
    #[error("reflection proposal not found: {0}")]
    ProposalNotFound(i64),
    #[error("invalid reflection proposal record: {0}")]
    InvalidProposalRecord(i64),
    #[error("reflection proposal has duplicate node_ref: {0}")]
    DuplicateNodeRef(String),
}

pub fn inventory_sessions(
    connection: &Connection,
) -> Result<ReflectionInventoryReport, ReflectionError> {
    if connection.is_autocommit() {
        let transaction = connection.unchecked_transaction()?;
        let report = inventory_sessions_in_transaction(&transaction)?;
        transaction.commit()?;
        return Ok(report);
    }

    run_nested_reflection_savepoint(
        connection,
        REFLECTION_INVENTORY_SAVEPOINT,
        inventory_sessions_in_transaction,
    )
}

fn run_nested_reflection_savepoint<T>(
    connection: &Connection,
    savepoint: &'static str,
    operation: impl FnOnce(&Connection) -> Result<T, ReflectionError>,
) -> Result<T, ReflectionError> {
    connection.execute_batch(&format!("SAVEPOINT {savepoint};"))?;
    match operation(connection) {
        Ok(value) => {
            connection.execute_batch(&format!("RELEASE SAVEPOINT {savepoint};"))?;
            Ok(value)
        }
        Err(error) => {
            connection.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {savepoint};\
                 RELEASE SAVEPOINT {savepoint};"
            ))?;
            Err(error)
        }
    }
}

fn inventory_sessions_in_transaction(
    connection: &Connection,
) -> Result<ReflectionInventoryReport, ReflectionError> {
    let sessions = list_reflected_sessions(connection)?;
    let reflected_session_ids = sessions
        .iter()
        .map(|session| session.session_id.clone())
        .collect::<Vec<_>>();
    let inventory_status = if reflected_session_ids.is_empty() {
        InventoryStatus::Empty
    } else {
        InventoryStatus::Tracked
    };
    let record = ReflectionInventoryRecord {
        inventory_status: inventory_status.clone(),
        reflected_session_ids: reflected_session_ids.clone(),
    };
    let inventory_node = match store_inventory_snapshot(connection, &record)? {
        InventoryWrite::Created(node) => {
            audit::record_reflection_event(
                connection,
                audit::ReflectionEventKind::InventoryCreated,
                node.id,
            )?;
            node
        }
        InventoryWrite::Updated(node) => {
            audit::record_reflection_event(
                connection,
                audit::ReflectionEventKind::InventoryUpdated,
                node.id,
            )?;
            node
        }
        InventoryWrite::Unchanged(node) => node,
    };

    Ok(ReflectionInventoryReport {
        inventory_id: inventory_node.id,
        inventory_status,
        reflected_session_ids,
        sessions,
        created_at: inventory_node.created_at,
    })
}

enum InventoryWrite {
    Created(storage::Node),
    Updated(storage::Node),
    Unchanged(storage::Node),
}

fn store_inventory_snapshot(
    connection: &Connection,
    record: &ReflectionInventoryRecord,
) -> Result<InventoryWrite, ReflectionError> {
    let record_body = serde_json::to_string(record)?;
    let latest_inventory =
        storage::list_nodes_with_summaries(connection, &[REFLECTION_INVENTORY_SUMMARY])?
            .into_iter()
            .next_back();

    let Some(existing) = latest_inventory else {
        return Ok(InventoryWrite::Created(create_inventory_snapshot(
            connection,
            record_body,
        )?));
    };

    let is_current = existing
        .body
        .as_deref()
        .map(serde_json::from_str::<ReflectionInventoryRecord>)
        .transpose()?
        .as_ref()
        == Some(record);
    if is_current {
        return Ok(InventoryWrite::Unchanged(existing));
    }

    let update = storage::NodeUpdate {
        id: existing.id,
        status: existing.status,
        title: existing.title,
        summary: existing.summary,
        body: Some(record_body.clone()),
        source_ref: existing.source_ref,
        confidence: existing.confidence,
        trust_level: existing.trust_level,
    };

    let existing_id = existing.id;
    storage::update_node(connection, &update)?
        .map(InventoryWrite::Updated)
        .ok_or(ReflectionError::InventoryNodeDisappeared(existing_id))
}

fn create_inventory_snapshot(
    connection: &Connection,
    body: String,
) -> Result<storage::Node, ReflectionError> {
    Ok(storage::create_node(
        connection,
        &storage::NewNode {
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: "Reflection inventory".to_string(),
            summary: Some(REFLECTION_INVENTORY_SUMMARY.to_string()),
            body: Some(body),
            source_ref: None,
            confidence: None,
            trust_level: None,
        },
    )?)
}

pub fn store_proposal(
    connection: &Connection,
    session_id: &str,
    proposal: &ReflectionProposalInput,
) -> Result<ReflectionProposal, ReflectionError> {
    if connection.is_autocommit() {
        let transaction = connection.unchecked_transaction()?;
        let stored = store_proposal_in_transaction(&transaction, session_id, proposal)?;
        transaction.commit()?;
        return Ok(stored);
    }

    run_nested_reflection_savepoint(connection, REFLECTION_PROPOSAL_SAVEPOINT, |connection| {
        store_proposal_in_transaction(connection, session_id, proposal)
    })
}

fn store_proposal_in_transaction(
    connection: &Connection,
    session_id: &str,
    proposal: &ReflectionProposalInput,
) -> Result<ReflectionProposal, ReflectionError> {
    let input = reflection_proposal_node_input(session_id, proposal)?;
    let proposal_node = storage::create_node(connection, &input)?;
    audit::record_reflection_event(
        connection,
        audit::ReflectionEventKind::ProposalCreated,
        proposal_node.id,
    )?;

    Ok(ReflectionProposal {
        proposal_id: proposal_node.id,
        session_id: session_id.to_string(),
        items: proposal.items.clone(),
        created_at: proposal_node.created_at,
    })
}

pub fn validate_proposal_input(
    session_id: &str,
    proposal: &ReflectionProposalInput,
) -> Result<(), ReflectionError> {
    let input = reflection_proposal_node_input(session_id, proposal)?;
    storage::validate_new_node_input(&input)
        .map_err(storage::NodeStorageError::Validation)
        .map_err(ReflectionError::Node)
}

fn reflection_proposal_node_input(
    session_id: &str,
    proposal: &ReflectionProposalInput,
) -> Result<storage::NewNode, ReflectionError> {
    if session_id.trim().is_empty() {
        return Err(ReflectionError::EmptySessionId);
    }
    validate_reflection_proposal(proposal)?;
    let record = ReflectionProposalRecordRef {
        session_id,
        items: &proposal.items,
    };
    Ok(storage::NewNode {
        node_type: "raw_note".to_string(),
        status: "draft".to_string(),
        title: format!("Reflection proposal {session_id}"),
        summary: Some(REFLECTION_PROPOSAL_SUMMARY.to_string()),
        body: Some(serde_json::to_string(&record)?),
        source_ref: None,
        confidence: None,
        trust_level: None,
    })
}

pub fn apply_proposal(
    connection: &Connection,
    proposal_id: i64,
) -> Result<ReflectionApplyReport, ReflectionError> {
    if connection.is_autocommit() {
        let transaction = connection.unchecked_transaction()?;
        let report = apply_proposal_in_transaction(&transaction, proposal_id)?;
        transaction.commit()?;
        Ok(report)
    } else {
        apply_proposal_in_transaction(connection, proposal_id)
    }
}

#[cfg(not(test))]
fn maybe_fail_apply_savepoint_rollback() -> rusqlite::Result<()> {
    Ok(())
}

#[cfg(test)]
fn maybe_fail_apply_savepoint_rollback() -> rusqlite::Result<()> {
    let should_fail = REFLECTION_SAVEPOINT_FAILURE.with(|current| {
        if current.get() == Some(ReflectionSavepointFailure::Rollback) {
            current.set(None);
            true
        } else {
            false
        }
    });
    if should_fail {
        Err(rusqlite::Error::InvalidQuery)
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
fn maybe_fail_apply_savepoint_release() -> rusqlite::Result<()> {
    Ok(())
}

#[cfg(test)]
fn maybe_fail_apply_savepoint_release() -> rusqlite::Result<()> {
    let should_fail = REFLECTION_SAVEPOINT_FAILURE.with(|current| {
        if current.get() == Some(ReflectionSavepointFailure::Release) {
            current.set(None);
            true
        } else {
            false
        }
    });
    if should_fail {
        Err(rusqlite::Error::InvalidQuery)
    } else {
        Ok(())
    }
}

fn apply_proposal_in_transaction(
    connection: &Connection,
    proposal_id: i64,
) -> Result<ReflectionApplyReport, ReflectionError> {
    let proposal = load_proposal(connection, proposal_id)?;
    validate_reflection_items(&proposal.items)?;

    connection.execute_batch(&format!("SAVEPOINT {REFLECTION_APPLY_SAVEPOINT};"))?;
    match apply_loaded_proposal_in_transaction(connection, proposal) {
        Ok(report) => {
            connection
                .execute_batch(&format!("RELEASE SAVEPOINT {REFLECTION_APPLY_SAVEPOINT};"))?;
            Ok(report)
        }
        Err(error) => {
            connection.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {REFLECTION_APPLY_SAVEPOINT};\
                 RELEASE SAVEPOINT {REFLECTION_APPLY_SAVEPOINT};"
            ))?;
            Err(error)
        }
    }
}

/// Applies a proposal under a savepoint owned by the existing workspace
/// mutation transaction. Normal proposal failures become durable failed
/// attempts; infrastructure failures still abort the complete mutation.
pub(crate) fn attempt_apply_proposal(
    connection: &Connection,
    proposal_id: i64,
) -> Result<ReflectionApplyAttempt, ReflectionError> {
    if connection.is_autocommit() {
        return Err(ReflectionError::ApplyAttemptRequiresTransaction);
    }

    let proposal = load_proposal(connection, proposal_id)?;
    validate_reflection_items(&proposal.items)?;

    connection.execute_batch(&format!("SAVEPOINT {REFLECTION_APPLY_SAVEPOINT};"))?;
    match apply_loaded_proposal_in_transaction(connection, proposal) {
        Ok(report) => {
            maybe_fail_apply_savepoint_release()?;
            connection
                .execute_batch(&format!("RELEASE SAVEPOINT {REFLECTION_APPLY_SAVEPOINT};"))?;
            Ok(ReflectionApplyAttempt::Applied(report))
        }
        Err(error) => {
            maybe_fail_apply_savepoint_rollback()?;
            connection.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {REFLECTION_APPLY_SAVEPOINT};\
                 RELEASE SAVEPOINT {REFLECTION_APPLY_SAVEPOINT};"
            ))?;
            audit::record_reflection_event(
                connection,
                audit::ReflectionEventKind::ApplyFailed,
                proposal_id,
            )?;
            Ok(ReflectionApplyAttempt::Failed { error })
        }
    }
}

fn apply_loaded_proposal_in_transaction(
    connection: &Connection,
    proposal: ReflectionProposal,
) -> Result<ReflectionApplyReport, ReflectionError> {
    validate_reflection_items(&proposal.items)?;
    let proposal_id = proposal.proposal_id;

    let mut resolved_node_refs = HashMap::<String, i64>::with_capacity(proposal.items.len());
    let mut applied_item_indexes = Vec::new();
    let mut draft_items = Vec::new();
    let mut created_node_ids = Vec::new();
    let mut created_alias_ids = Vec::new();
    let mut created_tag_ids = Vec::new();
    let mut created_source_ids = Vec::new();
    let mut created_link_ids = Vec::new();
    let mut alias_node_ids = BTreeSet::new();

    for (index, item) in proposal.items.iter().enumerate() {
        if item.risk() == ReflectionRisk::High {
            draft_items.push(ReflectionDraftItem {
                index,
                reason: "high_risk_item".to_string(),
            });
            continue;
        }

        let outcome = apply_low_risk_item(
            connection,
            item,
            &mut resolved_node_refs,
            &mut alias_node_ids,
        )?;
        match outcome {
            ReflectionApplyOutcome::Applied(applied) => {
                applied_item_indexes.push(index);
                if let Some(node_id) = applied.created_node_id {
                    created_node_ids.push(node_id);
                }
                if let Some(alias_id) = applied.created_alias_id {
                    created_alias_ids.push(alias_id);
                }
                if let Some(tag_id) = applied.created_tag_id {
                    created_tag_ids.push(tag_id);
                }
                if let Some(source_id) = applied.created_source_id {
                    created_source_ids.push(source_id);
                }
                if let Some(link_id) = applied.created_link_id {
                    created_link_ids.push(link_id);
                }
            }
            ReflectionApplyOutcome::Draft(reason) => {
                draft_items.push(ReflectionDraftItem { index, reason });
            }
        }
    }

    let record = ReflectionApplyRecordRef {
        session_id: &proposal.session_id,
        proposal_id,
        applied_item_indexes: &applied_item_indexes,
        draft_items: &draft_items,
        created_node_ids: &created_node_ids,
        created_alias_ids: &created_alias_ids,
        created_tag_ids: &created_tag_ids,
        created_source_ids: &created_source_ids,
        created_link_ids: &created_link_ids,
    };
    let apply_node = storage::create_node(
        connection,
        &storage::NewNode {
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: format!("Reflection apply {}/{}", proposal.session_id, proposal_id),
            summary: Some(REFLECTION_APPLY_SUMMARY.to_string()),
            body: Some(serde_json::to_string(&record)?),
            source_ref: None,
            confidence: None,
            trust_level: None,
        },
    )?;
    storage::refresh_fts_nodes(connection, &alias_node_ids)?;
    // Proposal lifecycle events point to the durable proposal node. The
    // separate apply receipt holds the exact applied and drafted item indexes.
    audit::record_reflection_event(
        connection,
        audit::ReflectionEventKind::ProposalApplied,
        proposal_id,
    )?;
    if !draft_items.is_empty() {
        audit::record_reflection_event(
            connection,
            audit::ReflectionEventKind::ProposalDrafted,
            proposal_id,
        )?;
    }

    Ok(ReflectionApplyReport {
        apply_id: apply_node.id,
        proposal_id,
        session_id: proposal.session_id,
        applied_item_indexes,
        draft_items,
        created_node_ids,
        created_alias_ids,
        created_tag_ids,
        created_source_ids,
        created_link_ids,
        created_at: apply_node.created_at,
    })
}

pub fn list_reflected_sessions(
    connection: &Connection,
) -> Result<Vec<ReflectionInventorySession>, ReflectionError> {
    let nodes = storage::list_nodes_with_summaries(
        connection,
        &[
            REFLECTION_MATERIAL_SUMMARY,
            REFLECTION_SANITIZED_MATERIAL_SUMMARY,
            REFLECTION_PROPOSAL_SUMMARY,
            REFLECTION_APPLY_SUMMARY,
        ],
    )?;
    let mut tracked = BTreeMap::<String, Vec<i64>>::new();

    for node in nodes {
        let Some(summary) = node.summary.as_deref() else {
            continue;
        };
        let Some(body) = node.body.as_deref() else {
            continue;
        };

        match summary {
            REFLECTION_MATERIAL_SUMMARY
            | REFLECTION_SANITIZED_MATERIAL_SUMMARY
            | REFLECTION_PROPOSAL_SUMMARY
            | REFLECTION_APPLY_SUMMARY => {
                let record: ReflectionSessionRecord = serde_json::from_str(body)?;
                insert_tracked_session(&mut tracked, record.session_id, node.id)?;
            }
            _ => {}
        }
    }

    Ok(tracked
        .into_iter()
        .map(|(session_id, source_node_ids)| ReflectionInventorySession {
            session_id,
            source_node_ids,
            inventory_status: InventoryStatus::Tracked,
        })
        .collect())
}

fn insert_tracked_session(
    tracked: &mut BTreeMap<String, Vec<i64>>,
    session_id: String,
    node_id: i64,
) -> Result<(), ReflectionError> {
    if session_id.trim().is_empty() {
        return Err(ReflectionError::EmptySessionId);
    }

    tracked.entry(session_id).or_default().push(node_id);
    Ok(())
}

fn load_proposal(
    connection: &Connection,
    proposal_id: i64,
) -> Result<ReflectionProposal, ReflectionError> {
    let node = storage::get_node(connection, proposal_id)?
        .ok_or(ReflectionError::ProposalNotFound(proposal_id))?;
    if node.summary.as_deref() != Some(REFLECTION_PROPOSAL_SUMMARY) {
        return Err(ReflectionError::InvalidProposalRecord(proposal_id));
    }
    let record: ReflectionProposalRecord = serde_json::from_str(
        node.body
            .as_deref()
            .ok_or(ReflectionError::InvalidProposalRecord(proposal_id))?,
    )?;

    Ok(ReflectionProposal {
        proposal_id: node.id,
        session_id: record.session_id,
        items: record.items,
        created_at: node.created_at,
    })
}

fn validate_reflection_proposal(
    proposal: &ReflectionProposalInput,
) -> Result<(), ReflectionValidationError> {
    validate_reflection_items(&proposal.items)
}

fn validate_reflection_items(
    items: &[ReflectionProposalItem],
) -> Result<(), ReflectionValidationError> {
    if items.is_empty() {
        return Err(ReflectionValidationError::EmptyProposal);
    }
    if items.len() > storage::MAX_PROPOSAL_ITEMS {
        return Err(ReflectionValidationError::TooManyProposalItems {
            max_items: storage::MAX_PROPOSAL_ITEMS,
            actual: items.len(),
        });
    }

    for (index, item) in items.iter().enumerate() {
        validate_reflection_item(index, item)?;
    }

    Ok(())
}

fn validate_reflection_item(
    index: usize,
    item: &ReflectionProposalItem,
) -> Result<(), ReflectionValidationError> {
    let actual = item.risk();
    let expected = item.expected_risk()?;

    if actual != expected {
        return Err(ReflectionValidationError::InvalidRisk {
            index,
            expected: expected.as_str(),
            actual: actual.as_str(),
        });
    }

    match item {
        ReflectionProposalItem::CreateNode {
            node_ref,
            title,
            status,
            ..
        } => {
            validate_optional_ref(index, node_ref.as_deref())?;
            if title.trim().is_empty() {
                return Err(ReflectionValidationError::EmptyTitle { index });
            }
            if status.trim().is_empty() {
                return Err(ReflectionValidationError::EmptyStatus { index });
            }
        }
        ReflectionProposalItem::AddAlias {
            node_id,
            node_ref,
            alias,
            ..
        }
        | ReflectionProposalItem::AddTag {
            node_id,
            node_ref,
            tag: alias,
            ..
        }
        | ReflectionProposalItem::AddSource {
            node_id,
            node_ref,
            source_ref: alias,
            ..
        } => {
            validate_node_selector(index, *node_id, node_ref.as_deref())?;
            if alias.trim().is_empty() {
                return Err(ReflectionValidationError::EmptyValue { index });
            }
        }
        ReflectionProposalItem::AddLink {
            source_node_id,
            source_node_ref,
            target_node_id,
            target_node_ref,
            link_type,
            ..
        } => {
            let has_source_id = source_node_id.is_some();
            let has_source_ref = non_empty_option(source_node_ref.as_deref());
            let has_target_id = target_node_id.is_some();
            let has_target_ref = non_empty_option(target_node_ref.as_deref());
            validate_optional_ref(index, source_node_ref.as_deref())?;
            validate_optional_ref(index, target_node_ref.as_deref())?;
            if has_source_id == has_source_ref || has_target_id == has_target_ref {
                return Err(ReflectionValidationError::InvalidLinkSelector { index });
            }
            if link_type.trim().is_empty() {
                return Err(ReflectionValidationError::EmptyValue { index });
            }
        }
        ReflectionProposalItem::UpdateNodeBody { body, .. } => {
            if body.trim().is_empty() {
                return Err(ReflectionValidationError::EmptyBody { index });
            }
        }
        ReflectionProposalItem::UpdateNodeStatus { status, .. } => {
            if status.trim().is_empty() {
                return Err(ReflectionValidationError::EmptyStatus { index });
            }
        }
        ReflectionProposalItem::DeleteNode { .. } => {}
    }

    Ok(())
}

fn validate_optional_ref(
    index: usize,
    node_ref: Option<&str>,
) -> Result<(), ReflectionValidationError> {
    if matches!(node_ref, Some(value) if value.len() > storage::MAX_PROPOSAL_NODE_REF_BYTES) {
        return Err(ReflectionValidationError::NodeRefTooLong {
            index,
            max_bytes: storage::MAX_PROPOSAL_NODE_REF_BYTES,
        });
    }
    if matches!(node_ref, Some(value) if value.trim().is_empty()) {
        return Err(ReflectionValidationError::EmptyNodeRef { index });
    }

    Ok(())
}

fn validate_node_selector(
    index: usize,
    node_id: Option<i64>,
    node_ref: Option<&str>,
) -> Result<(), ReflectionValidationError> {
    validate_optional_ref(index, node_ref)?;

    let has_id = node_id.is_some();
    let has_ref = non_empty_option(node_ref);
    if has_id == has_ref {
        return Err(ReflectionValidationError::InvalidNodeSelector { index });
    }

    Ok(())
}

fn non_empty_option(value: Option<&str>) -> bool {
    matches!(value, Some(text) if !text.trim().is_empty())
}

#[derive(Debug, Default)]
struct ReflectionAppliedItemIds {
    created_node_id: Option<i64>,
    created_alias_id: Option<i64>,
    created_tag_id: Option<i64>,
    created_source_id: Option<i64>,
    created_link_id: Option<i64>,
}

enum ReflectionApplyOutcome {
    Applied(ReflectionAppliedItemIds),
    Draft(String),
}

fn apply_low_risk_item(
    connection: &Connection,
    item: &ReflectionProposalItem,
    resolved_node_refs: &mut HashMap<String, i64>,
    alias_node_ids: &mut BTreeSet<i64>,
) -> Result<ReflectionApplyOutcome, ReflectionError> {
    match item {
        ReflectionProposalItem::CreateNode {
            node_ref,
            node_type,
            status,
            title,
            summary,
            body,
            source_ref,
            confidence,
            trust_level,
            ..
        } => {
            let created = storage::create_node_borrowed(
                connection,
                storage::BorrowedNodeInput {
                    node_type,
                    status,
                    title,
                    summary: summary.as_deref(),
                    body: body.as_deref(),
                    source_ref: source_ref.as_deref(),
                    confidence: *confidence,
                    trust_level: trust_level.as_deref(),
                },
            )?;
            if let Some(node_ref) = node_ref {
                if resolved_node_refs.contains_key(node_ref) {
                    return Err(ReflectionError::DuplicateNodeRef(node_ref.clone()));
                }
                resolved_node_refs.insert(node_ref.clone(), created.id);
            }

            Ok(ReflectionApplyOutcome::Applied(ReflectionAppliedItemIds {
                created_node_id: Some(created.id),
                ..ReflectionAppliedItemIds::default()
            }))
        }
        ReflectionProposalItem::AddAlias {
            node_id,
            node_ref,
            alias,
            ..
        } => {
            let Some(target_id) =
                resolve_reflection_target(*node_id, node_ref.as_deref(), resolved_node_refs)
            else {
                return Ok(ReflectionApplyOutcome::Draft(draft_reason(
                    "unresolved_node_target",
                    node_ref.as_deref().unwrap_or("missing"),
                )));
            };
            let created = storage::create_alias_deferred_fts(
                connection,
                &storage::NewAlias {
                    node_id: target_id,
                    alias: alias.clone(),
                },
            )?;
            alias_node_ids.insert(created.node_id);
            Ok(ReflectionApplyOutcome::Applied(ReflectionAppliedItemIds {
                created_alias_id: Some(created.id),
                ..ReflectionAppliedItemIds::default()
            }))
        }
        ReflectionProposalItem::AddTag {
            node_id,
            node_ref,
            tag,
            ..
        } => {
            let Some(target_id) =
                resolve_reflection_target(*node_id, node_ref.as_deref(), resolved_node_refs)
            else {
                return Ok(ReflectionApplyOutcome::Draft(draft_reason(
                    "unresolved_node_target",
                    node_ref.as_deref().unwrap_or("missing"),
                )));
            };
            let created = storage::create_tag(
                connection,
                &storage::NewTag {
                    node_id: target_id,
                    tag: tag.clone(),
                },
            )?;
            Ok(ReflectionApplyOutcome::Applied(ReflectionAppliedItemIds {
                created_tag_id: Some(created.id),
                ..ReflectionAppliedItemIds::default()
            }))
        }
        ReflectionProposalItem::AddSource {
            node_id,
            node_ref,
            source_ref,
            ..
        } => {
            let Some(target_id) =
                resolve_reflection_target(*node_id, node_ref.as_deref(), resolved_node_refs)
            else {
                return Ok(ReflectionApplyOutcome::Draft(draft_reason(
                    "unresolved_node_target",
                    node_ref.as_deref().unwrap_or("missing"),
                )));
            };
            let created = storage::create_source(
                connection,
                &storage::NewSource {
                    node_id: target_id,
                    source_ref: source_ref.clone(),
                },
            )?;
            Ok(ReflectionApplyOutcome::Applied(ReflectionAppliedItemIds {
                created_source_id: Some(created.id),
                ..ReflectionAppliedItemIds::default()
            }))
        }
        ReflectionProposalItem::AddLink {
            source_node_id,
            source_node_ref,
            target_node_id,
            target_node_ref,
            link_type,
            ..
        } => {
            let Some(source_id) = resolve_reflection_target(
                *source_node_id,
                source_node_ref.as_deref(),
                resolved_node_refs,
            ) else {
                return Ok(ReflectionApplyOutcome::Draft(draft_reason(
                    "unresolved_source_target",
                    source_node_ref.as_deref().unwrap_or("missing"),
                )));
            };
            let Some(target_id) = resolve_reflection_target(
                *target_node_id,
                target_node_ref.as_deref(),
                resolved_node_refs,
            ) else {
                return Ok(ReflectionApplyOutcome::Draft(draft_reason(
                    "unresolved_target_target",
                    target_node_ref.as_deref().unwrap_or("missing"),
                )));
            };
            let created = storage::create_link(
                connection,
                &storage::NewLink {
                    source_node_id: source_id,
                    target_node_id: target_id,
                    link_type: link_type.clone(),
                },
            )?;
            Ok(ReflectionApplyOutcome::Applied(ReflectionAppliedItemIds {
                created_link_id: Some(created.id),
                ..ReflectionAppliedItemIds::default()
            }))
        }
        ReflectionProposalItem::UpdateNodeBody { .. }
        | ReflectionProposalItem::UpdateNodeStatus { .. }
        | ReflectionProposalItem::DeleteNode { .. } => {
            Ok(ReflectionApplyOutcome::Draft("high_risk_item".to_string()))
        }
    }
}

fn resolve_reflection_target(
    node_id: Option<i64>,
    node_ref: Option<&str>,
    resolved_node_refs: &HashMap<String, i64>,
) -> Option<i64> {
    match (node_id, node_ref) {
        (Some(node_id), None) => Some(node_id),
        (Some(node_id), Some(node_ref)) if node_ref.trim().is_empty() => Some(node_id),
        (None, Some(node_ref)) => resolved_node_refs.get(node_ref).copied(),
        _ => None,
    }
}

fn draft_reason(prefix: &str, value: &str) -> String {
    format!("{prefix}:{value}")
}

impl ReflectionProposalItem {
    fn risk(&self) -> ReflectionRisk {
        match self {
            Self::CreateNode { risk, .. }
            | Self::AddAlias { risk, .. }
            | Self::AddTag { risk, .. }
            | Self::AddSource { risk, .. }
            | Self::AddLink { risk, .. }
            | Self::UpdateNodeBody { risk, .. }
            | Self::UpdateNodeStatus { risk, .. }
            | Self::DeleteNode { risk, .. } => *risk,
        }
    }

    fn expected_risk(&self) -> Result<ReflectionRisk, ReflectionValidationError> {
        match self {
            Self::CreateNode {
                node_type, status, ..
            } => Ok(match node_type.as_str() {
                "correction"
                | "failure_mode"
                | "lesson"
                | "raw_note"
                | "reflection_observation" => ReflectionRisk::Low,
                "workflow" | "tool_contract" if status == "draft" => ReflectionRisk::Low,
                _ => ReflectionRisk::High,
            }),
            Self::AddAlias { .. }
            | Self::AddTag { .. }
            | Self::AddSource { .. }
            | Self::AddLink { .. } => Ok(ReflectionRisk::Low),
            Self::UpdateNodeBody { .. }
            | Self::UpdateNodeStatus { .. }
            | Self::DeleteNode { .. } => Ok(ReflectionRisk::High),
        }
    }
}

impl ReflectionRisk {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::High => "high",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reflection_events(connection: &Connection) -> Vec<audit::Event> {
        audit::list_events(connection)
            .expect("events should list")
            .into_iter()
            .filter(|event| event.event_type.starts_with("reflection."))
            .collect()
    }

    fn table_count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table};"), [], |row| {
                row.get(0)
            })
            .expect("table count should query")
    }

    fn duplicate_alias_failure_proposal() -> ReflectionProposalInput {
        ReflectionProposalInput {
            items: vec![
                ReflectionProposalItem::CreateNode {
                    risk: ReflectionRisk::Low,
                    node_ref: Some("failed_lesson".to_string()),
                    node_type: "lesson".to_string(),
                    status: "draft".to_string(),
                    title: "reflection_failure_fts_sentinel".to_string(),
                    summary: None,
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
                ReflectionProposalItem::AddAlias {
                    risk: ReflectionRisk::Low,
                    node_id: None,
                    node_ref: Some("failed_lesson".to_string()),
                    alias: "duplicate-reflection-alias".to_string(),
                },
                ReflectionProposalItem::AddAlias {
                    risk: ReflectionRisk::Low,
                    node_id: None,
                    node_ref: Some("failed_lesson".to_string()),
                    alias: "duplicate-reflection-alias".to_string(),
                },
            ],
        }
    }

    #[test]
    fn borrowed_reflection_records_preserve_exact_json() {
        let proposal_items = duplicate_alias_failure_proposal().items;
        let owned_proposal = ReflectionProposalRecord {
            session_id: "session-1".to_string(),
            items: proposal_items.clone(),
        };
        let borrowed_proposal = ReflectionProposalRecordRef {
            session_id: "session-1",
            items: &proposal_items,
        };
        assert_eq!(
            serde_json::to_vec(&borrowed_proposal).expect("borrowed proposal should serialize"),
            serde_json::to_vec(&owned_proposal).expect("owned proposal should serialize")
        );

        let applied_item_indexes = vec![0, 2];
        let draft_items = vec![ReflectionDraftItem {
            index: 1,
            reason: "high_risk_item".to_string(),
        }];
        let created_node_ids = vec![10, 11];
        let created_alias_ids = vec![20];
        let created_tag_ids = vec![30];
        let created_source_ids = vec![40];
        let created_link_ids = vec![50];
        let owned_apply = ReflectionApplyRecord {
            session_id: "session-1".to_string(),
            proposal_id: 7,
            applied_item_indexes: applied_item_indexes.clone(),
            draft_items: draft_items.clone(),
            created_node_ids: created_node_ids.clone(),
            created_alias_ids: created_alias_ids.clone(),
            created_tag_ids: created_tag_ids.clone(),
            created_source_ids: created_source_ids.clone(),
            created_link_ids: created_link_ids.clone(),
        };
        let borrowed_apply = ReflectionApplyRecordRef {
            session_id: "session-1",
            proposal_id: 7,
            applied_item_indexes: &applied_item_indexes,
            draft_items: &draft_items,
            created_node_ids: &created_node_ids,
            created_alias_ids: &created_alias_ids,
            created_tag_ids: &created_tag_ids,
            created_source_ids: &created_source_ids,
            created_link_ids: &created_link_ids,
        };
        assert_eq!(
            serde_json::to_vec(&borrowed_apply).expect("borrowed apply should serialize"),
            serde_json::to_vec(&owned_apply).expect("owned apply should serialize")
        );
    }

    #[test]
    fn inventory_sessions_records_empty_status_when_no_sessions_exist() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");

        let report = inventory_sessions(&connection).expect("inventory should succeed");
        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let inventory = nodes.last().expect("inventory node should exist");
        let record: ReflectionInventoryRecord = serde_json::from_str(
            inventory
                .body
                .as_deref()
                .expect("inventory body should exist"),
        )
        .expect("inventory body should parse");

        assert_eq!(report.inventory_status, InventoryStatus::Empty);
        assert!(report.reflected_session_ids.is_empty());
        assert!(report.sessions.is_empty());
        assert_eq!(
            inventory.summary.as_deref(),
            Some(REFLECTION_INVENTORY_SUMMARY)
        );
        assert_eq!(record.inventory_status, InventoryStatus::Empty);
        assert!(record.reflected_session_ids.is_empty());
    }

    #[test]
    fn inventory_create_update_and_noop_keep_one_current_node_and_exact_events() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");

        let first = inventory_sessions(&connection).expect("first inventory should succeed");
        let first_events = reflection_events(&connection);
        assert_eq!(first_events.len(), 1);
        assert_eq!(
            first_events[0].event_type,
            audit::REFLECTION_INVENTORY_CREATED_EVENT
        );
        assert_eq!(first_events[0].subject_id, first.inventory_id);

        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Sanitized reflection material".to_string(),
                summary: Some(REFLECTION_SANITIZED_MATERIAL_SUMMARY.to_string()),
                body: Some("{\"session_id\":\"changed-session\"}".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("reflection material should create");

        let second = inventory_sessions(&connection).expect("changed inventory should succeed");
        let second_events = reflection_events(&connection);
        assert_eq!(second.inventory_id, first.inventory_id);
        assert_eq!(second_events.len(), 2);
        assert_eq!(
            second_events[1].event_type,
            audit::REFLECTION_INVENTORY_UPDATED_EVENT
        );
        assert_eq!(second_events[1].subject_id, first.inventory_id);

        let third = inventory_sessions(&connection).expect("identical inventory should succeed");
        let third_events = reflection_events(&connection);
        let inventory_nodes = storage::list_nodes(&connection)
            .expect("nodes should list")
            .into_iter()
            .filter(|node| node.summary.as_deref() == Some(REFLECTION_INVENTORY_SUMMARY))
            .collect::<Vec<_>>();

        assert_eq!(third.inventory_id, first.inventory_id);
        assert_eq!(third.created_at, first.created_at);
        assert_eq!(third.inventory_status, second.inventory_status);
        assert_eq!(third.reflected_session_ids, second.reflected_session_ids);
        assert_eq!(third.sessions, second.sessions);
        assert_eq!(
            third_events, second_events,
            "identical inventory is a no-op"
        );
        assert_eq!(inventory_nodes.len(), 1);
        assert_eq!(inventory_nodes[0].id, first.inventory_id);
    }

    #[test]
    fn inventory_sessions_tracks_existing_reflection_session_ids() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let seeded_inventory = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Prior reflection inventory".to_string(),
                summary: Some(REFLECTION_INVENTORY_SUMMARY.to_string()),
                body: Some(
                    serde_json::to_string(&ReflectionInventoryRecord {
                        inventory_status: InventoryStatus::Tracked,
                        reflected_session_ids: vec![
                            "chat-alpha".to_string(),
                            "chat-beta".to_string(),
                        ],
                    })
                    .expect("record should serialize"),
                ),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("seed inventory should be created");
        let material = storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Reflection material".to_string(),
                summary: Some(REFLECTION_MATERIAL_SUMMARY.to_string()),
                body: Some(
                    serde_json::to_string(&ReflectionSessionRecord {
                        session_id: "chat-alpha".to_string(),
                    })
                    .expect("record should serialize"),
                ),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("seed material should be created");

        let report = inventory_sessions(&connection).expect("inventory should succeed");
        let alpha = report
            .sessions
            .iter()
            .find(|session| session.session_id == "chat-alpha")
            .expect("alpha session should exist");
        assert_eq!(report.inventory_status, InventoryStatus::Tracked);
        assert_eq!(report.inventory_id, seeded_inventory.id);
        assert_eq!(report.reflected_session_ids, vec!["chat-alpha".to_string()]);
        assert_eq!(alpha.source_node_ids, vec![material.id]);
        assert_eq!(
            storage::list_nodes(&connection)
                .expect("nodes should list")
                .into_iter()
                .filter(|node| node.summary.as_deref() == Some(REFLECTION_INVENTORY_SUMMARY))
                .count(),
            1
        );
    }

    #[test]
    fn legacy_inventory_history_is_preserved_without_multiplying_current_node() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let legacy_body = serde_json::to_string(&ReflectionInventoryRecord {
            inventory_status: InventoryStatus::Empty,
            reflected_session_ids: Vec::new(),
        })
        .expect("legacy inventory should serialize");
        let first_legacy = create_inventory_snapshot(&connection, legacy_body.clone())
            .expect("first legacy inventory should create");
        let current = create_inventory_snapshot(&connection, legacy_body)
            .expect("current legacy inventory should create");
        storage::create_node(
            &connection,
            &storage::NewNode {
                node_type: "raw_note".to_string(),
                status: "draft".to_string(),
                title: "Sanitized legacy material".to_string(),
                summary: Some(REFLECTION_SANITIZED_MATERIAL_SUMMARY.to_string()),
                body: Some("{\"session_id\":\"legacy-session\"}".to_string()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            },
        )
        .expect("legacy material should create");

        let updated = inventory_sessions(&connection).expect("legacy inventory should update");
        let unchanged = inventory_sessions(&connection).expect("repeat should be a no-op");
        let inventory_nodes =
            storage::list_nodes_with_summaries(&connection, &[REFLECTION_INVENTORY_SUMMARY])
                .expect("inventory history should list");
        let events = reflection_events(&connection);

        assert_eq!(updated.inventory_id, current.id);
        assert_eq!(unchanged.inventory_id, current.id);
        assert_eq!(inventory_nodes.len(), 2, "legacy history must be preserved");
        assert_eq!(inventory_nodes[0].id, first_legacy.id);
        assert_eq!(inventory_nodes[1].id, current.id);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event_type,
            audit::REFLECTION_INVENTORY_UPDATED_EVENT
        );
        assert_eq!(events[0].subject_id, current.id);
    }

    #[test]
    fn store_proposal_accepts_low_and_high_risk_items() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let proposal = ReflectionProposalInput {
            items: vec![
                ReflectionProposalItem::CreateNode {
                    risk: ReflectionRisk::Low,
                    node_ref: Some("lesson_1".to_string()),
                    node_type: "lesson".to_string(),
                    status: "draft".to_string(),
                    title: "Record release lesson".to_string(),
                    summary: Some("Keep rollback steps visible".to_string()),
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
                ReflectionProposalItem::UpdateNodeBody {
                    risk: ReflectionRisk::High,
                    node_id: 7,
                    body: "rewrite workflow body".to_string(),
                },
            ],
        };

        let report =
            store_proposal(&connection, "codex-chat-42", &proposal).expect("proposal should store");
        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let proposal_node = nodes.last().expect("proposal node should exist");
        let record: ReflectionProposalRecord = serde_json::from_str(
            proposal_node
                .body
                .as_deref()
                .expect("proposal body should exist"),
        )
        .expect("proposal body should parse");
        let sessions =
            list_reflected_sessions(&connection).expect("reflected sessions should list");

        assert_eq!(report.session_id, "codex-chat-42");
        assert_eq!(report.items, proposal.items);
        assert_eq!(
            proposal_node.summary.as_deref(),
            Some(REFLECTION_PROPOSAL_SUMMARY)
        );
        assert_eq!(record.session_id, "codex-chat-42");
        assert_eq!(record.items, proposal.items);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "codex-chat-42");
        let events = reflection_events(&connection);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event_type,
            audit::REFLECTION_PROPOSAL_CREATED_EVENT
        );
        assert_eq!(events[0].subject_id, report.proposal_id);
    }

    #[test]
    fn nested_inventory_and_proposal_roll_back_when_their_reflection_event_fails() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        connection
            .execute_batch(
                "
                CREATE TRIGGER fail_reflection_lifecycle_event
                BEFORE INSERT ON events
                WHEN NEW.type IN (
                    'reflection.inventory.created',
                    'reflection.proposal.created'
                )
                BEGIN
                    SELECT RAISE(ABORT, 'forced reflection lifecycle event failure');
                END;
                ",
            )
            .expect("failure trigger should create");

        let proposal = ReflectionProposalInput {
            items: vec![ReflectionProposalItem::CreateNode {
                risk: ReflectionRisk::Low,
                node_ref: None,
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "Atomic proposal".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };

        let transaction = connection
            .unchecked_transaction()
            .expect("outer transaction should start");
        inventory_sessions(&transaction).expect_err("inventory event failure must roll back");
        store_proposal(&transaction, "atomic-session", &proposal)
            .expect_err("proposal event failure must roll back");
        transaction
            .commit()
            .expect("caller commit must preserve neither failed operation");

        assert_eq!(table_count(&connection, "nodes"), 0);
        assert_eq!(table_count(&connection, "events"), 0);
    }

    #[test]
    fn store_proposal_rejects_mismatched_risk_types() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let proposal = ReflectionProposalInput {
            items: vec![ReflectionProposalItem::CreateNode {
                risk: ReflectionRisk::Low,
                node_ref: None,
                node_type: "kernel_contract".to_string(),
                status: "draft".to_string(),
                title: "Kernel rewrite".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };

        let error = store_proposal(&connection, "codex-chat-42", &proposal)
            .expect_err("proposal should fail");

        assert!(matches!(
            error,
            ReflectionError::Validation(ReflectionValidationError::InvalidRisk {
                index: 0,
                expected: "high",
                actual: "low",
            })
        ));
    }

    #[test]
    fn reflection_item_slice_validation_matches_wrapper_and_exact_error() {
        let valid = ReflectionProposalInput {
            items: vec![ReflectionProposalItem::CreateNode {
                risk: ReflectionRisk::Low,
                node_ref: Some("lesson".to_string()),
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "Validated lesson".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };
        assert_eq!(
            validate_reflection_items(&valid.items),
            validate_reflection_proposal(&valid)
        );

        let invalid = ReflectionProposalInput {
            items: vec![ReflectionProposalItem::CreateNode {
                risk: ReflectionRisk::Low,
                node_ref: Some(" ".to_string()),
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "Invalid lesson".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };
        let slice_error = validate_reflection_items(&invalid.items)
            .expect_err("slice validation should reject an empty node_ref");
        let wrapper_error = validate_reflection_proposal(&invalid)
            .expect_err("wrapper validation should reject an empty node_ref");

        assert_eq!(slice_error, wrapper_error);
        assert_eq!(
            slice_error.to_string(),
            "reflection proposal item 0 has an empty node_ref"
        );
    }

    #[test]
    fn reflection_proposal_rejects_excess_items_and_long_node_refs() {
        let item = ReflectionProposalItem::CreateNode {
            risk: ReflectionRisk::Low,
            node_ref: Some("lesson".to_string()),
            node_type: "lesson".to_string(),
            status: "draft".to_string(),
            title: "Bounded lesson".to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
        };
        let too_many = ReflectionProposalInput {
            items: vec![item.clone(); storage::MAX_PROPOSAL_ITEMS + 1],
        };
        let long_ref = ReflectionProposalInput {
            items: vec![ReflectionProposalItem::CreateNode {
                risk: ReflectionRisk::Low,
                node_ref: Some("n".repeat(storage::MAX_PROPOSAL_NODE_REF_BYTES + 1)),
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "Bounded lesson".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };

        assert!(matches!(
            validate_reflection_proposal(&too_many),
            Err(ReflectionValidationError::TooManyProposalItems {
                max_items: storage::MAX_PROPOSAL_ITEMS,
                actual,
            }) if actual == storage::MAX_PROPOSAL_ITEMS + 1
        ));
        assert!(matches!(
            validate_reflection_proposal(&long_ref),
            Err(ReflectionValidationError::NodeRefTooLong {
                index: 0,
                max_bytes: storage::MAX_PROPOSAL_NODE_REF_BYTES,
            })
        ));
    }

    #[test]
    fn apply_proposal_auto_applies_low_risk_and_keeps_high_risk_draft() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let proposal = ReflectionProposalInput {
            items: vec![
                ReflectionProposalItem::CreateNode {
                    risk: ReflectionRisk::Low,
                    node_ref: Some("lesson_1".to_string()),
                    node_type: "lesson".to_string(),
                    status: "draft".to_string(),
                    title: "Record lesson".to_string(),
                    summary: Some("Keep the recovery plan".to_string()),
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
                ReflectionProposalItem::AddAlias {
                    risk: ReflectionRisk::Low,
                    node_id: None,
                    node_ref: Some("lesson_1".to_string()),
                    alias: "recovery-note".to_string(),
                },
                ReflectionProposalItem::UpdateNodeBody {
                    risk: ReflectionRisk::High,
                    node_id: 42,
                    body: "rewrite active workflow".to_string(),
                },
            ],
        };
        let stored =
            store_proposal(&connection, "codex-chat-43", &proposal).expect("proposal should store");

        let report = apply_proposal(&connection, stored.proposal_id).expect("apply should succeed");
        let nodes = storage::list_nodes(&connection).expect("nodes should list");
        let created_node = nodes
            .iter()
            .find(|node| node.title == "Record lesson")
            .expect("low-risk node should be created");
        let aliases =
            storage::list_aliases(&connection, Some(created_node.id)).expect("aliases should list");
        let apply_node = nodes.last().expect("apply record should exist");
        let apply_record: ReflectionApplyRecord =
            serde_json::from_str(apply_node.body.as_deref().expect("apply body should exist"))
                .expect("apply body should parse");

        assert_eq!(report.session_id, "codex-chat-43");
        assert_eq!(report.applied_item_indexes, vec![0, 1]);
        assert_eq!(
            report.draft_items,
            vec![ReflectionDraftItem {
                index: 2,
                reason: "high_risk_item".to_string(),
            }]
        );
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].alias, "recovery-note");
        assert_eq!(
            apply_node.summary.as_deref(),
            Some(REFLECTION_APPLY_SUMMARY)
        );
        assert_eq!(apply_record.proposal_id, stored.proposal_id);
        assert_eq!(apply_record.applied_item_indexes, vec![0, 1]);
        assert_ne!(report.apply_id, stored.proposal_id);
        assert_eq!(apply_node.id, report.apply_id);
        assert_eq!(
            reflection_events(&connection)
                .into_iter()
                .map(|event| (event.event_type, event.subject_id))
                .collect::<Vec<_>>(),
            vec![
                (
                    audit::REFLECTION_PROPOSAL_CREATED_EVENT.to_string(),
                    stored.proposal_id,
                ),
                (
                    audit::REFLECTION_PROPOSAL_APPLIED_EVENT.to_string(),
                    stored.proposal_id,
                ),
                (
                    audit::REFLECTION_PROPOSAL_DRAFTED_EVENT.to_string(),
                    stored.proposal_id,
                ),
            ]
        );
    }

    #[test]
    fn apply_proposal_keeps_dependent_low_risk_items_draft_when_ref_is_unresolved() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let proposal = ReflectionProposalInput {
            items: vec![
                ReflectionProposalItem::CreateNode {
                    risk: ReflectionRisk::High,
                    node_ref: Some("kernel_1".to_string()),
                    node_type: "kernel_contract".to_string(),
                    status: "draft".to_string(),
                    title: "Rewrite kernel".to_string(),
                    summary: None,
                    body: None,
                    source_ref: None,
                    confidence: None,
                    trust_level: None,
                },
                ReflectionProposalItem::AddTag {
                    risk: ReflectionRisk::Low,
                    node_id: None,
                    node_ref: Some("kernel_1".to_string()),
                    tag: "needs-audit".to_string(),
                },
            ],
        };
        let stored =
            store_proposal(&connection, "codex-chat-44", &proposal).expect("proposal should store");

        let report = apply_proposal(&connection, stored.proposal_id).expect("apply should succeed");
        let tags = storage::list_tags(&connection, None).expect("tags should list");

        assert!(report.applied_item_indexes.is_empty());
        assert_eq!(tags.len(), 0);
        assert_eq!(
            report.draft_items,
            vec![
                ReflectionDraftItem {
                    index: 0,
                    reason: "high_risk_item".to_string(),
                },
                ReflectionDraftItem {
                    index: 1,
                    reason: "unresolved_node_target:kernel_1".to_string(),
                },
            ]
        );
    }

    #[test]
    fn failed_apply_attempt_rolls_back_effects_and_commits_only_failure_event() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let stored = store_proposal(
            &connection,
            "failed-apply-session",
            &duplicate_alias_failure_proposal(),
        )
        .expect("failure proposal should store");
        let baseline = ["nodes", "aliases", "tags", "sources", "links"]
            .map(|table| table_count(&connection, table));
        let baseline_events = table_count(&connection, "events");
        let baseline_fts_matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM fts_nodes WHERE fts_nodes MATCH 'reflection_failure_fts_sentinel';",
                [],
                |row| row.get(0),
            )
            .expect("baseline FTS proof should query");

        let transaction = connection
            .unchecked_transaction()
            .expect("outer transaction should start");
        let attempt = attempt_apply_proposal(&transaction, stored.proposal_id)
            .expect("normal apply failure should become a durable attempt");
        match attempt {
            ReflectionApplyAttempt::Failed { error } => {
                assert!(
                    error.to_string().contains("UNIQUE constraint failed"),
                    "unexpected failed-attempt error: {error}"
                );
            }
            ReflectionApplyAttempt::Applied(_) => panic!("duplicate alias must not apply"),
        }
        transaction
            .commit()
            .expect("failure event transaction should commit");

        let after = ["nodes", "aliases", "tags", "sources", "links"]
            .map(|table| table_count(&connection, table));
        let failed_events = reflection_events(&connection);
        let fts_matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM fts_nodes WHERE fts_nodes MATCH 'reflection_failure_fts_sentinel';",
                [],
                |row| row.get(0),
            )
            .expect("FTS rollback proof should query");

        assert_eq!(after, baseline);
        assert_eq!(table_count(&connection, "events"), baseline_events + 1);
        assert_eq!(fts_matches, baseline_fts_matches);
        assert!(storage::list_nodes(&connection)
            .expect("nodes should list")
            .iter()
            .all(|node| node.summary.as_deref() != Some(REFLECTION_APPLY_SUMMARY)));
        assert_eq!(
            failed_events
                .into_iter()
                .map(|event| (event.event_type, event.subject_id))
                .collect::<Vec<_>>(),
            vec![
                (
                    audit::REFLECTION_PROPOSAL_CREATED_EVENT.to_string(),
                    stored.proposal_id,
                ),
                (
                    audit::REFLECTION_APPLY_FAILED_EVENT.to_string(),
                    stored.proposal_id,
                ),
            ]
        );
    }

    #[test]
    fn failure_event_insert_error_rolls_back_outer_transaction_without_false_history() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let stored = store_proposal(
            &connection,
            "failed-event-session",
            &duplicate_alias_failure_proposal(),
        )
        .expect("failure proposal should store");
        connection
            .execute_batch(
                "
                CREATE TRIGGER fail_reflection_apply_failed_event
                BEFORE INSERT ON events
                WHEN NEW.type = 'reflection.apply.failed'
                BEGIN
                    SELECT RAISE(ABORT, 'forced reflection apply.failed event failure');
                END;
                ",
            )
            .expect("failure-event trigger should create");
        let baseline = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .map(|table| table_count(&connection, table));

        let transaction = connection
            .unchecked_transaction()
            .expect("outer transaction should start");
        let error = attempt_apply_proposal(&transaction, stored.proposal_id)
            .expect_err("failure-event insert must abort the outer mutation");
        assert!(matches!(
            error,
            ReflectionError::Audit(audit::AuditError::Db(_))
        ));
        transaction
            .rollback()
            .expect("outer transaction should fully roll back");

        let after = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .map(|table| table_count(&connection, table));
        assert_eq!(after, baseline);
        assert!(reflection_events(&connection)
            .iter()
            .all(|event| event.event_type != audit::REFLECTION_APPLY_FAILED_EVENT));
    }

    #[test]
    fn injected_savepoint_rollback_failure_requires_full_outer_rollback() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let stored = store_proposal(
            &connection,
            "rollback-failure-session",
            &duplicate_alias_failure_proposal(),
        )
        .expect("failure proposal should store");
        let baseline = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .map(|table| table_count(&connection, table));

        let transaction = connection
            .unchecked_transaction()
            .expect("outer transaction should start");
        inject_reflection_savepoint_failure(ReflectionSavepointFailure::Rollback);
        let error = attempt_apply_proposal(&transaction, stored.proposal_id)
            .expect_err("injected savepoint rollback failure must abort");
        assert!(matches!(
            error,
            ReflectionError::Db(rusqlite::Error::InvalidQuery)
        ));
        transaction
            .rollback()
            .expect("outer transaction should recover all effects");

        let after = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .map(|table| table_count(&connection, table));
        assert_eq!(after, baseline);
        assert!(reflection_events(&connection)
            .iter()
            .all(|event| event.event_type != audit::REFLECTION_APPLY_FAILED_EVENT));
    }

    #[test]
    fn injected_savepoint_release_failure_requires_full_outer_rollback() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let proposal = ReflectionProposalInput {
            items: vec![ReflectionProposalItem::CreateNode {
                risk: ReflectionRisk::Low,
                node_ref: None,
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "release_failure_sentinel".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };
        let stored = store_proposal(&connection, "release-failure-session", &proposal)
            .expect("release failure proposal should store");
        let baseline = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .map(|table| table_count(&connection, table));

        let transaction = connection
            .unchecked_transaction()
            .expect("outer transaction should start");
        inject_reflection_savepoint_failure(ReflectionSavepointFailure::Release);
        let error = attempt_apply_proposal(&transaction, stored.proposal_id)
            .expect_err("injected savepoint release failure must abort");
        assert!(matches!(
            error,
            ReflectionError::Db(rusqlite::Error::InvalidQuery)
        ));
        transaction
            .rollback()
            .expect("outer transaction should recover all effects");

        let after = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .map(|table| table_count(&connection, table));
        assert_eq!(after, baseline);
        assert!(storage::list_nodes(&connection)
            .expect("nodes should list")
            .iter()
            .all(|node| node.title != "release_failure_sentinel"));
    }

    #[test]
    fn late_applied_event_failure_rolls_back_public_apply_savepoint() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let proposal = ReflectionProposalInput {
            items: vec![ReflectionProposalItem::CreateNode {
                risk: ReflectionRisk::Low,
                node_ref: None,
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "late_event_failure_sentinel".to_string(),
                summary: None,
                body: None,
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };
        let stored = store_proposal(&connection, "late-event-session", &proposal)
            .expect("late event proposal should store");
        connection
            .execute_batch(
                "
                CREATE TRIGGER fail_reflection_applied_event
                BEFORE INSERT ON events
                WHEN NEW.type = 'reflection.proposal.applied'
                BEGIN
                    SELECT RAISE(ABORT, 'forced proposal.applied event failure');
                END;
                ",
            )
            .expect("late event trigger should create");
        let baseline = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .map(|table| table_count(&connection, table));

        connection
            .execute_batch("BEGIN IMMEDIATE;")
            .expect("outer transaction should begin");
        apply_proposal(&connection, stored.proposal_id)
            .expect_err("late applied event must fail the savepoint");
        connection
            .execute_batch("COMMIT;")
            .expect("caller commit must not preserve failed apply effects");

        let after = ["nodes", "aliases", "tags", "sources", "links", "events"]
            .map(|table| table_count(&connection, table));
        assert_eq!(after, baseline);
        assert!(storage::list_nodes(&connection)
            .expect("nodes should list")
            .iter()
            .all(|node| node.title != "late_event_failure_sentinel"));
    }

    #[test]
    fn explicit_memory_body_is_not_copied_into_inventory_receipt_or_events() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        let explicit_body = "EXPLICIT_USER_SELECTED_MEMORY_BODY_SENTINEL".to_string();
        let proposal = ReflectionProposalInput {
            items: vec![ReflectionProposalItem::CreateNode {
                risk: ReflectionRisk::Low,
                node_ref: None,
                node_type: "lesson".to_string(),
                status: "draft".to_string(),
                title: "Explicit user payload".to_string(),
                summary: None,
                body: Some(explicit_body.clone()),
                source_ref: None,
                confidence: None,
                trust_level: None,
            }],
        };
        let stored = store_proposal(&connection, "privacy-session", &proposal)
            .expect("explicit proposal should store");
        let report = apply_proposal(&connection, stored.proposal_id)
            .expect("explicit proposal should apply");
        let inventory = inventory_sessions(&connection).expect("inventory should store");
        let proposal_node = storage::get_node(&connection, stored.proposal_id)
            .expect("proposal should query")
            .expect("proposal should exist");
        let receipt = storage::get_node(&connection, report.apply_id)
            .expect("receipt should query")
            .expect("receipt should exist");
        let inventory_node = storage::get_node(&connection, inventory.inventory_id)
            .expect("inventory should query")
            .expect("inventory should exist");
        let control_records = format!(
            "{}\n{}\n{}",
            receipt.body.as_deref().unwrap_or_default(),
            inventory_node.body.as_deref().unwrap_or_default(),
            serde_json::to_string(&reflection_events(&connection))
                .expect("events should serialize")
        );

        assert!(proposal_node
            .body
            .as_deref()
            .expect("explicit proposal body should exist")
            .contains(&explicit_body));
        let applied_node = storage::get_node(
            &connection,
            *report
                .created_node_ids
                .first()
                .expect("apply should create the explicit memory node"),
        )
        .expect("applied memory node should query")
        .expect("applied memory node should exist");
        assert_eq!(applied_node.body.as_deref(), Some(explicit_body.as_str()));
        assert!(
            !control_records.contains(&explicit_body),
            "inventory, receipt, and events must not copy explicit node bodies"
        );
    }
}
