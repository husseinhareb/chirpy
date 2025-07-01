/// src/main.rs
use anyhow::Result;

mod file_metadata;
mod folder_content;

fn main() -> Result<()> {
    folder_content::run()
}
