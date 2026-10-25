# Phase 3 – Sync-Härtung

> Datum: 2026-10-25 · Bezug: PLAN.md 10 (Phase 3), 2, 5.4, 5.5, 5.7, 5.9, 11.3, 14.1
> Ergebnis: **Phase 3 gebaut und gemessen** (Tag `v0.3.0-phase3`). Median-Fehler 96 ms, Seek 30 ms, Nudge pro Song nachgewiesen. Das p95 liegt mit 289 ms über dem Ziel von 200 ms, siehe [sync-report.md](sync-report.md).

## Was gebaut wurde

| Bereich | Umsetzung | Dateien |
|---|---|---|
| Nudge pro Song | Strg+Alt+, und Strg+Alt+. (mit Umschalt ±25 ms) ändern nur den Song-Offset des laufenden Songs, gespeichert in `song_overrides`. Beim Songwechsel gilt sofort der Wert des neuen Songs, 0 ohne Korrektur. Wird ein Song nachträglich anders erkannt, zieht sein Offset mit. Der globale Offset ändert sich nur noch in den Einstellungen. Toast „Song-Offset +200 ms · Lyrics später“. | `sync/offsets.rs`, `app_state.rs`, `ipc/mod.rs`, `hotkeys.rs` |
| Umstieg vom globalen Nudge | Settings-Schema v2. Die Migration setzt `sync.global_offset_ms` auf 0 (Nutzerentscheidung, der Wert stammte aus Phase-2-Nudges) und schreibt die Datei sofort. Unbekannte Felder bleiben beim Speichern erhalten, etwa `appearance.romaji` vom Romaji-Branch. | `settings/schema.rs`, `settings/store.rs` |
| Quellen-Fusion (5.4) | Pro Quelle ein Fenster der Residuen, Korrektur über den nach Confidence gewichteten Median. Konfliktregel: Weichen Quellen über mindestens 5 Samples um mehr als 400 ms ab, fällt die mit der größeren Streuung heraus und gilt im Sync Lab als „abweichend“. Sprünge verschieben die Fenster, Seek, Statuswechsel und Songwechsel leeren sie. Mit nur SMTC ändert sich nichts; getestet mit einer künstlichen zweiten Quelle. | `sync/clock.rs` |
| Seek und Pause | Das Overlay meldet mit `clock_applied`, wann es den Anker eines Seeks gezeichnet hat; LyricHub loggt „seek shown in overlay“ mit beiden Laufzeiten. Seeks im Pausenzustand übernimmt der Clock direkt. | `app_state.rs`, `Overlay.svelte` |
| Geräte-Offsets | Core Audio liefert alle 2 s das Standard-Ausgabegerät mit Name und Bus. Jedes Gerät hat einen eigenen Offset in `device_offsets`; ein neues Gerät startet mit 0, Bluetooth mit +150 ms, und meldet sich im Overlay (außer beim allerersten Gerät). Tabelle auf der Seite „Synchronisation“. | `audio_devices.rs`, `lyrics/cache.rs` |
| Kalibrier-Assistent (5.7) | Nutzt den laufenden Song. Das Overlay zeigt die nächste Zeile mit einem Countdown-Balken; fünfmal Leertaste beim hörbaren Einsatz. Ausreißer über 600 ms fallen heraus, der Median wird als Geräte-Offset vorgeschlagen. Leertaste und Esc sind nur während einer Messung globale Hotkeys. | `calibration.rs`, `sync/taps.rs`, `hotkeys.rs`, `Karaoke.svelte` |
| Sync-Protokoll per Tap | Gleiche Mechanik ohne Ende: Leertaste bei jedem hörbaren Zeilenbeginn. Fehler je Tap, Median, p95, Tendenz und Anteil unter 100 ms, gesamt und pro Song; Datei `logs/sync-protocol-<unix>.json` ohne Liedtext. | `calibration.rs`, `sync/taps.rs` |
| Sync Lab (5.9) | Diagnose-Seite: Positionsberichte der letzten 60 s als Abweichung von der Vorhersage, Farbe und Form nach Clock-Entscheidung (keine Korrektur, Slew, Sprung, Seek), dazu Taps und senkrechte Marken für Songwechsel und Status; Tabelle als Alternative. Live-Werte: Position, Tempo, Slew-Rest, alle Offsets, Bericht-Kadenz, Alter des letzten Berichts, letzte Clock-Entscheidung, abweichende Quellen. „Logs öffnen“ und „Diagnose exportieren“ (`logs/diagnostics-<unix>.json`). | `sync_lab.rs`, `SyncLab.svelte` |
| Sonderzustände (5.5) | Werbung: Titel „Advertisement“ oder „Spotify“, kein Artist, höchstens 40 s; Overlay blendet aus. Podcast: über 20 min und kein Album; nur Titel, keine Lyrics-Suche. Spotify geschlossen: Overlay aus, Tray-Icon grau. Instrumental, keine synchronen Lyrics und Pause (dimmen, ausblenden) gab es schon. | `sources/types.rs`, `sources/smtc.rs`, `tray.rs`, `Overlay.svelte` |
| Seite „Synchronisation“ | Offset dieses Songs mit „Auf 0“, Audiogeräte, Kalibrieren, Sync-Protokoll, globaler Offset, Lyrics-Status. | `SyncPage.svelte` |
| Test-Skripte | `seek-test.ps1` (Seek-Latenz bis zum gezeichneten Overlay), `nudge-test.ps1` (Nudge pro Song über einen echten Songwechsel). | `tools/sync/` |

## Definition of Done

| Kriterium | Ergebnis | Nachweis |
|---|---|---|
| Sync-Protokoll 11.3 mit Kabel: Median ≤ 100 ms | **erfüllt**: 96 ms (Geräte-Offset +100 ms, 115 Taps, 13 Songs) | [sync-report.md](sync-report.md) |
| p95 ≤ 200 ms | **nicht erfüllt**: 289 ms. Ursache sind Versatz und Streuung der LRC-Dateien plus Tipp-Streuung, nicht die Zeitkette. Keine zweite Runde (Nutzerentscheidung) | [sync-report.md](sync-report.md) |
| Seek-Test | **erfüllt**: Median 30 ms, höchstens 34 ms vom Seek bis zum gezeichneten Overlay (Ziel 300 ms) | `tools/sync/seek-test.ps1` |
| Nudge pro Song greift beim Songwechsel | **erfüllt**: 4 von 4, Log-Auszug im Bericht | `tools/sync/nudge-test.ps1` |
| `run-spike.ps1` grün | plain 13/13, game-once 15/15, game-aggressive 13/14 (der bekannte Worst-Case-Befund „Overlay wird gezeichnet“ aus Phase 0) | `docs/spike-overlay-*.json` |
| CPU ≤ 2 % bei laufender Wiedergabe | **erfüllt**: 0,67 % von 16 Kernen | `measure-cpu.ps1` |
| Edit-Modus weiter intakt | 13 von 13 | `edit-mode-test.ps1` |
| Unit-Tests | **76 grün**, darunter Clock 14 (3 neue zur Fusion), Taps 2, Offsets 2, Werbung und Podcast 2, Sync Lab 1, Migration und unbekannte Settings-Felder 2 | `cargo test` |

Außerdem live geprüft: Der Wechsel von „Speakers“ auf „Headphones (High Definition Audio Device)“ wurde erkannt und als neues Gerät gemeldet. Nicht live geprüft: Werbung (kam in den Tests nicht vor), Podcast und das graue Tray-Icon; die Erkennung von Werbung und Podcast deckt `classify()` mit Unit-Tests ab.

## Abweichungen vom Plan

- **Tap-Messung statt Schätzung in 100-ms-Schritten** (11.3): Leertaste bei jedem hörbaren Zeilenbeginn, Fehler in Millisekunden. Die Reaktionszeit beim Tippen steckt mit in jedem Wert.
- **Kein japanischer Titel im Protokoll** (verlangt: zwei). Der Nutzer kann die Zeilen nicht lesen; Romaji kam erst nach dem Tag mit dem Merge von `claude/unruffled-dirac-66618b` nach main.
- **Kalibrierung über einen Song überschießt:** Sie misst das LRC-Timing dieses Songs mit. Der erste Vorschlag (151 ms, über „My Ordinary Life“) lag 51 ms zu hoch; der Nutzer hat den Geräte-Offset nach dem Protokoll auf +100 ms gesetzt.
- **Geräteerkennung per Abfrage alle 2 s** statt `IMMNotificationClient::OnDefaultDeviceChanged`: kein COM-Callback-Objekt, gleiche Wirkung bei einem Gerätewechsel pro Sitzung.
- **Geräte-Offsets in der Datenbank** (`device_offsets`, Schema aus 7.3) statt in `settings.json`, neben den Song-Offsets.
- **Sync Lab zeichnet Abweichungen statt Positionen:** Eine Positionslinie über 60 s wäre eine gerade Diagonale ohne sichtbaren Fehler. Die Modellposition steht als Wert daneben. Offsets zeigt es in drei Teilen (global, Gerät, Song); das LRC-`[offset:]` steckt schon in den Zeilenzeiten.
- **Werbung bis 40 s statt „≈ 30 s“**, damit auch längere Spots greifen.
- **Phase 3b (Web API) entfällt** (Entscheidung der übergeordneten Sitzung). Die Fusion ist damit nur mit einer künstlichen zweiten Quelle getestet.
- **Vorzeichen in PLAN.md 5.2** weiterhin wie in Phase 2: umgesetzt ist der Satz „positiv = Lyrics später“, nicht die Formel.

## Offene Punkte

- **p95:** Mit Song-Offsets lässt sich der Versatz pro Song beheben (im Protokoll −140 bis +151 ms), die Streuung innerhalb eines Songs (p95 267 ms) nicht. Ob sie von den LRC-Dateien oder vom Tippen kommt, lässt sich mit dieser Methode nicht trennen. Eine Gegenprobe wäre ein Song mit Wortzeiten (Enhanced LRC) oder eine selbst gesetzte LRC-Datei.
- **Geräte-Offset aus dem Protokoll vorschlagen:** Die Tendenz über mehrere Songs ist ein besserer Geräte-Offset als fünf Taps auf einem Song. Ein Knopf „Tendenz als Geräte-Offset übernehmen“ am Ende einer Messung wäre wenig Aufwand.
- **Japanische Messung** mit Romaji nachholen (Strg+Alt+R, seit dem Merge nach `v0.3.0-phase3` in main).
- **Fokus-Test mit Tippen** (11.3) ist nur über die Vordergrund-Prüfungen von `run-spike.ps1` und `edit-mode-test.ps1` abgedeckt. Während einer Messung fängt LyricHub die Leertaste absichtlich ab; Esc beendet die Messung.
