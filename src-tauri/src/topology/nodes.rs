use crate::types::{TopologyNode, NodeKind, NodeStatus, NodeMetrics};

pub fn make_network_node(id: &str, name: &str) -> TopologyNode {
    TopologyNode {
        id: format!("net:{}", id),
        name: name.to_string(),
        kind: NodeKind::Network,
        parent_id: None,
        status: NodeStatus::Running,
        metrics: NodeMetrics {
            cpu_percent: 0.0,
            memory_bytes: 0,
            rx_rate_bps: 0.0,
            tx_rate_bps: 0.0,
        },
        ports: Vec::new(),
    }
}

pub fn make_container_node(
    id: &str,
    name: &str,
    parent_id: Option<String>,
    status: NodeStatus,
    ports: Vec<crate::types::PortMapping>,
) -> TopologyNode {
    TopologyNode {
        id: format!("ctr:{}", id),
        name: name.to_string(),
        kind: NodeKind::Container,
        parent_id: parent_id.map(|p| format!("net:{}", p)),
        status,
        metrics: NodeMetrics {
            cpu_percent: 0.0,
            memory_bytes: 0,
            rx_rate_bps: 0.0,
            tx_rate_bps: 0.0,
        },
        ports,
    }
}

pub fn map_container_status(state: &str) -> NodeStatus {
    match state.to_lowercase().as_str() {
        "running" => NodeStatus::Running,
        "paused" => NodeStatus::Paused,
        "exited" | "dead" | "created" => NodeStatus::Exited,
        "restarting" | "removing" => NodeStatus::Restarting,
        _ => NodeStatus::Exited,
    }
}
