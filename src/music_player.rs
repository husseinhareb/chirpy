// src/music_player.rs

use std::{fs::File, io::BufReader, path::PathBuf};
use anyhow::Result;

// Lofty: file probing + tag reading
use lofty::probe::Probe;
use lofty::file::TaggedFileExt;
use lofty::prelude::ItemKey;

// Rodio: audio decoding & playback
use rodio::{Decoder, OutputStream, Sink};

/// Simple holder for the title/artist/album of a track.
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub title:  String,
    pub artist: String,
    pub album:  String,
}

/// A tiny music‐player: can `play()` a file (stopping any prior playback)
/// and expose its metadata via `self.metadata`.
pub struct MusicPlayer {
    // Must keep the stream alive or audio will immediately stop.
    _stream: Option<OutputStream>,
    sink:      Option<Sink>,
    /// Most‐recently loaded metadata (if any)
    pub metadata: Option<TrackMetadata>,
}

impl MusicPlayer {
    /// Construct an idle player (no stream, no metadata).
    pub fn new() -> Self {
        Self {
            _stream: None,
            sink: None,
            metadata: None,
        }
    }

    /// Stop any existing playback, start playing `path`, and load its tags.
    pub fn play(&mut self, path: &PathBuf) -> Result<()> {
        // 1) Stop previous sink (if any)
        if let Some(old) = self.sink.take() {
            old.stop();
        }

        // 2) Open a new audio stream + sink
        let (stream, handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&handle)?;

        // 3) Decode & queue the file
        let file   = File::open(path)?;
        let source = Decoder::new(BufReader::new(file))?;
        sink.append(source);
        sink.play();

        // 4) Store them so playback actually continues
        self._stream = Some(stream);
        self.sink     = Some(sink);

        // 5) Probe & read tags (no write)
        let tagged = Probe::open(path)?.read()?;
        self.metadata = tagged.primary_tag().map(|tag| {
            TrackMetadata {
                title:  tag.get_string(&ItemKey::TrackTitle).unwrap_or_default().to_string(),
                artist: tag.get_string(&ItemKey::TrackArtist).unwrap_or_default().to_string(),
                album:  tag.get_string(&ItemKey::AlbumTitle).unwrap_or_default().to_string(),
            }
        });

        Ok(())
    }

    /// Immediately halt playback (if any).
    pub fn stop(&mut self) {
        if let Some(s) = self.sink.take() {
            s.stop();
        }
    }
}
