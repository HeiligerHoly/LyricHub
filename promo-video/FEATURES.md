# LyricHub feature inventory for the promo video

Source of truth: the LyricHub code at tag `v1.0.0` (repo root `../`). Every item carries a file reference, relative to the repo root.
Abbreviations: `S/` = `src-tauri/src/`, `F/` = `src/lib/`, `L/en` = `src/lib/shared/locales/en.ts`.
Numbers in section 13 are quoted from the project docs (`docs/en/`). Nothing below is invented. Open points are listed in section 15.

---

## 1. What LyricHub is

- A Windows overlay that shows the current Spotify song with synced, karaoke-style lyrics on top of games, in sync with what you hear (`README.md`).
- It reads the playing song from the Windows media session (SMTC). It needs no Spotify login and no Spotify API (`S/sources/smtc.rs`, `docs/release-notes/v1.0.0.md`).
- Lyrics come from LRCLIB, a free community database (`S/lyrics/lrclib.rs`).
- It runs in the system tray, with a separate settings window. The overlay never takes keyboard focus away from the game (`S/overlay/win32.rs:43-46`).

## 2. Hotkeys

Defaults are in `S/settings/schema.rs:309-323` and the dispatch is in `S/hotkeys.rs:126-176`.
- All 8 global hotkeys can be changed or turned off on the Hotkeys settings page.
- A combination needs a modifier, unless it is F1–F12 (`S/hotkeys.rs:91-109`).
- While a new combination is being recorded, every hotkey is paused (`S/hotkeys.rs:224-237`).

| Key caps (default) | Action | Source |
|---|---|---|
| Ctrl + Alt + L | Show or hide the overlay. Hiding also cancels the edit mode. | `S/hotkeys.rs:146-148`, `S/overlay/mod.rs` |
| Ctrl + Alt + E | Enter the edit mode; press again to save. | `S/hotkeys.rs:150-156` |
| Ctrl + Alt + K | Next layout: Karaoke → Bar → Stack → Minimal. | `S/hotkeys.rs:157-159`, `S/settings/schema.rs:55-60` |
| Ctrl + Alt + R | Romaji on/off for Japanese lines (saved). | `S/hotkeys.rs:160-162` |
| Ctrl + Alt + , | Current song: lyrics 100 ms earlier. | `S/hotkeys.rs:163-165` |
| Ctrl + Alt + . | Current song: lyrics 100 ms later. | `S/hotkeys.rs:166-168` |
| Ctrl + Alt + Shift + , | Current song: lyrics 25 ms earlier. | `S/hotkeys.rs:169-171` |
| Ctrl + Alt + Shift + . | Current song: lyrics 25 ms later. | `S/hotkeys.rs:172-174` |

Fixed keys, registered only in certain modes because the overlay never holds the focus (`S/hotkeys.rs:1-7`, `:240-247`):

| Key | When | Action |
|---|---|---|
| Enter | edit mode | Save position and size |
| Esc | edit mode | Cancel |
| Space | calibration or sync measurement | Tap at the line start you hear |
| Esc | calibration or sync measurement | Stop |

Labels on the Hotkeys page: "Overlay on/off", "Edit mode (again: save)", "Switch layout", "Romaji on/off", "Song: lyrics 100 ms earlier/later", "Song: lyrics 25 ms earlier/later" (`L/en`, `F/settings/pages/HotkeysPage.svelte:123-168`). Each row has Change, Off and Default buttons, plus "Reset all to default" and a "Fixed keys" card.

## 3. Tray icon

(`S/tray.rs`)
- **Left click:** toggles the overlay. It does not open the menu (`:192`, `:230-236`).
- **Right-click menu** (`:173-188`), in order:
  - an optional hint line;
  - Show overlay (check item);
  - Change position and size; while editing it reads "Save edit mode (Enter)";
  - Song: lyrics earlier, −100 ms;
  - Song: lyrics later, +100 ms;
  - Layout submenu (Karaoke / Bar / Stack / Minimal);
  - Settings …;
  - Run overlay self-test;
  - Quit.
  The entries show the configured key combinations.
- **Tooltip:** "LyricHub – Title – Artist", or "LyricHub – Spotify is not playing" (`:350-367`).
- **Grey icon:** shown while no Spotify session exists (`:268-277`).
- **Hint line and tooltip:** appear when the overlay stays covered for 2 s or a game runs in exclusive fullscreen (`S/overlay/guard.rs:309-326`).

## 4. Overlay window behaviour

- **Never steals focus:** created with focus off and not focusable, shown with `SW_SHOWNOACTIVATE`, and always carries NOACTIVATE + TOOLWINDOW + TOPMOST. It is not in Alt+Tab or the taskbar (`src-tauri/tauri.conf.json`, `S/overlay/win32.rs:43-46`, `:179-184`).
- **Click-through:** on by default and off only in edit mode (`S/overlay/mod.rs:198`, `:446`).
- **Stays on top:**
  - Out-of-process WinEvent hooks (`EVENT_OBJECT_REORDER`, `EVENT_SYSTEM_FOREGROUND`, no injection) put the overlay back on top at most 10× per second (`S/overlay/guard.rs:45`, `:143-147`).
  - The guard leaves menus, the taskbar, Start, Alt+Tab and IME windows alone (`:55-70`).
  - A 500 ms watchdog restores lost window styles and visibility (`S/overlay/mod.rs:579-617`).
- **"Only with a fullscreen app" (option):** hides the overlay whenever the foreground window is not a borderless or exclusive fullscreen app. Edit mode and the settings preview are exempt (`S/overlay/guard.rs:329-371`, `S/overlay/mod.rs:408-426`).
- **Pause:** after 2 s paused, the overlay dims to 60 % opacity. After "Hide when paused after" (default 10 s, 0 = never) it fades out. The fade is a 400 ms ease (`F/overlay/Overlay.svelte:45`, `:107-113`, `:186-198`, `:545-548`).
- **Multi-monitor and DPI:**
  - Monitor choice is stored by display name, with fallback to the primary monitor (`S/overlay/window.rs:83-87`).
  - Size scales with the monitor: `scale × min(width, height) / 1080` (`S/overlay/window.rs:45-48`).
  - Resolution and DPI changes are picked up live (`S/overlay/guard.rs:206-214`, `S/lib.rs:124-129`).
- **Sleep and wake:** on resume, LyricHub reconnects to Spotify and places the overlay again (`S/overlay/guard.rs:216-226`).
- **Start:** the overlay is visible at every start. The documented start-to-visible time is 0.46 s (`docs/en/checks.md:29`).

## 5. Edit mode

(`F/overlay/EditFrame.svelte`, `F/overlay/editMode.ts`, `S/overlay/mod.rs:440-545`, `S/overlay/window.rs`)
- **Opening:** Ctrl+Alt+E, the tray or the settings button. The overlay window then covers the whole monitor, which is dimmed with `rgba(6,8,12,.35)`. The game keeps the keyboard focus.
- **Frame:** dashed amber outline `rgba(255,211,110,.95)`.
- **Handles:** corner handles (14×14, `#ffd36e`) scale; side handles (10×26) change the width.
- **Snap points:** nine snap markers (crosses), with a 14 px snap distance and a 24 px margin. A marker glows when near and turns amber and scales up when hit.
- **Toolbar:**
  - Title "Position and size".
  - Readout like "Top center · Width 35% · Size 1.00".
  - Help "Drag to move · Corners scale · Sides change the width · Markers snap" (`L/en:68-72`).
  - Buttons: Save [Enter], Cancel [Esc], and "Next monitor" when there is more than one.
- **Saving:** stores the anchor (one of 9 cells, picked by the third of the screen the centre is in), offsets in %, width in %, scale and monitor.
- **Limits:** scale 0.5–3, minimum width 200 px.

## 6. Layouts

(`F/overlay/layouts/*.svelte`, `F/shared/types.ts:40-47`)

| Layout | Content |
|---|---|
| Karaoke (default) | Header (24 px cover, title – artist), active line 28 px / 650 with karaoke fill, next line 18 px / 500 dimmed |
| Bar | One row: cover · title – artist │ current lyric line (18 px). No karaoke fill and no next line. |
| Stack | Header, previous line (dimmed), active line with fill, two upcoming lines; the list scrolls up on each line change |
| Minimal | Active line only, no plate, outlined text; "Title – Artist" shows only while no line is shown |

- Switch layouts with Ctrl+Alt+K, the tray submenu or the Overlay settings page.
- **Japanese lines with romaji on:** the romaji replaces the Japanese text in the active and next rows (`F/overlay/lyricsTimeline.ts:55-57`).

## 7. Karaoke animation and on-screen states

- **Fill** (`F/overlay/KaraokeLine.svelte`):
  - The accent-coloured copy of the line is revealed by two opposing transforms, so the text stands still and only the lit part grows.
  - The leading edge is soft (18 px mask).
  - It is driven by Web Animations with `steps(N)` at 30 updates per second of song time.
  - It follows the playback clock: pausing freezes it and seeking jumps it.
- **Word by word:** the fill moves through the words at their estimated times (`F/overlay/wordFill.ts:31-45`; times from `S/lyrics/words.rs`: 60 ms lead-in, 120 ms tail, kana weight 1, kanji 1.5). Real word times are used when an Enhanced-LRC file has them (`S/lyrics/parser.rs:169-196`).
- **Word glow:** only with real word times (`F/overlay/KaraokeLine.svelte:84-102`). Open point 15.4.
- **Line change:** the new line slides up 12 px and fades from 0.55 to 1 over 220 ms with `cubic-bezier(0.2,0.8,0.2,1)`. The next line fades in, and Stack scrolls the list. The fill itself never eases (`F/overlay/layouts/ActiveRow.svelte:95-107`).
- **Song change:** the old content fades to 15 % at the first sign of a new song. The new header fades in over 300 ms and the cover scales 0.96 → 1 (`F/overlay/Overlay.svelte:49`, `:561-566`; `F/overlay/TrackHeader.svelte`).
- **Loading:** three pulsing dots, at most 1.5 s (`F/overlay/layouts/ActiveRow.svelte:28-51`).
- **No synced lyrics:** "No synced lyrics found" for 4 s, then the header only (`L/en:57`).
- **Instrumental:** "Instrumental" for 4 s. An empty lyric line shows ♪ (`L/en:58`).
- **Lyrics service unreachable:** a small hollow dot in the header (tooltip "Lyrics service unreachable"). Failed lookups are retried after 5 s, 30 s and 120 s (`F/overlay/TrackHeader.svelte:105-111`, `S/lyrics/service.rs:17-19`).
- **Waiting for Spotify:** "LyricHub – waiting for Spotify" for 3 s when the overlay is shown with nothing playing (`L/en:54`).
- **Ads:** the overlay hides and no lyrics are looked up (`S/sources/types.rs:25`, `:40-43`; `F/overlay/Overlay.svelte:111`).
- **Podcasts:** no lyrics lookup (`S/sources/types.rs:26`, `:44-46`). Open point 15.5.
- **Toasts** (1.5 s pill in the accent colour):
  - "Song offset +100 ms · Lyrics later" (or "earlier") after a nudge;
  - "Romaji on" / "Romaji off" (`L/en:61-63`).
- **Reduced motion:** all transitions shrink to 80 ms and the word glow turns off (`F/overlay/Overlay.svelte:115`, `:537-540`).

## 8. Appearance and customization

(`F/overlay/theme.ts:37-113`, `F/settings/pages/AppearancePage.svelte`, `S/settings/schema.rs:162-246`)

| Theme | Plate | Text | Outline | Accent fallback |
|---|---|---|---|---|
| Glass Dark (default) | rgb(12,12,16) at plate opacity (default 0.55), radius 14 | white | no | #ffd36e |
| Glass Light | white, opacity at least 0.7 | #111111 | no | #9a6700 |
| Neon | none | white | yes, plus a strong accent glow | #ffd36e |
| Mono | black, opacity at least 0.85, radius 10 | white | no | white (no accent) |
| Custom | own plate colour (on/off) | own text colour | on/off | fixed colour; radius 0–32 px |

- **Accent colour:** taken from the album cover (default) or fixed.
  - Cover algorithm (`F/overlay/accent.ts`): 32×32 sample, k-means with 4 clusters, the most vivid cluster.
  - The hue is pushed away from Spotify green, saturation is raised, and lightness is stepped until the contrast reaches 4.5:1 against the plate.
- **Plate opacity:** slider 0–100 %.
- **Font:** "Inter (bundled)" or any installed font.
- **Reduce motion:** checkbox.
- **Live preview:** while the Appearance page is open, the overlay shows sample lines in the chosen look (`L/en:107`).

## 9. Sync tools

- **Offsets:** global + per audio device + per song, each limited to ±5000 ms. Positive means the lyrics show later (`S/sync/offsets.rs:8-26`, `S/app_state.rs:49-50`).
- **Nudge per song:**
  - Hotkeys or tray, ±100 ms or ±25 ms.
  - Stored per song and used again when the song plays again.
  - The next song starts with its own value, so a correction never carries over (`S/app_state.rs:246-259`).
- **Audio devices:**
  - One offset per output device, switched automatically when the device changes.
  - A new Bluetooth device starts at +150 ms, with a notice (`S/audio_devices.rs:23-24`, `:111-143`).
- **Calibration assistant:**
  - Tap Space at 5 line starts you hear; outliers over 600 ms are dropped.
  - The median is suggested as the offset for the current device. It needs synced lyrics (`S/calibration.rs`).
  - Overlay text: "Calibration {n}/{needed} · Space when you hear the next line start · Esc ends". A countdown bar shows in the Karaoke layout.
- **Sync measurement (protocol):** unlimited taps, written to a JSON log for analysis (`S/calibration.rs:258-285`).
- **Sync Lab (Diagnostics):**
  - A 60 s chart of the clock against the reports, ±300 ms.
  - Buttons "Show as table" and "Export diagnostics" (a JSON with no lyrics text) (`S/sync_lab.rs`, `F/settings/SyncLab.svelte`).
- **Clock** (`S/sync/clock.rs:22-28`):
  - extrapolates between Spotify's position reports;
  - ignores corrections under 20 ms;
  - smooths corrections up to 300 ms at ≤ 10 % speed;
  - jumps on seeks.

## 10. Lyrics

- **LRCLIB lookup** in up to five stages, started 350 ms apart (`S/lyrics/lrclib.rs:160-243`):
  1. exact match;
  2. without album;
  3. normalized title;
  4. field search;
  5. free-text search.
  The first synced hit wins. Each request has a 4 s timeout and a 4 MB answer limit.
- **Matching:** duration within 8 s, title similarity ≥ 0.85 and artist similarity ≥ 0.6 (`S/lyrics/matcher.rs:7-11`).
- **Local cache:** SQLite. Misses are kept 7 days; songs unseen for a year are pruned. "Reload lyrics" bypasses the cache (`S/lyrics/cache.rs`, `S/lyrics/service.rs`).
- **Romaji:** offline conversion of Japanese lines (lindera + IPADIC, modified Hepburn), toggled with Ctrl+Alt+R (`S/lyrics/romaji.rs`).

## 11. Settings window

(`F/settings/App.svelte`, `S/settings_window.rs:30-36`)
- **Window:** 960×640, dark (`#111318`), with a 232 px sidebar (logo, now-playing card, navigation, version).
- **Lifetime:** the window exists only while it is open.

| Page | Controls |
|---|---|
| Overlay | Start edit mode; Layout; Monitor; Anchor (3×3); Fine position X/Y (−50..50); Width (10–100 %); Scale (0.5–3); Japanese lines (romaji); Hide when paused after (0–3600 s); Only with a fullscreen app; Overlay on/off; Reset position and size |
| Appearance | Theme cards (Glass Dark, Glass Light, Neon, Mono, Custom); Accent colour (from the cover / fixed); Plate opacity; Font; Reduce motion; Custom: plate, text colour, outline, corner radius |
| Sync | Offset for this song (Reset to 0); Audio devices; Calibrate (Start / Apply); Sync log (Start / Stop measurement); Global offset (−2000..2000 slider, ±5000 box, Earlier ↔ Later); Lyrics of the current song (Reload lyrics) |
| Hotkeys | 8 rows with key caps, Change / Off / Default, Reset all, Fixed keys |
| Behavior | Start with Windows; Start in the tray even when started manually; Language (System / Deutsch / English); Files (settings and logs paths, Open logs) |
| Diagnostics | Spotify info, Sync Lab, lyrics state, track-change latency, overlay window info, Run self-test |

## 12. App lifecycle, installer, licence

- **Start-up:**
  - Autostart (setting, starts minimized) (`S/lib.rs:46-49`).
  - Single instance: a second start opens the settings (`S/lib.rs:40-44`).
  - Closing the settings window keeps the app in the tray.
- **Languages:** German and English. "System" follows the Windows display language (`S/i18n.rs:30-35`).
- **Files:** settings in `%APPDATA%\LyricHub\settings.json` (written atomically); daily logs, 14 kept.
- **Installer:**
  - NSIS for the current user only, so no admin rights are needed (`src-tauri/tauri.conf.json`).
  - About 10 MB, version 1.0.0.
  - Not signed, so Windows SmartScreen warns once (`docs/release-notes/v1.0.0.md`).
- **Licence:** source available, MIT + Commons Clause (no selling) (`LICENSE`).

## 13. Tests and reliability

- **93 Rust unit tests** (`#[test]` count in `S/`). Verified on 2026-10-26: `cargo test` reported "93 passed; 0 failed" in 0.37 s.

  | Module | Tests |
  |---|---|
  | lyrics | 46 |
  | sync | 18 |
  | settings | 11 |
  | overlay window | 8 |
  | sources | 7 |
  | hotkeys | 2 |
  | Sync Lab | 1 |

  The LRCLIB tests use recorded answers and never touch the network.
- **Overlay self-test** (`S/overlay/selftest.rs`), started from the tray, the settings or the command line. It checks:
  - focus kept;
  - window styles;
  - overlay drawn (pixel comparison);
  - click-through;
  - Alt+Tab exclusion;
  - hide and show;
  - staying on top over a simulated fullscreen game.
- **Spike scenarios** (`tools/overlay-spike/run-spike.ps1`, `docs/spike-overlay-*.json`):

  | Scenario | Result |
  |---|---|
  | No game | 13/13 |
  | Game that sets topmost once | 15/15 |
  | Game that forces itself on top every 500 ms | 15/15, overlay on top in 120 of 120 samples |

  Before the WinEvent guard, polling every 100 ms reached only 82 %.
- **Edit-mode test** (`tools/overlay-spike/edit-mode-test.ps1`): 13 checks with synthetic mouse and keys.
- **Measured numbers:**

  | Measurement | Result | Source |
  |---|---|---|
  | Median sync error (13 songs, 115 taps, wired) | 96 ms | `docs/en/sync-report.md:12` |
  | Seek correct again | median 30 ms | `docs/en/sync-report.md:14` |
  | SMTC report to drawn line | 8–25 ms | `docs/en/sync-report.md:17` |
  | CPU while playing, of all cores | 0.48 % | `docs/en/phase5.md:40` |
  | CPU while paused, of all cores | 0.03 % | `docs/en/phase5.md:39` |
  | RAM for app + WebView2 | 179 MB | `docs/en/phase5.md:41` |
  | Lyrics from cache | about 3 ms | `docs/en/phase5.md:35` |
  | Online lookups under 1.5 s | 76 % | `docs/en/phase5.md:36` |
  | Start to visible overlay | 0.46 s | `docs/en/checks.md:29` |

## 14. Visual spec used for the recreation

- **Overlay font:** "Inter Variable" (bundled), fallback Segoe UI Variable. Japanese uses Yu Gothic UI and Meiryo UI (`F/overlay/Overlay.svelte:529`).
- **Sizes at scale 1** (on 1080p):
  - header 13 px (title weight 600), cover 24 px with radius 5;
  - active line 28 px / weight 650 / line height 1.25;
  - next line 18 px / 500;
  - plate padding 8 / 14 px, radius 14, default width 35 % of the screen, anchored top centre, 2.2 % from the top.
- **Text alphas (Glass Dark):**

  | Variable | Alpha |
  |---|---|
  | `--text` | 0.92 |
  | `--text-meta` | 0.75 |
  | `--text-base` (unsung) | 0.55 |
  | `--text-next` | 0.5 |

  The text shadow is `0 0 2px rgba(0,0,0,.8), 0 2px 8px rgba(0,0,0,.7)`.
- **Accent:** #ffd36e by default; with "From the cover" it follows the cover colour.
- **Settings window colours:**

  | Element | Colour |
  |---|---|
  | Background | #111318 |
  | Sidebar | #0d0f13 |
  | Card | #1a1d24 |
  | Line | #2a2f3a |
  | Text | #e8eaf0 |
  | Muted | #9aa1b2 |
  | Accent | #ffd36e on #1a1400 |
  | OK | #4ade80 |
  | Fail | #f87171 |
- **App icon "Half-Lit Notes":** `src-tauri/icons/app-icon.svg`, LyricHub's own artwork.

## 15. Open points and decisions

**Decided on 2026-10-26.** The user approved the proposals below, with one change: exclusive fullscreen works. The user tested it in a current shooter in exclusive fullscreen, and the overlay was visible. The video therefore says "borderless, windowed and fullscreen games" and names no game.

1. **Sync accuracy wording.** The median target (≤ 100 ms) is met at 96 ms. The p95 target (≤ 200 ms) is **not** met (289 ms); the report blames LRC timing per song and tap spread. Proposal: the video states "96 ms median" and "30 ms after a seek" and makes no p95 claim.
2. **Exclusive fullscreen.** The release notes list it as a known limitation. **Resolved:** the user confirmed the overlay works over a game in exclusive fullscreen, so the video claims fullscreen support. The tray hint for a covered overlay is shown as the fallback.
3. **"Only with a fullscreen app".** The code hides the overlay whenever the foreground window is not fullscreen, not just on the desktop as the README says. The video will say "shows only while a fullscreen game is in front".
4. **Word glow.** It only appears with real word timing, which LRCLIB practically never has (0 of 160 hits, `docs/en/phase4.md:59`). Proposal: show the estimated word-by-word fill and leave the glow out.
5. **Podcasts.** There is no podcast-specific overlay UI, only the skipped lookup. Proposal: mention it in one line ("no lookups for ads and podcasts") or leave it out.
6. **Windows 10.** It is untested (the README says it "should work"). Proposal: say "for Windows" without naming a version.
7. **Settings window font.** The code lists Inter first, but only the overlay loads the font, so the real settings window may render in Segoe UI. Proposal: Segoe UI style in the settings mock-ups.
8. **Stale release notes.** The release notes mention only three changeable hotkeys; the code makes all eight changeable. The video follows the code.
9. **Mentioning Spotify.** The plan is to name Spotify in voice-over and plain text only (nominative use): no logo, no Spotify green, plus a short "not affiliated" line in the outro. Please confirm.

## 16. Copyright-safe substitutions (planned)

- **Songs:** invented titles and artists only, e.g. "Paper Satellites – Neon Tide".
- **Lyrics:** original placeholder lines written for this video. The Japanese demo line is LyricHub's own sample text ("次の歌詞はここに少し小さく表示されます" / romaji "tsugi no kashi wa koko ni sukoshi chiisaku hyōji saremasu", `F/overlay/Overlay.svelte:42-44`).
- **Covers:** abstract gradient squares, not album art.
- **Game footage:** an abstract, generated "game scene" (shapes, parallax, HUD-like placeholders), with no real game, logo or UI.
- **Spotify:** no logo and no green; LyricHub itself already steers cover accents away from Spotify green.

## 17. Draft scene outline (for your feedback, timings rough)

| # | Scene | Content | ~Duration |
|---|---|---|---|
| 1 | Hook | Abstract game scene; a karaoke line lights up at the top | 8 s |
| 2 | What it is | Overlay + tray + "reads Spotify via Windows, no login" | 14 s |
| 3 | Setup | Installer (current user, no admin), tray icon, settings window opens | 14 s |
| 4 | Overlay in action | Karaoke layout: fill, word-by-word, line change, song change with cover accent | 22 s |
| 5 | Never in the way | Click-through, no focus steal, stays on top (guard), Alt+Tab exclusion | 16 s |
| 6 | Hotkeys | Key caps for all 8 and the fixed keys, each with its on-screen effect | 34 s |
| 7 | Edit mode | Drag, scale, width, snap markers, Save / Cancel, next monitor | 16 s |
| 8 | Behaviour | Pause dim and hide, no synced lyrics, instrumental, loading, waiting for Spotify, ads hidden, offline dot, only-fullscreen rule | 26 s |
| 9 | Layouts | Karaoke, Bar, Stack, Minimal | 14 s |
| 10 | Appearance | 5 themes, cover accent vs fixed, opacity, font, custom, reduced motion, live preview | 24 s |
| 11 | Sync tools | Offsets model, nudge per song, device offsets, calibration, Sync Lab, export | 26 s |
| 12 | Japanese and romaji | Japanese line → romaji with Ctrl+Alt+R | 10 s |
| 13 | Settings tour | Overlay, Appearance, Sync, Hotkeys, Behavior, Diagnostics; languages | 18 s |
| 14 | Tests and reliability | 93 unit tests, self-test, spike scenarios (120/120), measured numbers | 22 s |
| 15 | Outro | Installer size, licence (no selling), free, link to the repo; Spotify disclaimer | 10 s |

Rough total: about 4.5 minutes. The final timings follow the voice-over script, at 150 words per minute.
