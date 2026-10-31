//! Works through the songs without lyrics in the background (PLAN-plugins-2.md 4): one
//! computation at a time across the audio plugins, only while no fullscreen app is in front,
//! two threads, the model loaded only while there is work. Per song it computes the model's
//! output for the whole song once, then tries in this order: lyrics without timestamps aligned
//! over the song; a broader LRCLIB search whose best candidate by text is aligned; the
//! recognition itself, when it looks sure enough.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use lyrichub_align::align::{Model, piece_count, sustain};
use lyrichub_align::capture::RATE;
use lyrichub_align::features::SAMPLE_RATE;
use lyrichub_align::lock::ComputeLock;
use lyrichub_align::model;
use lyrichub_align::recognize::{RecWord, align_timed, align_whole, confidence, greedy_words, line_text, lines_from_words, match_share};
use lyrichub_align::recorder::RawJob;
use lyrichub_align::resample::resample;
use lyrichub_align::rpc::Out;
use lyrichub_align::status::{Problem, Status};
use serde_json::{Value, json};

use crate::lrclib;

const THREADS: usize = 2;
const QUEUE_BYTES: usize = 256 * 1024 * 1024;
/// A line appears this long before its first word.
const LEAD_MS: u32 = 150;
/// A candidate counts when this share of the recognized words occurs in its lyrics.
const MATCH_MIN: f64 = 0.45;
/// Recognized lyrics appear only from this confidence and this many words on. On 12 JamendoLyrics
/// songs every one from 0.84 on had at most 15 % of its characters wrong; the four with more than
/// a quarter wrong were all below 0.83 (docs/plugin-recognize.md).
const CONFIDENCE_MIN: f32 = 0.84;
const MIN_WORDS: usize = 12;
/// Candidates must be this close to the song's length.
const LENGTH_SLACK_S: f64 = 10.0;

/// What LyricHub asked for: the song and, if LRCLIB had them, lyrics without timestamps.
#[derive(Clone, Debug)]
pub struct Request {
    pub key: String,
    pub title: String,
    pub duration_ms: u32,
    pub plain: Option<String>,
}

struct Job {
    req: Request,
    audio: Vec<i16>,
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
    pub fn add(&self, req: Request, raw: RawJob) {
        let me = self.clone();
        std::thread::spawn(move || {
            let x: Vec<f32> = raw.audio.iter().map(|&s| f32::from(s) / 32768.0).collect();
            let audio: Vec<i16> = resample(&x, RATE, SAMPLE_RATE).iter().map(|s| (s * 32768.0).round().clamp(-32768.0, 32767.0) as i16).collect();
            let mut q = me.0.queue.lock().unwrap();
            let mut bytes: usize = q.iter().map(|j| j.audio.len() * 2).sum();
            while !q.is_empty() && bytes + audio.len() * 2 > QUEUE_BYTES {
                bytes -= q.pop_front().map_or(0, |j| j.audio.len() * 2);
                me.0.out.log("info", "queue full: the oldest song was dropped");
            }
            me.0.out.log("info", format!("song queued: {} s of sound", audio.len() / SAMPLE_RATE as usize));
            q.push_back(Job { req, audio });
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
        loop {
            let job = {
                let mut q = s.queue.lock().unwrap();
                loop {
                    let fullscreen = s.fullscreen.load(Ordering::SeqCst);
                    if s.ready.load(Ordering::SeqCst)
                        && !fullscreen
                        && let Some(job) = q.pop_front()
                    {
                        break job;
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
            // The search needs no model: it runs before the lock, so a slow LRCLIB never holds up
            // the other audio plugin.
            let candidates = lrclib::search(&job.req.title);
            // Without a search, recognized lyrics could hide real ones for good (the cache keeps
            // them); the song waits for the next start of the plugin instead.
            if let Err(e) = &candidates
                && job.req.plain.is_none()
            {
                s.out.log("warn", format!("search failed, the song is left for the next start: {e}"));
                continue;
            }
            let _lock = loop {
                if let Some(lock) = ComputeLock::acquire(1000) {
                    break lock;
                }
                s.status.update(|b| b.waiting = true);
            };
            let queued = s.queue.lock().unwrap().len();
            s.status.update(|b| {
                b.queued = queued;
                b.working = Some(0.0);
                b.paused = false;
                b.waiting = false;
            });
            let started = Instant::now();
            match self.process(&job, &candidates, &mut model) {
                Ok(Some((how, params))) => {
                    s.out.notify("lyrics_found", params);
                    s.out.log("info", format!("lyrics sent ({how}), {:.0} s for {} s of sound", started.elapsed().as_secs_f64(), job.audio.len() / SAMPLE_RATE as usize));
                    s.status.update(|b| b.aligned += 1);
                }
                Ok(None) => s.out.log("info", "no lyrics sent: nothing fitting found, and the recognition was not sure enough or held back"),
                Err(e) => {
                    s.out.log("error", format!("model: {e}"));
                    s.status.update(|b| b.problem = Some(Problem::Model(e)));
                    s.ready.store(false, Ordering::SeqCst);
                    model = None;
                }
            }
            // Nothing else waits: the model goes before the lock does (see Align's worker).
            if s.queue.lock().unwrap().is_empty() && model.take().is_some() {
                s.out.log("info", "model unloaded");
            }
        }
    }

    /// Lyrics for one song: `(how, lyrics_found params)`, or `None`.
    fn process(&self, job: &Job, candidates: &Result<Vec<lrclib::Candidate>, String>, model: &mut Option<Model>) -> Result<Option<(&'static str, Value)>, String> {
        let s = &self.0;
        if model.is_none() {
            let vocab = std::fs::read_to_string(model::vocab(&s.model_dir)).map_err(|e| e.to_string())?;
            *model = Some(Model::load(&model::onnx(&s.model_dir), &vocab, THREADS)?);
        }
        let m = model.as_mut().unwrap();
        let x: Vec<f32> = job.audio.iter().map(|&v| f32::from(v) / 32768.0).collect();
        let total = piece_count(x.len()).max(1);
        let mut done = 0;
        let between = || {
            while s.fullscreen.load(Ordering::SeqCst) {
                s.status.update(|b| b.paused = true);
                std::thread::sleep(Duration::from_secs(1));
            }
            s.status.update(|b| {
                b.paused = false;
                b.working = Some(0.9 * done as f64 / total as f64);
            });
            done += 1;
        };
        let Some((logp, frames, v)) = m.logp_paced(&x, between)? else { return Ok(None) };
        let frame_ms = x.len() as f64 * 1000.0 / SAMPLE_RATE as f64 / frames as f64;
        let words = greedy_words(&logp, frames, v, &m.vocab, frame_ms);
        let conf = confidence(&words);
        s.out.log("info", format!("recognized {} words, confidence {conf:.2}", words.len()));

        // 1. Lyrics without timestamps from LRCLIB: align them over the whole song.
        if let Some(plain) = job.req.plain.as_deref() {
            let lines: Vec<String> = plain.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect();
            if let Some(times) = align_whole(&logp, frames, v, &m.vocab, frame_ms, &lines) {
                let found = lines.into_iter().zip(times).filter_map(|(text, w)| w.map(|w| (w[0][0].saturating_sub(LEAD_MS), text, w))).collect();
                return Ok(Some(("aligned_plain", params(&job.req.key, "aligned_plain", None, found))));
            }
        }

        // 2. A broader search: the candidate whose text fits the recognition best.
        s.status.update(|b| b.working = Some(0.95));
        match candidates {
            Ok(candidates) => {
                let want_s = job.req.duration_ms as f64 / 1000.0;
                let best = candidates
                    .iter()
                    .filter(|c| want_s <= 0.0 || c.duration_s <= 0.0 || (c.duration_s - want_s).abs() <= LENGTH_SLACK_S)
                    .map(|c| (match_share(&words, &c.text()), c))
                    .max_by(|a, b| a.0.total_cmp(&b.0));
                s.out.log("info", format!("search: {} candidates, best match {:.2}", candidates.len(), best.as_ref().map_or(0.0, |b| b.0)));
                if let Some((share, c)) = best.filter(|b| b.0 >= MATCH_MIN) {
                    let found = if let Some(lrc) = &c.synced {
                        let lines = lrclib::parse_lrc(lrc);
                        let times = align_timed(&logp, frames, v, &m.vocab, frame_ms, &lines);
                        lines.into_iter().zip(times).map(|((start, text), w)| (start, text, w.unwrap_or_default())).collect()
                    } else {
                        let lines: Vec<String> = c.text().lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect();
                        match align_whole(&logp, frames, v, &m.vocab, frame_ms, &lines) {
                            Some(times) => lines.into_iter().zip(times).filter_map(|(t, w)| w.map(|w| (w[0][0].saturating_sub(LEAD_MS), t, w))).collect(),
                            None => Vec::new(),
                        }
                    };
                    if !found.is_empty() {
                        s.out.log("info", format!("found on LRCLIB with a match of {share:.2}"));
                        return Ok(Some(("lrclib", params(&job.req.key, "lrclib", Some(c.id), found))));
                    }
                }
            }
            Err(e) => {
                s.out.log("warn", format!("search failed, no recognized lyrics: {e}"));
                return Ok(None);
            }
        }

        // 3. The recognition itself, only when it looks sure enough.
        if conf < CONFIDENCE_MIN || words.len() < MIN_WORDS {
            return Ok(None);
        }
        let found = lines_from_words(&words)
            .iter()
            .map(|l| {
                let times: Vec<[u32; 2]> = l.iter().map(|w: &RecWord| [w.start_ms, w.end_ms]).collect();
                (l[0].start_ms.saturating_sub(LEAD_MS), line_text(l), sustain(times))
            })
            .collect();
        Ok(Some(("recognized", params(&job.req.key, "recognized", None, found))))
    }
}

/// `lyrics_found` for LyricHub; line starts never fall.
fn params(key: &str, how: &str, lrclib_id: Option<i64>, lines: Vec<(u32, String, Vec<[u32; 2]>)>) -> Value {
    let mut floor = 0;
    let lines: Vec<Value> = lines
        .into_iter()
        .map(|(start, text, words)| {
            floor = start.max(floor);
            json!({ "start_ms": floor, "text": text, "words": words })
        })
        .collect();
    json!({ "key": key, "source": how, "lrclib_id": lrclib_id, "lines": lines })
}
