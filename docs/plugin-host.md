# Plugin-Host (P1 und P2 aus PLAN-plugins.md)

> Datum: 2026-10-28 · nach v1.1.0 · Bezug: [PLAN-plugins.md](../PLAN-plugins.md) Abschnitte 4 und 6
> Ergebnis: **LyricHub installiert, startet, überwacht und beendet Plugins (P1) und übernimmt Wortzeiten von ihnen (P2). Die Abnahmekriterien beider Phasen sind in der echten App geprüft.** Das erste echte Plugin folgt in P3.

## Was gebaut ist

| Teil | Umsetzung | Code |
|---|---|---|
| Manifest | `plugin.json` mit Kennung, Name, Version, Protokollversion, Programm, Fähigkeiten und Rechten. Unsichere oder unbekannte Felder werden abgelehnt, eine andere Protokollversion gilt als „inkompatibel“. | `plugins/manifest.rs` |
| Installation | ZIP wird vollständig geprüft, bevor etwas geschrieben wird: Manifest, Programm, keine Pfade außerhalb des Ordners, keine absoluten Pfade, keine Links, Größengrenzen. Entpackt wird in einen Zwischenordner, erst dann ersetzt er die alte Version. | `plugins/install.rs` |
| Prozess | Eigene `.exe` ohne Konsolenfenster, Priorität „unter normal“, in einem Job-Objekt mit `KILL_ON_JOB_CLOSE`. JSON-RPC 2.0, eine Nachricht pro Zeile; stderr landet gekürzt im Log. | `plugins/process.rs` |
| Aufpasser | Handshake `initialize` (5 s), nach einem Absturz Neustart nach 2, 10 und 60 s, danach „Fehler“ mit der letzten stderr-Zeile; beim Ausschalten `shutdown` und 2 s Gnadenfrist. | `plugins/supervisor.rs` |
| App | `plugins.enabled` in den Einstellungen, Start beim Programmstart, Ende beim Beenden, Seite „Plugins“ (Installieren mit Bestätigung samt SHA-256, Schalter, Status, Entfernen), Plugins in der Diagnose und im Diagnose-Export (nur Kennung, Version, Zustand). | `plugins/mod.rs`, `PluginsPage.svelte` |
| Beispiel-Plugin | Antwortet auf `initialize`, meldet „ready“, endet auf `shutdown`; eine `behavior.json` erzeugt Absturz, falsche Version oder Schweigen. Nur für Tests, wird nicht ausgeliefert. | `plugins/example/` |

Ordner: Programme unter `%APPDATA%\LyricHub\plugins\<id>`, große Daten wie Modelle unter `%LOCALAPPDATA%\LyricHub\plugins\<id>`.

## Prüfung

**Unit-Tests (laufen mit `cargo test`):**

- Manifest: gültig, unsichere Werte, andere Protokollversion.
- ZIP:
  - Installation aus dem Wurzelordner und aus einem Unterordner.
  - Ersetzen der alten Version.
  - Abgelehnt werden übergeordnete und absolute Pfade, fehlende Manifeste, fehlende Programme und mehrere Wurzelordner.
- Nachrichten: Format und Kürzung, Abstände der Neustarts.
- Job-Objekt: Ein Prozess darin endet, sobald das Job-Objekt geschlossen wird.

**Mit dem Beispiel-Plugin** (`cargo test --lib supervisor -- --ignored`):

- Start, Status und Beenden.
- Absturz mit Neustart nach 2 s und 10 s.
- Andere Protokollversion ergibt „inkompatibel“, Schweigen ergibt „Fehler“.

**In der echten App:**

1. ZIP geprüft (Kennung, Version, SHA-256) und installiert: Das Plugin ist danach aus.
2. Eingeschaltet: nach etwa 1 s „läuft“ mit dem Status „ready“.
3. LyricHub hart beendet (`taskkill /F`): Der Plugin-Prozess war 2 s später weg.
4. Neu gestartet: Das eingeschaltete Plugin lief 40 ms nach dem Start wieder.
5. Ausgeschaltet: Der Prozess endet.
6. Entfernt: Programm- und Datenordner sind leer, und es ist aus den Einstellungen verschwunden.

Ohne Plugins startet nichts. Der Installer bleibt klein: Neu sind nur die Bibliotheken `zip` und `typed-path`.

## P2: Wortzeiten von Plugins

Ein Plugin mit der Fähigkeit `word_timing` bekommt:

- **`track`:** Kennung, Titel, Artist, Album, Länge und den Player. Die Prozess-ID des Players bekommt nur ein Plugin mit `audio:player`.
- **`playback`:** bei jeder Änderung und alle 2 s, mit Position, Tempo und dem Zeitpunkt als QueryPerformanceCounter-Wert in 100 ns. Das ist die Einheit, in der WASAPI aufgenommenen Ton stempelt.
- **`lyrics`:** die synchronen Zeilen mit Start, Ende, Text, Romaji und LyricHubs Wortaufteilung, dazu einen Fingerabdruck aus Startzeiten und Texten.
- **Gerade gestartete Plugins** hören sofort, was läuft.

Die Antwort `word_times` prüft LyricHub Zeile für Zeile: eine Zeit pro Wort, aufsteigende Starts, jedes Wort in seiner Zeile mit 1 s Spielraum. Eine Schwelle für die Konfidenz gibt es nicht (P0). Danach:

- **Speichern:** Die Zeiten landen in der Cache-Tabelle `plugin_words`, nach Song und Fingerabdruck. Ändert sich der Liedtext, gelten sie nicht mehr.
- **Übernehmen:** Sie ersetzen die Schätzung, gelten als gemessen und schalten damit den Wort-Glow an. Läuft der Song gerade, zeigt das Overlay sie sofort.
- **Herkunft:** Die Diagnose nennt die Herkunft der Wortzeiten: geschätzt, aus der LRC-Datei oder vom Plugin.
- **Entfernen:** Ein entferntes Plugin nimmt seine Wortzeiten mit. Der laufende Song fällt sofort auf die Schätzung zurück.

**Prüfung**

- Unit-Tests: Fingerabdruck, Prüfung der Antwort, Übernahme, Cache-Tabelle samt Aufräumen.
- Mit dem Beispiel-Plugin: Es antwortet auf `lyrics` mit Wortzeiten, die gleichmäßig über vier Fünftel jeder Zeile verteilt sind.
- In der echten App mit einem Spotify-Song:
  1. Das Plugin bekam die Lyrics und antwortete. Alle 202 Wörter kamen danach vom Plugin, die Diagnose zeigte „vom Plugin“.
  2. Plugin aus, LyricHub neu gestartet: Die 202 Wörter kamen aus dem Cache.
  3. Plugin entfernt: Der Song fiel auf die Schätzung zurück.
