//! What plays right now, shared by all windows (PLAN.md 4.2). A consumer thread turns source
//! events into `track`, `playback` and `clock` events and asks the lyrics worker for the lyrics
//! of each new song; windows that open later read the state with `get_now_playing`,
//! `get_lyrics` and `get_clock`.
//!
//! Clock (PLAN.md 3.4, 4.4): `sync::clock` models the position; an anchor goes out on status
//! changes, corrections, seeks and every 2 s while playing. The frontend extrapolates itself,
//! there is no IPC per frame.
//!
//! Latency (PLAN.md 2, DoD Phase 1): the overlay confirms each new song once it has drawn it
//! (`overlay_rendered`). The time from the source's first sign of the change to that
//! confirmation is logged and kept for the diagnostics page.

use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::ipc::events;
use crate::lyrics::lrclib::{HttpTransport, Transport};
use crate::lyrics::romaji;
use crate::lyrics::service::{LyricsService, Request};
use crate::lyrics::types::{LyricsEvent, LyricsState};
use crate::settings::SettingsState;
use crate::sources::{PlaybackStatus, SourceEvent, TrackInfo};
use crate::sync::clock::{self, PlaybackClock};

const LATENCY_HISTORY: usize = 50;
const CHANGING_EXPIRY: Duration = Duration::from_secs(1);
const CLOCK_HEARTBEAT: Duration = Duration::from_secs(2);

#[derive(Serialize, Clone, Debug)]
pub struct NowPlaying {
    pub track: Option<TrackInfo>,
    pub status: PlaybackStatus,
}

#[derive(Serialize, Clone, Copy, Debug)]
pub struct PlaybackEvent {
    pub status: PlaybackStatus,
}

/// Payload of the `clock` event (PLAN.md 4.3 `Anchor`). The frontend shows the lyrics at
/// `pos_ms + elapsed × rate (for slew_ms, then 1.0) − offset_ms`.
#[derive(Serialize, Clone, Copy, Debug)]
pub struct ClockEvent {
    /// Raw position at the moment the event was sent, without offsets.
    pub pos_ms: f64,
    pub playing: bool,
    pub rate: f64,
    pub slew_ms: f64,
    pub seq: u64,
    /// Sum of the user offsets; positive shows the lyrics later (PLAN.md 5.2).
    pub offset_ms: i32,
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
    clock: Mutex<PlaybackClock>,
    lyrics_service: OnceLock<LyricsService>,
}

struct Inner {
    now: NowPlaying,
    /// When `track-changing` went out; cleared by the next song. A seek to 0 brings no new
    /// song, so the mark expires after `CHANGING_EXPIRY`.
    changing_since: Option<Instant>,
    pending: Option<Pending>,
    latencies: VecDeque<LatencySample>,
    /// Lyrics of the current song, once the worker answered.
    lyrics: Option<LyricsEvent>,
}

impl Default for NowPlayingState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                now: NowPlaying { track: None, status: PlaybackStatus::Closed },
                changing_since: None,
                pending: None,
                latencies: VecDeque::new(),
                lyrics: None,
            }),
            clock: Mutex::new(PlaybackClock::new(Instant::now())),
            lyrics_service: OnceLock::new(),
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

    pub fn lyrics(&self) -> Option<LyricsEvent> {
        self.inner.lock().unwrap().lyrics.clone()
    }
}

/// Starts the lyrics worker, the consumer of source events and the clock heartbeat.
pub fn start(app: AppHandle, rx: Receiver<SourceEvent>) {
    let transport: Arc<dyn Transport> = match HttpTransport::new() {
        Ok(t) => Arc::new(t),
        Err(e) => {
            tracing::error!(error = %e, "HTTP client unavailable, lyrics lookups will fail");
            Arc::new(Unreachable(e))
        }
    };
    let emit_app = app.clone();
    let service = LyricsService::spawn(crate::paths::data_dir().join("lyrichub.db"), transport, move |event| {
        on_lyrics(&emit_app, event)
    });
    let _ = app.state::<NowPlayingState>().lyrics_service.set(service);

    let consumer = app.clone();
    std::thread::Builder::new()
        .name("playback".into())
        .spawn(move || {
            for event in rx {
                handle(&consumer, event);
            }
        })
        .expect("failed to spawn playback thread");

    std::thread::Builder::new()
        .name("clock-heartbeat".into())
        .spawn(move || {
            loop {
                std::thread::sleep(CLOCK_HEARTBEAT);
                let playing = app.state::<NowPlayingState>().clock.lock().unwrap().anchor().playing;
                if playing {
                    emit_clock(&app);
                }
            }
        })
        .expect("failed to spawn clock heartbeat");
}

struct Unreachable(String);

impl Transport for Unreachable {
    fn get(&self, _: &str, _: &[(&str, String)]) -> Result<(u16, String), String> {
        Err(self.0.clone())
    }
}

/// Current clock anchor with the offsets from the settings.
pub fn clock_event(app: &AppHandle) -> ClockEvent {
    let snap = app.state::<NowPlayingState>().clock.lock().unwrap().snapshot(Instant::now());
    ClockEvent {
        pos_ms: snap.pos_ms,
        playing: snap.playing,
        rate: snap.rate,
        slew_ms: snap.slew_ms,
        seq: snap.seq,
        offset_ms: app.state::<SettingsState>().get().sync.global_offset_ms,
    }
}

pub fn emit_clock(app: &AppHandle) {
    emit(app, events::CLOCK, &clock_event(app));
}

/// Asks LRCLIB again for the current song, bypassing the cache.
pub fn reload_lyrics(app: &AppHandle) {
    let track = app.state::<NowPlayingState>().snapshot().track;
    if let Some(t) = track {
        tracing::info!(key = %t.key, "lyrics reload requested");
        request_lyrics(app, &t, true);
    }
}

fn request_lyrics(app: &AppHandle, t: &TrackInfo, force: bool) {
    if let Some(service) = app.state::<NowPlayingState>().lyrics_service.get() {
        service.request(Request {
            key: t.key.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            duration_ms: t.duration_ms,
            force,
        });
    }
}

fn on_lyrics(app: &AppHandle, mut event: LyricsEvent) {
    if app.state::<SettingsState>().get().appearance.romaji {
        add_romaji(&mut event);
    }
    {
        let state = app.state::<NowPlayingState>();
        let mut inner = state.inner.lock().unwrap();
        // The worker may answer for a song that is no longer playing.
        if inner.now.track.as_ref().map(|t| t.key.as_str()) != Some(event.key.as_str()) {
            tracing::debug!(key = %event.key, "dropping lyrics of a previous song");
            return;
        }
        inner.lyrics = Some(event.clone());
    }
    emit(app, events::LYRICS, &event);
}

/// Romaji for the Japanese lines of a song (docs/romaji.md). Runs once per song; the result
/// stays with the lyrics of the current song. Returns how many lines got romaji.
fn add_romaji(event: &mut LyricsEvent) -> usize {
    if event.state != LyricsState::Synced {
        return 0;
    }
    let started = Instant::now();
    let count = romaji::annotate(&mut event.lines);
    if count > 0 {
        tracing::info!(key = %event.key, lines = count, took_ms = ms(started.elapsed()), "romaji ready");
    }
    count
}

/// The romaji setting changed. Turned on, the current song gets its romaji right away; the
/// overlay switches with the `settings` event that follows. Turned off, the overlay shows the
/// original lines again and the romaji just stay unused.
pub fn romaji_changed(app: &AppHandle, on: bool) {
    let state = app.state::<NowPlayingState>();
    let Some(mut lyrics) = state.lyrics().filter(|_| on) else { return };
    if add_romaji(&mut lyrics) == 0 {
        return;
    }
    {
        let mut inner = state.inner.lock().unwrap();
        // The song may have changed meanwhile.
        if inner.lyrics.as_ref().map(|l| l.key.as_str()) != Some(lyrics.key.as_str()) {
            return;
        }
        inner.lyrics = Some(lyrics.clone());
    }
    emit(app, events::LYRICS, &lyrics);
}

fn handle(app: &AppHandle, event: SourceEvent) {
    let state = app.state::<NowPlayingState>();
    match event {
        SourceEvent::Track { info, first_sign } => {
            let tooltip = info.as_ref().map(|t| format!("{} – {}", t.title, t.artist));
            let (new_song, rekeyed) = {
                let mut inner = state.inner.lock().unwrap();
                let new_song = match (&inner.now.track, &info) {
                    (Some(cur), Some(next)) => !cur.same_song(next),
                    (None, Some(_)) => true,
                    _ => false,
                };
                // Only a change from one song to another counts for the latency statistics;
                // the first song after the start or after Spotify opened includes page loading.
                if new_song && inner.now.track.is_some() {
                    let key = info.as_ref().map(|t| t.key.clone()).unwrap_or_default();
                    inner.pending = Some(Pending { key, first_sign, emitted: Instant::now() });
                }
                // Same song with a new key (the duration arrived later): the lyrics stay its own.
                let old_key = inner.now.track.as_ref().map(|t| t.key.clone());
                let mut rekeyed = None;
                if let (false, Some(next), Some(lyrics)) = (new_song, &info, inner.lyrics.as_mut()) {
                    if old_key.as_deref() == Some(lyrics.key.as_str()) && lyrics.key != next.key {
                        lyrics.key = next.key.clone();
                        rekeyed = Some(lyrics.clone());
                    }
                }
                if new_song || info.is_none() {
                    inner.lyrics = None;
                }
                inner.now.track = info.clone();
                inner.changing_since = None;
                (new_song, rekeyed)
            };
            emit(app, events::TRACK, &info);
            if let Some(lyrics) = rekeyed {
                emit(app, events::LYRICS, &lyrics);
            }
            if new_song {
                state.clock.lock().unwrap().on_track_change();
                if let Some(t) = &info {
                    request_lyrics(app, t, false);
                }
            }
            crate::tray::set_tooltip(app, tooltip.as_deref());
        }
        SourceEvent::Status(status) => {
            state.inner.lock().unwrap().now.status = status;
            emit(app, events::PLAYBACK, &PlaybackEvent { status });
            let update = state.clock.lock().unwrap().on_status(status == PlaybackStatus::Playing, Instant::now());
            if update.changed() {
                tracing::debug!(?update, "clock status");
                emit_clock(app);
            }
        }
        SourceEvent::Observation(obs) => {
            if obs.likely_track_change {
                let first = {
                    let mut inner = state.inner.lock().unwrap();
                    let fresh = inner.changing_since.is_none_or(|t| t.elapsed() > CHANGING_EXPIRY);
                    if fresh {
                        inner.changing_since = Some(Instant::now());
                    }
                    fresh
                };
                if first {
                    emit(app, events::TRACK_CHANGING, &());
                }
            }
            let now = Instant::now();
            let update = state.clock.lock().unwrap().on_observation(
                clock::Observation { pos_ms: obs.pos_ms, valid_at: obs.valid_at, playing: obs.playing },
                now,
            );
            // Every sync decision is logged with its values (PLAN.md 15).
            tracing::debug!(
                source = ?obs.source,
                pos_ms = obs.pos_ms,
                age_ms = now.saturating_duration_since(obs.valid_at).as_millis() as u64,
                playing = obs.playing,
                confidence = obs.confidence,
                ?update,
                "clock observation"
            );
            if update.changed() {
                emit_clock(app);
            }
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

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 10_000.0).round() / 10.0
}
