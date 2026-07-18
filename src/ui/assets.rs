//! Compile-time embedded Stage 30 read-only desktop UI assets.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Asset {
    pub body: &'static [u8],
    pub content_type: &'static str,
}

const INDEX_HTML: &str = include_str!("assets/index.html");
const APP_CSS: &str = include_str!("assets/app.css");
const APP_JS: &str = include_str!("assets/app.js");

pub(super) fn for_path(path: &str) -> Option<Asset> {
    match path {
        "" | "index.html" => Some(Asset {
            body: INDEX_HTML.as_bytes(),
            content_type: "text/html; charset=utf-8",
        }),
        "app.css" => Some(Asset {
            body: APP_CSS.as_bytes(),
            content_type: "text/css; charset=utf-8",
        }),
        "app.js" => Some(Asset {
            body: APP_JS.as_bytes(),
            content_type: "application/javascript; charset=utf-8",
        }),
        _ => None,
    }
}

#[cfg(test)]
pub(super) const ALL_TEXT: [&str; 3] = [INDEX_HTML, APP_CSS, APP_JS];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assets_are_embedded_and_use_only_local_relative_resources() {
        assert!(INDEX_HTML.contains("href=\"app.css\""));
        assert!(INDEX_HTML.contains("src=\"app.js\""));
        for asset in ALL_TEXT {
            for forbidden in [
                "http://",
                "https://",
                "href=\"//",
                "src=\"//",
                "@import",
                "localStorage",
                "sessionStorage",
                "document.cookie",
                "WebSocket",
                "EventSource",
                "XMLHttpRequest",
            ] {
                assert!(
                    !asset.contains(forbidden),
                    "embedded UI asset contains forbidden resource or browser storage: {forbidden}"
                );
            }
        }
    }

    #[test]
    fn desktop_shell_exposes_six_semantic_read_only_sections() {
        assert!(INDEX_HTML.contains("class=\"skip-link\""));
        assert!(INDEX_HTML.contains("<nav id=\"primary-nav\""));
        assert!(INDEX_HTML.contains("<main id=\"main-content\""));
        assert!(INDEX_HTML.contains("<aside id=\"detail-panel\""));
        assert!(INDEX_HTML.contains("role=\"status\""));
        assert!(INDEX_HTML.contains("aria-live=\"polite\""));
        assert!(INDEX_HTML.contains("id=\"live-status\""));
        let view_start = INDEX_HTML
            .find("id=\"view-content\"")
            .expect("view content should exist");
        let view_opening_end = INDEX_HTML[view_start..]
            .find('>')
            .expect("view content opening tag should close");
        let view_opening = &INDEX_HTML[view_start..view_start + view_opening_end];
        assert!(view_opening.contains("aria-busy=\"true\""));
        assert!(!view_opening.contains("aria-live"));
        assert_eq!(INDEX_HTML.matches("data-view=\"").count(), 6);
        for view in [
            "overview",
            "memory",
            "graph",
            "activity",
            "effectiveness",
            "tools",
        ] {
            assert!(
                INDEX_HTML.contains(&format!("data-view=\"{view}\"")),
                "desktop shell is missing view: {view}"
            );
        }
        for forbidden in [
            "contenteditable",
            "type=\"file\"",
            "formaction=",
            "download=",
        ] {
            assert!(
                !INDEX_HTML.contains(forbidden),
                "read-only shell contains an editing or transfer control: {forbidden}"
            );
        }
    }

    #[test]
    fn javascript_uses_safe_dom_sinks_and_read_only_fetch() {
        for required in [
            "document.createElement(",
            "document.createElementNS(",
            ".textContent",
            ".replaceChildren(",
            "new URL(\"./\", window.location.href)",
            "credentials: \"omit\"",
            "cache: \"no-store\"",
            "redirect: \"error\"",
            "referrerPolicy: \"no-referrer\"",
            "AbortController",
            "requestIsCurrent",
        ] {
            assert!(
                APP_JS.contains(required),
                "read-only UI JavaScript is missing required control: {required}"
            );
        }
        for forbidden in [
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "document.write",
            "eval(",
            "new Function",
            "window.open",
            "method: \"POST\"",
            "method: \"PUT\"",
            "method: \"PATCH\"",
            "method: \"DELETE\"",
        ] {
            assert!(
                !APP_JS.contains(forbidden),
                "read-only UI JavaScript contains forbidden behavior: {forbidden}"
            );
        }
    }

    #[test]
    fn javascript_consumes_every_frozen_v1_read_route() {
        for endpoint in [
            "bootstrap",
            "overview",
            "memory",
            "node",
            "node-links",
            "graph",
            "activity",
            "bundle",
            "effectiveness",
            "tools",
            "mcp",
        ] {
            assert!(
                APP_JS.contains(&format!("\"{endpoint}\"")),
                "desktop UI does not consume frozen endpoint: {endpoint}"
            );
        }
        assert!(APP_JS.contains("const MAX_PAGE_SIZE = 500;"));
        assert!(APP_JS.contains("const MAX_GRAPH_NODES = 200;"));
        assert!(APP_JS.contains("const MAX_GRAPH_EDGES = 500;"));
        assert!(APP_JS.contains("UI_GRAPH_CENTER_MISSING"));
        assert!(APP_JS.contains("responseCenter !== centerId"));
        assert!(APP_JS.contains("deduplicated.set"));
        assert!(APP_JS.contains("deduplicated.size > MAX_GRAPH_NODES"));
        assert!(!APP_JS.contains("MAX_GRAPH_NODES + (centerNode"));
        assert!(APP_JS.contains("nodes_next_cursor"));
        assert!(APP_JS.contains("edges_complete"));
    }

    #[test]
    fn javascript_reuses_static_lookups_and_preserves_graph_ordering_sorts() {
        for (value, class_name) in [
            ("active", "badge-success"),
            ("success", "badge-success"),
            ("installed", "badge-success"),
            ("configured", "badge-success"),
            ("useful", "badge-success"),
            ("applied", "badge-success"),
            ("recorded", "badge-success"),
            ("ready", "badge-success"),
            ("completed", "badge-success"),
            ("draft", "badge-warning"),
            ("warning", "badge-warning"),
            ("pending", "badge-warning"),
            ("partial", "badge-warning"),
            ("configured_unverified", "badge-warning"),
            ("proposed", "badge-warning"),
            ("drafted", "badge-warning"),
            ("started", "badge-warning"),
            ("truncated", "badge-warning"),
            ("broken", "badge-danger"),
            ("failure", "badge-danger"),
            ("failed", "badge-danger"),
            ("missing", "badge-danger"),
            ("wrong", "badge-danger"),
            ("timeout", "badge-danger"),
            ("overflow", "badge-danger"),
            ("blocked", "badge-danger"),
            ("deprecated", "badge-accent"),
            ("superseded", "badge-accent"),
        ] {
            assert!(
                APP_JS.contains(&format!("[\"{value}\", \"{class_name}\"]")),
                "badge class lookup lost {value} -> {class_name}"
            );
        }
        assert!(APP_JS.contains("const BADGE_CLASS_BY_VALUE = new Map(["));
        assert!(APP_JS.contains("BADGE_CLASS_BY_VALUE.get(normalized)"));
        assert!(
            APP_JS.contains("const className = badgeClass ? `badge ${badgeClass}` : \"badge\";")
        );

        assert_eq!(APP_JS.matches(".sort(compareNodes)").count(), 1);
        for forbidden in [
            "const nodeIds = new Set(nodes.map",
            "const sortedNodes = [...nodes].sort(compareNodes)",
            "for (const node of [...nodes].sort(compareNodes))",
        ] {
            assert!(
                !APP_JS.contains(forbidden),
                "UI restored redundant graph work: {forbidden}"
            );
        }
        for required in [
            "deduplicated.has(Number(edge.source_node_id))",
            "deduplicated.has(Number(edge.target_node_id))",
            "const sortedNodes = nodes;",
            "const neighbors = Array.from(",
            "const orderedLayers = Array.from(layers.entries()).sort(",
            "layer[1].sort(",
        ] {
            assert!(
                APP_JS.contains(required),
                "UI lost required deterministic graph work: {required}"
            );
        }
    }

    #[test]
    fn styles_are_desktop_dense_accessible_and_system_themed() {
        for required in [
            "min-width: 1100px",
            "system-ui",
            "@media (prefers-color-scheme: dark)",
            "@media (prefers-reduced-motion: reduce)",
            ":focus-visible",
            ".data-table",
            ".graph-viewport",
            ".detail-panel",
        ] {
            assert!(
                APP_CSS.contains(required),
                "desktop UI stylesheet is missing required rule: {required}"
            );
        }
        for forbidden in ["@font-face", "url(http", "url(//"] {
            assert!(
                !APP_CSS.contains(forbidden),
                "desktop UI stylesheet contains an external resource: {forbidden}"
            );
        }
        let table_header_rule = APP_CSS
            .split_once(".data-table th {")
            .and_then(|(_, suffix)| suffix.split_once('}'))
            .map(|(rule, _)| rule)
            .expect("table header rule should exist");
        for forbidden in ["position: sticky", "top:", "z-index:"] {
            assert!(
                !table_header_rule.contains(forbidden),
                "scrollable table header must keep normal row order: {forbidden}"
            );
        }
    }

    #[test]
    fn effectiveness_and_split_registry_failures_remain_fact_complete() {
        for required in [
            "period.retention_truncated === true",
            "retention removed part of the requested period",
            "recall.more_results_bundles",
            "recall.terminal_more_results_bundles",
            "tasks.started_without_apply",
            "tasks.applied_context_by_type",
            "facts.tool_duplicate_blocks",
            "facts.alias_resolutions",
            "facts.unresolved_tool_overlaps",
            "facts.last_successful_audit_repair_at",
            "item.canonical_tool_id",
            "item.aliases",
            "item.duplicate_classifications",
            "item.superseded_duplicates",
            "item.unresolved_overlaps",
            "response.duplicate_analysis_complete === false",
            "const anyFailed = toolsFailed || mcpFailed",
            "if (toolsFailed && mcpFailed)",
            "setViewState(\"error\", false)",
            "setReadyState(false, anyFailed)",
            "MCP data loaded. Tool data is unavailable.",
            "Tool data loaded. MCP data is unavailable.",
        ] {
            assert!(
                APP_JS.contains(required),
                "desktop UI is missing a fact or partial-failure guard: {required}"
            );
        }
        assert!(!APP_JS.contains("const bothFailed = toolsResult.status === \"rejected\""));
    }

    #[test]
    fn asset_lookup_is_an_exact_allowlist() {
        for path in ["", "index.html", "app.css", "app.js"] {
            let asset = for_path(path).expect("allowlisted asset should exist");
            assert!(!asset.body.is_empty());
            assert!(!asset.content_type.is_empty());
        }
        for path in [
            "/",
            "../index.html",
            "%2e%2e/index.html",
            "app.js?query=1",
            "APP.JS",
            "api",
        ] {
            assert!(for_path(path).is_none(), "path must not be served: {path}");
        }
    }
}
