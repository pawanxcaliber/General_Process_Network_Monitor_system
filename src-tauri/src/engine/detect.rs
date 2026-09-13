use std::path::PathBuf;

use crate::types::{RuntimeKind, RuntimeStatus};

pub async fn detect_all() -> Vec<RuntimeStatus> {
    let mut out = Vec::new();
    out.push(detect_docker().await);
    out.push(detect_podman().await);
    out.push(detect_kubernetes().await);
    out.push(detect_vm().await);
    out
}

fn find_in_path(cmd: &str) -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_env) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

async fn detect_docker() -> RuntimeStatus {
    let mut socket: Option<String> = None;

    #[cfg(unix)]
    {
        if std::path::Path::new("/var/run/docker.sock").exists() {
            socket = Some("/var/run/docker.sock".to_string());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if std::fs::metadata(r"\\.\pipe\docker_engine").is_ok() {
            socket = Some(r"\\.\pipe\docker_engine".to_string());
        }
    }

    let active = socket.is_some();
    RuntimeStatus {
        kind: RuntimeKind::Docker,
        name: "Docker".to_string(),
        active,
        detail: socket,
        hypervisors: Vec::new(),
    }
}

async fn detect_podman() -> RuntimeStatus {
    let mut socket: Option<String> = None;

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

        for c in candidates {
            if std::path::Path::new(&c).exists() {
                socket = Some(c);
                break;
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if std::fs::metadata(r"\\.\pipe\podman").is_ok() {
            socket = Some(r"\\.\pipe\podman".to_string());
        }
    }

    RuntimeStatus {
        kind: RuntimeKind::Podman,
        name: "Podman".to_string(),
        active: socket.is_some(),
        detail: socket,
        hypervisors: Vec::new(),
    }
}

async fn detect_kubernetes() -> RuntimeStatus {
    match kubernetes_flavor() {
        Some((flavor, server)) => RuntimeStatus {
            kind: RuntimeKind::Kubernetes,
            name: "Kubernetes".to_string(),
            active: true,
            detail: Some(if server.is_empty() {
                flavor
            } else {
                format!("{} — {}", flavor, server)
            }),
            hypervisors: Vec::new(),
        },
        None => RuntimeStatus {
            kind: RuntimeKind::Kubernetes,
            name: "Kubernetes".to_string(),
            active: false,
            detail: None,
            hypervisors: Vec::new(),
        },
    }
}

fn kubernetes_flavor() -> Option<(String, String)> {    let mut paths: Vec<PathBuf> = Vec::new();

    if let Ok(kc) = std::env::var("KUBECONFIG") {
        for p in kc.split(':') {
            if !p.is_empty() {
                paths.push(PathBuf::from(p));
            }
        }
    }
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".kube").join("config"));
    }
    paths.push(PathBuf::from("/etc/rancher/k3s/k3s.yaml"));
    paths.push(PathBuf::from("/etc/rancher/k3s/k3s.yml"));

    for path in paths {
        if !path.exists() {
            continue;
        }
        if let Ok(cfg) = kube::config::Kubeconfig::read_from(&path) {
            let current = cfg.current_context.clone().unwrap_or_default();
            let ctx = cfg.contexts.iter().find(|c| c.name == current);
            let cluster_name = ctx
                .and_then(|c| c.context.clone())
                .map(|c| c.cluster.clone())
                .unwrap_or_default();
            let server = cfg
                .clusters
                .iter()
                .find(|c| c.name == cluster_name)
                .and_then(|c| c.cluster.clone())
                .and_then(|c| c.server)
                .unwrap_or_default();

            let lower = format!("{} {}", cluster_name, server).to_lowercase();
            let flavor = if lower.contains("k3s") || path.to_string_lossy().contains("k3s") {
                "k3s"
            } else if lower.contains("k3d") {
                "k3d"
            } else if lower.contains("minikube") {
                "minikube"
            } else if lower.contains("kind") {
                "kind"
            } else if lower.contains("colima") {
                "colima"
            } else if lower.contains("rancher") {
                "rancher"
            } else {
                "cluster"
            };

            return Some((flavor.to_string(), server));
        }
    }
    None
}

pub fn k8s_flavor_hint() -> Option<String> {
    kubernetes_flavor().map(|(flavor, _)| flavor)
}

async fn detect_vm() -> RuntimeStatus {
    let mut hyps: Vec<String> = Vec::new();

    #[cfg(unix)]
    {
        let virsh = find_in_path("virsh");
        let socket = std::path::Path::new("/var/run/libvirt/libvirt-sock").exists()
            || std::path::Path::new("/run/libvirt/libvirt-sock").exists();
        if virsh.is_some() && socket {
            hyps.push("libvirt".to_string());
        }
    }

    if find_in_path("VBoxManage").is_some() {
        hyps.push("virtualbox".to_string());
    }
    if find_in_path("vmrun").is_some() {
        hyps.push("vmware".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let out = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-VM).Count"])
            .output()
            .await;
        if let Ok(o) = out {
            if o.status.success() {
                hyps.push("hyperv".to_string());
            }
        }
    }

    RuntimeStatus {
        kind: RuntimeKind::Vm,
        name: "VMs".to_string(),
        active: !hyps.is_empty(),
        detail: if hyps.is_empty() {
            None
        } else {
            Some(hyps.join(", "))
        },
        hypervisors: hyps,
    }
}
