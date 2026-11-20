//! web-agent shell: dashboard over an MCP server registry.
//! Pass 1 only: list servers, paste Anthropic JSON, spawn, list tools.

mod mcp;
mod registry;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    mcp::ToolInfo,
    registry::ServerSpec,
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
struct TunnelRow {
    status: &'static str,
    public_url: Option<String>,
    note: &'static str,
}

#[derive(Debug, Serialize)]
struct Dashboard {
    servers: Vec<ServerRow>,
    tunnel: TunnelRow,
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
    inner: Mutex<RegistryState>,
}

fn dashboard_of(state: &AppState) -> Dashboard {
    let guard = match state.inner.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    Dashboard {
        servers: guard.rows.clone(),
        tunnel: TunnelRow {
            status: "off",
            public_url: None,
            note: "OpenTunnel lands in P2",
        },
        approvals_default: "ask",
        renderer: RENDERER,
        registry_path: state.registry_path.display().to_string(),
    }
}

/// Probe every registered server sequentially. Demo servers answer in
/// milliseconds; each probe is capped at 20 s.
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
        match mcp::list_tools(&spec).await {
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

            let rows = specs.iter().map(ServerRow::pending).collect();
            app.manage(AppState {
                registry_path,
                inner: Mutex::new(RegistryState { specs, rows }),
            });

            refresh_in_background(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![dashboard, refresh, add_servers])
        .run(tauri::generate_context!())
        .expect("error while running web-agent");
}
