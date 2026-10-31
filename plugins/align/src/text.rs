//! Lyrics as the aligner's targets (model README): NFKC, lower case, letters and digits,
//! apostrophes only inside words, Hangul syllables as jamo; `|` between words and at both ends.
//! LyricHub sends each line split into units (words, or single kana and kanji in Japanese);
//! every target remembers its unit, so the alignment maps back one to one.

use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

pub struct Vocab {
    ids: HashMap<char, u32>,
    /// The character of each id; `None` for `<blank>`, `<unk>` and other non-characters.
    chars: Vec<Option<char>>,
    pub bar: u32,
    pub size: usize,
}

impl Vocab {
    /// `vocab.json` of the model: `{ "char": id }`, `<blank>` is 0.
    pub fn parse(json: &str) -> Result<Self, String> {
        let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(json).map_err(|e| format!("vocab: {e}"))?;
        let mut ids = HashMap::new();
        let mut by_id = vec![None; map.len()];
        for (k, v) in &map {
            let mut chars = k.chars();
            if let (Some(c), None, Some(id)) = (chars.next(), chars.next(), v.as_u64()) {
                ids.insert(c, id as u32);
                if let Some(slot) = by_id.get_mut(id as usize) {
                    *slot = Some(c);
                }
            }
        }
        let bar = *ids.get(&'|').ok_or("vocab without |")?;
        Ok(Vocab { ids, chars: by_id, bar, size: map.len() })
    }

    /// The character of a token id, if it is one.
    pub fn char_of(&self, id: usize) -> Option<char> {
        self.chars.get(id).copied().flatten()
    }
}

/// Token ids for the CTC alignment and, per token, its unit (`None` for `|`).
#[derive(Debug, PartialEq)]
pub struct Targets {
    pub ids: Vec<u32>,
    pub unit: Vec<Option<usize>>,
}

/// Targets of a line. `|` goes where the text has a space between two units, so Japanese
/// characters follow each other without one.
pub fn targets(text: &str, units: &[String], vocab: &Vocab) -> Targets {
    let mut t = Targets { ids: vec![vocab.bar], unit: vec![None] };
    let mut cursor = 0;
    for (i, unit) in units.iter().enumerate() {
        let found = text[cursor..].find(unit.as_str()).map(|p| cursor + p);
        if let Some(at) = found {
            if i > 0 && text[cursor..at].chars().any(char::is_whitespace) {
                push_bar(&mut t, vocab.bar);
            }
            cursor = at + unit.len();
        } else if i > 0 {
            push_bar(&mut t, vocab.bar);
        }
        for c in normalize(unit).chars() {
            if let Some(&id) = vocab.ids.get(&c) {
                t.ids.push(id);
                t.unit.push(Some(i));
            }
        }
    }
    push_bar(&mut t, vocab.bar);
    t
}

fn push_bar(t: &mut Targets, bar: u32) {
    if t.ids.last() != Some(&bar) {
        t.ids.push(bar);
        t.unit.push(None);
    }
}

/// One unit as the model reads it.
pub fn normalize(unit: &str) -> String {
    let kept: String = unit
        .nfkc()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric() || *c == '\'')
        .collect();
    let mut out = String::new();
    for c in kept.trim_matches('\'').chars() {
        jamo(c, &mut out);
    }
    out
}

/// A Hangul syllable as its leading consonant, vowel and optional final consonant (the model
/// knows the conjoining jamo, not the syllables); every other character as it is.
fn jamo(c: char, out: &mut String) {
    const BASE: u32 = 0xAC00;
    let s = c as u32;
    if !(BASE..BASE + 11172).contains(&s) {
        out.push(c);
        return;
    }
    let i = s - BASE;
    let (l, v, t) = (i / 588, (i % 588) / 28, i % 28);
    out.extend(char::from_u32(0x1100 + l));
    out.extend(char::from_u32(0x1161 + v));
    if t > 0 {
        out.extend(char::from_u32(0x11A7 + t));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab() -> Vocab {
        let chars = "abcdefghijklmnopqrstuvwxyz'éあいう花\u{1100}\u{1161}\u{11A8}";
        let mut map = serde_json::Map::new();
        map.insert("<blank>".into(), 0.into());
        map.insert("|".into(), 1.into());
        for (i, c) in chars.chars().enumerate() {
            map.insert(c.to_string(), (i + 2).into());
        }
        Vocab::parse(&serde_json::Value::Object(map).to_string()).unwrap()
    }

    fn units(s: &[&str]) -> Vec<String> {
        s.iter().map(|u| u.to_string()).collect()
    }

    #[test]
    fn normalizes_like_the_prototype() {
        assert_eq!(normalize("Don't,"), "don't");
        assert_eq!(normalize("'Cause"), "cause");
        assert_eq!(normalize("ＨＥＹ!"), "hey");
        assert_eq!(normalize("Été"), "été");
        assert_eq!(normalize("…"), "");
    }

    #[test]
    fn hangul_becomes_jamo() {
        assert_eq!(normalize("각"), "\u{1100}\u{1161}\u{11A8}");
        assert_eq!(normalize("가"), "\u{1100}\u{1161}");
    }

    #[test]
    fn words_get_bars_between_them() {
        let v = vocab();
        let t = targets("Paper boats, ok", &units(&["Paper", "boats,", "ok"]), &v);
        let bars = t.unit.iter().filter(|u| u.is_none()).count();
        assert_eq!(bars, 4, "both ends and two gaps");
        assert_eq!(t.unit.first(), Some(&None));
        assert_eq!(t.unit.last(), Some(&None));
        assert_eq!(t.unit.iter().filter(|u| **u == Some(1)).count(), 5);
    }

    #[test]
    fn japanese_characters_follow_each_other() {
        let v = vocab();
        let t = targets("あい花", &units(&["あ", "い", "花"]), &v);
        assert_eq!(t.unit, vec![None, Some(0), Some(1), Some(2), None]);
    }

    #[test]
    fn unknown_characters_and_empty_units_are_left_out() {
        let v = vocab();
        let t = targets("x ж y", &units(&["x", "ж", "y"]), &v);
        assert_eq!(t.unit, vec![None, Some(0), None, Some(2), None]);
    }
}
