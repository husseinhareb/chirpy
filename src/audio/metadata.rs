// src/audio/metadata.rs
//! Track metadata extraction using Lofty.

use std::path::PathBuf;

use anyhow::Result;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::ItemKey;

/// One metadata entry: raw tag key & value.
pub type TagEntry = (String, String);

/// Collected metadata for the current track, including its real duration, lyrics, and artwork.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    /// All tag-frame key/value pairs from the primary tag.
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

/// Load metadata for a file path without touching player state.
/// This is safe to call from a background thread.
pub fn load_metadata(path: PathBuf) -> Result<TrackMetadata> {
    // Probe the file with Lofty
    let tagged_file = Probe::open(&path)?.read()?;

    // Extract lyrics from the first comment frame with description "lyrics"
    let lyrics = tagged_file.primary_tag().and_then(|tag| {
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
