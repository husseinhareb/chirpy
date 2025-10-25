//src/main.rs
mod file_metadata;
mod icons;
mod folder_content;
mod visualizer;
mod ui;
mod music_player;
fn main() -> anyhow::Result<()> {
    ui::tui::run()
}
