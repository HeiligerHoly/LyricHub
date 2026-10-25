# Spike A – SMTC-Messung (Spotify, Microsoft-Store-Version)

> Datum: 2026-10-25 · Phase 0 · Bezug: PLAN.md 3.2 und 5.8
> Ergebnis: **SMTC reicht als alleinige Positionsquelle für v1.** Plan B (Spicetify) und Plan C (Web API) werden nicht gebraucht.

## Aufbau

| Punkt | Wert |
|---|---|
| Spotify | Microsoft Store, Paket `SpotifyAB.SpotifyMusic` 1.306.1921.0 |
| Session-ID (AUMID) | `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify` |
| Werkzeug | `tools/smtc-probe/smtc-probe.ps1` (Windows PowerShell 5.1, WinRT `Windows.Media.Control`) |
| Auswertung | `tools/smtc-probe/analyze.py` |
| Abtastung | alle 50 ms: Status, `Position`, `LastUpdatedTime`, `EndTime`; Titel alle 250 ms |
| Zeitbasis | `GetSystemTimePreciseAsFileTime` (Systemzeit mit Sub-Millisekunden-Auflösung) und Stopwatch |
| Aktiver Lauf | 240 s, das Tool steuert Spotify per SMTC: alle 40 s Spulen um +30 s, danach 3 s Pause |
| Passiver Lauf | 360 s, nur Mitlesen, normale Wiedergabe über drei Songs |

Die Rohdaten liegen lokal in `docs/spike-smtc-active.csv` und `docs/spike-smtc-passive.csv` und sind nicht eingecheckt.

PowerShell statt Rust ist hier gleichwertig. Die Timeline-Werte erzeugt Spotify selbst, der Leser beeinflusst weder Takt noch Jitter. Die Rust-Anbindung über den `windows`-Crate wird in Phase 1 gebaut.

## Kennzahlen

| Messgröße | Aktiv | Passiv |
|---|---|---|
| Abstand zwischen Timeline-Updates beim Abspielen (Median) | 4502 ms | 4508 ms |
| Alter eines Updates beim ersten Lesen (Median / p95) | 25 / 48 ms | 27 / 51 ms |
| **Korrektursprung der extrapolierten Position, \|p95\|** | 101 ms | **1 ms** |
| Korrektursprung, \|max\| | 216 ms | 45 ms |
| Rückwärtssprünge > 50 ms | 6 (alle direkt nach Spulen) | 0 |
| Wiedergaberate laut Spotify-Zeitstempeln | 1,00000 (−4 ppm) | 1,00007 (+74 ppm) |
| Position verändert sich während Pause | nie | – |

Reaktion auf SMTC-Befehle im aktiven Lauf, gemessen ab Befehl bis zum ersten sichtbaren Effekt. Die Abtastung alle 50 ms ist darin enthalten.

| Befehl | n | Median | p95 | Anmerkung |
|---|---|---|---|---|
| Play | 7 | 51 ms | 65 ms | |
| Spulen | 6 | 50 ms | 51 ms | Landeposition im Median 15 ms vor dem Ziel |
| Pause | 6 | 305 ms | 307 ms | Status springt erst nach Spotifys Ausblenden um |

## Beobachtungen

1. **Spotify veröffentlicht die Timeline alle 4,5 s** und zusätzlich sofort bei Play, Pause, Spulen und Songwechsel. Zwischen zwei Updates muss der Leser selbst extrapolieren: `Position + (jetzt − LastUpdatedTime)`.
2. **Die Extrapolation ist im Normalbetrieb praktisch exakt.** Bei ungestörter Wiedergabe weicht das nächste Update im 95. Perzentil um 1 ms ab. Die Ratenabweichung von höchstens 74 ppm macht über 4,5 s weniger als 0,4 ms aus.
3. **Nach dem Spulen kommen zwei Updates.** Das erste trifft nach etwa 50 ms ein und liegt 100 bis 216 ms zu weit vorn. Das zweite folgt 100 bis 250 ms später und korrigiert zurück. Alle sechs Rückwärtssprünge des aktiven Laufs sind diese Korrekturen.
4. **Beim Songwechsel kommt die neue Timeline vor dem neuen Titel.** Für bis zu 250 ms steht Position 0 noch beim alten Titel. Ohne Sonderbehandlung sähe das wie ein Rückwärts-Seek im alten Song aus.
5. **Pause:** Der Status wechselt rund 305 ms nach dem Befehl. Die eingefrorene Position liegt 20 bis 136 ms hinter dem Befehlszeitpunkt. Spotify spielt also während des Ausblendens kurz weiter.
6. **Spotify erneuert `LastUpdatedTime` auch im Pausenzustand**, ohne dass sich die Position ändert.
7. **Das Alter eines Updates beim Lesen** liegt im Median bei 25 ms und entspricht damit der halben Abtastperiode. Spotify setzt den Zeitstempel also praktisch im Moment der Veröffentlichung.
8. Eine zweite SMTC-Session ist aktiv (`F0DC299D809B9700`, vermutlich ein Browser-Video). Der Filter auf `Spotify` in der AUMID ist nötig.

## Entscheidung

**SMTC allein ist die Positionsquelle für v1.** Das Kriterium aus PLAN.md 5.8 lag bei einem Jitter-p95 unter 150 ms. Gemessen wurden 1 ms im Normalbetrieb und 101 ms inklusive der Korrekturen direkt nach dem Spulen. Ein Wechsel auf Desktop-Spotify mit Spicetify ist nicht nötig, die Web API bleibt optional.

## Folgen für Phase 1 bis 3

- **Seek-Behandlung im `PlaybackClock`:** Das erste Update nach einem Seek hart übernehmen. Eine Rückwärtskorrektur bis 300 ms innerhalb von 500 ms nach einem Seek direkt anwenden statt sie zu slewen. Die Anzeige springt in diesem Moment ohnehin, und so ist die Zeile 200 ms früher korrekt.
- **Songwechsel-Erkennung:** Fällt die Position auf unter 1 s und weicht stark nach unten ab, ist das ein wahrscheinlicher Songwechsel und kein Seek. Dann kurz auf `MediaPropertiesChanged` warten (Timeout 400 ms), bevor Lyrics gesucht werden. Den Anker trotzdem sofort auf 0 setzen.
- **Pause:** Auf das Status-Event reagieren und die Position einfrieren, die im Event mitkommt. Die Anzeige läuft bis dahin etwa 300 ms weiter, das deckt sich mit dem hörbaren Ausblenden.
- **Polling:** Timeline-Events plus Polling alle 250 ms reichen. Ein schnellerer Takt bringt nichts, weil Spotify nur alle 4,5 s neue Werte liefert.
- **Diagnose:** Das Sync Lab soll die Update-Kadenz anzeigen. Ändert ein Spotify-Update dieses Verhalten, fällt es dort sofort auf.

## Was diese Messung nicht zeigt

- **Den Abstand zum hörbaren Ton.** Die Messung belegt, dass Spotifys Positionsangaben in sich konsistent sind. Wie weit sie vor oder hinter dem Ton liegen, inklusive Ausgabelatenz von Windows und Gerät, misst sie nicht. Dieser konstante Versatz wird über den Geräte-Offset kalibriert und in Phase 2 mit dem Ohr geprüft (PLAN.md 11.3).
- **Andere Spotify-Versionen.** Gemessen wurde nur die Store-Version 1.306. Die Desktop-Version könnte sich anders verhalten.
- **Crossfade.** Crossfade war nicht Teil der Messung.
