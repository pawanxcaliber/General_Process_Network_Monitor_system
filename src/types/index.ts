export type RuntimeKind = "docker" | "podman" | "kubernetes" | "vm";

export interface RuntimeStatus {
  kind: RuntimeKind;
  name: string;
  active: boolean;
  detail?: string | null;
  hypervisors: string[];
}

export interface RuntimeSnapshot {
  kind?: RuntimeKind;
  available: boolean;
  detail?: string | null;
  containers: ContainerInfo[];
  networks: NetworkInfo[];
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  pods: K8sPod[];
  services: K8sService[];
  vms: VmInfo[];
  metrics_available: boolean;
  history: RuntimeHistory;
}

export interface RuntimeHistory {
  cpu: number[];
  mem_bytes: number[];
  net_rx_bps: number[];
  net_tx_bps: number[];
}

export interface RuntimeTick {
  kind: RuntimeKind;
  snapshot: RuntimeSnapshot;
}

export interface K8sPod {
  name: string;
  namespace: string;
  phase: string;
  node: string;
  pod_ip: string;
  restarts: number;
  created: number;
  cpu_millis?: number | null;
  mem_bytes?: number | null;
  containers: string[];
}

export interface K8sService {
  name: string;
  namespace: string;
  cluster_ip: string;
  ports: string[];
}

export interface VmInfo {
  name: string;
  state: string;
  hypervisor: string;
  vcpus: number;
  memory_bytes: number;
}

export interface HostPayload {
  timestamp: number;
  cpu_percent: number;
  cpu_per_core: number[];
  ram_used_bytes: number;
  ram_total_bytes: number;
  swap_used_bytes: number;
  swap_total_bytes: number;
  disks: DiskInfo[];
  interfaces: InterfaceInfo[];
  ports: PortInfo[];
  processes: ProcessInfo[];
  ping_ms?: number | null;
  cpu_temp_c?: number | null;
  gpu_igpu_used_bytes?: number | null;
  gpu_dgpu_used_bytes?: number | null;
  gpu_igpu_present?: boolean;
  gpu_dgpu_present?: boolean;
  gpu_dgpu_driver_unavailable?: boolean;
  history: HostHistory;
}

export type ProcessSortKey =
  | "cpu"
  | "mem"
  | "disk"
  | "net"
  | "gpu"
  | "igpu"
  | "dgpu"
  | "pid"
  | "name";

export interface ProcessInfo {
  pid: number;
  name: string;
  cpu_percent: number;
  memory_bytes: number;
  disk_read_bps: number;
  disk_write_bps: number;
  net_rx_bps: number;
  net_tx_bps: number;
  gpu_igpu_bytes?: number | null;
  gpu_dgpu_bytes?: number | null;
}

export interface HostHistory {
  cpu: number[];
  ram_percent: number[];
  net_rx_bps: number[];
  net_tx_bps: number[];
  disk_read_bps: number[];
  disk_write_bps: number[];
  ping_ms: number[];
  cpu_temp_c: number[];
  gpu_igpu_used_bytes: number[];
  gpu_dgpu_used_bytes: number[];
}

export interface DiskInfo {
  name: string;
  read_bps: number;
  write_bps: number;
  total_bytes: number;
  used_bytes: number;
}

export interface InterfaceInfo {
  name: string;
  rx_bps: number;
  tx_bps: number;
  rx_total: number;
  tx_total: number;
}

export interface PortInfo {
  port: number;
  proto: string;
  process?: string | null;
}

export interface ContainerInfo {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  created: number;
  ip?: string | null;
  cpu_percent: number;
  memory_bytes: number;
  memory_limit_bytes: number;
  memory_percent: number;
  rx_bps: number;
  tx_bps: number;
  rx_total: number;
  tx_total: number;
  ports: PortMapping[];
  networks: string[];
  history: ContainerHistory;
}

export interface ContainerHistory {
  cpu: number[];
  mem_bytes: number[];
}

export interface NetworkInfo {
  name: string;
  driver: string;
  containers: string[];
}

export interface TopologyNode {
  id: string;
  name: string;
  kind:
    | "host"
    | "network"
    | "container"
    | "process"
    | "k8s_namespace"
    | "k8s_pod"
    | "k8s_service"
    | "vm";
  parent_id?: string;
  status: "running" | "paused" | "exited" | "restarting";
  image?: string;
  metrics: NodeMetrics;
  ports: PortMapping[];
}

export interface NodeMetrics {
  cpu_percent: number;
  memory_bytes: number;
  rx_rate_bps: number;
  tx_rate_bps: number;
}

export interface PortMapping {
  host_port?: number;
  container_port: number;
  proto: string;
}

export interface TopologyEdge {
  id: string;
  source: string;
  target: string;
  label?: string;
  traffic_bps: number;
}

export interface LogLine {
  stream: string;
  ts?: string | null;
  text: string;
}

export type MapNodeKind = "process" | "docker" | "podman" | "pod" | "vm" | "external" | "system";

export interface MapNode {
  id: string;
  kind: MapNodeKind;
  label: string;
  detail?: string | null;
  rx_bps: number;
  tx_bps: number;
  rate_bps: number;
  conn_count: number;
  listeners: number[];
}

export interface MapEdge {
  id: string;
  source: string;
  target: string;
  rate_bps: number;
  tx_bps: number;
  rx_bps: number;
  conn_count: number;
  ports: number[];
}

export interface NetworkMapPayload {
  timestamp: number;
  nodes: MapNode[];
  edges: MapEdge[];
}

export interface LayoutNode {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface LayoutEdge {
  id: string;
  source: string;
  target: string;
  points: Array<{ x: number; y: number }>;
}

export interface LayoutResult {
  nodes: LayoutNode[];
  edges: LayoutEdge[];
}
