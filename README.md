# LyricHub

**English** · [Deutsch](README.de.md)

A Windows overlay that shows the current Spotify song with karaoke-style lyrics over games, in sync with what you hear. The full plan is in [PLAN.en.md](PLAN.en.md) (German original: [PLAN.md](PLAN.md)).

Status: phase 5 (`v0.5.0-phase5`), robustness and release. The overlay shows the synced lyrics of the current Spotify song from [LRCLIB](https://lrclib.net) with a karaoke fill that moves word by word, plus cover, title and artist; position and size can be set right on the screen in the edit mode. Four layouts, five themes with an accent colour taken from the album cover, German and English. The overlay stays on top even over games that push themselves to the front, and it can be limited to fullscreen apps. Corrections are kept per song, every output device has its own offset, and a calibration assistant and the Sync Lab help with the setup. Japanese lines can be shown in romaji ([docs/en/romaji.md](docs/en/romaji.md)). Results in [docs/en/phase5.md](docs/en/phase5.md) and the security, performance and disk checks in [docs/en/checks.md](docs/en/checks.md), [docs/en/phase4.md](docs/en/phase4.md), [docs/en/phase3.md](docs/en/phase3.md) with the measurement report [docs/en/sync-report.md](docs/en/sync-report.md), plus [docs/en/phase2.md](docs/en/phase2.md) and [docs/en/phase1.md](docs/en/phase1.md); the phase 0 spikes are in [docs/en/spike-smtc.md](docs/en/spike-smtc.md) and [docs/en/spike-overlay.md](docs/en/spike-overlay.md).

![Overlay, layout Karaoke, theme Glass Dark](docs/img/overlay-karaoke.png)

| | |
|---|---|
| ![Layout Stack, theme Glass Light](docs/img/overlay-stack-light.png) | ![Layout Minimal, theme Neon](docs/img/overlay-minimal-neon.png) |
| ![Settings → Appearance](docs/img/settings-appearance.png) | ![Settings → Hotkeys](docs/img/settings-hotkeys.png) |

The pictures show sample lines; the repository contains no song lyrics. The interface follows the Windows display language (German or English) and can be switched under Settings → Behavior → Language.

## Disclaimer: vibe-coded

LyricHub was built with AI ("vibe coding"). Claude Fable wrote the plan, and Claude Opus 5.5 wrote the code in Claude Code, phase by phase. A human set the direction, tested the results on their own PC and signed off each phase, but did not review the code line by line. There is no warranty; use it at your own risk.

If you do not trust the binaries, you can follow or rebuild everything yourself:

- [docs/original-plan/](docs/original-plan/): the plan as it was written before the first line of code (German and English). With it you can build LyricHub from scratch, with an AI assistant or by hand.
- [PLAN.en.md](PLAN.en.md) / [PLAN.md](PLAN.md): the same plan with all later decisions and the result of each phase.
- [docs/en/](docs/en/) (German: [docs/](docs/)): the report of each phase, with measurements and deviations from the plan.
- All of the code is here; build the installer yourself with `npx tauri build` (see below).

## Install

Run `LyricHub_<version>_x64-setup.exe` (to build it, see below: `npx tauri build`, output in `src-tauri/target/release/bundle/nsis/`). The installer sets LyricHub up for the current user only and needs no administrator rights. It is not signed: Windows SmartScreen reports an unknown publisher; choose "More info" → "Run anyway". If the WebView2 Runtime is missing (Windows 11 has it), the installer fetches it. Then start Spotify; LyricHub sits in the tray and shows the current song.

## Requirements for building

- Windows 11 (Windows 10 from 2004 should work)
- Node.js 20 or newer
- Rust (stable, MSVC toolchain) via rustup
- Visual Studio Build Tools with the C++ workload and the Windows SDK
- WebView2 Runtime (preinstalled on Windows 11)
- Spotify desktop app (Microsoft Store or desktop version); LyricHub needs no Spotify login

Install the Build Tools (asks once for administrator rights through UAC):

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

## Development

```powershell
npm install
npm run tauri dev      # app with hot reload
npm run check          # check Svelte/TypeScript
npx tauri build        # release build and installer (NSIS)
```

The app starts in the tray. A left click on the tray icon shows or hides the overlay; the right-click menu opens the edit mode and the settings. The settings window only exists while it is open. With `--minimized` (as the autostart uses it) no window opens.

Default shortcuts; change or turn them off under Settings → Hotkeys:

| Shortcut | Action |
|---|---|
| Ctrl+Alt+L | Overlay on/off |
| Ctrl+Alt+E | Edit mode: drag the overlay, scale at the corners, change the width at the sides |
| Enter / Esc | In the edit mode: save / cancel (registered only then) |
| Ctrl+Alt+K | Switch layout (Karaoke, Bar, Stack, Minimal) |
| Ctrl+Alt+R | Japanese lines in romaji (Hepburn) and back |
| Ctrl+Alt+, / Ctrl+Alt+. | Lyrics of the current song 100 ms earlier / later (with Shift 25 ms); LyricHub keeps the value per song |
| Space / Esc | During calibration or the sync log: tap at each line start you hear / end the measurement (registered only then) |

The game keeps the focus in the edit mode; the overlay never takes keyboard input. If a game moves over the overlay, LyricHub puts it back on top at once (window-order events, no injection); if that fails, the tray names the covering window. "Only with a fullscreen app" (Settings → Overlay) hides the overlay on the desktop.

Appearance (Settings → Appearance): themes Glass Dark, Glass Light, Neon, Mono and a custom one; accent colour from the cover (checked for contrast against the plate) or fixed; opacity, font (Inter is bundled) and "Reduce motion". While the page is open, the overlay shows sample lines in the chosen look.

Offsets (Settings → Sync) add up; positive means lyrics later: **device** for the output latency (every output device has its own value, Bluetooth starts at +150 ms), **song** for LRC files that are early or late (by nudging), and **global** as a reserve. The calibration assistant suggests a device offset from five taps. The sync log measures over any number of songs and writes median, p95 and bias per song to `logs\sync-protocol-<unix>.json`. The Sync Lab under Settings → Diagnostics shows the position reports of the last 60 s and exports them as JSON.

Settings live in `%APPDATA%\LyricHub\settings.json`; the lyrics cache and the song and device offsets in `%APPDATA%\LyricHub\lyrichub.db`; logs under `%APPDATA%\LyricHub\logs`. `LYRICHUB_LOG` sets the log level (e.g. `debug`). The logs contain no lyrics.

## Tests and spikes

```powershell
# Spike A: measure the Spotify timeline through the Windows media session
powershell -NoProfile -ExecutionPolicy Bypass -File tools\smtc-probe\smtc-probe.ps1 -DurationSec 360
python tools\smtc-probe\analyze.py docs\spike-smtc.csv

# Spike B: build the app and run the overlay self-test in three scenarios
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\run-spike.ps1

# CPU and memory of a running LyricHub instance
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\measure-cpu.ps1 -Seconds 10

# Start without taking the focus (release build; LyricHub must not be running)
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\start-focus-test.ps1

# Edit mode over the fake game (needs a running instance, moves the mouse for a few seconds)
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\edit-mode-test.ps1

# Count layouts of the overlay page during playback ("no layout per frame"); start LyricHub with
# a debug port for it, in Git Bash: WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223 src-tauri/target/debug/lyrichub.exe --minimized
node tools\overlay-spike\layout-count.mjs 30

# Seek latency until the overlay has drawn (needs a running instance and Spotify, seeks in the current song)
powershell -NoProfile -ExecutionPolicy Bypass -File tools\sync\seek-test.ps1 -Count 5

# Nudge per song across a real track change (skips one song forward and back)
powershell -NoProfile -ExecutionPolicy Bypass -File tools\sync\nudge-test.ps1
```

Rust unit tests (LRC parser, estimated word times, playback clock with source fusion, offsets, tap evaluation, matcher, LRCLIB client against recorded answers, cache, geometry, settings store with migration, SMTC track-change detection, ads and podcasts, shortcuts): `cargo test` in `src-tauri`. The tests make no live requests to LRCLIB.

## Notes

Lyrics are protected by copyright. LyricHub only displays them and does not redistribute them. The lyrics come from [LRCLIB](https://lrclib.net), a community-maintained database; LyricHub queries it with its own user agent and stores the results locally, so it does not ask again for every repeat. The test fixtures in the repository contain invented text.

Third-party licenses (Rust crates, npm packages, the IPADIC dictionary, the Inter font and NSIS) are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md); the installer puts the file next to `lyrichub.exe`.
