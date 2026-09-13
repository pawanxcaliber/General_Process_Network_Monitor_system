use bollard::container::StatsOptions;
use bollard::Docker;
use futures_util::StreamExt;
use serde_json::Value;

use crate::types::RuntimeKind;

pub async fn connect_for(kind: RuntimeKind) -> Result<(Docker, String), String> {
    match kind {
        RuntimeKind::Docker => {
            let d = Docker::connect_with_local_defaults()
                .map_err(|e| format!("docker connect: {}", e))?;
            Ok((d, "local defaults".to_string()))
        }
        RuntimeKind::Podman => {
            #[cfg(unix)]
            {
                let candidates: Vec<String> = [
                    std::env::var("XDG_RUNTIME_DIR")
                        .ok()
                        .map(|d| format!("{}/podman/podman.sock", d)),
                    Some("/run/podman/podman.sock".to_string()),
                    dirs::home_dir()
                        .map(|h| format!("{}/.local/share/containers/podman.sock", h.display())),
                ]
                .into_iter()
                .flatten()
                .collect();

                for path in candidates {
                    if std::path::Path::new(&path).exists() {
                        let d = Docker::connect_with_unix(&path, 120, &bollard::API_DEFAULT_VERSION)
                            .map_err(|e| format!("podman connect: {}", e))?;
                        return Ok((d, path));
                    }
                }
                Err("podman socket not found".to_string())
            }

            #[cfg(target_os = "windows")]
            {
                let d = Docker::connect_with_named_pipe(r"\\.\pipe\podman", 120, &bollard::API_DEFAULT_VERSION)
                    .map_err(|e| format!("podman connect: {}", e))?;
                Ok((d, r"\\.\pipe\podman".to_string()))
            }

            #[cfg(all(not(unix), not(target_os = "windows")))]
            Err("podman unsupported on this platform".to_string())
        }
        _ => Err("not a docker-like runtime".to_string()),
    }
}

pub struct ContainerStats {
    pub cpu_percent: f64,
    pub mem_bytes: u64,
    pub mem_limit: u64,
    pub mem_percent: f64,
    pub rx_total: u64,
    pub tx_total: u64,
}

fn f(v: &Value, keys: &[&str]) -> f64 {
    let mut cur = v;
    for k in keys {
        cur = &cur[*k];
    }
    cur.as_f64().unwrap_or(0.0)
}

pub async fn container_stats(docker: &Docker, id: &str) -> Option<ContainerStats> {
    let mut stream = docker.stats(id, Some(StatsOptions { stream: false, one_shot: false }));
    let raw = match stream.next().await {
        Some(Ok(s)) => s,
        _ => return None,
    };
    let v = serde_json::to_value(&raw).ok()?;

    let cpu_total = f(&v, &["cpu_stats", "cpu_usage", "total_usage"]);
    let cpu_prev = f(&v, &["precpu_stats", "cpu_usage", "total_usage"]);
    let sys_total = f(&v, &["cpu_stats", "system_cpu_usage"]);
    let sys_prev = f(&v, &["precpu_stats", "system_cpu_usage"]);

    let online = v["cpu_stats"]["online_cpus"]
        .as_f64()
        .or_else(|| v["precpu_stats"]["online_cpus"].as_f64())
        .unwrap_or_else(|| {
            v["cpu_stats"]["cpu_usage"]["percpu_usage"]
                .as_array()
                .map(|a| a.len() as f64)
                .unwrap_or(1.0)
        });

    let cpu_delta = cpu_total - cpu_prev;
    let sys_delta = sys_total - sys_prev;
    let cpu_percent = if sys_delta > 0.0 {
        (cpu_delta / sys_delta) * online * 100.0
    } else {
        0.0
    };

    let usage = f(&v, &["memory_stats", "usage"]);
    let mstats = &v["memory_stats"]["stats"];
    let inactive = mstats["inactive_file"]
        .as_f64()
        .or_else(|| mstats["total_inactive_file"].as_f64())
        .unwrap_or(0.0);
    let mem_bytes = (usage - inactive).max(0.0) as u64;
    let mem_limit = v["memory_stats"]["limit"].as_u64().unwrap_or(0);
    let mem_percent = if mem_limit > 0 {
        mem_bytes as f64 / mem_limit as f64 * 100.0
    } else {
        0.0
    };

    let mut rx_total = 0u64;
    let mut tx_total = 0u64;
    if let Some(nets) = v["networks"].as_object() {
        for (_, data) in nets {
            rx_total += data["rx_bytes"].as_u64().unwrap_or(0);
            tx_total += data["tx_bytes"].as_u64().unwrap_or(0);
        }
    }

    Some(ContainerStats {
        cpu_percent,
        mem_bytes,
        mem_limit,
        mem_percent,
        rx_total,
        tx_total,
    })
}
