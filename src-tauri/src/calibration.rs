//! Calibration assistant and sync protocol (PLAN.md 5.7, 11.3). Both use the running song:
//! the user presses Space at the audible start of a line, and each tap records where the
//! overlay stood (clock minus offsets) relative to that line's timestamp.
//!
//! - Calibrate: five taps; outliers beyond 600 ms are dropped and the median is proposed as
//!   the offset of the current audio device.
//! - Protocol: any number of taps over several songs; median, p95 and bias per song and in
//!   total, written to `logs/sync-protocol-<unix>.json` for docs/sync-report.md.
//!
//! Space (and Esc to stop) are registered as global keys only while a session runs, like
//! Enter and Esc in the edit mode: the overlay never holds the keyboard focus.

use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::app_state::{self, NowPlayingState};
use crate::i18n::{current, pick};
use crate::ipc::events;
use crate::lyrics::types::LyricsState;
use crate::sync::offsets::calibration_median;
use crate::sync::taps::{self, TapStats};
use crate::sync_lab;

pub const CALIBRATION_TAPS: usize = 5;
pub const OUTLIER_MS: f64 = 600.0;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Calibrate,
    Protocol,
}

#[derive(Serialize, Clone, Debug)]
pub struct Tap {
    pub key: String,
    pub title: String,
    pub line_index: usize,
    pub line_start_ms: u32,
    /// Overlay position at the tap minus the line start; positive = lyrics too early.
    pub error_ms: f64,
    pub unix_ms: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct SongStats {
    pub key: String,
    pub title: String,
    pub stats: TapStats,
}

/// Payload of the `calibration` event and of `get_calibration`.
#[derive(Serialize, Clone, Debug)]
pub struct CalibrationView {
    pub active: bool,
    pub mode: Mode,
    pub taps: Vec<Tap>,
    /// Taps that hit no line (too far from any line start).
    pub missed: usize,
    pub needed: usize,
    pub total: Option<TapStats>,
    pub per_song: Vec<SongStats>,
    /// Proposed new offset of the current audio device (calibrate mode, once complete).
    pub suggestion_ms: Option<i32>,
    pub device_offset_ms: i32,
    /// Path of the written protocol file (protocol mode, after stop).
    pub report_path: Option<String>,
    /// Why a session could not start or a tap was ignored.
    pub message: Option<String>,
}

struct Session {
    active: bool,
    mode: Mode,
    taps: Vec<Tap>,
    missed: usize,
    report_path: Option<String>,
    message: Option<String>,
    device_offset_at_start: i32,
}

#[derive(Default)]
pub struct CalibrationState(Mutex<Option<Session>>);

pub fn view(app: &AppHandle) -> Option<CalibrationView> {
    let state = app.state::<CalibrationState>();
    let guard = state.0.lock().unwrap();
    guard.as_ref().map(|s| build_view(app, s))
}

fn build_view(app: &AppHandle, s: &Session) -> CalibrationView {
    let errors: Vec<f64> = s.taps.iter().map(|t| t.error_ms).collect();
    let mut per_song: Vec<SongStats> = Vec::new();
    for tap in &s.taps {
        if per_song.iter().any(|p| p.key == tap.key) {
            continue;
        }
        let song: Vec<f64> = s.taps.iter().filter(|t| t.key == tap.key).map(|t| t.error_ms).collect();
        if let Some(stats) = taps::stats(&song) {
            per_song.push(SongStats { key: tap.key.clone(), title: tap.title.clone(), stats });
        }
    }
    let suggestion_ms = (s.mode == Mode::Calibrate)
        .then(|| calibration_median(&errors, OUTLIER_MS, CALIBRATION_TAPS.min(3)))
        .flatten()
        .filter(|_| s.taps.len() >= CALIBRATION_TAPS)
        .map(|m| (s.device_offset_at_start as f64 + m).round() as i32);
    CalibrationView {
        active: s.active,
        mode: s.mode,
        taps: s.taps.clone(),
        missed: s.missed,
        needed: if s.mode == Mode::Calibrate { CALIBRATION_TAPS } else { 0 },
        total: taps::stats(&errors),
        per_song,
        suggestion_ms,
        device_offset_ms: app_state::current_offsets(app).device_ms,
        report_path: s.report_path.clone(),
        message: s.message.clone(),
    }
}

fn emit_view(app: &AppHandle) {
    if let Some(v) = view(app) {
        if let Err(e) = app.emit(events::CALIBRATION, &v) {
            tracing::warn!(error = %e, "could not emit calibration state");
        }
    }
}

/// Starts a session; fails without synced lyrics for the current song.
pub fn start(app: &AppHandle, mode: Mode) -> Result<CalibrationView, String> {
    if app.state::<crate::overlay::OverlayState>().is_editing() {
        return Err(pick(current(app), "Erst den Edit-Modus beenden.", "Finish the edit mode first.").into());
    }
    let lyrics_ok = app.state::<NowPlayingState>().lyrics().is_some_and(|l| l.state == LyricsState::Synced);
    if !lyrics_ok {
        return Err(pick(
            current(app),
            "Für die Messung braucht der laufende Song synchrone Lyrics.",
            "The measurement needs synced lyrics for the current song.",
        )
        .into());
    }
    {
        let state = app.state::<CalibrationState>();
        *state.0.lock().unwrap() = Some(Session {
            active: true,
            mode,
            taps: Vec::new(),
            missed: 0,
            report_path: None,
            message: None,
            device_offset_at_start: app_state::current_offsets(app).device_ms,
        });
    }
    crate::hotkeys::set_tap_keys(app, true);
    tracing::info!(?mode, "tap measurement started");
    emit_view(app);
    Ok(view(app).expect("session just created"))
}

/// One Space press at `at`.
pub fn tap(app: &AppHandle, at: Instant) {
    let display = app_state::display_position(app, at);
    let track = app.state::<NowPlayingState>().snapshot().track;
    let lyrics = app.state::<NowPlayingState>().lyrics();
    let finished_calibration = {
        let state = app.state::<CalibrationState>();
        let mut guard = state.0.lock().unwrap();
        let Some(s) = guard.as_mut().filter(|s| s.active) else { return };
        let line = match (&track, &lyrics) {
            (Some(t), Some(l)) if l.state == LyricsState::Synced && l.key == t.key => {
                let starts: Vec<(u32, bool)> = l.lines.iter().map(|x| (x.start_ms, !x.text.is_empty())).collect();
                taps::nearest_line(&starts, display).map(|i| (t, i, l.lines[i].start_ms))
            }
            _ => None,
        };
        match line {
            Some((t, index, start)) => {
                let error_ms = (display - start as f64).round();
                let unix_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
                tracing::info!(title = %t.title, line = index, line_start_ms = start, error_ms, "tap");
                sync_lab::record(app, at, sync_lab::entry("tap", "user", error_ms));
                s.taps.push(Tap {
                    key: t.key.clone(),
                    title: t.title.clone(),
                    line_index: index,
                    line_start_ms: start,
                    error_ms,
                    unix_ms,
                });
                s.message = None;
            }
            None => {
                s.missed += 1;
                s.message = Some(
                    pick(
                        current(app),
                        "Kein Zeilenanfang in der Nähe, Tap nicht gezählt.",
                        "No line start nearby, tap not counted.",
                    )
                    .into(),
                );
            }
        }
        s.mode == Mode::Calibrate && s.taps.len() >= CALIBRATION_TAPS
    };
    if finished_calibration {
        stop(app);
    } else {
        emit_view(app);
    }
}

/// Ends the session; a protocol is written to the logs folder.
pub fn stop(app: &AppHandle) {
    let protocol = {
        let state = app.state::<CalibrationState>();
        let mut guard = state.0.lock().unwrap();
        let Some(s) = guard.as_mut().filter(|s| s.active) else { return };
        s.active = false;
        s.mode == Mode::Protocol && !s.taps.is_empty()
    };
    crate::hotkeys::set_tap_keys(app, false);
    if protocol {
        let path = write_protocol(app);
        let state = app.state::<CalibrationState>();
        if let Some(s) = state.0.lock().unwrap().as_mut() {
            s.report_path = path;
        }
    }
    if let Some(v) = view(app) {
        tracing::info!(mode = ?v.mode, taps = v.taps.len(), missed = v.missed, total = ?v.total, suggestion_ms = v.suggestion_ms, "tap measurement ended");
    }
    emit_view(app);
}

/// Takes the calibration suggestion as the offset of the current audio device.
pub fn apply_suggestion(app: &AppHandle) -> Result<i32, String> {
    let lang = current(app);
    let suggestion =
        view(app).and_then(|v| v.suggestion_ms).ok_or(pick(lang, "Kein Vorschlag vorhanden.", "No suggestion yet."))?;
    let device = crate::audio_devices::current_id(app).ok_or(pick(
        lang,
        "Kein Audiogerät erkannt.",
        "No audio device found.",
    ))?;
    crate::audio_devices::set_offset(app, &device, suggestion);
    tracing::info!(device_offset_ms = suggestion, "calibration applied");
    emit_view(app);
    Ok(suggestion)
}

fn write_protocol(app: &AppHandle) -> Option<String> {
    let v = view(app)?;
    let unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let report = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "unix_time": unix,
        "offsets": app_state::current_offsets(app),
        "devices": crate::audio_devices::list(app),
        "total": v.total,
        "per_song": v.per_song,
        "taps": v.taps,
        "missed": v.missed,
    });
    let path = crate::logging::log_dir().join(format!("sync-protocol-{unix}.json"));
    match serde_json::to_string_pretty(&report)
        .map_err(|e| e.to_string())
        .and_then(|t| std::fs::write(&path, t).map_err(|e| e.to_string()))
    {
        Ok(()) => {
            tracing::info!(path = %path.display(), "sync protocol written");
            Some(path.display().to_string())
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not write sync protocol");
            None
        }
    }
}
