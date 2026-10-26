# Phase 5 – Robustness and release

*English translation of [phase5.md](../phase5.md).*

> Date: 2026-10-26 · Reference: [PLAN.md](../../PLAN.en.md) 10 (Phase 5), 2, 6.3, 6.4, 9.6–9.8, 14.2
> Result: **Phase 5 built and tested** (tag `v0.5.0-phase5`). The overlay stays on top even against a game that brings itself to the front every 500 ms: `game-aggressive` is green for the first time. Added: shortcuts that can be changed and turned off, an app icon of its own and an NSIS installer without administrator rights. The release build needs 179 MB of private memory and 0.48 % CPU. Still open is the installer test on another PC; a friend of the user is doing it.

## What was built

| Area | Implementation | Files |
|---|---|---|
| Always on top (6.3) | Out-of-context WinEvent hooks for `EVENT_OBJECT_REORDER` and `EVENT_SYSTEM_FOREGROUND`, without injection. If a topmost window lands above the overlay and covers it, the overlay puts itself back on top, at most ten times per second. Menus, tooltips, taskbar, Start, Alt+Tab and IME are left alone. The 2 s watchdog stays as a fallback. | `overlay/guard.rs` |
| Fullscreen detection | Once per second: foreground normal, borderless fullscreen (window covers its monitor) or exclusive fullscreen (`SHQueryUserNotificationState`). New option "Only with a fullscreen app" („Nur bei Vollbild-App zeigen“); edit mode and preview always show the overlay. | `guard.rs`, `overlay/mod.rs`, `OverlayPage.svelte` |
| Tray notice | If a window stays above the overlay for more than 2 s, the tooltip and a first, grayed-out menu entry name the window and recommend "Borderless window" („Randloses Fenster“). In exclusive fullscreen there is a cautious notice, because Windows usually shows the overlay there anyway. | `tray.rs` |
| Monitors (6.4) | A hidden window of the watchdog receives `WM_DISPLAYCHANGE` and repositions the overlay; the 2 s polling stays. If the configured monitor is missing, the primary display is used (since Phase 1). | `guard.rs` |
| Sleep | `WM_POWERBROADCAST`/`PBT_APMRESUMEAUTOMATIC`: the SMTC connection is rebuilt and the overlay repositioned. | `guard.rs`, `sources/smtc.rs` |
| Startup | Single instance and minimized autostart have existed since Phase 0/1. New: a second launch with `--minimized` does not open the settings. | `lib.rs` |
| Hotkeys (9.7, item 6) | All eight shortcuts come from the settings (`hotkeys`) and can be changed, turned off and reset. Recording by key press, while all shortcuts are paused. A shortcut needs Ctrl, Alt, Shift or Win (except F1–F12); Enter, Esc and Space stay reserved for edit mode and measurement. Each one is registered separately, and its state (active, off, invalid, duplicate, reserved, taken by another program) is shown on the page. The tray menu shows the configured shortcuts. | `hotkeys.rs`, `HotkeysPage.svelte` |
| Error UX | Notice bar in the settings window when shortcuts are not active or LRCLIB is unreachable for the current song. In game, only the subtle marker from Phase 4. | `App.svelte` |
| Diagnostics export | Additionally: monitors, shortcut states, watchdog status and the last 400 log lines (logs contain no lyrics). | `sync_lab.rs` |
| App icon | "Half-Lit Notes", chosen by the user from twelve drafts (replaces the first speech bubble): a beamed double note on a tile that the karaoke fill runs through. On the left the tile is amber and the note dark, on the right the tile is dark and the note amber. No brand colors. Source `src-tauri/icons/app-icon.svg` (1024), all sizes via `npx tauri icon`. The tray uses a variant with a wider, brighter rim (`icons/tray.svg`, pre-rendered for 16–48 px, picked by the system DPI): on a dark taskbar the right edge of the tile then has a contrast of 3.5:1 in color and 2.5:1 in gray instead of 1.03:1. The app icon and the installer stay unchanged. | `src-tauri/icons/` |
| Installer | NSIS via the Tauri bundler, `installMode: currentUser` (no UAC dialog), German and English: `LyricHub_0.5.0_x64-setup.exe`, 10 MB. No code signing; the license stays `UNLICENSED` (14.2). | `tauri.conf.json` |
| Memory | `--disable-gpu-rasterization` for both WebViews: GPU process about 14 MB smaller, CPU unchanged. | `tauri.conf.json`, `settings_window.rs` |
| Test tools | `start-focus-test.ps1` (startup without stealing focus). `run-spike.ps1` now measures the watchdog; `-TopmostMs` stays for the comparison from Spike B. | `tools/overlay-spike/` |

## Acceptance criteria from PLAN.md 2

| Criterion | Target | Result | Evidence |
|---|---|---|---|
| Sync accuracy, median | ≤ 100 ms | **96 ms** (13 English and German songs, 115 taps); 108 ms with two Japanese songs | [sync-report.md](sync-report.md) |
| Sync accuracy, p95 | ≤ 200 ms | 289 ms (309 ms with Japanese). **Accepted by the user:** every tap includes hearing, reaction and key press; LyricHub's timing chain accounts for 8–25 ms, the rest is the LRC files and the measurement method | [sync-report.md](sync-report.md) |
| Sync stability | no jitter, slew ≤ ±10 %, no backward jump < 300 ms | **met**: clock tests (`constant_offset_is_slewed_away_without_backward_steps`, `medium_error_jumps_instead_of_slewing_for_seconds`), no jitter in the listening tests | `cargo test`, Phase 2/3 |
| Seek | ≤ 300 ms | **met**: 30 ms median | [sync-report.md](sync-report.md) |
| Track change: title | ≤ 300 ms | **met**: 7–10 ms after the SMTC event | [phase1.md](phase1.md) |
| Track change: lyrics from the cache | ≤ 100 ms | **met**: about 3 ms | [phase2.md](phase2.md) |
| Track change: lyrics online | ≤ 1.5 s | **partly**: 76 % of 135 lookups. Direct hits 120 ms median; fallback searches at LRCLIB 1.5–2.3 s (median 818 ms, p95 2.3 s) | Logs, see below |
| Focus | never take focus, input keeps working | **met**: focus steps in run-spike, edit mode test 13/13, the user's Notepad test with song changes, Ctrl+Alt+L and edit mode | `docs/spike-overlay-*.json` |
| Visibility over borderless games, also with Topmost reassert | visible | **met**: `game-aggressive` 15/15, overlay on top in 120 of 120 samples, reasserted 12 times in 6 s. The user checked real games (borderless, exclusive fullscreen) and fullscreen video in Phase 0/1 | `docs/spike-overlay-game-aggressive.json`, [spike-overlay.md](spike-overlay.md) |
| CPU paused | < 0.5 % | **met**: 0.03 % of all cores (release, 30 s) | `measure-cpu.ps1` |
| CPU playing | < 2 % | **met**: 0.48 % of all cores (release, 60 s) | `measure-cpu.ps1` |
| RAM | < 200 MB private | **met**: 179 MB for LyricHub and all WebView2 processes, release, playback | see below |
| In-game FPS | no noticeable loss | waived by the user in Phase 4 | [phase4.md](phase4.md) |
| Robustness: Spotify restart | recovers | **met**: session gone, bound again after the restart, title and lyrics back | Log 14:39 UTC |
| Robustness: device change | recovers | speakers → headphones detected (Phase 3); no Bluetooth device available | [phase3.md](phase3.md) |
| Robustness: monitor change, scaling | recovers | **met**: the user checked 125 % and 150 %; every change arrived as `WM_DISPLAYCHANGE`, and the overlay was repositioned immediately | Log 14:51 UTC |
| Robustness: sleep | recovers | **not tested live**: reconnection is built; the logs contain no sleep (the PC was off overnight) | – |
| No focus stealing at startup | autostart minimized, no window | **met**: the foreground stayed with the browser for 6 s | `start-focus-test.ps1` |

### Measurements

- **Memory, release build, playback:** without the switch, 188 MB in steady state, 202–204 MB after an edit mode session. With `--disable-gpu-rasterization`, 172–184 MB, 179 MB on average. The largest item is the GPU process (80 instead of 97 MB).
- **Lyrics online:** 135 lookups from the logs of 25 and 26 October. Direct hit: 87 lookups, median 120 ms. Without album: 23, median 1.5 s. Search: 21, median 1.9 s. Free text: 4, median 2.3 s. LRCLIB answers misses and searches slowly under load (Phase 2). If all stages started in parallel right away instead of staggered by 350 ms, it would be faster, but with up to four times the load on a free service. That is deliberately not done.
- **Spike scenarios:** plain 13/13, game-once 15/15, game-aggressive 15/15. The two failures of the "tauri" group are tao findings documented since Phase 0, not a production path.

## Definition of Done

| Item | Status |
|---|---|
| All acceptance criteria from section 2 met and documented | Met except for two items: p95 accepted by the user; online lyrics 76 % under 1.5 s, depending on LRCLIB. Sleep not tested live. |
| Installer tested on a fresh profile | **open**: the user passes the installer on to a friend. Checklist in the handover to the user. |

## Deviations from the plan

- **Sync log 11.3 not repeated:** The user accepted the p95, pointing to the human reaction when tapping; the measurements from Phase 3 (runs 1–3) apply.
- **Installer test on another PC** instead of a fresh profile here: The user is not creating a test account; a friend tests.
- **No balloon notification:** The tray notice is in the tooltip and the first menu entry; Tauri has no tray notifications.
- **Exclusive fullscreen:** only a cautious notice. Whether Windows draws the overlay via the fullscreen optimizations cannot be detected reliably.
- **GPU rasterization off:** a deviation from the Chromium default, to stay under 200 MB. The overlay text is only rasterized on line changes.
- **Glass Light with at least 70 % opacity:** At 20 %, dark text over dark games was no longer readable (noticed while taking the screenshot for the README).
- **Self-test report stays German**, as in Phase 4.
- **No code signing:** Windows SmartScreen shows "Unknown publisher" the first time the installer is run.

## Found and fixed

- The self-test's Topmost check overflowed without a fixed interval (`u64::MAX + 50`) and aborted in the debug build.
- In Neon and Minimal, the edit mode toolbar inherited the outline and shadow of the lyrics; "Save" („Speichern“) was hard to read (reported by the user).
- Glass Light at low opacity (see above).

## Open items

- Installer test by the user's friend.
- Test sleep live once (go to sleep, wake up, play a song; `resumed from sleep` in the log).
- The settings pages "Lyrics" (clear cache, import LRC) and "Connection" („Verbindung“) (9.7, items 1 and 5) do not exist; the Web API has been dropped.
- `npm audit`: three low-severity vulnerabilities in `cookie` via `@sveltejs/kit` (a fix would need Kit 3).
