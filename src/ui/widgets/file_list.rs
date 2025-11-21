// src/ui/widgets/file_list.rs
//! File browser list widget.

use ratatui::{
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
    layout::Rect,
};

use crate::fs::FileCategory;
use crate::ui::icons::icon_for_entry;

/// Render the file browser list.
pub fn render_file_list(
    f: &mut Frame<'_>,
    area: Rect,
    title: &str,
    entries: &[(String, bool, FileCategory, String)],
    state: &mut ListState,
) {
    let items: Vec<ListItem> = entries
        .iter()
        .map(|(name, is_dir, category, _)| {
            ListItem::new(format!("{} {}", icon_for_entry(*is_dir, category), name))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title.to_string()))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, state);
}
