//! Word times from plugins (PLAN-plugins.md 4.5, 4.6): the lyrics fingerprint both sides agree
//! on, the check of a plugin's answer against the lines, and taking the times over. Pure
//! functions, tested on their own.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::lyrics::types::{Line, Word};

/// A word may start this much before its line or end this much after it.
const LINE_SLACK_MS: u32 = 1000;
/// Auto-Sync: a line appears this long before its first word, and moves at most this far from
/// the LRC file's stamp (PLAN-plugins-2.md 3.1).
const SYNC_LEAD_MS: i64 = 150;
const SYNC_LIMIT_MS: i64 = 1500;
/// Lyrics from a plugin: at most this many lines of at most this many characters.
const MAX_FOUND_LINES: usize = 600;
const MAX_LINE_CHARS: usize = 300;
/// The last line has no next one to end it; its words may run this long after its start.
const LAST_LINE_MS: u32 = 30_000;

/// Word times for one line from a plugin: `[start_ms, end_ms]` per unit of the line.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PluginLine {
    pub index: usize,
    pub words: Vec<[u32; 2]>,
}

/// Fingerprint of synced lyrics: line starts and texts. A changed LRC (another LRCLIB entry,
/// a reload) gets another fingerprint, and stored word times no longer apply.
pub fn lyrics_hash(lines: &[Line]) -> String {
    let mut hasher = Sha256::new();
    for l in lines {
        hasher.update(format!("{}\t{}\n", l.start_ms, l.text).as_bytes());
    }
    hasher.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// The units LyricHub splits a line into (lyrics/words.rs), so an answer maps one to one.
pub fn units(line: &Line) -> Vec<&str> {
    line.words.as_deref().unwrap_or_default().iter().map(|w| w.text.as_str()).collect()
}

/// `word_times` from a plugin: key, fingerprint and lines.
pub fn parse_word_times(params: &Value) -> Result<(String, String, Vec<PluginLine>), String> {
    let text = |k: &str| params.get(k).and_then(Value::as_str).map(str::to_owned).ok_or(format!("word_times without {k}"));
    let lines = serde_json::from_value(params.get("lines").cloned().unwrap_or(Value::Null))
        .map_err(|e| format!("word_times lines: {e}"))?;
    Ok((text("key")?, text("lyrics_hash")?, lines))
}

/// The lines of an answer that fit: an existing line with estimated words, one time pair per
/// unit, starts rising, each word inside its line give or take a second. There is no
/// confidence threshold: in P0 even uncertain alignments beat the estimate (docs/spike-align.md).
pub fn valid_lines(lines: &[Line], answer: &[PluginLine]) -> Vec<PluginLine> {
    answer
        .iter()
        .filter(|a| {
            let Some(line) = lines.get(a.index) else { return false };
            let Some(words) = &line.words else { return false };
            if words.is_empty() || words.iter().any(|w| !w.estimated) || words.len() != a.words.len() {
                return false;
            }
            let from = line.start_ms.saturating_sub(LINE_SLACK_MS);
            let to = line.end_ms.unwrap_or(line.start_ms.saturating_add(LAST_LINE_MS)).max(line.fill_end_ms).saturating_add(LINE_SLACK_MS);
            a.words.iter().all(|[s, e]| s <= e && *s >= from && *e <= to)
                && a.words.windows(2).all(|p| p[0][0] <= p[1][0])
        })
        .cloned()
        .collect()
}

/// Replaces the estimated words of the given lines. The words count as measured
/// (`estimated: false`), which also lights the word glow in the overlay. Returns how many lines
/// changed.
pub fn apply(lines: &mut [Line], answer: &[PluginLine]) -> usize {
    let mut changed = 0;
    for a in valid_lines(lines, answer) {
        let line = &mut lines[a.index];
        let Some(words) = &line.words else { continue };
        let new: Vec<Word> = words
            .iter()
            .zip(&a.words)
            .map(|(w, [s, e])| Word { start_ms: *s, end_ms: *e, text: w.text.clone(), estimated: false })
            .collect();
        let last_end = new.last().map(|w| w.end_ms).unwrap_or(line.fill_end_ms);
        // The fill ends with the last word, but never after the next line starts.
        line.fill_end_ms = last_end.min(line.end_ms.unwrap_or(u32::MAX)).max(line.start_ms);
        line.words = Some(new);
        changed += 1;
    }
    changed
}

/// Auto-Sync (PLAN-plugins-2.md 3): every line with measured words starts shortly before its
/// first word, at most 1.5 s from the LRC stamp and never before the line before it. Lines
/// without a measurement move by the median of the measured ones, words included. Returns that
/// median in ms (positive: the lines came too early), or `None` without measured lines.
pub fn auto_sync(lines: &mut [Line]) -> Option<i32> {
    let measured = |l: &Line| l.words.as_ref().and_then(|w| w.first()).filter(|w| !w.estimated).map(|w| w.start_ms);
    let mut shifts: Vec<i64> = lines.iter().filter_map(|l| measured(l).map(|w| w as i64 - SYNC_LEAD_MS - l.start_ms as i64)).collect();
    if shifts.is_empty() {
        return None;
    }
    shifts.sort_unstable();
    let median = shifts[shifts.len() / 2].clamp(-SYNC_LIMIT_MS, SYNC_LIMIT_MS);
    let mut floor = 0i64;
    for line in lines.iter_mut() {
        let old = line.start_ms as i64;
        let wanted = match measured(line) {
            Some(w) => (w as i64 - SYNC_LEAD_MS).clamp(old - SYNC_LIMIT_MS, old + SYNC_LIMIT_MS),
            None => old + median,
        };
        let start = wanted.max(floor).max(0);
        let shift = start - old;
        line.start_ms = start as u32;
        line.fill_end_ms = (line.fill_end_ms as i64 + shift).max(start) as u32;
        if measured(line).is_none() {
            // Estimated words go along with their line.
            for w in line.words.iter_mut().flatten() {
                w.start_ms = (w.start_ms as i64 + shift).max(0) as u32;
                w.end_ms = (w.end_ms as i64 + shift).max(0) as u32;
            }
        }
        floor = start + 1;
    }
    // A line ends where the next one starts.
    for i in 0..lines.len().saturating_sub(1) {
        let next = lines[i + 1].start_ms;
        if lines[i].end_ms.is_some() {
            lines[i].end_ms = Some(next);
        }
        lines[i].fill_end_ms = lines[i].fill_end_ms.min(next).max(lines[i].start_ms);
    }
    Some(median as i32)
}

/// One line of lyrics from a plugin (`lyrics_found`): start, text and, optionally, word times.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct FoundLine {
    pub start_ms: u32,
    pub text: String,
    #[serde(default)]
    pub words: Vec<[u32; 2]>,
}

/// `lyrics_found`: where the lyrics come from and the lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub key: String,
    /// "lrclib" (found with a broader search), "aligned_plain" or "recognized".
    pub how: String,
    pub lrclib_id: Option<i64>,
    pub lines: Vec<FoundLine>,
}

pub fn parse_lyrics_found(params: &Value) -> Result<Found, String> {
    let key = params.get("key").and_then(Value::as_str).ok_or("lyrics_found without key")?.to_owned();
    let how = params.get("source").and_then(Value::as_str).unwrap_or("");
    if !["lrclib", "aligned_plain", "recognized"].contains(&how) {
        return Err(format!("lyrics_found with unknown source {how:?}"));
    }
    let mut lines: Vec<FoundLine> =
        serde_json::from_value(params.get("lines").cloned().unwrap_or(Value::Null)).map_err(|e| format!("lyrics_found lines: {e}"))?;
    if lines.is_empty() || lines.len() > MAX_FOUND_LINES {
        return Err(format!("lyrics_found with {} lines", lines.len()));
    }
    if lines.windows(2).any(|p| p[1].start_ms < p[0].start_ms) {
        return Err("lyrics_found with falling line starts".into());
    }
    for l in &mut lines {
        l.text = l.text.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_owned();
        if l.text.chars().count() > MAX_LINE_CHARS {
            return Err("lyrics_found with a line that is too long".into());
        }
    }
    Ok(Found { key, how: how.to_owned(), lrclib_id: params.get("lrclib_id").and_then(Value::as_i64), lines })
}

/// The lines as an LRC file, which the cache stores like lyrics from LRCLIB.
pub fn to_lrc(lines: &[FoundLine]) -> String {
    let mut out = String::new();
    for l in lines {
        let cs = l.start_ms / 10;
        out.push_str(&format!("[{:02}:{:02}.{:02}]{}\n", cs / 6000, cs / 100 % 60, cs % 100, l.text));
    }
    out
}

/// The plugin's word times for the parsed lines, where the counts fit LyricHub's units.
pub fn found_words(parsed: &[Line], found: &[FoundLine]) -> Vec<PluginLine> {
    parsed
        .iter()
        .zip(found)
        .enumerate()
        .filter(|(_, (l, f))| !f.words.is_empty() && l.words.as_ref().is_some_and(|w| w.len() == f.words.len()))
        .map(|(index, (_, f))| PluginLine { index, words: f.words.clone() })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::lyrics::parser;

    // Invented lines.
    const LRC: &str = "[00:01.00]paper boats at night\n[00:05.00]drifting slowly home\n[00:09.00]";

    fn lines() -> Vec<Line> {
        parser::parse(LRC).lines
    }

    #[test]
    fn the_fingerprint_follows_starts_and_texts() {
        let a = lyrics_hash(&lines());
        assert_eq!(a.len(), 16);
        assert_eq!(a, lyrics_hash(&lines()));
        let other = parser::parse(&LRC.replace("night", "noon")).lines;
        assert_ne!(a, lyrics_hash(&other));
        let shifted = parser::parse(&LRC.replace("00:05.00", "00:05.10")).lines;
        assert_ne!(a, lyrics_hash(&shifted));
    }

    #[test]
    fn answers_are_parsed_and_checked_line_by_line() {
        let ls = lines();
        assert_eq!(units(&ls[0]), ["paper", "boats", "at", "night"]);
        let params = json!({ "key": "k", "lyrics_hash": "h", "lines": [
            { "index": 0, "words": [[1000, 1300], [1300, 1700], [1700, 1900], [1900, 2600]] },
            // Wrong number of words.
            { "index": 1, "words": [[5000, 5500]] },
            // No such line.
            { "index": 7, "words": [[1, 2]] },
        ]});
        let (key, hash, answer) = parse_word_times(&params).unwrap();
        assert_eq!((key.as_str(), hash.as_str(), answer.len()), ("k", "h", 3));
        assert_eq!(valid_lines(&ls, &answer).iter().map(|l| l.index).collect::<Vec<_>>(), [0]);
        // Falling starts and words far outside the line are rejected.
        let bad = [
            PluginLine { index: 0, words: vec![[1500, 1600], [1000, 1100], [1700, 1800], [1900, 2000]] },
            PluginLine { index: 1, words: vec![[5000, 5200], [5200, 5400], [9000, 20_000]] },
        ];
        assert!(valid_lines(&ls, &bad).is_empty());
        // The last sung line may run long: nothing ends it.
        let open = parser::parse("[00:01.00]paper boats\n[00:05.00]drifting home").lines;
        let last = [PluginLine { index: 1, words: vec![[5200, 9000], [12_000, 19_500]] }];
        assert_eq!(valid_lines(&open, &last).len(), 1);
        assert!(parse_word_times(&json!({ "key": "k" })).is_err());
    }

    #[test]
    fn auto_sync_moves_lines_to_their_first_word() {
        let mut ls = lines();
        // Line 0 is measured: its first word comes 400 ms after the stamp.
        apply(&mut ls, &[PluginLine { index: 0, words: vec![[1400, 1700], [1700, 1900], [1900, 2100], [2100, 2600]] }]);
        let median = auto_sync(&mut ls).unwrap();
        assert_eq!(median, 250, "400 ms late minus the 150 ms lead");
        assert_eq!(ls[0].start_ms, 1250);
        // Line 1 had no measurement: it moves by the median, words included.
        assert_eq!(ls[1].start_ms, 5250);
        assert!(ls[1].words.as_ref().unwrap()[0].start_ms >= 5250);
        assert_eq!(ls[0].end_ms, Some(5250));
    }

    #[test]
    fn auto_sync_stays_within_limits_and_order() {
        let mut ls = lines();
        // A first word 3 s after the stamp moves the line by at most 1.5 s.
        apply(&mut ls, &[PluginLine { index: 0, words: vec![[4000, 4100], [4100, 4200], [4200, 4300], [4300, 4400]] }]);
        auto_sync(&mut ls);
        assert_eq!(ls[0].start_ms, 2500);
        assert!(ls.windows(2).all(|p| p[0].start_ms < p[1].start_ms));
        let mut none = lines();
        assert_eq!(auto_sync(&mut none), None, "nothing measured, nothing moves");
        assert_eq!(none, lines());
    }

    #[test]
    fn found_lyrics_are_checked_and_become_lrc() {
        let params = json!({ "key": "k", "source": "recognized", "lines": [
            { "start_ms": 1000, "text": "paper boats at night", "words": [[1000, 1300], [1300, 1600], [1600, 1800], [1800, 2400]] },
            { "start_ms": 5000, "text": "drifting\u{7} slowly home" },
        ]});
        let found = parse_lyrics_found(&params).unwrap();
        assert_eq!(found.lines[1].text, "drifting slowly home", "control characters go");
        let lrc = to_lrc(&found.lines);
        assert_eq!(lrc, "[00:01.00]paper boats at night\n[00:05.00]drifting slowly home\n");
        let parsed = parser::parse(&lrc).lines;
        assert_eq!(found_words(&parsed, &found.lines), vec![PluginLine { index: 0, words: found.lines[0].words.clone() }]);
        assert!(parse_lyrics_found(&json!({ "key": "k", "source": "guess", "lines": [{ "start_ms": 1, "text": "x" }] })).is_err());
        let falling = json!({ "key": "k", "source": "recognized", "lines": [{ "start_ms": 9, "text": "a" }, { "start_ms": 5, "text": "b" }] });
        assert!(parse_lyrics_found(&falling).is_err());
    }

    #[test]
    fn applied_times_count_as_measured_and_end_the_fill() {
        let mut ls = lines();
        let answer = [PluginLine { index: 1, words: vec![[5100, 5600], [5600, 6400], [6400, 7300]] }];
        assert_eq!(apply(&mut ls, &answer), 1);
        let w = ls[1].words.as_ref().unwrap();
        assert!(w.iter().all(|w| !w.estimated));
        assert_eq!((w[0].start_ms, w[2].end_ms, w[1].text.as_str()), (5100, 7300, "slowly"));
        assert_eq!(ls[1].fill_end_ms, 7300);
        // Line 0 keeps its estimate; applying again finds no estimated words in line 1.
        assert!(ls[0].words.as_ref().unwrap().iter().all(|w| w.estimated));
        assert_eq!(apply(&mut ls, &answer), 0);
    }
}
