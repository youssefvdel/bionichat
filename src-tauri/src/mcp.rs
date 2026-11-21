//! Thin wrapper over the `rmcp` client: spawn a stdio MCP server, complete the
//! handshake, and keep the session alive for the hub to route calls through.

use std::time::Duration;

use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, CallToolResult, JsonObject},
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess, which_command},
};
use serde::Serialize;
use serde_json::Value;

use crate::registry::{ServerSpec, Transport};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

pub type Client = RunningService<RoleClient, ()>;

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub read_only: bool,
    pub input_schema: Value,
}

pub fn tool_info(tool: &rmcp::model::Tool) -> ToolInfo {
    ToolInfo {
        name: tool.name.to_string(),
        description: tool.description.as_ref().map(|d| d.to_string()),
        read_only: tool
            .annotations
            .as_ref()
            .and_then(|a| a.read_only_hint)
            .unwrap_or(false),
        input_schema: Value::Object(tool.input_schema.as_ref().clone()),
    }
}

/// Spawn the server and complete the MCP handshake.
pub async fn connect(spec: &ServerSpec) -> Result<Client, String> {
    let Transport::Stdio { command, args, env } = &spec.transport else {
        return Err("http transport is not wired yet".to_string());
    };

    let args = args.clone();
    let env = env.clone();
    let command_name = command.clone();

    let child = which_command(command).map_err(|e| format!("cannot find `{command}`: {e}"))?;
    let child = child.configure(move |cmd| {
        cmd.args(&args);
        for (key, value) in &env {
            cmd.env(key, value);
        }
    });
    let transport = TokioChildProcess::new(child)
        .map_err(|e| format!("cannot spawn `{command_name}`: {e}"))?;

    tokio::time::timeout(CONNECT_TIMEOUT, ().serve(transport))
        .await
        .map_err(|_| format!("no handshake within {}s", CONNECT_TIMEOUT.as_secs()))?
        .map_err(|e| format!("handshake failed: {e}"))
}

pub async fn list_tools(client: &Client) -> Result<Vec<ToolInfo>, String> {
    let tools = client
        .list_all_tools()
        .await
        .map_err(|e| format!("tools/list failed: {e}"))?;
    Ok(tools.iter().map(tool_info).collect())
}

pub async fn call_tool(
    client: &Client,
    name: &str,
    arguments: Option<JsonObject>,
) -> Result<CallToolResult, String> {
    let mut params = CallToolRequestParams::new(name.to_string());
    params.arguments = arguments;
    client
        .call_tool(params)
        .await
        .map_err(|e| format!("tools/call `{name}` failed: {e}"))
}
