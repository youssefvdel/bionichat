//! A chat that runs on this machine: pick a project, talk to any
//! OpenAI-compatible model, and let it use the same tools the hub exposes.
//! The model never gets a network path to the machine: every tool call comes
//! back through `Hub::handle_rpc`, so policies, activity and the server clock
//! all behave exactly as they do for a web client.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::hub::Hub;

pub const KEY_MASK: &str = "••••••••";

/// Where the model lives. Any OpenAI-compatible endpoint works.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelView {
    pub base_url: String,
    pub model: String,
    /// The key itself never leaves the backend; the UI only learns whether one
    /// is stored, and the last four characters to recognise it.
    pub key_hint: String,
    pub configured: bool,
}

impl ModelConfig {
    pub fn view(&self) -> ModelView {
        let hint = if self.api_key.len() > 4 {
            format!("…{}", &self.api_key[self.api_key.len() - 4..])
        } else if self.api_key.is_empty() {
            String::new()
        } else {
            KEY_MASK.to_string()
        };
        ModelView {
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            key_hint: hint,
            configured: !self.api_key.trim().is_empty() && !self.model.trim().is_empty(),
        }
    }
}

pub fn load_config(path: &Path) -> ModelConfig {
    let Ok(text) = std::fs::read_to_string(path) else {
        return ModelConfig::default();
    };
    serde_json::from_str::<ModelConfig>(&text).unwrap_or_default()
}

pub fn save_config(path: &Path, config: &ModelConfig) -> Result<(), String> {
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("cannot save model config: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// One message in the conversation as the UI keeps it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
    /// Tool calls this assistant turn made, already rendered by the UI.
    #[serde(default)]
    pub calls: Vec<ChatCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCall {
    pub name: String,
    pub arguments: Value,
    pub result: String,
    pub ok: bool,
}

pub struct Chat {
    pub hub: Arc<Hub>,
    pub config_path: PathBuf,
}

impl Chat {
    pub fn config(&self) -> ModelConfig {
        load_config(&self.config_path)
    }

    pub fn save(&self, config: &ModelConfig) -> Result<ModelView, String> {
        save_config(&self.config_path, config)?;
        Ok(config.view())
    }

    /// Tool definitions in the shape OpenAI expects, taken from the hub so a
    /// local chat sees exactly what a web client sees.
    async fn tools(&self) -> Vec<Value> {
        let (_, response) = self
            .hub
            .handle_rpc(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .await;
        let Some(response) = response else {
            return Vec::new();
        };
        response
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|tool| {
                        let name = tool.get("name")?.as_str()?;
                        Some(json!({
                            "type": "function",
                            "function": {
                                "name": name,
                                "description": tool.get("description").and_then(Value::as_str).unwrap_or(""),
                                "parameters": tool.get("inputSchema").cloned().unwrap_or(json!({"type":"object","properties":{}})),
                            }
                        }))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Run one tool through the hub, which applies policy, activity, and the
    /// server clock exactly like a remote call. Shell commands default to the
    /// project folder, so "ls" means the project, not the home directory.
    async fn run_tool(&self, project: &str, name: &str, mut arguments: Value) -> (String, bool) {
        if name == "bash" {
            let missing = arguments
                .get("workdir")
                .and_then(Value::as_str)
                .map(|workdir| workdir.trim().is_empty())
                .unwrap_or(true);
            if missing {
                if let Some(map) = arguments.as_object_mut() {
                    map.insert("workdir".to_string(), Value::String(project.to_string()));
                }
            }
        }
        let (_, response) = self
            .hub
            .handle_rpc(json!({
                "jsonrpc":"2.0","id":1,"method":"tools/call",
                "params": {"name": name, "arguments": arguments}
            }))
            .await;
        let Some(response) = response else {
            return ("the hub closed the call".to_string(), false);
        };
        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("tool failed");
            return (message.to_string(), false);
        }
        let ok = response
            .pointer("/result/isError")
            .and_then(Value::as_bool)
            .map(|failed| !failed)
            .unwrap_or(true);
        let text = response
            .pointer("/result/content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| block.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        (text, ok)
    }

    /// Send the conversation, let the model call tools until it answers, and
    /// return the assistant turn plus everything it ran on the way.
    pub async fn send(
        &self,
        project: &str,
        history: &[ChatTurn],
        prompt: &str,
        emit: impl Fn(&str, Value),
    ) -> Result<ChatTurn, String> {
        let config = self.config();
        if config.api_key.trim().is_empty() {
            return Err("No model key yet. Settings → Model.".to_string());
        }
        let tools = self.tools().await;
        let system = format!(
            "You are bionichat, a coding agent running on the user's own machine.\n\
             The project folder is {project}. Tools run with that machine's full access and\n\
             do not ask for approval. Use bash for anything a terminal could do (ls, grep,\n\
             git, tests, builds) and read/write/edit for files. Keep answers short and\n\
             concrete: say what you ran and what came back. Never invent output."
        );

        let mut messages: Vec<Value> = vec![json!({"role":"system","content":system})];
        for turn in history {
            let mut content = turn.content.clone();
            if !turn.calls.is_empty() {
                let ran: Vec<String> = turn
                    .calls
                    .iter()
                    .map(|call| format!("{}({})", call.name, call.arguments))
                    .collect();
                if content.is_empty() {
                    content = format!("[ran: {}]", ran.join(", "));
                }
            }
            messages.push(json!({"role": turn.role, "content": content}));
        }
        messages.push(json!({"role":"user","content": prompt}));

        let mut calls: Vec<ChatCall> = Vec::new();
        let mut text = String::new();

        for round in 0..8 {
            let body = json!({
                "model": config.model,
                "messages": messages,
                "tools": tools,
                "tool_choice": "auto",
                "temperature": 0.2,
            });
            let reply = self.complete(&config, &body).await?;
            let message = reply
                .pointer("/choices/0/message")
                .cloned()
                .ok_or("the model sent no message")?;

            text = message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let wanted = message
                .get("tool_calls")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            if wanted.is_empty() || round == 7 {
                break;
            }

            messages.push(message.clone());
            for call in wanted {
                let name = call
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let raw = call
                    .pointer("/function/arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                let arguments: Value = serde_json::from_str(raw).unwrap_or(json!({}));
                emit("chat-tool-start", json!({"name": name, "arguments": arguments}));
                let (result, ok) = self.run_tool(project, &name, arguments.clone()).await;
                emit(
                    "chat-tool-done",
                    json!({"name": name, "arguments": arguments, "result": result, "ok": ok}),
                );
                calls.push(ChatCall {
                    name,
                    arguments: arguments.clone(),
                    result: result.clone(),
                    ok,
                });
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call.get("id").cloned().unwrap_or(Value::Null),
                    "content": result,
                }));
            }
        }

        Ok(ChatTurn {
            role: "assistant".to_string(),
            content: text,
            calls,
        })
    }

    async fn complete(&self, config: &ModelConfig, body: &Value) -> Result<Value, String> {
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .post(&url)
            .bearer_auth(&config.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("cannot reach {url}: {e}"))?;
        let status = response.status();
        let text = response.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("{}: {}", status.as_u16(), shorten(&text, 400)));
        }
        serde_json::from_str(&text).map_err(|e| format!("bad JSON from {url}: {e}"))
    }
}

fn shorten(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    format!("{}…", &text[..limit])
}

/// The hub answers 202 for notifications; a chat call never is one, so this
/// keeps the borrow checker happy about the unused status.
#[allow(dead_code)]
fn is_notification(status: StatusCode) -> bool {
    status == StatusCode::ACCEPTED
}
