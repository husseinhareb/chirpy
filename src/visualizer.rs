// src/visualizer.rs

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::{Arc, Mutex};
use ringbuf::{traits::*, HeapRb};

/// Real-time audio spectrum visualizer using FFT analysis
pub struct Visualizer {
    /// FFT planner for frequency analysis
    fft_planner: FftPlanner<f32>,
    /// Number of frequency bands to display
    num_bands: usize,
    /// Smoothed magnitude values for each band (for visual smoothing)
    smoothed_magnitudes: Vec<f32>,
    /// Smoothing factor (0.0 = no smoothing, 1.0 = maximum smoothing)
    smoothing_factor: f32,
    /// Peak hold values for each band (for classic visualizer effect)
    peak_holds: Vec<f32>,
    /// Peak hold decay rate
    peak_decay: f32,
    /// Auto-sensitivity: current minimum dB threshold (rolling)
    min_db: f32,
    /// Auto-sensitivity: current maximum dB threshold (rolling)
    max_db: f32,
}

impl Visualizer {
    /// Create a new visualizer with specified number of frequency bands
    pub fn new() -> Self {
        let num_bands = 64; // Display 64 frequency bands
        Self {
            fft_planner: FftPlanner::new(),
            num_bands,
            smoothed_magnitudes: vec![0.0; num_bands],
            smoothing_factor: 0.70, // Balanced smoothing
            peak_holds: vec![0.0; num_bands],
            peak_decay: 0.87, // Balanced decay
            min_db: -80.0,
            max_db: -10.0,
        }
    }

    /// Analyze audio samples and update frequency magnitudes
    pub fn update(&mut self, sample_buffer: &Arc<Mutex<HeapRb<f32>>>) {
        // Try to read samples from the buffer
        let samples = if let Ok(buf) = sample_buffer.lock() {
            let available = buf.occupied_len();
            if available < 512 {
                // Not enough samples for analysis
                // Apply decay to existing values
                for i in 0..self.num_bands {
                    self.smoothed_magnitudes[i] *= 0.9;
                    self.peak_holds[i] *= self.peak_decay;
                }
                return;
            }
            
            // Read up to 2048 samples (good FFT size) - copy without consuming
            let sample_count = available.min(2048);
            
            // Skip older samples to get the most recent ones (reduces lag)
            let start = available.saturating_sub(sample_count);
            let samples: Vec<f32> = buf.iter().skip(start).take(sample_count).copied().collect();
            samples
        } else {
            return;
        };

        // Perform FFT analysis
        let magnitudes = self.compute_fft(&samples);
        
        // Update smoothed magnitudes and peaks
        for (i, &mag) in magnitudes.iter().enumerate() {
            // Smooth the magnitude for visual appeal
            self.smoothed_magnitudes[i] = 
                self.smoothing_factor * self.smoothed_magnitudes[i] + 
                (1.0 - self.smoothing_factor) * mag;
            
            // Update peak hold
            if mag > self.peak_holds[i] {
                self.peak_holds[i] = mag;
            } else {
                self.peak_holds[i] *= self.peak_decay;
            }
        }
    }

    /// Compute FFT and return magnitude spectrum grouped into frequency bands
    fn compute_fft(&mut self, samples: &[f32]) -> Vec<f32> {
        let fft_size = samples.len().next_power_of_two().min(2048);
        
        // Prepare input buffer with windowing (Hann window)
        let mut buffer: Vec<Complex<f32>> = samples.iter()
            .take(fft_size)
            .enumerate()
            .map(|(i, &sample)| {
                // Apply Hann window to reduce spectral leakage
                let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos());
                Complex::new(sample * window, 0.0)
            })
            .collect();
        
        // Pad with zeros if needed
        buffer.resize(fft_size, Complex::new(0.0, 0.0));
        
        // Perform FFT
        let fft = self.fft_planner.plan_fft_forward(fft_size);
        fft.process(&mut buffer);
        
        // Compute magnitude spectrum (only first half due to symmetry)
        let spectrum_size = fft_size / 2;
        // Normalize by FFT size to get proper dBFS values in ~[-80, 0] range
        let scale = 1.0 / fft_size as f32;
        let magnitudes: Vec<f32> = buffer.iter()
            .take(spectrum_size)
            .map(|c| {
                let mag = (c.re * c.re + c.im * c.im).sqrt() * scale;
                // Convert to dB scale (now properly normalized to dBFS)
                20.0 * mag.max(1e-10).log10()
            })
            .collect();
        
        // Group spectrum into frequency bands (logarithmic scale for better perception)
        self.group_into_bands(&magnitudes, spectrum_size)
    }

    /// Group FFT bins into logarithmic frequency bands
    fn group_into_bands(&mut self, magnitudes: &[f32], spectrum_size: usize) -> Vec<f32> {
        let mut bands_db = vec![0.0f32; self.num_bands];
        
        // Use logarithmic spacing for frequency bands (more natural perception)
        for (i, band) in bands_db.iter_mut().enumerate() {
            let freq_start = (i as f32 / self.num_bands as f32).powf(2.5);
            let freq_end = ((i + 1) as f32 / self.num_bands as f32).powf(2.5);
            
            let bin_start = (freq_start * spectrum_size as f32) as usize;
            let bin_end = (freq_end * spectrum_size as f32).min(spectrum_size as f32) as usize;
            
            if bin_start < bin_end && bin_end <= magnitudes.len() {
                // Average magnitude in this band
                let sum: f32 = magnitudes[bin_start..bin_end].iter().sum();
                let count = (bin_end - bin_start) as f32;
                *band = if count > 0.0 { sum / count } else { -80.0 };
            } else {
                *band = -80.0;
            }
        }
        
        // Auto-sensitivity: track rolling max and adjust dB window like CAVA
        let frame_max = bands_db.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        
        // Move max_db slowly toward the observed max (like CAVA's autosens)
        let target_max = frame_max.max(-30.0); // don't go too low
        self.max_db = 0.9 * self.max_db + 0.1 * target_max;
        self.min_db = self.max_db - 60.0; // fixed 60 dB window
        
        let db_range = self.max_db - self.min_db;
        
        // Convert from dB to linear scale for segment-based rendering
        bands_db.iter()
            .map(|&db| {
                // Map dB range to 0.0-1.0 for smooth segment filling
                let normalized = ((db - self.min_db) / db_range).clamp(0.0, 1.0);
                // Use exponent > 1 to emphasize differences at the bottom (not top)
                normalized.powf(1.2)
            })
            .collect()
    }

    /// Render the frequency spectrum as mirrored bars (CAVA-style)
    pub fn render(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("4: Spectrum Visualizer (FFT)");
        
        let inner = block.inner(area);
        
        // Render custom mirrored visualization
        self.render_mirrored(f, inner);
        
        // Render the border
        f.render_widget(block, area);
    }
    
    /// Render mirrored bars like CAVA (symmetric around center)
    fn render_mirrored(&self, f: &mut Frame<'_>, area: Rect) {
        if area.height < 2 || area.width < 2 {
            return;
        }
        
        let bar_width = 2;
        let bar_gap = 1;
        let bar_spacing = bar_width + bar_gap;
        
        // Calculate how many bars can fit in half the width
        let half_width = area.width / 2;
        let bars_per_side = (half_width as usize / bar_spacing).min(self.num_bands / 2);
        
        if bars_per_side == 0 {
            return;
        }
        
        let height = area.height as usize;
        
        // Define 10 segments with different characters for smooth gradation
        // Each segment represents 10% of the bar height
        const SEGMENTS: usize = 10;
        let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█', '█', '█'];
        
        // Build the entire visualization as a single multi-line string
        let mut full_content = String::new();
        
        // Build the visualization line by line from top to bottom
        for row in 0..height {
            let mut line = String::with_capacity(area.width as usize);
            
            // Left side (mirrored)
            for i in (0..bars_per_side).rev() {
                let magnitude = self.smoothed_magnitudes[i];
                
                // Calculate the filled height for this bar (0.0 to 1.0)
                let filled_height = magnitude;
                
                // Convert to actual pixel height (minimum 1 to always show at least ▁)
                let pixels_filled = (filled_height * height as f32) as usize;
                
                // Current row from bottom (0 = bottom, height-1 = top)
                let row_from_bottom = height - row - 1;
                
                // Determine what character to show at this row
                let char_to_show = if pixels_filled == 0 && row_from_bottom == 0 {
                    // Always show minimum bar at bottom row
                    '▁'
                } else if row_from_bottom < pixels_filled {
                    // This row is filled
                    let segment_idx = ((filled_height * SEGMENTS as f32) as usize).min(SEGMENTS - 1);
                    
                    // Check if we're at the top of the filled area for gradient effect
                    if row_from_bottom == pixels_filled - 1 {
                        // Top segment - use gradient character
                        let fractional = (filled_height * height as f32) - pixels_filled as f32;
                        if fractional > 0.5 {
                            chars[segment_idx]
                        } else {
                            chars[segment_idx.saturating_sub(1)]
                        }
                    } else {
                        // Fully filled segment
                        '█'
                    }
                } else {
                    ' '
                };
                
                // Add bar
                for _ in 0..bar_width {
                    line.push(char_to_show);
                }
                
                // Add gap
                for _ in 0..bar_gap {
                    line.push(' ');
                }
            }
            
            // Center gap
            line.push(' ');
            
            // Right side (same as left, mirrored)
            for i in 0..bars_per_side {
                // Add gap
                for _ in 0..bar_gap {
                    line.push(' ');
                }
                
                let magnitude = self.smoothed_magnitudes[i];
                
                // Calculate the filled height for this bar (0.0 to 1.0)
                let filled_height = magnitude;
                
                // Convert to actual pixel height (minimum 1 to always show at least ▁)
                let pixels_filled = (filled_height * height as f32) as usize;
                
                // Current row from bottom (0 = bottom, height-1 = top)
                let row_from_bottom = height - row - 1;
                
                // Determine what character to show at this row
                let char_to_show = if pixels_filled == 0 && row_from_bottom == 0 {
                    // Always show minimum bar at bottom row
                    '▁'
                } else if row_from_bottom < pixels_filled {
                    // This row is filled
                    let segment_idx = ((filled_height * SEGMENTS as f32) as usize).min(SEGMENTS - 1);
                    
                    // Check if we're at the top of the filled area for gradient effect
                    if row_from_bottom == pixels_filled - 1 {
                        // Top segment - use gradient character
                        let fractional = (filled_height * height as f32) - pixels_filled as f32;
                        if fractional > 0.5 {
                            chars[segment_idx]
                        } else {
                            chars[segment_idx.saturating_sub(1)]
                        }
                    } else {
                        // Fully filled segment
                        '█'
                    }
                } else {
                    ' '
                };
                
                // Add bar
                for _ in 0..bar_width {
                    line.push(char_to_show);
                }
            }
            
            // Ensure the line fits within the available width
            // Count characters, not bytes, and truncate safely
            let mut char_count = line.chars().count();
            
            // Pad with spaces if too short
            while char_count < area.width as usize {
                line.push(' ');
                char_count += 1;
            }
            
            // Truncate safely if too long (respect UTF-8 boundaries)
            if char_count > area.width as usize {
                line = line.chars().take(area.width as usize).collect();
            }
            
            // Add line to full content
            full_content.push_str(&line);
            if row < height - 1 {
                full_content.push('\n');
            }
        }
        
        // Render the entire visualization as a single widget to ensure proper clearing
        let paragraph = Paragraph::new(full_content).style(Style::default().fg(Color::White));
        f.render_widget(paragraph, area);
    }
}
