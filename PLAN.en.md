# LyricHub – Implementation plan for Opus

*English translation of [PLAN.md](PLAN.md), the plan with the user's decisions and the results of each phase. The plan as first written is in [docs/original-plan](docs/original-plan/).*

> As of 2026-10-25. Written by Fable as the working basis for Opus.
> Language in code/comments: English. Language of the UI: German + English (i18n-ready, German first).
> Target platform: Windows 11 (Windows 10 version 2004 and later where possible).
> Updated on 2026-10-25 with the user's answers. The decisions are in section 14.

---

## 0. Summary

LyricHub is a **game overlay for Windows**. It detects what is currently playing in Spotify and shows, at the edge of the screen (position, size and monitor freely selectable in the settings), the current song and its lyrics, running along **in sync with the audible audio output**. The line currently being sung lights up **karaoke-style** (a smooth fill moving across the text).

The three hard core problems, in order of importance:

1. **Sync**: The lyrics must match what the user *hears*, not what an API reports. For this there is a dedicated time model (`PlaybackClock`) with source fusion, a jitter filter and three offset levels (global, per audio device, per song).
2. **Overlay technology**: Transparent, always on top, click-through, **must never steal focus from the game**, survives fullscreen switches and multi-monitor setups.
3. **Lyrics retrieval**: Timestamped lyrics (LRC) from LRCLIB with robust matching, a local cache and fallbacks.

**Central architecture decision**: The app works fully **without the Spotify Web API**. The primary source for "what is playing right now" is the Windows media session (SMTC, `Windows.Media.Control`), which Spotify fills locally. The Spotify Web API is an **optional precision and metadata layer**. Reason: In February 2026 Spotify heavily restricted Development Mode (Premium required for the app owner, max. 5 users, endpoint restrictions announced). An app that runs without the API is more robust against this policy.

---

## 1. Product definition

### 1.1 Target state (v1)

- Runs in the tray, optionally starts with Windows.
- Automatically detects title, artist, album, duration, cover and playback status from Spotify (desktop and Store version).
- Fetches synced lyrics, shows them in the overlay and highlights the current line with a smooth fill.
- Overlay position (9 anchor points + free), scale, width, monitor and layout can be set in the settings and directly on screen in edit mode.
- Sync tools: global offset, nudge hotkeys (±100 ms), offset per audio device (e.g. Bluetooth headphones), offset per song, calibration wizard, diagnostics view.
- Works over games in borderless/windowed fullscreen.

### 1.2 Non-goals (v1)

- No overlay in **exclusive fullscreen** (DirectX FSE). That would need DLL injection into the game process (like Steam/Discord), which carries anti-cheat risks. Deliberately excluded. Most games run borderless today; in addition, the Windows "fullscreen optimizations" usually let other windows draw over FSE.
- No playback control (play/pause/skip) in v1. Read only.
- No word-level timestamps from sources that do not provide them. Word-level display is **estimated** (see 5.6); real word timings only when Enhanced LRC is available.
- No access to Spotify's internal lyrics endpoints (sp_dc cookie, spclient). This violates the ToS and breaks regularly.

### 1.3 The user's setup (confirmed on 2026-10-25, details in section 14)

- A Windows PC on which Spotify runs locally (not Spotify Connect on another device; that would strictly require the Web API).
- **Spotify from the Microsoft Store**, account with **Premium**.
- Audio output mostly **wired** (low, stable latency).
- Games run in a **mix** of borderless and exclusive fullscreen.
- Lyrics languages by frequency: **English, German, Japanese**, occasionally others.
- The user accepts calibrating an offset once if the automatic sync is not perfect.
- **Personal use first.** A later free release is possible, so make no decisions that would rule out distribution (see 14.2).

---

## 2. Hard requirements and acceptance criteria

| Area | Criterion | Measurement method |
|---|---|---|
| Sync accuracy | After calibration with wired output: median error ≤ 100 ms, p95 ≤ 200 ms between the line start in the overlay and the audible onset | Manual log with 10 songs (section 11.3) |
| Sync stability | No visible jitter. Corrections are worked in via slew (max. ±10 % speed), no backward jump < 300 ms | Sync Lab diagnostics + visual inspection |
| Seek detection | Correct again within ≤ 300 ms (SMTC event) or ≤ 1 s (Web API only) after seeking | Manual |
| Track change | New title in the overlay ≤ 300 ms; lyrics ≤ 1.5 s online, ≤ 100 ms from cache | Logging |
| Focus | The overlay **never** takes input focus. Keyboard input in the game continues undisturbed, also on track changes and when edit mode ends | Test game + Notepad test |
| Overlay visibility | Stays visible over borderless fullscreen games, even when the game reasserts topmost | 3 test games + fullscreen browser video |
| Performance | Paused: < 0.5 % CPU; playing: < 2 % CPU on a mid-range CPU (share of all cores, including the WebView2 processes); RAM < 200 MB private; no noticeable FPS loss in the game | `tools/overlay-spike/measure-cpu.ps1`, 10-minute measurement |
| Robustness | Spotify restart, device change (BT headphones), monitor change, sleep: the app recovers without a restart | Manual |
| No focus stealing at startup | The app starts minimized to the tray and opens no window when autostart is active | Manual |

---

## 3. Architecture decisions (with rationale)

### 3.1 Framework: Tauri 2 (Rust backend + WebView2 frontend)

**Decision**: Tauri 2, Rust backend, frontend with Svelte 5 + TypeScript (React is acceptable if Opus is faster with it; the number of components is small).

**Why**:
- The backend needs direct access to Win32 and WinRT (SMTC, window flags, audio devices). The `windows` crate supports this first-class.
- The frontend needs first-class typography and animation (text fill with a gradient, glow, easing). CSS/Canvas in WebView2 is ideal for this and much simpler than WPF shaders.
- Tauri supports transparent, frameless, always-on-top windows and `set_ignore_cursor_events` out of the box; known pitfalls are documented (section 6).
- Small bundle, low idle load.

**Alternative** (if Tauri unexpectedly causes problems with overlay transparency): C# / WPF with `Windows.Media.Control` (native, very simple) and WPF text animations. Opus decides on its own after Spike B (Phase 0) and justifies the choice in `docs/en/spike-overlay.md` (user decision, 14.1).

**Not chosen**: Electron (heavier, transparency on Windows historically finicky, SMTC needs a native addon).

### 3.2 Source for "what is playing": SMTC primary, Spotify Web API optional

| Source | Provides | Latency | Costs/hurdles |
|---|---|---|---|
| **SMTC** (`GlobalSystemMediaTransportControlsSessionManager`) | Title, artist, album, cover (thumbnail stream), status, timeline (position + `LastUpdatedTime`, start/end = duration) | Events in < 100 ms on play/pause/change | None. No login, no network, no limit |
| **Spotify Web API** `GET /me/player` | Track ID, `progress_ms`, `is_playing`, `currently_playing_type` (ad/episode), `is_local`, cover URLs | Polling 1 s + RTT (typically 100–400 ms), jitter | OAuth PKCE, own developer app, **Premium required for the owner, 5-user limit, rate limits, policy risk** |

**Decision**: SMTC is mandatory and is enough for v1 if Spike 0 confirms this. Fallback order if the SMTC position is too inaccurate (confirmed by the user, see 14.1):

1. **Plan B**: The user switches from the Store version to the desktop version of Spotify, and the Spicetify companion (13.1) is brought forward as a precise position source.
2. **Plan C**: Spotify Web API as an additional source (Phase 3b, section 8). The user has Premium, so this path is open.

Independently of this, the Web API can be added later for stable track IDs.

**Spike A result (2026-10-25)**: SMTC is enough. Spotify publishes the timeline every 4.5 s plus on every event, and in normal operation the extrapolation deviates by 1 ms at the 95th percentile. Plan B and Plan C are dropped for now. Details and the consequences for the handling of seeks and song changes are in `docs/en/spike-smtc.md`.

**Open uncertainty that Spike 0 must resolve**: How often does the Spotify client update the SMTC timeline? Community reports (e.g. a fix in the Soggfy project) show that Spotify refreshes the timeline and that readers must extrapolate the position from `Position + (now - LastUpdatedTime)`, where refreshes can make the extrapolated position jump back slightly. The exact cadence and the jitter must be measured (section 5.8).

### 3.3 Lyrics source: LRCLIB primary, provider interface, local cache

- **LRCLIB** (`https://lrclib.net/api`): free, no API key, provides `syncedLyrics` in LRC format (line-level), `plainLyrics` and an `instrumental` flag. Exact lookup via `/api/get` (track_name, artist_name, album_name, duration; duration tolerance about ±2 s), fallback via `/api/search`. `404 TrackNotFound` when nothing is found. Set a meaningful `User-Agent` (LRCLIB asks for this, e.g. `LyricHub/0.1.0 (https://github.com/<repo>)`).
- **Provider trait** (`LyricsProvider`), so that NetEase, Musixmatch (with its own key) or local LRC folders can be added later without touching the rest.
- **Local cache** (SQLite): raw LRC, source, timestamp, user overrides (song offset, manually imported/edited LRC).
- **Manual import**: The user can store a `.lrc` file for a song.

### 3.4 Sync model: `PlaybackClock` in pure Rust, the frontend keeps computing locally

- The backend holds a time model (`Anchor`: position at a monotonic point in time, status, rate) and fuses observations from all sources (median residual filter, seek detection, slew).
- The frontend does **not** get a position per frame via IPC. Instead it rarely gets an anchor (`{pos_ms, playing, rate, seq}`) and keeps computing on its own with `performance.now()`. IPC jitter therefore does not affect the display.
- The sync logic lives in a UI-free module with unit tests against synthetic jitter.

### 3.5 Overlay: separate window with Win32 flags, as small as its content

- Separate Tauri window `overlay`: `transparent`, `decorations: false`, `alwaysOnTop`, `skipTaskbar`, `shadow: false`, `focus: false`.
- Additionally via Win32: `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOPMOST`.
- The window is **only as large as the overlay content**, not full-screen (saves compositing bandwidth, important for game FPS).

---

## 4. System architecture

### 4.1 Overview

```
┌──────────────────────────────── Rust backend (Tauri) ────────────────────────────────┐
│                                                                                       │
│  sources/smtc  ──┐                                                                    │
│  (Events+Poll)   │   Observation{src,pos,valid_at}      ┌──────────────┐              │
│                  ├────────────────────────────────────▶ │  sync/clock  │─── Anchor ──┼──▶ IPC event "clock"
│  sources/spotify │                                      │  (Fusion,    │              │
│  _api (optional) ┘                                      │  Filter,     │              │
│                                                         │  Offsets)    │              │
│  TrackInfo{title,artist,album,dur,art} ───┐             └──────────────┘              │
│                                           ▼                                           │
│                                   lyrics/matcher ─▶ lyrics/cache ─▶ lyrics/lrclib     │
│                                           │                                           │
│                                           ▼                                           │
│                                 Lyrics{lines[],words?} ───────────────────────────────┼──▶ IPC event "lyrics"
│                                                                                       │
│  overlay/win32 (flags, topmost reassert, fullscreen detection, monitor/DPI)           │
│  audio_devices (default output watcher → device offset)                               │
│  hotkeys, tray, settings(store+schema), logging                                       │
└───────────────────────────────────────────────────────────────────────────────────────┘
                 │                                               │
                 ▼                                               ▼
   ┌── Overlay window (WebView2) ───┐             ┌── Settings window (WebView2) ───┐
   │ local clock (rAF)              │             │ Connection, Overlay, Appearance  │
   │ KaraokeLine (gradient fill)    │             │ Sync, Lyrics, Hotkeys, Behavior  │
   │ layouts, themes, edit mode     │             │ Diagnostics ("Sync Lab")         │
   └────────────────────────────────┘             └──────────────────────────────────┘
```

### 4.2 Project structure

```
LyricHub/
  PLAN.md
  README.md                      # incl. setup guide (Spotify app optional)
  docs/
    spike-smtc.md                # measurement report from Phase 0
    sync-design.md               # detailed notes on the clock model (derive from section 5)
  src-tauri/
    tauri.conf.json              # two windows: "main" (settings), "overlay"
    Cargo.toml
    src/
      main.rs / lib.rs
      app_state.rs               # tokio::sync::watch for Anchor, Track, Lyrics
      sources/
        mod.rs                   # trait PlaybackSource { fn subscribe(&self) -> Receiver<SourceEvent> }
        smtc.rs                  # WinRT GlobalSystemMediaTransportControlsSessionManager
        spotify_api.rs           # optional, feature-gated: PKCE, polling, 429 handling
        types.rs                 # TrackInfo, PlaybackStatus, Observation, SourceEvent
      sync/
        clock.rs                 # PlaybackClock: anchor, residual filter, slew, seek detection (pure, testable)
        fusion.rs                # weighting of the sources, latency estimation
        offsets.rs               # global + device + song, sign convention
      lyrics/
        mod.rs                   # trait LyricsProvider
        lrclib.rs
        matcher.rs               # normalization, candidate scoring
        parser.rs                # LRC, Enhanced LRC (<mm:ss.xx>), [offset:] tag
        estimator.rs             # word boundary estimation for line-level LRC
        cache.rs                 # SQLite (rusqlite, bundled)
        types.rs                 # Lyrics, Line, Word
      overlay/
        window.rs                # geometry (anchor, monitor, scale), edit mode
        win32.rs                 # ex-styles, SetWindowPos topmost reassert, NoActivate
        fullscreen.rs            # SHQueryUserNotificationState + foreground rect check
      audio_devices.rs           # IMMDeviceEnumerator + IMMNotificationClient → default output ID
      hotkeys.rs                 # tauri-plugin-global-shortcut
      tray.rs
      settings/
        schema.rs                # versioned settings struct + migration
        store.rs                 # JSON in %APPDATA%/LyricHub/settings.json
      ipc/
        commands.rs              # #[tauri::command] get_settings, set_settings, nudge_offset, ...
        events.rs                # "track", "lyrics", "clock", "settings", "overlay-mode"
      logging.rs                 # tracing + file rotation in %APPDATA%/LyricHub/logs
  src/                           # frontend (Svelte 5 + TS + Vite)
    overlay/
      Overlay.svelte
      KaraokeLine.svelte         # gradient fill + word glow
      clock.ts                   # local anchor + slew, performance.now()
      layouts/ (Bar, Karaoke, Stack, Minimal)
      themes.ts
      editMode.ts
    settings/
      App.svelte + pages/ (Connection, Overlay, Appearance, Sync, Lyrics, Hotkeys, Behavior, Diagnostics)
    shared/
      types.ts                   # mirrored from Rust (ts-rs or manually)
      ipc.ts                     # typed invoke/listen wrapper
      i18n/ (de.json, en.json)
```

### 4.3 Data model (core)

```rust
pub struct TrackInfo {
    pub key: TrackKey,            // Spotify ID if available, otherwise norm(artist)|norm(title)|round(duration_s)
    pub title: String,
    pub artist: String,           // "A, B" for several
    pub album: Option<String>,
    pub duration_ms: Option<u32>, // from SMTC EndTime or API
    pub art: Option<ArtRef>,      // bytes from the SMTC thumbnail or URL from the API
    pub kind: TrackKind,          // Music | Ad | Podcast | Unknown
    pub source: SourceId,
}

pub struct Observation {
    pub source: SourceId,         // Smtc | SpotifyApi
    pub pos_ms: f64,
    pub valid_at: Instant,        // estimated point in time at which pos_ms was valid
    pub playing: bool,
    pub confidence: f32,          // 0..1, depends on the source
}

pub struct Anchor {               // sent to the frontend via IPC
    pub pos_ms: f64,              // RAW position at send time (without offsets)
    pub playing: bool,
    pub rate: f64,                // 1.0 normal; 0.9..1.1 during slew
    pub seq: u64,
    pub offset_ms_total: i32,     // global + device + song + lrc, separate so that the frontend can show diagnostics
}

pub struct Lyrics {
    pub key: TrackKey,
    pub source: String,           // "lrclib" | "local" | "user"
    pub synced: bool,
    pub lines: Vec<Line>,
    pub lrc_offset_ms: i32,       // from the [offset:] tag
}
pub struct Line { pub start_ms: u32, pub end_ms: Option<u32>, pub text: String, pub words: Option<Vec<Word>> }
pub struct Word { pub start_ms: u32, pub end_ms: u32, pub text: String, pub estimated: bool }
```

### 4.4 IPC contracts

Events (backend → frontend):

| Event | Payload | When |
|---|---|---|
| `track` | `TrackInfo` | Track change, metadata update (cover loaded later) |
| `lyrics` | `Lyrics` or `{ key, state: "loading" \| "missing" \| "instrumental" \| "unsynced", plain?: string }` | After matching |
| `clock` | `Anchor` | Status change, correction > 20 ms, seek, plus a heartbeat every 2 s |
| `settings` | complete settings object | After every change |
| `overlay-mode` | `{ edit: bool }` | Edit mode on/off |
| `diag` | Sync Lab sample (only while the diagnostics page is open) | 4 Hz |

Commands (frontend → backend): `get_settings`, `set_settings(patch)`, `nudge_offset(delta_ms)`, `set_song_offset(key, ms)`, `set_device_offset(device_id, ms)`, `reload_lyrics`, `import_lrc(path)`, `enter_edit_mode`, `save_overlay_geometry(geom)`, `cancel_edit_mode`, `spotify_login`, `spotify_logout`, `open_logs_folder`, `run_calibration_step(...)`.

---

## 5. Sync in detail (the core)

### 5.1 Error sources and how they are handled

| Error source | Magnitude | Handling |
|---|---|---|
| The SMTC timeline is updated only sporadically | unknown, to be measured (Spike 0) | Extrapolation from `LastUpdatedTime`; every update is an observation for the filter |
| Web API `progress_ms` is already stale at response time | 100–1000 ms, jitters | `valid_at = t_send + RTT/2`; do **not** interpret the API's `timestamp` field as "now" (its semantics are inconsistent in practice) |
| Polling gaps | up to 1 s | Local extrapolation with a monotonic clock |
| Audio output latency (Windows audio stack, Bluetooth 100–300 ms, wireless headsets, TV via HDMI) | 20–300 ms, device-specific | **Offset per audio device**, applied automatically on a device change |
| Inaccurate LRC timestamps / different edition of the song | ±0–2000 ms | **Offset per song**, nudge hotkeys, candidate scoring by duration |
| Spotify crossfade / Automix | Track boundaries blur | Note in the settings to disable crossfade for the best sync; track change via SMTC event, not via position |
| Seek by the user | any | Residual > 1500 ms → hard reset of the anchor, clear the filter window |
| Pause/resume | – | SMTC status event immediately; freeze the position on pause |
| Backward jumps after a timeline refresh (SMTC) | 50–300 ms | Residual filter + slew; never jump back hard if the residual is < 300 ms |

### 5.2 Sign convention for offsets

`display_pos_ms = clock_pos_ms − (offset_global + offset_device[current_output] + offset_song[key])`

*Corrected after phase 2: the original formula added the offsets (`clock_pos_ms + …`) and would have shown the lyrics earlier for a positive offset, against the sentence below. The sentence is what is implemented. The LRC tag `[offset:]` (positive = earlier, LRC convention) is already applied to the line times (`lyrics/parser.rs`) and does not belong in this formula. The original stays unchanged in `docs/original-plan/PLAN.en.md`.*

**Positive = lyrics appear later** (so: "lyrics come too early → increase the offset"). In the UI, always phrase it as a sentence: "Lyrics too early? → to the right" („Lyrics zu früh? → nach rechts“), not just as a number.

### 5.3 `PlaybackClock` (pseudocode, close to Rust)

```rust
const SEEK_THRESHOLD_MS: f64 = 1500.0;
const WINDOW: usize = 8;               // residuals per source
const SLEW_MAX_RATE_DELTA: f64 = 0.10; // ±10 % speed while converging
const DEADBAND_MS: f64 = 20.0;         // ignore smaller corrections

struct PlaybackClock {
    anchor: Anchor,                    // pos_ms, at: Instant, playing, rate
    residuals: HashMap<SourceId, VecDeque<f64>>,
    track_key: TrackKey,
    slew_target: Option<f64>,
}

impl PlaybackClock {
    fn predicted(&self, at: Instant) -> f64 {
        if !self.anchor.playing { return self.anchor.pos_ms; }
        self.anchor.pos_ms + (at - self.anchor.at).as_millis_f64() * self.anchor.rate
    }

    fn on_track_change(&mut self, key: TrackKey) {
        self.track_key = key; self.residuals.clear(); self.slew_target = None;
        // set the anchor to 0 until the first observation arrives (track start is a good assumption)
        self.anchor = Anchor { pos_ms: 0.0, at: Instant::now(), playing: true, rate: 1.0 };
    }

    fn on_status(&mut self, playing: bool, at: Instant) {
        let pos = self.predicted(at);
        self.anchor = Anchor { pos_ms: pos, at, playing, rate: 1.0 };
        self.emit();
    }

    fn on_observation(&mut self, obs: Observation) {
        let residual = obs.pos_ms - self.predicted(obs.valid_at);
        if residual.abs() > SEEK_THRESHOLD_MS {
            // seek or completely wrong anchor → set it hard
            self.anchor = Anchor { pos_ms: obs.pos_ms, at: obs.valid_at, playing: obs.playing, rate: 1.0 };
            self.residuals.clear(); self.slew_target = None;
            self.emit(); return;
        }
        let w = self.residuals.entry(obs.source).or_default();
        w.push_back(residual); if w.len() > WINDOW { w.pop_front(); }

        // robust fusion: weighted median over all sources (weight = confidence)
        let corr = weighted_median(self.residuals.iter(), confidences);
        if corr.abs() < DEADBAND_MS { return; }

        // slew instead of a jump: adjust the anchor rate briefly until corr is worked off
        let now = Instant::now();
        let cur = self.predicted(now);
        self.anchor = Anchor { pos_ms: cur, at: now, playing: self.anchor.playing, rate: 1.0 };
        self.slew_target = Some(cur + corr);
        // choose the rate so that corr is worked off within max(300 ms, |corr|/0.1)
        let dur_ms = (corr.abs() / SLEW_MAX_RATE_DELTA).max(300.0);
        self.anchor.rate = 1.0 + corr / dur_ms;        // e.g. corr=+60 → rate 1.1 for 600 ms
        self.schedule_rate_reset_after(dur_ms);        // afterwards rate=1.0, set a new anchor
        self.emit();
    }
}
```

Notes for Opus:
- `weighted_median` with a single window is sufficient for v1; a Kalman filter is not needed and is harder to debug.
- The frontend implements the same slew mechanism locally: when an anchor arrives, do not jump; instead (if the difference is < 300 ms) converge over 300 ms. This keeps the display calm even with slight backend jitter.
- **Never** render negative corrections (lyrics too far ahead) as a backward jump if they are < 300 ms; lower the rate to 0.9 instead.

### 5.4 Source fusion and latency estimation

- **SMTC**: `valid_at` from `LastUpdatedTime` (UTC `DateTime`) → compute the age → `Instant::now() - age`. Confidence 0.8. In addition, poll the timeline every 250 ms (cheap, local) instead of only waiting for events; count only **changed** `LastUpdatedTime` values as observations, otherwise autocorrelation arises.
- **Web API** (optional): `valid_at = t_send + rtt/2`. Confidence 0.6 (higher after 3 consistent samples). Adaptive polling: 1000 ms while playing, 500 ms in the last 3 s of a track, 3000–5000 ms while paused, immediately after an SMTC event; on HTTP 429, respect `Retry-After`.
- **Track change detection**: The SMTC event `MediaPropertiesChanged` (fast) takes priority. The API confirms the track ID later.
- **Conflict rule**: If SMTC and the API differ persistently (> 5 samples) by more than 400 ms, the source with the lower variance wins; the other one is marked "deviating" („abweichend“) in the Sync Lab.

### 5.5 Special states

| State | Detection | Overlay behavior |
|---|---|---|
| Ads (free account) | API: `currently_playing_type == "ad"`; without the API: SMTC title "Advertisement"/"Spotify", artist empty, duration ≈ 30 s | Hide the overlay or show only a subtle "Ad" („Werbung“) |
| Podcast | API: `"episode"`; SMTC: duration > 20 min and no album | Title only, no lyrics search |
| Local file | API: `is_local`; SMTC as usual | Matching via title/artist/duration |
| Instrumental | LRCLIB `instrumental: true` | A subtle "Instrumental", after 5 s the title only |
| No synced lyrics | only `plainLyrics` | Title + note "No synced lyrics" („Keine synchronen Lyrics“); optionally the plain text, static |
| Spotify closed | The SMTC session disappears | Hide the overlay, tray icon gray |
| Paused | SMTC status | The overlay dims to 60 % after 2 s and fades out after a configurable time (default 10 s) |

### 5.6 Word-level highlight without word timestamps (estimation)

LRCLIB almost always provides only line timestamps. A true word-by-word light-up would be visibly wrong with estimated boundaries. Therefore:

1. **Display as a continuous fill** (a gradient moves across the line), not as discrete word switching. Estimation errors are then hardly noticeable.
2. Line end: `end = min(next.start, start + max_dur)`; if the gap to the next line is > 2.5 s, estimate the line duration from the text length (`chars * 70 ms`, clamped to 1–8 s) and treat the rest as an instrumental break (the fill stays full, the line stays, a subtle pulse).
3. Word boundaries within the line are proportional to the character count without punctuation, with a 60 ms lead-in and a 120 ms tail. Syllable counting is not needed for German/English; the character count correlates well enough.
   **Japanese** (the user's third most frequent language) has no spaces. Here the line is split per character instead of per word: kana count 1, kanji 1.5 (usually several morae), punctuation 0. Chinese like kanji, Korean per syllable block. The script is detected per line via Unicode ranges.
4. If **Enhanced LRC** is available (`<mm:ss.xx>` before words) or a user LRC with word marks, real word times are used (`estimated: false`), and the glow jumps word by word.

### 5.7 Offsets, nudge and calibration wizard

- **Nudge hotkeys**: Default `Ctrl+Alt+,` (−100 ms) / `Ctrl+Alt+.` (+100 ms), with Shift ±25 ms. A small toast in the overlay shows the new value for 1.5 s. The change goes to the **song offset** of the current song and is stored per song (user decision, 14.1). On a song change, the song offset of the new song applies immediately, or 0 if it was never corrected. So a correction never carries over into the next song. When the same song plays again, its stored correction applies. The global offset changes only in the settings (permanent output latency), never via nudge. The settings page "Sync" („Synchronisation“) shows the song offset of the current song and can reset it.
- **Device offset**: `audio_devices.rs` watches the default render device (`IMMNotificationClient::OnDefaultDeviceChanged`). The settings store `device_offsets: { "<device-id>": ms }`. When a new device appears for the first time: toast "New audio device detected. Check sync?" („Neues Audiogerät erkannt. Sync prüfen?“) with a link to the wizard. Bluetooth devices get a suggested starting value of +150 ms. The user mostly listens over a cable. The device offset is therefore not a critical path and is implemented simply in Phase 3 (table + toast, no separate wizard per device).
- **Calibration wizard** (Settings → Sync): plays nothing itself but uses the current song. Flow: the overlay shows the next line large with a countdown bar; the user presses Space exactly at the audible onset; 3–5 repetitions; the median of the differences is suggested as the device offset. Discard outliers (> 600 ms).
- **Global offset**: A reserve for cases that can be attributed neither to the device nor to the song. Default 0.

### 5.8 Spike 0: SMTC measurement (do this before anything else)

Goal: know whether SMTC alone meets the sync requirement.

1. A small Rust binary (or Tauri command) opens the SMTC session to Spotify and logs, on every `TimelinePropertiesChanged` event and additionally via 100 ms polling: `Position`, `LastUpdatedTime`, `now`, status, title. Output as CSV.
2. 10 minutes of music with play/pause/seek/track change, measured with the **Microsoft Store version** of Spotify (the user's setup). The desktop version is only measured if the user switches for Plan B.
3. Analysis in `docs/en/spike-smtc.md`: update cadence (events per minute), distribution of `now - LastUpdatedTime` at events, jitter of the extrapolated position between two updates, behavior on seek (latency until the event), behavior on pause.
4. Decision: jitter p95 < 150 ms → SMTC alone is the v1 source. Otherwise → Plan B from 3.2: present the result to the user, the user switches to desktop Spotify, and the Spicetify companion (13.1) is brought forward. The Web API (Phase 3b) only comes if that is not enough either. Spicetify does not reliably support the Store version, so the switch is a prerequisite.

Pitfalls: WinRT calls need an initialized apartment (MTA thread with `RoInitialize`), not the Tauri main thread. Event handlers run on arbitrary threads; send the data to the backend via `tokio::sync::mpsc`. Store Spotify has the AUMID `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`, desktop Spotify `Spotify.exe`; filter on `contains("Spotify")` and test both variants.

### 5.9 Diagnostics view "Sync Lab" (Settings → Diagnostics)

- Timeline of the last 60 s: observations per source as dots, the model position as a line, residuals as bars.
- Live values: current position, rate, remaining slew, offsets (all four components), last latency estimate, source of the last anchor.
- Buttons: "Open logs" („Logs öffnen“), "Diagnostics export (JSON, 60 s)" („Diagnose-Export (JSON, 60 s)“).
- This view is the tool used to prove the acceptance criteria in section 2. Build it early (Phase 3), not at the end.

---

## 6. Overlay window (Windows technology)

### 6.1 Window configuration

`tauri.conf.json` (window `overlay`): `transparent: true`, `decorations: false`, `alwaysOnTop: true`, `skipTaskbar: true`, `shadow: false`, `focus: false`, `resizable: false`, `visible: false` (show it only after the geometry setup).

After creation, in `overlay/win32.rs` via the `windows` crate:

```rust
let hwnd = window.hwnd()?;
let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
SetWindowLongPtrW(hwnd, GWL_EXSTYLE,
    ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize | WS_EX_TOPMOST.0 as isize);
window.set_ignore_cursor_events(true)?;   // sets WS_EX_TRANSPARENT|WS_EX_LAYERED
```

**Known pitfall** (from community reports on Tauri 2): `set_ignore_cursor_events` overwrites the ex-styles and loses `WS_EX_TOOLWINDOW` in the process; the overlay then shows up in Alt+Tab. Therefore set the ex-styles again **after every toggle** (helper function `apply_overlay_exstyles(hwnd, click_through: bool)`).

**Update after Spike B (source code analysis of tao 0.37, details in `docs/en/spike-overlay.md`)**: After every flag change on a visible window, tao additionally calls `ShowWindow(SW_SHOW)`, which can steal focus. The rule is therefore: **After creation, no more Tauri setters that change flags on the overlay** (`show`, `hide`, `set_ignore_cursor_events`, `set_always_on_top` …). Showing, click-through and topmost run directly through Win32 in `overlay/win32.rs`. `set_size` and `set_position` remain allowed. `focusable: false` in the window config sets `WS_EX_NOACTIVATE` inside tao and is mandatory.

### 6.2 Never steal focus

- `WS_EX_NOACTIVATE` on the overlay. All `SetWindowPos` calls with `SWP_NOACTIVATE`.
- When showing/hiding, use `ShowWindow(hwnd, SW_SHOWNOACTIVATE)`, or check whether Tauri's `show()` activates the window; if it does, use a custom Win32 variant.
- The overlay is **never** hidden and shown again when it is merely "empty". Set the content to opacity 0 instead (community report: a re-show can flash an old frame and is more expensive). Only really hide it when Spotify exits.
- Edit mode: click-through off, but still `WS_EX_NOACTIVATE`; drag/resize is handled in the frontend via pointer events, not by moving the window through Windows. On exit, turn click-through back on and set the ex-styles again.

### 6.3 Holding topmost against games

- Some games make themselves topmost. Every 2 s, while a fullscreen app is in the foreground: `SetWindowPos(hwnd, HWND_TOPMOST, 0,0,0,0, SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE)`.
- Fullscreen detection: `SHQueryUserNotificationState` → `QUNS_RUNNING_D3D_FULL_SCREEN` or `QUNS_BUSY` (fullscreen app), supplemented by comparing the `GetForegroundWindow` rect with the monitor rect.
- Option "Show the overlay only while a fullscreen app is running" („Overlay nur zeigen, wenn eine Vollbild-App läuft“) (default off).
- Exclusive fullscreen: overlay invisible. Show this clearly in the diagnostics panel and in the tray tooltip: "Game is running in exclusive fullscreen – overlay cannot be displayed. Tip: choose Borderless Window." („Spiel läuft im exklusiven Vollbild – Overlay nicht darstellbar. Tipp: Borderless Window wählen.“)
- The user plays in a mix of borderless and exclusive fullscreen. This detection is therefore an everyday case and mandatory for v1. On detection, the tray icon switches to a warning symbol, and a note appears in the tooltip and on the diagnostics page. None of this may interrupt the game or take focus.
- Many DX11/DX12 games that offer "Fullscreen" actually run in the flip model on Windows 11 thanks to fullscreen optimizations and allow topmost windows above them. Spike B checks this with at least one of the user's games in exclusive fullscreen and documents which games show the overlay.

### 6.4 Geometry, multi-monitor, DPI

- The settings store: `monitor_id` (device name), `anchor` (9 points: tl, tc, tr, ml, mc, mr, bl, bc, br), `offset_x_pct`, `offset_y_pct` (relative to the monitor size), `width_pct`, `scale` (0.5–3.0). This way positions survive resolution changes.
- Recompute the geometry at startup and on `WM_DISPLAYCHANGE`; if the monitor is missing, fall back to the primary one.
- Per-monitor DPI: Tauri reports `scale_factor`; compute the window size in logical pixels, the frontend scales via the CSS variable `--scale`.
- Window size = content size: the frontend reports the bounding box via `ResizeObserver`, the backend adjusts the window (debounced, 50 ms). Minimum size 200×60 logical pixels.

### 6.5 Performance

- Animate only properties that are cheap for the GPU: `transform`, `opacity`, `background-position`/`background-size` on the gradient text, `clip-path`. No layout per frame.
- The `requestAnimationFrame` loop runs only with `playing == true` and a visible overlay; otherwise stop it. Paused > 10 s: loop off.
- Disable WebView2 background throttling (otherwise the animation stutters when the overlay counts as being in the background).
- Do not expect `backdrop-filter` blur effects: the WebView can **not** blur the game behind the transparent window. Use a semi-transparent dark plate instead of blur. Acrylic blur-behind via the `window-vibrancy` crate is an **experimental** option (Windows 11); it does not work over fullscreen apps and costs performance; off by default.

---

## 7. Lyrics pipeline

### 7.1 Matching

1. **Normalization** of title and artist: lowercase, Unicode NFKC, remove bracketed additions (`(feat. …)`, `[Remastered 2011]`, `- Live`, `- Radio Edit` only as a fallback stage, not right away), reduce the artist to the main artist for "A, B" / "A & B" / "A feat. B".
2. **Stage 1**: `GET /api/get?track_name=&artist_name=&album_name=&duration=` with the full values. Duration in seconds (SMTC EndTime or API `duration_ms/1000`).
3. **Stage 2** (on 404): `/api/get` without `album_name`, then with the normalized title.
4. **Stage 3**: `/api/search?track_name=&artist_name=` → score the candidates: duration difference (≤ 3 s strongly preferred, > 8 s discard), title similarity (Jaro-Winkler ≥ 0.85), `syncedLyrics != null` preferred, take `instrumental` into account.
5. Cache the result under `key`, negative results too (for 7 days), so that LRCLIB is not queried on every replay.
6. Time budget: 1.5 s in total; network error → retry with backoff in the background, until then the overlay shows only the title.

### 7.2 Parser

- Standard LRC: `[mm:ss.xx]Text`, several timestamps per line allowed (`[00:10.00][01:20.00]Refrain`), metadata tags (`[offset:+300]`, `[ti:]`, `[ar:]`), keep empty lines as instrumental markers.
- Enhanced LRC: `<mm:ss.xx>` word marks within the line.
- Sort lines by time, remove duplicates, derive `end_ms` from the successor.
- Tolerate Windows line endings, a BOM, and timestamps with 3 decimal places.
- Unit tests with at least 10 real LRC examples (including broken ones).

### 7.3 Cache (SQLite)

Tables: `tracks(key, title, artist, album, duration_ms, last_seen)`, `lyrics(key, source, format, raw, synced, instrumental, fetched_at)`, `negative_cache(key, until)`, `song_overrides(key, offset_ms, user_lrc)`, `device_offsets(device_id, name, offset_ms)`. Path: `%APPDATA%/LyricHub/lyrichub.db`.

### 7.4 Legal (note for the README, not a blocker for personal use)

Lyrics are protected by copyright. LRCLIB is community-maintained; the app displays third-party content. Not critical for personal use. On release: a disclaimer, no redistribution of lyrics, credit LRCLIB as the source. No use of unofficial Musixmatch/Spotify endpoints.

---

## 8. Spotify Web API (optional layer, Phase 3b)

Build it only if Spike 0 or the user requires it. Feature flag `spotify-api` in Cargo.

- **Prerequisites (as of Feb/Mar 2026)**: The user creates their own app in the Spotify Developer Dashboard (Development Mode). The owner needs **Spotify Premium** (the user has it); max. 5 authorized users; the client ID is entered in the settings (public, PKCE needs no secret). Before implementing, read the official **Migration Guide "February 2026 Web API Dev Mode Changes"** and check whether `GET /me/player` is still available in Dev Mode.
- **Auth**: Authorization Code with PKCE. Redirect URI `http://127.0.0.1:<port>/callback` (for loopback, Spotify accepts only the IP literal form, not `localhost`). A local mini HTTP listener in Rust (`tiny_http`), port from a fixed range; open the browser via `tauri-plugin-opener`.
- **Scopes**: `user-read-playback-state` (covers currently-playing).
- **Token storage**: Windows Credential Manager via the `keyring` crate. The refresh token rotates with PKCE; always store the new one.
- **Polling**: see 5.4. HTTP 429 → respect `Retry-After`, back off, the Sync Lab shows "rate limited".
- **Added value**: a stable track ID as the cache key, podcast detection, cover URLs in several sizes, a second independent position source. Ad detection is irrelevant for the user because Premium plays no ads.
- **Release**: Because of the 5-user limit, the Web API cannot carry a public version. If the app were passed on, every user would have to enter their own client ID. It therefore stays a personal add-on feature.

---

## 9. Design

### 9.1 Design principles

1. **Readable over any game**: White text with a dark outline or on a semi-transparent plate. Never plain color without contrast protection.
2. **Calm**: The overlay is the passenger, not the lead. No constant motion except the fill of the active line. Everything else happens in ≤ 250 ms and then stays still.
3. **Never in the way**: Default top center, narrow; disappears on pause; never takes focus; click-through.
4. **Time is truth**: The fill strictly follows the position (linear, no easing). Easing exists only for line changes, never for the progress.
5. **Configurable, but with good defaults**: A user who configures nothing gets a finished result.

### 9.2 Layout modes

| Mode | Content | Target audience |
|---|---|---|
| **Karaoke** (default) | Current line large with the fill, the next line below it small and dimmed, mini header (cover 24 px, title – artist) | Singing along |
| **Bar** | One line: cover · title – artist · \| · current lyric line (ellipsis instead of marquee) | Minimal space, at the top edge |
| **Stack** | 3–5 lines: previous (dimmed), current (fill), upcoming (dimmed), smooth scrolling | Reading, overview |
| **Minimal** | Only the active line, without a plate, with outline + shadow | Clean, streaming |

### 9.3 Karaoke highlight (technique)

- The text line as two stacked layers: the base color (dimmed, e.g. `rgba(255,255,255,0.55)`) and above it a copy in the accent color inside a clip layer. The clip layer slides in from the left via `transform: translateX()`, and its content slides in the opposite direction, so the text stands still and only the visible part grows. A soft transition edge via `mask-image` at the leading edge. **No animated gradient with `background-clip: text`**: it repaints every frame and in Spike B cost 55 % of a core instead of 13 % (see `docs/en/spike-overlay.md`).
- Throttle the fill to 30 to 60 updates per second, independent of the monitor refresh rate. Otherwise WebView2 composites 164 frames per second at 164 Hz.
- With real word timings: additionally a `span` per word with a `text-shadow` glow that fades in over 120 ms and out over 300 ms.
- Font rendering: `text-rendering: optimizeLegibility`, a variable font with `font-variation-settings` for weight changes without a layout jump.
- Outline without a plate: `paint-order: stroke fill` + `-webkit-text-stroke` (2 px at scale 1) plus `text-shadow 0 2px 8px rgba(0,0,0,.7)`.

### 9.4 Motion specification

| Event | Duration | Easing | Description |
|---|---|---|---|
| Line change | 220 ms | cubic-bezier(.2,.8,.2,1) | The active line fades 0.55 → 1, the next one moves up 12 px |
| Fill | continuous | linear | follows `display_pos` |
| Word glow (real word times only) | 120 ms in / 300 ms out | ease-out | `text-shadow` intensity |
| Track change | 300 ms | ease-in-out | Crossfade of the whole overlay, the cover swaps with a small scale 0.96 → 1 |
| Pause | after 2 s, 400 ms | ease | Opacity to 0.6; after `hide_after_pause_s` (default 10) to 0 |
| Nudge toast | visible for 1.5 s | – | small pill below the overlay: "Song offset +100 ms" („Song-Offset +100 ms“) |
| Reduced motion (option) | – | – | all transitions ≤ 80 ms, no glow |

### 9.5 Themes, typography, colors

- **Font**: Inter Variable (bundled), fallback Segoe UI Variable; optionally a user font from the system list. Japanese is the user's third most frequent lyrics language, so CJK rendering is mandatory for v1. Fallback chain: `"Inter Variable", "Segoe UI Variable", "Yu Gothic UI", "Meiryo UI", "Noto Sans JP", sans-serif`. Check Yu Gothic UI and Meiryo UI for availability at startup; if they are missing, bundle a subset of Noto Sans JP (OFL license). Set `line-break: strict` for Japanese lines. Base size 28 px at scale 1 on 1080p; active line weight 650, next line 500; line height 1.25.
- **Themes**:
  - *Glass Dark* (default): plate `rgba(12,12,16,.55)`, radius 14 px, white text, accent from the album cover.
  - *Glass Light*: plate `rgba(255,255,255,.7)`, text `#111`.
  - *Neon*: no plate, outline, strong accent glow; for dark games.
  - *Mono*: high contrast, black/white, no accent; accessibility.
  - *Custom*: plate, text color, accent, outline and radius freely adjustable.
- **Accent color from the cover**: draw the cover as an image into a canvas, scale it down to 32×32, simple quantization (median cut or k-means with k=4), pick the candidate with the highest saturation, raise the luminance to ≥ 0.6, ensure a contrast against the plate of ≥ 4.5:1; otherwise a neutral fallback accent. Do not copy Spotify's brand green exactly (branding guidelines).
- **Opacity/size/width** as sliders, live preview in the overlay.

### 9.6 States in the overlay

- *Loading*: title visible, the lyrics area shows three pulsing dots (max. 1.5 s).
- *No synced lyrics*: title + a small line "No synced lyrics found" („Keine synchronen Lyrics gefunden“) (disappears after 4 s, the title stays).
- *Instrumental*: "Instrumental" for 4 s, then the title only.
- *Ad*: overlay off (option: show "Ad").
- *Error (network)*: a subtle icon in the header, details in the settings window; never large error texts in the game.
- *Edit mode*: dashed frame, 9 snap points on the monitor as small markers, a handle for dragging, corner handles for scaling, sample text "This is a sample line for alignment" („Dies ist eine Beispielzeile zum Ausrichten“), toast "Enter = save, Esc = cancel, drag = move" („Enter = speichern, Esc = abbrechen, Ziehen = verschieben“).

### 9.7 Settings window

Page navigation on the left, content on the right, 960×640, dark theme matching the overlay.

1. **Connection**: Spotify status (SMTC found yes/no, client version desktop/Store), optional Web API login (client ID field, login button, note on the Premium requirement and the 5-user limit).
2. **Overlay**: monitor, anchor, fine position, width, scale, layout, "Start edit mode" („Edit-Modus starten“), "Only with a fullscreen app" („Nur bei Vollbild-App zeigen“), "Hide on pause after … s" („Bei Pause ausblenden nach … s“).
3. **Appearance**: theme, font, accent (auto from the cover / fixed), plate on/off, outline, opacity, reduced motion, live preview.
4. **Sync**: global offset, device offsets (table: device, offset, "Calibrate" („Kalibrieren“)), song offset of the current track, nudge target (song/global), calibration wizard, note on crossfade.
5. **Lyrics**: provider order, cache size/clear, import an LRC for the current song, reset the negative cache, "Reload lyrics" („Lyrics neu laden“).
6. **Hotkeys**: overlay on/off (`Ctrl+Alt+L`), edit mode (`Ctrl+Alt+E`), nudge −/+ (`Ctrl+Alt+,` / `Ctrl+Alt+.`), switch layout (`Ctrl+Alt+K`), all can be disabled (conflicts with games).
7. **Behavior**: autostart, start minimized, language, check for updates (optional), logs.
8. **Diagnostics**: Sync Lab (5.9), SMTC raw data, API status, fullscreen status, open logs, export diagnostics.

### 9.8 Tray

Left click: overlay on/off. Right-click menu: show overlay (checkbox), edit mode, nudge −100/+100, layout submenu, settings, quit. Tooltip: "LyricHub – <title> – <artist>" or status messages.

---

## 10. Implementation phases

Every phase ends with a working state. Keep the order: measure first (Phase 0), then a thin vertical slice (Phase 1), then sync (2–3), then polish (4), then robustness (5).

### Phase 0 – Spike and scaffold

Tasks:
- `git init`, create a Tauri 2 project with two windows (`main`, `overlay`) and a tray; plugins: global-shortcut, autostart, single-instance, opener, store (or a custom JSON store).
- Run **Spike A (SMTC measurement)** as described in 5.8, write `docs/en/spike-smtc.md`.
- **Spike B (overlay)**: test a transparent, click-through, topmost, no-activate window with static text, namely over a borderless game, a game in exclusive fullscreen (see 6.3) and a fullscreen video; check the Alt+Tab behavior; implement the ex-style reapply.
- Set up logging (tracing → file).

Definition of Done: Both spike reports exist (`docs/en/spike-smtc.md`, `docs/en/spike-overlay.md`). The decision "SMTC is enough / Plan B needed" is documented. Opus makes the decision "Tauri stays / WPF" itself and justifies it in the report (user decision, 14.1). If Plan B is needed, the result is presented to the user before Phase 1 begins.

### Phase 1 – Thin vertical slice "Now Playing"

Tasks:
- `sources/smtc.rs`: session discovery (Spotify filter), events for metadata, status, timeline; cover thumbnail as bytes → Base64/temp file for the frontend.
- `TrackInfo` event; the overlay shows cover, title, artist (Bar layout).
- Settings schema v1 + store; settings window with the pages Overlay and Behavior.
- Overlay geometry (monitor, anchor, offsets, scale) + edit mode.
- Tray with basic functions; hotkey for overlay on/off.
- Create the settings window only when it is opened, and destroy it when it is closed (saves about 30 MB, Spike B).
- The Phase 0 scaffold is the basis: `overlay/win32.rs`, the guard, the self-test, the tray and logging already exist. The rule from 6.1 applies: no Tauri setters that change flags on the overlay.

DoD: A Spotify track change appears in the overlay over a game within ≤ 300 ms; position/size can be set in edit mode and survive a restart.

**Phase 1 result (2026-10-25)**: completed, tag `v0.1.0-phase1`. LyricHub takes 7–10 ms from the SMTC event to the drawn title. Spotify, however, delivers the title up to 299 ms after the new timeline, so the old title already fades out at the first sign of a change. Edit mode, geometry after a restart, the self-test and CPU are verified, and the user passed the visual check in a game. Most important finding: the layered overlay needs `SetLayeredWindowAttributes`, otherwise clicks fall through in edit mode. Details and deviations in `docs/en/phase1.md`.

### Phase 2 – Lyrics and clock

Tasks:
- `lyrics/lrclib.rs`, `matcher.rs`, `parser.rs`, `cache.rs` with tests.
- `sync/clock.rs` as described in 5.3, with tests against synthetic observations (jitter ±200 ms, seek, pause).
- Frontend `clock.ts` with a local anchor + slew; `KaraokeLine` with a gradient fill (line-level); Karaoke layout.
- Global offset + nudge hotkeys + toast.

DoD: Ten songs with synced lyrics run along line by line; nudge visibly corrects; parser tests green.

**Phase 2 result (2026-10-25)**: completed, tag `v0.2.0-phase2`. Listening test with ten songs (English, German, Japanese, French): the lines change in sync, without an offset. As expected, the line-level fill mostly does not hit the sung word; that is Phase 4 (5.6). CPU with the fill running: 0.59 % of all cores. Important findings: the formula in 5.2 has the sign reversed (what is implemented is the sentence: positive = later); LRCLIB answers unknown titles and empty searches with 503 and needs ~1.1 s per response under load, so the search stages run in parallel, staggered. Details in `docs/en/phase2.md`.

### Phase 3 – Sync hardening

Tasks:
- Source fusion (5.4), residual filter, slew rate reset, seek and pause behavior.
- `audio_devices.rs` + device offsets + toast on a new device.
- Store the song offset per song; nudge now acts only on the song offset (5.7). In Phase 2, nudge shifted the global offset. When switching over, check the user's stored global offset: it probably comes from Phase 2 nudges. Ask the user whether it should go back to 0.
- Calibration wizard.
- Sync Lab (5.9) including the diagnostics export.
- Special states (5.5): ads, podcast, instrumental, no lyrics, Spotify closed.

**Phase 3b (optional, after the Spike 0 decision)**: Web API layer as described in section 8.

DoD: The sync acceptance criteria (section 2) are proven in the manual log and documented in `docs/en/sync-report.md`.

**Phase 3 result (2026-10-25)**: tag `v0.3.0-phase3`, with one open gap. Sync log by tapping Space over 115 line starts in 13 English and German songs, wired, device offset +100 ms: median 96 ms (target ≤ 100 ✓), p95 289 ms (target ≤ 200 ✗). Seek until the overlay is redrawn: 30 ms median. Per-song nudge is proven across a real song change. The timing chain takes 8–25 ms; the p95 comes from the differing timing of the LRC files (shift per song −140 to +151 ms) and from the scatter of the taps. Japanese was measured after the tag, with romaji: two songs, 40 taps, median 144 ms, because one LRC file is consistently 144 ms early; across all 15 songs, median 108 ms, p95 309 ms. A calibration over only one song also measures that song's LRC timing. Phase 3b (Web API) is dropped. Details in `docs/en/phase3.md` and `docs/en/sync-report.md`.

### Phase 4 – Karaoke polish and design

Tasks:
- Word boundary estimation (5.6) + Enhanced LRC word times; word glow.
- All four layouts; themes; cover accent color with a contrast safeguard; reduced motion.
- Motion as described in 9.4; states as described in 9.6; settings page Appearance with live preview.
- i18n (de/en).

DoD: Design review against 9.1–9.6; no layout per frame (DevTools performance check); FPS test in the game with no measurable loss.

**Phase 4 result (2026-10-26)**: completed, tag `v0.4.0-phase4`. The fill runs word by word on estimated word times (letters, kana 1, kanji 1.5). LRCLIB has practically no real word times (0 of 160 hits), so there is no calibration against them. Five themes with an accent from the cover, four layouts, an "Appearance" page („Darstellung“) with live preview, German and English. No layout per frame (0.2 per second), CPU 0.48 % of all cores. The user declared the FPS test unnecessary. Before this phase, romaji was merged and Japanese was measured again. Details in `docs/en/phase4.md`.

### Phase 5 – Robustness and release

Tasks:
- Fullscreen detection, topmost reassert, "only with a fullscreen app".
- Defend topmost driven by events: `SetWinEventHook` with `EVENT_OBJECT_REORDER` and `EVENT_SYSTEM_FOREGROUND` (`WINEVENT_OUTOFCONTEXT`, no injection). Periodic polling is not enough against games that keep putting themselves on top: at 100 ms, the overlay was on top only 82 % of the time (Spike B).
- `WM_DISPLAYCHANGE`, monitor fallback, DPI tests at 100/125/150 %.
- Autostart minimized, single instance, sleep/wake (rebuild the SMTC session).
- Error UX, logs, diagnostics export.
- Installer (NSIS via the Tauri bundler), README with screenshots and setup (including the optional Spotify app guide), versioning.

DoD: All acceptance criteria from section 2 met; installer tested on a fresh Windows profile.

**Phase 5 result (2026-10-26)**: tag `v0.5.0-phase5`. A WinEvent guard (`EVENT_OBJECT_REORDER`, `EVENT_SYSTEM_FOREGROUND`, without injection) keeps the overlay on top: `game-aggressive` is green for the first time (120 of 120 samples on top). In addition: fullscreen detection with "Only with a fullscreen app", a tray note when the overlay is covered, repositioning on `WM_DISPLAYCHANGE`, SMTC reconnection after wake-up, a hotkeys page, a custom icon and an NSIS installer that needs no administrator rights. Release build: 179 MB private (with `--disable-gpu-rasterization`), CPU 0.48 % while playing and 0.03 % while paused. The user accepted the p95 of the sync measurement in view of the human reaction time when tapping. Online lyrics arrive in under 1.5 s in 76 % of cases (LRCLIB). Sleep mode was not checked live; a friend of the user is testing the installer. Details in `docs/en/phase5.md`.

### Phase 6 – Extensions (as needed, see section 13)

---

## 11. Test strategy

### 11.1 Unit tests (Rust)
- `parser.rs`: 10+ real LRC files, Enhanced LRC, broken lines, `[offset:]`.
- `matcher.rs`: normalization, scoring, feat./remaster variants.
- `clock.rs`: synthetic observation series (constant offset, jitter ±200 ms, drift, seek, pause/resume, source conflict) → the model error stays below 50 ms after 3 samples; no backward jumps < 300 ms.
- `estimator.rs`: word boundaries monotonic, within the line, lead/tail correct.

### 11.2 Integration tests
- A mock `PlaybackSource` that replays events from a fixture file (track changes, seeks) → the complete pipeline up to the `clock` events, without Spotify.
- LRCLIB client against recorded HTTP responses (no live calls in CI).

### 11.3 Manual sync log (mandatory before completing Phase 3 and Phase 5)
- 10 songs of different genres (fast rap lines, ballads, songs with a long intro, one live recording; a language mix like the user's: mostly English, at least two German and two Japanese tracks).
- For each line in the first 60 s, note: overlay too early / on time / too late, estimated in 100 ms steps. Compute the median and p95. With the user's wired output; Bluetooth only if such a device is available.
- Seek test: 5 jumps per song, time until the overlay is correct (stopwatch or Sync Lab timestamps).
- Focus test: type in a game (or in Notepad in the foreground) while track changes, edit mode exits and hotkeys happen; no character may be lost.

---

## 12. Risks and countermeasures

| Risk | Likelihood | Impact | Countermeasure |
|---|---|---|---|
| Spotify's SMTC position too coarse for ±100 ms | medium | high | Spike 0 first; Plan B desktop Spotify + Spicetify companion (13.1), Plan C Web API (3b) |
| Spicetify does not run with Store Spotify | certain | medium | Switch to desktop Spotify only if Plan B becomes necessary (user decision) |
| Spotify restricts the Web API further / Premium expires | high | medium | The app works without the API; the API is behind a feature flag |
| Overlay invisible in exclusive fullscreen | high (the user plays in a mix) | medium | Detection + note in the tray (6.3), test in Spike B; no injection |
| Anti-cheat classifies the overlay as suspicious | low | high | Only a normal topmost window, no hooks, no reading of other processes |
| Wrong lyrics (different song/version) | medium | medium | Duration scoring, a "Wrong lyrics? → choose another version" („Lyrics falsch? → andere Version wählen“) dialog with a candidate list (Phase 4) |
| LRCLIB outage | low | medium | Cache, backoff, the overlay degrades to the title |
| Tauri transparency/click-through bugs on certain GPUs/drivers | low–medium | high | Spike B early; decide on the WPF fallback before Phase 2 begins |
| Hotkey conflicts with games | medium | low | All hotkeys can be disabled, the tray as an alternative |
| Copyright on release | – | medium | Disclaimer, no redistribution, display only |

---

## 13. Brainstorm: further ideas (v2+)

Sorted by benefit and effort. None of this is needed for v1.

### 13.1 Exact position via a Spicetify companion (high benefit, medium effort)
A small Spicetify extension in the Spotify client sends `Spicetify.Player.getProgress()` at 10 Hz via WebSocket to `ws://127.0.0.1:<port>`. Accuracy ±20 ms, including seek events in real time. Downside: the user must install Spicetify; it occasionally breaks with Spotify updates. Offer it as a "power user mode"; LyricHub detects the companion automatically as a third source with confidence 0.95. **Prerequisite**: the desktop version of Spotify, because Spicetify does not reliably support the Store version. For this user it is Plan B (3.2) and is brought forward from Phase 6 into Phase 3 if needed.

### 13.2 Audio loopback as a sync signal (medium benefit, medium effort)
Windows 10 2004+ allows **per-process loopback** (`ActivateAudioInterfaceAsync` with `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` on the Spotify PID). Even without reference audio, this allows a few things:
- Track start anchor: the transition from silence to signal after a track change marks position ≈ 0 (with a lead silence correction from the first samples). Useless with crossfade, otherwise very precise.
- Instant pause detection.
- **Beat-reactive glow**: the RMS envelope drives the glow intensity of the active line (cheap, looks good, needs no analysis API).
Not possible: fingerprint alignment, since no reference audio of the track is available.

### 13.3 Real word times via forced alignment (high benefit, high effort)
With loopback audio (13.2) and the known lyrics text, forced alignment (e.g. WhisperX, wav2vec2-CTC) can compute a word-level timeline, once per song, cached. Needs a local model and ideally a GPU; the result is not available in real time but from the second listen on. As an optional module with its own binary (Python sidecar or ONNX in Rust).

### 13.4 Player-agnostic through SMTC (high benefit, low effort)
Since SMTC is generic, the pipeline also works with minimal changes for YouTube Music, Apple Music, Tidal and browser players. Only the session filter needs to become configurable ("Preferred player" („Bevorzugter Player“)). Spotify stays the default.

### 13.5 Streaming output (medium benefit, low effort)
A local HTTP server serves the same overlay page as an OBS browser source (with a chroma key mode or a transparent background). Streamers get lyrics in their stream without having to capture the game overlay.

### 13.6 Translation and romanization (medium benefit, medium effort)
A second line below the active line: a translation (DeepL/LibreTranslate API, cached) or a romanization for Japanese/Korean (kuroshiro or a Hangul romanizer). Very valuable for K-pop/J-pop listeners. For this user (Japanese in third place), romanization is the first candidate for Phase 6.

### 13.7 Lyrics editor and contributing to LRCLIB
Mini editor: re-stamp lines in time with a key press, nudge single lines, save as LRC; optional upload to LRCLIB (its publish API with a proof-of-work challenge). Makes the community database better for everyone.

### 13.8 Smaller ideas
- "Wrong lyrics?" dialog with a candidate list from the LRCLIB search and a preview.
- A second overlay as a "Now Playing" card on a second monitor.
- Cover background as a softly blurred gradient behind the plate (inside the WebView, so feasible).
- Game detection via the process list: overlay profiles per game (position/theme), e.g. top left in shooter A, bottom center in strategy game B.
- Spotify Connect receiver mode via librespot (the app itself is the player and knows the position exactly). Premium required, ToS gray area, the audio then runs through LyricHub. Only as an experiment.

---

## 14. The user's decisions (2026-10-25)

### 14.1 Answers

| Question | Answer | Consequence for the plan |
|---|---|---|
| Spotify Premium? | Yes | The Web API (section 8) is possible but stays optional. No ads, so no ad detection is needed. |
| Spotify version? | Microsoft Store | Spike 0 measures with the Store version (AUMID `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`). |
| Audio output? | Wired | Calibration targets wired output. Device offsets stay as a feature but are not critical. |
| Target audience? | Personal use first, possibly a free release later | See 14.2. |
| Lyrics languages? | English, German, Japanese (in this order), occasionally others | Japanese font fallbacks and character-based word estimation are mandatory for v1 (5.6, 9.5). Romanization (13.6) is the first candidate for Phase 6. |
| Fullscreen mode of the games? | Mixed | Exclusive fullscreen is an everyday case. Spike B tests both modes; the detection including the note (6.3) is mandatory for v1. |
| Spicetify as Plan B? | Yes | Fallback order if SMTC is too inaccurate: Spicetify first, then the Web API (3.2). |
| Store or desktop for Spicetify? | Switch to desktop Spotify only if needed | No switch in advance. Only if Spike 0 misses the sync requirement does the user switch to the desktop version. |
| Framework if the spike runs into problems? | Opus decides | Tauri stays the default. A switch to C#/WPF is justified in `docs/en/spike-overlay.md`, without asking the user. |
| Edit mode via the mouse? | No, only via hotkey and tray | Outside edit mode, the overlay always stays click-through. |
| Nudge on a song change? (2026-10-25, after Phase 2) | Remember the correction per song | Nudge changes only the song offset of the current song. The next song starts with its own value, a returning song with its stored correction (5.7). |
| Code hosting? (2026-10-25) | Private GitHub repo `HeiligerHoly/LyricsHub` | All branches and tags are pushed there. Releasing remains a separate decision (14.2). |

### 14.2 Staying releasable

A later free release must not be ruled out. Therefore:

- The core function may require neither Spicetify nor the Web API. Both stay optional additional sources.
- Because of the 5-user limit in Development Mode, the Web API cannot carry a release. Every user would have to enter their own client ID.
- No personal paths, keys or IDs in the code. Everything user-specific lives in the settings.
- Settle the license disclaimer for lyrics (7.4) and an open-source license for the code early. Dependencies only with compatible licenses (MIT, Apache-2.0, BSD; OFL for fonts).
- Code signing, auto-update and Store publication only become necessary after the release decision.

### 14.3 Still open

No blocking questions. Opus only asks if Spike 0 or Spike B produces a result that affects one of these decisions, for example the switch to desktop Spotify.

---

## 15. Working rules for Opus

- **Spike first, then build.** No sync implementation before the SMTC measurement report.
- **Thin vertical slice before polish.** Phase 1 must run completely over a game before lyrics come.
- **Sync logic stays UI-free and tested.** `sync/clock.rs` must not depend on Tauri.
- **No IPC per frame.** The frontend computes the position itself.
- **Never steal focus.** Every window operation is tested against this.
- **No DLL injection, no hooks into game processes, no unofficial Spotify endpoints, no cookies.**
- **Secrets**: tokens only in the Credential Manager; the client ID is public, no client secret in the project.
- **Version the settings schema** (`schema_version`) and write migrations as soon as fields change.
- **Logs instead of guesses**: every sync decision (seek reset, correction, source conflict) is logged with its values. The Sync Lab reads this data.
- **Small, focused commits**, Conventional Commits (`feat(sync): …`, `fix(overlay): …`). Every phase ends with a tag (`v0.1.0-phase1` …).
- **Before completing each phase**, tick off the phase's DoD list and the affected acceptance criteria from section 2, and record the result in `docs/`.
- **Verify uncertain facts**: Spotify API rules (Migration Guide Feb 2026), LRCLIB parameters, Tauri behavior of the version in use. This plan reflects the state as of October 2026; document deviations instead of silently working around them.

---

## Appendix A: Sources (research of 2026-10-25)

- Spotify: Quota Modes – https://developer.spotify.com/documentation/web-api/concepts/quota-modes
- Spotify Blog: Update on Developer Access and Platform Security (2026-02-06) – https://developer.spotify.com/blog/2026-02-06-update-on-developer-access-and-platform-security
- Spotify: February 2026 Web API Dev Mode Changes – Migration Guide – https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide
- Spotify Blog: Updating the Criteria for Web API Extended Access (2025-04-15) – https://developer.spotify.com/blog/2025-04-15-updating-the-criteria-for-web-api-extended-access
- TechCrunch on the Dev Mode changes – https://techcrunch.com/2026/02/06/spotify-changes-developer-mode-api-to-require-premium-accounts-limits-test-users/
- LRCLIB endpoints (third-party documentation) – https://amll.dev/en/reference/http-api/lrclib.html
- LRCLIB repo (issue on /api/get and duration) – https://github.com/tranxuanthang/lrclib/issues/129
- Microsoft: GlobalSystemMediaTransportControlsSessionTimelineProperties – https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessiontimelineproperties
- Microsoft: Windows.Media.Control Namespace – https://learn.microsoft.com/en-us/uwp/api/windows.media.control
- Soggfy PR on the Spotify timeline refresh and position extrapolation – https://github.com/wefalltomorrow/Soggfy/pull/48
- WindowsMediaController (C# reference implementation for SMTC) – https://github.com/DubyaDude/WindowsMediaController
- Tauri 2 experience report on ex-styles and set_ignore_cursor_events – https://www.producthunt.com/p/tauri-2/i-built-sticky-notes-that-hide-in-your-windows-screen-edge-with-tauri-2-6-lessons-learned
- Tauri issue on hover with ignored cursor events – https://github.com/tauri-apps/tauri/issues/9250
