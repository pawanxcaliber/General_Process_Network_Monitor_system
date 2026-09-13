use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Docker,
    Podman,
    Kubernetes,
    Vm,
}

impl RuntimeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeKind::Docker => "docker",
            RuntimeKind::Podman => "podman",
            RuntimeKind::Kubernetes => "kubernetes",
            RuntimeKind::Vm => "vm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub kind: RuntimeKind,
    pub name: String,
    pub active: bool,
    pub detail: Option<String>,
    pub hypervisors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeSnapshot {
    pub kind: Option<RuntimeKind>,
    pub available: bool,
    pub detail: Option<String>,
    pub containers: Vec<ContainerInfo>,
    pub networks: Vec<NetworkInfo>,
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub pods: Vec<K8sPod>,
    pub services: Vec<K8sService>,
    pub vms: Vec<VmInfo>,
    pub metrics_available: bool,
    pub history: RuntimeHistory,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeHistory {
    pub cpu: Vec<f64>,
    pub mem_bytes: Vec<f64>,
    pub net_rx_bps: Vec<f64>,
    pub net_tx_bps: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sPod {
    pub name: String,
    pub namespace: String,
    pub phase: String,
    pub node: String,
    pub pod_ip: String,
    pub restarts: u32,
    pub created: i64,
    pub cpu_millis: Option<u64>,
    pub mem_bytes: Option<u64>,
    pub containers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sService {
    pub name: String,
    pub namespace: String,
    pub cluster_ip: String,
    pub ports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInfo {
    pub name: String,
    pub state: String,
    pub hypervisor: String,
    pub vcpus: u32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessSortKey {
    Cpu,
    Mem,
    Disk,
    Net,
    Gpu,
    Igpu,
    Dgpu,
    Pid,
    Name,
}

impl ProcessSortKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessSortKey::Cpu => "cpu",
            ProcessSortKey::Mem => "mem",
            ProcessSortKey::Disk => "disk",
            ProcessSortKey::Net => "net",
            ProcessSortKey::Gpu => "gpu",
            ProcessSortKey::Igpu => "igpu",
            ProcessSortKey::Dgpu => "dgpu",
            ProcessSortKey::Pid => "pid",
            ProcessSortKey::Name => "name",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostPayload {
    pub timestamp: f64,
    pub cpu_percent: f32,
    pub cpu_per_core: Vec<f32>,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub disks: Vec<DiskInfo>,
    pub interfaces: Vec<InterfaceInfo>,
    pub ports: Vec<PortInfo>,
    pub processes: Vec<ProcessInfo>,
    pub ping_ms: Option<f64>,
    pub gpu_igpu_used_bytes: u64,
    pub gpu_dgpu_used_bytes: u64,
    pub gpu_igpu_present: bool,
    pub gpu_dgpu_present: bool,
    pub gpu_dgpu_driver_unavailable: bool,
    pub history: HostHistory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub disk_read_bps: f64,
    pub disk_write_bps: f64,
    pub net_rx_bps: f64,
    pub net_tx_bps: f64,
    pub gpu_igpu_bytes: Option<u64>,
    pub gpu_dgpu_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostHistory {
    pub cpu: Vec<f64>,
    pub ram_percent: Vec<f64>,
    pub net_rx_bps: Vec<f64>,
    pub net_tx_bps: Vec<f64>,
    pub disk_read_bps: Vec<f64>,
    pub disk_write_bps: Vec<f64>,
    pub ping_ms: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub read_bps: f64,
    pub write_bps: f64,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: u16,
    pub proto: String,
    pub process: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    pub ip: Option<String>,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub memory_limit_bytes: u64,
    pub memory_percent: f64,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub rx_total: u64,
    pub tx_total: u64,
    pub ports: Vec<PortMapping>,
    pub networks: Vec<String>,
    pub history: ContainerHistory,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerHistory {
    pub cpu: Vec<f64>,
    pub mem_bytes: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub driver: String,
    pub containers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub parent_id: Option<String>,
    pub status: NodeStatus,
    pub image: Option<String>,
    pub metrics: NodeMetrics,
    pub ports: Vec<PortMapping>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Host,
    Network,
    Container,
    K8sNamespace,
    K8sPod,
    K8sService,
    Vm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Running,
    Paused,
    Exited,
    Restarting,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub rx_rate_bps: f64,
    pub tx_rate_bps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub host_port: Option<u16>,
    pub container_port: u16,
    pub proto: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub traffic_bps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub stream: String,
    pub ts: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTick {
    pub kind: RuntimeKind,
    pub snapshot: RuntimeSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapNodeKind {
    Process,
    Docker,
    Podman,
    Pod,
    Vm,
    External,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapNode {
    pub id: String,
    pub kind: MapNodeKind,
    pub label: String,
    pub detail: Option<String>,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub rate_bps: f64,
    pub conn_count: u32,
    pub listeners: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub rate_bps: f64,
    pub tx_bps: f64,
    pub rx_bps: f64,
    pub conn_count: u32,
    pub ports: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkMapPayload {
    pub timestamp: f64,
    pub nodes: Vec<MapNode>,
    pub edges: Vec<MapEdge>,
}
