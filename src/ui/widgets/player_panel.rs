// src/ui/widgets/player_panel.rs
//! Player information panel widget.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};

use crate::audio::TrackMetadata;

/// Render the player information panel.
pub fn render_player_panel(
    f: &mut Frame<'_>,
    area: Rect,
    metadata: Option<&TrackMetadata>,
    elapsed: u64,
    duration: u64,
    is_playing: bool,
    is_paused: bool,
) {
    let title = "2: Player";
    f.render_widget(
        Block::default().borders(Borders::ALL).title(title),
        area,
    );

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);

    if let Some(TrackMetadata {
        tags,
        properties,
        duration_secs,
        ..
    }) = metadata
    {
        let mut lines = vec![format!("Duration: {}s", duration_secs)];
        for (k, v) in tags {
            lines.push(format!("{}: {}", k, v));
        }
        for (k, v) in properties {
            lines.push(format!("{}: {}", k, v));
        }
        f.render_widget(
            Paragraph::new(lines.join("\n")).wrap(Wrap { trim: true }),
            inner[0],
        );
    } else {
        f.render_widget(
            Paragraph::new("No track playing").wrap(Wrap { trim: true }),
            inner[0],
        );
    }

    // Playback control buttons
    let play_pause_icon = if !is_playing {
        Span::styled(" ⏵ ", Style::default().fg(Color::Gray))
    } else if is_paused {
        Span::styled(" ⏵ ", Style::default().fg(Color::Yellow))
    } else {
        Span::styled(" ⏸ ", Style::default().fg(Color::Green))
    };

    let controls = Line::from(vec![
        Span::styled(" ⏮ ", Style::default().fg(Color::Cyan)),  // Previous (p/<)
        Span::raw(" "),
        Span::styled(" ⏹ ", Style::default().fg(Color::Red)),   // Stop (s)
        Span::raw(" "),
        play_pause_icon,                                         // Play/Pause (space)
        Span::raw(" "),
        Span::styled(" ⏭ ", Style::default().fg(Color::Cyan)),  // Next (n/>)
    ]);

    f.render_widget(
        Paragraph::new(controls).alignment(Alignment::Center),
        inner[1],
    );

    // Progress bar with time display
    let ratio = (elapsed as f64 / duration as f64).clamp(0.0, 1.0);
    let elapsed_min = elapsed / 60;
    let elapsed_sec = elapsed % 60;
    let duration_min = duration / 60;
    let duration_sec = duration % 60;
    let time_label = format!(
        "{:02}:{:02} / {:02}:{:02}",
        elapsed_min, elapsed_sec, duration_min, duration_sec
    );

    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC))
            .ratio(ratio)
            .label(time_label),
        inner[2],
    );
}
