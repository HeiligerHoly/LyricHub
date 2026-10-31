# LyricHub – Ideen für die nächsten Versionen

> Stand: 2026-10-31 · Basis: 1.2.0 plus Auto-Sync und Recognize (für 1.3.0 gebaut, noch nicht veröffentlicht)
> Herkunft: vier Fable-Agenten mit je einem Blickwinkel (Kern-App, Plugins, Gaming/VR/Streaming, Technik und Verteilung), zusammengeführt; die Aussagen über den heutigen Code sind nachgeprüft.
> Status: **Vorschlag. Nichts davon ist entschieden oder gebaut.** Die Entscheidungen stehen in Abschnitt 6.

## 1. Kurzfassung

Vier Pakete, jedes für sich lieferbar:

| Paket | Inhalt | Warum zuerst |
|---|---|---|
| **A. Fundament** (1.3.x) | Absturzspuren, Cache prüft Plugin-Lyrics nach, Speichergrenze für Plugins, CI, Release-Skript härten | klein, schließt echte Lücken, macht jede weitere Phase sicherer |
| **B. Mehr Songs mit Lyrics** (1.4) | Nightcore/Sped-up aus dem Original, Auswahl unter LRCLIB-Treffern, lokale LRC-Dateien | trifft die größte Lücke: im Cache hatten 21 von 47 Songs keine Lyrics |
| **C. VR** (1.5) | SteamVR-Overlay als Plugin, VR-Modus im Kern, VRChat-Chatbox per OSC | der eigene Anwendungsfall: in VRChat ist das Desktop-Overlay unsichtbar |
| **D. Danach** | Übersetzen, weitere Romanisierungen, Spielprofile, Gamepad-Tasten, Update-Pfad, OBS-Quelle und mehr | nach Bedarf |

Empfohlene Reihenfolge: **A, dann B oder C**. A braucht wenig und senkt das Risiko für alles Weitere. Zwischen B und C entscheidet, was öfter fehlt: Lyrics zu Nightcore- und Underground-Songs (B) oder Lyrics im Headset (C).

## 2. Paket A – Fundament

| Idee | Was | Beleg im Code | Aufwand | Prüfung |
|---|---|---|---|---|
| **Absturzspuren** | Panic-Hook in der App, der Ort, Version und die letzten Logzeilen in `logs\crash-<zeit>.txt` schreibt; optional ein lokaler Minidump. Plugin-stderr im Log auf `warn` statt `debug`. Diagnose zeigt „letzter Absturz“ und nimmt die Datei in den Export auf. | `src-tauri/Cargo.toml` hat `panic = "abort"` und `strip = true`, es gibt keinen Panic-Hook; Plugin-stderr wird nur mit `debug` geloggt (`plugins/mod.rs`), Release loggt ab `info`. Ein Absturz hinterlässt heute nichts. | S | Test-Schalter erzeugt eine Crash-Datei; die Panic-Zeile eines Plugins steht im Release-Log |
| **Cache prüft nach** | Lyrics von Plugins und reine Texte ohne Zeitstempel nach z. B. 14 Tagen sofort zeigen und im Hintergrund LRCLIB neu fragen; synchrone Lyrics ersetzen sie. | `lyrics/service.rs` liefert jeden Cache-Treffer für immer; der Recognize-Bericht nennt das als Grenze. Erkannte Lyrics mit etwa 20 % Wortfehlern verdecken sonst dauerhaft echte. | M | Unit-Test mit aufgezeichneten Antworten: alter Plugin-Treffer ergibt genau eine Anfrage |
| **Speichergrenze für Plugins** | `JOB_OBJECT_LIMIT_PROCESS_MEMORY` (etwa 2 GB) im Job-Objekt der Plugins. | `plugins/process.rs` setzt nur „beenden mit LyricHub“. Die Doku verspricht unter 1,5 GB, nichts erzwingt es. | S | Testplugin belegt 3 GB, wird beendet und neu gestartet |
| **CI auf GitHub Actions** | Windows-Lauf: Tests von App und Plugins, `npm run check`, clippy, `cargo audit`, `npm audit`. Releases bleiben lokal. | Es gibt kein `.github/workflows`. Über 160 Tests laufen nur, wenn jemand daran denkt. | M | grüner Lauf unter 25 min mit Cache |
| **Release-Skript härten** | `tools/build-release.ps1` lässt vorher die Tests und `cargo audit` laufen, baut mit `--locked`, schreibt `SHA256SUMS.txt` und bricht ab, wenn der Arbeitsbaum nicht sauber ist. | Heute baut es nur; die Prüfsummen stehen von Hand in den Release-Notizen. | S | Skript verweigert bei rotem Test |

## 3. Paket B – Mehr Songs mit Lyrics

| Idee | Was | Aufwand | Risiken | Prüfung |
|---|---|---|---|---|
| **Nightcore, Sped up, Slowed** | Zusätze im Titel erkennen, das Original bei LRCLIB suchen, Kandidaten mit passendem Längenverhältnis (etwa 1,1–1,45 schneller oder 0,7–0,9 langsamer) annehmen und alle Zeiten strecken. Im Overlay als „gestreckt“ markiert. Heute schließt der Abgleich solche Versionen aus: mehr als 8 s Längenunterschied gilt als andere Ausgabe (`lyrics/matcher.rs`). | M | Remixe mit anderem Aufbau passen nicht; „Lyrics verwerfen“ reicht als Ausweg | Anteil der heutigen Fehltreffer, der zum Treffer wird; Sync-Protokoll an drei Nightcore-Songs im Median höchstens 100 ms |
| **Falsche Lyrics? Treffer wählen** | Unter Einstellungen → Sync die LRCLIB-Treffer mit Titel, Artist, Länge und „synchron ja/nein“ zeigen; die Wahl wird pro Song gemerkt, dazu „für diesen Song keine Lyrics“. Steht schon als Idee in PLAN.md 13.8. | M | keine; nur Anzeige | Trefferquote auf den Fehltreffern vorher und nachher |
| **Lokale LRC-Dateien** | Eigene `.lrc`-Dateien in `%APPDATA%\LyricHub\lrc` haben Vorrang vor LRCLIB. Später ein Tap-Editor auf Basis der Kalibrierung, der Zeilen neu stempelt, nur lokal, ohne Upload. | S (Ordner), L (Editor) | Copyright: bleibt lokal | Datei im Ordner erscheint beim nächsten Songwechsel |

Recognize ergänzt das: Es hilft erst ab dem zweiten Hören, B schon beim ersten.

## 4. Paket C – VR

| Idee | Was | Kern oder Plugin | Aufwand | Risiken | Prüfung |
|---|---|---|---|---|---|
| **Fähigkeit `lyrics_display`** | Neue Plugin-Fähigkeit: das Plugin bekommt die fertigen Zeilen mit den endgültigen Wortzeiten (nach Align, Auto-Sync und Offsets) und die Uhr. Grundlage für VR, OSC, OBS-Datei, Stream Deck. | Kern | S–M | Text verlässt LyricHub in ein anderes lokales Programm: Plugin muss die Fähigkeit erklären | Testplugin folgt der Uhr wie das Overlay |
| **SteamVR-Overlay** | Lyrics im Headset in jedem SteamVR-Spiel, auch VRChat: OpenVR-Overlay-API, Text in Rust gerastert, Bild nur bei Wortwechsel neu; Position am Kopf (unten), am Controller („Armbanduhr“) oder fest im Raum; Tasten über SteamVRs eigene Belegung. OpenXR hilft nicht: SteamVR unterstützt dessen Overlay-Erweiterung nicht. | Plugin `lyrichub-vr` | L | VRChat stuft OpenVR-Overlays wie OVR Toolkit und XSOverlay ausdrücklich als zulässig ein, auch mit EAC; nichts verlässt den PC | in VRChat sichtbar; 10 min SteamVR ohne zusätzliche verlorene Frames; unter 1 % CPU und 100 MB |
| **VR-Modus im Kern** | Läuft SteamVR, blendet LyricHub das Desktop-Overlay aus und gibt die Hotkeys ans VR-Plugin. | Kern | S | keine | SteamVR an: Overlay nach 2 s weg; aus: wieder da |
| **VRChat-Chatbox per OSC** | Mitspieler sehen „♫ Titel – Artist“ über dem Avatar; als zweiter Modus auch die aktuelle Zeile. OSC an 127.0.0.1:9000, höchstens 144 Zeichen, gedrosselt auf ein Update pro 1,5 s (VRChat sperrt bei schnellerem Senden still; das Limit ist nicht offiziell dokumentiert und muss vorher gemessen werden). | Plugin `lyrichub-osc` | S–M | **Urheberrecht:** Der Zeilen-Modus zeigt Liedtext Dritten, das ist öffentliche Wiedergabe. Deshalb Standard „nur Titel“, Zeilen nur mit deutlichem Hinweis, eventuell nur in Friends- und Invite-Instanzen | zweiter Account sieht jeden Wechsel, keine Sperre bei 1,5 s |

## 5. Paket D – Danach

| Idee | Was | Aufwand | Anmerkung |
|---|---|---|---|
| **Übersetzen** (Plugin) | Jede Zeile offline in die Sprache der Oberfläche, als Zweitzeile wie Romaji. Neue Fähigkeit `line_annotation`. Modelle: Mozillas Bergamot-Modelle (MPL-2.0) oder OPUS-MT (CC-BY 4.0, ONNX int8 etwa 200 MB pro Richtung). | M | wirkt ab dem ersten Hören; Übersetzung ist ein abgeleitetes Werk, also nur lokal anzeigen; Lizenz pro Sprachpaar prüfen |
| **Romanisierung+** (Plugin) | Koreanisch, Chinesisch (Pinyin), Kyrillisch, Griechisch, Thai; Japanisch bleibt im Kern. Gleiche Fähigkeit wie Übersetzen. | S nach dem Übersetzen | deterministisch, MIT-Crates |
| **Original und Romaji gleichzeitig** | Zwei Zeilen untereinander statt Ersetzen; im Stack-Layout Zahl der Zeilen davor und danach wählbar. | S | feste Zeilenhöhen, damit das Fenster nicht springt |
| **Overlay-Profile pro Spiel** | Position, Größe, Layout und Theme je Programm, „für dieses Spiel merken“ im Bearbeitungsmodus. Den Programmnamen des Vordergrundfensters kennt LyricHub schon. | M | laut Web-Vergleich hat das kein vergleichbares Programm |
| **Gamepad-Tasten** | Overlay an/aus, Nudge und Layout über Tastenkombinationen am Controller (XInput, alle 100 ms abgefragt, nur lesend). Kam von zwei Agenten. | S–M | Standard aus, frei belegbar |
| **Update-Pfad** | Manueller Knopf „Nach Update suchen“ und auf Wunsch ein täglicher Check (Tauri-Updater mit eigenem Signaturschlüssel); dazu ein winget-Eintrag. Heute erfahren Nutzer von Updates nur über GitHub. | M | standardmäßig ohne Anfrage ins Netz |
| **Code-Signierung** | Erst entscheiden, nicht sofort kaufen: Azure Artifact Signing für etwa 10 $ im Monat; SignPath Foundation wäre kostenlos, verlangt aber eine OSI-Lizenz, und MIT mit Commons Clause ist keine. | S | lohnt sich erst bei vielen Downloads pro Version |
| **OBS-Browserquelle** | Lyrics als eigene Ebene im Stream: lokaler Server nur auf 127.0.0.1 mit Token, dieselben Svelte-Layouts; dazu `now-playing.txt`. | M | Lyrics im Stream sind Wiedergabe vor Publikum; Standard aus |
| **Erststart-Assistent** | Player erkennen, Overlay mit Beispielzeilen platzieren, optional kalibrieren, Hotkey-Karte. | M | verkettet nur vorhandene Funktionen |
| **Instrumental-Dimmen** | Bei Pausen über 10 s ohne Gesang dimmen, 1 s vor der nächsten Zeile zurück; dazu ein Hotkey, der ein verstecktes Overlay 5 s einblendet. | S | |
| **Latenz messen** (Plugin) | Echte Ausgabelatenz per Mikrofon gegen Aligns Mitschnitt, als Vorschlag für den Geräte-Offset (hilft bei Bluetooth-Lautsprechern). | M | braucht die Erlaubnis `audio:microphone`; geschlossene Kopfhörer hört das Mikrofon nicht |
| **Kommandozeile** | `lyrichub.exe --toggle`, `--nudge -100` an die laufende Instanz, für Stream Deck und AutoHotkey ohne Plugin. | S | |
| **Plugin-Rechenleistung** | Wahl „schonend / normal / voll“ oder automatisch mehr Threads, wenn der Nutzer länger nichts tut. Eine Grafikkarte bringt beim int8-Modell wenig. | S | Messung: Verhältnis Rechenzeit zu Songlänge, Genauigkeit unverändert |
| **Beat-Puls** (Plugin) | Overlay pulst im Takt, Countdown bis zur nächsten Gesangszeile. | M | eigener Detektor, GPL-Bibliotheken scheiden aus |
| **Mitsingen** (Plugin) | Tonhöhe des Nutzers gegen die Melodie, Punkte pro Zeile. | L | braucht Stimmtrennung und Mikrofon; erst ein Spike |
| **Song per Audio erkennen** (Plugin) | Bei schlechten Browser-Metadaten den Song per Fingerabdruck bestimmen. | L | schickt einen Fingerabdruck an acoustid.org; widerspricht fast der Linie „alles lokal“, nur mit ausdrücklicher Zustimmung |

**Plugins von anderen:** Technisch geht das heute schon, das Protokoll ist JSON über stdin/stdout. Sicher geöffnet heißt: Protokoll als `api: 1` dokumentieren und einfrieren, ein kleines SDK-Crate aus dem Code von Align herauslösen, Plugin-ZIPs signieren (Ed25519) und optional ein von LyricHub signierter Katalog. Ehrlich bleiben: Ohne Sandbox sind Rechte nur Erklärungen; durchsetzen lassen sich Nachrichten pro Fähigkeit, Größen- und Ratenlimits und das Job-Objekt. Seit dieser Nacht nimmt LyricHub Lyrics nur noch von Plugins mit der Fähigkeit `lyrics_source` an.

**Verworfen:** eigene Wortzeiten oder erkannte Texte zu LRCLIB hochladen (das wäre Weitergabe von Liedtext), weitere Lyrics-Dienste wie Musixmatch (Nutzungsbedingungen), Discord Rich Presence (Spotify meldet Discord das schon, Zeilen scheitern am Rate-Limit), Steam-Overlay (keine öffentliche Schnittstelle), Aufwand für Windows 10 (ohne Support seit Oktober 2025; ein FAQ-Hinweis reicht).

## 6. Entscheidungen für den Nutzer

1. **Welches Paket zuerst?** Empfehlung: A, dann B oder C.
2. **VR:** SteamVR-Overlay bauen? Dafür braucht es zum Testen eine Brille und SteamVR, also deine Mithilfe.
3. **VRChat-Chatbox:** nur den Titel (rechtlich sauber) oder auch die aktuelle Zeile als ausdrücklich eingeschaltete Option?
4. **Update-Pfad:** nur der manuelle Knopf, oder auch ein täglicher Check auf Wunsch?
5. **Code-Signierung:** vorerst nicht, oder etwa 10 $ im Monat für Azure Artifact Signing?

## 7. Quellen der Agenten

Vergleichbare Programme: [Lyric Overlay](https://alternativeto.net/software/lyric-overlay/about), [Lyricify](https://docs.lyricify.app/en/lyricify-4/getting-started/), [Musixmatch FloatingLyrics](https://musixmatch.helpscoutdocs.com/article/76-floatinglyrics-what-is-it-how-does-it-work), [Beautiful Lyrics](https://gittrend.io/repo/surfbryce/beautiful-lyrics), [Moegi](https://www.github.com/sglkc/moegi). VR und VRChat: [VRChat OSC](https://docs.vrchat.com/docs/osc-as-input-controller), [Chatbox](https://wiki.vrchat.com/wiki/Chatbox), [Rate-Limit-Diskussion](https://feedback.vrchat.com/bug-reports/p/chatbox-rate-limiting-too-aggressive), [VRChat Security Update (EAC, Overlays)](https://hello.vrchat.com/blog/vrchat-security-update), [IVROverlay](https://github.com/ValveSoftware/openvr/wiki/IVROverlay_Overview), [OpenVR-Lizenz](https://github.com/ValveSoftware/openvr/blob/master/LICENSE). Modelle: [OPUS-MT ONNX int8](https://huggingface.co/Zweiarm/opus-mt-onnx-int8), [firefox-translations](https://pypi.org/project/firefox-translations/). Signierung: [Azure Artifact Signing](https://azure.microsoft.com/en-us/products/artifact-signing), [SignPath-Bedingungen](https://signpath.org/terms). Windows 10: [Edge/WebView2 bis 2028](https://borncity.com/win/2025/08/12/windows-10-22h2-microsoft-edge-will-receive-updates-until-october-2028/).
