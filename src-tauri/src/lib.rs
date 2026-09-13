pub mod engine;
pub mod gpu;
pub mod host;
pub mod netmap;
pub mod ping;
pub mod ports;
pub mod procnet;
pub mod topology;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tauri::{AppHandle, Emitter, Manager};

use engine::{logs, RuntimeKind, RuntimeState};
use types::{HostPayload, LogLine, NetworkMapPayload, ProcessSortKey, RuntimeSnapshot, RuntimeStatus};

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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            let mut states_map: HashMap<RuntimeKind, Arc<RwLock<RuntimeState>>> = HashMap::new();
            for kind in [
                RuntimeKind::Docker,
                RuntimeKind::Podman,
                RuntimeKind::Kubernetes,
                RuntimeKind::Vm,
            ] {
                states_map.insert(kind, Arc::new(RwLock::new(RuntimeState::new(kind))));
            }
            let states = Arc::new(Mutex::new(states_map));
            let snapshots: Arc<RwLock<HashMap<RuntimeKind, RuntimeSnapshot>>> =
                Arc::new(RwLock::new(HashMap::new()));

            let host_cache = Arc::new(RwLock::new(HostPayload::default()));
            let netmap_cache = Arc::new(RwLock::new(NetworkMapPayload::default()));
            let ping_cache: Arc<RwLock<Option<f64>>> = Arc::new(RwLock::new(None));
            let gpu_cache: Arc<RwLock<gpu::GpuSnapshot>> =
                Arc::new(RwLock::new(gpu::GpuSnapshot::default()));
            let sort_key: Arc<RwLock<ProcessSortKey>> =
                Arc::new(RwLock::new(ProcessSortKey::Cpu));

            app.manage(AppState {
                states: states.clone(),
                snapshots: snapshots.clone(),
                latest_host: host_cache.clone(),
                latest_netmap: netmap_cache.clone(),
                log_streams: Arc::new(Mutex::new(HashMap::new())),
                sort_key: sort_key.clone(),
            });

            let host_app = app_handle.clone();
            let host_cache_task = host_cache.clone();
            let ping_task = ping_cache.clone();
            let gpu_task = gpu_cache.clone();
            let sort_task = sort_key.clone();
            tauri::async_runtime::spawn(async move {
                host::start_host_polling(host_app, host_cache_task, ping_task, gpu_task, sort_task)
                    .await;
            });

            let ping_cache_task = ping_cache.clone();
            tauri::async_runtime::spawn(async move {
                ping::start_ping_poll(ping_cache_task).await;
            });

            let gpu_cache_task = gpu_cache.clone();
            tauri::async_runtime::spawn(async move {
                gpu::start_gpu_poll(gpu_cache_task).await;
            });

            for (kind, secs) in [(RuntimeKind::Docker, 2u64), (RuntimeKind::Podman, 2u64)] {
                let app2 = app_handle.clone();
                let states2 = states.clone();
                let snapshots2 = snapshots.clone();
                tauri::async_runtime::spawn(async move {
                    topology::start_docker_like_poll(
                        app2,
                        kind,
                        states2,
                        snapshots2,
                        Duration::from_secs(secs),
                    )
                    .await;
                });
            }

            let k8s_app = app_handle.clone();
            let k8s_states = states.clone();
            let k8s_snapshots = snapshots.clone();
            tauri::async_runtime::spawn(async move {
                topology::start_k8s_poll(
                    k8s_app,
                    k8s_states,
                    k8s_snapshots,
                    Duration::from_secs(5),
                )
                .await;
            });

            let vm_app = app_handle.clone();
            let vm_states = states.clone();
            let vm_snapshots = snapshots.clone();
            tauri::async_runtime::spawn(async move {
                topology::start_vm_poll(vm_app, vm_states, vm_snapshots, Duration::from_secs(10))
                    .await;
            });

            let netmap_app = app_handle.clone();
            let netmap_snapshots = snapshots.clone();
            let netmap_cache_task = netmap_cache.clone();
            tauri::async_runtime::spawn(async move {
                netmap::start_network_map_poll(
                    netmap_app,
                    netmap_snapshots,
                    netmap_cache_task,
                    Duration::from_secs(2),
                )
                .await;
            });

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
