/// src/folder_content.rs
use std::{env, fs, io, time::{Duration, Instant}};
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

/// Run the TUI file browser
pub fn run() -> Result<()> {
    // Load entries + types
    let cwd = env::current_dir()?;
    let mut entries: Vec<(String, String)> = fs::read_dir(&cwd)?
        .filter_map(|e| e.ok())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            let kind = if fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false) {
                "Folder".to_string()
            } else {
                match detect_file_type(&path) {
                    Ok(ft) => ft.category.to_string(),
                    Err(_) => "Unknown".into(),
                }
            };
            (name, kind)
        })
        .collect();
    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // State
    let mut state = ListState::default();
    let mut selected = 0;
    let tick = Duration::from_millis(200);
    let mut last = Instant::now();

    // Main loop
    loop {
        state.select(Some(selected));
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(area);

            // Header
            let header = Block::default()
                .title(format!("Directory: {}", cwd.display()))
                .borders(Borders::ALL);
            f.render_widget(header, chunks[0]);

            // Build list
            let items: Vec<ListItem> = entries.iter()
                .map(|(name, kind)| {
                    // Name padded to 40 chars
                    let line = format!("{:<40} {}", name, kind);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Files"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, chunks[1], &mut state);
        })?;

        // Input
        let timeout = tick.checked_sub(last.elapsed()).unwrap_or_default();
        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down if selected + 1 < entries.len() => selected += 1,
                    KeyCode::Up if selected > 0 => selected -= 1,
                    _ => {}
                }
            }
        }
        if last.elapsed() >= tick {
            last = Instant::now();
        }
    }

    // Restore
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
