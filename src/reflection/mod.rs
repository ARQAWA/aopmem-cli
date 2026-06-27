use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::storage;

pub const REFLECTION_INVENTORY_SUMMARY: &str = "reflection_inventory_v1";
pub const REFLECTION_MATERIAL_SUMMARY: &str = "reflection_material_v1";
pub const REFLECTION_SANITIZED_MATERIAL_SUMMARY: &str = "reflection_sanitized_material_v1";
pub const REFLECTION_PROPOSAL_SUMMARY: &str = "reflection_proposal_v1";
pub const REFLECTION_APPLY_SUMMARY: &str = "reflection_apply_v1";

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

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReflectionValidationError {
    #[error("reflection proposal must contain at least one item")]
    EmptyProposal,
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
    #[error("reflection session id cannot be empty")]
    EmptySessionId,
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
    let inventory_node = storage::create_node(
        connection,
        &storage::NewNode {
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: "Reflection inventory".to_string(),
            summary: Some(REFLECTION_INVENTORY_SUMMARY.to_string()),
            body: Some(serde_json::to_string(&record)?),
            source_ref: None,
            confidence: None,
            trust_level: None,
        },
    )?;

    Ok(ReflectionInventoryReport {
        inventory_id: inventory_node.id,
        inventory_status,
        reflected_session_ids,
        sessions,
        created_at: inventory_node.created_at,
    })
}

pub fn store_proposal(
    connection: &Connection,
    session_id: &str,
    proposal: &ReflectionProposalInput,
) -> Result<ReflectionProposal, ReflectionError> {
    if session_id.trim().is_empty() {
        return Err(ReflectionError::EmptySessionId);
    }

    validate_reflection_proposal(proposal)?;

    let record = ReflectionProposalRecord {
        session_id: session_id.to_string(),
        items: proposal.items.clone(),
    };
    let proposal_node = storage::create_node(
        connection,
        &storage::NewNode {
            node_type: "raw_note".to_string(),
            status: "draft".to_string(),
            title: format!("Reflection proposal {session_id}"),
            summary: Some(REFLECTION_PROPOSAL_SUMMARY.to_string()),
            body: Some(serde_json::to_string(&record)?),
            source_ref: None,
            confidence: None,
            trust_level: None,
        },
    )?;

    Ok(ReflectionProposal {
        proposal_id: proposal_node.id,
        session_id: session_id.to_string(),
        items: proposal.items.clone(),
        created_at: proposal_node.created_at,
    })
}

pub fn apply_proposal(
    connection: &Connection,
    proposal_id: i64,
) -> Result<ReflectionApplyReport, ReflectionError> {
    let proposal = load_proposal(connection, proposal_id)?;
    validate_reflection_proposal(&ReflectionProposalInput {
        items: proposal.items.clone(),
    })?;

    let mut resolved_node_refs = BTreeMap::<String, i64>::new();
    let mut applied_item_indexes = Vec::new();
    let mut draft_items = Vec::new();
    let mut created_node_ids = Vec::new();
    let mut created_alias_ids = Vec::new();
    let mut created_tag_ids = Vec::new();
    let mut created_source_ids = Vec::new();
    let mut created_link_ids = Vec::new();

    for (index, item) in proposal.items.iter().enumerate() {
        if item.risk() == ReflectionRisk::High {
            draft_items.push(ReflectionDraftItem {
                index,
                reason: "high_risk_item".to_string(),
            });
            continue;
        }

        let outcome = apply_low_risk_item(connection, item, &mut resolved_node_refs)?;
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

    let record = ReflectionApplyRecord {
        session_id: proposal.session_id.clone(),
        proposal_id,
        applied_item_indexes: applied_item_indexes.clone(),
        draft_items: draft_items.clone(),
        created_node_ids: created_node_ids.clone(),
        created_alias_ids: created_alias_ids.clone(),
        created_tag_ids: created_tag_ids.clone(),
        created_source_ids: created_source_ids.clone(),
        created_link_ids: created_link_ids.clone(),
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
    let nodes = storage::list_nodes(connection)?;
    let mut tracked = BTreeMap::<String, Vec<i64>>::new();

    for node in nodes {
        let Some(summary) = node.summary.as_deref() else {
            continue;
        };
        let Some(body) = node.body.as_deref() else {
            continue;
        };

        match summary {
            REFLECTION_INVENTORY_SUMMARY => {
                let record: ReflectionInventoryRecord = serde_json::from_str(body)?;
                for session_id in record.reflected_session_ids {
                    insert_tracked_session(&mut tracked, session_id, node.id)?;
                }
            }
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
    if proposal.items.is_empty() {
        return Err(ReflectionValidationError::EmptyProposal);
    }

    for (index, item) in proposal.items.iter().enumerate() {
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
    resolved_node_refs: &mut BTreeMap<String, i64>,
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
            let created = storage::create_node(
                connection,
                &storage::NewNode {
                    node_type: node_type.clone(),
                    status: status.clone(),
                    title: title.clone(),
                    summary: summary.clone(),
                    body: body.clone(),
                    source_ref: source_ref.clone(),
                    confidence: *confidence,
                    trust_level: trust_level.clone(),
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
            let created = storage::create_alias(
                connection,
                &storage::NewAlias {
                    node_id: target_id,
                    alias: alias.clone(),
                },
            )?;
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
    resolved_node_refs: &BTreeMap<String, i64>,
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
    fn inventory_sessions_tracks_existing_reflection_session_ids() {
        let mut connection = Connection::open_in_memory().expect("in-memory DB should open");
        crate::schema::apply_migrations(&mut connection).expect("migrations should apply");
        storage::create_node(
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
        let beta = report
            .sessions
            .iter()
            .find(|session| session.session_id == "chat-beta")
            .expect("beta session should exist");

        assert_eq!(report.inventory_status, InventoryStatus::Tracked);
        assert_eq!(
            report.reflected_session_ids,
            vec!["chat-alpha".to_string(), "chat-beta".to_string()]
        );
        assert_eq!(alpha.source_node_ids.len(), 2);
        assert!(alpha.source_node_ids.contains(&material.id));
        assert_eq!(beta.source_node_ids.len(), 1);
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
}
