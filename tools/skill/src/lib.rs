//! skill: the skill manager — load project playbooks on demand.
//!
//! Skills live in the `.agents` folder: `<base>/skills/<name>/SKILL.md`,
//! where `<base>` is argv[1], `BIONICHAT_SKILLS_DIR`, or `~/.agents`.
//! The built-in `bionichat` skill (how to use this hub) is always listed
//! unless the folder overrides it. Loading is read-only; adding a skill
//! is just dropping a folder with a SKILL.md in it.

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, ContentBlock, Tool, ToolAnnotations},
};
use serde_json::{Map, Value, json};

/// The built-in skill: always available, overridden by a folder skill.
const BUILTIN: &str = include_str!("builtin-bionichat.md");

#[derive(Clone)]
pub struct Skill {
    base: Arc<std::path::PathBuf>,
}

impl Skill {
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

impl Skill {
    pub fn new(base: std::path::PathBuf) -> Self {
        Self { base: Arc::new(base) }
    }

    pub fn tools() -> Vec<Tool> {
        vec![
            Tool::new(
                "skill",
                "Load a specialized skill's instructions into the current conversation when the task at hand matches its description, or list the skills available in this project. Call it with action list first when you do not know the name, then action read with the name. Load a skill before doing the work it covers.",
                schema(
                    json!({
                        "action": { "type": "string", "enum": ["list", "read"], "description": "list every available skill, or read one skill's playbook" },
                        "name": { "type": "string", "description": "The ID of an available skill or a skill explicitly referenced by the user. Required when action is read." }
                    }),
                    &["action"],
                ),
            )
            .with_annotations(ToolAnnotations::new().read_only(true)),
        ]
    }

    pub fn call(
        &self,
        name: &str,
        arguments: &Map<String, Value>,
    ) -> Result<CallToolResult, McpError> {
        match name {
            "skill" => match arguments.get("action").and_then(Value::as_str).unwrap_or("") {
                "list" => Ok(self.list()),
                "read" => Ok(self.read(arguments)),
                _ => Ok(CallToolResult::error(vec![ContentBlock::text(
                    "skill requires action \"list\" or \"read\"",
                )])),
            },
            other => Err(McpError::invalid_params(
                format!("unknown tool `{other}`"),
                None,
            )),
        }
    }
}
