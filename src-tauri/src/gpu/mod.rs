use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Per-PID GPU memory split by GPU class (integrated, discrete), in bytes.
pub type GpuUsage = (u64, u64);

#[derive(Debug, Clone, Default)]
pub struct GpuSnapshot {
    pub per_pid: HashMap<u32, GpuUsage>,
    pub igpu_used_bytes: u64,
    pub dgpu_used_bytes: u64,
    pub igpu_present: bool,
    pub dgpu_present: bool,
    /// True when a discrete GPU exists but its driver is not loaded/usable.
    pub dgpu_driver_unavailable: bool,
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Polls per-process GPU memory usage every 5s, split by GPU class.
pub async fn start_gpu_poll(cache: Arc<RwLock<GpuSnapshot>>) {
    loop {
        let snap = scan_gpu_full().await;
        *cache.write().await = snap;
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

pub async fn scan_gpu_full() -> GpuSnapshot {
    #[cfg(target_os = "linux")]
    {
        linux::scan().await
    }
    #[cfg(target_os = "windows")]
    {
        windows::scan().await
    }
    #[cfg(target_os = "macos")]
    {
        macos::scan().await
    }
}
