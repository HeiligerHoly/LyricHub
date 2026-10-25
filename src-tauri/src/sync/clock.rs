//! PlaybackClock (PLAN.md 3.4, 5.3): a model of the playback position built from source
//! observations. Pure: no Tauri, no threads, every call gets the current time.
//!
//! - Between observations the model extrapolates: `pos + elapsed × rate`.
//! - A residual beyond 1.5 s is a seek or a track change: the model jumps there. Within
//!   500 ms after that, the next observation is taken as it is, because Spotify's second update
//!   after a seek corrects 100–216 ms backwards (docs/spike-smtc.md).
//! - Otherwise residuals collect in a window; their median is the correction. Up to 300 ms it is
//!   slewed in at most ±10 % speed, so the display never jumps backwards; larger corrections
//!   jump. Corrections below 20 ms are ignored.
//! - While paused, observations are adopted directly; nothing moves, so nothing can jitter.
//!
//! A slew is part of the anchor (`rate` until `slew_until`, then 1.0), so the frontend can run
//! the same piecewise model without timers.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const SEEK_THRESHOLD_MS: f64 = 1500.0;
pub const WINDOW: usize = 8;
pub const SLEW_MAX_RATE_DELTA: f64 = 0.10;
pub const DEADBAND_MS: f64 = 20.0;
/// Larger corrections jump instead of slewing for seconds.
pub const SLEW_LIMIT_MS: f64 = 300.0;
pub const MIN_SLEW_MS: f64 = 300.0;
/// After a seek, the next observation within this time is taken as it is.
pub const POST_SEEK_MS: u64 = 500;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub pos_ms: f64,
    pub at: Instant,
    pub playing: bool,
    /// Speed until `slew_until`, afterwards 1.0.
    pub rate: f64,
    pub slew_until: Option<Instant>,
}

impl Anchor {
    fn fixed(pos_ms: f64, at: Instant, playing: bool) -> Self {
        Self { pos_ms, at, playing, rate: 1.0, slew_until: None }
    }

    pub fn position(&self, at: Instant) -> f64 {
        if !self.playing {
            return self.pos_ms;
        }
        let dt = signed_ms(self.at, at);
        match self.slew_until {
            Some(end) if dt > 0.0 => {
                let slew = signed_ms(self.at, end);
                if dt <= slew { self.pos_ms + dt * self.rate } else { self.pos_ms + slew * self.rate + (dt - slew) }
            }
            _ => self.pos_ms + dt,
        }
    }
}

/// One position report of a source.
#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub pos_ms: f64,
    pub valid_at: Instant,
    pub playing: bool,
}

/// What an input changed, for logging and for deciding whether to emit a new anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Update {
    None,
    Status { playing: bool },
    /// Position taken as reported (paused, resume, right after a seek).
    Adopt { by_ms: f64 },
    Seek { from_ms: f64, to_ms: f64 },
    Jump { by_ms: f64 },
    Slew { by_ms: f64, over_ms: f64 },
}

impl Update {
    pub fn changed(&self) -> bool {
        !matches!(self, Update::None)
    }
}

/// Snapshot for the frontend (the `clock` event).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub pos_ms: f64,
    pub playing: bool,
    pub rate: f64,
    /// How long `rate` still applies before the speed returns to 1.0.
    pub slew_ms: f64,
    pub seq: u64,
}

pub struct PlaybackClock {
    anchor: Anchor,
    residuals: VecDeque<f64>,
    post_seek_until: Option<Instant>,
    seq: u64,
}

impl PlaybackClock {
    pub fn new(now: Instant) -> Self {
        Self { anchor: Anchor::fixed(0.0, now, false), residuals: VecDeque::new(), post_seek_until: None, seq: 0 }
    }

    pub fn anchor(&self) -> Anchor {
        self.anchor
    }

    pub fn position(&self, at: Instant) -> f64 {
        self.anchor.position(at)
    }

    pub fn snapshot(&self, now: Instant) -> Snapshot {
        let slew_ms = match self.anchor.slew_until {
            Some(end) if self.anchor.playing && end > now => signed_ms(now, end),
            _ => 0.0,
        };
        Snapshot {
            pos_ms: self.position(now),
            playing: self.anchor.playing,
            rate: if slew_ms > 0.0 { self.anchor.rate } else { 1.0 },
            slew_ms,
            seq: self.seq,
        }
    }

    /// New song: forget residuals of the old one. The anchor stays, because the jump to the
    /// start already came with the timeline before the title (docs/spike-smtc.md).
    pub fn on_track_change(&mut self) {
        self.residuals.clear();
    }

    pub fn on_status(&mut self, playing: bool, now: Instant) -> Update {
        if playing == self.anchor.playing {
            return Update::None;
        }
        self.anchor = Anchor::fixed(self.position(now), now, playing);
        self.residuals.clear();
        self.seq += 1;
        Update::Status { playing }
    }

    pub fn on_observation(&mut self, obs: Observation, now: Instant) -> Update {
        if !obs.playing || !self.anchor.playing {
            // Paused, or resuming before the status event: nothing is moving, take the report.
            let before = self.position(now);
            let at = if obs.playing { obs.valid_at } else { now };
            self.anchor = Anchor::fixed(obs.pos_ms, at, obs.playing);
            self.residuals.clear();
            let by_ms = self.position(now) - before;
            if by_ms.abs() < 0.5 && !obs.playing {
                return Update::None;
            }
            self.seq += 1;
            return Update::Adopt { by_ms };
        }

        let residual = obs.pos_ms - self.position(obs.valid_at);
        if residual.abs() > SEEK_THRESHOLD_MS {
            let from_ms = self.position(now);
            self.anchor = Anchor::fixed(obs.pos_ms, obs.valid_at, true);
            self.residuals.clear();
            self.post_seek_until = Some(now + Duration::from_millis(POST_SEEK_MS));
            self.seq += 1;
            return Update::Seek { from_ms, to_ms: self.position(now) };
        }
        if self.post_seek_until.take().is_some_and(|until| now <= until) {
            if residual.abs() < 0.5 {
                return Update::None;
            }
            self.anchor = Anchor::fixed(obs.pos_ms, obs.valid_at, true);
            self.residuals.clear();
            self.seq += 1;
            return Update::Adopt { by_ms: residual };
        }

        self.residuals.push_back(residual);
        if self.residuals.len() > WINDOW {
            self.residuals.pop_front();
        }
        let correction = median(&self.residuals);
        if correction.abs() < DEADBAND_MS {
            return Update::None;
        }
        let current = self.position(now);
        self.seq += 1;
        if correction.abs() > SLEW_LIMIT_MS {
            self.anchor = Anchor::fixed(current + correction, now, true);
            self.residuals.clear();
            return Update::Jump { by_ms: correction };
        }
        let over_ms = (correction.abs() / SLEW_MAX_RATE_DELTA).max(MIN_SLEW_MS);
        self.anchor = Anchor {
            pos_ms: current,
            at: now,
            playing: true,
            rate: 1.0 + correction / over_ms,
            slew_until: Some(now + Duration::from_secs_f64(over_ms / 1000.0)),
        };
        // The stored residuals were measured against the old model, which now absorbs the correction.
        for r in self.residuals.iter_mut() {
            *r -= correction;
        }
        Update::Slew { by_ms: correction, over_ms }
    }
}

fn median(values: &VecDeque<f64>) -> f64 {
    let mut v: Vec<f64> = values.iter().copied().collect();
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Milliseconds from `from` to `to`, negative if `to` is earlier.
fn signed_ms(from: Instant, to: Instant) -> f64 {
    if to >= from { (to - from).as_secs_f64() * 1000.0 } else { -((from - to).as_secs_f64() * 1000.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulated playback: the true position runs at `rate` from `start_pos`.
    struct Sim {
        t0: Instant,
        start_pos: f64,
        rate: f64,
    }
    impl Sim {
        fn at(&self, ms: f64) -> Instant {
            self.t0 + Duration::from_secs_f64(ms / 1000.0)
        }
        fn truth(&self, ms: f64) -> f64 {
            self.start_pos + ms * self.rate
        }
    }

    fn obs(pos_ms: f64, valid_at: Instant) -> Observation {
        Observation { pos_ms, valid_at, playing: true }
    }

    /// Deterministic pseudo-random noise in [-amp, amp].
    fn noise(i: u64, amp: f64) -> f64 {
        let x = i.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33;
        (x as f64 / (1u64 << 31) as f64 * 2.0 - 1.0) * amp
    }

    /// Feeds an observation every `every_ms` (with `jitter`) for `duration_ms`, sampling the
    /// display every 16 ms. Returns (max error after `settle_ms`, largest backward step).
    fn run(sim: &Sim, clock: &mut PlaybackClock, every_ms: f64, jitter: f64, duration_ms: f64, settle_ms: f64) -> (f64, f64) {
        let (mut max_err, mut max_back, mut last) = (0.0f64, 0.0f64, f64::MIN);
        let mut next_obs = 0.0;
        let mut i = 0u64;
        let mut t = 0.0;
        while t <= duration_ms {
            if t >= next_obs {
                clock.on_observation(obs(sim.truth(t) + noise(i, jitter), sim.at(t)), sim.at(t));
                i += 1;
                next_obs += every_ms;
            }
            let shown = clock.position(sim.at(t));
            if last != f64::MIN {
                max_back = max_back.max(last - shown);
            }
            last = shown;
            if t >= settle_ms {
                max_err = max_err.max((shown - sim.truth(t)).abs());
            }
            t += 16.0;
        }
        (max_err, max_back)
    }

    fn playing_clock(sim: &Sim, pos_ms: f64) -> PlaybackClock {
        let mut c = PlaybackClock::new(sim.t0);
        c.on_status(true, sim.t0);
        c.anchor = Anchor::fixed(pos_ms, sim.t0, true);
        c
    }

    #[test]
    fn constant_offset_is_slewed_away_without_backward_steps() {
        let sim = Sim { t0: Instant::now(), start_pos: 10_000.0, rate: 1.0 };
        for offset in [-250.0, -120.0, 60.0, 280.0] {
            let mut c = playing_clock(&sim, 10_000.0 + offset);
            // SMTC cadence: one update every 4.5 s. After the slew the error is below 20 ms.
            let (err, back) = run(&sim, &mut c, 4500.0, 0.0, 15_000.0, 3000.0);
            assert!(err < DEADBAND_MS, "offset {offset}: error {err}");
            assert!(back <= 0.0, "offset {offset}: went back by {back}");
        }
    }

    #[test]
    fn error_is_small_after_three_samples() {
        let sim = Sim { t0: Instant::now(), start_pos: 0.0, rate: 1.0 };
        let mut c = playing_clock(&sim, 150.0);
        let (err, _) = run(&sim, &mut c, 1000.0, 0.0, 3000.0, 2900.0);
        assert!(err < 50.0, "error {err}");
    }

    #[test]
    fn jitter_200_ms_stays_bounded_and_never_steps_back() {
        let sim = Sim { t0: Instant::now(), start_pos: 30_000.0, rate: 1.0 };
        let mut c = playing_clock(&sim, 30_000.0);
        let (err, back) = run(&sim, &mut c, 1000.0, 200.0, 120_000.0, 10_000.0);
        assert!(err < 120.0, "error {err}");
        assert!(back <= 0.0, "went back by {back}");
    }

    #[test]
    fn spotify_like_jitter_keeps_error_tiny() {
        // Spike A: extrapolation error p95 1 ms, rate +74 ppm.
        let sim = Sim { t0: Instant::now(), start_pos: 0.0, rate: 1.000074 };
        let mut c = playing_clock(&sim, 0.0);
        let (err, back) = run(&sim, &mut c, 4500.0, 1.0, 240_000.0, 0.0);
        assert!(err < DEADBAND_MS + 2.0, "error {err}");
        assert!(back <= 0.0);
    }

    #[test]
    fn drift_is_followed() {
        // 0.1 % is far beyond the 74 ppm measured in spike A and still well tracked.
        let sim = Sim { t0: Instant::now(), start_pos: 0.0, rate: 1.001 };
        let mut c = playing_clock(&sim, 0.0);
        let (err, back) = run(&sim, &mut c, 1000.0, 0.0, 120_000.0, 5000.0);
        assert!(err < 30.0, "error {err}");
        assert!(back <= 0.0);
        // An extreme 1 % drift lags behind the median window but stays bounded.
        let sim = Sim { t0: Instant::now(), start_pos: 0.0, rate: 1.01 };
        let mut c = playing_clock(&sim, 0.0);
        let (err, back) = run(&sim, &mut c, 1000.0, 0.0, 120_000.0, 5000.0);
        assert!(err < 100.0, "error {err}");
        assert!(back <= 0.0);
    }

    #[test]
    fn seek_jumps_and_the_spotify_correction_is_taken_directly() {
        let sim = Sim { t0: Instant::now(), start_pos: 20_000.0, rate: 1.0 };
        let mut c = playing_clock(&sim, 20_000.0);
        // Seek to 0:50; Spotify's first update lands 160 ms too far ...
        let u = c.on_observation(obs(50_160.0, sim.at(1000.0)), sim.at(1000.0));
        assert!(matches!(u, Update::Seek { .. }), "{u:?}");
        // ... the second one, 180 ms later, corrects back: taken as it is.
        let u = c.on_observation(obs(50_180.0, sim.at(1180.0)), sim.at(1180.0));
        assert!(matches!(u, Update::Adopt { by_ms } if (by_ms + 160.0).abs() < 1.0), "{u:?}");
        assert!((c.position(sim.at(2180.0)) - 51_180.0).abs() < 1.0);
    }

    #[test]
    fn track_change_resets_to_the_start() {
        let sim = Sim { t0: Instant::now(), start_pos: 185_000.0, rate: 1.0 };
        let mut c = playing_clock(&sim, 185_000.0);
        let u = c.on_observation(obs(0.0, sim.at(500.0)), sim.at(520.0));
        assert!(matches!(u, Update::Seek { .. }));
        c.on_track_change();
        assert!((c.position(sim.at(1500.0)) - 1000.0).abs() < 1.0);
    }

    #[test]
    fn pause_freezes_and_resume_continues() {
        let sim = Sim { t0: Instant::now(), start_pos: 60_000.0, rate: 1.0 };
        let mut c = playing_clock(&sim, 60_000.0);
        // Status arrives 300 ms after the command at t=1000; Spotify froze at 61_080.
        assert_eq!(c.on_status(false, sim.at(1300.0)), Update::Status { playing: false });
        assert!((c.position(sim.at(5000.0)) - 61_300.0).abs() < 1.0);
        let u = c.on_observation(Observation { pos_ms: 61_080.0, valid_at: sim.at(1290.0), playing: false }, sim.at(1310.0));
        assert!(matches!(u, Update::Adopt { .. }));
        assert_eq!(c.position(sim.at(9000.0)), 61_080.0);
        // Repeated paused reports with the same position change nothing (spike observation 6).
        let u = c.on_observation(Observation { pos_ms: 61_080.0, valid_at: sim.at(5000.0), playing: false }, sim.at(5000.0));
        assert_eq!(u, Update::None);
        // Resume at t=10 s.
        assert_eq!(c.on_status(true, sim.at(10_000.0)), Update::Status { playing: true });
        assert!((c.position(sim.at(11_000.0)) - 62_080.0).abs() < 1.0);
    }

    #[test]
    fn resume_report_before_the_status_event_is_taken() {
        let sim = Sim { t0: Instant::now(), start_pos: 0.0, rate: 1.0 };
        let mut c = PlaybackClock::new(sim.t0);
        let u = c.on_observation(obs(42_000.0, sim.at(100.0)), sim.at(120.0));
        assert!(matches!(u, Update::Adopt { .. }));
        assert!(c.anchor().playing);
        assert!((c.position(sim.at(1100.0)) - 43_000.0).abs() < 1.0);
    }

    #[test]
    fn medium_error_jumps_instead_of_slewing_for_seconds() {
        let sim = Sim { t0: Instant::now(), start_pos: 0.0, rate: 1.0 };
        let mut c = playing_clock(&sim, 0.0);
        let u = c.on_observation(obs(5_800.0, sim.at(5000.0)), sim.at(5000.0));
        assert!(matches!(u, Update::Jump { by_ms } if (by_ms - 800.0).abs() < 1.0), "{u:?}");
    }

    #[test]
    fn snapshot_reports_the_slew() {
        let sim = Sim { t0: Instant::now(), start_pos: 0.0, rate: 1.0 };
        let mut c = playing_clock(&sim, 0.0);
        let u = c.on_observation(obs(1100.0, sim.at(1000.0)), sim.at(1000.0));
        assert!(matches!(u, Update::Slew { by_ms, over_ms } if (by_ms - 100.0).abs() < 1.0 && (over_ms - 1000.0).abs() < 1.0));
        let s = c.snapshot(sim.at(1500.0));
        assert!((s.rate - 1.1).abs() < 1e-9 && (s.slew_ms - 500.0).abs() < 1.0);
        // The report said "100 ms ahead": after the slew the model runs 100 ms ahead of t.
        let s = c.snapshot(sim.at(2500.0));
        assert_eq!((s.rate, s.slew_ms), (1.0, 0.0));
        assert!((s.pos_ms - 2600.0).abs() < 1.0);
    }
}
