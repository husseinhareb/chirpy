// src/folder_content.rs

use std::{
    env,
    fs,
    io,
    path::{PathBuf},
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
use crate::file_metadata::detect_file_type;

/// Load directory entries for `dir`, returning a Vec of (name, category, mime)
fn load_entries(dir: &PathBuf) -> Vec<(String, String, String)> {
    let mut list = fs::read_dir(dir)
        .unwrap() // in real code, handle errors
        .filter_map(|e| e.ok())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if path.is_dir() {
                (name, "Folder".to_string(), String::new())
            } else {
                match detect_file_type(&path) {
                    Ok(ft) => (name, ft.category.to_string(), ft.mime),
                    Err(_) => (name, "Unknown".to_string(), String::new()),
                }
            }
        })
        .collect::<Vec<_>>();
    list.sort_by_key(|(n, _, _)| n.to_lowercase());
    list
}

/// Run the TUI file browser with left/right navigation
pub fn run() -> Result<()> {
    // 1. Track current directory and entries
    let mut current_dir = env::current_dir()?;
    let mut entries = load_entries(&current_dir);

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // State
    let mut state = ListState::default();
    let mut selected = 0;
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    // Main loop
    loop {
        // ensure selection is within bounds
        if selected >= entries.len() {
            selected = entries.len().saturating_sub(1);
        }
        state.select(Some(selected));

        // Draw UI
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(area);

            // Header block: show current path
            let header = Block::default()
                .title(format!("Directory: {}", current_dir.display()))
                .borders(Borders::ALL);
            f.render_widget(header, chunks[0]);

            // List items: name | category | mime
            let items: Vec<ListItem> = entries
                .iter()
                .map(|(name, cat, mime)| {
                    let line = format!(
                        "{:<30} {:<10} {}",
                        name,
                        cat,
                        if mime.is_empty() { "-" } else { mime }
                    );
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Files"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, chunks[1], &mut state);
        })?;

        // Input handling
        let timeout = tick_rate.checked_sub(last_tick.elapsed()).unwrap_or_default();
        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    // Quit
                    KeyCode::Char('q') => break,

                    // Move down/up
                    KeyCode::Down if selected + 1 < entries.len() => selected += 1,
                    KeyCode::Up if selected > 0 => selected -= 1,

                    // Enter folder
                    KeyCode::Right => {
                        let (ref name, ref kind, _) = entries[selected];
                        if kind == "Folder" {
                            current_dir.push(name);
                            entries = load_entries(&current_dir);
                            selected = 0;
                        }
                    }

                    // Go to parent
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

        // Tick update (if you had dynamic content)
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
