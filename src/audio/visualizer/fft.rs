// src/audio/visualizer/fft.rs
//! FFT computation and frequency band grouping.

use rustfft::{num_complex::Complex, FftPlanner};

/// FFT processor for audio spectrum analysis.
pub struct FftProcessor {
    /// FFT planner for frequency analysis
    fft_planner: FftPlanner<f32>,
    /// Number of frequency bands to output
    num_bands: usize,
    /// Auto-sensitivity: current minimum dB threshold (rolling)
    min_db: f32,
    /// Auto-sensitivity: current maximum dB threshold (rolling)
    max_db: f32,
}

impl FftProcessor {
    /// Create a new FFT processor with the specified number of output bands.
    pub fn new(num_bands: usize) -> Self {
        Self {
            fft_planner: FftPlanner::new(),
            num_bands,
            min_db: -80.0,
            max_db: -10.0,
        }
    }

    /// Compute FFT and return magnitude spectrum grouped into frequency bands.
    pub fn compute(&mut self, samples: &[f32]) -> Vec<f32> {
        let fft_size = samples.len().next_power_of_two().min(2048);

        // Prepare input buffer with windowing (Hann window)
        let mut buffer: Vec<Complex<f32>> = samples
            .iter()
            .take(fft_size)
            .enumerate()
            .map(|(i, &sample)| {
                // Apply Hann window to reduce spectral leakage
                let window = 0.5
                    * (1.0
                        - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos());
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
        let magnitudes: Vec<f32> = buffer
            .iter()
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

    /// Group FFT bins into logarithmic frequency bands.
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
        let frame_max = bands_db
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);

        // Move max_db slowly toward the observed max (like CAVA's autosens)
        let target_max = frame_max.max(-30.0); // don't go too low
        self.max_db = 0.9 * self.max_db + 0.1 * target_max;
        self.min_db = self.max_db - 60.0; // fixed 60 dB window

        let db_range = self.max_db - self.min_db;

        // Convert from dB to linear scale for segment-based rendering
        bands_db
            .iter()
            .map(|&db| {
                // Map dB range to 0.0-1.0 for smooth segment filling
                let normalized = ((db - self.min_db) / db_range).clamp(0.0, 1.0);
                // Use exponent > 1 to emphasize differences at the bottom (not top)
                normalized.powf(1.2)
            })
            .collect()
    }
}
