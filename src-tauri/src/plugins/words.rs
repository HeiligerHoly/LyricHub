//! Word times from plugins (PLAN-plugins.md 4.5, 4.6): the lyrics fingerprint both sides agree
//! on, the check of a plugin's answer against the lines, and taking the times over. Pure
//! functions, tested on their own.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::lyrics::types::{Line, Word};

/// A word may start this much before its line or end this much after it.
const LINE_SLACK_MS: u32 = 1000;
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
