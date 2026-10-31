# Spike – Auto-Sync und Erkennen (Q0 aus PLAN-plugins-2.md)

> Datum: 2026-10-30 · nach v1.2.0 · Bezug: [PLAN-plugins-2.md](../PLAN-plugins-2.md) Abschnitt 6, [docs/spike-align.md](spike-align.md)
> Ergebnis: **Auto-Sync (M1) und das Ausrichten von Text ohne Zeitstempel (M3) schaffen das Gate deutlich. Die breitere Suche (M4) findet für 7 von 19 Songs ohne Lyrics Kandidaten. Die Erkennung aus dem Gesang (M2) ist knapp: Das Align-Modell schafft die Fehlergrenze, ist aber zu langsam; Whisper ist genauer und schneller, braucht aber zu viel Speicher und erfindet Text in Instrumentalteilen.**

## Aufbau

| Punkt | Wert |
|---|---|
| Songs | **M1:** die 24 Songs aus P0. **M2 und M3:** die 4 Songs der P0-Laufzeitmessung, je einer auf Deutsch, Englisch, Französisch und Spanisch (13,7 min, 182 Zeilen, 1100 Wörter). |
| Pipeline | Die des Plugins Align in Rust, Ton wie in der Warteschlange über 16 Bit. Lange Fenster gehen in Stücken von 14 s mit 2 s Kontext an das Modell. |
| Kandidat A | Align-Modell (w2v-bert-2.0 music-lyrics CTC, int8) mit Greedy-Dekodierung, ohne Sprachmodell |
| Kandidat B | Whisper large-v3-turbo über faster-whisper, int8 auf der CPU, Spracherkennung an, ohne VAD |
| PC | Ryzen 7 3800X, 32 GB RAM, kein Spiel; Priorität „unter normal“, 2 Threads |
| Skripte | `plugins/align/examples/recognize.rs` (M2 A, M3) sowie im Scratchpad `m1_linestart.py`, `m2_whisper.py` und `m4_search.py`. Sie geben nur Zahlen aus, keine Liedtexte und keinen erkannten Text. |

## M1 – Zeilenbeginn für Auto-Sync

Gemessen wurde, wie weit das erste ausgerichtete Wort jeder Zeile vom annotierten Beginn liegt:

| | Median | p90 | höchstens 100 ms daneben |
|---|---|---|---|
| **Zeilenbeginn, 24 Songs, 993 Zeilen (P0-Pipeline)** | **37 ms** | **138 ms** | 84,4 % |
| übrige Wörter zum Vergleich | 37 ms | 115 ms | 87,0 % |
| Zeilenbeginn, 4 Songs, Rust mit Stücken | 40 ms | 161 ms | 69,8 % |

Nach Sprache (24 Songs, Median / p90): Deutsch 25 / 122 ms, Englisch 42 / 183 ms, Französisch 37 / 143 ms, Spanisch 22 / 64 ms.

Zum Vergleich der erste echte Spotify-Song mit Align: Dort lag das erste Wort im Median 41 ms nach dem Stempel der LRC-Datei, p10 99 ms davor, p90 243 ms danach. Auto-Sync rückt solche Zeilen auf den Gesang.

Die Fenster beginnen 0,5 s vor dem LRC-Stempel. Singt jemand früher, findet die Ausrichtung das erste Wort nicht richtig. Für Auto-Sync braucht es deshalb in Q2 ein früheres Fenster.

## M2 – Erkennung ohne Liedtext

Zeichen- und Wortfehlerrate gegen die annotierten Liedtexte, beide Texte gleich normalisiert (NFKC, Kleinschreibung, nur Buchstaben, Ziffern und Apostrophe):

| Sprache | A: Zeichen / Wörter | B: Zeichen / Wörter |
|---|---|---|
| Deutsch | 8,1 / 20,2 % | 7,0 / 13,9 % |
| Englisch | 46,2 / 75,1 % | 44,2 / 69,3 % |
| Französisch | 31,4 / 53,4 % | 32,0 / 50,0 % |
| Spanisch | 20,0 / 41,2 % | 8,9 / 22,2 % |
| **alle** | **23,9 / 43,9 %** | **21,4 / 36,4 %** |

| | A | B |
|---|---|---|
| Erkannter Text außerhalb des Gesangs (mehr als 1 s von jeder Zeile entfernt) | 0,0 % | 4,3 % |
| Rechenzeit mit 2 Threads | 1,86 × Songlänge | 0,90 × Songlänge |
| Spitzenspeicher des Prüfprogramms | 1,50 GB | 2,15 GB (Python mit PyTorch) |
| Sprache richtig erkannt | – | 4 von 4 |
| zusätzlicher Download | keiner | etwa 1,6 GB |

- **Beide stolpern über denselben englischen Song**, der schon in P0 der schwerste war. Deutsch und Spanisch erkennen beide gut lesbar.
- **A liefert Text und Zeiten in einem Lauf.** Es ist dieselbe Modellausgabe, die auch die Ausrichtung braucht. Bei B kommen die Zeiten erst aus einem zweiten Lauf mit dem Align-Modell, insgesamt also etwa 2,2–2,8 × Songlänge und zwei Modelle nacheinander.
- **Das Plugin bräuchte für B einen eigenen Weg.** faster-whisper ist Python. Im Plugin bräuchte es Whisper über ONNX oder whisper.cpp; wie viel Speicher dann bleibt, ist nicht gemessen.
- **A war hier langsamer als in P0** (1,86 statt 1,39 × Songlänge mit Stücken): Bei ganzen Songs geht jedes 10-s-Stück mit 4 s Kontext an das Modell. Während des Laufs suchte zudem M4 im Netz.

## M3 – Text ohne Zeitstempel ausrichten

Der ganze Liedtext auf einmal über den ganzen Song, ohne Zeilenzeiten. Gemessen wurde der Abstand von Zeilen- und Wortbeginn zur Annotation:

| Sprache | Zeilenbeginn Median / p90 | Wortbeginn Median / p90 |
|---|---|---|
| Deutsch | 36 / 138 ms | 51 / 108 ms |
| Englisch | 120 / 297 ms | 142 / 309 ms |
| Französisch | 37 / 253 ms | 43 / 369 ms |
| Spanisch | 22 / 63 ms | 27 / 70 ms |
| **alle** | **41 / 197 ms** | **50 / 184 ms** |

Das ist fast so genau wie Align mit Zeilenzeiten, obwohl hier keine einzige Zeit vorgegeben war.

## M4 – Breitere Suche bei LRCLIB

Für die Songs ohne Lyrics im Cache des Nutzers wurde LRCLIB nur nach dem bereinigten Titel gefragt, als `track_name` und als `q`, ohne Artist. Gezählt wurden nur Treffer, keine Namen:

| Songs (von 19) | Anzahl |
|---|---|
| mit irgendeinem Treffer | 11 |
| mit synchronen Lyrics | 10 |
| mit synchronen Lyrics und ±3 s Länge | **7** |
| mit synchronen Lyrics und ±10 s Länge | 8 |
| davon vom selben Artist, ±3 s | 4 |

- **Untergrenze:** LRCLIB war überlastet. Auch nach geduldigen Wiederholungen mit der vom Server verlangten Wartezeit scheiterten 8 von 38 Anfragen. Die Zahlen sind deshalb eine Untergrenze.
- **Nicht alle sind echte Fehltreffer:** 6 von 20 Einträgen im Fehltreffer-Cache laufen in etwa einem Tag ab. Das ist LyricHubs Frist, wenn LRCLIB überlastet war („heute aufgeben“). Diese Songs fehlen also nur vorübergehend.
- **Kandidaten vom selben Artist:** 4 Songs haben einen Kandidaten vom selben Artist mit passender Länge. Die heutigen Stufen fragen immer mit Artist. Eine zusätzliche Stufe „nur Titel“ mit Artist-Abgleich könnte sie schon ohne Erkennung finden. Für die übrigen 3 muss der Textvergleich entscheiden, ob der Kandidat der gehörte Song ist.

## Bewertung gegen das Gate

| Messung | Gate | Ergebnis | |
|---|---|---|---|
| M1 Zeilenbeginn | Median ≤ 50 ms, p90 ≤ 200 ms | 37 / 138 ms | erfüllt |
| M2 Erkennung, A | CER ≤ 25 %, ≤ 1,6 × Songlänge, ≤ 1,5 GB | 23,9 %, 1,86 ×, 1,50 GB | Geschwindigkeit verfehlt |
| M2 Erkennung, B | wie oben | 21,4 %, 0,90 ×, 2,15 GB | Speicher verfehlt (Python) |
| M3 Text ausrichten | Median ≤ 100 ms, p90 ≤ 500 ms | 41 / 197 ms | erfüllt |
| M4 Suche | Bericht | Kandidaten für 7 von 19 Songs | Kandidaten vorhanden |

## Empfehlung

- **Auto-Sync bauen (Q2)**, mit früherem Fenster für den Zeilenbeginn.
- **Erkennen bauen (Q3)**, zuerst die sicheren Wege: breitere Suche mit Textvergleich, Ausrichten echter Lyrics und Ausrichten von Text ohne Zeitstempel.
- **Für die Anzeige erkannter Lyrics Kandidat A.**
  - **Dafür spricht:** Es braucht kein zweites Modell und keinen zweiten Download. Ein Lauf liefert Text und Zeiten, und in Instrumentalteilen erfindet A nichts.
  - **Geschwindigkeit:** Zu verbessern ist sie über weniger Kontext pro Stück, 1 statt 2 s. Das kostet erst in Q3 Messzeit.
  - **Schwelle:** Erkannte Lyrics erscheinen nur, wenn die Erkennung sicher genug wirkt. Die Schwelle wird in Q3 an echten Songs festgelegt. Unter der Schwelle bleibt es bei der kompakten Karte.
  - **Whisper** bleibt eine Option für später, falls die Qualität von A nicht reicht.
- **Unabhängig von den Plugins, im Kern von LyricHub:**
  - Eine LRCLIB-Stufe „nur Titel“ mit Artist-Abgleich.
  - Bei Überlast nicht mehr einen ganzen Tag aufgeben, sondern es nach kurzer Zeit erneut versuchen.

## Was diese Messung nicht zeigt

- **Kleine Stichprobe für M2 und M3:** nur 4 Songs. Die Spanne zwischen den Sprachen ist groß, ein Song kann das Bild verschieben.
- **Echte Spotify-Songs:** Dicht produzierter Pop und Rap sind womöglich schwerer als die CC-Musik von Jamendo.
- **Japanisch und Koreanisch:** nicht gemessen.
- **Textvergleich bei der Suche:** Ob er den richtigen Kandidaten sicher erkennt, braucht den Ton der echten Fehltreffer. Das geht erst mit dem Plugin.
- **Speicher von Whisper im Plugin:** nicht gemessen.
