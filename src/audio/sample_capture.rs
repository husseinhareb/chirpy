// src/audio/sample_capture.rs
//! A wrapper source that captures audio samples into a circular buffer for visualization.

use std::sync::{Arc, Mutex};

use ringbuf::{traits::*, HeapRb};
use rodio::Source;

/// A wrapper source that captures samples into a circular buffer while passing them through.
pub struct SampleCapture<S> {
    source: S,
    buffer: Arc<Mutex<HeapRb<f32>>>,
}

impl<S> SampleCapture<S> {
    /// Create a new sample capture wrapper around an existing source.
    pub fn new(source: S, buffer: Arc<Mutex<HeapRb<f32>>>) -> Self {
        Self { source, buffer }
    }
}

impl<S> Iterator for SampleCapture<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(sample) = self.source.next() {
            // Push sample to circular buffer (overwrites oldest if full)
            if let Ok(mut buf) = self.buffer.lock() {
                // If buffer is full, pop the oldest sample to make room
                if buf.is_full() {
                    let _ = buf.try_pop();
                }
                let _ = buf.try_push(sample);
            }
            Some(sample)
        } else {
            None
        }
    }
}

impl<S> Source for SampleCapture<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.source.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.source.total_duration()
    }
}
