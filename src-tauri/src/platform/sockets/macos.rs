use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

use super::{ConnRecord, SockProto};

/// macOS backend: `lsof -nP -iTCP -iUDP` for the socket table (with owning
/// PID), and `nettop` for per-process cumulative byte counters.
pub fn connections() -> Vec<ConnRecord> {
    let Ok(out) = std::process::Command::new("lsof")
        .args(["-nP", "-iTCP", "-iUDP"])
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().skip(1).filter_map(parse_lsof).collect()
}

fn parse_lsof(line: &str) -> Option<ConnRecord> {
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() < 9 {
        return None;
    }
    let label = f[0].to_string();
    let pid = f[1].parse::<u32>().ok();

    let idx = f.iter().position(|t| *t == "TCP" || *t == "UDP")?;
    let proto = if f[idx] == "TCP" {
        SockProto::Tcp
    } else {
        SockProto::Udp
    };
    let rest = f[idx + 1..].join(" ");
    let listening = rest.contains("(LISTEN)");
    let established = rest.contains("(ESTABLISHED)");

    let addr_part = match rest.rfind('(') {
        Some(p) => rest[..p].trim(),
        None => rest.trim(),
    };
    let (local_s, remote_s) = match addr_part.split_once("->") {
        Some((l, r)) => (l, Some(r)),
        None => (addr_part, None),
    };
    let (local_ip, local_port) = parse_lsof_addr(local_s)?;
    let (remote_ip, remote_port) = remote_s
        .and_then(parse_lsof_addr)
        .map(|(ip, p)| (Some(ip), p))
        .unwrap_or((None, 0));

    Some(ConnRecord {
        pid,
        label: Some(label),
        proto,
        local_ip,
        local_port,
        remote_ip,
        remote_port,
        listening,
        established,
        rx_bytes: None,
        tx_bytes: None,
    })
}

/// Parses `host:port`, `[v6]:port`, `*:port`, or `v6%iface:port`.
fn parse_lsof_addr(s: &str) -> Option<(IpAddr, u16)> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('[') {
        // [addr]:port
        let end = rest.find(']')?;
        let ip_part = &rest[..end];
        let port = rest[end + 1..].strip_prefix(':')?.parse::<u16>().ok()?;
        return Some((parse_ip(ip_part)?, port));
    }
    let (host, port) = s.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    if host == "*" {
        return Some((IpAddr::V4(Ipv4Addr::UNSPECIFIED), port));
    }
    Some((parse_ip(host)?, port))
}

fn parse_ip(s: &str) -> Option<IpAddr> {
    // Strip a zone/interface suffix (fe80::1%en0).
    let s = s.split('%').next().unwrap_or(s);
    s.parse::<IpAddr>().ok()
}

/// Best-effort per-process cumulative bytes via `nettop`.
/// Returns `None` if nettop is unavailable or its output isn't recognised,
/// so callers fall back to connection-derived counters.
pub fn nettop() -> Option<HashMap<u32, (u64, u64)>> {
    let out = std::process::Command::new("nettop")
        .args(["-P", "-x", "-l", "1"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut map: HashMap<u32, (u64, u64)> = HashMap::new();
    let mut saw_header = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !saw_header {
            if line.contains("bytes_in") || line.starts_with("time") {
                saw_header = true;
            }
            continue;
        }
        let mut it = line.split_whitespace();
        let Some(proc_tok) = it.next() else { continue };
        let Some(pid) = proc_pid(proc_tok) else { continue };

        let nums: Vec<u64> = it.filter_map(parse_bytes).collect();
        if nums.len() < 2 {
            continue;
        }
        map.insert(pid, (nums[0], nums[1]));
    }
    (!map.is_empty()).then_some(map)
}

/// Extracts the PID from a nettop process token (`name.pid` or `pid`).
fn proc_pid(tok: &str) -> Option<u32> {
    if let Ok(p) = tok.parse::<u32>() {
        return Some(p);
    }
    tok.rsplit_once('.').and_then(|(_, p)| p.parse::<u32>().ok())
}

/// Parses a byte count that may carry a K/M/G suffix.
fn parse_bytes(tok: &str) -> Option<u64> {
    let t = tok.trim();
    if t.is_empty() {
        return None;
    }
    let (num, mult) = match t.chars().last()? {
        'K' | 'k' => (&t[..t.len() - 1], 1024u64),
        'M' | 'm' => (&t[..t.len() - 1], 1024 * 1024),
        'G' | 'g' => (&t[..t.len() - 1], 1024 * 1024 * 1024),
        c if c.is_ascii_digit() => (t, 1),
        _ => return None,
    };
    num.trim()
        .parse::<f64>()
        .ok()
        .map(|v| (v * mult as f64) as u64)
}
