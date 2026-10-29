# Align plugin (P3 of PLAN-plugins.md)

> Date: 2026-10-29 · after v1.1.0 · Reference: [PLAN-plugins.md](../../PLAN-plugins.md) sections 5 and 6 (German), [docs/en/spike-align.md](spike-align.md), [docs/en/plugin-host.md](plugin-host.md)
> Result: **lyrichub-align delivers real word times for Spotify songs. The Rust pipeline matches the Python prototype from P0 (median 48 instead of 49 ms). Idle it takes 31 MB, capturing 0.03 % CPU, computing 2 cores and under 1.5 GB, and while a fullscreen game runs it does not compute.**

## What is built

`plugins/align/` is a Cargo package of its own. It builds a single `lyrichub-align.exe` (24 MB, 8.4 MB as a ZIP). ONNX Runtime is built in, so no DLL is needed.

| Part | Implementation | Code |
|---|---|---|
| Capture | Process loopback on the player's process tree, 48 kHz, 16 bit, stereo mixed down to mono. Every packet carries its QueryPerformanceCounter time. Runs only while a song with estimated word times plays. | `capture.rs` |
| Placement | LyricHub's `playback` anchors put each packet on the lyrics clock (position minus offset), the clock the overlay shows lines by. A packet within 30 ms of where the last one ended continues it seamlessly. Seeks and pauses leave gaps. A line is aligned when 95 % of its window was heard, a song when that holds for 85 % of its lines. | `recorder.rs` |
| Queue | Heard songs at 16 kHz and 16 bit, at most 256 MB (a good two hours of music); then the oldest goes. The sound stays in memory. | `worker.rs` |
| Alignment | As in P0: one window per line from 0.5 s before its start to 0.5 s after the next one starts, at most 28 s. Windows over 14 s go to the model in pieces with 2 s of context each. Inside are the SeamlessM4T features (ported), the w2v-bert-2.0 music-lyrics CTC model (int8, 2 threads) and the CTC Viterbi path (ported from torchaudio). A word starts with its first character. A word lasts until the next one, unless more than 1 s of pause lies between them. | `features.rs`, `align.rs`, `ctc.rs`, `text.rs`, `resample.rs` |
| Schedule | It computes only while no fullscreen window is in front (LyricHub's `screen` notification). The pause applies between two lines. The model (0.6 GB) is loaded only when there is work and unloaded after 60 s without any. | `worker.rs` |
| Model | On first start from huggingface.co, pinned to a revision, resumed after an interruption and checked by SHA-256, into `%LOCALAPPDATA%\LyricHub\plugins\lyrichub-align\model` (588 MB). | `model.rs` |
| Status | One line on the plugin page, for example "Downloading the model: 34 %", "Listening", "Aligning a song: 45 %" or "Paused while a fullscreen game runs", with a progress bar. | `status.rs` |

**Changes to the host**

- `playback` now carries `offset_ms`.
- `lyrics` now carries `word_source`. The plugin does not capture songs whose word times are already measured.
- New notification `screen` with `fullscreen`, sent when a fullscreen window comes to the front or leaves it.
- The last line of a song may carry words up to 30 s after its start.
- A plugin that just ended can be replaced and removed even while Windows still holds its exe for a moment. The old folder is moved aside whole first.

## Checks

### Unit tests (`cargo test` in `plugins/align`)

29 tests:

- **Reference values:** resampler, features and CTC path against values from transformers and torchaudio. `tests/make_golden.py` makes the values from synthetic sound.
- **Text:** normalisation including the Hangul decomposition.
- **Windows:** windows, pieces and word times.
- **Recorder:** coverage, offset, pause, seeks and jitter.
- **Other:** status lines and model files.

One more test downloads the vocabulary and resumes an interrupted download. It needs the network, so it only runs with `--ignored`.

### Comparison with P0

`examples/jamendo.rs` runs the plugin pipeline like the prototype over the four P0 songs, one each in German, English, French and Spanish (13.7 min). The sound goes through 16 bit, as in the queue. The result is the distance between the aligned and the annotated word start, with 2 threads:

| Language | Median (P0) | p90 (P0) |
|---|---|---|
| German | 44 ms (44) | 103 ms (103) |
| English | 145 ms (143) | 659 ms (659) |
| French | 45 ms (46) | 683 ms (694) |
| Spanish | 25 ms (26) | 76 ms (76) |
| **all** | **48 ms (49)** | **264 ms (244)** |

The table shows the windows whole, as in P0, computing time 1.55 × the song length. With pieces for windows over 14 s (see Memory) the median stays at 48 ms. p90 rises to 271 ms, only the English song from 659 to 751 ms. 82.8 % of the words are at most 150 ms off (P0: 83.8 %). The computing time falls to 1.39 × the song length and the check program's peak memory from 3.12 to 1.33 GB.

The requirement was at most 10 ms of difference in the median.

### Memory

The model takes 0.6 GB once loaded. On top of that comes memory that grows with the square of the window length. Measured with synthetic noise, 2 threads:

| Window | Time | Peak, whole | Peak in 14 s pieces |
|---|---|---|---|
| 5 s | 6.4 s | 686 MB | 686 MB |
| 10 s | 13.8 s | 888 MB | 888 MB |
| 15 s | 21–26 s | 1268 MB | 1034 MB |
| 20 s | 30–34 s | 1732 MB | 1039 MB |
| 28 s | 45–50 s | 2808 MB | 1161 MB |

After every run ONNX Runtime hands the memory back (`memory.enable_memory_arena_shrinkage`). Without that, the peak of the longest window would stay taken. Together with the 256 MB queue the plugin stays under 1.5 GB.

### In the real app (Spotify, 2026-10-29)

1. **Install:** ZIP installed and switched on. The model arrived in under a minute, and its checksum matched. Idle the plugin takes 31 MB.
2. **Capture:** 0.03 % of the whole PC's CPU (0.55 % of one core), 16 MB.
3. **First song:** 193 s of sound, 68 lines. The plugin computed for 217 s, including a 15 s fullscreen pause. All 68 lines reached LyricHub as valid.
4. **Computing:** 1.94 cores at "below normal" priority, 710–775 MB, measured before the pieces.
5. **Fullscreen:** a borderless window over the whole screen brought the load to zero within 3 s, and the status line showed the pause. After it closed, the work went on at once.
6. **Second listen:** all 474 words of the song came from the plugin. In the overlay the words lit up one by one, at most 2 at a time, and no animation jumped back. The plugin did not capture the song a second time.
7. **Plausibility:** a line's first word starts 41 ms after the LRC line start in the median (10 % more than 99 ms before it, 10 % more than 243 ms after it). The words were only 81 ms long, though (median), because CTC marks each character only briefly. Since then a word lasts until the next one.

**Found and fixed:**

- **Crash on song change:** the plugin crashed every time the capture ended. The cause was a `PROPVARIANT` pointing at a local variable: on cleanup Windows freed that memory, and the heap was corrupted.
- **Reinstall without effect:** after the crash, Windows Error Reporting held the old exe locked for a few seconds. The host could not replace it on reinstall. Now it waits up to 5 s.

## Limits

- **Output latency:** the capture comes before the output device. The words therefore light up early by that device's latency. With a cable that is about 10–40 ms, with Bluetooth clearly more.
- **Only songs heard through:** only songs heard to 85 % are aligned. Lines in gaps stay estimated, also on the next listen.
- **Offset changed afterwards:** the word times depend on the offset that applied during the capture. Changing the song offset afterwards shifts the words against the singing.
- **Japanese and Korean:** the model knows kana, kanji and jamo, but this is not measured.
- **Computing time:** 1.1–1.6 × the song length with 2 threads. Someone who listens a lot and games a lot collects songs in the queue. After a good two hours the oldest goes.
