use std::net::{IpAddr, Ipv4Addr};

use super::{ConnRecord, SockProto};

/// Windows backend: `netstat -ano` for the TCP/UDP tables with owning PID.
/// Cumulative byte counters aren't exposed by netstat, so rates fall back to
/// zero (connection topology is still complete).
pub fn connections() -> Vec<ConnRecord> {
    let mut cmd = std::process::Command::new("netstat");
    let Ok(out) = crate::platform::exec::set_no_window(&mut cmd)
        .args(["-ano"])
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().filter_map(parse_netstat).collect()
}

fn parse_netstat(line: &str) -> Option<ConnRecord> {
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() < 4 {
        return None;
    }
    let proto = match f[0] {
        "TCP" | "TCPv6" => SockProto::Tcp,
        "UDP" | "UDPv6" => SockProto::Udp,
        _ => return None,
    };

    let (local_ip, local_port) = parse_win_addr(f[1])?;
    let (remote_ip, remote_port) = parse_win_addr(f[2])?;

    let (listening, established, pid) = match proto {
        SockProto::Tcp => {
            if f.len() < 5 {
                return None;
            }
            let state = f[3];
            (
                state == "LISTENING",
                state == "ESTABLISHED",
                f[4].parse::<u32>().ok(),
            )
        }
        SockProto::Udp => (true, false, f[3].parse::<u32>().ok()),
    };

    Some(ConnRecord {
        pid,
        label: None,
        proto,
        local_ip,
        local_port,
        remote_ip: (remote_port != 0).then_some(remote_ip),
        remote_port,
        listening,
        established,
        rx_bytes: None,
        tx_bytes: None,
    })
}

/// Parses `addr:port`, `[v6]:port`, or `*:*`.
fn parse_win_addr(s: &str) -> Option<(IpAddr, u16)> {
    if let Some(rest) = s.strip_prefix('[') {
        let end = rest.find(']')?;
        let ip_part = &rest[..end];
        let port = rest[end + 1..].strip_prefix(':')?.parse::<u16>().ok()?;
        return Some((ip_part.parse::<IpAddr>().ok()?, port));
    }
    let (host, port) = s.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    if host == "*" {
        return Some((IpAddr::V4(Ipv4Addr::UNSPECIFIED), port));
    }
    Some((host.parse::<IpAddr>().ok()?, port))
}
