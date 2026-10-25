# LyricHub

Windows-Overlay, das den laufenden Spotify-Song und karaoke-artig mitlaufende Lyrics über Spielen anzeigt, synchron zum hörbaren Ton. Der vollständige Plan steht in [PLAN.md](PLAN.md).

Stand: Phase 4 (`v0.4.0-phase4`), Karaoke-Feinschliff und Design. Das Overlay zeigt die synchronen Lyrics des laufenden Spotify-Songs von [LRCLIB](https://lrclib.net) mit einer Karaoke-Füllung, die Wort für Wort läuft, dazu Cover, Titel und Artist; Position und Größe lassen sich im Edit-Modus direkt am Bildschirm einstellen. Vier Layouts, fünf Themes mit Akzentfarbe aus dem Albumcover, Deutsch und Englisch. Korrekturen gelten pro Song, Ausgabegeräte haben eigene Offsets, und ein Kalibrier-Assistent und das Sync Lab helfen beim Einstellen. Japanische Zeilen lassen sich in Romaji anzeigen ([docs/romaji.md](docs/romaji.md)). Ergebnisse in [docs/phase4.md](docs/phase4.md), [docs/phase3.md](docs/phase3.md) mit dem Messbericht [docs/sync-report.md](docs/sync-report.md), dazu [docs/phase2.md](docs/phase2.md) und [docs/phase1.md](docs/phase1.md), die Spikes aus Phase 0 in [docs/spike-smtc.md](docs/spike-smtc.md) und [docs/spike-overlay.md](docs/spike-overlay.md).

## Voraussetzungen

- Windows 11 (Windows 10 ab 2004 sollte gehen)
- Node.js 20 oder neuer
- Rust (stable, MSVC-Toolchain) über rustup
- Visual Studio Build Tools mit C++-Workload und Windows SDK
- WebView2 Runtime (auf Windows 11 vorinstalliert)
- Spotify-Desktop-App (Store- oder Desktop-Version); ein Spotify-Login in LyricHub ist nicht nötig

Build Tools installieren (fragt einmal per UAC nach Admin-Rechten):

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

## Entwickeln

```powershell
npm install
npm run tauri dev      # App mit Hot-Reload
npm run check          # Svelte/TypeScript prüfen
```

Die App startet im Tray. Linksklick auf das Tray-Icon blendet das Overlay ein und aus, das Rechtsklick-Menü öffnet den Edit-Modus und die Einstellungen. Das Einstellungsfenster existiert nur, solange es offen ist. Mit `--minimized` (so startet der Autostart) öffnet sich kein Fenster.

| Kürzel | Funktion |
|---|---|
| Strg+Alt+L | Overlay ein/aus |
| Strg+Alt+E | Edit-Modus: Overlay ziehen, an den Ecken skalieren, an den Seiten die Breite ändern |
| Enter / Esc | im Edit-Modus speichern / abbrechen (nur dann registriert) |
| Strg+Alt+K | Layout wechseln (Karaoke, Leiste, Stapel, Minimal) |
| Strg+Alt+R | Japanische Zeilen in Romaji (Hepburn) und zurück |
| Strg+Alt+, / Strg+Alt+. | Lyrics des laufenden Songs 100 ms früher / später (mit Umschalt 25 ms); LyricHub merkt sich den Wert pro Song |
| Leertaste / Esc | während Kalibrierung oder Sync-Protokoll: Tap beim hörbaren Zeilenbeginn / Messung beenden (nur dann registriert) |

Das Spiel behält im Edit-Modus den Fokus; das Overlay nimmt nie Tastatureingaben an.

Darstellung (Einstellungen → Darstellung): Themes Glass Dark, Glass Light, Neon, Mono und ein eigenes; Akzentfarbe aus dem Cover (mit Kontrastprüfung gegen die Platte) oder fest; Deckkraft, Schrift (Inter ist mitgeliefert) und „Bewegung reduzieren“. Solange die Seite offen ist, zeigt das Overlay Beispielzeilen im gewählten Look. Die Sprache (System, Deutsch, Englisch) steht unter Verhalten.

Offsets (Einstellungen → Synchronisation) addieren sich, positiv heißt Lyrics später: **Gerät** für die Ausgabelatenz (jedes Ausgabegerät hat einen eigenen Wert, Bluetooth startet mit +150 ms), **Song** für LRC-Dateien, die zu früh oder zu spät liegen (per Nudge), und **global** als Reserve. Der Kalibrier-Assistent schlägt aus fünf Taps einen Geräte-Offset vor. Das Sync-Protokoll misst über beliebig viele Songs und schreibt Median, p95 und Tendenz pro Song nach `logs\sync-protocol-<unix>.json`. Das Sync Lab unter Einstellungen → Diagnose zeigt die Positionsberichte der letzten 60 s und exportiert sie als JSON.

Einstellungen liegen in `%APPDATA%\LyricHub\settings.json`, Lyrics-Cache sowie Song- und Geräte-Offsets in `%APPDATA%\LyricHub\lyrichub.db`, Logs unter `%APPDATA%\LyricHub\logs`. Das Log-Level steuert `LYRICHUB_LOG` (z. B. `debug`). Die Logs enthalten keine Liedtexte.

## Tests und Spikes

```powershell
# Spike A: Spotify-Timeline über die Windows-Media-Session messen
powershell -NoProfile -ExecutionPolicy Bypass -File tools\smtc-probe\smtc-probe.ps1 -DurationSec 360
python tools\smtc-probe\analyze.py docs\spike-smtc.csv

# Spike B: App bauen und Overlay-Selbsttest in drei Szenarien
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\run-spike.ps1

# CPU und Speicher einer laufenden LyricHub-Instanz messen
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\measure-cpu.ps1 -Seconds 10

# Edit-Modus über dem Fake-Game prüfen (laufende Instanz nötig, bewegt die Maus einige Sekunden)
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\edit-mode-test.ps1

# Layouts der Overlay-Seite während der Wiedergabe zählen ("kein Layout pro Frame"); LyricHub dafür mit
# Debug-Port starten, in Git Bash: WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223 src-tauri/target/debug/lyrichub.exe --minimized
node tools\overlay-spike\layout-count.mjs 30

# Seek-Latenz bis zum gezeichneten Overlay (laufende Instanz und Spotify nötig, spult im aktuellen Song)
powershell -NoProfile -ExecutionPolicy Bypass -File tools\sync\seek-test.ps1 -Count 5

# Nudge pro Song über einen echten Songwechsel (springt einen Song vor und zurück)
powershell -NoProfile -ExecutionPolicy Bypass -File tools\sync\nudge-test.ps1
```

Rust-Unit-Tests (LRC-Parser, geschätzte Wortzeiten, Playback-Clock mit Quellen-Fusion, Offsets, Tap-Auswertung, Matcher, LRCLIB-Client gegen aufgezeichnete Antworten, Cache, Geometrie, Settings-Store mit Migration, SMTC-Songwechsel-Erkennung, Werbung und Podcasts): `cargo test` in `src-tauri`. Live-Anfragen an LRCLIB machen die Tests nicht.

## Hinweise

Lyrics sind urheberrechtlich geschützt. LyricHub zeigt sie nur an und verteilt sie nicht weiter. Quelle der Lyrics ist [LRCLIB](https://lrclib.net), eine von der Community gepflegte Datenbank; LyricHub fragt sie mit eigenem User-Agent an und speichert Ergebnisse lokal, um sie nicht bei jeder Wiederholung erneut abzufragen. Die Test-Fixtures im Repository enthalten erfundenen Text.
