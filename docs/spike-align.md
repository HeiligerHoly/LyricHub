# Spike – Wortzeiten aus dem Ton (P0 aus PLAN-plugins.md)

> Datum: 2026-10-28 · nach v1.0.1 · Bezug: [PLAN-plugins.md](../PLAN-plugins.md) Abschnitt 6, [docs/word-timing.md](word-timing.md)
> Ergebnis: **Die Genauigkeit erfüllt das Gate deutlich: Wortbeginn im Median 37 statt 129 ms, p90 119 statt 713 ms (Option B). Die Rechenzeit verfehlt es auf der CPU: 1,5 × Songlänge mit 2 Threads statt höchstens 0,6. Der Mitschnitt von Spotify funktioniert.**

## Aufbau

| Punkt | Wert |
|---|---|
| Songs | 24 aus JamendoLyrics MultiLang, je 6 auf Deutsch, Englisch, Französisch und Spanisch (jeder dritte Song pro Sprache, alphabetisch, nicht nach Klang ausgewählt), 85,6 min Audio, 6 027 Wörter |
| Simulation | wie in `docs/word-timing.md`: jede Zeile als LRC-Zeile, gestempelt beim ersten Wort; der Aligner bekommt den Ton von 0,5 s vor diesem Stempel bis 0,5 s nach dem nächsten |
| Option A | htdemucs (Gesang abtrennen, PyTorch) + MMS-Forced-Aligner aus torchaudio (fp32); Text ohne Akzente, a–z |
| Option B | w2v-bert-2.0 „music-lyrics-ctc“ als ONNX int8 (588 MB), direkt auf dem Mix; Text in NFKC, Kleinschreibung |
| Ausrichtung | CTC-Viterbi (`torchaudio.functional.forced_align`), Wortbeginn = erstes Zeichen des Worts |
| PC | AMD Ryzen 7 3800X (8 Kerne, 16 Threads), 32 GB RAM, GTX 1080 Ti; Python-Prototyp, Priorität „unter normal“ |
| Skript | `align.py` im Scratchpad, gibt nur Zahlen aus; Audio, Datensatz und Modelle liegen nicht im Repository |

## Genauigkeit

Abstand zwischen ausgerichtetem und annotiertem Wortbeginn, alle 24 Songs:

| | Schätzung (seit 2026-10-28) | Option A | Option B |
|---|---|---|---|
| Median | 129 ms | 37 ms | 37 ms |
| Mittel | 268 ms | 145 ms | 86 ms |
| p90 | 713 ms | 152 ms | 119 ms |
| höchstens 150 ms daneben | 54,0 % | 89,9 % | 92,8 % |

Nach Sprache (Median / p90):

| Sprache | Schätzung | Option A | Option B |
|---|---|---|---|
| Deutsch | 70 / 306 ms | 30 / 103 ms | 38 / 100 ms |
| Englisch | 183 / 783 ms | 44 / 406 ms | 57 / 194 ms |
| Französisch | 155 / 808 ms | 38 / 118 ms | 32 / 138 ms |
| Spanisch | 152 / 829 ms | 36 / 88 ms | 27 / 77 ms |

Beide Optionen stolpern über denselben englischen Song (Median 124–143 ms). Option B hat weniger grobe Ausreißer: Ihr Mittel ist 86 ms, das von A 145 ms.

**Konfidenz taugt nicht als Filter.** Bei Option B sind selbst Zeilen mit geringer Konfidenz (mittlere Zeichenwahrscheinlichkeit) genauer als die Schätzung. Unsichere Zeilen auf die Schätzung zurückfallen zu lassen, verschlechtert das Ergebnis: Ab Schwelle 0,5 steigt p90 auf 190 ms, ab 0,7 auf 314 ms. Bei Option A ist es noch deutlicher.

## Rechenzeit und Speicher

Gemessen allein auf dem PC, ohne zweiten Lauf, je ein Song pro Sprache (13,7 min Audio):

| | 2 Threads | 4 Threads | Speicher (Prozess) |
|---|---|---|---|
| Option A (CPU) | – | 0,92 × Songlänge, davon 0,67 Trennung | 3,6 GB |
| Option B (CPU, int8) | 1,50 × Songlänge | 0,89 × Songlänge | 3,5 GB |
| Option B (GPU über DirectML, fp16, 1 Song) | 0,57 × Songlänge, dazu 1,9 CPU-Kerne ausgelastet | – | 1,55 GB |

- **Wo die Zeit steckt:** Bei Option B braucht die Vorverarbeitung unter 1 s pro Song. Fast die ganze Zeit liegt in ONNX Runtime selbst. Ein Rust-Plugin wäre auf der CPU also kaum schneller.
- **Fenstergröße:** Ganze 30-s-Fenster statt der kurzen Fenster pro Zeile waren sogar langsamer, 1,27 statt 0,89 × Songlänge mit 4 Threads.
- **Speicher von ONNX Runtime allein:** Die Sitzung mit dem int8-Modell belegt etwa 620 MB, während der Berechnung kommt bis etwa 1 GB dazu. Der Rest der 3,5 GB ist Python mit PyTorch und Transformers.
- **GPU-Messung:** Sie lief, während ein Spiel die Grafikkarte zu etwa 70 % auslastete. Ohne Spiel wäre die GPU vermutlich schneller. Gemessen ist das nicht.

## Mitschnitt von Spotify

Process Loopback auf den Prozessbaum von Spotify (`scratchpad/captest`, Rust mit der `windows`-Bibliothek), 10 s im RAM:

- **Aufnahme:** 9,97 s in 997 Paketen, kein Stille-Anteil.
- **Pause zwischen zwei Paketen:** höchstens 33 ms.
- **Pegel:** RMS −36,7 dBFS, Spitze −25,4 dBFS. Das ist echtes Musiksignal und hängt von der Lautstärke in Spotify ab.
- **Spielsound:** Dass er draußen bleibt, sichert die Windows-Schnittstelle zu. Mit einem laufenden Spiel geprüft ist das nicht.

Gespeichert wurde nichts.

## Bewertung gegen das Gate

| Kriterium | Ziel | Ergebnis | |
|---|---|---|---|
| Median Wortbeginn | ≤ 70 ms | 37 ms (B), 37 ms (A) | erfüllt |
| p90 Wortbeginn | ≤ 300 ms | 119 ms (B), 152 ms (A) | erfüllt |
| Rechenzeit mit 2 Threads | ≤ 0,6 × Songlänge | 1,50 (B, CPU); 0,57 (B, GPU, aber 1,9 Kerne) | auf der CPU verfehlt |
| Speicher | ≤ 1,5 GB | ONNX Runtime ~1,6 GB Spitze, Prototyp 3,5 GB | knapp verfehlt |
| Mitschnitt ohne Spielton | klappt | klappt, Spielton nicht geprüft | erfüllt |

**Empfehlung:** Option B. Sie ist genauer in den Ausreißern, steht unter MIT, braucht kein zweites Modell und kommt mit Kana und Kanji ohne Romaji aus. Die Rechenlast passt aber nicht dazu, neben Spielen nebenbei zu laufen. Auf der CPU braucht eine Stunde Musik etwa 0,9 Stunden Rechenzeit mit 4 Threads.

Machbar ist das nur, wenn das Plugin ausschließlich rechnet, wenn kein Vollbildspiel läuft, und die Songs bis dahin im RAM sammelt. Bei 16 kHz Mono sind das etwa 1,9 MB pro Minute, für 2 Stunden Musik also rund 230 MB. Ob P1–P4 unter dieser Bedingung gebaut werden, entscheidet der Nutzer. Nach den Regeln des Plans ist das Gate nicht erfüllt.

## Was diese Messung nicht zeigt

- **Echte Spotify-Songs:** Die Genauigkeit ist an CC-Musik von Jamendo gemessen. Bekannte Popsongs mit dichter Produktion oder Rap können schlechter abschneiden.
- **Japanisch:** Option B kennt Kana und Kanji, gemessen ist das nicht. Der Datensatz hat keine japanischen Songs.
- **Rust und quantisierte Modelle für Option A:** Die Zeiten von Option A gelten für fp32 in PyTorch. PotLyrics nennt für int8-ONNX etwa 0,45 × Songlänge mit 8 Threads.
- **GPU ohne Spiel und Auswirkung auf das Spiel:** Ob das Spiel während der Läufe geruckelt hat, ist nicht gemessen.
