use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Gauge, Paragraph, Row, Sparkline, Table, TableState, Wrap,
};
use ratatui::Frame;

use super::ui::{self, Theme};

pub fn draw(f: &mut Frame, app: &mut super::CliApp) {
    let t = ui::theme(app.dark);
    let h = &app.model.host;

    let cores = core_lines(&h.cpu_per_core, f.area().width, &t);
    let core_n = cores.len().max(1) as u16;

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(4),
        Constraint::Length(5),
        Constraint::Length(core_n),
        Constraint::Fill(1),
    ])
    .areas(inner_area(f));
    let [gauges, info, hist, core_rows, procs] = chunks;

    gauge_cells(f, gauges, app, &t);
    info_cells(f, info, app, &t);
    history_cells(f, hist, app, &t);
    f.render_widget(
        Paragraph::new(cores).style(Style::new().fg(t.fg)),
        core_rows,
    );
    processes(f, procs, app, &t);
}

fn inner_area(f: &mut Frame) -> Rect {
    let a = f.area();
    Rect {
        x: 0,
        y: 1,
        width: a.width,
        height: a.height.saturating_sub(2),
    }
}

fn gauge_cells(f: &mut Frame, area: Rect, app: &super::CliApp, t: &Theme) {
    let h = &app.model.host;
    let cells = Layout::horizontal([Constraint::Fill(1); 3])
        .areas(area);
    let [cpu_c, ram_c, swap_c] = cells;

    let cpu = Gauge::default()
        .block(Block::bordered().title(Span::styled(
            " CPU ",
            ui::title_style(t),
        )))
        .gauge_style(Style::new().fg(ui::pct_color(t, h.cpu_percent)))
        .ratio((h.cpu_percent as f64 / 100.0).clamp(0.0, 1.0));
    f.render_widget(cpu, cpu_c);

    let ram_pct = if h.ram_total_bytes > 0 {
        h.ram_used_bytes as f64 / h.ram_total_bytes as f64 * 100.0
    } else {
        0.0
    };
    let ram = Gauge::default()
        .block(Block::bordered().title(Span::styled(
            format!(
                " RAM {} / {} ",
                ui::fmt_bytes(h.ram_used_bytes),
                ui::fmt_bytes(h.ram_total_bytes)
            ),
            ui::title_style(t),
        )))
        .gauge_style(Style::new().fg(ui::pct_color(t, ram_pct as f32)))
        .ratio((ram_pct / 100.0).clamp(0.0, 1.0));
    f.render_widget(ram, ram_c);

    if h.swap_total_bytes > 0 {
        let swap_pct = h.swap_used_bytes as f64 / h.swap_total_bytes as f64 * 100.0;
        let swap = Gauge::default()
            .block(Block::bordered().title(Span::styled(
                format!(
                    " SWAP {} / {} ",
                    ui::fmt_bytes(h.swap_used_bytes),
                    ui::fmt_bytes(h.swap_total_bytes)
                ),
                ui::title_style(t),
            )))
            .gauge_style(Style::new().fg(ui::pct_color(t, swap_pct as f32)))
            .ratio((swap_pct / 100.0).clamp(0.0, 1.0));
        f.render_widget(swap, swap_c);
    }
}

fn info_cells(f: &mut Frame, area: Rect, app: &super::CliApp, t: &Theme) {
    let h = &app.model.host;
    let cells = Layout::horizontal([Constraint::Fill(1); 3])
        .areas(area);
    let [net_c, temp_c, gpu_c] = cells;

    let total_rx: f64 = h.interfaces.iter().map(|i| i.rx_bps).sum();
    let total_tx: f64 = h.interfaces.iter().map(|i| i.tx_bps).sum();
    let disk_line = disk_summary(&app.model.host, t);

    let first: Vec<Line> = vec![
        Line::from(format!(
            "rx {}   tx {}",
            ui::fmt_bps(total_rx),
            ui::fmt_bps(total_tx)
        )),
        Line::from(format!(
            "ping {}",
            h.ping_ms
                .map(|p| format!("{:.1} ms", p))
                .unwrap_or_else(|| "n/a".into())
        )),
        disk_line,
    ];
    f.render_widget(
        Paragraph::new(first)
            .block(Block::bordered().title(Span::styled(" NET ", ui::title_style(t))))
            .style(Style::new().fg(t.fg))
            .wrap(Wrap { trim: true }),
        net_c,
    );

    let mut temp_lines: Vec<Line> = Vec::new();
    if let Some(c) = h.cpu_temp_c {
        temp_lines.push(Line::from(format!("{:.1}°C", c)));
    } else {
        temp_lines.push(Line::from(Span::styled(
            "no sensor",
            Style::new().fg(t.dim),
        )));
    }
    f.render_widget(
        Paragraph::new(temp_lines)
            .block(Block::bordered().title(Span::styled(" TEMP ", ui::title_style(t))))
            .style(Style::new().fg(t.fg)),
        temp_c,
    );

    let mut gpu_lines: Vec<Line> = Vec::new();
    if h.gpu_igpu_present {
        gpu_lines.push(Line::from(format!(
            "igpu {}",
            ui::fmt_bytes(h.gpu_igpu_used_bytes)
        )));
    }
    if h.gpu_dgpu_present {
        gpu_lines.push(Line::from(format!(
            "dgpu {}",
            ui::fmt_bytes(h.gpu_dgpu_used_bytes)
        )));
    }
    if h.gpu_dgpu_driver_unavailable {
        gpu_lines.push(Line::from(Span::styled(
            "dgpu driver unavailable",
            Style::new().fg(t.warn),
        )));
    }
    if !h.gpu_igpu_present && !h.gpu_dgpu_present {
        gpu_lines.push(Line::from(Span::styled("none detected", Style::new().fg(t.dim))));
    }
    f.render_widget(
        Paragraph::new(gpu_lines)
            .block(Block::bordered().title(Span::styled(" GPU ", ui::title_style(t))))
            .style(Style::new().fg(t.fg)),
        gpu_c,
    );
}

fn disk_summary(h: &crate::types::HostPayload, t: &Theme) -> Line<'static> {
    let total_r: f64 = h.disks.iter().map(|d| d.read_bps).sum();
    let total_w: f64 = h.disks.iter().map(|d| d.write_bps).sum();
    Line::from(vec![
        Span::raw("disk "),
        Span::styled(
            format!("r {} w {}", ui::fmt_bps(total_r), ui::fmt_bps(total_w)),
            Style::new().fg(t.warn),
        ),
    ])
}

fn history_cells(f: &mut Frame, area: Rect, app: &super::CliApp, t: &Theme) {
    let cells = Layout::horizontal([Constraint::Fill(1); 3])
        .areas(area);
    let [cpu_c, net_c, disk_c] = cells;
    let h = &app.model.host;

    let cpu_data = u64s(&h.history.cpu);
    f.render_widget(
        Sparkline::default()
            .block(Block::bordered().title(Span::styled(" cpu % ", ui::title_style(t))))
            .style(Style::new().fg(t.accent))
            .data(&cpu_data),
        cpu_c,
    );

    let mut acc = u64s(&h.history.net_rx_bps);
    for (i, tx) in u64s(&h.history.net_tx_bps).into_iter().enumerate() {
        if let Some(slot) = acc.get_mut(i) {
            *slot = slot.saturating_add(tx);
        }
    }
    f.render_widget(
        Sparkline::default()
            .block(Block::bordered().title(Span::styled(" net B/s ", ui::title_style(t))))
            .style(Style::new().fg(t.ok))
            .data(&acc),
        net_c,
    );

    let mut dio = u64s(&h.history.disk_read_bps);
    for (i, w) in u64s(&h.history.disk_write_bps).into_iter().enumerate() {
        if let Some(slot) = dio.get_mut(i) {
            *slot = slot.saturating_add(w);
        }
    }
    f.render_widget(
        Sparkline::default()
            .block(Block::bordered().title(Span::styled(" disk B/s ", ui::title_style(t))))
            .style(Style::new().fg(t.warn))
            .data(&dio),
        disk_c,
    );
}

fn u64s(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.round().max(0.0) as u64).collect()
}

/// 8-core mini bars per line, e.g. `c0 12% ███░░░░`.
fn core_lines<'a>(per_core: &[f32], width: u16, t: &Theme) -> Vec<Line<'a>> {
    if per_core.is_empty() {
        return vec![Line::from(Span::styled("no cpu data", Style::new().fg(t.dim)))];
    }
    let bar_w = 10;
    let per_line = ((width as usize).saturating_sub(2) / (bar_w + 8)).max(1);
    let mut out: Vec<Line<'a>> = Vec::new();
    let mut idx = 0;
    while idx < per_core.len() {
        let mut spans: Vec<Span<'a>> = Vec::new();
        for _ in 0..per_line {
            if idx >= per_core.len() {
                break;
            }
            let pct = per_core[idx].clamp(0.0, 100.0);
            spans.push(Span::styled(
                format!("c{:<4}", idx),
                Style::new().fg(t.dim),
            ));
            spans.push(ui::bar(pct, bar_w, ui::pct_color(t, pct)));
            spans.push(Span::styled(
                format!("{:>3.0}%", pct),
                Style::new().fg(t.fg),
            ));
            spans.push(Span::raw("  "));
            idx += 1;
        }
        out.push(Line::from(spans));
    }
    out
}

fn processes(f: &mut Frame, area: Rect, app: &mut super::CliApp, t: &Theme) {
    let procs = app.sorted_processes();
    let sel_style = Style::new().bg(t.accent).fg(Color::Black);
    let header_style = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let rows: Vec<Row> = procs
        .iter()
        .map(|p| {
            Row::new(vec![
                format!("{}  ", p.pid),
                truncate(&p.name, 30),
                format!("{:>5.1}", p.cpu_percent),
                ui::fmt_bytes(p.memory_bytes),
                ui::fmt_bps(p.disk_read_bps + p.disk_write_bps),
                ui::fmt_bps(p.net_rx_bps + p.net_tx_bps),
                p.gpu_igpu_bytes.map(ui::fmt_bytes).unwrap_or("--".into()),
                p.gpu_dgpu_bytes.map(ui::fmt_bytes).unwrap_or("--".into()),
            ])
        })
        .collect();
    let header = Row::new(vec![
        "PID", "NAME", "CPU%", "MEM", "DISK", "NET", "IGPU", "DGPU",
    ])
    .style(header_style);
    let widths = vec![
        Constraint::Length(9),
        Constraint::Min(30),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
    ];
    let title = format!(
        " PROCESSES [{}] ({} shown) ",
        app.model.sort.as_str(),
        procs.len()
    );
    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::bordered().title(Span::styled(title, ui::title_style(t))),
        )
        .row_highlight_style(sel_style);
    let mut st = TableState::default().with_selected(Some(app.proc_sel));
    f.render_stateful_widget(table, area, &mut st);
}

fn truncate(s: &str, n: usize) -> String {
    let taken: String = s.chars().take(n).collect();
    if taken.len() == s.len() {
        taken
    } else {
        format!("{}~", taken.trim_end())
    }
}
