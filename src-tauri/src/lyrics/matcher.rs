//! Track matching (PLAN.md 7.1): normalisation of titles and artists, and scoring of LRCLIB
//! candidates against what the player reports.

use unicode_normalization::UnicodeNormalization;

/// Candidates whose duration differs more than this are a different edition.
pub const MAX_DURATION_DIFF_S: f64 = 8.0;
/// Up to this difference the duration counts as matching.
pub const GOOD_DURATION_DIFF_S: f64 = 3.0;
pub const MIN_TITLE_SIMILARITY: f64 = 0.85;
pub const MIN_ARTIST_SIMILARITY: f64 = 0.6;

/// Lowercase, NFKC (full-width letters, ligatures), punctuation dropped, whitespace collapsed.
pub fn normalize(s: &str) -> String {
    let folded: String =
        s.nfkc().flat_map(char::to_lowercase).map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Title without additions in brackets and without a " - Remastered 2011" style suffix.
/// Used only in fallback stages: the exact title comes first.
pub fn strip_decorations(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in title.chars() {
        match c {
            '(' | '[' | '（' | '【' | '「' => depth += 1,
            ')' | ']' | '）' | '】' | '」' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let cut = [" - ", " – ", " — "].iter().filter_map(|sep| out.find(sep)).min();
    let out = cut.map(|i| &out[..i]).unwrap_or(&out).trim().to_owned();
    if out.is_empty() { title.trim().to_owned() } else { out }
}

/// The first of several artists: "A, B", "A & B", "A feat. B", "A x B" → "A".
pub fn main_artist(artist: &str) -> String {
    let lower = artist.to_lowercase();
    let separators = [",", " & ", ";", " feat.", " feat ", " ft.", " featuring ", " with ", " x ", " × ", "、", " / "];
    let cut = separators.iter().filter_map(|sep| lower.find(sep)).min();
    // `find` on the lowercased text gives a byte index into it; map back through char counts.
    let main = match cut {
        Some(i) => {
            let chars = lower[..i].chars().count();
            artist.chars().take(chars).collect::<String>()
        }
        None => artist.to_owned(),
    };
    let main = main.trim();
    if main.is_empty() { artist.trim().to_owned() } else { main.to_owned() }
}

pub fn title_similarity(a: &str, b: &str) -> f64 {
    let exact = strsim::jaro_winkler(&normalize(a), &normalize(b));
    let stripped = strsim::jaro_winkler(&normalize(&strip_decorations(a)), &normalize(&strip_decorations(b)));
    exact.max(stripped)
}

pub fn artist_similarity(a: &str, b: &str) -> f64 {
    let full = strsim::jaro_winkler(&normalize(a), &normalize(b));
    let main = strsim::jaro_winkler(&normalize(&main_artist(a)), &normalize(&main_artist(b)));
    full.max(main)
}

/// What the player reports.
#[derive(Clone, Debug)]
pub struct Wanted<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub duration_s: Option<f64>,
}

/// A candidate as LRCLIB describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub title: String,
    pub artist: String,
    pub duration_s: f64,
    pub synced: bool,
    pub instrumental: bool,
}

/// Score in 0..1, or `None` if the candidate is a different song or edition.
pub fn score(c: &Candidate, w: &Wanted) -> Option<f64> {
    let duration = match w.duration_s {
        Some(want) => {
            let diff = (c.duration_s - want).abs();
            if diff > MAX_DURATION_DIFF_S {
                return None;
            }
            if diff <= GOOD_DURATION_DIFF_S {
                1.0
            } else {
                1.0 - (diff - GOOD_DURATION_DIFF_S) / (MAX_DURATION_DIFF_S - GOOD_DURATION_DIFF_S) * 0.7
            }
        }
        None => 0.5,
    };
    let title = title_similarity(&c.title, w.title);
    if title < MIN_TITLE_SIMILARITY {
        return None;
    }
    let artist = artist_similarity(&c.artist, w.artist);
    if artist < MIN_ARTIST_SIMILARITY {
        return None;
    }
    let usable = if c.synced {
        1.0
    } else if c.instrumental {
        0.6
    } else {
        0.0
    };
    Some(title * 0.35 + artist * 0.2 + duration * 0.3 + usable * 0.15)
}

/// Index of the best candidate, if any is acceptable.
pub fn best(candidates: &[Candidate], w: &Wanted) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| score(c, w).map(|s| (i, s)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(title: &str, artist: &str, duration_s: f64, synced: bool) -> Candidate {
        Candidate { title: title.into(), artist: artist.into(), duration_s, synced, instrumental: false }
    }

    #[test]
    fn normalisation() {
        assert_eq!(normalize("  Don’t  STOP – Me!  "), "don t stop me");
        assert_eq!(normalize("ＡＢＣ　１２３"), "abc 123");
        assert_eq!(normalize("夜明けの街"), "夜明けの街");
        assert_eq!(normalize("Ça Plane Pour Moi"), "ça plane pour moi");
    }

    #[test]
    fn decorations_are_stripped() {
        assert_eq!(strip_decorations("Paper Boats (feat. Somebody)"), "Paper Boats");
        assert_eq!(strip_decorations("Paper Boats - Remastered 2011"), "Paper Boats");
        assert_eq!(strip_decorations("Paper Boats [Live at Home] - Radio Edit"), "Paper Boats");
        assert_eq!(strip_decorations("夜明け（アニメ版）"), "夜明け");
        assert_eq!(strip_decorations("(Untitled)"), "(Untitled)");
    }

    #[test]
    fn main_artist_is_the_first_one() {
        assert_eq!(main_artist("Alpha, Beta"), "Alpha");
        assert_eq!(main_artist("Alpha & Beta"), "Alpha");
        assert_eq!(main_artist("Alpha feat. Beta"), "Alpha");
        assert_eq!(main_artist("Alpha FT. Beta"), "Alpha");
        assert_eq!(main_artist("Ärzte x Beta"), "Ärzte");
        assert_eq!(main_artist("Simon & Garfunkel"), "Simon");
        assert_eq!(main_artist("Solo"), "Solo");
    }

    #[test]
    fn duration_decides_the_edition() {
        let w = Wanted { title: "Paper Boats", artist: "The Inventions", duration_s: Some(231.0) };
        let cands = [
            cand("Paper Boats", "The Inventions", 223.0, true), // other edition, 8 s off: allowed but weak
            cand("Paper Boats", "The Inventions", 280.0, true), // live version: rejected
            cand("Paper Boats", "The Inventions", 232.0, true), // the one
            cand("Paper Boats", "The Inventions", 231.0, false), // exact, but only plain lyrics
        ];
        assert_eq!(best(&cands, &w), Some(2));
        assert!(score(&cands[1], &w).is_none());
    }

    #[test]
    fn different_songs_are_rejected() {
        let w = Wanted { title: "Paper Boats", artist: "The Inventions", duration_s: Some(200.0) };
        assert!(score(&cand("Paper Planes", "Someone Else", 200.0, true), &w).is_none());
        assert!(score(&cand("Stone Bridges", "The Inventions", 200.0, true), &w).is_none());
        // Decorations and featured artists still match.
        assert!(score(&cand("Paper Boats (Remastered)", "The Inventions, Guest", 201.0, true), &w).is_some());
    }

    #[test]
    fn japanese_titles_match_when_written_alike() {
        let w = Wanted { title: "夜明けの街", artist: "架空バンド", duration_s: Some(250.0) };
        assert!(score(&cand("夜明けの街", "架空バンド", 251.0, true), &w).is_some());
        assert!(score(&cand("ＹＯＡＫＥ", "架空バンド", 251.0, true), &w).is_none());
    }

    #[test]
    fn unknown_duration_relies_on_names() {
        let w = Wanted { title: "Paper Boats", artist: "The Inventions", duration_s: None };
        assert!(score(&cand("Paper Boats", "The Inventions", 999.0, true), &w).is_some());
    }
}
