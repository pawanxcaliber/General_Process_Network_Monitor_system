use crate::types::PortInfo;
use std::collections::HashMap;

/// Listening TCP/UDP ports with the owning process, cross-platform.
pub fn listening_ports() -> Vec<PortInfo> {
    let mut map: HashMap<(u16, String), PortInfo> = HashMap::new();

    for c in crate::platform::sockets::connections() {
        if !c.listening || c.local_port == 0 {
            continue;
        }
        let proto = match c.proto {
            crate::platform::sockets::SockProto::Tcp => "tcp",
            crate::platform::sockets::SockProto::Udp => "udp",
        };
        let process = c
            .label
            .clone()
            .or_else(|| c.pid.map(|p| format!("pid {}", p)));
        let key = (c.local_port, proto.to_string());
        match map.get_mut(&key) {
            Some(existing) => {
                if existing.process.is_none() {
                    existing.process = process;
                }
            }
            None => {
                map.insert(
                    key,
                    PortInfo {
                        port: c.local_port,
                        proto: proto.to_string(),
                        process,
                    },
                );
            }
        }
    }

    let mut ports: Vec<PortInfo> = map.into_values().collect();
    ports.sort_by(|a, b| a.port.cmp(&b.port).then(a.proto.cmp(&b.proto)));
    ports
}
