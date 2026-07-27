use crate::swarm::RelaySnapshot;
use kursal_core::stats::{NodeStats, StatsCollector};
use ratatui::{
    Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Gauge, List, ListItem, Paragraph, Sparkline},
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const SPARK_LEN: usize = 60;

pub fn run(rx: watch::Receiver<RelaySnapshot>, mut collector: StatsCollector) {
    let mut terminal = ratatui::init();
    let mut spark_in: VecDeque<u64> = VecDeque::with_capacity(SPARK_LEN);
    let mut spark_out: VecDeque<u64> = VecDeque::with_capacity(SPARK_LEN);

    loop {
        let stats = collector.sample();
        push_sample(&mut spark_in, rate_sample(stats.rate_in));
        push_sample(&mut spark_out, rate_sample(stats.rate_out));
        let snapshot = rx.borrow().clone();
        let in_data: Vec<u64> = spark_in.iter().copied().collect();
        let out_data: Vec<u64> = spark_out.iter().copied().collect();

        if terminal
            .draw(|f| draw(f, &snapshot, &stats, &in_data, &out_data))
            .is_err()
        {
            break;
        }
        if wait_for_quit(Duration::from_secs(1)) {
            break;
        }
    }

    ratatui::restore();
}

fn push_sample(buf: &mut VecDeque<u64>, value: u64) {
    if buf.len() >= SPARK_LEN {
        buf.pop_front();
    }
    buf.push_back(value);
}

fn wait_for_quit(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        if !event::poll(remaining).unwrap_or(false) {
            return false;
        }
        if let Ok(Event::Key(key)) = event::read() {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl_c =
                key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) || ctrl_c {
                return true;
            }
        }
    }
}

fn draw(f: &mut Frame, snap: &RelaySnapshot, stats: &NodeStats, in_data: &[u64], out_data: &[u64]) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(4),
        ])
        .split(f.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" kursal-relay v{} ", env!("CARGO_PKG_VERSION")),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("· {} ", snap.peer_id)),
        Span::styled(
            format!("· up {} · q to quit", fmt_duration(stats.uptime_secs)),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::bordered());
    f.render_widget(header, rows[0]);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(rows[1]);

    let cpu = Gauge::default()
        .block(Block::bordered().title("cpu"))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(f64::from(stats.cpu_percent.clamp(0.0, 100.0)) / 100.0)
        .label(format!("{:.1}%", stats.cpu_percent));
    f.render_widget(cpu, mid[0]);

    let counters = Paragraph::new(Line::from(vec![
        Span::raw(format!(" mem {} ", fmt_bytes(stats.mem_bytes))),
        Span::styled("· ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("connections {} ", snap.connections)),
        Span::styled("· ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("reservations {} ", snap.reservations)),
        Span::styled("· ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("circuits {} ", snap.circuits)),
        Span::styled("· ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("listeners {}", snap.listen_addrs.len())),
    ]))
    .block(Block::bordered().title("node"));
    f.render_widget(counters, mid[1]);

    let traffic = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);

    let spark_in = Sparkline::default()
        .block(Block::bordered().title(format!(
            "in {}/s · total {}",
            fmt_rate(stats.rate_in),
            fmt_bytes(stats.bytes_in)
        )))
        .style(Style::default().fg(Color::Green))
        .data(in_data);
    f.render_widget(spark_in, traffic[0]);

    let spark_out = Sparkline::default()
        .block(Block::bordered().title(format!(
            "out {}/s · total {}",
            fmt_rate(stats.rate_out),
            fmt_bytes(stats.bytes_out)
        )))
        .style(Style::default().fg(Color::Blue))
        .data(out_data);
    f.render_widget(spark_out, traffic[1]);

    let log_height = rows[3].height.saturating_sub(2) as usize;
    let items: Vec<ListItem> = snap
        .events
        .iter()
        .rev()
        .take(log_height)
        .rev()
        .map(|line| ListItem::new(line.as_str()))
        .collect();
    let events = List::new(items).block(Block::bordered().title("events"));
    f.render_widget(events, rows[3]);
}

fn fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn fmt_rate(rate: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if !rate.is_finite() || rate <= 0.0 {
        return "0 B".to_string();
    }

    let mut value = rate;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{value:.0} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn rate_sample(rate: f64) -> u64 {
    if !rate.is_finite() || rate <= 0.0 {
        return 0;
    }

    if rate >= u64::MAX as f64 {
        return u64::MAX;
    }

    rate.round().to_string().parse().unwrap_or(u64::MAX)
}

fn fmt_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs / 60) % 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}
