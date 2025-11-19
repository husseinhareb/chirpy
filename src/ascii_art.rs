// src/ascii_art.rs

use image::{DynamicImage, GenericImageView, imageops::FilterType};

/// Convert an image to ASCII art with specified dimensions
pub fn image_to_ascii(img: &DynamicImage, width: usize, height: usize) -> String {
    // ASCII characters ordered from darkest to lightest
    const ASCII_CHARS: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
    
    // Resize image to target dimensions (ASCII cells are roughly 2:1 aspect ratio)
    let resized = img.resize_exact(
        width as u32,
        (height * 2) as u32,  // Double height to compensate for character aspect ratio
        FilterType::Lanczos3,
    );
    
    let mut ascii_art = String::with_capacity(width * height + height);
    
    for y in 0..height {
        for x in 0..width {
            // Sample two vertical pixels and average them for this character cell
            let y1 = (y * 2) as u32;
            let y2 = (y * 2 + 1).min(resized.height() as usize - 1) as u32;
            
            let pixel1 = resized.get_pixel(x as u32, y1);
            let pixel2 = resized.get_pixel(x as u32, y2);
            
            // Average the two pixels
            let r = (pixel1[0] as u16 + pixel2[0] as u16) / 2;
            let g = (pixel1[1] as u16 + pixel2[1] as u16) / 2;
            let b = (pixel1[2] as u16 + pixel2[2] as u16) / 2;
            
            // Convert to grayscale using luminosity method
            let brightness = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8;
            
            // Map brightness to ASCII character
            let char_index = (brightness as usize * (ASCII_CHARS.len() - 1)) / 255;
            ascii_art.push(ASCII_CHARS[char_index]);
        }
        
        if y < height - 1 {
            ascii_art.push('\n');
        }
    }
    
    ascii_art
}

/// Convert an image to colored ASCII art (using ANSI colors)
pub fn image_to_colored_ascii(img: &DynamicImage, width: usize, height: usize) -> String {
    // Use block characters for better visual density
    const BLOCKS: &[char] = &[' ', '░', '▒', '▓', '█'];
    
    // Resize image
    let resized = img.resize_exact(
        width as u32,
        (height * 2) as u32,
        FilterType::Lanczos3,
    );
    
    let mut ascii_art = String::with_capacity(width * height * 20); // Extra space for ANSI codes
    
    for y in 0..height {
        for x in 0..width {
            // Sample two vertical pixels and average
            let y1 = (y * 2) as u32;
            let y2 = (y * 2 + 1).min(resized.height() as usize - 1) as u32;
            
            let pixel1 = resized.get_pixel(x as u32, y1);
            let pixel2 = resized.get_pixel(x as u32, y2);
            
            let r = (pixel1[0] as u16 + pixel2[0] as u16) / 2;
            let g = (pixel1[1] as u16 + pixel2[1] as u16) / 2;
            let b = (pixel1[2] as u16 + pixel2[2] as u16) / 2;
            
            // Calculate brightness for block selection
            let brightness = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8;
            let block_index = (brightness as usize * (BLOCKS.len() - 1)) / 255;
            
            // Add ANSI color code and character
            ascii_art.push_str(&format!(
                "\x1b[38;2;{};{};{}m{}",
                r, g, b, BLOCKS[block_index]
            ));
        }
        
        // Reset color at end of line
        ascii_art.push_str("\x1b[0m");
        
        if y < height - 1 {
            ascii_art.push('\n');
        }
    }
    
    ascii_art
}
