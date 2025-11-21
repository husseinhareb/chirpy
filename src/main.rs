//src/main.rs
//! Chirpy - A terminal-based music player.

mod app;
mod audio;
mod config;
mod fs;
mod ui;

fn main() -> anyhow::Result<()> {
    ui::run()
}
