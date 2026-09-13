use std::collections::HashMap;

use serde_json::Value;

use super::GpuSnapshot;

/// Windows backend: `Win32_VideoController` for adapters and the
/// `\GPU Process Memory` performance counter for per-process VRAM.
pub async fn scan() -> GpuSnapshot {
    let mut snap = GpuSnapshot::default();

    let adapters = query_adapters().await;
    for (_, discrete) in &adapters {
        if *discrete {
            snap.dgpu_present = true;
        } else {
            snap.igpu_present = true;
        }
    }
    let any_dgpu = adapters.iter().any(|(_, d)| *d);

    let mut per = process_gpu_mem().await;
    if per.is_empty() {
        per = nvidia_map().await;
    }
    for (pid, bytes) in per {
        let e = snap.per_pid.entry(pid).or_insert((0, 0));
        if any_dgpu {
            e.1 += bytes;
            snap.dgpu_used_bytes += bytes;
        } else {
            e.0 += bytes;
            snap.igpu_used_bytes += bytes;
        }
    }
    snap
}

async fn powershell(script: &str) -> Option<String> {
    let out = tokio::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn as_array(v: Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a,
        Value::Object(_) => vec![v],
        _ => Vec::new(),
    }
}

/// `(name, is_discrete)` for each video controller.
async fn query_adapters() -> Vec<(String, bool)> {
    let script = "Get-CimInstance Win32_VideoController | Select-Object Name,PNPDeviceID | ConvertTo-Json -Compress";
    let Some(out) = powershell(script).await else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(out.trim()) else {
        return Vec::new();
    };
    as_array(v)
        .iter()
        .filter_map(|a| {
            let name = a.get("Name").and_then(|n| n.as_str())?.to_string();
            Some((name.clone(), classify(&name)))
        })
        .collect()
}

fn classify(name: &str) -> bool {
    let n = name.to_lowercase();
    if n.contains("nvidia") {
        return true;
    }
    if n.contains("intel") || n.contains("apple") {
        return false;
    }
    if n.contains("amd") || n.contains("radeon") {
        return n.contains("rx")
            || n.contains("pro")
            || n.contains("firepro")
            || n.contains("instinct");
    }
    true
}

/// Per-PID dedicated VRAM via the `GPU Process Memory` PDH counter set.
async fn process_gpu_mem() -> HashMap<u32, u64> {
    let script = r#"(Get-Counter '\GPU Process Memory(*)\Dedicated Usage' -ErrorAction SilentlyContinue).CounterSamples | Select-Object InstanceName,CookedValue | ConvertTo-Json -Compress"#;
    let Some(out) = powershell(script).await else {
        return HashMap::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(out.trim()) else {
        return HashMap::new();
    };
    let mut map: HashMap<u32, u64> = HashMap::new();
    for s in as_array(v) {
        let Some(inst) = s.get("InstanceName").and_then(|n| n.as_str()) else {
            continue;
        };
        let Some(val) = s.get("CookedValue").and_then(|n| n.as_f64()) else {
            continue;
        };
        let Some(pid) = inst
            .split('_')
            .nth(1)
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        *map.entry(pid).or_insert(0) += val.max(0.0) as u64;
    }
    map
}

/// Fallback: `nvidia-smi` compute apps (NVIDIA only).
async fn nvidia_map() -> HashMap<u32, u64> {
    let Ok(out) = tokio::process::Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=pid,used_memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .await
    else {
        return HashMap::new();
    };
    if !out.status.success() {
        return HashMap::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut map = HashMap::new();
    for line in text.lines() {
        let mut parts = line.split(',');
        let pid = parts
            .next()
            .and_then(|p| p.trim().parse::<u32>().ok());
        let mib = parts
            .next()
            .and_then(|m| m.trim().parse::<u64>().ok())
            .unwrap_or(0);
        if let Some(pid) = pid {
            map.insert(pid, mib * 1024 * 1024);
        }
    }
    map
}
