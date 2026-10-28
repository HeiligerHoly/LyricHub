# LyricHub – Plan: Plugin-Support und Plugin „Align“

> Stand: 2026-10-28 · Basis: v1.0.1 plus die lokalen Commits zu Füll-Schalter und Wortschätzung · Zielversion: 1.2.0 (nach 1.1.0 mit Browser-Support)
> Status: **P0 erledigt (2026-10-28, [docs/spike-align.md](docs/spike-align.md)): Genauigkeit erfüllt das Gate deutlich, Rechenzeit auf der CPU nicht.** Der Nutzer hat P1–P4 mit Option B freigegeben, nach dem Release 1.1.0 (Abschnitt 8). **P1 erledigt (2026-10-28, [docs/plugin-host.md](docs/plugin-host.md)).**

## 1. Kurzfassung

LyricHub bekommt eine Plugin-Schnittstelle. Plugins sind eigene Programme, die LyricHub startet und über eine einfache Textverbindung (JSON-Zeilen über stdin/stdout) anspricht. Version 1 der Schnittstelle kann genau eine Sache: **Wortzeiten liefern.**

Das erste Plugin, **lyrichub-align**, schneidet nur den Ton von Spotify mit (nicht das Spiel), trennt nach dem Song den Gesang ab und richtet den bekannten Liedtext darauf aus. Die echten Wortzeiten landen im Cache. Ab dem zweiten Hören leuchten die Wörter dann im Takt des Gesangs, mit Wort-Glow.

Vor dem Bau steht eine **Messung (P0)**: Nur wenn die Ausrichtung auf dem Datensatz JamendoLyrics deutlich besser ist als die heutige Schätzung (Median 136 ms, p90 728 ms) und auf dem PC des Nutzers schnell genug läuft, wird das Plugin gebaut.

## 2. Ziele und Nicht-Ziele

**Ziele**

- LyricHub selbst bleibt klein: Der Installer bleibt bei etwa 10 MB, die CPU-Last ohne Plugins bei etwa 0,5 %.
- Plugins sind optional, abschaltbar und können LyricHub nicht zum Absturz bringen (eigener Prozess).
- Echte Wortzeiten für Songs, die schon einmal gelaufen sind, ohne Handarbeit.
- Kein Ton verlässt den PC und keiner wird auf der Festplatte gespeichert; gespeichert werden nur Wortzeiten.

**Nicht-Ziele in Version 1**

- Kein Plugin-Store und kein Download fremder Plugins aus der App.
- Keine Plugins von Dritten: Die Schnittstelle bleibt vorerst intern und darf sich jederzeit ändern; es gibt keine Doku für fremde Autoren.
- Keine Plugins, die das Overlay zeichnen, andere Lyrics-Quellen einbinden oder Einstellungen anderer Bereiche ändern. Das Manifest ist dafür vorbereitet (`capabilities`), umgesetzt wird nur `word_timing`.
- Keine Sandbox. Plugins sind normale Programme mit den Rechten des Nutzers; das steht deutlich in der App.
- Keine Ausrichtung in Echtzeit beim ersten Hören.

## 3. Ausgangslage

- **Wortschätzung (2026-10-28):** Silben plus Buchstaben, gehaltenes letztes Wort, Singdauer aus dem Zeilenabstand. Gegen 79 Songs mit Wortzeiten: Median 136 ms, p90 728 ms (`docs/word-timing.md`).
- **LRCLIB:** praktisch keine Wortzeiten (0 von 160 Treffern, Phase 4).
- **Recherche zu Ausrichtungsmodellen (2026-10-28):**

| Option | Bausteine | Lizenz | Bekannte Werte |
|---|---|---|---|
| A | htdemucs (Gesang abtrennen) + MMS-Forced-Aligner (Zeichen, romanisiert, ~1100 Sprachen) | htdemucs MIT, MMS-Gewichte **CC-BY-NC 4.0** | PotLyrics: 67 ms Median pro Zeichen an **einem** japanischen Song; 100–130 s für 4 min auf 8 Threads (int8-Modelle) |
| B | w2v-bert-2.0 „music-lyrics-ctc“, arbeitet direkt auf dem Mix, ohne Trennung | MIT; Trainingsdaten nicht dokumentiert | keine veröffentlichten Messwerte; großes Modell (w2v-bert ~600 M Parameter, int8-Variante vorhanden) |
| C | wav2vec2-base-960h | Apache 2.0 | nur Englisch, nicht für Gesang trainiert |

- **Mitschnitt:** Windows kann mit `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` gezielt den Ton eines Prozesses samt Kindprozessen mitschneiden (`PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`), ab Windows 10 Build 20348, also auf jedem Windows 11. OBS nutzt dieselbe Schnittstelle. Es ist keine Injection.
- **Lyrics-Kette in LyricHub:** Der Cache liegt in SQLite (`lyrics/cache.rs`, Schema-Version 1). `Word` hat das Feld `estimated`; echte Wortzeiten schalten im Overlay den Wort-Glow frei (`KaraokeLine.svelte`). Romaji für japanische Zeilen erzeugt LyricHub selbst (`lyrics/romaji.rs`).

## 4. Architektur des Plugin-Systems

### 4.1 Prozessmodell

- Jedes Plugin ist eine eigene `.exe`. LyricHub startet sie ohne Konsolenfenster mit Pipes für stdin/stdout/stderr.
- **Protokoll:** JSON-RPC 2.0, eine Nachricht pro Zeile (UTF-8, `\n`). stderr landet zeilenweise im LyricHub-Log, gekürzt und ohne Songangaben.
- Alle Plugin-Prozesse hängen in einem **Job Object** mit `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: Endet LyricHub, auch durch einen Absturz, enden die Plugins mit.
- **Prozesspriorität:** `BELOW_NORMAL_PRIORITY_CLASS` als Standard; ein Plugin kann sie nicht anheben.

**Verworfen:** DLLs im LyricHub-Prozess (ein Absturz reißt LyricHub mit, ABI-Probleme) und WASM (kein Audio-Mitschnitt, keine GPU, schwache Threads).

### 4.2 Manifest `plugin.json`

```json
{
  "id": "lyrichub-align",
  "name": "Align – echte Wortzeiten",
  "version": "0.1.0",
  "api": 1,
  "executable": "lyrichub-align.exe",
  "capabilities": ["word_timing"],
  "permissions": ["audio:player", "network:huggingface.co"],
  "homepage": "https://github.com/HeiligerHoly/LyricHub",
  "description": { "de": "…", "en": "…" }
}
```

`permissions` dient der Information beim Aktivieren. Durchsetzen kann LyricHub sie nicht (keine Sandbox), mit einer Ausnahme: Die Prozess-ID des Players bekommt nur ein Plugin, das `audio:player` erklärt.

### 4.3 Ablage, Installation, Aktivierung

- **Ordner:** `%APPDATA%\LyricHub\plugins\<id>\` mit `plugin.json`, der exe und ihren Dateien. Große Daten wie Modelle legt das Plugin unter `%LOCALAPPDATA%\LyricHub\plugins\<id>\` ab; LyricHub übergibt beide Pfade.
- **Installation:** Einstellungen → „Plugins“ → „Plugin aus ZIP installieren …“. LyricHub prüft das Manifest, zeigt Name, Version, Rechte und SHA-256 der ZIP und installiert erst nach Bestätigung. Deinstallieren löscht beide Ordner.
- Neu installierte Plugins sind **aus**. Beim ersten Einschalten erscheint ein Hinweis: „Plugins sind normale Programme. Installiere nur Plugins, denen du vertraust.“

### 4.4 Lebenszyklus

1. LyricHub startet jedes aktive Plugin beim Programmstart oder beim Einschalten.
2. **Handshake:** `initialize` mit `api`, Sprache, Datenpfaden und LyricHub-Version. Das Plugin antwortet mit seiner `api`-Version und seinen Fähigkeiten. Passt die Version nicht, wird das Plugin beendet und als „inkompatibel“ angezeigt.
3. **Absturz:** höchstens 3 Neustarts mit 2 s, 10 s und 60 s Abstand, danach Status „abgestürzt“ mit der letzten stderr-Zeile.
4. **Beenden:** `shutdown`, 2 s warten, dann beenden.

### 4.5 Protokoll v1

**Host → Plugin (Notifications)**

| Methode | Inhalt |
|---|---|
| `track` | `key`, `title`, `artist`, `album`, `duration_ms`, `player`: `{ app_id, pid }` (`pid` nur mit `audio:player`) |
| `playback` | `status` (playing/paused/stopped), `position_ms`, `at` (Zeitanker, QPC in ms), bei jeder Änderung und alle 2 s |
| `lyrics` | `key`, `lyrics_hash`, `lines`: `[{ start_ms, end_ms, text, romaji, units }]`; `units` sind die Wörter so, wie LyricHub sie zerlegt (`words::split`), damit die Antwort eindeutig zuzuordnen ist |
| `settings` | Werte der Plugin-Einstellungen (4.7) |

**Plugin → Host (Notifications)**

| Methode | Inhalt |
|---|---|
| `word_times` | `key`, `lyrics_hash`, `lines`: `[{ index, words: [[start_ms, end_ms], …] }]`, `confidence` 0..1 pro Zeile |
| `status` | kurzer Text und optional `progress` 0..1, erscheint auf der Plugin-Seite |
| `log` | Stufe und Text, landet im LyricHub-Log |

### 4.6 Host-Seite: Cache und Übernahme

- **Neue Tabelle `plugin_words`** (Schema-Version 2, Migration): `key`, `lyrics_hash`, `plugin_id`, `plugin_version`, `words_json`, `created_at`.
- **Prüfung:** Eine Zeile wird nur übernommen, wenn die Wortzahl zu `units` passt, die Zeiten aufsteigen und innerhalb der Zeile plus 1 s liegen. Eine Schwelle für `confidence` gibt es nicht: In P0 waren selbst unsichere Ausrichtungen genauer als die Schätzung.
- **Übernahme:** Beim Laden der Lyrics ersetzt LyricHub geschätzte Wörter durch Plugin-Wörter (`estimated: false`), wenn `lyrics_hash` passt. Ändert sich der Text bei LRCLIB, verfallen die alten Zeiten. Kommen Plugin-Wörter, während der Song läuft, schickt LyricHub die Lyrics neu; die Füllung springt dann von der Schätzung auf die echten Zeiten.
- **Sichtbarkeit:**
  - Sync Lab und Diagnose zeigen die Wortquelle pro Song: „geschätzt“, „LRC“ oder „Plugin Align“.
  - Der Diagnose-Export enthält Plugin-Namen und -Status, aber keine Songangaben.

### 4.7 Einstellungsseite „Plugins“

- Liste mit Name, Version, Status (aus, läuft, arbeitet, Fehler, inkompatibel), Schalter, Info zu den Rechten und Entfernen.
- **Plugin-Einstellungen:** Ein Plugin kann im Manifest wenige einfache Einstellungen erklären (`bool`, `choice`, `number` mit Grenzen, Texte in de/en). LyricHub zeichnet sie und schickt die Werte über `settings`. Freie Oberflächen der Plugins gibt es nicht.

## 5. Erstes Plugin: lyrichub-align

### 5.1 Ablauf

1. **`track`:** Neuer Song. Gibt es für `key` und `lyrics_hash` schon Plugin-Wörter, passiert nichts.
2. **Mitschnitt:** Process Loopback auf den Spotify-Prozess (`pid` vom Host, Prozessbaum eingeschlossen). PCM landet in einem RAM-Puffer, jeder Block mit Zeitstempel.
3. **Zuordnung:** Die `playback`-Anker ordnen jedem Block seine Songposition zu. Bei Spulen oder Pause entstehen Lücken. Ausgerichtet wird nur, wenn mindestens 85 % des Songs gehört wurden; die Lücken bleiben geschätzt.
4. **Songende:** Der Puffer geht in die Warteschlange, der RAM-Puffer für den nächsten Song beginnt neu.
5. **Verarbeitung:** im Hintergrund, nach den Regeln in 5.4 → Gesang abtrennen (nur Option A) → 16 kHz mono → CTC-Emissionen → Viterbi-Ausrichtung des normalisierten Texts → Wortzeiten → `word_times`.
6. Der Ton wird danach verworfen.

### 5.2 Text normalisieren

- **Lateinische und kyrillische Schrift:** NFKC, Kleinschreibung, Satzzeichen weg, Apostrophe in Wörtern bleiben. Option A braucht romanisierten Text; Kyrillisch wird transliteriert.
- **Japanisch:** Der Host schickt die Romaji, die LyricHub ohnehin erzeugt (`romaji`-Feld). So braucht das Plugin kein eigenes Wörterbuch. Die Zeichen werden auf die Kana/Kanji-Einheiten der Originalzeile zurückverteilt.
- **Koreanisch und Chinesisch:** in Version 1 nicht unterstützt; diese Songs bleiben geschätzt.

### 5.3 Modelle (Entscheidung nach P0)

- **Option A:** htdemucs-Gesangstrennung (MIT) plus MMS-Forced-Aligner (CC-BY-NC 4.0). Die Lizenz verbietet kommerzielle Nutzung. LyricHub ist kostenlos und darf nach Commons Clause nicht verkauft werden. Das Plugin lädt die Gewichte beim Nutzer direkt von der Originalquelle herunter und verteilt sie nicht selbst. Es zeigt Lizenz und Namensnennung an.
- **Option B:** w2v-bert-2.0-music-lyrics-ctc (MIT). Ohne Trennung, also ein Modell weniger. Die Trainingsdaten sind unbekannt.
- **Option C** dient nur als Rückfall für Englisch.
- **Ausführung:** ONNX Runtime über das Rust-Crate `ort`, CPU mit int8-Modellen; DirectML (GPU) optional, wenn P0 zeigt, dass es sich lohnt.
- **Download beim ersten Einschalten:** Die Größe wird vorher angezeigt (geschätzt 300–700 MB, genau nach P0). Die SHA-256 ist im Plugin festgelegt, abgebrochene Downloads werden fortgesetzt. Ein Knopf „Modelle löschen“ gibt den Platz frei.

### 5.4 Rechenlast und Zeitplan

- **Ein Schalter in den Einstellungen:** Das Plugin lässt sich jederzeit ein- und ausschalten (Entscheidung des Nutzers). Ist es an, schneidet es mit und rechnet nach dem Song im Hintergrund. Ist es aus, schneidet es nicht mit und rechnet nicht.
- **Spiele:** Läuft ein Vollbildspiel im Vordergrund, rechnet das Plugin nicht; eine laufende Berechnung pausiert zwischen zwei Zeilenfenstern und macht danach weiter (Bedingung aus P0, Abschnitt 8). Ob das zusätzlich abschaltbar sein soll, entscheidet der Nutzer nach P3.
- **Ressourcen:**
  - höchstens 2 Threads (einstellbar 1–4), Priorität „unter normal“
  - Arbeitsspeicher während der Verarbeitung höchstens 1,5 GB samt Warteschlange, im Leerlauf ohne Warteschlange unter 100 MB
  - Mitschnitt unter 1 % CPU
- **Warteschlange:** höchstens 500 MB Ton im RAM. Für Option A sind das 44,1 kHz Stereo in 16 Bit, etwa 10,6 MB pro Minute, also rund 12 Songs à 4 Minuten. Für Option B reichen 16 kHz Mono, etwa 1,9 MB pro Minute. Ist die Grenze erreicht, fällt der älteste Song weg.
- **Leerlauf:** Das Plugin bleibt geladen, verbraucht aber ohne Spotify-Ton keine CPU.

### 5.5 Datenschutz, Anti-Cheat, Lizenzen

- **Datenschutz:** Der Ton bleibt im RAM und wird nach der Ausrichtung verworfen. Gespeichert werden nur Wortzeiten. Netzwerkzugriff gibt es nur für den Modell-Download.
- **Anti-Cheat:** Process Loopback ist eine normale Windows-Schnittstelle, wie bei OBS oder Discord: kein Zugriff auf Spielprozesse, keine Hooks. Das kommt so ins FAQ.
- **Lizenzen:** Lizenz und Namensnennung der Modelle erscheinen auf der Plugin-Seite und in `THIRD_PARTY_NOTICES` des Plugins.

### 5.6 Code und Bau

- **Ordner:** `plugins/align/` als eigenes Cargo-Paket im Repository, nicht im LyricHub-Installer.
- **Abhängigkeiten:**
  - `ort` für ONNX
  - `windows` (WASAPI, Process Loopback)
  - `rubato` für Resampling
  - `serde_json`
  - die STFT für htdemucs, falls Option A gewählt wird (`realfft`)
- **Veröffentlichung:** eigenes Release-Asset `lyrichub-align-<version>.zip` mit exe, ONNX-Runtime-DLL, `plugin.json`, Lizenzdateien und SHA-256 in den Release-Notes.
- **Beispiel-Plugin:** `plugins/example/` antwortet mit festen Wortzeiten. Es dient nur den Tests des Hosts.

## 6. Phasen und Abnahmekriterien

### P0 – Messung (Gate, vor jedem Bau)

- **Genauigkeit:** Ein Python-Prototyp im Scratchpad richtet die Songs von JamendoLyrics aus, deren Audio CC-lizenziert ist. Gemessen werden Option A und Option B mit derselben Messung wie in `docs/word-timing.md`, also Median und p90 des Wortbeginns, nach Sprache.
- **Laufzeit:** Laufzeit und Arbeitsspeicher pro Song auf dem PC des Nutzers, mit 2 und 4 Threads, int8.
- **Mitschnitt-Test:** Process Loopback auf Spotify, 10 s in den RAM, Pegel und Lücken prüfen, nichts speichern. Braucht das OK des Nutzers, weil Spotify dafür spielen muss.
- **Abnahme:**
  - Median höchstens 70 ms und p90 höchstens 300 ms über alle vier Sprachen
  - Laufzeit höchstens 0,6 × Songlänge mit 2 Threads
  - Arbeitsspeicher höchstens 1,5 GB
  - Mitschnitt klappt ohne Spielton
- Schafft keine Option das Gate, wird P1–P4 nicht gebaut, und der Bericht hält fest, woran es scheiterte.
- **Ergebnis:** `docs/spike-align.md` und `docs/en/spike-align.md`.

### P1 – Plugin-Host

- Manifest, Ablage, ZIP-Installation, Job Object, Lebenszyklus, JSON-RPC, Log-Weiterleitung, Einstellungsseite, Diagnose.
- **Beispiel-Plugin** unter `plugins/example/`.
- **Tests:**
  - Manifest-Prüfung
  - Protokoll mit erfundenem Text
  - Neustart nach Absturz
  - Beenden mit LyricHub
  - inkompatible API
  - ZIP mit Pfaden außerhalb des Zielordners wird abgelehnt
- **Abnahme:** Das Beispiel-Plugin läuft, stürzt kontrolliert ab und wird neu gestartet. Endet LyricHub hart, verschwindet es mit. Ohne aktive Plugins bleiben CPU-Last und Installergröße unverändert.

### P2 – Schnittstelle `word_timing`

- Cache-Migration auf Schema 2, Prüfung, Übernahme, Neusenden der Lyrics, Anzeige der Wortquelle.
- **Tests:**
  - Migration
  - passende und unpassende Wortzahl
  - geänderter `lyrics_hash`
  - Übernahme mitten im Song
- **Abnahme:** Mit dem Beispiel-Plugin leuchten die Wörter nach dessen Zeiten, mit Wort-Glow. Nach einem Neustart kommen die Zeiten aus dem Cache.

### P3 – Plugin Align

- Mitschnitt, Zuordnung zur Songposition, Warteschlange, Modell-Download, Pipeline aus P0 in Rust, Zeitplan nach 5.4, Status auf der Plugin-Seite.
- **Abnahme:**
  - Die Rust-Pipeline trifft die P0-Werte auf JamendoLyrics, höchstens 10 ms Abweichung im Median.
  - Ein echter Spotify-Song hat nach dem Hören Plugin-Wortzeiten, beim zweiten Hören mit Glow.
  - Läuft ein Vollbildspiel, wird nicht gerechnet.
  - Die CPU-Grenzen aus 5.4 halten.

### P4 – Doku und Release 1.2.0

- **Plugin-Doku:** `docs/plugins.md` und `docs/en/plugins.md` beschreiben für Nutzer Installation, Datenschutz und Speicherbedarf; das Protokoll steht nur im Code und in diesem Plan.
- **FAQ:** Datenschutz, Anti-Cheat, Speicherbedarf.
- **Projektdokumente:** Changelog, README, Release-Notes, `THIRD_PARTY_NOTICES` des Plugins.
- **Release:** LyricHub 1.2.0 plus `lyrichub-align-0.1.0.zip` als zweites Asset.

## 7. Risiken

| Risiko | Auswirkung | Gegenmittel |
|---|---|---|
| Ausrichtung auf echten Mixes schlechter als im Datensatz (Hall, Chöre, Rap) | falsche Wortzeiten | Plausibilitätsprüfung pro Zeile (4.6); eine Konfidenz-Schwelle hilft laut P0 nicht |
| Rechenlast stört Spiele | Ruckler | Standard „nur ohne Vollbildspiel“, Priorität unter normal, 2 Threads |
| CC-BY-NC bei Option A | rechtlich heikel, falls LyricHub je kommerziell würde | Download beim Nutzer statt Verteilung, Hinweis; Option B als MIT-Alternative |
| Process Loopback nimmt bei manchen Spotify-Versionen nichts auf | kein Plugin-Nutzen | P0-Test, klare Statusmeldung |
| Plugins als Einfallstor für Schadsoftware | Vertrauen | keine Downloads aus der App, Warnhinweis, SHA-256 bei der Installation |
| Modelle sind Hunderte MB groß | Platz, Downloadzeit | Download erst beim Einschalten, Größe vorher anzeigen, Löschknopf |

## 7a. Ergebnis von P0 (2026-10-28)

Bericht: [docs/spike-align.md](docs/spike-align.md). 24 Songs aus JamendoLyrics, je 6 pro Sprache:

| | Schätzung | Option A | Option B |
|---|---|---|---|
| Median / p90 Wortbeginn | 129 / 713 ms | 37 / 152 ms | 37 / 119 ms |
| Rechenzeit, 2 Threads (CPU) | – | – | 1,50 × Songlänge |
| Rechenzeit, 4 Threads (CPU) | – | 0,92 × | 0,89 × |
| GPU (DirectML, fp16, Spiel lief parallel) | – | – | 0,57 ×, dazu 1,9 CPU-Kerne |

- **Genauigkeit:** Gate erfüllt.
- **Rechenzeit:** Auf der CPU verfehlt. Die Zeit steckt fast ganz im Modell, Rust würde daran wenig ändern.
- **Speicher:** knapp verfehlt; ONNX Runtime allein braucht etwa 0,6 GB plus bis 1 GB während der Rechnung.
- **Mitschnitt:** funktioniert, ohne Lücken.
- **Empfehlung:** Option B (MIT, weniger Ausreißer, kein zweites Modell). Machbar nur, wenn das Plugin ausschließlich ohne Vollbildspiel rechnet und Songs bis dahin im RAM sammelt (etwa 230 MB für 2 Stunden Musik).

## 8. Entscheidungen (2026-10-28)

1. **Modell:** P0 misst Option A und Option B; entschieden wird mit Zahlen.
2. **Wann ausrichten:** Ein Schalter in den Einstellungen schaltet das Plugin an oder aus (5.4).
3. **Fremde Plugins:** erst nur eigene; die Schnittstelle bleibt intern.
4. **Reihenfolge:** jetzt nur P0. Danach 1.1.0 mit Browser-Support, Füll-Schalter und neuer Wortschätzung, erst dann P1–P4 als 1.2.0.
5. **Nach P0 (2026-10-28):** P1–P4 werden mit Option B gebaut, nach dem Release 1.1.0, obwohl das Gate bei der Rechenzeit verfehlt ist. Bedingung: Das Plugin rechnet nur, wenn kein Vollbildspiel im Vordergrund läuft, und sammelt die Songs bis dahin im RAM (5.4). Option A entfällt; der Download in 5.3 betrifft nur das int8-Modell von Option B (588 MB).

## 9. Arbeitsregeln für die Umsetzung

- Deutsch mit dem Nutzer; Code, Bezeichner und Commit-Nachrichten Englisch, Doku auf Deutsch und Englisch (`docs/` und `docs/en/`).
- Keine echten Liedtexte in Tests, Fixtures, Logs oder Chat. Tests nutzen erfundenen Text; Messungen geben nur Zahlen aus. Datensätze und Modelle kommen nicht ins Repository.
- Kein Ton auf die Festplatte, auch nicht in Tests. Ausnahme ist das CC-lizenzierte Audio von JamendoLyrics im Scratchpad für P0.
- Eingriffe am PC des Nutzers (Spotify steuern, Mitschnitt-Test, Vollbild simulieren) nur nach seinem OK.
- Committen und pushen nur auf Anfrage; Git-Identität HeiligerHoly mit noreply-Adresse, nie die private E-Mail.
- Behauptungen über Genauigkeit oder Last nur mit Messwert. Jede Phase endet mit einem kurzen Bericht in `docs/`.
