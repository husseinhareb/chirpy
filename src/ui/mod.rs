// src/ui/mod.rs
//! UI module - handles terminal interface rendering and input.

pub mod icons;
pub mod keybindings;
pub mod layout;
pub mod tui;
pub mod widgets;

// Re-export main entry point
pub use tui::run;