use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Clear, Gauge, Paragraph, Row, Tabs, Table, TableState, Wrap,
};
use ratatui::Frame;

use crate::engine::RuntimeKind;
use crate::types::RuntimeSnapshot;


use super::ui::{self, Theme};
use super::CliApp;

pub fn draw(f: &mut Frame, app: &mut CliApp) {
    let t = ui::theme(app.dark);
    let area = inner_area(f);

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
        .areas(area);
    let [tab_row, body] = chunks;

    let titles: Vec<Line> = [
        RuntimeKind::Docker,
        RuntimeKind::Podman,
        RuntimeKind::Kubernetes,
        RuntimeKind::Vm,
    ]
    .iter()
    .map(|k| Line::from(format!(" {} ", kind_label(*k))))
    .collect();
    let cur_idx = [
        RuntimeKind::Docker,
        RuntimeKind::Podman,
        RuntimeKind::Kubernetes,
        RuntimeKind::Vm,
    ]
    .iter()
    .position(|k| *k == app.rt_tab)
    .unwrap_or(0);
    f.render_widget(Tabs::new(titles).select(cur_idx).highlight_style(Style::new().fg(t.accent).add_modifier(Modifier::BOLD)), tab_row);

    let snap = app.snapshot(app.rt_tab);
    if !snap.available {
        let msg = match app.rt_tab {
            RuntimeKind::Vm => "no VM hypervisor detected (supported: libvirt, VirtualBox, VMware, Hyper-V)".to_string(),
            _ => format!(
                "{} not connected — {}",
                kind_label(app.rt_tab),
                snap.detail.unwrap_or_else(|| "waiting for poll".to_string())
            ),
        };
        f.render_widget(
            Paragraph::new(msg)
                .style(Style::new().fg(t.dim))
                .wrap(Wrap { trim: true }),
            body,
        );
        return;
    }

    match app.rt_tab {
        RuntimeKind::Kubernetes => k8s_view(f, body, app, &snap, &t),
        RuntimeKind::Vm => vm_view(f, body, app, &snap, &t),
        _ => docker_like_view(f, body, app, &snap, &t),
    }

    if app.detail.is_some() {
        draw_detail(f, f.area(), app, &snap, &t);
    }
}

fn kind_label(k: RuntimeKind) -> &'static str {
    match k {
        RuntimeKind::Docker => "Docker",
        RuntimeKind::Podman => "Podman",
        RuntimeKind::Kubernetes => "Kubernetes",
        RuntimeKind::Vm => "VMs",
    }
}

fn inner_area(f: &mut Frame) -> ratatui::layout::Rect {
    let a = f.area();
    ratatui::layout::Rect {
        x: 0,
        y: 1,
        width: a.width,
        height: a.height.saturating_sub(2),
    }
}

fn docker_like_view(f: &mut Frame, body: ratatui::layout::Rect, app: &mut CliApp, snap: &RuntimeSnapshot, t: &Theme) {
    let running = snap.containers.iter().filter(|c| c.state == "running").count();
    let total_cpu: f64 = snap.containers.iter().map(|c| c.cpu_percent).sum();
    let total_mem: u64 = snap.containers.iter().map(|c| c.memory_bytes).sum();

    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)])
        .areas(body);
    let [stats, list] = chunks;
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title(Span::styled(
                format!(
                    " {} running / {} total   CPU {:.1}%   MEM {} ",
                    running,
                    snap.containers.len(),
                    total_cpu,
                    ui::fmt_bytes(total_mem)
                ),
                ui::title_style(t),
            )))
            .gauge_style(Style::new().fg(ui::pct_color(t, total_cpu as f32)))
            .ratio((total_cpu / 100.0).clamp(0.0, 1.0)),
        stats,
    );

    let sel_style = Style::new().bg(t.accent).fg(Color::Black);
    let header_style = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let rows: Vec<Row> = snap
        .containers
        .iter()
        .map(|c| {
            Row::new(vec![
                truncate(&c.name, 22),
                truncate(&c.image, 28),
                state_span(&c.state, t).to_string(),
                format!("{:>5.1}", c.cpu_percent),
                ui::fmt_bytes(c.memory_bytes),
                ui::fmt_bps(c.rx_bps),
                ui::fmt_bps(c.tx_bps),
                truncate(
                    &c.ports
                        .iter()
                        .map(|p| match p.host_port {
                            Some(h) => format!("{}:{}", h, p.container_port),
                            None => format!("{}", p.container_port),
                        })
                        .collect::<Vec<_>>()
                        .join(","),
                    18,
                ),
            ])
        })
        .collect();
    let header = Row::new(vec!["NAME", "IMAGE", "STATE", "CPU%", "MEM", "RX/s", "TX/s", "PORTS"])
        .style(header_style);
    let widths = vec![
        Constraint::Min(22),
        Constraint::Min(28),
        Constraint::Length(12),
        Constraint::Length(7),
        Constraint::Length(10),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(18),
    ];
    let title = format!(
        " CONTAINERS [{}/{}]  enter=detail ",
        running,
        snap.containers.len()
    );
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(Span::styled(title, ui::title_style(t))))
        .row_highlight_style(sel_style);
    let mut st = TableState::default().with_selected(Some(app.rt_sel));
    f.render_stateful_widget(table, list, &mut st);
}

fn state_span(state: &str, t: &Theme) -> Span<'static> {
    let color = match state {
        "running" => t.ok,
        "paused" => t.warn,
        "restarting" => t.warn,
        _ => t.dim,
    };
    Span::styled(state.to_string(), Style::new().fg(color))
}

fn k8s_view(f: &mut Frame, body: ratatui::layout::Rect, app: &mut CliApp, snap: &RuntimeSnapshot, t: &Theme) {
    let chunks = Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)])
        .areas(body);
    let [pods_area, svcs_area] = chunks;

    let metrics_tag = if snap.metrics_available { "" } else { " (no metrics server)" };
    let sel_style = Style::new().bg(t.accent).fg(Color::Black);
    let header_style = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let rows: Vec<Row> = snap
        .pods
        .iter()
        .map(|p| {
            Row::new(vec![
                truncate(&p.namespace, 14),
                truncate(&p.name, 38),
                phase(p.phase.clone(), t).to_string(),
                truncate(&p.node, 16),
                p.cpu_millis.map(|c| format!("{}m", c)).unwrap_or("--".into()),
                p.mem_bytes.map(ui::fmt_bytes).unwrap_or("--".into()),
                format!("{}", p.restarts),
            ])
        })
        .collect();
    let header = Row::new(vec!["NAMESPACE", "POD", "PHASE", "NODE", "CPU", "MEM", "RST"])
        .style(header_style);
    let widths = vec![
        Constraint::Length(14),
        Constraint::Min(38),
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(5),
    ];
    let title = format!(
        " PODS [{}] {} ",
        snap.pods.len(),
        metrics_tag
    );
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(Span::styled(title, ui::title_style(t))))
        .row_highlight_style(sel_style);
    let mut st = TableState::default().with_selected(Some(app.rt_sel));
    f.render_stateful_widget(table, pods_area, &mut st);

    let svc_rows: Vec<Row> = snap
        .services
        .iter()
        .map(|s| {
            Row::new(vec![
                truncate(&s.namespace, 14),
                truncate(&s.name, 30),
                s.cluster_ip.clone(),
                truncate(&s.ports.join(","), 30),
            ])
        })
        .collect();
    let svc_header = Row::new(vec!["NAMESPACE", "SERVICE", "CLUSTER-IP", "PORTS"]).style(header_style);
    let svc_widths = vec![
        Constraint::Length(14),
        Constraint::Min(30),
        Constraint::Length(15),
        Constraint::Length(30),
    ];
    let svc_table = Table::new(svc_rows, svc_widths)
        .header(svc_header)
        .block(Block::bordered().title(Span::styled(
            format!(" SERVICES [{}] ", snap.services.len()),
            ui::title_style(t),
        )));
    f.render_widget(svc_table, svcs_area);
}

fn phase(p: String, t: &Theme) -> Span<'static> {
    let color = match p.as_str() {
        "Running" => t.ok,
        "Pending" => t.warn,
        _ => t.err,
    };
    Span::styled(p, Style::new().fg(color))
}

fn vm_view(f: &mut Frame, body: ratatui::layout::Rect, app: &mut CliApp, snap: &RuntimeSnapshot, t: &Theme) {
    let running = snap.vms.iter().filter(|v| v.state == "running").count();
    let sel_style = Style::new().bg(t.accent).fg(Color::Black);
    let header_style = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let rows: Vec<Row> = snap
        .vms
        .iter()
        .map(|v| {
            Row::new(vec![
                truncate(&v.name, 30),
                state_span(&v.state, t).to_string(),
                truncate(&v.hypervisor, 14),
                format!("{}", v.vcpus),
                ui::fmt_bytes(v.memory_bytes),
            ])
        })
        .collect();
    let header = Row::new(vec!["NAME", "STATE", "HYPERVISOR", "VCPUS", "MEM"])
        .style(header_style);
    let widths = vec![
        Constraint::Min(30),
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(7),
        Constraint::Length(12),
    ];
    let title = format!(
        " VMS {} running / {} total ",
        running,
        snap.vms.len()
    );
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(Span::styled(title, ui::title_style(t))))
        .row_highlight_style(sel_style);
    let mut st = TableState::default().with_selected(Some(app.rt_sel));
    f.render_stateful_widget(table, body, &mut st);
}

/// Overlay detail: header line then log tail. Logs follow via LogBuf.
fn draw_detail(f: &mut Frame, area: ratatui::layout::Rect, app: &CliApp, snap: &RuntimeSnapshot, t: &Theme) {
    let Some((kind, id)) = app.detail.clone() else { return };
    let overlay = ui::centered_rect(92, 88, area);
    f.render_widget(Clear, overlay);

    let mut header_line = format!("[{}] {}", kind_label(kind), short_id(&id));
    if let Some(c) = snap.containers.iter().find(|c| c.id == id) {
        header_line = format!(
            "{}  image={} state={} cpu={:.1}% mem={} rx={} tx={} ports={}",
            c.name,
            truncate(&c.image, 28),
            c.state,
            c.cpu_percent,
            ui::fmt_bytes(c.memory_bytes),
            ui::fmt_bps(c.rx_bps),
            ui::fmt_bps(c.tx_bps),
            c.ports
                .iter()
                .map(|p| match p.host_port {
                    Some(h) => format!("{}:{}", h, p.container_port),
                    None => format!("{}", p.container_port),
                })
                .collect::<Vec<_>>()
                .join(",")
        );
    }

    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)])
        .areas(overlay);
    let [info_area, logs_area] = chunks;

    let live = &app.logs_buf;
    let buf_state = live.lock().unwrap();
    let tail: Vec<Line> = buf_state
        .lines
        .iter()
        .rev()
        .take(logs_area.height.saturating_sub(2) as usize * 4)
        .map(|l| Line::from(format!(
            "{} {}",
            l.stream,
            truncate(&l.text, 250)
        )))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let status = if buf_state.live {
        Span::styled(" (streaming — esc to close)", Style::new().fg(t.dim))
    } else {
        Span::styled(" (stream ended — esc to close)", Style::new().fg(t.dim))
    };
    drop(buf_state);

    f.render_widget(
        Paragraph::new(header_line)
            .block(Block::bordered().title(Span::styled(
                " APP ",
                ui::title_style(t),
            )))
            .style(Style::new().fg(t.fg))
            .wrap(Wrap { trim: true }),
        info_area,
    );

    let title_line = Line::from(vec![
        Span::styled(" LOGS ", ui::title_style(t)),
        status,
    ]);
    f.render_widget(
        Paragraph::new(tail)
            .block(Block::bordered().title(title_line))
            .style(Style::new().fg(t.fg))
            .wrap(Wrap { trim: false }),
        logs_area,
    );
}

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

fn truncate(s: &str, n: usize) -> String {
    let taken: String = s.chars().take(n).collect();
    if taken.len() == s.len() {
        taken
    } else {
        format!("{}~", taken.trim_end())
    }
}
