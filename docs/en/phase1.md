# Phase 1 – Thin vertical slice "Now Playing"

*English translation of [phase1.md](../phase1.md).*

> Date: 2026-10-25 · Reference: [PLAN.md](../../PLAN.en.md) 10 (Phase 1), 4.2–4.4, 6.4, 9.6, 9.7
> Result: **Phase 1 complete** (tag `v0.1.0-phase1`). Automated tests are green; the user's visual check over a game passed.

## What was built

| Area | Implementation | Files |
|---|---|---|
| SMTC source | One worker thread owns all WinRT objects. The three session events and `SessionsChanged` only send a message; the reading happens in the worker. In addition, status and timeline are polled every 250 ms, and media properties every second as a safety net. If the position falls back to the start of the song, the worker polls the properties every 40 ms for 400 ms, because Spotify delivers the title up to 250 ms after the timeline. The title goes out before the cover. | `src-tauri/src/sources/` |
| `track` event | `TrackInfo` as in PLAN.md 4.3, cover as a `data:` URL. `null` when Spotify disappears. Plus `playback` with the status. | `app_state.rs`, `ipc/events.rs` |
| Overlay "Bar" | Cover · Title – Artist on the Glass Dark plate. A long title is shortened first; the artist keeps up to 35 %. If the position jumps to the start of a song, the old title fades out immediately (event `track-changing`); the new one fades in as soon as Spotify delivers it. Dims after 2 s of pause, hides after the configured time. No continuous animation. | `src/lib/overlay/` |
| Settings v1 | `schema_version`, JSON in `%APPDATA%/LyricHub/settings.json`, writes go through a temp file, a broken file is backed up and replaced with defaults, changes as JSON Merge Patch with range checks. | `src-tauri/src/settings/` |
| Geometry | Monitor (GDI device name, fallback to the primary display), 9 anchors, offsets and width in percent, scale, DPI per monitor. Window = plate; the page measures and reports, the backend places the window with one `SetWindowPos`. | `overlay/window.rs`, `overlay/mod.rs` |
| Edit mode | Ctrl+Alt+E: the window covers the monitor, click-through is off (only via `win32.rs`), `WS_EX_NOACTIVATE` stays. Dragging with snapping to edges and center, corners scale proportionally, sides change the width, monitor change via a button. Enter saves, Esc cancels; both are registered globally only during edit mode. The anchor follows from the third in which the center lies. | `EditFrame.svelte`, `editMode.ts`, `hotkeys.rs` |
| Settings window | Created when opened and destroyed when closed. Pages Overlay, Behavior, Diagnostics (latency, window state, self-test). | `settings_window.rs`, `src/lib/settings/` |
| Tray | Show overlay (checkmark), Edit mode, Settings, Self-test, Quit. Tooltip "LyricHub – Title – Artist" („LyricHub – Titel – Artist“). | `tray.rs` |

## Definition of Done

| Criterion | Result | Evidence |
|---|---|---|
| Title change appears in the overlay within ≤ 300 ms | **LyricHub's share: 7–10 ms.** Total from the first sign: 196–308 ms; the two values just above 300 ms are caused by Spotify, which published the title 298 ms after the timeline. That is why the old title already fades out at the first sign. Confirmed by the user over a game. | Log `track change shown in overlay`, Diagnostics page, table below |
| Set position and size via edit mode | **met**: automated 13 of 13, checked by the user in game | `tools/overlay-spike/edit-mode-test.ps1` |
| Position and size survive a restart | **met**: pixel-exact on all three monitors; after saving in edit mode ("Middle right" („Mitte rechts“)), the overlay was back at 1632,688 after the restart | Table below |
| `run-spike.ps1` green | **met**: plain 13/13, game-once 15/15, game-aggressive 13/14 (Phase 0: 12/14; the failure is the known worst case, see below) | `docs/spike-overlay-*.json` |
| CPU ≤ 2 % of all cores during playback | **met**: 0.1 % of one core = 0.00 % of 16 cores | `measure-cpu.ps1 -Seconds 60` |

### Title change latency

Measured from the first sign in the media session (the timeline jumps to the start of the song, or a new title appears) until the overlay page has drawn the new title (confirmation after two `requestAnimationFrame`). All five changes were natural song changes during playback.

| Change | Total | of which Spotify (timeline → title) | of which LyricHub (event → frame) |
|---|---|---|---|
| Payphone | 232 ms | 222 ms | 10 ms |
| Küss dein Herz | 307 ms | 299 ms | 8 ms |
| i fight my demons when i sleep | 308 ms | 298 ms | 9 ms |
| It's Been So Long | 196 ms | 186 ms | 10 ms |
| Aces | 292 ms | 285 ms | 7 ms |

Spotify currently publishes the new title up to 299 ms after the new timeline (Spike A: up to 250 ms). Polling faster does not help: during this time the worker polls every 40 ms, and each time the title only arrived with the event. So that the old title does not stay visible in the game during this gap, it already fades out at the first sign; LyricHub therefore reacts visibly after about 10 ms. The first song after startup does not count; it includes loading the page.

### Edit mode

`edit-mode-test.ps1` starts the fake game (borderless fullscreen, holds the foreground) and controls LyricHub with synthetic input.

| Check | Result |
|---|---|
| Ctrl+Alt+E: window covers the monitor, catches clicks, stays `WS_EX_NOACTIVATE` | passed |
| Mouse hits the overlay (`WindowFromPoint`) | passed |
| Drag by +400/+300, Enter: overlay at 1232,332, `settings.json` updated (x 15.625 %, y 23.033 %) | passed |
| Drag, Esc: overlay back at the saved position | passed |
| Drag back, Enter: snaps exactly to the starting position 832,32 | passed |
| After every step: the game has the foreground, no Esc reached the game | passed |

**Finding and fix:** In the first run, the drag did not get through. During hit testing, Windows treated the layered overlay as fully transparent, even though `WS_EX_TRANSPARENT` was off. WebView2 draws via DirectComposition, not into the surface that Windows checks for hit testing. Since `SetLayeredWindowAttributes` is set with a constant alpha of 255, the click hits the overlay; the rendering stays transparent. `win32.rs` sets the attributes on every style reconciliation, and the watchdog checks them. The self-test from Phase 0 had not noticed this, because it only measured in the small window.

### Self-test from Spike B

| Scenario | Phase 0 | Phase 1 |
|---|---|---|
| plain | 13 of 13 | 13 of 13 |
| game-once (game sets Topmost once) | 15 of 15 | 15 of 15, overlay on top 100 % of the time |
| game-aggressive (game sets Topmost every 500 ms) | 12 of 14 | 13 of 14 |

The remaining failure in the worst case: the game was above the overlay during the click test. Event-driven Topmost comes in Phase 5 (PLAN.md 10).

### Placement after restart

Settings written, app started, window rectangle measured with `GetWindowRect`.

| Setting | Expected | Measured |
|---|---|---|
| Primary display 2560×1440, bottom right, −2 %/−3 %, width 30 %, scale 1.2 | 1741,1320 · 768×77 | 1741,1320 · 768×77 |
| Monitor 4 (1200×1920 portrait), top left, width 50 % | 2560,−267 · 600×54 | 2560,−267 · 600×54 |
| Monitor 3 (1920×1080 on the left), bottom center, −2.2 %, width 40 % | −1344,1158 · 768×48 | −1344,1158 · 768×48 |
| Monitor does not exist → primary display, center, width 20 %, scale 0.5 | 1024,704 · 512×32 | 1024,704 · 512×32 |
| Invalid JSON | Backup `settings.broken-<unix>.json`, defaults | as expected |

All three monitors run at 100 % scaling. Mixed DPI values are only covered by unit tests; the DPI tests at 125/150 % are part of Phase 5.

### Memory

| State | Processes | Private |
|---|---|---|
| Freshly started, settings window never opened | 7 | 181 MB |
| Settings window open | 8 | 237 MB |
| After closing (window destroyed, not hidden) | 7 | 203 MB |
| Phase 0, settings window hidden | – | 203 MB |

Closing frees 34 MB. A remainder stays in the shared WebView2 browser process. The 200 MB limit (PLAN.md 2) applies to the acceptance in Phase 5 and will be measured again there with a release build.

## Deviations from the plan

- **`overlay-layout` instead of `overlay-mode`**: One event carries width, scale, alignment and the edit state together, so that mode and geometry never drift apart.
- **Size relative to the resolution**: Scale 1.0 corresponds to the design size at 1080 logical pixels on the shorter side of the monitor; a 1440p monitor shows the overlay 1.33 times as large. This way, width in percent and font size fit together on every monitor (PLAN.md 9.5: "28 px at scale 1 on 1080p").
- **Minimum size**: Only the minimum width of 200 px applies. The window is exactly as tall as the plate; a minimum height of 60 px would have visibly lifted the plate off the edge for bottom anchors.
- **Window geometry via Win32**: `SetWindowPos` with `SWP_ASYNCWINDOWPOS` instead of Tauri's `set_size`/`set_position`. One call instead of two, and no waiting for the main thread while a lock is held.
- **`WM_DISPLAYCHANGE`**: The watchdog compares the monitor list and the window rectangle every 2 s and repositions the window on any mismatch. DPI changes of the window also trigger an immediate repositioning.
- **Settings fields without a function are still missing**: layout selection (Phase 2/4), "Show only with a fullscreen app" („Nur bei Vollbild-App zeigen“) (Phase 5), language (Phase 4). They arrive together with their function; because every field has a default value, no migration is needed.
- **Ctrl+Alt+E in edit mode saves.** The plan does not specify this; Esc remains the way to cancel.

## Visual check over a game

By the user on 2026-10-25, game in borderless mode: **passed.**

- Spotify title change in game: the old title fades out, the new one appears.
- Edit mode in game: dragging, scaling, width, Enter and Esc work; the game keeps its input.
- Ctrl+Alt+L hides and shows the overlay.
- Quit via the tray and restart: the overlay is at the saved position in the saved size.
- The settings window with the pages Overlay, Behavior and Diagnostics is fine.
