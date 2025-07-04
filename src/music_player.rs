// src/music_player.rs

use std::{fs::File, io::BufReader, path::PathBuf};
use anyhow::Result;

// Lofty: probing + tag reading (including comments for lyrics)
use lofty::probe::Probe;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{ItemKey};
// Rodio: decode, play, pause & resume audio
use rodio::{Decoder, OutputStream, Sink};

/// One metadata entry: raw tag key & value.
pub type TagEntry = (String, String);

/// Collected metadata for the current track, including its real duration and any embedded lyrics.
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    /// All tag‐frame key/value pairs from the primary tag.
    pub tags: Vec<TagEntry>,
    /// Audio properties (bitrate, sample rate, channels, etc.)
    pub properties: Vec<(String, String)>,
    /// Total track length in seconds.
    pub duration_secs: u64,
    /// Unsynchronized lyrics, if stored in a comment frame (description = "lyrics").
    pub lyrics: Option<String>,
}

/// Simple player that can `play()`, `pause()`, `resume()` or `stop()` a file
/// (stopping any prior playback) and exposes all its metadata.
pub struct MusicPlayer {
    _stream: Option<OutputStream>,
    sink:     Option<Sink>,
    pub metadata: Option<TrackMetadata>,
}

impl MusicPlayer {
    /// Create an idle player.
    pub fn new() -> Self {
        Self {
            _stream:  None,
            sink:     None,
            metadata: None,
        }
    }

    /// Stop any existing playback, start playing `path`, and load its metadata + lyrics.
    pub fn play(&mut self, path: &PathBuf) -> Result<()> {
        // 1) Stop previous sink
        if let Some(old) = self.sink.take() {
            old.stop();
        }

        // 2) Open audio output & sink
        let (stream, handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&handle)?;

        // 3) Decode & queue the file
        let file   = File::open(path)?;
        let source = Decoder::new(BufReader::new(file))?;
        sink.append(source);
        sink.play();

        // 4) Retain stream & sink so playback continues
        self._stream = Some(stream);
        self.sink     = Some(sink);

        // 5) Probe the file with Lofty
        let tagged_file = Probe::open(path)?.read()?;

        // 5a) Extract lyrics from the first comment frame with description "lyrics"
        let lyrics = tagged_file
            .primary_tag()
            .and_then(|tag| {
                tag.get_items(&ItemKey::Comment)
                    .find(|item| item.description().eq_ignore_ascii_case("lyrics"))
                    .cloned()
                    .and_then(|item| item.into_value().into_string())
            });

        // 5b) Collect all other tag key/value pairs
        let mut tags = Vec::new();
        if let Some(tag) = tagged_file.primary_tag() {
            for item in tag.items() {
                tags.push((
                    format!("{:?}", item.key()),
                    format!("{:?}", item.value()),
                ));
            }
        }

        // 5c) Collect core audio properties
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

        // 5d) Extract real duration in seconds
        let duration_secs = props.duration().as_secs();

        // 6) Store metadata
        self.metadata = Some(TrackMetadata {
            tags,
            properties,
            duration_secs,
            lyrics,
        });

        Ok(())
    }

    /// Pause playback if currently playing.
    pub fn pause(&mut self) {
        if let Some(s) = &self.sink {
            s.pause();
        }
    }

    /// Resume playback if currently paused.
    pub fn resume(&mut self) {
        if let Some(s) = &self.sink {
            s.play();
        }
    }

    /// Immediately halt playback (if any).
    pub fn stop(&mut self) {
        if let Some(s) = self.sink.take() {
            s.stop();
        }
    }

    /// Returns true if there’s an active sink (i.e. something is playing or paused).
    pub fn is_playing(&self) -> bool {
        self.sink.is_some()
    }

    /// Returns true if playback is currently paused.
    pub fn is_paused(&self) -> bool {
        self.sink.as_ref().map_or(false, |s| s.is_paused())
    }
}
