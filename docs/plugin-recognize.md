# Plugin Recognize (Q1 und Q3 aus PLAN-plugins-2.md)

> Datum: 2026-10-31 · nach v1.2.0 · Bezug: [PLAN-plugins-2.md](../PLAN-plugins-2.md) Abschnitte 4–6, [docs/spike-recognize.md](spike-recognize.md), [docs/plugin-align.md](plugin-align.md)
> Ergebnis: **Recognize liefert Lyrics für Songs ohne synchrone Lyrics: Text ohne Zeitstempel richtet es über den ganzen Song aus, sonst sucht es breiter bei LRCLIB und vergleicht mit dem Gehörten, und erst zuletzt zeigt es die Erkennung selbst, gekennzeichnet als „erkannt“. Die Schwelle dafür steht nach 12 Songs bei 0,84: Darüber hatte kein Song mehr als 15 % falsche Zeichen. Live rechnet es mit 2 Threads bis etwa zum Doppelten der Songlänge und bleibt unter 1,5 GB.**

Englisch: [docs/en/plugin-recognize.md](en/plugin-recognize.md)

## Was gebaut ist

`plugins/recognize/` ist ein eigenes Cargo-Paket. Daraus entsteht `lyrichub-recognize.exe` (24 MB, als ZIP 8,3 MB). Mitschnitt, Modell, Warteschlange, Status und Protokoll teilt es sich mit Align: Sie stecken in der Bibliothek von `plugins/align`, die Recognize einbindet. Ein eigenes Paket `audio-core`, wie im Plan, war dafür nicht nötig.

| Teil | Umsetzung | Code |
|---|---|---|
| Bedarf | LyricHub schickt `lyrics_wanted`, sobald ein Song ohne synchrone Lyrics läuft (Zustand „nicht gefunden“ oder „nur ohne Zeitangaben“), mit Titel, Artist, Album, Länge und dem Text ohne Zeitstempel, falls LRCLIB ihn hat. | `src-tauri/src/plugins/mod.rs` |
| Mitschnitt | Der ganze Song, wie bei Align nur der Ton des Players. Er zählt, wenn 85 % gehört wurden; Songs über 15 Minuten bleiben außen vor. Ein Song mit zu wenig Gehörtem steht im Log („song left out: 25 % of it heard“). | `plugins/align/src/recorder.rs` |
| Zeitplan | Wie Align: Warteschlange bis 256 MB, 2 Threads bei „unter normal“, Pause zwischen zwei Stücken, solange ein Vollbildfenster vorne ist. Es rechnet immer nur ein Audio-Plugin (benannte Sperre `Local\LyricHub.PluginCompute`). Das Modell wird für die Arbeit geladen und freigegeben, sobald nichts mehr wartet, bevor die Sperre frei wird. | `plugins/recognize/src/worker.rs`, `plugins/align/src/lock.rs` |
| Modell | Dasselbe wie bei Align. Ist es dort schon vollständig, übernimmt Recognize es als Hardlinks, umgekehrt genauso: Auf der Festplatte liegt es einmal, und das Entfernen eines Plugins lässt das Modell des anderen ganz. | `plugins/align/src/model.rs` (`share_from`) |
| Modellausgabe | Einmal für den ganzen Song, in Stücken von 14 s mit 2 s Kontext. Alle drei Wege nutzen dieselbe Ausgabe. | `plugins/align/src/align.rs` |
| Weg 1: Text ohne Zeitstempel | Der ganze Text auf einmal über den ganzen Song ausgerichtet (CTC). Eine Zeile beginnt 150 ms vor ihrem ersten Wort. | `plugins/align/src/recognize.rs` (`align_whole`) |
| Weg 2: breitere Suche | LRCLIB `/api/search` mit dem bereinigten Titel, als `track_name` und als `q`, ohne Artist; höchstens alle 0,5 s, mit der vom Server verlangten Wartezeit. Sie läuft vor der Rechnung, damit ein langsames LRCLIB nie die Sperre hält. Kandidaten müssen ±10 s zur Songlänge passen. Der Textvergleich zählt, wie viele der erkannten Wörter mit drei oder mehr Buchstaben im Kandidaten vorkommen; ab 45 % gilt der beste. Synchrone Kandidaten richtet Recognize Zeile für Zeile aus wie Align, Text ohne Zeitstempel wie in Weg 1. | `plugins/recognize/src/lrclib.rs`, `worker.rs` |
| Weg 3: Erkennung | Greedy-Dekodierung der Modellausgabe zu Wörtern: Ein Wortende ist das Trennzeichen des Modells oder eine Pause über 400 ms. Zeilen entstehen an Pausen ab 700 ms oder nach 8 Wörtern. Angezeigt wird das nur ab einer Sicherheit von 0,84 und ab 12 Wörtern (Abschnitt „Schwelle“), und nur, wenn die Suche gelaufen ist: War LRCLIB nicht erreichbar, wartet der Song auf den nächsten Start des Plugins, damit erkannter Text keine echten Lyrics im Cache verdeckt. | `plugins/align/src/recognize.rs`, `worker.rs` |
| Antwort | `lyrics_found` mit Quelle (`aligned_plain`, `lrclib` oder `recognized`), LRCLIB-ID und den Zeilen samt Wortzeiten. | `worker.rs` |

**Änderungen am Host (Q1)**

- **Fähigkeit `lyrics_source`:** `lyrics_wanted` hin, `lyrics_found` zurück; Lyrics nimmt LyricHub nur von Plugins an, die diese Fähigkeit erklären. LyricHub prüft die Zeilen (aufsteigende Starts, höchstens 600 Zeilen zu je 300 Zeichen, Steuerzeichen raus) und legt sie als LRC in den Cache, mit der Quelle `plugin:<id>:<weg>`. Synchrone Lyrics von LRCLIB gewinnen immer. Die Wortzeiten landen wie bei Align im Cache, und läuft der Song gerade, zeigt das Overlay die Lyrics sofort.
- **Kennzeichnung:** Bei erkannten Lyrics steht im Overlay neben dem Titel „erkannt“. Einstellungen → Sync → Lyrics und die Diagnose zeigen, woher die Lyrics kommen.
- **Verwerfen:** Einstellungen → Sync → „Lyrics vom Plugin verwerfen“ löscht die Lyrics des Plugins für diesen Song und fragt LRCLIB neu. Der Song kommt in die Tabelle `plugin_rejects`; LyricHub bittet für ihn kein Plugin mehr und nimmt keine Plugin-Lyrics mehr an. Wird ein Plugin entfernt, gehen auch seine Lyrics.
- **Kern:** Eine letzte LRCLIB-Stufe sucht nur nach dem Titel, der Abgleich prüft weiter Artist und Länge. Bleibt LRCLIB nach allen Wiederholungen unerreichbar, gilt ein Song zwei Stunden statt einen Tag als fehlend.

## Prüfung

### Unit-Tests

- **Bibliothek** (`cargo test` in `plugins/align`, 39 Tests): unter anderem Greedy-Dekodierung, Wortenden und Zeilen, Textvergleich und das Ausrichten über den ganzen Song an synthetischen Modellausgaben; der Mitschnitt ganzer Songs; die Rechensperre; das Teilen des Modells.
- **Recognize** (`cargo test` in `plugins/recognize`, 2 Tests und einer mit Netz): bereinigte Titel für die Suche und LRC-Zeilen; mit `--ignored` eine echte Suche bei lrclib.net.
- **Host** (`cargo test` in `src-tauri`): Prüfung von `lyrics_found` und Umwandlung in LRC, Verwerfen und Vergessen von Plugin-Lyrics im Cache, die Stufe „nur Titel“ mit Artist-Abgleich.

### Schwelle für erkannte Lyrics

Die Sicherheit ist die mittlere Wahrscheinlichkeit, die das Modell den erkannten Zeichen gibt. Gemessen an 12 JamendoLyrics-Songs (je drei auf Deutsch, Englisch, Französisch und Spanisch, Pop, Rock, Hip-Hop, Jazz und Reggae) mit dem Prüfprogramm `--example recognize`, sortiert nach Sicherheit:

| Sicherheit | Sprache | Zeichen- / Wortfehler |
|---|---|---|
| 0,941 | Spanisch | 8,1 / 18,3 % |
| 0,905 | Französisch | 7,4 / 20,3 % |
| 0,902 | Englisch | 15,3 / 47,5 % |
| 0,892 | Deutsch | 8,0 / 20,3 % |
| 0,892 | Deutsch | 5,6 / 17,8 % |
| 0,869 | Spanisch | 8,8 / 35,8 % |
| 0,842 | Deutsch | 8,1 / 20,2 % |
| *Schwelle 0,84* | | |
| 0,828 | Französisch | 31,4 / 53,4 % |
| 0,797 | Französisch | 34,0 / 48,7 % |
| 0,789 | Spanisch | 20,0 / 41,2 % |
| 0,709 | Englisch | 32,1 / 62,5 % |
| 0,644 | Englisch | 46,2 / 75,1 % |

- **Die Schwelle steht bei 0,84.** Alle 7 Songs darüber haben höchstens 15 % falsche Zeichen. Die vier Songs mit mehr als einem Viertel falscher Zeichen liegen alle unter 0,83.
- **Der vorläufige Wert 0,75 war zu niedrig:** Er hätte 10 Songs gezeigt, darunter zwei, bei denen ein Drittel der Zeichen falsch ist.
- **Preis der Schwelle:** Ein brauchbarer spanischer Song (20 % Zeichenfehler) bleibt bei 0,789 außen vor. Ohne Lyrics zeigt LyricHub dann wie bisher die kompakte Karte.
- **Auch über der Schwelle ist etwa jedes fünfte Wort falsch,** beim englischen Song fast jedes zweite. Meist fehlt nur ein Buchstabe, und ein Wort mit einem falschen Buchstaben zählt schon als falsch. Deshalb sind erkannte Lyrics als solche gekennzeichnet und lassen sich verwerfen.
- **Text ohne Zeitstempel (Weg 1)** traf an denselben 12 Songs den Zeilenbeginn je Song im Median auf 18–120 ms, p90 46–297 ms. Das bestätigt M3 aus Q0 an dreimal so vielen Songs.
- **Rechenzeit des Prüfprogramms:** im Mittel das 1,24-Fache der Songlänge mit 4 Threads, während die Plugins nebenher rechneten.

### In der echten App

Getestet mit Spotify und der Playlist des Nutzers, 2026-10-30/31, Align und Recognize gleichzeitig installiert. Während der Tests liefen nebenher die Messungen an JamendoLyrics mit 4 Threads; die Rechenzeiten unten sind deshalb eher zu lang.

| Song | Weg | Ergebnis | Rechenzeit |
|---|---|---|---|
| ohne Lyrics bei LRCLIB, 207 s | Erkennung | 403 Wörter, Sicherheit 0,78, als „erkannt“ gezeigt (67 Zeilen mit Wortzeiten). Mit der Schwelle 0,84 bliebe der Song ohne Lyrics. | 455 s (2,2 × Songlänge) |
| nur Text ohne Zeitstempel, 214 s | Text ausrichten | 66 Zeilen, alle mit Wortzeiten | 403 s (1,9 × Songlänge) |
| ohne Lyrics bei LRCLIB, Titel mit Zusätzen in Klammern, 92 s | breitere Suche | Erkennung mit Sicherheit 0,86; 15 Kandidaten, der beste mit 89 % der erkannten Wörter. Übernommen: seine synchronen Lyrics, 39 Zeilen, alle mit Wortzeiten. | 214 s, darin 40 s Pause für den Vollbild-Test |

- **Die Suche war beim ersten Song kaputt** und wurde dabei gefunden. Das Plugin prüfte das Zertifikat von lrclib.net gegen die Stammzertifikate, die ureq mitbringt; die Kette, die Windows für lrclib.net baut, endet aber in keinem davon, und jede Anfrage scheiterte. Jetzt nutzt das Plugin die Stammzertifikate von Windows, wie LyricHub selbst. Eine echte Suche findet damit Kandidaten (37 für einen häufigen Titel, Test mit `--ignored`). Außerdem gibt es seitdem keine erkannte Fassung mehr, wenn die Suche scheitert.
- **Beim zweiten Hören** des erkannten Songs kamen die 67 Zeilen samt Wortzeiten aus dem Cache. Das Overlay zeigte „erkannt“, Einstellungen → Sync die Quelle „Lyrics vom Plugin, aus dem Gesang erkannt“ und den Knopf zum Verwerfen. Ein Klick darauf löschte die Lyrics, LyricHub fragte LRCLIB neu (nichts gefunden), und für den Song wurde kein Plugin mehr gefragt. (Die Sperre wurde danach von Hand wieder entfernt, damit das reparierte Plugin den Song neu versucht.)
- **Rechensperre:** Align und Recognize wechselten sich ab. Wer wartete, zeigte „Wartet, bis ein anderes Plugin fertig gerechnet hat“. Nach jeder Rechnung gab das Plugin sein Modell sofort frei, bevor das andere an die Reihe kam.
- **Speicher:** im Leerlauf 10 MB, beim Rechnen bis 1,2 GB.
- **Modell:** Recognize übernahm Aligns Modell als Hardlinks (Linkzähler 2), ohne Download.
- **Halb gehörte Songs** stehen im Log, etwa „song left out: 75 % of it heard, 85 % needed“, wenn das Plugin mitten im Song neu startete.
- **Vollbild:** Ein schwarzes Vollbildfenster für 40 s, während Recognize rechnete. Es lief mit 1,9 Kernen weiter, bis das laufende Stück nach 16 s fertig war, dann mit 0,00 Kernen, und die Statuszeile zeigte „Pausiert, solange ein Vollbildspiel läuft“. Nach dem Fenster rechnete es weiter.

## Grenzen

- **Erst ab dem zweiten Hören,** und nur für Songs, die zu 85 % gehört wurden.
- **Erkannter Text hat Fehler:** auch über der Schwelle etwa jedes fünfte Wort, beim englischen Testsong fast jedes zweite. Unter der Schwelle bleibt es bei der kompakten Karte.
- **Nur 12 Testsongs** für die Schwelle. Dicht produzierter Pop und Rap von Spotify sind womöglich schwerer als die CC-Musik von Jamendo.
- **Bessere Lyrics erst nach 14 Tagen:** Lyrics eines Plugins liegen im Cache wie gefundene Lyrics. Sind sie 14 Tage alt, fragt LyricHub beim nächsten Hören LRCLIB erneut (IDEEN.md, Paket A). Bekommt LRCLIB früher synchrone Lyrics, sieht LyricHub das vorher nur nach „Lyrics neu laden“ oder „Lyrics vom Plugin verwerfen“.
- **Japanisch und Koreanisch:** nicht gemessen.
- **Ausgabelatenz und Song-Offset** wirken wie bei Align ([docs/plugin-align.md](plugin-align.md)).
