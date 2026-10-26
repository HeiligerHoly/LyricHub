# Sync report (Phase 3)

*English translation of [sync-report.md](../sync-report.md).*

> Date: 2026-10-25 · Reference: [PLAN.md](../../PLAN.en.md) 2 (sync acceptance criteria), 5.7, 11.3
> Setup: Spotify from the Microsoft Store, SMTC as the only source, wired headphones "Headphones (High Definition Audio Device)", global offset 0. Runs 1 and 2 were measured with the Phase 3 state before version number 0.3.0, which is why these log files still say `0.2.0`. Run 3 (Japanese) ran with 0.3.0 after the romaji merge.

## Result

| Criterion (PLAN.md 2) | Target | Result | |
|---|---|---|---|
| Median error after calibration, wired | ≤ 100 ms | **96 ms** across 13 English and German songs (115 taps); **108 ms** with the two Japanese ones (15 songs, 155 taps), all at device offset +100 ms | English and German met, with Japanese narrowly **not met** |
| p95 error | ≤ 200 ms | **289 ms**, 309 ms with Japanese | **not met** |
| Seek correct again | ≤ 300 ms | Median **30 ms**, at most 34 ms (5 jumps) | met |
| Nudge affects only the current song | A song change shows its own value | 4 of 4 checks, also in the log of the first run | met |

The timing chain itself is accurate: 8 to 25 ms pass from the SMTC report to the drawn anchor. The p95 comes from two other sources. First, the LRC files place their lines earlier or later depending on the song, with a per-song median between −140 and +151 ms. Second, the individual taps scatter within a song: if the shift of each song is removed, a p95 of 267 ms remains. This spread contains the reaction time when tapping as well as individual lines that were placed imprecisely; the two cannot be separated with this method. No offset can fix it. The Japanese songs confirm the picture: "Oddloop" is consistently 144 ms early and raises the median above 100 ms; without the per-song shift, the p95 of all 15 songs is 264 ms. The user decided not to do a second measurement round.

## Method

PLAN.md 11.3 plans to estimate "too early / right / too late" for every line in 100 ms steps. A tap measurement was implemented instead: the user presses Space at every audible line start (Settings → Sync → "Start measurement" („Messung starten“)). LyricHub takes the time in the hotkey handler and computes:

```
Error = display position of the overlay at the tap − start of the nearest line (at most ±1.5 s)
```

A positive error means the overlay was already further ahead: the lyrics came too early. A negative error means the lyrics came too late. This gives milliseconds instead of estimated 100 ms steps. In exchange, the user's reaction time is included in every value. Median and p95 refer to the absolute value; p95 by nearest rank. The raw data (timestamps, line numbers and titles, no lyrics) is in `%APPDATA%\LyricHub\logs\sync-protocol-<unix>.json`.

Device, song and global offset add up (positive = lyrics later). That is why every result can be converted exactly to a different device offset: at 100 instead of 151 ms, every error is 51 ms higher.

## Calibration

The calibration wizard (5.7) ran four times with five taps each on "My Ordinary Life" (The Living Tombstone). The suggestions were 187, 161, 203 and 151 ms; 151 ms was adopted.

After that, the sync log showed a bias of −51 ms across all songs: on average, the lyrics came too late. The reason: of all songs, "My Ordinary Life" has the earliest LRC file in the entire log (+151 ms at device offset 100). A calibration over a single song also measures that song's LRC timing, not only the device. Following the recommendation from the log, the user set the device offset to **+100 ms**; with that, the bias across all songs is 0.

Conclusion for later: suggest the device offset from the bias of a sync log across several songs instead of from five taps on one song (see [phase3.md](phase3.md), open points).

## Sync log

Run 1: 115 taps, 13 songs in English and German, no missed tap (`sync-protocol-1792962624.json`).

| Evaluation | Taps | Median \|e\| | p95 \|e\| | Bias | ≤ 100 ms | ≤ 200 ms |
|---|---|---|---|---|---|---|
| As measured, device offset 151 | 115 | 110 ms | 340 ms | −51 ms | 46 % | 78 % |
| **Converted to device offset 100** | 115 | **96 ms** | **289 ms** | 0 ms | 53 % | 81 % |
| Shift of each song removed | 115 | 84 ms | 267 ms | 0 ms | 57 % | 87 % |
| Run 1 and "Alien Boy" from run 2, offset 100 | 126 | 98 ms | 289 ms | −13 ms | 52 % | 83 % |

Per song, converted to device offset 100. "Bias" is the median of the signed errors, and thus exactly the song offset that a nudge would have to set for this song: a bias of −140 ms means a song offset of −140 ms; the lyrics then come earlier. "Spread" is the mean absolute deviation from this bias.

| Song | Artist | Taps | Bias | Median \|e\| | Spread |
|---|---|---|---|---|---|
| Bis zu ihr | Papaplatte | 11 | −140 ms | 140 ms | 142 ms |
| Everybody (Backstreet's Back) – Radio Edit | Backstreet Boys | 8 | −122 ms | 122 ms | 88 ms |
| Money Game, Pt. 3 | Ren | 8 | −94 ms | 120 ms | 94 ms |
| Alien Blues | Vundabar | 4 | −86 ms | 93 ms | 93 ms |
| It Doesn't Matter | The Living Tombstone | 14 | −34 ms | 64 ms | 72 ms |
| You're Gonna Go Far, Kid | The Offspring | 7 | −10 ms | 91 ms | 101 ms |
| Nachbeben | Alligatoah | 11 | 0 ms | 77 ms | 77 ms |
| It's Been So Long | The Living Tombstone | 11 | +11 ms | 97 ms | 89 ms |
| Ali Ali | Diorange | 6 | +22 ms | 52 ms | 52 ms |
| Giant Woman (feat. Zach Callison) | Steven Universe | 8 | +30 ms | 97 ms | 98 ms |
| Hate. | thxsomch | 12 | +92 ms | 106 ms | 81 ms |
| HUSO | Ritter Lean | 8 | +120 ms | 120 ms | 60 ms |
| My Ordinary Life | The Living Tombstone | 7 | +151 ms | 151 ms | 38 ms |

Three taps are off by more than 400 ms (−539, −452, −432 ms), probably a missed or double-tapped line. They are included in all numbers.

Run 2 (`sync-protocol-1792962744.json`, 14 taps, 2 missed): "Alien Boy" (Oliver Tree, 11 taps, bias −106 ms, spread 39 ms) and three taps on "Only Wonder" (Frederic, Japanese). The user cannot read kana and kanji and therefore cannot recognize a line start; the Japanese part is not evaluated.

### Run 3: Japanese with romaji

`sync-protocol-1792965126.json`: 40 taps, no missed tap, device offset 100, version 0.3.0 after the romaji merge. The overlay showed the lines in romaji; the line times are the same as in the original script.

| Evaluation | Taps | Median \|e\| | p95 \|e\| | Bias | ≤ 100 ms | ≤ 200 ms |
|---|---|---|---|---|---|---|
| Japanese | 40 | 144 ms | 312 ms | +106 ms | 35 % | 65 % |
| Japanese, shift of each song removed | 40 | 75 ms | 225 ms | 0 ms | 57 % | 88 % |
| **All 15 songs** (run 1 converted, and run 3) | 155 | **108 ms** | **309 ms** | +28 ms | 48 % | 77 % |
| All 15 songs, shift of each song removed | 155 | 81 ms | 264 ms | 0 ms | 57 % | 87 % |

| Song | Artist | Taps | Bias | Median \|e\| | Spread |
|---|---|---|---|---|---|
| Only Wonder | frederic | 14 | −42 ms | 128 ms | 112 ms |
| Oddloop | frederic | 26 | +144 ms | 144 ms | 69 ms |

- **Oddloop:** All 26 errors are between 0 and +366 ms; the LRC file places the lines consistently too early. A song offset of about +150 ms fixes this (Ctrl+Alt+. once and Ctrl+Alt+Shift+. twice while the song is playing).
- **Only Wonder:** Within the first minute, the errors swing from +222 to −317 and back to +151 ms. There is no fixed shift; individual lines of the LRC file are placed unevenly, or tapping by romaji was harder here. The two cannot be separated.

### Deviations from 11.3

- **Japanese tapped by romaji:** The user cannot read kana and kanji. Run 3 therefore only ran after the romaji merge (`claude/unruffled-dirac-66618b`), with two Japanese songs.
- **15 instead of 10 songs:** 13 in English and German, from rap through pop and punk to electronic, plus 2 Japanese; no live recording.
- **Tap measurement instead of estimates in 100 ms steps**, see Method.

## Seek test

`tools/sync/seek-test.ps1 -Count 5` jumps alternately 25 s forward and back via SMTC and waits for the log line "seek shown in overlay". LyricHub writes it as soon as the overlay has drawn the anchor of this seek (`clock_applied`).

| Measurement | Count | Median | Maximum |
|---|---|---|---|
| Seek command until the overlay is correct (`seek-test.ps1`) | 5 | 30 ms | 34 ms |
| SMTC report until drawn, all seeks and song changes of the session | 60 | 13 ms | 25 ms (p95 19 ms) |

The clock adopts seeks in the paused state directly, without a seek anchor. The script therefore starts playback for the test and pauses again afterwards.

## Nudge per song

`tools/sync/nudge-test.ps1` presses Ctrl+Alt+. twice on the current song A, jumps via SMTC to the next song B and back again, and reads the log lines "song offset nudged/applied". All values are relative to the offset that A had at the start; at the end, the script restores it.

```
[PASS] nudge on song A - 8ball & mjg|space age pimpin'|310: 0 -> 200 ms
[PASS] next song B starts with its own offset - camp lo|luchini aka this is it|240: 0 ms
[PASS] back on song A, its stored offset applies again - 8ball & mjg|space age pimpin'|310: 200 ms
[PASS] nudged back to the start value - 8ball & mjg|space age pimpin'|310: 0 ms
all passed
```

The corresponding log (UTC, shortened):

```
21:24:25.958 song offset nudged  key=8ball & mjg|space age pimpin'|310 delta_ms=100 song_offset_ms=100
21:24:26.812 song offset nudged  key=8ball & mjg|space age pimpin'|310 delta_ms=100 song_offset_ms=200
21:24:27.708 track changed       title=Luchini AKA This Is It artist=Camp Lo
21:24:27.709 song offset applied key=camp lo|luchini aka this is it|240 song_offset_ms=0
21:24:29.545 track changed       title=Space Age Pimpin' artist=8Ball & MJG
21:24:29.546 song offset applied key=8ball & mjg|space age pimpin'|310 song_offset_ms=200
21:24:31.359 song offset nudged  key=8ball & mjg|space age pimpin'|310 delta_ms=-100 song_offset_ms=100
21:24:32.165 song offset nudged  key=8ball & mjg|space age pimpin'|310 delta_ms=-100 song_offset_ms=0
```

The same sequence, done by hand, can already be seen in the log of the first run: "Big Poppa" at +200 ms, then "Shook Ones, Pt. II" at 0, and back on "Big Poppa" at +200 ms. The first run of the script still had a bug in the log pattern and shifted a different song in the process. Those values are back at 0, and since then the script measures relative to the start value.
