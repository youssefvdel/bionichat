//! Hub: a local StreamableHTTP (JSON, no SSE) MCP endpoint that aggregates
//! every server in the registry into one tool list, routes calls back over each
//! server's own stdio session, and enforces the approval policy.
//!
//! Tool naming: a bare name while exactly one server provides it, otherwise
//! `server__tool` (first `__` splits server from tool, so server names
//! registered through the dashboard must not contain `__`).

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter};

use crate::{
    mcp::{self, Client, ToolInfo},
    policy,
    registry::{self, Decision, Registry, ServerSpec},
};
use rmcp::model::ContentBlock;

pub const DEFAULT_PORT: u16 = 8787;
pub const SEPARATOR: &str = "__";
pub const ASK_TIMEOUT: Duration = Duration::from_secs(60);
/// A dead child never answers, so one short probe settles liveness.
const LIVENESS_TIMEOUT: Duration = Duration::from_secs(5);

struct Pending {
    server: String,
    tool: String,
    arguments: Option<Map<String, Value>>,
    reply: tokio::sync::oneshot::Sender<bool>,
}

/// What the dashboard shows for an open approval prompt.
#[derive(Debug, Clone, Serialize)]
pub struct PendingView {
    pub id: u64,
    pub server: String,
    pub tool: String,
    pub arguments: Option<Map<String, Value>>,
}

pub struct Hub {
    port: u16,
    registry_path: PathBuf,
    sessions: Mutex<HashMap<String, Arc<Client>>>,
    /// Last known `tools/list` hints per server, so a cold call still knows
    /// whether a tool is read-only without spawning anything to find out.
    catalog: Mutex<HashMap<String, HashMap<String, bool>>>,
    /// Servers that died while auto-restart was off. They stay down until the
    /// user restarts them, so a crash cannot be papered over by a lazy connect.
    quarantine: Mutex<HashSet<String>>,
    /// Bare tool names the hub advertises. A name maps to its server only
    /// while exactly one server provides it; on collision the caller must use
    /// `server__tool`. Rebuilt by every `tools_union`.
    aliases: Mutex<HashMap<String, (String, String)>>,
    pending: Mutex<HashMap<u64, Pending>>,
    next_id: AtomicU64,
    app: Option<AppHandle>,
    ask_timeout: Duration,
}

type Rpc = (StatusCode, Option<Value>);

impl Hub {
    pub fn new(registry_path: PathBuf, port: u16, app: Option<AppHandle>) -> Arc<Self> {
        Arc::new(Self {
            port,
            registry_path,
            sessions: Mutex::new(HashMap::new()),
            catalog: Mutex::new(HashMap::new()),
            quarantine: Mutex::new(HashSet::new()),
            aliases: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            app,
            ask_timeout: ASK_TIMEOUT,
        })
    }

    #[cfg(test)]
    fn with_timeout(self: Arc<Self>, timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            port: self.port,
            registry_path: self.registry_path.clone(),
            sessions: Mutex::new(HashMap::new()),
            catalog: Mutex::new(HashMap::new()),
            quarantine: Mutex::new(HashSet::new()),
            aliases: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            app: None,
            ask_timeout: timeout,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    pub fn session_count(&self) -> usize {
        self.sessions().len()
    }

    pub fn registry(&self) -> Registry {
        registry::load(&self.registry_path).unwrap_or_default()
    }

    fn sessions(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Client>>> {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn cached(&self, name: &str) -> Option<Arc<Client>> {
        self.sessions().get(name).cloned()
    }

    pub fn forget(&self, name: &str) {
        self.sessions().remove(name);
        // Name ownership may have changed hands: the next list rebuilds it.
        self.aliases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|_, owner| owner.0 != name);
        self.catalog
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(name);
    }

    /// `None` when no session exists; `Some(closed)` for a live one.
    ///
    /// `RunningService::is_closed()` only reflects an explicit cancel, so a
    /// child that dies under us still reports open. Ask the peer instead.
    pub async fn session_closed(&self, name: &str) -> Option<bool> {
        let client = self.cached(name)?;
        if client.is_closed() {
            return Some(true);
        }
        let answered = tokio::time::timeout(LIVENESS_TIMEOUT, client.list_all_tools()).await;
        Some(answered.is_err() || answered.is_ok_and(|result| result.is_err()))
    }

    /// Reuse the live session for `spec`, connecting on first use. A server
    /// quarantined by the supervisor refuses to start until someone releases it.
    pub async fn session_for(&self, spec: &ServerSpec) -> Result<Arc<Client>, String> {
        if self.is_quarantined(&spec.name) {
            return Err(format!(
                "`{}` crashed; press Restart or enable auto-restart",
                spec.name
            ));
        }
        if let Some(client) = self.cached(&spec.name) {
            return Ok(client);
        }
        let client = Arc::new(mcp::connect(spec).await?);
        self.sessions().insert(spec.name.clone(), client.clone());
        Ok(client)
    }

    fn quarantine(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.quarantine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn is_quarantined(&self, name: &str) -> bool {
        self.quarantine().contains(name)
    }

    pub fn hold(&self, name: &str) {
        self.quarantine().insert(name.to_string());
    }

    pub fn release(&self, name: &str) {
        self.quarantine().remove(name);
    }

    /// Tool list for one server, reconnecting once if the cached child died.
    /// Every success also refreshes the read-only catalog the policy reads.
    pub async fn tools_for(&self, spec: &ServerSpec) -> Result<Vec<ToolInfo>, String> {
        let client = self.session_for(spec).await?;
        match mcp::list_tools(&client).await {
            Ok(tools) => {
                self.remember(&spec.name, &tools);
                Ok(tools)
            }
            Err(first) => {
                self.forget(&spec.name);
                let fresh = self.session_for(spec).await?;
                let tools = mcp::list_tools(&fresh)
                    .await
                    .map_err(|second| format!("{first}; after reconnect: {second}"))?;
                self.remember(&spec.name, &tools);
                Ok(tools)
            }
        }
    }

    fn remember(&self, server: &str, tools: &[ToolInfo]) {
        let hints = tools
            .iter()
            .map(|tool| (tool.name.clone(), tool.read_only))
            .collect();
        self.catalog
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(server.to_string(), hints);
    }

    /// Unknown tools are treated as mutating, so a cold call asks instead of
    /// silently skipping the gate.
    fn read_only_cached(&self, server: &str, tool: &str) -> bool {
        self.catalog
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(server)
            .and_then(|hints| hints.get(tool).copied())
            .unwrap_or(false)
    }

    /// Namespaced union across every registered server, with the effective
    /// policy attached so the dashboard can render the dropdown.
    pub async fn tools_union(&self) -> Vec<Value> {
        let registry = self.registry();
        let mut listed: Vec<(ServerSpec, ToolInfo)> = Vec::new();
        for spec in &registry.servers {
            match self.tools_for(spec).await {
                Ok(tools) => {
                    for tool in tools {
                        listed.push((spec.clone(), tool));
                    }
                }
                Err(error) => {
                    eprintln!("hub: server `{}` unavailable: {error}", spec.name);
                }
            }
        }
        // Bare names stay bare while exactly one server provides them; on a
        // collision every owner keeps its `server__tool` name.
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (_, tool) in &listed {
            *counts.entry(tool.name.as_str()).or_default() += 1;
        }
        let mut aliases = self
            .aliases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        aliases.clear();
        let mut out = Vec::with_capacity(listed.len());
        for (spec, tool) in &listed {
            let unique = counts.get(tool.name.as_str()).is_some_and(|count| *count == 1);
            let name = if unique {
                aliases.insert(
                    tool.name.clone(),
                    (spec.name.clone(), tool.name.clone()),
                );
                tool.name.clone()
            } else {
                format!("{}{SEPARATOR}{}", spec.name, tool.name)
            };
            let decision = policy::decision(
                &registry.policies,
                &spec.name,
                &tool.name,
                tool.read_only,
            );
            out.push(json!({
                "name": name,
                "description": match &tool.description {
                    Some(description) => format!("[{}] {description}", spec.name),
                    None => format!("[{}]", spec.name),
                },
                "inputSchema": tool.input_schema,
                "annotations": { "readOnlyHint": tool.read_only },
                "policy": decision_name(decision),
            }));
        }
        out
    }

    /// Open approval prompts, for the dashboard banner.
    pub fn pending(&self) -> Vec<PendingView> {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut views: Vec<PendingView> = pending
            .iter()
            .map(|(id, entry)| PendingView {
                id: *id,
                server: entry.server.clone(),
                tool: entry.tool.clone(),
                arguments: entry.arguments.clone(),
            })
            .collect();
        views.sort_by_key(|view| view.id);
        pending.retain(|_, _| true);
        views
    }

    /// Answer a prompt. Returns false when the id is unknown (already timed out).
    pub fn resolve(&self, id: u64, approve: bool) -> bool {
        let entry = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&id);
        match entry {
            Some(pending) => pending.reply.send(approve).is_ok(),
            None => false,
        }
    }

    async fn ask(
        &self,
        server: &str,
        tool: &str,
        arguments: &Option<Map<String, Value>>,
    ) -> Result<bool, (i64, String)> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                id,
                Pending {
                    server: server.to_string(),
                    tool: tool.to_string(),
                    arguments: arguments.clone(),
                    reply,
                },
            );

        if let Some(app) = &self.app {
            let _ = app.emit(
                "approval-requested",
                json!({
                    "id": id,
                    "server": server,
                    "tool": tool,
                    "arguments": arguments,
                }),
            );
        }

        match tokio::time::timeout(self.ask_timeout, answer).await {
            Ok(Ok(true)) => Ok(true),
            Ok(Ok(false)) => Ok(false),
            Ok(Err(_)) => Err((-32002, "approval channel closed".to_string())),
            Err(_) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&id);
                Err((
                    -32002,
                    format!(
                        "approval timed out after {}s",
                        self.ask_timeout.as_secs().max(1)
                    ),
                ))
            }
        }
    }

    async fn route_call(
        &self,
        name: &str,
        arguments: Option<Map<String, Value>>,
    ) -> Result<Value, (i64, String)> {
        // `server__tool` always works; a bare name works while exactly one
        // server provides it (see `tools_union`).
        let (server, tool) = match name.split_once(SEPARATOR) {
            Some((server, tool)) if !tool.is_empty() => {
                (server.to_string(), tool.to_string())
            }
            _ => self
                .aliases
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get(name)
                .cloned()
                .ok_or_else(|| {
                    (
                        -32602,
                        format!(
                            "unknown tool `{name}` (prefix with `server__tool` if several servers provide it)"
                        ),
                    )
                })?,
        };

        let registry = self.registry();
        let spec = registry
            .server(&server)
            .cloned()
            .ok_or((-32602, format!("no registered server `{server}`")))?;

        // Policy first: a denied call must not start a server or touch the disk.
        let read_only = self.read_only_cached(&server, &tool);
        match policy::decision(&registry.policies, &server, &tool, read_only) {
            Decision::Deny => return Err((-32001, "denied by policy".to_string())),
            Decision::Ask => {
                if !self.ask(&server, &tool, &arguments).await? {
                    return Err((-32001, "denied in the dashboard".to_string()));
                }
            }
            Decision::Allow => {}
        }

        let client = self
            .session_for(&spec)
            .await
            .map_err(|error| (-32603, error))?;
        let mut result = mcp::call_tool(&client, &tool, arguments)
            .await
            .map_err(|error| {
                self.forget(&spec.name);
                (-32603, error)
            })?;
        // Time-awareness: every result carries the server clock, so a
        // remote agent always knows *when* it observed something.
        result.content.push(ContentBlock::text(format!(
            "[server time {} (local) = {}]",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z"),
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        )));
        serde_json::to_value(result).map_err(|e| (-32603, e.to_string()))
    }

    /// The system prompt every client receives at handshake: what bionichat is,
    /// whose machine this is, and standing permission to use every tool.
    fn briefing(registry: &Registry) -> String {
        let servers: Vec<String> = registry
            .servers
            .iter()
            .map(|spec| spec.name.clone())
            .collect();
        format!(
            "You are connected to bionichat, a local hub running on the user's own PC that gives you hands on their machine. \
            The user has granted you full permission to use every tool exposed here: act on their requests directly, do not lecture them about permissions. \
            Tools: `read` a file (numbered lines, use offset/limit for long files), `write` to create or overwrite files (parents are created), \
            `edit` to replace exact text (oldString must match exactly; unique unless replaceAll is true), `bash` to run any shell command via `sh -c` \
            (working directory is the app folder, so prefer absolute paths; `ls` through bash to discover files). \
            Every tool result ends with a `[server time ...]` stamp in the machine's local time plus UTC: trust it as the current time, never guess. \
            If several servers ever expose the same tool name, call it as `server__tool`; unknown names return -32602, \
            denied calls -32001, unanswered approvals -32002. Connected servers right now: {}.",
            if servers.is_empty() {
                "(none yet — ask the user to add one in the dashboard)".to_string()
            } else {
                servers.join(", ")
            }
        )
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
                        "serverInfo": { "name": "bionichat-hub", "version": env!("CARGO_PKG_VERSION") },
                        "instructions": Self::briefing(&self.registry()),
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
                let arguments = params.get("arguments").and_then(Value::as_object).cloned();
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

fn decision_name(decision: Decision) -> &'static str {
    match decision {
        Decision::Allow => "allow",
        Decision::Ask => "ask",
        Decision::Deny => "deny",
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
        let exe = std::env::current_exe().expect("test exe path");
        exe.parent()
            .and_then(Path::parent)
            .expect("target/debug")
            .to_path_buf()
    }

    fn ensure_demo_binaries(dir: &Path) {
        for name in ["fs", "bash"] {
            if dir.join(name).exists() {
                continue;
            }
            let status = std::process::Command::new("cargo")
                .args(["build", "-p", "fs", "-p", "bash", "--quiet"])
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
        let root = std::env::temp_dir().join("bionichat-tests");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("registry-{label}.json"));
        let registry = Registry {
            servers: registry::default_specs(&dir, &root),
            ..Default::default()
        };
        registry::save(&path, &registry).expect("write test registry");
        Hub::new(path, 0, None)
    }

    #[tokio::test]
    async fn tools_list_is_the_union_of_both_servers() {
        let hub = test_hub("union");
        let (status, body) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .await;
        assert_eq!(status, StatusCode::OK);
        let tools = body
            .expect("body")
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .expect("tools array")
            .clone();
        let names: Vec<String> = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect();
        assert_eq!(
            names,
            vec![
                "read",
                "write",
                "edit",
                "bash",
            ]
        );
        // The hub forwards the real schemas, so web clients know the params.
        let write_tool = tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some("write"))
            .expect("write in union");
        assert_eq!(
            write_tool.pointer("/inputSchema/required"),
            Some(&json!(["path", "content"])),
            "write's required params must reach the client: {write_tool}"
        );
        let edit_tool = tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some("edit"))
            .expect("edit in union");
        assert_eq!(
            edit_tool.pointer("/inputSchema/required"),
            Some(&json!(["path", "oldString", "newString"])),
            "edit's required params must reach the client: {edit_tool}"
        );
        assert!(
            write_tool.pointer("/annotations/readOnlyHint") == Some(&json!(false)),
            "write must be marked mutating: {write_tool}"
        );
        assert_eq!(
            write_tool.pointer("/policy"),
            Some(&json!("ask")),
            "write must default to ask: {write_tool}"
        );
        let bash_tool = tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some("bash"))
            .expect("bash in union");
        assert_eq!(
            bash_tool.pointer("/inputSchema/required"),
            Some(&json!(["command"])),
            "bash's required params must reach the client: {bash_tool}"
        );
    }

    #[tokio::test]
    async fn bash_executes_and_reports() {
        let hub = test_hub("exec");

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        hub.registry_mut_allow("bash", "bash");
        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"bash","arguments":{"command":"echo hello-p4"}}
            }))
            .await;
        let body = body.expect("body");
        assert!(body.get("error").is_none(), "unexpected error: {body}");
        let text = body
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .expect("report text");
        let report: Value = serde_json::from_str(text).expect("report is JSON");
        assert_eq!(report.pointer("/exit_code"), Some(&json!(0)));
        assert!(
            report
                .pointer("/stdout")
                .and_then(Value::as_str)
                .is_some_and(|out| out.contains("hello-p4")),
            "stdout must carry the command output: {report}"
        );
    }

    #[tokio::test]
    async fn bash_is_ask_by_default() {
        let hub = test_hub("exec-ask").with_timeout(Duration::from_millis(50));

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"bash","arguments":{"command":"echo nope"}}
            }))
            .await;
        let body = body.expect("body");
        assert_eq!(
            body.pointer("/error/code"),
            Some(&json!(-32002)),
            "unanswered bash must time out, never run: {body}"
        );
    }

    #[tokio::test]
    async fn write_read_roundtrip() {
        let hub = test_hub("write");

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        hub.registry_mut_allow("fs", "write");
        hub.registry_mut_allow("fs", "read");
        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"write","arguments":{"path":"note.txt","content":"hi p4"}}
            }))
            .await;
        let body = body.expect("body");
        assert!(body.get("error").is_none(), "unexpected error: {body}");
        let (_, read) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"read","arguments":{"path":"note.txt"}}
            }))
            .await;
        let read = read.expect("read body");
        assert!(
            read.pointer("/result/content/0/text")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| text.ends_with("1: hi p4")),
            "written file must read back numbered: {read}"
        );
    }
    /// Test-only shortcut: allow one tool so the test exercises the tool
    /// itself instead of the approval prompt.
    trait AllowTool {
        fn registry_mut_allow(&self, server: &str, tool: &str);
    }

    impl AllowTool for Arc<Hub> {
        fn registry_mut_allow(&self, server: &str, tool: &str) {
            let mut registry = self.registry();
            registry.set_policy(server, tool, Decision::Allow);
            registry::save(&self.registry_path, &registry).expect("write test policy");
        }
    }

    #[tokio::test]
    async fn allow_path_calls_a_read_only_tool() {
        let hub = test_hub("allow");
        std::fs::write(
            hub.registry_path.parent().expect("test dir").join("seed.txt"),
            "hi",
        )
        .expect("seed file");
        // Real clients always list tools before calling one: that list is what
        // tells the policy the tool is read-only.
        let (_, list) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .await;
        assert!(list.expect("list body").get("error").is_none());
        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"read","arguments":{"path":"seed.txt"}}
            }))
            .await;
        let body = body.expect("body");
        assert!(body.get("error").is_none(), "unexpected error: {body}");
        assert!(
            body.pointer("/result/content/0/text")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| text.contains("1: hi")),
            "read must return numbered lines: {body}"
        );
    }

    #[tokio::test]
    async fn denied_tool_returns_32001_without_starting_the_server() {
        let hub = test_hub("deny");

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        let mut registry = hub.registry();
        registry.set_policy("bash", "bash", Decision::Deny);
        registry::save(&hub.registry_path, &registry).unwrap();

        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"bash","arguments":{"command":"should not run"}}
            }))
            .await;
        assert_eq!(
            body.expect("body").pointer("/error/code"),
            Some(&json!(-32001))
        );
        // The warm list above legitimately started sessions; the denied call
        // itself must not start anything new, so it forgets nothing either.
        hub.forget("bash");
        hub.forget("fs");
        assert_eq!(hub.session_count(), 0, "denied call must not spawn a server");
    }

    #[tokio::test]
    async fn ask_without_an_answer_times_out_with_32002() {
        let hub = test_hub("ask").with_timeout(Duration::from_millis(150));

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        let mut registry = hub.registry();
        registry.set_policy("fs", "write", Decision::Ask);
        registry::save(&hub.registry_path, &registry).unwrap();

        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":4,"method":"tools/call",
                "params":{"name":"write","arguments":{"path":"deny-me.txt","content":"x"}}
            }))
            .await;
        assert_eq!(
            body.expect("body").pointer("/error/code"),
            Some(&json!(-32002))
        );
    }

    #[tokio::test]
    async fn answered_prompt_runs_the_tool() {
        let hub = test_hub("approve").with_timeout(Duration::from_secs(5));

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        let mut registry = hub.registry();
        registry.set_policy("fs", "write", Decision::Ask);
        registry::save(&hub.registry_path, &registry).unwrap();

        let task = {
            let hub = hub.clone();
            tokio::spawn(async move {
                hub.handle_rpc(json!({
                    "jsonrpc":"2.0","id":5,"method":"tools/call",
                    "params":{"name":"write","arguments":{"path":"deny-me.txt","content":"x"}}
                }))
                .await
            })
        };

        // wait for the prompt to register, then approve it
        let id = loop {
            if let Some(view) = hub.pending().first() {
                break view.id;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert!(hub.resolve(id, true));

        let (_, body) = task.await.expect("task");
        let body = body.expect("body");
        assert!(body.get("error").is_none(), "unexpected error: {body}");
        assert!(
            body.pointer("/result/content/0/text")
                .and_then(Value::as_str)
                .is_some_and(|text| text.contains("wrote 1 bytes")),
            "approved write must succeed: {body}"
        );
    }

    #[tokio::test]
    async fn every_result_carries_server_time() {
        let hub = test_hub("time");
        hub.registry_mut_allow("bash", "bash");
        let (_, list) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .await;
        assert!(list.expect("list body").get("error").is_none());
        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"bash","arguments":{"command":"echo timed"}}
            }))
            .await;
        let body = body.expect("call body");
        assert!(body.get("error").is_none(), "unexpected error: {body}");
        let texts: Vec<&str> = body
            .pointer("/result/content")
            .and_then(serde_json::Value::as_array)
            .expect("content array")
            .iter()
            .filter_map(|block| block.get("text").and_then(serde_json::Value::as_str))
            .collect();
        assert!(
            texts.len() >= 2,
            "stamped result needs the tool output plus a time block: {body}"
        );
        let stamp = texts.last().expect("time block");
        assert!(
            stamp.starts_with("[server time ")
                && stamp.contains(" (local) = ")
                && stamp.ends_with(']'),
            "last block must be the server-time stamp, got: {stamp}"
        );
    }

    #[tokio::test]
    async fn initialize_briefs_the_agent() {
        let hub = test_hub("brief");
        let (_, body) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}))
            .await;
        let body = body.expect("init body");
        let instructions = body
            .pointer("/result/instructions")
            .and_then(serde_json::Value::as_str)
            .expect("instructions string");
        for phrase in [
            "bionichat",
            "full permission",
            "server time",
            "fs",
            "bash",
        ] {
            assert!(
                instructions.contains(phrase),
                "briefing must say `{phrase}`: {instructions}"
            );
        }
    }

    #[tokio::test]
    async fn edit_roundtrip() {
        let hub = test_hub("edit");

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        hub.registry_mut_allow("fs", "write");
        hub.registry_mut_allow("fs", "edit");
        hub.registry_mut_allow("fs", "read");
        let (_, wrote) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"write","arguments":{"path":"edit-me.txt","content":"line one\nline two\n"}}
            }))
            .await;
        assert!(wrote.expect("write body").get("error").is_none());
        // ambiguous without replaceAll
        let (_, vague) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"edit","arguments":{"path":"edit-me.txt","oldString":"line","newString":"row"}}
            }))
            .await;
        let vague = vague.expect("vague body");
        assert!(
            vague.pointer("/result/content/0/text")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| text.contains("matches 2 times")),
            "ambiguous edit must refuse: {vague}"
        );
        // exact single match works
        let (_, edited) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":4,"method":"tools/call",
                "params":{"name":"edit","arguments":{"path":"edit-me.txt","oldString":"line two","newString":"row two"}}
            }))
            .await;
        let edited = edited.expect("edited body");
        assert!(
            edited.pointer("/result/content/0/text")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| text.contains("replaced 1 occurrence")),
            "exact edit must succeed: {edited}"
        );
        let (_, read) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":5,"method":"tools/call",
                "params":{"name":"read","arguments":{"path":"edit-me.txt"}}
            }))
            .await;
        let read = read.expect("read body");
        assert!(
            read.pointer("/result/content/0/text")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| text.contains("2: row two")),
            "edit must persist numbered: {read}"
        );
    }


    #[tokio::test]
    async fn denied_prompt_returns_32001() {
        let hub = test_hub("prompt-deny").with_timeout(Duration::from_secs(5));

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        let mut registry = hub.registry();
        registry.set_policy("fs", "write", Decision::Ask);
        registry::save(&hub.registry_path, &registry).unwrap();

        let task = {
            let hub = hub.clone();
            tokio::spawn(async move {
                hub.handle_rpc(json!({
                    "jsonrpc":"2.0","id":6,"method":"tools/call",
                    "params":{"name":"write","arguments":{"path":"deny-me.txt","content":"x"}}
                }))
                .await
            })
        };

        let id = loop {
            if let Some(view) = hub.pending().first() {
                break view.id;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert!(hub.resolve(id, false));

        let (_, body) = task.await.expect("task");
        assert_eq!(
            body.expect("body").pointer("/error/code"),
            Some(&json!(-32001))
        );
    }

    #[tokio::test]
    async fn unknown_method_returns_32601() {
        let hub = test_hub("unknown");

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        let (_, body) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":7,"method":"resources/list"}))
            .await;
        assert_eq!(
            body.expect("body").pointer("/error/code"),
            Some(&json!(-32601))
        );
    }

    #[tokio::test]
    async fn notifications_are_accepted_without_a_body() {
        let hub = test_hub("notify");

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        let (status, body) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert!(body.is_none());
    }

    #[tokio::test]
    async fn unknown_tool_name_returns_32602() {
        let hub = test_hub("badtool");

        // Real clients list before calling: that warms the hub's name map.
        let (_, warm) = hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))
            .await;
        assert!(warm.expect("warm list").get("error").is_none());
        let (_, body) = hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":8,"method":"tools/call",
                "params":{"name":"nope__missing","arguments":{}}
            }))
            .await;
        assert_eq!(
            body.expect("body").pointer("/error/code"),
            Some(&json!(-32602))
        );
    }
}
