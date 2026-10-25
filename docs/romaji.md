# Romaji für japanische Zeilen

> Datum: 2026-10-25 · Bezug: PLAN.md 13.6, Wunsch aus dem Hörtest (`docs/phase2.md`), PLAN.md 2 und 14.2
> Ergebnis: **Romanisierung im Backend** mit lindera 6.2 und eingebettetem IPADIC, eigene Hepburn-Regeln in Rust. Schalter `appearance.romaji` (Standard aus), Strg+Alt+R.

## Entscheidung: Backend statt Frontend

Gemessen auf dem PC des Nutzers (Windows 11, 16 Kerne), Release-Build für lindera, Node 24 für kuromoji. Alle Testsätze sind erfunden.

| Kriterium | **lindera 6.2 + IPADIC (Rust)** | kuroshiro 1.2.1 + kuromoji 0.1.2 (Frontend) | lindera + UniDic 2.1.2 | lindera + SudachiDict |
|---|---|---|---|---|
| Lizenz Code | MIT | MIT, Apache-2.0 | MIT | MIT |
| Lizenz Wörterbuch | IPADIC (NAIST/ICOT, BSD-artig, siehe unten) | IPADIC (dieselbe Datei) | BSD-3-Clause | Apache-2.0 |
| Größe | Binary +45 MB (unkomprimiert eingebettet), komprimiert etwa 7 MB (xz) | 17 MB Wörterbuch (gzip) in den Assets | Binary +195 MB | 200 MB schon als Download |
| Privater Speicher | **+0,3 MB**; das Wörterbuch bleibt im gemappten Image, das Working Set wächst nach 200 Zeilen um 2 MB | **+310 MB RSS** (ArrayBuffers 222 MB, Heap 84 MB), und zwar im Overlay-WebView | nicht gemessen | nicht gemessen |
| Startzeit | Wörterbuch 0,7 ms, 200 Zeilen 2,3 ms | Wörterbuch 272 ms (lokale Datei, ohne `fetch`) | 1 ms | nicht gemessen |
| Lesungen | Lesung und Aussprache getrennt: は/へ/を, lange Vokale und „ei“ lassen sich sauber trennen | dasselbe IPADIC, aber die Ausgabe hat Fehler: „tsu te“ für って, „kikoe ta“, „ki mashi ta“ | nur die Aussprache der Oberfläche: 先生 → „sensē“, お母さん als お+母+さん → „ohahasan“ | keine Aussprache, nur Lesung |

Begründung:

1. **Speicher entscheidet.** PLAN.md 2 erlaubt unter 200 MB privat für die ganze App. kuromoji allein braucht mehr, und das im Overlay-Prozess, der dauerhaft läuft. lindera 6 bettet das Wörterbuch unkomprimiert ein und liest es direkt aus dem Image. Diese Seiten sind dateigestützt und teilbar, kein privater Speicher, und es wird nur geladen, was gebraucht wird.
2. **Einmal pro Song, nicht pro Frame.** Das Backend rechnet, wenn die Lyrics eines Songs ankommen. Das Ergebnis liegt bei den Lyrics des aktuellen Songs (`app_state`), das Overlay bekommt fertige Strings mit dem `lyrics`-Event. Refrains werden einmal umgewandelt. Kein JavaScript und kein IPC pro Frame.
3. **Qualität.** Nur IPADIC liefert Lesung und Aussprache getrennt, beides braucht Hepburn: Lange Vokale (トーキョー) und die Partikel (ワ für は) stehen in der Aussprache, „sensei“ und „iu“ (言う) in der Lesung. kuroshiro nutzt dasselbe Wörterbuch, verbindet die Wörter aber falsch. Die eigenen Regeln sind unten beschrieben und getestet.
4. **Testbar wie der Rest der Pipeline.** Rust-Unit-Tests mit `cargo test`; das Frontend hat keinen Test-Runner.
5. **Startzeit.** Das Wörterbuch lädt erst beim ersten japanischen Song mit eingeschaltetem Schalter, in unter 1 ms. Der App-Start bleibt unverändert.

### Lizenz von IPADIC (Abweichung von PLAN.md 14.2, zur Kenntnis)

PLAN.md 14.2 nennt MIT, Apache-2.0 und BSD. IPADIC steht unter einer eigenen, BSD-artigen Lizenz von NAIST mit den Bedingungen der ICOT Free Software: Nutzung, Änderung und Weitergabe sind frei. Einzige Bedingung: Der Gewährleistungsausschluss muss beiliegen. Dieselbe Datei liefern kuromoji.js (Apache-2.0) und der Kuromoji-Analyzer von Apache Lucene aus. Ein Wörterbuch mit Kanji-Lesungen unter MIT, Apache-2.0 oder BSD gibt es nur in groß: UniDic (BSD-3) braucht viermal so viel Platz und liest für Hepburn schlechter, SudachiDict (Apache-2.0) ist noch größer und hat keine Aussprache. Der Hinweistext steht in `THIRD_PARTY_NOTICES.md` und muss bei einer Veröffentlichung mit.

Alle Rust-Abhängigkeiten, die lindera zur Laufzeit mitbringt, stehen unter MIT, Apache-2.0, BSD-3-Clause oder Unlicense/MIT. Crates mit ISC- oder CDLA-Lizenz (`ring`, `rustls-webpki`, `webpki-roots`) braucht nur das Build-Skript für den Download, im Binary landen sie nicht.

### Build

Das Build-Skript von `lindera-ipadic` lädt beim ersten Build einmal `mecab-ipadic-2.7.0-20250920.tar.gz` von `lindera.dev` (MD5 geprüft) und baut daraus das Wörterbuch, etwa 45 s. Danach liegt es in `target/`. Für Builds ohne Netz kann `LINDERA_BUILD_DICTIONARY_CACHE_DIR` auf einen Ordner mit dem fertigen Wörterbuch zeigen.

## Umsetzung

| Teil | Datei |
|---|---|
| Zerlegung, Wortbildung, Kana → Hepburn, Tests | `src-tauri/src/lyrics/romaji.rs` |
| `romaji` pro Zeile (`null`, solange aus) | `src-tauri/src/lyrics/types.rs`, `src/lib/shared/types.ts` |
| Einmal pro Song beim Eintreffen der Lyrics; beim Einschalten für den laufenden Song, vor dem `settings`-Event | `src-tauri/src/app_state.rs` (`add_romaji`, `romaji_changed`), `ipc/mod.rs` |
| Setting `appearance.romaji`, Standard aus, ohne Schemawechsel | `src-tauri/src/settings/schema.rs` |
| Strg+Alt+R | `hotkeys.rs`, `ipc::toggle_romaji` |
| Anzeige: `shownText()` für aktuelle und nächste Zeile (auch Layout „Leiste“); die Breite pro Song misst den angezeigten Text; Hinweis „Romaji an/aus“ im Overlay | `src/lib/overlay/` |
| Schalter „Japanische Zeilen: in Romaji (Hepburn) zeigen“ | Einstellungen → Overlay |

Karaoke-Füllung und Zeilentiming bleiben unverändert: Die Zeile bekommt nur einen anderen Text. Zeitpunkte kommen weiter aus `start_ms` und `fill_end_ms`, die Füllung bleibt ein `transform` auf derselben Animation. Wird während einer Zeile umgeschaltet, läuft die Füllung an derselben Stelle weiter, und die Zeile passt ihre Schriftgröße neu ein.

### Regeln (modifiziertes Hepburn)

- **Erkennung** wie `langOf`: Zeilen mit Hiragana, Katakana, Kanji oder Halbbreiten-Katakana. Andere Zeilen bleiben, wie sie sind. Lateinische Wörter in japanischen Zeilen auch.
- **Partikel** は, へ, を → wa, e, o, ebenso in festen Wendungen (こんにちは → konnichiwa). Quelle ist die Aussprache.
- **Lange Vokale** mit Makron, wo IPADIC sie in der Aussprache als ー führt: Tōkyō, arigatō, okāsan, kōhī, sūpā. „ei“ bleibt ei (sensei), „ii“ bleibt ii (chiisai, oniisan). Über Morphemgrenzen hinweg wird nichts gedehnt: 思う bleibt omou. Das volitionale う nach o wird ō: ikō, deshō, darō.
- **っ** verdoppelt den folgenden Konsonanten (hashitte, gakkō), vor ch wird es t (itchau). Am Wortende entfällt es.
- **ん** vor Vokal oder y wird n' (kin'en, kon'ya), vor b, m, p bleibt es n (shinbun).
- **Wörter**: Konjugationsendungen und Hilfsverben hängen am Verb (kimashita, hashitte, matteru, iwanaide, tsunaida). Partikeln, die Kopula (da, desu, datta) und Hilfsverben wie いる und しまう stehen allein (kimi no, suki da, hashitte iru). Das Präfix お bindet an das nächste Wort, Suffixe wie ら und さん an das vorige (bokura, okāsan).
- **Großschreibung** nur für Eigennamen aus dem Wörterbuch (Tōkyō), sonst klein.
- **Zeichen**: NFKC macht Halbbreiten-Katakana breit und Vollbreiten-ASCII schmal. 、 → „,“, 。 → „.“, 「」 → Anführungszeichen, ・ trennt Wörter.
- **Unbekannte Kanji** ohne Lesung bleiben als Kanji stehen. Das ist besser als falsche Romaji.

### Grenzen

- IPADIC wählt Lesungen nach Kosten im Satz. Mehrdeutige Kanji können falsch gelesen werden (次の行 → „tsugi no kudari“ statt „gyō“), ebenso Namen.
- Liedtexte schreiben manchmal eine Lesung über die Kanji, die nicht im Wörterbuch steht, etwa 本気 als „maji“ oder 運命 als „sadame“. LRC-Dateien enthalten diese Lesung nicht.
- Leerzeichen zwischen Wörtern folgen festen Regeln. Schreibweisen wie „suki nan da“ gegen „suki nanda“ sind Geschmackssache.

## Prüfung

| Kriterium | Ergebnis | Nachweis |
|---|---|---|
| Unit-Tests der Umwandlung, nur erfundene Sätze | 8 Tests (Kana-Tabelle, Lesung/Aussprache, Partikel, lange Vokale, Verbendungen, gemischte Schrift, Breiten, einmal pro Song), dazu 1 Settings-Test | `cargo test`: 73 grün |
| `npm run check` | 0 Fehler, 0 Warnungen | |
| Schalter live | **erfüllt**: Nutzer am PC, „Only Wonder“ (frederic). Strg+Alt+R: 38 von 41 Zeilen in Romaji, 12,5 ms nach dem Tastendruck, inklusive Wörterbuch-Laden, im Debug-Build. Aus und wieder an ohne Neuberechnung. Mit gespeicherter Einstellung beim nächsten Start sofort mit dem Song (19 ms). Füllung lief normal weiter. | Log `romaji toggled`, `romaji ready` |
| CPU bei laufender Füllung ≤ 2 % aller Kerne | **erfüllt**: 8,5 % eines Kerns = **0,53 % von 16 Kernen**, Romaji an, japanischer Song, 60 s (Phase 2: 0,59 %) | `measure-cpu.ps1 -Seconds 60` |
| Privater Speicher | LyricHub-Prozess 9,8 MB privat, das Wörterbuch eingeschlossen. Mit allen WebView2-Prozessen 195 MB privat (GPU-Prozess 89 MB), Working Set 480 MB. Der Anteil von Romaji liegt bei etwa 0,3 MB (Messung am Prototyp), die Gesamtsumme liegt trotzdem knapp unter der Grenze von 200 MB: Befund für Phase 5. | `Get-Process`, Prozessbaum wie in `measure-cpu.ps1` |

Wunsch des Nutzers aus dem Live-Test: Die Vorschauzeile zeigt nur die Romaji der nächsten Zeile. Die erste Fassung hängte das Original klein dahinter.
