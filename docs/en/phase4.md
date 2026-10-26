# Phase 4 – Karaoke polish and design

*English translation of [phase4.md](../phase4.md).*

> Date: 2026-10-26 · Reference: [PLAN.md](../../PLAN.en.md) 10 (Phase 4), 5.6, 9.1–9.7
> Result: **Phase 4 complete** (tag `v0.4.0-phase4`). The fill runs word by word. There are five themes with an accent color from the cover, four layouts, the "Appearance" page („Darstellung“) with a live preview, and German and English. The fill triggers no layout per frame, and the CPU load is 0.48 % of all cores. The user declared the in-game FPS test unnecessary.

Before Phase 4, the romaji display came from `claude/unruffled-dirac-66618b` into main (merge `9a3d938`, see [romaji.md](romaji.md)). With it, the pending Japanese sync measurement was done ([sync-report.md](sync-report.md), run 3).

## What was built

| Area | Implementation | Files |
|---|---|---|
| Word timings (5.6) | Lines without Enhanced LRC tags get estimated word timings. Latin, Cyrillic and Hangul are split at spaces, kana and kanji per character. Weights: letter and kana 1, kanji 1.5, punctuation 0 (attached to the word before). 60 ms lead-in, 120 ms tail. Romaji lines get their own word timings. | `lyrics/words.rs`, `parser.rs`, `romaji.rs` |
| Word-by-word fill (9.3) | The fill keyframes have stops at the word edges, measured once per line as a fraction of the text width. That way they hold at any font size. Still one compositor animation per layer, 30 steps per second, no JavaScript per frame. | `overlay/wordFill.ts`, `KaraokeLine.svelte` |
| Word glow (9.4) | Only with real word timings: a copy with a text shadow fades in per word in 120 ms and fades out 300 ms after the end of the word, via opacity. | `KaraokeLine.svelte` |
| Themes (9.5) | Glass Dark (default), Glass Light, Neon, Mono, Custom. The themes are CSS variables on the stage; the layouts read plate, text levels, accent, outline and glow from them. The "Minimal" layout always has an outline and no plate. | `overlay/theme.ts`, `color.ts`, layouts |
| Accent from the cover (9.5) | The cover is scaled down to 32×32, and k-means finds four colors. The most vivid one that covers at least 6 % of the cover is chosen. Its lightness is adjusted until it has at least 4.5:1 contrast against the plate. Spotify's green is shifted by 8°. Gray covers get the fallback accent. | `overlay/accent.ts` |
| Layouts (9.2) | New: "Stack" („Stapel“) (previous line, current line, two upcoming lines; smooth scrolling, 220 ms) and "Minimal" (only the active line, without a plate). The header and the active line are shared building blocks; the tray and Ctrl+Alt+K cycle through all four. | `layouts/Stack.svelte`, `Minimal.svelte`, `TrackHeader.svelte`, `ActiveRow.svelte` |
| Font (9.5) | Inter Variable (OFL-1.1) ships with the app (`@fontsource-variable/inter`). Custom font from the system list via DirectWrite. Is Yu Gothic UI or Meiryo UI installed? If both are missing, the Appearance page names the matching Windows feature. | `fonts.rs`, `routes/overlay/+page.svelte` |
| Motion (9.4) | Line change 220 ms, song change 300 ms with the cover going from 0.96 to 1, pause dims after 2 s over 400 ms. "Reduce motion" („Bewegung reduzieren“) in the settings or in Windows: all transitions at most 80 ms, no word glow. | `Overlay.svelte`, layouts |
| States (9.6) | If the lyrics service is unreachable, the header shows a small hollow dot; the details are in the settings. Loading, instrumental, no synced lyrics and ads have existed since Phase 2 and 3. | `TrackHeader.svelte` |
| "Appearance" page (9.7) | Theme cards with a mini preview, accent from the cover or fixed, plate opacity (moved here from the Overlay page), font, "Reduce motion", custom theme (plate, colors, outline, radius). While the page is open, the overlay shows sample lines in the current look; closing the window ends the preview. | `AppearancePage.svelte`, `set_overlay_preview` |
| Language (i18n) | `behavior.language`: System, German, English. "System" follows the Windows display language. 247 strings per language in `locales/de.ts` and `en.ts`; the German ones are unchanged. Both windows switch immediately. The backend translates the tray menu, notices and errors itself. | `shared/i18n.svelte.ts`, `locales/`, `src-tauri/src/i18n.rs` |
| Test tool | `layout-count.mjs` counts the layouts and style recalculations of the overlay page via the WebView2 DevTools protocol. | `tools/overlay-spike/` |

## Definition of Done

| Criterion | Result | Evidence |
|---|---|---|
| Design review against 9.1–9.6 | **met**, see the table below. All layouts and themes checked by screenshot over a real desktop (Steam, Explorer). Found and fixed along the way: outline and shadow were missing in Minimal and Neon, because the scale `--s` was only set on the plate. | Screenshots (not in the repo; they show third-party windows) |
| No layout per frame | **met**: 6 layouts and 38 style recalculations in 30 s of playback, 0.2 layouts per second (one per frame would be 30). | `node tools/overlay-spike/layout-count.mjs 30` |
| In-game FPS test without measurable loss | **dropped** (user decision: "We don't need an FPS test, that's fine" („FPS-Test brauchen wir nicht, das passt“)). Indications: CPU 0.48 % of all cores, fill only on the compositor. | – |
| `run-spike.ps1` green | plain 13/13, game-once 15/15, game-aggressive 13/14 (known worst case from Phase 0: "Edit mode catches clicks" („Edit-Modus fängt Klicks ab“) with aggressive Topmost) | `docs/spike-overlay-*.json` |
| Edit mode still intact | 13 of 13 | `edit-mode-test.ps1` |
| CPU ≤ 2 % while the fill runs | **met**: 7.7 % of one core = 0.48 % of 16 cores over 60 s, Japanese song with romaji | `measure-cpu.ps1 -Seconds 60` |
| Unit tests | 90 green (new: 5 for word estimation, settings validation extended) | `cargo test` |

Memory: 196 MB private for LyricHub and all WebView2 processes. That is just under the 200 MB limit from section 2, measured with debug access switched on. The romaji dictionary sits in the mapped image and does not count as private memory. The proof belongs to Phase 5.

### Design review

| Section | Item | Status |
|---|---|---|
| 9.1 | Readable over any game | Plate in Glass and Mono; outline with halo and shadow in Neon, Minimal and, on request, in Custom. Checked over a bright Steam header and a dark Explorer. |
| 9.1 | Calm | Only the fill moves continuously; everything else ≤ 300 ms, then still. |
| 9.1 | Time is truth | Fill is linear between the word stops, without easing; easing only on line changes. |
| 9.1 | Good defaults | Glass Dark with accent from the cover, Karaoke, Inter. |
| 9.2 | Four layouts | Karaoke, Bar, Stack (4 lines), Minimal. |
| 9.3 | Fill technique | Two layers, clip via `translateX`, soft edge via `mask-image`, no `background-clip: text`, 30 steps per second. Inter Variable, weight 650/500 without a layout jump. |
| 9.3 | Word glow | With real word timings, via opacity instead of an animated shadow (see deviations). |
| 9.4 | Motion | Line change 220 ms `cubic-bezier(.2,.8,.2,1)`, song change 300 ms with cover scale, pause 2 s → 0.6 in 400 ms, toast 1.5 s, reduced motion ≤ 80 ms without glow. |
| 9.5 | Typography | 28 px at scale 1 on 1080p, line height 1.25, `line-break: strict`, fallback chain with Yu Gothic UI and Meiryo UI. |
| 9.5 | Themes and accent | Five themes, accent by the method described above with a contrast check, Spotify green avoided. |
| 9.6 | States | Loading ≤ 1.5 s, no synced lyrics 4 s, instrumental 4 s, ads off, network error as a subtle marker, edit mode as in Phase 1. |

## Deviations from the plan

- **Word estimation not calibrated:** Phase 2 suggested calibrating the estimate against LRCLIB songs with word timings. LRCLIB provides the fields `hasWordSync` and `lyricsfile`, but for eight well-known songs with 20 hits each, not a single hit had word timings (0 of 160). The parameters therefore follow PLAN.md 5.6.
- **Estimation in the backend:** The estimation runs in Rust, with unit tests. The fill in the overlay now only measures the word edges. The `estimated` field on the word tells real timings from estimated ones.
- **Word glow via opacity:** 9.3/9.4 describe an animated `text-shadow`. That would repaint during every glow. Instead, each word has a copy with a fixed shadow on top, and the compositor animates its opacity.
- **Outline 3.5 px instead of 2 px** (at scale 1): The stroke lies behind the fill (`paint-order`), so only about half of it is visible. A tight halo shadow is added. 2 px was too thin over bright scenes.
- **Next line on a line change:** It fades in instead of moving up 12 px. The active line rises by 12 px and fades in from 0.55, as in 9.4.
- **No bundled Noto Sans JP subset:** Yu Gothic UI is present on the user's PC. If Yu Gothic UI and Meiryo UI are missing, the Appearance page shows the Windows feature "Japanese Supplemental Fonts". Bundling a font would make the app several MB larger.
- **Opacity applies to all themes with a plate:** Glass Light uses the same slider instead of a fixed 0.7; Mono has at least 85 %.
- **Option to show "Ad"** („Werbung“) is missing: The user has Premium (14.1); ads hide the overlay.
- **Network error marker** only in Karaoke and Stack, which have a header. Bar and Minimal go without it.
- **Self-test report stays German:** It is a developer diagnostic, and the spike scripts read it.
- **Toast in the header instead of a pill below the overlay:** as since Phase 2, so that the window keeps its size.

## Open items

- The word-by-word fill is an estimate based on the number of letters. How well it matches the singing depends on the song (tempo within the line, melismas). Only Enhanced LRC files or forced alignment provide real word timings (PLAN.md 13.3).
- A "Hotkeys" page (9.7, item 6) and "Only with a fullscreen app" („Nur bei Vollbild-App zeigen“) follow in Phase 5.
- `npm audit` reports three low-severity vulnerabilities in `cookie` via `@sveltejs/kit`. The fix would need Kit 3 (breaking change); the app does not serve any cookies.
