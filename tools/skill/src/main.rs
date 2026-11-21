//! skill: the skill manager — load project playbooks on demand.
//!
//! Skills live in the `.agents` folder: `<base>/skills/<name>/SKILL.md`,
//! where `<base>` is argv[1], `BIONICHAT_SKILLS_DIR`, or `~/.agents`.
//! The built-in `bionichat` skill (how to use this hub) is always listed
//! unless the folder overrides it. Loading is read-only; adding a skill
//! is just dropping a folder with a SKILL.md in it.

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

/// The built-in skill: always available, overridden by a folder skill.
const BUILTIN: &str = include_str!("builtin-bionichat.md");

#[derive(Clone)]
struct SkillServer {
    base: Arc<std::path::PathBuf>,
}

impl SkillServer {
    fn base_dir() -> std::path::PathBuf {
        if let Some(arg) = std::env::args().nth(1) {
            return std::path::PathBuf::from(arg);
        }
        if let Ok(dir) = std::env::var("BIONICHAT_SKILLS_DIR") {
            if !dir.trim().is_empty() {
                return std::path::PathBuf::from(dir);
            }
        }
        let mut home = dirs_home();
        home.push(".agents");
        home
    }

    fn skills_dir(&self) -> std::path::PathBuf {
        self.base.join("skills")
    }

    /// Names that are safe to join onto the skills dir: no separators,
    /// no climbing out.
    fn valid_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    fn read_skill(&self, name: &str) -> Option<String> {
        if !Self::valid_name(name) {
            return None;
        }
        let path = self.skills_dir().join(name).join("SKILL.md");
        std::fs::read_to_string(path).ok()
    }

    fn summary(text: &str) -> String {
        for line in text.lines() {
            let line = line.trim().trim_start_matches('#').trim();
            if !line.is_empty() {
                let short: String = line.chars().take(120).collect();
                return short;
            }
        }
        "(no summary)".to_string()
    }

    fn list(&self) -> CallToolResult {
        let mut entries: Vec<(String, String)> = Vec::new();
        if let Ok(dir) = std::fs::read_dir(self.skills_dir()) {
            let mut names: Vec<String> = dir
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|entry| {
                    entry.file_name().into_string().ok().filter(|n| Self::valid_name(n))
                })
                .filter(|name| self.skills_dir().join(name).join("SKILL.md").is_file())
                .collect();
            names.sort();
            names.dedup();
            for name in names {
                let summary = self
                    .read_skill(&name)
                    .map(|text| Self::summary(&text))
                    .unwrap_or_else(|| "(unreadable)".to_string());
                entries.push((name, summary));
            }
        }
        if !entries.iter().any(|(name, _)| name == "bionichat") {
            entries.insert(0, ("bionichat".to_string(), Self::summary(BUILTIN)));
        }
        if entries.is_empty() {
            return CallToolResult::success(vec![ContentBlock::text(
                "no skills installed — drop a folder with a SKILL.md into <base>/skills/",
            )]);
        }
        let lines: Vec<String> = entries
            .iter()
            .map(|(name, summary)| format!("{name} — {summary}"))
            .collect();
        CallToolResult::success(vec![ContentBlock::text(format!(
            "skills (read one with action \"read\"):\n{}",
            lines.join("\n")
        ))])
    }

    fn read(&self, arguments: &Map<String, Value>) -> CallToolResult {
        let name = match arguments.get("name").and_then(Value::as_str) {
            Some(name) => name.trim().to_string(),
            None => {
                return CallToolResult::error(vec![ContentBlock::text(
                    "skill read requires a string `name`",
                )])
            }
        };
        if !Self::valid_name(&name) {
            return CallToolResult::error(vec![ContentBlock::text(format!(
                "`{name}` is not a valid skill (letters, digits, -, _)"
            ))]);
        }
        if name == "bionichat" && self.skills_dir().join(&name).join("SKILL.md").is_file() {
            // A folder skill wins over the built-in one; fall through to read_skill.
        } else if name == "bionichat" {
            return CallToolResult::success(vec![ContentBlock::text(BUILTIN)]);
        }
        match self.read_skill(&name) {
            Some(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            None => CallToolResult::error(vec![ContentBlock::text(format!(
                "no skill named `{name}`"
            ))]),
        }
    }
}

fn dirs_home() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        return std::path::PathBuf::from(home);
    }
    std::path::PathBuf::from(".")
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

impl ServerHandler for SkillServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("skill", env!("CARGO_PKG_VERSION")))
            .with_instructions("Skill manager: list and load the project's playbooks.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![
            Tool::new(
                "skill",
                "Lists the project's skills or loads one by name. Load the skill before doing the work it covers.",
                schema(
                    json!({
                        "action": { "type": "string", "enum": ["list", "read"], "description": "list skills, or read one skill's playbook" },
                        "name": { "type": "string", "description": "Skill name for action read" }
                    }),
                    &["action"],
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
        let result = match request.name.as_ref() {
            "skill" => {
                let action = arguments.get("action").and_then(Value::as_str).unwrap_or("");
                match action {
                    "list" => self.list(),
                    "read" => self.read(&arguments),
                    _ => CallToolResult::error(vec![ContentBlock::text(
                        "skill requires action \"list\" or \"read\"",
                    )]),
                }
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown tool `{other}`"),
                    None,
                ));
            }
        };
        Ok(result.into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = SkillServer {
        base: Arc::new(SkillServer::base_dir()),
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
