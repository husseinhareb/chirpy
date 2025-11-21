// src/ui/widgets/spectrum.rs
//! Spectrum visualizer widget wrapper.

use ratatui::{layout::Rect, Frame};

use crate::audio::Visualizer;

/// Render the spectrum visualizer.
pub fn render_spectrum(f: &mut Frame<'_>, area: Rect, visualizer: &Visualizer) {
    visualizer.render(f, area);
}
