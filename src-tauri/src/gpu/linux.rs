use std::collections::HashMap;

use super::{GpuSnapshot, GpuUsage};

/// Linux backend: `/sys/bus/pci` for adapter classification, `/proc/<pid>/fdinfo`
/// DRM stats for per-process usage, and nvidia-smi for NVIDIA cards.
pub async fn scan() -> GpuSnapshot {
    let (mut snap, nv_ok) = tokio::join!(scan_gpu(), nvidia_driver_available());
    let (igpu_present, dgpu_present, _) = scan_presence().await;
    snap.igpu_present = igpu_present;
    snap.dgpu_present = dgpu_present;
    snap.dgpu_driver_unavailable = dgpu_present && !nv_ok && snap.dgpu_used_bytes == 0;
    snap
}

pub async fn scan_gpu() -> GpuSnapshot {
    let pci = tokio::task::spawn_blocking(scan_pci_gpus)
        .await
        .unwrap_or_default();
    let (drm, nv) = tokio::join!(
        tokio::task::spawn_blocking(move || scan_drm_blocking(&pci)),
        nvidia_map()
    );

    let mut snap = GpuSnapshot::default();
    snap.per_pid = drm.unwrap_or_default();

    // nvidia-smi reports compute apps; every NVIDIA GPU is discrete.
    for (pid, bytes) in nv {
        let e = snap.per_pid.entry(pid).or_insert((0, 0));
        e.1 += bytes;
    }
    for (_, (ig, dg)) in snap.per_pid.iter() {
        snap.igpu_used_bytes += ig;
        snap.dgpu_used_bytes += dg;
    }
    snap
}

/// A PCI GPU and its class.
#[derive(Debug, Clone, Copy)]
struct PciGpu {
    is_discrete: bool,
}

/// Scans `/sys/bus/pci/devices/*` for display-class devices and classifies
/// them: NVIDIA -> discrete; Intel/AMD -> integrated (when several non-NVIDIA
/// GPUs exist, the `boot_vga` one wins as integrated).
fn scan_pci_gpus() -> HashMap<String, PciGpu> {
    let mut gpus: HashMap<String, PciGpu> = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/sys/bus/pci/devices") else {
        return gpus;
    };
    for e in entries.flatten() {
        // Class file looks like "0x030000" (VGA) or "0x030200" (3D controller).
        let Some(class) = read_trimmed(&e.path().join("class")) else {
            continue;
        };
        if !class.starts_with("0x03") {
            continue;
        }
        let addr = e.file_name().to_string_lossy().to_string();
        let vendor = read_trimmed(&e.path().join("vendor")).unwrap_or_default();
        let boot_vga = read_trimmed(&e.path().join("boot_vga"))
            .map(|s| s == "1")
            .unwrap_or(false);
        let is_discrete = match vendor.as_str() {
            "0x10de" => true,
            "0x8086" | "0x1002" | "0x1022" => !boot_vga,
            _ => true,
        };
        gpus.insert(addr, PciGpu { is_discrete });
    }
    // If multiple non-NVIDIA GPUs exist, prefer the boot_vga one as iGPU and
    // mark the rest discrete.
    let non_nvidia: Vec<String> = gpus
        .iter()
        .filter(|(_, g)| !g.is_discrete)
        .map(|(a, _)| a.clone())
        .collect();
    if non_nvidia.len() > 1 {
        let boot: Option<String> = gpus
            .iter()
            .find(|(a, _)| {
                std::fs::read_to_string(format!("/sys/bus/pci/devices/{}/boot_vga", a))
                    .map(|s| s.trim() == "1")
                    .unwrap_or(false)
            })
            .map(|(a, _)| a.clone());
        let keep = boot.unwrap_or_else(|| non_nvidia[0].clone());
        for a in &non_nvidia {
            if *a != keep {
                gpus.insert(a.clone(), PciGpu { is_discrete: true });
            }
        }
    }
    gpus
}

fn read_trimmed(p: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Parses `/proc/<pid>/fdinfo/*` DRM entries and attributes usage to the GPU
/// named by `drm-pdev` (exact PCI address). Stats are identical for every fd
/// of the same `drm-client-id`, so each client is counted once (last fd wins).
/// Sums `drm-total-*` regions (VRAM + GTT on amdgpu).
fn scan_drm_blocking(pci_gpus: &HashMap<String, PciGpu>) -> HashMap<u32, GpuUsage> {
    let mut map: HashMap<u32, GpuUsage> = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return map;
    };
    for e in entries.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(e.path().join("fdinfo")) else {
            continue;
        };
        // (pdev, client) -> totals; identical across fds of one client.
        let mut clients: HashMap<(String, String), (u64, Option<bool>)> = HashMap::new();
        for fd in fds.flatten() {
            let Ok(content) = std::fs::read_to_string(fd.path()) else {
                continue;
            };
            let mut client: Option<String> = None;
            let mut pdev: Option<String> = None;
            let mut total: u64 = 0;
            for line in content.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let k = k.trim();
                    let v = v.trim();
                    if k == "drm-client-id" {
                        client = Some(v.to_string());
                    } else if k == "drm-pdev" {
                        pdev = Some(v.to_string());
                    } else if k.starts_with("drm-total-") {
                        total += parse_kib(v);
                    }
                }
            }
            // Old amdgpu kernels only expose drm-memory-*.
            if total == 0 {
                for line in content.lines() {
                    if let Some((k, v)) = line.split_once(':') {
                        if k.trim().starts_with("drm-memory-") {
                            total += parse_kib(v.trim());
                        }
                    }
                }
            }
            let Some(c) = client else { continue };
            let key_pdev = pdev.unwrap_or_default();
            let discrete = pci_gpus.get(&key_pdev).map(|g| g.is_discrete);
            clients.insert((key_pdev, c), (total, discrete));
        }

        let mut ig: u64 = 0;
        let mut dg: u64 = 0;
        for (bytes, discrete) in clients.values() {
            match discrete {
                Some(false) => ig += bytes,
                Some(true) => dg += bytes,
                None => ig += bytes,
            }
        }
        if ig > 0 || dg > 0 {
            map.insert(pid, (ig, dg));
        }
    }
    map
}

/// fdinfo memory values are reported in KiB.
fn parse_kib(v: &str) -> u64 {
    v.split_whitespace()
        .next()
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(0)
}

/// nvidia-smi compute apps; every NVIDIA GPU is discrete.
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

/// True when nvidia-smi can actually talk to the driver.
async fn nvidia_driver_available() -> bool {
    matches!(
        tokio::process::Command::new("nvidia-smi")
            .arg("-L")
            .output()
            .await,
        Ok(out) if out.status.success()
    )
}

/// Presence flags come from the PCI hardware scan, not from usage, so an
/// idle iGPU still counts as present.
async fn scan_presence() -> (bool, bool, bool) {
    tokio::task::spawn_blocking(|| {
        let gpus = scan_pci_gpus();
        let igpu = gpus.values().any(|g| !g.is_discrete);
        let dgpu = gpus.values().any(|g| g.is_discrete);
        (igpu, dgpu, dgpu)
    })
    .await
    .unwrap_or((false, false, false))
}
