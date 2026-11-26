//! bash: the shell tool. Runs a command via `sh -c` (or `cmd /C` on Windows)
//! and returns exit code, stdout and stderr. A call may run in the background:
//! it returns a job id immediately and the job's log and exit file can be read
//! later with `job`.
//!
//! Foreground calls are capped (default 30 s, max 120 s). Background jobs have
//! no cap; that is the point of them.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

use rmcp::{
    ErrorData as McpError,
    model::{
        CallToolResult, ContentBlock, Tool, ToolAnnotations,
    },
};
use serde_json::{Map, Value, json};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_TIMEOUT_MS: u64 = 120_000;
const OUTPUT_LIMIT: usize = 8 * 1024;

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

pub fn tools() -> Vec<Tool> {
    vec![Tool::new(
        "bash",
        "Execute a shell command and return its output (exit code, stdout and stderr). Quote file paths containing spaces or special characters. Prefer the read, write and edit tools over shell commands when one of them does the job. Commands still running after their timeout are stopped. Pass workdir instead of changing directories inside the command. Set `background` true for a long command: it returns a job id at once, and `job` reads its log and state later.",
        schema(
            json!({
                "command": {
                    "type": "string",
                    "description": "Shell command to run. Omit when reading or killing a job."
                },
                "workdir": {
                    "type": "string",
                    "description": "Working directory to execute the command in. Defaults to the user's home directory. When possible, avoid changing directories in the command and set the working directory here instead."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Optional timeout in milliseconds (default 30000, max 120000). When it expires the command is killed. Ignored for background jobs."
                },
                "background": {
                    "type": "boolean",
                    "description": "Run detached and return a job id immediately. No timeout. Read the log with `job`."
                },
                "job": {
                    "type": "string",
                    "description": "Job id to inspect: returns its state (running or exit code) and the tail of its output."
                },
                "kill": {
                    "type": "boolean",
                    "description": "With `job`: stop that job."
                }
            }),
            &[],
        ),
    )
    .with_annotations(ToolAnnotations::new().read_only(false))]
}

fn jobs_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("bionichat/jobs");
        let _ = std::fs::create_dir_all(&dir);
        dir
    })
    .clone()
}

fn new_job_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{millis:x}{:04x}", std::process::id() & 0xffff)
}

fn shell(command: &str, workdir: Option<&str>) -> tokio::process::Command {
    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt as _;
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.arg("/C");
        cmd.as_std_mut().raw_arg(command);
        cmd
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut sh = tokio::process::Command::new("sh");
        sh.arg("-c").arg(command);
        sh
    };
    match workdir {
        Some(dir) => {
            cmd.current_dir(dir);
        }
        None => {
            if let Some(home) =
                std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
            {
                cmd.current_dir(home);
            }
        }
    }
    cmd
}

fn report(exit: Option<i32>, stdout: &[u8], stderr: &[u8]) -> String {
    let report = json!({
        "exit_code": exit,
        "stdout": truncate(&String::from_utf8_lossy(stdout)),
        "stderr": truncate(&String::from_utf8_lossy(stderr)),
    });
    serde_json::to_string_pretty(&report).expect("report serializes")
}

/// Start a detached job. Returns the job id, or an internal error.
fn start_background(command: &str, workdir: Option<&str>) -> Result<String, McpError> {
    let id = new_job_id();
    let dir = jobs_dir();
    let log = dir.join(format!("{id}.log"));
    let out = std::fs::File::create(&log)
        .map_err(|e| McpError::internal_error(format!("cannot create job log: {e}"), None))?;
    let err = out
        .try_clone()
        .map_err(|e| McpError::internal_error(format!("cannot clone job log: {e}"), None))?;
    // The wrapper writes the exit code once the command ends, so `job` can tell
    // running from finished without a process table.
    let wrapped = format!("{{ {command}\n}} > '{}' 2>&1; echo $? > '{}'", log.display(), dir.join(format!("{id}.exit")).display());
    let mut child = shell(&wrapped, workdir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| McpError::internal_error(format!("cannot spawn job: {e}"), None))?;
    if let Some(pid) = child.id() {
        let _ = std::fs::write(dir.join(format!("{id}.pid")), pid.to_string());
    }
    // Detach: do not wait, and do not kill on drop.
    drop(child);
    let _ = err;
    Ok(id)
}

fn read_job(id: &str) -> Result<String, McpError> {
    let dir = jobs_dir();
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(McpError::invalid_params("bash: bad job id", None));
    }
    let log = dir.join(format!("{id}.log"));
    let exit = dir.join(format!("{id}.exit"));
    if !log.exists() && !exit.exists() {
        return Err(McpError::invalid_params(format!("bash: no job `{id}`"), None));
    }
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let tail: String = if text.len() > OUTPUT_LIMIT {
        let start = text.len() - OUTPUT_LIMIT;
        format!("…[earlier output omitted]{}", &text[start..])
    } else {
        text
    };
    let state = match std::fs::read_to_string(&exit) {
        Ok(code) => json!({ "state": "done", "exit_code": code.trim().parse::<i32>().ok() }),
        Err(_) => json!({ "state": "running" }),
    };
    let mut report = state;
    report["job"] = json!(id);
    report["output"] = json!(tail);
    Ok(serde_json::to_string_pretty(&report).expect("job report serializes"))
}

fn kill_job(id: &str) -> Result<String, McpError> {
    let dir = jobs_dir();
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(McpError::invalid_params("bash: bad job id", None));
    }
    let pid = std::fs::read_to_string(dir.join(format!("{id}.pid")))
        .map_err(|_| McpError::invalid_params(format!("bash: no job `{id}`"), None))?;
    let pid = pid.trim();
    if pid.is_empty() {
        return Err(McpError::invalid_params(format!("bash: job `{id}` has no pid"), None));
    }
    #[cfg(windows)]
    let status = std::process::Command::new("taskkill")
        .args(["/PID", pid, "/T", "/F"])
        .status();
    #[cfg(not(windows))]
    let status = std::process::Command::new("kill").args(["-TERM", pid]).status();
    let ok = status.map(|s| s.success()).unwrap_or(false);
    Ok(json!({ "job": id, "killed": ok }).to_string())
}

pub async fn call(arguments: &Map<String, Value>) -> Result<CallToolResult, McpError> {
    if let Some(id) = arguments.get("job").and_then(Value::as_str) {
        if arguments.get("kill").and_then(Value::as_bool).unwrap_or(false) {
            return Ok(CallToolResult::success(vec![ContentBlock::text(kill_job(id)?)]));
        }
        return Ok(CallToolResult::success(vec![ContentBlock::text(read_job(id)?)]));
    }
    let command = arguments
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| McpError::invalid_params("bash requires a string `command`", None))?;
    if command.trim().is_empty() {
        return Err(McpError::invalid_params("bash: empty command", None));
    }
    let workdir = arguments.get("workdir").and_then(Value::as_str);
    if arguments.get("background").and_then(Value::as_bool).unwrap_or(false) {
        let id = start_background(command, workdir)?;
        let dir = jobs_dir();
        let body = json!({
            "job": id,
            "state": "running",
            "log": dir.join(format!("{id}.log")).display().to_string(),
            "note": "Call bash with {\"job\": \"<id>\"} for output and state, {\"job\": \"<id>\", \"kill\": true} to stop it.",
        });
        return Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&body).expect("job body serializes"),
        )]));
    }
    let timeout_ms = arguments
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1_000, MAX_TIMEOUT_MS);
    let child = shell(command, workdir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| McpError::internal_error(format!("cannot spawn shell: {e}"), None))?;
    let output = tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait_with_output())
        .await
        .map_err(|_| McpError::internal_error("bash timed out — process killed", None))?
        .map_err(|e| McpError::internal_error(format!("bash failed: {e}"), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(report(
        output.status.code(),
        &output.stdout,
        &output.stderr,
    ))]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(out: &CallToolResult) -> String {
        serde_json::to_value(out).unwrap()["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    #[tokio::test]
    async fn runs_and_reports_exit_code() {
        let mut args = Map::new();
        args.insert("command".into(), json!("printf hi; exit 3"));
        let text = text_of(&call(&args).await.unwrap());
        assert!(text.contains("\"exit_code\": 3"), "{text}");
        assert!(text.contains("hi"), "{text}");
    }

    #[tokio::test]
    async fn background_job_reports_running_then_done() {
        let mut args = Map::new();
        args.insert("command".into(), json!("sleep 0.2; echo bg-ok"));
        args.insert("background".into(), json!(true));
        let started = text_of(&call(&args).await.unwrap());
        let id = started
            .split("\"job\": \"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("job id");
        tokio::time::sleep(Duration::from_millis(700)).await;
        let mut inspect = Map::new();
        inspect.insert("job".into(), json!(id));
        let text = text_of(&call(&inspect).await.unwrap());
        assert!(text.contains("\"state\": \"done\""), "{text}");
        assert!(text.contains("bg-ok"), "{text}");
    }
}
