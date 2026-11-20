//! demo-ping: the read-only smoke server for the P1 dashboard.
//! Two tools, no arguments worth validating, no side effects.

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
        ToolAnnotations,
    },
    service::RequestContext,
    transport::stdio,
};
use serde_json::{Map, Value, json};

#[derive(Clone)]
struct PingServer;

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

impl ServerHandler for PingServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("demo-ping", env!("CARGO_PKG_VERSION")))
            .with_instructions("Read-only demo server: ping and echo.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![
            Tool::new(
                "ping",
                "Returns the literal string pong.",
                schema(json!({}), &[]),
            )
            .with_annotations(ToolAnnotations::new().read_only(true)),
            Tool::new(
                "echo",
                "Echoes the given text back unchanged.",
                schema(
                    json!({
                        "text": { "type": "string", "description": "Text to echo back" }
                    }),
                    &["text"],
                ),
            )
            .with_annotations(ToolAnnotations::new().read_only(true)),
        ]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = request.arguments.unwrap_or_default();
        let reply = match request.name.as_ref() {
            "ping" => "pong".to_string(),
            "echo" => arguments
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| McpError::invalid_params("echo requires a string `text`", None))?
                .to_string(),
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown tool `{other}`"),
                    None,
                ));
            }
        };
        Ok(CallToolResult::success(vec![ContentBlock::text(reply)]).into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = PingServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
