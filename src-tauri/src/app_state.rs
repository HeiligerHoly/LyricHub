//! What plays right now, shared by all windows (PLAN.md 4.2). A consumer thread turns source
//! events into `track` and `playback` events; windows that open later read the state with
//! `get_now_playing`.
//!
//! Latency (PLAN.md 2, DoD Phase 1): the overlay confirms each new song once it has drawn it
//! (`overlay_rendered`). The time from the source's first sign of the change to that
//! confirmation is logged and kept for the diagnostics page.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::ipc::events;
use crate::sources::{PlaybackStatus, SourceEvent, TrackInfo};

const LATENCY_HISTORY: usize = 50;

#[derive(Serialize, Clone, Debug)]
pub struct NowPlaying {
    pub track: Option<TrackInfo>,
    pub status: PlaybackStatus,
}

#[derive(Serialize, Clone, Copy, Debug)]
pub struct PlaybackEvent {
    pub status: PlaybackStatus,
}

#[derive(Serialize, Clone, Debug)]
pub struct LatencySample {
    pub title: String,
    pub artist: String,
    /// From the source's first sign of the change (new timeline or new title) to the overlay
    /// having drawn the new title.
    pub first_sign_to_render_ms: f64,
    /// From emitting the `track` event to the overlay having drawn it.
    pub emit_to_render_ms: f64,
    pub unix_ms: u64,
}

struct Pending {
    key: String,
    first_sign: Instant,
    emitted: Instant,
}

pub struct NowPlayingState {
    inner: Mutex<Inner>,
}

struct Inner {
    now: NowPlaying,
    pending: Option<Pending>,
    latencies: VecDeque<LatencySample>,
}

impl Default for NowPlayingState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                now: NowPlaying { track: None, status: PlaybackStatus::Closed },
                pending: None,
                latencies: VecDeque::new(),
            }),
        }
    }
}

impl NowPlayingState {
    pub fn snapshot(&self) -> NowPlaying {
        self.inner.lock().unwrap().now.clone()
    }

    pub fn latencies(&self) -> Vec<LatencySample> {
        self.inner.lock().unwrap().latencies.iter().cloned().collect()
    }
}

pub fn spawn_consumer(app: AppHandle, rx: Receiver<SourceEvent>) {
    std::thread::Builder::new()
        .name("playback".into())
        .spawn(move || {
            for event in rx {
                handle(&app, event);
            }
        })
        .expect("failed to spawn playback thread");
}

fn handle(app: &AppHandle, event: SourceEvent) {
    let state = app.state::<NowPlayingState>();
    match event {
        SourceEvent::Track { info, first_sign } => {
            let tooltip = info.as_ref().map(|t| format!("{} – {}", t.title, t.artist));
            {
                let mut inner = state.inner.lock().unwrap();
                // Only a change from one song to another counts for the latency statistics;
                // the first song after the start or after Spotify opened includes page loading.
                let track_change = matches!((&inner.now.track, &info), (Some(cur), Some(next)) if !cur.same_song(next));
                if track_change {
                    let key = info.as_ref().map(|t| t.key.clone()).unwrap_or_default();
                    inner.pending = Some(Pending { key, first_sign, emitted: Instant::now() });
                }
                inner.now.track = info.clone();
            }
            emit(app, events::TRACK, &info);
            crate::tray::set_tooltip(app, tooltip.as_deref());
        }
        SourceEvent::Status(status) => {
            state.inner.lock().unwrap().now.status = status;
            emit(app, events::PLAYBACK, &PlaybackEvent { status });
        }
        SourceEvent::Observation(obs) => {
            // Phase 2 feeds these into sync/clock.rs; until then they are only logged.
            tracing::trace!(
                source = ?obs.source,
                pos_ms = obs.pos_ms,
                age_ms = obs.valid_at.elapsed().as_millis() as u64,
                playing = obs.playing,
                confidence = obs.confidence,
                likely_track_change = obs.likely_track_change,
                "observation"
            );
        }
    }
}

fn emit<S: Serialize + Clone>(app: &AppHandle, name: &str, payload: &S) {
    if let Err(e) = app.emit(name, payload) {
        tracing::warn!(event = name, error = %e, "could not emit event");
    }
}

/// The overlay has drawn the song with `key`.
pub fn rendered(app: &AppHandle, key: &str) {
    let state = app.state::<NowPlayingState>();
    let mut inner = state.inner.lock().unwrap();
    let current_key = inner.now.track.as_ref().map(|t| t.key.as_str());
    let matches = inner.pending.as_ref().is_some_and(|p| p.key == key || current_key == Some(key));
    if !matches {
        return;
    }
    let Some(p) = inner.pending.take() else { return };
    let now = Instant::now();
    let (title, artist) = inner.now.track.as_ref().map(|t| (t.title.clone(), t.artist.clone())).unwrap_or_default();
    let sample = LatencySample {
        first_sign_to_render_ms: ms(now - p.first_sign),
        emit_to_render_ms: ms(now - p.emitted),
        title,
        artist,
        unix_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
    };
    tracing::info!(
        title = %sample.title,
        first_sign_to_render_ms = sample.first_sign_to_render_ms,
        emit_to_render_ms = sample.emit_to_render_ms,
        "track change shown in overlay"
    );
    if inner.latencies.len() >= LATENCY_HISTORY {
        inner.latencies.pop_front();
    }
    inner.latencies.push_back(sample);
}

fn ms(d: std::time::Duration) -> f64 {
    (d.as_secs_f64() * 10_000.0).round() / 10.0
}
