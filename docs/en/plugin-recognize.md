# Plugin Recognize (Q1 and Q3 of PLAN-plugins-2.md)

> Date: 2026-10-31 · after v1.2.0 · Related: [PLAN-plugins-2.md](../../PLAN-plugins-2.md) sections 4–6, [docs/en/spike-recognize.md](spike-recognize.md), [docs/en/plugin-align.md](plugin-align.md)
> Result: **Recognize gets lyrics for songs without synced lyrics: it aligns lyrics without timestamps over the whole song, otherwise searches LRCLIB more broadly and compares with what it heard, and only last shows the recognition itself, marked "recognized". After 12 songs the threshold for that is 0.84: above it no song had more than 15 % wrong characters. Live it computes with 2 threads for up to about twice the song length and stays under 1.5 GB.**

German: [docs/plugin-recognize.md](../plugin-recognize.md)

## What was built

`plugins/recognize/` is a Cargo package of its own. It builds `lyrichub-recognize.exe` (24 MB, 8.3 MB as a ZIP). Capture, model, queue, status and protocol it shares with Align: they live in the library of `plugins/align`, which Recognize links. A separate `audio-core` package, as in the plan, was not needed for that.

| Part | Implementation | Code |
|---|---|---|
| Need | LyricHub sends `lyrics_wanted` as soon as a song without synced lyrics plays (state "not found" or "only without timing"), with title, artist, album, length and the lyrics without timestamps if LRCLIB has them. | `src-tauri/src/plugins/mod.rs` |
| Capture | The whole song, like Align only the player's sound. It counts when 85 % were heard; songs over 15 minutes are left out. A song with too little heard shows in the log ("song left out: 25 % of it heard"). | `plugins/align/src/recorder.rs` |
| Schedule | As in Align: queue up to 256 MB, 2 threads at "below normal", a pause between two pieces while a fullscreen window is in front. Only one audio plugin computes at a time (named lock `Local\LyricHub.PluginCompute`). The model is loaded for the work and freed as soon as nothing waits, before the lock is released. | `plugins/recognize/src/worker.rs`, `plugins/align/src/lock.rs` |
| Model | The same as Align's. If it is complete there, Recognize takes it over as hard links, and the other way round: it is on disk once, and removing one plugin leaves the other's model whole. | `plugins/align/src/model.rs` (`share_from`) |
| Model output | Once for the whole song, in pieces of 14 s with 2 s of context. All three paths use the same output. | `plugins/align/src/align.rs` |
| Path 1: lyrics without timestamps | The whole text aligned over the whole song at once (CTC). A line starts 150 ms before its first word. | `plugins/align/src/recognize.rs` (`align_whole`) |
| Path 2: broader search | LRCLIB `/api/search` with the cleaned title, as `track_name` and as `q`, without the artist; at most every 0.5 s, honouring the wait the server asks for. It runs before the computation, so a slow LRCLIB never holds the lock. Candidates must be within ±10 s of the song's length. The text comparison counts how many recognized words of three or more letters occur in the candidate; from 45 % on, the best one counts. Synced candidates are aligned line by line as in Align, lyrics without timestamps as in path 1. | `plugins/recognize/src/lrclib.rs`, `worker.rs` |
| Path 3: recognition | Greedy decoding of the model output into words: a word ends at the model's separator or at a pause over 400 ms. Lines form at pauses from 700 ms on or after 8 words. This shows only from a confidence of 0.84 and 12 words on (section "Threshold"), and only when the search ran: if LRCLIB could not be reached, the song waits for the plugin's next start, so recognized text cannot hide real lyrics in the cache. | `plugins/align/src/recognize.rs`, `worker.rs` |
| Answer | `lyrics_found` with the source (`aligned_plain`, `lrclib` or `recognized`), the LRCLIB ID and the lines with word times. | `worker.rs` |

**Host changes (Q1)**

- **Capability `lyrics_source`:** `lyrics_wanted` out, `lyrics_found` back; LyricHub takes lyrics only from plugins that declare this capability. It checks the lines (rising starts, at most 600 lines of 300 characters, control characters removed) and puts them into the cache as LRC, with the source `plugin:<id>:<path>`. Synced lyrics from LRCLIB always win. The word times go into the cache as with Align, and if the song is playing, the overlay shows the lyrics at once.
- **Marking:** for recognized lyrics the overlay shows "recognized" next to the title. Settings → Sync → Lyrics and Diagnostics show where the lyrics come from.
- **Discarding:** Settings → Sync → "Discard the plugin's lyrics" deletes the plugin's lyrics for this song and asks LRCLIB again. The song goes into the table `plugin_rejects`; LyricHub asks no plugin for it any more and takes no plugin lyrics for it. Removing a plugin removes its lyrics too.
- **Core:** a last LRCLIB stage searches by the title alone; the matcher still checks artist and length. If LRCLIB stays unreachable through all retries, a song counts as missing for two hours instead of a day.

## Checks

### Unit tests

- **Library** (`cargo test` in `plugins/align`, 39 tests): among them greedy decoding, word ends and lines, the text comparison and the alignment over the whole song on synthetic model output; capturing whole songs; the compute lock; sharing the model.
- **Recognize** (`cargo test` in `plugins/recognize`, 2 tests and 1 with network): cleaned titles for the search and LRC lines; with `--ignored`, a real search on lrclib.net.
- **Host** (`cargo test` in `src-tauri`): checking `lyrics_found` and turning it into LRC, discarding and forgetting plugin lyrics in the cache, the title-only stage with the artist check.

### Threshold for recognized lyrics

The confidence is the mean probability the model gives the recognized characters. Measured on 12 JamendoLyrics songs (three each in German, English, French and Spanish; pop, rock, hip-hop, jazz and reggae) with the check program `--example recognize`, sorted by confidence:

| Confidence | Language | Character / word errors |
|---|---|---|
| 0.941 | Spanish | 8.1 / 18.3 % |
| 0.905 | French | 7.4 / 20.3 % |
| 0.902 | English | 15.3 / 47.5 % |
| 0.892 | German | 8.0 / 20.3 % |
| 0.892 | German | 5.6 / 17.8 % |
| 0.869 | Spanish | 8.8 / 35.8 % |
| 0.842 | German | 8.1 / 20.2 % |
| *threshold 0.84* | | |
| 0.828 | French | 31.4 / 53.4 % |
| 0.797 | French | 34.0 / 48.7 % |
| 0.789 | Spanish | 20.0 / 41.2 % |
| 0.709 | English | 32.1 / 62.5 % |
| 0.644 | English | 46.2 / 75.1 % |

- **The threshold is 0.84.** All 7 songs above it have at most 15 % wrong characters. The four songs with more than a quarter of the characters wrong are all below 0.83.
- **The provisional 0.75 was too low:** it would have shown 10 songs, two of them with a third of the characters wrong.
- **The price:** a usable Spanish song (20 % character errors) stays out at 0.789. Without lyrics LyricHub shows the compact card, as before.
- **Even above the threshold about every fifth word is wrong,** almost every second in the English song. Mostly a single letter is off, and a word with one wrong letter already counts as wrong. That is why recognized lyrics are marked as such and can be discarded.
- **Lyrics without timestamps (path 1)** hit the line start on the same 12 songs at 18–120 ms in the median per song, p90 46–297 ms. That confirms M3 from Q0 on three times as many songs.
- **Computing time of the check program:** 1.24 times the song length on average with 4 threads, while the plugins computed alongside.

### In the real app

Tested with Spotify and the user's playlist, 2026-10-30/31, with Align and Recognize installed together. The JamendoLyrics measurements ran alongside with 4 threads, so the computing times below are on the long side.

| Song | Path | Result | Computing time |
|---|---|---|---|
| no lyrics on LRCLIB, 207 s | recognition | 403 words, confidence 0.78, shown as "recognized" (67 lines with word times). With the 0.84 threshold the song would stay without lyrics. | 455 s (2.2 × song length) |
| only lyrics without timestamps, 214 s | align the text | 66 lines, all with word times | 403 s (1.9 × song length) |
| no lyrics on LRCLIB, title with extras in brackets, 92 s | broader search | recognition with confidence 0.86; 15 candidates, the best with 89 % of the recognized words. Taken: its synced lyrics, 39 lines, all with word times. | 214 s, 40 s of it paused for the fullscreen test |

- **The search was broken for the first song** and was found that way. The plugin checked lrclib.net's certificate against the root certificates ureq brings along; the chain Windows builds for lrclib.net ends in none of them, and every request failed. The plugin now uses Windows' root certificates, as LyricHub itself does. A real search finds candidates with it (37 for a common title, test with `--ignored`). Since then a failed search also means no recognized lyrics.
- **On the second listen** of the recognized song, the 67 lines with their word times came from the cache. The overlay showed "recognized", Settings → Sync the source "Lyrics from a plugin, recognized from the singing" and the discard button. Clicking it deleted the lyrics, LyricHub asked LRCLIB again (nothing found), and no plugin was asked for the song any more. (The block was then removed by hand, so the fixed plugin tries the song again.)
- **Compute lock:** Align and Recognize took turns. The one waiting showed "Waiting for another plugin to finish". After each computation the plugin freed its model at once, before the other one's turn.
- **Memory:** 10 MB idle, up to 1.2 GB computing.
- **Model:** Recognize took Align's model over as hard links (link count 2), without a download.
- **Half-heard songs** show in the log, for example "song left out: 75 % of it heard, 85 % needed", when the plugin restarted in the middle of a song.
- **Fullscreen:** A black fullscreen window for 40 s while Recognize computed. It went on with 1.9 cores until the piece it was on finished after 16 s, then used 0.00 cores, and the status line showed "Paused while a fullscreen game runs". After the window it carried on.

## Limits

- **Only from the second listen on,** and only for songs heard to 85 %.
- **Recognized text has errors:** about every fifth word even above the threshold, almost every second in the English test song. Below the threshold the compact card stays.
- **Only 12 test songs** for the threshold. Densely produced pop and rap on Spotify may be harder than Jamendo's CC music.
- **Better lyrics only after 14 days:** a plugin's lyrics sit in the cache like found lyrics. Once they are 14 days old, LyricHub asks LRCLIB again at the next listen (IDEEN.md, package A). If LRCLIB gets synced lyrics sooner, LyricHub only sees them before that after "Reload lyrics" or "Discard the plugin's lyrics".
- **Japanese and Korean:** not measured.
- **Output latency and song offset** work as with Align ([docs/en/plugin-align.md](plugin-align.md)).
