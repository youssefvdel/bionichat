//! core: the one built-in MCP server. `bash`, `read`, `write`, `edit` and
//! `skill` are all advertised from this single process, so the user sees one
//! server named core and one set of `core_*` tools.

use std::{path::PathBuf, sync::Arc};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
    transport::stdio,
};

#[derive(Clone)]
struct Core {
    fs: fs::Fs,
    skill: skill::Skill,
}

impl ServerHandler for Core {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("core", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "The user's own machine: shell, files and skills. No approval gate: call the tools directly.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut tools = fs::Fs::tools();
        tools.extend(bash::tools());
        tools.extend(skill::Skill::tools());
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = request.arguments.unwrap_or_default();
        let name = request.name.as_ref();
        match name {
            "bash" => bash::call(&arguments).await.map(Into::into),
            "read" | "write" | "edit" => self.fs.call(name, &arguments).map(Into::into),
            "skill" => self.skill.call(name, &arguments).map(Into::into),
            other => Err(McpError::invalid_params(
                format!("unknown tool `{other}`"),
                None,
            )),
        }
    }
}

/// `core <file-root> <skills-dir>`: the root bounds relative paths, the skills
/// directory is where project playbooks live.
fn config() -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = args
        .first()
        .map(PathBuf::from)
        .or_else(|| std::env::var("BIONICHAT_FS_ROOT").ok().map(PathBuf::from))
        .unwrap_or_else(|| std::env::current_dir().expect("working directory"));
    if !root.is_absolute() {
        return Err("core file root must be an absolute path".into());
    }
    let skills = args
        .get(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var("BIONICHAT_SKILLS_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    Ok((root, skills))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (root, skills) = config()?;
    let core = Core {
        fs: fs::Fs::new(root),
        skill: skill::Skill::new(skills),
    };
    let _ = Arc::new(core.clone());
    let service = core.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
