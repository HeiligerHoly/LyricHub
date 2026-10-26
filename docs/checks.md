# Sicherheit, Performance und Speicherplatz

> Datum: 2026-10-26 · Stand: nach `v0.5.0-phase5` · Release-Build, Windows 11, 16 Kerne

## Sicherheit

| Prüfung | Ergebnis |
|---|---|
| Content Security Policy | **War aus** (`csp: null`), jetzt an: Skripte, Styles, Schriften und Bilder nur aus der App selbst (Inline-Styles für Svelte, `data:`-Bilder für die Cover), IPC über `ipc:`. Ein in die Seite eingeschleustes Inline-Skript wird in beiden Fenstern blockiert (geprüft im Release-Build). Der Dev-Server läuft weiter ohne CSP. |
| Ungefiltertes HTML | Keine Stelle: kein `{@html}`, kein `innerHTML`, kein `eval`. Titel, Artist und Lyrics landen als Text im DOM. |
| Tauri-Berechtigungen | Nur `core:default` für beide Fenster, kein Dateisystem-, Shell- oder Opener-Zugriff aus der Webseite. Ordner öffnet das Backend selbst. |
| Eingaben aus den Einstellungen | Farben auf `#rrggbb` geprüft, Schriftname ohne Zeichen, die CSS beenden könnten, Kürzel nur mit Modifier (außer F1–F12), Zahlen begrenzt (`sanitize`, Unit-Tests). |
| Netzwerk | Nur LRCLIB über HTTPS (native TLS mit Zertifikatsprüfung), eigener User-Agent, 4 s Timeout. Neu: Antworten höchstens 4 MB statt unbegrenzt. Keine Telemetrie, keine anderen Verbindungen. |
| Cover | Aus der Windows-Media-Session, höchstens 4 MB, MIME-Typ aus den Bytes bestimmt, nur als `data:`-Bild. |
| SQL | Alle Abfragen mit Parametern, keine aus Strings zusammengesetzten Befehle. |
| Spiele | Keine DLL-Injection, keine Hooks im Spielprozess: Die WinEvent-Hooks laufen außerhalb des Spielprozesses (`WINEVENT_OUTOFCONTEXT`). |
| Rust-Abhängigkeiten | `cargo audit`: 558 Crates, **0 Sicherheitslücken**. 5 Warnungen (nicht mehr gepflegt bzw. unsicher: `glib`, `zbus_names`, `zvariant`, `zvariant_derive`, `proc-macro-error`) betreffen nur den Linux-Teil von Tauri und kommen im Windows-Build nicht vor (`cargo tree --target x86_64-pc-windows-msvc`). |
| npm-Abhängigkeiten | Was in die App kommt (`npm audit --omit=dev`): **0 Lücken**. 3 Lücken mit niedrigem Schweregrad in `cookie` über `@sveltejs/kit` betreffen nur die Build-Werkzeuge; die App nutzt kein Server-Rendering und keine Cookies. |
| Geheimnisse und persönliche Daten im Repo | Keine Schlüssel, Tokens oder Passwörter. Pfade nur über `%APPDATA%`/`%LOCALAPPDATA%`. **Achtung:** Ältere Fassungen der Spike-Berichte in der Git-Historie enthalten den Fenstertitel eines Discord-Servers. Vor einer Veröffentlichung das Repo neu anlegen oder die Historie bereinigen (Entscheidung des Nutzers). |
| Debug-Zugang | Den DevTools-Port gibt es nur, wenn man ihn über eine Umgebungsvariable selbst setzt (Entwicklung); im Release ist er aus. |
| Installer | Nur aktueller Benutzer, kein Administrator. Nicht signiert: SmartScreen warnt vor einem unbekannten Herausgeber. |

## Performance

| Messung | Ergebnis |
|---|---|
| Start bis sichtbares Overlay | 0,46 s; Lyrics aus dem Cache nach 0,49 s |
| CPU spielend / pausiert | 0,48 % / 0,03 % aller 16 Kerne (Ziele 2 % / 0,5 %) |
| Speicher privat (LyricHub + WebView2) | 169–179 MB (Ziel < 200 MB) |
| Layouts der Overlay-Seite | 0,2 pro Sekunde während der Wiedergabe, also keins pro Frame (Phase 4) |
| Release-Profil | LTO, eine Codegen-Einheit, `opt-level = 3`, `panic = "abort"`, Symbole entfernt (schon seit Phase 0) |

## Speicherplatz

| Bereich | Vorher | Nachher |
|---|---|---|
| WebView2-Ordner (`%LOCALAPPDATA%\dev.lyrichub.app`) | 91 MB | 23 MB: `--disable-component-update` lädt keine DRM-, Filter-, Sprach- oder Silbentrennungs-Komponenten mehr; vorhandene löscht LyricHub beim Start; HTTP-Cache auf 1 MB begrenzt |
| Logs | Debug-Ausgabe auch im Release, rund 1,4 MB pro Tag bei 14 aufbewahrten Tagen | nur `info` im Release (etwa ein Viertel), Debug nur in Entwicklungs-Builds; nur die zehn neuesten Selbsttest-Berichte bleiben |
| Lyrics-Datenbank | Write-Ahead-Log bis 4 MB, Cache unbegrenzt | WAL höchstens 1 MB (Checkpoint alle 200 Seiten); Lyrics von Songs, die ein Jahr nicht liefen, und abgelaufene Fehlschläge werden beim Start entfernt. Song- und Geräte-Offsets bleiben. |
| Programmdatei | 57 MB | unverändert, bewusst: 45 MB davon sind das Romaji-Wörterbuch (IPADIC). Unkomprimiert eingebettet bleibt es im gemappten Programmabbild; komprimiert wären es etwa 7 MB, aber es läge dann entpackt im privaten Speicher (+45 MB) und die App über 200 MB (`docs/romaji.md`). |
| Installer | 10 MB | unverändert (NSIS komprimiert das Wörterbuch) |
