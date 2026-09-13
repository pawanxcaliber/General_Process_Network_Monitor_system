use std::collections::HashMap;
use std::net::IpAddr;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SockProto {
    Tcp,
    Udp,
}

/// A single socket/connection with optional owning process and cumulative
/// byte counters (only available where the OS exposes them).
#[derive(Debug, Clone)]
pub struct ConnRecord {
    pub pid: Option<u32>,
    pub label: Option<String>,
    pub proto: SockProto,
    pub local_ip: IpAddr,
    pub local_port: u16,
    pub remote_ip: Option<IpAddr>,
    pub remote_port: u16,
    pub listening: bool,
    pub established: bool,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
}

/// All TCP/UDP sockets visible to the current user.
pub fn connections() -> Vec<ConnRecord> {
    #[cfg(target_os = "linux")]
    {
        linux::connections()
    }
    #[cfg(target_os = "macos")]
    {
        macos::connections()
    }
    #[cfg(target_os = "windows")]
    {
        windows::connections()
    }
}

/// Cumulative bytes received/sent per PID. Derived from per-connection
/// counters where available; macOS uses `nettop`.
pub fn per_process_net() -> HashMap<u32, (u64, u64)> {
    #[cfg(target_os = "macos")]
    {
        if let Some(m) = macos::nettop() {
            return m;
        }
    }

    let mut map: HashMap<u32, (u64, u64)> = HashMap::new();
    for c in connections() {
        if let Some(pid) = c.pid {
            let e = map.entry(pid).or_insert((0, 0));
            e.0 += c.rx_bytes.unwrap_or(0);
            e.1 += c.tx_bytes.unwrap_or(0);
        }
    }
    map
}
