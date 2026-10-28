# Spike – a browser as the source (Zen and YouTube Music)

> Date: 2026-10-27 · after v1.0.0 · Reference: PLAN.en.md 13.4
> Result: **A Firefox-based browser delivers enough for synced lyrics on YouTube Music.** Browser support is planned for v1.1. Regular YouTube videos and Chromium browsers are still open.

## Setup

| Item | Value |
|---|---|
| Browser | Zen (Firefox-based) |
| Site | music.youtube.com |
| Session ID (AUMID) | `F0DC299D809B9700`, an ID instead of an app name |
| Tool | `tools/smtc-probe/smtc-probe.ps1 -AppFilter F0DC299D809B9700`; list sessions with `tools/smtc-probe/list-sessions.ps1` |
| Analysis | `tools/smtc-probe/analyze.py`, the same figures as in [spike A](spike-smtc.md) |
| Sampling | every 50 ms: status, `Position`, `LastUpdatedTime`, `EndTime`; title every 250 ms |
| Run | 200 s, read only. The user controlled playback in the browser: two seeks, a pause of about 9 s, then a song change |

The raw data is kept locally in `docs/spike-browser-zen.csv` and is not checked in.

## Figures

| Measure | Zen + YouTube Music | Spotify (spike A, passive) |
|---|---|---|
| Timeline updates while playing | about 3 per minute, irregular | every 4.5 s, about 13 per minute |
| Time between updates (median / p90 / max) | 2.8 / 14.2 / 60.1 s | 4.5 s (median) |
| Age of an update when first read (median / p95) | 28 / 51 ms | 27 / 51 ms |
| **Correction jump of the extrapolated position, \|p95\|** | **75 ms** (n = 10) | 1 ms |
| Correction jump, \|max\| | 75 ms | 45 ms |
| Backward jumps > 50 ms | 1 | 0 |
| Playback rate from the browser's timestamps | 0.99983 (−169 ppm) | 1.00007 (+74 ppm) |
| Position changes while paused | never | – |

## Observations

1. **Clean metadata on YouTube Music.** Title and artist come without additions such as "(Official Video)", and the length is right. The album was not recorded.
2. **Zen reports the position almost only on events.** Play, pause, seek and song change arrive at once (age when read: median 28 ms, as with Spotify). In between there is only an occasional update; once in the run, 60 s passed.
3. **Extrapolation holds over long gaps.** The rate deviation of −169 ppm adds up to about 10 ms over 60 s. Correction jumps were at most 75 ms. LyricHub's `PlaybackClock` extrapolates between updates anyway and has no time limit for long gaps.
4. **On a song change the new position arrives before the new title**, as with Spotify (one case in the run). The existing song-change detection (spike A, consequence 2) therefore applies here too.
5. **The session ID is an ID, not a name.** Firefox-based browsers report a 16-digit hex ID, probably derived from the installation path. No display name is registered for it (`HKCU\Software\Classes\AppUserModelId`). So LyricHub cannot recognise browsers by name the way it recognises Spotify.
6. **Zen reports every playback as "Music"**, videos included. The media type is no use as a music filter.
7. **Regular YouTube videos gave no position.** In a session list before the measurement, Zen reported position 0 and length 0 for a video on youtube.com. This was not measured systematically. Without a position, syncing is impossible.

## Assessment

For YouTube Music in a Firefox-based browser, SMTC is enough for synced lyrics. The criterion from PLAN.en.md 5.8 (jitter p95 below 150 ms) is met at 75 ms, and events arrive as fast as with Spotify. Browser support is therefore planned for v1.1, at first as an experimental option (PLAN.en.md 13.4).

## Consequences for v1.1

- **Player choice instead of a fixed filter:** the fixed filter on `Spotify` (`lib.rs`) becomes a setting. Spotify stays the default. Because browsers report no name, the choice lists the detected sessions with the title playing in each, so the browser can be recognised.
- **Music filter and opt-in:** browsers also report videos, and the media type does not help (observation 6). Look up lyrics only when length and title look like music, and use browser sessions only after they are switched on. Otherwise every video title would go to LRCLIB.
- **Clean up titles:** for sites like youtube.com, remove additions such as "(Official Video)" and split "Artist - Song" before asking LRCLIB.
- **An offset per player**, as there is per output device today.
- **Sync Lab:** show the update cadence per source. Gaps of up to 60 s are normal for browsers and not an error.

## Follow-up: Edge and regular YouTube (2026-10-28)

Measured before building, as PLAN.en.md 13.4 asks. Same tool; the user controlled playback in the browser.

| Run | Session ID | Length | Result |
|---|---|---|---|
| Edge + YouTube Music | `MSEdge` | 150 s | Position and length correct. Seek, pause and resume arrive after 4–51 ms; in between up to 37 s without an update. Correction jumps at most 3 ms (few values). |
| Edge + youtube.com (music video) | `MSEdge` | 90 s | Position and length correct, events after 15–51 ms. On a seek Edge briefly reports "paused" and right after "playing" with the new position. |
| Zen + youtube.com (same video) | `F0DC299D809B9700` | 60 s | Position 0 and length 0 almost all the time; only for about 0.2 s after the seek did the real values arrive, then 0 again. |

**New observations**

8. **Edge briefly reports "closed" on a song change.** For about 0.3 s the session has no title and length 0, then the new position arrives, the new title 0.2 s later. LyricHub must not treat this as a closed player and must skip empty titles.
9. **youtube.com puts "Artist - Song" into the title**; the artist field holds the channel name (for example with "VEVO" or "Official"). For the lyrics lookup LyricHub has to split the title and drop the channel.
10. **Zen on youtube.com cannot be synced**, while Zen on YouTube Music and Edge on both sites can. A session that reports length 0 while playing has no usable timeline; LyricHub then shows no lyrics, only the compact card.

**Assessment:** Chromium browsers are at least as suitable for synced lyrics as Firefox-based ones, and better on youtube.com. Browser support comes in 1.1.0 with the points above.

## What this measurement does not show

- **The offset to the audible sound.** As in spike A, only the internal consistency is measured. The browser's output latency is calibrated with the offset per player.
- **Chrome and other Chromium browsers.** Only Edge is measured (follow-up); Chrome uses the same technology but is not checked.
- **Other sites** such as the Spotify web player or SoundCloud. Regular YouTube videos are checked in the follow-up (observations 9 and 10).
- **A large sample.** One run of 200 s with ten corrections and three short follow-up runs; the figures are a first indication.
