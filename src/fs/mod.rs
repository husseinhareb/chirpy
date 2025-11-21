// src/fs/mod.rs
//! Filesystem module - handles file browsing and type detection.

pub mod browser;
pub mod detection;

// Re-export commonly used types
pub use browser::{load_entries, tail_path};
pub use detection::{detect_file_type, FileCategory, FileType};
