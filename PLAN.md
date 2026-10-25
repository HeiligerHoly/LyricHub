# LyricHub – Implementierungsplan für Opus

> Stand: 2026-10-25. Erstellt von Fable als Arbeitsgrundlage für Opus.
> Sprache im Code/Kommentaren: Englisch. Sprache der UI: Deutsch + Englisch (i18n-fähig, Deutsch zuerst).
> Zielplattform: Windows 11 (Windows 10 ab 2004 wo möglich).
> Aktualisiert am 2026-10-25 mit den Antworten des Nutzers. Die Entscheidungen stehen in Abschnitt 14.

---

## 0. Kurzfassung

LyricHub ist ein **Game-Overlay für Windows**, das erkennt, was gerade in Spotify läuft, und am Bildschirmrand (Position, Größe und Monitor in den Einstellungen frei wählbar) den aktuellen Song sowie die **synchron zur hörbaren Audioausgabe** mitlaufenden Lyrics anzeigt. Die aktuell gesungene Zeile leuchtet **karaoke-artig** auf (fließende Füllung über den Text).

Die drei harten Kernprobleme, in Reihenfolge ihrer Wichtigkeit:

1. **Sync**: Die Lyrics müssen zu dem passen, was der Nutzer *hört*, nicht zu dem, was eine API meldet. Dafür gibt es ein eigenes Zeitmodell (`PlaybackClock`) mit Quellen-Fusion, Jitter-Filter und drei Offset-Ebenen (global, pro Audiogerät, pro Song).
2. **Overlay-Technik**: Transparent, immer oben, click-through, **darf niemals den Fokus vom Spiel klauen**, überlebt Fullscreen-Wechsel und Multi-Monitor.
3. **Lyrics-Beschaffung**: Zeitgestempelte Lyrics (LRC) aus LRCLIB mit robustem Matching, lokalem Cache und Fallbacks.

**Zentrale Architekturentscheidung**: Die App funktioniert **ohne Spotify Web API** vollständig. Primäre Quelle für "was läuft gerade" ist die Windows-Media-Session (SMTC, `Windows.Media.Control`), die Spotify lokal befüllt. Die Spotify Web API ist ein **optionaler Präzisions- und Metadaten-Layer**. Grund: Spotify hat im Februar 2026 den Development Mode stark eingeschränkt (Premium-Pflicht für den App-Besitzer, max. 5 Nutzer, Endpoint-Beschränkungen angekündigt). Eine App, die ohne API läuft, ist robuster gegen diese Politik.

---

## 1. Produktdefinition

### 1.1 Zielbild (v1)

- Läuft im Tray, startet optional mit Windows.
- Erkennt automatisch Titel, Artist, Album, Dauer, Cover und Wiedergabestatus von Spotify (Desktop- und Store-Version).
- Holt synchronisierte Lyrics, zeigt sie im Overlay, hebt die aktuelle Zeile mit fließender Füllung hervor.
- Overlay-Position (9 Ankerpunkte + frei), Skalierung, Breite, Monitor und Layout in den Einstellungen und per Edit-Modus direkt am Bildschirm einstellbar.
- Sync-Werkzeuge: globaler Offset, Nudge-Hotkeys (±100 ms), Offset pro Audiogerät (z. B. Bluetooth-Kopfhörer), Offset pro Song, Kalibrier-Assistent, Diagnose-Ansicht.
- Funktioniert über Spielen im Borderless-/Windowed-Fullscreen.

### 1.2 Nicht-Ziele (v1)

- Kein Overlay in **Exclusive Fullscreen** (DirectX-FSE). Das bräuchte DLL-Injection in den Spielprozess (wie Steam/Discord), was Anti-Cheat-Risiken birgt. Bewusst ausgeschlossen. Die meisten Spiele laufen heute borderless; Windows-"Fullscreen-Optimierungen" machen FSE außerdem meist überlagerbar.
- Keine Wiedergabesteuerung (Play/Pause/Skip) in v1. Nur lesen.
- Keine wortgenauen Timestamps aus Quellen, die es nicht liefern. Wortgenaue Darstellung wird **geschätzt** (siehe 5.6), echte Wort-Timings nur, wenn Enhanced LRC vorliegt.
- Kein Zugriff auf Spotifys interne Lyrics-Endpoints (sp_dc-Cookie, spclient). Verstößt gegen die ToS und bricht regelmäßig.

### 1.3 Setup des Nutzers (bestätigt am 2026-10-25, Details in Abschnitt 14)

- Ein Windows-PC, auf dem Spotify lokal läuft (nicht Spotify Connect auf einem anderen Gerät; dafür bräuchte es zwingend die Web API).
- **Spotify aus dem Microsoft Store**, Account mit **Premium**.
- Audioausgabe überwiegend **kabelgebunden** (geringe, stabile Latenz).
- Spiele laufen **gemischt** in Borderless- und exklusivem Vollbild.
- Lyrics-Sprachen nach Häufigkeit: **Englisch, Deutsch, Japanisch**, vereinzelt weitere.
- Nutzer akzeptiert, einmalig einen Offset zu kalibrieren, wenn die automatische Sync nicht perfekt ist.
- **Zunächst Eigennutzung.** Eine spätere kostenlose Veröffentlichung ist möglich, deshalb keine Entscheidungen treffen, die eine Weitergabe verbauen (siehe 14.2).

---

## 2. Harte Anforderungen und Akzeptanzkriterien

| Bereich | Kriterium | Messmethode |
|---|---|---|
| Sync-Genauigkeit | Nach Kalibrierung mit kabelgebundener Ausgabe: Median-Fehler ≤ 100 ms, p95 ≤ 200 ms zwischen Zeilenstart im Overlay und hörbarem Einsatz | Manuelles Protokoll mit 10 Songs (Abschnitt 11.3) |
| Sync-Stabilität | Kein sichtbares Zittern. Korrekturen werden über Slew (max. ±10 % Geschwindigkeit) eingearbeitet, kein Rückwärtssprung < 300 ms | Sync-Lab-Diagnose + Augenschein |
| Seek-Erkennung | Nach Spulen innerhalb ≤ 300 ms (SMTC-Event) bzw. ≤ 1 s (nur Web API) wieder korrekt | Manuell |
| Track-Wechsel | Neuer Titel im Overlay ≤ 300 ms; Lyrics ≤ 1,5 s online, ≤ 100 ms aus Cache | Logging |
| Fokus | Overlay nimmt **nie** den Eingabefokus. Tastatureingaben im Spiel laufen ungestört weiter, auch bei Track-Wechsel und Edit-Modus-Ende | Testspiel + Notepad-Test |
| Overlay-Sichtbarkeit | Bleibt über Borderless-Fullscreen-Spielen sichtbar, auch wenn das Spiel topmost reasserted | 3 Testspiele + Browser-Vollbild-Video |
| Performance | Pausiert: < 0,5 % CPU; spielend: < 2 % CPU auf Mittelklasse-CPU; RAM < 200 MB; keine spürbare FPS-Einbuße im Spiel | Task-Manager, 10 Minuten Messung |
| Robustheit | Spotify-Neustart, Gerätewechsel (BT-Kopfhörer), Monitorwechsel, Schlafmodus: App erholt sich ohne Neustart | Manuell |
| Kein Fokusklau beim Start | App startet minimiert in den Tray, öffnet kein Fenster, wenn Autostart aktiv | Manuell |

---

## 3. Architekturentscheidungen (mit Begründung)

### 3.1 Framework: Tauri 2 (Rust-Backend + WebView2-Frontend)

**Entscheidung**: Tauri 2, Rust-Backend, Frontend mit Svelte 5 + TypeScript (React ist akzeptabel, falls Opus damit schneller ist; die Komponentenanzahl ist klein).

**Warum**:
- Das Backend braucht direkten Zugriff auf Win32 und WinRT (SMTC, Fensterflags, Audio-Geräte). Der `windows`-Crate macht das erstklassig.
- Das Frontend braucht erstklassige Typografie und Animationen (Textfüllung mit Gradient, Glow, Easing). CSS/Canvas im WebView2 ist dafür ideal und deutlich einfacher als WPF-Shader.
- Tauri unterstützt transparente, rahmenlose, always-on-top Fenster und `set_ignore_cursor_events` out of the box; bekannte Fallstricke sind dokumentiert (Abschnitt 6).
- Kleines Bundle, geringe Idle-Last.

**Alternative** (falls Tauri bei der Overlay-Transparenz unerwartet Probleme macht): C# / WPF mit `Windows.Media.Control` (nativ, sehr einfach) und WPF-Textanimationen. Opus entscheidet nach Spike B (Phase 0) selbst und begründet die Wahl in `docs/spike-overlay.md` (Nutzerentscheidung, 14.1).

**Nicht gewählt**: Electron (schwerer, Transparenz unter Windows historisch zickig, SMTC braucht Native-Addon).

### 3.2 Quelle für "Was läuft": SMTC primär, Spotify Web API optional

| Quelle | Liefert | Latenz | Kosten/Hürden |
|---|---|---|---|
| **SMTC** (`GlobalSystemMediaTransportControlsSessionManager`) | Titel, Artist, Album, Cover (Thumbnail-Stream), Status, Timeline (Position + `LastUpdatedTime`, Start/End = Dauer) | Events in < 100 ms bei Play/Pause/Wechsel | Keine. Kein Login, kein Netz, kein Limit |
| **Spotify Web API** `GET /me/player` | Track-ID, `progress_ms`, `is_playing`, `currently_playing_type` (ad/episode), `is_local`, Cover-URLs | Polling 1 s + RTT (typisch 100–400 ms), Jitter | OAuth PKCE, eigene Developer-App, **Premium-Pflicht des Besitzers, 5-Nutzer-Limit, Rate Limits, Politikrisiko** |

**Entscheidung**: SMTC ist Pflicht und reicht für v1, wenn Spike 0 das bestätigt. Fallback-Reihenfolge, falls die SMTC-Position zu ungenau ist (vom Nutzer bestätigt, siehe 14.1):

1. **Plan B**: Der Nutzer wechselt von der Store- auf die Desktop-Version von Spotify, und der Spicetify-Companion (13.1) wird als präzise Positionsquelle vorgezogen.
2. **Plan C**: Spotify Web API als zusätzliche Quelle (Phase 3b, Abschnitt 8). Der Nutzer hat Premium, der Weg ist also offen.

Die Web API kann unabhängig davon später für stabile Track-IDs ergänzt werden.

**Ergebnis Spike A (2026-10-25)**: SMTC reicht. Spotify veröffentlicht die Timeline alle 4,5 s plus bei jedem Ereignis, und die Extrapolation weicht im Normalbetrieb im 95. Perzentil um 1 ms ab. Plan B und C entfallen vorerst. Details und Folgen für die Seek- und Songwechsel-Behandlung stehen in `docs/spike-smtc.md`.

**Offene Unsicherheit, die Spike 0 klären muss**: Wie oft aktualisiert der Spotify-Client die SMTC-Timeline? Community-Berichte (z. B. ein Fix im Projekt Soggfy) zeigen, dass Spotify die Timeline refresht und Leser die Position aus `Position + (now - LastUpdatedTime)` extrapolieren müssen, wobei Refreshes die extrapolierte Position leicht rückwärts springen lassen können. Die genaue Kadenz und der Jitter müssen gemessen werden (Abschnitt 5.8).

### 3.3 Lyrics-Quelle: LRCLIB primär, Provider-Schnittstelle, lokaler Cache

- **LRCLIB** (`https://lrclib.net/api`): frei, ohne API-Key, liefert `syncedLyrics` im LRC-Format (zeilengenau), `plainLyrics`, `instrumental`-Flag. Exakte Abfrage über `/api/get` (track_name, artist_name, album_name, duration; Dauer-Toleranz ca. ±2 s), Fallback über `/api/search`. `404 TrackNotFound` wenn nichts gefunden. Aussagekräftigen `User-Agent` setzen (LRCLIB bittet darum, z. B. `LyricHub/0.1.0 (https://github.com/<repo>)`).
- **Provider-Trait** (`LyricsProvider`), damit später NetEase, Musixmatch (mit eigenem Key) oder lokale LRC-Ordner ergänzt werden können, ohne den Rest anzufassen.
- **Lokaler Cache** (SQLite): Roh-LRC, Quelle, Zeitpunkt, Nutzer-Overrides (Song-Offset, manuell importierte/editierte LRC).
- **Manueller Import**: Nutzer kann eine `.lrc`-Datei für einen Song hinterlegen.

### 3.4 Sync-Modell: `PlaybackClock` in reinem Rust, Frontend rechnet lokal weiter

- Das Backend hält ein Zeitmodell (`Anchor`: Position zu monotonem Zeitpunkt, Status, Rate) und fusioniert Beobachtungen aus allen Quellen (Median-Residual-Filter, Seek-Erkennung, Slew).
- Das Frontend bekommt **nicht pro Frame** eine Position per IPC, sondern selten einen Anker (`{pos_ms, playing, rate, seq}`) und rechnet mit `performance.now()` selbst weiter. IPC-Jitter beeinflusst die Darstellung damit nicht.
- Die Sync-Logik lebt in einem UI-freien Modul mit Unit-Tests gegen synthetischen Jitter.

### 3.5 Overlay: eigenes Fenster mit Win32-Flags, so klein wie der Inhalt

- Separates Tauri-Fenster `overlay`: `transparent`, `decorations: false`, `alwaysOnTop`, `skipTaskbar`, `shadow: false`, `focus: false`.
- Zusätzlich per Win32: `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOPMOST`.
- Das Fenster ist **nur so groß wie der Overlay-Inhalt**, nicht bildschirmfüllend (spart Compositing-Bandbreite, wichtig für Spiel-FPS).

---

## 4. Systemarchitektur

### 4.1 Überblick

```
┌──────────────────────────────── Rust-Backend (Tauri) ────────────────────────────────┐
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
│  overlay/win32 (Flags, Topmost-Reassert, Fullscreen-Detect, Monitor/DPI)              │
│  audio_devices (Default-Output-Watcher → Geräte-Offset)                               │
│  hotkeys, tray, settings(store+schema), logging                                       │
└───────────────────────────────────────────────────────────────────────────────────────┘
                 │                                               │
                 ▼                                               ▼
   ┌── Overlay-Fenster (WebView2) ──┐             ┌── Settings-Fenster (WebView2) ──┐
   │ lokaler Clock (rAF)            │             │ Verbindung, Overlay, Darstellung │
   │ KaraokeLine (Gradient-Fill)    │             │ Sync, Lyrics, Hotkeys, Verhalten │
   │ Layouts, Themes, Edit-Modus    │             │ Diagnose ("Sync Lab")            │
   └────────────────────────────────┘             └──────────────────────────────────┘
```

### 4.2 Projektstruktur

```
LyricHub/
  PLAN.md
  README.md                      # inkl. Setup-Anleitung (Spotify-App optional)
  docs/
    spike-smtc.md                # Messbericht aus Phase 0
    sync-design.md               # Detailnotizen zum Clock-Modell (aus Abschnitt 5 ableiten)
  src-tauri/
    tauri.conf.json              # zwei Fenster: "main" (Settings), "overlay"
    Cargo.toml
    src/
      main.rs / lib.rs
      app_state.rs               # tokio::sync::watch für Anchor, Track, Lyrics
      sources/
        mod.rs                   # trait PlaybackSource { fn subscribe(&self) -> Receiver<SourceEvent> }
        smtc.rs                  # WinRT GlobalSystemMediaTransportControlsSessionManager
        spotify_api.rs           # optional, feature-gated: PKCE, Polling, 429-Handling
        types.rs                 # TrackInfo, PlaybackStatus, Observation, SourceEvent
      sync/
        clock.rs                 # PlaybackClock: Anchor, Residual-Filter, Slew, Seek-Detect (pure, testbar)
        fusion.rs                # Gewichtung der Quellen, Latenzschätzung
        offsets.rs               # global + device + song, Vorzeichenkonvention
      lyrics/
        mod.rs                   # trait LyricsProvider
        lrclib.rs
        matcher.rs               # Normalisierung, Kandidaten-Scoring
        parser.rs                # LRC, Enhanced LRC (<mm:ss.xx>), [offset:] Tag
        estimator.rs             # Wortgrenzen-Schätzung für zeilengenaue LRC
        cache.rs                 # SQLite (rusqlite, bundled)
        types.rs                 # Lyrics, Line, Word
      overlay/
        window.rs                # Geometrie (Anker, Monitor, Skalierung), Edit-Modus
        win32.rs                 # Ex-Styles, SetWindowPos Topmost-Reassert, NoActivate
        fullscreen.rs            # SHQueryUserNotificationState + Foreground-Rect-Check
      audio_devices.rs           # IMMDeviceEnumerator + IMMNotificationClient → Default-Output-ID
      hotkeys.rs                 # tauri-plugin-global-shortcut
      tray.rs
      settings/
        schema.rs                # versioniertes Settings-Struct + Migration
        store.rs                 # JSON in %APPDATA%/LyricHub/settings.json
      ipc/
        commands.rs              # #[tauri::command] get_settings, set_settings, nudge_offset, ...
        events.rs                # "track", "lyrics", "clock", "settings", "overlay-mode"
      logging.rs                 # tracing + Datei-Rotation in %APPDATA%/LyricHub/logs
  src/                           # Frontend (Svelte 5 + TS + Vite)
    overlay/
      Overlay.svelte
      KaraokeLine.svelte         # Gradient-Fill + Wort-Glow
      clock.ts                   # lokaler Anchor + Slew, performance.now()
      layouts/ (Bar, Karaoke, Stack, Minimal)
      themes.ts
      editMode.ts
    settings/
      App.svelte + pages/ (Connection, Overlay, Appearance, Sync, Lyrics, Hotkeys, Behavior, Diagnostics)
    shared/
      types.ts                   # gespiegelt aus Rust (ts-rs oder manuell)
      ipc.ts                     # typed invoke/listen Wrapper
      i18n/ (de.json, en.json)
```

### 4.3 Datenmodell (Kern)

```rust
pub struct TrackInfo {
    pub key: TrackKey,            // Spotify-ID wenn vorhanden, sonst norm(artist)|norm(title)|round(duration_s)
    pub title: String,
    pub artist: String,           // "A, B" bei mehreren
    pub album: Option<String>,
    pub duration_ms: Option<u32>, // aus SMTC EndTime oder API
    pub art: Option<ArtRef>,      // Bytes aus SMTC-Thumbnail oder URL aus API
    pub kind: TrackKind,          // Music | Ad | Podcast | Unknown
    pub source: SourceId,
}

pub struct Observation {
    pub source: SourceId,         // Smtc | SpotifyApi
    pub pos_ms: f64,
    pub valid_at: Instant,        // geschätzter Zeitpunkt, zu dem pos_ms galt
    pub playing: bool,
    pub confidence: f32,          // 0..1, quellenabhängig
}

pub struct Anchor {               // geht per IPC ans Frontend
    pub pos_ms: f64,              // ROHE Position zum Sendezeitpunkt (ohne Offsets)
    pub playing: bool,
    pub rate: f64,                // 1.0 normal; während Slew 0.9..1.1
    pub seq: u64,
    pub offset_ms_total: i32,     // global + device + song + lrc, separat, damit das Frontend Diagnose zeigen kann
}

pub struct Lyrics {
    pub key: TrackKey,
    pub source: String,           // "lrclib" | "local" | "user"
    pub synced: bool,
    pub lines: Vec<Line>,
    pub lrc_offset_ms: i32,       // aus [offset:] Tag
}
pub struct Line { pub start_ms: u32, pub end_ms: Option<u32>, pub text: String, pub words: Option<Vec<Word>> }
pub struct Word { pub start_ms: u32, pub end_ms: u32, pub text: String, pub estimated: bool }
```

### 4.4 IPC-Verträge

Events (Backend → Frontend):

| Event | Payload | Wann |
|---|---|---|
| `track` | `TrackInfo` | Track-Wechsel, Metadaten-Update (Cover nachgeladen) |
| `lyrics` | `Lyrics` oder `{ key, state: "loading" \| "missing" \| "instrumental" \| "unsynced", plain?: string }` | Nach Matching |
| `clock` | `Anchor` | Statuswechsel, Korrektur > 20 ms, Seek, plus Heartbeat alle 2 s |
| `settings` | komplettes Settings-Objekt | Nach jeder Änderung |
| `overlay-mode` | `{ edit: bool }` | Edit-Modus an/aus |
| `diag` | Sync-Lab-Sample (nur wenn Diagnose-Seite offen) | 4 Hz |

Commands (Frontend → Backend): `get_settings`, `set_settings(patch)`, `nudge_offset(delta_ms)`, `set_song_offset(key, ms)`, `set_device_offset(device_id, ms)`, `reload_lyrics`, `import_lrc(path)`, `enter_edit_mode`, `save_overlay_geometry(geom)`, `cancel_edit_mode`, `spotify_login`, `spotify_logout`, `open_logs_folder`, `run_calibration_step(...)`.

---

## 5. Der Sync im Detail (Kernstück)

### 5.1 Fehlerquellen und wie sie behandelt werden

| Fehlerquelle | Größenordnung | Behandlung |
|---|---|---|
| SMTC-Timeline wird nur sporadisch aktualisiert | unbekannt, zu messen (Spike 0) | Extrapolation ab `LastUpdatedTime`; jede Aktualisierung ist eine Beobachtung für den Filter |
| Web-API `progress_ms` ist zum Antwortzeitpunkt schon veraltet | 100–1000 ms, jittert | `valid_at = t_send + RTT/2`; `timestamp`-Feld der API **nicht** als "jetzt" interpretieren (Semantik in der Praxis inkonsistent) |
| Polling-Lücken | bis 1 s | Lokale Extrapolation mit monotoner Uhr |
| Audio-Ausgabelatenz (Windows-Audiostack, Bluetooth 100–300 ms, Wireless-Headsets, TV-HDMI) | 20–300 ms, gerätespezifisch | **Offset pro Audiogerät**, automatisch angewendet beim Gerätewechsel |
| LRC-Timestamps ungenau / andere Edition des Songs | ±0–2000 ms | **Offset pro Song**, Nudge-Hotkeys, Kandidaten-Scoring über Dauer |
| Spotify-Crossfade / Automix | Trackgrenzen verschwimmen | Hinweis in Einstellungen, Crossfade für beste Sync zu deaktivieren; Track-Wechsel per SMTC-Event, nicht per Position |
| Seek durch Nutzer | beliebig | Residual > 1500 ms → Hard-Reset des Ankers, Filterfenster leeren |
| Pause/Resume | – | SMTC-Status-Event sofort; Position bei Pause einfrieren |
| Rückwärtssprünge nach Timeline-Refresh (SMTC) | 50–300 ms | Residual-Filter + Slew, nie hart rückwärts springen, wenn Residual < 300 ms |

### 5.2 Vorzeichenkonvention für Offsets

`display_pos_ms = clock_pos_ms + offset_global + offset_device[current_output] + offset_song[key] + lrc_offset`

**Positiv = Lyrics erscheinen später** (also: "Lyrics kommen zu früh → Offset erhöhen"). In der UI immer als Satz formulieren: "Lyrics zu früh? → nach rechts", nicht nur als Zahl.

### 5.3 `PlaybackClock` (Pseudocode, Rust-nah)

```rust
const SEEK_THRESHOLD_MS: f64 = 1500.0;
const WINDOW: usize = 8;               // Residuen pro Quelle
const SLEW_MAX_RATE_DELTA: f64 = 0.10; // ±10 % Geschwindigkeit beim Angleichen
const DEADBAND_MS: f64 = 20.0;         // kleinere Korrekturen ignorieren

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
        // Anker auf 0 setzen, bis erste Beobachtung kommt (Track-Start ist eine gute Annahme)
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
            // Seek oder völlig falscher Anker → hart setzen
            self.anchor = Anchor { pos_ms: obs.pos_ms, at: obs.valid_at, playing: obs.playing, rate: 1.0 };
            self.residuals.clear(); self.slew_target = None;
            self.emit(); return;
        }
        let w = self.residuals.entry(obs.source).or_default();
        w.push_back(residual); if w.len() > WINDOW { w.pop_front(); }

        // robuste Fusion: gewichteter Median über alle Quellen (Gewicht = confidence)
        let corr = weighted_median(self.residuals.iter(), confidences);
        if corr.abs() < DEADBAND_MS { return; }

        // Slew statt Sprung: Anker-Rate kurzzeitig anpassen, bis corr abgebaut ist
        let now = Instant::now();
        let cur = self.predicted(now);
        self.anchor = Anchor { pos_ms: cur, at: now, playing: self.anchor.playing, rate: 1.0 };
        self.slew_target = Some(cur + corr);
        // Rate so wählen, dass corr innerhalb von max(300 ms, |corr|/0.1) abgebaut ist
        let dur_ms = (corr.abs() / SLEW_MAX_RATE_DELTA).max(300.0);
        self.anchor.rate = 1.0 + corr / dur_ms;        // z. B. corr=+60 → rate 1.1 für 600 ms
        self.schedule_rate_reset_after(dur_ms);        // danach rate=1.0, Anker neu setzen
        self.emit();
    }
}
```

Hinweise für Opus:
- `weighted_median` mit einem einzigen Fenster ist für v1 ausreichend; ein Kalman-Filter ist nicht nötig und schwerer zu debuggen.
- Das Frontend implementiert denselben Slew-Mechanismus lokal: Beim Empfang eines Ankers nicht springen, sondern (wenn Differenz < 300 ms) über 300 ms angleichen. Dadurch bleibt die Darstellung auch bei leichtem Backend-Jitter ruhig.
- Negative Korrekturen (Lyrics zu weit vorne) **nie** als Rückwärtssprung rendern, wenn sie < 300 ms sind; stattdessen Rate auf 0,9 senken.

### 5.4 Quellen-Fusion und Latenzschätzung

- **SMTC**: `valid_at` aus `LastUpdatedTime` (UTC `DateTime`) → Alter berechnen → `Instant::now() - age`. Confidence 0,8. Zusätzlich alle 250 ms die Timeline pollen (billig, lokal), nicht nur auf Events warten; nur **geänderte** `LastUpdatedTime`-Werte als Beobachtung zählen, sonst entsteht Autokorrelation.
- **Web API** (optional): `valid_at = t_send + rtt/2`. Confidence 0,6 (höher nach 3 konsistenten Samples). Adaptives Polling: 1000 ms spielend, 500 ms in den letzten 3 s eines Tracks, 3000–5000 ms pausiert, sofort nach SMTC-Event; bei HTTP 429 `Retry-After` respektieren.
- **Track-Wechsel-Erkennung**: SMTC-Event `MediaPropertiesChanged` (schnell) hat Vorrang. Die API bestätigt später die Track-ID.
- **Konfliktregel**: Weichen SMTC und API dauerhaft (> 5 Samples) um mehr als 400 ms voneinander ab, gewinnt die Quelle mit geringerer Varianz; die andere wird im Sync-Lab als "abweichend" markiert.

### 5.5 Sonderzustände

| Zustand | Erkennung | Overlay-Verhalten |
|---|---|---|
| Werbung (Free-Account) | API: `currently_playing_type == "ad"`; ohne API: SMTC-Titel "Advertisement"/"Spotify", Artist leer, Dauer ≈ 30 s | Overlay ausblenden oder nur "Werbung" dezent |
| Podcast | API: `"episode"`; SMTC: Dauer > 20 min und kein Album | Nur Titel, keine Lyrics-Suche |
| Lokale Datei | API: `is_local`; SMTC normal | Matching über Titel/Artist/Dauer |
| Instrumental | LRCLIB `instrumental: true` | "Instrumental" dezent, nach 5 s nur Titel |
| Keine synchronen Lyrics | nur `plainLyrics` | Titel + Hinweis "Keine synchronen Lyrics"; optional Plain-Text statisch |
| Spotify geschlossen | SMTC-Session verschwindet | Overlay ausblenden, Tray-Icon grau |
| Pause | SMTC-Status | Overlay dimmt nach 2 s auf 60 %, blendet nach konfigurierbarer Zeit (Standard 10 s) aus |

### 5.6 Wortgenaues Highlight ohne Wort-Timestamps (Schätzung)

LRCLIB liefert fast immer nur Zeilen-Timestamps. Ein echtes Wort-für-Wort-Aufleuchten wäre bei geschätzten Grenzen sichtbar falsch. Deshalb:

1. **Darstellung als kontinuierliche Füllung** (Gradient wandert über die Zeile), nicht als diskretes Wort-Umschalten. Schätzfehler fallen so kaum auf.
2. Zeilenende: `end = min(next.start, start + max_dur)`; wenn die Lücke zur nächsten Zeile > 2,5 s ist, Zeilendauer aus Textlänge schätzen (`chars * 70 ms`, geclamped auf 1–8 s) und den Rest als Instrumentalpause behandeln (Füllung bleibt voll, Zeile bleibt stehen, dezenter Pulsierer).
3. Wortgrenzen innerhalb der Zeile proportional zur Zeichenanzahl ohne Satzzeichen, mit 60 ms Lead-in und 120 ms Tail. Silbenzählung ist für Deutsch/Englisch nicht nötig; Zeichenanzahl korreliert ausreichend.
   **Japanisch** (dritthäufigste Sprache des Nutzers) hat keine Leerzeichen. Hier wird pro Zeichen statt pro Wort aufgeteilt: Kana zählen 1, Kanji 1,5 (meist mehrere Moren), Satzzeichen 0. Chinesisch wie Kanji, Koreanisch pro Silbenblock. Die Schrift wird pro Zeile über Unicode-Bereiche erkannt.
4. Liegt **Enhanced LRC** vor (`<mm:ss.xx>` vor Wörtern) oder eine Nutzer-LRC mit Wortmarken, werden echte Wortzeiten verwendet (`estimated: false`), und der Glow springt wortgenau.

### 5.7 Offsets, Nudge und Kalibrier-Assistent

- **Nudge-Hotkeys**: Standard `Ctrl+Alt+,` (−100 ms) / `Ctrl+Alt+.` (+100 ms), mit Shift ±25 ms. Ein kleines Toast im Overlay zeigt den neuen Wert 1,5 s lang. Die Änderung geht standardmäßig auf den **Song-Offset**; in den Einstellungen umschaltbar auf global.
- **Geräte-Offset**: `audio_devices.rs` überwacht das Default-Render-Gerät (`IMMNotificationClient::OnDefaultDeviceChanged`). Settings speichern `device_offsets: { "<device-id>": ms }`. Beim ersten Auftauchen eines neuen Geräts: Toast "Neues Audiogerät erkannt. Sync prüfen?" mit Link zum Assistenten. Bluetooth-Geräte bekommen einen Startwert von +150 ms als Vorschlag. Der Nutzer hört überwiegend über Kabel. Der Geräte-Offset ist deshalb kein kritischer Pfad und wird in Phase 3 schlicht umgesetzt (Tabelle + Toast, kein eigener Assistent pro Gerät).
- **Kalibrier-Assistent** (Settings → Synchronisation): spielt nichts selbst ab, sondern nutzt den laufenden Song. Ablauf: Overlay zeigt die nächste Zeile groß mit Countdown-Balken; Nutzer drückt Leertaste genau beim hörbaren Einsatz; 3–5 Wiederholungen; Median der Differenzen wird als Geräte-Offset vorgeschlagen. Ausreißer (> 600 ms) verwerfen.
- **Globaler Offset**: Reserve für Fälle, die weder Gerät noch Song zuzuordnen sind. Standard 0.

### 5.8 Spike 0: SMTC-Messung (vor allem anderen durchführen)

Ziel: Wissen, ob SMTC allein die Sync-Anforderung erfüllt.

1. Kleines Rust-Binary (oder Tauri-Command) baut die SMTC-Session zu Spotify auf, loggt bei jedem `TimelinePropertiesChanged`-Event und zusätzlich per 100-ms-Polling: `Position`, `LastUpdatedTime`, `now`, Status, Titel. Ausgabe als CSV.
2. 10 Minuten Musik mit Play/Pause/Seek/Track-Wechsel, gemessen mit der **Microsoft-Store-Version** von Spotify (Setup des Nutzers). Die Desktop-Version wird nur gemessen, wenn der Nutzer für Plan B wechselt.
3. Auswertung in `docs/spike-smtc.md`: Update-Kadenz (Events pro Minute), Verteilung von `now - LastUpdatedTime` bei Events, Jitter der extrapolierten Position zwischen zwei Updates, Verhalten bei Seek (Latenz bis Event), Verhalten bei Pause.
4. Entscheidung: Jitter p95 < 150 ms → SMTC allein ist v1-Quelle. Sonst → Plan B aus 3.2: Ergebnis dem Nutzer vorlegen, der Nutzer wechselt auf Desktop-Spotify, und der Spicetify-Companion (13.1) wird vorgezogen. Die Web API (Phase 3b) kommt nur, wenn auch das nicht reicht. Spicetify unterstützt die Store-Version nicht zuverlässig, deshalb ist der Wechsel Voraussetzung.

Fallstricke: WinRT-Aufrufe brauchen einen initialisierten Apartment (MTA-Thread mit `RoInitialize`), nicht den Tauri-Main-Thread. Event-Handler laufen auf beliebigen Threads; Daten per `tokio::sync::mpsc` ins Backend schicken. Store-Spotify hat die AUMID `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`, Desktop-Spotify `Spotify.exe`; auf `contains("Spotify")` filtern und beide Varianten testen.

### 5.9 Diagnose-Ansicht "Sync Lab" (Settings → Diagnose)

- Zeitstrahl der letzten 60 s: Beobachtungen je Quelle als Punkte, Modellposition als Linie, Residuen als Balken.
- Live-Werte: aktuelle Position, Rate, Slew-Rest, Offsets (alle vier Komponenten), letzte Latenzschätzung, Quelle des letzten Ankers.
- Buttons: "Logs öffnen", "Diagnose-Export (JSON, 60 s)".
- Diese Ansicht ist das Werkzeug, mit dem die Akzeptanzkriterien in Abschnitt 2 nachgewiesen werden. Früh bauen (Phase 3), nicht am Ende.

---

## 6. Overlay-Fenster (Windows-Technik)

### 6.1 Fenster-Konfiguration

`tauri.conf.json` (Fenster `overlay`): `transparent: true`, `decorations: false`, `alwaysOnTop: true`, `skipTaskbar: true`, `shadow: false`, `focus: false`, `resizable: false`, `visible: false` (erst nach Geometrie-Setup zeigen).

Nach dem Erzeugen in `overlay/win32.rs` per `windows`-Crate:

```rust
let hwnd = window.hwnd()?;
let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
SetWindowLongPtrW(hwnd, GWL_EXSTYLE,
    ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize | WS_EX_TOPMOST.0 as isize);
window.set_ignore_cursor_events(true)?;   // setzt WS_EX_TRANSPARENT|WS_EX_LAYERED
```

**Bekannter Fallstrick** (aus Community-Berichten zu Tauri 2): `set_ignore_cursor_events` überschreibt die Ex-Styles und verliert dabei `WS_EX_TOOLWINDOW`; das Overlay taucht dann in Alt+Tab auf. Deshalb **nach jedem Toggle** die Ex-Styles erneut setzen (Hilfsfunktion `apply_overlay_exstyles(hwnd, click_through: bool)`).

**Update nach Spike B (Quelltext-Analyse von tao 0.37, Details in `docs/spike-overlay.md`)**: tao ruft nach jeder Flag-Änderung an einem sichtbaren Fenster zusätzlich `ShowWindow(SW_SHOW)` auf, was den Fokus klauen kann. Regel deshalb: **Nach dem Erzeugen keine Tauri-Setter mit Flag-Änderung mehr auf dem Overlay** (`show`, `hide`, `set_ignore_cursor_events`, `set_always_on_top` …). Einblenden, Klickdurchlässigkeit und Topmost laufen direkt über Win32 in `overlay/win32.rs`. `set_size` und `set_position` bleiben erlaubt. `focusable: false` im Fenster-Config setzt `WS_EX_NOACTIVATE` innerhalb von tao und ist Pflicht.

### 6.2 Nie den Fokus klauen

- `WS_EX_NOACTIVATE` auf dem Overlay. Alle `SetWindowPos`-Aufrufe mit `SWP_NOACTIVATE`.
- Beim Ein-/Ausblenden `ShowWindow(hwnd, SW_SHOWNOACTIVATE)` bzw. prüfen, ob Tauris `show()` aktiviert; falls ja, eigene Win32-Variante nutzen.
- Das Overlay wird **nie** versteckt und neu gezeigt, wenn es nur "leer" ist: Stattdessen Inhalt auf Opacity 0 setzen (Community-Bericht: Re-Show kann einen alten Frame flashen und ist teurer). Nur bei Spotify-Ende wirklich verstecken.
- Edit-Modus: Click-through aus, aber weiterhin `WS_EX_NOACTIVATE`; Drag/Resize wird im Frontend über Pointer-Events gelöst, nicht über Windows-Fensterverschiebung. Beim Verlassen Click-through wieder an und Ex-Styles neu setzen.

### 6.3 Topmost gegen Spiele behaupten

- Manche Spiele setzen sich selbst topmost. Alle 2 s, solange eine Fullscreen-App im Vordergrund ist: `SetWindowPos(hwnd, HWND_TOPMOST, 0,0,0,0, SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE)`.
- Fullscreen-Erkennung: `SHQueryUserNotificationState` → `QUNS_RUNNING_D3D_FULL_SCREEN` oder `QUNS_BUSY` (Fullscreen-App), ergänzt durch Vergleich `GetForegroundWindow`-Rect mit Monitor-Rect.
- Option "Overlay nur zeigen, wenn eine Vollbild-App läuft" (Standard aus).
- Exclusive Fullscreen: Overlay unsichtbar. Im Diagnose-Panel und im Tray-Tooltip klar anzeigen: "Spiel läuft im exklusiven Vollbild – Overlay nicht darstellbar. Tipp: Borderless Window wählen."
- Der Nutzer spielt gemischt in Borderless und exklusivem Vollbild. Diese Erkennung ist deshalb ein Alltagsfall und v1-Pflicht. Beim Erkennen wechselt das Tray-Icon auf ein Warnsymbol, und ein Hinweis erscheint im Tooltip und auf der Diagnose-Seite. Nichts davon darf das Spiel unterbrechen oder den Fokus nehmen.
- Viele DX11/DX12-Spiele, die "Vollbild" anbieten, laufen unter Windows 11 dank Fullscreen-Optimierungen tatsächlich im Flip-Modell und lassen Topmost-Fenster darüber zu. Spike B prüft das mit mindestens einem Spiel des Nutzers im exklusiven Vollbild und dokumentiert, welche Spiele das Overlay zeigen.

### 6.4 Geometrie, Multi-Monitor, DPI

- Settings speichern: `monitor_id` (Gerätename), `anchor` (9 Punkte: tl, tc, tr, ml, mc, mr, bl, bc, br), `offset_x_pct`, `offset_y_pct` (relativ zur Monitorgröße), `width_pct`, `scale` (0,5–3,0). Dadurch überleben Positionen Auflösungswechsel.
- Beim Start und bei `WM_DISPLAYCHANGE` Geometrie neu berechnen; fehlt der Monitor, auf den Primären zurückfallen.
- Per-Monitor-DPI: Tauri meldet `scale_factor`; Fenstergröße in logischen Pixeln rechnen, Frontend skaliert per CSS-Variable `--scale`.
- Fenstergröße = Inhaltsgröße: Frontend meldet per `ResizeObserver` die Bounding-Box, Backend passt das Fenster an (gedebounced, 50 ms). Mindestgröße 200×60 logische Pixel.

### 6.5 Performance

- Nur GPU-billige Eigenschaften animieren: `transform`, `opacity`, `background-position`/`background-size` auf dem Gradient-Text, `clip-path`. Kein Layout pro Frame.
- `requestAnimationFrame`-Loop läuft nur bei `playing == true` und sichtbarem Overlay; sonst stoppen. Pausiert > 10 s: Loop aus.
- Background-Throttling des WebView2 deaktivieren (sonst stottert die Animation, wenn das Overlay als Hintergrund gilt).
- Keine `backdrop-filter`-Blur-Effekte erwarten: Der WebView kann **nicht** das Spiel hinter dem transparenten Fenster blurren. Halbtransparente dunkle Platte statt Blur. Acrylic-Blur-Behind per `window-vibrancy`-Crate ist eine **experimentelle** Option (Windows 11), funktioniert nicht über Fullscreen-Apps und kostet Performance; standardmäßig aus.

---

## 7. Lyrics-Pipeline

### 7.1 Matching

1. **Normalisierung** von Titel und Artist: Kleinbuchstaben, Unicode-NFKC, Klammerzusätze entfernen (`(feat. …)`, `[Remastered 2011]`, `- Live`, `- Radio Edit` nur als Fallback-Stufe, nicht sofort), Artist bei "A, B" / "A & B" / "A feat. B" auf Haupt-Artist reduzieren.
2. **Stufe 1**: `GET /api/get?track_name=&artist_name=&album_name=&duration=` mit vollen Werten. Dauer in Sekunden (SMTC EndTime oder API `duration_ms/1000`).
3. **Stufe 2** (bei 404): `/api/get` ohne `album_name`, dann mit normalisiertem Titel.
4. **Stufe 3**: `/api/search?track_name=&artist_name=` → Kandidaten scoren: Dauerdifferenz (≤ 3 s stark bevorzugt, > 8 s verwerfen), Titel-Ähnlichkeit (Jaro-Winkler ≥ 0,85), `syncedLyrics != null` bevorzugt, `instrumental` berücksichtigen.
5. Ergebnis mit `key` cachen, auch negative Ergebnisse (für 7 Tage), um LRCLIB nicht bei jedem Replay anzufragen.
6. Zeitbudget: 1,5 s gesamt; Netzwerkfehler → Retry mit Backoff im Hintergrund, Overlay zeigt bis dahin nur Titel.

### 7.2 Parser

- Standard-LRC: `[mm:ss.xx]Text`, mehrere Timestamps pro Zeile erlaubt (`[00:10.00][01:20.00]Refrain`), Metadaten-Tags (`[offset:+300]`, `[ti:]`, `[ar:]`), leere Zeilen als Instrumentalmarker behalten.
- Enhanced LRC: `<mm:ss.xx>` Wortmarken innerhalb der Zeile.
- Zeilen nach Zeit sortieren, Duplikate entfernen, `end_ms` aus Nachfolger ableiten.
- Toleranz gegen Windows-Zeilenenden, BOM, Timestamps mit 3 Nachkommastellen.
- Unit-Tests mit mindestens 10 realen LRC-Beispielen (inkl. kaputten).

### 7.3 Cache (SQLite)

Tabellen: `tracks(key, title, artist, album, duration_ms, last_seen)`, `lyrics(key, source, format, raw, synced, instrumental, fetched_at)`, `negative_cache(key, until)`, `song_overrides(key, offset_ms, user_lrc)`, `device_offsets(device_id, name, offset_ms)`. Pfad: `%APPDATA%/LyricHub/lyrichub.db`.

### 7.4 Rechtliches (Hinweis für die README, keine Blockade für Eigennutzung)

Lyrics sind urheberrechtlich geschützt. LRCLIB ist community-gepflegt; die App zeigt Drittinhalte an. Für die Eigennutzung unkritisch. Bei Veröffentlichung: Disclaimer, keine Weiterverteilung von Lyrics, LRCLIB als Quelle nennen. Keine Nutzung inoffizieller Musixmatch-/Spotify-Endpoints.

---

## 8. Spotify Web API (optionaler Layer, Phase 3b)

Nur bauen, wenn Spike 0 oder der Nutzer es verlangt. Feature-Flag `spotify-api` in Cargo.

- **Voraussetzungen (Stand Feb/Mär 2026)**: Der Nutzer legt im Spotify Developer Dashboard eine eigene App an (Development Mode). Der Besitzer braucht **Spotify Premium** (beim Nutzer vorhanden); max. 5 autorisierte Nutzer; Client-ID wird in den Settings eingetragen (öffentlich, PKCE braucht kein Secret). Vor der Umsetzung den offiziellen **Migration Guide "February 2026 Web API Dev Mode Changes"** lesen und prüfen, ob `GET /me/player` im Dev Mode weiterhin verfügbar ist.
- **Auth**: Authorization Code mit PKCE. Redirect-URI `http://127.0.0.1:<port>/callback` (Spotify akzeptiert für Loopback nur die IP-Literal-Form, nicht `localhost`). Lokaler Mini-HTTP-Listener in Rust (`tiny_http`), Port aus einem festen Bereich; Browser öffnen per `tauri-plugin-opener`.
- **Scopes**: `user-read-playback-state` (deckt currently-playing ab).
- **Token-Speicher**: Windows Credential Manager über `keyring`-Crate. Refresh-Token rotiert bei PKCE; immer den neuen speichern.
- **Polling**: siehe 5.4. HTTP 429 → `Retry-After` beachten, Backoff, Sync-Lab zeigt "rate limited".
- **Mehrwert**: stabile Track-ID als Cache-Key, Podcast-Erkennung, Cover-URLs in mehreren Größen, zweite unabhängige Positionsquelle. Ad-Erkennung ist für den Nutzer irrelevant, weil Premium keine Werbung spielt.
- **Veröffentlichung**: Wegen des 5-Nutzer-Limits trägt die Web API keine öffentliche Version. Bei Weitergabe müsste jeder Nutzer eine eigene Client-ID eintragen. Sie bleibt deshalb ein persönliches Zusatzfeature.

---

## 9. Design

### 9.1 Designprinzipien

1. **Lesbar über jedem Spiel**: Weißer Text mit dunkler Kontur oder auf halbtransparenter Platte. Niemals reine Farbe ohne Kontrastschutz.
2. **Ruhe**: Das Overlay ist Beifahrer, nicht Hauptdarsteller. Keine Dauerbewegung außer der Füllung der aktiven Zeile. Alles andere passiert in ≤ 250 ms und bleibt dann stehen.
3. **Nie im Weg**: Standard oben mittig, schmal; verschwindet bei Pause; nimmt nie Fokus; click-through.
4. **Zeit ist Wahrheit**: Die Füllung folgt streng der Position (linear, ohne Easing). Easing gibt es nur für Zeilenwechsel, nie für den Fortschritt.
5. **Einstellbar, aber mit guten Defaults**: Ein Nutzer, der nichts konfiguriert, bekommt ein fertiges Ergebnis.

### 9.2 Layout-Modi

| Modus | Inhalt | Zielgruppe |
|---|---|---|
| **Karaoke** (Standard) | Aktuelle Zeile groß mit Füllung, nächste Zeile darunter klein und gedimmt, Mini-Header (Cover 24 px, Titel – Artist) | Mitsingen |
| **Bar** | Eine Zeile: Cover · Titel – Artist · \| · aktuelle Lyric-Zeile (Ellipsis statt Marquee) | minimaler Platz, oben am Rand |
| **Stack** | 3–5 Zeilen: vorherige (gedimmt), aktuelle (Füllung), kommende (gedimmt), weiches Scrollen | Lesen, Übersicht |
| **Minimal** | Nur die aktive Zeile, ohne Platte, mit Kontur + Schatten | Clean, Streaming |

### 9.3 Karaoke-Highlight (Technik)

- Textzeile als zwei übereinander liegende Ebenen: Basisfarbe (gedimmt, z. B. `rgba(255,255,255,0.55)`) und darüber die Akzentfarbe mit `background-clip: text` und `linear-gradient(90deg, accent 0 var(--p), transparent var(--p))`, wobei `--p` der Fortschritt in Prozent ist. Weicher Übergangsrand (2–3 % breit) statt harter Kante.
- Bei echten Wort-Timings: zusätzlich `span` pro Wort mit `text-shadow`-Glow, der 120 ms ein- und 300 ms ausblendet.
- Font-Rendering: `text-rendering: optimizeLegibility`, Variable Font mit `font-variation-settings` für Gewichtswechsel ohne Layout-Sprung.
- Kontur ohne Platte: `paint-order: stroke fill` + `-webkit-text-stroke` (2 px bei scale 1) plus `text-shadow 0 2px 8px rgba(0,0,0,.7)`.

### 9.4 Motion-Spezifikation

| Ereignis | Dauer | Easing | Beschreibung |
|---|---|---|---|
| Zeilenwechsel | 220 ms | cubic-bezier(.2,.8,.2,1) | Aktive Zeile fadet 0,55 → 1, nächste rückt 12 px hoch |
| Füllung | kontinuierlich | linear | folgt `display_pos` |
| Wort-Glow (nur echte Wortzeiten) | 120 ms ein / 300 ms aus | ease-out | `text-shadow`-Intensität |
| Track-Wechsel | 300 ms | ease-in-out | Crossfade des gesamten Overlays, Cover tauscht mit kleinem Scale 0,96 → 1 |
| Pause | nach 2 s, 400 ms | ease | Opacity auf 0,6; nach `hide_after_pause_s` (Standard 10) auf 0 |
| Nudge-Toast | 1,5 s sichtbar | – | kleine Pille unter dem Overlay: "Song-Offset +100 ms" |
| Reduced Motion (Option) | – | – | alle Transitionen ≤ 80 ms, kein Glow |

### 9.5 Themes, Typografie, Farben

- **Schrift**: Inter Variable (gebündelt), Fallback Segoe UI Variable; optional Nutzer-Font aus Systemliste. Japanisch ist die dritthäufigste Lyrics-Sprache des Nutzers, deshalb ist die CJK-Darstellung v1-Pflicht. Fallback-Kette: `"Inter Variable", "Segoe UI Variable", "Yu Gothic UI", "Meiryo UI", "Noto Sans JP", sans-serif`. Yu Gothic UI und Meiryo UI beim Start auf Verfügbarkeit prüfen; fehlen sie, ein Subset von Noto Sans JP (OFL-Lizenz) bündeln. Für japanische Zeilen `line-break: strict` setzen. Grundgröße 28 px bei scale 1 auf 1080p; aktive Zeile Gewicht 650, nächste 500; Zeilenhöhe 1,25.
- **Themes**:
  - *Glass Dark* (Standard): Platte `rgba(12,12,16,.55)`, Radius 14 px, Text weiß, Akzent aus Albumcover.
  - *Glass Light*: Platte `rgba(255,255,255,.7)`, Text `#111`.
  - *Neon*: keine Platte, Kontur, Akzent-Glow kräftig; für dunkle Spiele.
  - *Mono*: Hochkontrast, Schwarz/Weiß, kein Akzent; Barrierefreiheit.
  - *Custom*: Platte, Textfarbe, Akzent, Kontur, Radius frei.
- **Akzentfarbe aus dem Cover**: Cover als Bild in ein Canvas, auf 32×32 verkleinern, einfache Quantisierung (Median Cut oder k-means mit k=4), Kandidat mit höchster Sättigung wählen, Luminanz auf ≥ 0,6 anheben, Kontrast gegen Platte ≥ 4,5:1 sicherstellen; sonst neutraler Fallback-Akzent. Spotifys Markengrün nicht exakt übernehmen (Branding-Richtlinien).
- **Opazität/Größe/Breite** als Slider, Live-Vorschau im Overlay.

### 9.6 Zustände im Overlay

- *Laden*: Titel sichtbar, Lyric-Bereich zeigt drei pulsierende Punkte (max. 1,5 s).
- *Keine synchronen Lyrics*: Titel + kleine Zeile "Keine synchronen Lyrics gefunden" (verschwindet nach 4 s, Titel bleibt).
- *Instrumental*: "Instrumental" 4 s, dann nur Titel.
- *Werbung*: Overlay aus (Option: "Werbung" anzeigen).
- *Fehler (Netz)*: dezentes Symbol im Header, Details im Settings-Fenster; nie Fehlertexte groß im Spiel.
- *Edit-Modus*: gestrichelter Rahmen, 9 Snap-Punkte am Monitor als kleine Markierungen, Griff zum Ziehen, Eck-Griffe für Skalierung, Beispieltext "Dies ist eine Beispielzeile zum Ausrichten", Toast "Enter = speichern, Esc = abbrechen, Ziehen = verschieben".

### 9.7 Settings-Fenster

Seitennavigation links, Inhalt rechts, 960×640, Dark Theme passend zum Overlay.

1. **Verbindung**: Status Spotify (SMTC gefunden ja/nein, Client-Version Desktop/Store), optional Web-API-Login (Client-ID-Feld, Login-Button, Hinweis auf Premium-Pflicht und 5-Nutzer-Limit).
2. **Overlay**: Monitor, Anker, Feinposition, Breite, Skalierung, Layout, "Edit-Modus starten", "Nur bei Vollbild-App zeigen", "Bei Pause ausblenden nach … s".
3. **Darstellung**: Theme, Schrift, Akzent (Cover-Auto / fest), Platte an/aus, Kontur, Opazität, Reduced Motion, Live-Vorschau.
4. **Synchronisation**: globaler Offset, Geräte-Offsets (Tabelle: Gerät, Offset, "Kalibrieren"), Song-Offset des aktuellen Titels, Nudge-Ziel (Song/global), Kalibrier-Assistent, Hinweis zu Crossfade.
5. **Lyrics**: Provider-Reihenfolge, Cache-Größe/leeren, LRC importieren für aktuellen Song, Negative-Cache zurücksetzen, "Lyrics neu laden".
6. **Hotkeys**: Overlay ein/aus (`Ctrl+Alt+L`), Edit-Modus (`Ctrl+Alt+E`), Nudge −/+ (`Ctrl+Alt+,` / `Ctrl+Alt+.`), Layout wechseln (`Ctrl+Alt+K`), alle deaktivierbar (Spiele-Konflikte).
7. **Verhalten**: Autostart, minimiert starten, Sprache, Updates prüfen (optional), Logs.
8. **Diagnose**: Sync Lab (5.9), SMTC-Rohdaten, API-Status, Fullscreen-Status, Logs öffnen, Diagnose exportieren.

### 9.8 Tray

Linksklick: Overlay ein/aus. Rechtsklick-Menü: Overlay anzeigen (Checkbox), Edit-Modus, Nudge −100/+100, Layout-Untermenü, Einstellungen, Beenden. Tooltip: "LyricHub – <Titel> – <Artist>" bzw. Statusmeldungen.

---

## 10. Umsetzungsphasen

Jede Phase endet mit einem lauffähigen Stand. Reihenfolge einhalten: erst messen (Phase 0), dann dünner Durchstich (Phase 1), dann Sync (2–3), dann Politur (4), dann Robustheit (5).

### Phase 0 – Spike und Gerüst

Tasks:
- `git init`, Tauri-2-Projekt mit zwei Fenstern (`main`, `overlay`) und Tray anlegen; Plugins: global-shortcut, autostart, single-instance, opener, store (oder eigener JSON-Store).
- **Spike A (SMTC-Messung)** nach 5.8 durchführen, `docs/spike-smtc.md` schreiben.
- **Spike B (Overlay)**: transparentes, click-through, topmost, no-activate Fenster mit statischem Text testen, und zwar über einem Borderless-Spiel, einem Spiel im exklusiven Vollbild (siehe 6.3) und einem Vollbild-Video; Alt+Tab-Verhalten prüfen; Ex-Style-Reapply implementieren.
- Logging (tracing → Datei) einrichten.

Definition of Done: Beide Spike-Berichte liegen vor (`docs/spike-smtc.md`, `docs/spike-overlay.md`). Die Entscheidung "SMTC reicht / Plan B nötig" ist dokumentiert. Die Entscheidung "Tauri bleibt / WPF" trifft Opus selbst und begründet sie im Bericht (Nutzerentscheidung, 14.1). Ist Plan B nötig, wird das Ergebnis dem Nutzer vorgelegt, bevor Phase 1 beginnt.

### Phase 1 – Dünner Durchstich "Now Playing"

Tasks:
- `sources/smtc.rs`: Session-Discovery (Spotify-Filter), Events für Metadaten, Status, Timeline; Cover-Thumbnail als Bytes → Base64/Temp-Datei fürs Frontend.
- `TrackInfo`-Event, Overlay zeigt Cover, Titel, Artist (Layout Bar).
- Settings-Schema v1 + Store; Settings-Fenster mit Seiten Overlay und Verhalten.
- Overlay-Geometrie (Monitor, Anker, Offsets, Skalierung) + Edit-Modus.
- Tray mit Grundfunktionen; Hotkey Overlay ein/aus.

DoD: Spotify-Titelwechsel erscheint ≤ 300 ms im Overlay über einem Spiel; Position/Größe lassen sich per Edit-Modus setzen und überleben Neustart.

### Phase 2 – Lyrics und Clock

Tasks:
- `lyrics/lrclib.rs`, `matcher.rs`, `parser.rs`, `cache.rs` mit Tests.
- `sync/clock.rs` nach 5.3 mit Tests gegen synthetische Beobachtungen (Jitter ±200 ms, Seek, Pause).
- Frontend `clock.ts` mit lokalem Anchor + Slew; `KaraokeLine` mit Gradient-Füllung (zeilengenau); Layout Karaoke.
- Globaler Offset + Nudge-Hotkeys + Toast.

DoD: Zehn Songs mit synchronen Lyrics laufen zeilengenau mit; Nudge korrigiert sichtbar; Parser-Tests grün.

### Phase 3 – Sync-Härtung

Tasks:
- Quellen-Fusion (5.4), Residual-Filter, Slew-Rate-Reset, Seek- und Pause-Verhalten.
- `audio_devices.rs` + Geräte-Offsets + Toast bei neuem Gerät.
- Song-Offset-Persistenz; Nudge-Ziel umschaltbar.
- Kalibrier-Assistent.
- Sync Lab (5.9) inkl. Diagnose-Export.
- Sonderzustände (5.5): Werbung, Podcast, Instrumental, keine Lyrics, Spotify geschlossen.

**Phase 3b (optional, nach Spike-0-Entscheidung)**: Web-API-Layer nach Abschnitt 8.

DoD: Akzeptanzkriterien Sync (Abschnitt 2) im manuellen Protokoll nachgewiesen und in `docs/sync-report.md` dokumentiert.

### Phase 4 – Karaoke-Feinschliff und Design

Tasks:
- Wortgrenzen-Schätzung (5.6) + Enhanced-LRC-Wortzeiten; Wort-Glow.
- Alle vier Layouts; Themes; Cover-Akzentfarbe mit Kontrastsicherung; Reduced Motion.
- Motion nach 9.4; Zustände nach 9.6; Settings-Seite Darstellung mit Live-Vorschau.
- i18n (de/en).

DoD: Design-Review gegen 9.1–9.6; kein Layout pro Frame (DevTools Performance-Check); FPS-Test im Spiel ohne messbare Einbuße.

### Phase 5 – Robustheit und Release

Tasks:
- Fullscreen-Erkennung, Topmost-Reassert, "nur bei Vollbild-App".
- `WM_DISPLAYCHANGE`, Monitor-Fallback, DPI-Tests mit 100/125/150 %.
- Autostart minimiert, Single-Instance, Schlaf/Aufwachen (SMTC-Session neu aufbauen).
- Fehler-UX, Logs, Diagnose-Export.
- Installer (NSIS über Tauri-Bundler), README mit Screenshots und Setup (inkl. optionaler Spotify-App-Anleitung), Versionierung.

DoD: Alle Akzeptanzkriterien aus Abschnitt 2 erfüllt; Installer auf frischem Windows-Profil getestet.

### Phase 6 – Erweiterungen (nach Bedarf, siehe Abschnitt 13)

---

## 11. Teststrategie

### 11.1 Unit-Tests (Rust)
- `parser.rs`: 10+ reale LRC-Dateien, Enhanced LRC, kaputte Zeilen, `[offset:]`.
- `matcher.rs`: Normalisierung, Scoring, Feat-/Remaster-Varianten.
- `clock.rs`: synthetische Beobachtungsreihen (konstanter Offset, Jitter ±200 ms, Drift, Seek, Pause/Resume, Quellenkonflikt) → Modellfehler bleibt unter 50 ms nach 3 Samples; keine Rückwärtssprünge < 300 ms.
- `estimator.rs`: Wortgrenzen monoton, innerhalb der Zeile, Lead/Tail korrekt.

### 11.2 Integrationstests
- Mock-`PlaybackSource`, das Events aus einer Fixture-Datei abspielt (Track-Wechsel, Seeks) → komplette Pipeline bis `clock`-Events ohne Spotify.
- LRCLIB-Client gegen aufgezeichnete HTTP-Antworten (keine Live-Calls in CI).

### 11.3 Manuelles Sync-Protokoll (Pflicht vor Abschluss Phase 3 und 5)
- 10 Songs unterschiedlicher Genres (schnelle Rap-Zeilen, Balladen, Songs mit langem Intro, ein Live-Mitschnitt; Sprachmix wie beim Nutzer: überwiegend Englisch, mindestens zwei deutsche und zwei japanische Titel).
- Für jede Zeile der ersten 60 s notieren: Overlay zu früh / passend / zu spät, geschätzt in 100-ms-Schritten. Median und p95 berechnen. Mit der kabelgebundenen Ausgabe des Nutzers; Bluetooth nur, falls ein Gerät vorhanden ist.
- Seek-Test: 5 Sprünge pro Song, Zeit bis Overlay korrekt (Stoppuhr oder Sync-Lab-Zeitstempel).
- Fokus-Test: In einem Spiel (oder Notepad im Vordergrund) tippen, während Track-Wechsel, Edit-Modus-Ende und Hotkeys passieren; kein Zeichen darf verloren gehen.

---

## 12. Risiken und Gegenmaßnahmen

| Risiko | Wahrscheinlichkeit | Auswirkung | Gegenmaßnahme |
|---|---|---|---|
| SMTC-Position von Spotify zu grob für ±100 ms | mittel | hoch | Spike 0 zuerst; Plan B Desktop-Spotify + Spicetify-Companion (13.1), Plan C Web API (3b) |
| Spicetify läuft nicht mit Store-Spotify | sicher | mittel | Wechsel auf Desktop-Spotify nur, wenn Plan B nötig wird (Nutzerentscheidung) |
| Spotify schränkt Web API weiter ein / Premium läuft aus | hoch | mittel | App funktioniert ohne API; API ist Feature-Flag |
| Overlay unsichtbar in Exclusive Fullscreen | hoch (Nutzer spielt gemischt) | mittel | Erkennung + Hinweis im Tray (6.3), Test in Spike B; keine Injection |
| Anti-Cheat stuft Overlay als verdächtig ein | niedrig | hoch | Nur normales Topmost-Fenster, keine Hooks, kein Lesen fremder Prozesse |
| Falsche Lyrics (anderer Song/Version) | mittel | mittel | Dauer-Scoring, "Lyrics falsch? → andere Version wählen"-Dialog mit Kandidatenliste (Phase 4) |
| LRCLIB-Ausfall | niedrig | mittel | Cache, Backoff, Overlay degradiert auf Titel |
| Tauri-Transparenz/Click-through-Bugs auf bestimmten GPUs/Treibern | niedrig–mittel | hoch | Spike B früh; Fallback WPF entscheiden, bevor Phase 2 beginnt |
| Hotkey-Konflikte mit Spielen | mittel | niedrig | alle Hotkeys deaktivierbar, Tray als Alternative |
| Urheberrecht bei Veröffentlichung | – | mittel | Disclaimer, keine Weiterverteilung, nur Anzeige |

---

## 13. Brainstorm: Weitergehende Ideen (v2+)

Nach Nutzen und Aufwand sortiert. Nichts davon ist für v1 nötig.

### 13.1 Exakte Position per Spicetify-Companion (hoher Nutzen, mittlerer Aufwand)
Eine kleine Spicetify-Extension im Spotify-Client sendet `Spicetify.Player.getProgress()` mit 10 Hz per WebSocket an `ws://127.0.0.1:<port>`. Genauigkeit ±20 ms, inklusive Seek-Events in Echtzeit. Nachteil: Nutzer muss Spicetify installieren; bricht gelegentlich bei Spotify-Updates. Als "Power-User-Modus" anbieten; LyricHub erkennt den Companion automatisch als dritte Quelle mit Confidence 0,95. **Voraussetzung**: Desktop-Version von Spotify, denn die Store-Version wird von Spicetify nicht zuverlässig unterstützt. Für diesen Nutzer ist das Plan B (3.2) und wird bei Bedarf aus Phase 6 in Phase 3 vorgezogen.

### 13.2 Audio-Loopback als Sync-Signal (mittlerer Nutzen, mittlerer Aufwand)
Windows 10 2004+ erlaubt **prozessbezogenes Loopback** (`ActivateAudioInterfaceAsync` mit `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` auf die Spotify-PID). Damit lässt sich ohne Referenzaudio bereits einiges tun:
- Trackstart-Anker: Übergang Stille → Signal nach einem Track-Wechsel markiert Position ≈ 0 (mit Lead-Silence-Korrektur aus den ersten Samples). Bei Crossfade unbrauchbar, sonst sehr präzise.
- Pause-Erkennung instantan.
- **Beat-reaktiver Glow**: RMS-Hüllkurve steuert die Glow-Intensität der aktiven Zeile (billig, sieht gut aus, braucht keine Analyse-API).
Nicht möglich: Fingerprint-Alignment, da kein Referenz-Audio des Tracks vorliegt.

### 13.3 Echte Wortzeiten per Forced Alignment (hoher Nutzen, hoher Aufwand)
Mit Loopback-Audio (13.2) und dem bekannten Lyrics-Text lässt sich per Forced Alignment (z. B. WhisperX, wav2vec2-CTC) eine wortgenaue Zeitleiste berechnen, pro Song einmal, gecached. Braucht ein lokales Modell und idealerweise GPU; Latenz nicht in Echtzeit, sondern beim zweiten Hören verfügbar. Als optionales Modul mit eigener Binary (Python-Sidecar oder ONNX in Rust).

### 13.4 Player-agnostisch durch SMTC (hoher Nutzen, geringer Aufwand)
Da SMTC generisch ist, funktioniert die Pipeline mit minimalen Anpassungen auch für YouTube Music, Apple Music, Tidal, Browser-Player. Nur den Session-Filter konfigurierbar machen ("Bevorzugter Player"). Spotify bleibt Standard.

### 13.5 Streaming-Ausgabe (mittlerer Nutzen, geringer Aufwand)
Lokaler HTTP-Server liefert dieselbe Overlay-Seite als OBS-Browser-Source (mit Chroma-Key-Modus oder transparentem Hintergrund). Streamer bekommen Lyrics im Stream, ohne dass das Spiel-Overlay gecaptured werden muss.

### 13.6 Übersetzung und Romanisierung (mittlerer Nutzen, mittlerer Aufwand)
Zweite Zeile unter der aktiven Zeile: Übersetzung (DeepL/LibreTranslate-API, gecached) oder Romanisierung für Japanisch/Koreanisch (kuroshiro bzw. Hangul-Romanizer). Für K-Pop/J-Pop-Hörer sehr wertvoll. Für diesen Nutzer (Japanisch auf Platz 3) ist Romanisierung der erste Kandidat für Phase 6.

### 13.7 Lyrics-Editor und Beitrag an LRCLIB
Mini-Editor: Zeilen per Tastendruck im Takt neu stempeln, einzelne Zeilen nudgen, als LRC speichern; optional Upload an LRCLIB (deren Publish-API mit Proof-of-Work-Challenge). Macht die Community-Datenbank für alle besser.

### 13.8 Kleinere Ideen
- "Lyrics falsch?"-Dialog mit Kandidatenliste aus LRCLIB-Suche und Vorschau.
- Zweites Overlay als "Now Playing"-Karte auf einem Zweitmonitor.
- Cover-Hintergrund als weich geblurrter Verlauf hinter der Platte (innerhalb des WebViews, also machbar).
- Spielerkennung per Prozessliste: Overlay-Profile pro Spiel (Position/Theme), z. B. oben links in Shooter A, unten mittig in Strategie B.
- Spotify-Connect-Receiver-Modus über librespot (App ist selbst der Player, kennt die Position exakt). Premium-Pflicht, ToS-Grauzone, Audio läuft dann durch LyricHub. Nur als Experiment.

---

## 14. Entscheidungen des Nutzers (2026-10-25)

### 14.1 Antworten

| Frage | Antwort | Folge für den Plan |
|---|---|---|
| Spotify Premium? | Ja | Web API (Abschnitt 8) ist möglich, bleibt aber optional. Keine Werbung, also keine Ad-Erkennung nötig. |
| Spotify-Version? | Microsoft Store | Spike 0 misst mit der Store-Version (AUMID `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`). |
| Audioausgabe? | Kabel | Kalibrierung zielt auf kabelgebundene Ausgabe. Geräte-Offsets bleiben als Feature, sind aber nicht kritisch. |
| Zielgruppe? | Erst Eigennutzung, später eventuell kostenlos veröffentlichen | Siehe 14.2. |
| Lyrics-Sprachen? | Englisch, Deutsch, Japanisch (in dieser Reihenfolge), vereinzelt weitere | Japanische Schrift-Fallbacks und zeichenbasierte Wortschätzung sind v1-Pflicht (5.6, 9.5). Romanisierung (13.6) ist erster Kandidat für Phase 6. |
| Vollbild-Modus der Spiele? | Gemischt | Exklusives Vollbild ist ein Alltagsfall. Spike B testet beide Modi, die Erkennung samt Hinweis (6.3) ist v1-Pflicht. |
| Spicetify als Plan B? | Ja | Fallback-Reihenfolge bei zu ungenauer SMTC: erst Spicetify, dann Web API (3.2). |
| Store oder Desktop für Spicetify? | Wechsel auf Desktop-Spotify nur bei Bedarf | Kein Wechsel vorab. Nur wenn Spike 0 die Sync-Anforderung verfehlt, wechselt der Nutzer auf die Desktop-Version. |
| Framework bei Spike-Problemen? | Opus entscheidet | Tauri bleibt Standard. Ein Wechsel auf C#/WPF wird in `docs/spike-overlay.md` begründet, ohne Rückfrage. |
| Edit-Modus per Maus? | Nein, nur Hotkey und Tray | Das Overlay bleibt außerhalb des Edit-Modus immer klickdurchlässig. |

### 14.2 Veröffentlichungsfähig bleiben

Eine spätere kostenlose Veröffentlichung soll nicht verbaut werden. Deshalb gilt:

- Die Kernfunktion darf weder Spicetify noch die Web API voraussetzen. Beide bleiben optionale Zusatzquellen.
- Die Web API trägt eine Veröffentlichung wegen des 5-Nutzer-Limits im Development Mode nicht. Jeder Nutzer müsste eine eigene Client-ID eintragen.
- Keine persönlichen Pfade, Keys oder IDs im Code. Alles Nutzerspezifische liegt in den Settings.
- Lizenz-Disclaimer für Lyrics (7.4) und eine Open-Source-Lizenz für den Code früh festlegen. Abhängigkeiten nur mit kompatiblen Lizenzen (MIT, Apache-2.0, BSD; OFL für Fonts).
- Code-Signing, Auto-Update und Store-Veröffentlichung werden erst nach der Veröffentlichungsentscheidung nötig.

### 14.3 Noch offen

Keine blockierenden Fragen. Opus fragt nur nach, wenn Spike 0 oder Spike B ein Ergebnis liefert, das eine dieser Entscheidungen berührt, zum Beispiel den Wechsel auf Desktop-Spotify.

---

## 15. Arbeitsregeln für Opus

- **Spike zuerst, dann bauen.** Keine Sync-Implementierung vor dem SMTC-Messbericht.
- **Dünner Durchstich vor Politur.** Phase 1 muss komplett über einem Spiel laufen, bevor Lyrics kommen.
- **Sync-Logik bleibt UI-frei und getestet.** `sync/clock.rs` darf keine Tauri-Abhängigkeit haben.
- **Kein IPC pro Frame.** Das Frontend rechnet die Position selbst.
- **Nie den Fokus klauen.** Jede Fensteroperation wird dagegen getestet.
- **Keine DLL-Injection, keine Hooks in Spielprozesse, keine inoffiziellen Spotify-Endpoints, keine Cookies.**
- **Secrets**: Tokens nur im Credential Manager; Client-ID ist öffentlich, kein Client-Secret im Projekt.
- **Settings-Schema versionieren** (`schema_version`) und Migrationen schreiben, sobald sich Felder ändern.
- **Logs statt Vermutungen**: Jede Sync-Entscheidung (Seek-Reset, Korrektur, Quellenkonflikt) wird mit Werten geloggt. Das Sync Lab liest diese Daten.
- **Commits klein und thematisch**, Conventional Commits (`feat(sync): …`, `fix(overlay): …`). Jede Phase endet mit einem Tag (`v0.1.0-phase1` …).
- **Vor jedem Phasenabschluss** die DoD-Liste der Phase und die betroffenen Akzeptanzkriterien aus Abschnitt 2 abhaken und das Ergebnis in `docs/` festhalten.
- **Unklare Fakten prüfen**: Spotify-API-Regeln (Migration Guide Feb 2026), LRCLIB-Parameter, Tauri-Verhalten der eingesetzten Version. Dieser Plan nennt die Stände von Oktober 2026; Abweichungen dokumentieren, nicht stillschweigend umgehen.

---

## Anhang A: Quellen (Recherche vom 2026-10-25)

- Spotify: Quota Modes – https://developer.spotify.com/documentation/web-api/concepts/quota-modes
- Spotify Blog: Update on Developer Access and Platform Security (2026-02-06) – https://developer.spotify.com/blog/2026-02-06-update-on-developer-access-and-platform-security
- Spotify: February 2026 Web API Dev Mode Changes – Migration Guide – https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide
- Spotify Blog: Updating the Criteria for Web API Extended Access (2025-04-15) – https://developer.spotify.com/blog/2025-04-15-updating-the-criteria-for-web-api-extended-access
- TechCrunch zu den Dev-Mode-Änderungen – https://techcrunch.com/2026/02/06/spotify-changes-developer-mode-api-to-require-premium-accounts-limits-test-users/
- LRCLIB-Endpunkte (Drittdoku) – https://amll.dev/en/reference/http-api/lrclib.html
- LRCLIB-Repo (Issue zu /api/get und duration) – https://github.com/tranxuanthang/lrclib/issues/129
- Microsoft: GlobalSystemMediaTransportControlsSessionTimelineProperties – https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessiontimelineproperties
- Microsoft: Windows.Media.Control Namespace – https://learn.microsoft.com/en-us/uwp/api/windows.media.control
- Soggfy PR zu Spotify-Timeline-Refresh und Positions-Extrapolation – https://github.com/wefalltomorrow/Soggfy/pull/48
- WindowsMediaController (C#-Referenzimplementierung für SMTC) – https://github.com/DubyaDude/WindowsMediaController
- Tauri-2-Erfahrungsbericht zu Ex-Styles und set_ignore_cursor_events – https://www.producthunt.com/p/tauri-2/i-built-sticky-notes-that-hide-in-your-windows-screen-edge-with-tauri-2-6-lessons-learned
- Tauri Issue zu Hover bei ignorierten Cursor-Events – https://github.com/tauri-apps/tauri/issues/9250
