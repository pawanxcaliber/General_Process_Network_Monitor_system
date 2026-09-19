pub mod cli;
pub mod engine;
pub mod server;
pub mod gpu;
pub mod host;
pub mod netmap;
pub mod ping;
pub mod platform;
pub mod ports;
pub mod procnet;
pub mod temp;
pub mod topology;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tauri::{AppHandle, Emitter, Manager};

use engine::{logs, RuntimeKind, RuntimeState};
use types::{HostPayload, LogLine, NetworkMapPayload, ProcessSortKey, RuntimeSnapshot, RuntimeStatus};

/// Shared backend data: the same caches the GUI state exposes, used by both
/// the Tauri event loop and the CLI's TUI.
#[derive(Clone)]
pub struct Backend {
    pub states: Arc<Mutex<HashMap<RuntimeKind, Arc<RwLock<RuntimeState>>>>>,
    pub snapshots: Arc<RwLock<HashMap<RuntimeKind, RuntimeSnapshot>>>,
    pub host: Arc<RwLock<HostPayload>>,
    pub netmap: Arc<RwLock<NetworkMapPayload>>,
    pub ping: Arc<RwLock<Option<f64>>>,
    pub gpu: Arc<RwLock<gpu::GpuSnapshot>>,
    pub temp: Arc<RwLock<temp::TempSnapshot>>,
    pub sort_key: Arc<RwLock<ProcessSortKey>>,
}

/// App-wide state managed by Tauri (`AppState` in the frontend contract).
pub struct AppState {
    pub states: Arc<Mutex<HashMap<RuntimeKind, Arc<RwLock<RuntimeState>>>>>,
    pub snapshots: Arc<RwLock<HashMap<RuntimeKind, RuntimeSnapshot>>>,
    pub latest_host: Arc<RwLock<HostPayload>>,
    pub latest_netmap: Arc<RwLock<NetworkMapPayload>>,
    pub log_streams: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    pub sort_key: Arc<RwLock<ProcessSortKey>>,
}

#[tauri::command]
async fn detect_runtimes(_state: tauri::State<'_, AppState>) -> Result<Vec<RuntimeStatus>, String> {
    Ok(engine::detect::detect_all().await)
}

#[tauri::command]
async fn get_host_metrics(state: tauri::State<'_, AppState>) -> Result<HostPayload, String> {
    Ok(state.latest_host.read().await.clone())
}

#[tauri::command]
async fn set_process_sort(state: tauri::State<'_, AppState>, key: ProcessSortKey) -> Result<(), String> {
    *state.sort_key.write().await = key;
    Ok(())
}

#[tauri::command]
async fn get_runtime_snapshot(
    state: tauri::State<'_, AppState>,
    kind: RuntimeKind,
) -> Result<RuntimeSnapshot, String> {
    Ok(state
        .snapshots
        .read()
        .await
        .get(&kind)
        .cloned()
        .unwrap_or_default())
}

#[tauri::command]
async fn get_network_map(state: tauri::State<'_, AppState>) -> Result<NetworkMapPayload, String> {
    Ok(state.latest_netmap.read().await.clone())
}

#[tauri::command]
async fn start_log_stream(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    kind: RuntimeKind,
    container_id: String,
) -> Result<(), String> {
    let key = format!("{}:{}", kind.as_str(), container_id);
    {
        let mut streams = state.log_streams.lock().await;
        if let Some(handle) = streams.remove(&key) {
            handle.abort();
        }
    }

    let runtime_state = {
        let states = state.states.lock().await;
        states
            .get(&kind)
            .cloned()
            .ok_or_else(|| "unknown runtime".to_string())?
    };
    let docker_opt = {
        let g = runtime_state.read().await;
        g.docker.clone()
    };
    let docker = docker_opt.ok_or_else(|| "runtime unavailable".to_string())?;

    let app2 = app.clone();
    let id2 = container_id.clone();
    let kind_str = kind.as_str();
    let handle = tauri::async_runtime::spawn(async move {
        logs::stream_logs(app2, docker, kind_str.to_string(), id2).await;
    });

    state.log_streams.lock().await.insert(key, handle);
    Ok(())
}

#[tauri::command]
async fn stop_log_stream(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    kind: RuntimeKind,
    container_id: String,
) -> Result<(), String> {
    let key = format!("{}:{}", kind.as_str(), container_id);
    if let Some(handle) = state.log_streams.lock().await.remove(&key) {
        handle.abort();
    }
    let event = format!("container-logs:{}:{}", kind.as_str(), container_id);
    let _ = app.emit(
        &event,
        &LogLine {
            stream: "status".to_string(),
            ts: None,
            text: "__EOF__".to_string(),
        },
    );
    Ok(())
}

/// Creates the shared caches used by GUI and CLI mode.
pub fn new_backend() -> Backend {
    let mut states_map: HashMap<RuntimeKind, Arc<RwLock<RuntimeState>>> = HashMap::new();
    for kind in [
        RuntimeKind::Docker,
        RuntimeKind::Podman,
        RuntimeKind::Kubernetes,
        RuntimeKind::Vm,
    ] {
        states_map.insert(kind, Arc::new(RwLock::new(RuntimeState::new(kind))));
    }
    Backend {
        states: Arc::new(Mutex::new(states_map)),
        snapshots: Arc::new(RwLock::new(HashMap::new())),
        host: Arc::new(RwLock::new(HostPayload::default())),
        netmap: Arc::new(RwLock::new(NetworkMapPayload::default())),
        ping: Arc::new(RwLock::new(None)),
        gpu: Arc::new(RwLock::new(gpu::GpuSnapshot::default())),
        temp: Arc::new(RwLock::new(temp::TempSnapshot::default())),
        sort_key: Arc::new(RwLock::new(ProcessSortKey::Cpu)),
    }
}

/// Spawns a background task on the correct runtime: Tauri's async runtime in
/// GUI mode (its reactor context isn't active for raw `tokio::spawn` at
/// `setup()` time), a plain tokio spawn in CLI mode.
fn spawn_bg<F>(app: &Option<AppHandle>, fut: F)
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    match app {
        Some(_) => {
            let _ = tauri::async_runtime::spawn(fut);
        }
        None => {
            let _ = tokio::spawn(fut);
        }
    }
}

/// Spawns every background sampler. `app` is `Some` in GUI mode (events are
/// emitted) and `None` in CLI mode (readers poll the caches directly).
pub fn spawn_pollers(app: Option<AppHandle>, backend: &Backend) {
    let Backend {
        states,
        snapshots,
        host: host_cache,
        netmap: netmap_cache,
        ping: ping_cache,
        gpu: gpu_cache,
        temp: temp_cache,
        sort_key,
    } = backend;

    let host_app = app.clone();
    let host_cache = host_cache.clone();
    let host_topology1 = host_cache.clone();
    let ping_task = ping_cache.clone();
    let gpu_task = gpu_cache.clone();
    let sort_task = sort_key.clone();
    let temp_task = temp_cache.clone();
    let _host_topology = host_cache.clone();
    spawn_bg(&app, async move {
        host::start_host_polling(
            host_app,
            host_cache,
            ping_task,
            gpu_task,
            temp_task,
            sort_task,
        )
        .await;
    });

    let temp_cache = temp_cache.clone();
    spawn_bg(&app, async move {
        temp::start_temp_poll(temp_cache).await;
    });

    let ping_cache = ping_cache.clone();
    spawn_bg(&app, async move {
        ping::start_ping_poll(ping_cache).await;
    });

    let gpu_cache = gpu_cache.clone();
    spawn_bg(&app, async move {
        gpu::start_gpu_poll(gpu_cache).await;
    });

    for (kind, secs) in [(RuntimeKind::Docker, 2u64), (RuntimeKind::Podman, 2u64)] {
        let app2 = app.clone();
        let states2 = states.clone();
        let snapshots2 = snapshots.clone();
        let host2 = host_topology1.clone();
        spawn_bg(&app, async move {
            topology::start_docker_like_poll(
                app2,
                kind,
                states2,
                snapshots2,
                Duration::from_secs(secs),
                host2,
            )
            .await;
        });
    }

    let k8s_app = app.clone();
    let k8s_states = states.clone();
    let k8s_snapshots = snapshots.clone();
    spawn_bg(&app, async move {
        topology::start_k8s_poll(
            k8s_app,
            k8s_states,
            k8s_snapshots,
            Duration::from_secs(5),
        )
        .await;
    });

    let vm_app = app.clone();
    let vm_states = states.clone();
    let vm_snapshots = snapshots.clone();
    spawn_bg(&app, async move {
        topology::start_vm_poll(
            vm_app,
            vm_states,
            vm_snapshots,
            Duration::from_secs(10),
            host_topology1.clone(),
        )
        .await;
    });

    let netmap_app = app.clone();
    let netmap_snapshots = snapshots.clone();
    let netmap_cache = netmap_cache.clone();
    spawn_bg(&app, async move {
        netmap::start_network_map_poll(
            netmap_app,
            netmap_snapshots,
            netmap_cache,
            Duration::from_secs(2),
        )
        .await;
    });
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            let backend = new_backend();

            app.manage(AppState {
                states: backend.states.clone(),
                snapshots: backend.snapshots.clone(),
                latest_host: backend.host.clone(),
                latest_netmap: backend.netmap.clone(),
                log_streams: Arc::new(Mutex::new(HashMap::new())),
                sort_key: backend.sort_key.clone(),
            });

            spawn_pollers(Some(app_handle.clone()), &backend);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_runtimes,
            get_host_metrics,
            get_runtime_snapshot,
            get_network_map,
            set_process_sort,
            start_log_stream,
            stop_log_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// CLI mode: `monitor --cli` runs a btop-style terminal UI sharing the same
/// backend collectors as the GUI.
pub async fn run_cli() {
    let backend = new_backend();
    cli::start_app(backend).await;
}

/// Server mode: `monitor --server` serves the web UI from the same backend
/// over HTTP + WebSocket so it can be opened from any browser.
pub async fn run_server(
    bind: std::net::IpAddr,
    port: u16,
    token: Option<String>,
) {
    let backend = new_backend();
    spawn_pollers(None, &backend);
    if let Err(e) = server::run(bind, port, token, backend).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}
