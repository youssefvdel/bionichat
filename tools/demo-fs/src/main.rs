//! demo-fs: read-only, scoped filesystem server for the P1 dashboard.
//! The root comes from argv[1] (or `BIONICHAT_DEMO_ROOT`); every path is
//! canonicalised and must stay inside that root.

use std::{path::{Path, PathBuf}, sync::Arc};

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
struct FsServer {
    root: Arc<PathBuf>,
}

impl FsServer {
    fn resolve(&self, requested: &str) -> Result<PathBuf, String> {
        let candidate = self.root.join(requested);
        let canonical = candidate
            .canonicalize()
            .map_err(|e| format!("cannot open `{requested}`: {e}"))?;
        if !canonical.starts_with(self.root.as_path()) {
            return Err(format!("`{requested}` escapes the demo root"));
        }
        Ok(canonical)
    }
}

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

impl ServerHandler for FsServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("demo-fs", env!("CARGO_PKG_VERSION")))
            .with_instructions("Read-only filesystem demo, scoped to one directory.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![Tool::new(
            "fs.read",
            "Reads a UTF-8 text file inside the demo root.",
            schema(
                json!({
                    "path": {
                        "type": "string",
                        "description": "Path relative to the demo root"
                    }
                }),
                &["path"],
            ),
        )
        .with_annotations(ToolAnnotations::new().read_only(true))]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name.as_ref() != "fs.read" {
            return Err(McpError::invalid_params(
                format!("unknown tool `{}`", request.name),
                None,
            ));
        }
        let arguments = request.arguments.unwrap_or_default();
        let requested = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| McpError::invalid_params("fs.read requires a string `path`", None))?;

        Ok(match self.resolve(requested) {
            Ok(path) => match std::fs::read_to_string(&path) {
                Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
                Err(e) => CallToolResult::error(vec![ContentBlock::text(format!("{e}"))]),
            },
            Err(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
        }
        .into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("BIONICHAT_DEMO_ROOT").ok())
        .ok_or("demo-fs needs a root directory as argv[1] or BIONICHAT_DEMO_ROOT")?;
    let root = Path::new(&root);
    std::fs::create_dir_all(root)?;
    let root = root.canonicalize()?;

    let service = FsServer {
        root: Arc::new(root),
    }
    .serve(stdio())
    .await?;
    service.waiting().await?;
    Ok(())
}
