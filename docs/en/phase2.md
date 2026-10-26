# Phase 2 – Lyrics and clock

*English translation of [phase2.md](../phase2.md).*

> Date: 2026-10-25 · Reference: [PLAN.md](../../PLAN.en.md) 10 (Phase 2), 3.3, 5.1–5.7, 7, 9.2–9.6
> Result: **Phase 2 complete** (tag `v0.2.0-phase2`). Listening test with ten songs by the user: the lines change in sync.

## What was built

| Area | Implementation | Files |
|---|---|---|
| LRC parser | Multiple timestamps per line, metadata, `[offset:]` (positive = earlier, LRC convention), Enhanced LRC word tags, BOM, CRLF/CR, 0–3 decimal places, `:` as a separator, unsorted and duplicate lines, section markers. Empty lines remain instrumental markers. For each line, the end of the fill; before long pauses it is estimated from the text length (5.6, item 2). | `src-tauri/src/lyrics/parser.rs` |
| Matcher | NFKC, lowercase, no punctuation; parenthetical additions and " - Remastered" only as a fallback stage; main artist; a duration difference over 8 s = a different edition; title with Jaro-Winkler ≥ 0.85. | `lyrics/matcher.rs` |
| LRCLIB client | Stages: get, get without album, normalized get, search by fields, free-text search. Staggered: stage 1 immediately; every 350 ms without a response, or after a failure, the next stage starts in parallel; the first matching hit with synced lyrics wins. User agent `LyricHub/<version> (Windows desktop lyrics overlay)`. | `lyrics/lrclib.rs` |
| Cache | SQLite `%APPDATA%/LyricHub/lyrichub.db` with the tables from 7.3 (song and device offsets prepared), failures 7 days, "permanently unreachable" 24 h. | `lyrics/cache.rs` |
| Lyrics worker | One thread, only the newest request counts, retries after 5 s, 30 s and 2 min. Logs contain line counts, never lyrics. | `lyrics/service.rs` |
| PlaybackClock | Following 5.3 and the spike rules: seek from 1.5 s, the update directly after a seek is adopted, corrections up to 300 ms via slew (at most ±10 %), larger ones as a jump, below 20 ms ignored, adopted directly while paused. Residuals are shifted after a correction so that nothing counts twice. UI-free; every decision is logged. | `src-tauri/src/sync/clock.rs` |
| Clock in the frontend | `clock.ts` advances the anchor with `performance.now()`, mirrors the slew and smooths out deviations below 300 ms over 300 ms. One timer per line. | `src/lib/overlay/clock.ts` |
| Karaoke fill | Two text layers; the accent layer is revealed via `transform`, driven by Web Animations with `steps()`, 30 steps per second of song time. The compositor animates without JavaScript per frame. | `KaraokeLine.svelte` |
| Layout "Karaoke" | New default: header row with cover, current line large, next line dimmed, fixed line heights. For songs with long lines, the box gets wider once per song (up to the monitor width minus a margin); only after that is the text scaled down. `lang="ja"` for lines with kana or kanji. Loading dots for at most 1.5 s, notices "No synced lyrics found" („Keine synchronen Lyrics gefunden“) and "Instrumental" for 4 s. | `layouts/Karaoke.svelte`, `Overlay.svelte` |
| Offset and nudge | Global offset in the settings, positive = lyrics later. Ctrl+Alt+, and Ctrl+Alt+. by ±100 ms, with Shift ±25 ms, toast in the overlay, also in the tray. | `ipc/mod.rs`, `hotkeys.rs`, `tray.rs` |
| Settings | Layout selection (also Ctrl+Alt+K and tray), background opacity, new "Sync" page („Synchronisation“) with the offset as a sentence ("earlier / later" („früher / später“)), nudge shortcuts, lyrics status, "Reload lyrics" („Lyrics neu laden“) and a crossfade hint. Diagnostics shows clock and lyrics. | `src/lib/settings/` |

## Definition of Done

| Criterion | Result | Evidence |
|---|---|---|
| Ten songs with synced lyrics follow along line by line (EN, DE, JA) | **met**: user: "the song lines all switch perfectly" („die Song-Zeilen switchen alle perfekt“), also for the French song | Table below |
| Nudge corrects visibly | Used by the user during the listening test on "Discord": four times +100 ms, then back to 0; each change went to the overlay as a new clock anchor. Feedback without complaints; an explicit statement about the shift is missing | Log `offset nudged` |
| Parser and clock tests green | **met**: 64 unit tests, of which parser 12, clock 11, LRCLIB 8, matcher 7, cache 3 | `cargo test` |
| `run-spike.ps1` green | plain 13/13, game-once 15/15, game-aggressive 12/14 (the two known worst-case findings from Phase 0) | `docs/spike-overlay-*.json` |
| CPU ≤ 2 % of all cores while the fill runs | **met**: 9.4 % of one core = **0.59 % of 16 cores** over a whole song with 60 lines | `measure-cpu.ps1 -Seconds 60` |
| Edit mode still intact | 13 of 13 | `edit-mode-test.ps1` |

### Listening test (user at the PC, wired)

| # | Song | Language | Lines | Stage | Search |
|---|---|---|---|---|---|
| 1 | MÄNNERGEFÜHLE – SALÒ | DE | 43 | get | 1249 ms |
| 2 | Jane! – The Long Faces | EN | 29 | get | 56 ms |
| 3 | Discord – The Living Tombstone | EN | 40 | get | 88 ms |
| 4 | Time Moves Slow – BADBADNOTGOOD | EN | 57 | get | 55 ms |
| 5 | Oddloop – frederic | JA (all 50 text lines with kana) | 54 | get without album | 468 ms |
| 6 | jester – Kenricx | EN | 40 | get without album | 1454 ms |
| 7 | You Got Jokes – Khantrast | DE | 59 | get without album | 410 ms |
| 8 | makkohatjedenliebaussersichselbst – makko | DE | 50 | search | 900 ms |
| 9 | Dernière danse – Indila | FR | 43 | get | 60 ms |
| 10 | WOLKE – Leftovers | DE | 36 | search | 1869 ms |

Also: "Ich will nur dass du weißt" (SDP) only has lyrics without timestamps on LRCLIB and shows the notice; "chess" (joyful) is detected as instrumental. Title changes appeared in the overlay after 90–180 ms. The offset stayed at 0: with wired output, no calibration was needed.

### Online lyrics search

Median about 0.4 s, three of twelve searches over 1.5 s (up to 1.9 s). The cause is LRCLIB: under load, every response takes about 1.1 s, and the same request returns 200 one time and 503 another time. LRCLIB currently answers unknown titles and empty searches with 503 "ServerOverloaded" instead of 404. Countermeasures: staggered stages (test: a first stage that is 2 s slow is overtaken after 350 ms), never cache a 503 as a failure, 24 h of rest after all retries. Lyrics come from the cache in about 3 ms.

## Findings from the listening test

- **Line changes:** in sync for all songs.
- **The yellow fill mostly does not match the sung word.** Expected: LRCLIB only delivers line timestamps. The fill runs linearly from the start of the line to the next line and knows neither breathing pauses nor the tempo within the line. Word-accurate highlighting is Phase 4 (PLAN.md 5.6): estimating word boundaries, and real word timings where LRCLIB has them (field `hasWordSync`, structured lines in the new field `lyricsfile`). Approach for Phase 4: calibrate the estimate against LRCLIB songs with word timings.
- **Long lines were cut off.** Fixed: per song, the box becomes as wide as the longest line.
- **Request: higher opacity.** Implemented: slider "Background opacity" („Deckkraft des Hintergrunds“).
- **Request: romaji for Japanese songs.** Listed in PLAN.md 13.6 as the first candidate for Phase 6; needs a dictionary with kanji readings.

## Deviations from the plan

- **Sign of the offset:** PLAN.md 5.2 says "positive = lyrics later", but the formula `display = clock + offset` would give the opposite. The sentence is what was implemented: `display = clock − offset`. The `[offset:]` tag follows the LRC convention (positive = earlier) and is already included in the times.
- **In Phase 2, the nudge target is the global offset.** The song offset as the default target, and switching between targets, come with Phase 3 (song offset persistence).
- **Event `clock`** additionally carries `slew_ms`: the slew is part of the anchor, so that backend and frontend compute the same piecewise model without timers. `lyrics` always carries `{ key, state, source, lines, lrc_offset_ms }`, even while loading.
- **Test fixtures with invented text** instead of "real LRC examples" (7.2) and recorded responses (11.2): song lyrics are protected by copyright. The fixtures reproduce the format and JSON structure of real files and responses.
- **Drift test:** a clock rate deviation of 0.1 % stays below 30 ms of error, 1 % below 100 ms. The goal "below 50 ms" therefore holds far beyond the measured 74 ppm.
- **New settings fields without a schema change:** `overlay.layout`, `appearance.plate_opacity`, `sync.global_offset_ms`. All have default values; files from Phase 1 load unchanged (test exists).
- **Layouts:** Karaoke is the default; "Bar" is labelled "Bar" („Leiste“) in the UI and shows the current line without fill. Stack and Minimal follow in Phase 4.
- **The fill color** is fixed at `#ffd36e`; the accent color from the cover comes in Phase 4.
