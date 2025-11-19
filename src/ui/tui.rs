use std::{
    io,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEvent, KeyModifiers},
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
    picker::{Picker, ProtocolType},
};

use crate::{
    ascii_art,
    file_metadata::FileCategory,
    folder_content::{load_entries, tail_path},
    icons::icon_for_entry,
    music_player::{MusicPlayer, TrackMetadata},
    visualizer::Visualizer,
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
    // visualizer
    visualizer: Visualizer,
    // Section visibility toggles (1..4)
    show_files: bool,
    show_player: bool,
    show_artwork: bool,
    show_visualizer: bool,
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
            show_files: true,
            show_player: true,
            show_artwork: true,
            show_visualizer: true,
            visualizer: Visualizer::new(),
        })
    }
    fn on_key(&mut self, key: KeyEvent) {
        // Helper: map digit/shifted-digit to section number (1..4)
        fn map_key_to_digit(k: &KeyEvent) -> Option<usize> {
            if let KeyCode::Char(c) = k.code {
                match c {
                    '1' | '!' => Some(1),
                    '2' | '@' => Some(2),
                    '3' | '#' => Some(3),
                    '4' | '$' => Some(4),
                    _ => None,
                }
            } else {
                None
            }
        }

        // Toggle section visibility when Shift+number pressed. Terminals differ:
        // some report the shifted symbol (e.g. '!' for Shift+1) with no modifier,
        // others set the SHIFT modifier and report '1'. Accept both cases.
        if let Some(d) = map_key_to_digit(&key) {
            let is_shifted_symbol = matches!(key.code,
                KeyCode::Char('!') | KeyCode::Char('@') | KeyCode::Char('#') | KeyCode::Char('$')
            );
            if key.modifiers.contains(KeyModifiers::SHIFT) || is_shifted_symbol {
                match d {
                    1 => self.show_files = !self.show_files,
                    2 => self.show_player = !self.show_player,
                    3 => self.show_artwork = !self.show_artwork,
                    4 => self.show_visualizer = !self.show_visualizer,
                    _ => {}
                }
                return;
            }
        }

        match key.code {
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

        // Reserve bottom 20% of the terminal for the audio visualizer only if
        // the visualizer is enabled; otherwise the main UI gets 100% of the area.
        let (main_area, bottom_area_opt) = if self.show_visualizer {
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(area);
            (vertical_chunks[0], Some(vertical_chunks[1]))
        } else {
            (area, None)
        };

        // Build column weights dynamically based on visible sections so remaining
        // components expand to fill space (responsive behavior).
        let mut section_order = Vec::new();
        let mut weights = Vec::new();
        if self.show_files {
            section_order.push("files");
            weights.push(18u16);
        }
        if self.show_player {
            section_order.push("player");
            weights.push(54u16);
        }
        if self.show_artwork {
            section_order.push("artwork");
            weights.push(28u16);
        }

        let cols: Vec<Rect> = if !weights.is_empty() {
            let sum: u16 = weights.iter().copied().sum();
            let constraints: Vec<Constraint> = weights
                .into_iter()
                .map(|w| Constraint::Percentage((w as u32 * 100 / sum as u32) as u16))
                .collect();
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .split(main_area)
                .iter()
                .cloned()
                .collect()
        } else {
            // If no columns visible, create a single full-width column so code below
            // can still index safely (it will be unused).
            vec![main_area]
        };

        // Render visible columns in order; map each visible section to the next
        // rect in `cols` so that hiding/showing sections reflows the UI.
        let mut col_index = 0usize;

        // Prepare file list widget data once
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|(name, is_dir, category, _)| {
                ListItem::new(format!("{} {}", icon_for_entry(*is_dir, category), name))
            })
            .collect();

        for section in section_order.iter() {
            match *section {
                "files" => {
                    if col_index < cols.len() {
                        let title = format!("1:  {}", tail_path(&self.current_dir, 3));
                        let list = List::new(items.clone())
                            .block(Block::default().borders(Borders::ALL).title(title))
                            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                            .highlight_symbol(">> ");
                        f.render_stateful_widget(list, cols[col_index], &mut self.state);
                    }
                    col_index += 1;
                }
                "player" => {
                    if col_index < cols.len() {
                        let title = "2: Player";
                        f.render_widget(Block::default().borders(Borders::ALL).title(title), cols[col_index]);
                        let inner = Layout::default()
                            .direction(Direction::Vertical)
                            .margin(1)
                            .constraints([Constraint::Min(1), Constraint::Length(3)])
                            .split(cols[col_index]);

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
                    }
                    col_index += 1;
                }
                "artwork" => {
                    if col_index < cols.len() {
                        let title = "3: Artwork (ASCII)";
                        let art_area = cols[col_index];
                        let block = Block::default().borders(Borders::ALL).title(title);
                        let inner = block.inner(art_area);
                        f.render_widget(block, art_area);

                        if let Some(dyn_img) = &self.artwork {
                            // Use inner area for ASCII art (no extra margins needed)
                            let width = inner.width as usize;
                            let height = inner.height as usize;
                            
                            if width > 0 && height > 0 {
                                // Convert image to ASCII art
                                let ascii_art = ascii_art::image_to_colored_ascii(dyn_img, width, height);
                                
                                // Render ASCII art as a paragraph
                                let paragraph = Paragraph::new(ascii_art)
                                    .wrap(Wrap { trim: false });
                                f.render_widget(paragraph, inner);
                            }
                        }
                    }
                    col_index += 1;
                }
                _ => {}
            }
        }

        // Bottom pane: audio spectrum visualizer (20% height, full width)
        if let Some(bottom_area) = bottom_area_opt {
            // Render the real-time spectrum visualizer
            self.visualizer.render(f, bottom_area);
        }
    }

}

pub fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new()?;
    
    // High refresh rate for smooth drawing (60 Hz = ~16ms per frame)
    let frame_rate = Duration::from_millis(16);
    let mut last_frame = Instant::now();
    
    // Update visualizer less frequently (30 Hz = ~33ms)
    let visualizer_update_rate = Duration::from_millis(33);
    let mut last_visualizer_update = Instant::now();
    
    // Track elapsed seconds separately (update every second)
    let mut last_second = Instant::now();

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

        // Update visualizer at a slower rate (30 Hz) for smoother, more responsive animation
        if last_visualizer_update.elapsed() >= visualizer_update_rate {
            app.visualizer.update(&app.player.sample_buffer);
            last_visualizer_update = Instant::now();
        }
        
        terminal.draw(|f| app.draw(f))?;
        let timeout = frame_rate.checked_sub(last_frame.elapsed()).unwrap_or_default();

        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                app.on_key(key);
            }
        }

        if last_frame.elapsed() >= frame_rate {
            last_frame = Instant::now();
        }
        
        // Update elapsed time counter every second
        if last_second.elapsed() >= Duration::from_secs(1) {
            last_second = Instant::now();
            if app.player.is_playing() && !app.player.is_paused() {
                app.elapsed = (app.elapsed + 1).min(app.duration);
            }
        }
    }
}
