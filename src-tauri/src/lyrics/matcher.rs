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

/// Which way a song's speed was changed (IDEEN.md, package B).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    /// Nightcore, "Sped Up": the original is longer.
    Faster,
    /// "Slowed", "Slowed + Reverb", Daycore: the original is shorter.
    Slower,
}

impl Speed {
    /// Accepted length of the original divided by the length of this version.
    fn ratio(self) -> (f64, f64) {
        match self {
            Speed::Faster => (1.08, 1.5),
            Speed::Slower => (0.65, 0.92),
        }
    }
}

/// A sped-up or slowed version: the original's title and which way the speed was changed.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedVariant {
    pub title: String,
    pub speed: Speed,
    /// The artist field names an upload channel ("Nightcore"), not the singer.
    pub artist_is_channel: bool,
}

fn speed_of(text: &str) -> Option<Speed> {
    let n = format!(" {} ", normalize(text));
    let any = |words: &[&str]| words.iter().any(|w| n.contains(&format!(" {w} ")));
    if any(&["nightcore", "sped up", "speed up", "spedup", "sped"]) {
        Some(Speed::Faster)
    } else if any(&["slowed", "daycore"]) {
        Some(Speed::Slower)
    } else {
        None
    }
}

/// Recognises "Song (Sped Up)", "Song - Slowed + Reverb", "[Nightcore] Song", "Nightcore - Song"
/// and an artist such as "Nightcore". The tag must stand in brackets, after a dash or before the
/// title, so a song called "Speed Up" stays itself.
pub fn speed_variant(title: &str, artist: &str) -> Option<SpeedVariant> {
    let mut found = None;
    let mut base = String::new();
    let mut rest = title;
    while let Some(open) = rest.find(['(', '[']) {
        let close = if rest.as_bytes()[open] == b'(' { ')' } else { ']' };
        let Some(len) = rest[open..].find(close) else { break };
        base.push_str(&rest[..open]);
        let group = &rest[open..=open + len];
        match speed_of(&group[1..group.len() - 1]) {
            Some(s) => found = found.or(Some(s)),
            None => base.push_str(group),
        }
        rest = &rest[open + len + 1..];
    }
    base.push_str(rest);
    let mut base = base.split_whitespace().collect::<Vec<_>>().join(" ");
    for sep in [" - ", " – ", " — "] {
        if let Some(i) = base.find(sep) {
            let (head, tail) = (&base[..i], &base[i + sep.len()..]);
            if let Some(s) = speed_of(tail) {
                found = found.or(Some(s));
                base = head.to_owned();
            } else if let Some(s) = speed_of(head).filter(|_| normalize(head).split(' ').count() <= 2) {
                found = found.or(Some(s));
                base = tail.to_owned();
            }
            break;
        }
    }
    let channel = speed_of(artist);
    let speed = found.or(channel)?;
    let base = base.trim().to_owned();
    Some(SpeedVariant { title: if base.is_empty() { title.trim().to_owned() } else { base }, speed, artist_is_channel: channel.is_some() })
}

/// What the player reports.
#[derive(Clone, Debug)]
pub struct Wanted<'a> {
    pub title: &'a str,
    /// Empty when unknown (an upload channel): then only the title decides.
    pub artist: &'a str,
    pub duration_s: Option<f64>,
    /// Set when looking for the original of a sped-up or slowed version: the length must then
    /// differ by the right ratio instead of matching.
    pub speed: Option<Speed>,
}

/// A candidate as LRCLIB describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub title: String,
    pub artist: String,
    pub duration_s: f64,
    pub synced: bool,
    pub instrumental: bool,
    /// Start of the last synced line. Some entries carry a length that does not fit their
    /// lyrics (a "Blinding Lights" of 160 s whose last line starts at 195 s).
    pub last_line_s: Option<f64>,
}

/// A line may start this long after the end of the song it belongs to (rounded lengths).
const LAST_LINE_SLACK_S: f64 = 2.0;

/// Score in 0..1, or `None` if the candidate is a different song or edition.
pub fn score(c: &Candidate, w: &Wanted) -> Option<f64> {
    // Lyrics that run past the end of the song cannot be this edition: past the player's length,
    // or, for the original of a sped-up or slowed version, past the entry's own length.
    let end = if w.speed.is_some() { Some(c.duration_s) } else { w.duration_s };
    if let (Some(end), Some(last)) = (end, c.last_line_s)
        && last > end + LAST_LINE_SLACK_S
    {
        return None;
    }
    let duration = match (w.duration_s, w.speed) {
        (Some(want), Some(speed)) => {
            let (low, high) = speed.ratio();
            if !(low..=high).contains(&(c.duration_s / want)) {
                return None;
            }
            0.8
        }
        (Some(want), None) => {
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
        (None, _) => 0.5,
    };
    let title = title_similarity(&c.title, w.title);
    if title < MIN_TITLE_SIMILARITY {
        return None;
    }
    let artist = if w.artist.is_empty() { 0.5 } else { artist_similarity(&c.artist, w.artist) };
    if artist < MIN_ARTIST_SIMILARITY && !w.artist.is_empty() {
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
        .filter_map(|(i, c)| score(c, w).map(|s| (i, s + if w.speed.is_some() { agreement(candidates, c) } else { 0.0 })))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// For the original of a sped-up or slowed version all fitting entries score alike, and LRCLIB
/// lists some songs with wrong lengths too (a 199 s song also as 246 s, its lyrics ending at
/// 191 s). The length that most entries of the same artist agree on breaks the tie; it decides
/// how far the lyrics are stretched.
fn agreement(candidates: &[Candidate], c: &Candidate) -> f64 {
    let artist = normalize(&main_artist(&c.artist));
    let alike = candidates
        .iter()
        .filter(|o| (o.duration_s - c.duration_s).abs() <= 2.0 && normalize(&main_artist(&o.artist)) == artist)
        .count();
    alike as f64 * 0.001
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(title: &str, artist: &str, duration_s: f64, synced: bool) -> Candidate {
        Candidate { title: title.into(), artist: artist.into(), duration_s, synced, instrumental: false, last_line_s: None }
    }

    #[test]
    fn the_length_most_entries_agree_on_wins_for_an_original() {
        let w = Wanted { title: "Paper Boats", artist: "", duration_s: Some(170.0), speed: Some(Speed::Faster) };
        let c = |artist: &str, duration_s| Candidate { last_line_s: Some(190.0), ..cand("Paper Boats", artist, duration_s, true) };
        let list = [c("The Inventions", 199.0), c("The Inventions", 246.0), c("The Inventions", 198.0), c("Covers Inc", 198.0), c("The Inventions, Guest", 200.0), c("The Inventions", 246.0)];
        let chosen = &list[best(&list, &w).unwrap()];
        assert!((198.0..=200.0).contains(&chosen.duration_s) && chosen.artist.starts_with("The Inventions"), "{chosen:?}");
    }

    #[test]
    fn lyrics_past_the_end_do_not_fit() {
        let w = Wanted { title: "Paper Boats", artist: "The Inventions", duration_s: Some(160.0), speed: None };
        let late = Candidate { last_line_s: Some(195.0), ..cand("Paper Boats", "The Inventions", 161.0, true) };
        assert!(score(&late, &w).is_none(), "its length says 161 s, its lyrics run to 195 s");
        assert!(score(&Candidate { last_line_s: Some(150.0), ..late.clone() }, &w).is_some());
        // The original of a sped-up version is checked against its own length.
        let faster = Wanted { speed: Some(Speed::Faster), ..w };
        let original = Candidate { last_line_s: Some(195.0), ..cand("Paper Boats", "The Inventions", 200.0, true) };
        assert!(score(&original, &faster).is_some());
        assert!(score(&Candidate { duration_s: 190.0, ..original }, &faster).is_none());
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
        let w = Wanted { title: "Paper Boats", artist: "The Inventions", duration_s: Some(231.0), speed: None };
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
        let w = Wanted { title: "Paper Boats", artist: "The Inventions", duration_s: Some(200.0), speed: None };
        assert!(score(&cand("Paper Planes", "Someone Else", 200.0, true), &w).is_none());
        assert!(score(&cand("Stone Bridges", "The Inventions", 200.0, true), &w).is_none());
        // Decorations and featured artists still match.
        assert!(score(&cand("Paper Boats (Remastered)", "The Inventions, Guest", 201.0, true), &w).is_some());
    }

    #[test]
    fn japanese_titles_match_when_written_alike() {
        let w = Wanted { title: "夜明けの街", artist: "架空バンド", duration_s: Some(250.0), speed: None };
        assert!(score(&cand("夜明けの街", "架空バンド", 251.0, true), &w).is_some());
        assert!(score(&cand("ＹＯＡＫＥ", "架空バンド", 251.0, true), &w).is_none());
    }

    #[test]
    fn speed_variants_are_recognised() {
        let v = |title: &str, artist: &str| speed_variant(title, artist).map(|v| (v.title, v.speed, v.artist_is_channel));
        assert_eq!(v("Paper Boats (Sped Up)", "The Inventions"), Some(("Paper Boats".into(), Speed::Faster, false)));
        assert_eq!(v("Paper Boats - Slowed + Reverb", "The Inventions"), Some(("Paper Boats".into(), Speed::Slower, false)));
        assert_eq!(v("Paper Boats (feat. Guest) [super slowed]", "The Inventions"), Some(("Paper Boats (feat. Guest)".into(), Speed::Slower, false)));
        assert_eq!(v("[Nightcore] Paper Boats", "Some Channel"), Some(("Paper Boats".into(), Speed::Faster, false)));
        assert_eq!(v("Nightcore - Paper Boats", "Some Channel"), Some(("Paper Boats".into(), Speed::Faster, false)));
        assert_eq!(v("Paper Boats", "Nightcore"), Some(("Paper Boats".into(), Speed::Faster, true)));
        assert_eq!(v("Paper Boats (Daycore)", "sped up songs"), Some(("Paper Boats".into(), Speed::Slower, true)));
        // A tag in the title itself, or none at all, is no variant.
        assert_eq!(v("Speed Up", "The Inventions"), None);
        assert_eq!(v("Paper Boats (Remastered) - Radio Edit", "The Inventions"), None);
    }

    #[test]
    fn the_original_of_a_speed_variant_is_longer_or_shorter() {
        let faster = Wanted { title: "Paper Boats", artist: "The Inventions", duration_s: Some(180.0), speed: Some(Speed::Faster) };
        assert!(score(&cand("Paper Boats", "The Inventions", 230.0, true), &faster).is_some(), "x1.28");
        assert!(score(&cand("Paper Boats", "The Inventions", 182.0, true), &faster).is_none(), "same length");
        assert!(score(&cand("Paper Boats", "The Inventions", 300.0, true), &faster).is_none(), "x1.67");
        let slower = Wanted { speed: Some(Speed::Slower), duration_s: Some(250.0), ..faster.clone() };
        assert!(score(&cand("Paper Boats", "The Inventions", 205.0, true), &slower).is_some(), "x0.82");
        // Without an artist (an upload channel) only the title decides.
        let channel = Wanted { artist: "", ..faster };
        assert!(score(&cand("Paper Boats", "Anyone", 230.0, true), &channel).is_some());
        assert!(score(&cand("Stone Bridges", "Anyone", 230.0, true), &channel).is_none());
    }

    #[test]
    fn unknown_duration_relies_on_names() {
        let w = Wanted { title: "Paper Boats", artist: "The Inventions", duration_s: None, speed: None };
        assert!(score(&cand("Paper Boats", "The Inventions", 999.0, true), &w).is_some());
    }
}
