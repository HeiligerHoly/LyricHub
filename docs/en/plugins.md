# Plugins

German: [docs/plugins.md](../plugins.md)

Plugins are programs of their own that LyricHub starts and feeds with data. They are optional and can be switched off or removed at any time. If a plugin crashes, LyricHub keeps running. There are two plugins:

- **Align** delivers real word times from the sound, so the words light up with the singing. With **Auto-Sync** it also moves the lines onto the singing.
- **Recognize** gets lyrics for songs that LRCLIB has no synced lyrics for: it searches more broadly, aligns lyrics without timestamps or recognizes the text from the singing. It needs LyricHub 1.3.0.

Plugins are normal programs with your rights. Only install plugins you trust. So far there are only LyricHub's own plugins, on the [releases page](https://github.com/HeiligerHoly/LyricHub/releases).

## Installing

1. Download the plugin's ZIP file from the release's assets, for example `lyrichub-align-0.2.0.zip` or `lyrichub-recognize-0.1.0.zip`. Do not unpack it.
2. In LyricHub: Settings → Plugins → "Install plugin from ZIP …" and choose the ZIP file.
3. LyricHub checks the file and shows name, version, permissions and the SHA-256 checksum. The checksum should match the one in the release notes. Then "Install".
4. A newly installed plugin is off. Switch it on.

When switched on for the first time, the plugin downloads its model, 588 MB from huggingface.co. The plugin page shows the progress. An interrupted download continues where it stopped on the next start. Align and Recognize use the same model: if one of them has it already, the other takes it over without a download, and it is on disk only once.

## How Align works

LRCLIB almost only has line times. So LyricHub estimates when each word is sung. Align replaces that estimate with measurements:

1. **On the first listen** Align captures the player's sound. The status line then shows "Listening".
2. **After the song** Align aligns the lyrics to the singing ("Aligning a song: 45 %"). This takes about 1 to 1.5 times the song length.
3. **From the second listen on** the word times come from the cache. The words light up with the singing, with the word glow. Diagnostics then shows "from the plugin lyrichub-align" under "Word times".

Only songs heard to at least 85 % are aligned. Lines missing from the capture (skipped, seeked) stay estimated. Align leaves songs with real word times from the LRC file alone.

### Auto-Sync

People set LRCLIB's timestamps by hand, and they are often a few tenths of a second off the singing. With **Auto-Sync** (Settings → Plugins → Align, on by default) each line starts 150 ms before its first sung word once Align has measured the song:

- A line moves at most 1.5 s from its timestamp and never before the line above.
- Lines without a measurement (gaps in the capture) move by the song's median.
- Diagnostics then shows "moved onto the singing (Auto-Sync)" under "Lines", with the median shift.
- A song offset still applies on top of Auto-Sync.

Switched off, LRCLIB's timestamps apply again; the word times stay.

## How Recognize works

LRCLIB has no synced lyrics for some songs. Then Recognize steps in:

1. **On the first listen** it captures the whole song ("Listening"), like Align only the player's sound.
2. **After the song** it computes what was sung ("Recognizing a song: 45 %"). This takes up to about twice the song length. Then it tries, in this order:
   - **Lyrics without timestamps:** if LRCLIB has the text without times, Recognize aligns it over the whole song, lines and words.
   - **Broader search:** it searches LRCLIB by the title alone and compares each result's lyrics with what it heard. If one fits, it takes those lyrics and aligns them to the singing.
   - **Recognition:** otherwise the recognized text itself becomes the lyrics, but only when the recognition is sure enough. The overlay then shows "recognized" next to the title.
3. **From the second listen on** the lyrics are there, with word times. Settings → Sync → Lyrics shows where they come from.

Recognized text has errors (in the test about every fifth word, even when the recognition was sure). If lyrics from Recognize are wrong, throw them away with Settings → Sync → "Discard the plugin's lyrics". For that song LyricHub then asks LRCLIB again and no longer asks a plugin.

## Games and performance

The same holds for Align and Recognize. Only one of them computes at a time; the other waits.

- **Fullscreen:** while a game or another program runs fullscreen in front, the plugin does not compute. The status line shows "Paused while a fullscreen game runs". Heard songs wait until the game is closed or runs in a window.
- **CPU:** computing, the plugin uses 2 cores at "below normal" priority; capturing, almost nothing (under 0.1 % of the CPU).
- **Memory:** idle a plugin takes about 30 MB, computing up to 1.5 GB. The model is loaded only while there is work.
- **Queue:** listening a lot while gaming for long collects songs. The queue holds a good two hours of music; then the oldest song goes.

## Privacy

- **What is captured:** only the sound of the player and its child processes, never the game's sound or the microphone. If a browser is the player, that is the sound of all its tabs.
- **Where the sound stays:** only in memory, and it is dropped after the alignment. It never goes to disk and never leaves the PC.
- **What is stored:** only word times and lyrics, in LyricHub's cache on your PC. Recognized text is never uploaded.
- **Network:** both connect to huggingface.co for the one-time model download. Recognize also asks lrclib.net, like LyricHub itself only with the song's title, never with sound or text.

## Anti-cheat

Align and Recognize use the same Windows interface as OBS or Discord to record the sound of a single program (process loopback). It does not touch games, injects nothing and hooks into nothing. Anti-cheat systems have no reason to mind it. There is no guarantee, though: no game publisher has reviewed LyricHub or its plugins.

## Disk space

| What | Where | Size |
|---|---|---|
| Programs | `%APPDATA%\LyricHub\plugins\lyrichub-align` and `…\lyrichub-recognize` | 24 MB each |
| Model | `%LOCALAPPDATA%\LyricHub\plugins\lyrichub-align` and `…\lyrichub-recognize` | 588 MB, taken only once with both plugins |
| Word times and lyrics | LyricHub's cache | a few KB per song |

## Removing

Settings → Plugins → "Remove". This deletes the program, the model and everything the plugin delivered. After Align the songs fall back to estimated word times, after Recognize to what LRCLIB has. If the other plugin is still there, it keeps its model.

## Limits

- **Output latency:** Align hears the sound before it leaves the speaker. The words therefore light up early by the output device's latency. With a cable you will not notice, with Bluetooth headphones you might.
- **Song offset:** changing a song's offset after the alignment shifts the words against the singing.
- **Japanese and Korean:** the model knows these scripts, but this is not tested.
- **Recognition:** Recognize knows no dictionaries and writes what it hears. That gives typos and sometimes wrong words. If the recognition is unsure, Recognize rather shows nothing: in the test 5 of 12 songs got no recognized lyrics, one of them with a recognition that would have been usable.

How accurate the plugins are and how they are built is in the reports on [Align](plugin-align.md), [Auto-Sync](auto-sync.md) and [Recognize](plugin-recognize.md).
