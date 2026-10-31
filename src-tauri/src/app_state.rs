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
use crate::lyrics::romaji;
use crate::lyrics::service::{LyricsService, Request};
use crate::lyrics::types::{LyricsEvent, LyricsState};
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
    /// The worker's way to LRCLIB, also for the list of entries to pick from.
    transport: OnceLock<Arc<dyn Transport>>,
    /// Device and song parts; the global part comes from the settings when an anchor is built.
    offsets: Mutex<Offsets>,
    /// Own connection to lyrichub.db for song and device offsets (the lyrics worker has one too).
    db: Mutex<Option<Cache>>,
    /// Last seek anchor: (seq, when Spotify's report was valid, when the anchor went out).
    pending_seek: Mutex<Option<(u64, Instant, Instant)>>,
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
            transport: OnceLock::new(),
            offsets: Mutex::new(Offsets::default()),
            db: Mutex::new(None),
            pending_seek: Mutex::new(None),
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
    let local_dir = crate::lyrics::local::dir();
    // Made at once, so the folder is there to be found (Settings → Sync opens it).
    let _ = std::fs::create_dir_all(&local_dir);
    let _ = app.state::<NowPlayingState>().transport.set(transport.clone());
    let emit_app = app.clone();
    let service = LyricsService::spawn(db_path, local_dir, transport, move |event| on_lyrics(&emit_app, event));
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
    let s = app.state::<SettingsState>().get();
    o.global_ms = s.sync.global_offset_ms;
    o.player_ms = s.sync.player_offsets_ms.get(&s.player.app_id).copied().unwrap_or(0);
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
    let event = clock_event(app, kind);
    crate::plugins::on_playback(app, &event);
    emit(app, events::CLOCK, &event);
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

fn on_lyrics(app: &AppHandle, mut event: LyricsEvent) {
    if app.state::<SettingsState>().get().appearance.romaji {
        add_romaji(&mut event);
    }
    add_plugin_words(app, &mut event);
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
    crate::plugins::on_lyrics(app, &event);
    crate::plugins::on_lyrics_wanted(app, &event);
}

/// Word times a plugin stored for these lyrics (PLAN-plugins.md 4.6); they replace the estimate.
fn add_plugin_words(app: &AppHandle, event: &mut LyricsEvent) {
    if event.state != LyricsState::Synced {
        return;
    }
    let hash = crate::plugins::words::lyrics_hash(&event.lines);
    let state = app.state::<NowPlayingState>();
    let Some((plugin, json)) = state.with_db(|db| db.plugin_words(&event.key, &hash)).flatten() else { return };
    let Ok(answer) = serde_json::from_str::<Vec<crate::plugins::words::PluginLine>>(&json) else { return };
    let lines = crate::plugins::words::apply(&mut event.lines, &answer);
    if lines > 0 {
        event.word_source = format!("plugin:{plugin}");
        tracing::info!(key = %event.key, plugin = %plugin, lines, "word times from a plugin");
        // Auto-Sync, a switch of the plugin Align (PLAN-plugins-2.md 3).
        if crate::plugins::setting_on(app, "lyrichub-align", "auto_sync") {
            event.line_sync_ms = crate::plugins::words::auto_sync(&mut event.lines);
            tracing::info!(key = %event.key, shift_ms = ?event.line_sync_ms, "lines moved onto the singing");
        }
    }
}

/// Shows the current song's lyrics again, after a plugin setting changed. Through the worker, so
/// a local file or the user's pick wins as always.
pub fn refresh_lyrics(app: &AppHandle) {
    if let Some(t) = app.state::<NowPlayingState>().snapshot().track {
        request_lyrics(app, &t, false);
    }
}

/// The user's own lyrics file for a song, if one fits (IDEEN.md, package B).
fn own_file(app: &AppHandle, key: &str) -> Option<(std::path::PathBuf, String)> {
    let (title, artist) = app.state::<NowPlayingState>().with_db(|db| db.track_names(key)).flatten()?;
    crate::lyrics::local::find(&crate::lyrics::local::dir(), &title, &artist)
}

/// The lyrics without timestamps of a song, for a plugin that aligns them: the user's own file,
/// or what LRCLIB had.
pub fn plain_lyrics(app: &AppHandle, key: &str) -> Option<String> {
    if let Some((_, text)) = own_file(app, key) {
        return (!crate::lyrics::parser::parse(&text).synced).then_some(text);
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    match app.state::<NowPlayingState>().with_db(|db| db.get(key, now)) {
        Some(crate::lyrics::cache::Lookup::Hit(c)) if c.format == crate::lyrics::cache::Format::Plain => Some(c.raw),
        _ => None,
    }
}

/// Lyrics a plugin found or recognized for a song without synced lyrics (PLAN-plugins-2.md 4):
/// stored like lyrics from LRCLIB, with the plugin as source, and its word times next to them.
/// Synced lyrics from LRCLIB always win.
pub fn store_plugin_lyrics(app: &AppHandle, plugin: (&str, &str), found: crate::plugins::words::Found) {
    use crate::lyrics::cache::{Cached, Format, Lookup};
    use crate::plugins::words;
    let state = app.state::<NowPlayingState>();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    if plugin_lyrics_rejected(app, &found.key) {
        return tracing::info!(plugin = plugin.0, key = %found.key, "plugin lyrics ignored: the user threw them away for this song");
    }
    // Own text without timestamps asked for this alignment, even where LRCLIB has synced lyrics.
    if let Some(Lookup::Hit(c)) = state.with_db(|db| db.get(&found.key, now))
        && c.format == Format::Lrc
        && !c.source.starts_with("plugin:")
        && own_file(app, &found.key).is_none()
    {
        return tracing::info!(plugin = plugin.0, key = %found.key, "plugin lyrics ignored: the song has synced lyrics");
    }
    let raw = words::to_lrc(&found.lines);
    let cached = Cached {
        source: format!("plugin:{}:{}", plugin.0, found.how),
        source_id: found.lrclib_id,
        format: Format::Lrc,
        raw: raw.clone(),
        instrumental: false,
        fetched_at: now,
    };
    if state.with_db(|db| db.put(&found.key, &cached)).is_none() {
        return;
    }
    let parsed = crate::lyrics::parser::parse(&raw).lines;
    let times = words::valid_lines(&parsed, &words::found_words(&parsed, &found.lines));
    if !times.is_empty() {
        let json = serde_json::to_string(&times).unwrap_or_default();
        state.with_db(|db| db.set_plugin_words(&found.key, &words::lyrics_hash(&parsed), plugin, &json, now));
    }
    tracing::info!(plugin = plugin.0, key = %found.key, how = %found.how, lines = found.lines.len(), timed = times.len(), "lyrics from a plugin");
    if state.current_key().as_deref() == Some(found.key.as_str()) {
        on_lyrics(app, crate::lyrics::service::to_event(&found.key, &cached, "cache"));
    }
}

/// No plugin may bring lyrics for this song: the user threw a plugin's lyrics away, or picked
/// lyrics (or none) themselves.
pub fn plugin_lyrics_rejected(app: &AppHandle, key: &str) -> bool {
    app.state::<NowPlayingState>()
        .with_db(|db| Ok(db.plugin_lyrics_rejected(key)? || db.lyrics_choice(key)?.is_some()))
        .unwrap_or(false)
}

/// An entry the user can pick for the current song (Settings → Sync, IDEEN.md, package B).
#[derive(Serialize)]
pub struct LyricsCandidate {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_s: f64,
    pub synced: bool,
    pub plain: bool,
    pub instrumental: bool,
}

/// Two fields, as JSON would turn both "no lyrics" and "automatic" of a nested option into null.
#[derive(Serialize)]
pub struct LyricsCandidates {
    /// The picked entry, if the user picked one.
    pub chosen_id: Option<i64>,
    /// The user wants no lyrics for this song.
    pub chose_none: bool,
    pub items: Vec<LyricsCandidate>,
}

/// LRCLIB's entries for the current song, for the user to pick from. Blocks on the network.
pub fn lyrics_candidates(app: &AppHandle) -> Result<LyricsCandidates, String> {
    let state = app.state::<NowPlayingState>();
    let t = state.snapshot().track.ok_or("nothing plays")?;
    let transport = state.transport.get().ok_or("LRCLIB client not ready")?.clone();
    let found = crate::lyrics::lrclib::candidates(transport.as_ref(), &t.title, &t.artist, t.duration_ms.map(|d| f64::from(d) / 1000.0))?;
    tracing::info!(key = %t.key, candidates = found.len(), "lyrics candidates listed");
    let choice = state.with_db(|db| db.lyrics_choice(&t.key)).flatten();
    Ok(LyricsCandidates {
        chosen_id: choice.flatten(),
        chose_none: choice == Some(None),
        items: found
            .into_iter()
            .map(|c| LyricsCandidate {
                synced: c.has_synced(),
                plain: c.plain_lyrics.as_deref().is_some_and(|p| !p.trim().is_empty()),
                id: c.id,
                title: c.track_name,
                artist: c.artist_name,
                album: c.album_name,
                duration_s: c.duration,
                instrumental: c.instrumental,
            })
            .collect(),
    })
}

/// Longest own lyrics; a whole song in LRC is a few KB.
const OWN_LYRICS_MAX_BYTES: usize = 256 * 1024;

/// The user's own lyrics for the current song: the text of the local file that fits it.
pub fn own_lyrics(app: &AppHandle) -> Option<String> {
    let t = app.state::<NowPlayingState>().snapshot().track?;
    crate::lyrics::local::find(&crate::lyrics::local::dir(), &t.title, &t.artist).map(|f| f.1)
}

/// Saves own lyrics for the current song as a local LRC file (the one that fits it already, or
/// "Artist - Title.lrc") and shows them. With timestamps they run synced; without, Recognize can
/// align them.
pub fn save_own_lyrics(app: &AppHandle, text: &str) -> Result<(), String> {
    let t = app.state::<NowPlayingState>().snapshot().track.ok_or("nothing plays")?;
    if text.trim().is_empty() || text.len() > OWN_LYRICS_MAX_BYTES {
        return Err("lyrics empty or too long".into());
    }
    let dir = crate::lyrics::local::dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = crate::lyrics::local::find(&dir, &t.title, &t.artist)
        .map(|f| f.0)
        .unwrap_or_else(|| dir.join(crate::lyrics::local::file_name(&t.title, &t.artist)));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    let synced = crate::lyrics::parser::parse(text).synced;
    tracing::info!(key = %t.key, synced, lines = text.lines().count(), "own lyrics saved");
    request_lyrics(app, &t, false);
    Ok(())
}

/// Removes the own lyrics file of the current song; LRCLIB's lyrics show again.
pub fn delete_own_lyrics(app: &AppHandle) -> Result<(), String> {
    let t = app.state::<NowPlayingState>().snapshot().track.ok_or("nothing plays")?;
    let (path, _) = crate::lyrics::local::find(&crate::lyrics::local::dir(), &t.title, &t.artist).ok_or("no own lyrics")?;
    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    tracing::info!(key = %t.key, "own lyrics removed");
    request_lyrics(app, &t, false);
    Ok(())
}

/// The user's pick for the current song: `Some(Some(id))` a LRCLIB entry, `Some(None)` no
/// lyrics, `None` back to the automatic search. Kept per song; plugins bring no lyrics for it.
pub fn choose_lyrics(app: &AppHandle, choice: Option<Option<i64>>) {
    let state = app.state::<NowPlayingState>();
    let Some(t) = state.snapshot().track else { return };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    match choice {
        Some(id) => state.with_db(|db| db.set_lyrics_choice(&t.key, id, now)),
        None => state.with_db(|db| db.clear_lyrics_choice(&t.key)),
    };
    tracing::info!(key = %t.key, choice = ?choice, "lyrics choice set");
    request_lyrics(app, &t, true);
}

/// Throws away the plugin's lyrics of the current song for good (PLAN-plugins-2.md 4.4) and asks
/// LRCLIB again; no plugin is asked for this song any more.
pub fn discard_plugin_lyrics(app: &AppHandle) {
    let state = app.state::<NowPlayingState>();
    let (Some(t), Some(lyrics)) = (state.snapshot().track, state.lyrics()) else { return };
    if lyrics.key != t.key || !lyrics.source.contains("plugin:") {
        return;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    state.with_db(|db| db.reject_plugin_lyrics(&t.key, now));
    tracing::info!(key = %t.key, source = %lyrics.source, "plugin lyrics thrown away");
    request_lyrics(app, &t, true);
}

/// A removed plugin's word times and lyrics go; the song that plays falls back to the estimate
/// or to LRCLIB at once.
pub fn forget_plugin_results(app: &AppHandle, plugin_id: &str) {
    let state = app.state::<NowPlayingState>();
    let songs = state.with_db(|db| db.forget_plugin_words(plugin_id)).unwrap_or(0);
    let lyrics = state.with_db(|db| db.forget_plugin_lyrics(plugin_id)).unwrap_or(0);
    tracing::info!(plugin = plugin_id, songs, lyrics, "plugin word times and lyrics removed");
    if let (Some(t), Some(l)) = (state.snapshot().track, state.lyrics())
        && l.key == t.key
        && l.source.contains(&format!("plugin:{plugin_id}:"))
    {
        return request_lyrics(app, &t, true);
    }
    if state.lyrics().is_some_and(|l| l.word_source == format!("plugin:{plugin_id}")) {
        refresh_lyrics(app);
    }
}

/// Lines of the lyrics a plugin answers for: the current song, or the cache (a plugin may answer
/// after the song is over).
pub fn lyrics_lines_for(app: &AppHandle, key: &str) -> Option<Vec<crate::lyrics::types::Line>> {
    let state = app.state::<NowPlayingState>();
    if let Some(current) = state.lyrics().filter(|l| l.key == key) {
        return Some(current.lines);
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    match state.with_db(|db| db.get(key, now))? {
        crate::lyrics::cache::Lookup::Hit(c) => Some(crate::lyrics::service::to_event(key, &c, "cache").lines),
        _ => None,
    }
}

/// Stores word times from a plugin and, for the song that plays, shows them at once.
pub fn store_plugin_words(app: &AppHandle, key: &str, hash: &str, plugin: (&str, &str), json: &str) {
    let state = app.state::<NowPlayingState>();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    state.with_db(|db| db.set_plugin_words(key, hash, plugin, json, now));
    let Some(mut lyrics) = state.lyrics().filter(|l| l.key == key) else { return };
    add_plugin_words(app, &mut lyrics);
    {
        let mut inner = state.inner.lock().unwrap();
        if inner.lyrics.as_ref().map(|l| l.key.as_str()) != Some(key) {
            return;
        }
        inner.lyrics = Some(lyrics.clone());
    }
    emit(app, events::LYRICS, &lyrics);
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
                if let (false, Some(old), Some(next)) = (new_song, old_key.as_deref(), &info)
                    && old != next.key
                {
                    rekeyed_offset = Some((old.to_owned(), next.key.clone()));
                }
                let mut rekeyed = None;
                if let (false, Some(next), Some(lyrics)) = (new_song, &info, inner.lyrics.as_mut())
                    && old_key.as_deref() == Some(lyrics.key.as_str())
                    && lyrics.key != next.key
                {
                    lyrics.key = next.key.clone();
                    rekeyed = Some(lyrics.clone());
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
            let key_changed = rekeyed_offset.is_some();
            if let Some((old, new)) = rekeyed_offset {
                rekey_song_offset(app, &old, &new);
            }
            if new_song || key_changed || info.is_none() {
                crate::plugins::on_track(app, info.as_ref());
            }
            if new_song {
                sync_lab::record(app, Instant::now(), sync_lab::entry("track", "smtc", 0.0));
                state.clock.lock().unwrap().on_track_change();
                if let Some(t) = &info {
                    apply_song_offset(app, &t.key);
                    // Ads, podcast episodes and browser videos have no lyrics to look for
                    // (PLAN.md 5.5, 13.4).
                    match t.kind {
                        TrackKind::Ad | TrackKind::Podcast | TrackKind::Video => {
                            tracing::info!(kind = ?t.kind, "no lyrics lookup for this kind")
                        }
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
                sync_lab::record(
                    app,
                    now,
                    sync_lab::LabEntry { update: "status", ..sync_lab::entry("status", "smtc", pos) },
                );
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
                let residual =
                    (obs.playing && clock.anchor().playing).then(|| obs.pos_ms - clock.position(obs.valid_at));
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
                let event = clock_event(app, update_kind(&update));
                if matches!(update, Update::Seek { .. }) {
                    *state.pending_seek.lock().unwrap() = Some((event.seq, obs.valid_at, Instant::now()));
                }
                emit(app, events::CLOCK, &event);
            }
        }
    }
}

/// The overlay drew the anchor `seq`. For a seek this logs how long it took from Spotify's
/// report to the corrected display (PLAN.md 2: within 300 ms of the SMTC event).
pub fn clock_applied(app: &AppHandle, seq: u64) {
    let state = app.state::<NowPlayingState>();
    let pending = state.pending_seek.lock().unwrap().take_if(|p| p.0 == seq);
    if let Some((_, valid_at, emitted)) = pending {
        tracing::info!(
            seq,
            since_report_ms = ms(valid_at.elapsed()),
            since_anchor_ms = ms(emitted.elapsed()),
            "seek shown in overlay"
        );
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
