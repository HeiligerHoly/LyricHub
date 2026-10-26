//! Source-independent playback data (PLAN.md 4.3). `TrackInfo` goes to the frontend as the
//! `track` event; `Observation` feeds the playback clock from Phase 2 on.

use std::time::Instant;

use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    Smtc,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Music,
    /// Spotify Free advertisement: the overlay hides (PLAN.md 5.5).
    Ad,
    /// Podcast episode: title only, no lyrics search.
    Podcast,
    Unknown,
}

const AD_MAX_MS: u32 = 40_000;
const PODCAST_MIN_MS: u32 = 20 * 60_000;

/// Heuristics without the Web API (PLAN.md 5.5): an ad is titled "Advertisement" or "Spotify"
/// with no artist and lasts at most 40 s; a podcast episode is longer than 20 min and has no
/// album. Premium (the user's account) plays no ads, so a simple rule suffices.
pub fn classify(
    title: &str,
    artist: &str,
    album: Option<&str>,
    duration_ms: Option<u32>,
    music_type: bool,
) -> TrackKind {
    let title_l = title.trim().to_lowercase();
    let artist_l = artist.trim().to_lowercase();
    let ad_title = matches!(title_l.as_str(), "advertisement" | "spotify" | "spotify free" | "werbung");
    if ad_title && (artist_l.is_empty() || artist_l == "spotify") && duration_ms.is_none_or(|d| d <= AD_MAX_MS) {
        return TrackKind::Ad;
    }
    if album.is_none_or(|a| a.trim().is_empty()) && duration_ms.is_some_and(|d| d > PODCAST_MIN_MS) {
        return TrackKind::Podcast;
    }
    if music_type { TrackKind::Music } else { TrackKind::Unknown }
}

/// Cover art the frontend can put into `<img src>`: a `data:` URL built from the SMTC
/// thumbnail. A later Web API source can hand out https URLs through the same field.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ArtRef {
    pub url: String,
    /// Hash of the image bytes, to tell a real cover change from a repeated event.
    #[serde(skip)]
    pub hash: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TrackInfo {
    /// `norm(artist)|norm(title)|round(duration_s)`; a Spotify ID once the Web API exists.
    pub key: String,
    pub title: String,
    /// "A, B" for several artists, as Spotify reports it.
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub art: Option<ArtRef>,
    pub kind: TrackKind,
    pub source: SourceId,
}

impl TrackInfo {
    /// Same song, possibly with different duration or art. Used to tell a track change
    /// from a metadata update.
    pub fn same_song(&self, other: &TrackInfo) -> bool {
        self.title == other.title && self.artist == other.artist && self.album == other.album
    }
}

pub fn track_key(artist: &str, title: &str, duration_ms: Option<u32>) -> String {
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let secs = duration_ms.map(|d| ((d as f64) / 1000.0).round().to_string()).unwrap_or_else(|| "?".into());
    format!("{}|{}|{}", norm(artist), norm(title), secs)
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStatus {
    /// No player session (Spotify closed).
    Closed,
    Opened,
    Changing,
    Stopped,
    Playing,
    Paused,
}

/// One position report of a source (PLAN.md 4.3, 5.4).
#[derive(Clone, Debug)]
pub struct Observation {
    pub source: SourceId,
    pub pos_ms: f64,
    /// Estimated moment at which `pos_ms` was true.
    pub valid_at: Instant,
    pub playing: bool,
    pub confidence: f32,
    /// Position fell to the start of a song: most likely a track change whose new title
    /// has not arrived yet (docs/spike-smtc.md, observation 4), not a seek.
    pub likely_track_change: bool,
}

#[derive(Clone, Debug)]
pub enum SourceEvent {
    /// New song, or new metadata (duration, cover) for the current one. `None` when the
    /// player session is gone.
    Track {
        info: Option<TrackInfo>,
        /// Earliest sign of this change seen by the source, for latency logging.
        first_sign: Instant,
    },
    Status(PlaybackStatus),
    Observation(Observation),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_kinds() {
        assert_eq!(classify("Advertisement", "", None, Some(30_000), true), TrackKind::Ad);
        assert_eq!(classify("Spotify", "Spotify", None, None, true), TrackKind::Ad);
        // A real song called "Spotify" with an artist is music.
        assert_eq!(classify("Spotify", "Some Band", Some("Album"), Some(200_000), true), TrackKind::Music);
        assert_eq!(classify("Episode 12: Long Talk", "Some Show", None, Some(55 * 60_000), true), TrackKind::Podcast);
        // A long album track is no podcast.
        assert_eq!(classify("Echoes", "Band", Some("Meddle"), Some(23 * 60_000), true), TrackKind::Music);
        assert_eq!(classify("Song", "Band", Some("Album"), Some(180_000), false), TrackKind::Unknown);
    }

    #[test]
    fn key_normalises_case_and_whitespace() {
        assert_eq!(track_key("  Daft  Punk ", "One More Time", Some(320_400)), "daft punk|one more time|320");
        assert_eq!(track_key("A", "B", None), "a|b|?");
    }
}
