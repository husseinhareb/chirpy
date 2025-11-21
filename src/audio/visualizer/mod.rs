// src/audio/visualizer/mod.rs
//! Real-time audio spectrum visualizer using FFT analysis.

mod fft;
mod renderer;

use std::sync::{Arc, Mutex};

use ratatui::{layout::Rect, Frame};
use ringbuf::HeapRb;

use fft::FftProcessor;
use renderer::SpectrumRenderer;

/// Real-time audio spectrum visualizer using FFT analysis.
pub struct Visualizer {
    /// FFT processor for frequency analysis
    fft_processor: FftProcessor,
    /// Spectrum renderer for bar visualization
    renderer: SpectrumRenderer,
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
    /// Create a new visualizer with specified number of frequency bands.
    pub fn new() -> Self {
        let num_bands = 64; // Display 64 frequency bands
        Self {
            fft_processor: FftProcessor::new(num_bands),
            renderer: SpectrumRenderer::new(),
            num_bands,
            smoothed_magnitudes: vec![0.0; num_bands],
            smoothing_factor: 0.70, // Balanced smoothing
            peak_holds: vec![0.0; num_bands],
            peak_decay: 0.87, // Balanced decay
        }
    }

    /// Analyze audio samples and update frequency magnitudes.
    pub fn update(&mut self, sample_buffer: &Arc<Mutex<HeapRb<f32>>>) {
        use ringbuf::traits::*;

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
        let magnitudes = self.fft_processor.compute(&samples);

        // Update smoothed magnitudes and peaks
        for (i, &mag) in magnitudes.iter().enumerate() {
            // Smooth the magnitude for visual appeal
            self.smoothed_magnitudes[i] = self.smoothing_factor * self.smoothed_magnitudes[i]
                + (1.0 - self.smoothing_factor) * mag;

            // Update peak hold
            if mag > self.peak_holds[i] {
                self.peak_holds[i] = mag;
            } else {
                self.peak_holds[i] *= self.peak_decay;
            }
        }
    }

    /// Render the frequency spectrum as mirrored bars (CAVA-style).
    pub fn render(&self, f: &mut Frame<'_>, area: Rect) {
        self.renderer
            .render(f, area, &self.smoothed_magnitudes, self.num_bands);
    }
}

impl Default for Visualizer {
    fn default() -> Self {
        Self::new()
    }
}
