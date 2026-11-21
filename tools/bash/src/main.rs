//! bash: the shell server — one tool, `bash`.
//!
//! Runs a command via `sh -c` and returns exit code, stdout and stderr.
//! It is never read-only, so the hub's default policy is Ask: every call waits
//! for dashboard approval. A hard cap (default 30 s, max 120 s) kills runaways.

use std::{sync::Arc, time::Duration};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    },
    service::RequestContext,
    transport::stdio,
};
use serde_json::{Map, Value, json};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_TIMEOUT_MS: u64 = 120_000;
const OUTPUT_LIMIT: usize = 8 * 1024;

#[derive(Clone)]
struct BashServer;

fn schema(properties: Value, required: &[&str]) -> Arc<Map<String, Value>> {
    let mut obj = json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false,
    });
    if !required.is_empty() {
        obj["required"] = json!(required);
    }
    Arc::new(obj.as_object().expect("object schema").clone())
}

fn truncate(text: &str) -> String {
    if text.len() <= OUTPUT_LIMIT {
        return text.to_string();
    }
    format!("{}…[truncated, {} bytes total]", &text[..OUTPUT_LIMIT], text.len())
}

impl ServerHandler for BashServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("bash", env!("CARGO_PKG_VERSION")))
            .with_instructions("Runs shell commands. Every call needs approval.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![Tool::new(
            "bash",
            "Runs a shell command via `sh -c` and returns exit code, stdout and stderr. Mutating: approval required.",
            schema(
                json!({
                    "command": {
                        "type": "string",
                        "description": "Shell command to run"
                    },
                    "workdir": {
                        "type": "string",
                        "description": "Working directory (default: the server's own)"
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Kill the command after this long (default 30000, max 120000)"
                    }
                }),
                &["command"],
            ),
        )]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name.as_ref() != "bash" {
            return Err(McpError::invalid_params(
                format!("unknown tool `{}`", request.name),
                None,
            ));
        }
        let arguments = request.arguments.unwrap_or_default();
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| McpError::invalid_params("bash requires a string `command`", None))?;
        if command.trim().is_empty() {
            return Err(McpError::invalid_params("bash: empty command", None));
        }
        let timeout_ms = arguments
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .clamp(1_000, MAX_TIMEOUT_MS);
        let mut child = tokio::process::Command::new("sh");
        child.arg("-c").arg(command);
        if let Some(workdir) = arguments.get("workdir").and_then(Value::as_str) {
            child.current_dir(workdir);
        }
        let child = child
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| McpError::internal_error(format!("cannot spawn shell: {e}"), None))?;

        let output = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| McpError::internal_error("bash timed out — process killed", None))?
        .map_err(|e| McpError::internal_error(format!("bash failed: {e}"), None))?;
        let report = json!({
            "exit_code": output.status.code(),
            "stdout": truncate(&String::from_utf8_lossy(&output.stdout)),
            "stderr": truncate(&String::from_utf8_lossy(&output.stderr)),
        });
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&report).expect("report serializes"),
        )])
        .into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = BashServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
