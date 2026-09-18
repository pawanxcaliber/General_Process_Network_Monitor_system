use std::pin::Pin;
use std::future::Future;

use crate::types::VmInfo;

type VmFuture = Pin<Box<dyn Future<Output = (Vec<VmInfo>, &'static str, bool)> + Send>>;

pub async fn list_vms() -> (Vec<VmInfo>, Vec<String>) {
    #[cfg(unix)]
    let tasks: Vec<VmFuture> = vec![
        Box::pin(libvirt_vms()),
        Box::pin(virtualbox_vms()),
        Box::pin(vmware_vms()),
    ];
    #[cfg(target_os = "windows")]
    let tasks: Vec<VmFuture> = vec![
        Box::pin(virtualbox_vms()),
        Box::pin(vmware_vms()),
        Box::pin(hyperv_vms()),
    ];
    #[cfg(all(not(unix), not(target_os = "windows")))]
    let tasks: Vec<VmFuture> = vec![Box::pin(virtualbox_vms()), Box::pin(vmware_vms())];

    let results = futures_util::future::join_all(tasks).await;

    let mut vms = Vec::new();
    let mut hypervisors = Vec::new();
    for (list, hyp, present) in results {
        if present {
            hypervisors.push(hyp.to_string());
        }
        vms.extend(list);
    }
    (vms, hypervisors)
}

fn cmd_output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

async fn blocking<F>(f: F) -> (Vec<VmInfo>, bool)
where
    F: FnOnce() -> (Vec<VmInfo>, bool) + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .unwrap_or((Vec::new(), false))
}

async fn libvirt_vms() -> (Vec<VmInfo>, &'static str, bool) {
    let (vms, present) = blocking(move || {
        let mut vms = Vec::new();
        let Some(names) = cmd_output("virsh", &["list", "--all", "--name"]) else {
            return (vms, false);
        };
        for name in names.lines().filter(|l| !l.trim().is_empty()) {
            let name = name.trim().to_string();
            let mut state = "unknown".to_string();
            let mut vcpus = 0u32;
            let mut mem_kib = 0u64;

            if let Some(info) = cmd_output("virsh", &["dominfo", &name]) {
                for line in info.lines() {
                    let line = line.trim();
                    if let Some(v) = line.strip_prefix("State:") {
                        state = v.trim().to_lowercase();
                    } else if let Some(v) = line.strip_prefix("CPU(s):") {
                        vcpus = v.trim().parse().unwrap_or(0);
                    } else if let Some(v) = line.strip_prefix("Max memory:") {
                        mem_kib = v
                            .split_whitespace()
                            .next()
                            .and_then(|x| x.parse().ok())
                            .unwrap_or(0);
                    }
                }
            }

            vms.push(VmInfo {
                name,
                state,
                hypervisor: "libvirt".to_string(),
                vcpus,
                memory_bytes: mem_kib * 1024,
            });
        }
        (vms, true)
    })
    .await;
    (vms, "libvirt", present)
}

async fn virtualbox_vms() -> (Vec<VmInfo>, &'static str, bool) {
    let (vms, present) = blocking(move || {
        let mut vms = Vec::new();
        let Some(list) = cmd_output("VBoxManage", &["list", "vms"]) else {
            return (vms, false);
        };
        for line in list.lines().filter(|l| !l.trim().is_empty()) {
            let name = match line.split('"').nth(1) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let mut state = "unknown".to_string();
            let mut vcpus = 0u32;
            let mut mem_mb = 0u64;

            if let Some(info) = cmd_output("VBoxManage", &["showvminfo", &name, "--machinereadable"])
            {
                for l in info.lines() {
                    if let Some(v) = l.strip_prefix("VMState=") {
                        state = v.trim_matches('"').to_lowercase();
                    } else if let Some(v) = l.strip_prefix("cpus=") {
                        vcpus = v.trim().parse().unwrap_or(0);
                    } else if let Some(v) = l.strip_prefix("memory=") {
                        mem_mb = v.trim().parse().unwrap_or(0);
                    }
                }
            }

            vms.push(VmInfo {
                name,
                state,
                hypervisor: "virtualbox".to_string(),
                vcpus,
                memory_bytes: mem_mb * 1024 * 1024,
            });
        }
        (vms, true)
    })
    .await;
    (vms, "virtualbox", present)
}

async fn vmware_vms() -> (Vec<VmInfo>, &'static str, bool) {
    let (vms, present) = blocking(move || {
        let mut vms = Vec::new();
        let Some(list) = cmd_output("vmrun", &["list"]) else {
            return (vms, false);
        };
        for line in list.lines().skip(1).filter(|l| !l.trim().is_empty()) {
            let path = line.trim().to_string();
            let name = std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());

            let (mut vcpus, mut mem_mb) = (0u32, 0u64);
            if let Ok(vmx) = std::fs::read_to_string(&path) {
                for l in vmx.lines() {
                    if let Some(v) = l.strip_prefix("numvcpus = ") {
                        vcpus = v.trim_matches('"').parse().unwrap_or(0);
                    } else if let Some(v) = l.strip_prefix("memsize = ") {
                        mem_mb = v.trim_matches('"').parse().unwrap_or(0);
                    }
                }
            }

            vms.push(VmInfo {
                name,
                state: "running".to_string(),
                hypervisor: "vmware".to_string(),
                vcpus,
                memory_bytes: mem_mb * 1024 * 1024,
            });
        }
        (vms, true)
    })
    .await;
    (vms, "vmware", present)
}

#[cfg(target_os = "windows")]
async fn hyperv_vms() -> (Vec<VmInfo>, &'static str, bool) {
    let (vms, present) = blocking(move || {
        let mut vms = Vec::new();
        let Some(csv) = cmd_output(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "Get-VM | Select-Object Name,State,ProcessorCount,MemoryAssigned | ConvertTo-Csv -NoTypeInformation",
            ],
        ) else {
            return (vms, false);
        };
        for (i, line) in csv.lines().enumerate() {
            if i == 0 || line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split(',').map(|c| c.trim_matches('"')).collect();
            if cols.len() < 4 {
                continue;
            }
            vms.push(VmInfo {
                name: cols[0].to_string(),
                state: cols[1].to_lowercase(),
                hypervisor: "hyperv".to_string(),
                vcpus: cols[2].parse().unwrap_or(0),
                memory_bytes: cols[3].parse().unwrap_or(0),
            });
        }
        (vms, true)
    })
    .await;
    (vms, "hyperv", present)
}
