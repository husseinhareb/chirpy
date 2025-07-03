// src/ui/tui.rs

use std::{
    io::{self, Stdout},
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
    Frame, Terminal,
};
use crate::folder_content::{load_entries, tail_path};
use crate::file_metadata::FileCategory;
use crate::icons::icon_for_entry;

pub struct App {
    current_dir: PathBuf,
    entries: Vec<(String, bool, FileCategory, String)>,
    state: ListState,
    selected: usize,
}

impl App {
    pub fn new() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let mut state = ListState::default();
        state.select(Some(0));
        Ok(Self {
            current_dir: cwd.clone(),
            entries: load_entries(&cwd),
            state,
            selected: 0,
        })
    }

    fn on_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Down => {
                if self.selected + 1 < self.entries.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Right => {
                let (name, is_dir, _, _) = &self.entries[self.selected];
                if *is_dir {
                    self.current_dir.push(name);
                    self.entries = load_entries(&self.current_dir);
                    self.selected = 0;
                }
            }
            KeyCode::Left => {
                if self.current_dir.pop() {
                    self.entries = load_entries(&self.current_dir);
                    self.selected = 0;
                }
            }
            _ => {}
        }
        self.state.select(Some(self.selected));
    }

    fn draw(&mut self, f: &mut Frame<'_>) {
        // Use .area() instead of deprecated .size()
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(18), Constraint::Percentage(82)].as_ref())
            .split(area);

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|(name, is_dir, category, _)| {
                let icon = icon_for_entry(*is_dir, category);
                ListItem::new(format!("{} {}", icon, name))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {}", tail_path(&self.current_dir, 3))),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, chunks[0], &mut self.state);
    }
}

pub fn run() -> Result<()> {
    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup app state
    let mut app = App::new()?;
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    // Main event loop
    loop {
        terminal.draw(|f| app.draw(f))?;

        let timeout = tick_rate.checked_sub(last_tick.elapsed()).unwrap_or_default();
        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
                app.on_key(key.code);
            }
        }

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

