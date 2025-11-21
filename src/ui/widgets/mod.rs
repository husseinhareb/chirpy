// src/ui/widgets/mod.rs
//! Custom widgets for the chirpy UI.

pub mod artwork;
pub mod file_list;
pub mod player_panel;
pub mod spectrum;

// Re-export widget rendering functions
pub use artwork::render_artwork;
pub use file_list::render_file_list;
pub use player_panel::render_player_panel;
pub use spectrum::render_spectrum;
