//! Model input as SeamlessM4TFeatureExtractor (transformers) makes it, which the aligner was
//! trained with: a Kaldi-style log-mel filterbank with 80 bins (25 ms frames every 10 ms, Povey
//! window, DC removal, pre-emphasis 0.97, samples scaled to 16 bit), normalised per bin over the
//! window, then two frames stacked into one of 160 values. The model gives one frame per 20 ms.

use crate::fft::Fft;

pub const SAMPLE_RATE: u32 = 16_000;
const FRAME: usize = 400;
const HOP: usize = 160;
const FFT: usize = 512;
const BINS: usize = FFT / 2 + 1;
const MELS: usize = 80;
pub const DIM: usize = 2 * MELS;
const MEL_FLOOR: f64 = 1.192092955078125e-07;

pub struct Frontend {
    window: Vec<f64>,
    /// Weight of each FFT bin in each mel filter, `[bin][mel]`.
    mel: Vec<[f64; MELS]>,
    fft: Fft,
}

/// Stacked features, `frames × DIM`, and the attention mask per stacked frame.
pub struct Features {
    pub frames: usize,
    pub data: Vec<f32>,
    pub mask: Vec<i64>,
}

impl Default for Frontend {
    fn default() -> Self {
        Self::new()
    }
}

impl Frontend {
    pub fn new() -> Self {
        // Povey window: a Hann window (symmetric, numpy.hanning) to the power 0.85.
        let window = (0..FRAME)
            .map(|n| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / (FRAME - 1) as f64).cos()).powf(0.85))
            .collect();
        Frontend { window, mel: mel_filters(), fft: Fft::new(FFT) }
    }

    /// Features of 16 kHz samples in -1..1; `None` for less than one frame.
    pub fn features(&self, x: &[f32]) -> Option<Features> {
        if x.len() < FRAME {
            return None;
        }
        let n = 1 + (x.len() - FRAME) / HOP;
        let mut logmel = vec![[0f64; MELS]; n];
        let mut buf = vec![0f64; FRAME];
        let mut power = vec![0f64; BINS];
        for (f, row) in logmel.iter_mut().enumerate() {
            let frame = &x[f * HOP..f * HOP + FRAME];
            for (b, &s) in buf.iter_mut().zip(frame) {
                *b = (s * 32768.0) as f64;
            }
            let mean = buf.iter().sum::<f64>() / FRAME as f64;
            buf.iter_mut().for_each(|b| *b -= mean);
            for i in (1..FRAME).rev() {
                buf[i] -= 0.97 * buf[i - 1];
            }
            buf[0] *= 1.0 - 0.97;
            buf.iter_mut().zip(&self.window).for_each(|(b, w)| *b *= w);
            self.fft.power(&buf, &mut power);
            for (m, out) in row.iter_mut().enumerate() {
                let e: f64 = power.iter().zip(&self.mel).map(|(p, w)| p * w[m]).sum();
                *out = e.max(MEL_FLOOR).ln() as f32 as f64;
            }
        }
        // Zero mean and unit variance per bin over the window (variance with n - 1).
        for m in 0..MELS {
            let mean = logmel.iter().map(|r| r[m]).sum::<f64>() / n as f64;
            let var = if n > 1 { logmel.iter().map(|r| (r[m] - mean).powi(2)).sum::<f64>() / (n - 1) as f64 } else { f64::NAN };
            let scale = (var + 1e-7).sqrt();
            logmel.iter_mut().for_each(|r| r[m] = (r[m] - mean) / scale);
        }
        // Padded to an even count with a zero frame that the mask hides, then stacked in pairs.
        let padded = n.div_ceil(2) * 2;
        let frames = padded / 2;
        let mut data = Vec::with_capacity(frames * DIM);
        for f in 0..padded {
            match logmel.get(f) {
                Some(r) => data.extend(r.iter().map(|&v| v as f32)),
                None => data.extend([0f32; MELS]),
            }
        }
        let mask = (0..frames).map(|i| i64::from(2 * i + 1 < n)).collect();
        Some(Features { frames, data, mask })
    }
}

/// Kaldi mel filters, triangles in mel space from 20 Hz to 8 kHz (transformers mel_filter_bank
/// with mel_scale "kaldi" and triangularize_in_mel_space).
fn mel_filters() -> Vec<[f64; MELS]> {
    let mel = |hz: f64| 1127.0 * (1.0 + hz / 700.0).ln();
    let (lo, hi) = (mel(20.0), mel(SAMPLE_RATE as f64 / 2.0));
    let edges: Vec<f64> = (0..MELS + 2).map(|i| lo + (hi - lo) * i as f64 / (MELS + 1) as f64).collect();
    let bin_hz = SAMPLE_RATE as f64 / FFT as f64;
    (0..BINS)
        .map(|k| {
            let x = mel(bin_hz * k as f64);
            let mut row = [0f64; MELS];
            for (m, w) in row.iter_mut().enumerate() {
                let down = (x - edges[m]) / (edges[m + 1] - edges[m]);
                let up = (edges[m + 2] - x) / (edges[m + 2] - edges[m + 1]);
                *w = down.min(up).max(0.0);
            }
            row
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{golden, signal};

    fn check(n: usize) {
        let g = &golden()[&format!("features_{n}")];
        let f = Frontend::new().features(&signal(n, SAMPLE_RATE, 7)).unwrap();
        let shape: Vec<u64> = g["shape"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!((f.frames as u64, DIM as u64), (shape[0], shape[1]));
        let mask: Vec<i64> = g["mask"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
        assert_eq!(f.mask, mask);
        for (row, want) in g["rows"].as_object().unwrap() {
            let r: usize = row.parse().unwrap();
            for (c, w) in want.as_array().unwrap().iter().enumerate() {
                let (got, w) = (f.data[r * DIM + c], w.as_f64().unwrap() as f32);
                assert!((got - w).abs() < 2e-3, "{n}: row {r} col {c}: {got} vs {w}");
            }
        }
        for (c, w) in g["col_mean"].as_array().unwrap().iter().enumerate() {
            let mean = (0..f.frames).map(|r| f.data[r * DIM + c]).sum::<f32>() / f.frames as f32;
            assert!((mean - w.as_f64().unwrap() as f32).abs() < 1e-3, "{n}: mean of col {c}");
        }
    }

    #[test]
    fn matches_transformers_for_an_even_frame_count() {
        check(16_000);
    }

    #[test]
    fn matches_transformers_with_the_padding_frame() {
        check(7_900);
    }

    #[test]
    fn too_short_gives_nothing() {
        assert!(Frontend::new().features(&[0.0; 399]).is_none());
    }
}
