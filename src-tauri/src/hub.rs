//! Hub: a local StreamableHTTP (JSON, no SSE) MCP endpoint that aggregates
//! every server in the registry into one tool list and routes calls back over
//! each server's own stdio session.
//!
//! Tool naming: `server__tool` (first `__` splits server from tool, so server
//! names registered through the dashboard must not contain `__`).

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::{Map, Value, json};

use crate::{
    mcp::{self, Client, ToolInfo},
    registry::{self, ServerSpec},
};

pub const DEFAULT_PORT: u16 = 8787;
pub const SEPARATOR: &str = "__";

pub struct Hub {
    port: u16,
    registry_path: PathBuf,
    sessions: Mutex<HashMap<String, Arc<Client>>>,
}

type Rpc = (StatusCode, Option<Value>);

impl Hub {
    pub fn new(registry_path: PathBuf, port: u16) -> Arc<Self> {
        Arc::new(Self {
            port,
            registry_path,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    pub fn session_count(&self) -> usize {
        self.guard().len()
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Client>>> {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn specs(&self) -> Vec<ServerSpec> {
        registry::load(&self.registry_path).unwrap_or_default()
    }

    fn cached(&self, name: &str) -> Option<Arc<Client>> {
        self.guard().get(name).cloned()
    }

    fn forget(&self, name: &str) {
        self.guard().remove(name);
    }

    /// Reuse the live session for `spec`, connecting on first use.
    pub async fn session_for(&self, spec: &ServerSpec) -> Result<Arc<Client>, String> {
        if let Some(client) = self.cached(&spec.name) {
            return Ok(client);
        }
        let client = Arc::new(mcp::connect(spec).await?);
        self.guard().insert(spec.name.clone(), client.clone());
        Ok(client)
    }

    /// Tool list for one server, reconnecting once if the cached child died.
    pub async fn tools_for(&self, spec: &ServerSpec) -> Result<Vec<ToolInfo>, String> {
        let client = self.session_for(spec).await?;
        match mcp::list_tools(&client).await {
            Ok(tools) => Ok(tools),
            Err(first) => {
                self.forget(&spec.name);
                let fresh = self.session_for(spec).await?;
                mcp::list_tools(&fresh)
                    .await
                    .map_err(|second| format!("{first}; after reconnect: {second}"))
            }
        }
    }

    /// Namespaced union across every registered server.
    pub async fn tools_union(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for spec in self.specs() {
            match self.tools_for(&spec).await {
                Ok(tools) => out.extend(tools.into_iter().map(|tool| {
                    json!({
                        "name": format!("{}{SEPARATOR}{}", spec.name, tool.name),
                        "description": match tool.description {
                            Some(description) => format!("[{}] {description}", spec.name),
                            None => format!("[{}]", spec.name),
                        },
                        "inputSchema": { "type": "object" },
                        "annotations": { "readOnlyHint": tool.read_only },
                    })
                })),
                Err(error) => {
                    eprintln!("hub: server `{}` unavailable: {error}", spec.name);
                }
            }
        }
        out
    }

    async fn route_call(
        &self,
        prefixed: &str,
        arguments: Option<Map<String, Value>>,
    ) -> Result<Value, (i64, String)> {
        let (server, tool) = prefixed
            .split_once(SEPARATOR)
            .ok_or((-32602, format!("tool name `{prefixed}` is not namespaced")))?;

        let spec = self
            .specs()
            .into_iter()
            .find(|candidate| candidate.name == server)
            .ok_or((-32602, format!("no registered server `{server}`")))?;

        if tool.is_empty() {
            return Err((-32602, format!("empty tool name in `{prefixed}`")));
        }

        let client = self
            .session_for(&spec)
            .await
            .map_err(|error| (-32603, error))?;
        let result = mcp::call_tool(&client, tool, arguments)
            .await
            .map_err(|error| {
                self.forget(&spec.name);
                (-32603, error)
            })?;
        serde_json::to_value(result).map_err(|e| (-32603, e.to_string()))
    }

    /// One JSON-RPC request in, one HTTP status (+ body unless notification).
    pub async fn handle_rpc(&self, payload: Value) -> Rpc {
        let id = payload.get("id").cloned();
        let method = payload
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let params = payload.get("params").cloned().unwrap_or(Value::Null);

        if id.is_none() {
            return (StatusCode::ACCEPTED, None);
        }
        let id = id.unwrap_or(Value::Null);

        match method.as_str() {
            "initialize" => {
                let protocol = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("2025-06-18")
                    .to_string();
                ok(
                    id,
                    json!({
                        "protocolVersion": protocol,
                        "capabilities": { "tools": { "listChanged": false } },
                        "serverInfo": { "name": "web-agent-hub", "version": env!("CARGO_PKG_VERSION") },
                        "instructions": "Aggregated MCP hub. Tool names are `server__tool`; unknown names return -32602."
                    }),
                )
            }
            "ping" => ok(id, json!({})),
            "tools/list" => {
                let tools = self.tools_union().await;
                ok(id, json!({ "tools": tools }))
            }
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return rpc_error(id, -32602, "tools/call needs a string `name`".into());
                };
                let arguments = params
                    .get("arguments")
                    .and_then(Value::as_object)
                    .cloned();
                match self.route_call(name, arguments).await {
                    Ok(result) => ok(id, result),
                    Err((code, message)) => rpc_error(id, code, message),
                }
            }
            other => rpc_error(id, -32601, format!("Method not found: {other}")),
        }
    }

    /// Bind 127.0.0.1:<port> and serve until the process exits.
    pub async fn serve(self: Arc<Self>) -> Result<(), String> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", self.port))
            .await
            .map_err(|e| format!("cannot bind 127.0.0.1:{}: {e}", self.port))?;
        let app = Router::new()
            .route("/mcp", post(post_mcp))
            .with_state(self);
        axum::serve(listener, app)
            .await
            .map_err(|e| format!("hub stopped: {e}"))
    }
}

fn ok(id: Value, result: Value) -> Rpc {
    (
        StatusCode::OK,
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result })),
    )
}

fn rpc_error(id: Value, code: i64, message: String) -> Rpc {
    (
        StatusCode::OK,
        Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })),
    )
}

async fn post_mcp(State(hub): State<Arc<Hub>>, body: axum::body::Bytes) -> Response {
    let parsed = serde_json::from_slice::<Value>(&body);
    let Ok(payload) = parsed else {
        let body = json!({
            "jsonrpc": "2.0",
            "id": Value::Null,
            "error": { "code": -32700, "message": "Parse error" }
        });
        return (StatusCode::BAD_REQUEST, Json(body)).into_response();
    };

    let (status, body) = hub.handle_rpc(payload).await;
    match body {
        Some(body) => (status, Json(body)).into_response(),
        None => status.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn demo_dir() -> PathBuf {
        // test binary lives in target/debug/deps; the demo servers sit in target/debug
        let exe = std::env::current_exe().expect("test exe path");
        exe.parent()
            .and_then(Path::parent)
            .expect("target/debug")
            .to_path_buf()
    }

    fn ensure_demo_binaries(dir: &Path) {
        for name in ["demo-ping", "demo-fs"] {
            if dir.join(name).exists() {
                continue;
            }
            let status = std::process::Command::new("cargo")
                .args(["build", "-p", "demo-ping", "-p", "demo-fs", "--quiet"])
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .status()
                .expect("cargo build for demo servers");
            assert!(status.success(), "demo server build failed");
            break;
        }
    }

    fn test_hub(label: &str) -> Arc<Hub> {
        let dir = demo_dir();
        ensure_demo_binaries(&dir);
        let root = std::env::temp_dir().join("web-agent-tests");
        std::fs::create_dir_all(&root).unwrap();
        let registry = root.join(format!("registry-{label}.json"));
        let specs = registry::default_specs(&dir, &root);
        registry::save(&registry, &specs).expect("write test registry");
        Hub::new(registry, 0)
    }

    #[tokio::test]
    async fn tools_list_is_the_union_of_both_servers() {
        let hub = test_hub("union");
        let (status, body) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .await;
        assert_eq!(status, StatusCode::OK);
        let names: Vec<String> = body
            .expect("body")
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .expect("tools array")
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect();
        assert_eq!(names, vec!["demo-ping__ping", "demo-ping__echo", "demo-fs__fs.read"]);
    }

    #[tokio::test]
    async fn tools_call_ping_returns_pong_through_the_hub() {
        let hub = test_hub("call");
        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"demo-ping__ping","arguments":{}}
            }))
            .await;
        let body = body.expect("body");
        assert!(body.get("error").is_none(), "unexpected error: {body}");
        assert_eq!(
            body.pointer("/result/content/0/text").and_then(Value::as_str),
            Some("pong")
        );
    }

    #[tokio::test]
    async fn unknown_method_returns_32601() {
        let hub = test_hub("unknown");
        let (_, body) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":3,"method":"resources/list"}))
            .await;
        assert_eq!(body.expect("body").pointer("/error/code"), Some(&json!(-32601)));
    }

    #[tokio::test]
    async fn notifications_are_accepted_without_a_body() {
        let hub = test_hub("notify");
        let (status, body) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert!(body.is_none());
    }

    #[tokio::test]
    async fn unknown_tool_name_returns_32602() {
        let hub = test_hub("badtool");
        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":4,"method":"tools/call",
                "params":{"name":"nope__missing","arguments":{}}
            }))
            .await;
        assert_eq!(body.expect("body").pointer("/error/code"), Some(&json!(-32602)));
    }
}
