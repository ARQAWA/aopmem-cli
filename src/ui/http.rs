//! Loopback-only HTTP transport for the embedded desktop UI.

use super::{assets, data};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Cursor};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
use std::sync::Arc;
use thiserror::Error;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const LOOPBACK_ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; style-src 'self'; script-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const NOT_FOUND_BODY: &[u8] = b"Not Found\n";
const METHOD_NOT_ALLOWED_BODY: &[u8] = b"Method Not Allowed\n";

#[derive(Debug, Error)]
pub(crate) enum HttpError {
    #[cfg(test)]
    #[error("UI bind address must be exactly 127.0.0.1")]
    InvalidBindAddress,
    #[error("could not generate the local UI session token")]
    RandomToken,
    #[error("could not bind the local UI listener")]
    Bind(#[source] io::Error),
    #[error("local UI listener did not bind IPv4 loopback")]
    UnexpectedBoundAddress,
    #[error("could not initialize the local UI HTTP server")]
    Server(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("local UI HTTP receive failed")]
    Receive(#[source] io::Error),
    #[error("local UI HTTP response failed")]
    Respond(#[source] io::Error),
    #[error("an internal UI security header is invalid")]
    InvalidStaticHeader,
    #[error("could not serialize the local UI response")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BindConfig {
    address: Ipv4Addr,
    port: u16,
}

impl BindConfig {
    #[cfg(test)]
    pub(super) fn new(address: std::net::IpAddr, port: u16) -> Result<Self, HttpError> {
        match address {
            std::net::IpAddr::V4(address) if address == LOOPBACK_ADDRESS => {
                Ok(Self { address, port })
            }
            std::net::IpAddr::V4(_) | std::net::IpAddr::V6(_) => Err(HttpError::InvalidBindAddress),
        }
    }

    pub(super) fn loopback(port: u16) -> Self {
        Self {
            address: LOOPBACK_ADDRESS,
            port,
        }
    }

    fn socket_address(self) -> SocketAddrV4 {
        SocketAddrV4::new(self.address, self.port)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SessionToken(Box<str>);

impl fmt::Debug for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionToken([REDACTED])")
    }
}

impl SessionToken {
    pub(super) fn generate() -> Result<Self, HttpError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| HttpError::RandomToken)?;
        Ok(Self::from_bytes(bytes))
    }

    fn from_bytes(bytes: [u8; 32]) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut value = String::with_capacity(64);
        for byte in bytes {
            value.push(char::from(HEX[usize::from(byte >> 4)]));
            value.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Self(value.into_boxed_str())
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

pub(super) struct HttpServer {
    server: Arc<Server>,
    address: SocketAddrV4,
    token: SessionToken,
    context: Arc<data::UiDataContext>,
}

impl HttpServer {
    pub(super) fn bind(
        config: BindConfig,
        context: Arc<data::UiDataContext>,
    ) -> Result<Self, HttpError> {
        Self::bind_with_token(config, SessionToken::generate()?, context)
    }

    fn bind_with_token(
        config: BindConfig,
        token: SessionToken,
        context: Arc<data::UiDataContext>,
    ) -> Result<Self, HttpError> {
        let listener = TcpListener::bind(config.socket_address()).map_err(HttpError::Bind)?;
        let address = match listener.local_addr().map_err(HttpError::Bind)? {
            SocketAddr::V4(address) if *address.ip() == LOOPBACK_ADDRESS => address,
            SocketAddr::V4(_) | SocketAddr::V6(_) => return Err(HttpError::UnexpectedBoundAddress),
        };
        let server = Server::from_listener(listener, None).map_err(HttpError::Server)?;
        Ok(Self {
            server: Arc::new(server),
            address,
            token,
            context,
        })
    }

    pub(super) fn address(&self) -> SocketAddrV4 {
        self.address
    }

    pub(super) fn url(&self) -> String {
        format!(
            "http://{}:{}/{}/",
            self.address.ip(),
            self.address.port(),
            self.token.as_str()
        )
    }

    pub(super) fn serve(self) -> Result<(), HttpError> {
        loop {
            let request = self.server.recv().map_err(HttpError::Receive)?;
            respond(request, &self.token, &self.context)?;
        }
    }

    #[cfg(test)]
    fn serve_until(self, stop: &std::sync::atomic::AtomicBool) -> Result<(), HttpError> {
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        while !stop.load(Ordering::Acquire) {
            match self.server.recv_timeout(Duration::from_millis(50)) {
                Ok(Some(request)) => respond(request, &self.token, &self.context)?,
                Ok(None) => {}
                Err(_) if stop.load(Ordering::Acquire) => return Ok(()),
                Err(error) => return Err(HttpError::Receive(error)),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ResponseSpec {
    status: StatusCode,
    body: Cow<'static, [u8]>,
    content_type: &'static str,
    allow_get: bool,
}

fn respond(
    request: Request,
    token: &SessionToken,
    context: &data::UiDataContext,
) -> Result<(), HttpError> {
    let spec = if request.remote_addr().is_some_and(
        |address| matches!(address, SocketAddr::V4(address) if *address.ip() == LOOPBACK_ADDRESS),
    ) {
        route(request.method(), request.url(), token, context)?
    } else {
        not_found()
    };
    request
        .respond(build_response(spec)?)
        .map_err(HttpError::Respond)
}

fn route(
    method: &Method,
    url: &str,
    token: &SessionToken,
    context: &data::UiDataContext,
) -> Result<ResponseSpec, HttpError> {
    let Some(path) = authorized_path(url, token) else {
        return Ok(not_found());
    };
    if path.starts_with("api/") {
        let raw_route = path.split_once('?').map_or(path, |(route, _)| route);
        if !is_api_route(raw_route) {
            return api_error(data::ApiError::not_found());
        }
        if method != &Method::Get {
            return api_error(data::ApiError::method_not_allowed());
        }
        let request = match ApiRequest::parse(path) {
            Ok(request) => request,
            Err(error) => return api_error(error),
        };
        return route_api(request, context);
    }
    let Some(asset) = assets::for_path(path) else {
        return Ok(not_found());
    };
    if method != &Method::Get {
        return Ok(ResponseSpec {
            status: StatusCode(405),
            body: Cow::Borrowed(METHOD_NOT_ALLOWED_BODY),
            content_type: "text/plain; charset=utf-8",
            allow_get: true,
        });
    }
    Ok(ResponseSpec {
        status: StatusCode(200),
        body: Cow::Borrowed(asset.body),
        content_type: asset.content_type,
        allow_get: false,
    })
}

fn is_api_route(path: &str) -> bool {
    matches!(
        path,
        "api/v1/bootstrap"
            | "api/v1/overview"
            | "api/v1/memory"
            | "api/v1/node"
            | "api/v1/node-links"
            | "api/v1/graph"
            | "api/v1/activity"
            | "api/v1/bundle"
            | "api/v1/effectiveness"
            | "api/v1/tools"
            | "api/v1/mcp"
    )
}

fn route_api(
    request: ApiRequest,
    context: &data::UiDataContext,
) -> Result<ResponseSpec, HttpError> {
    match request.path.as_str() {
        "api/v1/bootstrap" => request
            .require_only(&[])
            .and_then(|()| data::bootstrap(context))
            .map_or_else(api_error, |body| json_response(StatusCode(200), &body)),
        "api/v1/overview" => request
            .require_only(&[])
            .and_then(|()| data::overview(context))
            .map_or_else(api_error, |body| json_response(StatusCode(200), &body)),
        "api/v1/memory" => {
            let result = request
                .memory_query()
                .and_then(|query| data::memory(context, &query));
            result.map_or_else(api_error, |body| json_response(StatusCode(200), &body))
        }
        "api/v1/node" => {
            let result = request.required_positive_i64("id").and_then(|id| {
                request.require_only(&["id"])?;
                data::node(context, id)
            });
            result.map_or_else(api_error, |body| json_response(StatusCode(200), &body))
        }
        "api/v1/node-links" => {
            let result = request
                .node_links_query()
                .and_then(|query| data::node_links(context, &query));
            result.map_or_else(api_error, |body| json_response(StatusCode(200), &body))
        }
        "api/v1/graph" => {
            let result = request
                .graph_query()
                .and_then(|query| data::graph(context, &query));
            result.map_or_else(api_error, |body| json_response(StatusCode(200), &body))
        }
        "api/v1/activity" => {
            let result = request
                .activity_query()
                .and_then(|query| data::activity(context, &query));
            result.map_or_else(api_error, |body| json_response(StatusCode(200), &body))
        }
        "api/v1/bundle" => {
            let result = request
                .bundle_query()
                .and_then(|query| data::bundle(context, &query));
            result.map_or_else(api_error, |body| json_response(StatusCode(200), &body))
        }
        "api/v1/effectiveness" => request
            .require_only(&[])
            .and_then(|()| data::effectiveness(context))
            .map_or_else(api_error, |body| json_response(StatusCode(200), &body)),
        "api/v1/tools" => {
            let result = request
                .tools_query()
                .and_then(|query| data::tools(context, &query));
            result.map_or_else(api_error, |body| json_response(StatusCode(200), &body))
        }
        "api/v1/mcp" => {
            let result = request
                .mcp_query()
                .and_then(|query| data::mcp(context, &query));
            result.map_or_else(api_error, |body| json_response(StatusCode(200), &body))
        }
        _ => api_error(data::ApiError::not_found()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApiRequest {
    path: String,
    query: BTreeMap<String, String>,
}

impl ApiRequest {
    fn parse(request_target: &str) -> Result<Self, data::ApiError> {
        if request_target.is_empty() || request_target.len() > 8_192 || request_target.contains('#')
        {
            return Err(data::ApiError::bad_request());
        }
        let (raw_path, raw_query) = request_target
            .split_once('?')
            .map_or((request_target, None), |(path, query)| (path, Some(query)));
        if raw_path.is_empty()
            || raw_path.contains('%')
            || raw_path.contains('\\')
            || raw_path.chars().any(char::is_control)
        {
            return Err(data::ApiError::bad_request());
        }
        let mut query = BTreeMap::new();
        if let Some(raw_query) = raw_query {
            if raw_query.is_empty() {
                return Err(data::ApiError::bad_request());
            }
            for pair in raw_query.split('&') {
                let (raw_name, raw_value) = pair
                    .split_once('=')
                    .ok_or_else(data::ApiError::bad_request)?;
                let name = percent_decode(raw_name)?;
                let value = percent_decode(raw_value)?;
                if name.is_empty()
                    || value.chars().any(char::is_control)
                    || query.insert(name, value).is_some()
                {
                    return Err(data::ApiError::bad_request());
                }
            }
        }
        Ok(Self {
            path: raw_path.to_string(),
            query,
        })
    }

    fn require_only(&self, allowed: &[&str]) -> Result<(), data::ApiError> {
        self.query
            .keys()
            .all(|key| allowed.contains(&key.as_str()))
            .then_some(())
            .ok_or_else(data::ApiError::bad_request)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.query.get(name).map(String::as_str)
    }

    fn required_positive_i64(&self, name: &str) -> Result<i64, data::ApiError> {
        parse_positive_i64(self.value(name).ok_or_else(data::ApiError::bad_request)?)
    }

    fn page_limit(&self, maximum: usize) -> Result<usize, data::ApiError> {
        let Some(value) = self.value("limit") else {
            return Ok(data::DEFAULT_PAGE_SIZE.min(maximum));
        };
        let limit = value
            .parse::<usize>()
            .map_err(|_| data::ApiError::bad_request())?;
        if limit == 0 || limit > maximum || limit.to_string() != value {
            return Err(data::ApiError::bad_request());
        }
        Ok(limit)
    }

    fn memory_query(&self) -> Result<data::MemoryQuery, data::ApiError> {
        self.require_only(&["limit", "cursor", "type", "status", "q"])?;
        Ok(data::MemoryQuery {
            limit: self.page_limit(data::MAX_PAGE_SIZE)?,
            cursor: self.value("cursor").map(str::to_string),
            node_type: self.value("type").map(str::to_string),
            status: self.value("status").map(str::to_string),
            search: self.value("q").map(str::to_string),
        })
    }

    fn node_links_query(&self) -> Result<data::NodeLinksQuery, data::ApiError> {
        self.require_only(&["id", "limit", "cursor", "direction"])?;
        Ok(data::NodeLinksQuery {
            node_id: self.required_positive_i64("id")?,
            limit: self.page_limit(data::MAX_PAGE_SIZE)?,
            cursor: self.value("cursor").map(str::to_string),
            direction: data::LinkDirection::parse(self.value("direction"))?,
        })
    }

    fn graph_query(&self) -> Result<data::GraphQuery, data::ApiError> {
        self.require_only(&["limit", "cursor", "type", "status", "center"])?;
        Ok(data::GraphQuery {
            limit: self.page_limit(data::MAX_GRAPH_NODES)?,
            cursor: self.value("cursor").map(str::to_string),
            node_type: self.value("type").map(str::to_string),
            status: self.value("status").map(str::to_string),
            center: self.value("center").map(parse_positive_i64).transpose()?,
        })
    }

    fn activity_query(&self) -> Result<data::ActivityQuery, data::ApiError> {
        self.require_only(&["limit", "cursor", "event", "outcome", "command"])?;
        Ok(data::ActivityQuery {
            limit: self.page_limit(data::MAX_PAGE_SIZE)?,
            cursor: self.value("cursor").map(str::to_string),
            event_type: self.value("event").map(str::to_string),
            outcome: self.value("outcome").map(str::to_string),
            command: self.value("command").map(str::to_string),
        })
    }

    fn bundle_query(&self) -> Result<data::BundleQuery, data::ApiError> {
        self.require_only(&["id", "limit", "cursor"])?;
        Ok(data::BundleQuery {
            bundle_id: self
                .value("id")
                .ok_or_else(data::ApiError::bad_request)?
                .to_string(),
            limit: self.page_limit(data::MAX_PAGE_SIZE)?,
            cursor: self.value("cursor").map(str::to_string),
        })
    }

    fn tools_query(&self) -> Result<data::ToolsQuery, data::ApiError> {
        self.require_only(&["limit", "cursor", "status", "side_effects"])?;
        Ok(data::ToolsQuery {
            limit: self.page_limit(data::MAX_PAGE_SIZE)?,
            cursor: self.value("cursor").map(str::to_string),
            status: self.value("status").map(str::to_string),
            side_effects: self.value("side_effects").map(str::to_string),
        })
    }

    fn mcp_query(&self) -> Result<data::McpQuery, data::ApiError> {
        self.require_only(&["limit", "cursor", "status", "kind"])?;
        Ok(data::McpQuery {
            limit: self.page_limit(data::MAX_PAGE_SIZE)?,
            cursor: self.value("cursor").map(str::to_string),
            status: self.value("status").map(str::to_string),
            kind: self.value("kind").map(str::to_string),
        })
    }
}

fn percent_decode(value: &str) -> Result<String, data::ApiError> {
    if value.len() > 4_096 {
        return Err(data::ApiError::bad_request());
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes
                .get(index + 1)
                .ok_or_else(data::ApiError::bad_request)?;
            let low = *bytes
                .get(index + 2)
                .ok_or_else(data::ApiError::bad_request)?;
            let high = hex_nibble(high)?;
            let low = hex_nibble(low)?;
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|_| data::ApiError::bad_request())?;
    if decoded.chars().any(char::is_control) {
        return Err(data::ApiError::bad_request());
    }
    Ok(decoded)
}

fn hex_nibble(byte: u8) -> Result<u8, data::ApiError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(data::ApiError::bad_request()),
    }
}

fn parse_positive_i64(value: &str) -> Result<i64, data::ApiError> {
    let parsed = value
        .parse::<i64>()
        .map_err(|_| data::ApiError::bad_request())?;
    if parsed <= 0 || parsed.to_string() != value {
        Err(data::ApiError::bad_request())
    } else {
        Ok(parsed)
    }
}

fn authorized_path<'a>(url: &'a str, token: &SessionToken) -> Option<&'a str> {
    let request_target = url.strip_prefix('/')?;
    let (candidate, path) = request_target.split_once('/')?;
    constant_time_token_match(candidate.as_bytes(), token.as_str().as_bytes()).then_some(path)
}

fn constant_time_token_match(candidate: &[u8], expected: &[u8]) -> bool {
    let mut difference = candidate.len() ^ expected.len();
    for (index, expected_byte) in expected.iter().enumerate() {
        difference |=
            usize::from(candidate.get(index).copied().unwrap_or_default() ^ *expected_byte);
    }
    difference == 0
}

fn not_found() -> ResponseSpec {
    ResponseSpec {
        status: StatusCode(404),
        body: Cow::Borrowed(NOT_FOUND_BODY),
        content_type: "text/plain; charset=utf-8",
        allow_get: false,
    }
}

fn api_error(error: data::ApiError) -> Result<ResponseSpec, HttpError> {
    json_response(StatusCode(error.status()), &error.body())
}

fn json_response<T: serde::Serialize>(
    status: StatusCode,
    body: &T,
) -> Result<ResponseSpec, HttpError> {
    Ok(ResponseSpec {
        status,
        body: Cow::Owned(serde_json::to_vec(body)?),
        content_type: "application/json; charset=utf-8",
        allow_get: status.0 == 405,
    })
}

fn build_response(spec: ResponseSpec) -> Result<Response<Cursor<Cow<'static, [u8]>>>, HttpError> {
    let body_len = spec.body.len();
    let mut headers = Vec::with_capacity(if spec.allow_get { 6 } else { 5 });
    headers.push(static_header("Content-Type", spec.content_type)?);
    headers.push(static_header(
        "Content-Security-Policy",
        CONTENT_SECURITY_POLICY,
    )?);
    headers.push(static_header("X-Content-Type-Options", "nosniff")?);
    headers.push(static_header("Referrer-Policy", "no-referrer")?);
    headers.push(static_header("Cache-Control", "no-store")?);
    if spec.allow_get {
        headers.push(static_header("Allow", "GET")?);
    }
    Ok(Response::new(
        spec.status,
        headers,
        Cursor::new(spec.body),
        Some(body_len),
        None,
    ))
}

fn static_header(name: &'static str, value: &'static str) -> Result<Header, HttpError> {
    Header::from_bytes(name.as_bytes(), value.as_bytes())
        .map_err(|()| HttpError::InvalidStaticHeader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::ffi::OsString;
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv6Addr, Shutdown, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::MutexGuard;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const UI_BUNDLE_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
    const FOREIGN_BUNDLE_ID: &str = "660e8400-e29b-41d4-a716-446655440000";
    const FOREIGN_EVENT_ID: &str = "770e8400-e29b-41d4-a716-446655440000";
    const FOREIGN_CORRELATION_ID: &str = "880e8400-e29b-41d4-a716-446655440000";

    struct DataFixture {
        context: data::UiDataContext,
        home: std::path::PathBuf,
        node_ids: [i64; 3],
        _lock: MutexGuard<'static, ()>,
    }

    impl DataFixture {
        fn new() -> Self {
            let lock = crate::install::test_env_lock()
                .lock()
                .expect("test environment lock should not be poisoned");
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("test clock should be after Unix epoch")
                .as_nanos();
            let home = env::temp_dir().join(format!(
                "aopmem-ui-http-data-{}-{nonce}",
                std::process::id()
            ));
            let previous = env::var_os("AOPMEM_HOME");
            env::set_var("AOPMEM_HOME", &home);
            let paths = crate::storage::resolve_paths().expect("test paths should resolve");
            crate::storage::ensure_global_dirs(&paths).expect("test global dirs should create");
            let workspace_paths =
                crate::storage::ensure_workspace_dirs(&paths, "ui-test-workspace")
                    .expect("test workspace dirs should create");
            let connection = crate::storage::open_workspace_db(&workspace_paths)
                .expect("test workspace DB should initialize");
            let first = crate::storage::create_node(
                &connection,
                &crate::storage::NewNode {
                    node_type: "project_profile".to_string(),
                    status: "active".to_string(),
                    title: "Alpha project".to_string(),
                    summary: Some("First bounded summary".to_string()),
                    body: Some("FULL_BODY_CANARY".to_string()),
                    source_ref: Some("user:fixture".to_string()),
                    confidence: Some(0.9),
                    trust_level: Some("user".to_string()),
                },
            )
            .expect("first UI node should create");
            let second = crate::storage::create_node(
                &connection,
                &crate::storage::NewNode {
                    node_type: "workflow".to_string(),
                    status: "active".to_string(),
                    title: "Alpha workflow".to_string(),
                    summary: Some("Second bounded summary".to_string()),
                    body: Some("SECOND_BODY_CANARY".to_string()),
                    source_ref: Some("user:fixture".to_string()),
                    confidence: Some(0.8),
                    trust_level: Some("user".to_string()),
                },
            )
            .expect("second UI node should create");
            let third = crate::storage::create_node(
                &connection,
                &crate::storage::NewNode {
                    node_type: "failure_mode".to_string(),
                    status: "deprecated".to_string(),
                    title: "Alpha old failure".to_string(),
                    summary: Some("Third bounded summary".to_string()),
                    body: Some("THIRD_BODY_CANARY".to_string()),
                    source_ref: Some("user:fixture".to_string()),
                    confidence: Some(0.7),
                    trust_level: Some("user".to_string()),
                },
            )
            .expect("third UI node should create");
            for (source_node_id, target_node_id, link_type) in [
                (first.id, second.id, "uses"),
                (second.id, first.id, "supports"),
                (first.id, first.id, "self"),
                (third.id, first.id, "old_context"),
                (third.id, second.id, "old_context"),
            ] {
                crate::storage::create_link(
                    &connection,
                    &crate::storage::NewLink {
                        source_node_id,
                        target_node_id,
                        link_type: link_type.to_string(),
                    },
                )
                .expect("UI fixture link should create");
            }
            drop(connection);
            restore_env("AOPMEM_HOME", previous);
            Self {
                context: data::UiDataContext::new("ui-test-workspace".to_string(), workspace_paths),
                home,
                node_ids: [first.id, second.id, third.id],
                _lock: lock,
            }
        }

        fn seed_observability(&self) {
            use crate::observability::{
                CollectorEvent, CountItem, CountsPayload, EventOutcome, EventPayload, EventType,
                LocalCollector, RecallBundleNode, RecallBundleRecord, RecallPayload,
                SelectionReason, ToolPayload,
            };

            let nodes = self
                .node_ids
                .iter()
                .take(2)
                .map(|node_id| {
                    RecallBundleNode::new(
                        *node_id,
                        "workflow",
                        &format!("Bundle node {node_id}"),
                        Some("Bundle summary"),
                        Some("user:fixture"),
                        Some("user"),
                        Some(0.8),
                        Some(-1.0),
                        vec![SelectionReason::FtsBm25, SelectionReason::Workflow],
                    )
                    .expect("UI bundle node should validate")
                })
                .collect::<Vec<_>>();
            let record = RecallBundleRecord::success(UI_BUNDLE_ID, 12, true, false, nodes)
                .expect("UI bundle should validate");
            let recall_payload = RecallPayload::new(2, true, 0, true, true)
                .with_selected_node_ids(self.node_ids[..2].to_vec())
                .expect("selected node ids should validate")
                .with_selection_reasons(vec![SelectionReason::FtsBm25, SelectionReason::Workflow])
                .expect("selection reasons should validate");
            let recall_events = [
                CollectorEvent::new(
                    EventType::RecallStarted,
                    EventOutcome::Started,
                    EventPayload::Empty,
                )
                .expect("recall start should validate")
                .with_bundle_id(UI_BUNDLE_ID)
                .expect("recall start bundle id should validate"),
                CollectorEvent::new(
                    EventType::RecallCompleted,
                    EventOutcome::Success,
                    EventPayload::Recall(recall_payload),
                )
                .expect("recall completion should validate")
                .with_bundle_id(UI_BUNDLE_ID)
                .expect("recall completion bundle id should validate")
                .with_duration_ms(12)
                .expect("recall duration should validate"),
            ];
            let mut collector = LocalCollector::new(self.context.workspace_paths(), "ui-test")
                .expect("UI collector should construct");
            assert_eq!(
                collector.record_recall_bundle(&record, &recall_events),
                None
            );
            let tool_failure = CollectorEvent::new(
                EventType::ToolRunFailed,
                EventOutcome::Failure,
                EventPayload::Tool(
                    ToolPayload::new("ui-tool", false).expect("tool payload should validate"),
                ),
            )
            .expect("tool failure should validate")
            .with_error_code("UI_TOOL_FAILED")
            .expect("tool error should validate");
            assert_eq!(collector.record(&tool_failure), None);
            let doctor_counts = [("checks", 4), ("ready", 4), ("missing", 0), ("error", 0)]
                .into_iter()
                .map(|(name, count)| {
                    CountItem::new(name, count).expect("doctor count should validate")
                })
                .collect();
            let doctor = CollectorEvent::new(
                EventType::Doctor,
                EventOutcome::Success,
                EventPayload::Counts(
                    CountsPayload::new(doctor_counts).expect("doctor counts should validate"),
                ),
            )
            .expect("doctor event should validate");
            assert_eq!(collector.record(&doctor), None);
            let verify_counts = [
                ("total", 1),
                ("duplicate_ids", 0),
                ("broken_links", 0),
                ("deprecated_active_links", 0),
                ("missing_source", 0),
                ("missing_summary", 0),
                ("missing_gates", 1),
                ("adapter_block_drift", 0),
                ("schema_drift", 0),
                ("forbidden_feature_terms", 0),
                ("pending_audit_snapshot", 0),
            ]
            .into_iter()
            .map(|(name, count)| CountItem::new(name, count).expect("verify count should validate"))
            .collect();
            let verify = CollectorEvent::new(
                EventType::Verify,
                EventOutcome::Warning,
                EventPayload::Counts(
                    CountsPayload::new(verify_counts).expect("verify counts should validate"),
                ),
            )
            .expect("verify event should validate");
            assert_eq!(collector.record(&verify), None);
            drop(collector);

            let connection =
                rusqlite::Connection::open(self.context.workspace_paths().observability_db())
                    .expect("UI observability fixture should open");
            connection
                .execute(
                    "UPDATE bundle_nodes
                     SET node_title = 'Authorization: Bearer BUNDLE_SECRET',
                         bounded_summary = 'token=BUNDLE_SUMMARY_SECRET',
                         source_ref = 'cookie=BUNDLE_SOURCE_SECRET'
                     WHERE bundle_id = ?1",
                    [UI_BUNDLE_ID],
                )
                .expect("bundle redaction canaries should seed");
            connection
                .execute(
                    "UPDATE observability_events
                     SET product_version = 'Authorization: Bearer ACTIVITY_SECRET'
                     WHERE event_type = 'tool.run.failed'",
                    [],
                )
                .expect("activity redaction canary should seed");
            connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .expect("UI observability fixture should checkpoint");
        }

        fn seed_foreign_observability_rows(&self) {
            let connection =
                rusqlite::Connection::open(self.context.workspace_paths().observability_db())
                    .expect("UI observability fixture should open for contamination proof");
            connection
                .execute(
                    "INSERT INTO observability_events (
                        id, timestamp, product_version, workspace_key, event_type,
                        command, correlation_id, bundle_id, duration_ms, outcome,
                        error_code, payload_json
                     )
                     SELECT ?1, timestamp, product_version, 'foreign-workspace',
                            event_type, command, correlation_id, bundle_id,
                            duration_ms, outcome, error_code, payload_json
                     FROM observability_events WHERE event_type = 'doctor' LIMIT 1",
                    [FOREIGN_EVENT_ID],
                )
                .expect("foreign event should seed");
            connection
                .execute(
                    "INSERT INTO recall_bundles (
                        bundle_id, timestamp, product_version, workspace_key,
                        correlation_id, outcome, error_code, duration_ms,
                        more_results, continuation_count
                     ) VALUES (
                        ?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?2,
                        'foreign-workspace', ?3, 'success', NULL, 1, 0, 0
                     )",
                    rusqlite::params![
                        FOREIGN_BUNDLE_ID,
                        env!("CARGO_PKG_VERSION"),
                        FOREIGN_CORRELATION_ID
                    ],
                )
                .expect("foreign bundle should seed");
            connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .expect("foreign observability rows should checkpoint");
        }

        fn seed_tools_and_mcp(&self) {
            let connection = crate::storage::open_workspace_db(self.context.workspace_paths())
                .expect("UI tools fixture DB should open");
            for (tool_id, name, status, side_effects, approval, canary) in [
                (
                    "alpha-tool",
                    "Alpha tool",
                    "active",
                    "none",
                    "none",
                    "ALPHA_CONTRACT_SECRET",
                ),
                (
                    "omega-tool",
                    "Omega tool",
                    "draft",
                    "external_write",
                    "explicit",
                    "OMEGA_CONTRACT_SECRET",
                ),
            ] {
                connection
                    .execute(
                        "INSERT INTO tool_contracts (
                            tool_id, name, status, owner_workflow, side_effects,
                            approval_requirement, contract_json
                         ) VALUES (?1, ?2, ?3, 'fixture-workflow', ?4, ?5, ?6)",
                        rusqlite::params![
                            tool_id,
                            name,
                            status,
                            side_effects,
                            approval,
                            format!(r#"{{"secret":"{canary}"}}"#)
                        ],
                    )
                    .expect("UI tool fixture should insert");
            }
            for (id, name, kind, status, side_effects, approval, credential_canary, notes_canary) in [
                (
                    "alpha-mcp",
                    "Alpha MCP",
                    "stdio",
                    "installed",
                    "local_read",
                    "none",
                    "ALPHA_CREDENTIAL_SECRET",
                    "ALPHA_NOTES_SECRET",
                ),
                (
                    "omega-mcp",
                    "Omega MCP",
                    "http",
                    "configured_unverified",
                    "external_read",
                    "none",
                    "OMEGA_CREDENTIAL_SECRET",
                    "OMEGA_NOTES_SECRET",
                ),
            ] {
                connection
                    .execute(
                        "INSERT INTO mcp_profiles (
                            id, name, kind, status, read_operations,
                            write_operations, side_effects,
                            approval_requirement, credentials_source, notes
                         ) VALUES (
                            ?1, ?2, ?3, ?4, 'read', 'none', ?5, ?6, ?7, ?8
                         )",
                        rusqlite::params![
                            id,
                            name,
                            kind,
                            status,
                            side_effects,
                            approval,
                            credential_canary,
                            notes_canary
                        ],
                    )
                    .expect("UI MCP fixture should insert");
            }
            connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .expect("UI operational fixture should checkpoint");
        }

        fn seed_graph_boundaries(&self) {
            let mut connection = crate::storage::open_workspace_db(self.context.workspace_paths())
                .expect("UI graph boundary DB should open");
            let transaction = connection
                .transaction()
                .expect("UI graph boundary transaction should start");
            for ordinal in 0..198 {
                transaction
                    .execute(
                        "INSERT INTO nodes (
                            node_type, status, title, summary, body,
                            source_ref, confidence, trust_level
                         ) VALUES (
                            'project_fact', 'active', ?1, 'Boundary summary',
                            NULL, 'test:graph-boundary', 0.5, 'test'
                         )",
                        [format!("Boundary node {ordinal:03}")],
                    )
                    .expect("UI graph boundary node should insert");
            }
            let center_id = transaction.last_insert_rowid();
            for ordinal in 0..496 {
                transaction
                    .execute(
                        "INSERT INTO links (
                            source_node_id, target_node_id, link_type
                         ) VALUES (1, 2, ?1)",
                        [format!("boundary_{ordinal:03}")],
                    )
                    .expect("UI graph boundary edge should insert");
            }
            for target_node_id in 1..=200 {
                transaction
                    .execute(
                        "INSERT INTO links (
                            source_node_id, target_node_id, link_type
                         ) VALUES (?1, ?2, ?3)",
                        rusqlite::params![
                            center_id,
                            target_node_id,
                            format!("center_{target_node_id:03}")
                        ],
                    )
                    .expect("UI centered boundary edge should insert");
            }
            transaction
                .commit()
                .expect("UI graph boundary transaction should commit");
            connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .expect("UI graph boundary DB should checkpoint");
        }
    }

    impl Drop for DataFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.home).expect("test UI data home should remove");
        }
    }

    fn restore_env(name: &str, previous: Option<OsString>) {
        match previous {
            Some(value) => env::set_var(name, value),
            None => env::remove_var(name),
        }
    }

    fn fixture_token() -> SessionToken {
        SessionToken::from_bytes([0xab; 32])
    }

    fn fixture_context() -> data::UiDataContext {
        let paths = crate::storage::resolve_paths().expect("test paths should resolve");
        data::UiDataContext::new(
            "ui-test-workspace".to_string(),
            crate::storage::workspace_paths_for_key(&paths, "ui-test-workspace"),
        )
    }

    fn test_route(method: &Method, url: &str, token: &SessionToken) -> ResponseSpec {
        route(method, url, token, &fixture_context()).expect("test route should serialize")
    }

    fn raw_request(address: SocketAddrV4, request: &str) -> String {
        let mut stream =
            TcpStream::connect(address).expect("loopback UI should accept connections");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout should configure");
        stream
            .write_all(request.as_bytes())
            .expect("HTTP request should write");
        stream
            .shutdown(Shutdown::Write)
            .expect("HTTP request write side should close");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("HTTP response should read");
        response
    }

    fn status(response: &str) -> &str {
        response
            .lines()
            .next()
            .expect("HTTP response should have a status line")
    }

    fn body(response: &str) -> &str {
        response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .expect("HTTP response should have a body separator")
    }

    fn decoded_body(response: &str) -> Vec<u8> {
        let (headers, encoded_body) = response
            .split_once("\r\n\r\n")
            .expect("HTTP response should have a body separator");
        if !headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Transfer-Encoding: chunked"))
        {
            return encoded_body.as_bytes().to_vec();
        }

        let mut encoded = encoded_body.as_bytes();
        let mut decoded = Vec::new();
        loop {
            let line_end = encoded
                .windows(2)
                .position(|window| window == b"\r\n")
                .expect("chunk size should end with CRLF");
            let length = std::str::from_utf8(&encoded[..line_end])
                .ok()
                .and_then(|value| value.split(';').next())
                .and_then(|value| usize::from_str_radix(value, 16).ok())
                .expect("chunk size should be hexadecimal");
            encoded = &encoded[line_end + 2..];
            if length == 0 {
                assert!(encoded.starts_with(b"\r\n"));
                break;
            }
            assert!(encoded.len() >= length + 2);
            decoded.extend_from_slice(&encoded[..length]);
            assert_eq!(&encoded[length..length + 2], b"\r\n");
            encoded = &encoded[length + 2..];
        }
        decoded
    }

    fn json_spec(spec: &ResponseSpec) -> serde_json::Value {
        serde_json::from_slice(&spec.body).expect("UI response should contain JSON")
    }

    #[derive(Debug, PartialEq, Eq)]
    struct DatabaseFingerprint {
        bytes: Vec<u8>,
        size: u64,
        modified: SystemTime,
        schema: Vec<(String, String, Option<String>)>,
        counts: Vec<(String, i64)>,
    }

    fn database_fingerprint(path: &std::path::Path, tables: &[&str]) -> DatabaseFingerprint {
        let canonical_path = path
            .parent()
            .expect("fingerprint database should have a parent")
            .canonicalize()
            .expect("fingerprint database parent should canonicalize")
            .join(
                path.file_name()
                    .expect("fingerprint database should have a file name"),
            );
        let connection = rusqlite::Connection::open_with_flags(
            canonical_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .expect("fingerprint database should open read-only");
        let schema = {
            let mut statement = connection
                .prepare(
                    "SELECT type, name, sql FROM sqlite_schema
                     WHERE name NOT LIKE 'sqlite_%'
                     ORDER BY type, name",
                )
                .expect("fingerprint schema query should prepare");
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .expect("fingerprint schema should query")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("fingerprint schema should collect")
        };
        let counts = tables
            .iter()
            .map(|table| {
                let count = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .expect("fingerprint count should query");
                ((*table).to_string(), count)
            })
            .collect();
        drop(connection);
        let metadata = std::fs::metadata(path).expect("fingerprint metadata should read");
        DatabaseFingerprint {
            bytes: std::fs::read(path).expect("fingerprint bytes should read"),
            size: metadata.len(),
            modified: metadata.modified().expect("fingerprint mtime should read"),
            schema,
            counts,
        }
    }

    #[test]
    fn bind_config_rejects_every_address_except_exact_ipv4_localhost() {
        assert_eq!(
            BindConfig::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
                .expect("exact IPv4 localhost should be accepted"),
            BindConfig::loopback(0)
        );
        for address in [
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        ] {
            assert!(matches!(
                BindConfig::new(address, 0),
                Err(HttpError::InvalidBindAddress)
            ));
        }
    }

    #[test]
    fn requested_nonzero_port_is_honored_and_busy_port_fails_closed() {
        let occupied = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .expect("occupied-port fixture should bind");
        let port = occupied
            .local_addr()
            .expect("occupied-port address should resolve")
            .port();

        assert!(matches!(
            HttpServer::bind(BindConfig::loopback(port), Arc::new(fixture_context())),
            Err(HttpError::Bind(_))
        ));
    }

    #[test]
    fn session_token_is_32_random_bytes_as_lowercase_hex_and_debug_is_redacted() {
        let deterministic = fixture_token();
        assert_eq!(deterministic.as_str().len(), 64);
        assert!(deterministic
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        let debug = format!("{deterministic:?}");
        assert_eq!(debug, "SessionToken([REDACTED])");
        assert!(!debug.contains(deterministic.as_str()));

        let generated = SessionToken::generate().expect("OS randomness should be available");
        assert_eq!(generated.as_str().len(), 64);
        assert!(generated
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    }

    #[test]
    fn routing_authenticates_before_method_and_uses_exact_asset_allowlist() {
        let token = fixture_token();
        let valid_index = format!("/{}/", token.as_str());
        assert_eq!(test_route(&Method::Get, &valid_index, &token).status.0, 200);

        for method in [
            Method::Head,
            Method::Post,
            Method::Put,
            Method::Delete,
            Method::Connect,
            Method::Options,
            Method::Trace,
            Method::Patch,
        ] {
            let response = test_route(&method, &valid_index, &token);
            assert_eq!(response.status.0, 405, "method {method} must be rejected");
            assert!(response.allow_get);
        }

        let missing = test_route(&Method::Post, "/", &token);
        let invalid = test_route(&Method::Post, "/wrong/", &token);
        assert_eq!(missing.status.0, 404);
        assert_eq!(invalid.status.0, 404);
        assert_eq!(missing.body, invalid.body);
        assert!(!missing.allow_get);
        assert!(!invalid.allow_get);

        for suffix in [
            "../index.html",
            "%2e%2e/index.html",
            "app.js?query=1",
            "app.js#fragment",
            "api/nodes",
            "index.html/extra",
            "",
        ] {
            let url = if suffix.is_empty() {
                format!("/{}", token.as_str())
            } else {
                format!("/{}/{suffix}", token.as_str())
            };
            assert_eq!(
                test_route(&Method::Get, &url, &token).status.0,
                404,
                "non-allowlisted path must not be served: {url}"
            );
        }
    }

    #[test]
    fn static_response_bodies_are_borrowed_and_json_bodies_are_owned() {
        let token = fixture_token();
        for path in ["", "app.css", "app.js"] {
            let url = format!("/{}/{path}", token.as_str());
            let response = test_route(&Method::Get, &url, &token);
            let asset = assets::for_path(path).expect("test asset should be allowlisted");
            assert_eq!(response.status.0, 200);
            assert_eq!(response.content_type, asset.content_type);
            assert!(!response.allow_get);
            match response.body {
                Cow::Borrowed(body) => assert_eq!(body, asset.body),
                Cow::Owned(_) => panic!("static asset body must stay borrowed"),
            }
        }

        let not_found = test_route(&Method::Get, "/wrong/", &token);
        match not_found.body {
            Cow::Borrowed(body) => assert_eq!(body, NOT_FOUND_BODY),
            Cow::Owned(_) => panic!("fixed 404 body must stay borrowed"),
        }

        let method_not_allowed = test_route(
            &Method::Post,
            &format!("/{}/app.js", token.as_str()),
            &token,
        );
        assert!(method_not_allowed.allow_get);
        match method_not_allowed.body {
            Cow::Borrowed(body) => assert_eq!(body, METHOD_NOT_ALLOWED_BODY),
            Cow::Owned(_) => panic!("fixed 405 body must stay borrowed"),
        }

        let json = json_response(StatusCode(200), &serde_json::json!({ "ok": true }))
            .expect("JSON response should serialize");
        assert_eq!(json.content_type, "application/json; charset=utf-8");
        match json.body {
            Cow::Owned(body) => assert_eq!(body.as_slice(), br#"{"ok":true}"#),
            Cow::Borrowed(_) => panic!("serialized JSON body must stay owned"),
        }
    }

    #[test]
    fn live_server_serves_all_assets_with_security_headers_and_stops_internally() {
        let token = fixture_token();
        let server = HttpServer::bind_with_token(
            BindConfig::loopback(0),
            token.clone(),
            Arc::new(fixture_context()),
        )
        .expect("test UI server should bind");
        assert_eq!(*server.address().ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(server.address().port(), 0);
        let address = server.address();
        let unblocker = Arc::clone(&server.server);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || server.serve_until(thread_stop.as_ref()));

        let mut index_response = None;
        for (path, content_type) in [
            ("", "text/html; charset=utf-8"),
            ("app.css", "text/css; charset=utf-8"),
            ("app.js", "application/javascript; charset=utf-8"),
        ] {
            let asset = assets::for_path(path).expect("live test asset should be allowlisted");
            let response = raw_request(
                address,
                &format!(
                    "GET /{}/{path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                    token.as_str()
                ),
            );
            assert!(status(&response).contains(" 200 "));
            assert!(response.contains(&format!("Content-Type: {content_type}\r\n")));
            assert!(response.contains(&format!(
                "Content-Security-Policy: {CONTENT_SECURITY_POLICY}\r\n"
            )));
            assert!(response.contains("X-Content-Type-Options: nosniff\r\n"));
            assert!(response.contains("Referrer-Policy: no-referrer\r\n"));
            assert!(response.contains("Cache-Control: no-store\r\n"));
            if let Some(content_length) = response
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length: "))
            {
                assert_eq!(content_length.parse::<usize>().ok(), Some(asset.body.len()));
            } else {
                assert!(response.contains("Transfer-Encoding: chunked\r\n"));
            }
            assert!(!response.to_ascii_lowercase().contains("access-control-"));
            assert_eq!(decoded_body(&response), asset.body);
            if path.is_empty() {
                index_response = Some(response);
            }
        }

        let missing = raw_request(
            address,
            "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        );
        let invalid = raw_request(
            address,
            "POST /wrong/ HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        );
        assert!(status(&missing).contains(" 404 "));
        assert!(status(&invalid).contains(" 404 "));
        assert_eq!(body(&missing), body(&invalid));
        assert_eq!(body(&missing), "Not Found\n");
        assert!(missing.contains(&format!("Content-Length: {}\r\n", NOT_FOUND_BODY.len())));

        let post = raw_request(
            address,
            &format!(
                "POST /{}/app.js HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                token.as_str()
            ),
        );
        assert!(status(&post).contains(" 405 "));
        assert!(post.contains("Allow: GET\r\n"));
        assert_eq!(body(&post), "Method Not Allowed\n");
        assert!(post.contains(&format!(
            "Content-Length: {}\r\n",
            METHOD_NOT_ALLOWED_BODY.len()
        )));

        let index_response = index_response.expect("index response should be captured");
        assert!(body(&index_response).contains("Local read-only browser UI."));

        stop.store(true, Ordering::Release);
        unblocker.unblock();
        worker
            .join()
            .expect("UI test server thread should join")
            .expect("UI test server should stop cleanly");
    }

    #[test]
    fn live_bootstrap_authenticates_first_and_returns_bounded_json() {
        let fixture = DataFixture::new();
        let token = fixture_token();
        let server = HttpServer::bind_with_token(
            BindConfig::loopback(0),
            token.clone(),
            Arc::new(fixture.context.clone()),
        )
        .expect("test UI server should bind");
        let address = server.address();
        let unblocker = Arc::clone(&server.server);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || server.serve_until(thread_stop.as_ref()));

        let unauthorized = raw_request(
            address,
            "POST /wrong/api/v1/bootstrap HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        );
        assert!(status(&unauthorized).contains(" 404 "));
        assert_eq!(body(&unauthorized), "Not Found\n");

        let post = raw_request(
            address,
            &format!(
                "POST /{}/api/v1/bootstrap HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                token.as_str()
            ),
        );
        assert!(status(&post).contains(" 405 "));
        assert!(post.contains("Content-Type: application/json; charset=utf-8\r\n"));
        assert!(post.contains("Allow: GET\r\n"));
        assert!(post.contains("Cache-Control: no-store\r\n"));

        let get = raw_request(
            address,
            &format!(
                "GET /{}/api/v1/bootstrap HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                token.as_str()
            ),
        );
        assert!(status(&get).contains(" 200 "));
        assert!(get.contains("Content-Type: application/json; charset=utf-8\r\n"));
        assert!(get.contains(&format!(
            "Content-Security-Policy: {CONTENT_SECURITY_POLICY}\r\n"
        )));
        assert!(get.contains("X-Content-Type-Options: nosniff\r\n"));
        assert!(get.contains("Referrer-Policy: no-referrer\r\n"));
        assert!(get.contains("Cache-Control: no-store\r\n"));
        assert!(!get.to_ascii_lowercase().contains("access-control-"));
        let json: serde_json::Value =
            serde_json::from_str(body(&get)).expect("bootstrap body should be JSON");
        assert_eq!(json["api_version"], "v1");
        assert_eq!(json["product_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(json["workspace_key"], "ui-test-workspace");
        assert_eq!(json["read_only"], true);
        assert_eq!(json["observability_available"], false);
        assert_eq!(json["capabilities"].as_array().map(Vec::len), Some(11));
        assert!(!body(&get).contains(token.as_str()));
        assert!(!fixture.context.workspace_paths().observability().exists());

        stop.store(true, Ordering::Release);
        unblocker.unblock();
        worker
            .join()
            .expect("UI test server thread should join")
            .expect("UI test server should stop cleanly");
    }

    #[test]
    fn api_route_order_and_query_parser_fail_closed() {
        let token = fixture_token();
        let token = token.as_str();

        let unknown = test_route(
            &Method::Post,
            &format!("/{token}/api/v1/not-real?bad=%ZZ"),
            &fixture_token(),
        );
        assert_eq!(unknown.status.0, 404);
        assert_eq!(json_spec(&unknown)["error"]["code"], "UI_API_NOT_FOUND");

        let known_post = test_route(
            &Method::Post,
            &format!("/{token}/api/v1/memory?bad=%ZZ"),
            &fixture_token(),
        );
        assert_eq!(known_post.status.0, 405);
        assert_eq!(
            json_spec(&known_post)["error"]["code"],
            "UI_API_METHOD_NOT_ALLOWED"
        );

        for query in [
            "limit=1&limit=2",
            "unknown=1",
            "q=%ZZ",
            "q=%FF",
            "q=%00",
            "limit=0",
            "limit=501",
            "limit=184467440737095516160",
        ] {
            let response = test_route(
                &Method::Get,
                &format!("/{token}/api/v1/memory?{query}"),
                &fixture_token(),
            );
            assert_eq!(response.status.0, 400, "query must fail: {query}");
            assert_eq!(json_spec(&response)["error"]["code"], "UI_INVALID_REQUEST");
        }
    }

    #[test]
    fn memory_node_links_and_graph_are_bounded_and_body_safe() {
        let fixture = DataFixture::new();
        let token = fixture_token();
        let prefix = format!("/{}/api/v1", token.as_str());

        let empty = route(
            &Method::Get,
            &format!("{prefix}/memory?type=rule"),
            &token,
            &fixture.context,
        )
        .expect("empty memory response should serialize");
        let empty_json = json_spec(&empty);
        assert_eq!(empty_json["body_omitted"], true);
        assert_eq!(empty_json["complete"], true);
        assert_eq!(empty_json["items"].as_array().map(Vec::len), Some(0));

        let first = route(
            &Method::Get,
            &format!("{prefix}/memory?limit=1&q=Alpha"),
            &token,
            &fixture.context,
        )
        .expect("first memory page should serialize");
        let first_json = json_spec(&first);
        assert_eq!(first.status.0, 200);
        assert_eq!(first_json["limit"], 1);
        assert_eq!(first_json["more_results"], true);
        assert_eq!(first_json["complete"], false);
        assert_eq!(first_json["body_omitted"], true);
        assert!(first_json["items"][0].get("body").is_none());
        assert!(!String::from_utf8_lossy(&first.body).contains("FULL_BODY_CANARY"));
        let cursor = first_json["next_cursor"]
            .as_str()
            .expect("incomplete memory page should have cursor");

        let second = route(
            &Method::Get,
            &format!("{prefix}/memory?limit=1&q=Alpha&cursor={cursor}"),
            &token,
            &fixture.context,
        )
        .expect("second memory page should serialize");
        assert_eq!(second.status.0, 200);
        assert_ne!(
            json_spec(&first)["items"][0]["id"],
            json_spec(&second)["items"][0]["id"]
        );

        let wrong_scope = route(
            &Method::Get,
            &format!("{prefix}/memory?limit=1&q=Other&cursor={cursor}"),
            &token,
            &fixture.context,
        )
        .expect("wrong-scope cursor response should serialize");
        assert_eq!(wrong_scope.status.0, 400);
        assert_eq!(
            json_spec(&wrong_scope)["error"]["code"],
            "UI_INVALID_CURSOR"
        );

        let deprecated = route(
            &Method::Get,
            &format!("{prefix}/memory?q=Alpha&status=deprecated"),
            &token,
            &fixture.context,
        )
        .expect("deprecated FTS result should serialize");
        let deprecated_json = json_spec(&deprecated);
        assert_eq!(deprecated_json["items"].as_array().map(Vec::len), Some(1));
        assert_eq!(deprecated_json["items"][0]["id"], fixture.node_ids[2]);
        let combined_filters = route(
            &Method::Get,
            &format!("{prefix}/memory?type=failure_mode&status=deprecated&q=Alpha"),
            &token,
            &fixture.context,
        )
        .expect("combined memory filters should serialize");
        assert_eq!(
            json_spec(&combined_filters)["items"][0]["id"],
            fixture.node_ids[2]
        );
        let mismatched_filters = route(
            &Method::Get,
            &format!("{prefix}/memory?type=workflow&status=deprecated&q=Alpha"),
            &token,
            &fixture.context,
        )
        .expect("mismatched memory filters should serialize");
        assert_eq!(
            json_spec(&mismatched_filters)["items"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );

        let node = route(
            &Method::Get,
            &format!("{prefix}/node?id={}", fixture.node_ids[0]),
            &token,
            &fixture.context,
        )
        .expect("node detail should serialize");
        assert_eq!(json_spec(&node)["node"]["body"], "FULL_BODY_CANARY");

        let links = route(
            &Method::Get,
            &format!("{prefix}/node-links?id={}", fixture.node_ids[0]),
            &token,
            &fixture.context,
        )
        .expect("node links should serialize");
        let links_json = json_spec(&links);
        assert_eq!(links_json["limit"], data::DEFAULT_PAGE_SIZE);
        assert!(links_json["items"]
            .as_array()
            .expect("links should be an array")
            .iter()
            .any(|item| item["direction"] == "both"));

        let graph = route(
            &Method::Get,
            &format!("{prefix}/graph?center={}", fixture.node_ids[0]),
            &token,
            &fixture.context,
        )
        .expect("centered graph should serialize");
        let graph_json = json_spec(&graph);
        assert!(graph_json["nodes"].as_array().map(Vec::len) <= Some(data::MAX_GRAPH_NODES));
        assert!(graph_json["edges"].as_array().map(Vec::len) <= Some(data::MAX_GRAPH_EDGES));
        assert!(graph_json.get("nodes_more_results").is_some());
        assert!(graph_json.get("edges_more_results").is_some());
        assert!(graph_json.get("nodes_complete").is_some());
        assert!(graph_json.get("edges_complete").is_some());

        let mut cursor = None;
        let mut first_cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        let mut pages = 0;
        loop {
            let cursor_query = cursor
                .as_deref()
                .map_or_else(String::new, |cursor| format!("&cursor={cursor}"));
            let page = route(
                &Method::Get,
                &format!(
                    "{prefix}/graph?center={}&limit=1{cursor_query}",
                    fixture.node_ids[2]
                ),
                &token,
                &fixture.context,
            )
            .expect("center graph page should serialize");
            let page_json = json_spec(&page);
            pages += 1;
            assert_eq!(page_json["center_node"]["id"], fixture.node_ids[2]);
            assert!(
                page_json["nodes"]
                    .as_array()
                    .map(Vec::len)
                    .unwrap_or_default()
                    < data::MAX_GRAPH_NODES
            );
            let node_id = page_json["nodes"][0]["id"]
                .as_i64()
                .expect("center graph node id should be an integer");
            assert!(
                seen.insert(node_id),
                "center graph duplicated node {node_id}"
            );
            if pages == 1 {
                assert_eq!(node_id, fixture.node_ids[2]);
            } else {
                assert!(page_json["edges"]
                    .as_array()
                    .expect("continued graph edges should be an array")
                    .iter()
                    .any(|edge| {
                        edge["source_node_id"] == fixture.node_ids[2]
                            || edge["target_node_id"] == fixture.node_ids[2]
                    }));
            }
            let allowed_endpoints = page_json["nodes"]
                .as_array()
                .expect("center graph nodes should be an array")
                .iter()
                .filter_map(|node| node["id"].as_i64())
                .chain(std::iter::once(fixture.node_ids[2]))
                .collect::<std::collections::BTreeSet<_>>();
            for edge in page_json["edges"]
                .as_array()
                .expect("center graph edges should be an array")
            {
                assert!(allowed_endpoints.contains(
                    &edge["source_node_id"]
                        .as_i64()
                        .expect("edge source should be an integer")
                ));
                assert!(allowed_endpoints.contains(
                    &edge["target_node_id"]
                        .as_i64()
                        .expect("edge target should be an integer")
                ));
            }
            if page_json["nodes_more_results"] == true {
                let next = page_json["nodes_next_cursor"]
                    .as_str()
                    .expect("incomplete center page should have a cursor")
                    .to_string();
                first_cursor.get_or_insert_with(|| next.clone());
                cursor = Some(next);
            } else {
                assert_eq!(page_json["nodes_next_cursor"], serde_json::Value::Null);
                assert_eq!(page_json["nodes_complete"], true);
                break;
            }
        }
        assert_eq!(pages, 3);
        assert_eq!(
            seen,
            fixture
                .node_ids
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
        let first_cursor = first_cursor.expect("center traversal should produce a cursor");
        for changed_scope in [
            format!(
                "center={}&limit=1&cursor={first_cursor}",
                fixture.node_ids[1]
            ),
            format!(
                "center={}&limit=1&status=active&cursor={first_cursor}",
                fixture.node_ids[2]
            ),
        ] {
            let response = route(
                &Method::Get,
                &format!("{prefix}/graph?{changed_scope}"),
                &token,
                &fixture.context,
            )
            .expect("wrong-scope graph cursor should serialize");
            assert_eq!(response.status.0, 400);
            assert_eq!(json_spec(&response)["error"]["code"], "UI_INVALID_CURSOR");
        }
    }

    #[test]
    fn centered_graph_preserves_missing_cursor_and_corrupt_data_error_order() {
        let fixture = DataFixture::new();
        let token = fixture_token();
        let prefix = format!("/{}/api/v1/graph", token.as_str());
        let invalid_cursor = "not-a-center-cursor";

        let missing = route(
            &Method::Get,
            &format!("{prefix}?center=999999&cursor={invalid_cursor}"),
            &token,
            &fixture.context,
        )
        .expect("missing centered graph should serialize");
        assert_eq!(missing.status.0, 404);
        assert_eq!(
            json_spec(&missing),
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "UI_NODE_NOT_FOUND",
                    "message": "Memory node was not found"
                }
            })
        );

        let connection = crate::storage::open_workspace_db(fixture.context.workspace_paths())
            .expect("UI error-order DB should open");
        connection
            .execute(
                "UPDATE nodes SET node_type = 'invalid-ui-test-type' WHERE id = ?1",
                [fixture.node_ids[0]],
            )
            .expect("UI center semantic-corruption fixture should update");
        drop(connection);

        let invalid = route(
            &Method::Get,
            &format!(
                "{prefix}?center={}&cursor={invalid_cursor}",
                fixture.node_ids[0]
            ),
            &token,
            &fixture.context,
        )
        .expect("invalid centered graph cursor should serialize");
        assert_eq!(invalid.status.0, 400);
        assert_eq!(
            json_spec(&invalid),
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "UI_INVALID_CURSOR",
                    "message": "Pagination cursor is invalid"
                }
            })
        );

        let connection = crate::storage::open_workspace_db(fixture.context.workspace_paths())
            .expect("UI unreadable-center DB should open");
        connection
            .execute(
                "UPDATE nodes SET created_at = ?1 WHERE id = ?2",
                rusqlite::params![vec![0xff_u8], fixture.node_ids[0]],
            )
            .expect("UI center read-corruption fixture should update");
        drop(connection);

        let unreadable = route(
            &Method::Get,
            &format!(
                "{prefix}?center={}&cursor={invalid_cursor}",
                fixture.node_ids[0]
            ),
            &token,
            &fixture.context,
        )
        .expect("unreadable centered graph should serialize");
        assert_eq!(unreadable.status.0, 500);
        assert_eq!(
            json_spec(&unreadable),
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "UI_DATA_UNAVAILABLE",
                    "message": "Local UI data is unavailable"
                }
            })
        );
    }

    #[test]
    fn missing_observability_is_not_collected_and_is_never_created() {
        let fixture = DataFixture::new();
        let token = fixture_token();
        let prefix = format!("/{}/api/v1", token.as_str());
        let observability_path = fixture.context.workspace_paths().observability();
        assert!(!observability_path.exists());

        for (endpoint, expected_path) in [
            ("overview", vec!["observability", "collection_status"]),
            ("activity", vec!["collection_status"]),
            ("effectiveness", vec!["collection_status"]),
        ] {
            let response = route(
                &Method::Get,
                &format!("{prefix}/{endpoint}"),
                &token,
                &fixture.context,
            )
            .expect("missing-observability response should serialize");
            assert_eq!(response.status.0, 200);
            let json = json_spec(&response);
            let mut value = &json;
            for segment in expected_path {
                value = &value[segment];
            }
            assert_eq!(value, "not_collected");
            assert!(!observability_path.exists());
        }

        for id in [
            "not-a-uuid",
            "550E8400-E29B-41D4-A716-446655440000",
            "550e8400-e29b-11d4-a716-446655440000",
        ] {
            let response = route(
                &Method::Get,
                &format!("{prefix}/bundle?id={id}"),
                &token,
                &fixture.context,
            )
            .expect("invalid bundle response should serialize");
            assert_eq!(response.status.0, 400);
            assert_eq!(json_spec(&response)["error"]["code"], "UI_INVALID_REQUEST");
            assert!(!observability_path.exists());
        }
        let unknown = route(
            &Method::Get,
            &format!("{prefix}/bundle?id={UI_BUNDLE_ID}"),
            &token,
            &fixture.context,
        )
        .expect("unknown bundle response should serialize");
        assert_eq!(unknown.status.0, 404);
        assert_eq!(json_spec(&unknown)["error"]["code"], "UI_BUNDLE_NOT_FOUND");
        assert!(!observability_path.exists());
    }

    #[test]
    fn activity_bundle_overview_and_effectiveness_are_private_and_correlated() {
        let fixture = DataFixture::new();
        fixture.seed_observability();
        let token = fixture_token();
        let prefix = format!("/{}/api/v1", token.as_str());

        let first = route(
            &Method::Get,
            &format!("{prefix}/activity?limit=2"),
            &token,
            &fixture.context,
        )
        .expect("activity first page should serialize");
        let first_json = json_spec(&first);
        assert_eq!(first.status.0, 200);
        assert_eq!(first_json["collection_status"], "ready");
        assert_eq!(first_json["limit"], 2);
        assert_eq!(first_json["more_results"], true);
        assert_eq!(first_json["items"].as_array().map(Vec::len), Some(2));
        let first_body = String::from_utf8_lossy(&first.body);
        for forbidden in [
            "payload_json",
            "ACTIVITY_SECRET",
            "BUNDLE_SECRET",
            "BUNDLE_SUMMARY_SECRET",
            "BUNDLE_SOURCE_SECRET",
            FOREIGN_EVENT_ID,
            "foreign-workspace",
        ] {
            assert!(
                !first_body.contains(forbidden),
                "activity leaked {forbidden}"
            );
        }
        let redacted_activity = route(
            &Method::Get,
            &format!("{prefix}/activity?event=tool.run.failed"),
            &token,
            &fixture.context,
        )
        .expect("targeted activity response should serialize");
        let redacted_body = String::from_utf8_lossy(&redacted_activity.body);
        assert_eq!(
            json_spec(&redacted_activity)["items"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert!(redacted_body.contains("[REDACTED]"));
        assert!(!redacted_body.contains("ACTIVITY_SECRET"));
        assert!(!redacted_body.contains("payload_json"));
        let combined_filters = route(
            &Method::Get,
            &format!("{prefix}/activity?event=tool.run.failed&outcome=failure&command=ui-test"),
            &token,
            &fixture.context,
        )
        .expect("combined activity filters should serialize");
        assert_eq!(
            json_spec(&combined_filters)["items"],
            json_spec(&redacted_activity)["items"]
        );
        let mismatched_filters = route(
            &Method::Get,
            &format!("{prefix}/activity?event=tool.run.failed&outcome=success&command=ui-test"),
            &token,
            &fixture.context,
        )
        .expect("mismatched activity filters should serialize");
        assert_eq!(
            json_spec(&mismatched_filters)["items"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );
        let cursor = first_json["next_cursor"]
            .as_str()
            .expect("activity first page should have a cursor");
        let second = route(
            &Method::Get,
            &format!("{prefix}/activity?limit=2&cursor={cursor}"),
            &token,
            &fixture.context,
        )
        .expect("activity second page should serialize");
        let first_ids = first_json["items"]
            .as_array()
            .expect("first activity items should be an array")
            .iter()
            .map(|item| item["id"].as_str().expect("event id should be a string"))
            .collect::<std::collections::BTreeSet<_>>();
        for item in json_spec(&second)["items"]
            .as_array()
            .expect("second activity items should be an array")
        {
            assert!(!first_ids.contains(item["id"].as_str().expect("event id should be a string")));
        }
        let wrong_scope = route(
            &Method::Get,
            &format!("{prefix}/activity?limit=2&event=doctor&cursor={cursor}"),
            &token,
            &fixture.context,
        )
        .expect("wrong-scope activity cursor should serialize");
        assert_eq!(wrong_scope.status.0, 400);
        assert_eq!(
            json_spec(&wrong_scope)["error"]["code"],
            "UI_INVALID_CURSOR"
        );

        let bundle = route(
            &Method::Get,
            &format!("{prefix}/bundle?id={UI_BUNDLE_ID}&limit=1"),
            &token,
            &fixture.context,
        )
        .expect("bundle first page should serialize");
        let bundle_json = json_spec(&bundle);
        assert_eq!(bundle_json["bundle"]["bundle_id"], UI_BUNDLE_ID);
        assert_eq!(bundle_json["limit"], 1);
        assert_eq!(bundle_json["more_results"], true);
        assert_eq!(bundle_json["nodes"].as_array().map(Vec::len), Some(1));
        let bundle_body = String::from_utf8_lossy(&bundle.body);
        assert!(bundle_body.contains("[REDACTED]"));
        for secret in [
            "BUNDLE_SECRET",
            "BUNDLE_SUMMARY_SECRET",
            "BUNDLE_SOURCE_SECRET",
        ] {
            assert!(!bundle_body.contains(secret));
        }
        let bundle_cursor = bundle_json["next_cursor"]
            .as_str()
            .expect("bundle first page should have a cursor");
        let bundle_second = route(
            &Method::Get,
            &format!("{prefix}/bundle?id={UI_BUNDLE_ID}&limit=1&cursor={bundle_cursor}"),
            &token,
            &fixture.context,
        )
        .expect("bundle second page should serialize");
        assert_ne!(
            bundle_json["nodes"][0]["node_id"],
            json_spec(&bundle_second)["nodes"][0]["node_id"]
        );
        let overview = route(
            &Method::Get,
            &format!("{prefix}/overview"),
            &token,
            &fixture.context,
        )
        .expect("overview should serialize");
        let overview_json = json_spec(&overview);
        assert_eq!(overview_json["observability_available"], true);
        assert_eq!(overview_json["observability"]["collection_status"], "ready");
        assert_eq!(
            overview_json["observability"]["last_recall"]["bundle_id"],
            UI_BUNDLE_ID
        );
        assert_eq!(
            overview_json["observability"]["health"]["doctor"]["status"],
            "success"
        );
        assert_eq!(
            overview_json["observability"]["health"]["verify"]["status"],
            "warning"
        );

        let effectiveness = route(
            &Method::Get,
            &format!("{prefix}/effectiveness"),
            &token,
            &fixture.context,
        )
        .expect("effectiveness should serialize");
        assert_eq!(effectiveness.status.0, 200);
        let direct_report = crate::observability::report::effectiveness_report(
            fixture.context.workspace_paths(),
            fixture.context.workspace_key(),
        )
        .expect("direct effectiveness report should build");
        let effectiveness_json = json_spec(&effectiveness);
        let direct_json =
            serde_json::to_value(direct_report).expect("direct report should serialize");
        for field in [
            "product_version",
            "workspace",
            "collection_status",
            "complete",
            "observability_schema_version",
            "facts",
        ] {
            assert_eq!(effectiveness_json[field], direct_json[field]);
        }
        for field in [
            "days",
            "retention_max_bytes",
            "retention_floor_at",
            "retention_truncated",
        ] {
            assert_eq!(
                effectiveness_json["period"][field],
                direct_json["period"][field]
            );
        }
        let effectiveness_body = String::from_utf8_lossy(&effectiveness.body);
        for secret in [
            "ACTIVITY_SECRET",
            "BUNDLE_SECRET",
            "BUNDLE_SUMMARY_SECRET",
            "BUNDLE_SOURCE_SECRET",
        ] {
            assert!(!effectiveness_body.contains(secret));
        }

        fixture.seed_foreign_observability_rows();
        let local_doctor = route(
            &Method::Get,
            &format!("{prefix}/activity?event=doctor&limit=500"),
            &token,
            &fixture.context,
        )
        .expect("workspace-filtered activity should serialize");
        let local_doctor_body = String::from_utf8_lossy(&local_doctor.body);
        assert_eq!(local_doctor.status.0, 200);
        assert!(!local_doctor_body.contains(FOREIGN_EVENT_ID));
        assert!(!local_doctor_body.contains("foreign-workspace"));
        let foreign = route(
            &Method::Get,
            &format!("{prefix}/bundle?id={FOREIGN_BUNDLE_ID}"),
            &token,
            &fixture.context,
        )
        .expect("foreign bundle response should serialize");
        assert_eq!(foreign.status.0, 404);
        let contaminated_report = route(
            &Method::Get,
            &format!("{prefix}/effectiveness"),
            &token,
            &fixture.context,
        )
        .expect("contaminated report error should serialize");
        assert_eq!(contaminated_report.status.0, 500);
        assert_eq!(
            json_spec(&contaminated_report)["error"]["code"],
            "UI_DATA_UNAVAILABLE"
        );
    }

    #[test]
    fn tools_mcp_and_exact_route_allowlist_are_bounded_and_read_only() {
        let fixture = DataFixture::new();
        fixture.seed_observability();
        fixture.seed_tools_and_mcp();
        let token = fixture_token();
        let prefix = format!("/{}/api/v1", token.as_str());

        let tools = route(
            &Method::Get,
            &format!("{prefix}/tools?limit=1"),
            &token,
            &fixture.context,
        )
        .expect("tools first page should serialize");
        let tools_json = json_spec(&tools);
        assert_eq!(tools.status.0, 200);
        assert_eq!(tools_json["limit"], 1);
        assert_eq!(tools_json["more_results"], true);
        assert_eq!(tools_json["items"].as_array().map(Vec::len), Some(1));
        let tools_body = String::from_utf8_lossy(&tools.body);
        for forbidden in [
            "contract_json",
            "ALPHA_CONTRACT_SECRET",
            "OMEGA_CONTRACT_SECRET",
        ] {
            assert!(!tools_body.contains(forbidden));
        }
        let tools_cursor = tools_json["next_cursor"]
            .as_str()
            .expect("tools first page should have a cursor");
        let tools_second = route(
            &Method::Get,
            &format!("{prefix}/tools?limit=1&cursor={tools_cursor}"),
            &token,
            &fixture.context,
        )
        .expect("tools second page should serialize");
        assert_ne!(
            tools_json["items"][0]["tool_id"],
            json_spec(&tools_second)["items"][0]["tool_id"]
        );
        let tools_wrong_scope = route(
            &Method::Get,
            &format!("{prefix}/tools?limit=1&status=active&cursor={tools_cursor}"),
            &token,
            &fixture.context,
        )
        .expect("tools wrong-scope cursor should serialize");
        assert_eq!(tools_wrong_scope.status.0, 400);
        let external_write = route(
            &Method::Get,
            &format!("{prefix}/tools?side_effects=external_write"),
            &token,
            &fixture.context,
        )
        .expect("tools side-effect filter should serialize");
        assert_eq!(
            json_spec(&external_write)["items"][0]["tool_id"],
            "omega-tool"
        );
        let combined_tool_filters = route(
            &Method::Get,
            &format!("{prefix}/tools?status=draft&side_effects=external_write"),
            &token,
            &fixture.context,
        )
        .expect("combined tool filters should serialize");
        assert_eq!(
            json_spec(&combined_tool_filters)["items"][0]["tool_id"],
            "omega-tool"
        );
        let mismatched_tool_filters = route(
            &Method::Get,
            &format!("{prefix}/tools?status=active&side_effects=external_write"),
            &token,
            &fixture.context,
        )
        .expect("mismatched tool filters should serialize");
        assert_eq!(
            json_spec(&mismatched_tool_filters)["items"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );

        let mcp = route(
            &Method::Get,
            &format!("{prefix}/mcp?limit=1"),
            &token,
            &fixture.context,
        )
        .expect("MCP first page should serialize");
        let mcp_json = json_spec(&mcp);
        assert_eq!(mcp.status.0, 200);
        assert_eq!(mcp_json["more_results"], true);
        let mcp_body = String::from_utf8_lossy(&mcp.body);
        for forbidden in [
            "credentials_source",
            "notes",
            "ALPHA_CREDENTIAL_SECRET",
            "OMEGA_CREDENTIAL_SECRET",
            "ALPHA_NOTES_SECRET",
            "OMEGA_NOTES_SECRET",
        ] {
            assert!(!mcp_body.contains(forbidden));
        }
        let mcp_cursor = mcp_json["next_cursor"]
            .as_str()
            .expect("MCP first page should have a cursor");
        let mcp_second = route(
            &Method::Get,
            &format!("{prefix}/mcp?limit=1&cursor={mcp_cursor}"),
            &token,
            &fixture.context,
        )
        .expect("MCP second page should serialize");
        assert_ne!(
            mcp_json["items"][0]["id"],
            json_spec(&mcp_second)["items"][0]["id"]
        );
        let mcp_wrong_scope = route(
            &Method::Get,
            &format!("{prefix}/mcp?limit=1&kind=stdio&cursor={mcp_cursor}"),
            &token,
            &fixture.context,
        )
        .expect("MCP wrong-scope cursor should serialize");
        assert_eq!(mcp_wrong_scope.status.0, 400);
        let configured = route(
            &Method::Get,
            &format!("{prefix}/mcp?status=configured_unverified"),
            &token,
            &fixture.context,
        )
        .expect("MCP status filter should serialize");
        assert_eq!(json_spec(&configured)["items"][0]["id"], "omega-mcp");
        let combined_mcp_filters = route(
            &Method::Get,
            &format!("{prefix}/mcp?status=configured_unverified&kind=http"),
            &token,
            &fixture.context,
        )
        .expect("combined MCP filters should serialize");
        assert_eq!(
            json_spec(&combined_mcp_filters)["items"][0]["id"],
            "omega-mcp"
        );
        let mismatched_mcp_filters = route(
            &Method::Get,
            &format!("{prefix}/mcp?status=installed&kind=http"),
            &token,
            &fixture.context,
        )
        .expect("mismatched MCP filters should serialize");
        assert_eq!(
            json_spec(&mismatched_mcp_filters)["items"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );

        for endpoint in [
            "bootstrap".to_string(),
            "overview".to_string(),
            "memory".to_string(),
            format!("node?id={}", fixture.node_ids[0]),
            format!("node-links?id={}", fixture.node_ids[0]),
            "graph".to_string(),
            "activity".to_string(),
            format!("bundle?id={UI_BUNDLE_ID}"),
            "effectiveness".to_string(),
            "tools".to_string(),
            "mcp".to_string(),
        ] {
            let get = route(
                &Method::Get,
                &format!("{prefix}/{endpoint}"),
                &token,
                &fixture.context,
            )
            .expect("allowlisted GET should serialize");
            assert_eq!(get.status.0, 200, "GET endpoint failed: {endpoint}");
            let post = route(
                &Method::Post,
                &format!("{prefix}/{endpoint}"),
                &token,
                &fixture.context,
            )
            .expect("allowlisted POST rejection should serialize");
            assert_eq!(
                post.status.0, 405,
                "POST endpoint was not blocked: {endpoint}"
            );
        }
        for write_path in ["node/create", "memory/update", "tool/run", "mcp/install"] {
            let response = route(
                &Method::Post,
                &format!("{prefix}/{write_path}"),
                &token,
                &fixture.context,
            )
            .expect("unknown write route should serialize");
            assert_eq!(response.status.0, 404);
        }
    }

    #[test]
    fn graph_enforces_exact_node_and_edge_boundaries() {
        let fixture = DataFixture::new();
        fixture.seed_graph_boundaries();
        let token = fixture_token();
        let response = route(
            &Method::Get,
            &format!("/{}/api/v1/graph?limit=200", token.as_str()),
            &token,
            &fixture.context,
        )
        .expect("boundary graph should serialize");
        let json = json_spec(&response);
        assert_eq!(response.status.0, 200);
        assert_eq!(json["nodes"].as_array().map(Vec::len), Some(200));
        assert_eq!(json["edges"].as_array().map(Vec::len), Some(500));
        assert_eq!(json["nodes_more_results"], true);
        assert_eq!(json["nodes_complete"], false);
        assert!(json["nodes_next_cursor"].is_string());
        assert_eq!(json["edges_more_results"], true);
        assert_eq!(json["edges_complete"], false);
        assert_eq!(json["complete"], false);
        assert_eq!(json["center"], serde_json::Value::Null);
        assert_eq!(json["center_node"], serde_json::Value::Null);

        let centered = route(
            &Method::Get,
            &format!("/{}/api/v1/graph?limit=200&center=201", token.as_str()),
            &token,
            &fixture.context,
        )
        .expect("centered boundary graph should serialize");
        let centered_json = json_spec(&centered);
        assert_eq!(centered.status.0, 200);
        assert_eq!(centered_json["nodes"].as_array().map(Vec::len), Some(199));
        assert_eq!(centered_json["center_node"]["id"], 201);
        assert!(
            centered_json["nodes"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default()
                + usize::from(!centered_json["center_node"].is_null())
                <= data::MAX_GRAPH_NODES
        );
        assert_eq!(centered_json["nodes_more_results"], true);
    }

    #[test]
    fn every_ui_get_preserves_database_bytes_schema_mtime_and_counts() {
        const OPERATIONAL_TABLES: &[&str] = &[
            "nodes",
            "links",
            "aliases",
            "tags",
            "sources",
            "events",
            "tool_contracts",
            "mcp_profiles",
            "schema_migrations",
        ];
        const OBSERVABILITY_TABLES: &[&str] = &[
            "observability_events",
            "recall_bundles",
            "bundle_nodes",
            "feedback",
            "collector_state",
        ];
        let fixture = DataFixture::new();
        fixture.seed_observability();
        fixture.seed_tools_and_mcp();
        let operational_path = fixture.context.workspace_paths().db();
        let observability_path = fixture.context.workspace_paths().observability_db();
        let operational_before = database_fingerprint(operational_path, OPERATIONAL_TABLES);
        let observability_before = database_fingerprint(observability_path, OBSERVABILITY_TABLES);
        let token = fixture_token();
        let prefix = format!("/{}/api/v1", token.as_str());

        for endpoint in [
            "bootstrap".to_string(),
            "overview".to_string(),
            "memory?limit=1&q=Alpha".to_string(),
            format!("node?id={}", fixture.node_ids[0]),
            format!("node-links?id={}&limit=1", fixture.node_ids[0]),
            format!("graph?center={}&limit=1", fixture.node_ids[2]),
            "activity?limit=1".to_string(),
            format!("bundle?id={UI_BUNDLE_ID}&limit=1"),
            "effectiveness".to_string(),
            "tools?limit=1".to_string(),
            "mcp?limit=1".to_string(),
        ] {
            let response = route(
                &Method::Get,
                &format!("{prefix}/{endpoint}"),
                &token,
                &fixture.context,
            )
            .expect("read-only endpoint should serialize");
            assert_eq!(response.status.0, 200, "GET endpoint failed: {endpoint}");
        }

        let operational_after = database_fingerprint(operational_path, OPERATIONAL_TABLES);
        let observability_after = database_fingerprint(observability_path, OBSERVABILITY_TABLES);
        assert_eq!(operational_after, operational_before);
        assert_eq!(observability_after, observability_before);
    }
}
