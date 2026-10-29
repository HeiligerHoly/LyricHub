# Plugin Align (P3 aus PLAN-plugins.md)

> Datum: 2026-10-29 · nach v1.1.0 · Bezug: [PLAN-plugins.md](../PLAN-plugins.md) Abschnitte 5 und 6, [docs/spike-align.md](spike-align.md), [docs/plugin-host.md](plugin-host.md)
> Ergebnis: **lyrichub-align liefert echte Wortzeiten für Spotify-Songs. Die Rust-Pipeline trifft die Werte des Python-Prototyps aus P0 (Median 48 statt 49 ms). Im Leerlauf belegt das Plugin 31 MB, beim Mitschnitt 0,03 % CPU, beim Rechnen 2 Kerne und unter 1,5 GB, und solange ein Vollbildspiel läuft, rechnet es nicht.**

## Was gebaut ist

`plugins/align/` ist ein eigenes Cargo-Paket. Daraus entsteht eine einzige `lyrichub-align.exe` (24 MB, als ZIP 8,4 MB). ONNX Runtime ist fest eingebaut, es braucht also keine DLL.

| Teil | Umsetzung | Code |
|---|---|---|
| Mitschnitt | Process Loopback auf den Prozessbaum des Players, 48 kHz, 16 Bit, Stereo zu Mono gemischt. Jedes Paket trägt seine QueryPerformanceCounter-Zeit. Läuft nur, solange ein Song mit geschätzten Wortzeiten spielt. | `capture.rs` |
| Zuordnung | Die `playback`-Anker von LyricHub legen jedes Paket auf die Lyrics-Uhr (Position minus Offset), nach der das Overlay die Zeilen zeigt. Pakete, die höchstens 30 ms vom letzten Ende abweichen, schließen nahtlos an. Spulen und Pause lassen Lücken. Eine Zeile wird ausgerichtet, wenn 95 % ihres Fensters gehört wurden, ein Song, wenn das für 85 % seiner Zeilen gilt. | `recorder.rs` |
| Warteschlange | Gehörte Songs in 16 kHz und 16 Bit, höchstens 256 MB (gut 2 Stunden Musik), dann fällt der älteste weg. Der Ton bleibt im Arbeitsspeicher. | `worker.rs` |
| Ausrichtung | Wie in P0: ein Fenster pro Zeile von 0,5 s vor ihrem Start bis 0,5 s nach dem Start der nächsten, höchstens 28 s. Fenster über 14 s gehen in Stücken mit je 2 s Kontext ans Modell. Darin stecken die Merkmale von SeamlessM4T (nachgebaut), das Modell w2v-bert-2.0 music-lyrics CTC (int8, 2 Threads) und der CTC-Viterbi-Pfad (nachgebaut nach torchaudio). Wortbeginn ist das erste Zeichen. Ein Wort dauert bis zum nächsten, außer es liegt mehr als 1 s Pause dazwischen. | `features.rs`, `align.rs`, `ctc.rs`, `text.rs`, `resample.rs` |
| Zeitplan | Gerechnet wird nur, solange kein Vollbildfenster im Vordergrund ist (Meldung `screen` von LyricHub). Die Pause greift zwischen zwei Zeilen. Das Modell (0,6 GB) wird erst bei Arbeit geladen und nach 60 s ohne Arbeit entladen. | `worker.rs` |
| Modell | Beim ersten Start von huggingface.co, festgelegt auf eine Revision, mit Fortsetzen nach Abbruch und SHA-256-Prüfung, nach `%LOCALAPPDATA%\LyricHub\plugins\lyrichub-align\model` (588 MB). | `model.rs` |
| Status | Eine Zeile auf der Plugin-Seite, zum Beispiel „Lädt das Modell: 34 %“, „Hört mit“, „Richtet einen Song aus: 45 %“ oder „Pausiert, solange ein Vollbildspiel läuft“, dazu ein Fortschrittsbalken. | `status.rs` |

**Änderungen am Host**

- `playback` enthält jetzt `offset_ms`.
- `lyrics` enthält jetzt `word_source`. Songs mit schon gemessenen Zeiten schneidet das Plugin nicht mit.
- Neue Meldung `screen` mit `fullscreen`, sobald ein Vollbildfenster in den Vordergrund kommt oder ihn verlässt.
- Die letzte Zeile eines Songs darf Wörter bis 30 s nach ihrem Start haben.
- Ein Plugin, das gerade beendet wurde, lässt sich ersetzen und entfernen, auch wenn Windows seine exe noch kurz sperrt. Der alte Ordner wird zuerst als Ganzes beiseitegeschoben.

## Prüfung

### Unit-Tests (`cargo test` in `plugins/align`)

29 Tests:

- **Referenzwerte:** Resampler, Merkmale und CTC-Pfad gegen Werte aus transformers und torchaudio. Die Werte erzeugt `tests/make_golden.py` mit synthetischem Ton.
- **Text:** Normalisierung samt Hangul-Zerlegung.
- **Fenster:** Fenster, Stücke und Wortzeiten.
- **Rekorder:** Abdeckung, Offset, Pause, Spulen und Zeitschwankungen.
- **Sonstiges:** Statuszeilen und Modelldateien.

Ein weiterer Test lädt die Vokabeldatei herunter und setzt einen abgebrochenen Download fort. Er braucht das Netz und läuft deshalb nur mit `--ignored`.

### Vergleich mit P0

`examples/jamendo.rs` lässt die Plugin-Pipeline wie der Prototyp über die vier Songs aus P0 laufen, je einen auf Deutsch, Englisch, Französisch und Spanisch (13,7 min). Der Ton läuft dabei wie in der Warteschlange über 16 Bit. Das Ergebnis gibt den Abstand des Wortbeginns zur Annotation an, mit 2 Threads:

| Sprache | Median (P0) | p90 (P0) |
|---|---|---|
| Deutsch | 44 ms (44) | 103 ms (103) |
| Englisch | 145 ms (143) | 659 ms (659) |
| Französisch | 45 ms (46) | 683 ms (694) |
| Spanisch | 25 ms (26) | 76 ms (76) |
| **alle** | **48 ms (49)** | **264 ms (244)** |

Die Tabelle zeigt die Fenster am Stück wie in P0, Rechenzeit 1,55 × Songlänge. Mit den Stücken für Fenster über 14 s (siehe Speicher) bleibt der Median bei 48 ms. p90 steigt auf 271 ms, nur beim englischen Song von 659 auf 751 ms. 82,8 % der Wörter liegen höchstens 150 ms daneben (P0: 83,8 %). Die Rechenzeit sinkt auf 1,39 × Songlänge und der Spitzenspeicher des Prüfprogramms von 3,12 auf 1,33 GB.

Gefordert waren höchstens 10 ms Abweichung im Median.

### Speicher

Das Modell nach dem Laden belegt 0,6 GB. Dazu kommt Speicher, der quadratisch mit der Fensterlänge wächst. Gemessen mit synthetischem Rauschen, 2 Threads:

| Fenster | Rechenzeit | Spitze am Stück | Spitze in 14-s-Stücken |
|---|---|---|---|
| 5 s | 6,4 s | 686 MB | 686 MB |
| 10 s | 13,8 s | 888 MB | 888 MB |
| 15 s | 21–26 s | 1268 MB | 1034 MB |
| 20 s | 30–34 s | 1732 MB | 1039 MB |
| 28 s | 45–50 s | 2808 MB | 1161 MB |

Nach jedem Lauf gibt ONNX Runtime den Speicher zurück (`memory.enable_memory_arena_shrinkage`). Ohne das bliebe die Spitze des längsten Fensters belegt. Zusammen mit der Warteschlange von 256 MB bleibt das Plugin unter 1,5 GB.

### In der echten App (Spotify, 2026-10-29)

1. **Installation:** ZIP installiert und eingeschaltet. Das Modell kam in unter einer Minute, die Prüfsumme stimmte. Im Leerlauf belegt das Plugin 31 MB.
2. **Mitschnitt:** 0,03 % CPU des ganzen PCs (0,55 % eines Kerns), 16 MB.
3. **Erster Song:** 193 s Ton, 68 Zeilen. Gerechnet hat das Plugin 217 s, darin 15 s Vollbild-Pause. Alle 68 Zeilen kamen gültig bei LyricHub an.
4. **Rechnen:** 1,94 Kerne bei Priorität „unter normal“, 710–775 MB, das war noch vor den Stücken.
5. **Vollbild:** Ein randloses Fenster über den ganzen Bildschirm ließ die Last innerhalb von 3 s auf null fallen, die Statuszeile zeigte die Pause. Nach dem Schließen ging es sofort weiter.
6. **Zweites Hören:** Alle 474 Wörter des Songs kamen vom Plugin. Im Overlay leuchteten die Wörter einzeln, höchstens 2 gleichzeitig, und keine Animation sprang zurück. Den Song schnitt das Plugin kein zweites Mal mit.
7. **Plausibilität:** Das erste Wort einer Zeile beginnt im Median 41 ms nach dem Zeilenstart der LRC-Datei (10 % mehr als 99 ms davor, 10 % mehr als 243 ms danach). Die Wörter waren aber nur 81 ms lang (Median), weil CTC jedes Zeichen nur kurz markiert. Seitdem dauert ein Wort bis zum nächsten.

**Gefunden und behoben:**

- **Absturz beim Songwechsel:** Das Plugin stürzte jedes Mal ab, wenn der Mitschnitt endete. Die Ursache war ein `PROPVARIANT` mit Zeiger auf eine lokale Variable: Beim Aufräumen gab Windows diesen Speicher frei, und der Heap war beschädigt.
- **Neuinstallation ohne Wirkung:** Nach dem Absturz hielt die Windows-Fehlerberichterstattung die alte exe noch einige Sekunden gesperrt. Der Host konnte sie beim Neuinstallieren nicht ersetzen. Jetzt wartet er bis zu 5 s.

## Grenzen

- **Ausgabelatenz:** Der Mitschnitt kommt vor dem Ausgabegerät. Die Wörter leuchten deshalb um dessen Latenz zu früh. Bei Kabel sind das etwa 10–40 ms, bei Bluetooth deutlich mehr.
- **Nur ganz gehörte Songs:** Ausgerichtet wird nur, was zu 85 % gehört wurde. Zeilen in Lücken bleiben geschätzt, auch beim nächsten Hören.
- **Offset danach geändert:** Die Wortzeiten hängen am Offset, der beim Mitschnitt galt. Wer danach den Song-Offset ändert, verschiebt die Wörter gegen den Gesang.
- **Japanisch und Koreanisch:** Das Modell kennt Kana, Kanji und Jamo, gemessen ist das aber nicht.
- **Rechenzeit:** 1,1–1,6 × Songlänge mit 2 Threads. Wer viel hört und viel spielt, sammelt Songs in der Warteschlange. Nach gut 2 Stunden fällt der älteste weg.
