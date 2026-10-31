//! Lyrics worker (PLAN.md 7.1): one thread that answers "lyrics for this song" from a local LRC
//! file, the user's pick, the cache or LRCLIB, and retries in the background while LRCLIB is
//! busy. Only the newest request counts; results go out through a callback, so the worker knows
//! nothing about Tauri. Logs carry line counts, never lyrics text.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::cache::{Cache, Cached, Format, Lookup};
use super::lrclib::{self, LrclibTrack, Outcome, Query, Transport};
use super::types::{LyricsEvent, LyricsState};
use super::{local, matcher, parser};

/// Sources besides "lrclib" and "plugin:<id>:<how>" (IDEEN.md, package B): the user's pick from
/// LRCLIB, the user's "no lyrics for this song", and the stretched lyrics of the original of a
/// sped-up or slowed version.
pub const CHOSEN: &str = "lrclib:chosen";
pub const CHOSE_NONE: &str = "choice:none";
pub const STRETCHED: &str = "lrclib:stretched";

/// Waits before retrying while LRCLIB is busy or unreachable.
const RETRY_DELAYS: [Duration; 3] = [Duration::from_secs(5), Duration::from_secs(30), Duration::from_secs(120)];
/// How long a song that stayed unavailable through all retries counts as missing. Short: in Q0
/// a third of the misses were only LRCLIB overload (docs/spike-recognize.md, M4).
const GIVE_UP_TTL_S: i64 = 2 * 3600;

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
/// Lyrics a plugin found or recognized, and lyrics without timestamps, may have a synced version
/// on LRCLIB by now. Once they are this old, they are still shown from the cache at once, and
/// LRCLIB is asked again right after (IDEEN.md, package A).
const RECHECK_AFTER_S: i64 = 14 * 86_400;

pub struct LyricsService {
    tx: Sender<Request>,
}

impl LyricsService {
    pub fn spawn(
        cache_path: PathBuf,
        local_dir: PathBuf,
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
                    let done = handle(req, attempt, cache.as_ref(), Some(&local_dir), &transport, &emit);
                    if !done {
                        match RETRY_DELAYS.get(attempt) {
                            Some(delay) => {
                                tracing::info!(key = %req.key, retry_in_s = delay.as_secs(), "LRCLIB unavailable, retrying later");
                                retry_at = Some(Instant::now() + *delay);
                            }
                            None => {
                                // LRCLIB answers 503 for empty searches too, so "busy every time"
                                // often means "not there", but just as often overload. Remember it
                                // for two hours, not a week.
                                tracing::warn!(key = %req.key, "LRCLIB unavailable, giving up for this song for two hours");
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
    local_dir: Option<&Path>,
    transport: &Arc<dyn Transport>,
    emit: &dyn Fn(LyricsEvent),
) -> bool {
    let now = unix_now();
    if let Some(cache) = cache {
        let _ = cache.touch_track(&req.key, &req.title, &req.artist, req.album.as_deref(), req.duration_ms, now);
    }
    // The user's own file wins, also on "reload": it may just have been edited.
    if let Some((path, text)) = local_dir.and_then(|dir| local::find(dir, &req.title, &req.artist)) {
        let own = local::cached(text, now);
        // Text without timestamps that a plugin aligned since the file last changed shows aligned.
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok());
        let aligned = match (own.format, cache.map(|c| c.get(&req.key, now))) {
            (Format::Plain, Some(Ok(Lookup::Hit(c))))
                if c.source.starts_with("plugin:")
                    && c.source.ends_with(":aligned_plain")
                    && modified.is_some_and(|m| c.fetched_at >= m.as_secs() as i64) =>
            {
                Some(c)
            }
            _ => None,
        };
        let event = match &aligned {
            Some(c) => to_event(&req.key, c, "cache"),
            None => to_event(&req.key, &own, "local"),
        };
        tracing::info!(key = %req.key, state = ?event.state, lines = event.lines.len(), aligned = aligned.is_some(), "lyrics from a local file");
        emit(event);
        return true;
    }
    if let Some(cache) = cache {
        if let Ok(Some(choice)) = cache.lyrics_choice(&req.key)
            && let Some(done) = chosen(req, choice, cache, transport, emit, now)
        {
            return done;
        }
        if req.force {
            let _ = cache.forget(&req.key);
        } else {
            match cache.get(&req.key, now) {
                Ok(Lookup::Hit(c)) => {
                    let event = to_event(&req.key, &c, "cache");
                    tracing::info!(key = %req.key, state = ?event.state, lines = event.lines.len(), "lyrics from cache");
                    emit(event);
                    if wants_recheck(&c, now) {
                        recheck(req, &c, cache, transport, emit, now);
                    }
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
    let mut outcome = lrclib::lookup(transport, &query(req));
    let mut stretch = None;
    if !matches!(&outcome, Outcome::Found { track, .. } if track.has_synced()) {
        match original_of_variant(req, transport) {
            Some(Ok((track, factor))) => {
                outcome = Outcome::Found { track, stage: "speed-variant" };
                stretch = Some(factor);
            }
            // A busy LRCLIB is no "not found": the original may well be there.
            Some(Err(e)) if outcome == Outcome::NotFound => outcome = Outcome::Unavailable(e),
            _ => {}
        }
    }
    let took_ms = started.elapsed().as_millis() as u64;
    match outcome {
        Outcome::Found { track, stage } => {
            let cached = match stretch {
                Some(factor) => Cached {
                    source: STRETCHED.into(),
                    raw: parser::stretch(track.synced_lyrics.as_deref().unwrap_or_default(), factor),
                    ..to_cached(&track, now)
                },
                None => to_cached(&track, now),
            };
            if let Some(cache) = cache
                && let Err(e) = cache.put(&req.key, &cached)
            {
                tracing::warn!(error = %e, "lyrics cache write failed");
            }
            let event = to_event(&req.key, &cached, "lrclib");
            tracing::info!(key = %req.key, stage, lrclib_id = track.id, state = ?event.state, lines = event.lines.len(), took_ms, stretch = ?stretch, "lyrics found");
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

fn query(req: &Request) -> Query<'_> {
    Query {
        title: &req.title,
        artist: &req.artist,
        album: req.album.as_deref(),
        duration_s: req.duration_ms.map(|d| d as f64 / 1000.0),
        speed: None,
    }
}

/// The original of a sped-up or slowed version (Nightcore, "Sped Up", "Slowed + Reverb") with
/// synced lyrics, and the factor that stretches its times onto this version (IDEEN.md,
/// package B): `None` for no such version, `Some(Err)` when LRCLIB was unavailable. LRCLIB knows
/// lengths in whole seconds, so late lines may be off by a few hundred milliseconds; Align's
/// Auto-Sync corrects that.
fn original_of_variant(req: &Request, transport: &Arc<dyn Transport>) -> Option<Result<(LrclibTrack, f64), String>> {
    let variant = matcher::speed_variant(&req.title, &req.artist)?;
    let ours = f64::from(req.duration_ms?) / 1000.0;
    let artist = if variant.artist_is_channel { "" } else { req.artist.as_str() };
    let q = Query { title: &variant.title, artist, album: None, duration_s: Some(ours), speed: Some(variant.speed) };
    match lrclib::lookup(transport, &q) {
        Outcome::Found { track, .. } if track.has_synced() && track.duration > 0.0 => {
            let factor = ours / track.duration;
            Some(Ok((track, factor)))
        }
        Outcome::Unavailable(e) => Some(Err(e)),
        _ => None,
    }
}

/// Lyrics the user picked for this song (IDEEN.md, package B): a LRCLIB entry, or none at all.
/// `None` when LRCLIB no longer has the entry: the pick is dropped and the normal search runs.
fn chosen(
    req: &Request,
    choice: Option<i64>,
    cache: &Cache,
    transport: &Arc<dyn Transport>,
    emit: &dyn Fn(LyricsEvent),
    now: i64,
) -> Option<bool> {
    let Some(id) = choice else {
        tracing::info!(key = %req.key, "no lyrics: the user chose none for this song");
        emit(LyricsEvent { source: CHOSE_NONE.into(), ..LyricsEvent::state(&req.key, LyricsState::Missing) });
        return Some(true);
    };
    if !req.force
        && let Ok(Lookup::Hit(c)) = cache.get(&req.key, now)
        && c.source == CHOSEN
        && c.source_id == Some(id)
    {
        let event = to_event(&req.key, &c, "cache");
        tracing::info!(key = %req.key, state = ?event.state, lines = event.lines.len(), "chosen lyrics from cache");
        emit(event);
        return Some(true);
    }
    match lrclib::by_id(transport.as_ref(), id) {
        Ok(Some(track)) => {
            let cached = Cached { source: CHOSEN.into(), ..to_cached(&track, now) };
            if let Err(e) = cache.put(&req.key, &cached) {
                tracing::warn!(error = %e, "lyrics cache write failed");
            }
            let event = to_event(&req.key, &cached, "lrclib");
            tracing::info!(key = %req.key, lrclib_id = id, state = ?event.state, lines = event.lines.len(), "chosen lyrics loaded");
            emit(event);
            Some(true)
        }
        Ok(None) => {
            tracing::warn!(key = %req.key, lrclib_id = id, "the chosen LRCLIB entry is gone, searching as usual");
            let _ = cache.clear_lyrics_choice(&req.key);
            None
        }
        Err(e) => {
            tracing::info!(key = %req.key, error = %e, "LRCLIB unavailable for the chosen lyrics");
            emit(LyricsEvent::state(&req.key, LyricsState::Error));
            Some(false)
        }
    }
}

fn wants_recheck(c: &Cached, now: i64) -> bool {
    (c.source.starts_with("plugin:") || c.format == Format::Plain) && now - c.fetched_at >= RECHECK_AFTER_S
}

/// Asks LRCLIB once more about lyrics that may have a synced version by now. Synced lyrics replace
/// them; any other answer counts as checked. A busy LRCLIB is asked again at the next play.
fn recheck(req: &Request, old: &Cached, cache: &Cache, transport: &Arc<dyn Transport>, emit: &dyn Fn(LyricsEvent), now: i64) {
    match lrclib::lookup(transport, &query(req)) {
        Outcome::Found { track, stage } if track.has_synced() => {
            let cached = to_cached(&track, now);
            if let Err(e) = cache.put(&req.key, &cached) {
                tracing::warn!(error = %e, "lyrics cache write failed");
            }
            let event = to_event(&req.key, &cached, "lrclib");
            tracing::info!(key = %req.key, stage, lrclib_id = track.id, replaced = %old.source, lines = event.lines.len(), "synced lyrics replace older ones from the cache");
            emit(event);
        }
        Outcome::Found { .. } | Outcome::NotFound => {
            let _ = cache.mark_checked(&req.key, now);
            tracing::info!(key = %req.key, kept = %old.source, "no synced lyrics on LRCLIB yet, the cached ones stay");
        }
        Outcome::Unavailable(e) => tracing::info!(key = %req.key, error = %e, "LRCLIB unavailable, the cached lyrics are checked at the next play"),
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

pub fn to_event(key: &str, c: &Cached, via: &str) -> LyricsEvent {
    let source = if via == "cache" { format!("cache:{}", c.source) } else { c.source.clone() };
    let state = match c.format {
        Format::Lrc => {
            let parsed = parser::parse(&c.raw);
            if parsed.synced && parsed.lines.iter().any(|l| !l.text.is_empty()) {
                let lrc_words = parsed.lines.iter().flat_map(|l| l.words.iter().flatten()).any(|w| !w.estimated);
                return LyricsEvent {
                    key: key.to_owned(),
                    state: LyricsState::Synced,
                    source,
                    lines: parsed.lines,
                    lrc_offset_ms: parsed.offset_ms,
                    word_source: if lrc_words { "lrc" } else { "estimate" }.into(),
                    line_sync_ms: None,
                };
            }
            LyricsState::Unsynced
        }
        Format::Plain => LyricsState::Unsynced,
        Format::None if c.instrumental => LyricsState::Instrumental,
        Format::None => LyricsState::Missing,
    };
    LyricsEvent { source, ..LyricsEvent::state(key, state) }
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn cached(format: Format, raw: &str, instrumental: bool) -> Cached {
        Cached { source: "lrclib".into(), source_id: Some(1), format, raw: raw.into(), instrumental, fetched_at: 0 }
    }

    /// LRCLIB with recorded-style answers: `/api/get` finds the song (with synced lyrics or
    /// without any), every search finds nothing. Counts the requests. The lyrics are invented.
    struct Lrclib {
        synced: bool,
        calls: AtomicUsize,
    }

    impl Transport for Lrclib {
        fn get(&self, path: &str, _: &[(&str, String)]) -> Result<(u16, String), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if path != "/api/get" {
                return Ok((200, "[]".into()));
            }
            if !self.synced {
                return Ok((404, r#"{"code":404,"name":"TrackNotFound","message":"Failed to find specified track"}"#.into()));
            }
            Ok((200, serde_json::json!({
                "id": 7, "trackName": "Paper Boats", "artistName": "The Inventions", "albumName": "Inventions",
                "duration": 231.0, "instrumental": false, "plainLyrics": "paper boats\ndrifting home",
                "syncedLyrics": "[00:01.00] paper boats\n[00:04.00] drifting home\n"
            }).to_string()))
        }
    }

    const DAY: i64 = 86_400;

    fn request() -> Request {
        Request {
            key: "the inventions|paper boats|231".into(),
            title: "Paper Boats".into(),
            artist: "The Inventions".into(),
            album: Some("Inventions".into()),
            duration_ms: Some(231_000),
            force: false,
        }
    }

    /// Runs one request against a cache holding `stored`; returns the emitted events and the
    /// number of LRCLIB requests.
    fn play(cache: &Cache, synced_on_lrclib: bool) -> (Vec<LyricsEvent>, usize) {
        let lrclib = Arc::new(Lrclib { synced: synced_on_lrclib, calls: AtomicUsize::new(0) });
        let transport: Arc<dyn Transport> = lrclib.clone();
        let events = Mutex::new(Vec::new());
        assert!(handle(&request(), 0, Some(cache), None, &transport, &|e| events.lock().unwrap().push(e)));
        (events.into_inner().unwrap(), lrclib.calls.load(Ordering::SeqCst))
    }

    fn store(source: &str, format: Format, raw: &str, age_s: i64) -> Cache {
        let cache = Cache::in_memory().unwrap();
        let c = Cached { source: source.into(), source_id: None, format, raw: raw.into(), instrumental: false, fetched_at: unix_now() - age_s };
        cache.put(&request().key, &c).unwrap();
        cache
    }

    #[test]
    fn old_plugin_lyrics_are_shown_and_then_replaced_by_synced_ones() {
        let cache = store("plugin:rec:recognized", Format::Lrc, "[00:01.00] papr bots\n", 15 * DAY);
        let (events, calls) = play(&cache, true);
        let sources: Vec<&str> = events.iter().map(|e| e.source.as_str()).collect();
        assert_eq!(sources, ["cache:plugin:rec:recognized", "lrclib"]);
        assert!(calls >= 1);
        let Ok(Lookup::Hit(now)) = cache.get(&request().key, unix_now()) else { panic!("cache entry expected") };
        assert_eq!(now.source, "lrclib");
    }

    #[test]
    fn young_plugin_lyrics_and_synced_lyrics_are_not_checked() {
        let (events, calls) = play(&store("plugin:rec:recognized", Format::Lrc, "[00:01.00] a\n", 13 * DAY), true);
        assert_eq!((events.len(), calls), (1, 0));
        let (events, calls) = play(&store("lrclib", Format::Lrc, "[00:01.00] a\n", 400 * DAY), true);
        assert_eq!((events.len(), calls), (1, 0));
    }

    /// LRCLIB as a function of the path, counting requests; the lyrics are invented.
    struct Answers {
        answer: fn(&str) -> (u16, String),
        calls: AtomicUsize,
    }

    impl Transport for Answers {
        fn get(&self, path: &str, _: &[(&str, String)]) -> Result<(u16, String), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok((self.answer)(path))
        }
    }

    fn original(id: i64) -> String {
        serde_json::json!({
            "id": id, "trackName": "Paper Boats", "artistName": "The Inventions", "albumName": "Inventions",
            "duration": 231.0, "instrumental": false, "plainLyrics": "paper boats\ndrifting home",
            "syncedLyrics": "[00:01.00] paper boats\n[00:04.00] drifting home\n"
        })
        .to_string()
    }

    fn run(req: &Request, cache: &Cache, local: Option<&Path>, answer: fn(&str) -> (u16, String)) -> (Vec<LyricsEvent>, usize) {
        let lrclib = Arc::new(Answers { answer, calls: AtomicUsize::new(0) });
        let transport: Arc<dyn Transport> = lrclib.clone();
        let events = Mutex::new(Vec::new());
        handle(req, 0, Some(cache), local, &transport, &|e| events.lock().unwrap().push(e));
        (events.into_inner().unwrap(), lrclib.calls.load(Ordering::SeqCst))
    }

    #[test]
    fn a_local_file_wins_without_asking_lrclib() {
        let dir = std::env::temp_dir().join(format!("lyrichub-service-local-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("The Inventions - Paper Boats.lrc"), "[00:02.00]paper boats\n").unwrap();
        let cache = store("lrclib", Format::Lrc, "[00:01.00] a\n", 0);
        let (events, calls) = run(&request(), &cache, Some(&dir), |_| (503, String::new()));
        assert_eq!((events[0].source.as_str(), events[0].lines[0].start_ms, calls), ("local", 2000, 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn own_text_without_timestamps_shows_aligned_once_a_plugin_did_it() {
        let dir = std::env::temp_dir().join(format!("lyrichub-service-plain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("The Inventions - Paper Boats.lrc"), "paper boats\ndrifting home\n").unwrap();
        let cache = Cache::in_memory().unwrap();
        let (events, _) = run(&request(), &cache, Some(&dir), |_| (503, String::new()));
        assert_eq!((events[0].state, events[0].source.as_str()), (LyricsState::Unsynced, "local"));
        let aligned = |at| Cached {
            source: "plugin:lyrichub-recognize:aligned_plain".into(),
            source_id: None,
            format: Format::Lrc,
            raw: "[00:01.00]paper boats\n[00:03.00]drifting home\n".into(),
            instrumental: false,
            fetched_at: at,
        };
        cache.put(&request().key, &aligned(unix_now() + 5)).unwrap();
        let (events, _) = run(&request(), &cache, Some(&dir), |_| (503, String::new()));
        assert_eq!((events[0].state, events[0].source.as_str()), (LyricsState::Synced, "cache:plugin:lyrichub-recognize:aligned_plain"));
        // Aligned before the file last changed: the edited text shows, not the old alignment.
        cache.put(&request().key, &aligned(0)).unwrap();
        let (events, _) = run(&request(), &cache, Some(&dir), |_| (503, String::new()));
        assert_eq!(events[0].source, "local");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_users_pick_wins_and_stays_on_reload() {
        let cache = Cache::in_memory().unwrap();
        cache.set_lyrics_choice(&request().key, Some(9), 0).unwrap();
        let by_id = |path: &str| if path == "/api/get/9" { (200, original(9)) } else { (503, String::new()) };
        let (events, calls) = run(&request(), &cache, None, by_id);
        assert_eq!((events.last().unwrap().source.as_str(), calls), (CHOSEN, 1));
        let (events, calls) = run(&request(), &cache, None, by_id);
        assert_eq!((events[0].source.as_str(), calls), ("cache:lrclib:chosen", 0));
        let (events, calls) = run(&Request { force: true, ..request() }, &cache, None, by_id);
        assert_eq!((events.last().unwrap().source.as_str(), calls), (CHOSEN, 1), "reload loads the pick again");
        cache.set_lyrics_choice(&request().key, None, 0).unwrap();
        let (events, calls) = run(&request(), &cache, None, by_id);
        assert_eq!((events[0].state, events[0].source.as_str(), calls), (LyricsState::Missing, CHOSE_NONE, 0));
    }

    #[test]
    fn a_sped_up_version_gets_the_originals_lyrics_stretched() {
        let cache = Cache::in_memory().unwrap();
        let req = Request { title: "Paper Boats (Sped Up)".into(), duration_ms: Some(185_000), key: "sped".into(), ..request() };
        // `/api/get` knows only the original's length, every search finds the original.
        let answer = |path: &str| if path == "/api/search" { (200, format!("[{}]", original(4))) } else { (404, String::new()) };
        let (events, _) = run(&req, &cache, None, answer);
        let last = events.last().unwrap();
        assert_eq!(last.source, STRETCHED);
        // 185 / 231 of the original's 1.00 s and 4.00 s, in LRC's hundredths.
        assert_eq!(last.lines.iter().map(|l| l.start_ms).collect::<Vec<_>>(), [800, 3200]);
    }

    #[test]
    fn a_busy_lrclib_while_looking_for_the_original_is_retried_not_remembered_as_missing() {
        let cache = Cache::in_memory().unwrap();
        let req = Request { title: "Paper Boats (Slowed)".into(), duration_ms: Some(280_000), key: "slow".into(), ..request() };
        // The version itself is unknown; the searches for the original find LRCLIB busy.
        let answer = |path: &str| if path == "/api/get" { (404, String::new()) } else { (503, String::new()) };
        let lrclib = Arc::new(Answers { answer, calls: AtomicUsize::new(0) });
        let transport: Arc<dyn Transport> = lrclib;
        let events = Mutex::new(Vec::new());
        assert!(!handle(&req, 0, Some(&cache), None, &transport, &|e| events.lock().unwrap().push(e)), "a retry makes sense");
        assert_eq!(events.into_inner().unwrap().last().unwrap().state, LyricsState::Error);
        assert_eq!(cache.get("slow", unix_now()).unwrap(), Lookup::Miss, "not cached as missing");
    }

    /// Against the real LRCLIB: a sped-up version (80 % of the length) of a known song finds the
    /// original. `cargo test --lib sped_up_version_on_lrclib -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn sped_up_version_on_lrclib() {
        let transport: Arc<dyn Transport> = Arc::new(lrclib::HttpTransport::new().unwrap());
        let cache = Cache::in_memory().unwrap();
        let req = Request {
            key: "live".into(),
            title: "Blinding Lights (Sped Up)".into(),
            artist: "The Weeknd".into(),
            album: None,
            duration_ms: Some(160_000),
            force: false,
        };
        let events = Mutex::new(Vec::new());
        handle(&req, 0, Some(&cache), None, &transport, &|e| events.lock().unwrap().push(e));
        let last = events.into_inner().unwrap().pop().unwrap();
        let end = last.lines.last().map_or(0, |l| l.start_ms);
        println!("source {}, state {:?}, {} lines, last line at {} ms", last.source, last.state, last.lines.len(), end);
        assert_eq!(last.source, STRETCHED);
    }

    #[test]
    fn old_plain_lyrics_without_a_better_version_are_checked_once() {
        let cache = store("lrclib", Format::Plain, "paper boats", 30 * DAY);
        let (events, calls) = play(&cache, false);
        assert_eq!(events.len(), 1);
        assert!(calls >= 1);
        let (events, calls) = play(&cache, false);
        assert_eq!((events.len(), calls), (1, 0), "checked just now");
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
