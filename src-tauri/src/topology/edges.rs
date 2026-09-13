use crate::types::TopologyEdge;

pub fn make_network_container_edge(network_id: &str, container_id: &str) -> TopologyEdge {
    TopologyEdge {
        id: format!("edge:net:{}->ctr:{}", network_id, container_id),
        source: format!("net:{}", network_id),
        target: format!("ctr:{}", container_id),
        label: Some("network".to_string()),
        traffic_bps: 0.0,
    }
}

pub fn make_host_port_edge(host_port: u16, container_id: &str, container_port: u16) -> TopologyEdge {
    TopologyEdge {
        id: format!("edge:host:{}->ctr:{}:{}", host_port, container_id, container_port),
        source: "host".to_string(),
        target: format!("ctr:{}", container_id),
        label: Some(format!("{}:{}", host_port, container_port)),
        traffic_bps: 0.0,
    }
}
