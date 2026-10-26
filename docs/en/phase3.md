# Phase 3 – Sync hardening

*English translation of [phase3.md](../phase3.md).*

> Date: 2026-10-25 · Reference: [PLAN.md](../../PLAN.en.md) 10 (Phase 3), 2, 5.4, 5.5, 5.7, 5.9, 11.3, 14.1
> Result: **Phase 3 built and measured** (tag `v0.3.0-phase3`). Median error 96 ms across English and German songs, 108 ms including the Japanese songs measured later. Seek 30 ms, nudge per song verified. At 289 ms (309 ms with Japanese), the p95 is above the target of 200 ms, see [sync-report.md](sync-report.md).

## What was built

| Area | Implementation | Files |
|---|---|---|
| Nudge per song | Ctrl+Alt+, and Ctrl+Alt+. (with Shift ±25 ms) change only the song offset of the current song, stored in `song_overrides`. On a song change, the new song's value applies immediately, 0 if it has no correction. If a song is later identified differently, its offset moves along with it. The global offset now only changes in the settings. Toast "Song offset +200 ms · Lyrics later" („Song-Offset +200 ms · Lyrics später“). | `sync/offsets.rs`, `app_state.rs`, `ipc/mod.rs`, `hotkeys.rs` |
| Moving away from the global nudge | Settings schema v2. The migration sets `sync.global_offset_ms` to 0 (user decision; the value came from Phase 2 nudges) and writes the file immediately. Unknown fields are preserved on save, for example `appearance.romaji` from the romaji branch. | `settings/schema.rs`, `settings/store.rs` |
| Source fusion (5.4) | One window of residuals per source, correction via the confidence-weighted median. Conflict rule: if sources differ by more than 400 ms over at least 5 samples, the one with the larger spread drops out and is shown as "deviating" („abweichend“) in the Sync Lab. Jumps shift the windows; seek, status changes and song changes clear them. With SMTC alone, nothing changes; tested with an artificial second source. | `sync/clock.rs` |
| Seek and pause | With `clock_applied`, the overlay reports when it has drawn the anchor of a seek; LyricHub logs "seek shown in overlay" with both latencies. The clock adopts seeks in the paused state directly. | `app_state.rs`, `Overlay.svelte` |
| Device offsets | Every 2 s, Core Audio delivers the default output device with name and bus. Every device has its own offset in `device_offsets`; a new device starts at 0, Bluetooth at +150 ms, and announces itself in the overlay (except for the very first device). Table on the "Sync" page („Synchronisation“). | `audio_devices.rs`, `lyrics/cache.rs` |
| Calibration wizard (5.7) | Uses the current song. The overlay shows the next line with a countdown bar; press Space five times at the audible onset. Outliers over 600 ms drop out; the median is suggested as the device offset. Space and Esc are global hotkeys only during a measurement. | `calibration.rs`, `sync/taps.rs`, `hotkeys.rs`, `Karaoke.svelte` |
| Tap-based sync log | The same mechanism without an end: Space at every audible line start. Error per tap, median, p95, bias and share under 100 ms, overall and per song; file `logs/sync-protocol-<unix>.json` without lyrics. | `calibration.rs`, `sync/taps.rs` |
| Sync Lab (5.9) | Diagnostics page: position reports of the last 60 s as deviation from the prediction, color and shape by clock decision (no correction, slew, jump, seek), plus taps and vertical markers for song changes and status; a table as an alternative. Live values: position, rate, remaining slew, all offsets, report cadence, age of the last report, last clock decision, deviating sources. "Open logs" („Logs öffnen“) and "Export diagnostics" („Diagnose exportieren“) (`logs/diagnostics-<unix>.json`). | `sync_lab.rs`, `SyncLab.svelte` |
| Special states (5.5) | Ads: title "Advertisement" or "Spotify", no artist, at most 40 s; the overlay hides. Podcast: over 20 min and no album; title only, no lyrics search. Spotify closed: overlay off, tray icon gray. Instrumental, no synced lyrics and pause (dim, hide) already existed. | `sources/types.rs`, `sources/smtc.rs`, `tray.rs`, `Overlay.svelte` |
| "Sync" page | Offset of this song with "Reset to 0" („Auf 0“), audio devices, calibration, sync log, global offset, lyrics status. | `SyncPage.svelte` |
| Test scripts | `seek-test.ps1` (seek latency until the overlay is drawn), `nudge-test.ps1` (nudge per song across a real song change). | `tools/sync/` |

## Definition of Done

| Criterion | Result | Evidence |
|---|---|---|
| Sync log 11.3, wired: median ≤ 100 ms | **met for English and German**: 96 ms (device offset +100 ms, 115 taps, 13 songs). With the two Japanese songs, measured with romaji after the tag: 108 ms (155 taps), because "Oddloop" is 144 ms early | [sync-report.md](sync-report.md) |
| p95 ≤ 200 ms | **not met**: 289 ms, 309 ms with Japanese. The cause is the shift and spread of the LRC files plus the spread of the taps, not the timing chain. No second round (user decision) | [sync-report.md](sync-report.md) |
| Seek test | **met**: median 30 ms, at most 34 ms from the seek to the drawn overlay (target 300 ms) | `tools/sync/seek-test.ps1` |
| Nudge per song applies on song change | **met**: 4 of 4, log excerpt in the report | `tools/sync/nudge-test.ps1` |
| `run-spike.ps1` green | plain 13/13, game-once 15/15, game-aggressive 13/14 (the known worst-case finding "Overlay is drawn" („Overlay wird gezeichnet“) from Phase 0) | `docs/spike-overlay-*.json` |
| CPU ≤ 2 % during playback | **met**: 0.67 % of 16 cores | `measure-cpu.ps1` |
| Edit mode still intact | 13 of 13 | `edit-mode-test.ps1` |
| Unit tests | **76 green**, including clock 14 (3 new ones for fusion), taps 2, offsets 2, ads and podcast 2, Sync Lab 1, migration and unknown settings fields 2 | `cargo test` |

Also checked live: the switch from "Speakers" to "Headphones (High Definition Audio Device)" was detected and announced as a new device. Not checked live: ads (none came up during the tests), podcast and the gray tray icon; `classify()` covers the detection of ads and podcasts with unit tests.

## Deviations from the plan

- **Tap measurement instead of estimates in 100 ms steps** (11.3): Space at every audible line start, error in milliseconds. The reaction time when tapping is included in every value.
- **Japanese measured only after the tag:** The user cannot read kana and kanji. The two Japanese songs ran with romaji after the merge of `claude/unruffled-dirac-66618b` (run 3 in the report).
- **Calibration over one song overshoots:** It also measures the LRC timing of that song. The first suggestion (151 ms, on "My Ordinary Life") was 51 ms too high; based on the sync log, the user set the device offset to +100 ms.
- **Device detection by polling every 2 s** instead of `IMMNotificationClient::OnDefaultDeviceChanged`: no COM callback object, same effect with one device change per session.
- **Device offsets in the database** (`device_offsets`, schema from 7.3) instead of in `settings.json`, next to the song offsets.
- **Sync Lab plots deviations instead of positions:** A position line over 60 s would be a straight diagonal without any visible error. The model position is shown next to it as a value. It shows offsets in three parts (global, device, song); the LRC `[offset:]` is already contained in the line times.
- **Ads up to 40 s instead of "≈ 30 s"**, so that longer spots are caught as well.
- **Phase 3b (Web API) is dropped** (decision of the parent session). The fusion is therefore only tested with an artificial second source.
- **Sign in PLAN.md 5.2** still as in Phase 2: what is implemented is the sentence "positive = lyrics later", not the formula.

## Open points

- **p95:** Song offsets can fix the shift per song (−140 to +151 ms in the log), but not the spread within a song (p95 267 ms). Whether it comes from the LRC files or from the tapping cannot be separated with this method. A cross-check would be a song with word timings (Enhanced LRC) or an LRC file made by hand.
- **Suggest the device offset from the sync log:** The bias across several songs is a better device offset than five taps on one song. A button "Use bias as device offset" („Tendenz als Geräte-Offset übernehmen“) at the end of a measurement would be little effort.
- **Song offset for "Oddloop":** about +150 ms; the LRC file is consistently 144 ms early. The user sets this by nudging while the song is playing.
- **Focus test with tapping** (11.3) is only covered by the foreground checks of `run-spike.ps1` and `edit-mode-test.ps1`. During a measurement, LyricHub intentionally captures the Space key; Esc ends the measurement.
