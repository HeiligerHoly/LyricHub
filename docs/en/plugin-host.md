# Plugin host (P1 and P2 of PLAN-plugins.md)

> Date: 2026-10-28 · after v1.1.0 · Reference: [PLAN-plugins.md](../../PLAN-plugins.md) sections 4 and 6 (German)
> Result: **LyricHub installs, starts, watches and ends plugins (P1) and takes word times from them (P2). The acceptance criteria of both phases are checked in the real app.** The first real plugin follows in P3.

## What is built

| Part | Implementation | Code |
|---|---|---|
| Manifest | `plugin.json` with ID, name, version, protocol version, program, capabilities and permissions. Unsafe or unknown fields are rejected; another protocol version counts as "incompatible". | `plugins/manifest.rs` |
| Installation | The ZIP is checked completely before anything is written: manifest, program, no paths outside the folder, no absolute paths, no links, size limits. It is unpacked into a staging folder that only then replaces the old version. | `plugins/install.rs` |
| Process | A separate `.exe` without a console window, at "below normal" priority, in a job object with `KILL_ON_JOB_CLOSE`. JSON-RPC 2.0, one message per line; stderr goes to the log, cut short. | `plugins/process.rs` |
| Supervisor | Handshake `initialize` (5 s); after a crash, restarts after 2, 10 and 60 s, then "error" with the last stderr line; when switched off, `shutdown` and 2 s of grace. | `plugins/supervisor.rs` |
| App | `plugins.enabled` in the settings, start at program start, end at exit, page "Plugins" (install with confirmation including SHA-256, switch, state, remove), plugins in Diagnostics and in the diagnostics export (only ID, version, state). | `plugins/mod.rs`, `PluginsPage.svelte` |
| Example plugin | Answers `initialize`, reports "ready", ends on `shutdown`; a `behavior.json` makes it crash, report another version or stay silent. Tests only, not shipped. | `plugins/example/` |

Folders: programs under `%APPDATA%\LyricHub\plugins\<id>`, large data such as models under `%LOCALAPPDATA%\LyricHub\plugins\<id>`.

## Checks

**Unit tests (run with `cargo test`):**

- Manifest: valid, unsafe values, another protocol version.
- ZIP:
  - Installing from the root folder and from a subfolder.
  - Replacing the old version.
  - Rejected are parent and absolute paths, missing manifests, missing programs and several root folders.
- Messages: format and cutting, restart pauses.
- Job object: a process in it ends as soon as the job object closes.

**With the example plugin** (`cargo test --lib supervisor -- --ignored`):

- Start, status and stop.
- Crash with restarts after 2 s and 10 s.
- Another protocol version gives "incompatible", silence gives "error".

**In the real app:**

1. ZIP checked (ID, version, SHA-256) and installed: the plugin is off afterwards.
2. Switched on: "running" with the status "ready" after about 1 s.
3. LyricHub ended hard (`taskkill /F`): the plugin process was gone 2 s later.
4. Restarted: the switched-on plugin ran again 40 ms after the start.
5. Switched off: the process ends.
6. Removed: program and data folders are empty, and it is gone from the settings.

Without plugins nothing starts. The installer stays small: the only new libraries are `zip` and `typed-path`.

## P2: word times from plugins

A plugin with the capability `word_timing` gets:

- **`track`:** key, title, artist, album, length and the player. Only a plugin with `audio:player` gets the player's process ID.
- **`playback`:** on every change and every 2 s, with position, rate and the moment as a QueryPerformanceCounter value in 100 ns. That is the unit WASAPI stamps captured audio with.
- **`lyrics`:** the synced lines with start, end, text, romaji and LyricHub's units, plus a fingerprint of start times and texts.
- **A plugin that just started** hears at once what plays.

LyricHub checks the answer `word_times` line by line: one time pair per unit, rising starts, each word inside its line with 1 s of slack. There is no confidence threshold (P0). Then:

- **Store:** The times go into the cache table `plugin_words`, by song and fingerprint. If the lyrics change, they no longer apply.
- **Take over:** They replace the estimate and count as measured, which turns on the word glow. If the song is playing, the overlay shows them at once.
- **Source:** Diagnostics names where the word times come from: estimated, from the LRC file or from the plugin.
- **Remove:** A removed plugin takes its word times along. The song that plays falls back to the estimate at once.

**Checks**

- Unit tests: fingerprint, checking the answer, taking it over, the cache table including clean-up.
- With the example plugin: it answers `lyrics` with word times spread evenly over four fifths of each line.
- In the real app with a Spotify song:
  1. The plugin got the lyrics and answered. All 202 words came from the plugin afterwards, and Diagnostics said "from the plugin".
  2. Plugin off, LyricHub restarted: the 202 words came from the cache.
  3. Plugin removed: the song fell back to the estimate.
