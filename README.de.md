# LyricHub

[English](README.md) · **Deutsch**

Synchrone Lyrics im Karaoke-Stil zum Song, der gerade in Spotify läuft, direkt über Spielen. Kostenlos für Windows, ohne Spotify-Login.

[![LyricHub-Promovideo: Lyrics über deinen Spielen](docs/img/promo-video.jpg)](https://youtu.be/lT9tKeel5nE)

**[Neueste Version herunterladen](https://github.com/HeiligerHoly/LyricHub/releases/latest)** · [Promovideo](https://youtu.be/lT9tKeel5nE) (5:47, englisch) · [FAQ](docs/faq.md) · [Auf Ko-fi unterstützen](https://ko-fi.com/heiliger)

> **Vibe-coded und source available.** KI hat Plan und Code geschrieben; ein Mensch hat jede Phase getestet und abgenommen (siehe den [Hinweis](#hinweis-vibe-coded)). Kostenlos nutzen und weitergeben, nicht verkaufen ([Lizenz](#lizenz)). Der vollständige Plan steht in [PLAN.md](PLAN.md) (englisch: [PLAN.en.md](PLAN.en.md)).

## Funktionen

- Synchrone Lyrics des laufenden Spotify-Songs von [LRCLIB](https://lrclib.net) mit einer Karaoke-Füllung, die Wort für Wort läuft (abschaltbar, weil die Wortzeiten meist geschätzt sind), dazu Cover, Titel und Artist.
- Position und Größe lassen sich im Edit-Modus direkt am Bildschirm einstellen.
- Vier Layouts und fünf Themes mit Akzentfarbe aus dem Albumcover.
- Bleibt auch über Spielen oben, die sich selbst nach vorne setzen, und lässt sich auf Vollbild-Apps beschränken.
- Korrekturen gelten pro Song, Ausgabegeräte haben eigene Offsets, und ein Kalibrier-Assistent und das Sync Lab helfen beim Einstellen.
- Japanische Zeilen lassen sich in Romaji anzeigen ([docs/romaji.md](docs/romaji.md)).
- Experimentell kann statt Spotify ein Browser der Player sein (YouTube Music, YouTube, Spotify-Webplayer).
- Optionales Plugin **Align**: echte Wortzeiten aus dem Ton des Players statt der Schätzung, gerechnet nach dem Song, solange kein Spiel im Vollbild läuft. Mit Auto-Sync rückt es auch jede Zeile an ihr erstes gesungenes Wort ([docs/plugins.md](docs/plugins.md)).
- Optionales Plugin **Recognize**: Lyrics für Songs, zu denen LRCLIB keine synchronen hat, gefunden mit breiterer Suche, ausgerichtet aus Text ohne Zeitstempel oder aus dem Gesang erkannt und als solche gekennzeichnet.
- Deutsche und englische Oberfläche. Sie folgt der Windows-Anzeigesprache und lässt sich unter Einstellungen → Verhalten → Sprache umstellen.

Aktuelle Version: `1.2.0` (2026-10-29), mit dem Plugin Align. Änderungen stehen in [CHANGELOG.md](CHANGELOG.md) (englisch). Das Promovideo ist mit Remotion im Code entstanden; das Projekt liegt in [promo-video/](promo-video/).

![Overlay im Layout Karaoke, Theme Glass Dark](docs/img/de/overlay-karaoke.png)

| | |
|---|---|
| ![Layout Stapel, Theme Glass Light](docs/img/de/overlay-stack-light.png) | ![Layout Minimal, Theme Neon](docs/img/de/overlay-minimal-neon.png) |
| ![Einstellungen → Darstellung](docs/img/de/settings-appearance.png) | ![Einstellungen → Hotkeys](docs/img/de/settings-hotkeys.png) |

Die Bilder zeigen Beispielzeilen; Liedtexte stehen nicht im Repository.

## Hinweis: vibe-coded

LyricHub ist mit KI entstanden („vibe coding“). Den Plan hat Claude Fable geschrieben, den Code Claude Opus 5.5 in Claude Code, Phase für Phase. Ein Mensch hat die Richtung vorgegeben, die Ergebnisse am eigenen PC getestet und jede Phase abgenommen, den Code aber nicht Zeile für Zeile geprüft. Es gibt keine Gewährleistung; Nutzung auf eigene Gefahr.

Wer den fertigen Dateien nicht traut, kann alles selbst nachvollziehen oder nachbauen:

- [docs/original-plan/](docs/original-plan/): der Plan, wie er vor der ersten Codezeile geschrieben wurde (Deutsch und Englisch). Mit ihm lässt sich LyricHub von Grund auf neu bauen, mit einem KI-Assistenten oder von Hand.
- [PLAN.md](PLAN.md) / [PLAN.en.md](PLAN.en.md): derselbe Plan mit allen späteren Entscheidungen und dem Ergebnis jeder Phase.
- [docs/](docs/) (englisch: [docs/en/](docs/en/)): Berichte jeder Phase mit Messwerten und Abweichungen vom Plan.
- Ergebnisse und Prüfungen: [Phase 5](docs/phase5.md), die Sicherheits-, Performance- und Speicherplatz-Prüfungen in [docs/checks.md](docs/checks.md), der Messbericht in [docs/sync-report.md](docs/sync-report.md), die Phasen [4](docs/phase4.md), [3](docs/phase3.md), [2](docs/phase2.md) und [1](docs/phase1.md) die Spikes aus Phase 0 zu [SMTC](docs/spike-smtc.md) und [Overlay](docs/spike-overlay.md) und der [Browser-Spike](docs/spike-browser.md) für v1.1.
- Der gesamte Code steht hier; der Installer lässt sich mit `npx tauri build` selbst bauen (siehe unten).

## Installieren

`LyricHub_<version>_x64-setup.exe` unter [Releases](https://github.com/HeiligerHoly/LyricHub/releases) herunterladen und ausführen (selbst bauen siehe unten: `npx tauri build`, Ergebnis unter `src-tauri/target/release/bundle/nsis/`). Der Installer richtet LyricHub nur für den aktuellen Benutzer ein und braucht keine Administratorrechte. Er ist nicht signiert: Windows SmartScreen meldet einen unbekannten Herausgeber, „Weitere Informationen“ → „Trotzdem ausführen“. Fehlt die WebView2 Runtime (auf Windows 11 vorhanden), lädt der Installer sie nach. Danach Spotify starten; LyricHub erscheint im Tray und zeigt den laufenden Song.

Die Plugins sind eigene Downloads, `lyrichub-align-<version>.zip` und `lyrichub-recognize-<version>.zip` im selben Release; installiert werden sie unter Einstellungen → Plugins ([docs/plugins.md](docs/plugins.md)).

## Voraussetzungen zum Bauen

- Windows 11 (getestet); Windows 10 ab Version 2004 ist nicht getestet
- Node.js 20 oder neuer
- Rust (stable, MSVC-Toolchain) über rustup
- Visual Studio Build Tools mit C++-Workload und Windows SDK
- WebView2 Runtime (auf Windows 11 vorinstalliert)
- Spotify-Desktop-App (Store- oder Desktop-Version); ein Spotify-Login in LyricHub ist nicht nötig. Experimentell seit 1.1: YouTube Music im Browser, normales YouTube und der Spotify-Webplayer in Edge

Build Tools installieren (fragt einmal per UAC nach Admin-Rechten):

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

## Entwickeln

```powershell
npm install
npm run tauri dev      # App mit Hot-Reload
npm run check          # Svelte/TypeScript prüfen
npx tauri build        # Release-Build und Installer (NSIS)
```

Veröffentlichte Releases entstehen mit `tools\build-release.ps1`: Es baut den Installer und die ZIPs der Plugins, ersetzt den Cargo-Ordner in den Quellpfaden, die Rust für Fehlermeldungen einbettet, und prüft, dass in den exe-Dateien kein Pfad mit einem lokalen Benutzerordner übrig ist.

Die App startet im Tray. Linksklick auf das Tray-Icon blendet das Overlay ein und aus, das Rechtsklick-Menü öffnet den Edit-Modus und die Einstellungen. Das Einstellungsfenster existiert nur, solange es offen ist. Mit `--minimized` (so startet der Autostart) öffnet sich kein Fenster.

Standard-Kürzel; alle acht globalen lassen sich unter Einstellungen → Hotkeys ändern oder abschalten:

| Kürzel | Funktion |
|---|---|
| Strg+Alt+L | Overlay ein/aus |
| Strg+Alt+E | Edit-Modus: Overlay ziehen, an den Ecken skalieren, an den Seiten die Breite ändern |
| Enter / Esc | im Edit-Modus speichern / abbrechen (fest, nur dann registriert) |
| Strg+Alt+K | Layout wechseln (Karaoke, Leiste, Stapel, Minimal) |
| Strg+Alt+R | Japanische Zeilen in Romaji (Hepburn) und zurück |
| Strg+Alt+, / Strg+Alt+. | Lyrics des laufenden Songs 100 ms früher / später; LyricHub merkt sich den Wert pro Song |
| Strg+Alt+Umschalt+, / Strg+Alt+Umschalt+. | dasselbe in 25-ms-Schritten |
| Leertaste / Esc | während Kalibrierung oder Sync-Protokoll: Tap beim hörbaren Zeilenbeginn / Messung beenden (fest, nur dann registriert) |

Das Spiel behält im Edit-Modus den Fokus; das Overlay nimmt nie Tastatureingaben an. Setzt sich ein Spiel über das Overlay, holt LyricHub es sofort zurück (Ereignisse der Fensterreihenfolge, keine Injection); gelingt das nicht, nennt der Tray das verdeckende Fenster. „Nur bei Vollbild-App zeigen“ (Einstellungen → Overlay) zeigt das Overlay nur, solange ein Spiel oder Video im Vollbild läuft (randlos oder exklusiv), und blendet es aus, sobald ein normales Fenster oder der Desktop vorne ist; Edit-Modus und Vorschau der Darstellung bleiben sichtbar. In einem getesteten Spiel war das Overlay auch im exklusiven Vollbild zu sehen; verdeckt ein Spiel es trotzdem, zeigt der Tray einen Hinweis, und der Modus „Randloses Fenster“ im Spiel hilft. Pausiert Spotify, wird das Overlay nach 2 s gedimmt und nach 10 s ausgeblendet („Bei Pause ausblenden nach“ unter Einstellungen → Overlay, 0 = nie).

Darstellung (Einstellungen → Darstellung): Themes Glass Dark, Glass Light, Neon, Mono und ein eigenes; Akzentfarbe aus dem Cover (mit Kontrastprüfung gegen die Platte) oder fest; Deckkraft, Schrift (Inter ist mitgeliefert) und „Bewegung reduzieren“. Solange die Seite offen ist, zeigt das Overlay Beispielzeilen im gewählten Look. Die Sprache (System, Deutsch, Englisch) steht unter Verhalten.

Offsets (Einstellungen → Synchronisation) addieren sich, positiv heißt Lyrics später: **Gerät** für die Ausgabelatenz (jedes Ausgabegerät hat einen eigenen Wert, Bluetooth startet mit +150 ms), **Song** für LRC-Dateien, die zu früh oder zu spät liegen (per Nudge), und **global** als Reserve. Der Kalibrier-Assistent schlägt aus fünf Taps einen Geräte-Offset vor. Das Sync-Protokoll misst über beliebig viele Songs und schreibt Median, p95 und Tendenz pro Song nach `logs\sync-protocol-<unix>.json`. Das Sync Lab unter Einstellungen → Diagnose zeigt die Positionsberichte der letzten 60 s und exportiert sie als JSON.

Einstellungen liegen in `%APPDATA%\LyricHub\settings.json`, Lyrics-Cache sowie Song- und Geräte-Offsets in `%APPDATA%\LyricHub\lyrichub.db`, Logs unter `%APPDATA%\LyricHub\logs`. Das Log-Level steuert `LYRICHUB_LOG` (z. B. `debug`). Die Logs enthalten keine Liedtexte.

## Tests und Spikes

```powershell
# Spike A: Spotify-Timeline über die Windows-Media-Session messen
powershell -NoProfile -ExecutionPolicy Bypass -File tools\smtc-probe\smtc-probe.ps1 -DurationSec 360
python tools\smtc-probe\analyze.py docs\spike-smtc.csv

# Alle Medien-Sitzungen auflisten, die Windows kennt, etwa um die Session-ID eines Browsers für -AppFilter zu finden
powershell -NoProfile -ExecutionPolicy Bypass -File tools\smtc-probe\list-sessions.ps1

# Spike B: App bauen und Overlay-Selbsttest in drei Szenarien
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\run-spike.ps1

# CPU und Speicher einer laufenden LyricHub-Instanz messen
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\measure-cpu.ps1 -Seconds 10

# Start ohne Fokusklau (Release-Build, LyricHub darf nicht laufen)
powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\start-focus-test.ps1

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

127 Rust-Unit-Tests (LRC-Parser, geschätzte Wortzeiten, Playback-Clock mit Quellen-Fusion, Offsets, Tap-Auswertung, Matcher, LRCLIB-Client gegen aufgezeichnete Antworten, Cache mit dem Nachprüfen älterer Lyrics, Geometrie, Settings-Store mit Migration, SMTC-Songwechsel-Erkennung, Browser-Titel, Werbung und Podcasts, Kürzel mit AltGr-Erkennung, Plugin-Manifeste mit Einstellungen, ZIP-Prüfung, Prozesse und ihre Speichergrenze, Wortzeiten, Lyrics und Auto-Sync von Plugins, Diagnose-Export ohne Songangaben, Absturzberichte): `cargo test` in `src-tauri`. Live-Anfragen an LRCLIB machen die Tests nicht. Sieben weitere laufen mit `cargo test -- --ignored`: einer braucht eine Desktop-Sitzung, einer misst die geschätzten Wortzeiten gegen den Datensatz JamendoLyrics ([docs/word-timing.md](docs/word-timing.md)), und fünf prüfen den Plugin-Host mit dem Beispiel-Plugin in `plugins/example`, das sie vorher bauen. Die CI auf GitHub Actions (`.github/workflows/ci.yml`) lässt bei jedem Push auf main die Tests von App und Plugins, `npm run check`, clippy, `cargo audit` und `npm audit` laufen; `tools\build-release.ps1` wiederholt sie vor jedem Release.

Das Align-Plugin in `plugins/align` hat 39 eigene Tests (`cargo test` dort): Resampler, Merkmale und CTC-Pfad gegen Referenzwerte aus torchaudio und transformers (`tests/make_golden.py`, synthetischer Ton), Text-Ziele, Fenster, der Rekorder, die Statuszeile, das gemeinsame Modell und die Rechensperre sowie die Erkennungs-Bausteine, die `plugins/recognize` nutzt (dort 2 weitere Tests für seine LRCLIB-Suche). `cargo run --release --example jamendo` vergleicht seine Pipeline mit dem P0-Prototyp auf JamendoLyrics-Songs, `--example memory` misst den Speicher des Modells je Fensterlänge ([docs/plugin-align.md](docs/plugin-align.md)). `--example jamendo` mit `--before` und `--jitter` prüft Auto-Sync ([docs/auto-sync.md](docs/auto-sync.md)), `--example recognize` die Erkennung und das Ausrichten von Text ohne Zeitstempel ([docs/plugin-recognize.md](docs/plugin-recognize.md)).

## Hinweise

Lyrics sind urheberrechtlich geschützt. LyricHub zeigt sie nur an und verteilt sie nicht weiter. Quelle der Lyrics ist [LRCLIB](https://lrclib.net), eine von der Community gepflegte Datenbank; LyricHub fragt sie mit eigenem User-Agent samt Link auf dieses Repository an und speichert Ergebnisse lokal, um sie nicht bei jeder Wiederholung erneut abzufragen. Die Test-Fixtures im Repository enthalten erfundenen Text.

LyricHub ist nicht mit Spotify AB verbunden und wird von Spotify weder unterstützt noch gesponsert. Spotify ist eine Marke von Spotify AB.

## Unterstützen

LyricHub ist kostenlos und bleibt es. Wer Danke sagen will, kann mir auf [Ko-fi](https://ko-fi.com/heiliger) einen Kaffee spendieren. Das ist ein Dankeschön, kein Kauf: Es gibt keine Extras und keine bezahlte Version. Fehler und Ideen bitte über die [Issue-Formulare](https://github.com/HeiligerHoly/LyricHub/issues/new/choose) melden; für Pull Requests siehe [CONTRIBUTING.md](CONTRIBUTING.md) (englisch).

Die Lyrics kommen von [LRCLIB](https://lrclib.net), einem kostenlosen Community-Dienst; der verdient auch Unterstützung.

## Lizenz

LyricHub ist „source available“ (der Quellcode ist öffentlich) unter der MIT-Lizenz mit der Commons Clause ([LICENSE](LICENSE)). Jeder darf es nutzen, verändern und weitergeben, auch beim Streamen. Verkaufen ist nicht erlaubt, auch nicht als bezahlter Dienst, dessen Wert ganz oder im Wesentlichen aus LyricHub kommt.

Fremde Komponenten behalten ihre eigenen Lizenzen. Sie stehen in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md), die der Installer zusammen mit der Lizenz neben `lyrichub.exe` legt.
