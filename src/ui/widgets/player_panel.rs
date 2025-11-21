// src/ui/widgets/player_panel.rs
//! Player information panel widget.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
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
) {
    let title = "2: Player";
    f.render_widget(
        Block::default().borders(Borders::ALL).title(title),
        area,
    );

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
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
            Paragraph::new("▶️ No track playing").wrap(Wrap { trim: true }),
            inner[0],
        );
    }

    let ratio = (elapsed as f64 / duration as f64).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().add_modifier(Modifier::ITALIC))
            .ratio(ratio),
        inner[1],
    );
}
