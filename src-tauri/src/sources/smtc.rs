//! Event-driven SMTC source for Spotify (PLAN.md 4.2, 5.4; measurements in docs/spike-smtc.md).
//!
//! One worker thread owns every WinRT object. Event handlers run on arbitrary WinRT threads
//! and only post a message to the worker; the worker does all reads, so blocking `join()`
//! calls never run on the UI thread. Besides the events, the worker polls status and timeline
//! every 250 ms and the media properties once a second as a safety net.
//!
//! Track-change order (spike, observation 4): Spotify publishes the new timeline (position 0)
//! up to 250 ms before the new title. A drop to the start of a song therefore switches the
//! worker to fast polling of the media properties for 400 ms, so a new title shows up as soon
//! as Spotify publishes it, even if `MediaPropertiesChanged` arrives late.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as WinStatus,
};
use windows::Media::MediaPlaybackType;
use windows::Storage::Streams::{Buffer, DataReader, InputStreamOptions};

use super::{
    ArtRef, Observation, PlaybackSource, PlaybackStatus, SourceEvent, SourceId, TrackInfo, TrackKind, classify,
    track_key,
};

const POLL: Duration = Duration::from_millis(250);
const FAST_POLL: Duration = Duration::from_millis(40);
/// Fast polling after the position fell to the start; the spike saw the title ≤ 250 ms later.
const FAST_POLL_WINDOW: Duration = Duration::from_millis(400);
const PROPS_POLL: Duration = Duration::from_secs(1);
const RETRY: Duration = Duration::from_secs(5);
/// Re-read a missing cover on polls this often per song; events always re-read it.
const ART_POLL_ATTEMPTS: u32 = 3;
const MAX_ART_BYTES: u64 = 4 * 1024 * 1024;
/// A position below this that lies far below the prediction is a jump back to the start.
const RESTART_MAX_POS_MS: f64 = 1000.0;
const RESTART_MIN_DROP_MS: f64 = 1500.0;
const SMTC_CONFIDENCE: f32 = 0.8;
/// 100 ns ticks between 1601-01-01 (WinRT DateTime epoch) and 1970-01-01.
const EPOCH_DIFF_TICKS: i64 = 116_444_736_000_000_000;

type WinResult<T> = windows::core::Result<T>;

/// Set after the PC woke up: WinRT event subscriptions may be dead, so the worker drops the
/// session manager and connects again.
static RECONNECT: AtomicBool = AtomicBool::new(false);

pub fn request_reconnect() {
    RECONNECT.store(true, Ordering::SeqCst);
}

pub struct SmtcSource {
    filter: String,
}

impl SmtcSource {
    /// `filter` is matched against the session's AppUserModelId: "Spotify" covers the Store
    /// build (`SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`) and the desktop build (`Spotify.exe`).
    pub fn new(filter: impl Into<String>) -> Self {
        Self { filter: filter.into() }
    }
}

impl PlaybackSource for SmtcSource {
    fn spawn(self) -> Receiver<SourceEvent> {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("smtc".into())
            .spawn(move || Worker::new(self.filter, tx).run())
            .expect("failed to spawn smtc thread");
        rx
    }
}

/// Messages from WinRT event handlers to the worker.
enum Msg {
    Sessions,
    /// Carries the moment the event fired, for latency logging.
    Media(Instant),
    Playback,
    Timeline,
}

struct Manager {
    inner: SessionManager,
    token: i64,
}

impl Drop for Manager {
    fn drop(&mut self) {
        let _ = self.inner.RemoveSessionsChanged(self.token);
    }
}

struct Bound {
    session: Session,
    tokens: [i64; 3],
}

impl Drop for Bound {
    fn drop(&mut self) {
        let _ = self.session.RemoveMediaPropertiesChanged(self.tokens[0]);
        let _ = self.session.RemovePlaybackInfoChanged(self.tokens[1]);
        let _ = self.session.RemoveTimelinePropertiesChanged(self.tokens[2]);
    }
}

#[derive(Clone, Copy, Debug)]
struct TimelineSample {
    pos_ms: f64,
    /// Spotify's `LastUpdatedTime` in WinRT ticks.
    updated: i64,
    end_ms: i64,
    valid_at: Instant,
    playing: bool,
}

impl TimelineSample {
    /// True if a new report at `pos_ms` means "back at the start of a song": a track change
    /// whose title is still pending, or a seek to 0 (docs/spike-smtc.md, Folgen).
    fn is_restart(&self, pos_ms: f64, valid_at: Instant) -> bool {
        let elapsed =
            if self.playing { valid_at.saturating_duration_since(self.valid_at).as_secs_f64() * 1000.0 } else { 0.0 };
        let predicted = self.pos_ms + elapsed;
        pos_ms < RESTART_MAX_POS_MS && predicted - pos_ms > RESTART_MIN_DROP_MS
    }
}

struct Worker {
    filter: String,
    out: Sender<SourceEvent>,
    msg_tx: Sender<Msg>,
    msg_rx: Receiver<Msg>,
    manager: Option<Manager>,
    bound: Option<Bound>,
    last_app_id: Option<String>,
    track: Option<TrackInfo>,
    art_attempts: u32,
    status: Option<PlaybackStatus>,
    timeline: Option<TimelineSample>,
    /// Set when the position fell to the start of a song; cleared by the next title or after
    /// `FAST_POLL_WINDOW`.
    restart_at: Option<Instant>,
    next_props_poll: Instant,
}

impl Worker {
    fn new(filter: String, out: Sender<SourceEvent>) -> Self {
        let (msg_tx, msg_rx) = mpsc::channel();
        Self {
            filter,
            out,
            msg_tx,
            msg_rx,
            manager: None,
            bound: None,
            last_app_id: None,
            track: None,
            art_attempts: 0,
            status: None,
            timeline: None,
            restart_at: None,
            next_props_poll: Instant::now(),
        }
    }

    fn run(mut self) {
        tracing::info!(filter = %self.filter, "smtc source started");
        loop {
            if RECONNECT.swap(false, Ordering::SeqCst) && self.manager.is_some() {
                tracing::info!("smtc reconnecting");
                self.bound = None;
                self.manager = None;
            }
            if self.manager.is_none()
                && let Err(e) = self.connect()
            {
                tracing::warn!(error = %e, "smtc session manager unavailable, retrying in 5 s");
                self.manager = None;
                std::thread::sleep(RETRY);
                continue;
            }
            let timeout = if self.restart_at.is_some() { FAST_POLL } else { POLL };
            let result = match self.msg_rx.recv_timeout(timeout) {
                Ok(Msg::Sessions) => self.rebind(),
                Ok(Msg::Media(at)) => self.read_media(Some(at)),
                Ok(Msg::Playback) => self.read_playback(),
                Ok(Msg::Timeline) => self.read_timeline(),
                Err(RecvTimeoutError::Timeout) => self.poll(),
                // Cannot happen while the worker holds `msg_tx`.
                Err(RecvTimeoutError::Disconnected) => return,
            };
            if let Err(e) = result {
                // The session can vanish between two calls (Spotify quits); rebinding sorts it out.
                tracing::debug!(error = %e, "smtc read failed, rebinding");
                if let Err(e) = self.rebind() {
                    tracing::warn!(error = %e, "smtc rebind failed, reconnecting");
                    self.bound = None;
                    self.manager = None;
                }
            }
        }
    }

    fn emit(&self, event: SourceEvent) {
        // The receiver lives as long as the app; a send error only happens at shutdown.
        let _ = self.out.send(event);
    }

    fn connect(&mut self) -> WinResult<()> {
        let inner = SessionManager::RequestAsync()?.join()?;
        let tx = self.msg_tx.clone();
        let token = inner.SessionsChanged(&TypedEventHandler::new(move |_, _| {
            let _ = tx.send(Msg::Sessions);
            Ok(())
        }))?;
        self.manager = Some(Manager { inner, token });
        self.rebind()
    }

    /// Finds the player session again. Runs on every `SessionsChanged`, because a restarted
    /// Spotify gets a new session object under the same AppUserModelId.
    fn rebind(&mut self) -> WinResult<()> {
        let Some(manager) = &self.manager else { return Ok(()) };
        let mut found = None;
        for session in manager.inner.GetSessions()? {
            let app_id = session.SourceAppUserModelId()?.to_string();
            if app_id.contains(&self.filter) {
                found = Some((session, app_id));
                break;
            }
        }
        self.bound = None; // removes the old handlers

        let Some((session, app_id)) = found else {
            if self.last_app_id.take().is_some() || self.track.is_some() {
                tracing::info!("player session gone");
            }
            self.timeline = None;
            self.restart_at = None;
            if self.track.take().is_some() {
                self.emit(SourceEvent::Track { info: None, first_sign: Instant::now() });
            }
            if self.status != Some(PlaybackStatus::Closed) {
                self.status = Some(PlaybackStatus::Closed);
                self.emit(SourceEvent::Status(PlaybackStatus::Closed));
            }
            return Ok(());
        };

        let tokens = {
            let tx = self.msg_tx.clone();
            let media = session.MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                let _ = tx.send(Msg::Media(Instant::now()));
                Ok(())
            }))?;
            let tx = self.msg_tx.clone();
            let playback = session.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                let _ = tx.send(Msg::Playback);
                Ok(())
            }))?;
            let tx = self.msg_tx.clone();
            let timeline = session.TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                let _ = tx.send(Msg::Timeline);
                Ok(())
            }))?;
            [media, playback, timeline]
        };
        if self.last_app_id.as_deref() != Some(app_id.as_str()) {
            tracing::info!(app_id = %app_id, "bound to player session");
            self.last_app_id = Some(app_id);
        }
        self.bound = Some(Bound { session, tokens });
        self.read_playback()?;
        self.read_timeline()?;
        self.read_media(None)
    }

    fn session(&self) -> Option<&Session> {
        self.bound.as_ref().map(|b| &b.session)
    }

    fn poll(&mut self) -> WinResult<()> {
        if self.bound.is_none() {
            return Ok(());
        }
        self.read_playback()?;
        self.read_timeline()?;
        let now = Instant::now();
        if let Some(at) = self.restart_at {
            if now.duration_since(at) > FAST_POLL_WINDOW {
                tracing::debug!("no new title after the jump to the start: seek to 0 or repeat");
                self.restart_at = None;
            } else {
                return self.read_media(None);
            }
        }
        if now >= self.next_props_poll {
            self.read_media(None)?;
        }
        Ok(())
    }

    fn read_playback(&mut self) -> WinResult<()> {
        let Some(session) = self.session() else { return Ok(()) };
        let status = map_status(session.GetPlaybackInfo()?.PlaybackStatus()?);
        if self.status != Some(status) {
            tracing::debug!(?status, "playback status");
            self.status = Some(status);
            self.emit(SourceEvent::Status(status));
        }
        Ok(())
    }

    fn read_timeline(&mut self) -> WinResult<()> {
        let Some(session) = self.session() else { return Ok(()) };
        let tl = session.GetTimelineProperties()?;
        let pos_ms = tl.Position()?.Duration as f64 / 10_000.0;
        let updated = tl.LastUpdatedTime()?.UniversalTime;
        let end_ms = tl.EndTime()?.Duration / 10_000;
        // Only changed reports count as observations, otherwise polling would autocorrelate (PLAN.md 5.4).
        if let Some(prev) = self.timeline
            && prev.updated == updated
            && prev.pos_ms == pos_ms
            && prev.end_ms == end_ms
        {
            return Ok(());
        }

        let now = Instant::now();
        let age_ms = ((now_winrt_ticks() - updated) as f64 / 10_000.0).max(0.0);
        let valid_at = now.checked_sub(Duration::from_secs_f64(age_ms / 1000.0)).unwrap_or(now);
        let playing = self.status == Some(PlaybackStatus::Playing);
        let likely_track_change = self.timeline.is_some_and(|prev| prev.is_restart(pos_ms, valid_at));
        if likely_track_change {
            tracing::debug!(pos_ms, "position fell to the start of a song, expecting a new title");
            self.restart_at.get_or_insert(now);
        }
        tracing::trace!(pos_ms, age_ms, end_ms, playing, "timeline");
        self.timeline = Some(TimelineSample { pos_ms, updated, end_ms, valid_at, playing });
        self.emit(SourceEvent::Observation(Observation {
            source: SourceId::Smtc,
            pos_ms,
            valid_at,
            playing,
            confidence: SMTC_CONFIDENCE,
            likely_track_change,
        }));

        // The duration may settle after the title. While a track change is pending, the new
        // timeline belongs to the next song, not the current one.
        if self.restart_at.is_none()
            && end_ms > 0
            && let Some(track) = &self.track
        {
            let duration = Some(end_ms as u32);
            if track.duration_ms != duration {
                let mut t = track.clone();
                t.duration_ms = duration;
                t.key = track_key(&t.artist, &t.title, duration);
                let music_type = t.kind != TrackKind::Unknown;
                t.kind = classify(&t.title, &t.artist, t.album.as_deref(), duration, music_type);
                tracing::debug!(key = %t.key, "track duration updated");
                self.track = Some(t.clone());
                self.emit(SourceEvent::Track { info: Some(t), first_sign: now });
            }
        }
        Ok(())
    }

    /// Reads title, artist, album and cover. `event_at` is set when a `MediaPropertiesChanged`
    /// event triggered the read.
    fn read_media(&mut self, event_at: Option<Instant>) -> WinResult<()> {
        let Some(session) = self.session() else { return Ok(()) };
        let props = session.TryGetMediaPropertiesAsync()?.join()?;
        self.next_props_poll = Instant::now() + PROPS_POLL;

        let title = props.Title()?.to_string();
        if title.trim().is_empty() {
            // Spotify clears the properties briefly while it loads the next song.
            tracing::debug!("empty title, waiting for metadata");
            return Ok(());
        }
        let artist = props.Artist()?.to_string();
        let album = Some(props.AlbumTitle()?.to_string()).filter(|a| !a.is_empty());
        let duration_ms = self.timeline.map(|t| t.end_ms).filter(|&e| e > 0).map(|e| e as u32);
        let music_type = matches!(props.PlaybackType().and_then(|r| r.Value()), Ok(MediaPlaybackType::Music));
        let kind = classify(&title, &artist, album.as_deref(), duration_ms, music_type);
        let mut info = TrackInfo {
            key: track_key(&artist, &title, duration_ms),
            title,
            artist,
            album,
            duration_ms,
            art: None,
            kind,
            source: SourceId::Smtc,
        };

        if let Some(cur) = self.track.clone().filter(|cur| cur.same_song(&info)) {
            // Same song. Spotify may publish or replace the cover after the title.
            if (event_at.is_some() || (cur.art.is_none() && self.art_attempts < ART_POLL_ATTEMPTS))
                && let Some(art) = self.read_art(&props)
                && cur.art.as_ref().map(|a| a.hash) != Some(art.hash)
            {
                tracing::debug!(bytes = art.url.len(), "cover updated");
                let mut t = cur;
                t.art = Some(art);
                self.track = Some(t.clone());
                self.emit(SourceEvent::Track { info: Some(t), first_sign: Instant::now() });
            }
            return Ok(());
        }

        let now = Instant::now();
        let restart = self.restart_at.take().filter(|at| now.duration_since(*at) <= FAST_POLL_WINDOW * 3);
        let first_sign = [restart, event_at].into_iter().flatten().min().unwrap_or(now);
        tracing::info!(
            title = %info.title,
            artist = %info.artist,
            album = info.album.as_deref().unwrap_or(""),
            duration_ms = info.duration_ms,
            via = if event_at.is_some() { "event" } else { "poll" },
            since_first_sign_ms = now.duration_since(first_sign).as_millis() as u64,
            "track changed"
        );
        self.art_attempts = 0;
        self.track = Some(info.clone());
        self.emit(SourceEvent::Track { info: Some(info.clone()), first_sign });

        // Text first, cover second: the title must not wait for the image.
        if let Some(art) = self.read_art(&props) {
            info.art = Some(art);
            self.track = Some(info.clone());
            self.emit(SourceEvent::Track { info: Some(info), first_sign });
        }
        Ok(())
    }

    fn read_art(&mut self, props: &MediaProperties) -> Option<ArtRef> {
        self.art_attempts += 1;
        let started = Instant::now();
        match read_thumbnail(props) {
            Ok(Some((mime, bytes))) => {
                let mut hasher = DefaultHasher::new();
                bytes.hash(&mut hasher);
                let url = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes));
                tracing::debug!(
                    mime,
                    bytes = bytes.len(),
                    took_ms = started.elapsed().as_millis() as u64,
                    "cover read"
                );
                Some(ArtRef { url, hash: hasher.finish() })
            }
            Ok(None) => None,
            Err(e) => {
                tracing::debug!(error = %e, "cover not readable");
                None
            }
        }
    }
}

fn read_thumbnail(props: &MediaProperties) -> WinResult<Option<(String, Vec<u8>)>> {
    // A song without cover has no thumbnail reference; windows-rs reports that as an error.
    let Ok(reference) = props.Thumbnail() else { return Ok(None) };
    let stream = reference.OpenReadAsync()?.join()?;
    let size = stream.Size()?;
    if size == 0 || size > MAX_ART_BYTES {
        return Ok(None);
    }
    let buffer = Buffer::Create(size as u32)?;
    let filled = stream.ReadAsync(&buffer, size as u32, InputStreamOptions::None)?.join()?;
    let mut bytes = vec![0u8; filled.Length()? as usize];
    DataReader::FromBuffer(&filled)?.ReadBytes(&mut bytes)?;
    let declared = stream.ContentType().map(|s| s.to_string()).unwrap_or_default();
    Ok(Some((sniff_mime(&bytes, &declared).to_string(), bytes)))
}

/// Image type from the magic bytes; Spotify's declared content type is not always set.
fn sniff_mime<'a>(bytes: &[u8], declared: &'a str) -> &'a str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'B', b'M', ..] => "image/bmp",
        _ if declared.starts_with("image/") => declared,
        _ => "image/jpeg",
    }
}

fn map_status(s: WinStatus) -> PlaybackStatus {
    match s {
        WinStatus::Playing => PlaybackStatus::Playing,
        WinStatus::Paused => PlaybackStatus::Paused,
        WinStatus::Stopped => PlaybackStatus::Stopped,
        WinStatus::Opened => PlaybackStatus::Opened,
        WinStatus::Closed => PlaybackStatus::Closed,
        _ => PlaybackStatus::Changing,
    }
}

fn now_winrt_ticks() -> i64 {
    let since_unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    EPOCH_DIFF_TICKS + (since_unix.as_nanos() / 100) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pos_ms: f64, playing: bool, valid_at: Instant) -> TimelineSample {
        TimelineSample { pos_ms, updated: 0, end_ms: 200_000, valid_at, playing }
    }

    #[test]
    fn drop_to_start_is_a_restart() {
        let t0 = Instant::now();
        let prev = sample(185_000.0, true, t0);
        assert!(prev.is_restart(0.0, t0 + Duration::from_millis(300)));
        assert!(prev.is_restart(800.0, t0 + Duration::from_millis(300)));
    }

    #[test]
    fn small_backward_correction_after_seek_is_no_restart() {
        // Spike: the second update after a seek corrects back by 100-216 ms.
        let t0 = Instant::now();
        let prev = sample(900.0, true, t0);
        assert!(!prev.is_restart(700.0, t0 + Duration::from_millis(150)));
    }

    #[test]
    fn normal_progress_and_mid_song_seek_are_no_restart() {
        let t0 = Instant::now();
        let prev = sample(10_000.0, true, t0);
        assert!(!prev.is_restart(14_500.0, t0 + Duration::from_millis(4500)));
        assert!(!prev.is_restart(60_000.0, t0 + Duration::from_millis(100)));
        // Seek back to 0:30 is far from the start of the song.
        assert!(!prev.is_restart(30_000.0, t0 + Duration::from_millis(100)));
    }

    #[test]
    fn restart_while_paused_uses_the_frozen_position() {
        let t0 = Instant::now();
        let prev = sample(5_000.0, false, t0);
        assert!(prev.is_restart(0.0, t0 + Duration::from_secs(60)));
        let prev = sample(1_200.0, false, t0);
        assert!(!prev.is_restart(0.0, t0 + Duration::from_secs(60)));
    }

    #[test]
    fn mime_sniffing() {
        assert_eq!(sniff_mime(&[0x89, b'P', b'N', b'G', 0x0D], ""), "image/png");
        assert_eq!(sniff_mime(&[0xFF, 0xD8, 0xFF, 0xE0], "image/png"), "image/jpeg");
        assert_eq!(sniff_mime(&[0, 1, 2], "image/x-icon"), "image/x-icon");
        assert_eq!(sniff_mime(&[0, 1, 2], ""), "image/jpeg");
    }
}
