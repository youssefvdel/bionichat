//! OpenTunnel helper: spawn `opentunnel route add <hub port>`, scrape the
//! public URL it prints, then poll the URL until it answers over TLS.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use serde::Serialize;
use std::process::Stdio;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
};

const START_TIMEOUT: Duration = Duration::from_secs(45);
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
pub struct TunnelInfo {
    pub status: String,
    pub public_url: Option<String>,
    pub error: Option<String>,
    /// `opentunnel` (stable URL) or `cloudflared` (bundled, fresh URL per run).
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

    /// Adopt a tunnel that `opentunnel` already has running (previous session,
    /// login service). Without this the app claims `off` while the URL is live.
    pub fn hydrate(&self) -> TunnelInfo {
        let Some(Flavour::OpenTunnel(binary)) = locate() else {
            return self.info();
        };
        let Ok(output) = cli(&binary).arg("status").output() else {
            return self.info();
        };
        if !output.status.success() {
            return self.info();
        }
        let text = String::from_utf8_lossy(&output.stdout);
        match parse_status(&text) {
            Some(info) => self.set(info),
            None => self.info(),
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

    /// Burn the route and take the service down. The public URL dies for
    /// good: the next start mints a fresh random route (new URL). Use this
    /// when a link leaks — plain stop keeps the same URL for next time.
    pub async fn revoke(&self) -> TunnelInfo {
        if let (Some(Flavour::OpenTunnel(binary)), Some(name)) = (
            locate(),
            self
                .info()
                .public_url
                .as_ref()
                .and_then(|url| route_name(url)),
        ) {
            let mut remove_command = cli_async(&binary);
            remove_command.args(["route", "remove", name.as_str()]);
            let remove = remove_command.output();
            match tokio::time::timeout(Duration::from_secs(10), remove).await {
                Ok(Ok(output)) if output.status.success() => {}
                Ok(Ok(output)) => eprintln!(
                    "tunnel: opentunnel route remove failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
                Ok(Err(error)) => {
                    eprintln!("tunnel: cannot run opentunnel route remove: {error}")
                }
                Err(_) => eprintln!("tunnel: opentunnel route remove timed out"),
            }
        }
        self.stop().await
    }

    /// Kill the launcher child and take the background service down. The
    /// routes stay configured, so starting again reuses the same public URL.
    pub async fn stop(&self) -> TunnelInfo {
        if let Some(mut child) = self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = child.start_kill();
        }

        if let Some(Flavour::OpenTunnel(binary)) = locate() {
            let down = cli_async(&binary).arg("down").output();
            match tokio::time::timeout(Duration::from_secs(10), down).await {
                Ok(Ok(output)) if output.status.success() => {}
                Ok(Ok(output)) => eprintln!(
                    "tunnel: opentunnel down failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
                Ok(Err(error)) => eprintln!("tunnel: cannot run opentunnel down: {error}"),
                Err(_) => eprintln!("tunnel: opentunnel down timed out"),
            }
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

        let Some(flavour) = locate() else {
            return self.set(TunnelInfo::failed(
                "no tunnel helper: ship cloudflared next to the app, install opentunnel, or set OPENTUNNEL_BIN",
            ));
        };

        let mut command = cli_async(flavour.binary());
        command
            .args(flavour.start_args(hub_port))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return self.set(TunnelInfo::failed(format!(
                    "cannot run {}: {error}",
                    flavour.binary().display()
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
                        // Nobody is reading any more (the URL was found). Keep
                        // draining: closing the pipe kills the tunnel helper
                        // on its next write.
                        while let Ok(Some(_)) = lines.next_line().await {}
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
                if let Some(url) = parse_public_url(&line).filter(|url| flavour.claims(url)) {
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
                    format!("{} exited without printing a public URL", flavour.label())
                } else {
                    format!("{} printed no public URL; last output:\n{last}", flavour.label())
                }
            })
        });

        let public_url = match public_url {
            Ok(url) => url,
            Err(error) => {
                self.stop().await;
                return self.set(TunnelInfo::failed(error));
            }
        };

        self.set(TunnelInfo {
            status: "starting".into(),
            public_url: Some(public_url.clone()),
            error: None,
            kind: Some(flavour.label().to_string()),
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
                        kind: Some(flavour.label().to_string()),
                    });
                }
                Ok(response) => last_error = format!("HTTP {}", response.status()),
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
            kind: Some(flavour.label().to_string()),
        };
        self.stop().await;
        self.set(info)
    }
}

/// Read `opentunnel status` output into a ready/off state.
fn parse_status(text: &str) -> Option<TunnelInfo> {
    if !text
        .lines()
        .any(|line| line.trim_start().starts_with("Service: connected"))
    {
        return None;
    }
    Some(TunnelInfo {
        status: "ready".into(),
        public_url: text.lines().filter_map(route_url).next(),
        error: None,
        kind: Some("opentunnel".into()),
    })
}

/// `e9f7….opentunnel.xyz  →  127.0.0.1:8787` becomes the public URL.
/// The route name `route remove` wants, from a public URL. The public host
/// is `<route>.<tunnel-id>.opentunnel.xyz`; the name is the first label:
/// `xxx.…​.opentunnel.xyz[/...]` -> `xxx`.
pub fn route_name(url: &str) -> Option<String> {
    let host = url.trim().trim_start_matches("https://").split('/').next()?;
    if !host.ends_with(".opentunnel.xyz") {
        return None;
    }
    let first = host.split('.').next()?;
    if first.is_empty() || first.contains(' ') {
        return None;
    }
    Some(first.to_string())
}

fn route_url(line: &str) -> Option<String> {
    let (left, _) = line.split_once('→')?;
    let host = left.trim();
    if host.is_empty() || !host.contains('.') || host.contains(' ') {
        return None;
    }
    Some(format!("https://{host}"))
}

/// Build the command that runs the tunnel CLI. On Windows an npm install
/// leaves `opentunnel.cmd`, which CreateProcess cannot run directly, so a
/// script goes through `cmd /C`.
fn cli(binary: &Path) -> std::process::Command {
    #[cfg(windows)]
    {
        let script = binary
            .extension()
            .map(|ext| {
                let ext = ext.to_string_lossy().to_ascii_lowercase();
                ext == "cmd" || ext == "bat"
            })
            .unwrap_or(false);
        if script {
            let mut command = std::process::Command::new("cmd");
            command.arg("/C").arg(binary);
            return command;
        }
    }
    std::process::Command::new(binary)
}

/// Same rule as `cli`, for the async spawn used while rewiring a live route.
fn cli_async(binary: &Path) -> tokio::process::Command {
    #[cfg(windows)]
    {
        let script = binary
            .extension()
            .map(|ext| {
                let ext = ext.to_string_lossy().to_ascii_lowercase();
                ext == "cmd" || ext == "bat"
            })
            .unwrap_or(false);
        if script {
            let mut command = tokio::process::Command::new("cmd");
            command.arg("/C").arg(binary);
            return command;
        }
    }
    tokio::process::Command::new(binary)
}

/// The tunnel helper in play. OpenTunnel keeps one stable URL per machine;
/// cloudflared ships with the app and mints a fresh URL on every run.
#[derive(Clone, Debug)]
pub enum Flavour {
    OpenTunnel(PathBuf),
    Cloudflared(PathBuf),
}

impl Flavour {
    pub fn label(&self) -> &'static str {
        match self {
            Flavour::OpenTunnel(_) => "opentunnel",
            Flavour::Cloudflared(_) => "cloudflared",
        }
    }

    fn binary(&self) -> &Path {
        match self {
            Flavour::OpenTunnel(path) | Flavour::Cloudflared(path) => path,
        }
    }

    /// Does a URL printed by this helper plausibly belong to it? cloudflared
    /// prints the Cloudflare terms link before the tunnel URL, so the first
    /// https:// token is the wrong one.
    fn claims(&self, url: &str) -> bool {
        match self {
            Flavour::OpenTunnel(_) => true,
            Flavour::Cloudflared(_) => url
                .split_once("://")
                .map(|(_, rest)| {
                    let host = rest.split('/').next().unwrap_or(rest);
                    host.ends_with(".trycloudflare.com")
                })
                .unwrap_or(false),
        }
    }

    /// The command that starts a tunnel to `port`, plus its arguments.
    fn start_args(&self, port: u16) -> Vec<String> {
        match self {
            Flavour::OpenTunnel(_) => vec!["route".into(), "add".into(), port.to_string()],
            Flavour::Cloudflared(_) => vec![
                "tunnel".into(),
                "--url".into(),
                format!("http://127.0.0.1:{port}"),
                "--no-autoupdate".into(),
            ],
        }
    }
}

/// A tunnel helper shipped beside the app binary, so a fresh install needs
/// nothing from the machine.
fn bundled(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let candidate = dir.join(file);
    candidate.is_file().then_some(candidate)
}

pub fn locate() -> Option<Flavour> {
    // BIONICHAT_TUNNEL=cloudflared skips a system opentunnel; handy for testing
    // the bundled path on a machine that has both.
    let forced = std::env::var("BIONICHAT_TUNNEL").unwrap_or_default();
    let want_cloudflared = forced.eq_ignore_ascii_case("cloudflared");

    if !want_cloudflared {
        if let Ok(path) = std::env::var("OPENTUNNEL_BIN") {
            return Some(Flavour::OpenTunnel(PathBuf::from(path)));
        }
        if let Ok(path) = which::which("opentunnel") {
            return Some(Flavour::OpenTunnel(path));
        }
    }
    if let Some(path) = bundled("cloudflared") {
        return Some(Flavour::Cloudflared(path));
    }
    which::which("cloudflared")
        .ok()
        .map(Flavour::Cloudflared)
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

    /// Tests that mutate process-wide env (PATH, OPENTUNNEL_BIN) must not race.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn cloudflared_banner_url_wins_over_its_terms_link() {
        let flavour = Flavour::Cloudflared(PathBuf::from("/usr/bin/cloudflared"));
        let terms = "INF |  https://www.cloudflare.com/website-terms  |";
        let banner = "INF |  https://theatre-near-compatible-mailto.trycloudflare.com  |";
        let picked = parse_public_url(terms)
            .filter(|url| flavour.claims(url))
            .or_else(|| parse_public_url(banner).filter(|url| flavour.claims(url)));
        assert_eq!(
            picked.as_deref(),
            Some("https://theatre-near-compatible-mailto.trycloudflare.com")
        );
    }

    #[test]
    fn finds_url_in_route_output() {
        assert_eq!(
            parse_public_url("route ready: https://demo-1234.opentunnel.dev/mcp"),
            Some("https://demo-1234.opentunnel.dev/mcp".to_string())
        );
    }

    #[test]
    fn route_name_strips_host_and_path() {
        assert_eq!(
            route_name("https://e9f7421df827e461.kfgepc72im6k.opentunnel.xyz/mcp"),
            Some("e9f7421df827e461".to_string())
        );
        assert_eq!(route_name("https://demo-1234.opentunnel.xyz"), Some("demo-1234".to_string()));
        assert_eq!(route_name("not a url"), None);
        assert_eq!(route_name("https://example.com/x"), None);
    }

    #[test]
    fn trims_trailing_punctuation() {
        assert_eq!(
            parse_public_url("open https://abc.tunnel.dev."),
            Some("https://abc.tunnel.dev".to_string())
        );
    }

    #[test]
    fn reads_a_running_service_from_status() {
        let status = "Profile: default\nTunnel ID: abc\nHostname: abc.opentunnel.xyz\n\nabc123.abc.opentunnel.xyz  →  127.0.0.1:8787\n\nService: connected, 0 active connection(s) (pid 42, profile default)\n";
        let info = parse_status(status).expect("running service");
        assert_eq!(info.status, "ready");
        assert_eq!(
            info.public_url.as_deref(),
            Some("https://abc123.abc.opentunnel.xyz")
        );
    }

    #[test]
    fn stopped_service_is_not_adopted() {
        let status = "Profile: default\n\nNo routes configured.\n\nService: stopped (connect with: opentunnel up)\n";
        assert!(parse_status(status).is_none());
    }

    #[test]
    fn ignores_lines_without_a_url() {
        assert_eq!(parse_public_url("starting transport…"), None);
        assert_eq!(parse_public_url("https://"), None);
    }

    #[tokio::test]
    async fn missing_binary_is_reported_verbatim() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Never let a test reach the real `opentunnel`: `stop()` also runs
        // `opentunnel down`, which would take a live tunnel off the air.
        unsafe { std::env::set_var("OPENTUNNEL_BIN", "/nonexistent/opentunnel") };
        let manager = TunnelManager::new();
        let info = manager.stop().await;
        unsafe { std::env::remove_var("OPENTUNNEL_BIN") };
        assert_eq!(info.status, "off");
        assert!(info.public_url.is_none());
    }

    #[tokio::test]
    async fn missing_binary_message_names_the_fix() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Empty PATH: `which("opentunnel")` must fail and the UI must say so.
        let original = std::env::var_os("PATH");
        let empty = std::env::temp_dir().join("bionichat-empty-path");
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
        assert!(
            error.contains("no tunnel helper"),
            "a missing helper must say so: {error}"
        );
        assert!(
            error.contains("OPENTUNNEL_BIN"),
            "the message must name the escape hatch: {error}"
        );
    }

    #[tokio::test]
    async fn unreachable_binary_surfaces_stderr_text() {
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
