use crate::types::PortInfo;
use std::collections::HashMap;

pub fn listening_ports() -> Vec<PortInfo> {
    let mut map: HashMap<(u16, String), PortInfo> = HashMap::new();

    #[cfg(unix)]
    {
        collect_ss(&mut map, "t", "tcp");
        collect_ss(&mut map, "u", "udp");
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("netstat").args(["-an"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }
                let proto = match parts[0] {
                    "TCP" => "tcp",
                    "UDP" => "udp",
                    _ => continue,
                };
                let is_listener = if proto == "tcp" {
                    parts.len() >= 4 && parts[3] == "LISTENING"
                } else {
                    parts.len() >= 2
                };
                if !is_listener {
                    continue;
                }
                if let Some(port) = parts[1].rsplit(':').next().and_then(|p| p.parse::<u16>().ok()) {
                    if port > 0 {
                        let key = (port, proto.to_string());
                        map.entry(key).or_insert(PortInfo {
                            port,
                            proto: proto.to_string(),
                            process: None,
                        });
                    }
                }
            }
        }
    }

    let mut ports: Vec<PortInfo> = map.into_values().collect();
    ports.sort_by(|a, b| a.port.cmp(&b.port).then(a.proto.cmp(&b.proto)));
    ports
}

#[cfg(unix)]
fn collect_ss(map: &mut HashMap<(u16, String), PortInfo>, flag: &str, proto: &str) {
    let Ok(output) = std::process::Command::new("ss")
        .arg(format!("-{}lnpH", flag))
        .output()
    else {
        return;
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let Some(port) = parts[3]
            .rsplit(':')
            .next()
            .and_then(|p| p.parse::<u16>().ok())
        else {
            continue;
        };
        if port == 0 {
            continue;
        }
        let process = extract_process(line);
        let key = (port, proto.to_string());
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
                        port,
                        proto: proto.to_string(),
                        process,
                    },
                );
            }
        }
    }
}

#[cfg(unix)]
fn extract_process(line: &str) -> Option<String> {
    let start = line.find("users:((\"")?;
    let rest = &line[start + 9..];
    let end = rest.find('"')?;
    if end == 0 {
        return None;
    }
    Some(rest[..end].to_string())
}

#[cfg(not(unix))]
fn extract_process(_line: &str) -> Option<String> {
    None
}
