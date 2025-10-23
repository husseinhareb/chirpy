// src/music_player.rs

use std::{fs::File, io::BufReader, path::PathBuf, sync::{mpsc::{self, Sender}, Arc}, thread};
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;

// Lofty: probing + tag reading (including comments for lyrics and pictures for artwork)
use lofty::probe::Probe;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::ItemKey;

// Rodio: decode, play, pause & resume audio
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

enum PlayerCommand {
    Play(PathBuf),
    Pause,
    Resume,
    Stop,
    Exit,
}

/// One metadata entry: raw tag key & value.
pub type TagEntry = (String, String);

/// Collected metadata for the current track, including its real duration, lyrics, and artwork.
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
}

impl MusicPlayer {
    /// Create an idle player.
    pub fn new() -> Self {
        // Channel to send commands to audio thread
        let (tx, rx) = mpsc::channel::<PlayerCommand>();

        let is_playing_flag = Arc::new(AtomicBool::new(false));
        let is_paused_flag = Arc::new(AtomicBool::new(false));

        // Clone flags for audio thread
        let ap = is_playing_flag.clone();
        let az = is_paused_flag.clone();

        // Spawn audio thread which owns the OutputStream and handles play/pause/stop
        thread::spawn(move || {
            // Try to create the output stream once
            let stream_res = OutputStream::try_default();
            if stream_res.is_err() {
                // If we can't create audio output, just drain commands and continue
                while let Ok(cmd) = rx.recv() {
                    if let PlayerCommand::Exit = cmd { break; }
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

                        // Try to create a new sink and queue the file. This happens off the UI thread.
                        if let Ok(new_sink) = Sink::try_new(&handle) {
                            if let Ok(file) = File::open(&path) {
                                if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                    new_sink.append(source);
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
                    PlayerCommand::Exit => {
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        break;
                    }
                }
            }
            // Keep stream alive until thread exits
            drop(stream);
        });

        Self { cmd_tx: tx, is_playing_flag, is_paused_flag, metadata: None }
    }

    /// Stop any existing playback, load metadata, and start playing `path`.
    pub fn play(&mut self, path: &PathBuf) -> Result<()> {
        // Send Play command to audio thread and return immediately.
        let p = path.clone();
        self.cmd_tx.send(PlayerCommand::Play(p)).ok();
        Ok(())
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
