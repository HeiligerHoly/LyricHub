//! Whole songs for the plugin Recognize (PLAN-plugins-2.md 4), from the model's output for the
//! whole song: the recognized words with times (greedy CTC decoding), lines at pauses, a text
//! comparison against LRCLIB candidates, and lyrics aligned over the whole song, with or without
//! line times. Pure functions, tested without the model.

use crate::align::{PAD_MS, sustain, windows, word_times};
use crate::text::{Vocab, normalize, targets};

/// A pause between two characters longer than this ends a word, even without a `|`.
const WORD_GAP_MS: f64 = 400.0;
/// A pause of at least this long, or this many words, starts a new line.
const LINE_GAP_MS: u32 = 700;
const LINE_WORDS: usize = 8;

/// A recognized word: text as the model writes it (lower case), times, and the mean
/// probability of its characters.
#[derive(Clone, Debug, PartialEq)]
pub struct RecWord {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub prob: f32,
}

/// Greedy decoding: the most likely token per frame, repeats merged, blanks dropped. `|` or a
/// pause of more than 400 ms ends a word.
pub fn greedy_words(logp: &[f32], frames: usize, v: usize, vocab: &Vocab, frame_ms: f64) -> Vec<RecWord> {
    let mut out = Vec::new();
    let mut word = String::new();
    let (mut start, mut end, mut probs) = (0usize, 0usize, Vec::new());
    let mut prev = usize::MAX;
    let close = |word: &mut String, start: usize, end: usize, probs: &mut Vec<f32>, out: &mut Vec<RecWord>| {
        if !word.is_empty() {
            let prob = probs.iter().sum::<f32>() / probs.len().max(1) as f32;
            out.push(RecWord { text: std::mem::take(word), start_ms: (start as f64 * frame_ms) as u32, end_ms: (end as f64 * frame_ms) as u32, prob });
        }
        probs.clear();
    };
    for t in 0..frames {
        let row = &logp[t * v..(t + 1) * v];
        let (id, lp) = row.iter().enumerate().fold((0, f32::NEG_INFINITY), |b, (i, &p)| if p > b.1 { (i, p) } else { b });
        if id == prev && id != 0 {
            if id as u32 != vocab.bar {
                end = t + 1;
            }
            continue;
        }
        prev = id;
        if id == 0 {
            continue;
        }
        if id as u32 == vocab.bar {
            close(&mut word, start, end, &mut probs, &mut out);
            continue;
        }
        let Some(c) = vocab.char_of(id) else { continue };
        if !word.is_empty() && (t - end) as f64 * frame_ms > WORD_GAP_MS {
            close(&mut word, start, end, &mut probs, &mut out);
        }
        if word.is_empty() {
            start = t;
        }
        word.push(c);
        probs.push(lp.exp());
        end = t + 1;
    }
    close(&mut word, start, end, &mut probs, &mut out);
    out
}

/// Recognized words grouped into lines: a pause of at least 700 ms, or 8 words, start a new one.
pub fn lines_from_words(words: &[RecWord]) -> Vec<Vec<RecWord>> {
    let mut lines: Vec<Vec<RecWord>> = Vec::new();
    for w in words {
        let new_line = match lines.last() {
            None => true,
            Some(l) => l.len() >= LINE_WORDS || w.start_ms.saturating_sub(l.last().map_or(0, |x| x.end_ms)) >= LINE_GAP_MS,
        };
        if new_line {
            lines.push(Vec::new());
        }
        lines.last_mut().unwrap().push(w.clone());
    }
    lines
}

/// The text of a recognized line, with a capital letter at the start.
pub fn line_text(words: &[RecWord]) -> String {
    let text = words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

/// Mean character probability of the recognition, weighted by length: how sure it looks.
pub fn confidence(words: &[RecWord]) -> f32 {
    let (sum, n) = words.iter().fold((0.0f32, 0usize), |(s, n), w| {
        let k = w.text.chars().count();
        (s + w.prob * k as f32, n + k)
    });
    if n == 0 { 0.0 } else { sum / n as f32 }
}

/// Share of the recognized words with three or more letters that occur in a candidate's lyrics.
/// Recognition gets about a quarter of the characters wrong (docs/spike-recognize.md), so whole
/// words are compared, not characters.
pub fn match_share(recognized: &[RecWord], candidate: &str) -> f64 {
    let known: std::collections::HashSet<String> = candidate.split_whitespace().map(normalize).filter(|w| w.chars().count() >= 3).collect();
    let ours: Vec<&str> = recognized.iter().map(|w| w.text.as_str()).filter(|w| w.chars().count() >= 3).collect();
    if ours.is_empty() {
        return 0.0;
    }
    ours.iter().filter(|w| known.contains(**w)).count() as f64 / ours.len() as f64
}

/// LyricHub's units of a line written with spaces (lyrics/words.rs): words, where a piece without
/// letters joins the word before it, or the next one at the start of the line.
pub fn units(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut pending = String::new();
    for piece in text.split_whitespace() {
        if piece.chars().any(char::is_alphanumeric) {
            out.push(std::mem::take(&mut pending) + piece);
        } else if let Some(last) = out.last_mut() {
            last.push(' ');
            last.push_str(piece);
        } else {
            pending.push_str(piece);
            pending.push(' ');
        }
    }
    out
}

/// Lyrics without timestamps over the whole song (Q0 M3): per line the start and word times of
/// its units, `None` for a line with nothing the model knows. `None` when the song cannot hold
/// the text.
pub fn align_whole(logp: &[f32], frames: usize, v: usize, vocab: &Vocab, frame_ms: f64, lines: &[String]) -> Option<Vec<Option<Vec<[u32; 2]>>>> {
    let mut ids = Vec::new();
    let mut unit_of = Vec::new();
    let mut ranges = Vec::new();
    let mut base = 0;
    for line in lines {
        let u = units(line);
        let t = targets(line, &u, vocab);
        for (k, (&id, unit)) in t.ids.iter().zip(&t.unit).enumerate() {
            if k == 0 && !ids.is_empty() {
                continue; // the bar between two lines is already there
            }
            ids.push(id);
            unit_of.push(unit.map(|x| x + base));
        }
        ranges.push(base..base + u.len());
        base += u.len();
    }
    let spans = crate::ctc::align(logp, frames, v, &ids)?;
    let ms = |f: usize| (f as f64 * frame_ms).round() as u32;
    let mut first = vec![None; base];
    let mut last = vec![None; base];
    for (s, u) in spans.iter().zip(&unit_of) {
        if let Some(u) = *u {
            first[u] = first[u].or(Some(ms(s.0)));
            last[u] = Some(ms(s.1));
        }
    }
    let mut prev_end = 0;
    Some(
        ranges
            .into_iter()
            .map(|r| {
                if r.clone().all(|u| first[u].is_none()) {
                    return None;
                }
                let words: Vec<[u32; 2]> = r
                    .map(|u| {
                        let pair = match (first[u], last[u]) {
                            (Some(a), Some(b)) => [a, b.max(a)],
                            _ => [prev_end, prev_end],
                        };
                        prev_end = pair[1];
                        pair
                    })
                    .collect();
                Some(sustain(words))
            })
            .collect(),
    )
}

/// Lines with times (an LRCLIB candidate) aligned on the whole song's output: one window per line
/// as in Align, cut from the frames that are already there. Per line its word times, or `None`.
pub fn align_timed(logp: &[f32], frames: usize, v: usize, vocab: &Vocab, frame_ms: f64, lines: &[(u32, String)]) -> Vec<Option<Vec<[u32; 2]>>> {
    let starts: Vec<u32> = lines.iter().map(|l| l.0).collect();
    let end_ms = (frames as f64 * frame_ms) as u32;
    windows(&starts, end_ms, PAD_MS)
        .iter()
        .zip(lines)
        .map(|(&(from, to), (start, text))| {
            let u = units(text);
            let t = targets(text, &u, vocab);
            if t.unit.iter().all(Option::is_none) {
                return None;
            }
            let (a, b) = (((from as f64 / frame_ms) as usize).min(frames), ((to as f64 / frame_ms) as usize).min(frames));
            if b <= a {
                return None;
            }
            let spans = crate::ctc::align(&logp[a * v..b * v], b - a, v, &t.ids)?;
            Some(sustain(word_times(&spans, &t, u.len(), frame_ms, a as f64 * frame_ms, *start)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vocabulary: blank 0, | 1, then a b c d e.
    fn vocab() -> Vocab {
        Vocab::parse(r#"{"<blank>": 0, "|": 1, "a": 2, "b": 3, "c": 4, "d": 5, "e": 6}"#).unwrap()
    }

    /// Log probabilities with one clear token per frame.
    fn frames_of(best: &[usize], v: usize) -> Vec<f32> {
        best.iter().flat_map(|&b| (0..v).map(move |i| if i == b { -0.05 } else { -6.0 })).collect()
    }

    #[test]
    fn greedy_decoding_finds_words_and_their_times() {
        // "ab" then "cd": a a _ b | _ c d d
        let best = [2, 2, 0, 3, 1, 0, 4, 5, 5];
        let w = greedy_words(&frames_of(&best, 7), best.len(), 7, &vocab(), 20.0);
        assert_eq!(w.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["ab", "cd"]);
        assert_eq!((w[0].start_ms, w[0].end_ms), (0, 80));
        assert_eq!((w[1].start_ms, w[1].end_ms), (120, 180));
        assert!(w[0].prob > 0.9);
        assert!(confidence(&w) > 0.9);
    }

    #[test]
    fn a_long_pause_ends_a_word_without_a_bar() {
        let mut best = vec![2];
        best.extend(std::iter::repeat_n(0, 30));
        best.push(3);
        let w = greedy_words(&frames_of(&best, 7), best.len(), 7, &vocab(), 20.0);
        assert_eq!(w.len(), 2, "600 ms of silence between a and b");
    }

    #[test]
    fn lines_break_at_pauses_and_length() {
        let word = |s: u32| RecWord { text: "abc".into(), start_ms: s, end_ms: s + 200, prob: 0.9 };
        let words: Vec<RecWord> = [0, 300, 600, 2000].into_iter().map(word).chain((0..9).map(|i| word(5000 + i * 300))).collect();
        let lines = lines_from_words(&words);
        assert_eq!(lines.iter().map(Vec::len).collect::<Vec<_>>(), [3, 1, 8, 1]);
        assert_eq!(line_text(&lines[0]), "Abc abc abc");
    }

    #[test]
    fn units_follow_lyrichub() {
        assert_eq!(units("paper boats , ok"), ["paper", "boats ,", "ok"]);
        assert_eq!(units("- paper boats"), ["- paper", "boats"]);
    }

    #[test]
    fn candidates_are_compared_by_words() {
        let rec: Vec<RecWord> = ["paper", "boats", "drifting", "at", "nite"]
            .iter()
            .map(|t| RecWord { text: t.to_string(), start_ms: 0, end_ms: 0, prob: 1.0 })
            .collect();
        assert_eq!(match_share(&rec, "Paper boats drifting at night"), 0.75, "three of four long words");
        assert_eq!(match_share(&rec, "Something else entirely"), 0.0);
    }

    #[test]
    fn plain_lyrics_align_over_the_whole_song() {
        // Frames: _ a b _ _ _ c d _ ; lines "ab" and "cd"
        let best = [0, 2, 3, 0, 0, 0, 4, 5, 0];
        let lp = frames_of(&best, 7);
        let out = align_whole(&lp, best.len(), 7, &vocab(), 20.0, &["ab".into(), "cd".into()]).unwrap();
        assert_eq!(out[0].as_ref().unwrap()[0][0], 20);
        assert_eq!(out[1].as_ref().unwrap()[0][0], 120);
        let missing = align_whole(&lp, best.len(), 7, &vocab(), 20.0, &["ab".into(), "xyz".into()]).unwrap();
        assert!(missing[1].is_none(), "nothing the model knows");
    }
}
