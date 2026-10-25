# Sync-Bericht (Phase 3)

> Datum: 2026-10-25 · Bezug: PLAN.md 2 (Akzeptanzkriterien Sync), 5.7, 11.3
> Aufbau: Spotify aus dem Microsoft Store, nur SMTC als Quelle, kabelgebundener Kopfhörer „Headphones (High Definition Audio Device)“, globaler Offset 0. Gemessen mit dem Phase-3-Stand vor der Versionsnummer 0.3.0, deshalb steht in den Protokolldateien noch `0.2.0`.

## Ergebnis

| Kriterium (PLAN.md 2) | Ziel | Ergebnis | |
|---|---|---|---|
| Median-Fehler nach Kalibrierung, Kabel | ≤ 100 ms | **96 ms** (Geräte-Offset +100 ms, 115 Taps, 13 Songs) | erfüllt |
| p95-Fehler | ≤ 200 ms | **289 ms** | **nicht erfüllt** |
| Seek wieder korrekt | ≤ 300 ms | Median **30 ms**, höchstens 34 ms (5 Sprünge) | erfüllt |
| Nudge wirkt nur auf den laufenden Song | Songwechsel zeigt eigenen Wert | 4 von 4 Prüfungen, zusätzlich im Log des ersten Laufs | erfüllt |

Die Zeitkette selbst ist genau: Vom SMTC-Bericht bis zum gezeichneten Anker vergehen 8 bis 25 ms. Das p95 kommt von zwei anderen Quellen. Erstens setzen die LRC-Dateien ihre Zeilen je Song unterschiedlich früh oder spät, im Median eines Songs zwischen −140 und +151 ms. Zweitens streuen die einzelnen Taps innerhalb eines Songs: Rechnet man den Versatz jedes Songs heraus, bleibt ein p95 von 267 ms. Diese Streuung enthält die Reaktionszeit beim Tippen und ungenau gesetzte einzelne Zeilen; beides lässt sich mit dieser Methode nicht trennen. Kein Offset kann sie beheben. Der Nutzer hat entschieden, keine zweite Messrunde zu machen.

## Methode

PLAN.md 11.3 sieht vor, für jede Zeile „zu früh / passend / zu spät“ in 100-ms-Schritten zu schätzen. Umgesetzt ist stattdessen eine Tap-Messung: Der Nutzer drückt bei jedem hörbaren Zeilenbeginn die Leertaste (Einstellungen → Synchronisation → „Messung starten“). LyricHub nimmt den Zeitpunkt im Hotkey-Handler und rechnet:

```
Fehler = Anzeigeposition des Overlays beim Tap − Start der nächstgelegenen Zeile (höchstens ±1,5 s)
```

Ein positiver Fehler heißt: Das Overlay war schon weiter, die Lyrics kamen zu früh. Ein negativer Fehler heißt: Die Lyrics kamen zu spät. Das ergibt Millisekunden statt geschätzter 100-ms-Stufen. Dafür steckt die Reaktionszeit des Nutzers mit in jedem Wert. Median und p95 beziehen sich auf den Betrag; p95 nach Nearest Rank. Die Rohdaten (Zeitpunkte, Zeilennummern und Titel, kein Liedtext) liegen in `%APPDATA%\LyricHub\logs\sync-protocol-<unix>.json`.

Geräte-, Song- und globaler Offset addieren sich (positiv = Lyrics später). Deshalb lässt sich jedes Ergebnis exakt auf einen anderen Geräte-Offset umrechnen: Bei 100 statt 151 ms liegt jeder Fehler um 51 ms höher.

## Kalibrierung

Der Kalibrier-Assistent (5.7) lief viermal mit je fünf Taps auf „My Ordinary Life“ (The Living Tombstone). Die Vorschläge lagen bei 187, 161, 203 und 151 ms; übernommen wurde 151 ms.

Das Protokoll zeigte danach über alle Songs eine Tendenz von −51 ms: Die Lyrics kamen im Mittel zu spät. Der Grund: Ausgerechnet „My Ordinary Life“ hat die früheste LRC-Datei im ganzen Protokoll (+151 ms bei Geräte-Offset 100). Eine Kalibrierung über einen einzigen Song misst dessen LRC-Timing mit, nicht nur das Gerät. Auf Empfehlung aus dem Protokoll hat der Nutzer den Geräte-Offset auf **+100 ms** gesetzt; damit liegt die Tendenz über alle Songs bei 0.

Folgerung für später: Den Geräte-Offset aus der Tendenz eines Protokolls über mehrere Songs vorschlagen statt aus fünf Taps eines Songs (siehe `docs/phase3.md`, offene Punkte).

## Sync-Protokoll

Lauf 1: 115 Taps, 13 Songs auf Englisch und Deutsch, kein verpasster Tap (`sync-protocol-1792962624.json`).

| Auswertung | Taps | Median \|e\| | p95 \|e\| | Tendenz | ≤ 100 ms | ≤ 200 ms |
|---|---|---|---|---|---|---|
| Wie gemessen, Geräte-Offset 151 | 115 | 110 ms | 340 ms | −51 ms | 46 % | 78 % |
| **Umgerechnet auf Geräte-Offset 100** | 115 | **96 ms** | **289 ms** | 0 ms | 53 % | 81 % |
| Versatz jedes Songs herausgerechnet | 115 | 84 ms | 267 ms | 0 ms | 57 % | 87 % |
| Lauf 1 und „Alien Boy“ aus Lauf 2, Offset 100 | 126 | 98 ms | 289 ms | −13 ms | 52 % | 83 % |

Pro Song, umgerechnet auf Geräte-Offset 100. „Tendenz“ ist der Median der vorzeichenbehafteten Fehler und damit genau der Song-Offset, den ein Nudge für diesen Song setzen müsste: Tendenz −140 ms heißt Song-Offset −140 ms, die Lyrics kommen dann früher. „Streuung“ ist die mittlere absolute Abweichung von dieser Tendenz.

| Song | Artist | Taps | Tendenz | Median \|e\| | Streuung |
|---|---|---|---|---|---|
| Bis zu ihr | Papaplatte | 11 | −140 ms | 140 ms | 142 ms |
| Everybody (Backstreet's Back) – Radio Edit | Backstreet Boys | 8 | −122 ms | 122 ms | 88 ms |
| Money Game, Pt. 3 | Ren | 8 | −94 ms | 120 ms | 94 ms |
| Alien Blues | Vundabar | 4 | −86 ms | 93 ms | 93 ms |
| It Doesn't Matter | The Living Tombstone | 14 | −34 ms | 64 ms | 72 ms |
| You're Gonna Go Far, Kid | The Offspring | 7 | −10 ms | 91 ms | 101 ms |
| Nachbeben | Alligatoah | 11 | 0 ms | 77 ms | 77 ms |
| It's Been So Long | The Living Tombstone | 11 | +11 ms | 97 ms | 89 ms |
| Ali Ali | Diorange | 6 | +22 ms | 52 ms | 52 ms |
| Giant Woman (feat. Zach Callison) | Steven Universe | 8 | +30 ms | 97 ms | 98 ms |
| Hate. | thxsomch | 12 | +92 ms | 106 ms | 81 ms |
| HUSO | Ritter Lean | 8 | +120 ms | 120 ms | 60 ms |
| My Ordinary Life | The Living Tombstone | 7 | +151 ms | 151 ms | 38 ms |

Drei Taps liegen über 400 ms daneben (−539, −452, −432 ms), vermutlich eine verpasste oder doppelt getippte Zeile. Sie sind in allen Zahlen enthalten.

Lauf 2 (`sync-protocol-1792962744.json`, 14 Taps, 2 verpasst): „Alien Boy“ (Oliver Tree, 11 Taps, Tendenz −106 ms, Streuung 39 ms) und drei Taps auf „Only Wonder“ (Frederic, Japanisch). Der Nutzer kann Kana und Kanji nicht lesen und deshalb keinen Zeilenbeginn erkennen; der japanische Teil ist nicht ausgewertet.

### Abweichungen von 11.3

- **Kein japanischer Titel ausgewertet** (verlangt: mindestens zwei). Ohne lesbare Zeilen ist keine Messung möglich. Die Romaji-Anzeige ist nach dieser Messung in main gekommen (Merge von `claude/unruffled-dirac-66618b`); mit ihr lässt sich die Messung nachholen.
- **13 statt 10 Songs** auf Englisch und Deutsch, von Rap über Pop und Punk bis Electronic, kein Live-Mitschnitt.
- **Tap-Messung statt Schätzung in 100-ms-Schritten**, siehe Methode.

## Seek-Test

`tools/sync/seek-test.ps1 -Count 5` springt über SMTC abwechselnd 25 s vor und zurück und wartet auf die Log-Zeile „seek shown in overlay“. Die schreibt LyricHub, sobald das Overlay den Anker dieses Seeks gezeichnet hat (`clock_applied`).

| Messung | Anzahl | Median | Höchstens |
|---|---|---|---|
| Seek-Befehl bis Overlay korrekt (`seek-test.ps1`) | 5 | 30 ms | 34 ms |
| SMTC-Bericht bis gezeichnet, alle Seeks und Songwechsel der Sitzung | 60 | 13 ms | 25 ms (p95 19 ms) |

Seeks im Pausenzustand übernimmt der Clock direkt, ohne Seek-Anker. Das Skript startet deshalb die Wiedergabe für den Test und pausiert danach wieder.

## Nudge pro Song

`tools/sync/nudge-test.ps1` drückt zweimal Strg+Alt+. auf dem laufenden Song A, springt über SMTC zum nächsten Song B und wieder zurück und liest die Log-Zeilen „song offset nudged/applied“. Alle Werte gelten relativ zum Offset, den A beim Start hatte; am Ende stellt das Skript ihn wieder her.

```
[PASS] nudge on song A - 8ball & mjg|space age pimpin'|310: 0 -> 200 ms
[PASS] next song B starts with its own offset - camp lo|luchini aka this is it|240: 0 ms
[PASS] back on song A, its stored offset applies again - 8ball & mjg|space age pimpin'|310: 200 ms
[PASS] nudged back to the start value - 8ball & mjg|space age pimpin'|310: 0 ms
all passed
```

Log dazu (UTC, gekürzt):

```
21:24:25.958 song offset nudged  key=8ball & mjg|space age pimpin'|310 delta_ms=100 song_offset_ms=100
21:24:26.812 song offset nudged  key=8ball & mjg|space age pimpin'|310 delta_ms=100 song_offset_ms=200
21:24:27.708 track changed       title=Luchini AKA This Is It artist=Camp Lo
21:24:27.709 song offset applied key=camp lo|luchini aka this is it|240 song_offset_ms=0
21:24:29.545 track changed       title=Space Age Pimpin' artist=8Ball & MJG
21:24:29.546 song offset applied key=8ball & mjg|space age pimpin'|310 song_offset_ms=200
21:24:31.359 song offset nudged  key=8ball & mjg|space age pimpin'|310 delta_ms=-100 song_offset_ms=100
21:24:32.165 song offset nudged  key=8ball & mjg|space age pimpin'|310 delta_ms=-100 song_offset_ms=0
```

Derselbe Ablauf ist schon im ersten Lauf von Hand im Log zu sehen: „Big Poppa“ auf +200 ms, danach „Shook Ones, Pt. II“ mit 0 und zurück auf „Big Poppa“ mit +200 ms. Der erste Lauf des Skripts hatte noch einen Fehler im Log-Muster und hat dabei einen anderen Song verschoben. Diese Werte sind wieder auf 0, und das Skript misst seitdem relativ zum Startwert.
