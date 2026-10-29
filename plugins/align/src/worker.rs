//! Aligns the heard songs in the background (PLAN-plugins.md 5.4): only while no fullscreen app
//! is in front, line by line so a game that starts pauses the work within a line, with two
//! threads at LyricHub's "below normal" priority for plugins. The model takes about 0.6 GB and
//! is loaded only while there is work. At most 256 MB of sound wait, a good two hours; then
//! the oldest song goes. Model, computation and queue stay under 1.5 GB together.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::align::Model;
use crate::capture::RATE;
use crate::features::SAMPLE_RATE;
use crate::model;
use crate::recorder::{JobLine, RawJob};
use crate::resample::resample;
use crate::rpc::Out;
use crate::status::{Problem, Status};

const THREADS: usize = 2;
const QUEUE_BYTES: usize = 256 * 1024 * 1024;
/// The model goes after this long without work, or paused by a game.
const UNLOAD_AFTER: Duration = Duration::from_secs(60);

/// A heard song at the model's rate.
pub struct Job {
    pub key: String,
    pub hash: String,
    pub lines: Vec<JobLine>,
    pub audio: Vec<i16>,
    pub start_ms: u32,
}

impl Job {
    fn from_raw(raw: RawJob) -> Job {
        let x: Vec<f32> = raw.audio.iter().map(|&s| f32::from(s) / 32768.0).collect();
        let audio = resample(&x, RATE, SAMPLE_RATE).iter().map(|s| (s * 32768.0).round().clamp(-32768.0, 32767.0) as i16).collect();
        Job { key: raw.key, hash: raw.hash, lines: raw.lines, audio, start_ms: raw.start_ms }
    }

    fn bytes(&self) -> usize {
        self.audio.len() * 2
    }
}

struct Shared {
    queue: Mutex<VecDeque<Job>>,
    wake: Condvar,
    fullscreen: AtomicBool,
    ready: AtomicBool,
    status: Arc<Status>,
    out: Out,
    model_dir: PathBuf,
}

#[derive(Clone)]
pub struct Worker(Arc<Shared>);

impl Worker {
    pub fn start(model_dir: PathBuf, status: Arc<Status>, out: Out) -> Worker {
        let w = Worker(Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            wake: Condvar::new(),
            fullscreen: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            status,
            out,
            model_dir,
        }));
        let me = w.clone();
        std::thread::spawn(move || me.run());
        w
    }

    /// A heard song; turning it into 16 kHz takes a moment, so not on the caller's thread.
    pub fn add(&self, raw: RawJob) {
        let me = self.clone();
        std::thread::spawn(move || {
            let job = Job::from_raw(raw);
            let mut q = me.0.queue.lock().unwrap();
            let mut bytes: usize = q.iter().map(Job::bytes).sum();
            while !q.is_empty() && bytes + job.bytes() > QUEUE_BYTES {
                bytes -= q.pop_front().map_or(0, |j| j.bytes());
                me.0.out.log("info", "queue full: the oldest song was dropped");
            }
            me.0.out.log("info", format!("song queued: {} lines, {} s of sound", job.lines.len(), job.audio.len() / SAMPLE_RATE as usize));
            q.push_back(job);
            let n = q.len();
            drop(q);
            me.0.status.update(|b| b.queued = n);
            me.0.wake.notify_all();
        });
    }

    pub fn set_fullscreen(&self, on: bool) {
        self.0.fullscreen.store(on, Ordering::SeqCst);
        self.0.wake.notify_all();
    }

    pub fn set_ready(&self) {
        self.0.ready.store(true, Ordering::SeqCst);
        self.0.wake.notify_all();
    }

    fn run(&self) {
        let s = &self.0;
        let mut model: Option<Model> = None;
        let mut idle = Instant::now();
        loop {
            let job = {
                let mut q = s.queue.lock().unwrap();
                loop {
                    let fullscreen = s.fullscreen.load(Ordering::SeqCst);
                    if s.ready.load(Ordering::SeqCst) && !fullscreen && let Some(job) = q.pop_front() {
                        break job;
                    }
                    if model.is_some() && idle.elapsed() > UNLOAD_AFTER {
                        model = None;
                        s.out.log("info", "model unloaded");
                    }
                    let n = q.len();
                    s.status.update(|b| {
                        b.queued = n;
                        b.working = None;
                        b.paused = fullscreen && n > 0;
                    });
                    q = s.wake.wait_timeout(q, Duration::from_secs(1)).unwrap().0;
                }
            };
            let queued = s.queue.lock().unwrap().len();
            s.status.update(|b| {
                b.queued = queued;
                b.working = Some(0.0);
                b.paused = false;
            });
            let started = Instant::now();
            match self.align(&job, &mut model) {
                Ok(lines) => {
                    let n = lines.len();
                    s.out.notify("word_times", json!({ "key": job.key, "lyrics_hash": job.hash, "lines": lines }));
                    let (secs, sound) = (started.elapsed().as_secs_f64(), job.audio.len() as f64 / SAMPLE_RATE as f64);
                    s.out.log("info", format!("song aligned: {n} of {} lines, {secs:.0} s for {sound:.0} s of sound", job.lines.len()));
                    s.status.update(|b| b.aligned += 1);
                }
                Err(e) => {
                    s.out.log("error", format!("model: {e}"));
                    s.status.update(|b| b.problem = Some(Problem::Model(e)));
                    s.ready.store(false, Ordering::SeqCst);
                    model = None;
                }
            }
            idle = Instant::now();
        }
    }

    /// Word times of a job's lines; pauses between lines while a fullscreen app is in front.
    fn align(&self, job: &Job, model: &mut Option<Model>) -> Result<Vec<serde_json::Value>, String> {
        let s = &self.0;
        let per_ms = SAMPLE_RATE as usize / 1000;
        let mut lines = Vec::new();
        for (i, line) in job.lines.iter().enumerate() {
            let mut paused_at = None;
            while s.fullscreen.load(Ordering::SeqCst) {
                let since = *paused_at.get_or_insert_with(Instant::now);
                s.status.update(|b| b.paused = true);
                if model.is_some() && since.elapsed() > UNLOAD_AFTER {
                    *model = None;
                    s.out.log("info", "model unloaded while paused");
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            s.status.update(|b| {
                b.paused = false;
                b.working = Some(i as f64 / job.lines.len() as f64);
            });
            if model.is_none() {
                let vocab = std::fs::read_to_string(model::vocab(&s.model_dir)).map_err(|e| e.to_string())?;
                *model = Some(Model::load(&model::onnx(&s.model_dir), &vocab, THREADS)?);
            }
            let m = model.as_mut().unwrap();
            let from = (line.window.0.saturating_sub(job.start_ms) as usize * per_ms).min(job.audio.len());
            let to = (line.window.1.saturating_sub(job.start_ms) as usize * per_ms).min(job.audio.len());
            let audio: Vec<f32> = job.audio[from..to].iter().map(|&x| f32::from(x) / 32768.0).collect();
            let window_ms = (job.start_ms as usize + from / per_ms) as f64;
            if let Some(words) = m.line(&audio, window_ms, line.start_ms, &line.text, &line.units)? {
                lines.push(json!({ "index": line.index, "words": words }));
            }
        }
        Ok(lines)
    }
}
