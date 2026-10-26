//! Lyrics worker (PLAN.md 7.1): one thread that answers "lyrics for this song" from the cache or
//! from LRCLIB, and retries in the background while LRCLIB is busy. Only the newest request
//! counts; results go out through a callback, so the worker knows nothing about Tauri. Logs
//! carry line counts, never lyrics text.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::cache::{Cache, Cached, Format, Lookup};
use super::lrclib::{self, LrclibTrack, Outcome, Query, Transport};
use super::parser;
use super::types::{LyricsEvent, LyricsState};

/// Waits before retrying while LRCLIB is busy or unreachable.
const RETRY_DELAYS: [Duration; 3] = [Duration::from_secs(5), Duration::from_secs(30), Duration::from_secs(120)];
/// How long a song that stayed unavailable through all retries counts as missing.
const GIVE_UP_TTL_S: i64 = 24 * 3600;

#[derive(Clone, Debug)]
pub struct Request {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    /// Skip the cache and ask LRCLIB again ("Lyrics neu laden").
    pub force: bool,
}

/// Lyrics of songs not played for a year leave the cache (LRCLIB has them if they come back).
const CACHE_MAX_AGE_S: i64 = 365 * 86_400;

pub struct LyricsService {
    tx: Sender<Request>,
}

impl LyricsService {
    pub fn spawn(
        cache_path: PathBuf,
        transport: Arc<dyn Transport>,
        emit: impl Fn(LyricsEvent) + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        std::thread::Builder::new()
            .name("lyrics".into())
            .spawn(move || {
                let cache = match Cache::open(&cache_path) {
                    Ok(c) => {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                        match c.prune(now, CACHE_MAX_AGE_S) {
                            Ok(n) => tracing::info!(removed = n, "lyrics cache pruned"),
                            Err(e) => tracing::warn!(error = %e, "lyrics cache prune failed"),
                        }
                        Some(c)
                    }
                    Err(e) => {
                        tracing::error!(path = %cache_path.display(), error = %e, "lyrics cache unavailable, working without");
                        None
                    }
                };
                let mut current: Option<Request> = None;
                let mut attempt = 0usize;
                let mut retry_at: Option<Instant> = None;
                loop {
                    let next = match retry_at {
                        Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
                        None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    match next {
                        Ok(mut req) => {
                            // Songs skipped quickly: only the newest request matters.
                            while let Ok(newer) = rx.try_recv() {
                                req = newer;
                            }
                            attempt = 0;
                            current = Some(req);
                        }
                        Err(RecvTimeoutError::Timeout) => attempt += 1,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                    retry_at = None;
                    let Some(req) = current.as_ref() else { continue };
                    let done = handle(req, attempt, cache.as_ref(), &transport, &emit);
                    if !done {
                        match RETRY_DELAYS.get(attempt) {
                            Some(delay) => {
                                tracing::info!(key = %req.key, retry_in_s = delay.as_secs(), "LRCLIB unavailable, retrying later");
                                retry_at = Some(Instant::now() + *delay);
                            }
                            None => {
                                // LRCLIB answers 503 for empty searches too, so "busy every time"
                                // usually means "not there". Remember it for a day, not a week.
                                tracing::warn!(key = %req.key, "LRCLIB unavailable, giving up for this song today");
                                if let Some(cache) = cache.as_ref() {
                                    let _ = cache.put_negative_for(&req.key, unix_now(), GIVE_UP_TTL_S);
                                }
                                emit(LyricsEvent::state(&req.key, LyricsState::Missing));
                            }
                        }
                    }
                }
            })
            .expect("failed to spawn lyrics thread");
        Self { tx }
    }

    pub fn request(&self, req: Request) {
        let _ = self.tx.send(req);
    }
}

/// Handles one request. Returns false if LRCLIB was unavailable and a retry makes sense.
fn handle(
    req: &Request,
    attempt: usize,
    cache: Option<&Cache>,
    transport: &Arc<dyn Transport>,
    emit: &dyn Fn(LyricsEvent),
) -> bool {
    let now = unix_now();
    if let Some(cache) = cache {
        let _ = cache.touch_track(&req.key, &req.title, &req.artist, req.album.as_deref(), req.duration_ms, now);
        if req.force {
            let _ = cache.forget(&req.key);
        } else {
            match cache.get(&req.key, now) {
                Ok(Lookup::Hit(c)) => {
                    let event = to_event(&req.key, &c, "cache");
                    tracing::info!(key = %req.key, state = ?event.state, lines = event.lines.len(), "lyrics from cache");
                    emit(event);
                    return true;
                }
                Ok(Lookup::Negative) => {
                    tracing::info!(key = %req.key, "no lyrics (cached miss)");
                    emit(LyricsEvent::state(&req.key, LyricsState::Missing));
                    return true;
                }
                Ok(Lookup::Miss) => {}
                Err(e) => tracing::warn!(error = %e, "lyrics cache read failed"),
            }
        }
    }
    if attempt == 0 {
        emit(LyricsEvent::state(&req.key, LyricsState::Loading));
    }

    let started = Instant::now();
    let query = Query {
        title: &req.title,
        artist: &req.artist,
        album: req.album.as_deref(),
        duration_s: req.duration_ms.map(|d| d as f64 / 1000.0),
    };
    let outcome = lrclib::lookup(transport, &query);
    let took_ms = started.elapsed().as_millis() as u64;
    match outcome {
        Outcome::Found { track, stage } => {
            let cached = to_cached(&track, now);
            if let Some(cache) = cache
                && let Err(e) = cache.put(&req.key, &cached)
            {
                tracing::warn!(error = %e, "lyrics cache write failed");
            }
            let event = to_event(&req.key, &cached, "lrclib");
            tracing::info!(key = %req.key, stage, lrclib_id = track.id, state = ?event.state, lines = event.lines.len(), took_ms, "lyrics found");
            emit(event);
            true
        }
        Outcome::NotFound => {
            if let Some(cache) = cache {
                let _ = cache.put_negative(&req.key, now);
            }
            tracing::info!(key = %req.key, took_ms, "no lyrics on LRCLIB");
            emit(LyricsEvent::state(&req.key, LyricsState::Missing));
            true
        }
        Outcome::Unavailable(e) => {
            tracing::info!(key = %req.key, error = %e, took_ms, attempt, "LRCLIB unavailable");
            emit(LyricsEvent::state(&req.key, LyricsState::Error));
            false
        }
    }
}

fn to_cached(track: &LrclibTrack, now: i64) -> Cached {
    let (format, raw) = if track.has_synced() {
        (Format::Lrc, track.synced_lyrics.clone().unwrap_or_default())
    } else if let Some(plain) = track.plain_lyrics.clone().filter(|p| !p.trim().is_empty()) {
        (Format::Plain, plain)
    } else {
        (Format::None, String::new())
    };
    Cached {
        source: "lrclib".into(),
        source_id: Some(track.id),
        format,
        raw,
        instrumental: track.instrumental,
        fetched_at: now,
    }
}

fn to_event(key: &str, c: &Cached, via: &str) -> LyricsEvent {
    let source = if via == "cache" { format!("cache:{}", c.source) } else { c.source.clone() };
    let state = match c.format {
        Format::Lrc => {
            let parsed = parser::parse(&c.raw);
            if parsed.synced && parsed.lines.iter().any(|l| !l.text.is_empty()) {
                return LyricsEvent {
                    key: key.to_owned(),
                    state: LyricsState::Synced,
                    source,
                    lines: parsed.lines,
                    lrc_offset_ms: parsed.offset_ms,
                };
            }
            LyricsState::Unsynced
        }
        Format::Plain => LyricsState::Unsynced,
        Format::None if c.instrumental => LyricsState::Instrumental,
        Format::None => LyricsState::Missing,
    };
    LyricsEvent { key: key.to_owned(), state, source, lines: Vec::new(), lrc_offset_ms: 0 }
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached(format: Format, raw: &str, instrumental: bool) -> Cached {
        Cached { source: "lrclib".into(), source_id: Some(1), format, raw: raw.into(), instrumental, fetched_at: 0 }
    }

    #[test]
    fn events_from_cached_entries() {
        let e = to_event("k", &cached(Format::Lrc, "[00:01.00]a\n[00:02.00]b", false), "cache");
        assert_eq!((e.state, e.lines.len(), e.source.as_str()), (LyricsState::Synced, 2, "cache:lrclib"));
        assert_eq!(to_event("k", &cached(Format::Plain, "a\nb", false), "lrclib").state, LyricsState::Unsynced);
        assert_eq!(to_event("k", &cached(Format::None, "", true), "lrclib").state, LyricsState::Instrumental);
        // LRC that holds only instrumental markers is no use for karaoke.
        assert_eq!(
            to_event("k", &cached(Format::Lrc, "[00:01.00]\n[00:02.00]", false), "lrclib").state,
            LyricsState::Unsynced
        );
    }
}
