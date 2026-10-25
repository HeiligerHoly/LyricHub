# LyricHub

Windows-Overlay, das den laufenden Spotify-Song und karaoke-artig mitlaufende Lyrics über Spielen anzeigt, synchron zum hörbaren Ton. Der vollständige Plan steht in [PLAN.md](PLAN.md).

Stand: Phase 0 (Spikes und Gerüst). Ergebnisse in [docs/spike-smtc.md](docs/spike-smtc.md) und [docs/spike-overlay.md](docs/spike-overlay.md).

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

Die App startet im Tray. Linksklick auf das Tray-Icon blendet das Overlay ein und aus, das Rechtsklick-Menü öffnet die Einstellungen.

| Kürzel | Funktion |
|---|---|
| Strg+Alt+L | Overlay ein/aus |
| Strg+Alt+E | Klickdurchlässigkeit umschalten (Vorstufe des Edit-Modus) |

Logs liegen unter `%APPDATA%\LyricHub\logs`, das Level steuert `LYRICHUB_LOG` (z. B. `debug`).

## Spikes erneut ausführen

```powershell
# Spike A: Spotify-Timeline über die Windows-Media-Session messen
powershell -NoProfile -ExecutionPolicy Bypass -File tools\smtc-probe\smtc-probe.ps1 -DurationSec 360
python tools\smtc-probe\analyze.py docs\spike-smtc.csv

# Spike B: App bauen und Overlay-Selbsttest in drei Szenarien
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\run-spike.ps1
```

## Hinweise

Lyrics sind urheberrechtlich geschützt. LyricHub zeigt sie nur an und verteilt sie nicht weiter. Quelle der Lyrics wird ab Phase 2 LRCLIB sein.
