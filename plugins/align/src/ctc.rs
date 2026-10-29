//! CTC forced alignment, as torchaudio.functional.forced_align in P0: the Viterbi path through
//! the targets with optional blanks between them (blank is id 0). Every target gets the frames
//! it holds on that path.

/// Frame range `[first, after_last)` of each target, or `None` when the frames cannot hold the
/// targets. `logp` is `frames × vocab` log probabilities.
pub fn align(logp: &[f32], frames: usize, vocab: usize, targets: &[u32]) -> Option<Vec<(usize, usize)>> {
    let l = targets.len();
    let repeats = targets.windows(2).filter(|p| p[0] == p[1]).count();
    if l == 0 || frames < l + repeats || logp.len() < frames * vocab {
        return None;
    }
    // State s: even is a blank, odd is target s / 2.
    let states = 2 * l + 1;
    let label = |s: usize| if s.is_multiple_of(2) { 0 } else { targets[s / 2] as usize };
    let lp = |t: usize, s: usize| logp[t * vocab + label(s)];
    let mut prev = vec![f32::NEG_INFINITY; states];
    let mut cur = vec![f32::NEG_INFINITY; states];
    let mut back = vec![0u8; frames * states];
    prev[0] = lp(0, 0);
    prev[1] = lp(0, 1);
    for t in 1..frames {
        for s in 0..states {
            let x0 = prev[s];
            let x1 = if s > 0 { prev[s - 1] } else { f32::NEG_INFINITY };
            let skip = s > 1 && !s.is_multiple_of(2) && targets[s / 2] != targets[s / 2 - 1];
            let x2 = if skip { prev[s - 2] } else { f32::NEG_INFINITY };
            // Ties as in torchaudio: the skip only when strictly best, then the step, else stay.
            let (best, from) = if x2 > x1 && x2 > x0 {
                (x2, 2)
            } else if x1 > x0 && x1 > x2 {
                (x1, 1)
            } else {
                (x0, 0)
            };
            cur[s] = best + lp(t, s);
            back[t * states + s] = from;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let mut s = if prev[states - 1] > prev[states - 2] { states - 1 } else { states - 2 };
    if !prev[s].is_finite() {
        return None;
    }
    let mut spans = vec![(usize::MAX, 0usize); l];
    for t in (0..frames).rev() {
        if !s.is_multiple_of(2) {
            let span = &mut spans[s / 2];
            span.0 = t;
            span.1 = span.1.max(t + 1);
        }
        s -= back[t * states + s] as usize;
    }
    spans.iter().all(|s| s.0 != usize::MAX).then_some(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{Lcg, golden};

    #[test]
    fn matches_torchaudio() {
        let g = &golden()["ctc"];
        let (t, v) = (g["T"].as_u64().unwrap() as usize, g["V"].as_u64().unwrap() as usize);
        let mut rng = Lcg::new(g["seed"].as_u64().unwrap());
        let scale = g["scale"].as_f64().unwrap();
        let mut logp = Vec::with_capacity(t * v);
        for _ in 0..t {
            let row: Vec<f32> = (0..v).map(|_| (rng.next() * scale) as f32).collect();
            let max = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let lse = max + row.iter().map(|x| (x - max).exp()).sum::<f32>().ln();
            logp.extend(row.iter().map(|x| x - lse));
        }
        let targets: Vec<u32> = g["targets"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u32).collect();
        let spans = align(&logp, t, v, &targets).unwrap();
        let want: Vec<(usize, usize)> = g["spans"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| (s[1].as_u64().unwrap() as usize, s[2].as_u64().unwrap() as usize))
            .collect();
        assert_eq!(spans, want);
    }

    #[test]
    fn too_few_frames_give_nothing() {
        let logp = vec![-1.0f32; 3 * 4];
        assert_eq!(align(&logp, 3, 4, &[1, 1, 2]), None, "a repeat needs a blank between");
        assert!(align(&logp, 3, 4, &[1, 2, 3]).is_some());
    }

    #[test]
    fn follows_clear_emissions() {
        // Frames: blank, a, a, blank, b, b; targets a b.
        let (v, best) = (3, [0, 1, 1, 0, 2, 2]);
        let logp: Vec<f32> = best.iter().flat_map(|&b| (0..v).map(move |i| if i == b { -0.1 } else { -5.0 })).collect();
        assert_eq!(align(&logp, 6, v, &[1, 2]), Some(vec![(1, 3), (4, 6)]));
    }
}
