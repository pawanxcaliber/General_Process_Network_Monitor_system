use std::collections::HashMap;

/// Per-process network rates, derived from cumulative per-PID byte counters
/// (Linux/macOS; Windows falls back to zero).
pub struct ProcNetState {
    prev: HashMap<u32, (u64, u64)>,
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
    let cur = tokio::task::spawn_blocking(crate::platform::sockets::per_process_net)
        .await
        .unwrap_or_default();

    let mut rx: HashMap<u32, f64> = HashMap::new();
    let mut tx: HashMap<u32, f64> = HashMap::new();
    for (pid, (recv, sent)) in &cur {
        let (prev_recv, prev_sent) = state.prev.get(pid).copied().unwrap_or((*recv, *sent));
        rx.insert(*pid, recv.saturating_sub(prev_recv) as f64 / elapsed);
        tx.insert(*pid, sent.saturating_sub(prev_sent) as f64 / elapsed);
    }

    state.prev = cur;
    ProcNetRates {
        rx_bps: rx,
        tx_bps: tx,
    }
}
