use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tauri::{AppHandle, Emitter};

use crate::types::{
    MapEdge, MapNode, MapNodeKind, NetworkMapPayload, RuntimeKind, RuntimeSnapshot,
};

pub type SnapshotsMap = Arc<RwLock<HashMap<RuntimeKind, RuntimeSnapshot>>>;

const GC_TICKS: u32 = 3;
const EXT_CAP: usize = 60;
const MAX_PORTS_PER_EDGE: usize = 6;

struct RawSock {
    lip: String,
    lport: u16,
    rip: String,
    rport: u16,
    listening: bool,
    established: bool,
    pid: Option<u32>,
    label: Option<String>,
    sent: u64,
    recv: u64,
}

fn ip_string(ip: IpAddr) -> String {
    match ip {
        IpAddr::V6(v6) => v6.to_canonical().to_string(),
        IpAddr::V4(v4) => v4.to_string(),
    }
}

/// Reads all sockets from the platform layer into the map's raw form.
fn read_sockets() -> Vec<RawSock> {
    crate::platform::sockets::connections()
        .into_iter()
        .map(|c| RawSock {
            lip: ip_string(c.local_ip),
            lport: c.local_port,
            rip: c
                .remote_ip
                .map(ip_string)
                .unwrap_or_else(|| "0.0.0.0".to_string()),
            rport: c.remote_port,
            listening: c.listening,
            established: c.established,
            pid: c.pid,
            label: c.label,
            sent: c.tx_bytes.unwrap_or(0),
            recv: c.rx_bytes.unwrap_or(0),
        })
        .collect()
}

#[derive(Clone)]
pub(crate) enum Entity {
    Proc { pid: u32, label: String, detail: Option<String> },
    Ctr { kind: MapNodeKind, id: String, label: String, detail: Option<String> },
    Pod { id: String, label: String },
    Vm { id: String, label: String, detail: Option<String> },
    Root,
    Ext { ip: String, port: u16 },
}

impl Entity {
    fn id(&self) -> String {
        match self {
            Entity::Proc { pid, .. } => format!("proc:{}", pid),
            Entity::Ctr { kind, id, .. } => format!("ctr:{}:{}", kind_str(*kind), id),
            Entity::Pod { id, .. } => format!("pod:{}", id),
            Entity::Vm { id, .. } => format!("vm:{}", id),
            Entity::Root => "root".to_string(),
            Entity::Ext { ip, port } => format!("ext:{}:{}", ip, port),
        }
    }

    fn node(&self) -> MapNode {
        let (kind, label, detail) = match self {
            Entity::Proc { label, detail, .. } => (MapNodeKind::Process, label.clone(), detail.clone()),
            Entity::Ctr { kind, label, detail, .. } => (*kind, label.clone(), detail.clone()),
            Entity::Pod { label, .. } => (MapNodeKind::Pod, label.clone(), None),
            Entity::Vm { label, detail, .. } => (MapNodeKind::Vm, label.clone(), detail.clone()),
            Entity::Root => (MapNodeKind::System, "system (root)".to_string(), None),
            Entity::Ext { ip, port } => (
                MapNodeKind::External,
                match svc_name(*port) {
                    Some(svc) => format!("{}:{} · {}", ip, port, svc),
                    None => format!("{}:{}", ip, port),
                },
                None,
            ),
        };
        MapNode {
            id: self.id(),
            kind,
            label,
            detail,
            rx_bps: 0.0,
            tx_bps: 0.0,
            rate_bps: 0.0,
            conn_count: 0,
            listeners: Vec::new(),
        }
    }
}

fn kind_str(kind: MapNodeKind) -> &'static str {
    match kind {
        MapNodeKind::Docker => "docker",
        MapNodeKind::Podman => "podman",
        MapNodeKind::Pod => "pod",
        MapNodeKind::Vm => "vm",
        _ => "other",
    }
}

fn svc_name(port: u16) -> Option<&'static str> {
    Some(match port {
        20 | 21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "dns",
        67 | 68 => "dhcp",
        80 => "http",
        110 => "pop3",
        123 => "ntp",
        143 => "imap",
        161 | 162 => "snmp",
        389 | 636 => "ldap",
        443 => "https",
        445 => "smb",
        465 | 587 => "smtps",
        993 => "imaps",
        995 => "pop3s",
        1433 => "mssql",
        1521 => "oracle",
        3306 => "mysql",
        3389 => "rdp",
        5432 => "postgres",
        5900 => "vnc",
        6379 => "redis",
        8080 | 8000 | 8888 => "http-alt",
        8443 => "https-alt",
        9200 | 9300 => "elastic",
        11211 => "memcached",
        27017 => "mongo",
        5672 => "amqp",
        9090 => "prom",
        3000 => "dev",
        5000 | 5001 => "flask",
        _ => return None,
    })
}

struct Endpoint {
    entity: Entity,
    listeners: Vec<u16>,
}

const LINGER: Duration = Duration::from_secs(10);

struct CachedNode {
    entity: Entity,
    listeners: Vec<u16>,
    last_seen: Instant,
}

pub struct Collector {
    conns: HashMap<String, (u64, u64, Instant)>,
    misses: HashMap<String, u32>,
    tick: Instant,
    cache: HashMap<String, CachedNode>,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            conns: HashMap::new(),
            misses: HashMap::new(),
            tick: Instant::now(),
            cache: HashMap::new(),
        }
    }

    pub fn collect(&mut self, ip_refs: &IpRefs) -> NetworkMapPayload {
        let now = Instant::now();
        let dt = self.tick.elapsed().as_secs_f64().max(0.001);
        self.tick = now;

        let socks = read_sockets();

        let mut endpoints: HashMap<String, Endpoint> = HashMap::new();
        let mut relay_ports: HashSet<u16> = HashSet::new();

        for s in &socks {
            let entity = match resolve_local(s, ip_refs) {
                Some(e) => e,
                None => {
                    relay_ports.insert(s.lport);
                    Entity::Root
                }
            };
            let key = format!("{}:{}", s.lip, s.lport);
            let entry = endpoints.entry(key).or_insert(Endpoint {
                entity,
                listeners: Vec::new(),
            });
            if s.listening && !entry.listeners.contains(&s.lport) {
                entry.listeners.push(s.lport);
            }
        }

        let mut edges: HashMap<(String, String), EdgeAcc> = HashMap::new();
        let mut entity_refs: HashMap<String, Entity> = HashMap::new();
        let mut seen_keys: HashSet<String> = HashSet::new();

        for s in &socks {
            if !s.established {
                continue;
            }
            let key = format!("{}:{}|{}:{}", s.lip, s.lport, s.rip, s.rport);
            seen_keys.insert(key.clone());

            let (cur_sent, cur_recv) = (s.sent, s.recv);

            let (d_sent, d_recv) = match self.conns.get(&key) {
                Some((ps, pr, _)) => (
                    cur_sent.saturating_sub(*ps),
                    cur_recv.saturating_sub(*pr),
                ),
                None => (0, 0),
            };
            self.conns.insert(key.clone(), (cur_sent, cur_recv, now));
            self.misses.remove(&key);

            let local_entity = endpoints
                .get(&format!("{}:{}", s.lip, s.lport))
                .map(|e| e.entity.clone())
                .unwrap_or(Entity::Root);
            let remote_entity = resolve_remote(s, &endpoints, ip_refs);

            let (a, b) = (&local_entity, &remote_entity);
            if a.id() == b.id() {
                continue;
            }

            entity_refs.insert(a.id(), a.clone());
            entity_refs.insert(b.id(), b.clone());

            let (x, y) = if a.id() < b.id() {
                (a.id(), b.id())
            } else {
                (b.id(), a.id())
            };
            let acc = edges.entry((x.clone(), y.clone())).or_insert(EdgeAcc {
                tx_ab: 0.0,
                rx_ba: 0.0,
                conns: HashSet::new(),
                ports: Vec::new(),
            });

            let tx = d_sent as f64 / dt;
            let rx = d_recv as f64 / dt;
            if a.id() == x {
                acc.tx_ab += tx;
                acc.rx_ba += rx;
            } else {
                acc.tx_ab += rx;
                acc.rx_ba += tx;
            }
            acc.conns.insert(canonical_conn(s));
            if !acc.ports.contains(&s.rport) {
                acc.ports.push(s.rport);
            }
            if !acc.ports.contains(&s.lport) && acc.ports.len() < MAX_PORTS_PER_EDGE * 2 {
                acc.ports.push(s.lport);
            }
        }

        self.conns.retain(|k, _| {
            self.misses.get(k).copied().unwrap_or(0) < GC_TICKS
                || seen_keys.contains(k)
        });
        for k in self.conns.keys() {
            if !seen_keys.contains(k) {
                *self.misses.entry(k.clone()).or_insert(0) += 1;
            }
        }

        let mut nodes: HashMap<String, (Entity, MapNode)> = HashMap::new();
        let mut out_edges: Vec<MapEdge> = Vec::new();

        for ((x, y), acc) in edges {
            let (tx_ab, rx_ba, conns) = (acc.tx_ab, acc.rx_ba, acc.conns.len() as u32);
            let mut ports = acc.ports;
            ports.sort();
            ports.truncate(MAX_PORTS_PER_EDGE);
            let rate = tx_ab + rx_ba;
            out_edges.push(MapEdge {
                id: format!("e:{}|{}", x, y),
                source: x.clone(),
                target: y.clone(),
                rate_bps: rate,
                tx_bps: tx_ab,
                rx_bps: rx_ba,
                conn_count: conns,
                ports,
            });

            for (eid, inflow_ab, inflow_ba) in [
                (x.clone(), tx_ab, rx_ba),
                (y.clone(), rx_ba, tx_ab),
            ] {
                let e = entity_refs.get(&eid).cloned().unwrap_or(Entity::Root);
                let entry = nodes
                    .entry(eid.clone())
                    .or_insert_with(|| {
                        let n = e.node();
                        (e.clone(), n)
                    });
                entry.1.tx_bps += inflow_ab;
                entry.1.rx_bps += inflow_ba;
                entry.1.rate_bps += inflow_ab + inflow_ba;
                entry.1.conn_count += conns;
            }
        }

        for (_key, ep) in &endpoints {
            if ep.listeners.is_empty() {
                continue;
            }
            if let Some(entry) = nodes.get_mut(&ep.entity.id()) {
                for p in &ep.listeners {
                    if !entry.1.listeners.contains(p) {
                        entry.1.listeners.push(*p);
                    }
                }
            } else {
                let e = ep.entity.clone();
                let mut n = e.node();
                n.listeners = ep.listeners.clone();
                nodes.insert(ep.entity.id(), (e, n));
            }
        }

        if !relay_ports.is_empty() {
            let mut sorted: Vec<u16> = relay_ports.into_iter().collect();
            sorted.sort();
            sorted.truncate(12);
            if let Some(entry) = nodes.get_mut("root") {
                entry.1.detail = Some(
                    sorted
                        .iter()
                        .map(|p| format!(":{}", p))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }

        let mut touch: HashMap<String, (Entity, Vec<u16>)> = entity_refs
            .iter()
            .map(|(k, v)| (k.clone(), (v.clone(), Vec::new())))
            .collect();
        for ep in endpoints.values() {
            if ep.listeners.is_empty() {
                continue;
            }
            let id = ep.entity.id();
            let tm = touch
                .entry(id)
                .or_insert_with(|| (ep.entity.clone(), Vec::new()));
            for p in &ep.listeners {
                if !tm.1.contains(p) {
                    tm.1.push(*p);
                }
            }
        }

        let cutoff = now.checked_sub(LINGER).unwrap_or(now);
        self.entities_retain(&touch, cutoff, now);

        for (id, cached) in &self.cache {
            if nodes.contains_key(id) {
                continue;
            }
            let mut n = cached.entity.node();
            n.listeners = cached.listeners.clone();
            nodes.insert(id.clone(), (cached.entity.clone(), n));
        }

        let dropped = cap_external(&mut nodes, &mut out_edges);
        for id in dropped {
            self.cache.remove(&id);
        }

        for e in &ip_refs.alive {
            let id = e.id();
            if !nodes.contains_key(&id) {
                nodes.insert(id, (e.clone(), e.node()));
            }
        }

        NetworkMapPayload {
            timestamp: chrono::Utc::now().timestamp_millis() as f64,
            nodes: nodes.into_values().map(|(_, n)| n).collect(),
            edges: out_edges,
        }
    }

    fn entities_retain(
        &mut self,
        touch: &HashMap<String, (Entity, Vec<u16>)>,
        cutoff: Instant,
        now: Instant,
    ) {
        self.cache.retain(|id, c| match touch.get(id) {
            Some((e, ls)) => {
                c.entity = e.clone();
                c.listeners = ls.clone();
                c.last_seen = now;
                true
            }
            None => c.last_seen > cutoff,
        });
    }
}

struct EdgeAcc {
    tx_ab: f64,
    rx_ba: f64,
    conns: HashSet<String>,
    ports: Vec<u16>,
}

fn canonical_conn(s: &RawSock) -> String {
    let a = format!("{}:{}", s.lip, s.lport);
    let b = format!("{}:{}", s.rip, s.rport);
    if a < b {
        format!("{}|{}", a, b)
    } else {
        format!("{}|{}", b, a)
    }
}

pub struct IpRefs {
    pub(crate) containers: HashMap<String, Entity>,
    pub(crate) pods: HashMap<String, Entity>,
    pub(crate) published: HashMap<u16, Entity>,
    pub(crate) alive: Vec<Entity>,
}

pub async fn gather_ip_refs(snapshots: &SnapshotsMap) -> IpRefs {
    let snap = snapshots.read().await;
    let mut containers = HashMap::new();
    let mut pods = HashMap::new();
    let mut published = HashMap::new();
    let mut alive = Vec::new();

    for kind in [RuntimeKind::Docker, RuntimeKind::Podman] {
        if let Some(s) = snap.get(&kind) {
            for c in &s.containers {
                if c.state != "running" {
                    continue;
                }
                let node_kind = if kind == RuntimeKind::Docker {
                    MapNodeKind::Docker
                } else {
                    MapNodeKind::Podman
                };
                if let Some(ip) = &c.ip {
                    if !ip.is_empty() {
                        let e = Entity::Ctr {
                            kind: node_kind,
                            id: c.id.clone(),
                            label: c.name.clone(),
                            detail: Some(c.image.clone()),
                        };
                        containers.insert(ip.clone(), e.clone());
                        if !alive
                            .iter()
                            .any(|x: &Entity| x.id() == e.id())
                        {
                            alive.push(e);
                        }
                    }
                }
                let e_ports = Entity::Ctr {
                    kind: node_kind,
                    id: c.id.clone(),
                    label: c.name.clone(),
                    detail: Some(c.image.clone()),
                };
                if !alive.iter().any(|x| x.id() == e_ports.id()) {
                    alive.push(e_ports.clone());
                }
                for p in &c.ports {
                    if let Some(hp) = p.host_port {
                        published.entry(hp).or_insert(e_ports.clone());
                    }
                }
            }
        }
    }

    if let Some(s) = snap.get(&RuntimeKind::Kubernetes) {
        for p in &s.pods {
            if p.phase.to_lowercase() == "running" {
                let id = format!("{}/{}", p.namespace, p.name);
                alive.push(Entity::Pod { id: id.clone(), label: id.clone() });
                if !p.pod_ip.is_empty() {
                    pods.insert(
                        p.pod_ip.clone(),
                        Entity::Pod { id, label: format!("{}/{}", p.namespace, p.name) },
                    );
                }
            }
        }
    }

    if let Some(s) = snap.get(&RuntimeKind::Vm) {
        for v in &s.vms {
            if v.state.to_lowercase().contains("running") {
                alive.push(Entity::Vm {
                    id: v.name.clone(),
                    label: v.name.clone(),
                    detail: Some(v.hypervisor.clone()),
                });
            }
        }
    }

    IpRefs { containers, pods, published, alive }
}

fn resolve_local(s: &RawSock, ip_refs: &IpRefs) -> Option<Entity> {
    if let Some(pid) = s.pid {
        let label = s
            .label
            .clone()
            .unwrap_or_else(|| format!("pid {}", pid));
        return Some(Entity::Proc {
            pid,
            label,
            detail: proc_detail(pid),
        });
    }
    ip_refs.published.get(&s.lport).cloned()
}

fn resolve_remote(
    s: &RawSock,
    endpoints: &HashMap<String, Endpoint>,
    ip_refs: &IpRefs,
) -> Entity {
    let remote_key = format!("{}:{}", s.rip, s.rport);
    if let Some(ep) = endpoints.get(&remote_key) {
        if !is_local_ip(&s.rip) {
            return ep.entity.clone();
        }
        if let Some(e) = ip_refs.published.get(&s.rport) {
            return e.clone();
        }
        return ep.entity.clone();
    }
    if is_local_ip(&s.rip) {
        if let Some(e) = ip_refs.published.get(&s.rport) {
            return e.clone();
        }
    }
    if let Some(e) = ip_refs.containers.get(&s.rip) {
        return e.clone();
    }
    if let Some(e) = ip_refs.pods.get(&s.rip) {
        return e.clone();
    }
    if let Some(pid) = s.pid {
        if is_local_ip(&s.rip) {
            let label = s
                .label
                .clone()
                .unwrap_or_else(|| format!("pid {}", pid));
            return Entity::Proc {
                pid,
                label,
                detail: proc_detail(pid),
            };
        }
    }
    if is_local_ip(&s.rip) {
        return Entity::Root;
    }
    Entity::Ext {
        ip: s.rip.clone(),
        port: s.rport,
    }
}

fn is_local_ip(ip: &str) -> bool {
    ip.starts_with("127.")
        || ip == "::1"
        || ip == "0.0.0.0"
        || ip.starts_with("169.254.")
}

fn proc_detail(pid: u32) -> Option<String> {
    let cmdline = std::fs::read_to_string(format!("/proc/{}/cmdline", pid)).ok()?;
    let trimmed: String = cmdline
        .split('\0')
        .filter(|a| !a.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    if trimmed.is_empty() {
        None
    } else if trimmed.len() > 90 {
        Some(format!("{}…", &trimmed[..90]))
    } else {
        Some(trimmed)
    }
}

fn cap_external(
    nodes: &mut HashMap<String, (Entity, MapNode)>,
    edges: &mut Vec<MapEdge>,
) -> Vec<String> {
    let mut ext_ids: Vec<(String, f64)> = nodes
        .iter()
        .filter(|(_, (e, _))| matches!(e, Entity::Ext { .. }))
        .map(|(id, (_, n))| (id.clone(), n.rate_bps))
        .collect();
    if ext_ids.len() <= EXT_CAP {
        return Vec::new();
    }
    ext_ids.sort_by(|a, b| b.1.total_cmp(&a.1));
    let keep: HashSet<String> = ext_ids
        .into_iter()
        .take(EXT_CAP)
        .map(|(id, _)| id)
        .collect();

    let others_id = "ext:others".to_string();
    let dropped: Vec<String> = nodes
        .keys()
        .filter(|id| {
            id.starts_with("ext:")
                && **id != others_id
                && !keep.contains(*id)
        })
        .cloned()
        .collect();

    if let Some((_, n)) = nodes.get_mut(&others_id) {
        n.label = "external (merged)".to_string();
    } else {
        let e = Entity::Ext {
            ip: "multiple".to_string(),
            port: 0,
        };
        let mut n = e.node();
        n.id = others_id.clone();
        n.label = "external (merged)".to_string();
        nodes.insert(others_id.clone(), (e, n));
    }

    for edge in edges.iter_mut() {
        if dropped.contains(&edge.source) {
            edge.source = others_id.clone();
        }
        if dropped.contains(&edge.target) {
            edge.target = others_id.clone();
        }
    }
    edges.retain(|e| e.source != e.target);

    for id in &dropped {
        nodes.remove(id);
    }
    dropped
}

pub async fn start_network_map_poll(
    app: AppHandle,
    snapshots: SnapshotsMap,
    cache: Arc<RwLock<NetworkMapPayload>>,
    interval: Duration,
) {
    let mut collector = Collector::new();
    loop {
        tokio::time::sleep(interval).await;
        let ip_refs = gather_ip_refs(&snapshots).await;
        let payload = collector.collect(&ip_refs);
        *cache.write().await = payload.clone();
        let _ = app.emit("network-map-tick", &payload);
    }
}
