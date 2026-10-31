//! lyrichub-recognize: a LyricHub plugin (PLAN-plugins-2.md, Q3). For a song without synced
//! lyrics it captures only the player's sound, and after the song, while no fullscreen app is
//! in front, it aligns lyrics without timestamps, finds the song on LRCLIB with a broader search
//! checked against what it heard, or recognizes the text from the singing. The sound stays in
//! memory; only lyrics and their times leave the plugin, and recognized ones are marked.
//!
//! Protocol (JSON-RPC 2.0 lines): `initialize`, then `track`, `playback`, `screen` and
//! `lyrics_wanted` from LyricHub; `lyrics_found`, `status` and `log` back; `shutdown` or the end
//! of stdin ends it.

mod lrclib;
mod worker;

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lyrichub_align::capture::Capture;
use lyrichub_align::model;
use lyrichub_align::recorder::{Clock, Recorder};
use lyrichub_align::rpc::Out;
use lyrichub_align::status::{Problem, Status};
use serde_json::{Value, json};
use worker::{Request, Worker};

/// Pauses before the next download attempt.
const RETRY: [u64; 4] = [60, 300, 1800, 1800];

struct Plugin {
    out: Out,
    status: Arc<Status>,
    worker: Worker,
    recorder: Arc<Mutex<Recorder>>,
    /// What LyricHub asked for, by song; the job takes it along.
    requests: Arc<Mutex<HashMap<String, Request>>>,
    capture: Option<Capture>,
    track: Option<String>,
    pid: Option<u32>,
}

impl Plugin {
    fn start(params: &Value, out: Out) -> Plugin {
        let data_dir = PathBuf::from(params["data_dir"].as_str().unwrap_or("."));
        let status = Arc::new(Status::with_work(params["language"].as_str().unwrap_or("en"), out.clone(), ("Erkennt einen Song", "Recognizing a song")));
        let dir = model::dir(&data_dir);
        // Align uses the same model: shared on disk instead of a second download.
        if let Some(root) = data_dir.parent() {
            model::share_from(&model::dir(&root.join("lyrichub-align")), &dir);
        }
        let worker = Worker::start(dir.clone(), status.clone(), out.clone());
        if model::ready(&dir) {
            worker.set_ready();
            status.update(|_| {});
        } else {
            let (status, worker, out) = (status.clone(), worker.clone(), out.clone());
            std::thread::spawn(move || download(&dir, &status, &worker, &out));
        }
        let recorder = Arc::new(Mutex::new(Recorder::with_log(out.clone())));
        Plugin { out, status, worker, recorder, requests: Arc::default(), capture: None, track: None, pid: None }
    }

    fn queue(&self, job: Option<lyrichub_align::recorder::RawJob>) {
        if let Some(job) = job
            && let Some(req) = self.requests.lock().unwrap().remove(&job.key)
        {
            self.worker.add(req, job);
        }
    }

    fn on_track(&mut self, p: &Value) {
        let key = p["key"].as_str().map(str::to_owned);
        if key != self.track {
            let job = self.recorder.lock().unwrap().clear();
            self.queue(job);
            self.track = key;
        }
        self.pid = p["player"]["pid"].as_u64().map(|x| x as u32);
        self.update_capture();
    }

    fn on_lyrics_wanted(&mut self, p: &Value) {
        let Some(key) = p["key"].as_str().filter(|k| Some(*k) == self.track.as_deref()) else { return };
        let req = Request {
            key: key.to_owned(),
            title: p["title"].as_str().unwrap_or("").to_owned(),
            duration_ms: p["duration_ms"].as_u64().unwrap_or(0) as u32,
            plain: p["plain"].as_str().map(str::to_owned),
        };
        let duration = req.duration_ms;
        self.requests.lock().unwrap().insert(key.to_owned(), req);
        let job = self.recorder.lock().unwrap().set_whole_song(key, duration);
        self.queue(job);
        self.update_capture();
    }

    fn on_playback(&mut self, p: &Value) {
        let clock = Clock {
            playing: p["playing"].as_bool().unwrap_or(false),
            position_ms: p["position_ms"].as_f64().unwrap_or(0.0),
            rate: p["rate"].as_f64().unwrap_or(1.0),
            qpc_100ns: p["qpc_100ns"].as_i64().unwrap_or(0),
            offset_ms: p["offset_ms"].as_f64().unwrap_or(0.0),
        };
        let job = self.recorder.lock().unwrap().set_clock(clock);
        self.queue(job);
    }

    /// Captures while a wanted song is being recorded and the player is known.
    fn update_capture(&mut self) {
        let wanted = if self.recorder.lock().unwrap().recording() { self.pid } else { None };
        if self.capture.as_ref().map(|c| c.pid) == wanted {
            return;
        }
        self.capture = None;
        if let Some(pid) = wanted {
            let (recorder, worker, requests) = (self.recorder.clone(), self.worker.clone(), self.requests.clone());
            let (status, out) = (self.status.clone(), self.out.clone());
            self.capture = Some(Capture::start(
                pid,
                move |qpc, samples| {
                    let job = recorder.lock().unwrap().push(qpc, samples);
                    if let Some(job) = job
                        && let Some(req) = requests.lock().unwrap().remove(&job.key)
                    {
                        worker.add(req, job);
                    }
                },
                move |e| {
                    out.log("warn", format!("capture failed: {e}"));
                    status.update(|b| b.problem = Some(Problem::Capture(e)));
                },
            ));
        }
        let listening = self.capture.is_some();
        self.status.update(|b| b.listening = listening);
    }
}

/// Fetches the model, with pauses between attempts.
fn download(dir: &Path, status: &Status, worker: &Worker, out: &Out) {
    for attempt in 0.. {
        let mut last = u64::MAX;
        let result = model::download(dir, |done, total| {
            let pct = done * 100 / total.max(1);
            if pct != last {
                last = pct;
                status.update(|b| {
                    b.download = Some((done, total));
                    b.problem = None;
                });
            }
        });
        match result {
            Ok(()) => {
                out.log("info", "model ready");
                status.update(|b| b.download = None);
                worker.set_ready();
                return;
            }
            Err(e) => {
                out.log("warn", format!("model download failed: {e}"));
                status.update(|b| {
                    b.download = None;
                    b.problem = Some(Problem::Download(e));
                });
                std::thread::sleep(Duration::from_secs(RETRY[attempt.min(RETRY.len() - 1)]));
            }
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| eprintln!("panic: {info}")));
    let out = Out::default();
    let mut plugin: Option<Plugin> = None;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let params = &msg["params"];
        match (msg["method"].as_str(), plugin.as_mut()) {
            (Some("initialize"), None) => {
                out.send(&json!({ "jsonrpc": "2.0", "id": msg["id"], "result": { "api": 1, "capabilities": ["lyrics_source"] } }));
                plugin = Some(Plugin::start(params, out.clone()));
            }
            (Some("track"), Some(p)) => p.on_track(params),
            (Some("lyrics_wanted"), Some(p)) => p.on_lyrics_wanted(params),
            (Some("playback"), Some(p)) => p.on_playback(params),
            (Some("screen"), Some(p)) => p.worker.set_fullscreen(params["fullscreen"].as_bool().unwrap_or(false)),
            (Some("shutdown"), _) => break,
            _ => {}
        }
    }
    if let Some(mut p) = plugin {
        p.capture = None;
    }
    std::process::exit(0);
}
