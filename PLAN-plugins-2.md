# LyricHub – Plan: Auto-Sync und Plugin „Erkennen“

> Stand: 2026-10-31 · Basis: v1.2.0 · Zielversion: 1.3.0 · Vorgänger: [PLAN-plugins.md](PLAN-plugins.md)
> Status: **Q0 erledigt (2026-10-30, [docs/spike-recognize.md](docs/spike-recognize.md)). Q1–Q3 erledigt (2026-10-31, [docs/auto-sync.md](docs/auto-sync.md), [docs/plugin-recognize.md](docs/plugin-recognize.md)): Auto-Sync legt Zeilen im Median 38 ms an den Gesang, Recognize liefert Lyrics über Text ohne Zeitstempel, breitere Suche oder die Erkennung ab einer Sicherheit von 0,84.** Abweichungen vom Plan: kein eigenes Paket `audio-core`, der gemeinsame Code steckt in der Bibliothek von `plugins/align`; Aligns Fenster bleibt mit Auto-Sync bei 0,5 s, weil 1,5 s deutlich schlechter misst (3.1); der Knopf zum Verwerfen sitzt unter Einstellungen → Sync statt in der Diagnose (4.4). Dabei gefunden und behoben: Align 0.1.0 aus 1.2.0 lud sein Modell nach der ersten Minute ohne Arbeit nicht mehr. **Q4:** Doku, FAQ, Changelog und Lizenzhinweise erledigt; das Release 1.3.0 steht aus.

## 1. Kurzfassung

Zwei Erweiterungen auf der Plugin-Grundlage aus 1.2.0:

- **Auto-Sync** wird ein Schalter im Plugin Align. Ab dem zweiten Hören beginnt jede Zeile dort, wo ihr erstes Wort gesungen wird. Die Zeiten kommen aus Aligns Messung, es braucht weder einen zweiten Mitschnitt noch zusätzliche Rechenzeit.
- **Erkennen** (`lyrichub-recognize`) wird ein eigenes Plugin für Songs ohne synchrone Lyrics:
  1. Es erkennt den Text aus dem Gesang.
  2. Damit sucht es breiter in LRCLIB und wählt den passenden Kandidaten per Textvergleich.
  3. Findet sich nichts, zeigt es die erkannte Fassung als synchrone Lyrics an, sichtbar als „erkannt“ markiert.
  4. Songs mit Text ohne Zeitstempel richtet es direkt aus.

Vor dem Bau steht eine **Messung (Q0)**: Genauigkeit, Rechenzeit und Speicher an JamendoLyrics, dazu die Suche an den echten Fehltreffern des Nutzers.

## 2. Ausgangslage

| Was | Wert | Quelle |
|---|---|---|
| Songs im Cache des Nutzers | 47 gesehen: 24 synchron, 1 nur Text, 1 instrumental, **21 ohne Lyrics** (18 bei LRCLIB nicht gefunden) | `lyrichub.db`, 2026-10-30 |
| Song-Offsets, die der Nutzer gesetzt hat | keine | `song_overrides` |
| Versatz der LRC-Dateien | je Song im Median −140 … +151 ms, p95 nach Kalibrierung 289 ms | `docs/sync-report.md` |
| Erster Song mit Align | erstes Wort im Median 41 ms nach dem Zeilenstempel, p10 −99 ms, p90 +243 ms | `docs/plugin-align.md` |
| Align, Wortbeginn | Median 37 ms, p90 119 ms (24 Songs) | `docs/spike-align.md` |
| LRCLIB-Suche | `q` durchsucht nur Titel, Artist und Album, **nicht den Liedtext**; höchstens 20 Treffer; etwa 90 Anfragen pro Minute mit User-Agent | lrclib.net/docs.md |

Weil LRCLIB nicht im Liedtext sucht, heißt „suchen“ hier: breitere Anfragen nach Titelvarianten. Der erkannte Text entscheidet dann, welcher Kandidat wirklich der gehörte Song ist.

## 3. Auto-Sync (Schalter in Align)

### 3.1 Verhalten

Ist der Schalter an und hat eine Zeile Plugin-Wortzeiten, beginnt sie bei ihrem ersten Wort minus einem Vorlauf. Der Vorlauf wird in Q2 festgelegt, als Startwert 150 ms: So steht die Zeile kurz vor dem Gesang da, und die Füllung beginnt genau mit dem ersten Wort.

- **Grenzen:** höchstens ±1,5 s vom LRC-Stempel. Eine Zeile beginnt nie vor der vorigen; die vorige endet am neuen Beginn.
- **Zeilen ohne Messung** (Lücken im Mitschnitt): Sie verschieben sich um den Median der gemessenen Zeilen des Songs. Ob das besser ist als gar keine Verschiebung, zeigt Q0.
- **Song-Offset des Nutzers:** wirkt weiter zusätzlich, als Feinkorrektur.

### 3.2 Umsetzung

- **Ort:** Host-seitig beim Übernehmen der Plugin-Wörter (`plugins/words.rs`). Die Daten liegen dort schon; Align selbst ändert sich kaum.
- **Schalter:** Plugin-Einstellungen aus PLAN-plugins.md 4.7 werden jetzt gebaut. Das Manifest erklärt `settings` (zunächst nur `bool`, Texte in de/en), die Plugin-Seite zeichnet sie, der Host schickt die Werte mit `settings`. Auto-Sync liest der Host aus Aligns Einstellungen.
- **Sichtbarkeit:** Die Diagnose zeigt „Zeilen nach dem Gesang ausgerichtet“ und die mittlere Korrektur des Songs.

## 4. Plugin „Erkennen“ (`lyrichub-recognize`)

### 4.1 Ablauf

1. **Bedarf:** Ein Song hat bei LRCLIB keine synchronen Lyrics. Der Host meldet das mit `lyrics_wanted`, samt Metadaten und, falls vorhanden, dem Text ohne Zeitstempel.
2. **Mitschnitt:** wie bei Align, mit gemeinsamem Code (Abschnitt 5).
3. **Nach dem Song**, nur ohne Vollbildspiel:
   - **Text vorhanden** (nur ohne Zeitstempel): Text über den ganzen Song ausrichten, Zeilen und Wörter, mit dem Align-Modell.
   - **Kein Text:** Spracherkennung liefert Text mit Zeiten.
4. **Suche:** breitere LRCLIB-Anfragen mit bereinigtem Titel, nur dem Titel und Artist-Varianten. Der Text jedes Kandidaten wird mit der Erkennung verglichen (Zeichenfehlerrate). Liegt der beste unter einer Schwelle, gilt er: Die echten Lyrics werden übernommen und wie oben ausgerichtet.
5. **Sonst** wird die Erkennung selbst zu Lyrics. Zeilen entstehen an Pausen im Gesang. Quelle ist „erkannt“, und das Overlay zeigt ein kleines Kennzeichen.
6. **Ergebnis** geht an den Host-Cache und ist ab dem zweiten Hören sichtbar.

### 4.2 Modelle (Entscheidung nach Q0)

| Kandidat | Eigenschaften |
|---|---|
| A: Align-Modell, Greedy-Dekodierung | schon vorhanden, MIT, kein zweiter Download; ohne Sprachmodell, also mit Rechtschreibfehlern |
| B: Whisper large-v3-turbo | MIT, robust und mehrsprachig; groß (etwa 0,8–1,6 GB), langsamer auf der CPU, erfindet in Instrumentalteilen gern Text |
| C: Whisper small oder medium | kleiner und schneller, schwächer |
| optional: Gesangstrennung (htdemucs, MIT) vor B oder C | genauer, aber ein drittes Modell und mehr Rechenzeit |

Die Zeiten kommen in jedem Fall aus der CTC-Ausrichtung mit dem Align-Modell. Damit gilt dieselbe Genauigkeit wie bei Align.

### 4.3 Protokoll (neue Fähigkeit `lyrics_source`)

- **Host → Plugin:** `lyrics_wanted` mit `key`, `title`, `artist`, `album`, `duration_ms` und `plain` (Text ohne Zeitstempel oder `null`).
- **Plugin → Host:** `lyrics_found` mit `key`, `source` (`lrclib`, `aligned_plain` oder `recognized`), `lrclib_id` (falls übernommen) und `lines`: `[{ start_ms, text, words: [[start_ms, end_ms], …] }]`.
- **Prüfung im Host:** aufsteigende Zeiten, Länge im Rahmen des Songs, nur Text. Dann Speichern als Lyrics mit eigener Quelle und erneutes Senden an das Overlay.

### 4.4 Datenschutz und Recht

- Der Ton bleibt im Arbeitsspeicher. Erkannte Texte liegen nur im lokalen Cache und werden nie hochgeladen.
- LRCLIB bekommt wie bisher nur Metadaten, keinen Ton und keinen Text.
- Erkannte Lyrics sind als solche gekennzeichnet. Die Diagnose bietet „Erkannte Lyrics dieses Songs verwerfen“.

## 5. Gemeinsamer Code und Ressourcen

- **`plugins/audio-core`:** Mitschnitt, Zuordnung zur Songposition, Warteschlange, Modell-Download, Status und RPC wandern aus `plugins/align` in eine gemeinsame Bibliothek. Align und Erkennen nutzen sie.
- **Eine Rechnung zur Zeit:** Eine benannte Sperre `Local\LyricHub.PluginCompute` sorgt dafür, dass nie zwei Audio-Plugins gleichzeitig rechnen.
- **Grenzen wie bei Align:** nur ohne Vollbildspiel, 2 Threads bei Priorität „unter normal“, höchstens 1,5 GB samt Warteschlange. Ob Whisper das schafft, zeigt Q0.

## 6. Phasen und Abnahmekriterien

### Q0 – Messung (Gate, vor jedem Bau)

| Messung | Was | Gate |
|---|---|---|
| **M1 Auto-Sync** | Zeilenbeginn aus dem ersten ausgerichteten Wort gegen die Annotation, 24 Songs aus P0. Dazu der Versatz zwischen LRC und Messung bei den echten Songs des Nutzers. | Median ≤ 50 ms, p90 ≤ 200 ms |
| **M2 Erkennung** | Zeichen- und Wortfehlerrate (CER/WER) für A und B, je Sprache. Dazu Rechenzeit, Speicher und erfundener Text in Instrumentalteilen. | CER ≤ 25 %, Rechenzeit ≤ 1,6 × Songlänge mit 2 Threads, ≤ 1,5 GB |
| **M3 Text ohne Zeitstempel** | Den ganzen Liedtext über den ganzen Song ausrichten und den Zeilenbeginn gegen die Annotation messen. | Median ≤ 100 ms, p90 ≤ 500 ms |
| **M4 Suche** | Für die 18 Fehltreffer des Nutzers: Liefert eine breitere LRCLIB-Suche überhaupt Kandidaten mit passender Länge? Nur Zahlen, keine Titel. | Bericht |

**Entscheidung nach Q0:**

- Auto-Sync wird gebaut, wenn M1 das Gate schafft.
- Für Erkennen wird Weg „Text ausrichten“ gebaut, wenn M3 das Gate schafft, und Weg „Erkennung“, wenn M2 es schafft.
- Die Suche wird gebaut, wenn M4 Kandidaten findet.
- Bericht: `docs/spike-recognize.md` und englisch.

### Q1 – Gemeinsamer Code, Plugin-Einstellungen, Protokoll

- `plugins/audio-core` aus Align herauslösen; Align verhält sich danach unverändert, die P0-Werte gelten weiter.
- Plugin-Einstellungen (`bool`) im Manifest, auf der Plugin-Seite und mit `settings`.
- Fähigkeit `lyrics_source` im Host, mit Cache-Quelle und Prüfung.
- **Abnahme:** Align besteht seine Tests und den P0-Vergleich unverändert, und ein Testplugin liefert Lyrics über `lyrics_found`.

### Q2 – Auto-Sync

- Schalter in Align, Zeilenbeginn aus dem ersten Wort, Grenzen, Diagnose.
- **Abnahme:** An einem echten Song liegen die Zeilen beim zweiten Hören am Gesang. Der Versatz LRC gegen Messung ist nach Auto-Sync im Median unter 50 ms. Mit dem Schalter aus ist alles wie vorher.

### Q3 – Plugin Erkennen

- Plugin nach Abschnitt 4, nur die Wege, die Q0 freigibt.
- **Abnahme:** Ein Song ohne Lyrics hat beim zweiten Hören synchrone Lyrics, gefunden oder erkannt, korrekt gekennzeichnet. Es wird nicht gerechnet, solange ein Vollbildspiel läuft. Die Grenzen aus Abschnitt 5 halten.

### Q4 – Doku und Release 1.3.0

- `docs/plugins.md`, FAQ, CHANGELOG, Release-Notizen, Lizenzhinweise der Modelle.
- Release 1.3.0 mit dem neuen Plugin-ZIP, gebaut mit `tools/build-release.ps1`.

## 7. Risiken

| Risiko | Auswirkung | Gegenmittel |
|---|---|---|
| Erkennung fehlerhaft oder erfunden | falsche Lyrics | Gate in Q0, Kennzeichnung „erkannt“, Verwerfen-Knopf, Suche nach echten Lyrics zuerst |
| Auto-Sync bei schlechter Ausrichtung (Rap, Chöre) | Zeilen springen | Grenze ±1,5 s, Plausibilität über den Median des Songs, Schalter |
| Whisper zu groß oder zu langsam | Speicher- oder Zeitgrenze verfehlt | Kandidaten A und C; Whisper nur, wenn Q0 es freigibt |
| Zwei Audio-Plugins rechnen gleichzeitig | doppelte Last | gemeinsame Sperre |
| Urheberrecht erkannter Texte | rechtlich heikel bei Verbreitung | nur lokal, nie hochgeladen, nur zur eigenen Anzeige |

## 8. Arbeitsregeln für die Umsetzung

- Deutsch mit dem Nutzer. Code, Bezeichner und Commit-Nachrichten Englisch. Doku auf Deutsch und Englisch.
- Keine echten Liedtexte in Tests, Fixtures, Logs, Berichten oder im Chat. Messungen geben nur Zahlen aus. Auch erkannte Texte werden nie ausgegeben.
- Kein Ton auf die Festplatte. Einzige Ausnahme ist das CC-Audio von JamendoLyrics im Scratchpad.
- Vor langen CPU- oder GPU-Läufen prüfen, ob ein Spiel läuft. Eingriffe am PC (Spotify steuern, Mitschnitt-Test, Vollbild) nur nach OK.
- Committen und pushen nur auf Anfrage, Git-Identität HeiligerHoly mit noreply-Adresse.
- Releases nur mit `tools/build-release.ps1`.
- Behauptungen über Genauigkeit oder Last nur mit Messwert. Jede Phase endet mit einem kurzen Bericht in `docs/`.

## 8a. Ergebnis von Q0 (2026-10-30)

| Messung | Ergebnis | Gate |
|---|---|---|
| M1 Zeilenbeginn aus dem ersten Wort (24 Songs) | Median 37 ms, p90 138 ms | erfüllt |
| M2 Erkennung A (Align-Modell, Greedy) | CER 23,9 %, WER 43,9 %, 1,86 × Songlänge, 1,50 GB, nichts erfunden | Geschwindigkeit verfehlt |
| M2 Erkennung B (Whisper turbo) | CER 21,4 %, WER 36,4 %, 0,90 × Songlänge, 2,15 GB, 4,3 % erfunden | Speicher verfehlt |
| M3 Text ohne Zeitstempel ausrichten | Zeilenbeginn Median 41 ms, p90 197 ms | erfüllt |
| M4 breitere Suche (19 Fehltreffer) | Kandidaten mit Länge ±3 s für 7, vom selben Artist für 4; 6 der Fehltreffer sind nur vorübergehend (Überlast) | Kandidaten vorhanden |

Empfehlung im Bericht: Auto-Sync bauen; Erkennen mit Suche, Ausrichtung und Kandidat A samt Sicherheitsschwelle; im Kern eine Stufe „nur Titel“ und kürzeres Aufgeben bei Überlast.

## 9. Entscheidungen (2026-10-30)

1. **Erkennen:** erkennen, damit suchen, sonst die erkannte Fassung anzeigen, sichtbar als „erkannt“ markiert.
2. **Auto-Sync:** ab dem zweiten Hören, aus Aligns Messung, ohne zusätzliche Rechenzeit.
3. **Paket:** Auto-Sync als Schalter in Align; Erkennen als eigenes Plugin; gemeinsamer Code für Mitschnitt und Warteschlange.
4. **Reihenfolge:** erst messen (Q0), gebaut wird nur, was die Grenzen schafft.
5. **Nach Q0 (2026-10-30):** Q1–Q4 werden wie empfohlen gebaut. Auto-Sync in Align. Erkennen mit breiterer Suche samt Textvergleich, Ausrichtung echter Lyrics und von Text ohne Zeitstempel. Erkannte Lyrics kommen von Kandidat A (Align-Modell) und erscheinen nur oberhalb einer Sicherheitsschwelle.
6. **Kern:** Die LRCLIB-Suche bekommt eine Stufe „nur Titel“ mit Artist-Abgleich. Bei Überlast versucht LyricHub es nach kurzer Zeit erneut, statt einen Tag aufzugeben.
