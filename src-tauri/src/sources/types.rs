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
    /// Browser playback that does not look like a song (no length, too short or long, no
    /// artist): title only, no lyrics search, so video titles do not go to LRCLIB.
    Video,
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

/// Songs in a browser are 45 s to 15 min long; anything else is a video, a stream or a clip.
const BROWSER_SONG_MS: std::ops::RangeInclusive<u32> = 45_000..=15 * 60_000;

/// A browser's media session (docs/spike-browser.md): Firefox-based browsers call everything
/// "Music", so the media type says nothing. Without a length there is no usable timeline
/// (Zen on youtube.com reports 0), and without an artist the lookup would only guess.
pub fn classify_browser(artist: &str, duration_ms: Option<u32>) -> TrackKind {
    if artist.trim().is_empty() || !duration_ms.is_some_and(|d| BROWSER_SONG_MS.contains(&d)) {
        return TrackKind::Video;
    }
    TrackKind::Music
}

/// Title and artist of a browser session, cleaned for display and the lyrics lookup.
/// youtube.com reports "Artist - Song (Official Video)" by "ArtistVEVO" or "Artist - Topic"
/// (docs/spike-browser.md, observation 9); YouTube Music reports clean fields.
pub fn clean_browser_title(title: &str, artist: &str) -> (String, String) {
    let mut channel = artist.trim().to_owned();
    for suffix in [" - Topic", "VEVO", "Vevo"] {
        if let Some(rest) = channel.strip_suffix(suffix) {
            channel = rest.trim().to_owned();
        }
    }
    let mut song = strip_video_notes(title);
    // "Artist - Song" when the artist field is a channel: empty, or naming the same artist.
    for dash in [" - ", " – ", " — "] {
        if let Some((left, right)) = song.split_once(dash) {
            let (left, right) = (left.trim(), right.trim());
            let same = !channel.is_empty() && squash(&channel).contains(&squash(left));
            if !left.is_empty() && !right.is_empty() && (channel.is_empty() || same || looks_like_channel(artist)) {
                channel = left.to_owned();
                song = right.to_owned();
            }
            break;
        }
    }
    (song, channel)
}

/// Drops bracketed notes such as "(Official Music Video)" or "[Lyrics]", and "| …" tails.
fn strip_video_notes(title: &str) -> String {
    const NOTES: [&str; 12] =
        ["official", "video", "audio", "lyric", "visuali", "hd", "4k", "mv", "m/v", "clip", "remaster", "hq"];
    let mut out = String::new();
    let mut rest = title.split('|').next().unwrap_or(title).trim();
    while let Some(open) = rest.find(['(', '[']) {
        let close_char = if rest[open..].starts_with('(') { ')' } else { ']' };
        let Some(len) = rest[open..].find(close_char) else { break };
        let inner = rest[open + 1..open + len].to_lowercase();
        out.push_str(&rest[..open]);
        if !NOTES.iter().any(|n| inner.contains(n)) {
            out.push_str(&rest[open..=open + len]);
        }
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Letters and digits only, lower case: "Some Artist" and "SomeArtistVEVO" compare.
fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn looks_like_channel(artist: &str) -> bool {
    let a = artist.to_lowercase();
    ["vevo", " - topic", "official", "records", "music"].iter().any(|n| a.contains(n))
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
    fn browser_sessions_need_a_song_length_and_an_artist() {
        assert_eq!(classify_browser("Some Band", Some(200_000)), TrackKind::Music);
        // Zen on youtube.com: no length.
        assert_eq!(classify_browser("Some Band", None), TrackKind::Video);
        assert_eq!(classify_browser("Some Band", Some(30_000)), TrackKind::Video);
        assert_eq!(classify_browser("Some Band", Some(2 * 3_600_000)), TrackKind::Video);
        assert_eq!(classify_browser("  ", Some(200_000)), TrackKind::Video);
    }

    #[test]
    fn youtube_titles_are_split_and_cleaned() {
        // Invented names throughout.
        let c = |t: &str, a: &str| clean_browser_title(t, a);
        assert_eq!(c("Paper Boats (Official Music Video)", "Tiny Harbor"), ("Paper Boats".into(), "Tiny Harbor".into()));
        assert_eq!(c("Tiny Harbor - Paper Boats [Lyrics]", "TinyHarborVEVO"), ("Paper Boats".into(), "Tiny Harbor".into()));
        assert_eq!(c("Paper Boats", "Tiny Harbor - Topic"), ("Paper Boats".into(), "Tiny Harbor".into()));
        assert_eq!(c("Tiny Harbor – Paper Boats | Live Session", "Harbor Records"), ("Paper Boats".into(), "Tiny Harbor".into()));
        assert_eq!(c("Tiny Harbor - Paper Boats (HD)", ""), ("Paper Boats".into(), "Tiny Harbor".into()));
        // YouTube Music: a dash inside the song title stays when the artist is a real artist.
        assert_eq!(c("Paper Boats - Acoustic", "Tiny Harbor"), ("Paper Boats - Acoustic".into(), "Tiny Harbor".into()));
        // Notes that belong to the song stay.
        assert_eq!(c("Paper Boats (feat. Quiet Fox)", "Tiny Harbor"), ("Paper Boats (feat. Quiet Fox)".into(), "Tiny Harbor".into()));
    }

    #[test]
    fn key_normalises_case_and_whitespace() {
        assert_eq!(track_key("  Daft  Punk ", "One More Time", Some(320_400)), "daft punk|one more time|320");
        assert_eq!(track_key("A", "B", None), "a|b|?");
    }
}
