//! Sample rate conversion as torchaudio.functional.resample does it with its defaults
//! (windowed sinc with a Hann window, 6 zero crossings, roll-off 0.99), which the P0 prototype
//! used. The plugin turns captured 48 kHz into the model's 16 kHz, the P0 check 44.1 kHz files.

/// `x` at `from` Hz, resampled to `to` Hz.
pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || x.is_empty() {
        return x.to_vec();
    }
    let g = gcd(from, to);
    let (orig, new) = ((from / g) as usize, (to / g) as usize);
    let (kernel, width) = kernel(orig, new);
    let taps = 2 * width + orig;
    // Zero padding: `width` before, `width + orig` after (torchaudio pads the same way).
    let at = |i: isize| -> f32 {
        if i < 0 || i as usize >= x.len() { 0.0 } else { x[i as usize] }
    };
    let blocks = x.len() / orig + 1;
    let target = (new * x.len()).div_ceil(orig);
    let mut out = Vec::with_capacity(target);
    'done: for j in 0..blocks {
        let base = (j * orig) as isize - width as isize;
        for phase in kernel.chunks_exact(taps) {
            if out.len() == target {
                break 'done;
            }
            let mut acc = 0.0f32;
            for (k, &w) in phase.iter().enumerate() {
                acc += at(base + k as isize) * w;
            }
            out.push(acc);
        }
    }
    out
}

/// One row of taps per output phase, and the half width in input samples.
fn kernel(orig: usize, new: usize) -> (Vec<f32>, usize) {
    const ZEROS: f64 = 6.0;
    const ROLLOFF: f64 = 0.99;
    let base = orig.min(new) as f64 * ROLLOFF;
    let width = (ZEROS * orig as f64 / base).ceil() as usize;
    let taps = 2 * width + orig;
    let scale = base / orig as f64;
    let mut kernel = Vec::with_capacity(new * taps);
    for phase in 0..new {
        for k in 0..taps {
            let idx = (k as f64 - width as f64) / orig as f64;
            let t = ((-(phase as f64) / new as f64 + idx) * base).clamp(-ZEROS, ZEROS);
            let window = (t * std::f64::consts::PI / ZEROS / 2.0).cos().powi(2);
            let t = t * std::f64::consts::PI;
            let sinc = if t == 0.0 { 1.0 } else { t.sin() / t };
            kernel.push((sinc * window * scale) as f32);
        }
    }
    (kernel, width)
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{golden, signal};

    fn check(from: u32) {
        let g = &golden()[&format!("resample_{from}")];
        let y = resample(&signal(from as usize / 2, from, 11), from, 16_000);
        assert_eq!(y.len() as u64, g["len"].as_u64().unwrap());
        for (part, at) in [("head", 0), ("mid", 3000), ("tail", y.len() - 16)] {
            for (i, want) in g[part].as_array().unwrap().iter().enumerate() {
                let (got, want) = (y[at + i], want.as_f64().unwrap() as f32);
                assert!((got - want).abs() < 2e-5, "{from} Hz {part}[{i}]: {got} vs {want}");
            }
        }
    }

    #[test]
    fn matches_torchaudio_from_44_1_khz() {
        check(44_100);
    }

    #[test]
    fn matches_torchaudio_from_48_khz() {
        check(48_000);
    }
}
