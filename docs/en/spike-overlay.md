# Spike B – Overlay window (Tauri 2 on Windows)

*English translation of [spike-overlay.md](../spike-overlay.md).*

> Date: 2026-10-25 · Phase 0 · Reference: [PLAN.md](../../PLAN.en.md) 6 and 10
> Result: **Tauri stays.** The overlay works over borderless games, in exclusive fullscreen, over fullscreen videos, click-through and without focus loss. The precondition is that it is controlled directly via Win32 and not via Tauri setters.

## Test system

| Item | Value |
|---|---|
| Operating system | Windows 11 Pro 10.0.26300 |
| Graphics | NVIDIA GeForce GTX 1080 Ti, 2560×1440 at 164 Hz, scaling 100 % |
| CPU | 16 logical cores |
| Stack | Tauri 2.12.5, tao 0.37.2, windows-rs 0.62.2, WebView2 155 |

## Findings from the tao source code

Checked in `tao/src/platform_impl/windows/window_state.rs`, confirmed in the self-test.

1. **Every flag change overwrites the entire extended window style.** After Tauri's `set_ignore_cursor_events(true)`, `WS_EX_TOOLWINDOW` was missing, and `WS_EX_APPWINDOW` was set instead. The overlay then appears in Alt+Tab and in the taskbar. Measured: style `0x080C0138` instead of `0x080801B8`.
2. **After every flag change on a visible window, tao calls `ShowWindow(SW_SHOW)`.** In the first test run, LyricHub itself held the foreground. After Tauri's `hide()` and `show()`, the focus was then on the overlay. If another process holds the foreground, Windows prevents the switch. However, LyricHub may take it over right after its own shortcut or a tray click. That is exactly when Tauri's `show()` would take the focus away from the game.
3. **`focusable: false` sets `WS_EX_NOACTIVATE` inside tao** and therefore survives every recalculation.
4. **A window with Tauri's default `focus: true` takes over the foreground at startup, even when it is invisible.** This is how the hidden settings window stole the focus at program start. Fixed with `focus: false`; `open_settings` focuses the window explicitly.

## Implemented strategy

- Overlay created invisible, with `focus: false`, `focusable: false`, transparent, without a frame, `alwaysOnTop`, `skipTaskbar` and `backgroundThrottling: disabled`.
- **After creation, no more Tauri setters that change flags.** Showing, hiding, click-through and Topmost go through Win32 in `src-tauri/src/overlay/win32.rs`.
- A watchdog checks the style bits and visibility every 500 ms and reasserts Topmost every 2 s. During the visual check, it did not have to step in a single time.

## Self-test (automatic)

`tools/overlay-spike/run-spike.ps1` builds the app and starts the self-test in three scenarios. The reports are in `../spike-overlay-*.json`.

| Scenario | Production path | Focus test meaningful | Overlay above game |
|---|---|---|---|
| No game, Claude in the foreground | 13 of 13 passed | yes | – |
| Game sets Topmost once (typical) | 15 of 15 passed | yes, game in the foreground | 100 % of the time |
| Game sets Topmost every 500 ms (worst case) | 12 of 14 passed | yes | 5 % of the time |

Checked: no focus loss when showing, toggling and hiding; complete style bits; the overlay is drawn (5.8 to 6.4 % of the pixels in the overlay area change); clicks pass through, or are caught in edit mode; exclusion from Alt+Tab. The two failures in the worst case result from the game covering the overlay.

**Worst case, aggressive Topmost:** Reasserting more often helps only to a limited extent.

| Reassert every | Overlay on top |
|---|---|
| 2000 ms | 5 % |
| 250 ms | 58 % |
| 100 ms | 82 % |

So polling at a fixed rate still flickers even at 100 ms. Games that keep putting themselves on top are rare. Recommended for Phase 5: react to changes in the window order (`SetWinEventHook` with `EVENT_OBJECT_REORDER` and `EVENT_SYSTEM_FOREGROUND`, `WINEVENT_OUTOFCONTEXT`). This runs outside the game process, so it is not injection.

## Visual check by the user

| Check | Result |
|---|---|
| Overlay at the top center, Spotify song in the settings window | passed |
| Game in borderless mode, Ctrl+Alt+L and Ctrl+Alt+E in game | passed, game keeps focus and input |
| Game in exclusive fullscreen | **Overlay visible** (probably via the Windows fullscreen optimizations) |
| Fullscreen video in the browser | passed |
| Alt+Tab | Overlay not in the list |

## Performance

Measured with `tools/overlay-spike/measure-cpu.ps1`: LyricHub including all WebView2 processes, overlay visible with the fill running, settings window hidden.

| Karaoke fill variant | CPU (one core = 100 %) | Share of 16 cores |
|---|---|---|
| Animated gradient with `background-clip: text` | 55 % | 3.45 % |
| Two text layers, reveal via `transform` | 31 % | 1.91 % |
| As above, throttled to 30 frames/s (`steps(150)` over 5 s) | **13 %** | **0.79 %** |

The largest item remains the WebView2 GPU process at 9 % of one core. Unthrottled, it recomposites every frame at 164 Hz.

**Memory:** 203 MB private, of which about 30 MB is for the hidden settings window. The working set of 432 MB counts shared memory several times.

**Consequences for Phase 2 and 4:**
- Implement the karaoke fill only via `transform` and `opacity`, never via animated gradients or layout properties.
- Limit the frame rate of the fill. In Phase 2, the playback clock drives the fill via `requestAnimationFrame`; throttle it there to 30 to 60 updates per second instead of following every monitor frame.
- Stop all animations when playback is paused and when the overlay is hidden.
- Create the settings window only when it is opened, and destroy it when it is closed. That saves about 30 MB.

## Decision

**Tauri stays.** All core requirements for the overlay are proven. The risks found are solved by direct Win32 control and an economical fill. WPF is not needed.

## Open items for later phases

- Event-driven Topmost against aggressive games (Phase 5).
- Create the settings window on demand (Phase 1 or 5).
- Measure the performance again with real lyrics and the playback clock (Phase 2), plus an in-game FPS test (Phase 4).
- Build the real edit mode with dragging and scaling (Phase 1). Currently, Ctrl+Alt+E only toggles click-through.
