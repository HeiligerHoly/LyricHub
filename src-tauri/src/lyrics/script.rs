//! Latin script for lyrics in Korean (Revised Romanization), Chinese (pinyin with tone marks),
//! Cyrillic and Greek (IDEEN.md, package D), next to the romaji for Japanese (romaji.rs). Plain
//! rules and tables; no line is guessed from a dictionary except Chinese, whose readings come from
//! the `pinyin` crate (the most common reading of each character).

use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Japanese,
    Chinese,
    Korean,
    Cyrillic,
    Greek,
}

fn is_kana(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{ff66}'..='\u{ff9f}')
}

fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
}

fn is_hangul(c: char) -> bool {
    matches!(c, '\u{ac00}'..='\u{d7a3}')
}

pub fn has_kana(text: &str) -> bool {
    text.chars().any(is_kana)
}

/// The script a line needs a Latin version for, if any. A line of only Han characters counts as
/// Japanese in a song with kana somewhere, else as Chinese.
pub fn of(text: &str, japanese_song: bool) -> Option<Script> {
    let has = |f: fn(char) -> bool| text.chars().any(f);
    if has(is_kana) {
        Some(Script::Japanese)
    } else if has(is_hangul) {
        Some(Script::Korean)
    } else if has(is_han) {
        Some(if japanese_song { Script::Japanese } else { Script::Chinese })
    } else if has(|c| matches!(c, '\u{0400}'..='\u{04ff}')) {
        Some(Script::Cyrillic)
    } else if has(|c| matches!(c, '\u{0370}'..='\u{03ff}' | '\u{1f00}'..='\u{1fff}')) {
        Some(Script::Greek)
    } else {
        None
    }
}

/// The Latin version of a line in a script other than Japanese.
pub fn latin(text: &str, script: Script) -> Option<String> {
    let out = match script {
        Script::Japanese => return None,
        Script::Korean => korean(text),
        Script::Chinese => chinese(text),
        Script::Cyrillic => by_table(text, cyrillic),
        Script::Greek => greek(text),
    };
    Some(out.split_whitespace().collect::<Vec<_>>().join(" "))
}

// ---- Korean, Revised Romanization ----

const INITIAL: [&str; 19] = ["g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p", "h"];
const MEDIAL: [&str; 21] = [
    "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we", "wi", "yu", "eu", "ui", "i",
];
/// A final consonant at the end of a syllable, as it sounds there.
const FINAL: [&str; 28] =
    ["", "k", "k", "k", "n", "n", "n", "t", "l", "k", "m", "l", "l", "l", "p", "l", "m", "p", "p", "t", "t", "ng", "t", "t", "k", "t", "p", "t"];
const SILENT_INITIAL: usize = 11;
const RIEUL_INITIAL: usize = 5;
const RIEUL_FINAL: usize = 8;

/// A single final consonant moved onto a following silent initial (한국어 → hangugeo).
fn linked(final_: usize) -> Option<&'static str> {
    Some(match final_ {
        1 => "g",
        2 => "kk",
        4 => "n",
        7 => "d",
        8 => "r",
        16 => "m",
        17 => "b",
        19 => "s",
        20 => "ss",
        22 => "j",
        23 => "ch",
        24 => "k",
        25 => "t",
        26 => "p",
        27 => "",
        _ => return None,
    })
}

fn korean(text: &str) -> String {
    let parts = |c: char| -> Option<(usize, usize, usize)> {
        let i = (c as u32).checked_sub(0xac00).filter(|&i| i < 11_172)? as usize;
        Some((i / 588, i % 588 / 28, i % 28))
    };
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut carried: Option<&str> = None;
    for (n, &c) in chars.iter().enumerate() {
        let Some((initial, medial, final_)) = parts(c) else {
            out.push(c);
            carried = None;
            continue;
        };
        out.push_str(carried.take().unwrap_or(INITIAL[initial]));
        out.push_str(MEDIAL[medial]);
        let next = chars.get(n + 1).and_then(|&c| parts(c));
        match next {
            Some((SILENT_INITIAL, _, _)) if linked(final_).is_some() => carried = linked(final_),
            // ㄹ before ㄹ is "ll" (몰라 → molla).
            Some((RIEUL_INITIAL, _, _)) if final_ == RIEUL_FINAL => {
                out.push('l');
                carried = Some("l");
            }
            _ => out.push_str(FINAL[final_]),
        }
    }
    out
}

// ---- Chinese, pinyin ----

fn chinese(text: &str) -> String {
    use pinyin::ToPinyin;
    let mut out = String::new();
    for c in text.chars() {
        match c.to_pinyin() {
            Some(p) => {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(p.with_tone());
                out.push(' ');
            }
            None => out.push(c),
        }
    }
    out
}

// ---- Cyrillic and Greek, by table ----

/// Maps each character through `table`, keeping capitals as capitals; unknown ones stay.
fn by_table(text: &str, table: fn(char) -> Option<&'static str>) -> String {
    let mut out = String::new();
    for c in text.chars() {
        let lower = c.to_lowercase().next().unwrap_or(c);
        match table(lower) {
            Some(s) if c != lower => {
                let mut it = s.chars();
                if let Some(first) = it.next() {
                    out.extend(first.to_uppercase());
                    out.push_str(it.as_str());
                }
            }
            Some(s) => out.push_str(s),
            None => out.push(c),
        }
    }
    out
}

/// Russian and Ukrainian letters, after the usual English transliteration.
fn cyrillic(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'ґ' => "g",
        'д' => "d",
        'е' => "e",
        'ё' => "yo",
        'є' => "ye",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'і' => "i",
        'ї' => "yi",
        'й' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' | 'ь' => "",
        'ы' => "y",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

/// Greek after ELOT 743, without accents; "ου" is "ou".
fn greek(text: &str) -> String {
    let plain: String = text.nfd().filter(|c| !matches!(c, '\u{0300}'..='\u{036f}')).collect();
    let plain = plain.replace("ου", "ou").replace("Ου", "Ou").replace("ΟΥ", "OU");
    by_table(&plain, |c| {
        Some(match c {
            'α' => "a",
            'β' => "v",
            'γ' => "g",
            'δ' => "d",
            'ε' => "e",
            'ζ' => "z",
            'η' => "i",
            'θ' => "th",
            'ι' => "i",
            'κ' => "k",
            'λ' => "l",
            'μ' => "m",
            'ν' => "n",
            'ξ' => "x",
            'ο' => "o",
            'π' => "p",
            'ρ' => "r",
            'σ' | 'ς' => "s",
            'τ' => "t",
            'υ' => "y",
            'φ' => "f",
            'χ' => "ch",
            'ψ' => "ps",
            'ω' => "o",
            _ => return None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_are_told_apart() {
        assert_eq!(of("夜明けの街", false), Some(Script::Japanese));
        assert_eq!(of("夜明街", true), Some(Script::Japanese), "kanji only, in a song with kana");
        assert_eq!(of("我爱你", false), Some(Script::Chinese));
        assert_eq!(of("사랑해", false), Some(Script::Korean));
        assert_eq!(of("Привет", false), Some(Script::Cyrillic));
        assert_eq!(of("Καλημέρα", false), Some(Script::Greek));
        assert_eq!(of("Paper boats", false), None);
    }

    #[test]
    fn korean_follows_revised_romanization() {
        assert_eq!(latin("한국어", Script::Korean).as_deref(), Some("hangugeo"));
        assert_eq!(latin("사랑해 너를", Script::Korean).as_deref(), Some("saranghae neoreul"));
        assert_eq!(latin("몰라", Script::Korean).as_deref(), Some("molla"));
        assert_eq!(latin("밤하늘", Script::Korean).as_deref(), Some("bamhaneul"));
    }

    #[test]
    fn chinese_gets_pinyin_with_tones() {
        assert_eq!(latin("我爱你", Script::Chinese).as_deref(), Some("wǒ ài nǐ"));
        assert_eq!(latin("你好, world", Script::Chinese).as_deref(), Some("nǐ hǎo , world"));
    }

    #[test]
    fn cyrillic_and_greek_by_table() {
        assert_eq!(latin("Привет, мир", Script::Cyrillic).as_deref(), Some("Privet, mir"));
        assert_eq!(latin("Щастя є", Script::Cyrillic).as_deref(), Some("Shchastya ye"));
        assert_eq!(latin("Καλημέρα ουρανέ", Script::Greek).as_deref(), Some("Kalimera ourane"));
    }
}
