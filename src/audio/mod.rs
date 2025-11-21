// src/audio/mod.rs
//! Audio module - handles all audio playback, metadata, and visualization.

pub mod metadata;
pub mod player;
pub mod sample_capture;
pub mod visualizer;

// Re-export commonly used types
pub use metadata::{TagEntry, TrackMetadata};
pub use player::MusicPlayer;
pub use sample_capture::SampleCapture;
pub use visualizer::Visualizer;
