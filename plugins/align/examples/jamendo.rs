//! P3 check against P0 (PLAN-plugins.md 6): runs the plugin's pipeline over JamendoLyrics songs
//! the way the P0 prototype did (one window per line, stamped at its first word) and compares
//! the error of the word starts with a P0 result file. Prints numbers only, never lyrics.
//!
//!   cargo run --release --example jamendo -- <jamendo> <raw> <model> <p0 results> <threads> [--float]
//!
//! `<raw>` holds `songs.txt` (song and language per line), `<song>.f32` (mono float32) and
//! `<song>.sr` from the scratchpad; audio and dataset stay out of the repository. Without
//! `--float` the audio goes through 16-bit samples, as in the plugin's queue.

use std::path::Path;
use std::time::Instant;

use lyrichub_align::align::{Model, windows};
use lyrichub_align::features::SAMPLE_RATE;
use lyrichub_align::resample::resample;
use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [jamendo, raw, model_dir, p0, threads] = &args[..5] else { panic!("see the header for the arguments") };
    let float = args.iter().any(|a| a == "--float");
    let (jamendo, raw, model_dir) = (Path::new(jamendo), Path::new(raw), Path::new(model_dir));
    let p0: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(p0).unwrap()).unwrap();
    let vocab = std::fs::read_to_string(model_dir.join("vocab.json")).unwrap();
    let mut model = Model::load(&model_dir.join("aligner-int8.onnx"), &vocab, threads.parse().unwrap()).unwrap();

    let (mut all, mut all_p0, mut audio_s, mut work_s) = (Vec::new(), Vec::new(), 0.0, 0.0);
    let mut results = Vec::new();
    for entry in std::fs::read_to_string(raw.join("songs.txt")).unwrap().lines() {
        let (name, lang) = entry.split_once('\t').unwrap();
        let lines = load_lines(jamendo, name);
        let bytes = std::fs::read(raw.join(format!("{name}.f32"))).unwrap();
        let audio: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        let sr: u32 = std::fs::read_to_string(raw.join(format!("{name}.sr"))).unwrap().trim().parse().unwrap();

        let t0 = Instant::now();
        let mut x = resample(&audio, sr, SAMPLE_RATE);
        if !float {
            // The plugin keeps 16-bit samples in its queue.
            x.iter_mut().for_each(|s| *s = (*s * 32768.0).round().clamp(-32768.0, 32767.0) / 32768.0);
        }
        let per_ms = SAMPLE_RATE as usize / 1000;
        let dur_ms = (x.len() / per_ms) as u32;
        let stamps: Vec<u32> = lines.iter().map(|l| (l[0].1 * 1000.0).round() as u32).collect();
        let mut errors = Vec::new();
        for ((line, (from, to)), &stamp) in lines.iter().zip(windows(&stamps, dur_ms)).zip(&stamps) {
            let seg = &x[from as usize * per_ms..(to as usize * per_ms).min(x.len())];
            let units: Vec<String> = line.iter().map(|(w, _)| w.clone()).collect();
            let text = units.join(" ");
            let starts: Vec<f64> = match model.line(seg, from as f64, stamp, &text, &units).unwrap() {
                Some(words) => words.iter().map(|w| w[0] as f64).collect(),
                None => vec![stamp as f64; units.len()],
            };
            errors.extend(starts.iter().zip(line).map(|(s, (_, truth))| (s - truth * 1000.0).abs()));
        }
        let secs = t0.elapsed().as_secs_f64();
        let dur = audio.len() as f64 / sr as f64;
        let theirs: Vec<f64> = p0
            .iter()
            .find(|r| r["song"] == name)
            .map(|r| r["errors"].as_array().unwrap().iter().map(|e| e.as_f64().unwrap()).collect())
            .unwrap_or_default();
        println!(
            "{} {dur:6.1} s  rtf {:.2}  median {:4.0} ms (P0 {:4.0})  p90 {:4.0} ms (P0 {:4.0})",
            &lang[..2],
            secs / dur,
            quantile(&errors, 0.5),
            quantile(&theirs, 0.5),
            quantile(&errors, 0.9),
            quantile(&theirs, 0.9)
        );
        audio_s += dur;
        work_s += secs;
        all.extend(&errors);
        all_p0.extend(theirs);
        results.push(serde_json::json!({ "song": name, "lang": lang, "dur": dur, "align_s": secs, "errors": errors }));
    }
    println!(
        "all: median {:.0} ms (P0 {:.0}), p90 {:.0} ms (P0 {:.0}), within 150 ms {:.1} % (P0 {:.1} %), {} words",
        quantile(&all, 0.5),
        quantile(&all_p0, 0.5),
        quantile(&all, 0.9),
        quantile(&all_p0, 0.9),
        within(&all, 150.0),
        within(&all_p0, 150.0),
        all.len()
    );
    println!("audio {:.1} min, work {:.1} min, rtf {:.2}, peak memory {:.2} GB", audio_s / 60.0, work_s / 60.0, work_s / audio_s, peak_gb());
    let out = raw.join(format!("results_rust_{threads}{}.json", if float { "_float" } else { "" }));
    std::fs::write(out, serde_json::to_string(&results).unwrap()).unwrap();
}

/// Lines of a song: its words with their annotated starts in seconds.
fn load_lines(jamendo: &Path, name: &str) -> Vec<Vec<(String, f64)>> {
    let words: Vec<String> = std::fs::read_to_string(jamendo.join(format!("lyrics/{name}.words.txt")))
        .unwrap()
        .split('\n')
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect();
    let csv = std::fs::read_to_string(jamendo.join(format!("annotations/words/{name}.csv"))).unwrap();
    let (mut lines, mut cur) = (Vec::new(), Vec::new());
    for (word, row) in words.into_iter().zip(csv.lines().skip(1)) {
        let cols: Vec<&str> = row.split(',').collect();
        cur.push((word, cols[0].parse::<f64>().unwrap()));
        if cols[2].trim() != "nan" {
            lines.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// As in P0: the value at index p·(n−1) of the sorted list.
fn quantile(x: &[f64], p: f64) -> f64 {
    if x.is_empty() {
        return f64::NAN;
    }
    let mut s = x.to_vec();
    s.sort_by(f64::total_cmp);
    s[(p * (s.len() - 1) as f64) as usize]
}

fn within(x: &[f64], ms: f64) -> f64 {
    100.0 * x.iter().filter(|&&e| e <= ms).count() as f64 / x.len().max(1) as f64
}

fn peak_gb() -> f64 {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut c = PROCESS_MEMORY_COUNTERS { cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
    // SAFETY: counters of this process into a local of the given size.
    let _ = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    c.PeakWorkingSetSize as f64 / 1e9
}
