# Spike A – SMTC measurement (Spotify, Microsoft Store version)

*English translation of [spike-smtc.md](../spike-smtc.md).*

> Date: 2026-10-25 · Phase 0 · Reference: [PLAN.md](../../PLAN.en.md) 3.2 and 5.8
> Result: **SMTC is enough as the sole position source for v1.** Plan B (Spicetify) and Plan C (Web API) are not needed.

## Setup

| Item | Value |
|---|---|
| Spotify | Microsoft Store, package `SpotifyAB.SpotifyMusic` 1.306.1921.0 |
| Session ID (AUMID) | `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify` |
| Tool | `tools/smtc-probe/smtc-probe.ps1` (Windows PowerShell 5.1, WinRT `Windows.Media.Control`) |
| Analysis | `tools/smtc-probe/analyze.py` |
| Sampling | every 50 ms: status, `Position`, `LastUpdatedTime`, `EndTime`; title every 250 ms |
| Time base | `GetSystemTimePreciseAsFileTime` (system time with sub-millisecond resolution) and Stopwatch |
| Active run | 240 s; the tool controls Spotify via SMTC: every 40 s a seek by +30 s, then a 3 s pause |
| Passive run | 360 s, reading only, normal playback across three songs |

The raw data is stored locally in `../spike-smtc-active.csv` and `../spike-smtc-passive.csv` and is not checked in.

PowerShell instead of Rust is equivalent here. Spotify generates the timeline values itself; the reader affects neither the cadence nor the jitter. The Rust binding via the `windows` crate will be built in Phase 1.

## Key figures

| Metric | Active | Passive |
|---|---|---|
| Interval between timeline updates during playback (median) | 4502 ms | 4508 ms |
| Age of an update when first read (median / p95) | 25 / 48 ms | 27 / 51 ms |
| **Correction jump of the extrapolated position, \|p95\|** | 101 ms | **1 ms** |
| Correction jump, \|max\| | 216 ms | 45 ms |
| Backward jumps > 50 ms | 6 (all directly after seeking) | 0 |
| Playback rate according to Spotify's timestamps | 1.00000 (−4 ppm) | 1.00007 (+74 ppm) |
| Position changes during pause | never | – |

Response to SMTC commands in the active run, measured from the command to the first visible effect. The 50 ms sampling is included in this.

| Command | n | Median | p95 | Note |
|---|---|---|---|---|
| Play | 7 | 51 ms | 65 ms | |
| Seek | 6 | 50 ms | 51 ms | Landing position 15 ms before the target (median) |
| Pause | 6 | 305 ms | 307 ms | Status only switches after Spotify's fade-out |

## Observations

1. **Spotify publishes the timeline every 4.5 s**, and also immediately on play, pause, seek and song change. Between two updates, the reader has to extrapolate on its own: `Position + (now − LastUpdatedTime)`.
2. **In normal operation, the extrapolation is practically exact.** With undisturbed playback, the next update deviates by 1 ms at the 95th percentile. The rate deviation of at most 74 ppm amounts to less than 0.4 ms over 4.5 s.
3. **After a seek, two updates arrive.** The first arrives after about 50 ms and is 100 to 216 ms too far ahead. The second follows 100 to 250 ms later and corrects back. All six backward jumps of the active run are these corrections.
4. **On a song change, the new timeline arrives before the new title.** For up to 250 ms, position 0 is still paired with the old title. Without special handling, this would look like a backward seek in the old song.
5. **Pause:** The status changes about 305 ms after the command. The frozen position is 20 to 136 ms past the time of the command. So Spotify keeps playing briefly during the fade-out.
6. **Spotify refreshes `LastUpdatedTime` even while paused**, without the position changing.
7. **The age of an update when read** is 25 ms at the median, which equals half the sampling period. So Spotify sets the timestamp practically at the moment of publication.
8. A second SMTC session is active (`F0DC299D809B9700`, probably a browser video). The filter on `Spotify` in the AUMID is necessary.

## Decision

**SMTC alone is the position source for v1.** The criterion from PLAN.md 5.8 was a jitter p95 below 150 ms. Measured: 1 ms in normal operation and 101 ms including the corrections directly after seeking. A switch to desktop Spotify with Spicetify is not necessary; the Web API stays optional.

## Consequences for Phases 1 to 3

- **Seek handling in the `PlaybackClock`:** Adopt the first update after a seek as a hard jump. Apply a backward correction of up to 300 ms within 500 ms after a seek directly instead of slewing it. The display jumps at that moment anyway, and this way the line is correct 200 ms earlier.
- **Song change detection:** If the position drops below 1 s and deviates strongly downward, this is a likely song change and not a seek. Then wait briefly for `MediaPropertiesChanged` (timeout 400 ms) before searching for lyrics. Still set the anchor to 0 immediately.
- **Pause:** React to the status event and freeze the position that comes with the event. Until then, the display keeps running for about 300 ms, which matches the audible fade-out.
- **Polling:** Timeline events plus polling every 250 ms are enough. A faster rate gains nothing, because Spotify only delivers new values every 4.5 s.
- **Diagnostics:** The Sync Lab should show the update cadence. If a Spotify update changes this behavior, it will be noticed there immediately.

## What this measurement does not show

- **The offset to the audible sound.** The measurement shows that Spotify's position values are consistent in themselves. It does not measure how far they are ahead of or behind the sound, including the output latency of Windows and the device. This constant offset is calibrated via the device offset and checked by ear in Phase 2 (PLAN.md 11.3).
- **Other Spotify versions.** Only the Store version 1.306 was measured. The desktop version could behave differently.
- **Crossfade.** Crossfade was not part of the measurement.
