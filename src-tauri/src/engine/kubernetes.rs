use k8s_openapi::api::core::v1::{Pod, Service};
use kube::api::{Api, ListParams};
use kube::{Client, CustomResource};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema, CustomResource)]
#[kube(group = "metrics.k8s.io", version = "v1beta1", kind = "PodMetrics", namespaced)]
#[serde(rename_all = "camelCase")]
pub struct PodMetricsSpec {
    #[serde(default)]
    pub containers: Vec<MetricsContainer>,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MetricsContainer {
    pub name: String,
    pub usage: MetricsUsage,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MetricsUsage {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

pub struct K8sData {
    pub client: Client,
    pub flavor: Option<String>,
    pub metrics_available: bool,
}

pub async fn connect() -> Result<K8sData, String> {
    let client = Client::try_default()
        .await
        .map_err(|e| format!("kube client: {}", e))?;
    let flavor = crate::engine::detect::k8s_flavor_hint();
    Ok(K8sData {
        client,
        flavor,
        metrics_available: false,
    })
}

fn parse_cpu_millis(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('m') {
        num.trim().parse::<f64>().ok().map(|v| v as u64)
    } else {
        s.parse::<f64>().ok().map(|v| (v * 1000.0) as u64)
    }
}

fn parse_mem_bytes(s: &str) -> Option<u64> {
    let s = s.trim();
    const UNITS: &[(&str, u64)] = &[
        ("Ki", 1024),
        ("Mi", 1024 * 1024),
        ("Gi", 1024 * 1024 * 1024),
        ("Ti", 1024u64 * 1024 * 1024 * 1024),
        ("K", 1000),
        ("M", 1000 * 1000),
        ("G", 1000 * 1000 * 1000),
        ("k", 1000),
    ];
    for (suffix, mult) in UNITS {
        if let Some(num) = s.strip_suffix(suffix) {
            return num.trim().parse::<f64>().ok().map(|v| (v * *mult as f64) as u64);
        }
    }
    s.parse::<u64>().ok()
}

pub async fn collect(data: &mut K8sData) -> Result<(Vec<crate::types::K8sPod>, Vec<crate::types::K8sService>, bool), String> {
    let lp = ListParams::default();

    let pods_api: Api<Pod> = Api::all(data.client.clone());
    let pod_list = pods_api
        .list(&lp)
        .await
        .map_err(|e| format!("list pods: {}", e))?;

    let svc_api: Api<Service> = Api::all(data.client.clone());
    let svc_list = svc_api
        .list(&lp)
        .await
        .map_err(|e| format!("list services: {}", e))?;

    let mut metrics_map: std::collections::HashMap<(String, String), (u64, u64)> =
        std::collections::HashMap::new();

    let metrics_api: Api<PodMetrics> = Api::all(data.client.clone());
    match metrics_api.list(&lp).await {
        Ok(m) => {
            data.metrics_available = true;
            for pm in m.items {
                let name = pm.metadata.name.clone().unwrap_or_default();
                let namespace = pm.metadata.namespace.clone().unwrap_or_default();
                let mut cpu = 0u64;
                let mut mem = 0u64;
                for c in &pm.spec.containers {
                    if let Some(u) = &c.usage.cpu {
                        cpu += parse_cpu_millis(u).unwrap_or(0);
                    }
                    if let Some(u) = &c.usage.memory {
                        mem += parse_mem_bytes(u).unwrap_or(0);
                    }
                }
                metrics_map.insert((namespace, name), (cpu, mem));
            }
        }
        Err(_) => {
            data.metrics_available = false;
        }
    }

    let mut pods = Vec::new();
    for p in pod_list.items {
        let name = p.metadata.name.clone().unwrap_or_default();
        let namespace = p.metadata.namespace.clone().unwrap_or_default();
        let phase = p
            .status
            .as_ref()
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        let node = p
            .spec
            .as_ref()
            .and_then(|s| s.node_name.clone())
            .unwrap_or_default();
        let restarts = p
            .status
            .as_ref()
            .and_then(|s| s.container_statuses.clone())
            .map(|cs| cs.iter().map(|c| c.restart_count.max(0) as u32).sum())
            .unwrap_or(0);
        let created = p
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|t| t.0.timestamp())
            .unwrap_or(0);
        let containers = p
            .spec
            .as_ref()
            .map(|s| s.containers.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default();

        let pod_ip = p
            .status
            .as_ref()
            .and_then(|s| s.pod_ip.clone())
            .unwrap_or_default();

        let (cpu_millis, mem_bytes) = match metrics_map.get(&(namespace.clone(), name.clone())) {
            Some((c, m)) => (Some(*c), Some(*m)),
            None => (None, None),
        };

        pods.push(crate::types::K8sPod {
            name,
            namespace,
            phase,
            node,
            pod_ip,
            restarts,
            created,
            cpu_millis,
            mem_bytes,
            containers,
        });
    }

    let mut services = Vec::new();
    for s in svc_list.items {
        let name = s.metadata.name.clone().unwrap_or_default();
        let namespace = s.metadata.namespace.clone().unwrap_or_default();
        let cluster_ip = s
            .spec
            .as_ref()
            .and_then(|sp| sp.cluster_ip.clone())
            .unwrap_or_default();
        let ports = s
            .spec
            .as_ref()
            .and_then(|sp| sp.ports.clone())
            .map(|ports| {
                ports
                    .iter()
                    .map(|p| {
                        format!(
                            "{}/{}",
                            p.port,
                            p.protocol.clone().unwrap_or_else(|| "tcp".to_string()).to_lowercase()
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        services.push(crate::types::K8sService {
            name,
            namespace,
            cluster_ip,
            ports,
        });
    }

    Ok((pods, services, data.metrics_available))
}
