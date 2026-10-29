# Plugins

Englisch: [docs/en/plugins.md](en/plugins.md)

Plugins sind eigene Programme, die LyricHub startet und mit Daten versorgt. Sie sind freiwillig und lassen sich jederzeit ausschalten oder entfernen. Stürzt ein Plugin ab, läuft LyricHub weiter. Bisher gibt es ein Plugin, **Align**. Es liefert echte Wortzeiten aus dem Ton, damit die Wörter im Takt des Gesangs leuchten.

Plugins sind normale Programme mit deinen Rechten. Installiere nur Plugins, denen du vertraust. Bisher gibt es nur eigene Plugins von LyricHub, und zwar auf der [Release-Seite](https://github.com/HeiligerHoly/LyricHub/releases).

## Installieren

1. `lyrichub-align-0.1.0.zip` aus den Assets des Releases herunterladen. Nicht entpacken.
2. In LyricHub: Einstellungen → Plugins → „Plugin aus ZIP installieren …“ und die ZIP-Datei wählen.
3. LyricHub prüft die Datei und zeigt Name, Version, Rechte und die SHA-256-Prüfsumme. Die Prüfsumme sollte mit der in den Release-Notizen übereinstimmen. Dann „Installieren“.
4. Ein neu installiertes Plugin ist aus. Mit dem Schalter einschalten.

Beim ersten Einschalten lädt Align sein Modell herunter, 588 MB von huggingface.co. Den Fortschritt zeigt die Plugin-Seite. Ein abgebrochener Download setzt beim nächsten Start dort fort, wo er war.

## So arbeitet Align

LRCLIB hat fast nur Zeilenzeiten. Für die einzelnen Wörter schätzt LyricHub deshalb, wann sie gesungen werden. Align ersetzt diese Schätzung durch Messwerte:

1. **Beim ersten Hören** schneidet Align den Ton des Players mit. Die Statuszeile zeigt dann „Hört mit“.
2. **Nach dem Song** richtet Align den Liedtext auf den Gesang aus („Richtet einen Song aus: 45 %“). Das dauert etwa das 1- bis 1,5-Fache der Songlänge.
3. **Ab dem zweiten Hören** kommen die Wortzeiten aus dem Cache. Die Wörter leuchten im Takt, mit Wort-Glow. Unter Diagnose steht dann bei „Wortzeiten“: „vom Plugin lyrichub-align“.

Ausgerichtet wird nur ein Song, der zu mindestens 85 % gehört wurde. Zeilen, die beim Mitschnitt fehlten (übersprungen, gespult), bleiben geschätzt. Songs mit echten Wortzeiten aus der LRC-Datei lässt Align in Ruhe.

## Spiele und Leistung

- **Vollbild:** Solange ein Spiel oder ein anderes Programm im Vollbild vorne läuft, rechnet Align nicht. Die Statuszeile zeigt „Pausiert, solange ein Vollbildspiel läuft“. Gehörte Songs warten, bis das Spiel aus ist oder im Fenster läuft.
- **Rechenlast:** Beim Rechnen nutzt Align 2 Kerne mit Priorität „unter normal“, beim Mitschnitt fast nichts (unter 0,1 % der CPU).
- **Arbeitsspeicher:** Im Leerlauf belegt Align etwa 30 MB, beim Rechnen bis 1,5 GB. Das Modell wird nur geladen, solange es Arbeit gibt.
- **Warteschlange:** Wer viel hört und lange spielt, sammelt Songs. Die Warteschlange fasst gut 2 Stunden Musik, danach fällt der älteste Song weg.

## Datenschutz

- **Was mitgeschnitten wird:** Nur der Ton des Players und seiner Unterprozesse, nie der Spielton oder das Mikrofon. Ist ein Browser der Player, ist es der Ton aller seiner Tabs.
- **Wo der Ton bleibt:** Er liegt nur im Arbeitsspeicher und wird nach der Ausrichtung verworfen. Er landet nie auf der Festplatte und verlässt den PC nicht.
- **Was gespeichert wird:** Nur die Wortzeiten, in LyricHubs Cache auf deinem PC.
- **Netzwerk:** Align verbindet sich nur für den einmaligen Modell-Download mit huggingface.co.

## Anti-Cheat

Align nutzt dieselbe Windows-Schnittstelle wie OBS oder Discord, um den Ton eines einzelnen Programms aufzunehmen (Process Loopback). Es greift nicht auf Spiele zu, schleust nichts ein und hängt sich nirgends ein. Anti-Cheat-Systeme haben keinen Grund, sich daran zu stören. Eine Garantie gibt es aber nicht: Kein Spielehersteller hat LyricHub oder Align geprüft.

## Speicherplatz

| Was | Wo | Größe |
|---|---|---|
| Programm | `%APPDATA%\LyricHub\plugins\lyrichub-align` | 24 MB |
| Modell | `%LOCALAPPDATA%\LyricHub\plugins\lyrichub-align` | 588 MB |
| Wortzeiten | LyricHubs Cache | einige KB pro Song |

## Entfernen

Einstellungen → Plugins → „Entfernen“. Das löscht das Programm, das Modell und die Wortzeiten, die Align geliefert hat. Die Songs fallen auf die Schätzung zurück.

## Grenzen

- **Ausgabelatenz:** Align hört den Ton, bevor er aus dem Lautsprecher kommt. Die Wörter leuchten deshalb um die Latenz des Ausgabegeräts zu früh. Bei Kabel merkt man das nicht, bei Bluetooth-Kopfhörern womöglich schon.
- **Song-Offset:** Wer nach der Ausrichtung den Offset eines Songs ändert, verschiebt die Wörter gegen den Gesang.
- **Japanisch und Koreanisch:** Das Modell kennt diese Schriften, getestet ist das aber nicht.

Wie genau Align ist und wie es gebaut ist, steht im [Bericht zu P3](plugin-align.md).
