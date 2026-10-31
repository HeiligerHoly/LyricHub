# Auto-Sync (Q1 und Q2 aus PLAN-plugins-2.md)

> Datum: 2026-10-31 · nach v1.2.0 · Bezug: [PLAN-plugins-2.md](../PLAN-plugins-2.md) Abschnitte 3, 5 und 6, [docs/spike-recognize.md](spike-recognize.md) (M1), [docs/plugin-align.md](plugin-align.md)
> Ergebnis: **Ist Align an einem Song fertig, beginnt jede Zeile 150 ms vor ihrem ersten gesungenen Wort. An vier JamendoLyrics-Songs mit absichtlich verschobenen Zeitstempeln liegt der Zeilenbeginn danach im Median 38 ms neben dem Gesang, p90 165 ms. Größere Suchfenster, die der Plan vorsah, messen deutlich schlechter und sind verworfen.**

Englisch: [docs/en/auto-sync.md](en/auto-sync.md)

## Was gebaut ist

Auto-Sync braucht weder einen zweiten Mitschnitt noch zusätzliche Rechenzeit. Die Daten liegen schon im Host: Aligns gemessene Wortzeiten.

| Teil | Umsetzung | Code |
|---|---|---|
| Schalter | Align erklärt in seinem Manifest die Einstellung `auto_sync` (an/aus, ab Werk an). Die Plugin-Seite zeichnet sie als Schalter. | `plugins/align/plugin.json` |
| Plugin-Einstellungen (Q1) | Manifeste dürfen bis zu 8 Schalter erklären: `id` aus Kleinbuchstaben, Ziffern und `_`, Typ `bool`, Standardwert, Beschriftung und Hilfe auf Deutsch und Englisch. LyricHub speichert die Werte in `settings.json` unter `plugins.values` und schickt sie dem Plugin mit `settings`, beim Start und bei jeder Änderung. | `plugins/manifest.rs`, `plugins/mod.rs`, `settings/schema.rs`, `PluginsPage.svelte` |
| Zeilenbeginn | Hat eine Zeile gemessene Wörter, beginnt sie beim ersten Wort minus 150 ms, höchstens 1,5 s vom LRC-Stempel entfernt und nie vor der Zeile davor. Die vorige Zeile endet am neuen Beginn. | `plugins/words.rs` (`auto_sync`) |
| Zeilen ohne Messung | Sie rücken samt geschätzter Wörter um den Median der gemessenen Zeilen des Songs. | ebenda |
| Anwendung | Beim Übernehmen der Plugin-Wörter, also bei jedem Anzeigen eines gemessenen Songs. Der Cache behält die LRC-Stempel; aus geschaltet gelten sie sofort wieder. Ein Song-Offset wirkt zusätzlich. | `app_state.rs` (`add_plugin_words`) |
| Diagnose | Zeile „Zeilen: nach dem Gesang ausgerichtet (Auto-Sync), im Mittel X ms“ (`line_sync_ms` im Lyrics-Ereignis). | `DiagnosticsPage.svelte` |

**Align selbst** ändert sich für Auto-Sync nicht. Sein Fenster beginnt wie in P0 0,5 s vor dem Zeitstempel einer Zeile (Abschnitt „Messung“).

## Prüfung

### Unit-Tests

- **Host** (`cargo test` in `src-tauri`): Auto-Sync rückt Zeilen an ihr erstes Wort und Zeilen ohne Messung um den Median; Grenzen, Reihenfolge und ein Song ohne Messung; Manifeste mit Schaltern samt Ablehnung falscher Einträge.
- **Align** (`cargo test` in `plugins/align`): Fenster mit frei wählbarem Vorlauf.

### Messung an JamendoLyrics

Vier Songs aus P0 (Deutsch, Englisch, Französisch, Spanisch, zusammen 13,7 min), Align-Pipeline mit 4 Threads (`cargo run --release --example jamendo`). Gemessen wird der Abstand des ersten ausgerichteten Worts jeder Zeile, minus 150 ms Vorlauf, zum annotierten Gesang. „Verschoben“ heißt: Die Zeitstempel wurden wie bei einer schlampigen LRC-Datei verschoben, je Song um bis zu ±300 ms und je Zeile um weitere bis zu ±200 ms (`--jitter 300 200`).

| Lauf | Fenster beginnt vor dem Stempel | Stempel | Zeilenbeginn Median / p90 | innerhalb 100 ms | Wortbeginn Median / p90 |
|---|---|---|---|---|---|
| M1 aus Q0 (24 Songs) | 0,5 s | genau | 37 / 138 ms | – | – |
| 0 | 0,5 s | genau | 40 / 161 ms | 69,8 % | 48 / 271 ms |
| a | 0,5 s | verschoben | **38 / 165 ms** | 72,5 % | 49 / 274 ms |
| b | 1,5 s | verschoben | 94 / 1225 ms | 50,5 % | 57 / 894 ms |
| c | 1,5 s | genau | 104 / 1279 ms | 46,7 % | 58 / 937 ms |

- **Align ist nach dem Umbau unverändert** (Lauf 0): Wortbeginn im Median 48 ms, p90 271 ms, 82,8 % höchstens 150 ms daneben, genau die Werte aus P3. Das war die Abnahme von Q1 (gemeinsamer Code, Einstellungen, neue Fähigkeit).
- **Das Gate (Median ≤ 50 ms, p90 ≤ 200 ms) hält auch mit verschobenen Stempeln** (Lauf a).
- **Größere Fenster schaden.** Der Plan sah vor, Aligns Fenster mit Auto-Sync 1,5 s vor dem Stempel beginnen zu lassen, damit auch ein sehr später Stempel sein erstes Wort noch findet. Gemessen ist das Gegenteil: Mit 1,5 s rutscht der Zeilenbeginn bei jeder zehnten Zeile über eine Sekunde daneben, auch mit genauen Stempeln (Lauf c). Vermutlich enthält das längere Fenster das Ende der Zeile davor, und die Ausrichtung legt die ersten Wörter dorthin.
- **Entscheidung:** Das Fenster bleibt bei 0,5 s. Auto-Sync kann eine Zeile deshalb höchstens etwa 0,65 s früher legen als ihr Stempel (Fensteranfang plus Vorlauf), später bis zur Grenze von 1,5 s. Typische LRC-Fehler liegen darunter: je Song im Median −140 … +151 ms ([docs/sync-report.md](sync-report.md)).

### In der echten App

Spotify, 2026-10-31, ein Song, den Align beim ersten Hören gemessen hatte (29 von 30 Zeilen mit Wortzeiten), beim zweiten Hören:

| Zustand | erstes Wort nach dem Zeilenbeginn, p10 / Median / p90 | Diagnose |
|---|---|---|
| Auto-Sync an | 150 / 150 / 150 ms | Verschiebung im Median −108 ms |
| Schalter aus | −79 / 42 / 181 ms (die LRC-Stempel) | keine Zeile |
| wieder an | 150 / 150 / 150 ms | −108 ms |

- **Jede gemessene Zeile beginnt genau 150 ms vor ihrem ersten Wort.** Ohne Auto-Sync kam bei jeder zehnten Zeile das erste Wort schon 79 ms vor der Zeile, bei einer anderen Zehntel erst 181 ms danach.
- **Der Schalter wirkt sofort** auf den laufenden Song, in beide Richtungen.
- **Lyrics von Recognize** rückt Auto-Sync kaum noch: Dort beginnen die Zeilen schon am ersten Wort (Verschiebung 4 ms).

## Grenzen

- **Erst ab dem zweiten Hören** und nur für Songs, die Align gemessen hat.
- **Früher als 0,65 s vor dem Stempel** findet Auto-Sync keine Zeile (siehe oben). Liegt ein ganzer Song so weit daneben, hilft der Song-Offset.
- **Wo Align schlecht ausrichtet,** rückt auch Auto-Sync daneben. Beim englischen Testsong liegt jeder zehnte Wortbeginn über 0,8 s daneben (Lauf a), bei den anderen drei unter 0,7 s. Die Grenze von 1,5 s fängt grobe Fehler ab, einzelne Zeilen können trotzdem springen. Dann hilft der Schalter.
- **Song-Offset nach der Messung:** Er verschiebt Zeilen und Wörter gemeinsam gegen den Gesang, wie bei Align.
