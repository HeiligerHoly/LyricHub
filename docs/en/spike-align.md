# Spike – word times from the audio (P0 of PLAN-plugins.md)

> Date: 2026-10-28 · after v1.0.1 · Reference: [PLAN-plugins.md](../../PLAN-plugins.md) section 6, [docs/en/word-timing.md](word-timing.md)
> Result: **Accuracy clearly meets the gate: median word start 37 instead of 129 ms, p90 119 instead of 713 ms (option B). Runtime misses it on the CPU: 1.5 × song length with 2 threads instead of at most 0.6. Capturing Spotify works.**

## Setup

| Item | Value |
|---|---|
| Songs | 24 from JamendoLyrics MultiLang, 6 each in German, English, French and Spanish (every third song per language, alphabetical, not picked by ear), 85.6 min of audio, 6,027 words |
| Simulation | as in `docs/en/word-timing.md`: each line as an LRC line stamped at its first word; the aligner gets the audio from 0.5 s before that stamp to 0.5 s after the next one |
| Option A | htdemucs (vocal separation, PyTorch) + the MMS forced aligner from torchaudio (fp32); text without accents, a–z |
| Option B | w2v-bert-2.0 "music-lyrics-ctc" as ONNX int8 (588 MB), directly on the mix; text in NFKC, lowercase |
| Alignment | CTC Viterbi (`torchaudio.functional.forced_align`), word start = first character of the word |
| PC | AMD Ryzen 7 3800X (8 cores, 16 threads), 32 GB RAM, GTX 1080 Ti; Python prototype at "below normal" priority |
| Script | `align.py` in the scratchpad, prints numbers only; audio, dataset and models are not in the repository |

## Accuracy

Distance between aligned and annotated word start, all 24 songs:

| | Estimate (since 2026-10-28) | Option A | Option B |
|---|---|---|---|
| Median | 129 ms | 37 ms | 37 ms |
| Mean | 268 ms | 145 ms | 86 ms |
| p90 | 713 ms | 152 ms | 119 ms |
| at most 150 ms off | 54.0 % | 89.9 % | 92.8 % |

By language (median / p90):

| Language | Estimate | Option A | Option B |
|---|---|---|---|
| German | 70 / 306 ms | 30 / 103 ms | 38 / 100 ms |
| English | 183 / 783 ms | 44 / 406 ms | 57 / 194 ms |
| French | 155 / 808 ms | 38 / 118 ms | 32 / 138 ms |
| Spanish | 152 / 829 ms | 36 / 88 ms | 27 / 77 ms |

Both options stumble over the same English song (median 124–143 ms). Option B has fewer gross outliers: its mean is 86 ms, A's is 145 ms.

**Confidence is no use as a filter.** With option B, even lines with low confidence (mean character probability) are more accurate than the estimate. Letting uncertain lines fall back to the estimate makes the result worse: from a threshold of 0.5, p90 rises to 190 ms, from 0.7 to 314 ms. With option A the effect is even stronger.

## Runtime and memory

Measured alone on the PC, without a second run, one song per language (13.7 min of audio):

| | 2 threads | 4 threads | Memory (process) |
|---|---|---|---|
| Option A (CPU) | – | 0.92 × song length, 0.67 of it separation | 3.6 GB |
| Option B (CPU, int8) | 1.50 × song length | 0.89 × song length | 3.5 GB |
| Option B (GPU through DirectML, fp16, 1 song) | 0.57 × song length, plus 1.9 CPU cores busy | – | 1.55 GB |

- **Where the time goes:** In option B the preprocessing takes under 1 s per song. Almost all the time is spent in ONNX Runtime itself, so a Rust plugin would hardly be faster on the CPU.
- **Window size:** Whole 30 s windows instead of the short per-line windows were even slower, 1.27 instead of 0.89 × song length with 4 threads.
- **ONNX Runtime memory alone:** The session with the int8 model takes about 620 MB, and up to about 1 GB more during computation. The rest of the 3.5 GB is Python with PyTorch and Transformers.
- **GPU measurement:** It ran while a game kept the graphics card about 70 % busy. Without the game the GPU would probably be faster; that is not measured.

## Capturing Spotify

Process loopback on Spotify's process tree (`scratchpad/captest`, Rust with the `windows` library), 10 s in RAM:

- **Captured:** 9.97 s in 997 packets, no silence.
- **Gap between two packets:** at most 33 ms.
- **Level:** RMS −36.7 dBFS, peak −25.4 dBFS. That is a real music signal and depends on the Spotify volume.
- **Game audio:** The Windows API guarantees that it stays out. This was not checked with a game running.

Nothing was saved.

## Against the gate

| Criterion | Target | Result | |
|---|---|---|---|
| Median word start | ≤ 70 ms | 37 ms (B), 37 ms (A) | met |
| p90 word start | ≤ 300 ms | 119 ms (B), 152 ms (A) | met |
| Runtime with 2 threads | ≤ 0.6 × song length | 1.50 (B, CPU); 0.57 (B, GPU, but 1.9 cores) | missed on the CPU |
| Memory | ≤ 1.5 GB | ONNX Runtime ~1.6 GB peak, prototype 3.5 GB | narrowly missed |
| Capture without game audio | works | works, game audio not checked | met |

**Recommendation:** option B. It has fewer outliers, is under MIT, needs no second model and handles kana and kanji without romaji. The computing load, however, does not fit running alongside games. On the CPU, one hour of music takes about 0.9 hours of computing with 4 threads.

This is only feasible if the plugin computes solely when no fullscreen game is running and collects the songs in RAM until then. At 16 kHz mono that is about 1.9 MB per minute, around 230 MB for 2 hours of music. Whether P1–P4 are built under this condition is the user's decision; by the plan's rules, the gate is not met.

## What this measurement does not show

- **Real Spotify songs:** Accuracy is measured on CC music from Jamendo. Well-known pop songs with dense production, or rap, may do worse.
- **Japanese:** Option B knows kana and kanji, but this is not measured. The dataset has no Japanese songs.
- **Rust and quantized models for option A:** Option A's times are for fp32 in PyTorch. PotLyrics reports about 0.45 × song length with 8 threads for int8 ONNX.
- **GPU without a game, and the effect on the game:** Whether the game stuttered during the runs is not measured.
