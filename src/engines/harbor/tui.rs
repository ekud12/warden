// ─── Engine: Harbor — CLI: tui ────────────────────────────────────────────────
//
// `warden tui [--live]` — real-time session dashboard using ratatui.
// Shows: token burn sparkline, phase timeline, quality score, anomaly alerts,
// restriction fire counts, session stats.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::engines::dream::imprint as anomaly;
use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{prelude::*, widgets::*};
use std::io::stdout;
use std::time::Duration;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    loop {
        terminal.draw(draw)?;

        // Poll for input (refresh every 2s)
        if event::poll(Duration::from_secs(2))?
            && let Event::Key(key) = event::read()?
            && (key.code == KeyCode::Char('q') || key.code == KeyCode::Esc)
        {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn draw(frame: &mut Frame) {
    let state = common::read_session_state();
    let project_dir = common::project_dir();
    let stats = anomaly::load_stats(&project_dir);

    let area = frame.area();

    // Layout: 3 rows
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Footer
        ])
        .split(area);

    // Header
    let phase = format!("{:?}", state.adaptive.phase);
    let header = Paragraph::new(format!(
        " Warden v{} | Turn {} | Phase: {} | Errors: {} | Edits: {}",
        env!("CARGO_PKG_VERSION"),
        state.turn,
        phase,
        state.errors_unresolved,
        state.files_edited.len(),
    ))
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Main: split into left (metrics) + right (sparkline + phase)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Left: session stats
    draw_stats(frame, main_chunks[0], &state, &stats);

    // Right: token burn sparkline + phase timeline
    draw_charts(frame, main_chunks[1], &state);

    // Footer
    let footer = Paragraph::new(" Press 'q' to quit | Refreshes every 2s")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);
}

fn draw_stats(
    frame: &mut Frame,
    area: Rect,
    state: &common::SessionState,
    stats: &anomaly::ProjectStats,
) {
    let items: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Turn: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", state.turn),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Files edited: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", state.files_edited.len()),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Files read: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", state.files_read.len()),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(vec![
            Span::styled("Errors: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", state.errors_unresolved),
                Style::default().fg(if state.errors_unresolved > 0 {
                    Color::Red
                } else {
                    Color::Green
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled("Explore count: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", state.explore_count),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("Tokens in: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}K", state.estimated_tokens_in / 1000),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Tokens saved: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}K", state.estimated_tokens_saved / 1000),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Project avg quality: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if stats.quality_score.n >= 3 {
                    format!("{:.0}/100", stats.quality_score.mean)
                } else {
                    "N/A".to_string()
                },
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::styled("Project avg turns: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if stats.session_length.n >= 3 {
                    format!("{:.0}", stats.session_length.mean)
                } else {
                    "N/A".to_string()
                },
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(items).block(
        Block::default()
            .title(" Session Stats ")
            .borders(Borders::ALL),
    );
    frame.render_widget(paragraph, area);
}

fn draw_charts(frame: &mut Frame, area: Rect, state: &common::SessionState) {
    let chart_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Sparkline
            Constraint::Min(5),    // Phase timeline
        ])
        .split(area);

    // Token burn sparkline
    let token_data: Vec<u64> = state
        .turn_snapshots
        .iter()
        .map(|s| (s.tokens_in_delta + s.tokens_out_delta) / 1000) // in K
        .collect();

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(" Token Burn (K/turn) ")
                .borders(Borders::ALL),
        )
        .data(&token_data)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(sparkline, chart_chunks[0]);

    // Phase timeline
    let transitions: Vec<Line> = state
        .adaptive
        .transitions
        .iter()
        .map(|t| {
            let color = match t.to.as_str() {
                "Productive" => Color::Green,
                "Exploring" => Color::Yellow,
                "Struggling" => Color::Red,
                "Late" => Color::Magenta,
                _ => Color::White,
            };
            Line::from(vec![
                Span::styled(
                    format!("T{}: ", t.turn),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(&t.from, Style::default().fg(Color::Gray)),
                Span::raw(" → "),
                Span::styled(
                    &t.to,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({})", t.reason),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        })
        .collect();

    let phase_text = if transitions.is_empty() {
        vec![Line::from(Span::styled(
            "No phase transitions yet",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        transitions
    };

    let phase_widget = Paragraph::new(phase_text).block(
        Block::default()
            .title(" Phase Timeline ")
            .borders(Borders::ALL),
    );
    frame.render_widget(phase_widget, chart_chunks[1]);
}
