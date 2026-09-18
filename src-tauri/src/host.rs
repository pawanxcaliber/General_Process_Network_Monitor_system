use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::types::{DiskInfo, HostHistory, HostPayload, InterfaceInfo, ProcessInfo, ProcessSortKey};

const HISTORY_CAP: usize = 60;
const MAX_PROCESSES: usize = 300;

fn push_capped(v: &mut Vec<f64>, x: f64) {
    if v.len() >= HISTORY_CAP {
        v.remove(0);
    }
    v.push(x);
}

pub async fn start_host_polling(
    app: Option<AppHandle>,
    cache: Arc<RwLock<HostPayload>>,
    ping_cache: Arc<RwLock<Option<f64>>>,
    gpu_cache: Arc<RwLock<crate::gpu::GpuSnapshot>>,
    temp_cache: Arc<RwLock<crate::temp::TempSnapshot>>,
    sort_key: Arc<RwLock<ProcessSortKey>>,
) {
    let mut sys = System::new_all();
    let mut disks = Disks::new_with_refreshed_list();
    let mut networks = Networks::new_with_refreshed_list();
    sys.refresh_all();
    let mut procnet = crate::procnet::ProcNetState::new();

    let mut prev_disk: HashMap<String, (u64, u64)> = HashMap::new();
    for d in disks.list() {
        let usage = d.usage();
        prev_disk.insert(
            d.mount_point().display().to_string(),
            (usage.read_bytes, usage.written_bytes),
        );
    }

    let mut prev_net: HashMap<String, (u64, u64)> = HashMap::new();
    for (name, data) in networks.iter() {
        prev_net.insert(name.clone(), (data.received(), data.transmitted()));
    }

    let mut prev_t = Instant::now();
    let mut history = HostHistory {
        cpu: vec![0.0],
        ram_percent: vec![0.0],
        net_rx_bps: vec![0.0],
        net_tx_bps: vec![0.0],
        disk_read_bps: vec![0.0],
        disk_write_bps: vec![0.0],
        ping_ms: vec![],
        cpu_temp_c: vec![],
        gpu_igpu_used_bytes: vec![],
        gpu_dgpu_used_bytes: vec![],
    };

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        disks.refresh(true);
        networks.refresh(true);

        let cpu_percent = sys.global_cpu_usage();
        let cpu_per_core: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let ram_total = sys.total_memory();
        let ram_used = sys.used_memory();
        let swap_total = sys.total_swap();
        let swap_used = sys.used_swap();
        let ram_percent = if ram_total > 0 {
            ram_used as f64 / ram_total as f64 * 100.0
        } else {
            0.0
        };

        let elapsed = prev_t.elapsed().as_secs_f64().max(0.001);

        let mut disk_infos: Vec<DiskInfo> = Vec::new();
        let mut seen_devices: HashSet<String> = HashSet::new();
        let mut total_read_bps = 0.0f64;
        let mut total_write_bps = 0.0f64;

        for d in disks.list() {
            let total = d.total_space();
            if total == 0 {
                continue;
            }
            let dev_key = {
                let name = d.name().to_string_lossy().to_string();
                if name.is_empty() {
                    d.mount_point().display().to_string()
                } else {
                    name
                }
            };
            if !seen_devices.insert(dev_key) {
                continue;
            }
            let key = d.mount_point().display().to_string();
            let usage = d.usage();
            let (prev_r, prev_w) = prev_disk.get(&key).copied().unwrap_or((0, 0));
            let read_bps = usage.read_bytes.saturating_sub(prev_r) as f64 / elapsed;
            let write_bps = usage.written_bytes.saturating_sub(prev_w) as f64 / elapsed;
            prev_disk.insert(key, (usage.read_bytes, usage.written_bytes));
            total_read_bps += read_bps;
            total_write_bps += write_bps;

            let available = d.available_space();
            disk_infos.push(DiskInfo {
                name: d
                    .mount_point()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string()),
                read_bps,
                write_bps,
                total_bytes: total,
                used_bytes: total.saturating_sub(available),
            });
        }

        let mut iface_infos: Vec<InterfaceInfo> = Vec::new();
        let mut total_rx_bps = 0.0f64;
        let mut total_tx_bps = 0.0f64;

        for (name, data) in networks.iter() {
            let (prev_rx, prev_tx) = prev_net.get(name).copied().unwrap_or((0, 0));
            let rx = data.received();
            let tx = data.transmitted();
            let rx_bps = rx.saturating_sub(prev_rx) as f64 / elapsed;
            let tx_bps = tx.saturating_sub(prev_tx) as f64 / elapsed;
            prev_net.insert(name.clone(), (rx, tx));
            total_rx_bps += rx_bps;
            total_tx_bps += tx_bps;
            iface_infos.push(InterfaceInfo {
                name: name.clone(),
                rx_bps,
                tx_bps,
                rx_total: rx,
                tx_total: tx,
            });
        }

        prev_t = Instant::now();

        let proc_net = crate::procnet::sample(&mut procnet, elapsed).await;
        let gpu_snap = gpu_cache.read().await.clone();
        let ping_ms = *ping_cache.read().await;
        let temp_snap = temp_cache.read().await.clone();
        let sort = *sort_key.read().await;

        // gnome-system-monitor style scale: % of total machine capacity
        // (100% = all cores), not % of a single core.
        let ncpu = sys.cpus().len().max(1) as f32;

        let mut processes: Vec<ProcessInfo> = sys
            .processes()
            .iter()
            .map(|(pid, p)| {
                let du = p.disk_usage();
                let gpu = gpu_snap.per_pid.get(&pid.as_u32()).copied();
                ProcessInfo {
                    pid: pid.as_u32(),
                    name: p.name().to_string_lossy().to_string(),
                    cpu_percent: p.cpu_usage() / ncpu,
                    memory_bytes: p.memory(),
                    disk_read_bps: du.read_bytes as f64 / elapsed,
                    disk_write_bps: du.written_bytes as f64 / elapsed,
                    net_rx_bps: proc_net.rx_bps.get(&pid.as_u32()).copied().unwrap_or(0.0),
                    net_tx_bps: proc_net.tx_bps.get(&pid.as_u32()).copied().unwrap_or(0.0),
                    gpu_igpu_bytes: gpu.map(|g| g.0),
                    gpu_dgpu_bytes: gpu.map(|g| g.1),
                }
            })
            .collect();
        sort_processes(&mut processes, sort);
        processes.truncate(MAX_PROCESSES);

        push_capped(&mut history.cpu, cpu_percent as f64);
        push_capped(&mut history.ram_percent, ram_percent);
        push_capped(&mut history.net_rx_bps, total_rx_bps);
        push_capped(&mut history.net_tx_bps, total_tx_bps);
        push_capped(&mut history.disk_read_bps, total_read_bps);
        push_capped(&mut history.disk_write_bps, total_write_bps);
        if let Some(ms) = ping_ms {
            push_capped(&mut history.ping_ms, ms);
        }
        if let Some(t) = temp_snap.cpu_c {
            push_capped(&mut history.cpu_temp_c, t);
        }
        push_capped(
            &mut history.gpu_igpu_used_bytes,
            gpu_snap.igpu_used_bytes as f64,
        );
        push_capped(
            &mut history.gpu_dgpu_used_bytes,
            gpu_snap.dgpu_used_bytes as f64,
        );

        let payload = HostPayload {
            timestamp: chrono::Utc::now().timestamp_millis() as f64,
            cpu_percent,
            cpu_per_core,
            ram_used_bytes: ram_used,
            ram_total_bytes: ram_total,
            swap_used_bytes: swap_used,
            swap_total_bytes: swap_total,
            disks: disk_infos,
            interfaces: iface_infos,
            ports: crate::ports::listening_ports(),
            processes,
            ping_ms,
            cpu_temp_c: temp_snap.cpu_c,
            gpu_igpu_used_bytes: gpu_snap.igpu_used_bytes,
            gpu_dgpu_used_bytes: gpu_snap.dgpu_used_bytes,
            gpu_igpu_present: gpu_snap.igpu_present,
            gpu_dgpu_present: gpu_snap.dgpu_present,
            gpu_dgpu_driver_unavailable: gpu_snap.dgpu_driver_unavailable,
            history: history.clone(),
        };

        *cache.write().await = payload.clone();
        if let Some(app) = &app {
            let _ = app.emit("host-metrics-tick", &payload);
        }
    }
}

fn sort_processes(processes: &mut [ProcessInfo], key: ProcessSortKey) {
    match key {
        ProcessSortKey::Cpu => processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        ProcessSortKey::Mem => {
            processes.sort_by_key(|p| std::cmp::Reverse(p.memory_bytes))
        }
        ProcessSortKey::Disk => processes.sort_by(|a, b| {
            let ka = a.disk_read_bps + a.disk_write_bps;
            let kb = b.disk_read_bps + b.disk_write_bps;
            kb.partial_cmp(&ka).unwrap_or(std::cmp::Ordering::Equal)
        }),
        ProcessSortKey::Net => processes.sort_by(|a, b| {
            let ka = a.net_rx_bps + a.net_tx_bps;
            let kb = b.net_rx_bps + b.net_tx_bps;
            kb.partial_cmp(&ka).unwrap_or(std::cmp::Ordering::Equal)
        }),
        ProcessSortKey::Gpu => processes.sort_by(|a, b| {
            let ka = a.gpu_igpu_bytes.unwrap_or(0) + a.gpu_dgpu_bytes.unwrap_or(0);
            let kb = b.gpu_igpu_bytes.unwrap_or(0) + b.gpu_dgpu_bytes.unwrap_or(0);
            kb.cmp(&ka)
        }),
        ProcessSortKey::Igpu => processes.sort_by(|a, b| {
            b.gpu_igpu_bytes
                .unwrap_or(0)
                .cmp(&a.gpu_igpu_bytes.unwrap_or(0))
        }),
        ProcessSortKey::Dgpu => processes.sort_by(|a, b| {
            b.gpu_dgpu_bytes
                .unwrap_or(0)
                .cmp(&a.gpu_dgpu_bytes.unwrap_or(0))
        }),
        ProcessSortKey::Pid => processes.sort_by_key(|p| p.pid),
        ProcessSortKey::Name => processes.sort_by(|a, b| a.name.cmp(&b.name)),
    }
}
