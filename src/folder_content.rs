// src/folder_content.rs

use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use crate::file_metadata::{detect_file_type, FileCategory};

/// Returns the last `n` components of `path` joined by `/`. If the path has
/// fewer than `n` components, returns the full path.
pub fn tail_path(path: &Path, n: usize) -> String {
    let comps: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            Component::RootDir => Some(String::from("/")),
            Component::Normal(os) => Some(os.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    let (prefix, body) = if comps.first().map(|s| s == "/").unwrap_or(false) {
        (Some("/"), &comps[1..])
    } else {
        (None, &comps[..])
    };

    let slice = if body.len() <= n {
        body
    } else {
        &body[body.len().saturating_sub(n)..]
    };

    match prefix {
        Some(_) => format!("/{}", slice.join("/")),
        None => slice.join("/"),
    }
}

/// Load the entries of `dir`, returning a Vec of
/// (name, is_dir, category, mime)
pub fn load_entries(dir: &PathBuf) -> Vec<(String, bool, FileCategory, String)> {
    let mut list = fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if path.is_dir() {
                (name, true, FileCategory::Binary, String::new())
            } else {
                match detect_file_type(&path) {
                    Ok(ft) => (name, false, ft.category, ft.mime),
                    Err(_) => (name, false, FileCategory::Binary, String::new()),
                }
            }
        })
        .collect::<Vec<_>>();
    list.sort_by_key(|(n, _, _, _)| n.to_lowercase());
    list
}
