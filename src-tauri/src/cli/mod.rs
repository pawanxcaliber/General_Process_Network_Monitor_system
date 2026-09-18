mod host_page;
mod net_page;
mod runtime_page;
mod ui;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use crate::engine::{logs, RuntimeKind, RuntimeState};
use crate::types::{
    HostPayload, LogLine, NetworkMapPayload, ProcessInfo, ProcessSortKey, RuntimeSnapshot,
    RuntimeStatus,
};

use crate::Backend;

type StatesMap = Arc<tokio::sync::Mutex<HashMap<RuntimeKind, Arc<tokio::sync::RwLock<RuntimeState>>>>>;

const LOG_CAP: usize = 2000;

const SORT_CYCLE: [ProcessSortKey; 9] = [
    ProcessSortKey::Cpu,
    ProcessSortKey::Mem,
    ProcessSortKey::Disk,
    ProcessSortKey::Net,
    ProcessSortKey::Gpu,
    ProcessSortKey::Igpu,
    ProcessSortKey::Dgpu,
    ProcessSortKey::Pid,
    ProcessSortKey::Name,
];

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Host,
    Runtimes,
    Net,
}

/// Terminal-side buffer for the open container/pod detail view. Shared with
/// the log-follow task via a std Mutex (locked briefly, never across awaits).
pub struct LogBuf {
    pub lines: Vec<LogLine>,
    pub live: bool,
    pub task: Option<tokio::task::JoinHandle<()>>,
}

impl Default for LogBuf {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            live: true,
            task: None,
        }
    }
}

/// Data snapshot taken once per frame from the shared backend caches, so
/// render code never touches async locks.
pub struct Model {
    pub host: HostPayload,
    pub netmap: NetworkMapPayload,
    pub snapshots: HashMap<RuntimeKind, RuntimeSnapshot>,
    pub sort: ProcessSortKey,
    pub statuses: Vec<RuntimeStatus>,
}

pub struct CliApp {
    pub backend: Backend,
    pub model: Model,
    pub page: Page,
    pub dark: bool,
    pub show_help: bool,
    pub quit: bool,
    pub proc_sel: usize,
    pub kill_confirm: Option<(u32, String)>,
    pub rt_tab: RuntimeKind,
    pub rt_sel: usize,
    pub detail: Option<(RuntimeKind, String)>,
    pub logs_buf: Arc<Mutex<LogBuf>>,
    pub net_sel: usize,
    pub net_threshold: u64,
    pub net_filter: Option<String>,
}

impl CliApp {
    fn new(backend: Backend) -> Self {
        Self {
            model: Model {
                host: HostPayload::default(),
                netmap: NetworkMapPayload::default(),
                snapshots: HashMap::new(),
                sort: ProcessSortKey::Cpu,
                statuses: vec![],
            },
            backend,
            page: Page::Host,
            dark: true,
            show_help: false,
            quit: false,
            proc_sel: 0,
            kill_confirm: None,
            rt_tab: RuntimeKind::Docker,
            rt_sel: 0,
            detail: None,
            logs_buf: Arc::new(Mutex::new(LogBuf::default())),
            net_sel: 0,
            net_threshold: 0,
            net_filter: None,
        }
    }

    pub fn snapshot(&self, kind: RuntimeKind) -> RuntimeSnapshot {
        self.model
            .snapshots
            .get(&kind)
            .cloned()
            .unwrap_or_default()
    }
    pub fn sorted_processes(&self) -> Vec<ProcessInfo> {
        let mut list = self.model.host.processes.clone();
        let key = self.model.sort;
        match key {
            ProcessSortKey::Cpu => list
                .sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal)),
            ProcessSortKey::Mem => list.sort_by_key(|x| std::cmp::Reverse(x.memory_bytes)),
            ProcessSortKey::Disk => list.sort_by(|a, b| {
                (b.disk_read_bps + b.disk_write_bps)
                    .partial_cmp(&(a.disk_read_bps + a.disk_write_bps))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            ProcessSortKey::Net => list.sort_by(|a, b| {
                (b.net_rx_bps + b.net_tx_bps)
                    .partial_cmp(&(a.net_rx_bps + a.net_tx_bps))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            ProcessSortKey::Gpu => list.sort_by(|a, b| {
                (b.gpu_igpu_bytes.unwrap_or(0) + b.gpu_dgpu_bytes.unwrap_or(0))
                    .cmp(&(a.gpu_igpu_bytes.unwrap_or(0) + a.gpu_dgpu_bytes.unwrap_or(0)))
            }),
            ProcessSortKey::Igpu => list.sort_by_key(|x| std::cmp::Reverse(x.gpu_igpu_bytes.unwrap_or(0))),
            ProcessSortKey::Dgpu => list.sort_by_key(|x| std::cmp::Reverse(x.gpu_dgpu_bytes.unwrap_or(0))),
            ProcessSortKey::Pid => list.sort_by_key(|x| x.pid),
            ProcessSortKey::Name => list.sort_by(|a, b| a.name.cmp(&b.name)),
        }
        list
    }

    pub fn selected_process(&self) -> Option<ProcessInfo> {
        self.sorted_processes().into_iter().nth(self.proc_sel)
    }

    fn cycle_tab(&mut self, dir: i32) {
        let all = [
            RuntimeKind::Docker,
            RuntimeKind::Podman,
            RuntimeKind::Kubernetes,
            RuntimeKind::Vm,
        ];
        let cur = all.iter().position(|k| *k == self.rt_tab).unwrap_or(0);
        let len = all.len() as i32;
        let next = ((cur as i32 + dir) + len) % len;
        self.rt_tab = all[next as usize];
        self.rt_sel = 0;
    }

    async fn open_detail(&mut self) {
        let kind = self.rt_tab;
        let snap = self.snapshot(kind);
        let id: String = match kind {
            RuntimeKind::Kubernetes => match snap.pods.get(self.rt_sel) {
                Some(p) => p.name.clone(),
                None => return,
            },
            RuntimeKind::Vm => match snap.vms.get(self.rt_sel) {
                Some(v) => v.name.clone(),
                None => return,
            },
            _ => match snap.containers.get(self.rt_sel) {
                Some(c) => c.id.clone(),
                None => return,
            },
        };

        self.detail = Some((kind, id.clone()));

        let mut b = self.logs_buf.lock().unwrap();
        if let Some(t) = b.task.take() {
            t.abort();
        }
        b.lines.clear();
        b.live = true;
        drop(b);

        if matches!(kind, RuntimeKind::Docker | RuntimeKind::Podman) {
            let buf = self.logs_buf.clone();
            let states = self.backend.states.clone();
            let cid = id.clone();
            let task = tokio::spawn(async move {
                let Ok(docker) = get_or_connect(&states, kind).await else {
                    return;
                };
                logs::follow_logs(docker, kind.as_str().to_string(), cid, move |line: LogLine| {
                    push_log_line(&buf, line);
                })
                .await;
            });
            self.logs_buf.lock().unwrap().task = Some(task);
        }
    }

    async fn close_detail(&mut self, key: &(RuntimeKind, String)) {
        let Some(d) = &self.detail else { return };
        if d != key {
            return;
        }
        self.detail = None;
        let mut b = self.logs_buf.lock().unwrap();
        if let Some(t) = b.task.take() {
            t.abort();
        }
        b.lines.clear();
        b.live = true;
    }

    fn cycle_net_filter(&mut self) {
        self.net_filter = match &self.net_filter {
            None => Some("docker".to_string()),
            Some(f) if f == "docker" => Some("pod".to_string()),
            Some(f) if f == "pod" => Some("external".to_string()),
            Some(_) => None,
        };
        self.net_sel = 0;
    }

    /// (src, src_group, dst, dst_group, rate, conns, ports)
    pub fn net_rows(&self) -> Vec<NetRow> {
        let labels: HashMap<String, (String, &'static str)> = self
            .model
            .netmap
            .nodes
            .iter()
            .map(|n| (n.id.clone(), (n.label.clone(), group_of(n.kind))))
            .collect();
        let mut rows: Vec<NetRow> = Vec::new();
        for e in &self.model.netmap.edges {
            if (e.rate_bps as u64) < self.net_threshold {
                continue;
            }
            let (src, src_grp) = labels
                .get(&e.source)
                .cloned()
                .unwrap_or_else(|| (e.source.clone(), "other"));
            let (dst, dst_grp) = labels
                .get(&e.target)
                .cloned()
                .unwrap_or_else(|| (e.target.clone(), "other"));
            rows.push(NetRow {
                src,
                src_grp: src_grp.to_string(),
                dst,
                dst_grp: dst_grp.to_string(),
                rate_bps: e.rate_bps,
                conns: e.conn_count,
                ports: short_ports(&e.ports),
            });
        }
        rows.sort_by(|a, b| b.rate_bps.partial_cmp(&a.rate_bps).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(f) = &self.net_filter {
            let f = f.as_str();
            rows.retain(|r| r.src_grp.contains(f) || r.dst_grp.contains(f));
        }
        rows
    }
}

pub struct NetRow {
    pub src: String,
    pub src_grp: String,
    pub dst: String,
    pub dst_grp: String,
    pub rate_bps: f64,
    pub conns: u32,
    pub ports: String,
}

fn group_of(kind: crate::types::MapNodeKind) -> &'static str {
    match kind {
        crate::types::MapNodeKind::Process => "process",
        crate::types::MapNodeKind::Docker | crate::types::MapNodeKind::Podman => "docker",
        crate::types::MapNodeKind::Pod => "pod",
        crate::types::MapNodeKind::Vm => "vm",
        crate::types::MapNodeKind::External => "external",
        crate::types::MapNodeKind::System => "system",
    }
}

fn short_ports(ports: &[u16]) -> String {
    ports
        .iter()
        .take(4)
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn statuses_from(snaps: &HashMap<RuntimeKind, RuntimeSnapshot>) -> Vec<RuntimeStatus> {
    [
        RuntimeKind::Docker,
        RuntimeKind::Podman,
        RuntimeKind::Kubernetes,
        RuntimeKind::Vm,
    ]
    .iter()
    .map(|k| {
        let s = snaps.get(k);
        RuntimeStatus {
            kind: *k,
            name: k.as_str().to_string(),
            active: s.map(|s| s.available).unwrap_or(false),
            detail: s.and_then(|s| s.detail.clone()),
            hypervisors: vec![],
        }
    })
    .collect()
}

fn push_log_line(buf: &Arc<Mutex<LogBuf>>, line: LogLine) {
    if line.text == "__EOF__" {
        buf.lock().unwrap().live = false;
        return;
    }
    let mut b = buf.lock().unwrap();
    b.lines.push(line);
    if b.lines.len() > LOG_CAP {
        b.lines.remove(0);
    }
}

async fn get_or_connect(states: &StatesMap, kind: RuntimeKind) -> Result<bollard::Docker, String> {
    let entry = {
        let map = states.lock().await;
        map.get(&kind).cloned().ok_or("unknown runtime")?
    };
    if let Some(d) = entry.read().await.docker.clone() {
        return Ok(d);
    }
    let (d, _) = crate::engine::docker::connect_for(kind).await?;
    Ok(d)
}

pub fn kill_pid(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    #[cfg(windows)]
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .status();
}

pub async fn start_app(backend: Backend) {
    crate::spawn_pollers(None, &backend);
    let mut app = CliApp::new(backend);
    let terminal = ratatui::init();
    let res = run_loop(&mut app, terminal).await;
    ratatui::restore();
    if let Err(e) = res {
        eprintln!("error: {e}");
    }
}

async fn run_loop(app: &mut CliApp, mut terminal: DefaultTerminal) -> Result<(), String> {
    while !app.quit {
        let host = app.backend.host.read().await.clone();
        let netmap = app.backend.netmap.read().await.clone();
        let snapshots = app.backend.snapshots.read().await.clone();
        let sort = *app.backend.sort_key.read().await;
        let statuses = statuses_from(&snapshots);
        app.model = Model {
            host,
            netmap,
            snapshots,
            sort,
            statuses,
        };
        {
            let mut drawer = draw_fn(app);
            terminal.draw(&mut drawer).map_err(|e| e.to_string())?;
        }
        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let event::Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                if key.kind == KeyEventKind::Press {
                    on_key(app, key.code, key.modifiers).await;
                }
            }
        }
        let procs = app.sorted_processes().len();
        app.proc_sel = app.proc_sel.min(procs.saturating_sub(1));
        app.rt_sel = app
            .rt_sel
            .min(runtime_rows_len(app).saturating_sub(1));
        app.net_sel = app.net_sel.min(app.net_rows().len().saturating_sub(1));
    }
    if let Some(d) = app.detail.clone() {
        app.close_detail(&d).await;
    }
    Ok(())
}

fn runtime_rows_len(app: &CliApp) -> usize {
    let snap = app.snapshot(app.rt_tab);
    match app.rt_tab {
        RuntimeKind::Kubernetes => snap.pods.len() + snap.services.len(),
        RuntimeKind::Vm => snap.vms.len(),
        _ => snap.containers.len(),
    }
}

async fn on_key(app: &mut CliApp, code: KeyCode, mods: KeyModifiers) {
    if let Some((pid, _)) = app.kill_confirm.clone() {
        if matches!(code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            kill_pid(pid);
        }
        app.kill_confirm = None;
        return;
    }
    if app.show_help {
        if matches!(code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Enter)
            || (code == KeyCode::Char('c') && mods.contains(KeyModifiers::CONTROL))
        {
            app.show_help = false;
        }
        return;
    }
    if app.detail.is_some() {
        match code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Backspace => {
                let d = app.detail.clone().unwrap();
                app.close_detail(&d).await;
            }
            KeyCode::Char('q') => app.quit = true,
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => app.quit = true,
            _ => {}
        }
        return;
    }
    match (app.page, code) {
        (_, KeyCode::Char('c')) if mods.contains(KeyModifiers::CONTROL) => app.quit = true,
        (_, KeyCode::Char('q')) => app.quit = true,
        (_, KeyCode::Char('?')) => app.show_help = true,
        (_, KeyCode::Char('t')) => app.dark = !app.dark,
        (_, KeyCode::Char('1')) => app.page = Page::Host,
        (_, KeyCode::Char('2')) => app.page = Page::Runtimes,
        (_, KeyCode::Char('3')) => app.page = Page::Net,
        (Page::Host, KeyCode::Down | KeyCode::Char('j')) => app.proc_sel += 1,
        (Page::Host, KeyCode::Up | KeyCode::Char('k')) => {
            app.proc_sel = app.proc_sel.saturating_sub(1);
        }
        (Page::Host, KeyCode::PageDown) => app.proc_sel += 20,
        (Page::Host, KeyCode::PageUp) => app.proc_sel = app.proc_sel.saturating_sub(20),
        (Page::Host, KeyCode::Char('s')) => {
            let now = app.model.sort;
            let idx = SORT_CYCLE.iter().position(|k| *k == now).unwrap_or(0);
            let next = SORT_CYCLE[(idx + 1) % SORT_CYCLE.len()];
            *app.backend.sort_key.write().await = next;
            app.model.sort = next;
        }
        (Page::Host, KeyCode::Char('K')) => {
            if let Some(p) = app.selected_process() {
                app.kill_confirm = Some((p.pid, p.name));
            }
        }
        (Page::Host, KeyCode::Tab) => app.page = Page::Runtimes,
        (Page::Runtimes, KeyCode::Right | KeyCode::Tab) => app.cycle_tab(1),
        (Page::Runtimes, KeyCode::Left | KeyCode::BackTab) => app.cycle_tab(-1),
        (Page::Runtimes, KeyCode::Down | KeyCode::Char('j')) => app.rt_sel += 1,
        (Page::Runtimes, KeyCode::Up | KeyCode::Char('k')) => {
            app.rt_sel = app.rt_sel.saturating_sub(1);
        }
        (Page::Runtimes, KeyCode::PageDown) => app.rt_sel += 20,
        (Page::Runtimes, KeyCode::PageUp) => app.rt_sel = app.rt_sel.saturating_sub(20),
        (Page::Runtimes, KeyCode::Enter) => app.open_detail().await,
        (Page::Net, KeyCode::Down | KeyCode::Char('j')) => app.net_sel += 1,
        (Page::Net, KeyCode::Up | KeyCode::Char('k')) => {
            app.net_sel = app.net_sel.saturating_sub(1);
        }
        (Page::Net, KeyCode::Right | KeyCode::Char('+')) => {
            app.net_threshold = (app.net_threshold + 1024).min(1 << 30);
        }
        (Page::Net, KeyCode::Char('-')) => {
            app.net_threshold = app.net_threshold.saturating_sub(1024);
        }
        (Page::Net, KeyCode::Char('f')) => app.cycle_net_filter(),
        _ => {}
    }
}

fn draw_fn(app: &mut CliApp) -> impl FnMut(&mut Frame) + '_ {
    move |f: &mut Frame| {
        match app.page {
            Page::Host => host_page::draw(f, app),
            Page::Runtimes => runtime_page::draw(f, app),
            Page::Net => net_page::draw(f, app),
        }
        ui::overlay_ui(f, app);
        if app.show_help {
            ui::draw_help(f, app);
        }
        if let Some((pid, name)) = &app.kill_confirm {
            ui::draw_confirm(f, format!("Kill {} {}? [y/N]", pid, name));
        }
    }
}
