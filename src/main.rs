use std::{
    env,
    error::Error,
    fs,
    io,
    time::{Duration, Instant},
};

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

fn main() -> Result<(), Box<dyn Error>> {
    // 1. Read current directory entries
    let cwd = env::current_dir()?;
    let mut entries: Vec<String> = fs::read_dir(&cwd)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();

    // 2. Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. App state: selected index + ListState
    let mut state = ListState::default();
    let mut selected = 0;

    // 4. Main loop
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();
    loop {
        // Update selection in state
        state.select(Some(selected));

        // Draw the UI
        terminal.draw(|f| {
            let area = f.area();  // replaced deprecated f.size()

            // split into title + list
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(area);

            // Title block
            let title = Block::default()
                .title(format!("Directory: {}", cwd.display()))
                .borders(Borders::ALL);
            f.render_widget(title, chunks[0]);

            // Build list items directly from &str
            let items: Vec<ListItem> = entries
                .iter()
                .map(|name| ListItem::new(name.as_str()))
                .collect();

            // List widget with highlighting
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Files"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol(">> ");

            // Render with our ListState
            f.render_stateful_widget(list, chunks[1], &mut state);
        })?;

        // Input handling
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,                              // quit
                    KeyCode::Down if selected + 1 < entries.len() => selected += 1,
                    KeyCode::Up if selected > 0 => selected -= 1,
                    _ => {}
                }
            }
        }

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
