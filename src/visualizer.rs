// src/visualizer.rs

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Bar, BarChart, BarGroup, Block, Borders},
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
            smoothing_factor: 0.7, // Smooth for visual appeal
            peak_holds: vec![0.0; num_bands],
            peak_decay: 0.95, // Peaks decay slowly
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
        
        // Normalize to 0.0-1.0 range
        let max_mag = bands.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min_mag = bands.iter().cloned().fold(f32::INFINITY, f32::min);
        let range = (max_mag - min_mag).max(1.0);
        
        bands.iter()
            .map(|&mag| ((mag - min_mag) / range).clamp(0.0, 1.0))
            .collect()
    }

    /// Render the frequency spectrum as a bar chart
    pub fn render(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("4: Spectrum Visualizer (FFT)");

        // Calculate available space for bars
        let inner = block.inner(area);
        let bar_width = 3; // Fixed width for each bar
        let bar_gap = 1; // Small gap between bars for distinction
        let bar_spacing = bar_width + bar_gap;
        let visible_bands = (inner.width as usize / bar_spacing as usize).min(self.num_bands);
        
        if visible_bands == 0 {
            f.render_widget(block, area);
            return;
        }

        // Create bars for the visualizer with uniform white color
        let bars: Vec<Bar> = (0..visible_bands)
            .map(|i| {
                let magnitude = self.smoothed_magnitudes[i];
                let height = (magnitude * 100.0) as u64;
                
                Bar::default()
                    .value(height)
                    .style(Style::default().fg(Color::White))
            })
            .collect();

        // Create bar groups (one bar per group)
        let bar_group = BarGroup::default().bars(&bars);
        
        // Create the bar chart with no labels
        let chart = BarChart::default()
            .block(block)
            .bar_width(bar_width)
            .bar_gap(bar_gap)
            .data(bar_group);

        f.render_widget(chart, area);
    }
}
