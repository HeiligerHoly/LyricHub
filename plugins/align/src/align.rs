//! Word times of one line from the sound around it, the pipeline of P0 (docs/spike-align.md):
//! features of the window, the model's log probabilities, the CTC path of the line's targets,
//! and per unit the frames of its first and last character.

use std::path::Path;
use std::sync::OnceLock;

use ort::environment::Environment;
use ort::session::{RunOptions, Session};
use ort::session::builder::GraphOptimizationLevel;
use ort::value::TensorRef;

use crate::features::{DIM, Frontend, SAMPLE_RATE};
use crate::text::{Targets, Vocab, targets};

/// A line's window starts this much before its start and ends this much after the next line
/// starts (P0).
/// Auto-Sync keeps it: windows that start 1.5 s ahead caught the end of the line before and
/// measured worse, also with exact stamps (docs/auto-sync.md).
pub const PAD_MS: u32 = 500;
/// The last line has no next one: 15 s after its start.
const LAST_MS: u32 = 15_000;
/// The model saw up to 30 s at once.
const MAX_WINDOW_MS: u32 = 28_000;
/// Longer windows go to the model in pieces of at most this many seconds with context on both
/// sides (the model README's way for long audio): its memory grows with the square of the
/// length, 0.65 GB for 14 s but 2.2 GB for 28 s on top of the model (docs/plugin-align.md).
const PIECE_S: usize = 14;
const CONTEXT_S: usize = 2;
/// Samples per model frame (20 ms).
const FRAME_SAMPLES: usize = 320;

/// `[from, to)` in ms for each line, from the line starts and the end of the song; the window
/// starts `before_ms` ahead of the line.
pub fn windows(starts: &[u32], end_ms: u32, before_ms: u32) -> Vec<(u32, u32)> {
    starts
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let from = s.saturating_sub(before_ms);
            let to = starts.get(i + 1).map_or(s + LAST_MS, |n| n + PAD_MS);
            (from, to.min(from + MAX_WINDOW_MS).min(end_ms.max(from)))
        })
        .collect()
}

pub struct Model {
    session: Session,
    run: RunOptions,
    frontend: Frontend,
    pub vocab: Vocab,
}

/// ONNX Runtime allows one environment per process, ever, even after the first one is gone: it
/// is built once and kept, so the model can be loaded again after it was unloaded.
fn environment() -> Result<Environment, String> {
    static ENV: OnceLock<Result<Environment, String>> = OnceLock::new();
    ENV.get_or_init(|| ort::init().with_name("lyrichub-align").with_telemetry(false).build().map_err(|e| e.to_string())).clone()
}

impl Model {
    /// Loads the ONNX model; `threads` for the computation (PLAN-plugins.md 5.4).
    pub fn load(onnx: &Path, vocab_json: &str, threads: usize) -> Result<Self, String> {
        Self::load_with(onnx, vocab_json, threads, true)
    }

    /// `shrink` hands the memory of a run back afterwards. Without it, ONNX Runtime keeps the
    /// largest window's memory: attention grows with the square of the window length.
    pub fn load_with(onnx: &Path, vocab_json: &str, threads: usize, shrink: bool) -> Result<Self, String> {
        let e = |e: ort::Error| e.to_string();
        let env = environment()?;
        let session = Session::builder(&env)
            .map_err(e)?
            .with_optimization_level(GraphOptimizationLevel::All)
            .map_err(|e| e.to_string())?
            .with_intra_threads(threads)
            .map_err(|e| e.to_string())?
            .with_inter_threads(1)
            .map_err(|e| e.to_string())?
            .commit_from_file(onnx)
            .map_err(e)?;
        let mut run = RunOptions::new().map_err(e)?;
        if shrink {
            run.set("memory.enable_memory_arena_shrinkage", "cpu:0").map_err(e)?;
        }
        Ok(Model { session, run, frontend: Frontend::new(), vocab: Vocab::parse(vocab_json)? })
    }

    /// Log probabilities of 16 kHz samples: `frames × vocab`, with frames and vocab size. Long
    /// windows run in pieces; each keeps the frames between its context parts.
    pub fn logp(&mut self, audio: &[f32]) -> Result<Option<(Vec<f32>, usize, usize)>, String> {
        self.logp_paced(audio, || {})
    }

    /// As `logp`, calling `between` before each piece: a whole song takes minutes, and the
    /// caller waits there while a fullscreen game runs.
    pub fn logp_paced(&mut self, audio: &[f32], mut between: impl FnMut()) -> Result<Option<(Vec<f32>, usize, usize)>, String> {
        let mut out = Vec::new();
        let (mut frames, mut vocab) = (0, 0);
        for p in pieces(audio.len()) {
            between();
            let Some((lp, n, v)) = self.run_once(&audio[p.from..p.to])? else { return Ok(None) };
            let first = ((p.keep_from - p.from) / FRAME_SAMPLES).min(n);
            let last = if p.keep_to == audio.len() { n } else { ((p.keep_to - p.from) / FRAME_SAMPLES).min(n) };
            out.extend_from_slice(&lp[first * v..last * v]);
            frames += last - first;
            vocab = v;
        }
        Ok((frames > 0).then_some((out, frames, vocab)))
    }

    fn run_once(&mut self, audio: &[f32]) -> Result<Option<(Vec<f32>, usize, usize)>, String> {
        let Some(f) = self.frontend.features(audio) else { return Ok(None) };
        let e = |e: ort::Error| e.to_string();
        let feats = TensorRef::from_array_view(([1usize, f.frames, DIM], &f.data[..])).map_err(e)?;
        let mask = TensorRef::from_array_view(([1usize, f.frames], &f.mask[..])).map_err(e)?;
        let out = self.session.run_with_options(ort::inputs!["features" => feats, "attention_mask" => mask], &self.run).map_err(e)?;
        let (shape, logp) = out["log_probs"].try_extract_tensor::<f32>().map_err(e)?;
        Ok(Some((logp.to_vec(), shape[1] as usize, shape[2] as usize)))
    }

    /// Word times of a line, `[start_ms, end_ms]` per unit. `audio` is the line's window at
    /// 16 kHz, starting at `window_ms` on the lyrics clock. `None` when the window is too short
    /// for the text or the line has nothing the model knows.
    pub fn line(&mut self, audio: &[f32], window_ms: f64, line_start_ms: u32, text: &str, units: &[String]) -> Result<Option<Vec<[u32; 2]>>, String> {
        let t = targets(text, units, &self.vocab);
        if t.unit.iter().all(Option::is_none) {
            return Ok(None);
        }
        let Some((logp, frames, vocab)) = self.logp(audio)? else { return Ok(None) };
        if vocab != self.vocab.size {
            return Err(format!("model gives {vocab} tokens, vocab has {}", self.vocab.size));
        }
        let Some(spans) = crate::ctc::align(&logp, frames, vocab, &t.ids) else { return Ok(None) };
        let frame_ms = audio.len() as f64 * 1000.0 / SAMPLE_RATE as f64 / frames as f64;
        Ok(Some(sustain(word_times(&spans, &t, units.len(), frame_ms, window_ms, line_start_ms))))
    }
}

/// A gap up to this long between two words counts as sung through.
const SUNG_THROUGH_MS: u32 = 1000;
/// A word before a longer pause, and the last one, is held this long after its last character.
const HOLD_MS: u32 = 300;

/// CTC marks each character for a frame or two, even a long sung vowel, so a word "ends" far too
/// early (81 ms in the median on a real song). For the fill, a word lasts until the next one
/// starts unless a real pause lies between; the starts stay as measured.
pub fn sustain(mut words: Vec<[u32; 2]>) -> Vec<[u32; 2]> {
    for i in 0..words.len() {
        let end = words[i][1];
        words[i][1] = match words.get(i + 1) {
            Some(next) if next[0] <= end + SUNG_THROUGH_MS => next[0].max(end),
            _ => end + HOLD_MS,
        };
    }
    words
}

/// A piece of a window for the model: the samples it gets and the part whose frames count.
#[derive(Debug, PartialEq)]
struct Piece {
    from: usize,
    to: usize,
    keep_from: usize,
    keep_to: usize,
}

/// How many pieces a window of `len` samples takes, for a progress bar.
pub fn piece_count(len: usize) -> usize {
    pieces(len).len()
}

/// Pieces of a window of `len` samples. Short windows are one piece, as in P0.
fn pieces(len: usize) -> Vec<Piece> {
    let piece = PIECE_S * SAMPLE_RATE as usize;
    if len <= piece {
        return vec![Piece { from: 0, to: len, keep_from: 0, keep_to: len }];
    }
    let ctx = CONTEXT_S * SAMPLE_RATE as usize;
    let hop = piece - 2 * ctx;
    let mut out = Vec::new();
    let mut keep_from = 0;
    while keep_from < len {
        let mut keep_to = (keep_from + hop).min(len);
        // A last bit shorter than a second joins this piece.
        if len - keep_to < SAMPLE_RATE as usize {
            keep_to = len;
        }
        out.push(Piece { from: keep_from.saturating_sub(ctx), to: (keep_to + ctx).min(len), keep_from, keep_to });
        keep_from = keep_to;
    }
    out
}

/// Per unit the start of its first and the end of its last character. A unit without
/// characters the model knows sits where the one before it ended (at the line start for the
/// first), as in P0.
pub fn word_times(spans: &[(usize, usize)], t: &Targets, units: usize, frame_ms: f64, offset_ms: f64, line_start_ms: u32) -> Vec<[u32; 2]> {
    let ms = |frame: usize| (offset_ms + frame as f64 * frame_ms).round().max(0.0) as u32;
    let mut out = Vec::with_capacity(units);
    let mut last_end = line_start_ms;
    for u in 0..units {
        let mine = spans.iter().zip(&t.unit).filter(|(_, x)| **x == Some(u)).map(|(s, _)| *s);
        let (first, last) = mine.fold((None, None), |(f, _), s| (f.or(Some(s)), Some(s)));
        let pair = match (first, last) {
            (Some(f), Some(l)) => [ms(f.0), ms(l.1).max(ms(f.0))],
            _ => [last_end, last_end],
        };
        last_end = pair[1];
        out.push(pair);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_outlives_a_model() {
        drop(environment().unwrap());
        assert!(environment().is_ok(), "a model loaded again after the first was unloaded");
    }

    #[test]
    fn windows_follow_p0() {
        assert_eq!(windows(&[1000, 4000, 40_000], 60_000, PAD_MS), vec![(500, 4500), (3500, 31_500), (39_500, 55_000)]);
        assert_eq!(windows(&[200], 5000, PAD_MS), vec![(0, 5000)], "clipped at the song's end");
        assert_eq!(windows(&[3000, 6000], 9000, 1500)[1], (4500, 9000), "an earlier start");
    }

    #[test]
    fn long_windows_go_in_pieces_with_context() {
        assert_eq!(pieces(160_000), vec![Piece { from: 0, to: 160_000, keep_from: 0, keep_to: 160_000 }]);
        let p = pieces(28 * 16_000);
        assert_eq!(
            p,
            vec![
                Piece { from: 0, to: 192_000, keep_from: 0, keep_to: 160_000 },
                Piece { from: 128_000, to: 352_000, keep_from: 160_000, keep_to: 320_000 },
                Piece { from: 288_000, to: 448_000, keep_from: 320_000, keep_to: 448_000 },
            ]
        );
        // Kept parts tile the window, start on frame borders and no piece is longer than 14 s
        // plus the short last bit.
        assert!(p.windows(2).all(|w| w[0].keep_to == w[1].keep_from));
        assert!(p.iter().all(|x| x.keep_from % FRAME_SAMPLES == 0 && x.from % FRAME_SAMPLES == 0));
        assert!(pieces(14 * 16_000 + 8000).iter().all(|x| x.to - x.from <= 15 * 16_000));
    }

    #[test]
    fn words_last_until_the_next_one_unless_there_is_a_pause() {
        let words = vec![[1000, 1080], [1400, 1450], [3000, 3100], [3100, 3100]];
        assert_eq!(sustain(words), vec![[1000, 1400], [1400, 1750], [3000, 3100], [3100, 3400]]);
    }

    #[test]
    fn units_take_their_characters_frames() {
        // Targets: | a b | c | ; unit 0 = "ab", unit 1 = "c".
        let t = Targets { ids: vec![1, 2, 3, 1, 4, 1], unit: vec![None, Some(0), Some(0), None, Some(1), None] };
        let spans = [(0, 2), (2, 4), (5, 7), (7, 9), (10, 12), (12, 15)];
        assert_eq!(word_times(&spans, &t, 2, 20.0, 1000.0, 1500), vec![[1040, 1140], [1200, 1240]]);
    }

    #[test]
    fn a_unit_without_known_characters_follows_the_one_before() {
        let t = Targets { ids: vec![1, 2, 1], unit: vec![None, Some(1), None] };
        let spans = [(0, 1), (3, 6), (6, 8)];
        assert_eq!(word_times(&spans, &t, 3, 20.0, 0.0, 700), vec![[700, 700], [60, 120], [120, 120]]);
    }
}
