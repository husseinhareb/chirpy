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
}

impl Visualizer {
    /// Create a new visualizer with specified number of frequency bands
    pub fn new() -> Self {
        let num_bands = 64; // Display 64 frequency bands
        Self {
            fft_planner: FftPlanner::new(),
            num_bands,
            smoothed_magnitudes: vec![0.0; num_bands],
            smoothing_factor: 0.85, // Higher smoothing for more stable visualization
            peak_holds: vec![0.0; num_bands],
            peak_decay: 0.92, // Peaks decay moderately
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
            let mut samples = Vec::with_capacity(sample_count);
            
            // Create a consuming iterator to get the samples
            let mut iter = buf.iter();
            
            // Take the most recent samples
            for _ in 0..sample_count {
                if let Some(&sample) = iter.next() {
                    samples.push(sample);
                }
            }
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
        let magnitudes: Vec<f32> = buffer.iter()
            .take(spectrum_size)
            .map(|c| {
                let mag = (c.re * c.re + c.im * c.im).sqrt();
                // Convert to dB scale for better visualization
                20.0 * mag.max(1e-10).log10()
            })
            .collect();
        
        // Group spectrum into frequency bands (logarithmic scale for better perception)
        self.group_into_bands(&magnitudes, spectrum_size)
    }

    /// Group FFT bins into logarithmic frequency bands
    fn group_into_bands(&self, magnitudes: &[f32], spectrum_size: usize) -> Vec<f32> {
        let mut bands = vec![0.0; self.num_bands];
        
        // Use logarithmic spacing for frequency bands (more natural perception)
        for (i, band) in bands.iter_mut().enumerate() {
            let freq_start = (i as f32 / self.num_bands as f32).powf(2.5);
            let freq_end = ((i + 1) as f32 / self.num_bands as f32).powf(2.5);
            
            let bin_start = (freq_start * spectrum_size as f32) as usize;
            let bin_end = (freq_end * spectrum_size as f32).min(spectrum_size as f32) as usize;
            
            if bin_start < bin_end && bin_end <= magnitudes.len() {
                // Average magnitude in this band
                let sum: f32 = magnitudes[bin_start..bin_end].iter().sum();
                let count = (bin_end - bin_start) as f32;
                *band = if count > 0.0 { sum / count } else { 0.0 };
            }
        }
        
        // Convert from dB to linear scale with reduced sensitivity
        // Expand the dB range to make it less sensitive
        const MIN_DB: f32 = -80.0;
        const MAX_DB: f32 = -10.0;  // Most music peaks around -10dB to -20dB, not 0dB
        const DB_RANGE: f32 = MAX_DB - MIN_DB;
        const SENSITIVITY: f32 = 0.5;  // Reduce overall sensitivity
        
        bands.iter()
            .map(|&db| {
                // Map dB range to 0.0-1.0, with reduced sensitivity
                let normalized = ((db - MIN_DB) / DB_RANGE).clamp(0.0, 1.0);
                // Apply sensitivity reduction and curve
                (normalized * SENSITIVITY).powf(0.7)
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
        
        // Build the visualization line by line from top to bottom
        for row in 0..height {
            let mut line = String::with_capacity(area.width as usize);
            // Calculate threshold: top rows have high threshold, bottom rows have low
            let threshold = (height - row) as f32 / height as f32;
            
            // Left side (mirrored)
            for i in (0..bars_per_side).rev() {
                let magnitude = self.smoothed_magnitudes[i];
                
                // Add bar
                for _ in 0..bar_width {
                    if magnitude >= threshold {
                        line.push('█');
                    } else {
                        line.push(' ');
                    }
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
                
                // Add bar
                for _ in 0..bar_width {
                    if magnitude >= threshold {
                        line.push('█');
                    } else {
                        line.push(' ');
                    }
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
            
            let y = area.y + row as u16;
            let paragraph = Paragraph::new(line).style(Style::default().fg(Color::White));
            let line_area = Rect::new(area.x, y, area.width, 1);
            f.render_widget(paragraph, line_area);
        }
    }
}
