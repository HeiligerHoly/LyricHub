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
use crate::lyrics::cache::Cache;
use crate::lyrics::lrclib::{HttpTransport, Transport};
use crate::lyrics::service::{LyricsService, Request};
use crate::lyrics::types::LyricsEvent;
use crate::settings::SettingsState;
use crate::sources::{PlaybackStatus, SourceEvent, TrackInfo, TrackKind};
use crate::sync::clock::{self, PlaybackClock, Update};
use crate::sync::offsets::{self, Offsets};
use crate::sync_lab;

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
    /// Sum of the offsets; positive shows the lyrics later (PLAN.md 5.2).
    pub offset_ms: i32,
    /// The parts of `offset_ms`, for the settings and the Sync Lab.
    pub offsets: Offsets,
    /// Why this anchor went out: status, seek, jump, slew, adopt, offset, heartbeat, query.
    pub kind: &'static str,
}

fn update_kind(update: &Update) -> &'static str {
    match update {
        Update::None => "none",
        Update::Status { .. } => "status",
        Update::Adopt { .. } => "adopt",
        Update::Seek { .. } => "seek",
        Update::Jump { .. } => "jump",
        Update::Slew { .. } => "slew",
    }
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
    /// Device and song parts; the global part comes from the settings when an anchor is built.
    offsets: Mutex<Offsets>,
    /// Own connection to lyrichub.db for song and device offsets (the lyrics worker has one too).
    db: Mutex<Option<Cache>>,
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
            offsets: Mutex::new(Offsets::default()),
            db: Mutex::new(None),
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

    fn current_key(&self) -> Option<String> {
        self.inner.lock().unwrap().now.track.as_ref().map(|t| t.key.clone())
    }

    fn with_db<T>(&self, f: impl FnOnce(&Cache) -> rusqlite::Result<T>) -> Option<T> {
        let db = self.db.lock().unwrap();
        match db.as_ref().map(f) {
            Some(Ok(v)) => Some(v),
            Some(Err(e)) => {
                tracing::warn!(error = %e, "offset store failed");
                None
            }
            None => None,
        }
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
    let db_path = crate::paths::data_dir().join("lyrichub.db");
    match Cache::open(&db_path) {
        Ok(db) => *app.state::<NowPlayingState>().db.lock().unwrap() = Some(db),
        Err(e) => tracing::error!(error = %e, "offset store unavailable, song and device offsets stay 0"),
    }
    let emit_app = app.clone();
    let service = LyricsService::spawn(db_path, transport, move |event| on_lyrics(&emit_app, event));
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
                    emit_clock(&app, "heartbeat");
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

pub fn current_offsets(app: &AppHandle) -> Offsets {
    let mut o = *app.state::<NowPlayingState>().offsets.lock().unwrap();
    o.global_ms = app.state::<SettingsState>().get().sync.global_offset_ms;
    o
}

/// Current clock anchor with all offsets.
pub fn clock_event(app: &AppHandle, kind: &'static str) -> ClockEvent {
    let snap = app.state::<NowPlayingState>().clock.lock().unwrap().snapshot(Instant::now());
    let offsets = current_offsets(app);
    ClockEvent {
        pos_ms: snap.pos_ms,
        playing: snap.playing,
        rate: snap.rate,
        slew_ms: snap.slew_ms,
        seq: snap.seq,
        offset_ms: offsets.total(),
        offsets,
        kind,
    }
}

pub fn emit_clock(app: &AppHandle, kind: &'static str) {
    emit(app, events::CLOCK, &clock_event(app, kind));
}

/// Nudge (PLAN.md 5.7): changes and stores the offset of the song that plays. Returns the new
/// song offset, or `None` when nothing plays.
pub fn nudge_song(app: &AppHandle, delta_ms: i32) -> Option<i32> {
    let state = app.state::<NowPlayingState>();
    let key = state.current_key()?;
    let ms = {
        let mut o = state.offsets.lock().unwrap();
        o.song_ms = offsets::nudged(o.song_ms, delta_ms);
        o.song_ms
    };
    state.with_db(|db| db.set_song_offset(&key, ms));
    tracing::info!(key = %key, delta_ms, song_offset_ms = ms, "song offset nudged");
    sync_lab::record(app, Instant::now(), sync_lab::entry("offset", "song", ms as f64));
    emit_clock(app, "offset");
    Some(ms)
}

/// Sets the song offset of the current song (settings page, e.g. reset to 0).
pub fn set_song_offset(app: &AppHandle, ms: i32) -> Option<i32> {
    let state = app.state::<NowPlayingState>();
    let key = state.current_key()?;
    let ms = ms.clamp(-offsets::LIMIT_MS, offsets::LIMIT_MS);
    state.offsets.lock().unwrap().song_ms = ms;
    state.with_db(|db| db.set_song_offset(&key, ms));
    tracing::info!(key = %key, song_offset_ms = ms, "song offset set");
    emit_clock(app, "offset");
    Some(ms)
}

/// Loads the stored offset of a new song; it applies at once (0 if never corrected).
fn apply_song_offset(app: &AppHandle, key: &str) {
    let state = app.state::<NowPlayingState>();
    let ms = state.with_db(|db| db.song_offset(key)).unwrap_or(0);
    let changed = {
        let mut o = state.offsets.lock().unwrap();
        std::mem::replace(&mut o.song_ms, ms) != ms
    };
    tracing::info!(key = %key, song_offset_ms = ms, "song offset applied");
    sync_lab::record(app, Instant::now(), sync_lab::entry("offset", "song", ms as f64));
    if changed {
        emit_clock(app, "offset");
    }
}

/// A song got a new key (its duration arrived late): its stored offset moves along.
fn rekey_song_offset(app: &AppHandle, old: &str, new: &str) {
    let state = app.state::<NowPlayingState>();
    let moved = state.with_db(|db| {
        if db.has_song_offset(old)? && !db.has_song_offset(new)? {
            let ms = db.song_offset(old)?;
            db.set_song_offset(new, ms)?;
            return Ok(true);
        }
        Ok(false)
    });
    if moved == Some(true) {
        tracing::debug!(old, new, "song offset follows the new key");
    }
}

/// Sets the offset of the current audio device (audio_devices.rs, settings, calibration).
pub fn set_device_offset_ms(app: &AppHandle, ms: i32) {
    let changed = {
        let state = app.state::<NowPlayingState>();
        let mut o = state.offsets.lock().unwrap();
        std::mem::replace(&mut o.device_ms, ms) != ms
    };
    sync_lab::record(app, Instant::now(), sync_lab::entry("offset", "device", ms as f64));
    if changed {
        emit_clock(app, "offset");
    }
}

pub fn with_offset_store<T>(app: &AppHandle, f: impl FnOnce(&Cache) -> rusqlite::Result<T>) -> Option<T> {
    app.state::<NowPlayingState>().with_db(f)
}

pub fn deviating_sources(app: &AppHandle) -> Vec<&'static str> {
    app.state::<NowPlayingState>().clock.lock().unwrap().deviating().to_vec()
}

/// Clock position at `at` minus all offsets: where the overlay stood at that moment.
pub fn display_position(app: &AppHandle, at: Instant) -> f64 {
    let raw = app.state::<NowPlayingState>().clock.lock().unwrap().position(at);
    raw - current_offsets(app).total() as f64
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

fn on_lyrics(app: &AppHandle, event: LyricsEvent) {
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

fn handle(app: &AppHandle, event: SourceEvent) {
    let state = app.state::<NowPlayingState>();
    match event {
        SourceEvent::Track { info, first_sign } => {
            let tooltip = info.as_ref().map(|t| format!("{} – {}", t.title, t.artist));
            let mut rekeyed_offset: Option<(String, String)> = None;
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
                if let (false, Some(old), Some(next)) = (new_song, old_key.as_deref(), &info) {
                    if old != next.key {
                        rekeyed_offset = Some((old.to_owned(), next.key.clone()));
                    }
                }
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
            if let Some((old, new)) = rekeyed_offset {
                rekey_song_offset(app, &old, &new);
            }
            if new_song {
                sync_lab::record(app, Instant::now(), sync_lab::entry("track", "smtc", 0.0));
                state.clock.lock().unwrap().on_track_change();
                if let Some(t) = &info {
                    apply_song_offset(app, &t.key);
                    // Ads and podcast episodes have no lyrics to look for (PLAN.md 5.5).
                    match t.kind {
                        TrackKind::Ad | TrackKind::Podcast => tracing::info!(kind = ?t.kind, "no lyrics lookup for this kind"),
                        _ => request_lyrics(app, t, false),
                    }
                }
            }
            crate::tray::set_tooltip(app, tooltip.as_deref());
            crate::tray::set_player_present(app, info.is_some());
        }
        SourceEvent::Status(status) => {
            state.inner.lock().unwrap().now.status = status;
            emit(app, events::PLAYBACK, &PlaybackEvent { status });
            let now = Instant::now();
            let update = state.clock.lock().unwrap().on_status(status == PlaybackStatus::Playing, now);
            if update.changed() {
                tracing::debug!(?update, "clock status");
                let pos = state.clock.lock().unwrap().position(now);
                sync_lab::record(app, now, sync_lab::LabEntry { update: "status", ..sync_lab::entry("status", "smtc", pos) });
                emit_clock(app, update_kind(&update));
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
            let source = match obs.source {
                crate::sources::SourceId::Smtc => "smtc",
            };
            let (update, residual) = {
                let mut clock = state.clock.lock().unwrap();
                let residual = (obs.playing && clock.anchor().playing).then(|| obs.pos_ms - clock.position(obs.valid_at));
                let update = clock.on_observation(
                    clock::Observation {
                        source,
                        pos_ms: obs.pos_ms,
                        valid_at: obs.valid_at,
                        playing: obs.playing,
                        confidence: obs.confidence as f64,
                    },
                    now,
                );
                (update, residual)
            };
            let age_ms = now.saturating_duration_since(obs.valid_at).as_secs_f64() * 1000.0;
            sync_lab::record(
                app,
                now,
                sync_lab::LabEntry {
                    residual_ms: residual,
                    age_ms: Some(age_ms),
                    update: update_kind(&update),
                    ..sync_lab::entry("obs", source, obs.pos_ms)
                },
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
                emit_clock(app, update_kind(&update));
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
