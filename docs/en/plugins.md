# Plugins

German: [docs/plugins.md](../plugins.md)

Plugins are programs of their own that LyricHub starts and feeds with data. They are optional and can be switched off or removed at any time. If a plugin crashes, LyricHub keeps running. So far there is one plugin, **Align**. It delivers real word times from the sound, so the words light up with the singing.

Plugins are normal programs with your rights. Only install plugins you trust. So far there are only LyricHub's own plugins, on the [releases page](https://github.com/HeiligerHoly/LyricHub/releases).

## Installing

1. Download `lyrichub-align-0.1.0.zip` from the release's assets. Do not unpack it.
2. In LyricHub: Settings → Plugins → "Install plugin from ZIP …" and choose the ZIP file.
3. LyricHub checks the file and shows name, version, permissions and the SHA-256 checksum. The checksum should match the one in the release notes. Then "Install".
4. A newly installed plugin is off. Switch it on.

When switched on for the first time, Align downloads its model, 588 MB from huggingface.co. The plugin page shows the progress. An interrupted download continues where it stopped on the next start.

## How Align works

LRCLIB almost only has line times. So LyricHub estimates when each word is sung. Align replaces that estimate with measurements:

1. **On the first listen** Align captures the player's sound. The status line then shows "Listening".
2. **After the song** Align aligns the lyrics to the singing ("Aligning a song: 45 %"). This takes about 1 to 1.5 times the song length.
3. **From the second listen on** the word times come from the cache. The words light up with the singing, with the word glow. Diagnostics then shows "from the plugin lyrichub-align" under "Word times".

Only songs heard to at least 85 % are aligned. Lines missing from the capture (skipped, seeked) stay estimated. Align leaves songs with real word times from the LRC file alone.

## Games and performance

- **Fullscreen:** while a game or another program runs fullscreen in front, Align does not compute. The status line shows "Paused while a fullscreen game runs". Heard songs wait until the game is closed or runs in a window.
- **CPU:** computing, Align uses 2 cores at "below normal" priority; capturing, almost nothing (under 0.1 % of the CPU).
- **Memory:** idle Align takes about 30 MB, computing up to 1.5 GB. The model is loaded only while there is work.
- **Queue:** listening a lot while gaming for long collects songs. The queue holds a good two hours of music; then the oldest song goes.

## Privacy

- **What is captured:** only the sound of the player and its child processes, never the game's sound or the microphone. If a browser is the player, that is the sound of all its tabs.
- **Where the sound stays:** only in memory, and it is dropped after the alignment. It never goes to disk and never leaves the PC.
- **What is stored:** only the word times, in LyricHub's cache on your PC.
- **Network:** Align connects to huggingface.co only for the one-time model download.

## Anti-cheat

Align uses the same Windows interface as OBS or Discord to record the sound of a single program (process loopback). It does not touch games, injects nothing and hooks into nothing. Anti-cheat systems have no reason to mind it. There is no guarantee, though: no game publisher has reviewed LyricHub or Align.

## Disk space

| What | Where | Size |
|---|---|---|
| Program | `%APPDATA%\LyricHub\plugins\lyrichub-align` | 24 MB |
| Model | `%LOCALAPPDATA%\LyricHub\plugins\lyrichub-align` | 588 MB |
| Word times | LyricHub's cache | a few KB per song |

## Removing

Settings → Plugins → "Remove". This deletes the program, the model and the word times Align delivered. The songs fall back to the estimate.

## Limits

- **Output latency:** Align hears the sound before it leaves the speaker. The words therefore light up early by the output device's latency. With a cable you will not notice, with Bluetooth headphones you might.
- **Song offset:** changing a song's offset after the alignment shifts the words against the singing.
- **Japanese and Korean:** the model knows these scripts, but this is not tested.

How accurate Align is and how it is built is in the [P3 report](plugin-align.md).
