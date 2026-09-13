use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct TempSnapshot {
    pub cpu_c: Option<f64>,
    pub gpu_c: Option<f64>,
}

/// Polls system temperatures every 2s from `/sys/class/hwmon`.
pub async fn start_temp_poll(cache: Arc<RwLock<TempSnapshot>>) {
    loop {
        let snap = scan_temps();
        *cache.write().await = snap;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

pub fn scan_temps() -> TempSnapshot {
    let mut snap = TempSnapshot::default();
    let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") else {
        return snap;
    };

    // CPU temp by chip priority; gpu temp from amdgpu/nouveau/i915.
    let cpu_priority = ["k10temp", "zenpower", "coretemp", "cpu_thermal", "acpitz"];
    let mut best_cpu_rank: Option<usize> = None;

    for e in entries.flatten() {
        let name = std::fs::read_to_string(e.path().join("name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let temps = read_temps(&e.path());
        if temps.is_empty() {
            continue;
        }
        let max_temp = temps.iter().copied().fold(f64::MIN, f64::max);

        if let Some(rank) = cpu_priority.iter().position(|p| *p == name) {
            if best_cpu_rank.map_or(true, |r| rank < r) {
                best_cpu_rank = Some(rank);
                snap.cpu_c = Some(max_temp);
            }
        }

        match name.as_str() {
            "amdgpu" | "nouveau" | "i915" | "radeon" => {
                if snap.gpu_c.is_none_or(|g| max_temp < g) {
                    snap.gpu_c = Some(max_temp);
                }
            }
            _ => {}
        }
    }

    // Fallback: no known CPU chip found, use the hottest sensor.
    if snap.cpu_c.is_none() {
        if let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") {
            let mut all = Vec::new();
            for e in entries.flatten() {
                all.extend(read_temps(&e.path()));
            }
            if !all.is_empty() {
                snap.cpu_c = Some(all.into_iter().fold(f64::MIN, f64::max));
            }
        }
    }
    snap
}

/// Reads all `tempN_input` files (millidegrees C) in a hwmon dir.
fn read_temps(hwmon_dir: &std::path::Path) -> Vec<f64> {
    let mut temps = Vec::new();
    let Ok(entries) = std::fs::read_dir(hwmon_dir) else {
        return temps;
    };
    for e in entries.flatten() {
        let fname = e.file_name().to_string_lossy().to_string();
        if fname.starts_with("temp") && fname.ends_with("_input") {
            if let Some(v) = read_trimmed(&e.path()).and_then(|s| s.parse::<f64>().ok()) {
                temps.push(v / 1000.0);
            }
        }
    }
    temps
}

fn read_trimmed(p: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
}
