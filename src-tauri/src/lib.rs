//! bionichat shell: dashboard over an MCP registry, the local hub that
//! aggregates every registered server, approval policy, and supervision.

mod hub;
mod mcp;
mod policy;
mod registry;
mod tunnel;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    hub::{DEFAULT_PORT, Hub, PendingView},
    mcp::ToolInfo,
    registry::{Decision, Registry, ServerSpec},
    tunnel::{TunnelInfo, TunnelManager},
};

#[cfg(all(feature = "cef", feature = "wry"))]
compile_error!("features `cef` and `wry` are mutually exclusive (gtk4 vs gtk3)");

#[cfg(feature = "cef")]
const RENDERER: &str = "cef";

#[cfg(not(feature = "cef"))]
const RENDERER: &str = "wry (system webview)";

const SUPERVISE_INTERVAL: Duration = Duration::from_secs(2);
const MAX_RESTART_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Pending,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
struct ToolRow {
    name: String,
    description: Option<String>,
    read_only: bool,
    #[allow(dead_code)]
    input_schema: Value,
    policy: Decision,
}

impl ToolRow {
    fn from_info(info: ToolInfo, decision: Decision) -> Self {
        Self {
            name: info.name,
            description: info.description,
            read_only: info.read_only,
            input_schema: info.input_schema,
            policy: decision,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ServerRow {
    name: String,
    /// The prefix its tools are advertised under (`core` for the built-ins).
    namespace: String,
    transport: String,
    secret_keys: Vec<String>,
    autorestart: bool,
    status: Status,
    error: Option<String>,
    tools: Vec<ToolRow>,
}

impl ServerRow {
    fn pending(spec: &ServerSpec) -> Self {
        Self {
            name: spec.name.clone(),
            namespace: spec.namespace().to_string(),
            transport: spec.transport.label(),
            secret_keys: spec.transport.secret_keys(),
            autorestart: spec.autorestart,
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
    approvals: Vec<PendingView>,
    approvals_default: &'static str,
    renderer: &'static str,
    registry_path: String,
    version: &'static str,
}

struct AppState {
    registry_path: PathBuf,
    projects_path: PathBuf,
    hub: Arc<Hub>,
    tunnel: TunnelManager,
    rows: Mutex<Vec<ServerRow>>,
    retries: Mutex<HashMap<String, u32>>,
}

fn lock_rows(rows: &Mutex<Vec<ServerRow>>) -> std::sync::MutexGuard<'_, Vec<ServerRow>> {
    rows.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_retries(
    retries: &Mutex<HashMap<String, u32>>,
) -> std::sync::MutexGuard<'_, HashMap<String, u32>> {
    retries
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn dashboard_of(state: &AppState) -> Dashboard {
    Dashboard {
        servers: lock_rows(&state.rows).clone(),
        hub: HubRow {
            url: state.hub.url(),
            port: state.hub.port(),
            sessions: state.hub.session_count(),
            note: "JSON-RPC over POST, no SSE",
        },
        tunnel: state.tunnel.info(),
        approvals: state.hub.pending(),
        approvals_default: "everything allowed · set Ask or Deny per tool",
        version: env!("CARGO_PKG_VERSION"),
        renderer: RENDERER,
        registry_path: state.registry_path.display().to_string(),
    }
}

/// Probe every registered server through the hub's persistent sessions and
/// attach the effective policy to each tool.
async fn probe_all(state: &AppState) -> Vec<ServerRow> {
    let registry = state.hub.registry();
    let mut rows = Vec::with_capacity(registry.servers.len());
    for spec in &registry.servers {
        let mut row = ServerRow::pending(spec);
        match state.hub.tools_for(spec).await {
            Ok(tools) => {
                row.tools = tools
                    .into_iter()
                    .map(|tool| {
                        let decision = policy::decision(
                            &registry.policies,
                            &spec.name,
                            &tool.name,
                            tool.read_only,
                        );
                        ToolRow::from_info(tool, decision)
                    })
                    .collect();
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

fn store_rows(state: &AppState, rows: Vec<ServerRow>) {
    *lock_rows(&state.rows) = rows;
}

fn find_row<'a>(rows: &'a mut [ServerRow], name: &str) -> Option<&'a mut ServerRow> {
    rows.iter_mut().find(|row| row.name == name)
}

/// A folder the user chose to work in.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectRow {
    pub name: String,
    pub path: String,
}

/// One sub-folder in the picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DirRow {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BrowseView {
    pub path: String,
    pub parent: Option<String>,
    pub dirs: Vec<DirRow>,
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

fn load_projects(path: &Path) -> Vec<ProjectRow> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<ProjectRow>>(&text).unwrap_or_default()
}

fn save_projects(path: &Path, rows: &[ProjectRow]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(rows).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("cannot save projects: {e}"))
}

/// Saved projects, dropping any folder that has since been moved or deleted.
#[tauri::command]
fn projects(state: State<'_, AppState>) -> Vec<ProjectRow> {
    let mut rows = load_projects(&state.projects_path);
    rows.retain(|row| Path::new(&row.path).is_dir());
    rows
}

/// Add a working folder. The same folder twice is a no-op.
#[tauri::command]
fn add_project(path: String, state: State<'_, AppState>) -> Result<Vec<ProjectRow>, String> {
    let given = PathBuf::from(path.trim());
    let folder = given
        .canonicalize()
        .map_err(|e| format!("{}: {e}", given.display()))?;
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    let name = folder
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| folder.display().to_string());

    let mut rows = load_projects(&state.projects_path);
    let display = folder.display().to_string();
    if !rows.iter().any(|row| row.path == display) {
        rows.push(ProjectRow {
            name,
            path: display,
        });
        save_projects(&state.projects_path, &rows)?;
    }
    Ok(rows)
}

#[tauri::command]
fn remove_project(path: String, state: State<'_, AppState>) -> Vec<ProjectRow> {
    let mut rows = load_projects(&state.projects_path);
    rows.retain(|row| row.path != path);
    let _ = save_projects(&state.projects_path, &rows);
    rows
}

/// List the sub-folders of `path` so the app can offer its own folder picker
/// (no portal, no GTK dialog: it works the same under CEF and wry).
#[tauri::command]
fn browse(path: Option<String>) -> Result<BrowseView, String> {
    let folder = match path.filter(|value| !value.trim().is_empty()) {
        Some(value) => PathBuf::from(value),
        None => home_dir(),
    };
    let folder = folder
        .canonicalize()
        .map_err(|e| format!("{}: {e}", folder.display()))?;
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }

    let mut dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(&folder) {
        for entry in entries.flatten() {
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            dirs.push(DirRow {
                name,
                path: child.display().to_string(),
            });
        }
    }
    dirs.sort_by_key(|dir| dir.name.to_lowercase());

    Ok(BrowseView {
        parent: folder
            .parent()
            .map(|parent| parent.display().to_string()),
        path: folder.display().to_string(),
        dirs,
    })
}

#[tauri::command]
fn dashboard(state: State<'_, AppState>) -> Dashboard {
    dashboard_of(&state)
}

#[tauri::command]
async fn refresh(state: State<'_, AppState>) -> Result<Vec<ServerRow>, String> {
    let rows = probe_all(&state).await;
    store_rows(&state, rows.clone());
    Ok(rows)
}

#[tauri::command]
async fn add_servers(json: String, state: State<'_, AppState>) -> Result<Vec<ServerRow>, String> {
    let incoming = registry::parse_anthropic(&json)?;
    let mut registry = state.hub.registry();
    for spec in incoming {
        state.hub.release(&spec.name);
        registry.servers.retain(|existing| existing.name != spec.name);
        registry.servers.push(spec);
    }
    registry::save(&state.registry_path, &registry)?;
    let rows = probe_all(&state).await;
    store_rows(&state, rows.clone());
    Ok(rows)
}

/// Drop a server for good: its process, its tools, its stored policies.
#[tauri::command]
fn remove_server(name: String, state: State<'_, AppState>) -> Result<Vec<ServerRow>, String> {
    let mut registry = state.hub.registry();
    let before = registry.servers.len();
    registry.servers.retain(|spec| spec.name != name);
    if registry.servers.len() == before {
        return Err(format!("no registered server `{name}`"));
    }
    registry.policies.retain(|key, _| !key.starts_with(&format!("{name}__")));
    registry::save(&state.registry_path, &registry)?;
    state.hub.release(&name);
    state.hub.forget(&name);
    lock_retries(&state.retries).remove(&name);

    let mut rows = lock_rows(&state.rows);
    rows.retain(|row| row.name != name);
    let rows = rows.clone();
    store_rows(&state, rows.clone());
    Ok(rows)
}

#[tauri::command]
fn set_policy(
    server: String,
    tool: String,
    decision: Decision,
    state: State<'_, AppState>,
) -> Result<Vec<ServerRow>, String> {
    let mut registry = state.hub.registry();
    registry.set_policy(&server, &tool, decision);
    registry::save(&state.registry_path, &registry)?;
    let mut rows = lock_rows(&state.rows);
    if let Some(row) = find_row(&mut rows, &server) {
        if let Some(tool_row) = row.tools.iter_mut().find(|candidate| candidate.name == tool) {
            tool_row.policy = decision;
        }
    }
    Ok(rows.clone())
}

#[tauri::command]
fn set_autorestart(
    server: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<Vec<ServerRow>, String> {
    let mut registry = state.hub.registry();
    let Some(spec) = registry.server_mut(&server) else {
        return Err(format!("no registered server `{server}`"));
    };
    spec.autorestart = enabled;
    if enabled {
        state.hub.release(&server);
    }
    registry::save(&state.registry_path, &registry)?;
    let mut rows = lock_rows(&state.rows);
    if let Some(row) = find_row(&mut rows, &server) {
        row.autorestart = enabled;
    }
    Ok(rows.clone())
}

#[tauri::command]
fn approvals(state: State<'_, AppState>) -> Vec<PendingView> {
    state.hub.pending()
}

#[tauri::command]
fn activity(state: State<'_, AppState>) -> Vec<crate::hub::ActivityEntry> {
    state.hub.activity()
}

/// Window controls for the custom title bar (the embedded runtime does not
/// expose the window module to the frontend, so these run server-side).
#[tauri::command]
fn window_minimize(app: AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or_else(|| "no main window".to_string())?
        .minimize()
        .map_err(|e| e.to_string())
}

/// The CEF runtime implements neither `data-tauri-drag-region` nor
/// `start_dragging`, so the title bar moves the window itself: it reads where
/// the window is, then asks for a new position on every pointer move.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowSpot {
    pub x: i32,
    pub y: i32,
}

#[tauri::command]
fn window_spot(app: AppHandle) -> Result<WindowSpot, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "no main window".to_string())?;
    let position = window.outer_position().map_err(|e| e.to_string())?;
    Ok(WindowSpot {
        x: position.x,
        y: position.y,
    })
}

#[tauri::command]
fn window_move(x: i32, y: i32, app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "no main window".to_string())?;
    window
        .set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn window_toggle_maximize(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "no main window".to_string())?;
    let maximized = window.is_maximized().map_err(|e| e.to_string())?;
    if maximized {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn window_close(app: AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or_else(|| "no main window".to_string())?
        .close()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn resolve_approval(id: u64, approve: bool, state: State<'_, AppState>) -> bool {
    state.hub.resolve(id, approve)
}

#[tauri::command]
fn resolve_session(id: u64, state: State<'_, AppState>) -> bool {
    state.hub.resolve_session(id)
}

#[tauri::command]
async fn restart_server(
    app: AppHandle,
    server: String,
    state: State<'_, AppState>,
) -> Result<Vec<ServerRow>, String> {
    state.hub.release(&server);
    state.hub.forget(&server);
    lock_retries(&state.retries).remove(&server);

    let registry = state.hub.registry();
    let spec = registry
        .server(&server)
        .cloned()
        .ok_or_else(|| format!("no registered server `{server}`"))?;

    let row = match state.hub.tools_for(&spec).await {
        Ok(tools) => ServerRow {
            tools: tools
                .into_iter()
                .map(|tool| {
                    let decision = policy::decision(
                        &registry.policies,
                        &server,
                        &tool.name,
                        tool.read_only,
                    );
                    ToolRow::from_info(tool, decision)
                })
                .collect(),
            status: Status::Ready,
            ..ServerRow::pending(&spec)
        },
        Err(error) => ServerRow {
            status: Status::Error,
            error: Some(error),
            ..ServerRow::pending(&spec)
        },
    };

    let rows = {
        let mut rows = lock_rows(&state.rows);
        match find_row(&mut rows, &server) {
            Some(existing) => *existing = row,
            None => rows.push(row),
        }
        rows.clone()
    };
    let _ = app.emit("servers-updated", rows.clone());
    Ok(rows)
}

#[tauri::command]
async fn tunnel_start(app: AppHandle, state: State<'_, AppState>) -> Result<TunnelInfo, String> {
    let info = state.tunnel.start(state.hub.port()).await;
    let _ = app.emit("tunnel-updated", info.clone());
    Ok(info)
}

#[tauri::command]
async fn tunnel_stop(app: AppHandle, state: State<'_, AppState>) -> Result<TunnelInfo, String> {
    let info = state.tunnel.stop().await;
    let _ = app.emit("tunnel-updated", info.clone());
    Ok(info)
}

#[tauri::command]
async fn tunnel_revoke(app: AppHandle, state: State<'_, AppState>) -> Result<TunnelInfo, String> {
    let info = state.tunnel.revoke().await;
    let _ = app.emit("tunnel-updated", info.clone());
    Ok(info)
}

/// Spawn the dashboard refresh in the background so the window paints first.
fn refresh_in_background(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<AppState>();
        let rows = probe_all(&state).await;
        store_rows(&state, rows.clone());
        let _ = handle.emit("servers-updated", rows);
    });
}

/// Detect externally killed servers within one interval, then restart them
/// when the server asked for it (1 s, 2 s, 4 s backoff, three attempts).
fn spawn_supervisor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(SUPERVISE_INTERVAL).await;
            let state = app.state::<AppState>();
            let registry = state.hub.registry();

            for spec in &registry.servers {
                if state.hub.session_closed(&spec.name).await != Some(true) {
                    continue;
                }
                state.hub.forget(&spec.name);

                let attempts = lock_retries(&state.retries)
                    .get(&spec.name)
                    .copied()
                    .unwrap_or(0);

                if !spec.autorestart || attempts >= MAX_RESTART_ATTEMPTS {
                    if !spec.autorestart {
                        state.hub.hold(&spec.name);
                    }
                    let message = if spec.autorestart {
                        format!("exited; gave up after {MAX_RESTART_ATTEMPTS} restart attempts")
                    } else {
                        "exited or was killed (restart button, or enable auto-restart)".to_string()
                    };
                    {
                        let mut rows = lock_rows(&state.rows);
                        if let Some(row) = find_row(&mut rows, &spec.name) {
                            row.status = Status::Error;
                            row.error = Some(message.clone());
                            row.tools.clear();
                        }
                    }
                    let _ = app.emit("servers-updated", lock_rows(&state.rows).clone());
                    continue;
                }

                let next_attempt = attempts + 1;
                lock_retries(&state.retries).insert(spec.name.clone(), next_attempt);

                {
                    let mut rows = lock_rows(&state.rows);
                    if let Some(row) = find_row(&mut rows, &spec.name) {
                        row.status = Status::Error;
                        row.error = Some(format!(
                            "exited; restarting ({next_attempt}/{MAX_RESTART_ATTEMPTS})"
                        ));
                    }
                }
                let _ = app.emit("servers-updated", lock_rows(&state.rows).clone());

                tokio::time::sleep(Duration::from_secs(1u64 << attempts)).await;

                match state.hub.tools_for(spec).await {
                    Ok(tools) => {
                        lock_retries(&state.retries).remove(&spec.name);
                        let mut rows = lock_rows(&state.rows);
                        if let Some(row) = find_row(&mut rows, &spec.name) {
                            row.tools = tools
                                .into_iter()
                                .map(|tool| {
                                    let decision = policy::decision(
                                        &registry.policies,
                                        &spec.name,
                                        &tool.name,
                                        tool.read_only,
                                    );
                                    ToolRow::from_info(tool, decision)
                                })
                                .collect();
                            row.status = Status::Ready;
                            row.error = None;
                        }
                    }
                    Err(error) => {
                        let mut rows = lock_rows(&state.rows);
                        if let Some(row) = find_row(&mut rows, &spec.name) {
                            row.status = Status::Error;
                            row.error = Some(format!("restart failed: {error}"));
                        }
                    }
                }
                let _ = app.emit("servers-updated", lock_rows(&state.rows).clone());
            }
        }
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
            let file_root = data_dir.join("fs-root");
            fs::create_dir_all(&file_root)?;
            let skills_dir = data_dir.join(".agents");
            fs::create_dir_all(&skills_dir)?;
            let registry_path = data_dir.join("registry.json");

            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
                .ok_or("cannot locate the binary directory")?;

            // A registry that exists but will not parse is left untouched:
            // reseeding over it would delete the user's own servers.
            let registry_file_exists = registry_path.exists();
            let mut registry = match registry::load(&registry_path) {
                Some(registry) => registry,
                None => Registry {
                    servers: Vec::new(),
                    ..Default::default()
                },
            };
            let registry_broken = registry_file_exists && registry.servers.is_empty();
            // New built-in tools join an existing registry; user servers win.
            // A built-in whose binary is missing is repointed at this build:
            // moving or renaming the project folder must not break the app.
            let mut added = false;
            for spec in registry::default_specs(&exe_dir, &file_root, &skills_dir) {
                match registry.servers.iter_mut().find(|s| s.name == spec.name) {
                    Some(existing) => {
                        // Older registries predate namespaces: built-ins now
                        // share the `core` prefix.
                        if existing.namespace.is_none() {
                            existing.namespace = spec.namespace.clone();
                            added = true;
                        }
                        if let (
                            registry::Transport::Stdio { command, .. },
                            registry::Transport::Stdio {
                                command: fresh,
                                args,
                                ..
                            },
                        ) = (&existing.transport, &spec.transport)
                        {
                            if !Path::new(command).exists() {
                                log::info!("repointing {} at {fresh}", spec.name);
                                existing.transport = registry::Transport::Stdio {
                                    command: fresh.clone(),
                                    args: args.clone(),
                                    env: Default::default(),
                                };
                                added = true;
                            }
                        }
                    }
                    None => {
                        registry.servers.push(spec);
                        added = true;
                    }
                }
            }
            if !registry_broken && (registry.servers.is_empty() || added) {
                if let Err(error) = registry::save(&registry_path, &registry) {
                    log::warn!("could not seed the registry: {error}");
                }
            }

            let port = std::env::var("BIONICHAT_HUB_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(DEFAULT_PORT);
            let hub = Hub::new(registry_path.clone(), port, Some(app.handle().clone()));

            let tunnel = TunnelManager::new();
            tunnel.hydrate();

            let rows = registry
                .servers
                .iter()
                .map(ServerRow::pending)
                .collect::<Vec<_>>();
            app.manage(AppState {
                registry_path,
                hub: hub.clone(),
                tunnel,
                projects_path: data_dir.join("projects.json"),
                rows: Mutex::new(rows),
                retries: Mutex::new(HashMap::new()),
            });

            let hub_task = hub.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = hub_task.serve().await {
                    log::error!("hub: {error}");
                    eprintln!("hub: {error}");
                }
            });

            refresh_in_background(app.handle());
            spawn_supervisor(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            dashboard,
            projects,
            add_project,
            remove_project,
            browse,
            refresh,
            add_servers,
            set_policy,
            set_autorestart,
            approvals,
            activity,
            resolve_approval,
            resolve_session,
            restart_server,
            remove_server,
            tunnel_start,
            tunnel_stop,
            tunnel_revoke,
            window_minimize,
            window_spot,
            window_move,
            window_toggle_maximize,
            window_close
        ])
        .run(tauri::generate_context!())
        .expect("error while running bionichat");
}
