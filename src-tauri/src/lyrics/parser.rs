//! LRC parser (PLAN.md 7.2): standard LRC with one or more timestamps per line, metadata tags
//! including `[offset:]`, Enhanced LRC word marks `<mm:ss.xx>`, and the quirks of files in
//! the wild: BOM, CRLF or CR line ends, 0–3 fraction digits, `:` as fraction separator,
//! unsorted or duplicated lines, section labels and stray text without timestamps.

use super::types::{Line, Word};

/// Fill duration per character when the line is followed by a long instrumental gap.
const MS_PER_CHAR: u32 = 70;
const EST_MIN_MS: u32 = 1000;
const EST_MAX_MS: u32 = 8000;
/// A line whose interval exceeds the estimated singing time by this much ends in a pause.
const GAP_MS: u32 = 2500;

#[derive(Debug, Default, PartialEq)]
pub struct ParsedLrc {
    pub lines: Vec<Line>,
    /// Value of `[offset:]`, already applied: positive moves the lyrics earlier (LRC convention).
    pub offset_ms: i32,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// True if at least one timestamped line was found.
    pub synced: bool,
}

struct RawLine {
    start: i64,
    text: String,
    words: Option<Vec<(i64, String)>>,
}

pub fn parse(input: &str) -> ParsedLrc {
    let input = input.trim_start_matches('\u{feff}');
    let mut out = ParsedLrc::default();
    let mut raw: Vec<RawLine> = Vec::new();

    for line in input.split(['\n', '\r']) {
        let mut rest = line.trim_start();
        let mut stamps: Vec<i64> = Vec::new();
        while let Some(inner) = rest.strip_prefix('[') {
            let Some(close) = inner.find(']') else { break };
            let tag = &inner[..close];
            if let Some(ms) = parse_timestamp(tag) {
                stamps.push(ms);
                rest = inner[close + 1..].trim_start_matches(' ');
                continue;
            }
            if stamps.is_empty() {
                if let Some((key, value)) = parse_meta(tag) {
                    match key.as_str() {
                        "offset" => out.offset_ms = value.trim_start_matches('+').parse().unwrap_or(0),
                        "ti" => out.title = Some(value.to_owned()).filter(|v| !v.is_empty()),
                        "ar" => out.artist = Some(value.to_owned()).filter(|v| !v.is_empty()),
                        _ => {}
                    }
                }
            }
            // A section label such as "[Chorus]" after timestamps stays part of the text.
            break;
        }
        if stamps.is_empty() {
            continue;
        }
        let (text, words) = parse_words(rest.trim());
        let first = stamps[0];
        for ts in stamps {
            let shifted = words.as_ref().map(|w| w.iter().map(|(t, s)| (t + ts - first, s.clone())).collect());
            raw.push(RawLine { start: ts, text: text.clone(), words: shifted });
        }
    }

    out.synced = !raw.is_empty();
    let offset = out.offset_ms as i64;
    let clamp = |t: i64| (t - offset).clamp(0, u32::MAX as i64) as u32;
    raw.sort_by_key(|l| l.start); // stable: equal times keep file order
    raw.dedup_by(|b, a| a.start == b.start && a.text == b.text);

    let starts: Vec<u32> = raw.iter().map(|l| clamp(l.start)).collect();
    out.lines = raw
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let start_ms = starts[i];
            let end_ms = starts.get(i + 1).copied();
            let fill_end_ms = fill_end(start_ms, end_ms, &l.text);
            let words = l.words.map(|ws| {
                let times: Vec<u32> = ws.iter().map(|(t, _)| clamp(*t).max(start_ms)).collect();
                ws.into_iter()
                    .enumerate()
                    .map(|(j, (_, text))| Word {
                        start_ms: times[j],
                        end_ms: times.get(j + 1).copied().unwrap_or(fill_end_ms).max(times[j]),
                        text,
                        estimated: false,
                    })
                    .collect()
            });
            Line { start_ms, end_ms, fill_end_ms, text: l.text, words }
        })
        .collect();
    out
}

/// When the fill of a line should be complete (PLAN.md 5.6, point 2). The fill spans the whole
/// interval to the next line unless that interval exceeds the estimated singing time by more
/// than 2.5 s; then the estimate applies and the rest is an instrumental pause.
pub fn fill_end(start_ms: u32, end_ms: Option<u32>, text: &str) -> u32 {
    let chars = text.chars().filter(|c| c.is_alphanumeric()).count() as u32;
    if chars == 0 {
        return start_ms;
    }
    let estimate = (chars * MS_PER_CHAR).clamp(EST_MIN_MS, EST_MAX_MS);
    match end_ms {
        Some(end) if end.saturating_sub(start_ms) <= estimate + GAP_MS => end.max(start_ms),
        _ => start_ms + estimate,
    }
}

/// `mm:ss`, `mm:ss.f`, `mm:ss.ff`, `mm:ss.fff`, `mm:ss:ff` and `h:mm:ss.ff`, in milliseconds.
fn parse_timestamp(tag: &str) -> Option<i64> {
    let tag = tag.trim();
    let parts: Vec<&str> = tag.split(':').collect();
    let (hours, minutes, seconds, fraction) = match parts.as_slice() {
        [m, s] => {
            let (s, f) = s.split_once('.').unwrap_or((s, ""));
            ("0", *m, s, f)
        }
        [h, m, s] if s.contains('.') => {
            let (s, f) = s.split_once('.')?;
            (*h, *m, s, f)
        }
        [m, s, f] => ("0", *m, *s, *f),
        _ => return None,
    };
    let num = |s: &str| (!s.is_empty() && s.len() <= 4 && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<i64>().ok()).flatten();
    let (h, m, s) = (num(hours)?, num(minutes)?, num(seconds)?);
    if s > 59 || seconds.len() > 2 {
        return None;
    }
    let frac_ms = if fraction.is_empty() {
        0
    } else {
        if !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let digits: String = fraction.chars().chain("00".chars()).take(3).collect();
        digits.parse::<i64>().ok()?
    };
    Some(((h * 60 + m) * 60 + s) * 1000 + frac_ms)
}

/// `[key:value]` with an alphabetic key, e.g. `[ar:...]`, `[offset:+300]`.
fn parse_meta(tag: &str) -> Option<(String, &str)> {
    let (key, value) = tag.split_once(':')?;
    let key = key.trim();
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic() || c == '#'))
        .then(|| (key.to_ascii_lowercase(), value.trim()))
}

/// Splits Enhanced LRC word marks off the text. Text before the first mark belongs to the
/// line start and is dropped from the word list only if it is whitespace.
fn parse_words(text: &str) -> (String, Option<Vec<(i64, String)>>) {
    if !text.contains('<') {
        return (text.to_owned(), None);
    }
    let mut plain = String::new();
    let mut words: Vec<(i64, String)> = Vec::new();
    let mut rest = text;
    let mut current: Option<i64> = None;
    let mut segment = String::new();
    while !rest.is_empty() {
        if let Some(inner) = rest.strip_prefix('<') {
            if let Some(close) = inner.find('>') {
                if let Some(ms) = parse_timestamp(&inner[..close]) {
                    flush(&mut words, current, &mut segment);
                    current = Some(ms);
                    rest = &inner[close + 1..];
                    continue;
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        segment.push(ch);
        plain.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    flush(&mut words, current, &mut segment);
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.is_empty() { (plain, None) } else { (plain, Some(words)) }
}

fn flush(words: &mut Vec<(i64, String)>, at: Option<i64>, segment: &mut String) {
    let text = segment.trim().to_owned();
    segment.clear();
    if let (Some(at), false) = (at, text.is_empty()) {
        words.push((at, text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixtures mirror the format quirks of LRC files found on LRCLIB and elsewhere. The text is
    // invented: real lyrics are copyrighted and stay out of the repository.

    fn starts(p: &ParsedLrc) -> Vec<u32> {
        p.lines.iter().map(|l| l.start_ms).collect()
    }
    fn texts(p: &ParsedLrc) -> Vec<&str> {
        p.lines.iter().map(|l| l.text.as_str()).collect()
    }

    #[test]
    fn plain_lrclib_style() {
        let p = parse("[00:00.06] Morning light on empty streets\n[00:04.25] I count the steps back home\n[00:08.86] \n[00:10.92] Every window holds a story\n");
        assert!(p.synced);
        assert_eq!(starts(&p), [60, 4250, 8860, 10920]);
        assert_eq!(texts(&p), ["Morning light on empty streets", "I count the steps back home", "", "Every window holds a story"]);
        assert_eq!(p.lines[0].end_ms, Some(4250));
        assert_eq!(p.lines[3].end_ms, None);
    }

    #[test]
    fn crlf_and_bom() {
        let p = parse("\u{feff}[ti:Paper Boats]\r\n[ar:The Inventions]\r\n[00:01.00]First line\r\n[00:03.50]Second line\r\n");
        assert_eq!(p.title.as_deref(), Some("Paper Boats"));
        assert_eq!(p.artist.as_deref(), Some("The Inventions"));
        assert_eq!(starts(&p), [1000, 3500]);
        assert_eq!(texts(&p), ["First line", "Second line"]);
    }

    #[test]
    fn fraction_variants() {
        let p = parse("[00:01]a\n[00:02.5]b\n[00:03.25]c\n[00:04.125]d\n[00:05:50]e\n[1:02:03.40]f\n[00:06.1234]g");
        assert_eq!(starts(&p), [1000, 2500, 3250, 4125, 5500, 6123, 3_723_400]);
    }

    #[test]
    fn several_timestamps_on_one_line() {
        let p = parse("[00:10.00][01:20.00]Here comes the chorus again\n[00:15.00]Verse line\n");
        assert_eq!(starts(&p), [10_000, 15_000, 80_000]);
        assert_eq!(p.lines[0].text, p.lines[2].text);
    }

    #[test]
    fn offset_tag_moves_lyrics_earlier_or_later() {
        let p = parse("[offset:+300]\n[00:01.00]a\n[00:02.00]b");
        assert_eq!(p.offset_ms, 300);
        assert_eq!(starts(&p), [700, 1700]);
        let p = parse("[offset:-250]\n[00:00.10]a");
        assert_eq!(starts(&p), [350]);
        // never below zero
        let p = parse("[offset:500]\n[00:00.10]a");
        assert_eq!(starts(&p), [0]);
    }

    #[test]
    fn unsorted_and_duplicated_lines() {
        let p = parse("[00:20.00]third\n[00:05.00]first\n[00:10.00]second\n[00:10.00]second\n[00:10.00]also at ten");
        assert_eq!(texts(&p), ["first", "second", "also at ten", "third"]);
    }

    #[test]
    fn enhanced_lrc_word_marks() {
        let p = parse("[00:12.00]<00:12.00>Tiny <00:12.40>paper <00:12.90>boats\n[00:14.00]Next");
        let l = &p.lines[0];
        assert_eq!(l.text, "Tiny paper boats");
        let w = l.words.as_ref().unwrap();
        assert_eq!(w.iter().map(|w| (w.start_ms, w.end_ms)).collect::<Vec<_>>(), [(12_000, 12_400), (12_400, 12_900), (12_900, 14_000)]);
        assert!(w.iter().all(|w| !w.estimated));
        assert!(p.lines[1].words.is_none());
    }

    #[test]
    fn broken_lines_are_skipped_or_kept_as_text() {
        let input = "garbage without tag\n[xx:yy]not a time\n[00:61.00]bad seconds\n[Chorus]\n[00:01.00][Chorus] Section label stays\n[00:02.00\n[by:someone]\n[00:03.00]ok";
        let p = parse(input);
        assert_eq!(texts(&p), ["[Chorus] Section label stays", "ok"]);
    }

    #[test]
    fn japanese_text_and_fullwidth_space() {
        let p = parse("[00:01.00]　夜明けの街を歩いていく\n[00:04.00]ことばはまだ見つからない");
        assert_eq!(texts(&p), ["夜明けの街を歩いていく", "ことばはまだ見つからない"]);
    }

    #[test]
    fn plain_lyrics_are_not_synced() {
        let p = parse("Line one\nLine two\n\nLine three");
        assert!(!p.synced);
        assert!(p.lines.is_empty());
    }

    #[test]
    fn cr_only_line_ends() {
        let p = parse("[00:01.00]a\r[00:02.00]b\r");
        assert_eq!(starts(&p), [1000, 2000]);
    }

    #[test]
    fn fill_end_follows_interval_or_estimate() {
        // 18 letters → 1260 ms estimate. An interval up to 3760 ms is filled completely ...
        assert_eq!(fill_end(0, Some(3500), "twenty letters here x"), 3500);
        // ... a longer one ends in a pause after the estimated singing time.
        assert_eq!(fill_end(0, Some(12_000), "twenty letters here x"), 1260);
        // Short text is at least 1 s, long text at most 8 s.
        assert_eq!(fill_end(1000, None, "hi"), 2000);
        assert_eq!(fill_end(0, None, &"a".repeat(500)), 8000);
        // Instrumental marker: nothing to fill.
        assert_eq!(fill_end(5000, Some(9000), "  "), 5000);
        // Kana and kanji count like letters.
        assert_eq!(fill_end(0, None, "夜明けの街"), 1000);
    }
}
