// src/visualizer.rs

use ratatui::{
    backend::Backend,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Very small placeholder visualizer. It doesn't analyze audio yet; instead it
/// produces a deterministic animated pattern based on the tick counter. This
/// gives a visual area to iterate on while we wire real audio analysis later.
pub struct Visualizer {
    tick: u64,
}

impl Visualizer {
    pub fn new() -> Self {
        Self { tick: 0 }
    }

    /// Advance internal tick counter (call each UI tick)
    pub fn update(&mut self, tick: u64) {
        self.tick = tick;
    }

    /// Render a simple ASCII waveform-like visualization into `area`.
    pub fn render<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        // Leave room for borders
        let inner_w = area.width.saturating_sub(2) as usize;
        let inner_h = area.height.saturating_sub(2) as usize;

        // Build rows from top to bottom
        let mut rows: Vec<String> = Vec::new();
        for row in 0..inner_h.max(1) {
            let mut line = String::with_capacity(inner_w.saturating_add(2));
            for col in 0..inner_w.max(1) {
                // Simple deterministic pattern: vary amplitude with (tick + col)
                let phase = ((self.tick as usize + col) % 16) as i32;
                // map phase and row to a character density
                let amp = ((phase + (row as i32 * 3)) % 16).abs();
                let ch = match amp {
                    0 | 1 | 2 => ' ',
                    3 | 4 | 5 => '.',
                    6 | 7 | 8 => '*',
                    9 | 10 | 11 => '█',
                    _ => '█',
                };
                line.push(ch);
            }
            rows.push(line);
        }

        let text = rows.join("\n");

        let block = Block::default().borders(Borders::ALL).title("Visualizer");
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, area);
    }
}
