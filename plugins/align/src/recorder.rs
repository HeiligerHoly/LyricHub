//! Puts captured sound where it belongs in the song (PLAN-plugins.md 5.1). LyricHub's playback
//! anchors give each packet its place on the lyrics clock (position minus offsets, the clock the
//! overlay shows lines by), so the word times line up with what the user sees. Pauses and seeks
//! leave gaps. When the song is over, the lines whose windows were heard become a job; the
//! others keep the estimate.

use std::collections::HashSet;

use crate::align::{PAD_MS, windows};
use crate::capture::RATE;
use crate::rpc::Out;

/// A packet within this much of where the last one ended continues it, so the timing jitter of
/// the anchors does not cut the sound into pieces.
const JOIN_MS: f64 = 30.0;
/// Coverage is tracked in blocks of this many samples (10 ms).
const BLOCK: usize = RATE as usize / 100;
/// A line is aligned when this share of its window was heard.
const LINE_HEARD: f64 = 0.95;
/// A song is aligned when this share of its lines can be (PLAN-plugins.md 5.1: 85 %).
const SONG_HEARD: f64 = 0.85;

/// Playback anchor from LyricHub.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub playing: bool,
    pub position_ms: f64,
    pub rate: f64,
    pub qpc_100ns: i64,
    pub offset_ms: f64,
}

impl Clock {
    /// Lyrics clock at a QPC time.
    fn lyrics_ms(&self, qpc_100ns: i64) -> f64 {
        self.position_ms + (qpc_100ns - self.qpc_100ns) as f64 / 10_000.0 * self.rate - self.offset_ms
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineIn {
    pub start_ms: u32,
    pub text: String,
    pub units: Vec<String>,
}

/// A heard song for the worker, still at the capture rate.
pub struct RawJob {
    pub key: String,
    pub hash: String,
    pub lines: Vec<JobLine>,
    /// Mono samples at `RATE` from `start_ms` on.
    pub audio: Vec<i16>,
    pub start_ms: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JobLine {
    pub index: usize,
    pub start_ms: u32,
    pub window: (u32, u32),
    pub text: String,
    pub units: Vec<String>,
}

struct Song {
    key: String,
    hash: String,
    /// The whole song counts, not line windows: a song without lyrics (Recognize).
    whole: bool,
    lines: Vec<LineIn>,
    windows: Vec<(u32, u32)>,
    end_ms: u32,
    buf: Vec<i16>,
    covered: Vec<bool>,
    /// Where the last packet ended in `buf`.
    cursor: Option<usize>,
}

#[derive(Default)]
pub struct Recorder {
    song: Option<Song>,
    clock: Option<Clock>,
    /// Songs already handed to the worker in this session (key, lyrics hash).
    done: HashSet<(String, String)>,
    /// Where to say that a song was left out; none in tests.
    log: Option<Out>,
}

impl Recorder {
    pub fn with_log(out: Out) -> Recorder {
        Recorder { log: Some(out), ..Recorder::default() }
    }

    /// A song that ended with too little of it heard: said in the log, so a missing result can
    /// be told apart from a failed one. Songs skipped right away stay quiet.
    fn left_out(&self, what: &str, share: f64) {
        if let Some(out) = self.log.as_ref().filter(|_| share > 0.0) {
            out.log("info", format!("song left out: {:.0} % of {what} heard, {:.0} % needed", share * 100.0, SONG_HEARD * 100.0));
        }
    }

    /// The song to record from now on. The same song again keeps what was heard; another one
    /// finishes the one before.
    pub fn set_song(&mut self, key: &str, hash: &str, lines: Vec<LineIn>, duration_ms: u32) -> Option<RawJob> {
        if self.song.as_ref().is_some_and(|s| s.key == key && s.hash == hash) {
            return None;
        }
        let job = self.finish();
        if self.done.contains(&(key.to_owned(), hash.to_owned())) || !lines.iter().any(|l| !l.units.is_empty()) {
            return job;
        }
        let starts: Vec<u32> = lines.iter().map(|l| l.start_ms).collect();
        let last = lines.last().map_or(0, |l| l.start_ms) + 15_000;
        let windows = windows(&starts, if duration_ms > 0 { duration_ms } else { last }, PAD_MS);
        // Recording ends with the last sung line; an empty line after it only marks its end.
        let end_ms = windows.iter().zip(&lines).filter(|(_, l)| !l.units.is_empty()).map(|(w, _)| w.1).max().unwrap_or(0);
        let samples = end_ms as usize * RATE as usize / 1000;
        self.song = Some(Song {
            key: key.to_owned(),
            hash: hash.to_owned(),
            whole: false,
            lines,
            windows,
            end_ms,
            buf: vec![0; samples],
            covered: vec![false; samples.div_ceil(BLOCK)],
            cursor: None,
        });
        job
    }

    /// Records a whole song without lyrics (PLAN-plugins-2.md 4.1): it counts when 85 % of it
    /// was heard, and the job holds all of it. Songs over 15 minutes are left out.
    pub fn set_whole_song(&mut self, key: &str, duration_ms: u32) -> Option<RawJob> {
        if self.song.as_ref().is_some_and(|s| s.key == key && s.whole) {
            return None;
        }
        let job = self.finish();
        if self.done.contains(&(key.to_owned(), String::new())) || duration_ms == 0 || duration_ms > 15 * 60_000 {
            return job;
        }
        let samples = duration_ms as usize * RATE as usize / 1000;
        self.song = Some(Song {
            key: key.to_owned(),
            hash: String::new(),
            whole: true,
            lines: Vec::new(),
            windows: vec![(0, duration_ms)],
            end_ms: duration_ms,
            buf: vec![0; samples],
            covered: vec![false; samples.div_ceil(BLOCK)],
            cursor: None,
        });
        job
    }

    /// Stops recording; returns the song if enough of it was heard.
    pub fn clear(&mut self) -> Option<RawJob> {
        self.finish()
    }

    pub fn recording(&self) -> bool {
        self.song.is_some()
    }

    /// Share of the song's line windows heard so far.
    pub fn heard(&self) -> f64 {
        let Some(s) = &self.song else { return 0.0 };
        let total: usize = s.covered.len().max(1);
        s.covered.iter().filter(|c| **c).count() as f64 / total as f64
    }

    pub fn set_clock(&mut self, clock: Clock) -> Option<RawJob> {
        // The write position carries on: a seek shows as a packet far from it (JOIN_MS), a pause
        // as `playing: false`. Resetting it at every anchor would cut the sound every 2 s.
        self.clock = Some(clock);
        // Past the last window: the song is over even if the player still runs.
        let past = self.song.as_ref().is_some_and(|s| clock.playing && clock.lyrics_ms(clock.qpc_100ns) > s.end_ms as f64 + 1000.0);
        if past { self.finish() } else { None }
    }

    /// A captured packet: QPC time of its first sample and mono samples at `RATE`.
    pub fn push(&mut self, qpc_100ns: i64, samples: &[f32]) -> Option<RawJob> {
        let (Some(clock), Some(song)) = (self.clock, self.song.as_mut()) else { return None };
        if !clock.playing || (clock.rate - 1.0).abs() > 0.01 {
            song.cursor = None;
            return None;
        }
        let ms = clock.lyrics_ms(qpc_100ns);
        if ms > song.end_ms as f64 + 1000.0 {
            return self.finish();
        }
        let wanted = ms * RATE as f64 / 1000.0;
        let join = JOIN_MS * RATE as f64 / 1000.0;
        let at = match song.cursor {
            Some(c) if (c as f64 - wanted).abs() <= join => c as isize,
            _ => wanted.round() as isize,
        };
        for (i, &s) in samples.iter().enumerate() {
            let pos = at + i as isize;
            if pos >= 0 && (pos as usize) < song.buf.len() {
                song.buf[pos as usize] = (s * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
            }
        }
        let (from, to) = (at.max(0) as usize, (at + samples.len() as isize).max(0) as usize);
        for b in from / BLOCK..to.div_ceil(BLOCK).min(song.covered.len()) {
            song.covered[b] = true;
        }
        song.cursor = Some(to);
        None
    }

    fn finish(&mut self) -> Option<RawJob> {
        let song = self.song.take()?;
        let per_ms = RATE as usize / 1000;
        if song.whole {
            let heard = song.covered.iter().filter(|c| **c).count() as f64 / song.covered.len().max(1) as f64;
            if heard < SONG_HEARD {
                self.left_out("it", heard);
                return None;
            }
            self.done.insert((song.key.clone(), String::new()));
            return Some(RawJob { key: song.key, hash: String::new(), lines: Vec::new(), audio: song.buf, start_ms: 0 });
        }
        let heard = |(a, b): (u32, u32)| {
            let (from, to) = (a as usize * per_ms / BLOCK, (b as usize * per_ms).div_ceil(BLOCK).min(song.covered.len()));
            if to <= from {
                return 0.0;
            }
            song.covered[from..to].iter().filter(|c| **c).count() as f64 / (to - from) as f64
        };
        let singing: Vec<usize> = (0..song.lines.len()).filter(|&i| !song.lines[i].units.is_empty()).collect();
        let lines: Vec<JobLine> = singing
            .iter()
            .filter(|&&i| heard(song.windows[i]) >= LINE_HEARD)
            .map(|&i| JobLine {
                index: i,
                start_ms: song.lines[i].start_ms,
                window: song.windows[i],
                text: song.lines[i].text.clone(),
                units: song.lines[i].units.clone(),
            })
            .collect();
        if singing.is_empty() || (lines.len() as f64) < SONG_HEARD * singing.len() as f64 {
            self.left_out("its lines", lines.len() as f64 / singing.len().max(1) as f64);
            return None;
        }
        self.done.insert((song.key.clone(), song.hash.clone()));
        let start_ms = lines.iter().map(|l| l.window.0).min().unwrap_or(0);
        let end_ms = lines.iter().map(|l| l.window.1).max().unwrap_or(0).min(song.end_ms);
        let audio = song.buf[start_ms as usize * per_ms..(end_ms as usize * per_ms).min(song.buf.len())].to_vec();
        Some(RawJob { key: song.key, hash: song.hash, lines, audio, start_ms })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> Vec<LineIn> {
        [(1000, "paper boats"), (4000, "drifting home"), (7000, "")]
            .iter()
            .map(|(s, t)| LineIn { start_ms: *s, text: t.to_string(), units: t.split_whitespace().map(str::to_owned).collect() })
            .collect()
    }

    fn clock(position_ms: f64, offset_ms: f64) -> Clock {
        Clock { playing: true, position_ms, rate: 1.0, qpc_100ns: 0, offset_ms }
    }

    /// Feeds `ms` of a constant signal in 10 ms packets, starting at the anchor.
    fn play(r: &mut Recorder, from_ms: u32, ms: u32, value: f32) -> Option<RawJob> {
        let mut job = None;
        for k in 0..ms / 10 {
            let qpc = (from_ms + k * 10) as i64 * 10_000;
            job = job.or(r.push(qpc, &vec![value; BLOCK]));
        }
        job
    }

    #[test]
    fn a_fully_heard_song_becomes_a_job() {
        let mut r = Recorder::default();
        assert!(r.set_song("k", "h", lines(), 30_000).is_none());
        r.set_clock(clock(0.0, 0.0));
        let job = play(&mut r, 0, 30_000, 0.25).expect("past the end finishes the song");
        assert_eq!(job.lines.iter().map(|l| l.index).collect::<Vec<_>>(), vec![0, 1], "the empty line is not aligned");
        assert_eq!(job.start_ms, 500);
        assert_eq!(job.audio.len(), (7500 - 500) * 48);
        assert!(job.audio.iter().all(|&s| s == 8192));
        assert!(!r.recording());
        assert!(r.set_song("k", "h", lines(), 30_000).is_none() && !r.recording(), "a song already handed over is not recorded again");
    }

    #[test]
    fn the_offset_moves_the_sound_on_the_lyrics_clock() {
        let mut r = Recorder::default();
        r.set_song("k", "h", lines(), 30_000);
        // Position 2000 with offset 500 is 1500 on the lyrics clock.
        r.set_clock(clock(2000.0, 500.0));
        r.push(0, &vec![0.5; 480]);
        let s = r.song.as_ref().unwrap();
        assert_eq!(s.buf[1500 * 48], 16384);
        assert_eq!(s.buf[1500 * 48 - 1], 0);
    }

    #[test]
    fn too_little_heard_gives_nothing() {
        let mut r = Recorder::default();
        r.set_song("k", "h", lines(), 30_000);
        r.set_clock(clock(0.0, 0.0));
        play(&mut r, 0, 4600, 0.1);
        assert!(r.clear().is_none(), "only the first of two lines was heard");
    }

    #[test]
    fn a_pause_records_nothing() {
        let mut r = Recorder::default();
        r.set_song("k", "h", lines(), 30_000);
        r.set_clock(Clock { playing: false, ..clock(0.0, 0.0) });
        play(&mut r, 0, 9000, 0.1);
        assert_eq!(r.heard(), 0.0);
    }

    #[test]
    fn jitter_joins_and_a_seek_jumps() {
        let mut r = Recorder::default();
        r.set_song("k", "h", lines(), 30_000);
        r.set_clock(clock(0.0, 0.0));
        r.push(0, &vec![0.1; 480]);
        // 5 ms late: continues at the cursor, also across a new anchor.
        r.set_clock(clock(0.0, 0.0));
        r.push(150_000, &vec![0.1; 480]);
        assert_eq!(r.song.as_ref().unwrap().cursor, Some(960));
        // 3 s later: a seek.
        r.push(30_000_000, &vec![0.1; 480]);
        assert_eq!(r.song.as_ref().unwrap().cursor, Some(3000 * 48 + 480));
    }

    #[test]
    fn a_whole_song_counts_when_most_of_it_was_heard() {
        let mut r = Recorder::default();
        r.set_whole_song("w", 10_000);
        r.set_clock(clock(0.0, 0.0));
        play(&mut r, 0, 8000, 0.2);
        assert!(r.clear().is_none(), "80 % is not enough");
        r.set_whole_song("w2", 10_000);
        r.set_clock(clock(0.0, 0.0));
        let job = play(&mut r, 0, 11_100, 0.2).expect("past the end finishes it");
        assert!(job.lines.is_empty());
        assert_eq!((job.start_ms, job.audio.len()), (0, 10_000 * 48));
    }

    #[test]
    fn another_song_finishes_the_one_before() {
        let mut r = Recorder::default();
        r.set_song("a", "h", lines(), 30_000);
        r.set_clock(clock(0.0, 0.0));
        play(&mut r, 0, 7600, 0.1);
        let job = r.set_song("b", "h", lines(), 30_000).expect("both lines heard");
        assert_eq!(job.key, "a");
        assert!(r.recording());
    }
}
