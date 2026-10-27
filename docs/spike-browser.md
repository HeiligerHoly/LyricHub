# Spike – Browser als Quelle (Zen und YouTube Music)

> Datum: 2026-10-27 · nach v1.0.0 · Bezug: PLAN.md 13.4
> Ergebnis: **Ein Firefox-basierter Browser liefert für YouTube Music genug für synchrone Lyrics.** Browser-Support ist für v1.1 eingeplant. Offen sind normale YouTube-Videos und Chromium-Browser.

## Aufbau

| Punkt | Wert |
|---|---|
| Browser | Zen (Firefox-basiert) |
| Seite | music.youtube.com |
| Session-ID (AUMID) | `F0DC299D809B9700`, eine Kennung statt eines App-Namens |
| Werkzeug | `tools/smtc-probe/smtc-probe.ps1 -AppFilter F0DC299D809B9700`; Sitzungen auflisten mit `tools/smtc-probe/list-sessions.ps1` |
| Auswertung | `tools/smtc-probe/analyze.py`, dieselben Kennzahlen wie in [Spike A](spike-smtc.md) |
| Abtastung | alle 50 ms: Status, `Position`, `LastUpdatedTime`, `EndTime`; Titel alle 250 ms |
| Lauf | 200 s, nur Mitlesen. Der Nutzer steuerte im Browser: zweimal Spulen, eine Pause von etwa 9 s, danach ein Songwechsel |

Die Rohdaten liegen lokal in `docs/spike-browser-zen.csv` und sind nicht eingecheckt.

## Kennzahlen

| Messgröße | Zen + YouTube Music | Spotify (Spike A, passiv) |
|---|---|---|
| Timeline-Updates beim Abspielen | etwa 3 pro Minute, unregelmäßig | alle 4,5 s, etwa 13 pro Minute |
| Abstand zwischen Updates (Median / p90 / max) | 2,8 / 14,2 / 60,1 s | 4,5 s (Median) |
| Alter eines Updates beim ersten Lesen (Median / p95) | 28 / 51 ms | 27 / 51 ms |
| **Korrektursprung der extrapolierten Position, \|p95\|** | **75 ms** (n = 10) | 1 ms |
| Korrektursprung, \|max\| | 75 ms | 45 ms |
| Rückwärtssprünge > 50 ms | 1 | 0 |
| Wiedergaberate laut Browser-Zeitstempeln | 0,99983 (−169 ppm) | 1,00007 (+74 ppm) |
| Position verändert sich während Pause | nie | – |

## Beobachtungen

1. **Saubere Metadaten auf YouTube Music.** Titel und Artist kommen ohne Zusätze wie „(Official Video)“, die Länge stimmt. Das Album wurde nicht aufgezeichnet.
2. **Zen meldet die Position fast nur bei Ereignissen.** Play, Pause, Spulen und Songwechsel kommen sofort (Alter beim Lesen im Median 28 ms, wie bei Spotify). Dazwischen gibt es nur gelegentlich ein Update, im Lauf einmal erst nach 60 s.
3. **Die Extrapolation trägt auch über lange Lücken.** Die Ratenabweichung von −169 ppm macht über 60 s etwa 10 ms aus. Die Korrektursprünge lagen bei höchstens 75 ms. LyricHubs `PlaybackClock` rechnet zwischen Updates ohnehin selbst weiter und hat kein Zeitlimit für lange Pausen.
4. **Beim Songwechsel kommt die neue Position vor dem neuen Titel**, wie bei Spotify (ein Fall im Lauf). Die bestehende Songwechsel-Erkennung (Spike A, Folge 2) greift also auch hier.
5. **Die Session-ID ist eine Kennung, kein Name.** Firefox-Browser melden eine 16-stellige Hex-Kennung, vermutlich aus dem Installationspfad abgeleitet. In der Registry ist dafür kein Anzeigename eingetragen (`HKCU\Software\Classes\AppUserModelId`). LyricHub kann Browser also nicht wie Spotify am Namen erkennen.
6. **Zen meldet jede Wiedergabe als „Music“**, auch Videos. Der Medientyp taugt nicht als Musikfilter.
7. **Normale YouTube-Videos lieferten keine Position.** In einer Sitzungsliste vor der Messung meldete Zen für ein Video auf youtube.com Position 0 und Länge 0. Das wurde nicht systematisch gemessen. Ohne Position ist keine Synchronisierung möglich.

## Bewertung

Für YouTube Music in einem Firefox-basierten Browser reicht SMTC für synchrone Lyrics. Das Kriterium aus PLAN.md 5.8 (Jitter-p95 unter 150 ms) ist mit 75 ms erfüllt, und Ereignisse kommen so schnell wie bei Spotify. Browser-Support ist deshalb für v1.1 eingeplant, zunächst als experimentelle Option (PLAN.md 13.4).

## Folgen für v1.1

- **Player-Auswahl statt festem Filter:** Der feste Filter auf `Spotify` (`lib.rs`) wird zur Einstellung. Spotify bleibt Standard. Weil Browser keinen Namen melden, zeigt die Auswahl die erkannten Sitzungen mit ihrem gerade laufenden Titel, damit man den Browser erkennt.
- **Musikfilter und Opt-in:** Browser liefern auch Videos, und der Medientyp hilft nicht (Beobachtung 6). Lyrics nur suchen, wenn Länge und Titel nach Musik aussehen, und Browser-Sitzungen nur nach Einschalten nutzen. Sonst gingen alle Videotitel an LRCLIB.
- **Titel bereinigen:** Für Seiten wie youtube.com Zusätze wie „(Official Video)“ und „Artist - Song“ im Titel zerlegen, bevor LRCLIB gefragt wird.
- **Eigener Offset pro Player**, wie heute pro Ausgabegerät.
- **Sync Lab:** die Update-Kadenz pro Quelle anzeigen. Lücken bis 60 s sind bei Browsern normal und kein Fehler.

## Was diese Messung nicht zeigt

- **Den Abstand zum hörbaren Ton.** Wie bei Spike A ist nur die innere Konsistenz gemessen. Die Ausgabelatenz des Browsers wird über den Offset pro Player kalibriert.
- **Chromium-Browser** (Edge, Chrome). Sie binden SMTC anders an und können sich anders verhalten.
- **Andere Seiten** wie den Spotify-Webplayer oder SoundCloud, und normale YouTube-Videos (Beobachtung 7).
- **Eine große Stichprobe.** Ein Lauf von 200 s mit zehn Korrekturen; die Kennzahlen sind ein erster Anhalt.
