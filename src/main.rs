//src/main.rs
mod file_metadata;
mod icons;
mod folder_content;        
mod ui;          

fn main() -> anyhow::Result<()> {
    ui::tui::run()
}
