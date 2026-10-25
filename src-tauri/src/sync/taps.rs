//! Tap measurement (PLAN.md 5.7, 11.3): the user taps at the audible start of a line; the
//! error of a tap is where the overlay stood at that moment minus the line's timestamp.
//! Positive error: the overlay was ahead, the lyrics came too early. Pure functions only.

use serde::Serialize;

/// Taps further than this from any line start belong to no line.
pub const MAX_LINE_DISTANCE_MS: f64 = 1500.0;

/// Index of the line with text whose start is closest to `display_ms`, within the limit.
pub fn nearest_line(starts: &[(u32, bool)], display_ms: f64) -> Option<usize> {
    starts
        .iter()
        .enumerate()
        .filter(|(_, (_, has_text))| *has_text)
        .map(|(i, (start, _))| (i, (display_ms - *start as f64).abs()))
        .filter(|(_, d)| *d <= MAX_LINE_DISTANCE_MS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct TapStats {
    pub count: usize,
    /// Median of the absolute errors (PLAN.md 2: ≤ 100 ms).
    pub median_abs_ms: f64,
    /// 95th percentile of the absolute errors, nearest rank (PLAN.md 2: ≤ 200 ms).
    pub p95_abs_ms: f64,
    /// Median of the signed errors: a lasting shift that an offset could remove.
    pub bias_ms: f64,
    /// Share of taps within ±100 ms.
    pub within_100: f64,
}

pub fn stats(errors: &[f64]) -> Option<TapStats> {
    if errors.is_empty() {
        return None;
    }
    let mut abs: Vec<f64> = errors.iter().map(|e| e.abs()).collect();
    abs.sort_by(|a, b| a.total_cmp(b));
    let mut signed = errors.to_vec();
    signed.sort_by(|a, b| a.total_cmp(b));
    let n = errors.len();
    let rank = ((0.95 * n as f64).ceil() as usize).clamp(1, n) - 1;
    Some(TapStats {
        count: n,
        median_abs_ms: median_sorted(&abs),
        p95_abs_ms: abs[rank],
        bias_ms: median_sorted(&signed),
        within_100: abs.iter().filter(|e| **e <= 100.0).count() as f64 / n as f64,
    })
}

fn median_sorted(v: &[f64]) -> f64 {
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_line_skips_instrumental_markers_and_far_taps() {
        let lines = [(1000, true), (4000, false), (4200, true), (9000, true)];
        assert_eq!(nearest_line(&lines, 1080.0), Some(0));
        assert_eq!(nearest_line(&lines, 4050.0), Some(2));
        assert_eq!(nearest_line(&lines, 6700.0), None);
        assert_eq!(nearest_line(&[], 0.0), None);
    }

    #[test]
    fn stats_of_errors() {
        let s = stats(&[-40.0, 20.0, 60.0, -10.0, 150.0, 30.0, -90.0, 10.0, 0.0, 250.0]).unwrap();
        assert_eq!(s.count, 10);
        assert_eq!(s.median_abs_ms, 35.0);
        assert_eq!(s.p95_abs_ms, 250.0);
        assert_eq!(s.bias_ms, 15.0);
        assert!((s.within_100 - 0.8).abs() < 1e-9);
        assert!(stats(&[]).is_none());
    }
}
