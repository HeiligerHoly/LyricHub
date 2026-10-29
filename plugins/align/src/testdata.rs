//! Shared helpers for the tests: the reference values from the P0 Python environment
//! (tests/golden.json, made by transformers and torchaudio) and the synthetic signal they used.

use serde_json::Value;

pub fn golden() -> Value {
    serde_json::from_str(include_str!("../tests/golden.json")).unwrap()
}

/// 64-bit LCG, the same in the Python script that made the reference values.
pub struct Lcg(u64);

impl Lcg {
    pub fn new(seed: u64) -> Self {
        Lcg(seed)
    }

    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// 0.3·sin(220 Hz) + 0.3·sin(1 kHz) + 0.3·noise, as 32-bit floats.
pub fn signal(n: usize, rate: u32, seed: u64) -> Vec<f32> {
    let mut rng = Lcg::new(seed);
    (0..n)
        .map(|i| {
            let t = i as f64 / rate as f64;
            let noise = 2.0 * rng.next() - 1.0;
            let two_pi = 2.0 * std::f64::consts::PI;
            (0.3 * (two_pi * 220.0 * t).sin() + 0.3 * (two_pi * 1000.0 * t).sin() + 0.3 * noise) as f32
        })
        .collect()
}
