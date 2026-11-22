//! Registry: the on-disk list of MCP servers plus the Anthropic-format parser
//! used by the paste/import path.

use std::{
    collections::BTreeMap,
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Transport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl Transport {
    /// Human label for the server row. Never includes env values.
    pub fn label(&self) -> String {
        match self {
            Transport::Stdio { command, .. } => format!("stdio · {command}"),
            Transport::Http { url, .. } => format!("http · {url}"),
        }
    }

    /// Env/header *names* only: secrets stay out of the UI and the logs.
    pub fn secret_keys(&self) -> Vec<String> {
        match self {
            Transport::Stdio { env, .. } => env.keys().cloned().collect(),
            Transport::Http { headers, .. } => headers.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSpec {
    pub name: String,
    /// The tool namespace this server advertises (`core_read`). Defaults to
    /// the server name, so a server called context7 gives `context7_query-docs`.
    /// Built-in tools share one namespace, `core`.
    #[serde(default)]
    pub namespace: Option<String>,
    /// Restart the crashed server automatically (max 3 backoff attempts).
    #[serde(default)]
    pub autorestart: bool,
    #[serde(flatten)]
    pub transport: Transport,
}

impl ServerSpec {
    /// The namespace tools are advertised under.
    pub fn namespace(&self) -> &str {
        self.namespace.as_deref().unwrap_or(&self.name)
    }

    /// The advertised name for one of its tools, e.g. `core_bash`.
    pub fn advertised(&self, tool: &str) -> String {
        format!("{}_{tool}", self.namespace())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default)]
    pub servers: Vec<ServerSpec>,
    /// `server__tool` -> decision. Absent keys fall back to the defaults in
    /// `policy::decision` (read-only tools allow, everything else ask).
    #[serde(default)]
    pub policies: BTreeMap<String, Decision>,
}

fn one() -> u32 {
    1
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            version: 1,
            servers: Vec::new(),
            policies: BTreeMap::new(),
        }
    }
}

impl Registry {
    pub fn set_policy(&mut self, server: &str, tool: &str, decision: Decision) {
        self.policies
            .insert(format!("{server}__{tool}"), decision);
    }

    pub fn server(&self, name: &str) -> Option<&ServerSpec> {
        self.servers.iter().find(|spec| spec.name == name)
    }

    pub fn server_mut(&mut self, name: &str) -> Option<&mut ServerSpec> {
        self.servers.iter_mut().find(|spec| spec.name == name)
    }
}

pub fn load(path: &Path) -> Option<Registry> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(registry) => Some(registry),
        Err(error) => {
            // Never lose a user's servers to a bad edit: keep the broken file
            // beside the original and say so, instead of silently reseeding.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default();
            let backup = path.with_extension(format!("json.bad-{stamp}"));
            let _ = fs::copy(path, &backup);
            eprintln!(
                "registry: {} is not valid JSON ({error}); kept a copy at {}",
                path.display(),
                backup.display()
            );
            None
        }
    }
}

pub fn save(path: &Path, registry: &Registry) -> Result<(), String> {
    let text = serde_json::to_string_pretty(registry).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// The two seeded servers. `exe_dir` is where cargo puts sibling binaries.
pub fn default_specs(exe_dir: &Path, file_root: &Path, skills_dir: &Path) -> Vec<ServerSpec> {
    vec![
        ServerSpec {
            name: "fs".into(),
            namespace: Some("core".into()),
            autorestart: false,
            transport: Transport::Stdio {
                command: exe_dir.join("fs").display().to_string(),
                args: vec![file_root.display().to_string()],
                env: BTreeMap::new(),
            },
        },
        ServerSpec {
            name: "bash".into(),
            namespace: Some("core".into()),
            autorestart: false,
            transport: Transport::Stdio {
                command: exe_dir.join("bash").display().to_string(),
                args: vec![],
                env: BTreeMap::new(),
            },
        },
        ServerSpec {
            name: "skill".into(),
            namespace: Some("core".into()),
            autorestart: false,
            transport: Transport::Stdio {
                command: exe_dir.join("skill").display().to_string(),
                args: vec![skills_dir.display().to_string()],
                env: BTreeMap::new(),
            },
        },
    ]
}

/// Parse Anthropic-style config:
/// `{ "mcpServers": { "name": { "command": ..., "args": [...], "env": {...} }
///                                 | { "url": ..., "headers": {...} } } }`.
///
/// Every failure names the exact JSON path so the dashboard can print one line.
pub fn parse_anthropic(input: &str) -> Result<Vec<ServerSpec>, String> {
    let root: Value = serde_json::from_str(input).map_err(|e| {
        format!(
            "invalid JSON at line {} column {}: {}",
            e.line(),
            e.column(),
            e
        )
    })?;

    let servers = root
        .get("mcpServers")
        .ok_or("root: missing `mcpServers` object")?
        .as_object()
        .ok_or("mcpServers: expected an object")?;

    if servers.is_empty() {
        return Err("mcpServers: no servers defined".into());
    }

    let mut specs = Vec::with_capacity(servers.len());
    for (name, entry) in servers {
        let name = name.trim();
        if name.is_empty() {
            return Err("mcpServers: server name cannot be empty".into());
        }
        let at = format!("mcpServers.{name}");
        let entry = entry
            .as_object()
            .ok_or_else(|| format!("{at}: expected an object"))?;

        let transport = match (entry.get("command"), entry.get("url")) {
            (Some(command), None) => {
                let command = command
                    .as_str()
                    .ok_or_else(|| format!("{at}.command: expected a string"))?
                    .trim()
                    .to_string();
                if command.is_empty() {
                    return Err(format!("{at}.command: cannot be empty"));
                }
                Transport::Stdio {
                    command,
                    args: string_list(entry.get("args"), &format!("{at}.args"))?,
                    env: string_map(entry.get("env"), &format!("{at}.env"))?,
                }
            }
            (None, Some(url)) => {
                let url = url
                    .as_str()
                    .ok_or_else(|| format!("{at}.url: expected a string"))?;
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err(format!("{at}.url: must start with http:// or https://"));
                }
                Transport::Http {
                    url: url.to_string(),
                    headers: string_map(entry.get("headers"), &format!("{at}.headers"))?,
                }
            }
            (Some(_), Some(_)) => {
                return Err(format!("{at}: has both `command` and `url`, pick one"));
            }
            (None, None) => {
                return Err(format!("{at}: needs `command` (stdio) or `url` (http)"));
            }
        };

        specs.push(ServerSpec {
            name: name.to_string(),
            namespace: None,
            autorestart: false,
            transport,
        });
    }

    let mut seen: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    seen.sort_unstable();
    if let Some(dup) = seen.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("mcpServers.{}: defined twice", dup[0]));
    }

    Ok(specs)
}

fn string_list(value: Option<&Value>, at: &str) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(vec![]);
    };
    if value.is_null() {
        return Ok(vec![]);
    }
    let array = value
        .as_array()
        .ok_or_else(|| format!("{at}: expected an array of strings"))?;
    array
        .iter()
        .enumerate()
        .map(|(i, item)| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{at}[{i}]: expected a string"))
        })
        .collect()
}

fn string_map(value: Option<&Value>, at: &str) -> Result<BTreeMap<String, String>, String> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    if value.is_null() {
        return Ok(BTreeMap::new());
    }
    let map = value
        .as_object()
        .ok_or_else(|| format!("{at}: expected an object of strings"))?;
    map.iter()
        .map(|(k, v)| {
            v.as_str()
                .map(|v| (k.clone(), v.to_string()))
                .ok_or_else(|| format!("{at}.{k}: expected a string"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stdio_and_http() {
        let specs = parse_anthropic(
            r#"{"mcpServers":{
                "local":{"command":"bun","args":["x.ts"],"env":{"TOKEN":"secret"}},
                "remote":{"url":"https://example.com/mcp","headers":{"Authorization":"bearer x"}}
            }}"#,
        )
        .expect("valid config");
        assert_eq!(specs.len(), 2);
        let local = &specs[0];
        assert_eq!(local.name, "local");
        assert_eq!(local.transport.secret_keys(), vec!["TOKEN".to_string()]);
        match &specs[1].transport {
            Transport::Http { url, .. } => assert_eq!(url, "https://example.com/mcp"),
            other => panic!("expected http, got {other:?}"),
        }
    }

    #[test]
    fn rejects_without_mcp_servers() {
        let err = parse_anthropic(r#"{"servers":{}}"#).unwrap_err();
        assert!(err.contains("mcpServers"), "{err}");
    }

    #[test]
    fn rejects_ambiguous_entry() {
        let err =
            parse_anthropic(r#"{"mcpServers":{"x":{"command":"a","url":"https://b"}}}"#).unwrap_err();
        assert!(err.contains("both"), "{err}");
    }

    #[test]
    fn reports_bad_json_with_position() {
        let err = parse_anthropic("{oops}").unwrap_err();
        assert!(err.starts_with("invalid JSON at line 1"), "{err}");
    }
}
