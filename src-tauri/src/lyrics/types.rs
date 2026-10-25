//! Lyrics data (PLAN.md 4.3). Times are milliseconds of the song, after the LRC `[offset:]`
//! tag has been applied.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Word {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    /// False when the timing comes from Enhanced LRC, true when estimated (Phase 4).
    pub estimated: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Line {
    pub start_ms: u32,
    /// Start of the next line; `None` for the last one.
    pub end_ms: Option<u32>,
    /// When the karaoke fill is complete. Usually `end_ms`; before a long instrumental gap it
    /// is estimated from the text length, and the line then stays filled (PLAN.md 5.6).
    pub fill_end_ms: u32,
    /// Empty for an instrumental marker.
    pub text: String,
    pub words: Option<Vec<Word>>,
    /// Romaji of a line with kana or kanji (docs/romaji.md). Filled once per song while the
    /// setting `appearance.romaji` is on, otherwise `None`.
    pub romaji: Option<String>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LyricsState {
    /// Lookup running; the overlay shows the title and a short loading hint.
    Loading,
    /// Line-synced lyrics.
    Synced,
    /// Only plain lyrics exist; the overlay shows a hint and the title.
    Unsynced,
    Instrumental,
    Missing,
    /// Network or server trouble; retried in the background.
    Error,
}

/// Payload of the `lyrics` event (PLAN.md 4.4).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LyricsEvent {
    /// Track key the lyrics belong to.
    pub key: String,
    pub state: LyricsState,
    /// "lrclib" or "cache:lrclib"; empty while loading.
    pub source: String,
    pub lines: Vec<Line>,
    /// The `[offset:]` tag, already applied to the times; kept for diagnostics.
    pub lrc_offset_ms: i32,
}

impl LyricsEvent {
    pub fn state(key: &str, state: LyricsState) -> Self {
        Self { key: key.to_owned(), state, source: String::new(), lines: Vec::new(), lrc_offset_ms: 0 }
    }
}
