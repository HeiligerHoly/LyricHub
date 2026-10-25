//! LRCLIB client (PLAN.md 3.3, 7.1). Exact lookups first, then a search with candidate
//! scoring. Observed behaviour (2026-10-25): known songs answer 200 within ~0.2 s; unknown
//! songs answer 503 "ServerOverloaded" instead of 404, so 503 means "maybe missing, maybe
//! busy" and is never cached as a miss. Without a duration `/api/get` returns some edition,
//! so every result is checked against the player's duration.

use serde::Deserialize;

use super::matcher::{self, Candidate, Wanted};

pub const BASE_URL: &str = "https://lrclib.net";
const TIMEOUT_S: u64 = 4;

/// LRCLIB asks clients to identify themselves.
pub fn user_agent() -> String {
    format!("LyricHub/{} (Windows desktop lyrics overlay)", env!("CARGO_PKG_VERSION"))
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
        }
    }
}

/// HTTP GET returning (status, body); swapped for recorded answers in tests.
pub trait Transport: Send {
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
        let response = self.client.get(format!("{}{path}", self.base)).query(query).send().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        Ok((status, response.text().unwrap_or_default()))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Query<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: Option<&'a str>,
    pub duration_s: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Found { track: LrclibTrack, stage: &'static str },
    /// LRCLIB answered and has nothing that fits: worth caching as a miss.
    NotFound,
    /// Server busy or network trouble: try again later, do not cache.
    Unavailable(String),
}

enum Answer {
    Track(LrclibTrack),
    Missing,
    Failed(String),
}

fn get(t: &dyn Transport, title: &str, artist: &str, album: Option<&str>, duration_s: Option<f64>) -> Answer {
    let mut q = vec![("track_name", title.to_owned()), ("artist_name", artist.to_owned())];
    if let Some(album) = album {
        q.push(("album_name", album.to_owned()));
    }
    if let Some(d) = duration_s {
        q.push(("duration", format!("{}", d.round() as i64)));
    }
    match t.get("/api/get", &q) {
        Ok((200, body)) => match serde_json::from_str(&body) {
            Ok(track) => Answer::Track(track),
            Err(e) => Answer::Failed(format!("unreadable answer: {e}")),
        },
        Ok((400 | 404, _)) => Answer::Missing,
        Ok((status, _)) => Answer::Failed(format!("HTTP {status}")),
        Err(e) => Answer::Failed(e),
    }
}

fn search(t: &dyn Transport, query: &[(&str, String)]) -> Result<Vec<LrclibTrack>, String> {
    match t.get("/api/search", query) {
        Ok((200, body)) => serde_json::from_str(&body).map_err(|e| format!("unreadable answer: {e}")),
        Ok((status, _)) => Err(format!("HTTP {status}")),
        Err(e) => Err(e),
    }
}

/// Runs the stages of PLAN.md 7.1. A result with synced lyrics ends the search; plain-only or
/// instrumental results are kept as a fallback in case a later stage has synced lyrics.
pub fn lookup(t: &dyn Transport, q: &Query) -> Outcome {
    let wanted = Wanted { title: q.title, artist: q.artist, duration_s: q.duration_s };
    let fits = |track: &LrclibTrack| matcher::score(&track.candidate(), &wanted).is_some();
    let stripped_title = matcher::strip_decorations(q.title);
    let main_artist = matcher::main_artist(q.artist);

    let mut stages: Vec<(&'static str, String, String, Option<&str>)> = vec![
        ("get", q.title.to_owned(), q.artist.to_owned(), q.album),
        ("get-without-album", q.title.to_owned(), q.artist.to_owned(), None),
        ("get-normalised", stripped_title.clone(), main_artist.clone(), None),
    ];
    if q.album.is_none() {
        stages.remove(0);
    }
    stages.dedup_by(|b, a| a.1 == b.1 && a.2 == b.2 && a.3 == b.3);

    let mut fallback: Option<(LrclibTrack, &'static str)> = None;
    let mut trouble: Option<String> = None;
    for (stage, title, artist, album) in &stages {
        match get(t, title, artist, *album, q.duration_s) {
            Answer::Track(track) if fits(&track) => {
                if track.has_synced() {
                    return Outcome::Found { track, stage };
                }
                fallback.get_or_insert((track, stage));
            }
            Answer::Track(_) | Answer::Missing => {}
            Answer::Failed(e) => trouble = Some(e),
        }
    }

    let searches: [(&'static str, Vec<(&str, String)>); 2] = [
        ("search", vec![("track_name", stripped_title.clone()), ("artist_name", main_artist.clone())]),
        ("search-text", vec![("q", format!("{stripped_title} {main_artist}"))]),
    ];
    let mut answered = false;
    for (stage, query) in searches {
        match search(t, &query) {
            Ok(list) => {
                answered = true;
                let candidates: Vec<Candidate> = list.iter().map(LrclibTrack::candidate).collect();
                if let Some(i) = matcher::best(&candidates, &wanted) {
                    let track = list[i].clone();
                    if track.has_synced() || fallback.is_none() {
                        return Outcome::Found { track, stage };
                    }
                }
            }
            Err(e) => trouble = Some(e),
        }
    }

    match (fallback, answered, trouble) {
        (Some((track, stage)), _, _) => Outcome::Found { track, stage },
        (None, true, _) => Outcome::NotFound,
        (None, false, Some(e)) => Outcome::Unavailable(e),
        (None, false, None) => Outcome::NotFound,
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

    const BUSY: &str = r#"{"message":"The server is busy, please retry in a moment","name":"ServerOverloaded","statusCode":503}"#;
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
        Query { title, artist: "The Inventions", album, duration_s: Some(231.0) }
    }

    #[test]
    fn exact_hit_in_the_first_stage() {
        let t = Recorded::new(&[(
            "/api/get?track_name=Paper Boats&artist_name=The Inventions&album_name=Inventions&duration=231",
            200,
            &track(1, "Paper Boats", "The Inventions", 231.0, true),
        )]);
        let out = lookup(&t, &query("Paper Boats", Some("Inventions")));
        assert!(matches!(out, Outcome::Found { stage: "get", ref track } if track.id == 1), "{out:?}");
    }

    #[test]
    fn wrong_edition_is_skipped_and_search_finds_the_right_one() {
        let search = format!("[{},{}]", track(7, "Paper Boats", "The Inventions", 280.0, true), track(8, "Paper Boats", "The Inventions", 232.0, true));
        let t = Recorded::new(&[
            ("/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231", 200, &track(7, "Paper Boats", "The Inventions", 280.0, true)),
            ("/api/search?track_name=Paper Boats&artist_name=The Inventions", 200, &search),
        ]);
        let out = lookup(&t, &query("Paper Boats", None));
        assert!(matches!(out, Outcome::Found { stage: "search", ref track } if track.id == 8), "{out:?}");
    }

    #[test]
    fn busy_gets_and_empty_search_mean_not_found() {
        let t = Recorded::new(&[
            ("/api/search?track_name=Unknown Song&artist_name=The Inventions", 200, "[]"),
            ("/api/search?q=Unknown Song The Inventions", 200, "[]"),
        ]);
        assert_eq!(lookup(&t, &query("Unknown Song", Some("Inventions"))), Outcome::NotFound);
    }

    #[test]
    fn busy_everywhere_is_unavailable() {
        let t = Recorded::new(&[]);
        assert!(matches!(lookup(&t, &query("Paper Boats", None)), Outcome::Unavailable(_)));
    }

    #[test]
    fn plain_only_result_waits_for_a_synced_one() {
        let t = Recorded::new(&[
            ("/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231", 200, &track(3, "Paper Boats", "The Inventions", 231.0, false)),
            ("/api/search?track_name=Paper Boats&artist_name=The Inventions", 200, &format!("[{}]", track(4, "Paper Boats", "The Inventions", 230.0, true))),
        ]);
        assert!(matches!(lookup(&t, &query("Paper Boats", None)), Outcome::Found { ref track, .. } if track.id == 4));
        // Without a synced alternative the plain lyrics are used.
        let t = Recorded::new(&[
            ("/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231", 200, &track(3, "Paper Boats", "The Inventions", 231.0, false)),
            ("/api/search?track_name=Paper Boats&artist_name=The Inventions", 200, "[]"),
            ("/api/search?q=Paper Boats The Inventions", 200, "[]"),
        ]);
        assert!(matches!(lookup(&t, &query("Paper Boats", None)), Outcome::Found { ref track, stage: "get-without-album" } if track.id == 3));
    }

    #[test]
    fn decorated_title_is_found_in_the_normalised_stage() {
        let t = Recorded::new(&[(
            "/api/get?track_name=Paper Boats&artist_name=The Inventions&duration=231",
            200,
            &track(5, "Paper Boats", "The Inventions", 231.0, true),
        )]);
        let q = Query { title: "Paper Boats - Remastered 2011", artist: "The Inventions, Guest", album: None, duration_s: Some(231.0) };
        assert!(matches!(lookup(&t, &q), Outcome::Found { stage: "get-normalised", ref track } if track.id == 5));
    }

    #[test]
    fn instrumental_is_found() {
        let body = serde_json::json!({ "id": 9, "trackName": "Interlude", "artistName": "The Inventions", "albumName": null,
            "duration": 95.0, "instrumental": true, "plainLyrics": null, "syncedLyrics": null }).to_string();
        let t = Recorded::new(&[
            ("/api/get?track_name=Interlude&artist_name=The Inventions&duration=95", 200, &body),
            ("/api/search?track_name=Interlude&artist_name=The Inventions", 200, "[]"),
            ("/api/search?q=Interlude The Inventions", 200, "[]"),
        ]);
        let q = Query { title: "Interlude", artist: "The Inventions", album: None, duration_s: Some(95.0) };
        assert!(matches!(lookup(&t, &q), Outcome::Found { ref track, .. } if track.instrumental));
    }
}
