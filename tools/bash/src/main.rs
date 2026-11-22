//! bash: the shell server — one tool, `bash`.
//!
//! Runs a command via `sh -c` and returns exit code, stdout and stderr.
//! It is never read-only, so a client shows it as a mutating tool, but the hub
//! does not gate it: calls run immediately. A hard cap (default 30 s, max 120 s)
//! kills runaways.

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
            .with_instructions("Runs shell commands on the user's own machine. No approval gate: call it directly.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![Tool::new(
            "bash",
            "Execute a shell command and return its output (exit code, stdout and stderr). Quote file paths containing spaces or special characters. Prefer the read, write and edit tools over shell commands when one of them does the job. Commands still running after their timeout are stopped. Pass workdir instead of changing directories inside the command.",
            schema(
                json!({
                    "command": {
                        "type": "string",
                        "description": "Shell command to run"
                    },
                    "workdir": {
                        "type": "string",
                        "description": "Working directory to execute the command in. Defaults to the user's home directory. When possible, avoid changing directories in the command and set the working directory here instead."
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Optional timeout in milliseconds (default 30000, max 120000). When it expires the command is killed."
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
        // Windows has no `sh`: use the command interpreter there, and keep the
        // POSIX shell everywhere else.
        let mut child = if cfg!(windows) {
            let mut cmd = tokio::process::Command::new("cmd");
            cmd.arg("/C").arg(command);
            cmd
        } else {
            let mut sh = tokio::process::Command::new("sh");
            sh.arg("-c").arg(command);
            sh
        };
        match arguments.get("workdir").and_then(Value::as_str) {
            Some(workdir) => {
                child.current_dir(workdir);
            }
            // No workdir given: run in the user's home, not the app folder.
            None => {
                if let Some(home) = std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                {
                    child.current_dir(home);
                }
            }
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
