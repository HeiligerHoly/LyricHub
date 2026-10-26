# Security, performance and disk space

*English translation of [checks.md](../checks.md).*

> Date: 2026-10-26 · State: after `v0.5.0-phase5` · release build, Windows 11, 16 cores

## Security

| Check | Result |
|---|---|
| Content Security Policy | **Was off** (`csp: null`), now on: scripts, styles, fonts and images only from the app itself (inline styles for Svelte, `data:` images for the covers), IPC through `ipc:`. An inline script injected into the page is blocked in both windows (checked in the release build). The dev server still runs without a CSP. |
| Unfiltered HTML | None: no `{@html}`, no `innerHTML`, no `eval`. Title, artist and lyrics reach the DOM as text. |
| Tauri permissions | Only `core:default` for both windows, no file system, shell or opener access from the page. The backend opens folders itself. |
| Input from the settings | Colours checked to `#rrggbb`, font name without characters that could end the CSS, shortcuts only with a modifier (except F1–F12), numbers clamped (`sanitize`, unit tests). |
| Network | Only LRCLIB over HTTPS (native TLS with certificate checks), own user agent, 4 s timeout. New: answers at most 4 MB instead of unlimited. No telemetry, no other connections. |
| Covers | From the Windows media session, at most 4 MB, MIME type taken from the bytes, used only as a `data:` image. |
| SQL | All queries use parameters; no commands built from strings. |
| Games | No DLL injection, no hooks inside the game process: the WinEvent hooks run outside of it (`WINEVENT_OUTOFCONTEXT`). |
| Rust dependencies | `cargo audit`: 558 crates, **0 vulnerabilities**. 5 warnings (unmaintained or unsound: `glib`, `zbus_names`, `zvariant`, `zvariant_derive`, `proc-macro-error`) concern only Tauri's Linux part and are not in the Windows build (`cargo tree --target x86_64-pc-windows-msvc`). |
| npm dependencies | What ships in the app (`npm audit --omit=dev`): **0 vulnerabilities**. In the build tools, 2 low-severity issues remain in `@sveltejs/kit` itself (form actions, cache headers of prerendered pages); the app uses no server rendering and no forms. `cookie` is raised to 0.7.2 by an override. |
| Secrets and personal data in the repository | No keys, tokens or passwords. Paths only through `%APPDATA%`/`%LOCALAPPDATA%`. **Note:** older versions of the spike reports in the Git history contain the window title of a Discord server. Before publishing, create a fresh repository or clean the history (the user's decision). |
| Debug access | The DevTools port exists only if you set it yourself through an environment variable (development); it is off in release. |
| Installer | Current user only, no administrator. Not signed: SmartScreen warns about an unknown publisher. |

## Performance

| Measurement | Result |
|---|---|
| Start until the overlay is visible | 0.46 s; lyrics from the cache after 0.49 s |
| CPU playing / paused | 0.48 % / 0.03 % of all 16 cores (targets 2 % / 0.5 %) |
| Private memory (LyricHub + WebView2) | 169–179 MB (target < 200 MB) |
| Layouts of the overlay page | 0.2 per second during playback, so none per frame (phase 4) |
| Release profile | LTO, one codegen unit, `opt-level = 3`, `panic = "abort"`, symbols stripped (since phase 0) |

## Disk space

| Area | Before | After |
|---|---|---|
| WebView2 folder (`%LOCALAPPDATA%\dev.lyrichub.app`) | 91 MB | 23 MB: `--disable-component-update` no longer downloads DRM, filter, speech or hyphenation components; LyricHub removes existing ones at start; HTTP cache capped at 1 MB |
| Logs | debug output in release too, about 1.4 MB per day with 14 days kept | only `info` in release (about a quarter), debug only in development builds; only the ten newest self-test reports are kept |
| Lyrics database | write-ahead log up to 4 MB, cache unlimited | WAL at most 1 MB (checkpoint every 200 pages); lyrics of songs not played for a year and expired misses are removed at start. Song and device offsets stay. |
| Program file | 57 MB | unchanged, on purpose: 45 MB of it is the romaji dictionary (IPADIC). Embedded uncompressed it stays in the mapped program image; compressed it would be about 7 MB, but it would then sit unpacked in private memory (+45 MB) and push the app over 200 MB ([romaji.md](romaji.md)). |
| Installer | 10 MB | unchanged (NSIS compresses the dictionary) |
