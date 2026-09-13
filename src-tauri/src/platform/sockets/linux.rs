use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::{ConnRecord, SockProto};

/// Linux backend: `/proc/net/{tcp,tcp6,udp,udp6}` for the socket table,
/// `/proc/<pid>/fd` for inode->PID attribution, and `ss -tinH` for
/// per-connection cumulative byte counters.
pub fn connections() -> Vec<ConnRecord> {
    let inode_map = scan_pids();
    let stats = read_ss_stats();
    let mut out = Vec::new();
    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        read_proc(path, SockProto::Tcp, &inode_map, &stats, &mut out);
    }
    for path in ["/proc/net/udp", "/proc/net/udp6"] {
        read_proc(path, SockProto::Udp, &inode_map, &stats, &mut out);
    }
    out
}

fn read_proc(
    path: &str,
    proto: SockProto,
    inode_map: &HashMap<String, (u32, String)>,
    stats: &HashMap<String, (u64, u64)>,
    out: &mut Vec<ConnRecord>,
) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    for line in content.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            continue;
        }
        let (Some((lip, lport)), Some((rip, rport))) = (parse_addr(f[1]), parse_addr(f[2])) else {
            continue;
        };
        let state_hex = f[3];
        let inode = f[9].to_string();

        let listening = match proto {
            SockProto::Tcp => state_hex == "0A",
            SockProto::Udp => rport == 0,
        };
        let established = proto == SockProto::Tcp && state_hex == "01";

        let (pid, label) = match inode_map.get(&inode) {
            Some((p, l)) => (Some(*p), Some(l.clone())),
            None => (None, None),
        };

        let (rx, tx) = if proto == SockProto::Tcp {
            let key = format!("{}:{}|{}:{}", ip_str(lip), lport, ip_str(rip), rport);
            match stats.get(&key) {
                Some((sent, recv)) => (Some(*recv), Some(*sent)),
                None => (None, None),
            }
        } else {
            (None, None)
        };

        out.push(ConnRecord {
            pid,
            label,
            proto,
            local_ip: lip,
            local_port: lport,
            remote_ip: (rport != 0).then_some(rip),
            remote_port: rport,
            listening,
            established,
            rx_bytes: rx,
            tx_bytes: tx,
        });
    }
}

fn ip_str(ip: IpAddr) -> String {
    match ip {
        IpAddr::V6(v6) => v6.to_canonical().to_string(),
        IpAddr::V4(v4) => v4.to_string(),
    }
}

fn parse_addr(raw: &str) -> Option<(IpAddr, u16)> {
    let (ip_hex, port_hex) = raw.split_once(':')?;
    let port = u16::from_str_radix(port_hex, 16).ok()?;
    let ip = match ip_hex.len() {
        8 => {
            let b = hex_bytes(ip_hex)?;
            IpAddr::V4(Ipv4Addr::new(b[3], b[2], b[1], b[0]))
        }
        32 => {
            let mut bytes = [0u8; 16];
            let hb = hex_bytes(ip_hex)?;
            for w in 0..4 {
                for i in 0..4 {
                    bytes[w * 4 + i] = hb[w * 4 + (3 - i)];
                }
            }
            IpAddr::V6(Ipv6Addr::from(bytes))
        }
        _ => return None,
    };
    Some((ip, port))
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Maps socket inodes to `(pid, comm)` by scanning `/proc/<pid>/fd`.
fn scan_pids() -> HashMap<String, (u32, String)> {
    let mut inode_map: HashMap<String, (u32, String)> = HashMap::new();
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return inode_map;
    };
    for entry in procs.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let comm = std::fs::read_to_string(format!("/proc/{}/comm", pid))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let label = if comm.is_empty() {
            format!("pid {}", pid)
        } else {
            comm
        };
        let Ok(fds) = std::fs::read_dir(format!("/proc/{}/fd", pid)) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(link) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let link = link.to_string_lossy().to_string();
            if let Some(rest) = link.strip_prefix("socket:[") {
                let inode = rest.trim_end_matches(']').to_string();
                inode_map.entry(inode).or_insert((pid, label.clone()));
            }
        }
    }
    inode_map
}

fn parse_ss_addr(raw: &str) -> Option<(IpAddr, u16)> {
    let (ip_part, port) = raw.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    let ip_part = ip_part.trim_matches(|c| c == '[' || c == ']');
    if let Ok(v4) = ip_part.parse::<Ipv4Addr>() {
        return Some((IpAddr::V4(v4), port));
    }
    if let Ok(v6) = ip_part.parse::<Ipv6Addr>() {
        return Some((v6.to_canonical(), port));
    }
    Some((ip_part.parse().ok()?, port))
}

/// Parses `ss -tinH` into `"lip:lport|rip:rport" -> (bytes_sent, bytes_recv)`.
fn read_ss_stats() -> HashMap<String, (u64, u64)> {
    let mut map: HashMap<String, (u64, u64)> = HashMap::new();
    let Ok(output) = std::process::Command::new("ss").args(["-tinH"]).output() else {
        return map;
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current: Option<String> = None;

    for line in stdout.lines() {
        if line.starts_with(char::is_whitespace) {
            let Some(ref key) = current else { continue };
            let (mut sent, mut recv) = (0u64, 0u64);
            for tok in line.split_whitespace() {
                if let Some(v) = tok.strip_prefix("bytes_sent:") {
                    sent = v.parse().unwrap_or(0);
                } else if let Some(v) = tok.strip_prefix("bytes_received:") {
                    recv = v.parse().unwrap_or(0);
                } else if let Some(v) = tok.strip_prefix("bytes_acked:") {
                    if sent == 0 {
                        sent = v.parse().unwrap_or(0);
                    }
                }
            }
            if sent > 0 || recv > 0 {
                map.insert(key.clone(), (sent, recv));
            }
        } else {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() >= 5 {
                if let (Some((lip, lport)), Some((rip, rport))) =
                    (parse_ss_addr(f[3]), parse_ss_addr(f[4]))
                {
                    current = Some(format!(
                        "{}:{}|{}:{}",
                        ip_str(lip),
                        lport,
                        ip_str(rip),
                        rport
                    ));
                }
            }
        }
    }
    map
}
