// src/ui/icons.rs
//! Icon mappings for file types in the browser.

use crate::fs::FileCategory;

/// Get the appropriate icon for a file or directory entry.
pub fn icon_for_entry(is_dir: bool, category: &FileCategory) -> &'static str {
    if is_dir {
        "\u{f07b}" // folder icon
    } else {
        match category {
            FileCategory::Audio => "\u{f1c7}",
            FileCategory::Image => "\u{f1c5}",
            FileCategory::Video => "\u{f1c8}",
            FileCategory::Document => "\u{f15c}",
            FileCategory::Binary => "\u{f1c6}",
        }
    }
}
