// src/ui/widgets/artwork.rs
//! Album artwork display widget.

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders},
    Frame,
};

/// Render the artwork panel.
/// Note: Actual image rendering requires ratatui-image integration.
pub fn render_artwork(f: &mut Frame<'_>, area: Rect) {
    let title = "3: Artwork";
    f.render_widget(
        Block::default().borders(Borders::ALL).title(title),
        area,
    );
    // TODO: Integrate with ratatui-image for actual artwork display
}
