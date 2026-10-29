//! Memory of the model per window length, with and without handing the memory back after each
//! run. Uses synthetic noise; the content does not matter for memory.
//!
//!   cargo run --release --example memory -- <model dir> <threads> <shrink 0|1>

use std::path::Path;
use std::time::Instant;

use lyrichub_align::align::Model;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = Path::new(&args[0]);
    let threads: usize = args[1].parse().unwrap();
    let shrink = args[2] == "1";
    let vocab = std::fs::read_to_string(dir.join("vocab.json")).unwrap();
    let mut model = Model::load_with(&dir.join("aligner-int8.onnx"), &vocab, threads, shrink).unwrap();
    println!("loaded: {}", mem());
    let mut x: u64 = 1;
    for secs in [5usize, 10, 15, 20, 28, 5] {
        let audio: Vec<f32> = (0..secs * 16_000)
            .map(|_| {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((x >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.4
            })
            .collect();
        let t0 = Instant::now();
        model.logp(&audio).unwrap();
        println!("{secs:2} s window: {:5.1} s, {}", t0.elapsed().as_secs_f64(), mem());
    }
}

fn mem() -> String {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut c = PROCESS_MEMORY_COUNTERS { cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
    // SAFETY: counters of this process into a local of the given size.
    let _ = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    format!("working set {:4} MB, peak {:4} MB", c.WorkingSetSize / 1_048_576, c.PeakWorkingSetSize / 1_048_576)
}
