# FAQ

Englisch: [docs/en/faq.md](en/faq.md)

## Ist LyricHub mit Anti-Cheat sicher?

LyricHub schleust nichts in Spiele ein, liest nicht ihren Speicher und hängt sich nicht in DirectX. Es ist ein eigenes, durchklickbares Fenster, das oben bleibt. Welches Fenster vorne ist, erfährt es über normale Windows-Ereignisse zur Fensterreihenfolge, ohne Injection. Anti-Cheat-Systeme haben keinen Grund, sich daran zu stören, aber es gibt keine Garantie: Kein Spielehersteller hat LyricHub geprüft oder freigegeben.

## Funktioniert es im exklusiven Vollbild?

In einem getesteten Spiel ja. Blendet ein Spiel das Overlay trotzdem aus, zeigt das Tray einen Hinweis; dann hilft der randlose Fenstermodus des Spiels. Randlose und gefensterte Spiele funktionieren immer.

## Windows meldet „Der Computer wurde durch Windows geschützt“. Ist das ein Virus?

Nein. Der Installer ist nicht signiert, deshalb meldet Windows SmartScreen einen unbekannten Herausgeber. „Weitere Informationen“ → „Trotzdem ausführen“ wählen. In den Release-Notizen steht die SHA-256-Prüfsumme des Installers, damit lässt sich prüfen, ob der Download die veröffentlichte Datei ist.

## Sammelt LyricHub Daten?

Nein. Es gibt keine Telemetrie und kein Konto. Der einzige Online-Dienst, mit dem LyricHub spricht, ist [LRCLIB](https://lrclib.net), über HTTPS: Um Lyrics zu finden, schickt es Titel, Artist, Album und Länge des Songs. Lyrics, Einstellungen und Logs bleiben auf dem PC unter `%APPDATA%\LyricHub`. Der Diagnose-Export verbirgt die Songangaben, er kann also in einen öffentlichen Fehlerbericht. Die Plugins Align und Recognize, falls installiert, laden einmal ihr Modell von huggingface.co. Recognize fragt außerdem LRCLIB nach dem Titel des Songs. Ton und Text schicken beide nie weg (nächste Frage).

## Hören die Plugins mit? Was passiert mit dem Ton?

Align und Recognize schneiden nur den Ton des Players mit, nie den Spielton oder das Mikrofon. Align tut das nur, solange ein Song läuft, dessen Wortzeiten geschätzt sind, Recognize nur bei einem Song ohne synchrone Lyrics. Der Ton bleibt im Arbeitsspeicher, wird nach der Rechnung verworfen und verlässt den PC nicht. Gespeichert werden nur Wortzeiten und Lyrics, auch erkannte Texte nur lokal. Beide Plugins sind freiwillig und müssen erst installiert werden ([docs/plugins.md](plugins.md)).

## Sind Align und Recognize mit Anti-Cheat sicher?

Beide nehmen den Ton über dieselbe Windows-Schnittstelle auf wie OBS oder Discord. Sie greifen nicht auf Spiele zu und schleusen nichts ein. Solange ein Spiel im Vollbild läuft, rechnen sie nicht einmal. Eine Garantie gibt es wie bei LyricHub selbst nicht.

## Wie viel Platz und Leistung brauchen die Plugins?

Das Modell braucht 588 MB auf der Festplatte, auch mit beiden Plugins nur einmal, jedes Programm 24 MB. Im Leerlauf belegt ein Plugin etwa 30 MB Arbeitsspeicher. Nach einem Song rechnet es mit 2 Kernen bei niedriger Priorität und bis zu 1,5 GB Arbeitsspeicher, Align etwa das 1- bis 1,5-Fache der Songlänge. Es rechnet immer nur ein Plugin, und nur, solange kein Vollbildprogramm vorne läuft.

## Brauche ich Spotify Premium oder ein Spotify-Login?

Nein. LyricHub liest, was die Spotify-Desktop-App an Windows meldet, und braucht kein Login. Werbung und Podcasts werden übersprungen.

## Welche Musik-Apps funktionieren?

Die Spotify-Desktop-App aus dem Microsoft Store oder von spotify.com. Seit 1.1 lässt sich stattdessen experimentell ein Browser wählen, unter Einstellungen → Verhalten → Player: YouTube Music klappt in Edge und Firefox-basierten Browsern, normales YouTube zuverlässig in Edge und Chrome (Firefox-basierte Browser melden dort oft keine Zeit). Ein Browser gibt den Ton oft etwas später aus als Spotify; das gleicht der Offset für diesen Player unter Einstellungen → Synchronisation aus. Der Spotify-Webplayer klappt in Edge; in anderen Browsern ist er nicht getestet.

## Die Lyrics laufen zu früh oder zu spät. Was tun?

Mit Strg+Alt+, oder Strg+Alt+. die Lyrics des laufenden Songs 100 ms früher oder später schieben (mit Umschalt in 25-ms-Schritten). LyricHub merkt sich die Korrektur für diesen Song. Liegen alle Songs ungefähr gleich daneben, hilft die Kalibrierung unter Einstellungen → Synchronisation: Sie setzt einen Offset für das Audiogerät. Liegen einzelne Zeilen daneben, rückt das Plugin Align mit Auto-Sync sie ab dem zweiten Hören an den Gesang.

## Ein Song zeigt keine Lyrics.

LyricHub holt die Lyrics von LRCLIB, einer von der Community gepflegten Datenbank. Hat LRCLIB keine synchronen Lyrics zu einem Song, schrumpft das Overlay zu einer kleinen Karte mit Cover, Titel und Artist. Beim nächsten Song mit Lyrics wird es wieder groß. Das Plugin Recognize kann solche Songs ab dem zweiten Hören mit Lyrics versorgen: Es sucht breiter, richtet Text ohne Zeitstempel aus oder erkennt den Text aus dem Gesang ([docs/plugins.md](plugins.md)).

## Läuft es unter Windows 10?

Es ist für Windows 10 ab Version 2004 und Windows 11 gebaut, getestet ist es aber nur unter Windows 11.

## Warum „source available“ und nicht „Open Source“?

LyricHub steht unter der MIT-Lizenz mit der Commons Clause. Der Code ist öffentlich, jeder darf ihn nutzen, verändern und weitergeben, aber niemand darf ihn verkaufen. Wegen dieser einen Einschränkung ist es keine Open-Source-Lizenz im strengen Sinn, deshalb nennt sich LyricHub „source available“.

## Darf ich es beim Streamen nutzen?

Die Lizenz erlaubt es. Ob Songtexte im Stream gezeigt werden dürfen, hängt von den Regeln der Streaming-Plattform und den Rechteinhabern ab.

## Wie entferne ich es vollständig?

LyricHub unter Windows-Einstellungen → Apps deinstallieren. Um auch Einstellungen, Lyrics-Cache und Logs zu entfernen, den Ordner `%APPDATA%\LyricHub` löschen. Ein Plugin entfernt man vorher unter Einstellungen → Plugins → „Entfernen“; das löscht auch sein Modell unter `%LOCALAPPDATA%\LyricHub\plugins`.
