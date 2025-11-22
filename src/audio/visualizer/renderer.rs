// src/audio/visualizer/renderer.rs
//! Spectrum bar rendering for the visualizer.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Renderer for spectrum visualization bars.
pub struct SpectrumRenderer {
    /// Bar width in characters
    bar_width: usize,
    /// Gap between bars
    bar_gap: usize,
    /// Block characters for smooth gradation
    chars: [char; 10],
}

impl SpectrumRenderer {
    /// Create a new spectrum renderer with default settings.
    pub fn new() -> Self {
        Self {
            bar_width: 2,
            bar_gap: 1,
            chars: ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█', '█', '█'],
        }
    }

    /// Render the frequency spectrum as mirrored bars (CAVA-style).
    pub fn render(
        &self,
        f: &mut Frame<'_>,
        area: Rect,
        magnitudes: &[f32],
        num_bands: usize,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("4: Spectrum Visualizer (FFT)");

        let inner = block.inner(area);

        // Render custom mirrored visualization
        self.render_mirrored(f, inner, magnitudes, num_bands);

        // Render the border
        f.render_widget(block, area);
    }

    /// Render mirrored bars like CAVA (symmetric around center).
    fn render_mirrored(
        &self,
        f: &mut Frame<'_>,
        area: Rect,
        magnitudes: &[f32],
        num_bands: usize,
    ) {
        if area.height < 2 || area.width < 2 {
            return;
        }

        let bar_spacing = self.bar_width + self.bar_gap;

        // Calculate how many bars can fit in half the width
        let half_width = area.width / 2;
        let bars_per_side = (half_width as usize / bar_spacing).min(num_bands / 2);

        if bars_per_side == 0 {
            return;
        }

        let height = area.height as usize;

        // Define 10 segments with different characters for smooth gradation
        const SEGMENTS: usize = 10;

        // Build the entire visualization as a single multi-line string
        let mut full_content = String::new();

        // Build the visualization line by line from top to bottom
        for row in 0..height {
            let mut line = String::with_capacity(area.width as usize);

            // Left side (mirrored)
            for i in (0..bars_per_side).rev() {
                let magnitude = magnitudes.get(i).copied().unwrap_or(0.0);
                let char_to_show =
                    self.get_char_for_row(magnitude, row, height, SEGMENTS);

                // Add bar
                for _ in 0..self.bar_width {
                    line.push(char_to_show);
                }

                // Add gap (but not after the last bar on left side)
                if i > 0 {
                    for _ in 0..self.bar_gap {
                        line.push(' ');
                    }
                }
            }

            // Right side (same as left, mirrored)
            for i in 0..bars_per_side {
                // Add gap before each bar
                for _ in 0..self.bar_gap {
                    line.push(' ');
                }

                let magnitude = magnitudes.get(i).copied().unwrap_or(0.0);
                let char_to_show =
                    self.get_char_for_row(magnitude, row, height, SEGMENTS);

                // Add bar
                for _ in 0..self.bar_width {
                    line.push(char_to_show);
                }
            }

            // Ensure the line fits within the available width
            let mut char_count = line.chars().count();

            // Pad with spaces if too short
            while char_count < area.width as usize {
                line.push(' ');
                char_count += 1;
            }

            // Truncate safely if too long (respect UTF-8 boundaries)
            if char_count > area.width as usize {
                line = line.chars().take(area.width as usize).collect();
            }

            // Add line to full content
            full_content.push_str(&line);
            if row < height - 1 {
                full_content.push('\n');
            }
        }

        // Render the entire visualization as a single widget to ensure proper clearing
        let paragraph = Paragraph::new(full_content).style(Style::default().fg(Color::White));
        f.render_widget(paragraph, area);
    }

    /// Determine what character to show at a specific row for a given magnitude.
    fn get_char_for_row(
        &self,
        magnitude: f32,
        row: usize,
        height: usize,
        segments: usize,
    ) -> char {
        // Calculate the filled height for this bar (0.0 to 1.0)
        let filled_height = magnitude;

        // Convert to actual pixel height
        let pixels_filled = (filled_height * height as f32) as usize;

        // Current row from bottom (0 = bottom, height-1 = top)
        let row_from_bottom = height - row - 1;

        // Determine what character to show at this row
        if pixels_filled == 0 && row_from_bottom == 0 {
            // Always show minimum bar at bottom row
            '▁'
        } else if row_from_bottom < pixels_filled {
            // This row is filled
            let segment_idx = ((filled_height * segments as f32) as usize).min(segments - 1);

            // Check if we're at the top of the filled area for gradient effect
            if row_from_bottom == pixels_filled - 1 {
                // Top segment - use gradient character
                let fractional = (filled_height * height as f32) - pixels_filled as f32;
                if fractional > 0.5 {
                    self.chars[segment_idx]
                } else {
                    self.chars[segment_idx.saturating_sub(1)]
                }
            } else {
                // Fully filled segment
                '█'
            }
        } else {
            ' '
        }
    }
}

impl Default for SpectrumRenderer {
    fn default() -> Self {
        Self::new()
    }
}
