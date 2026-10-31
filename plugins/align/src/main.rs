//! lyrichub-align: a LyricHub plugin (PLAN-plugins.md, P3). It captures only the player's sound
//! while a song with estimated word times plays, aligns the lyrics to it after the song while no
//! fullscreen app is in front, and answers with real word times. The sound stays in memory and
//! is dropped after the alignment; only word times leave the plugin.
//!
//! Protocol (JSON-RPC 2.0 lines, PLAN-plugins.md 4.5): `initialize`, then the notifications
//! `track`, `lyrics`, `playback` and `screen` from LyricHub; `word_times`, `status` and `log`
//! back; `shutdown` or the end of stdin ends it.

#![windows_subsystem = "console"]

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lyrichub_align::capture::Capture;
use lyrichub_align::model;
use lyrichub_align::recorder::{Clock, LineIn, RawJob, Recorder};
use lyrichub_align::rpc::Out;
use lyrichub_align::status::{Problem, Status};
use lyrichub_align::worker::Worker;
use serde_json::{Value, json};

/// Pauses before the next download attempt.
const RETRY: [u64; 4] = [60, 300, 1800, 1800];

struct Plugin {
    out: Out,
    status: Arc<Status>,
    worker: Worker,
    recorder: Arc<Mutex<Recorder>>,
    capture: Option<Capture>,
    track: Option<String>,
    pid: Option<u32>,
    duration_ms: u32,
}

impl Plugin {
    fn start(params: &Value, out: Out) -> Plugin {
        let data_dir = PathBuf::from(params["data_dir"].as_str().unwrap_or("."));
        let status = Arc::new(Status::new(params["language"].as_str().unwrap_or("en"), out.clone()));
        let dir = model::dir(&data_dir);
        // Recognize uses the same model: shared on disk instead of a second download.
        if let Some(root) = data_dir.parent() {
            model::share_from(&model::dir(&root.join("lyrichub-recognize")), &dir);
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
        Plugin { out, status, worker, recorder, capture: None, track: None, pid: None, duration_ms: 0 }
    }

    fn queue(&self, job: Option<RawJob>) {
        if let Some(job) = job {
            self.worker.add(job);
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
        self.duration_ms = p["duration_ms"].as_u64().unwrap_or(0) as u32;
        self.update_capture();
    }

    fn on_lyrics(&mut self, p: &Value) {
        let key = p["key"].as_str();
        // Songs whose word times are already measured (Enhanced LRC, this plugin) are left alone.
        let estimated = p["word_source"].as_str().is_none_or(|s| s == "estimate");
        let job = if let Some(key) = key.filter(|k| Some(*k) == self.track.as_deref() && estimated) {
            let lines: Vec<LineIn> = p["lines"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|l| LineIn {
                            start_ms: l["start_ms"].as_u64().unwrap_or(0) as u32,
                            text: l["text"].as_str().unwrap_or("").to_owned(),
                            units: l["units"].as_array().map(|u| u.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let hash = p["lyrics_hash"].as_str().unwrap_or("");
            self.recorder.lock().unwrap().set_song(key, hash, lines, self.duration_ms)
        } else {
            self.recorder.lock().unwrap().clear()
        };
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

    /// Captures while a song is being recorded and the player is known; otherwise not at all.
    fn update_capture(&mut self) {
        let wanted = if self.recorder.lock().unwrap().recording() { self.pid } else { None };
        if self.capture.as_ref().map(|c| c.pid) == wanted {
            return;
        }
        self.capture = None;
        if let Some(pid) = wanted {
            let (recorder, worker) = (self.recorder.clone(), self.worker.clone());
            let (status, out) = (self.status.clone(), self.out.clone());
            self.capture = Some(Capture::start(
                pid,
                move |qpc, samples| {
                    let job = recorder.lock().unwrap().push(qpc, samples);
                    if let Some(job) = job {
                        worker.add(job);
                    }
                },
                move |e| {
                    out.log("warn", format!("capture failed: {e}"));
                    status.update(|b| b.problem = Some(Problem::Capture(e)));
                },
            ));
            self.status.update(|b| {
                if matches!(b.problem, Some(Problem::Capture(_))) {
                    b.problem = None;
                }
            });
        }
        let listening = self.capture.is_some();
        self.status.update(|b| b.listening = listening);
    }
}

/// Fetches the model, with pauses between attempts.
fn download(dir: &std::path::Path, status: &Status, worker: &Worker, out: &Out) {
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
    // A panic would otherwise end the plugin without a word in LyricHub's log.
    std::panic::set_hook(Box::new(|info| eprintln!("panic: {info}")));
    let out = Out::default();
    let mut plugin: Option<Plugin> = None;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let params = &msg["params"];
        match (msg["method"].as_str(), plugin.as_mut()) {
            (Some("initialize"), None) => {
                out.send(&json!({ "jsonrpc": "2.0", "id": msg["id"], "result": { "api": 1, "capabilities": ["word_timing"] } }));
                plugin = Some(Plugin::start(params, out.clone()));
            }
            (Some("track"), Some(p)) => p.on_track(params),
            (Some("lyrics"), Some(p)) => p.on_lyrics(params),
            (Some("playback"), Some(p)) => p.on_playback(params),
            (Some("screen"), Some(p)) => p.worker.set_fullscreen(params["fullscreen"].as_bool().unwrap_or(false)),
            (Some("shutdown"), _) => break,
            _ => {}
        }
    }
    // Ends the capture thread; the worker may be inside the model and simply ends with us.
    if let Some(mut p) = plugin {
        p.capture = None;
    }
    std::process::exit(0);
}
