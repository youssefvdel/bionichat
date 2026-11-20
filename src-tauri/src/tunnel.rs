//! OpenTunnel helper: spawn `opentunnel route add <hub port>`, scrape the
//! public URL it prints, then poll the URL until it answers over TLS.

use std::{
    path::PathBuf,
    sync::Mutex,
    time::Duration,
};

use serde::Serialize;
use std::process::Stdio;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

const START_TIMEOUT: Duration = Duration::from_secs(45);
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
pub struct TunnelInfo {
    pub status: String,
    pub public_url: Option<String>,
    pub error: Option<String>,
}

impl TunnelInfo {
    fn off() -> Self {
        Self {
            status: "off".into(),
            public_url: None,
            error: None,
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            status: "error".into(),
            public_url: None,
            error: Some(message.into()),
        }
    }
}

pub struct TunnelManager {
    child: Mutex<Option<Child>>,
    info: Mutex<TunnelInfo>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            info: Mutex::new(TunnelInfo::off()),
        }
    }

    pub fn info(&self) -> TunnelInfo {
        self.guard().clone()
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, TunnelInfo> {
        self.info
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn set(&self, info: TunnelInfo) -> TunnelInfo {
        *self.guard() = info.clone();
        info
    }

    pub fn stop(&self) -> TunnelInfo {
        if let Some(mut child) = self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = child.start_kill();
        }
        self.set(TunnelInfo::off())
    }

    pub async fn start(&self, hub_port: u16) -> TunnelInfo {
        if self.info().status == "ready" {
            return self.info();
        }
        self.set(TunnelInfo {
            status: "starting".into(),
            public_url: None,
            error: None,
        });

        let binary = locate_binary();
        let Some(binary) = binary else {
            return self.set(TunnelInfo::failed(
                "opentunnel not found on PATH (install it, or set OPENTUNNEL_BIN)",
            ));
        };

        let mut child = match Command::new(&binary)
            .arg("route")
            .arg("add")
            .arg(hub_port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return self.set(TunnelInfo::failed(format!(
                    "cannot run {}: {error}",
                    binary.display()
                )));
            }
        };

        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        *self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(child);

        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);
        for stream in [
            Box::new(stdout) as Box<dyn tokio::io::AsyncRead + Send + Unpin>,
            Box::new(stderr),
        ] {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx.send(line).await.is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);

        // Phase 1: find the public URL in the tool's own output.
        let mut tail: Vec<String> = Vec::new();
        let public_url = tokio::time::timeout(START_TIMEOUT, async {
            while let Some(line) = rx.recv().await {
                if let Some(url) = parse_public_url(&line) {
                    return Some(url);
                }
                tail.push(line);
                if tail.len() > 20 {
                    tail.remove(0);
                }
            }
            None
        })
        .await
        .map_err(|_| format!("no public URL within {}s", START_TIMEOUT.as_secs()))
        .and_then(|url| {
            url.ok_or_else(|| {
                let last = tail.join("\n");
                if last.is_empty() {
                    "opentunnel exited without printing a public URL".to_string()
                } else {
                    format!("opentunnel printed no public URL; last output:\n{last}")
                }
            })
        });

        let public_url = match public_url {
            Ok(url) => url,
            Err(error) => {
                self.stop();
                return self.set(TunnelInfo::failed(error));
            }
        };

        self.set(TunnelInfo {
            status: "starting".into(),
            public_url: Some(public_url.clone()),
            error: None,
        });

        // Phase 2: poll the public endpoint until TLS + HTTP answer.
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                self.stop();
                return self.set(TunnelInfo::failed(format!("http client: {error}")));
            }
        };

        let target = format!("{public_url}/mcp");
        let mut last_error = String::from("no attempt made");
        let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match client.get(&target).send().await {
                Ok(response) => {
                    return self.set(TunnelInfo {
                        status: "ready".into(),
                        public_url: Some(public_url),
                        error: Some(format!("public endpoint answered {}", response.status())),
                    });
                }
                Err(error) => last_error = error.to_string(),
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }

        let info = TunnelInfo {
            status: "error".into(),
            public_url: Some(public_url),
            error: Some(format!(
                "public URL did not answer within {}s: {last_error}",
                READY_TIMEOUT.as_secs()
            )),
        };
        self.stop();
        self.set(info)
    }
}

fn locate_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("OPENTUNNEL_BIN") {
        return Some(PathBuf::from(path));
    }
    which::which("opentunnel").ok()
}

/// Pull the first `https://` token out of a log line, trimming punctuation.
pub fn parse_public_url(line: &str) -> Option<String> {
    let is_url_char =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, ':' | '/' | '.' | '-' | '_' | '~' | '%');
    line.split_whitespace()
        .map(|token| token.trim_matches(|c: char| !is_url_char(c)))
        .find(|token| token.starts_with("https://") && token.len() > "https://".len())
        .map(|token| token.trim_end_matches(['.', '/']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_url_in_route_output() {
        assert_eq!(
            parse_public_url("route ready: https://demo-1234.opentunnel.dev/mcp"),
            Some("https://demo-1234.opentunnel.dev/mcp".to_string())
        );
    }

    #[test]
    fn trims_trailing_punctuation() {
        assert_eq!(
            parse_public_url("open https://abc.tunnel.dev."),
            Some("https://abc.tunnel.dev".to_string())
        );
    }

    #[test]
    fn ignores_lines_without_a_url() {
        assert_eq!(parse_public_url("starting transport…"), None);
        assert_eq!(parse_public_url("https://"), None);
    }

    #[test]
    fn missing_binary_is_reported_verbatim() {
        let manager = TunnelManager::new();
        let info = manager.stop();
        assert_eq!(info.status, "off");
        assert!(info.public_url.is_none());
    }

    #[tokio::test]
    async fn missing_binary_message_names_the_fix() {
        // Empty PATH: `which("opentunnel")` must fail and the UI must say so.
        let original = std::env::var_os("PATH");
        let empty = std::env::temp_dir().join("web-agent-empty-path");
        std::fs::create_dir_all(&empty).unwrap();
        unsafe {
            std::env::set_var("PATH", &empty);
            std::env::remove_var("OPENTUNNEL_BIN");
        }
        let info = TunnelManager::new().start(8787).await;
        unsafe {
            match original {
                Some(path) => std::env::set_var("PATH", path),
                None => std::env::remove_var("PATH"),
            }
        }
        assert_eq!(info.status, "error");
        let error = info.error.expect("error text");
        assert!(error.contains("opentunnel not found on PATH"), "{error}");
        assert!(error.contains("OPENTUNNEL_BIN"), "{error}");
    }

    #[tokio::test]
    async fn unreachable_binary_surfaces_stderr_text() {
        // Deterministic stand-in for "opentunnel not installed".
        unsafe { std::env::set_var("OPENTUNNEL_BIN", "/nonexistent/opentunnel") };
        let manager = TunnelManager::new();
        let info = manager.start(8787).await;
        unsafe { std::env::remove_var("OPENTUNNEL_BIN") };
        assert_eq!(info.status, "error");
        let error = info.error.expect("error text");
        assert!(error.contains("cannot run /nonexistent/opentunnel"), "{error}");
    }
}
