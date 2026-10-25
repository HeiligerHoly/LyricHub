# LyricHub

Windows-Overlay, das den laufenden Spotify-Song und karaoke-artig mitlaufende Lyrics über Spielen anzeigt, synchron zum hörbaren Ton. Der vollständige Plan steht in [PLAN.md](PLAN.md).

Stand: Phase 2 abgeschlossen (`v0.2.0-phase2`), Lyrics und Clock. Das Overlay zeigt die synchronen Lyrics des laufenden Spotify-Songs von [LRCLIB](https://lrclib.net) mit zeilengenauer Karaoke-Füllung, dazu Cover, Titel und Artist; Position und Größe lassen sich im Edit-Modus direkt am Bildschirm einstellen. Ergebnisse in [docs/phase2.md](docs/phase2.md) und [docs/phase1.md](docs/phase1.md), die Spikes aus Phase 0 in [docs/spike-smtc.md](docs/spike-smtc.md) und [docs/spike-overlay.md](docs/spike-overlay.md).

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
| Strg+Alt+K | Layout wechseln (Karaoke, Leiste) |
| Strg+Alt+, / Strg+Alt+. | Lyrics 100 ms früher / später (mit Umschalt 25 ms) |

Das Spiel behält im Edit-Modus den Fokus; das Overlay nimmt nie Tastatureingaben an.

Einstellungen liegen in `%APPDATA%\LyricHub\settings.json`, der Lyrics-Cache in `%APPDATA%\LyricHub\lyrichub.db`, Logs unter `%APPDATA%\LyricHub\logs`. Das Log-Level steuert `LYRICHUB_LOG` (z. B. `debug`). Die Logs enthalten keine Liedtexte.

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
```

Rust-Unit-Tests (LRC-Parser, Playback-Clock, Matcher, LRCLIB-Client gegen aufgezeichnete Antworten, Cache, Geometrie, Settings-Store, SMTC-Songwechsel-Erkennung): `cargo test` in `src-tauri`. Live-Anfragen an LRCLIB machen die Tests nicht.

## Hinweise

Lyrics sind urheberrechtlich geschützt. LyricHub zeigt sie nur an und verteilt sie nicht weiter. Quelle der Lyrics ist [LRCLIB](https://lrclib.net), eine von der Community gepflegte Datenbank; LyricHub fragt sie mit eigenem User-Agent an und speichert Ergebnisse lokal, um sie nicht bei jeder Wiederholung erneut abzufragen. Die Test-Fixtures im Repository enthalten erfundenen Text.
