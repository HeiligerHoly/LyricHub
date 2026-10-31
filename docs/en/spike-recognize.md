# Spike – Auto-Sync and Recognize (Q0 of PLAN-plugins-2.md)

> Date: 2026-10-30 · after v1.2.0 · Reference: [PLAN-plugins-2.md](../../PLAN-plugins-2.md) section 6 (German), [docs/en/spike-align.md](spike-align.md)
> Result: **Auto-Sync (M1) and aligning lyrics without timestamps (M3) clear the gate by far. The broader search (M4) finds candidates for 7 of 19 songs without lyrics. Recognition from the singing (M2) is close: the align model meets the error limit but is too slow; Whisper is more accurate and faster but needs too much memory and invents text in instrumental parts.**

## Setup

| Item | Value |
|---|---|
| Songs | **M1:** the 24 songs from P0. **M2 and M3:** the 4 songs of the P0 runtime measurement, one each in German, English, French and Spanish (13.7 min, 182 lines, 1100 words). |
| Pipeline | The align plugin's, in Rust; the sound goes through 16 bit as in the queue. Long windows go to the model in 14 s pieces with 2 s of context. |
| Candidate A | Align model (w2v-bert-2.0 music-lyrics CTC, int8) with greedy decoding, no language model |
| Candidate B | Whisper large-v3-turbo through faster-whisper, int8 on the CPU, language detection on, no VAD |
| PC | Ryzen 7 3800X, 32 GB RAM, no game running; "below normal" priority, 2 threads |
| Scripts | `plugins/align/examples/recognize.rs` (M2 A, M3), plus `m1_linestart.py`, `m2_whisper.py` and `m4_search.py` in the scratchpad. They print numbers only, no lyrics and no recognized text. |

## M1 – Line start for Auto-Sync

Measured: how far the first aligned word of each line is from the annotated start.

| | Median | p90 | at most 100 ms off |
|---|---|---|---|
| **Line start, 24 songs, 993 lines (P0 pipeline)** | **37 ms** | **138 ms** | 84.4 % |
| other words, for comparison | 37 ms | 115 ms | 87.0 % |
| Line start, 4 songs, Rust with pieces | 40 ms | 161 ms | 69.8 % |

By language (24 songs, median / p90): German 25 / 122 ms, English 42 / 183 ms, French 37 / 143 ms, Spanish 22 / 64 ms.

For comparison, the first real Spotify song with Align: its first word came 41 ms after the LRC file's stamp in the median, p10 99 ms before, p90 243 ms after. Auto-Sync moves such lines onto the singing.

The windows start 0.5 s before the LRC stamp. If someone sings earlier, the alignment does not find the first word properly. So Auto-Sync needs an earlier window in Q2.

## M2 – Recognition without the lyrics

Character and word error rate against the annotated lyrics, both texts normalised the same way (NFKC, lower case, only letters, digits and apostrophes):

| Language | A: characters / words | B: characters / words |
|---|---|---|
| German | 8.1 / 20.2 % | 7.0 / 13.9 % |
| English | 46.2 / 75.1 % | 44.2 / 69.3 % |
| French | 31.4 / 53.4 % | 32.0 / 50.0 % |
| Spanish | 20.0 / 41.2 % | 8.9 / 22.2 % |
| **all** | **23.9 / 43.9 %** | **21.4 / 36.4 %** |

| | A | B |
|---|---|---|
| Recognized text outside the singing (more than 1 s from any line) | 0.0 % | 4.3 % |
| Computing time with 2 threads | 1.86 × song length | 0.90 × song length |
| Peak memory of the check program | 1.50 GB | 2.15 GB (Python with PyTorch) |
| Language detected correctly | – | 4 of 4 |
| Extra download | none | about 1.6 GB |

- **Both stumble over the same English song**, already the hardest in P0. Both recognize German and Spanish readably.
- **A gives text and times in one run.** It is the same model output the alignment needs. With B the times only come from a second run with the align model: about 2.2–2.8 × song length in total, with two models one after the other.
- **The plugin would need its own route for B.** faster-whisper is Python. The plugin would need Whisper through ONNX or whisper.cpp; how much memory that leaves is not measured.
- **A was slower here than in P0** (1.86 instead of 1.39 × song length with pieces): for whole songs every 10 s piece goes to the model with 4 s of context. M4 was also searching the web during the run.

## M3 – Aligning lyrics without timestamps

The whole lyrics at once over the whole song, without line times. Measured: distance of line and word starts to the annotation.

| Language | Line start median / p90 | Word start median / p90 |
|---|---|---|
| German | 36 / 138 ms | 51 / 108 ms |
| English | 120 / 297 ms | 142 / 309 ms |
| French | 37 / 253 ms | 43 / 369 ms |
| Spanish | 22 / 63 ms | 27 / 70 ms |
| **all** | **41 / 197 ms** | **50 / 184 ms** |

That is almost as accurate as Align with line times, although not a single time was given here.

## M4 – Broader search on LRCLIB

For the songs without lyrics in the user's cache, LRCLIB was asked for the cleaned title only, as `track_name` and as `q`, without the artist. Only hits were counted, no names:

| Songs (of 19) | Count |
|---|---|
| with any hit | 11 |
| with synced lyrics | 10 |
| with synced lyrics and ±3 s length | **7** |
| with synced lyrics and ±10 s length | 8 |
| of these by the same artist, ±3 s | 4 |

- **Lower bound:** LRCLIB was overloaded. Even after patient retries with the wait the server asked for, 8 of 38 requests failed. The numbers are therefore a lower bound.
- **Not all are real misses:** 6 of 20 entries in the miss cache expire in about a day. That is LyricHub's deadline when LRCLIB was overloaded ("give up for today"). These songs are missing only for now.
- **Candidates by the same artist:** 4 songs have a candidate by the same artist with a fitting length. Today's stages always ask with the artist. An extra "title only" stage with an artist check could find them without any recognition. For the other 3, the text comparison has to decide whether the candidate is the song heard.

## Against the gate

| Measurement | Gate | Result | |
|---|---|---|---|
| M1 line start | median ≤ 50 ms, p90 ≤ 200 ms | 37 / 138 ms | met |
| M2 recognition, A | CER ≤ 25 %, ≤ 1.6 × song length, ≤ 1.5 GB | 23.9 %, 1.86 ×, 1.50 GB | speed missed |
| M2 recognition, B | as above | 21.4 %, 0.90 ×, 2.15 GB | memory missed (Python) |
| M3 aligning lyrics | median ≤ 100 ms, p90 ≤ 500 ms | 41 / 197 ms | met |
| M4 search | report | candidates for 7 of 19 songs | candidates exist |

## Recommendation

- **Build Auto-Sync (Q2)**, with an earlier window for the line start.
- **Build Recognize (Q3)**, the safe routes first: broader search with text comparison, aligning real lyrics, and aligning lyrics without timestamps.
- **For showing recognized lyrics, candidate A.**
  - **For it:** it needs no second model and no second download. One run gives text and times, and A invents nothing in instrumental parts.
  - **Speed:** it can improve with less context per piece, 1 instead of 2 s. That costs measuring time only in Q3.
  - **Threshold:** recognized lyrics only appear when the recognition looks sure enough. The threshold is set on real songs in Q3. Below it, the compact card stays.
  - **Whisper** remains an option for later, if A's quality is not enough.
- **Independent of the plugins, in LyricHub's core:**
  - An LRCLIB stage "title only" with an artist check.
  - Under overload, no longer give up for a whole day, but try again after a short while.

## What this measurement does not show

- **Small sample for M2 and M3:** only 4 songs. The spread between languages is large; one song can shift the picture.
- **Real Spotify songs:** densely produced pop and rap may be harder than Jamendo's CC music.
- **Japanese and Korean:** not measured.
- **Text comparison in the search:** whether it reliably picks the right candidate needs the sound of the real misses. That only works with the plugin.
- **Whisper's memory inside the plugin:** not measured.
