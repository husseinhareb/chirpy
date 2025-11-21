// src/ui/tui.rs
//! Terminal UI event loop and rendering.

use std::{
    io,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, Event as CEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;

/// Run the terminal UI application.
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
        // Pull any ready metadata from background loader
        app.process_metadata();

        // Update visualizer at a slower rate (30 Hz)
        if last_visualizer_update.elapsed() >= visualizer_update_rate {
            app.update_visualizer();
            last_visualizer_update = Instant::now();
        }

        terminal.draw(|f| app.draw(f))?;
        let timeout = frame_rate.checked_sub(last_frame.elapsed()).unwrap_or_default();

        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                if app.on_key(key) {
                    // Quit requested
                    break;
                }
            }
        }

        if last_frame.elapsed() >= frame_rate {
            last_frame = Instant::now();
        }

        // Update elapsed time counter every second
        if last_second.elapsed() >= Duration::from_secs(1) {
            last_second = Instant::now();
            app.tick_elapsed();
        }
    }

    // Clean up terminal
    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}
