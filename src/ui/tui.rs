use std::{
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
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use crate::folder_content::{load_entries, tail_path};
use crate::file_metadata::FileCategory;
use crate::icons::icon_for_entry;
use crate::music_player::{MusicPlayer, TrackMetadata};

pub struct App {
    current_dir: PathBuf,
    entries: Vec<(String, bool, FileCategory, String)>,
    state: ListState,
    selected: usize,
    player: MusicPlayer,
    elapsed: u64,   // elapsed seconds
    duration: u64,  // total duration in seconds
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
            player: MusicPlayer::new(),
            elapsed: 0,
            duration: 1, // avoid division by zero
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
            KeyCode::Enter | KeyCode::Right => {
                let (name, is_dir, category, _) = &self.entries[self.selected];
                let path = self.current_dir.join(name);
                if *is_dir {
                    self.current_dir.push(name);
                    self.entries = load_entries(&self.current_dir);
                    self.selected = 0;
                } else if *category == FileCategory::Audio {
                    if self.player.play(&path).is_ok() {
                        self.elapsed = 0;
                        if let Some(TrackMetadata { duration_secs, .. }) = &self.player.metadata {
                            self.duration = *duration_secs.max(&1);
                        }
                    }
                }
            }
            KeyCode::Char(' ') => {
                // toggle pause/resume
                if self.player.is_paused() {
                    self.player.resume();
                } else {
                    self.player.pause();
                }
            }
            KeyCode::Left => {
                if self.current_dir.pop() {
                    self.entries = load_entries(&self.current_dir);
                    self.selected = 0;
                }
            }
            KeyCode::Char('q') => {
                // stop playback, restore terminal, and exit
                self.player.stop();
                execute!(io::stdout(), LeaveAlternateScreen).ok();
                disable_raw_mode().ok();
                std::process::exit(0);
            }
            _ => {}
        }
        self.state.select(Some(self.selected));
    }

    fn draw(&mut self, f: &mut Frame<'_>) {
        let area = f.area();
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(18),
                Constraint::Percentage(64),
                Constraint::Percentage(18),
            ])
            .split(area);

        // --- Left pane: folder list ---
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

        f.render_stateful_widget(list, cols[0], &mut self.state);

        // --- Middle pane: Player block ---
        let player_block = Block::default().borders(Borders::ALL).title("Player");
        f.render_widget(player_block, cols[1]);

        // split inside Player: metadata + progress bar
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(cols[1]);

        // --- Metadata display ---
        if let Some(TrackMetadata { tags, properties, duration_secs, lyrics: _ }) = &self.player.metadata {
            let mut lines = Vec::new();
            lines.push(format!("Duration: {}s", duration_secs));
            for (k, v) in tags {
                lines.push(format!("{}: {}", k, v));
            }
            for (k, v) in properties {
                lines.push(format!("{}: {}", k, v));
            }
            let paragraph = Paragraph::new(lines.join("\n"))
                .wrap(Wrap { trim: true });
            f.render_widget(paragraph, inner[0]);
        } else {
            let paragraph = Paragraph::new("▶️ No track playing")
                .wrap(Wrap { trim: true });
            f.render_widget(paragraph, inner[0]);
        }

        // --- Progress bar ---
        let ratio = (self.elapsed as f64 / self.duration as f64).clamp(0.0, 1.0);
        let gauge = Gauge::default()
            .gauge_style(Style::default().add_modifier(Modifier::ITALIC))
            .ratio(ratio);
        f.render_widget(gauge, inner[1]);
    }
}

pub fn run() -> Result<()> {
    // enter raw mode and alternate screen
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // create terminal and clear existing content
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // initialize app
    let mut app = App::new()?;
    let tick_rate = Duration::from_secs(1);
    let mut last_tick = Instant::now();

    // main loop
    loop {
        terminal.draw(|f| app.draw(f))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_default();

        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                app.on_key(key.code);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
            if app.player.is_playing() && !app.player.is_paused() {
                app.elapsed = (app.elapsed + 1).min(app.duration);
            }
        }
    }
}
