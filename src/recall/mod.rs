use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

use serde::Serialize;

use crate::storage::{FtsNodeSearchResult, LeastPrivilegeMetadata, Link, Node, SourceHierarchy};

const RECALL_TRAVERSAL_MAX_DEPTH: usize = 2;
const STRUCTURED_RECALL_SUFFICIENT_NODE_COUNT: usize = 3;
const MAX_HUNCHES: usize = 3;
const MAX_COMPACT_APPLICABLE_WORKFLOWS: usize = 1;
const MAX_COMPACT_SECTION_NODES: usize = 3;
const MAX_COMPACT_SOURCE_REFS: usize = 12;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructuredRecallBundle {
    pub project_profiles: RecallNodesByStatus,
    pub gates: RecallNodesByStatus,
    pub workflows: RecallNodesByStatus,
    pub linked_nodes: Vec<RecallLinkedNode>,
    pub fts_fallback: Vec<FtsNodeSearchResult>,
    pub hunches: Vec<RecallHunch>,
    pub compact: CompactRecallBundle,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RecallNodesByStatus {
    pub draft: Vec<Node>,
    pub active: Vec<Node>,
    pub deprecated: Vec<Node>,
    pub superseded: Vec<Node>,
    pub broken: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecallLinkedNode {
    pub depth: usize,
    pub source_node_id: i64,
    pub link_type: String,
    pub node: Node,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecallHunch {
    pub source_node_id: i64,
    pub source_node_type: String,
    pub linked_signal_node_id: Option<i64>,
    pub linked_signal_node_type: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub reason: String,
    pub source_updated_at: String,
    pub source_hierarchy: Option<SourceHierarchy>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CompactRecallBundle {
    pub applicable_workflows: Vec<CompactNodeRef>,
    pub active_gates: Vec<CompactNodeRef>,
    pub tool_contracts: Vec<CompactNodeRef>,
    pub mcp_profiles: Vec<CompactNodeRef>,
    pub project_profile_facts: Vec<CompactNodeRef>,
    pub relevant_corrections_lessons: Vec<CompactNodeRef>,
    pub hunches: Vec<CompactHunch>,
    pub source_refs: Vec<CompactSourceRef>,
    pub limits: CompactRecallLimits,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompactNodeRef {
    pub node_id: i64,
    pub node_type: String,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
    pub source_hierarchy: Option<SourceHierarchy>,
    pub least_privilege: Option<LeastPrivilegeMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompactHunch {
    pub source_node_id: i64,
    pub title: String,
    pub reason: String,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
    pub source_hierarchy: Option<SourceHierarchy>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompactSourceRef {
    pub node_id: i64,
    pub source_ref: Option<String>,
    pub confidence: Option<f64>,
    pub trust_level: Option<String>,
    pub source_hierarchy: Option<SourceHierarchy>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompactRecallLimits {
    pub applicable_workflows: usize,
    pub active_gates: usize,
    pub section_nodes: usize,
    pub hunches: usize,
    pub source_refs: usize,
}

impl Default for CompactRecallLimits {
    fn default() -> Self {
        Self {
            applicable_workflows: MAX_COMPACT_APPLICABLE_WORKFLOWS,
            active_gates: MAX_COMPACT_SECTION_NODES,
            section_nodes: MAX_COMPACT_SECTION_NODES,
            hunches: MAX_HUNCHES,
            source_refs: MAX_COMPACT_SOURCE_REFS,
        }
    }
}

#[derive(Debug, Clone)]
struct HunchLinkedSignal {
    node_id: i64,
    node_type: String,
    priority: u8,
}

pub fn build_structured_bundle(nodes: Vec<Node>) -> StructuredRecallBundle {
    build_structured_bundle_with_links(nodes, Vec::new())
}

pub fn build_structured_bundle_with_links(
    nodes: Vec<Node>,
    links: Vec<Link>,
) -> StructuredRecallBundle {
    let mut bundle = StructuredRecallBundle {
        project_profiles: RecallNodesByStatus::default(),
        gates: RecallNodesByStatus::default(),
        workflows: RecallNodesByStatus::default(),
        linked_nodes: traverse_links(&nodes, &links),
        fts_fallback: Vec::new(),
        hunches: Vec::new(),
        compact: CompactRecallBundle::default(),
    };

    for node in nodes {
        if should_exclude_from_normal_recall(&node) {
            continue;
        }

        match node.node_type.as_str() {
            "project_profile" => bundle.project_profiles.push(node),
            "gate" => bundle.gates.push(node),
            "workflow" => bundle.workflows.push(node),
            _ => {}
        }
    }

    bundle.compact = build_compact_bundle(&bundle);
    bundle
}

pub fn needs_fts_fallback(bundle: &StructuredRecallBundle) -> bool {
    structured_node_count(bundle) < STRUCTURED_RECALL_SUFFICIENT_NODE_COUNT
}

pub fn derive_fts_fallback_query(bundle: &StructuredRecallBundle) -> Option<String> {
    let mut titles = Vec::new();
    collect_status_titles(&mut titles, &bundle.project_profiles);
    collect_status_titles(&mut titles, &bundle.gates);
    collect_status_titles(&mut titles, &bundle.workflows);
    titles.extend(
        bundle
            .linked_nodes
            .iter()
            .map(|linked| linked.node.title.as_str()),
    );

    titles
        .into_iter()
        .map(str::trim)
        .find(|title| !title.is_empty())
        .map(ToOwned::to_owned)
}

pub fn add_fts_fallback(
    mut bundle: StructuredRecallBundle,
    results: Vec<FtsNodeSearchResult>,
) -> StructuredRecallBundle {
    let existing_ids = structured_node_ids(&bundle);
    bundle.fts_fallback = results
        .into_iter()
        .filter(|result| !should_exclude_from_normal_recall(&result.node))
        .filter(|result| !existing_ids.contains(&result.node.id))
        .collect();
    bundle
        .fts_fallback
        .sort_by(|left, right| compare_nodes_for_recall_priority(&left.node, &right.node));
    bundle.hunches = select_hunches(&bundle);
    bundle.compact = build_compact_bundle(&bundle);
    bundle
}

fn build_compact_bundle(bundle: &StructuredRecallBundle) -> CompactRecallBundle {
    let mut compact = CompactRecallBundle {
        applicable_workflows: collect_compact_nodes(
            bundle_nodes_by_type(bundle, "workflow"),
            MAX_COMPACT_APPLICABLE_WORKFLOWS,
        ),
        active_gates: collect_compact_nodes(
            active_status_nodes(&bundle.gates),
            MAX_COMPACT_SECTION_NODES,
        ),
        tool_contracts: collect_compact_nodes(
            bundle_nodes_by_type(bundle, "tool_contract"),
            MAX_COMPACT_SECTION_NODES,
        ),
        mcp_profiles: collect_compact_nodes(
            bundle_nodes_by_type(bundle, "mcp_profile"),
            MAX_COMPACT_SECTION_NODES,
        ),
        project_profile_facts: collect_compact_nodes(
            active_status_nodes(&bundle.project_profiles),
            MAX_COMPACT_SECTION_NODES,
        ),
        relevant_corrections_lessons: collect_compact_nodes(
            bundle_nodes_by_types(bundle, &["correction", "lesson"]),
            MAX_COMPACT_SECTION_NODES,
        ),
        hunches: collect_compact_hunches(bundle, MAX_HUNCHES),
        source_refs: Vec::new(),
        limits: CompactRecallLimits::default(),
    };

    compact.source_refs = collect_compact_source_refs(&compact, MAX_COMPACT_SOURCE_REFS);
    compact.limits = CompactRecallLimits {
        applicable_workflows: MAX_COMPACT_APPLICABLE_WORKFLOWS,
        active_gates: MAX_COMPACT_SECTION_NODES,
        section_nodes: MAX_COMPACT_SECTION_NODES,
        hunches: MAX_HUNCHES,
        source_refs: MAX_COMPACT_SOURCE_REFS,
    };
    compact
}

fn collect_compact_nodes(nodes: Vec<&Node>, limit: usize) -> Vec<CompactNodeRef> {
    let mut nodes = nodes;
    nodes.sort_by(|left, right| compare_nodes_for_recall_priority(left, right));

    nodes
        .into_iter()
        .take(limit)
        .map(|node| CompactNodeRef {
            node_id: node.id,
            node_type: node.node_type.clone(),
            status: node.status.clone(),
            title: node.title.clone(),
            summary: node.summary.clone(),
            source_ref: node.source_ref.clone(),
            confidence: node.confidence,
            trust_level: node.trust_level.clone(),
            source_hierarchy: node.source_hierarchy(),
            least_privilege: node.least_privilege_metadata(),
        })
        .collect()
}

fn collect_compact_hunches(bundle: &StructuredRecallBundle, limit: usize) -> Vec<CompactHunch> {
    bundle
        .hunches
        .iter()
        .take(limit)
        .map(|hunch| {
            let source = find_node_in_bundle(bundle, hunch.source_node_id);
            CompactHunch {
                source_node_id: hunch.source_node_id,
                title: hunch.title.clone(),
                reason: hunch.reason.clone(),
                source_ref: source.and_then(|node| node.source_ref.clone()),
                confidence: source.and_then(|node| node.confidence),
                trust_level: source.and_then(|node| node.trust_level.clone()),
                source_hierarchy: source.and_then(Node::source_hierarchy),
            }
        })
        .collect()
}

fn collect_compact_source_refs(
    compact: &CompactRecallBundle,
    limit: usize,
) -> Vec<CompactSourceRef> {
    let mut seen = HashSet::new();
    let mut refs = Vec::new();

    for node in compact_node_refs(compact) {
        if seen.insert(node.node_id) {
            refs.push(CompactSourceRef {
                node_id: node.node_id,
                source_ref: node.source_ref.clone(),
                confidence: node.confidence,
                trust_level: node.trust_level.clone(),
                source_hierarchy: node.source_hierarchy.clone(),
            });
        }

        if refs.len() >= limit {
            return refs;
        }
    }

    for hunch in &compact.hunches {
        if seen.insert(hunch.source_node_id) {
            refs.push(CompactSourceRef {
                node_id: hunch.source_node_id,
                source_ref: hunch.source_ref.clone(),
                confidence: hunch.confidence,
                trust_level: hunch.trust_level.clone(),
                source_hierarchy: hunch.source_hierarchy.clone(),
            });
        }

        if refs.len() >= limit {
            break;
        }
    }

    refs
}

fn compact_node_refs(compact: &CompactRecallBundle) -> impl Iterator<Item = &CompactNodeRef> {
    compact
        .applicable_workflows
        .iter()
        .chain(compact.active_gates.iter())
        .chain(compact.tool_contracts.iter())
        .chain(compact.mcp_profiles.iter())
        .chain(compact.project_profile_facts.iter())
        .chain(compact.relevant_corrections_lessons.iter())
}

fn bundle_nodes_by_type<'a>(bundle: &'a StructuredRecallBundle, node_type: &str) -> Vec<&'a Node> {
    bundle_nodes_by_types(bundle, &[node_type])
}

fn bundle_nodes_by_types<'a>(
    bundle: &'a StructuredRecallBundle,
    node_types: &[&str],
) -> Vec<&'a Node> {
    all_bundle_nodes(bundle)
        .into_iter()
        .filter(|node| node_types.contains(&node.node_type.as_str()))
        .collect()
}

fn all_bundle_nodes(bundle: &StructuredRecallBundle) -> Vec<&Node> {
    let mut nodes = Vec::new();
    collect_status_nodes(&mut nodes, &bundle.workflows);
    collect_status_nodes(&mut nodes, &bundle.gates);
    collect_status_nodes(&mut nodes, &bundle.project_profiles);
    nodes.extend(bundle.linked_nodes.iter().map(|linked| &linked.node));
    nodes.extend(bundle.fts_fallback.iter().map(|result| &result.node));
    nodes
}

fn active_status_nodes(nodes: &RecallNodesByStatus) -> Vec<&Node> {
    nodes.active.iter().collect()
}

fn collect_status_nodes<'a>(nodes: &mut Vec<&'a Node>, grouped: &'a RecallNodesByStatus) {
    nodes.extend(grouped.active.iter());
    nodes.extend(grouped.draft.iter());
    nodes.extend(grouped.broken.iter());
}

fn find_node_in_bundle(bundle: &StructuredRecallBundle, node_id: i64) -> Option<&Node> {
    all_bundle_nodes(bundle)
        .into_iter()
        .find(|node| node.id == node_id)
}

fn select_hunches(bundle: &StructuredRecallBundle) -> Vec<RecallHunch> {
    let mut candidates: Vec<&FtsNodeSearchResult> = bundle.fts_fallback.iter().collect();
    let linked_signal = strongest_linked_hunch_signal(bundle);

    candidates.sort_by(|left, right| {
        hunch_signal_priority(right, linked_signal.as_ref())
            .cmp(&hunch_signal_priority(left, linked_signal.as_ref()))
            .then_with(|| compare_nodes_for_recall_priority(&left.node, &right.node))
            .then_with(|| {
                left.rank
                    .partial_cmp(&right.rank)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| right.node.updated_at.cmp(&left.node.updated_at))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });

    candidates
        .into_iter()
        .take(MAX_HUNCHES)
        .map(|result| RecallHunch {
            source_node_id: result.node.id,
            source_node_type: result.node.node_type.clone(),
            linked_signal_node_id: linked_signal.as_ref().map(|signal| signal.node_id),
            linked_signal_node_type: linked_signal
                .as_ref()
                .map(|signal| signal.node_type.clone()),
            title: result.node.title.clone(),
            summary: result.node.summary.clone(),
            reason: hunch_reason(&result.node.node_type, linked_signal.as_ref()).to_string(),
            source_updated_at: result.node.updated_at.clone(),
            source_hierarchy: result.node.source_hierarchy(),
        })
        .collect()
}

fn compare_nodes_for_recall_priority(left: &Node, right: &Node) -> Ordering {
    source_priority(left)
        .cmp(&source_priority(right))
        .then_with(|| trust_level_priority(left).cmp(&trust_level_priority(right)))
        .then_with(|| {
            right
                .confidence
                .partial_cmp(&left.confidence)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.id.cmp(&right.id))
}

fn source_priority(node: &Node) -> (u8, u8) {
    let hierarchy_priority = node
        .source_hierarchy()
        .map(|hierarchy| hierarchy.priority)
        .unwrap_or(u8::MAX);
    let privilege_priority = node
        .least_privilege_metadata()
        .map(|metadata| metadata.privilege_rank)
        .unwrap_or(0);

    (hierarchy_priority, privilege_priority)
}

fn trust_level_priority(node: &Node) -> u8 {
    match node.trust_level.as_deref() {
        Some("high") => 0,
        Some("medium") => 1,
        Some("low") => 2,
        Some(_) => 3,
        None => 4,
    }
}

fn strongest_linked_hunch_signal(bundle: &StructuredRecallBundle) -> Option<HunchLinkedSignal> {
    bundle
        .linked_nodes
        .iter()
        .filter_map(|linked| {
            let priority = hunch_type_priority(&linked.node.node_type);
            (priority > 0).then(|| HunchLinkedSignal {
                node_id: linked.node.id,
                node_type: linked.node.node_type.clone(),
                priority,
            })
        })
        .max_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| right.node_id.cmp(&left.node_id))
        })
}

fn hunch_signal_priority(
    result: &FtsNodeSearchResult,
    linked_signal: Option<&HunchLinkedSignal>,
) -> u8 {
    hunch_type_priority(&result.node.node_type).max(
        linked_signal
            .map(|signal| signal.priority)
            .unwrap_or_default(),
    )
}

fn hunch_type_priority(node_type: &str) -> u8 {
    match node_type {
        "failure_mode" => 3,
        "tool_contract" => 2,
        "workflow" => 1,
        _ => 0,
    }
}

fn hunch_reason(node_type: &str, linked_signal: Option<&HunchLinkedSignal>) -> &'static str {
    if linked_signal.is_some() {
        return "fts_match_linked_signal_hotness";
    }

    match node_type {
        "failure_mode" => "fts_match_failure_mode_hotness",
        "tool_contract" => "fts_match_tool_hotness",
        "workflow" => "fts_match_workflow_hotness",
        _ => "fts_match_hotness",
    }
}

fn traverse_links(nodes: &[Node], links: &[Link]) -> Vec<RecallLinkedNode> {
    let nodes_by_id: HashMap<i64, &Node> = nodes.iter().map(|node| (node.id, node)).collect();
    let mut outgoing: HashMap<i64, Vec<&Link>> = HashMap::new();

    for link in links {
        outgoing.entry(link.source_node_id).or_default().push(link);
    }

    let mut visited: HashSet<i64> = HashSet::new();
    let mut queue = VecDeque::new();

    for node in nodes.iter().filter(|node| is_traversal_root(node)) {
        visited.insert(node.id);
        queue.push_back((node.id, 0));
    }

    let mut linked_nodes = Vec::new();

    while let Some((source_node_id, depth)) = queue.pop_front() {
        if depth >= RECALL_TRAVERSAL_MAX_DEPTH {
            continue;
        }

        for link in outgoing.get(&source_node_id).into_iter().flatten() {
            let Some(target) = nodes_by_id.get(&link.target_node_id) else {
                continue;
            };

            if should_exclude_from_normal_recall(target) || !visited.insert(target.id) {
                continue;
            }

            let next_depth = depth + 1;
            linked_nodes.push(RecallLinkedNode {
                depth: next_depth,
                source_node_id,
                link_type: link.link_type.clone(),
                node: (*target).clone(),
            });
            queue.push_back((target.id, next_depth));
        }
    }

    linked_nodes
}

fn structured_node_count(bundle: &StructuredRecallBundle) -> usize {
    structured_node_ids(bundle).len()
}

fn structured_node_ids(bundle: &StructuredRecallBundle) -> HashSet<i64> {
    let mut ids = HashSet::new();
    collect_status_ids(&mut ids, &bundle.project_profiles);
    collect_status_ids(&mut ids, &bundle.gates);
    collect_status_ids(&mut ids, &bundle.workflows);
    ids.extend(bundle.linked_nodes.iter().map(|linked| linked.node.id));
    ids
}

fn collect_status_ids(ids: &mut HashSet<i64>, nodes: &RecallNodesByStatus) {
    ids.extend(nodes.draft.iter().map(|node| node.id));
    ids.extend(nodes.active.iter().map(|node| node.id));
    ids.extend(nodes.deprecated.iter().map(|node| node.id));
    ids.extend(nodes.superseded.iter().map(|node| node.id));
    ids.extend(nodes.broken.iter().map(|node| node.id));
}

fn collect_status_titles<'a>(titles: &mut Vec<&'a str>, nodes: &'a RecallNodesByStatus) {
    titles.extend(nodes.active.iter().map(|node| node.title.as_str()));
    titles.extend(nodes.draft.iter().map(|node| node.title.as_str()));
    titles.extend(nodes.broken.iter().map(|node| node.title.as_str()));
}

fn is_traversal_root(node: &Node) -> bool {
    matches!(
        node.node_type.as_str(),
        "workflow" | "tool_contract" | "rule"
    ) && !should_exclude_from_normal_recall(node)
}

fn should_exclude_from_normal_recall(node: &Node) -> bool {
    matches!(node.status.as_str(), "deprecated" | "superseded")
}

impl RecallNodesByStatus {
    fn push(&mut self, node: Node) {
        match node.status.as_str() {
            "draft" => self.draft.push(node),
            "active" => self.active.push(node),
            "deprecated" => self.deprecated.push(node),
            "superseded" => self.superseded.push(node),
            "broken" => self.broken.push(node),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: i64, node_type: &str, status: &str, title: &str) -> Node {
        Node {
            id,
            node_type: node_type.to_string(),
            status: status.to_string(),
            title: title.to_string(),
            summary: None,
            body: None,
            source_ref: None,
            confidence: None,
            trust_level: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn structured_bundle_groups_project_profiles_gates_and_workflows_by_status() {
        let bundle = build_structured_bundle(vec![
            node(1, "project_profile", "active", "Project profile"),
            node(2, "gate", "draft", "Draft gate"),
            node(3, "workflow", "broken", "Broken workflow"),
            node(4, "decision", "active", "Ignored decision"),
            node(5, "workflow", "superseded", "Old workflow"),
        ]);

        assert_eq!(bundle.project_profiles.active[0].title, "Project profile");
        assert_eq!(bundle.gates.draft[0].title, "Draft gate");
        assert_eq!(bundle.workflows.broken[0].title, "Broken workflow");
        assert!(bundle.workflows.superseded.is_empty());
        assert!(bundle.gates.active.is_empty());
        assert!(bundle.project_profiles.draft.is_empty());
        assert!(bundle.linked_nodes.is_empty());
        assert!(bundle.fts_fallback.is_empty());
        assert!(bundle.hunches.is_empty());
    }

    #[test]
    fn structured_bundle_excludes_deprecated_and_superseded_from_normal_sections() {
        let bundle = build_structured_bundle(vec![
            node(1, "project_profile", "deprecated", "Old profile"),
            node(2, "gate", "superseded", "Old gate"),
            node(3, "workflow", "active", "Live workflow"),
        ]);

        assert!(bundle.project_profiles.deprecated.is_empty());
        assert!(bundle.gates.superseded.is_empty());
        assert_eq!(bundle.workflows.active[0].title, "Live workflow");
    }

    #[test]
    fn structured_bundle_traverses_links_from_selected_nodes_with_depth_limit() {
        let bundle = build_structured_bundle_with_links(
            vec![
                node(1, "workflow", "active", "Workflow"),
                node(2, "decision", "active", "Depth one"),
                node(3, "lesson", "active", "Depth two"),
                node(4, "project_fact", "active", "Too deep"),
            ],
            vec![
                link(1, 1, 2, "supports"),
                link(2, 2, 3, "supports"),
                link(3, 3, 4, "supports"),
            ],
        );

        assert_eq!(bundle.linked_nodes.len(), 2);
        assert_eq!(bundle.linked_nodes[0].depth, 1);
        assert_eq!(bundle.linked_nodes[0].node.title, "Depth one");
        assert_eq!(bundle.linked_nodes[1].depth, 2);
        assert_eq!(bundle.linked_nodes[1].node.title, "Depth two");
    }

    #[test]
    fn structured_bundle_excludes_deprecated_and_superseded_from_traversal() {
        let bundle = build_structured_bundle_with_links(
            vec![
                node(1, "workflow", "active", "Workflow"),
                node(2, "decision", "deprecated", "Deprecated"),
                node(3, "lesson", "superseded", "Superseded"),
                node(4, "project_fact", "active", "Active"),
            ],
            vec![
                link(1, 1, 2, "supports"),
                link(2, 1, 3, "supports"),
                link(3, 1, 4, "supports"),
            ],
        );

        assert_eq!(bundle.linked_nodes.len(), 1);
        assert_eq!(bundle.linked_nodes[0].node.title, "Active");
    }

    #[test]
    fn fts_fallback_is_used_only_when_structured_recall_is_small() {
        let small = build_structured_bundle(vec![node(1, "workflow", "active", "Needle")]);
        let enough = build_structured_bundle(vec![
            node(1, "workflow", "active", "One"),
            node(2, "gate", "active", "Two"),
            node(3, "project_profile", "active", "Three"),
        ]);

        assert!(needs_fts_fallback(&small));
        assert!(!needs_fts_fallback(&enough));
        assert_eq!(
            derive_fts_fallback_query(&small),
            Some("Needle".to_string())
        );
    }

    #[test]
    fn fts_fallback_is_additive_and_filters_existing_and_old_nodes() {
        let bundle = build_structured_bundle(vec![node(1, "workflow", "active", "Root")]);
        let with_fallback = add_fts_fallback(
            bundle,
            vec![
                fts_result(1, "active", "Existing"),
                fts_result(2, "deprecated", "Old"),
                fts_result(3, "active", "Fallback"),
            ],
        );

        assert_eq!(with_fallback.fts_fallback.len(), 1);
        assert_eq!(with_fallback.fts_fallback[0].node.title, "Fallback");
    }

    #[test]
    fn hunches_are_selected_from_fts_by_type_rank_hotness_and_id() {
        let bundle = build_structured_bundle(vec![node(1, "workflow", "active", "Root")]);
        let with_hunches = add_fts_fallback(
            bundle,
            vec![
                fts_result_with_rank(2, "raw_note", "active", "Raw", 0.1, "2026-01-03T00:00:00Z"),
                fts_result_with_rank(
                    3,
                    "workflow",
                    "active",
                    "Workflow",
                    0.4,
                    "2026-01-01T00:00:00Z",
                ),
                fts_result_with_rank(
                    4,
                    "failure_mode",
                    "active",
                    "Failure",
                    0.9,
                    "2026-01-02T00:00:00Z",
                ),
                fts_result_with_rank(
                    5,
                    "tool_contract",
                    "active",
                    "Tool",
                    0.2,
                    "2026-01-04T00:00:00Z",
                ),
                fts_result_with_rank(
                    6,
                    "tool_contract",
                    "active",
                    "Older tool",
                    0.2,
                    "2026-01-02T00:00:00Z",
                ),
            ],
        );

        assert_eq!(with_hunches.hunches.len(), 3);
        assert_eq!(with_hunches.hunches[0].source_node_id, 4);
        assert_eq!(
            with_hunches.hunches[0].reason,
            "fts_match_failure_mode_hotness"
        );
        assert_eq!(with_hunches.hunches[1].source_node_id, 5);
        assert_eq!(with_hunches.hunches[2].source_node_id, 6);
    }

    #[test]
    fn hunches_include_linked_workflow_tool_or_failure_mode_signal() {
        let bundle = build_structured_bundle_with_links(
            vec![
                node(1, "workflow", "active", "Root"),
                node(2, "failure_mode", "active", "Linked failure"),
            ],
            vec![link(1, 1, 2, "warns")],
        );
        let with_hunches = add_fts_fallback(
            bundle,
            vec![fts_result_with_rank(
                3,
                "raw_note",
                "active",
                "FTS match",
                0.1,
                "2026-01-03T00:00:00Z",
            )],
        );

        assert_eq!(with_hunches.hunches.len(), 1);
        assert_eq!(with_hunches.hunches[0].source_node_id, 3);
        assert_eq!(with_hunches.hunches[0].linked_signal_node_id, Some(2));
        assert_eq!(
            with_hunches.hunches[0].linked_signal_node_type,
            Some("failure_mode".to_string())
        );
        assert_eq!(
            with_hunches.hunches[0].reason,
            "fts_match_linked_signal_hotness"
        );
    }

    #[test]
    fn compact_bundle_has_source_confidence_trust_and_section_limits() {
        let bundle = build_structured_bundle(vec![
            sourced_node(1, "workflow", "active", "Workflow 1"),
            sourced_node(2, "workflow", "active", "Workflow 2"),
            sourced_node(3, "gate", "active", "Gate 1"),
            sourced_node(4, "gate", "active", "Gate 2"),
            sourced_node(5, "gate", "active", "Gate 3"),
            sourced_node(6, "gate", "active", "Gate 4"),
            sourced_node(7, "project_profile", "active", "Profile"),
        ]);
        let bundle = add_fts_fallback(
            bundle,
            vec![
                sourced_fts_result(8, "tool_contract", "Tool"),
                sourced_fts_result(9, "mcp_profile", "MCP"),
                sourced_fts_result(10, "lesson", "Lesson"),
                sourced_fts_result(11, "correction", "Correction"),
            ],
        );

        assert_eq!(bundle.compact.applicable_workflows.len(), 1);
        assert_eq!(bundle.compact.active_gates.len(), 3);
        assert_eq!(bundle.compact.tool_contracts.len(), 1);
        assert_eq!(bundle.compact.mcp_profiles.len(), 1);
        assert_eq!(bundle.compact.project_profile_facts.len(), 1);
        assert_eq!(bundle.compact.relevant_corrections_lessons.len(), 2);
        assert_eq!(
            bundle.compact.applicable_workflows[0].source_ref,
            Some("source=user_instruction".to_string())
        );
        assert_eq!(bundle.compact.applicable_workflows[0].confidence, Some(0.8));
        assert_eq!(
            bundle.compact.applicable_workflows[0].trust_level,
            Some("high".to_string())
        );
        assert_eq!(
            bundle.compact.applicable_workflows[0]
                .source_hierarchy
                .as_ref()
                .map(|hierarchy| hierarchy.source_root.as_str()),
            Some("user_instruction")
        );
        assert!(bundle.compact.source_refs.len() <= bundle.compact.limits.source_refs);
    }

    #[test]
    fn compact_bundle_prefers_higher_priority_sources() {
        let bundle = build_structured_bundle(vec![
            source_tuned_node(
                1,
                "workflow",
                "active",
                "External",
                "source=external/api",
                0.99,
            ),
            source_tuned_node(
                2,
                "workflow",
                "active",
                "Instruction",
                "source=user_instruction",
                0.70,
            ),
        ]);

        assert_eq!(bundle.compact.applicable_workflows.len(), 1);
        assert_eq!(bundle.compact.applicable_workflows[0].node_id, 2);
    }

    #[test]
    fn compact_tool_and_mcp_nodes_include_least_privilege_metadata() {
        let bundle = add_fts_fallback(
            build_structured_bundle(vec![sourced_node(99, "workflow", "active", "Root")]),
            vec![
                FtsNodeSearchResult {
                    rank: 0.0,
                    node: privileged_node(
                        1,
                        "tool_contract",
                        "Tool",
                        "source=tool/context-export",
                        r#"{
                            "side_effects":"local_write_artifact",
                            "approval_requirement":"manual_review",
                            "read_operations":["memory.read"],
                            "write_operations":["artifact.write"]
                        }"#,
                    ),
                },
                FtsNodeSearchResult {
                    rank: 0.0,
                    node: privileged_node(
                        2,
                        "mcp_profile",
                        "MCP",
                        "source=mcp/corporate/github",
                        r#"{
                            "side_effects":"external_read",
                            "approval_requirement":"none",
                            "read_operations":["repos.read"],
                            "write_operations":[]
                        }"#,
                    ),
                },
            ],
        );

        assert_eq!(
            bundle.compact.tool_contracts[0]
                .least_privilege
                .as_ref()
                .map(|metadata| metadata.side_effects.as_str()),
            Some("local_write_artifact")
        );
        assert_eq!(
            bundle.compact.mcp_profiles[0]
                .least_privilege
                .as_ref()
                .map(|metadata| metadata.side_effects.as_str()),
            Some("external_read")
        );
    }

    #[test]
    fn hunches_and_compact_hunches_are_capped_at_max_count() {
        let bundle = build_structured_bundle(vec![node(1, "workflow", "active", "Root")]);
        let with_hunches = add_fts_fallback(
            bundle,
            vec![
                fts_result_with_rank(
                    2,
                    "failure_mode",
                    "active",
                    "One",
                    0.1,
                    "2026-01-06T00:00:00Z",
                ),
                fts_result_with_rank(
                    3,
                    "failure_mode",
                    "active",
                    "Two",
                    0.1,
                    "2026-01-05T00:00:00Z",
                ),
                fts_result_with_rank(
                    4,
                    "failure_mode",
                    "active",
                    "Three",
                    0.1,
                    "2026-01-04T00:00:00Z",
                ),
                fts_result_with_rank(
                    5,
                    "failure_mode",
                    "active",
                    "Four",
                    0.1,
                    "2026-01-03T00:00:00Z",
                ),
                fts_result_with_rank(
                    6,
                    "failure_mode",
                    "active",
                    "Five",
                    0.1,
                    "2026-01-02T00:00:00Z",
                ),
            ],
        );

        assert_eq!(with_hunches.hunches.len(), MAX_HUNCHES);
        assert_eq!(with_hunches.compact.hunches.len(), MAX_HUNCHES);
        assert_eq!(with_hunches.hunches[0].source_node_id, 2);
        assert_eq!(with_hunches.hunches[2].source_node_id, 4);
    }

    fn link(id: i64, source_node_id: i64, target_node_id: i64, link_type: &str) -> Link {
        Link {
            id,
            source_node_id,
            target_node_id,
            link_type: link_type.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn fts_result(id: i64, status: &str, title: &str) -> FtsNodeSearchResult {
        fts_result_with_rank(id, "raw_note", status, title, 0.0, "2026-01-01T00:00:00Z")
    }

    fn sourced_node(id: i64, node_type: &str, status: &str, title: &str) -> Node {
        let mut node = node(id, node_type, status, title);
        node.source_ref = Some("source=user_instruction".to_string());
        node.confidence = Some(0.8);
        node.trust_level = Some("high".to_string());
        node
    }

    fn source_tuned_node(
        id: i64,
        node_type: &str,
        status: &str,
        title: &str,
        source_ref: &str,
        confidence: f64,
    ) -> Node {
        let mut node = node(id, node_type, status, title);
        node.source_ref = Some(source_ref.to_string());
        node.confidence = Some(confidence);
        node.trust_level = Some("high".to_string());
        node
    }

    fn privileged_node(
        id: i64,
        node_type: &str,
        title: &str,
        source_ref: &str,
        body: &str,
    ) -> Node {
        let mut node = source_tuned_node(id, node_type, "active", title, source_ref, 0.8);
        node.body = Some(body.to_string());
        node
    }

    fn sourced_fts_result(id: i64, node_type: &str, title: &str) -> FtsNodeSearchResult {
        FtsNodeSearchResult {
            rank: 0.0,
            node: sourced_node(id, node_type, "active", title),
        }
    }

    fn fts_result_with_rank(
        id: i64,
        node_type: &str,
        status: &str,
        title: &str,
        rank: f64,
        updated_at: &str,
    ) -> FtsNodeSearchResult {
        let mut node = node(id, node_type, status, title);
        node.updated_at = updated_at.to_string();
        FtsNodeSearchResult { rank, node }
    }
}
