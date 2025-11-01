// src/music_player.rs

use std::{fs::File, io::BufReader, path::PathBuf, sync::{mpsc::{self, Sender}, Arc, Mutex}, thread};
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;

// Lofty: probing + tag reading (including comments for lyrics and pictures for artwork)
use lofty::probe::Probe;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::ItemKey;

// Rodio: decode, play, pause & resume audio
use rodio::{Decoder, OutputStream, Sink, Source};

// Ringbuf for circular buffer to store audio samples
use ringbuf::{traits::*, HeapRb};

enum PlayerCommand {
    Play(PathBuf),
    Pause,
    Resume,
    Stop,
}

/// A wrapper source that captures samples into a circular buffer while passing them through
struct SampleCapture<S> {
    source: S,
    buffer: Arc<Mutex<HeapRb<f32>>>,
}

impl<S> SampleCapture<S> {
    fn new(source: S, buffer: Arc<Mutex<HeapRb<f32>>>) -> Self {
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

/// One metadata entry: raw tag key & value.
pub type TagEntry = (String, String);

/// Collected metadata for the current track, including its real duration, lyrics, and artwork.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    /// All tag‑frame key/value pairs from the primary tag.
    pub tags: Vec<TagEntry>,
    /// Audio properties (bitrate, sample rate, channels, etc.)
    pub properties: Vec<(String, String)>,
    /// Total track length in seconds.
    pub duration_secs: u64,
    /// Unsynchronized lyrics (from a comment frame "lyrics").
    pub lyrics: Option<String>,
    /// Raw image bytes (PNG/JPEG) for artwork, if available.
    pub artwork: Option<Vec<u8>>,
}

/// Simple player that can `play()`, `pause()`, `resume()`, or `stop()` a file,
/// stopping any prior playback, and exposes its metadata.
pub struct MusicPlayer {
    // Keep the stream alive or audio will stop immediately.
    // Sender to the audio thread for commands
    cmd_tx: Sender<PlayerCommand>,
    // Local flags mirrored from the audio thread for quick UI access
    is_playing_flag: Arc<AtomicBool>,
    is_paused_flag: Arc<AtomicBool>,
    /// Most‑recent metadata (if any).
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
        
        // Create a circular buffer for audio samples (4096 samples ~= 93ms at 44.1kHz)
        let sample_buffer = Arc::new(Mutex::new(HeapRb::<f32>::new(4096)));

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
                while let Ok(_cmd) = rx.recv() {
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

                        // Try to create a new sink and queue the file. This happens off the UI thread.
                        if let Ok(new_sink) = Sink::try_new(&handle) {
                            if let Ok(file) = File::open(&path) {
                                if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                    // Convert to f32 and wrap with sample capture
                                    let converted = source.convert_samples::<f32>();
                                    let capturing = SampleCapture::new(converted, sample_buf_clone.clone());
                                    
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
                    // (Exit variant removed) when the command channel is closed the
                    // `while let Ok(cmd) = rx.recv()` loop will exit; we'll stop the
                    // sink after the loop.
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
        // Probe the file with Lofty
        let tagged_file = Probe::open(&path)?.read()?;

        // Extract lyrics from the first comment frame with description "lyrics"
        let lyrics = tagged_file
            .primary_tag()
            .and_then(|tag| {
                tag.get_items(&ItemKey::Comment)
                    .find(|item| item.description().eq_ignore_ascii_case("lyrics"))
                    .cloned()
                    .and_then(|item| item.into_value().into_string())
            });

        // Extract artwork from the first embedded picture
        let artwork = tagged_file
            .primary_tag()
            .and_then(|tag| tag.pictures().first().map(|pic| pic.data().to_vec()));

        // Collect all other tag key/value pairs
        let mut tags = Vec::new();
        if let Some(tag) = tagged_file.primary_tag() {
            for item in tag.items() {
                tags.push((format!("{:?}", item.key()), format!("{:?}", item.value())));
            }
        }

        // Collect core audio properties
        let props = tagged_file.properties();
        let mut properties = Vec::new();
        if let Some(b) = props.audio_bitrate() {
            properties.push(("Bitrate (kbps)".into(), b.to_string()));
        }
        if let Some(sr) = props.sample_rate() {
            properties.push(("Sample Rate (Hz)".into(), sr.to_string()));
        }
        if let Some(ch) = props.channels() {
            properties.push(("Channels".into(), ch.to_string()));
        }
        let duration_secs = props.duration().as_secs();

        Ok(TrackMetadata {
            tags,
            properties,
            duration_secs,
            lyrics,
            artwork,
        })
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

    /// Returns true if there’s an active sink (i.e. playing or paused).
    pub fn is_playing(&self) -> bool {
        self.is_playing_flag.load(Ordering::SeqCst)
    }

    /// Returns true if playback is currently paused.
    pub fn is_paused(&self) -> bool {
        self.is_paused_flag.load(Ordering::SeqCst)
    }
}
