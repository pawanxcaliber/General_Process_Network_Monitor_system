use std::collections::HashMap;
use std::time::Instant;

pub use crate::types::RuntimeKind;
use crate::types::RuntimeHistory;

pub const HISTORY_CAP: usize = 60;

pub fn push_capped(v: &mut Vec<f64>, x: f64) {
    if v.len() >= HISTORY_CAP {
        v.remove(0);
    }
    v.push(x);
}

pub struct RuntimeState {
    pub kind: RuntimeKind,
    pub detail: Option<String>,
    pub ok: bool,
    pub docker: Option<bollard::Docker>,
    pub k8s: Option<crate::engine::kubernetes::K8sData>,
    pub net_totals: HashMap<String, (u64, u64, Instant)>,
    pub cpu_hist: HashMap<String, Vec<f64>>,
    pub mem_hist: HashMap<String, Vec<f64>>,
    pub agg: RuntimeHistory,
}

impl RuntimeState {
    pub fn new(kind: RuntimeKind) -> Self {
        Self {
            kind,
            detail: None,
            ok: false,
            docker: None,
            k8s: None,
            net_totals: HashMap::new(),
            cpu_hist: HashMap::new(),
            mem_hist: HashMap::new(),
            agg: RuntimeHistory::default(),
        }
    }

    pub fn push_agg(&mut self, cpu: f64, mem: f64, rx: f64, tx: f64) {
        push_capped(&mut self.agg.cpu, cpu);
        push_capped(&mut self.agg.mem_bytes, mem);
        push_capped(&mut self.agg.net_rx_bps, rx);
        push_capped(&mut self.agg.net_tx_bps, tx);
    }
}

pub mod detect;
pub mod docker;
pub mod kubernetes;
pub mod logs;
pub mod vm;
