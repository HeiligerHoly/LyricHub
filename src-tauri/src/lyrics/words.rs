//! Word timing estimate for lines without Enhanced LRC marks (PLAN.md 5.6, point 3). LRCLIB
//! nearly always has line times only; the fill then moves word by word, each word taking a share
//! of the line proportional to its letters. Latin, Cyrillic and Hangul text splits at spaces
//! (Hangul counts one per syllable block). Japanese has no spaces, so every kana or kanji is a
//! unit of its own: kana count 1, kanji 1.5 (usually more than one mora). Punctuation counts 0
//! and sticks to the unit before it.

use super::types::Word;

/// The voice usually starts a little after the line's time stamp ...
pub const LEAD_IN_MS: u32 = 60;
/// ... and ends a little before the fill end.
pub const TAIL_MS: u32 = 120;
/// Lines shorter than this keep their whole interval for the words.
const MIN_SPAN_MS: u32 = 400;

/// Estimated words of `text` between `start_ms` and `fill_end_ms`. Empty for a line without
/// letters. The word texts appear in `text` in this order (the overlay finds them there).
pub fn estimate(text: &str, start_ms: u32, fill_end_ms: u32) -> Vec<Word> {
    let units = split(text);
    let total: f64 = units.iter().map(|(_, w)| w).sum();
    if total <= 0.0 || fill_end_ms <= start_ms {
        return Vec::new();
    }
    let (from, to) = if fill_end_ms - start_ms >= MIN_SPAN_MS + LEAD_IN_MS + TAIL_MS {
        (start_ms + LEAD_IN_MS, fill_end_ms - TAIL_MS)
    } else {
        (start_ms, fill_end_ms)
    };
    let span = f64::from(to - from);
    let mut done = 0.0;
    let mut words = Vec::with_capacity(units.len());
    for (unit, weight) in units {
        let begin = from + (span * done / total).round() as u32;
        done += weight;
        let end = from + (span * done / total).round() as u32;
        words.push(Word { start_ms: begin, end_ms: end.max(begin), text: unit, estimated: true });
    }
    words
}

/// Units of a line with their weights. Zero-weight pieces join the unit before them, or the
/// next one at the start of the line.
fn split(text: &str) -> Vec<(String, f64)> {
    fn push(units: &mut Vec<(String, f64)>, unit: String, w: f64, pending: &mut String) {
        if unit.is_empty() {
            return;
        }
        if w > 0.0 {
            units.push((std::mem::take(pending) + &unit, w));
        } else if let Some(last) = units.last_mut() {
            last.0.push_str(&unit);
        } else {
            pending.push_str(&unit);
        }
    }

    let mut units: Vec<(String, f64)> = Vec::new();
    let mut word = String::new();
    let mut weight = 0.0;
    // Leading punctuation (an opening bracket) waits for the first unit.
    let mut pending = String::new();
    for c in text.chars() {
        if c.is_whitespace() {
            push(&mut units, std::mem::take(&mut word), weight, &mut pending);
            weight = 0.0;
        } else if let Some(w) = cjk_weight(c) {
            push(&mut units, std::mem::take(&mut word), weight, &mut pending);
            weight = 0.0;
            push(&mut units, c.to_string(), w, &mut pending);
        } else {
            word.push(c);
            if c.is_alphanumeric() {
                weight += 1.0;
            }
        }
    }
    push(&mut units, word, weight, &mut pending);
    units
}

/// Weight of a kana or Han character; `None` for everything that is written with spaces.
/// Japanese punctuation and the prolonged sound mark count 0 and join the unit before.
fn cjk_weight(c: char) -> Option<f64> {
    match c {
        // ー lengthens the vowel before it; small kana join their mora.
        'ー' | 'ぁ' | 'ぃ' | 'ぅ' | 'ぇ' | 'ぉ' | 'ゃ' | 'ゅ' | 'ょ' | 'ゎ' | 'ァ' | 'ィ' | 'ゥ' | 'ェ' | 'ォ'
        | 'ャ' | 'ュ' | 'ョ' | 'ヮ' => Some(0.0),
        '\u{3000}'..='\u{303f}' | '\u{ff01}'..='\u{ff0f}' | '\u{ff1a}'..='\u{ff20}' | '\u{30fb}' => Some(0.0),
        '\u{3040}'..='\u{30ff}' | '\u{ff66}'..='\u{ff9f}' => Some(1.0),
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' => Some(1.5),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(words: &[Word]) -> Vec<&str> {
        words.iter().map(|w| w.text.as_str()).collect()
    }

    #[test]
    fn latin_words_share_the_line_by_letters() {
        // 0..2000 ms: lead-in 60, tail 120 → words between 60 and 1880 (1820 ms for 12 letters).
        let w = estimate("one, three seven!", 0, 2000);
        assert_eq!(texts(&w), ["one,", "three", "seven!"]);
        assert_eq!((w[0].start_ms, w[2].end_ms), (60, 1880));
        // "three" has 5 of 13 letters.
        assert_eq!(w[1].end_ms - w[1].start_ms, 700);
        assert!(w.iter().all(|w| w.estimated));
        assert!(w.windows(2).all(|p| p[0].end_ms == p[1].start_ms));
    }

    #[test]
    fn japanese_splits_per_character_with_kanji_heavier() {
        // 夜明け: 夜 1.5, 明 1.5, け 1; 、 joins け; ゃ joins き.
        let w = estimate("夜明け、きゃ", 1000, 1000 + 60 + 120 + 1100);
        assert_eq!(texts(&w), ["夜", "明", "け、", "きゃ"]);
        let d: Vec<u32> = w.iter().map(|w| w.end_ms - w.start_ms).collect();
        assert_eq!(d, [330, 330, 220, 220]);
    }

    #[test]
    fn mixed_scripts_and_leading_punctuation() {
        let w = estimate("(Oh) 東京 yeah", 0, 3000);
        assert_eq!(texts(&w), ["(Oh)", "東", "京", "yeah"]);
        let w = estimate("「さよなら」", 0, 3000);
        assert_eq!(texts(&w), ["「さ", "よ", "な", "ら」"]);
    }

    #[test]
    fn short_lines_use_the_whole_interval_and_empty_lines_have_no_words() {
        let w = estimate("go go", 500, 800);
        assert_eq!((w[0].start_ms, w[1].end_ms), (500, 800));
        assert!(estimate("... !", 0, 2000).is_empty());
        assert!(estimate("text", 1000, 1000).is_empty());
    }

    #[test]
    fn hangul_counts_per_syllable_block() {
        let w = estimate("사랑해 너", 0, 1180);
        assert_eq!(texts(&w), ["사랑해", "너"]);
        assert_eq!(w[0].end_ms - w[0].start_ms, 750);
    }
}
