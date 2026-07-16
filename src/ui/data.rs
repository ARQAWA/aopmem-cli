use rusqlite::{params, Transaction, TransactionBehavior};
use serde::Serialize;

use crate::storage::{self, WorkspacePaths};

pub(crate) use crate::observability::ui::{ActivityQuery, BundleQuery};

pub(crate) const DEFAULT_PAGE_SIZE: usize = 100;
pub(crate) const MAX_PAGE_SIZE: usize = 500;
pub(crate) const MAX_GRAPH_NODES: usize = 200;
pub(crate) const MAX_GRAPH_EDGES: usize = 500;
pub(crate) const MAX_SEARCH_BYTES: usize = 512;
const MAX_CURSOR_BYTES: usize = 4_096;

pub(crate) const UI_API_VERSION: &str = "v1";
const UI_CAPABILITIES: &[&str] = &[
    "bootstrap",
    "overview",
    "memory",
    "node",
    "node_links",
    "graph",
    "activity",
    "bundle",
    "effectiveness",
    "tools",
    "mcp",
];

#[derive(Debug, Clone)]
pub(crate) struct UiDataContext {
    workspace_key: String,
    workspace_paths: WorkspacePaths,
}

impl UiDataContext {
    pub(crate) fn new(workspace_key: String, workspace_paths: WorkspacePaths) -> Self {
        Self {
            workspace_key,
            workspace_paths,
        }
    }

    pub(crate) fn workspace_key(&self) -> &str {
        &self.workspace_key
    }

    pub(crate) fn workspace_paths(&self) -> &WorkspacePaths {
        &self.workspace_paths
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ApiError {
    status: u16,
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    pub(crate) const fn bad_request() -> Self {
        Self {
            status: 400,
            code: "UI_INVALID_REQUEST",
            message: "API request is invalid",
        }
    }

    pub(crate) const fn invalid_cursor() -> Self {
        Self {
            status: 400,
            code: "UI_INVALID_CURSOR",
            message: "Pagination cursor is invalid",
        }
    }

    pub(crate) const fn not_found() -> Self {
        Self {
            status: 404,
            code: "UI_API_NOT_FOUND",
            message: "API route not found",
        }
    }

    pub(crate) const fn method_not_allowed() -> Self {
        Self {
            status: 405,
            code: "UI_API_METHOD_NOT_ALLOWED",
            message: "API route accepts GET only",
        }
    }

    pub(crate) const fn data_unavailable() -> Self {
        Self {
            status: 500,
            code: "UI_DATA_UNAVAILABLE",
            message: "Local UI data is unavailable",
        }
    }

    pub(crate) const fn node_not_found() -> Self {
        Self {
            status: 404,
            code: "UI_NODE_NOT_FOUND",
            message: "Memory node was not found",
        }
    }

    pub(crate) const fn bundle_not_found() -> Self {
        Self {
            status: 404,
            code: "UI_BUNDLE_NOT_FOUND",
            message: "Recall bundle was not found",
        }
    }

    pub(crate) const fn status(self) -> u16 {
        self.status
    }

    pub(crate) fn body(self) -> ApiErrorBody {
        ApiErrorBody {
            ok: false,
            error: ApiErrorDetail {
                code: self.code,
                message: self.message,
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiErrorBody {
    ok: bool,
    error: ApiErrorDetail,
}

#[derive(Debug, Serialize)]
struct ApiErrorDetail {
    code: &'static str,
    message: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct BootstrapResponse<'a> {
    pub(crate) api_version: &'static str,
    pub(crate) product_version: &'static str,
    pub(crate) workspace_key: &'a str,
    pub(crate) read_only: bool,
    pub(crate) capabilities: &'static [&'static str],
    pub(crate) observability_available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MemoryQuery {
    pub(crate) limit: usize,
    pub(crate) cursor: Option<String>,
    pub(crate) node_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) search: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NodeLinksQuery {
    pub(crate) node_id: i64,
    pub(crate) limit: usize,
    pub(crate) cursor: Option<String>,
    pub(crate) direction: LinkDirection,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LinkDirection {
    Incoming,
    Outgoing,
    #[default]
    Both,
}

impl LinkDirection {
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, ApiError> {
        match value {
            None | Some("both") => Ok(Self::Both),
            Some("incoming") => Ok(Self::Incoming),
            Some("outgoing") => Ok(Self::Outgoing),
            Some(_) => Err(ApiError::bad_request()),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GraphQuery {
    pub(crate) limit: usize,
    pub(crate) cursor: Option<String>,
    pub(crate) node_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) center: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ToolsQuery {
    pub(crate) limit: usize,
    pub(crate) cursor: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) side_effects: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct McpQuery {
    pub(crate) limit: usize,
    pub(crate) cursor: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) kind: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct OverviewResponse<'a> {
    product_version: &'static str,
    workspace: &'a str,
    read_only: bool,
    observability_available: bool,
    observability: crate::observability::ui::OverviewObservability,
    memory: crate::observability::export::MemorySummaryHeader,
    tool_count: u64,
    mcp_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct MemoryListItem {
    id: i64,
    node_type: String,
    status: String,
    title: String,
    summary: Option<String>,
    source_ref: Option<String>,
    confidence: Option<f64>,
    trust_level: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MemoryResponse {
    limit: usize,
    items: Vec<MemoryListItem>,
    more_results: bool,
    next_cursor: Option<String>,
    body_omitted: bool,
    complete: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct NodeResponse {
    node: storage::Node,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct NodeLinkItem {
    direction: &'static str,
    #[serde(flatten)]
    link: storage::Link,
}

#[derive(Debug, Serialize)]
pub(crate) struct NodeLinksResponse {
    node_id: i64,
    direction: &'static str,
    limit: usize,
    items: Vec<NodeLinkItem>,
    more_results: bool,
    next_cursor: Option<String>,
    complete: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct GraphResponse {
    center: Option<i64>,
    center_node: Option<MemoryListItem>,
    node_limit: usize,
    edge_limit: usize,
    nodes: Vec<MemoryListItem>,
    edges: Vec<storage::Link>,
    nodes_more_results: bool,
    nodes_next_cursor: Option<String>,
    nodes_complete: bool,
    edges_more_results: bool,
    edges_complete: bool,
    complete: bool,
}

#[derive(Serialize)]
pub(crate) struct ToolsResponse {
    limit: usize,
    items: Vec<crate::observability::export::ToolSummaryItem>,
    more_results: bool,
    next_cursor: Option<String>,
    complete: bool,
}

#[derive(Serialize)]
pub(crate) struct McpResponse {
    limit: usize,
    items: Vec<crate::observability::export::McpSummaryItem>,
    more_results: bool,
    next_cursor: Option<String>,
    complete: bool,
}

pub(crate) fn bootstrap(context: &UiDataContext) -> Result<BootstrapResponse<'_>, ApiError> {
    let observability_available =
        match crate::observability::ui::availability(context.workspace_paths()) {
            Ok(crate::observability::ui::UiObservabilityAvailability::Missing) => false,
            Ok(crate::observability::ui::UiObservabilityAvailability::Present) => true,
            Err(_) => return Err(ApiError::data_unavailable()),
        };
    Ok(BootstrapResponse {
        api_version: UI_API_VERSION,
        product_version: env!("CARGO_PKG_VERSION"),
        workspace_key: context.workspace_key(),
        read_only: true,
        capabilities: UI_CAPABILITIES,
        observability_available,
    })
}

pub(crate) fn overview(context: &UiDataContext) -> Result<OverviewResponse<'_>, ApiError> {
    let observability =
        crate::observability::ui::overview(context.workspace_paths(), context.workspace_key())
            .map_err(map_observability_error)?;
    let observability_available = observability.is_available();
    with_operational_read(context, |transaction| {
        let memory = crate::observability::export::build_memory_summary_header(transaction)
            .map_err(|_| ApiError::data_unavailable())?;
        let tool_count = scalar_count(transaction, "SELECT COUNT(*) FROM tool_contracts")?;
        let mcp_count = scalar_count(transaction, "SELECT COUNT(*) FROM mcp_profiles")?;
        Ok(OverviewResponse {
            product_version: env!("CARGO_PKG_VERSION"),
            workspace: context.workspace_key(),
            read_only: true,
            observability_available,
            observability,
            memory,
            tool_count,
            mcp_count,
        })
    })
}

pub(crate) fn activity(
    context: &UiDataContext,
    query: &ActivityQuery,
) -> Result<crate::observability::ui::ActivityResponse, ApiError> {
    crate::observability::ui::activity(context.workspace_paths(), context.workspace_key(), query)
        .map_err(map_observability_error)
}

pub(crate) fn bundle(
    context: &UiDataContext,
    query: &BundleQuery,
) -> Result<crate::observability::ui::BundleResponse, ApiError> {
    crate::observability::ui::bundle(context.workspace_paths(), context.workspace_key(), query)
        .map_err(map_observability_error)
}

pub(crate) fn effectiveness(
    context: &UiDataContext,
) -> Result<crate::observability::report::EffectivenessReport, ApiError> {
    crate::observability::report::effectiveness_report(
        context.workspace_paths(),
        context.workspace_key(),
    )
    .map_err(|_| ApiError::data_unavailable())
}

pub(crate) fn tools(
    context: &UiDataContext,
    query: &ToolsQuery,
) -> Result<ToolsResponse, ApiError> {
    validate_page_limit(query.limit, MAX_PAGE_SIZE)?;
    if query
        .status
        .as_deref()
        .is_some_and(|value| !valid_filter(value, 256))
        || query
            .side_effects
            .as_deref()
            .is_some_and(|value| !crate::tools::ALLOWED_TOOL_SIDE_EFFECTS.contains(&value))
    {
        return Err(ApiError::bad_request());
    }
    let scope = format!(
        "status={};side_effects={}",
        query.status.as_deref().unwrap_or(""),
        query.side_effects.as_deref().unwrap_or("")
    );
    let after_id = decode_numeric_cursor(query.cursor.as_deref(), "tools", &scope)?;
    with_operational_read(context, |transaction| {
        let mut statement = transaction
            .prepare(
                "SELECT tool_id, name, status, owner_workflow,
                        side_effects, approval_requirement, id
                 FROM tool_contracts
                 WHERE id > ?1
                   AND (?2 IS NULL OR status = ?2)
                   AND (?3 IS NULL OR side_effects = ?3)
                 ORDER BY id ASC LIMIT ?4",
            )
            .map_err(|_| ApiError::data_unavailable())?;
        let mut rows = statement
            .query(params![
                after_id,
                query.status.as_deref(),
                query.side_effects.as_deref(),
                fetch_limit(query.limit)?
            ])
            .map_err(|_| ApiError::data_unavailable())?;
        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(|_| ApiError::data_unavailable())? {
            let summary = crate::observability::export::tool_summary_from_row(row)
                .map_err(|_| ApiError::data_unavailable())?;
            let cursor_id = row
                .get::<_, i64>(6)
                .map_err(|_| ApiError::data_unavailable())?;
            if cursor_id <= 0 {
                return Err(ApiError::data_unavailable());
            }
            items.push((cursor_id, summary));
        }
        let more_results = items.len() > query.limit;
        items.truncate(query.limit);
        let next_cursor = if more_results {
            let id = items.last().ok_or_else(ApiError::data_unavailable)?.0;
            Some(encode_numeric_cursor("tools", &scope, id)?)
        } else {
            None
        };
        Ok(ToolsResponse {
            limit: query.limit,
            items: items.into_iter().map(|(_, item)| item).collect(),
            more_results,
            next_cursor,
            complete: !more_results,
        })
    })
}

pub(crate) fn mcp(context: &UiDataContext, query: &McpQuery) -> Result<McpResponse, ApiError> {
    validate_page_limit(query.limit, MAX_PAGE_SIZE)?;
    if query
        .status
        .as_deref()
        .is_some_and(|value| !valid_filter(value, 256))
        || query
            .kind
            .as_deref()
            .is_some_and(|value| !valid_filter(value, 256))
    {
        return Err(ApiError::bad_request());
    }
    let scope = format!(
        "status={};kind={}",
        query.status.as_deref().unwrap_or(""),
        query.kind.as_deref().unwrap_or("")
    );
    let after_id = decode_string_cursor(query.cursor.as_deref(), "mcp", &scope)?;
    if after_id.as_deref().is_some_and(|value| {
        value.trim().is_empty()
            || value.len() > storage::MAX_MCP_ID_BYTES
            || value.as_bytes().contains(&0)
    }) {
        return Err(ApiError::invalid_cursor());
    }
    with_operational_read(context, |transaction| {
        let mut statement = transaction
            .prepare(
                "SELECT id, name, kind, status, read_operations,
                        write_operations, side_effects, approval_requirement
                 FROM mcp_profiles
                 WHERE (?1 IS NULL OR id > ?1)
                   AND (?2 IS NULL OR status = ?2)
                   AND (?3 IS NULL OR kind = ?3)
                 ORDER BY id ASC LIMIT ?4",
            )
            .map_err(|_| ApiError::data_unavailable())?;
        let mut rows = statement
            .query(params![
                after_id.as_deref(),
                query.status.as_deref(),
                query.kind.as_deref(),
                fetch_limit(query.limit)?
            ])
            .map_err(|_| ApiError::data_unavailable())?;
        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(|_| ApiError::data_unavailable())? {
            let cursor_id = row
                .get::<_, String>(0)
                .map_err(|_| ApiError::data_unavailable())?;
            let summary = crate::observability::export::mcp_summary_from_row(row)
                .map_err(|_| ApiError::data_unavailable())?;
            items.push((cursor_id, summary));
        }
        let more_results = items.len() > query.limit;
        items.truncate(query.limit);
        let next_cursor = if more_results {
            let id = &items.last().ok_or_else(ApiError::data_unavailable)?.0;
            Some(encode_string_cursor("mcp", &scope, id)?)
        } else {
            None
        };
        Ok(McpResponse {
            limit: query.limit,
            items: items.into_iter().map(|(_, item)| item).collect(),
            more_results,
            next_cursor,
            complete: !more_results,
        })
    })
}

pub(crate) fn memory(
    context: &UiDataContext,
    query: &MemoryQuery,
) -> Result<MemoryResponse, ApiError> {
    validate_memory_query(query, MAX_PAGE_SIZE)?;
    with_operational_read(context, |transaction| {
        let page = load_memory_page(transaction, query, "memory")?;
        Ok(MemoryResponse {
            limit: query.limit,
            items: page.items,
            more_results: page.more_results,
            next_cursor: page.next_cursor,
            body_omitted: true,
            complete: !page.more_results,
        })
    })
}

pub(crate) fn node(context: &UiDataContext, node_id: i64) -> Result<NodeResponse, ApiError> {
    if node_id <= 0 {
        return Err(ApiError::bad_request());
    }
    with_operational_read(context, |transaction| {
        let node = storage::get_node(transaction, node_id)
            .map_err(|_| ApiError::data_unavailable())?
            .ok_or_else(ApiError::node_not_found)?;
        validate_full_node(&node)?;
        Ok(NodeResponse { node })
    })
}

pub(crate) fn node_links(
    context: &UiDataContext,
    query: &NodeLinksQuery,
) -> Result<NodeLinksResponse, ApiError> {
    validate_page_limit(query.limit, MAX_PAGE_SIZE)?;
    if query.node_id <= 0 {
        return Err(ApiError::bad_request());
    }
    let scope = format!(
        "node={};direction={}",
        query.node_id,
        query.direction.as_str()
    );
    let after_id = decode_numeric_cursor(query.cursor.as_deref(), "node-links", &scope)?;
    with_operational_read(context, |transaction| {
        if storage::get_node(transaction, query.node_id)
            .map_err(|_| ApiError::data_unavailable())?
            .is_none()
        {
            return Err(ApiError::node_not_found());
        }
        let fetch_limit = fetch_limit(query.limit)?;
        let sql = match query.direction {
            LinkDirection::Incoming => {
                "SELECT id, source_node_id, target_node_id, link_type, created_at
                 FROM links WHERE id > ?1 AND target_node_id = ?2
                 ORDER BY id ASC LIMIT ?3"
            }
            LinkDirection::Outgoing => {
                "SELECT id, source_node_id, target_node_id, link_type, created_at
                 FROM links WHERE id > ?1 AND source_node_id = ?2
                 ORDER BY id ASC LIMIT ?3"
            }
            LinkDirection::Both => {
                "SELECT id, source_node_id, target_node_id, link_type, created_at
                 FROM links
                 WHERE id > ?1 AND (source_node_id = ?2 OR target_node_id = ?2)
                 ORDER BY id ASC LIMIT ?3"
            }
        };
        let mut statement = transaction
            .prepare(sql)
            .map_err(|_| ApiError::data_unavailable())?;
        let mut items = statement
            .query_map(params![after_id, query.node_id, fetch_limit], |row| {
                let link = storage::Link {
                    id: row.get(0)?,
                    source_node_id: row.get(1)?,
                    target_node_id: row.get(2)?,
                    link_type: row.get(3)?,
                    created_at: row.get(4)?,
                };
                let direction = if link.source_node_id == query.node_id
                    && link.target_node_id == query.node_id
                {
                    "both"
                } else if link.target_node_id == query.node_id {
                    "incoming"
                } else {
                    "outgoing"
                };
                Ok(NodeLinkItem { direction, link })
            })
            .map_err(|_| ApiError::data_unavailable())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| ApiError::data_unavailable())?;
        validate_links(items.iter().map(|item| &item.link))?;
        let more_results = items.len() > query.limit;
        items.truncate(query.limit);
        let next_cursor = if more_results {
            let id = items.last().ok_or_else(ApiError::data_unavailable)?.link.id;
            Some(encode_numeric_cursor("node-links", &scope, id)?)
        } else {
            None
        };
        Ok(NodeLinksResponse {
            node_id: query.node_id,
            direction: query.direction.as_str(),
            limit: query.limit,
            items,
            more_results,
            next_cursor,
            complete: !more_results,
        })
    })
}

pub(crate) fn graph(
    context: &UiDataContext,
    query: &GraphQuery,
) -> Result<GraphResponse, ApiError> {
    if query.limit == 0 || query.limit > MAX_GRAPH_NODES {
        return Err(ApiError::bad_request());
    }
    let memory_query = MemoryQuery {
        limit: query.limit,
        cursor: query.cursor.clone(),
        node_type: query.node_type.clone(),
        status: query.status.clone(),
        search: None,
    };
    validate_memory_query(&memory_query, MAX_GRAPH_NODES)?;
    if query.center.is_some_and(|center| center <= 0) {
        return Err(ApiError::bad_request());
    }
    with_operational_read(context, |transaction| {
        let page = match query.center {
            Some(center) => load_center_graph_page(transaction, query, center)?,
            None => load_memory_page(transaction, &memory_query, "graph")?,
        };
        let center_node = query
            .center
            .map(|center| {
                storage::get_node(transaction, center)
                    .map_err(|_| ApiError::data_unavailable())?
                    .ok_or_else(ApiError::node_not_found)
                    .map(memory_list_item_from_node)
            })
            .transpose()?;
        if let Some(center_node) = center_node.as_ref() {
            validate_memory_items(std::slice::from_ref(center_node))?;
        }
        let mut node_ids = page.items.iter().map(|node| node.id).collect::<Vec<_>>();
        if let Some(center) = query.center {
            if !node_ids.contains(&center) {
                node_ids.push(center);
            }
        }
        let (edges, edges_more_results) = load_graph_edges(transaction, &node_ids)?;
        Ok(GraphResponse {
            center: query.center,
            center_node,
            node_limit: query.limit,
            edge_limit: MAX_GRAPH_EDGES,
            nodes: page.items,
            edges,
            nodes_more_results: page.more_results,
            nodes_next_cursor: page.next_cursor,
            nodes_complete: !page.more_results,
            edges_more_results,
            edges_complete: !edges_more_results,
            complete: !page.more_results && !edges_more_results,
        })
    })
}

struct MemoryPage {
    items: Vec<MemoryListItem>,
    more_results: bool,
    next_cursor: Option<String>,
}

fn load_memory_page(
    transaction: &Transaction<'_>,
    query: &MemoryQuery,
    cursor_kind: &str,
) -> Result<MemoryPage, ApiError> {
    let scope = memory_scope(query);
    let after_id = decode_numeric_cursor(query.cursor.as_deref(), cursor_kind, &scope)?;
    let fetch_limit = fetch_limit(query.limit)?;
    let match_query = match query.search.as_deref() {
        Some(search) => Some(storage::fts_match_query(search).ok_or_else(ApiError::bad_request)?),
        None => None,
    };
    let sql = if match_query.is_some() {
        "SELECT nodes.id, nodes.node_type, nodes.status, nodes.title,
                nodes.summary, nodes.source_ref, nodes.confidence,
                nodes.trust_level, nodes.created_at, nodes.updated_at
         FROM fts_nodes JOIN nodes ON nodes.id = fts_nodes.rowid
         WHERE fts_nodes MATCH ?4 AND nodes.id > ?1
           AND (?2 IS NULL OR nodes.node_type = ?2)
           AND (?3 IS NULL OR nodes.status = ?3)
         ORDER BY nodes.id ASC LIMIT ?5"
    } else {
        "SELECT id, node_type, status, title, summary, source_ref,
                confidence, trust_level, created_at, updated_at
         FROM nodes
         WHERE id > ?1 AND (?2 IS NULL OR node_type = ?2)
           AND (?3 IS NULL OR status = ?3)
         ORDER BY id ASC LIMIT ?5"
    };
    let mut statement = transaction
        .prepare(sql)
        .map_err(|_| ApiError::data_unavailable())?;
    let mut items = statement
        .query_map(
            params![
                after_id,
                query.node_type.as_deref(),
                query.status.as_deref(),
                match_query.as_deref(),
                fetch_limit
            ],
            row_to_memory_list_item,
        )
        .map_err(|_| ApiError::data_unavailable())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ApiError::data_unavailable())?;
    validate_memory_items(&items)?;
    let more_results = items.len() > query.limit;
    items.truncate(query.limit);
    let next_cursor = if more_results {
        let id = items.last().ok_or_else(ApiError::data_unavailable)?.id;
        Some(encode_numeric_cursor(cursor_kind, &scope, id)?)
    } else {
        None
    };
    Ok(MemoryPage {
        items,
        more_results,
        next_cursor,
    })
}

fn load_center_graph_page(
    transaction: &Transaction<'_>,
    query: &GraphQuery,
    center: i64,
) -> Result<MemoryPage, ApiError> {
    let page_limit = query.limit.min(MAX_GRAPH_NODES.saturating_sub(1));
    if storage::get_node(transaction, center)
        .map_err(|_| ApiError::data_unavailable())?
        .is_none()
    {
        return Err(ApiError::node_not_found());
    }
    let scope = format!(
        "type={};status={};center={center}",
        query.node_type.as_deref().unwrap_or(""),
        query.status.as_deref().unwrap_or("")
    );
    let (after_phase, after_id) = decode_center_cursor(query.cursor.as_deref(), &scope)?;
    let mut statement = transaction
        .prepare(
            "WITH candidates(id) AS (
                 SELECT ?2
                 UNION SELECT target_node_id FROM links WHERE source_node_id = ?2
                 UNION SELECT source_node_id FROM links WHERE target_node_id = ?2
             )
             SELECT nodes.id, nodes.node_type, nodes.status, nodes.title,
                    nodes.summary, nodes.source_ref, nodes.confidence,
                    nodes.trust_level, nodes.created_at, nodes.updated_at
             FROM candidates JOIN nodes ON nodes.id = candidates.id
             WHERE (?1 = -1
                    OR (?1 = 0 AND nodes.id != ?2)
                    OR (?1 = 1 AND nodes.id != ?2 AND nodes.id > ?5))
               AND (nodes.id = ?2 OR ?3 IS NULL OR nodes.node_type = ?3)
               AND (nodes.id = ?2 OR ?4 IS NULL OR nodes.status = ?4)
             ORDER BY CASE WHEN nodes.id = ?2 THEN 0 ELSE 1 END ASC,
                      nodes.id ASC
             LIMIT ?6",
        )
        .map_err(|_| ApiError::data_unavailable())?;
    let mut items = statement
        .query_map(
            params![
                after_phase,
                center,
                query.node_type.as_deref(),
                query.status.as_deref(),
                after_id,
                fetch_limit(page_limit)?
            ],
            row_to_memory_list_item,
        )
        .map_err(|_| ApiError::data_unavailable())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ApiError::data_unavailable())?;
    validate_memory_items(&items)?;
    let more_results = items.len() > page_limit;
    items.truncate(page_limit);
    let next_cursor = if more_results {
        let id = items.last().ok_or_else(ApiError::data_unavailable)?.id;
        let phase = u8::from(id != center);
        Some(encode_center_cursor(&scope, phase, id)?)
    } else {
        None
    };
    Ok(MemoryPage {
        items,
        more_results,
        next_cursor,
    })
}

fn encode_center_cursor(scope: &str, phase: u8, id: i64) -> Result<String, ApiError> {
    if phase > 1 || id <= 0 {
        return Err(ApiError::data_unavailable());
    }
    let cursor = format!(
        "u1.graph-center.{}.{phase}.{id}",
        lowercase_hex(scope.as_bytes())
    );
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(ApiError::data_unavailable());
    }
    Ok(cursor)
}

fn decode_center_cursor(cursor: Option<&str>, scope: &str) -> Result<(i64, i64), ApiError> {
    let Some(cursor) = cursor else {
        return Ok((-1, 0));
    };
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(ApiError::invalid_cursor());
    }
    let prefix = format!("u1.graph-center.{}.", lowercase_hex(scope.as_bytes()));
    let payload = cursor
        .strip_prefix(&prefix)
        .ok_or_else(ApiError::invalid_cursor)?;
    let (phase, id) = payload
        .split_once('.')
        .ok_or_else(ApiError::invalid_cursor)?;
    let phase = phase
        .parse::<u8>()
        .map_err(|_| ApiError::invalid_cursor())?;
    let id = id.parse::<i64>().map_err(|_| ApiError::invalid_cursor())?;
    if encode_center_cursor(scope, phase, id).as_deref() != Ok(cursor) {
        return Err(ApiError::invalid_cursor());
    }
    Ok((i64::from(phase), id))
}

fn row_to_memory_list_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryListItem> {
    Ok(MemoryListItem {
        id: row.get(0)?,
        node_type: row.get(1)?,
        status: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        source_ref: row.get(5)?,
        confidence: row.get(6)?,
        trust_level: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn memory_list_item_from_node(node: storage::Node) -> MemoryListItem {
    MemoryListItem {
        id: node.id,
        node_type: node.node_type,
        status: node.status,
        title: node.title,
        summary: node.summary,
        source_ref: node.source_ref,
        confidence: node.confidence,
        trust_level: node.trust_level,
        created_at: node.created_at,
        updated_at: node.updated_at,
    }
}

fn load_graph_edges(
    transaction: &Transaction<'_>,
    node_ids: &[i64],
) -> Result<(Vec<storage::Link>, bool), ApiError> {
    if node_ids.is_empty() {
        return Ok((Vec::new(), false));
    }
    let placeholders = (1..=node_ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let limit_parameter = node_ids.len() + 1;
    let sql = format!(
        "SELECT id, source_node_id, target_node_id, link_type, created_at
         FROM links
         WHERE source_node_id IN ({placeholders})
           AND target_node_id IN ({placeholders})
         ORDER BY id ASC LIMIT ?{limit_parameter}"
    );
    let mut parameters = node_ids
        .iter()
        .copied()
        .map(rusqlite::types::Value::Integer)
        .collect::<Vec<_>>();
    parameters.push(rusqlite::types::Value::Integer(fetch_limit(
        MAX_GRAPH_EDGES,
    )?));
    let mut statement = transaction
        .prepare(&sql)
        .map_err(|_| ApiError::data_unavailable())?;
    let mut edges = statement
        .query_map(rusqlite::params_from_iter(parameters), |row| {
            Ok(storage::Link {
                id: row.get(0)?,
                source_node_id: row.get(1)?,
                target_node_id: row.get(2)?,
                link_type: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|_| ApiError::data_unavailable())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ApiError::data_unavailable())?;
    validate_links(edges.iter())?;
    let more_results = edges.len() > MAX_GRAPH_EDGES;
    edges.truncate(MAX_GRAPH_EDGES);
    Ok((edges, more_results))
}

fn map_observability_error(error: crate::observability::ui::UiReadError) -> ApiError {
    match error {
        crate::observability::ui::UiReadError::InvalidRequest => ApiError::bad_request(),
        crate::observability::ui::UiReadError::InvalidCursor => ApiError::invalid_cursor(),
        crate::observability::ui::UiReadError::NotFound => ApiError::bundle_not_found(),
        crate::observability::ui::UiReadError::Unavailable => ApiError::data_unavailable(),
    }
}

fn with_operational_read<T>(
    context: &UiDataContext,
    operation: impl FnOnce(&Transaction<'_>) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let mut connection = storage::open_workspace_db_read_only(context.workspace_paths())
        .map_err(|_| ApiError::data_unavailable())?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|_| ApiError::data_unavailable())?;
    let result = operation(&transaction)?;
    transaction
        .commit()
        .map_err(|_| ApiError::data_unavailable())?;
    Ok(result)
}

fn scalar_count(transaction: &Transaction<'_>, sql: &str) -> Result<u64, ApiError> {
    let count = transaction
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .map_err(|_| ApiError::data_unavailable())?;
    u64::try_from(count).map_err(|_| ApiError::data_unavailable())
}

fn validate_memory_query(query: &MemoryQuery, maximum: usize) -> Result<(), ApiError> {
    validate_page_limit(query.limit, maximum)?;
    if query
        .node_type
        .as_deref()
        .is_some_and(|value| !storage::ALLOWED_NODE_TYPES.contains(&value))
        || query
            .status
            .as_deref()
            .is_some_and(|value| !storage::ALLOWED_NODE_STATUSES.contains(&value))
        || query
            .search
            .as_deref()
            .is_some_and(|value| value.is_empty() || value.len() > MAX_SEARCH_BYTES)
    {
        return Err(ApiError::bad_request());
    }
    Ok(())
}

fn validate_page_limit(limit: usize, maximum: usize) -> Result<(), ApiError> {
    if limit == 0 || limit > maximum {
        Err(ApiError::bad_request())
    } else {
        Ok(())
    }
}

fn fetch_limit(limit: usize) -> Result<i64, ApiError> {
    let limit = i64::try_from(limit).map_err(|_| ApiError::bad_request())?;
    limit.checked_add(1).ok_or_else(ApiError::bad_request)
}

fn memory_scope(query: &MemoryQuery) -> String {
    format!(
        "type={};status={};search={}",
        query.node_type.as_deref().unwrap_or(""),
        query.status.as_deref().unwrap_or(""),
        query.search.as_deref().unwrap_or("")
    )
}

fn encode_numeric_cursor(kind: &str, scope: &str, id: i64) -> Result<String, ApiError> {
    if id <= 0
        || !kind
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    {
        return Err(ApiError::data_unavailable());
    }
    let cursor = format!("u1.{kind}.{}.{}", lowercase_hex(scope.as_bytes()), id);
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(ApiError::data_unavailable());
    }
    Ok(cursor)
}

fn decode_numeric_cursor(cursor: Option<&str>, kind: &str, scope: &str) -> Result<i64, ApiError> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(ApiError::invalid_cursor());
    }
    let prefix = format!("u1.{kind}.{}.", lowercase_hex(scope.as_bytes()));
    let id = cursor
        .strip_prefix(&prefix)
        .ok_or_else(ApiError::invalid_cursor)?
        .parse::<i64>()
        .map_err(|_| ApiError::invalid_cursor())?;
    if id <= 0 || encode_numeric_cursor(kind, scope, id).as_deref() != Ok(cursor) {
        return Err(ApiError::invalid_cursor());
    }
    Ok(id)
}

fn encode_string_cursor(kind: &str, scope: &str, key: &str) -> Result<String, ApiError> {
    if key.is_empty() {
        return Err(ApiError::data_unavailable());
    }
    let cursor = format!(
        "u1.{kind}.{}.{}",
        lowercase_hex(scope.as_bytes()),
        lowercase_hex(key.as_bytes())
    );
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(ApiError::data_unavailable());
    }
    Ok(cursor)
}

fn decode_string_cursor(
    cursor: Option<&str>,
    kind: &str,
    scope: &str,
) -> Result<Option<String>, ApiError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(ApiError::invalid_cursor());
    }
    let prefix = format!("u1.{kind}.{}.", lowercase_hex(scope.as_bytes()));
    let payload = cursor
        .strip_prefix(&prefix)
        .ok_or_else(ApiError::invalid_cursor)?;
    let key = decode_lowercase_hex(payload)?;
    if encode_string_cursor(kind, scope, &key).as_deref() != Ok(cursor) {
        return Err(ApiError::invalid_cursor());
    }
    Ok(Some(key))
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn decode_lowercase_hex(value: &str) -> Result<String, ApiError> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err(ApiError::invalid_cursor());
    }
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = lowercase_hex_nibble(pair[0])?;
        let low = lowercase_hex_nibble(pair[1])?;
        decoded.push(high * 16 + low);
    }
    String::from_utf8(decoded).map_err(|_| ApiError::invalid_cursor())
}

fn lowercase_hex_nibble(byte: u8) -> Result<u8, ApiError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(ApiError::invalid_cursor()),
    }
}

fn valid_filter(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
}

fn validate_memory_items(items: &[MemoryListItem]) -> Result<(), ApiError> {
    for item in items {
        if item.id <= 0
            || !storage::ALLOWED_NODE_TYPES.contains(&item.node_type.as_str())
            || !storage::ALLOWED_NODE_STATUSES.contains(&item.status.as_str())
            || item.title.trim().is_empty()
            || item.title.len() > storage::MAX_NODE_TITLE_BYTES
            || item.title.chars().any(|character| character == '\0')
            || item
                .summary
                .as_deref()
                .is_some_and(|value| value.len() > storage::MAX_NODE_SUMMARY_BYTES)
            || item
                .source_ref
                .as_deref()
                .is_some_and(|value| value.len() > storage::MAX_NODE_SOURCE_REF_BYTES)
            || item
                .trust_level
                .as_deref()
                .is_some_and(|value| value.len() > storage::MAX_NODE_TRUST_LEVEL_BYTES)
            || item
                .confidence
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || !valid_timestamp(&item.created_at)
            || !valid_timestamp(&item.updated_at)
        {
            return Err(ApiError::data_unavailable());
        }
    }
    Ok(())
}

fn validate_full_node(node: &storage::Node) -> Result<(), ApiError> {
    let item = MemoryListItem {
        id: node.id,
        node_type: node.node_type.clone(),
        status: node.status.clone(),
        title: node.title.clone(),
        summary: node.summary.clone(),
        source_ref: node.source_ref.clone(),
        confidence: node.confidence,
        trust_level: node.trust_level.clone(),
        created_at: node.created_at.clone(),
        updated_at: node.updated_at.clone(),
    };
    validate_memory_items(&[item])?;
    if node
        .body
        .as_deref()
        .is_some_and(|value| value.len() > storage::MAX_NODE_BODY_BYTES)
    {
        return Err(ApiError::data_unavailable());
    }
    Ok(())
}

fn validate_links<'a>(links: impl Iterator<Item = &'a storage::Link>) -> Result<(), ApiError> {
    for link in links {
        if link.id <= 0
            || link.source_node_id <= 0
            || link.target_node_id <= 0
            || link.link_type.is_empty()
            || link.link_type.len() > storage::MAX_LINK_TYPE_BYTES
            || !valid_timestamp(&link.created_at)
        {
            return Err(ApiError::data_unavailable());
        }
    }
    Ok(())
}

fn valid_timestamp(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && !value.chars().any(char::is_control)
}
