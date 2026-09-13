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
    let socket = docker_socket();
    let active = socket.is_some();
    RuntimeStatus {
        kind: RuntimeKind::Docker,
        name: "Docker".to_string(),
        active,
        detail: socket,
        hypervisors: Vec::new(),
    }
}

/// Locates the Docker daemon socket/pipe for the current platform.
pub fn docker_socket() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if std::fs::metadata(r"\\.\pipe\docker_engine").is_ok() {
            return Some(r"\\.\pipe\docker_engine".to_string());
        }
        None
    }

    #[cfg(unix)]
    {
        if let Ok(host) = std::env::var("DOCKER_HOST") {
            if let Some(p) = host.strip_prefix("unix://") {
                if std::path::Path::new(p).exists() {
                    return Some(p.to_string());
                }
            }
        }
        let mut candidates = vec!["/var/run/docker.sock".to_string()];
        if let Some(h) = dirs::home_dir() {
            // Docker Desktop on macOS.
            candidates.push(h.join(".docker/run/docker.sock").display().to_string());
            candidates.push(h.join(".docker/desktop/docker.sock").display().to_string());
        }
        candidates
            .into_iter()
            .find(|p| std::path::Path::new(p).exists())
    }
}

/// Candidate Podman socket paths (Linux rootless/rootful + macOS Podman machine).
pub fn podman_socket_candidates() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(d) = std::env::var("XDG_RUNTIME_DIR") {
        v.push(format!("{}/podman/podman.sock", d));
    }
    v.push("/run/podman/podman.sock".to_string());
    if let Some(h) = dirs::home_dir() {
        v.push(
            h.join(".local/share/containers/podman.sock")
                .display()
                .to_string(),
        );
        // macOS Podman machine layouts (Podman 4.5+ flat, older per-vm dirs).
        let base = h.join(".local/share/containers/podman/machine");
        v.push(base.join("podman.sock").display().to_string());
        if let Ok(entries) = std::fs::read_dir(&base) {
            for e in entries.flatten() {
                v.push(e.path().join("podman.sock").display().to_string());
            }
        }
    }
    v
}

async fn detect_podman() -> RuntimeStatus {
    let socket = podman_socket_candidates()
        .into_iter()
        .find(|p| std::path::Path::new(p).exists());

    #[cfg(target_os = "windows")]
    let socket = socket.or_else(|| {
        std::fs::metadata(r"\\.\pipe\podman")
            .is_ok()
            .then(|| r"\\.\pipe\podman".to_string())
    });

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
        // Linux distro paths + Homebrew (Apple Silicon and Intel macOS).
        let socket = [
            "/var/run/libvirt/libvirt-sock",
            "/run/libvirt/libvirt-sock",
            "/opt/homebrew/var/run/libvirt/libvirt-sock",
            "/usr/local/var/run/libvirt/libvirt-sock",
        ]
        .iter()
        .any(|p| std::path::Path::new(p).exists());
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
