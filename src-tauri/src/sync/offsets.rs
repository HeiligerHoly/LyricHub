//! Offsets (PLAN.md 5.2, 5.7): global from the settings (a lasting output latency), per audio
//! device and per song. Positive shows the lyrics later; the overlay shows the lyrics at
//! `clock − total`. Nudge only ever changes the song offset of the song that plays (user
//! decision 14.1), so a correction never carries over into the next song.

use serde::Serialize;

pub const LIMIT_MS: i32 = 5000;

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offsets {
    pub global_ms: i32,
    pub device_ms: i32,
    /// Extra latency of the chosen player, e.g. a browser (settings `sync.player_offsets_ms`).
    pub player_ms: i32,
    pub song_ms: i32,
}

impl Offsets {
    pub fn total(&self) -> i32 {
        self.global_ms + self.device_ms + self.player_ms + self.song_ms
    }
}

/// New song offset after a nudge.
pub fn nudged(current: i32, delta: i32) -> i32 {
    (current + delta).clamp(-LIMIT_MS, LIMIT_MS)
}

/// Median of tap differences for the calibration assistant (PLAN.md 5.7). Taps more than
/// `outlier_ms` off are dropped; `None` if fewer than `min` taps remain.
pub fn calibration_median(diffs: &[f64], outlier_ms: f64, min: usize) -> Option<f64> {
    let mut kept: Vec<f64> = diffs.iter().copied().filter(|d| d.abs() <= outlier_ms).collect();
    if kept.len() < min {
        return None;
    }
    kept.sort_by(|a, b| a.total_cmp(b));
    let n = kept.len();
    Some(if n % 2 == 1 { kept[n / 2] } else { (kept[n / 2 - 1] + kept[n / 2]) / 2.0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_and_nudge() {
        let o = Offsets { global_ms: -50, device_ms: 150, player_ms: 40, song_ms: 100 };
        assert_eq!(o.total(), 240);
        assert_eq!(nudged(4950, 100), LIMIT_MS);
        assert_eq!(nudged(0, -25), -25);
    }

    #[test]
    fn calibration_drops_outliers() {
        assert_eq!(calibration_median(&[110.0, 90.0, 130.0, 900.0, 100.0], 600.0, 3), Some(105.0));
        assert_eq!(calibration_median(&[110.0, -700.0, 800.0], 600.0, 3), None);
        assert_eq!(calibration_median(&[-40.0, -60.0, -50.0], 600.0, 3), Some(-50.0));
    }
}
