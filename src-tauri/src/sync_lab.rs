//! Sync Lab data (PLAN.md 5.9): the last 60 s of observations, clock decisions, status and
//! offset changes, for the diagnostics page and the diagnostics export. The page polls
//! `sync_lab` four times a second while it is open; nothing is sent otherwise.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app_state::{self, ClockEvent, NowPlayingState};
use crate::settings::SettingsState;

const WINDOW: Duration = Duration::from_secs(60);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LabEntry {
    /// Milliseconds since LyricHub started (monotonic).
    pub t_ms: f64,
    /// "obs", "status", "track", "offset" or "tap".
    pub kind: &'static str,
    pub source: &'static str,
    /// Reported position (obs), or the offset (offset), or the tap error (tap).
    pub value_ms: f64,
    /// Observation minus model prediction at its time.
    pub residual_ms: Option<f64>,
    /// Age of the report when it arrived (latency estimate).
    pub age_ms: Option<f64>,
    /// What the clock did: none, slew, jump, seek, adopt, status.
    pub update: &'static str,
}

pub struct LabBuffer {
    start: Instant,
    entries: VecDeque<LabEntry>,
}

impl LabBuffer {
    pub fn new(start: Instant) -> Self {
        Self { start, entries: VecDeque::new() }
    }

    pub fn ms(&self, at: Instant) -> f64 {
        at.saturating_duration_since(self.start).as_secs_f64() * 1000.0
    }

    pub fn push(&mut self, at: Instant, mut entry: LabEntry) {
        entry.t_ms = self.ms(at);
        self.entries.push_back(entry);
        let cutoff = self.ms(at) - WINDOW.as_secs_f64() * 1000.0;
        while self.entries.front().is_some_and(|e| e.t_ms < cutoff) {
            self.entries.pop_front();
        }
    }

    pub fn entries(&self) -> Vec<LabEntry> {
        self.entries.iter().cloned().collect()
    }

    /// Median time between two reports of the source (spike A: 4.5 s for Spotify).
    pub fn cadence_ms(&self) -> Option<f64> {
        let times: Vec<f64> = self.entries.iter().filter(|e| e.kind == "obs").map(|e| e.t_ms).collect();
        let mut gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).filter(|g| *g > 50.0).collect();
        if gaps.is_empty() {
            return None;
        }
        gaps.sort_by(|a, b| a.total_cmp(b));
        Some(gaps[gaps.len() / 2])
    }
}

pub struct SyncLabState(pub Mutex<LabBuffer>);

impl Default for SyncLabState {
    fn default() -> Self {
        Self(Mutex::new(LabBuffer::new(Instant::now())))
    }
}

pub fn entry(kind: &'static str, source: &'static str, value_ms: f64) -> LabEntry {
    LabEntry { t_ms: 0.0, kind, source, value_ms, residual_ms: None, age_ms: None, update: "" }
}

pub fn record(app: &AppHandle, at: Instant, e: LabEntry) {
    if let Some(lab) = app.try_state::<SyncLabState>() {
        lab.0.lock().unwrap().push(at, e);
    }
}

#[derive(Serialize)]
pub struct SyncLab {
    pub now_ms: f64,
    pub entries: Vec<LabEntry>,
    pub clock: ClockEvent,
    pub cadence_ms: Option<f64>,
    /// Sources left out of the fusion because they disagree with the others.
    pub deviating: Vec<&'static str>,
    pub track_key: Option<String>,
}

pub fn snapshot(app: &AppHandle) -> SyncLab {
    let now = Instant::now();
    let (now_ms, entries, cadence_ms) = {
        let lab = app.state::<SyncLabState>();
        let buf = lab.0.lock().unwrap();
        (buf.ms(now), buf.entries(), buf.cadence_ms())
    };
    let state = app.state::<NowPlayingState>();
    SyncLab {
        now_ms,
        entries,
        clock: app_state::clock_event(app, "query"),
        cadence_ms,
        deviating: app_state::deviating_sources(app),
        track_key: state.snapshot().track.map(|t| t.key),
    }
}

#[derive(Serialize)]
struct LyricsSummary {
    key: String,
    state: crate::lyrics::types::LyricsState,
    source: String,
    lines: usize,
    lrc_offset_ms: i32,
}

/// Writes the last 60 s and the current state to `logs/diagnostics-<unix>.json` (PLAN.md 5.9).
/// Contains timings, settings and the song title, never lyrics text.
pub fn export(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let state = app.state::<NowPlayingState>();
    let lyrics = state.lyrics().map(|l| LyricsSummary {
        key: l.key,
        state: l.state,
        source: l.source,
        lines: l.lines.iter().filter(|x| !x.text.is_empty()).count(),
        lrc_offset_ms: l.lrc_offset_ms,
    });
    let report = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "unix_time": unix,
        "now_playing": state.snapshot(),
        "lyrics": lyrics,
        "sync_lab": snapshot(app),
        "track_change_latencies": state.latencies(),
        "devices": crate::audio_devices::list(app),
        "settings": app.state::<SettingsState>().get(),
        "overlay": crate::overlay::diagnostics(app),
    });
    let path = crate::logging::log_dir().join(format!("diagnostics-{unix}.json"));
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    tracing::info!(path = %path.display(), "diagnostics exported");
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_60_seconds_and_measures_cadence() {
        let t0 = Instant::now();
        let mut b = LabBuffer::new(t0);
        for i in 0..30u64 {
            b.push(t0 + Duration::from_millis(i * 4500), entry("obs", "smtc", i as f64));
        }
        let e = b.entries();
        assert!(e.first().unwrap().t_ms >= 29.0 * 4500.0 - 60_000.0);
        assert_eq!(e.len(), 14);
        assert_eq!(b.cadence_ms(), Some(4500.0));
    }
}
