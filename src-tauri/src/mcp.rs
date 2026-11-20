//! Thin wrapper over the `rmcp` client: spawn a stdio MCP server, complete the
//! handshake, ask for `tools/list`, then shut the child down again.

use std::time::Duration;

use rmcp::{
    ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess, which_command},
};
use serde::Serialize;

use crate::registry::{ServerSpec, Transport};

const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub read_only: bool,
}

pub async fn list_tools(spec: &ServerSpec) -> Result<Vec<ToolInfo>, String> {
    let Transport::Stdio { command, args, env } = &spec.transport else {
        return Err("http transport is not wired yet (P2)".to_string());
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

    let probe = async {
        let client = ()
            .serve(transport)
            .await
            .map_err(|e| format!("handshake failed: {e}"))?;
        let tools = client
            .list_all_tools()
            .await
            .map_err(|e| format!("tools/list failed: {e}"))?;
        let _ = client.cancel().await;
        Ok::<_, String>(
            tools
                .into_iter()
                .map(|tool| ToolInfo {
                    name: tool.name.to_string(),
                    description: tool.description.map(|d| d.to_string()),
                    read_only: tool
                        .annotations
                        .as_ref()
                        .and_then(|a| a.read_only_hint)
                        .unwrap_or(false),
                })
                .collect(),
        )
    };

    tokio::time::timeout(PROBE_TIMEOUT, probe)
        .await
        .map_err(|_| format!("no handshake within {}s", PROBE_TIMEOUT.as_secs()))?
}
