//! Q0 M2 and M3 (PLAN-plugins-2.md) on JamendoLyrics songs, with the plugin's own pipeline:
//! the model's output for the whole song (in pieces), then
//! - M2: recognition without the lyrics (greedy CTC decoding), character and word error rate
//!   against the annotated lyrics, and how much of the output falls outside the sung parts;
//! - M3: the whole lyrics without timestamps aligned over the whole song at once, error of the
//!   line and word starts.
//!
//! Prints numbers only, never lyrics or recognized text.
//!
//!   cargo run --release --example recognize -- <jamendo> <raw> <model> <threads>

use std::path::Path;
use std::time::Instant;

use lyrichub_align::align::Model;
use lyrichub_align::features::SAMPLE_RATE;
use lyrichub_align::recognize::{confidence, greedy_words};
use lyrichub_align::resample::resample;
use lyrichub_align::text::{Vocab, normalize, targets};

struct Word {
    text: String,
    start_ms: f64,
    end_ms: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [jamendo, raw, model_dir, threads] = &args[..4] else { panic!("see the header for the arguments") };
    let (jamendo, raw, model_dir) = (Path::new(jamendo), Path::new(raw), Path::new(model_dir));
    let vocab_json = std::fs::read_to_string(model_dir.join("vocab.json")).unwrap();
    let vocab = Vocab::parse(&vocab_json).unwrap();
    let id2char = inverse_vocab(&vocab_json);
    let mut model = Model::load(&model_dir.join("aligner-int8.onnx"), &vocab_json, threads.parse().unwrap()).unwrap();

    let (mut audio_s, mut work_s) = (0.0, 0.0);
    let (mut cer_e, mut cer_n, mut wer_e, mut wer_n) = (0usize, 0usize, 0usize, 0usize);
    let (mut outside, mut emitted) = (0usize, 0usize);
    let (mut all_line, mut all_word) = (Vec::new(), Vec::new());
    for entry in std::fs::read_to_string(raw.join("songs.txt")).unwrap().lines() {
        let (name, lang) = entry.split_once('\t').unwrap();
        let lines = load_lines(jamendo, name);
        let bytes = std::fs::read(raw.join(format!("{name}.f32"))).unwrap();
        let audio: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        let sr: u32 = std::fs::read_to_string(raw.join(format!("{name}.sr"))).unwrap().trim().parse().unwrap();

        let t0 = Instant::now();
        let mut x = resample(&audio, sr, SAMPLE_RATE);
        x.iter_mut().for_each(|s| *s = (*s * 32768.0).round().clamp(-32768.0, 32767.0) / 32768.0);
        let (logp, frames, v) = model.logp(&x).unwrap().unwrap();
        let frame_ms = x.len() as f64 * 1000.0 / SAMPLE_RATE as f64 / frames as f64;

        // M2: greedy decoding.
        let mut hyp = String::new();
        let mut prev = usize::MAX;
        let sung: Vec<(f64, f64)> = lines.iter().map(|l| (l[0].start_ms - 1000.0, l.last().unwrap().end_ms + 1000.0)).collect();
        let (mut song_out, mut song_emit) = (0usize, 0usize);
        for t in 0..frames {
            let row = &logp[t * v..(t + 1) * v];
            let id = row.iter().enumerate().fold((0, f32::NEG_INFINITY), |b, (i, &p)| if p > b.1 { (i, p) } else { b }).0;
            if id != prev && id != 0 {
                let c = id2char.get(id).cloned().unwrap_or_default();
                if c == "|" {
                    hyp.push(' ');
                } else {
                    hyp.push_str(&c);
                    song_emit += 1;
                    let ms = t as f64 * frame_ms;
                    if !sung.iter().any(|&(a, b)| ms >= a && ms <= b) {
                        song_out += 1;
                    }
                }
            }
            prev = id;
        }
        let reference: Vec<String> = lines.iter().flatten().map(|w| normalize(&w.text)).filter(|w| !w.is_empty()).collect();
        let hypothesis: Vec<String> = hyp.split_whitespace().map(normalize).filter(|w| !w.is_empty()).collect();
        let ref_chars: Vec<char> = reference.join(" ").chars().collect();
        let hyp_chars: Vec<char> = hypothesis.join(" ").chars().collect();
        let (ce, we) = (levenshtein(&ref_chars, &hyp_chars), levenshtein(&reference, &hypothesis));
        // The plugin's own decoding and its confidence (PLAN-plugins-2.md Q3: threshold).
        let rec = greedy_words(&logp, frames, v, &vocab, frame_ms);
        let conf = confidence(&rec);

        // M3: the whole lyrics over the whole song.
        let mut ids = Vec::new();
        let mut unit = Vec::new();
        let mut first_word = Vec::new();
        let mut base = 0;
        for line in &lines {
            let units: Vec<String> = line.iter().map(|w| w.text.clone()).collect();
            let t = targets(&units.join(" "), &units, &vocab);
            first_word.push(base);
            for (k, (&id, u)) in t.ids.iter().zip(&t.unit).enumerate() {
                if k == 0 && !ids.is_empty() {
                    continue; // the bar between two lines is already there
                }
                ids.push(id);
                unit.push(u.map(|u| u + base));
            }
            base += units.len();
        }
        let words: Vec<&Word> = lines.iter().flatten().collect();
        let mut starts = vec![None; words.len()];
        if let Some(spans) = lyrichub_align::ctc::align(&logp, frames, v, &ids) {
            for (s, u) in spans.iter().zip(&unit) {
                if let Some(u) = u
                    && starts[*u].is_none()
                {
                    starts[*u] = Some(s.0 as f64 * frame_ms);
                }
            }
        }
        // A word without a known character takes the start of the one before.
        let mut last = 0.0;
        let starts: Vec<f64> = starts.iter().map(|s| { last = s.unwrap_or(last); last }).collect();
        let word_err: Vec<f64> = starts.iter().zip(&words).map(|(s, w)| (s - w.start_ms).abs()).collect();
        let line_err: Vec<f64> = first_word.iter().map(|&i| word_err[i]).collect();

        let secs = t0.elapsed().as_secs_f64();
        let dur = audio.len() as f64 / sr as f64;
        println!(
            "{} {dur:6.1} s  rtf {:.2}  conf {conf:.3} ({} words)  CER {:5.1} %  WER {:5.1} %  outside {:4.1} %  plain-sync line {:4.0}/{:5.0} ms  word {:4.0}/{:5.0} ms",
            &lang[..2],
            secs / dur,
            rec.len(),
            100.0 * ce as f64 / ref_chars.len().max(1) as f64,
            100.0 * we as f64 / reference.len().max(1) as f64,
            100.0 * song_out as f64 / song_emit.max(1) as f64,
            quantile(&line_err, 0.5),
            quantile(&line_err, 0.9),
            quantile(&word_err, 0.5),
            quantile(&word_err, 0.9),
        );
        audio_s += dur;
        work_s += secs;
        cer_e += ce;
        cer_n += ref_chars.len();
        wer_e += we;
        wer_n += reference.len();
        outside += song_out;
        emitted += song_emit;
        all_line.extend(line_err);
        all_word.extend(word_err);
    }
    println!(
        "all: CER {:.1} %  WER {:.1} %  outside {:.1} %  plain-sync line {:.0}/{:.0} ms  word {:.0}/{:.0} ms  ({} lines)",
        100.0 * cer_e as f64 / cer_n.max(1) as f64,
        100.0 * wer_e as f64 / wer_n.max(1) as f64,
        100.0 * outside as f64 / emitted.max(1) as f64,
        quantile(&all_line, 0.5),
        quantile(&all_line, 0.9),
        quantile(&all_word, 0.5),
        quantile(&all_word, 0.9),
        all_line.len()
    );
    println!("audio {:.1} min, work {:.1} min, rtf {:.2}, peak memory {:.2} GB", audio_s / 60.0, work_s / 60.0, work_s / audio_s, peak_gb());
}

fn inverse_vocab(json: &str) -> Vec<String> {
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(json).unwrap();
    let mut out = vec![String::new(); map.len()];
    for (k, v) in map {
        if let Some(i) = v.as_u64().map(|i| i as usize).filter(|&i| i < out.len()) {
            out[i] = k;
        }
    }
    out
}

/// Lines of a song with their words, starts and ends in ms.
fn load_lines(jamendo: &Path, name: &str) -> Vec<Vec<Word>> {
    let words: Vec<String> = std::fs::read_to_string(jamendo.join(format!("lyrics/{name}.words.txt")))
        .unwrap()
        .split('\n')
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect();
    let csv = std::fs::read_to_string(jamendo.join(format!("annotations/words/{name}.csv"))).unwrap();
    let (mut lines, mut cur) = (Vec::new(), Vec::new());
    for (text, row) in words.into_iter().zip(csv.lines().skip(1)) {
        let cols: Vec<&str> = row.split(',').collect();
        cur.push(Word { text, start_ms: cols[0].parse::<f64>().unwrap() * 1000.0, end_ms: cols[1].parse::<f64>().unwrap() * 1000.0 });
        if cols[2].trim() != "nan" {
            lines.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn levenshtein<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, x) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(x != y)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn quantile(x: &[f64], p: f64) -> f64 {
    if x.is_empty() {
        return f64::NAN;
    }
    let mut s = x.to_vec();
    s.sort_by(f64::total_cmp);
    s[(p * (s.len() - 1) as f64) as usize]
}

fn peak_gb() -> f64 {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut c = PROCESS_MEMORY_COUNTERS { cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
    // SAFETY: counters of this process into a local of the given size.
    let _ = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    c.PeakWorkingSetSize as f64 / 1e9
}
