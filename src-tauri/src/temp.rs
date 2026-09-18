use std::sync::Arc;
use std::time::Duration;
use sysinfo::Components;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct TempSnapshot {
    pub cpu_c: Option<f64>,
    pub gpu_c: Option<f64>,
}

/// Polls system temperatures every 2s via `sysinfo`'s cross-platform
/// components (hwmon on Linux, WMI on Windows, SMC/IOKit on macOS).
pub async fn start_temp_poll(cache: Arc<RwLock<TempSnapshot>>) {
    loop {
        let snap = tokio::task::spawn_blocking(scan_temps)
            .await
            .unwrap_or_default();
        *cache.write().await = snap;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

pub fn scan_temps() -> TempSnapshot {
    let components = Components::new_with_refreshed_list();

    let mut cpu: Option<(usize, f64)> = None;
    let mut gpu: Option<f64> = None;
    let mut hottest = f64::MIN;

    for c in &components {
        let Some(t) = c
            .temperature()
            .map(|t| t as f64)
            .filter(|t| t.is_finite() && *t > 0.0 && *t < 200.0)
        else {
            continue;
        };
        hottest = hottest.max(t);

        let label = c.label().to_lowercase();
        if let Some(rank) = cpu_rank(&label) {
            if cpu.is_none_or(|(r, _)| rank < r) {
                cpu = Some((rank, t));
            }
        }
        if is_gpu(&label) {
            gpu = Some(gpu.map_or(t, |g| g.max(t)));
        }
    }

    TempSnapshot {
        cpu_c: cpu
            .map(|(_, t)| t)
            .or_else(|| (hottest > f64::MIN).then_some(hottest)),
        gpu_c: gpu,
    }
}

/// Ranks CPU sensor labels; lower is a better match.
fn cpu_rank(label: &str) -> Option<usize> {
    const KEYS: &[&str] = &[
        "k10temp",
        "zenpower",
        "coretemp",
        "tctl",
        "tdie",
        "package id",
        "cpu package",
        "cpu",
        "soc",
        "computer",
    ];
    KEYS.iter().position(|k| label.contains(k))
}

fn is_gpu(label: &str) -> bool {
    ["amdgpu", "gpu", "nouveau", "i915", "radeon", "edge"]
        .iter()
        .any(|k| label.contains(k))
}
