// src/audio/player.rs
//! Music playback engine using rodio with sample capture for visualization.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Result;
use ringbuf::{traits::*, HeapRb};
use rodio::{Decoder, OutputStream, Sink, Source};

use super::metadata::{load_metadata, TrackMetadata};
use super::sample_capture::SampleCapture;

/// Commands sent to the audio playback thread.
enum PlayerCommand {
    Play(PathBuf),
    Pause,
    Resume,
    Stop,
}

/// Simple player that can `play()`, `pause()`, `resume()`, or `stop()` a file,
/// stopping any prior playback, and exposes its metadata.
pub struct MusicPlayer {
    /// Sender to the audio thread for commands
    cmd_tx: Sender<PlayerCommand>,
    /// Local flags mirrored from the audio thread for quick UI access
    is_playing_flag: Arc<AtomicBool>,
    is_paused_flag: Arc<AtomicBool>,
    /// Most-recent metadata (if any).
    pub metadata: Option<TrackMetadata>,
    /// Shared circular buffer containing recent audio samples for visualization
    pub sample_buffer: Arc<Mutex<HeapRb<f32>>>,
}

impl MusicPlayer {
    /// Create an idle player.
    pub fn new() -> Self {
        // Channel to send commands to audio thread
        let (tx, rx) = mpsc::channel::<PlayerCommand>();

        let is_playing_flag = Arc::new(AtomicBool::new(false));
        let is_paused_flag = Arc::new(AtomicBool::new(false));

        // Create a larger circular buffer for audio samples (16384 samples ~= 372ms at 44.1kHz)
        let sample_buffer = Arc::new(Mutex::new(HeapRb::<f32>::new(16384)));

        // Clone flags for audio thread
        let ap = is_playing_flag.clone();
        let az = is_paused_flag.clone();
        let sample_buf_clone = sample_buffer.clone();

        // Spawn audio thread which owns the OutputStream and handles play/pause/stop
        thread::spawn(move || {
            // Try to create the output stream once
            let stream_res = OutputStream::try_default();
            if stream_res.is_err() {
                // If we can't create audio output, just drain commands until the
                // sender is dropped, then return.
                while rx.recv().is_ok() {
                    // ignore commands
                }
                return;
            }

            let (stream, handle) = stream_res.unwrap();
            // Current sink (if any)
            let mut sink: Option<Sink> = None;

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    PlayerCommand::Play(path) => {
                        // Stop previous sink
                        if let Some(s) = sink.take() {
                            s.stop();
                        }

                        // Clear the sample buffer when starting a new track
                        if let Ok(mut buf) = sample_buf_clone.lock() {
                            buf.clear();
                        }

                        // Try to create a new sink and queue the file
                        if let Ok(new_sink) = Sink::try_new(&handle) {
                            if let Ok(file) = File::open(&path) {
                                if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                    // Convert to f32 and wrap with sample capture
                                    let converted = source.convert_samples::<f32>();
                                    let capturing =
                                        SampleCapture::new(converted, sample_buf_clone.clone());

                                    new_sink.append(capturing);
                                    new_sink.play();
                                    ap.store(true, Ordering::SeqCst);
                                    az.store(false, Ordering::SeqCst);
                                    sink = Some(new_sink);
                                }
                            }
                        }
                    }
                    PlayerCommand::Pause => {
                        if let Some(s) = &sink {
                            s.pause();
                            az.store(true, Ordering::SeqCst);
                        }
                    }
                    PlayerCommand::Resume => {
                        if let Some(s) = &sink {
                            s.play();
                            az.store(false, Ordering::SeqCst);
                        }
                    }
                    PlayerCommand::Stop => {
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        ap.store(false, Ordering::SeqCst);
                        az.store(false, Ordering::SeqCst);
                    }
                }
            }
            // If the command channel closed, make sure to stop the sink.
            if let Some(s) = sink.take() {
                s.stop();
            }
            // Keep stream alive until thread exits
            drop(stream);
        });

        Self {
            cmd_tx: tx,
            is_playing_flag,
            is_paused_flag,
            metadata: None,
            sample_buffer,
        }
    }

    /// Stop any existing playback, load metadata, and start playing `path`.
    pub fn play(&mut self, path: &PathBuf) -> Result<()> {
        // Send Play command to audio thread and return immediately.
        let p = path.clone();
        self.cmd_tx.send(PlayerCommand::Play(p)).ok();
        Ok(())
    }

    /// Load metadata for `path` without touching player state. This is safe to call
    /// from a background thread and returns a plain `TrackMetadata` struct.
    pub fn load_metadata(path: PathBuf) -> Result<TrackMetadata> {
        load_metadata(path)
    }

    /// Pause playback if currently playing.
    pub fn pause(&mut self) {
        let _ = self.cmd_tx.send(PlayerCommand::Pause);
    }

    /// Resume playback if currently paused.
    pub fn resume(&mut self) {
        let _ = self.cmd_tx.send(PlayerCommand::Resume);
    }

    /// Immediately halt playback (if any).
    pub fn stop(&mut self) {
        let _ = self.cmd_tx.send(PlayerCommand::Stop);
    }

    /// Returns true if there's an active sink (i.e. playing or paused).
    pub fn is_playing(&self) -> bool {
        self.is_playing_flag.load(Ordering::SeqCst)
    }

    /// Returns true if playback is currently paused.
    pub fn is_paused(&self) -> bool {
        self.is_paused_flag.load(Ordering::SeqCst)
    }
}
