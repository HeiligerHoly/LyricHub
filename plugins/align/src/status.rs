//! The status line on LyricHub's plugin page, in German or English, with a progress bar for the
//! download and the alignment. Every part of the plugin sets its field; the line goes out when
//! its text changes.

use std::sync::Mutex;

use serde_json::json;

use crate::rpc::Out;

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Board {
    /// Model download: bytes done and total.
    pub download: Option<(u64, u64)>,
    pub problem: Option<Problem>,
    /// The player's sound is being captured for a song.
    pub listening: bool,
    pub queued: usize,
    /// Share of the current song aligned.
    pub working: Option<f64>,
    /// Songs wait while a fullscreen app is in front.
    pub paused: bool,
    pub aligned: usize,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Problem {
    Download(String),
    Capture(String),
    Model(String),
}

pub struct Status {
    german: bool,
    board: Mutex<(Board, String)>,
    out: Out,
}

impl Status {
    pub fn new(language: &str, out: Out) -> Self {
        Status { german: language.starts_with("de"), board: Mutex::new((Board::default(), String::new())), out }
    }

    pub fn update(&self, f: impl FnOnce(&mut Board)) {
        let mut guard = self.board.lock().unwrap();
        f(&mut guard.0);
        let text = render(self.german, &guard.0);
        if text != guard.1 {
            guard.1 = text.clone();
            self.out.notify("status", json!({ "text": text, "progress": progress(&guard.0) }));
        }
    }
}

/// The bar under the line: the download, or the song being aligned.
pub fn progress(b: &Board) -> Option<f64> {
    match (b.download, b.working) {
        (Some((done, total)), _) => Some(done as f64 / total.max(1) as f64),
        (None, Some(p)) if !b.paused && b.problem.is_none() => Some(p),
        _ => None,
    }
}

pub fn render(de: bool, b: &Board) -> String {
    let pct = |x: f64| (x * 100.0).floor().clamp(0.0, 100.0) as u32;
    let waiting = |n: usize| match (de, n) {
        (_, 0) => String::new(),
        (true, 1) => " · 1 Song wartet".into(),
        (true, n) => format!(" · {n} Songs warten"),
        (false, 1) => " · 1 song waiting".into(),
        (false, n) => format!(" · {n} songs waiting"),
    };
    if let Some((done, total)) = b.download {
        let mb = total / 1_000_000;
        let p = pct(done as f64 / total.max(1) as f64);
        return if de { format!("Lädt das Modell: {p} % von {mb} MB") } else { format!("Downloading the model: {p} % of {mb} MB") };
    }
    match &b.problem {
        Some(Problem::Download(e)) => {
            return if de { format!("Download fehlgeschlagen ({e}), neuer Versuch später") } else { format!("Download failed ({e}), retrying later") };
        }
        Some(Problem::Model(e)) => {
            return if de { format!("Das Modell lässt sich nicht laden: {e}") } else { format!("The model cannot be loaded: {e}") };
        }
        _ => {}
    }
    if b.paused {
        let w = waiting(b.queued);
        return if de { format!("Pausiert, solange ein Vollbildspiel läuft{w}") } else { format!("Paused while a fullscreen game runs{w}") };
    }
    if let Some(p) = b.working {
        let (p, w) = (pct(p), waiting(b.queued));
        return if de { format!("Richtet einen Song aus: {p} %{w}") } else { format!("Aligning a song: {p} %{w}") };
    }
    if let Some(Problem::Capture(e)) = &b.problem {
        return if de { format!("Kein Mitschnitt möglich: {e}") } else { format!("Cannot capture the sound: {e}") };
    }
    let w = waiting(b.queued);
    let done = match (de, b.aligned) {
        (_, 0) => String::new(),
        (true, 1) => " · 1 Song ausgerichtet".into(),
        (true, n) => format!(" · {n} Songs ausgerichtet"),
        (false, 1) => " · 1 song aligned".into(),
        (false, n) => format!(" · {n} songs aligned"),
    };
    match (b.listening, de) {
        (true, true) => format!("Hört mit{w}{done}"),
        (true, false) => format!("Listening{w}{done}"),
        (false, true) => format!("Bereit{w}{done}"),
        (false, false) => format!("Ready{w}{done}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_download_comes_first() {
        let b = Board { download: Some((294_000_000, 587_467_302)), listening: true, ..Default::default() };
        assert_eq!(render(true, &b), "Lädt das Modell: 50 % von 587 MB");
        assert_eq!(render(false, &b), "Downloading the model: 50 % of 587 MB");
    }

    #[test]
    fn a_fullscreen_game_pauses_the_work() {
        let b = Board { paused: true, queued: 2, working: Some(0.4), ..Default::default() };
        assert_eq!(render(true, &b), "Pausiert, solange ein Vollbildspiel läuft · 2 Songs warten");
    }

    #[test]
    fn idle_lines() {
        let b = Board { listening: true, queued: 1, aligned: 3, ..Default::default() };
        assert_eq!(render(true, &b), "Hört mit · 1 Song wartet · 3 Songs ausgerichtet");
        assert_eq!(render(false, &Board::default()), "Ready");
        let b = Board { working: Some(0.456), ..Default::default() };
        assert_eq!(render(false, &b), "Aligning a song: 45 %");
    }
}
