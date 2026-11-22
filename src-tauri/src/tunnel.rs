//! The public link.
//!
//! A chat app on the web cannot reach `127.0.0.1`, so the hub is published
//! through OpenTunnel. The app ships its own client (`bionichat-tunnel`),
//! built on the upstream library, because the upstream CLI refuses to build
//! outside Unix: its background service speaks over a Unix socket.
//!
//! The helper runs in the foreground and this module supervises it:
//!
//! * start - spawn it, read the public URL it prints, poll until the URL
//!   answers over TLS, then keep the child alive
//! * stop - kill the child; the hostname stays valid for next time
//! * revoke - delete the tunnel server-side, so the next start mints a new
//!   hostname and a leaked link is dead for good

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};

const START_TIMEOUT: Duration = Duration::from_secs(60);
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
pub struct TunnelInfo {
    pub status: String,
    pub public_url: Option<String>,
    pub error: Option<String>,
    /// Which helper serves the link; always `opentunnel` today.
    pub kind: Option<String>,
}

impl TunnelInfo {
    fn off() -> Self {
        Self {
            status: "off".into(),
            public_url: None,
            error: None,
            kind: None,
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            status: "error".into(),
            public_url: None,
            error: Some(message.into()),
            kind: None,
        }
    }
}

/// The helper that ships beside the app binary.
#[derive(Clone, Debug)]
pub struct Helper {
    path: PathBuf,
    profile: String,
}

impl Helper {
    pub fn label(&self) -> &'static str {
        "opentunnel"
    }

    fn start_args(&self, port: u16) -> Vec<String> {
        vec![
            "--port".into(),
            port.to_string(),
            "--profile".into(),
            self.profile.clone(),
        ]
    }
}

pub fn locate() -> Option<Helper> {
    if let Ok(path) = std::env::var("BIONICHAT_TUNNEL_BIN") {
        return Some(Helper {
            path: PathBuf::from(path),
            profile: profile(),
        });
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let file = if cfg!(windows) {
        "bionichat-tunnel.exe"
    } else {
        "bionichat-tunnel"
    };
    let candidate = dir.join(file);
    candidate.is_file().then(|| Helper {
        path: candidate,
        profile: profile(),
    })
}

fn profile() -> String {
    std::env::var("BIONICHAT_TUNNEL_PROFILE").unwrap_or_else(|_| "default".to_string())
}

/// Spread the child's stdout and stderr into whole lines.
fn pump(
    stream: impl tokio::io::AsyncRead + Send + Unpin + 'static,
    lines: tokio::sync::mpsc::Sender<String>,
) {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stream).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if lines.send(line).await.is_err() {
                // Nobody is reading any more (the URL was found). Keep
                // draining: closing the pipe kills the helper on its next
                // write, which takes the tunnel down with it.
                while let Ok(Some(_)) = reader.next_line().await {}
                break;
            }
        }
    });
}

pub struct TunnelManager {
    child: Mutex<Option<tokio::process::Child>>,
    info: Mutex<TunnelInfo>,
    /// Where failures are appended, so a headless run leaves evidence.
    log: PathBuf,
}

impl TunnelManager {
    pub fn new(log: PathBuf) -> Self {
        Self {
            child: Mutex::new(None),
            info: Mutex::new(TunnelInfo::off()),
            log,
        }
    }

    /// Append one line to the tunnel log; never fails the caller.
    fn note(&self, text: &str) {
        use std::io::Write as _;
        let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
        {
            let _ = writeln!(file, "[{stamp}] {text}");
        }
    }

    /// The helper runs in the foreground, so there is never a tunnel to adopt
    /// at launch. Kept for the API the app already calls.
    pub fn hydrate(&self) -> TunnelInfo {
        self.info()
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

    fn take_child(&self) -> Option<tokio::process::Child> {
        self.child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }

    /// Burn the hostname: delete the tunnel server-side, then stop. The next
    /// start provisions a fresh identity, so the old link is dead for good.
    pub async fn revoke(&self) -> TunnelInfo {
        self.stop().await;
        let Some(helper) = locate() else {
            return self.set(TunnelInfo::failed(
                "no tunnel helper beside the app (expected bionichat-tunnel)",
            ));
        };
        let mut command = tokio::process::Command::new(&helper.path);
        crate::hide_window(&mut command);
        command
            .arg("--revoke")
            .arg("--profile")
            .arg(&helper.profile)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = match tokio::time::timeout(Duration::from_secs(30), command.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                return self.set(TunnelInfo::failed(format!(
                    "cannot run {}: {error}",
                    helper.path.display()
                )));
            }
            Err(_) => {
                return self.set(TunnelInfo::failed(
                    "the tunnel helper did not answer within 30s while revoking",
                ));
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return self.set(TunnelInfo::failed(format!(
                "revoke failed: {}",
                stderr.trim()
            )));
        }
        self.set(TunnelInfo::off())
    }

    /// Kill the helper. The hostname survives, so starting again reuses it.
    pub async fn stop(&self) -> TunnelInfo {
        if let Some(mut child) = self.take_child() {
            let _ = child.start_kill();
            let _ = child.wait().await;
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
            kind: None,
        });

        let Some(helper) = locate() else {
            let message = "no tunnel helper beside the app (expected bionichat-tunnel)";
            self.note(message);
            return self.set(TunnelInfo::failed(message));
        };
        self.note(&format!(
            "starting {} (profile {})",
            helper.path.display(),
            helper.profile
        ));

        let mut command = tokio::process::Command::new(&helper.path);
        crate::hide_window(&mut command);
        command
            .args(helper.start_args(hub_port))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Piped stdin is the helper's lifeline: the pipe closes when this
            // app dies, and the helper exits instead of lingering as an orphan
            // that blocks the next helper from starting.
            .stdin(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!("cannot run {}: {error}", helper.path.display());
                self.note(&message);
                return self.set(TunnelInfo::failed(message));
            }
        };

        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        *self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(child);

        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);
        pump(stdout, tx.clone());
        pump(stderr, tx.clone());
        drop(tx);

        // Phase 1: the helper prints its public URL once it is provisioned.
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
                    "the tunnel helper exited without printing a public URL".to_string()
                } else {
                    format!("the tunnel helper printed no public URL; last output:\n{last}")
                }
            })
        });

        let public_url = match public_url {
            Ok(url) => {
                self.note(&format!("public URL {url}"));
                url
            }
            Err(error) => {
                self.note(&error);
                self.stop().await;
                return self.set(TunnelInfo::failed(error));
            }
        };

        self.set(TunnelInfo {
            status: "starting".into(),
            public_url: Some(public_url.clone()),
            error: None,
            kind: Some(helper.label().to_string()),
        });

        // Phase 2: poll the public endpoint until TLS + HTTP answer.
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                self.stop().await;
                return self.set(TunnelInfo::failed(format!("http client: {error}")));
            }
        };

        let target = format!("{public_url}/mcp");
        let probe = r#"{"jsonrpc":"2.0","id":0,"method":"ping"}"#;
        let mut last_error = String::from("no attempt made");
        let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match client
                .post(&target)
                .header("content-type", "application/json")
                .body(probe)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    return self.set(TunnelInfo {
                        status: "ready".into(),
                        public_url: Some(public_url),
                        error: None,
                        kind: Some(helper.label().to_string()),
                    });
                }
                Ok(response) => last_error = format!("HTTP {}", response.status()),
                Err(error) => last_error = error.to_string(),
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }

        let message = format!(
            "public URL did not answer within {}s: {last_error}",
            READY_TIMEOUT.as_secs()
        );
        self.note(&message);
        let info = TunnelInfo {
            status: "error".into(),
            public_url: Some(public_url),
            error: Some(message),
            kind: Some(helper.label().to_string()),
        };
        self.stop().await;
        self.set(info)
    }
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
    fn finds_the_url_the_helper_prints() {
        assert_eq!(
            parse_public_url("https://kfgepc72im6k.opentunnel.xyz").as_deref(),
            Some("https://kfgepc72im6k.opentunnel.xyz")
        );
        assert_eq!(
            parse_public_url("tunnel: serving 127.0.0.1:8787 as a.b.c").as_deref(),
            None
        );
    }

    #[test]
    fn trims_trailing_punctuation_and_paths() {
        assert_eq!(
            parse_public_url("see https://abc.opentunnel.xyz/mcp.").as_deref(),
            Some("https://abc.opentunnel.xyz/mcp")
        );
        assert_eq!(
            parse_public_url("(https://abc.opentunnel.xyz)").as_deref(),
            Some("https://abc.opentunnel.xyz")
        );
    }

    #[test]
    fn ignores_lines_without_a_url() {
        assert!(parse_public_url("INFO service started").is_none());
        assert!(parse_public_url("https://").is_none());
    }

    #[tokio::test]
    async fn missing_helper_names_what_is_missing() {
        // A path that cannot be run stands in for a missing helper.
        unsafe { std::env::set_var("BIONICHAT_TUNNEL_BIN", "/nonexistent/bionichat-tunnel") };
        let info = TunnelManager::new(std::env::temp_dir().join("bionichat-tunnel-test.log"))
            .start(8787)
            .await;
        unsafe { std::env::remove_var("BIONICHAT_TUNNEL_BIN") };
        assert_eq!(info.status, "error");
        let error = info.error.expect("error text");
        assert!(
            error.contains("cannot run"),
            "a helper that cannot start must say so: {error}"
        );
    }
}
