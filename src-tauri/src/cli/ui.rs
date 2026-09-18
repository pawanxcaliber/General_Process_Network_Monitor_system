use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::{CliApp, Page};

#[derive(Clone, Copy)]
pub struct Theme {
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
}

pub fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            fg: Color::Rgb(230, 232, 236),
            dim: Color::Rgb(120, 124, 132),
            accent: Color::LightCyan,
            ok: Color::Rgb(120, 220, 120),
            warn: Color::Rgb(240, 190, 100),
            err: Color::Rgb(240, 110, 110),
        }
    } else {
        Theme {
            fg: Color::Rgb(40, 42, 48),
            dim: Color::Rgb(120, 124, 132),
            accent: Color::Cyan,
            ok: Color::Rgb(30, 150, 60),
            warn: Color::Rgb(190, 130, 20),
            err: Color::Rgb(200, 60, 60),
        }
    }
}

pub fn title_style(t: &Theme) -> Style {
    Style::new().fg(t.accent).add_modifier(Modifier::BOLD)
}

pub fn key_style(t: &Theme) -> Style {
    Style::new().fg(t.accent).add_modifier(Modifier::BOLD)
}

pub fn pct_color(t: &Theme, p: f32) -> Color {
    if p > 90.0 {
        t.err
    } else if p > 70.0 {
        t.warn
    } else {
        t.ok
    }
}

pub fn fmt_bytes(b: u64) -> String {
    const K: f64 = 1024.0;
    let b = b as f64;
    if b >= K * K * K * K {
        format!("{:.2} TiB", b / (K * K * K * K))
    } else if b >= K * K * K {
        format!("{:.2} GiB", b / (K * K * K))
    } else if b >= K * K {
        format!("{:.1} MiB", b / (K * K))
    } else if b >= K {
        format!("{:.1} KiB", b / K)
    } else {
        format!("{} B", b as u64)
    }
}

pub fn fmt_bps(b: f64) -> String {
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GiB/s", b / (1024.0 * 1024.0 * 1024.0))
    } else if b >= 1024.0 * 1024.0 {
        format!("{:.1} MiB/s", b / (1024.0 * 1024.0))
    } else if b >= 1024.0 {
        format!("{:.1} KiB/s", b / 1024.0)
    } else {
        format!("{:.0} B/s", b)
    }
}

/// ████░░░ style bar with the given fill percentage.
pub fn bar(pct: f32, width: usize, fg: Color) -> Span<'static> {
    let pct = pct.clamp(0.0, 100.0);
    let filled = (pct / 100.0 * width as f32).round() as usize;
    let mut s = String::with_capacity(width);
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    Span::styled(s, Style::new().fg(fg))
}

/// Top status bar: page tabs + live host stats; bottom keybind hint bar.
pub fn overlay_ui(f: &mut Frame, app: &CliApp) {
    let t = theme(app.dark);
    let area = f.area();

    let top = Rect {
        x: 0,
        y: 0,
        width: area.width,
        height: 1,
    };
    let page_names = ["Host", "Runtimes", "Network"];
    let idx = match app.page {
        Page::Host => 0,
        Page::Runtimes => 1,
        Page::Net => 2,
    };
    let mut spans = Vec::new();
    for (i, name) in page_names.iter().enumerate() {
        if i == idx {
            spans.push(Span::styled(
                format!(" {} ", name),
                Style::new()
                    .bg(t.accent)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", name),
                Style::new().fg(t.dim),
            ));
        }
    }
    let h = &app.model.host;
    spans.push(Span::styled(
        format!(
            "  CPU {:>5.1}%   RAM {}   PING {}   TEMP {}",
            h.cpu_percent,
            fmt_bytes(h.ram_used_bytes),
            h.ping_ms
                .map(|p| format!("{:.0}ms", p))
                .unwrap_or_else(|| "--".to_string()),
            h.cpu_temp_c
                .map(|c| format!("{:.0}°C", c))
                .unwrap_or_else(|| "--".to_string()),
        ),
        Style::new().fg(t.fg),
    ));
    f.render_widget(Paragraph::new(Line::from(spans)), top);

    let foot = Rect {
        x: 0,
        y: area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    let footer = Line::from(vec![
        Span::styled("q", key_style(&t)),
        Span::styled(" quit   ", Style::new().fg(t.dim)),
        Span::styled("?", key_style(&t)),
        Span::styled(" help   ", Style::new().fg(t.dim)),
        Span::styled("t", key_style(&t)),
        Span::styled(
            format!(" theme ({})", if app.dark { "dark" } else { "light" }),
            Style::new().fg(t.dim),
        ),
    ]);
    f.render_widget(Paragraph::new(footer), foot);
}

pub fn draw_help(f: &mut Frame, app: &CliApp) {
    let t = theme(app.dark);
    let area = centered_rect(62, 74, f.area());
    f.render_widget(Clear, area);
    let lines = vec![
        help_line("q / Ctrl-c", "quit", &t),
        help_line("1 / 2 / 3", "host / runtimes / network page", &t),
        help_line("t", "toggle dark/light theme", &t),
        help_line("?", "toggle this help", &t),
        help_line("", "", &t),
        help_line("s", "cycle process sort key", &t),
        help_line("j / k / arrows / pgup / pgdn", "process selection", &t),
        help_line("K", "kill selected process (y to confirm)", &t),
        help_line("", "", &t),
        help_line("left / right / Tab", "switch runtime", &t),
        help_line("j / k / arrows", "move row selection", &t),
        help_line("Enter", "open container/pod detail + logs", &t),
        help_line("Esc / left", "close detail / overlay", &t),
        help_line("", "", &t),
        help_line("+ / - / left / right", "network traffic threshold", &t),
        help_line("f", "filter group (docker/pod/external)", &t),
    ];
    let mut lines = lines;
    lines.insert(
        0,
        Line::from(Span::styled(" Monitor CLI ", title_style(&t))),
    );
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::new().fg(t.fg))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn help_line(key: &str, desc: &str, t: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:<26}", key), key_style(t)),
        Span::raw(desc.to_string()),
    ])
}

pub fn draw_confirm(f: &mut Frame, msg: String) {
    let t = theme(true);
    let popup = centered_rect(50, 4, f.area());
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                msg,
                Style::new().fg(t.warn).add_modifier(Modifier::BOLD),
            )),
            Line::from("y = confirm   any other key = cancel"),
        ])
        .style(Style::new().fg(t.fg))
        .block(Block::bordered().title(Span::styled(
            " Confirm ",
            title_style(&t),
        )))
        .wrap(Wrap { trim: false }),
        popup,
    );
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y).div_ceil(2)),
    ])
    .areas(area);
    let [_, mid, _] = v;
    let h = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x).div_ceil(2)),
    ])
    .areas(mid);
    let [_, inner, _] = h;
    inner
}
