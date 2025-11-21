// src/fs/detection.rs
//! File type detection using magic numbers and extension-based fallback.

use std::{fmt, path::Path};

use anyhow::Result;
use infer::{Infer, MatcherType};
use mime_guess::MimeGuess;

/// High-level file categories.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FileCategory {
    Image,
    Audio,
    Video,
    Document,
    Binary,
}

impl fmt::Display for FileCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FileCategory::Image => "Image",
            FileCategory::Audio => "Audio",
            FileCategory::Video => "Video",
            FileCategory::Document => "Document",
            FileCategory::Binary => "Binary",
        };
        write!(f, "{}", s)
    }
}

/// Holds a detected MIME type + category.
#[derive(Debug)]
pub struct FileType {
    pub mime: String,
    pub category: FileCategory,
}

/// Detect MIME type & category for a given file path.
pub fn detect_file_type(path: &Path) -> Result<FileType> {
    // 1. Try magic-number sniffing
    if let Some(kind) = Infer::new().get_from_path(path)? {
        let mime = kind.mime_type().to_string();
        let category = match kind.matcher_type() {
            MatcherType::Image => FileCategory::Image,
            MatcherType::Audio => FileCategory::Audio,
            MatcherType::Video => FileCategory::Video,
            _ => FileCategory::Binary,
        };
        return Ok(FileType { mime, category });
    }

    // 2. Fallback to extension-based lookup
    let guess = MimeGuess::from_path(path);
    let mime = guess
        .first_or_octet_stream() // defaults to application/octet-stream
        .to_string();

    // 3. Map top-level type to category
    let category = match mime.split('/').next().unwrap_or("application") {
        "image" => FileCategory::Image,
        "audio" => FileCategory::Audio,
        "video" => FileCategory::Video,
        "text" => FileCategory::Document,
        "application" => FileCategory::Document,
        _ => FileCategory::Binary,
    };

    Ok(FileType { mime, category })
}
