//! Radix-2 FFT for the 512-point frames of the features (features.rs). Small and plenty for the
//! 100 frames per second of a song.

use std::f64::consts::PI;

pub struct Fft {
    n: usize,
    /// exp(-2πik/n) for k < n/2.
    twiddles: Vec<(f64, f64)>,
    /// Bit-reversed index of every position.
    rev: Vec<usize>,
}

impl Fft {
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 2);
        let bits = n.trailing_zeros();
        let twiddles = (0..n / 2)
            .map(|k| {
                let a = -2.0 * PI * k as f64 / n as f64;
                (a.cos(), a.sin())
            })
            .collect();
        let rev = (0..n).map(|i| i.reverse_bits() >> (usize::BITS - bits)).collect();
        Fft { n, twiddles, rev }
    }

    /// Power |X_k|² for k in 0..=n/2 of a real frame padded with zeros to n. Like numpy in
    /// transformers, the complex values are rounded to 32 bits before squaring.
    pub fn power(&self, frame: &[f64], out: &mut [f64]) {
        let n = self.n;
        let mut re = vec![0.0; n];
        let mut im = vec![0.0; n];
        for (i, &x) in frame.iter().enumerate().take(n) {
            re[self.rev[i]] = x;
        }
        let mut size = 2;
        while size <= n {
            let half = size / 2;
            let step = n / size;
            for start in (0..n).step_by(size) {
                for k in 0..half {
                    let (wr, wi) = self.twiddles[k * step];
                    let (a, b) = (start + k, start + k + half);
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            size *= 2;
        }
        for (k, o) in out.iter_mut().enumerate().take(n / 2 + 1) {
            let (r, i) = (re[k] as f32 as f64, im[k] as f32 as f64);
            *o = r * r + i * i;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_a_plain_dft() {
        let n = 64;
        let frame: Vec<f64> = (0..50).map(|i| ((i * 7919) % 101) as f64 / 50.0 - 1.0).collect();
        let mut fast = vec![0.0; n / 2 + 1];
        Fft::new(n).power(&frame, &mut fast);
        for (k, f) in fast.iter().enumerate() {
            let (mut r, mut i) = (0.0, 0.0);
            for (t, x) in frame.iter().enumerate() {
                let a = -2.0 * PI * (k * t) as f64 / n as f64;
                r += x * a.cos();
                i += x * a.sin();
            }
            let slow = r * r + i * i;
            assert!((f - slow).abs() <= 1e-4 * slow.max(1.0), "bin {k}: {f} vs {slow}");
        }
    }
}
