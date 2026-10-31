//! LRCLIB client (PLAN.md 3.3, 7.1). Exact lookups first, then a search with candidate
//! scoring. Observed behaviour (2026-10-25): known songs answer 200 within ~0.2 s; unknown
//! songs answer 503 "ServerOverloaded" instead of 404, so 503 means "maybe missing, maybe
//! busy" and is never cached as a miss. Under load every answer takes ~1.1 s and the same
//! request flips between 200 and 503. Without a duration `/api/get` returns some edition, so
//! every result is checked against the player's duration.
//!
//! Hedged stages: stage 1 starts at once; whenever 350 ms pass without an answer, or a stage
//! fails, the next one starts in parallel. The first fitting result with synced lyrics wins.
//! A fast server sees one request per song, a slow one at most one per stage.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::matcher::{self, Candidate, Wanted};

pub const BASE_URL: &str = "https://lrclib.net";
const TIMEOUT_S: u64 = 4;
/// A search with 20 results is about 200 KB; anything far beyond that is not a lyrics answer.
const MAX_BODY_BYTES: u64 = 4 * 1024 * 1024;
const HEDGE_AFTER: Duration = Duration::from_millis(350);

/// LRCLIB asks clients to identify themselves with name, version and a link to the project.
pub fn user_agent() -> String {
    format!("LyricHub/{} (https://github.com/HeiligerHoly/LyricHub)", env!("CARGO_PKG_VERSION"))
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LrclibTrack {
    pub id: i64,
    pub track_name: String,
    pub artist_name: String,
    #[serde(default)]
    pub album_name: Option<String>,
    pub duration: f64,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub synced_lyrics: Option<String>,
}

impl LrclibTrack {
    pub fn has_synced(&self) -> bool {
        self.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty())
    }

    fn candidate(&self) -> Candidate {
        Candidate {
            title: self.track_name.clone(),
            artist: self.artist_name.clone(),
            duration_s: self.duration,
            synced: self.has_synced(),
            instrumental: self.instrumental,
            last_line_s: self
                .synced_lyrics
                .as_deref()
                .and_then(|lrc| super::parser::parse(lrc).lines.last().map(|l| f64::from(l.start_ms) / 1000.0)),
        }
    }
}

/// HTTP GET returning (status, body); swapped for recorded answers in tests.
pub trait Transport: Send + Sync {
    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<(u16, String), String>;
}

pub struct HttpTransport {
    client: reqwest::blocking::Client,
    base: String,
}

impl HttpTransport {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(user_agent())
            .timeout(std::time::Duration::from_secs(TIMEOUT_S))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client, base: BASE_URL.to_owned() })
    }
}

impl Transport for HttpTransport {
    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<(u16, String), String> {
        use std::io::Read;
        let response =
            self.client.get(format!("{}{path}", self.base)).query(query).send().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let mut body = Vec::new();
        response.take(MAX_BODY_BYTES + 1).read_to_end(&mut body).map_err(|e| e.to_string())?;
        if body.len() as u64 > MAX_BODY_BYTES {
            return Err(format!("answer larger than {MAX_BODY_BYTES} bytes"));
        }
        Ok((status, String::from_utf8_lossy(&body).into_owned()))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Query<'a> {
    pub title: &'a str,
    /// Empty when unknown (an upload channel such as "Nightcore").
    pub artist: &'a str,
    pub album: Option<&'a str>,
    pub duration_s: Option<f64>,
    /// Looking for the original of a sped-up or slowed version (matcher::speed_variant).
    pub speed: Option<matcher::Speed>,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Found {
        track: LrclibTrack,
        stage: &'static str,
    },
    /// LRCLIB answered and has nothing that fits: worth caching as a miss.
    NotFound,
    /// Server busy or network trouble: try again later, do not cache.
    Unavailable(String),
}

/// Answer of one stage.
enum Answer {
    /// Fitting candidates, best first (one for `get`, the best of a search).
    Tracks(Vec<LrclibTrack>),
    /// LRCLIB answered and has nothing that fits.
    Missing,
    Failed(String),
}

struct Stage {
    name: &'static str,
    path: &'static str,
    query: Vec<(&'static str, String)>,
}

fn run_stage(t: &dyn Transport, stage: &Stage, wanted: &Wanted) -> Answer {
    let fits = |track: &LrclibTrack| matcher::score(&track.candidate(), wanted).is_some();
    match t.get(stage.path, &stage.query) {
        Ok((200, body)) if stage.path == "/api/get" => match serde_json::from_str::<LrclibTrack>(&body) {
            Ok(track) if fits(&track) => Answer::Tracks(vec![track]),
            Ok(_) => Answer::Missing,
            Err(e) => Answer::Failed(format!("unreadable answer: {e}")),
        },
        Ok((200, body)) => match serde_json::from_str::<Vec<LrclibTrack>>(&body) {
            Ok(list) => {
                let candidates: Vec<Candidate> = list.iter().map(LrclibTrack::candidate).collect();
                match matcher::best(&candidates, wanted) {
                    Some(i) => Answer::Tracks(vec![list[i].clone()]),
                    None => Answer::Missing,
                }
            }
            Err(e) => Answer::Failed(format!("unreadable answer: {e}")),
        },
        Ok((400 | 404, _)) => Answer::Missing,
        Ok((status, _)) => Answer::Failed(format!("HTTP {status}")),
        Err(e) => Answer::Failed(e),
    }
}

/// The stages of PLAN.md 7.1 in order of preference.
fn stages(q: &Query) -> Vec<Stage> {
    let stripped_title = matcher::strip_decorations(q.title);
    let main_artist = matcher::main_artist(q.artist);
    let get = |name, title: &str, artist: &str, album: Option<&str>| {
        let mut query = vec![("track_name", title.to_owned()), ("artist_name", artist.to_owned())];
        if let Some(album) = album {
            query.push(("album_name", album.to_owned()));
        }
        if let Some(d) = q.duration_s {
            query.push(("duration", format!("{}", d.round() as i64)));
        }
        Stage { name, path: "/api/get", query }
    };
    let mut list = Vec::new();
    // The original of a sped-up or slowed version has another length, which `/api/get` would
    // reject, and maybe no known artist: only the searches fit.
    if q.speed.is_some() {
        if !main_artist.is_empty() {
            list.push(Stage {
                name: "search",
                path: "/api/search",
                query: vec![("track_name", stripped_title.clone()), ("artist_name", main_artist.clone())],
            });
        }
        list.push(Stage { name: "search-title", path: "/api/search", query: vec![("track_name", stripped_title)] });
        return list;
    }
    if q.album.is_some() {
        list.push(get("get", q.title, q.artist, q.album));
    }
    list.push(get("get-without-album", q.title, q.artist, None));
    list.push(get("get-normalised", &stripped_title, &main_artist, None));
    list.dedup_by(|b, a| a.query == b.query);
    list.push(Stage {
        name: "search",
        path: "/api/search",
        query: vec![("track_name", stripped_title.clone()), ("artist_name", main_artist.clone())],
    });
    list.push(Stage {
        name: "search-text",
        path: "/api/search",
        query: vec![("q", format!("{stripped_title} {main_artist}"))],
    });
    // LRCLIB filters the artist strictly: a different spelling ("A & B", "A feat. C") hides the
    // song from both searches above. The title alone finds it; the matcher still checks the
    // artist and the length (docs/spike-recognize.md, M4).
    list.push(Stage { name: "search-title", path: "/api/search", query: vec![("track_name", stripped_title.clone())] });
    list
}

/// Runs the stages hedged (see module docs). A fitting result with synced lyrics ends the
/// lookup at once; plain-only or instrumental results win only when no stage has synced ones.
pub fn lookup(t: &Arc<dyn Transport>, q: &Query) -> Outcome {
    let stages: Arc<Vec<Stage>> = Arc::new(stages(q));
    let wanted = Arc::new((q.title.to_owned(), q.artist.to_owned(), q.duration_s, q.speed));
    let (tx, rx) = mpsc::channel::<(usize, Answer)>();
    let launch = |i: usize| {
        let (t, stages, wanted, tx) = (Arc::clone(t), Arc::clone(&stages), Arc::clone(&wanted), tx.clone());
        // Detached: a slow request may finish after the lookup has returned; its answer is dropped.
        std::thread::spawn(move || {
            let w = Wanted { title: &wanted.0, artist: &wanted.1, duration_s: wanted.2, speed: wanted.3 };
            let _ = tx.send((i, run_stage(t.as_ref(), &stages[i], &w)));
        });
    };

    let mut answers: Vec<Option<Answer>> = (0..stages.len()).map(|_| None).collect();
    let mut launched = 1;
    let mut next_hedge = Instant::now() + HEDGE_AFTER;
    launch(0);
    while answers.iter().any(Option::is_none) {
        let wait = next_hedge.saturating_duration_since(Instant::now());
        match rx.recv_timeout(if launched < stages.len() { wait } else { Duration::from_secs(TIMEOUT_S + 1) }) {
            Ok((i, answer)) => {
                if let Answer::Tracks(tracks) = &answer
                    && let Some(track) = tracks.iter().find(|t| t.has_synced())
                {
                    return Outcome::Found { track: track.clone(), stage: stages[i].name };
                }
                let next_now = !matches!(answer, Answer::Tracks(_));
                answers[i] = Some(answer);
                if next_now && launched < stages.len() {
                    launch(launched);
                    launched += 1;
                    next_hedge = Instant::now() + HEDGE_AFTER;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) if launched < stages.len() => {
                launch(launched);
                launched += 1;
                next_hedge = Instant::now() + HEDGE_AFTER;
            }
            // The transport has its own timeout; this only guards against a hung thread.
            Err(_) => break,
        }
        // Without a synced result, later stages still have to be asked before giving up.
        if launched < stages.len() && answers[..launched].iter().all(Option::is_some) {
            launch(launched);
            launched += 1;
            next_hedge = Instant::now() + HEDGE_AFTER;
        }
    }

    let mut answered = false;
    let mut trouble = None;
    let mut fallback = None;
    for (i, answer) in answers.into_iter().enumerate() {
        match answer {
            Some(Answer::Tracks(mut tracks)) => {
                answered = true;
                if fallback.is_none() && !tracks.is_empty() {
                    fallback = Some((tracks.remove(0), stages[i].name));
                }
            }
            Some(Answer::Missing) => answered = true,
            Some(Answer::Failed(e)) => trouble = Some(e),
            None => trouble = trouble.or(Some("no answer".into())),
        }
    }
    match (fallback, answered, trouble) {
        (Some((track, stage)), _, _) => Outcome::Found { track, stage },
        (None, true, _) => Outcome::NotFound,
        (None, false, Some(e)) => Outcome::Unavailable(e),
        (None, false, None) => Outcome::NotFound,
    }
}

/// Tries per search for the list to pick from: the user waits for it, and an overloaded LRCLIB
/// often answers the next request (module docs).
const PICK_ATTEMPTS: usize = 3;
const PICK_RETRY_AFTER: Duration = Duration::from_millis(600);

/// LRCLIB entries for a song that the user can pick from (IDEEN.md, package B): the search by
/// title and artist and the search by title alone, without the matcher's checks, merged, those
/// with synced lyrics and a fitting length first. At most 30.
pub fn candidates(t: &dyn Transport, title: &str, artist: &str, duration_s: Option<f64>) -> Result<Vec<LrclibTrack>, String> {
    let title = matcher::strip_decorations(title);
    let searches = [
        vec![("track_name", title.clone()), ("artist_name", matcher::main_artist(artist))],
        vec![("track_name", title)],
    ];
    let mut found: Vec<LrclibTrack> = Vec::new();
    let mut trouble = None;
    for query in &searches {
        for attempt in 1..=PICK_ATTEMPTS {
            let failed = match t.get("/api/search", query) {
                Ok((200, body)) => match serde_json::from_str::<Vec<LrclibTrack>>(&body) {
                    Ok(list) => {
                        let new: Vec<LrclibTrack> = list.into_iter().filter(|x| !found.iter().any(|f| f.id == x.id)).collect();
                        found.extend(new);
                        None
                    }
                    Err(e) => Some(format!("unreadable answer: {e}")),
                },
                Ok((400 | 404, _)) => None,
                Ok((status, _)) => Some(format!("HTTP {status}")),
                Err(e) => Some(e),
            };
            let Some(e) = failed else { break };
            trouble = Some(e);
            if attempt < PICK_ATTEMPTS {
                std::thread::sleep(PICK_RETRY_AFTER);
            }
        }
    }
    if found.is_empty()
        && let Some(e) = trouble
    {
        return Err(e);
    }
    let off = |x: &LrclibTrack| duration_s.map_or(0.0, |d| (x.duration - d).abs());
    found.sort_by(|a, b| b.has_synced().cmp(&a.has_synced()).then(off(a).total_cmp(&off(b))));
    found.truncate(30);
    Ok(found)
}

/// One LRCLIB entry by its id; `None` if LRCLIB no longer has it.
pub fn by_id(t: &dyn Transport, id: i64) -> Result<Option<LrclibTrack>, String> {
    match t.get(&format!("/api/get/{id}"), &[])? {
        (200, body) => serde_json::from_str(&body).map(Some).map_err(|e| format!("unreadable answer: {e}")),
        (404, _) => Ok(None),
        (status, _) => Err(format!("HTTP {status}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Recorded answers keyed by "path?k=v&k=v". Shapes follow real LRCLIB answers; the lyrics
    /// text is invented.
    struct Recorded(HashMap<String, (u16, String)>);

    impl Recorded {
        fn new(entries: &[(&str, u16, &str)]) -> Self {
            Self(entries.iter().map(|(k, s, b)| (k.to_string(), (*s, b.to_string()))).collect())
        }
    }

    impl Transport for Recorded {
        fn get(&self, path: &str, query: &[(&str, String)]) -> Result<(u16, String), String> {
            let key = format!("{path}?{}", query.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&"));
            Ok(self.0.get(&key).cloned().unwrap_or((503, BUSY.into())))
        }
    }

    const BUSY: &str =
        r#"{"message":"The server is busy, please retry in a moment","name":"ServerOverloaded","statusCode":503}"#;
    const SYNCED: &str = "[00:00.06] Morning light on empty streets\n[00:04.25] I count the steps back home\n";

    fn track(id: i64, title: &str, artist: &str, duration: f64, synced: bool) -> String {
        serde_json::json!({
            "id": id, "name": title, "trackName": title, "artistName": artist, "albumName": "Inventions",
            "duration": duration, "instrumental": false, "hasWordSync": false,
            "plainLyrics": "Morning light on empty streets\nI count the steps back home",
            "syncedLyrics": if synced { serde_json::Value::from(SYNCED) } else { serde_json::Value::Null },
            "lyricsfile": "version: '1.0'"
        })
        .to_string()
    }

    fn query<'a>(title: &'a str, album: Option<&'a str>) -> Query<'a> {
        Query { title, artist: "The Inventions", album, duration_s: Some(231.0), speed: None }
    }

    fn lookup(t: Recorded, q: &Query) -> Outcome {
        super::lookup(&(Arc::new(t) as Arc<dyn Transport>), q)
    }

    #[test]
    fn exact_hit_in_the_first_stage() {
        let t = Recorded::new(&[(
            "/api/get?track_name=Paper Boats&artist_name=The Inventions&album_name=Inventions&duration=231",
            200,
            &track(1, "Paper Boats", "The Inventions", 231.0, true),
        )]);
        let out = lookup(t, &query("Paper Boats", Some("Inventions")));
        assert!(matches!(out, Outcome::Found { stage: "get", ref track } if track.id == 1), "{out:?}");
    }

    #[test]
    fn wrong_edition_is_skipped_and_search_finds_the_right_one() {
        let search = format!(
            "[{},{}]",
            track(7, "Paper Boats", "The Inventions", 280.0, true),
            track(8, "Paper Boats", "The Inventions", 232.0, true)
        );
        let t = Recorded::new(&[
            (
                "/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231",
                200,
                &track(7, "Paper Boats", "The Inventions", 280.0, true),
            ),
            ("/api/search?track_name=Paper Boats&artist_name=The Inventions", 200, &search),
        ]);
        let out = lookup(t, &query("Paper Boats", None));
        assert!(matches!(out, Outcome::Found { stage: "search", ref track } if track.id == 8), "{out:?}");
    }

    #[test]
    fn busy_gets_and_empty_search_mean_not_found() {
        let t = Recorded::new(&[
            ("/api/search?track_name=Unknown Song&artist_name=The Inventions", 200, "[]"),
            ("/api/search?q=Unknown Song The Inventions", 200, "[]"),
        ]);
        assert_eq!(lookup(t, &query("Unknown Song", Some("Inventions"))), Outcome::NotFound);
    }

    #[test]
    fn the_title_alone_finds_another_spelling_of_the_artist() {
        let found = format!("[{}]", track(9, "Paper Boats", "The Inventions & Friends", 231.0, true));
        let t = Recorded::new(&[
            ("/api/search?track_name=Paper Boats&artist_name=The Inventions", 200, "[]"),
            ("/api/search?q=Paper Boats The Inventions", 200, "[]"),
            ("/api/search?track_name=Paper Boats", 200, &found),
        ]);
        let out = lookup(t, &query("Paper Boats", None));
        assert!(matches!(out, Outcome::Found { stage: "search-title", ref track } if track.id == 9), "{out:?}");
    }

    #[test]
    fn the_title_alone_does_not_take_another_artist() {
        let other = format!("[{}]", track(10, "Paper Boats", "Somebody Else Entirely", 231.0, true));
        let t = Recorded::new(&[
            ("/api/search?track_name=Paper Boats&artist_name=The Inventions", 200, "[]"),
            ("/api/search?q=Paper Boats The Inventions", 200, "[]"),
            ("/api/search?track_name=Paper Boats", 200, &other),
        ]);
        assert_eq!(lookup(t, &query("Paper Boats", None)), Outcome::NotFound);
    }

    #[test]
    fn busy_everywhere_is_unavailable() {
        let t = Recorded::new(&[]);
        assert!(matches!(lookup(t, &query("Paper Boats", None)), Outcome::Unavailable(_)));
    }

    #[test]
    fn plain_only_result_waits_for_a_synced_one() {
        let t = Recorded::new(&[
            (
                "/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231",
                200,
                &track(3, "Paper Boats", "The Inventions", 231.0, false),
            ),
            (
                "/api/search?track_name=Paper Boats&artist_name=The Inventions",
                200,
                &format!("[{}]", track(4, "Paper Boats", "The Inventions", 230.0, true)),
            ),
        ]);
        assert!(matches!(lookup(t, &query("Paper Boats", None)), Outcome::Found { ref track, .. } if track.id == 4));
        // Without a synced alternative the plain lyrics are used.
        let t = Recorded::new(&[
            (
                "/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231",
                200,
                &track(3, "Paper Boats", "The Inventions", 231.0, false),
            ),
            ("/api/search?track_name=Paper Boats&artist_name=The Inventions", 200, "[]"),
            ("/api/search?q=Paper Boats The Inventions", 200, "[]"),
        ]);
        assert!(
            matches!(lookup(t, &query("Paper Boats", None)), Outcome::Found { ref track, stage: "get-without-album" } if track.id == 3)
        );
    }

    #[test]
    fn decorated_title_is_found_in_the_normalised_stage() {
        let t = Recorded::new(&[(
            "/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231",
            200,
            &track(5, "Paper Boats", "The Inventions", 231.0, true),
        )]);
        let q = Query {
            title: "Paper Boats - Remastered 2011",
            artist: "The Inventions, Guest",
            album: None,
            duration_s: Some(231.0),
            speed: None,
        };
        assert!(matches!(lookup(t, &q), Outcome::Found { stage: "get-normalised", ref track } if track.id == 5));
    }

    #[test]
    fn the_original_of_a_sped_up_version_is_found_by_searching() {
        let t = Recorded::new(&[
            ("/api/search?track_name=Paper Boats&artist_name=The Inventions", 200, &format!("[{}]", track(6, "Paper Boats", "The Inventions", 231.0, true))),
            ("/api/search?track_name=Paper Boats", 200, "[]"),
        ]);
        let q = Query { title: "Paper Boats", artist: "The Inventions", album: None, duration_s: Some(180.0), speed: Some(matcher::Speed::Faster) };
        assert!(matches!(lookup(t, &q), Outcome::Found { stage: "search", ref track } if track.id == 6));
    }

    #[test]
    fn candidates_are_merged_with_synced_and_fitting_ones_first() {
        let t = Recorded::new(&[
            (
                "/api/search?track_name=Paper Boats&artist_name=The Inventions",
                200,
                &format!("[{},{}]", track(1, "Paper Boats", "The Inventions", 300.0, true), track(2, "Paper Boats", "The Inventions", 231.0, false)),
            ),
            (
                "/api/search?track_name=Paper Boats",
                200,
                &format!("[{},{}]", track(2, "Paper Boats", "The Inventions", 231.0, false), track(3, "Paper Boats", "Others", 232.0, true)),
            ),
        ]);
        let ids: Vec<i64> = candidates(&t, "Paper Boats (Live)", "The Inventions, Guest", Some(231.0)).unwrap().iter().map(|c| c.id).collect();
        assert_eq!(ids, [3, 1, 2]);
        let busy = Recorded::new(&[]);
        assert!(candidates(&busy, "Paper Boats", "The Inventions", None).is_err());
        assert_eq!(by_id(&Recorded::new(&[("/api/get/2?", 200, &track(2, "Paper Boats", "The Inventions", 231.0, false))]), 2).unwrap().map(|t| t.id), Some(2));
    }

    #[test]
    fn instrumental_is_found() {
        let body =
            serde_json::json!({ "id": 9, "trackName": "Interlude", "artistName": "The Inventions", "albumName": null,
            "duration": 95.0, "instrumental": true, "plainLyrics": null, "syncedLyrics": null })
            .to_string();
        let t = Recorded::new(&[
            ("/api/get?track_name=Interlude&artist_name=The Inventions&duration=95", 200, &body),
            ("/api/search?track_name=Interlude&artist_name=The Inventions", 200, "[]"),
            ("/api/search?q=Interlude The Inventions", 200, "[]"),
        ]);
        let q = Query { title: "Interlude", artist: "The Inventions", album: None, duration_s: Some(95.0), speed: None };
        assert!(matches!(lookup(t, &q), Outcome::Found { ref track, .. } if track.instrumental));
    }

    /// Answers after a delay, to check that a slow first stage does not hold up a fast later one.
    struct Slow(Recorded, std::collections::HashMap<String, u64>);

    impl Transport for Slow {
        fn get(&self, path: &str, query: &[(&str, String)]) -> Result<(u16, String), String> {
            let key = format!("{path}?{}", query.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&"));
            if let Some(ms) = self.1.get(&key) {
                std::thread::sleep(Duration::from_millis(*ms));
            }
            self.0.get(path, query)
        }
    }

    #[test]
    fn slow_first_stage_is_overtaken() {
        let first = "/api/get?track_name=Paper Boats&artist_name=The Inventions&album_name=Inventions&duration=231";
        let second = "/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231";
        let recorded = Recorded::new(&[
            (first, 503, BUSY),
            (second, 200, &track(2, "Paper Boats", "The Inventions", 231.0, true)),
        ]);
        let slow = Slow(recorded, [(first.to_owned(), 2000u64)].into_iter().collect());
        let started = Instant::now();
        let out = super::lookup(&(Arc::new(slow) as Arc<dyn Transport>), &query("Paper Boats", Some("Inventions")));
        assert!(matches!(out, Outcome::Found { stage: "get-without-album", ref track } if track.id == 2), "{out:?}");
        assert!(started.elapsed() < Duration::from_millis(1000), "took {:?}", started.elapsed());
    }
}
