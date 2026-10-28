//! Word timing estimate for lines without Enhanced LRC marks (PLAN.md 5.6, point 3). LRCLIB
//! nearly always has line times only; the fill then moves word by word. Each word takes a share
//! of the line by its syllables plus a little per letter, and the last word gets more, because
//! singers hold it. Latin and Cyrillic words count their vowel groups as syllables (at least
//! one), Hangul one per syllable block. Japanese has no spaces, so every kana or kanji is a unit
//! of its own: kana count 1, kanji 1.5 (usually more than one mora). Punctuation counts 0 and
//! sticks to the unit before it. The weights come from a measurement against word-aligned songs
//! (docs/word-timing.md).

use super::types::Word;

/// Weight per letter on top of the syllables: long syllables take longer to sing.
const LETTER_WEIGHT: f64 = 0.2;
/// The last word of a line is held.
const LAST_WORD_FACTOR: f64 = 1.3;

/// Estimated words of `text` between `start_ms` and `fill_end_ms`. Empty for a line without
/// letters. The word texts appear in `text` in this order (the overlay finds them there).
pub fn estimate(text: &str, start_ms: u32, fill_end_ms: u32) -> Vec<Word> {
    let mut units: Vec<(String, f64)> =
        split(text).into_iter().map(|u| (u.text, u.syllables + LETTER_WEIGHT * u.letters)).collect();
    if let Some(last) = units.last_mut() {
        last.1 *= LAST_WORD_FACTOR;
    }
    let total: f64 = units.iter().map(|(_, w)| w).sum();
    if total <= 0.0 || fill_end_ms <= start_ms {
        return Vec::new();
    }
    let span = f64::from(fill_end_ms - start_ms);
    let mut done = 0.0;
    let mut words = Vec::with_capacity(units.len());
    for (unit, weight) in units {
        let begin = start_ms + (span * done / total).round() as u32;
        done += weight;
        let end = start_ms + (span * done / total).round() as u32;
        words.push(Word { start_ms: begin, end_ms: end.max(begin), text: unit, estimated: true });
    }
    words
}

/// Syllables of a line (kana 1, kanji 1.5), for the estimated singing time (parser.rs).
pub fn syllables(text: &str) -> f64 {
    split(text).iter().map(|u| u.syllables).sum()
}

struct Unit {
    text: String,
    syllables: f64,
    letters: f64,
}

/// A word being read: its text, letters, vowel groups and Hangul syllable blocks.
#[derive(Default)]
struct Reading {
    text: String,
    letters: f64,
    groups: u32,
    blocks: u32,
    in_vowel: bool,
}

impl Reading {
    fn add(&mut self, c: char) {
        self.text.push(c);
        if c.is_alphanumeric() {
            self.letters += 1.0;
        }
        let vowel = is_vowel(c);
        if vowel && !self.in_vowel {
            self.groups += 1;
        }
        self.in_vowel = vowel;
        if ('\u{ac00}'..='\u{d7a3}').contains(&c) {
            self.blocks += 1;
        }
    }

    fn finish(&mut self) -> Unit {
        let r = std::mem::take(self);
        let syllables = if r.letters > 0.0 { f64::from((r.groups + r.blocks).max(1)) } else { 0.0 };
        Unit { text: r.text, syllables, letters: r.letters }
    }
}

/// Units of a line. Pieces without letters join the unit before them, or the next one at the
/// start of the line.
fn split(text: &str) -> Vec<Unit> {
    fn push(units: &mut Vec<Unit>, mut unit: Unit, lead: &mut String) {
        if unit.text.is_empty() {
            return;
        }
        if unit.syllables > 0.0 {
            unit.text.insert_str(0, &std::mem::take(lead));
            units.push(unit);
        } else if let Some(last) = units.last_mut() {
            last.text.push_str(&unit.text);
        } else {
            lead.push_str(&unit.text);
        }
    }

    let mut units = Vec::new();
    let mut word = Reading::default();
    // Leading punctuation (an opening bracket) waits for the first unit.
    let mut lead = String::new();
    for c in text.chars() {
        if c.is_whitespace() {
            push(&mut units, word.finish(), &mut lead);
        } else if let Some(w) = cjk_weight(c) {
            push(&mut units, word.finish(), &mut lead);
            push(&mut units, Unit { text: c.to_string(), syllables: w, letters: 0.0 }, &mut lead);
        } else {
            word.add(c);
        }
    }
    push(&mut units, word.finish(), &mut lead);
    units
}

/// Vowels of the Latin and Cyrillic scripts, with the usual accents. `y` counts: it carries the
/// syllable in "happy" and is a word of its own in Spanish.
fn is_vowel(c: char) -> bool {
    c.to_lowercase().any(|c| "aeiouyäöüàáâãåæèéêëìíîïòóôõøœùúûýÿıаеёиоуыэюяіїє".contains(c))
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

    fn durations(words: &[Word]) -> Vec<u32> {
        words.iter().map(|w| w.end_ms - w.start_ms).collect()
    }

    #[test]
    fn latin_words_share_the_line_by_syllables_letters_and_a_held_last_word() {
        // go 1 + 0.4, to 1 + 0.4, sleep 1 + 1.0 held ×1.3 = 2.6: 5.4 shares over 5400 ms.
        let w = estimate("go, to sleep!", 0, 5400);
        assert_eq!(texts(&w), ["go,", "to", "sleep!"]);
        assert_eq!(durations(&w), [1400, 1400, 2600]);
        assert_eq!((w[0].start_ms, w[2].end_ms), (0, 5400));
        assert!(w.iter().all(|w| w.estimated));
        assert!(w.windows(2).all(|p| p[0].end_ms == p[1].start_ms));
    }

    #[test]
    fn syllables_count_vowel_groups_blocks_and_kana() {
        // Beau-ti-ful dream-er
        assert_eq!(syllables("Beautiful dreamer"), 5.0);
        // A word without vowels still takes one syllable.
        assert_eq!(syllables("rhythm hmm 1999"), 3.0);
        assert_eq!(syllables("привет"), 2.0);
        assert_eq!(syllables("사랑해"), 3.0);
        assert_eq!(syllables("夜明け"), 4.0);
        assert_eq!(syllables("... !"), 0.0);
    }

    #[test]
    fn japanese_splits_per_character_with_kanji_heavier() {
        // 夜 1.5, 明 1.5, け 1 (、 joins it), きゃ 1 held ×1.3 (ゃ joins き): 5.3 shares.
        let w = estimate("夜明け、きゃ", 1000, 1000 + 1060);
        assert_eq!(texts(&w), ["夜", "明", "け、", "きゃ"]);
        assert_eq!(durations(&w), [300, 300, 200, 260]);
    }

    #[test]
    fn mixed_scripts_and_leading_punctuation() {
        let w = estimate("(Oh) 東京 yeah", 0, 3000);
        assert_eq!(texts(&w), ["(Oh)", "東", "京", "yeah"]);
        let w = estimate("「さよなら」", 0, 3000);
        assert_eq!(texts(&w), ["「さ", "よ", "な", "ら」"]);
    }

    #[test]
    fn words_fill_the_whole_interval_and_empty_lines_have_no_words() {
        let w = estimate("go go", 500, 800);
        assert_eq!((w[0].start_ms, w[1].end_ms), (500, 800));
        assert!(estimate("... !", 0, 2000).is_empty());
        assert!(estimate("text", 1000, 1000).is_empty());
    }

    /// The measurement behind the weights (docs/word-timing.md). Every annotated line of the
    /// JamendoLyrics MultiLang dataset becomes an LRC line stamped at its first word, filled as
    /// the parser does, and each estimated word start is compared with the annotated one. Needs
    /// the dataset folder (with `annotations/words` and `lyrics`) in LYRICHUB_WORD_TIMES and
    /// prints numbers only: `cargo test --lib measure_word_timing -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_word_timing() {
        let root = std::path::PathBuf::from(std::env::var("LYRICHUB_WORD_TIMES").expect("LYRICHUB_WORD_TIMES"));
        let mut errors: Vec<i64> = Vec::new();
        let mut skipped = 0;
        let mut files: Vec<_> = std::fs::read_dir(root.join("annotations/words")).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        for path in &files {
            let name = path.file_stem().unwrap().to_string_lossy();
            let words = std::fs::read_to_string(root.join(format!("lyrics/{name}.words.txt"))).unwrap();
            let words = words.lines().filter(|w| !w.trim().is_empty()).map(str::trim);
            let csv = std::fs::read_to_string(path).unwrap();
            // (text, start, line ends here)
            let mut lines: Vec<Vec<(String, i64)>> = vec![Vec::new()];
            for (word, row) in words.zip(csv.lines().skip(1)) {
                let cols: Vec<&str> = row.split(',').collect();
                let start = (cols[0].parse::<f64>().unwrap() * 1000.0).round() as i64;
                lines.last_mut().unwrap().push((word.to_owned(), start));
                if cols[2].trim() != "nan" {
                    lines.push(Vec::new());
                }
            }
            lines.retain(|l| !l.is_empty());
            for (i, line) in lines.iter().enumerate() {
                let start = line[0].1 as u32;
                let next = lines.get(i + 1).map(|l| l[0].1 as u32);
                let text = line.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(" ");
                let est = estimate(&text, start, super::super::parser::fill_end(start, next, &text));
                if est.len() != line.len() {
                    skipped += 1;
                    continue;
                }
                errors.extend(est.iter().zip(line).map(|(e, (_, truth))| (i64::from(e.start_ms) - truth).abs()));
            }
        }
        errors.sort_unstable();
        let at = |p: f64| errors[((errors.len() - 1) as f64 * p) as usize];
        let mean = errors.iter().sum::<i64>() / errors.len() as i64;
        let within = errors.iter().filter(|&&e| e <= 150).count() as f64 / errors.len() as f64 * 100.0;
        println!(
            "songs {} words {} skipped lines {skipped}: median {} ms, mean {mean} ms, p90 {} ms, within 150 ms {within:.1} %",
            files.len(),
            errors.len(),
            at(0.5),
            at(0.9)
        );
    }

    #[test]
    fn hangul_counts_per_syllable_block() {
        // 사랑해 3 + 0.6, 너 (1 + 0.2) ×1.3 = 1.56: 5.16 shares.
        let w = estimate("사랑해 너", 0, 5160);
        assert_eq!(texts(&w), ["사랑해", "너"]);
        assert_eq!(durations(&w), [3600, 1560]);
    }
}
