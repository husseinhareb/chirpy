// src/app/state.rs
//! Application state management.

use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    thread,
};

use anyhow::Result;
use image::DynamicImage;
use ratatui::{layout::Rect, widgets::ListState, Frame};
use ratatui_image::picker::{Picker, ProtocolType};

use crate::{
    audio::{MusicPlayer, TrackMetadata, Visualizer},
    fs::{load_entries, tail_path, FileCategory},
    ui::{
        keybindings::{key_to_action, NavigationAction},
        layout::{compute_layout, SectionVisibility},
        widgets::{render_artwork, render_file_list, render_player_panel, render_spectrum},
    },
};

use crossterm::event::KeyEvent;

/// Main application state.
pub struct App {
    /// Current directory being browsed
    pub current_dir: PathBuf,
    /// Directory entries (name, is_dir, category, mime)
    pub entries: Vec<(String, bool, FileCategory, String)>,
    /// List widget state
    pub state: ListState,
    /// Currently selected index
    pub selected: usize,

    /// Music player instance
    pub player: MusicPlayer,
    /// Elapsed playback time in seconds
    pub elapsed: u64,
    /// Total track duration in seconds
    pub duration: u64,

    /// Image picker for artwork rendering
    #[allow(dead_code)]
    picker: Picker,
    /// Current artwork image
    #[allow(dead_code)]
    pub artwork: Option<DynamicImage>,

    /// Metadata channel sender (background loader -> UI)
    pub meta_tx: Sender<TrackMetadata>,
    /// Metadata channel receiver
    pub meta_rx: Receiver<TrackMetadata>,

    /// Audio spectrum visualizer
    pub visualizer: Visualizer,

    /// Section visibility state
    pub visibility: SectionVisibility,
}

impl App {
    /// Create a new application instance.
    pub fn new() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let mut state = ListState::default();
        state.select(Some(0));

        // Create picker with fallback if stdio query fails
        let mut picker =
            Picker::from_query_stdio().unwrap_or_else(|_| Picker::from_fontsize((8, 12)));
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
            visibility: SectionVisibility::default(),
            visualizer: Visualizer::new(),
        })
    }

    /// Handle a key event and return true if the app should quit.
    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        let action = key_to_action(&key);

        match action {
            NavigationAction::ToggleSection(d) => {
                self.visibility.toggle(d);
            }
            NavigationAction::Down => {
                if self.selected + 1 < self.entries.len() {
                    self.selected += 1;
                }
            }
            NavigationAction::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            NavigationAction::Enter => {
                if !self.entries.is_empty() {
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

                            // Spawn a background thread to load metadata
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
            }
            NavigationAction::TogglePause => {
                if self.player.is_paused() {
                    self.player.resume();
                } else {
                    self.player.pause();
                }
            }
            NavigationAction::Back => {
                if self.current_dir.pop() {
                    self.entries = load_entries(&self.current_dir);
                    self.selected = 0;
                }
            }
            NavigationAction::Quit => {
                self.player.stop();
                return true; // Signal to quit
            }
            NavigationAction::None => {}
        }

        self.state.select(Some(self.selected));
        false
    }

    /// Draw the application UI.
    pub fn draw(&mut self, f: &mut Frame<'_>) {
        let area = f.area();
        let layout = compute_layout(area, &self.visibility);

        // Render visible columns in order
        let mut col_index = 0usize;

        for section in layout.section_order.iter() {
            match *section {
                "files" => {
                    if col_index < layout.columns.len() {
                        let title = format!("1:  {}", tail_path(&self.current_dir, 3));
                        render_file_list(
                            f,
                            layout.columns[col_index],
                            &title,
                            &self.entries,
                            &mut self.state,
                        );
                    }
                    col_index += 1;
                }
                "player" => {
                    if col_index < layout.columns.len() {
                        render_player_panel(
                            f,
                            layout.columns[col_index],
                            self.player.metadata.as_ref(),
                            self.elapsed,
                            self.duration,
                        );
                    }
                    col_index += 1;
                }
                "artwork" => {
                    if col_index < layout.columns.len() {
                        render_artwork(f, layout.columns[col_index]);
                    }
                    col_index += 1;
                }
                _ => {}
            }
        }

        // Bottom pane: audio spectrum visualizer
        if let Some(visualizer_area) = layout.visualizer_area {
            render_spectrum(f, visualizer_area, &self.visualizer);
        }
    }

    /// Update the visualizer with new audio samples.
    pub fn update_visualizer(&mut self) {
        self.visualizer.update(&self.player.sample_buffer);
    }

    /// Process any pending metadata from background loader.
    pub fn process_metadata(&mut self) {
        if let Ok(meta) = self.meta_rx.try_recv() {
            self.player.metadata = Some(meta);
            self.duration = self
                .player
                .metadata
                .as_ref()
                .map(|m| m.duration_secs.max(1))
                .unwrap_or(1);
        }
    }

    /// Update elapsed time if playing.
    pub fn tick_elapsed(&mut self) {
        if self.player.is_playing() && !self.player.is_paused() {
            self.elapsed = (self.elapsed + 1).min(self.duration);
        }
    }
}
