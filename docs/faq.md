# FAQ

Englisch: [docs/en/faq.md](en/faq.md)

## Ist LyricHub mit Anti-Cheat sicher?

LyricHub schleust nichts in Spiele ein, liest nicht ihren Speicher und hängt sich nicht in DirectX. Es ist ein eigenes, durchklickbares Fenster, das oben bleibt. Welches Fenster vorne ist, erfährt es über normale Windows-Ereignisse zur Fensterreihenfolge, ohne Injection. Anti-Cheat-Systeme haben keinen Grund, sich daran zu stören, aber es gibt keine Garantie: Kein Spielehersteller hat LyricHub geprüft oder freigegeben.

## Funktioniert es im exklusiven Vollbild?

In einem getesteten Spiel ja. Blendet ein Spiel das Overlay trotzdem aus, zeigt das Tray einen Hinweis; dann hilft der randlose Fenstermodus des Spiels. Randlose und gefensterte Spiele funktionieren immer.

## Windows meldet „Der Computer wurde durch Windows geschützt“. Ist das ein Virus?

Nein. Der Installer ist nicht signiert, deshalb meldet Windows SmartScreen einen unbekannten Herausgeber. „Weitere Informationen“ → „Trotzdem ausführen“ wählen. In den Release-Notizen steht die SHA-256-Prüfsumme des Installers, damit lässt sich prüfen, ob der Download die veröffentlichte Datei ist.

## Sammelt LyricHub Daten?

Nein. Es gibt keine Telemetrie und kein Konto. Der einzige Online-Dienst, mit dem LyricHub spricht, ist [LRCLIB](https://lrclib.net), über HTTPS: Um Lyrics zu finden, schickt es Titel, Artist, Album und Länge des Songs. Lyrics, Einstellungen und Logs bleiben auf dem PC unter `%APPDATA%\LyricHub`. Der Diagnose-Export verbirgt die Songangaben, er kann also in einen öffentlichen Fehlerbericht.

## Brauche ich Spotify Premium oder ein Spotify-Login?

Nein. LyricHub liest, was die Spotify-Desktop-App an Windows meldet, und braucht kein Login. Werbung und Podcasts werden übersprungen.

## Welche Musik-Apps funktionieren?

Die Spotify-Desktop-App, aus dem Microsoft Store oder von spotify.com. Der Spotify-Webplayer und andere Musik-Apps werden noch nicht unterstützt.

## Die Lyrics laufen zu früh oder zu spät. Was tun?

Mit Strg+Alt+, oder Strg+Alt+. die Lyrics des laufenden Songs 100 ms früher oder später schieben (mit Umschalt in 25-ms-Schritten). LyricHub merkt sich die Korrektur für diesen Song. Liegen alle Songs ungefähr gleich daneben, hilft die Kalibrierung unter Einstellungen → Synchronisation: Sie setzt einen Offset für das Audiogerät.

## Ein Song zeigt keine Lyrics.

LyricHub holt die Lyrics von LRCLIB, einer von der Community gepflegten Datenbank. Hat LRCLIB keine synchronen Lyrics zu einem Song, schrumpft das Overlay zu einer kleinen Karte mit Cover, Titel und Artist. Beim nächsten Song mit Lyrics wird es wieder groß.

## Läuft es unter Windows 10?

Es ist für Windows 10 ab Version 2004 und Windows 11 gebaut, getestet ist es aber nur unter Windows 11.

## Warum „source available“ und nicht „Open Source“?

LyricHub steht unter der MIT-Lizenz mit der Commons Clause. Der Code ist öffentlich, jeder darf ihn nutzen, verändern und weitergeben, aber niemand darf ihn verkaufen. Wegen dieser einen Einschränkung ist es keine Open-Source-Lizenz im strengen Sinn, deshalb nennt sich LyricHub „source available“.

## Darf ich es beim Streamen nutzen?

Die Lizenz erlaubt es. Ob Songtexte im Stream gezeigt werden dürfen, hängt von den Regeln der Streaming-Plattform und den Rechteinhabern ab.

## Wie entferne ich es vollständig?

LyricHub unter Windows-Einstellungen → Apps deinstallieren. Um auch Einstellungen, Lyrics-Cache und Logs zu entfernen, den Ordner `%APPDATA%\LyricHub` löschen.
