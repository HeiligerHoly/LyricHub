# Plugins

Englisch: [docs/en/plugins.md](en/plugins.md)

Plugins sind eigene Programme, die LyricHub startet und mit Daten versorgt. Sie sind freiwillig und lassen sich jederzeit ausschalten oder entfernen. Stürzt ein Plugin ab, läuft LyricHub weiter. Es gibt zwei Plugins:

- **Align** liefert echte Wortzeiten aus dem Ton, damit die Wörter im Takt des Gesangs leuchten. Mit **Auto-Sync** rückt es auch die Zeilen an den Gesang.
- **Recognize** besorgt Lyrics für Songs, zu denen LRCLIB keine synchronen hat: Es sucht breiter, richtet Text ohne Zeitstempel aus oder erkennt den Text aus dem Gesang. Braucht LyricHub 1.3.0.

Plugins sind normale Programme mit deinen Rechten. Installiere nur Plugins, denen du vertraust. Bisher gibt es nur eigene Plugins von LyricHub, und zwar auf der [Release-Seite](https://github.com/HeiligerHoly/LyricHub/releases).

## Installieren

1. Die ZIP-Datei des Plugins aus den Assets des Releases herunterladen, etwa `lyrichub-align-0.2.0.zip` oder `lyrichub-recognize-0.1.0.zip`. Nicht entpacken.
2. In LyricHub: Einstellungen → Plugins → „Plugin aus ZIP installieren …“ und die ZIP-Datei wählen.
3. LyricHub prüft die Datei und zeigt Name, Version, Rechte und die SHA-256-Prüfsumme. Die Prüfsumme sollte mit der in den Release-Notizen übereinstimmen. Dann „Installieren“.
4. Ein neu installiertes Plugin ist aus. Mit dem Schalter einschalten.

Beim ersten Einschalten lädt das Plugin sein Modell herunter, 588 MB von huggingface.co. Den Fortschritt zeigt die Plugin-Seite. Ein abgebrochener Download setzt beim nächsten Start dort fort, wo er war. Align und Recognize nutzen dasselbe Modell: Ist es beim einen schon da, übernimmt das andere es ohne Download, und auf der Festplatte liegt es nur einmal.

## So arbeitet Align

LRCLIB hat fast nur Zeilenzeiten. Für die einzelnen Wörter schätzt LyricHub deshalb, wann sie gesungen werden. Align ersetzt diese Schätzung durch Messwerte:

1. **Beim ersten Hören** schneidet Align den Ton des Players mit. Die Statuszeile zeigt dann „Hört mit“.
2. **Nach dem Song** richtet Align den Liedtext auf den Gesang aus („Richtet einen Song aus: 45 %“). Das dauert etwa das 1- bis 1,5-Fache der Songlänge.
3. **Ab dem zweiten Hören** kommen die Wortzeiten aus dem Cache. Die Wörter leuchten im Takt, mit Wort-Glow. Unter Diagnose steht dann bei „Wortzeiten“: „vom Plugin lyrichub-align“.

Ausgerichtet wird nur ein Song, der zu mindestens 85 % gehört wurde. Zeilen, die beim Mitschnitt fehlten (übersprungen, gespult), bleiben geschätzt. Songs mit echten Wortzeiten aus der LRC-Datei lässt Align in Ruhe.

### Auto-Sync

Die Zeitstempel bei LRCLIB setzen Menschen von Hand, und oft liegen sie ein paar Zehntelsekunden neben dem Gesang. Mit **Auto-Sync** (Einstellungen → Plugins → Align, ab Werk an) beginnt jede Zeile 150 ms vor ihrem ersten gesungenen Wort, sobald Align den Song gemessen hat:

- Eine Zeile rückt höchstens 1,5 s von ihrem Zeitstempel weg und nie vor die Zeile davor.
- Zeilen ohne Messung (Lücken im Mitschnitt) rücken um den Median des Songs.
- Unter Diagnose steht dann bei „Zeilen“: „nach dem Gesang ausgerichtet (Auto-Sync)“, mit der mittleren Verschiebung.
- Ein Song-Offset wirkt weiter, zusätzlich zu Auto-Sync.

Aus geschaltet gelten wieder die Zeitstempel von LRCLIB, die Wortzeiten bleiben.

## So arbeitet Recognize

Zu manchen Songs hat LRCLIB keine synchronen Lyrics. Dann springt Recognize ein:

1. **Beim ersten Hören** schneidet es den ganzen Song mit („Hört mit“), wie Align nur den Ton des Players.
2. **Nach dem Song** berechnet es, was gesungen wurde („Erkennt einen Song: 45 %“). Das dauert bis etwa zum Doppelten der Songlänge. Dann versucht es der Reihe nach:
   - **Text ohne Zeitstempel:** Hat LRCLIB den Text ohne Zeiten, richtet Recognize ihn über den ganzen Song aus, Zeilen und Wörter.
   - **Breitere Suche:** Es sucht bei LRCLIB nur nach dem Titel und vergleicht den Text jedes Treffers mit dem Gehörten. Passt einer, übernimmt es diese Lyrics und richtet sie am Gesang aus.
   - **Erkennung:** Sonst wird der erkannte Text selbst zu Lyrics, aber nur, wenn die Erkennung sicher genug ist. Das Overlay zeigt dann neben dem Titel „erkannt“.
3. **Ab dem zweiten Hören** sind die Lyrics da, mit Wortzeiten. Unter Einstellungen → Sync → Lyrics steht, woher sie kommen.

Erkannte Texte haben Fehler (im Test auch bei sicherer Erkennung etwa jedes fünfte Wort). Stimmen Lyrics von Recognize nicht, wirf sie unter Einstellungen → Sync → „Lyrics vom Plugin verwerfen“ weg. Für diesen Song fragt LyricHub dann wieder LRCLIB und bittet kein Plugin mehr darum. Auch ohne dein Zutun fragt LyricHub nach 14 Tagen erneut bei LRCLIB nach: Gibt es dort inzwischen synchrone Lyrics, ersetzen sie die von Recognize.

## Spiele und Leistung

Für Align und Recognize gilt dasselbe. Es rechnet immer nur eines von beiden, das andere wartet.

- **Vollbild:** Solange ein Spiel oder ein anderes Programm im Vollbild vorne läuft, rechnet das Plugin nicht. Die Statuszeile zeigt „Pausiert, solange ein Vollbildspiel läuft“. Gehörte Songs warten, bis das Spiel aus ist oder im Fenster läuft.
- **Rechenlast:** Beim Rechnen nutzt das Plugin 2 Kerne mit Priorität „unter normal“, beim Mitschnitt fast nichts (unter 0,1 % der CPU).
- **Arbeitsspeicher:** Im Leerlauf belegt ein Plugin etwa 30 MB, beim Rechnen bis 1,5 GB. Das Modell wird nur geladen, solange es Arbeit gibt. LyricHub erlaubt jedem Plugin höchstens 3 GB; ein Plugin, das mehr will, bekommt keinen Speicher, endet und startet neu.
- **Warteschlange:** Wer viel hört und lange spielt, sammelt Songs. Die Warteschlange fasst gut 2 Stunden Musik, danach fällt der älteste Song weg.

## Datenschutz

- **Was mitgeschnitten wird:** Nur der Ton des Players und seiner Unterprozesse, nie der Spielton oder das Mikrofon. Ist ein Browser der Player, ist es der Ton aller seiner Tabs.
- **Wo der Ton bleibt:** Er liegt nur im Arbeitsspeicher und wird nach der Ausrichtung verworfen. Er landet nie auf der Festplatte und verlässt den PC nicht.
- **Was gespeichert wird:** Nur Wortzeiten und Lyrics, in LyricHubs Cache auf deinem PC. Erkannte Texte werden nie hochgeladen.
- **Netzwerk:** Beide verbinden sich für den einmaligen Modell-Download mit huggingface.co. Recognize fragt außerdem lrclib.net, wie LyricHub selbst nur mit dem Titel des Songs, nie mit Ton oder Text.

## Anti-Cheat

Align und Recognize nutzen dieselbe Windows-Schnittstelle wie OBS oder Discord, um den Ton eines einzelnen Programms aufzunehmen (Process Loopback). Es greift nicht auf Spiele zu, schleust nichts ein und hängt sich nirgends ein. Anti-Cheat-Systeme haben keinen Grund, sich daran zu stören. Eine Garantie gibt es aber nicht: Kein Spielehersteller hat LyricHub oder seine Plugins geprüft.

## Speicherplatz

| Was | Wo | Größe |
|---|---|---|
| Programme | `%APPDATA%\LyricHub\plugins\lyrichub-align` und `…\lyrichub-recognize` | je 24 MB |
| Modell | `%LOCALAPPDATA%\LyricHub\plugins\lyrichub-align` und `…\lyrichub-recognize` | 588 MB, mit beiden Plugins nur einmal belegt |
| Wortzeiten und Lyrics | LyricHubs Cache | einige KB pro Song |

## Entfernen

Einstellungen → Plugins → „Entfernen“. Das löscht das Programm, das Modell und alles, was das Plugin geliefert hat. Nach Align fallen die Songs auf die geschätzten Wortzeiten zurück, nach Recognize auf das, was LRCLIB hat. Ist das andere Plugin noch da, behält es sein Modell.

## Grenzen

- **Ausgabelatenz:** Align hört den Ton, bevor er aus dem Lautsprecher kommt. Die Wörter leuchten deshalb um die Latenz des Ausgabegeräts zu früh. Bei Kabel merkt man das nicht, bei Bluetooth-Kopfhörern womöglich schon.
- **Song-Offset:** Wer nach der Ausrichtung den Offset eines Songs ändert, verschiebt die Wörter gegen den Gesang.
- **Japanisch und Koreanisch:** Das Modell kennt diese Schriften, getestet ist das aber nicht.
- **Erkennung:** Recognize kennt keine Wörterbücher und schreibt, was es hört. Das ergibt Tippfehler und manchmal falsche Wörter. Ist die Erkennung unsicher, zeigt Recognize lieber nichts: Im Test blieben 5 von 12 Songs ohne erkannte Lyrics, darunter einer, dessen Erkennung brauchbar gewesen wäre.

Wie genau die Plugins sind und wie sie gebaut sind, steht in den Berichten zu [Align](plugin-align.md), [Auto-Sync](auto-sync.md) und [Recognize](plugin-recognize.md).
