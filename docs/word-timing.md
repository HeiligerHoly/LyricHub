# Geschätzte Wortzeiten – Messung und neues Modell

> Datum: 2026-10-28 · nach v1.0.1 · Bezug: PLAN.md 5.6, `src-tauri/src/lyrics/words.rs`, `parser.rs`
> Ergebnis: **Der Wortbeginn liegt im Median 136 statt 184 ms neben dem gesungenen, das langsamste Zehntel ab 0,73 statt 1,28 s.**

## Ausgangslage

LRCLIB liefert fast nur Zeilenzeiten (0 von 160 Treffern hatten Wortzeiten, Phase 4). Für die Füllung Wort für Wort schätzt LyricHub die Wörter deshalb selbst. Bis v1.0.1 bekam jedes Wort einen Anteil der Zeile nach seinen Buchstaben, und die Zeile wurde bis zur nächsten gefüllt. Folgte eine Pause von mehr als 2,5 s über der geschätzten Singzeit, galten 70 ms pro Buchstabe.

## Daten und Methode

- **Datensatz:** JamendoLyrics MultiLang (Durand, Stoller, Ewert, ICASSP 2023), 79 Songs mit Annotationen, je etwa 20 auf Deutsch, Englisch, Französisch und Spanisch, 21 580 Wörter auf 3 383 Zeilen. Die Wortzeiten sind von Hand ausgerichtet. Die Annotationen stehen unter MIT, die Texte unter Creative-Commons-Lizenzen der einzelnen Songs. Der Datensatz liegt nicht im Repository, und die Messung gibt keine Texte aus.
- **Simulation:** Jede annotierte Zeile wird zu einer LRC-Zeile mit dem Zeitstempel ihres ersten Worts. Der Stempel der nächsten Zeile beendet sie, wie im Parser. Für jedes Wort zählt der Abstand zwischen geschätztem und annotiertem Beginn.
- **Einstellen und Prüfen:** Die Parameter wurden an jedem zweiten Song gewählt. Die andere Hälfte diente nur zur Kontrolle.
- **Nachmessen:** `cargo test --lib measure_word_timing -- --ignored --nocapture` in `src-tauri`, mit dem Datensatzordner in `LYRICHUB_WORD_TIMES` (Ordner `annotations/words` und `lyrics` aus github.com/f90/jamendolyrics).

## Was die Abweichung verursachte

| Fall | Anteil der Wörter | Abweichung im Median | Abweichung p90 |
|---|---|---|---|
| Füllung bis zur nächsten Zeile | 71 % | 159 ms (meist zu spät) | 604 ms |
| Lange Pause danach, 70 ms pro Buchstabe | 29 % | 417 ms (meist zu früh) | 2 431 ms |

Bei langen Pausen wird im Median 2,3-mal so lange gesungen, wie 70 ms pro Buchstabe annehmen. Die Wörter leuchten dann viel zu früh auf. Bis zur nächsten Zeile wird im Median 87–92 % des Abstands gesungen, nicht 100 %. Erst bei Abständen über 7 s folgt typischerweise eine Pause. Buchstaben gegen Silben zu tauschen allein änderte fast nichts (Median 185 → 191 ms).

## Neues Modell

- **Singzeit einer Zeile** (`parser::fill_end`): 90 % des Abstands zur nächsten Zeile, aber höchstens 400 ms pro Silbe, mindestens 1 s und ohne nächste Zeile höchstens 8 s. Danach bleibt die Zeile gefüllt stehen.
- **Anteil eines Worts** (`words::estimate`): Silben plus 0,2 pro Buchstabe. Das letzte Wort zählt 1,3-fach, weil es gehalten wird. Die Zeile wird ohne Vorlauf und Puffer vom Stempel bis zum Ende der Singzeit verteilt.
- **Silben:** Gruppen aufeinanderfolgender Vokale (lateinisch mit Akzenten und kyrillisch), mindestens eine pro Wort. Hangul zählt eine pro Silbenblock. Kana zählen 1 und Kanji 1,5, wie bisher. Eine Regel für das stumme „e“ (englisch, französisch) brachte 1 ms und wurde weggelassen.

## Ergebnis

Gemessen mit der Rust-Implementierung, alle 79 Songs:

| | bis v1.0.1 | neu |
|---|---|---|
| Median | 184 ms | 136 ms |
| Mittel | 461 ms | 283 ms |
| p90 | 1 282 ms | 728 ms |
| höchstens 150 ms daneben | 44,7 % | 52,5 % |

Nur die Kontrollhälfte, die nicht zum Einstellen benutzt wurde: Median 184 → 137 ms, Mittel 493 → 288 ms, p90 1 466 → 747 ms.

Nach Sprache (Median / p90):

| Sprache | bis v1.0.1 | neu |
|---|---|---|
| Deutsch | 172 / 1 176 ms | 111 / 652 ms |
| Englisch | 188 / 1 255 ms | 156 / 783 ms |
| Französisch | 183 / 1 132 ms | 152 / 761 ms |
| Spanisch | 193 / 1 549 ms | 131 / 687 ms |

Die Werte nach Sprache und für die Hälften stammen aus einem Python-Nachbau, der die Gesamtwerte der Rust-Implementierung auf wenige Millisekunden trifft.

## Was diese Messung nicht zeigt

- **Zeitstempel aus LRCLIB:** Die Simulation setzt jeden Stempel genau auf das erste Wort. Echte LRC-Dateien setzen ihre Zeilen je Song früher oder später, im Median eines Songs zwischen −140 und +151 ms ([Sync-Bericht](sync-report.md)). Mit ±150 ms Versatz liegt der Median bei 157–205 ms (Einstellhälfte, Python-Nachbau), um 150 ms zu späte Stempel also über dem alten Wert ohne Versatz. Mittel (319–332 ms) und p90 (712–727 ms) bleiben auch dann deutlich unter den alten Werten. Den Versatz pro Song gleicht der Song-Offset aus.
- **Japanisch und Koreanisch:** Der Datensatz enthält keine Songs in diesen Sprachen. Für Kana, Kanji und Hangul gelten weiter die bisherigen Gewichte, nur das gehaltene letzte Wort und die neue Singzeit kommen hinzu.
- **Gefühl:** Die Messung zählt den Wortbeginn. Wie flüssig die Füllung wirkt, zeigt sie nicht. Wem sie weiter zu ungenau ist, kann sie unter Darstellung → „Wort für Wort füllen“ abschalten.
