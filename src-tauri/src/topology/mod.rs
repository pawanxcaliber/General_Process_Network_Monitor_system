use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tauri::{AppHandle, Emitter};

use crate::engine::{docker, kubernetes, push_capped, vm, RuntimeKind, RuntimeState};
use crate::types::{
    ContainerHistory, ContainerInfo, K8sPod, K8sService, NetworkInfo, NodeKind, NodeMetrics,
    NodeStatus, PortMapping, RuntimeSnapshot, RuntimeStatus, RuntimeTick, TopologyEdge,
    TopologyNode,
};

type StatesMap = Arc<Mutex<HashMap<RuntimeKind, Arc<RwLock<RuntimeState>>>>>;
type SnapshotsMap = Arc<RwLock<HashMap<RuntimeKind, RuntimeSnapshot>>>;

async fn emit_snapshot(
    app: &AppHandle,
    cache: &SnapshotsMap,
    kind: RuntimeKind,
    snapshot: RuntimeSnapshot,
) {
    cache.write().await.insert(kind, snapshot.clone());
    let _ = app.emit(
        "runtime-tick",
        &RuntimeTick {
            kind,
            snapshot,
        },
    );
}

fn map_node_status(state: &str) -> NodeStatus {
    match state {
        "running" => NodeStatus::Running,
        "paused" => NodeStatus::Paused,
        "restarting" => NodeStatus::Restarting,
        _ => NodeStatus::Exited,
    }
}

pub async fn start_docker_like_poll(
    app: AppHandle,
    kind: RuntimeKind,
    states: StatesMap,
    snapshots: SnapshotsMap,
    interval: Duration,
) {
    let state_arc = {
        let m = states.lock().await;
        m.get(&kind).cloned().unwrap_or_else(|| {
            let arc = Arc::new(RwLock::new(RuntimeState::new(kind)));
            arc
        })
    };

    loop {
        tokio::time::sleep(interval).await;

        let result = {
            let mut s = state_arc.write().await;
            if s.docker.is_none() {
                match docker::connect_for(kind).await {
                    Ok((client, detail)) => {
                        s.docker = Some(client);
                        s.detail = Some(detail);
                    }
                    Err(e) => {
                        s.ok = false;
                        s.detail = Some(e);
                        continue;
                    }
                }
            }
            collect_docker_like(kind, &mut s).await
        };

        match result {
            Ok(snapshot) => {
                state_arc.write().await.ok = true;
                emit_snapshot(&app, &snapshots, kind, snapshot).await;
            }
            Err(e) => {
                eprintln!("{}: {}", kind.as_str(), e);
                let mut s = state_arc.write().await;
                s.ok = false;
                s.detail = Some(e);
                s.docker = None;
            }
        }
    }
}

fn js<'a>(v: &'a serde_json::Value, keys: &[&str]) -> &'a serde_json::Value {
    for k in keys {
        let val = &v[*k];
        if !val.is_null() {
            return val;
        }
    }
    &serde_json::Value::Null
}

struct BaseContainer {
    id: String,
    name: String,
    image: String,
    state: String,
    status: String,
    created: i64,
    ip: Option<String>,
    ports: Vec<PortMapping>,
    networks: Vec<String>,
}

async fn collect_docker_like(
    kind: RuntimeKind,
    state: &mut RuntimeState,
) -> Result<RuntimeSnapshot, String> {
    let client = state
        .docker
        .clone()
        .ok_or_else(|| "no client".to_string())?;

    let containers_res = client
        .list_containers(Some(bollard::container::ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(|e| format!("list containers: {}", e))?;

    let networks_res = client
        .list_networks(None::<bollard::network::ListNetworksOptions<String>>)
        .await
        .map_err(|e| format!("list networks: {}", e))?;

    let mut networks: Vec<NetworkInfo> = Vec::new();
    for n in &networks_res {
        let v = serde_json::to_value(n).unwrap_or(serde_json::Value::Null);
        let name = js(&v, &["Name", "name"])
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let driver = js(&v, &["Driver", "driver"]).as_str().unwrap_or("-").to_string();
        let mut members: Vec<String> = Vec::new();
        if let Some(conts) = js(&v, &["Containers", "containers"]).as_object() {
            for (_, c) in conts {
                if let Some(cname) = c["Name"].as_str() {
                    members.push(cname.to_string());
                }
            }
        }
        networks.push(NetworkInfo {
            name,
            driver,
            containers: members,
        });
    }

    let mut ip_by_id: HashMap<String, String> = HashMap::new();
    let mut ip_by_name: HashMap<String, String> = HashMap::new();
    for net in &networks {
        if let Ok(resp) = client
            .inspect_network(&net.name, None::<bollard::network::InspectNetworkOptions<String>>)
            .await
        {
            if let Ok(v) = serde_json::to_value(&resp) {
                if let Some(conts) = v["Containers"].as_object() {
                    for (id, c) in conts {
                        let ip = c["IPv4Address"]
                            .as_str()
                            .map(|s| s.split('/').next().unwrap_or(s).to_string());
                        let name = c["Name"].as_str().map(|s| s.to_string());
                        if let Some(ip) = ip {
                            ip_by_id.insert(id.clone(), ip.clone());
                            if let Some(n) = name {
                                ip_by_name.insert(n, ip);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut base: Vec<BaseContainer> = Vec::new();
    for c in &containers_res {
        let v = serde_json::to_value(c).unwrap_or(serde_json::Value::Null);
        let id = js(&v, &["Id", "id"]).as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let name = js(&v, &["Names", "names"])
            .as_array()
            .and_then(|a| a.first())
            .and_then(|n| n.as_str())
            .map(|s| s.trim_start_matches('/').to_string())
            .unwrap_or_else(|| id[..12.min(id.len())].to_string());
        let image = js(&v, &["Image", "image"]).as_str().unwrap_or("").to_string();
        let cstate = js(&v, &["State", "state"]).as_str().unwrap_or("unknown").to_string();
        let status = js(&v, &["Status", "status"]).as_str().unwrap_or("").to_string();
        let created = js(&v, &["Created", "created"]).as_i64().unwrap_or(0);

        let mut ports: Vec<PortMapping> = Vec::new();
        if let Some(parr) = js(&v, &["Ports", "ports"]).as_array() {
            for p in parr {
                ports.push(PortMapping {
                    host_port: p["PublicPort"]
                        .as_u64()
                        .or_else(|| p["publicPort"].as_u64())
                        .map(|x| x as u16),
                    container_port: p["PrivatePort"]
                        .as_u64()
                        .or_else(|| p["privatePort"].as_u64())
                        .unwrap_or(0) as u16,
                    proto: p["Type"]
                        .as_str()
                        .or_else(|| p["type"].as_str())
                        .unwrap_or("tcp")
                        .to_string(),
                });
            }
        }

        let mut net_names: Vec<String> = Vec::new();
        if let Some(nets) = js(&v, &["NetworkSettings", "networkSettings"])
            .get("Networks")
            .or_else(|| v.get("Networks"))
            .and_then(|n| n.as_object())
        {
            for k in nets.keys() {
                net_names.push(k.clone());
            }
        }

        base.push(BaseContainer {
            id: id.clone(),
            name,
            image,
            state: cstate,
            status,
            created,
            ip: ip_by_id.get(&id).cloned(),
            ports,
            networks: net_names,
        });
    }

    let mut stats_jobs: Vec<
        std::pin::Pin<Box<dyn std::future::Future<Output = Option<docker::ContainerStats>> + Send>>,
    > = Vec::new();
    for b in &base {
        let fut: std::pin::Pin<
            Box<dyn std::future::Future<Output = Option<docker::ContainerStats>> + Send>,
        > = if b.state == "running" {
            Box::pin(docker::container_stats(&client, &b.id))
        } else {
            Box::pin(async { None })
        };
        stats_jobs.push(fut);
    }
    let stats_results = futures_util::future::join_all(stats_jobs).await;

    let now = Instant::now();
    let mut containers: Vec<ContainerInfo> = Vec::new();
    let mut nodes: Vec<TopologyNode> = Vec::new();
    let mut edges: Vec<TopologyEdge> = Vec::new();
    let mut agg_cpu = 0.0f64;
    let mut agg_mem = 0.0f64;
    let mut agg_rx = 0.0f64;
    let mut agg_tx = 0.0f64;

    nodes.push(TopologyNode {
        id: "host".to_string(),
        name: "Host".to_string(),
        kind: NodeKind::Host,
        parent_id: None,
        status: NodeStatus::Running,
        image: None,
        metrics: NodeMetrics::default(),
        ports: Vec::new(),
    });

    for net in &networks {
        nodes.push(TopologyNode {
            id: format!("net:{}", net.name),
            name: net.name.clone(),
            kind: NodeKind::Network,
            parent_id: None,
            status: NodeStatus::Running,
            image: None,
            metrics: NodeMetrics::default(),
            ports: Vec::new(),
        });
    }

    for (b, stats) in base.iter().zip(stats_results) {
        let (mut cpu, mut mem, mut mem_limit, mut mem_pct, mut rx_total, mut tx_total) =
            (0.0f64, 0u64, 0u64, 0.0f64, 0u64, 0u64);

        if let Some(s) = stats {
            cpu = s.cpu_percent;
            mem = s.mem_bytes;
            mem_limit = s.mem_limit;
            mem_pct = s.mem_percent;
            rx_total = s.rx_total;
            tx_total = s.tx_total;
        }

        let (rx_bps, tx_bps) = match state.net_totals.get(&b.id) {
            Some((prx, ptx, t)) if rx_total > 0 || tx_total > 0 => {
                let dt = t.elapsed().as_secs_f64().max(0.001);
                (
                    rx_total.saturating_sub(*prx) as f64 / dt,
                    tx_total.saturating_sub(*ptx) as f64 / dt,
                )
            }
            _ => (0.0, 0.0),
        };
        state.net_totals.insert(b.id.clone(), (rx_total, tx_total, now));

        agg_cpu += if b.state == "running" { cpu } else { 0.0 };
        agg_mem += mem as f64;
        agg_rx += rx_bps;
        agg_tx += tx_bps;

        let cpu_hist = state.cpu_hist.entry(b.id.clone()).or_default();
        push_capped(cpu_hist, cpu);
        let mem_hist = state.mem_hist.entry(b.id.clone()).or_default();
        push_capped(mem_hist, mem as f64);

        let node_status = map_node_status(&b.state);
        let parent_id = b.networks.first().map(|n| format!("net:{}", n));
        let short = &b.id[..12.min(b.id.len())];

        for net_name in &b.networks {
            edges.push(TopologyEdge {
                id: format!("e:net:{}:{}", net_name, short),
                source: format!("net:{}", net_name),
                target: format!("ctr:{}", b.id),
                label: None,
                traffic_bps: rx_bps + tx_bps,
            });
        }

        for p in &b.ports {
            if let Some(hp) = p.host_port {
                edges.push(TopologyEdge {
                    id: format!("e:host:{}:{}", hp, short),
                    source: "host".to_string(),
                    target: format!("ctr:{}", b.id),
                    label: Some(format!("{}:{}", hp, p.container_port)),
                    traffic_bps: rx_bps + tx_bps,
                });
            }
        }

        nodes.push(TopologyNode {
            id: format!("ctr:{}", b.id),
            name: b.name.clone(),
            kind: NodeKind::Container,
            parent_id,
            status: node_status,
            image: Some(b.image.clone()),
            metrics: NodeMetrics {
                cpu_percent: cpu,
                memory_bytes: mem,
                rx_rate_bps: rx_bps,
                tx_rate_bps: tx_bps,
            },
            ports: b.ports.clone(),
        });

        containers.push(ContainerInfo {
            id: b.id.clone(),
            name: b.name.clone(),
            image: b.image.clone(),
            state: b.state.clone(),
            status: b.status.clone(),
            created: b.created,
            ip: b.ip.clone(),
            cpu_percent: cpu,
            memory_bytes: mem,
            memory_limit_bytes: mem_limit,
            memory_percent: mem_pct,
            rx_bps,
            tx_bps,
            rx_total,
            tx_total,
            ports: b.ports.clone(),
            networks: b.networks.clone(),
            history: ContainerHistory {
                cpu: state
                    .cpu_hist
                    .get(&b.id)
                    .map(|d| d.iter().copied().collect())
                    .unwrap_or_default(),
                mem_bytes: state
                    .mem_hist
                    .get(&b.id)
                    .map(|d| d.iter().copied().collect())
                    .unwrap_or_default(),
            },
        });
    }

    let traffic_by_net: HashMap<String, f64> = {
        let mut m: HashMap<String, f64> = HashMap::new();
        for e in &edges {
            if e.source.starts_with("net:") {
                *m.entry(e.source.clone()).or_default() += e.traffic_bps;
            }
        }
        m
    };
    for node in nodes.iter_mut() {
        if node.kind == NodeKind::Network {
            if let Some(t) = traffic_by_net.get(&node.id) {
                node.metrics.rx_rate_bps = *t;
            }
        }
    }

    let live_net_ids: HashSet<String> = {
        let mut s = HashSet::new();
        for c in &containers {
            for n in &c.networks {
                s.insert(format!("net:{}", n));
            }
        }
        s
    };
    nodes.retain(|n| n.kind != NodeKind::Network || live_net_ids.contains(&n.id));

    state.push_agg(agg_cpu, agg_mem, agg_rx, agg_tx);

    Ok(RuntimeSnapshot {
        kind: Some(kind),
        available: true,
        detail: state.detail.clone(),
        containers,
        networks,
        nodes,
        edges,
        pods: Vec::new(),
        services: Vec::new(),
        vms: Vec::new(),
        metrics_available: false,
        history: state.agg.clone(),
    })
}

pub async fn start_k8s_poll(
    app: AppHandle,
    states: StatesMap,
    snapshots: SnapshotsMap,
    interval: Duration,
) {
    let state_arc = {
        let m = states.lock().await;
        m.get(&RuntimeKind::Kubernetes).cloned().unwrap()
    };

    loop {
        tokio::time::sleep(interval).await;

        let result: Option<Result<(Vec<K8sPod>, Vec<K8sService>, bool, Option<String>), String>> = {
            let mut s = state_arc.write().await;
            if s.k8s.is_none() {
                match kubernetes::connect().await {
                    Ok(d) => s.k8s = Some(d),
                    Err(e) => {
                        s.ok = false;
                        s.detail = Some(e);
                        continue;
                    }
                }
            }
            match s.k8s.as_mut() {
                Some(data) => match kubernetes::collect(data).await {
                    Ok((pods, services, metrics_available)) => {
                        let detail = data.flavor.clone().map(|f| format!("context: {}", f));
                        Some(Ok((pods, services, metrics_available, detail)))
                    }
                    Err(e) => Some(Err(e)),
                },
                None => None,
            }
        };

        match result {
            Some(Ok((pods, services, metrics_available, detail))) => {
                let snapshot = build_k8s_snapshot(&pods, &services, metrics_available, detail);
                state_arc.write().await.ok = true;
                emit_snapshot(&app, &snapshots, RuntimeKind::Kubernetes, snapshot).await;
            }
            Some(Err(e)) => {
                eprintln!("kubernetes: {}", e);
                let mut s = state_arc.write().await;
                s.ok = false;
                s.detail = Some(e);
                s.k8s = None;
            }
            None => {}
        }
    }
}

fn k8s_pod_status(phase: &str) -> NodeStatus {
    match phase.to_lowercase().as_str() {
        "running" => NodeStatus::Running,
        "pending" => NodeStatus::Restarting,
        _ => NodeStatus::Exited,
    }
}

fn build_k8s_snapshot(
    pods: &[K8sPod],
    services: &[K8sService],
    metrics_available: bool,
    detail: Option<String>,
) -> RuntimeSnapshot {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut agg_cpu = 0.0f64;
    let mut agg_mem = 0.0f64;

    let mut namespaces: Vec<String> = pods.iter().map(|p| p.namespace.clone()).collect();
    namespaces.extend(services.iter().map(|s| s.namespace.clone()));
    namespaces.sort();
    namespaces.dedup();

    for ns in &namespaces {
        nodes.push(TopologyNode {
            id: format!("ns:{}", ns),
            name: ns.clone(),
            kind: NodeKind::K8sNamespace,
            parent_id: None,
            status: NodeStatus::Running,
            image: None,
            metrics: NodeMetrics::default(),
            ports: Vec::new(),
        });
    }

    for p in pods {
        agg_cpu += p.cpu_millis.unwrap_or(0) as f64 / 1000.0;
        agg_mem += p.mem_bytes.unwrap_or(0) as f64;
        nodes.push(TopologyNode {
            id: format!("pod:{}/{}", p.namespace, p.name),
            name: p.name.clone(),
            kind: NodeKind::K8sPod,
            parent_id: Some(format!("ns:{}", p.namespace)),
            status: k8s_pod_status(&p.phase),
            image: None,
            metrics: NodeMetrics {
                cpu_percent: p.cpu_millis.map(|c| c as f64 / 10.0).unwrap_or(0.0),
                memory_bytes: p.mem_bytes.unwrap_or(0),
                rx_rate_bps: 0.0,
                tx_rate_bps: 0.0,
            },
            ports: Vec::new(),
        });
        edges.push(TopologyEdge {
            id: format!("e:ns:{}:{}", p.namespace, p.name),
            source: format!("ns:{}", p.namespace),
            target: format!("pod:{}/{}", p.namespace, p.name),
            label: None,
            traffic_bps: 0.0,
        });
    }

    for s in services {
        nodes.push(TopologyNode {
            id: format!("svc:{}/{}", s.namespace, s.name),
            name: s.name.clone(),
            kind: NodeKind::K8sService,
            parent_id: Some(format!("ns:{}", s.namespace)),
            status: NodeStatus::Running,
            image: None,
            metrics: NodeMetrics::default(),
            ports: Vec::new(),
        });
        edges.push(TopologyEdge {
            id: format!("e:svc:{}:{}", s.namespace, s.name),
            source: format!("svc:{}/{}", s.namespace, s.name),
            target: format!("ns:{}", s.namespace),
            label: s.ports.first().cloned(),
            traffic_bps: 0.0,
        });
    }

    let history_cpu = vec![agg_cpu];
    let history_mem = vec![agg_mem];

    RuntimeSnapshot {
        kind: Some(RuntimeKind::Kubernetes),
        available: true,
        detail,
        containers: Vec::new(),
        networks: Vec::new(),
        nodes,
        edges,
        pods: pods.to_vec(),
        services: services.to_vec(),
        vms: Vec::new(),
        metrics_available,
        history: crate::types::RuntimeHistory {
            cpu: history_cpu,
            mem_bytes: history_mem,
            net_rx_bps: Vec::new(),
            net_tx_bps: Vec::new(),
        },
    }
}

pub async fn start_vm_poll(
    app: AppHandle,
    states: StatesMap,
    snapshots: SnapshotsMap,
    interval: Duration,
) {
    let state_arc = {
        let m = states.lock().await;
        m.get(&RuntimeKind::Vm).cloned().unwrap()
    };

    loop {
        tokio::time::sleep(interval).await;

        let (vms, hypervisors) = vm::list_vms().await;

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut running = 0u64;
        let mut agg_mem = 0.0f64;

        nodes.push(TopologyNode {
            id: "host".to_string(),
            name: "Host".to_string(),
            kind: NodeKind::Host,
            parent_id: None,
            status: NodeStatus::Running,
            image: None,
            metrics: NodeMetrics::default(),
            ports: Vec::new(),
        });

        for vm_info in &vms {
            let status = map_vm_status(&vm_info.state);
            if status == NodeStatus::Running {
                running += 1;
            }
            agg_mem += vm_info.memory_bytes as f64;
            nodes.push(TopologyNode {
                id: format!("vm:{}", vm_info.name),
                name: vm_info.name.clone(),
                kind: NodeKind::Vm,
                parent_id: Some("host".to_string()),
                status,
                image: Some(vm_info.hypervisor.clone()),
                metrics: NodeMetrics {
                    cpu_percent: 0.0,
                    memory_bytes: vm_info.memory_bytes,
                    rx_rate_bps: 0.0,
                    tx_rate_bps: 0.0,
                },
                ports: Vec::new(),
            });
            edges.push(TopologyEdge {
                id: format!("e:host:vm:{}", vm_info.name),
                source: "host".to_string(),
                target: format!("vm:{}", vm_info.name),
                label: Some(vm_info.hypervisor.clone()),
                traffic_bps: 0.0,
            });
        }

        let detail = if hypervisors.is_empty() {
            Some("hypervisor tools not responding".to_string())
        } else {
            Some(hypervisors.join(", "))
        };
        let available = !hypervisors.is_empty();

        let mut s = state_arc.write().await;
        s.push_agg(running as f64, agg_mem, 0.0, 0.0);
        s.ok = available;
        s.detail = detail.clone();

        let snapshot = RuntimeSnapshot {
            kind: Some(RuntimeKind::Vm),
            available,
            detail,
            containers: Vec::new(),
            networks: Vec::new(),
            nodes,
            edges,
            pods: Vec::new(),
            services: Vec::new(),
            vms,
            metrics_available: false,
            history: s.agg.clone(),
        };

        emit_snapshot(&app, &snapshots, RuntimeKind::Vm, snapshot).await;
    }
}

fn map_vm_status(state: &str) -> NodeStatus {
    match state {
        "running" => NodeStatus::Running,
        "paused" | "pmsuspended" | "suspended" => NodeStatus::Paused,
        "shut off" | "shut-off" | "poweroff" | "stopped" | "aborted" => NodeStatus::Exited,
        _ => NodeStatus::Exited,
    }
}

pub async fn detect_all_public() -> Vec<RuntimeStatus> {
    crate::engine::detect::detect_all().await
}
