// src/folder_content.rs

use std::{ fs, path::{ PathBuf, Component, Path } };
use crate::file_metadata::{ detect_file_type, FileCategory };

/// Returns the last `n` components of `path` joined by `/`. If the path has
/// fewer than `n` components, returns the full path.
pub fn tail_path(path: &Path, n: usize) -> String {
    let comps: Vec<String> = path
        .components()
        .filter_map(|c| {
            match c {
                Component::RootDir => Some("/".to_string()),
                Component::Normal(os) => Some(os.to_string_lossy().into_owned()),
                _ => None,
            }
        })
        .collect();

    let (prefix, body) = if
        comps
            .first()
            .map(|s| s == "/")
            .unwrap_or(false)
    {
        (Some("/"), &comps[1..])
    } else {
        (None, &comps[..])
    };

    let slice = if body.len() <= n { body } else { &body[body.len().saturating_sub(n)..] };

    match prefix {
        Some(_) => format!("/{}", slice.join("/")),
        None => slice.join("/"),
    }
}

/// Load **only** directories and audio files from `dir`, returning a Vec of
/// (name, is_dir, category, mime)
pub fn load_entries(dir: &PathBuf) -> Vec<(String, bool, FileCategory, String)> {
    let mut list = fs
        ::read_dir(dir)
        .unwrap() // you might replace this with `?` and a Result in real code
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();

            // Skip hidden files and folders (those starting with a dot)
            if name.starts_with('.') {
                return None;
            }

            if path.is_dir() {
                // Always include directories so we can navigate into them
                Some((name, true, FileCategory::Binary, String::new()))
            } else {
                // Only include if it's an audio file
                match detect_file_type(&path) {
                    Ok(ft) if ft.category == FileCategory::Audio => {
                        Some((name, false, ft.category, ft.mime))
                    }
                    _ => None, // skip non-audio files
                }
            }
        })
        .collect::<Vec<_>>();

    // Sort alphabetically
    list.sort_by_key(|(n, _, _, _)| n.to_lowercase());
    list
}
