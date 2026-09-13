use serde_json::Value;

use super::GpuSnapshot;

/// macOS backend: `system_profiler SPDisplaysDataType` for adapter
/// classification. Per-process GPU memory isn't exposed by a public macOS
/// interface, so `per_pid` stays empty.
pub async fn scan() -> GpuSnapshot {
    let mut snap = GpuSnapshot::default();

    let Ok(out) = tokio::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .await
    else {
        return snap;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return snap;
    };
    let Some(arr) = v.get("SPDisplaysDataType").and_then(|a| a.as_array()) else {
        return snap;
    };

    for g in arr {
        let name = g.get("_name").and_then(|n| n.as_str()).unwrap_or("");
        let vendor = g.get("sppci_vendor").and_then(|n| n.as_str()).unwrap_or("");
        let dtype = g
            .get("sppci_device_type")
            .and_then(|n| n.as_str())
            .unwrap_or("");
        if classify(name, vendor, dtype) {
            snap.dgpu_present = true;
        } else {
            snap.igpu_present = true;
        }
    }
    snap
}

fn classify(name: &str, vendor: &str, device_type: &str) -> bool {
    let n = format!("{} {} {}", name, vendor, device_type).to_lowercase();
    if n.contains("integrated") || n.contains("apple") {
        return false;
    }
    if n.contains("nvidia")
        || n.contains("amd")
        || n.contains("radeon")
        || n.contains("discrete")
    {
        return true;
    }
    // Intel / unknown: treat as integrated.
    false
}
