use std::collections::HashMap;

/// Per-process network rates, derived from per-socket TCP byte counters
/// (`ss -tinoeH`) attributed to PIDs via `/proc/<pid>/fd` socket inodes.
pub struct ProcNetState {
    prev: HashMap<u64, (u64, u64)>,
}

pub struct ProcNetRates {
    pub rx_bps: HashMap<u32, f64>,
    pub tx_bps: HashMap<u32, f64>,
}

impl ProcNetState {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
        }
    }
}

pub async fn sample(state: &mut ProcNetState, elapsed: f64) -> ProcNetRates {
    let sockets = read_ss().await;
    let ino_pid = tokio::task::spawn_blocking(scan_inode_pids_blocking)
        .await
        .unwrap_or_default();

    let mut cur_ino: HashMap<u64, (u64, u64)> = HashMap::new();
    let mut rx: HashMap<u32, f64> = HashMap::new();
    let mut tx: HashMap<u32, f64> = HashMap::new();

    for (ino, sent, recv) in &sockets {
        cur_ino.insert(*ino, (*sent, *recv));
        let (prev_sent, prev_recv) = state
            .prev
            .get(ino)
            .copied()
            .unwrap_or((*sent, *recv));
        let d_sent = sent.saturating_sub(prev_sent) as f64 / elapsed;
        let d_recv = recv.saturating_sub(prev_recv) as f64 / elapsed;
        if let Some(pid) = ino_pid.get(ino) {
            *rx.entry(*pid).or_insert(0.0) += d_recv;
            *tx.entry(*pid).or_insert(0.0) += d_sent;
        }
    }

    state.prev = cur_ino;
    ProcNetRates { rx_bps: rx, tx_bps: tx }
}

async fn read_ss() -> Vec<(u64, u64, u64)> {
    let out = match tokio::process::Command::new("ss")
        .args(["-tinoeH"])
        .output()
        .await
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };
    parse_ss(&String::from_utf8_lossy(&out))
}

/// Parses `ss -tinoeH` output into `(socket_inode, bytes_sent, bytes_received)`.
fn parse_ss(text: &str) -> Vec<(u64, u64, u64)> {
    let mut res = Vec::new();
    let mut cur_ino: Option<u64> = None;
    let mut cur_sent: Option<u64> = None;
    let mut cur_recv: Option<u64> = None;

    for line in text.lines() {
        if !line.starts_with(' ') {
            flush_record(&mut res, &mut cur_ino, &mut cur_sent, &mut cur_recv);
        }
        for tok in line.split_whitespace() {
            if let Some(v) = tok.strip_prefix("ino:") {
                cur_ino = v.parse().ok();
            } else if let Some(v) = tok.strip_prefix("bytes_sent:") {
                cur_sent = v.parse().ok();
            } else if let Some(v) = tok.strip_prefix("bytes_received:") {
                cur_recv = v.parse().ok();
            }
        }
    }
    flush_record(&mut res, &mut cur_ino, &mut cur_sent, &mut cur_recv);
    res
}

fn flush_record(
    res: &mut Vec<(u64, u64, u64)>,
    ino: &mut Option<u64>,
    sent: &mut Option<u64>,
    recv: &mut Option<u64>,
) {
    if let (Some(i), Some(s), Some(r)) = (*ino, *sent, *recv) {
        res.push((i, s, r));
    }
    *ino = None;
    *sent = None;
    *recv = None;
}

/// Maps socket inodes to owning PIDs by scanning `/proc/<pid>/fd` symlinks.
/// Only processes owned by the current user are visible without root.
fn scan_inode_pids_blocking() -> HashMap<u64, u32> {
    let mut map = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return map;
    };
    for e in entries.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let mut fd_path = e.path();
        fd_path.push("fd");
        let Ok(fds) = std::fs::read_dir(&fd_path) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(link) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let s = link.to_string_lossy();
            if let Some(rest) = s.strip_prefix("socket:[") {
                if let Some(ino) = rest.strip_suffix(']').and_then(|v| v.parse::<u64>().ok()) {
                    map.insert(ino, pid);
                }
            }
        }
    }
    map
}
