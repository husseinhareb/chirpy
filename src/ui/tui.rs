use std::{
    io,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::DynamicImage;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use ratatui_image::{
    Image,
    picker::{Picker, ProtocolType},
    Resize,
};

use crate::{
    file_metadata::FileCategory,
    folder_content::{load_entries, tail_path},
    icons::icon_for_entry,
    music_player::{MusicPlayer, TrackMetadata},
};

pub struct App {
    current_dir: PathBuf,
    entries: Vec<(String, bool, FileCategory, String)>,
    state: ListState,
    selected: usize,

    player: MusicPlayer,
    elapsed: u64,
    duration: u64,

    picker: Picker,
    artwork: Option<DynamicImage>,
    // metadata channel: background loader -> UI
    meta_tx: Sender<TrackMetadata>,
    meta_rx: Receiver<TrackMetadata>,
}

impl App {
    pub fn new() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let mut state = ListState::default();
        state.select(Some(0));

        let mut picker = Picker::from_query_stdio()?;
        picker.set_protocol_type(ProtocolType::Kitty);

        let (meta_tx, meta_rx) = std::sync::mpsc::channel::<TrackMetadata>();

        Ok(Self {
            current_dir: cwd.clone(),
            entries: load_entries(&cwd),
            state,
            selected: 0,

            player: MusicPlayer::new(),
            elapsed: 0,
            duration: 1,

            picker,
            artwork: None,
            meta_tx,
            meta_rx,
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
                        // Clear any prior metadata while background loader runs
                        self.player.metadata = None;
                        self.elapsed = 0;
                        self.duration = 1;
                        self.artwork = None;

                        // Spawn a background thread to load metadata (Lofty probing)
                        let tx = self.meta_tx.clone();
                        let path_clone = path.clone();
                        thread::spawn(move || {
                            if let Ok(meta) = MusicPlayer::load_metadata(path_clone) {
                                let _ = tx.send(meta);
                            }
                        });
                    }
                }
            }
            KeyCode::Char(' ') => {
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

        // Reserve bottom 20% of the terminal for the audio visualizer and use the
        // remaining top 80% for the existing UI (file list, player, artwork).
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
            .split(area);

        let main_area = vertical_chunks[0];
        let bottom_area = vertical_chunks[1];

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(18),
                Constraint::Percentage(54),
                Constraint::Percentage(28),
            ])
            .split(main_area);

        // Left pane: file list
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|(name, is_dir, category, _)| {
                ListItem::new(format!("{} {}", icon_for_entry(*is_dir, category), name))
            })
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                " {}",
                tail_path(&self.current_dir, 3)
            )))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ");
        f.render_stateful_widget(list, cols[0], &mut self.state);

        // Middle pane: metadata + progress
        f.render_widget(Block::default().borders(Borders::ALL).title("Player"), cols[1]);
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(cols[1]);

        if let Some(TrackMetadata { tags, properties, duration_secs, .. }) = &self.player.metadata {
            let mut lines = vec![format!("Duration: {}s", duration_secs)];
            for (k, v) in tags { lines.push(format!("{}: {}", k, v)); }
            for (k, v) in properties { lines.push(format!("{}: {}", k, v)); }
            f.render_widget(
                Paragraph::new(lines.join("\n")).wrap(Wrap { trim: true }),
                inner[0],
            );
        } else {
            f.render_widget(
                Paragraph::new("▶️ No track playing").wrap(Wrap { trim: true }),
                inner[0],
            );
        }

        let ratio = (self.elapsed as f64 / self.duration as f64).clamp(0.0, 1.0);
        f.render_widget(
            Gauge::default().gauge_style(Style::default().add_modifier(Modifier::ITALIC)).ratio(ratio),
            inner[1],
        );

        // Right pane: responsive artwork with margin
        let art_area = cols[2];
        f.render_widget(Block::default().borders(Borders::ALL).title("Artwork"), art_area);

        if let Some(dyn_img) = &self.artwork {
            // inner dimensions (leave 1-cell margin inside border)
            let inner_w = art_area.width.saturating_sub(2);
            let inner_h = art_area.height.saturating_sub(2);
            // square size no larger than inner_w and inner_h
            let size = inner_w.min(inner_h);
            // center in inner rect
            let offset_x = art_area.x + 1 + ((inner_w - size) / 2);
            let offset_y = art_area.y + 1 + ((inner_h - size) / 2);
            let draw_area = Rect::new(offset_x, offset_y, size, size);

            // protocol size matches draw_area cell dimensions
            let proto_size = Rect::new(0, 0, size, size);
            if let Ok(proto) = self.picker.new_protocol(dyn_img.clone(), proto_size, Resize::Fit(None)) {
                let img_widget = Image::new(&proto);
                f.render_widget(img_widget, draw_area);
            }
        }

        // Bottom pane: audio visualizer placeholder (20% height, full width)
        f.render_widget(
            Block::default().borders(Borders::ALL).title("Visualizer"),
            bottom_area,
        );
    }

pub fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new()?;
    let tick_rate = Duration::from_secs(1);
    let mut last_tick = Instant::now();

    loop {
        // Pull any ready metadata from background loader and apply it before drawing.
        if let Ok(meta) = app.meta_rx.try_recv() {
            // Only update metadata and duration here. Do NOT eagerly load artwork into
            // `app.artwork` because creating terminal image protocols on every draw
            // can flood the terminal and make it unresponsive. Artwork can be loaded
            // on demand (e.g., with a dedicated key) in the future.
            app.player.metadata = Some(meta);
            app.duration = app
                .player
                .metadata
                .as_ref()
                .map(|m| m.duration_secs.max(1))
                .unwrap_or(1);
        }

        terminal.draw(|f| app.draw(f))?;
        let timeout = tick_rate.checked_sub(last_tick.elapsed()).unwrap_or_default();

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
