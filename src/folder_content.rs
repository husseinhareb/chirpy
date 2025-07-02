// src/folder_content.rs

use std::{
    env,
    fs,
    io,
    path::PathBuf,
    time::{Duration, Instant},
};
use anyhow::Result;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Terminal,
};
use crate::file_metadata::{detect_file_type, FileCategory};
use crate::icons::icon_for_entry;

/// Load the entries of `dir`, returning a Vec of
/// (name, is_dir, category, mime)
fn load_entries(dir: &PathBuf) -> Vec<(String, bool, FileCategory, String)> {
    let mut list = fs::read_dir(dir)
        .unwrap() // handle errors appropriately in real code
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

/// Run the TUI file browser with Left/Right navigation and icons,
/// and reserve 15% of the width for the file list pane.
pub fn run() -> Result<()> {
    // 1. Track current directory and its entries
    let mut current_dir = env::current_dir()?;
    let mut entries = load_entries(&current_dir);

    // 2. Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. App state
    let mut list_state = ListState::default();
    let mut selected = 0;
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    // 4. Main event loop
    loop {
        // Clamp selection
        if selected >= entries.len() {
            selected = entries.len().saturating_sub(1);
        }
        list_state.select(Some(selected));

        // Draw the UI
        terminal.draw(|f| {
            let area = f.area();

            // Horizontal split: 15% for list pane, 85% for detail pane
            let h_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(15), Constraint::Percentage(85)].as_ref())
                .split(area);

            // Inside left pane, vertical split: header + list
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(h_chunks[0]);

            // Header: show current directory path
            let header = Block::default()
                .title(format!("Directory: {}", current_dir.display()))
                .borders(Borders::ALL);
            f.render_widget(header, left_chunks[0]);

            // Prepare list items: icon + name only
            let items: Vec<ListItem> = entries
                .iter()
                .map(|(name, is_dir, category, _)| {
                    let icon = icon_for_entry(*is_dir, category);
                    let line = format!("{} {}", icon, name);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Files"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, left_chunks[1], &mut list_state);

            // The right pane (h_chunks[1]) is left empty for future details...
        })?;

        // Handle input
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_default();
        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down if selected + 1 < entries.len() => selected += 1,
                    KeyCode::Up if selected > 0 => selected -= 1,
                    KeyCode::Right => {
                        let (name, is_dir, _, _) = &entries[selected];
                        if *is_dir {
                            current_dir.push(name);
                            entries = load_entries(&current_dir);
                            selected = 0;
                        }
                    }
                    KeyCode::Left => {
                        if current_dir.pop() {
                            entries = load_entries(&current_dir);
                            selected = 0;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Tick update
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // 5. Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
