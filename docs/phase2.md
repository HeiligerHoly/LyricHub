# Phase 2 – Lyrics und Clock

> Datum: 2026-10-25 · Bezug: PLAN.md 10 (Phase 2), 3.3, 5.1–5.7, 7, 9.2–9.6
> Ergebnis: **Phase 2 abgeschlossen** (Tag `v0.2.0-phase2`). Hörtest mit zehn Songs durch den Nutzer: Die Zeilen wechseln synchron.

## Was gebaut wurde

| Bereich | Umsetzung | Dateien |
|---|---|---|
| LRC-Parser | Mehrere Zeitstempel pro Zeile, Metadaten, `[offset:]` (positiv = früher, LRC-Konvention), Enhanced-LRC-Wortmarken, BOM, CRLF/CR, 0–3 Nachkommastellen, `:` als Trenner, unsortierte und doppelte Zeilen, Abschnittsmarken. Leere Zeilen bleiben Instrumental-Marken. Pro Zeile das Ende der Füllung; vor langen Pausen aus der Textlänge geschätzt (5.6, Punkt 2). | `src-tauri/src/lyrics/parser.rs` |
| Matcher | NFKC, Kleinschreibung, ohne Satzzeichen; Klammerzusätze und „ - Remastered“ nur als Ausweichstufe; Haupt-Artist; Dauerabweichung über 8 s = andere Ausgabe; Titel mit Jaro-Winkler ≥ 0,85. | `lyrics/matcher.rs` |
| LRCLIB-Client | Stufen: get, get ohne Album, normalisiertes get, Suche nach Feldern, Freitextsuche. Gestaffelt: Stufe 1 sofort, alle 350 ms ohne Antwort oder nach einem Fehlschlag die nächste parallel; der erste passende Treffer mit synchronen Lyrics gewinnt. User-Agent `LyricHub/<version> (Windows desktop lyrics overlay)`. | `lyrics/lrclib.rs` |
| Cache | SQLite `%APPDATA%/LyricHub/lyrichub.db` mit den Tabellen aus 7.3 (Song- und Geräte-Offsets vorbereitet), Fehlschläge 7 Tage, „dauerhaft nicht erreichbar“ 24 h. | `lyrics/cache.rs` |
| Lyrics-Worker | Ein Thread, nur die neueste Anfrage zählt, Wiederholungen nach 5 s, 30 s und 2 min. Logs enthalten Zeilenzahlen, nie Liedtext. | `lyrics/service.rs` |
| PlaybackClock | Nach 5.3 und den Spike-Regeln: Seek ab 1,5 s, das Update direkt nach einem Seek wird übernommen, Korrekturen bis 300 ms per Slew (höchstens ±10 %), größere als Sprung, unter 20 ms ignoriert, im Pausenzustand direkt übernommen. Residuen werden nach einer Korrektur verschoben, damit nichts doppelt zählt. UI-frei, jede Entscheidung wird geloggt. | `src-tauri/src/sync/clock.rs` |
| Clock im Frontend | `clock.ts` rechnet den Anker mit `performance.now()` weiter, spiegelt den Slew und gleicht Abweichungen unter 300 ms über 300 ms an. Ein Timer pro Zeile. | `src/lib/overlay/clock.ts` |
| Karaoke-Füllung | Zwei Textebenen; die Akzent-Ebene wird per `transform` aufgedeckt, angetrieben von Web Animations mit `steps()`, 30 Schritte pro Sekunde Songzeit. Der Compositor animiert ohne JavaScript pro Frame. | `KaraokeLine.svelte` |
| Layout „Karaoke“ | Neuer Standard: Kopfzeile mit Cover, aktuelle Zeile groß, nächste gedimmt, feste Zeilenhöhen. Bei Songs mit langen Zeilen wird die Box einmal pro Song breiter (bis Monitorbreite minus Rand); erst danach wird verkleinert. `lang="ja"` für Zeilen mit Kana oder Kanji. Ladepunkte höchstens 1,5 s, Hinweise „Keine synchronen Lyrics gefunden“ und „Instrumental“ 4 s. | `layouts/Karaoke.svelte`, `Overlay.svelte` |
| Offset und Nudge | Globaler Offset in den Einstellungen, positiv = Lyrics später. Strg+Alt+, und Strg+Alt+. um ±100 ms, mit Umschalt ±25 ms, Toast im Overlay, auch im Tray. | `ipc/mod.rs`, `hotkeys.rs`, `tray.rs` |
| Einstellungen | Layout-Auswahl (auch Strg+Alt+K und Tray), Deckkraft des Hintergrunds, neue Seite „Synchronisation“ mit Offset als Satz („früher / später“), Nudge-Kürzeln, Lyrics-Status, „Lyrics neu laden“ und Crossfade-Hinweis. Diagnose zeigt Clock und Lyrics. | `src/lib/settings/` |

## Definition of Done

| Kriterium | Ergebnis | Nachweis |
|---|---|---|
| Zehn Songs mit synchronen Lyrics laufen zeilengenau mit (EN, DE, JA) | **erfüllt**: Nutzer: „die Song-Zeilen switchen alle perfekt“, auch beim französischen Song | Tabelle unten |
| Nudge korrigiert sichtbar | Vom Nutzer im Hörtest bei „Discord“ benutzt: viermal +100 ms, dann zurück auf 0; jede Änderung ging als neuer Clock-Anker ans Overlay. Rückmeldung ohne Beanstandung, eine ausdrückliche Aussage zur Verschiebung fehlt | Log `offset nudged` |
| Parser- und Clock-Tests grün | **erfüllt**: 64 Unit-Tests, davon Parser 12, Clock 11, LRCLIB 8, Matcher 7, Cache 3 | `cargo test` |
| `run-spike.ps1` grün | plain 13/13, game-once 15/15, game-aggressive 12/14 (die zwei bekannten Worst-Case-Befunde aus Phase 0) | `docs/spike-overlay-*.json` |
| CPU ≤ 2 % aller Kerne bei laufender Füllung | **erfüllt**: 9,4 % eines Kerns = **0,59 % von 16 Kernen** über einen ganzen Song mit 60 Zeilen | `measure-cpu.ps1 -Seconds 60` |
| Edit-Modus weiter intakt | 13 von 13 | `edit-mode-test.ps1` |

### Hörtest (Nutzer am PC, kabelgebunden)

| # | Song | Sprache | Zeilen | Stufe | Suche |
|---|---|---|---|---|---|
| 1 | MÄNNERGEFÜHLE – SALÒ | DE | 43 | get | 1249 ms |
| 2 | Jane! – The Long Faces | EN | 29 | get | 56 ms |
| 3 | Discord – The Living Tombstone | EN | 40 | get | 88 ms |
| 4 | Time Moves Slow – BADBADNOTGOOD | EN | 57 | get | 55 ms |
| 5 | Oddloop – frederic | JA (alle 50 Textzeilen mit Kana) | 54 | get ohne Album | 468 ms |
| 6 | jester – Kenricx | EN | 40 | get ohne Album | 1454 ms |
| 7 | You Got Jokes – Khantrast | DE | 59 | get ohne Album | 410 ms |
| 8 | makkohatjedenliebaussersichselbst – makko | DE | 50 | Suche | 900 ms |
| 9 | Dernière danse – Indila | FR | 43 | get | 60 ms |
| 10 | WOLKE – Leftovers | DE | 36 | Suche | 1869 ms |

Außerdem: „Ich will nur dass du weißt“ (SDP) hat auf LRCLIB nur Lyrics ohne Zeitangaben und zeigt den Hinweis; „chess“ (joyful) ist als Instrumental erkannt. Titelwechsel standen nach 90–180 ms im Overlay. Der Offset blieb bei 0: Mit kabelgebundener Ausgabe brauchte es keine Kalibrierung.

### Lyrics-Suche online

Median etwa 0,4 s, drei von zwölf Suchen über 1,5 s (bis 1,9 s). Ursache ist LRCLIB: Unter Last braucht jede Antwort etwa 1,1 s, und dieselbe Anfrage liefert mal 200, mal 503. Unbekannte Titel und leere Suchen beantwortet LRCLIB derzeit mit 503 „ServerOverloaded“ statt 404. Gegenmaßnahmen: gestaffelte Stufen (Test: eine 2 s langsame erste Stufe wird nach 350 ms überholt), 503 nie als Fehlschlag cachen, nach allen Wiederholungen 24 h Ruhe. Aus dem Cache kommen Lyrics in etwa 3 ms.

## Befunde aus dem Hörtest

- **Zeilenwechsel:** synchron bei allen Songs.
- **Gelbe Füllung trifft das gesungene Wort meist nicht.** Erwartet: LRCLIB liefert nur Zeilen-Zeitstempel. Die Füllung läuft linear vom Zeilenbeginn bis zur nächsten Zeile und kennt weder Atempausen noch das Tempo innerhalb der Zeile. Wortgenaues Hervorheben ist Phase 4 (PLAN.md 5.6): Schätzung der Wortgrenzen und echte Wortzeiten, wo LRCLIB sie hat (Feld `hasWordSync`, strukturierte Zeilen im neuen Feld `lyricsfile`). Ansatz für Phase 4: die Schätzung an LRCLIB-Songs mit Wortzeiten kalibrieren.
- **Lange Zeilen wurden abgeschnitten.** Behoben: Die Box wird pro Song so breit wie die längste Zeile.
- **Wunsch: höhere Deckkraft.** Umgesetzt: Regler „Deckkraft des Hintergrunds“.
- **Wunsch: Romaji für japanische Songs.** Steht in PLAN.md 13.6 als erster Kandidat für Phase 6; braucht ein Wörterbuch mit Kanji-Lesungen.

## Abweichungen vom Plan

- **Vorzeichen des Offsets:** PLAN.md 5.2 sagt „positiv = Lyrics später“, die Formel `display = clock + offset` ergäbe das Gegenteil. Umgesetzt ist der Satz: `display = clock − offset`. Das `[offset:]`-Tag folgt der LRC-Konvention (positiv = früher) und ist bereits in die Zeiten eingerechnet.
- **Nudge-Ziel ist in Phase 2 der globale Offset.** Song-Offset als Standardziel und die Umschaltung kommen mit Phase 3 (Song-Offset-Persistenz).
- **Event `clock`** trägt zusätzlich `slew_ms`: Der Slew ist Teil des Ankers, damit Backend und Frontend dasselbe stückweise Modell ohne Timer rechnen. `lyrics` trägt immer `{ key, state, source, lines, lrc_offset_ms }`, auch beim Laden.
- **Test-Fixtures mit erfundenem Text** statt „realer LRC-Beispiele“ (7.2) und aufgezeichneter Antworten (11.2): Liedtexte sind urheberrechtlich geschützt. Die Fixtures bilden Format und JSON-Struktur echter Dateien und Antworten nach.
- **Drift-Test:** 0,1 % Gangabweichung bleibt unter 30 ms Fehler, 1 % unter 100 ms. Das Ziel „unter 50 ms“ gilt damit bis weit über die gemessenen 74 ppm.
- **Neue Einstellungsfelder ohne Schemawechsel:** `overlay.layout`, `appearance.plate_opacity`, `sync.global_offset_ms`. Alle haben Standardwerte; Dateien aus Phase 1 laden unverändert (Test vorhanden).
- **Layouts:** Karaoke ist Standard, „Bar“ heißt in der Oberfläche „Leiste“ und zeigt die aktuelle Zeile ohne Füllung. Stack und Minimal folgen in Phase 4.
- **Füllfarbe** ist fest `#ffd36e`; die Akzentfarbe aus dem Cover kommt in Phase 4.
