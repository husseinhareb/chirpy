// src/ui/layout.rs
//! Layout computation for the UI panels.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Visibility state for UI sections.
#[derive(Debug, Clone, Copy)]
pub struct SectionVisibility {
    pub files: bool,
    pub player: bool,
    pub artwork: bool,
    pub visualizer: bool,
}

impl Default for SectionVisibility {
    fn default() -> Self {
        Self {
            files: true,
            player: true,
            artwork: true,
            visualizer: true,
        }
    }
}

impl SectionVisibility {
    /// Toggle a section by number (1-4).
    pub fn toggle(&mut self, section: usize) {
        match section {
            1 => self.files = !self.files,
            2 => self.player = !self.player,
            3 => self.artwork = !self.artwork,
            4 => self.visualizer = !self.visualizer,
            _ => {}
        }
    }
}

/// Computed layout areas for rendering.
pub struct ComputedLayout {
    /// Main area (upper portion)
    #[allow(dead_code)]
    pub main_area: Rect,
    /// Bottom visualizer area (if visible)
    pub visualizer_area: Option<Rect>,
    /// Column areas within main area
    pub columns: Vec<Rect>,
    /// Order of sections in columns
    pub section_order: Vec<&'static str>,
}

/// Compute the layout based on total area and section visibility.
pub fn compute_layout(area: Rect, visibility: &SectionVisibility) -> ComputedLayout {
    // Reserve bottom 20% of the terminal for the audio visualizer only if
    // the visualizer is enabled; otherwise the main UI gets 100% of the area.
    let (main_area, visualizer_area) = if visibility.visualizer {
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
            .split(area);
        (vertical_chunks[0], Some(vertical_chunks[1]))
    } else {
        (area, None)
    };

    // Build column weights dynamically based on visible sections
    let mut section_order = Vec::new();
    let mut weights = Vec::new();

    if visibility.files {
        section_order.push("files");
        weights.push(18u16);
    }
    if visibility.player {
        section_order.push("player");
        weights.push(54u16);
    }
    if visibility.artwork {
        section_order.push("artwork");
        weights.push(28u16);
    }

    let columns: Vec<Rect> = if !weights.is_empty() {
        let sum: u16 = weights.iter().copied().sum();
        let constraints: Vec<Constraint> = weights
            .into_iter()
            .map(|w| Constraint::Percentage((w as u32 * 100 / sum as u32) as u16))
            .collect();
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(main_area)
            .iter()
            .cloned()
            .collect()
    } else {
        // If no columns visible, create a single full-width column
        vec![main_area]
    };

    ComputedLayout {
        main_area,
        visualizer_area,
        columns,
        section_order,
    }
}
