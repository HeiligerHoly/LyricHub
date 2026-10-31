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
    /// The chosen player is a browser: long gaps between reports are normal.
    pub browser: bool,
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
        browser: crate::sources::smtc::is_browser(&app.state::<crate::settings::SettingsState>().get().player.app_id),
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
/// Contains timings and settings, never lyrics text; song details are hidden (see `hide_song`).
const LOG_TAIL_LINES: usize = 400;

/// Song fields the export leaves out, so it can go into a public bug report.
const SONG_FIELDS: [&str; 5] = ["key", "title", "artist", "album", "art"];
const HIDDEN: &str = "(hidden)";

/// Replaces song details anywhere in a JSON value.
fn hide_song(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if SONG_FIELDS.contains(&k.as_str()) && !v.is_null() {
                    *v = serde_json::Value::from(HIDDEN);
                } else {
                    hide_song(v);
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(hide_song),
        _ => {}
    }
}

/// Replaces song details in a log line. Values are written as `title=Some Song artist=...`:
/// a plain value runs until the next ` name=`, a quoted one until its closing quote.
pub fn hide_song_in_line(line: &str) -> String {
    let mut out = line.to_owned();
    for name in ["key", "title", "artist", "album"] {
        let needle = format!(" {name}=");
        let mut from = 0;
        while let Some(pos) = out[from..].find(&needle) {
            let start = from + pos + needle.len();
            let rest = &out[start..];
            let len = if let Some(quoted) = rest.strip_prefix('"') {
                quoted.find('"').map_or(rest.len(), |i| i + 2)
            } else {
                next_field(rest).unwrap_or(rest.len())
            };
            out.replace_range(start..start + len, HIDDEN);
            from = start + HIDDEN.len();
        }
    }
    out
}

/// Replaces the account name in paths (`C:\Users\<name>\...`, also with doubled backslashes as in
/// debug output), so a shared report does not carry it.
pub fn hide_user_folder(line: &str) -> String {
    const MARK: &str = "Users\\";
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find(MARK) {
        let after = i + MARK.len();
        let name = after + (rest[after..].len() - rest[after..].trim_start_matches('\\').len());
        let len = rest[name..].find(['\\', '"']).unwrap_or(rest.len() - name);
        out.push_str(&rest[..name]);
        if len > 0 {
            out.push_str("<user>");
        }
        rest = &rest[name + len..];
    }
    out.push_str(rest);
    out
}

/// Byte offset of the next ` name=` in a log line, where name is a field identifier.
fn next_field(rest: &str) -> Option<usize> {
    rest.match_indices(' ').map(|(i, _)| i).find(|&i| {
        let tail = &rest[i + 1..];
        let ident = tail.find('=').map(|e| &tail[..e]);
        ident.is_some_and(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
    })
}

/// Writes everything a bug report needs into `logs/diagnostics-<unix>.json`: versions,
/// settings, monitors, overlay and guard state, shortcuts, Sync Lab and the log tail.
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
    let mut song = serde_json::json!({
        "now_playing": state.snapshot(),
        "lyrics": lyrics,
        "sync_lab": snapshot(app),
        "track_change_latencies": state.latencies(),
    });
    hide_song(&mut song);
    let report = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "unix_time": unix,
        "now_playing": song["now_playing"],
        "lyrics": song["lyrics"],
        "sync_lab": song["sync_lab"],
        "track_change_latencies": song["track_change_latencies"],
        "devices": crate::audio_devices::list(app),
        "settings": app.state::<SettingsState>().get(),
        "overlay": crate::overlay::diagnostics(app),
        "monitors": crate::overlay::window::list_monitors(),
        "hotkeys": crate::hotkeys::status(app),
        "plugins": crate::plugins::list(app)
            .iter()
            .map(|p| serde_json::json!({ "id": p.id, "version": p.version, "enabled": p.enabled, "state": p.state }))
            .collect::<Vec<_>>(),
        // The logs never contain lyrics (PLAN.md 7.4), so the tail can go along without song details.
        "log_tail": crate::logging::tail(LOG_TAIL_LINES).iter().map(|l| hide_user_folder(&hide_song_in_line(l))).collect::<Vec<_>>(),
        "last_crash": crate::crash::last().map(|c| serde_json::json!({ "unix_time": c.unix, "report": c.report })),
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
    fn export_hides_song_details() {
        let line = r#"INFO lyrichub_lib::sources::smtc: track changed title=Paper Kites Over Town artist=The Invented Band album="Made Up (Deluxe)" duration_ms=180000 via="poll""#;
        assert_eq!(
            hide_song_in_line(line),
            r#"INFO lyrichub_lib::sources::smtc: track changed title=(hidden) artist=(hidden) album=(hidden) duration_ms=180000 via="poll""#
        );
        assert_eq!(hide_song_in_line("tap title=Last Field line=3"), "tap title=(hidden) line=3");
        assert_eq!(hide_song_in_line("song offset set key=band|song|album song_offset_ms=100"), "song offset set key=(hidden) song_offset_ms=100");
        assert_eq!(
            hide_user_folder(r#"file logging enabled dir=C:\Users\Jo Ann\AppData\Roaming\LyricHub\logs report=Some("C:\\Users\\Jo\\x.txt")"#),
            r#"file logging enabled dir=C:\Users\<user>\AppData\Roaming\LyricHub\logs report=Some("C:\\Users\\<user>\\x.txt")"#
        );
        let mut v = serde_json::json!({ "track": { "title": "Invented", "artist": "Nobody", "album": null, "duration_ms": 1 }, "list": [{ "key": "k" }] });
        hide_song(&mut v);
        assert_eq!(v, serde_json::json!({ "track": { "title": "(hidden)", "artist": "(hidden)", "album": null, "duration_ms": 1 }, "list": [{ "key": "(hidden)" }] }));
    }

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
