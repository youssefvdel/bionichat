//! web-agent shell: dashboard over an MCP registry, plus the local hub that
//! aggregates every registered server behind one StreamableHTTP endpoint.

mod hub;
mod mcp;
mod registry;
mod tunnel;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    hub::{DEFAULT_PORT, Hub},
    mcp::ToolInfo,
    registry::ServerSpec,
    tunnel::{TunnelInfo, TunnelManager},
};

#[cfg(all(feature = "cef", feature = "wry"))]
compile_error!("features `cef` and `wry` are mutually exclusive (gtk4 vs gtk3)");

#[cfg(feature = "cef")]
const RENDERER: &str = "cef";

#[cfg(not(feature = "cef"))]
const RENDERER: &str = "wry (system webview)";

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Pending,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
struct ServerRow {
    name: String,
    transport: String,
    secret_keys: Vec<String>,
    status: Status,
    error: Option<String>,
    tools: Vec<ToolInfo>,
}

impl ServerRow {
    fn pending(spec: &ServerSpec) -> Self {
        Self {
            name: spec.name.clone(),
            transport: spec.transport.label(),
            secret_keys: spec.transport.secret_keys(),
            status: Status::Pending,
            error: None,
            tools: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct HubRow {
    url: String,
    port: u16,
    sessions: usize,
    note: &'static str,
}

#[derive(Debug, Serialize)]
struct Dashboard {
    servers: Vec<ServerRow>,
    hub: HubRow,
    tunnel: TunnelInfo,
    approvals_default: &'static str,
    renderer: &'static str,
    registry_path: String,
}

struct RegistryState {
    specs: Vec<ServerSpec>,
    rows: Vec<ServerRow>,
}

struct AppState {
    registry_path: PathBuf,
    hub: Arc<Hub>,
    tunnel: TunnelManager,
    inner: Mutex<RegistryState>,
}

fn dashboard_of(state: &AppState) -> Dashboard {
    let guard = match state.inner.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    Dashboard {
        servers: guard.rows.clone(),
        hub: HubRow {
            url: state.hub.url(),
            port: state.hub.port(),
            sessions: state.hub.session_count(),
            note: "JSON-RPC over POST, no SSE",
        },
        tunnel: state.tunnel.info(),
        approvals_default: "ask",
        renderer: RENDERER,
        registry_path: state.registry_path.display().to_string(),
    }
}

/// Probe every registered server through the hub's persistent sessions.
async fn probe_all(state: &AppState) -> Vec<ServerRow> {
    let specs = {
        let guard = match state.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.specs.clone()
    };

    let mut rows = Vec::with_capacity(specs.len());
    for spec in specs {
        let mut row = ServerRow::pending(&spec);
        match state.hub.tools_for(&spec).await {
            Ok(tools) => {
                row.tools = tools;
                row.status = Status::Ready;
            }
            Err(error) => {
                row.status = Status::Error;
                row.error = Some(error);
            }
        }
        rows.push(row);
    }
    rows
}

fn store_rows(state: &AppState, rows: &[ServerRow]) {
    let mut guard = match state.inner.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.rows = rows.to_vec();
}

#[tauri::command]
fn dashboard(state: State<'_, AppState>) -> Dashboard {
    dashboard_of(&state)
}

#[tauri::command]
async fn refresh(state: State<'_, AppState>) -> Result<Vec<ServerRow>, String> {
    let rows = probe_all(&state).await;
    store_rows(&state, &rows);
    Ok(rows)
}

#[tauri::command]
async fn add_servers(json: String, state: State<'_, AppState>) -> Result<Vec<ServerRow>, String> {
    let incoming = registry::parse_anthropic(&json)?;
    {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "registry state is poisoned".to_string())?;
        for spec in &incoming {
            guard.specs.retain(|existing| existing.name != spec.name);
            guard.specs.push(spec.clone());
        }
        registry::save(&state.registry_path, &guard.specs)?;
    }
    let rows = probe_all(&state).await;
    store_rows(&state, &rows);
    Ok(rows)
}

#[tauri::command]
async fn tunnel_start(app: AppHandle, state: State<'_, AppState>) -> Result<TunnelInfo, String> {
    let info = state.tunnel.start(state.hub.port()).await;
    let _ = app.emit("tunnel-updated", info.clone());
    Ok(info)
}

#[tauri::command]
fn tunnel_stop(app: AppHandle, state: State<'_, AppState>) -> TunnelInfo {
    let info = state.tunnel.stop();
    let _ = app.emit("tunnel-updated", info.clone());
    info
}

/// Spawn the dashboard refresh in the background so the window paints first.
fn refresh_in_background(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<AppState>();
        let rows = probe_all(&state).await;
        store_rows(&state, &rows);
        let _ = handle.emit("servers-updated", rows);
    });
}

fn builder() -> tauri::Builder {
    #[cfg(feature = "cef")]
    {
        tauri::Builder::default().runtime(tauri_runtime_cef::Cef::default())
    }
    #[cfg(not(feature = "cef"))]
    {
        tauri::Builder::default().runtime(tauri_runtime_wry::Wry::default())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    builder()
        .setup(|app| {
            #[cfg(debug_assertions)]
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            let data_dir = app.path().app_data_dir()?;
            let demo_root = data_dir.join("demo-root");
            fs::create_dir_all(&demo_root)?;
            let registry_path = data_dir.join("registry.json");

            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
                .ok_or("cannot locate the binary directory")?;

            let specs = registry::load(&registry_path).unwrap_or_else(|| {
                let specs = registry::default_specs(&exe_dir, &demo_root);
                if let Err(error) = registry::save(&registry_path, &specs) {
                    log::warn!("could not seed the registry: {error}");
                }
                specs
            });

            let port = std::env::var("WEB_AGENT_HUB_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(DEFAULT_PORT);
            let hub = Hub::new(registry_path.clone(), port);

            let rows = specs.iter().map(ServerRow::pending).collect();
            app.manage(AppState {
                registry_path,
                hub: hub.clone(),
                tunnel: TunnelManager::new(),
                inner: Mutex::new(RegistryState { specs, rows }),
            });

            let hub_task = hub.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = hub_task.serve().await {
                    log::error!("hub: {error}");
                    eprintln!("hub: {error}");
                }
            });

            refresh_in_background(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            dashboard,
            refresh,
            add_servers,
            tunnel_start,
            tunnel_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running web-agent");
}
