use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Row, Table, TableState};
use ratatui::Frame;

use super::ui::{self, Theme};
use super::{CliApp, NetRow};

pub fn draw(f: &mut Frame, app: &mut CliApp) {
    let t = ui::theme(app.dark);
    let a = f.area();
    let body = ratatui::layout::Rect {
        x: 0,
        y: 1,
        width: a.width,
        height: a.height.saturating_sub(2),
    };

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
        .areas(body);
    let [hint_row, list_area] = chunks;

    let hint = Line::from(Span::styled(
        format!(
            "filter: {}   threshold: {:.1} KiB/s   f=filter  +/-=threshold  [{} edges]",
            filter_name(&app.net_filter),
            app.net_threshold as f64 / 1024.0,
            app.net_rows_pending(),
        ),
        Style::new().fg(t.dim),
    ));
    f.render_widget(hint, hint_row);

    let rows: Vec<NetRow> = app.net_rows();
    let sel_style = Style::new().bg(t.accent).fg(Color::Black);
    let header_style = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let table_rows: Vec<Row> = rows
        .iter()
        .map(|r| {
            Row::new(vec![
                Span::styled(
                    format!(" {:<10} ", group_label(&r.src_grp)),
                    Style::new().fg(group_color(&r.src_grp, &t)),
                ),
                truncate(&r.src, 34).into(),
                Span::styled("→", Style::new().fg(t.dim)),
                Span::styled(
                    format!(" {:<10} ", group_label(&r.dst_grp)),
                    Style::new().fg(group_color(&r.dst_grp, &t)),
                ),
                truncate(&r.dst, 34).into(),
                truncate(&r.ports, 20).into(),
                format!("{}", r.conns).into(),
                Span::styled(
                    ui::fmt_bps(r.rate_bps),
                    Style::new()
                        .fg(if r.rate_bps > 1024.0 * 100.0 { t.warn } else { t.fg })
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        })
        .collect();
    let header = Row::new(vec!["SRC TYPE", "SOURCE", "→", "DST TYPE", "DESTINATION", "PORTS", "CONNS", "RATE/S"])
        .style(header_style);
    let widths = vec![
        Constraint::Length(12),
        Constraint::Min(34),
        Constraint::Length(1),
        Constraint::Length(12),
        Constraint::Min(34),
        Constraint::Length(20),
        Constraint::Length(7),
        Constraint::Length(12),
    ];
    let table = Table::new(table_rows, widths)
        .header(header)
        .block(Block::bordered().title(Span::styled(
            " LIVE CONNECTIONS ",
            ui::title_style(&t),
        )))
        .row_highlight_style(sel_style);
    let mut st = TableState::default().with_selected(Some(app.net_sel));
    f.render_stateful_widget(table, list_area, &mut st);
}

fn filter_name(f: &Option<String>) -> &'static str {
    match f.as_deref() {
        Some("docker") => "containers",
        Some("pod") => "pods",
        Some("external") => "external",
        _ => "all",
    }
}

fn group_label(g: &str) -> String {
    match g {
        "docker" => "container",
        "pod" => "pod",
        "external" => "external",
        "process" => "process",
        "system" => "system",
        "vm" => "vm",
        other => other,
    }
    .to_string()
}

fn group_color(g: &str, t: &Theme) -> Color {
    match g {
        "docker" => t.ok,
        "pod" => t.accent,
        "external" => t.warn,
        "system" => t.dim,
        _ => t.fg,
    }
}

fn truncate(s: &str, n: usize) -> String {
    let taken: String = s.chars().take(n).collect();
    if taken.len() == s.len() {
        taken
    } else {
        format!("{}~", taken.trim_end())
    }
}

impl CliApp {
    pub fn net_rows_pending(&self) -> usize {
        self.model.netmap.edges.len()
    }
}

