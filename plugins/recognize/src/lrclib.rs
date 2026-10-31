//! The broader LRCLIB search of Recognize (PLAN-plugins-2.md 4.1). LRCLIB filters the artist
//! strictly and never searches the lyrics text, so this asks for the title alone, as `track_name`
//! and as `q`; the recognition then decides which candidate is the song that was heard. Requests
//! go one after the other and honour `Retry-After` (lrclib.net/docs).

use std::time::Duration;

use serde_json::Value;

const USER_AGENT: &str = concat!("lyrichub-recognize/", env!("CARGO_PKG_VERSION"), " (+https://github.com/HeiligerHoly/LyricHub)");

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub id: i64,
    pub duration_s: f64,
    pub synced: Option<String>,
    pub plain: Option<String>,
}

impl Candidate {
    /// The candidate's lyrics as plain text, for the comparison.
    pub fn text(&self) -> String {
        match (&self.plain, &self.synced) {
            (Some(p), _) if !p.trim().is_empty() => p.clone(),
            (_, Some(s)) => parse_lrc(s).into_iter().map(|l| l.1).collect::<Vec<_>>().join("\n"),
            _ => String::new(),
        }
    }
}

/// "Song (feat. X) - Remastered 2011" → "Song".
pub fn strip_title(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in title.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let base = out.split(" - ").next().unwrap_or("").trim().to_owned();
    if base.is_empty() { title.trim().to_owned() } else { base }
}

/// Candidates for a title, both queries merged.
pub fn search(title: &str) -> Result<Vec<Candidate>, String> {
    // Windows' own roots, as LyricHub uses for LRCLIB: with ureq's bundled roots instead, the
    // chain Windows builds for lrclib.net does not end in one of them and every request fails.
    let tls = ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .into();
    let stripped = strip_title(title);
    let mut found: Vec<Candidate> = Vec::new();
    let (mut failures, mut last) = (0, String::new());
    for (k, v) in [("track_name", stripped.as_str()), ("q", stripped.as_str())] {
        match get(&agent, k, v) {
            Ok(list) => {
                for c in list {
                    if !found.iter().any(|x| x.id == c.id) {
                        found.push(c);
                    }
                }
            }
            Err(e) => {
                failures += 1;
                last = e;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    if failures == 2 { Err(format!("LRCLIB unavailable: {last}")) } else { Ok(found) }
}

fn get(agent: &ureq::Agent, key: &str, value: &str) -> Result<Vec<Candidate>, String> {
    for attempt in 0..3u64 {
        let resp = agent.get("https://lrclib.net/api/search").query(key, value).call().map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        if status == 200 {
            let body = resp.into_body().with_config().limit(4 * 1024 * 1024).read_to_string().map_err(|e| e.to_string())?;
            let list: Vec<Value> = serde_json::from_str(&body).map_err(|e| e.to_string())?;
            return Ok(list.iter().filter_map(candidate).collect());
        }
        let wait = resp.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|s| s.parse::<u64>().ok()).unwrap_or(3 + attempt * 4);
        std::thread::sleep(Duration::from_secs(wait.min(60)));
    }
    Err("LRCLIB busy".into())
}

fn candidate(v: &Value) -> Option<Candidate> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_owned);
    Some(Candidate { id: v.get("id")?.as_i64()?, duration_s: v.get("duration").and_then(Value::as_f64).unwrap_or(0.0), synced: text("syncedLyrics"), plain: text("plainLyrics") })
}

/// Lines of an LRC file with their start in ms, sorted; tags like `[ar:…]` and empty lines go.
pub fn parse_lrc(lrc: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            if let Some(ms) = timestamp(&stripped[..end]) {
                times.push(ms);
            }
            rest = stripped[end + 1..].trim_start();
        }
        let text = rest.trim();
        if !text.is_empty() {
            out.extend(times.into_iter().map(|t| (t, text.to_owned())));
        }
    }
    out.sort_by_key(|l| l.0);
    out
}

/// "mm:ss.xx" or "mm:ss" in ms.
fn timestamp(s: &str) -> Option<u32> {
    let (m, rest) = s.split_once(':')?;
    let m: u32 = m.trim().parse().ok()?;
    let secs: f64 = rest.trim().parse().ok()?;
    (secs >= 0.0 && secs < 60.0).then(|| m * 60_000 + (secs * 1000.0).round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_lose_their_decorations() {
        assert_eq!(strip_title("Paper Boats (feat. Somebody) - Remastered 2011"), "Paper Boats");
        assert_eq!(strip_title("Paper Boats [Live]"), "Paper Boats");
        assert_eq!(strip_title("(Intro)"), "(Intro)", "nothing left: keep the title");
    }

    #[test]
    fn lrc_lines_with_times() {
        let lrc = "[ar:Nobody]\n[00:01.50]paper boats\n[00:05.00][01:05.00]drifting home\n[00:09.00]\n";
        assert_eq!(parse_lrc(lrc), vec![(1500, "paper boats".to_owned()), (5000, "drifting home".to_owned()), (65_000, "drifting home".to_owned())]);
        let c = Candidate { id: 1, duration_s: 0.0, synced: Some(lrc.into()), plain: None };
        assert_eq!(c.text(), "paper boats\ndrifting home\ndrifting home");
    }

    /// Asks lrclib.net for a common title. Network, so not in the normal run; prints counts only.
    #[test]
    #[ignore]
    fn searches_lrclib() {
        let found = search("Hello").expect("LRCLIB answers");
        println!("{} candidates, {} synced", found.len(), found.iter().filter(|c| c.synced.is_some()).count());
        assert!(!found.is_empty());
    }
}
