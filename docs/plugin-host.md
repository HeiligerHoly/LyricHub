# Plugin-Host (P1 aus PLAN-plugins.md)

> Datum: 2026-10-28 · nach v1.1.0 · Bezug: [PLAN-plugins.md](../PLAN-plugins.md) Abschnitte 4 und 6
> Ergebnis: **LyricHub installiert, startet, überwacht und beendet Plugins. Alle Abnahmekriterien von P1 sind in der echten App geprüft.** Plugins können noch nichts liefern; die Wortzeiten folgen in P2.

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

## Offen für P2

- Die Fähigkeit `word_timing`: Benachrichtigungen `track`, `playback`, `lyrics` an das Plugin, `word_times` zurück, Cache-Tabelle und Übernahme in die Lyrics.
- Die Prozess-ID des Players nur für Plugins mit `audio:player`.
