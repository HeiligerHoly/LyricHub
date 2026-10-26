# Phase 5 – Robustheit und Release

> Datum: 2026-10-26 · Bezug: PLAN.md 10 (Phase 5), 2, 6.3, 6.4, 9.6–9.8, 14.2
> Ergebnis: **Phase 5 gebaut und geprüft** (Tag `v0.5.0-phase5`). Das Overlay bleibt auch gegen ein Spiel oben, das sich alle 500 ms selbst nach vorne setzt: `game-aggressive` ist zum ersten Mal grün. Dazu kommen Kürzel, die sich ändern und abschalten lassen, ein eigenes App-Icon und ein NSIS-Installer ohne Administratorrechte. Der Release-Build braucht 179 MB privaten Speicher und 0,48 % CPU. Offen ist der Installer-Test auf einem fremden PC; ihn macht ein Freund des Nutzers.

## Was gebaut wurde

| Bereich | Umsetzung | Dateien |
|---|---|---|
| Immer oben (6.3) | Out-of-context-WinEvent-Hooks für `EVENT_OBJECT_REORDER` und `EVENT_SYSTEM_FOREGROUND`, ohne Injection. Landet ein Topmost-Fenster über dem Overlay und überdeckt es, setzt sich das Overlay wieder nach oben, höchstens zehnmal pro Sekunde. Menüs, Tooltips, Taskleiste, Start, Alt+Tab und IME bleiben unberührt. Der 2-s-Wächter bleibt als Rückfallebene. | `overlay/guard.rs` |
| Vollbild-Erkennung | Einmal pro Sekunde: Vordergrund normal, randloses Vollbild (Fenster bedeckt seinen Monitor) oder exklusives Vollbild (`SHQueryUserNotificationState`). Neue Option „Nur bei Vollbild-App zeigen“; Edit-Modus und Vorschau zeigen das Overlay immer. | `guard.rs`, `overlay/mod.rs`, `OverlayPage.svelte` |
| Hinweis im Tray | Bleibt ein Fenster länger als 2 s über dem Overlay, nennen Tooltip und ein erster, grauer Menüeintrag das Fenster und empfehlen „Randloses Fenster“. Im exklusiven Vollbild steht ein vorsichtiger Hinweis, weil Windows das Overlay dort meist trotzdem zeigt. | `tray.rs` |
| Monitore (6.4) | Ein verstecktes Fenster des Wächters empfängt `WM_DISPLAYCHANGE` und platziert das Overlay neu; die 2-s-Abfrage bleibt. Fehlt der eingestellte Monitor, gilt der Hauptbildschirm (seit Phase 1). | `guard.rs` |
| Schlafmodus | `WM_POWERBROADCAST`/`PBT_APMRESUMEAUTOMATIC`: Die SMTC-Verbindung wird neu aufgebaut, das Overlay neu platziert. | `guard.rs`, `sources/smtc.rs` |
| Start | Single-Instance und minimierter Autostart gab es seit Phase 0/1. Neu: Ein zweiter Start mit `--minimized` öffnet keine Einstellungen. | `lib.rs` |
| Hotkeys (9.7, Punkt 6) | Alle acht Kürzel kommen aus den Einstellungen (`hotkeys`), lassen sich ändern, abschalten und zurücksetzen. Aufnahme per Tastendruck, solange alle Kürzel pausiert sind. Ein Kürzel braucht Strg, Alt, Umschalt oder Win (außer F1–F12); Enter, Esc und Leertaste bleiben für Edit-Modus und Messung reserviert. Jedes wird einzeln registriert, sein Zustand (aktiv, aus, ungültig, doppelt, reserviert, von anderem Programm belegt) steht auf der Seite. Das Tray-Menü zeigt die eingestellten Kürzel. | `hotkeys.rs`, `HotkeysPage.svelte` |
| Fehler-UX | Hinweisbalken im Einstellungsfenster, wenn Kürzel nicht aktiv sind oder LRCLIB für den aktuellen Song nicht erreichbar ist. Im Spiel nur die dezente Marke aus Phase 4. | `App.svelte` |
| Diagnose-Export | Zusätzlich Monitore, Kürzel-Zustände, Wächter-Status und die letzten 400 Log-Zeilen (Logs enthalten keine Liedtexte). | `sync_lab.rs` |
| App-Icon | „Half-Lit Notes“, vom Nutzer aus zwölf Entwürfen gewählt (ersetzt die erste Sprechblase): eine Doppelnote auf einer Kachel, durch die die Karaoke-Füllung läuft. Links ist die Kachel amber und die Note dunkel, rechts die Kachel dunkel und die Note amber. Keine Markenfarben. Vorlage `src-tauri/icons/app-icon.svg` (1024), alle Größen über `npx tauri icon`. Der Tray nutzt eine Variante mit breiterem, hellerem Rand (`icons/tray.svg`, vorgerendert für 16–48 px, Auswahl nach System-DPI): Auf dunkler Taskleiste hat die rechte Kachelkante damit 3,5:1 Kontrast in Farbe und 2,5:1 in Grau statt 1,03:1. App-Icon und Installer bleiben unverändert. | `src-tauri/icons/` |
| Installer | NSIS über den Tauri-Bundler, `installMode: currentUser` (kein UAC-Dialog), Deutsch und Englisch: `LyricHub_0.5.0_x64-setup.exe`, 10 MB. Kein Code-Signing, Lizenz bleibt `UNLICENSED` (14.2). | `tauri.conf.json` |
| Speicher | `--disable-gpu-rasterization` für beide WebViews: GPU-Prozess etwa 14 MB kleiner, CPU unverändert. | `tauri.conf.json`, `settings_window.rs` |
| Prüfwerkzeuge | `start-focus-test.ps1` (Start ohne Fokusklau). `run-spike.ps1` misst jetzt den Wächter; `-TopmostMs` bleibt für den Vergleich aus Spike B. | `tools/overlay-spike/` |

## Akzeptanzkriterien aus PLAN.md 2

| Kriterium | Ziel | Ergebnis | Nachweis |
|---|---|---|---|
| Sync-Genauigkeit, Median | ≤ 100 ms | **96 ms** (13 englische und deutsche Songs, 115 Taps); 108 ms mit zwei japanischen Songs | `docs/sync-report.md` |
| Sync-Genauigkeit, p95 | ≤ 200 ms | 289 ms (309 ms mit Japanisch). **Vom Nutzer akzeptiert:** Jeder Tap enthält Hören, Reaktion und Tastendruck; die Zeitkette von LyricHub liegt bei 8–25 ms, der Rest sind LRC-Dateien und die Messmethode | `docs/sync-report.md` |
| Sync-Stabilität | kein Zittern, Slew ≤ ±10 %, kein Rücksprung < 300 ms | **erfüllt**: Clock-Tests (`constant_offset_is_slewed_away_without_backward_steps`, `medium_error_jumps_instead_of_slewing_for_seconds`), in den Hörtests kein Zittern | `cargo test`, Phase 2/3 |
| Seek | ≤ 300 ms | **erfüllt**: 30 ms im Median | `docs/sync-report.md` |
| Track-Wechsel: Titel | ≤ 300 ms | **erfüllt**: 7–10 ms nach dem SMTC-Ereignis | `docs/phase1.md` |
| Track-Wechsel: Lyrics aus dem Cache | ≤ 100 ms | **erfüllt**: etwa 3 ms | `docs/phase2.md` |
| Track-Wechsel: Lyrics online | ≤ 1,5 s | **teilweise**: 76 % von 135 Suchen. Direkttreffer 120 ms im Median; Ausweichsuchen bei LRCLIB 1,5–2,3 s (Median 818 ms, p95 2,3 s) | Logs, siehe unten |
| Fokus | nie Fokus nehmen, Eingaben laufen weiter | **erfüllt**: Fokus-Schritte in run-spike, Edit-Modus-Test 13/13, Notepad-Test des Nutzers mit Songwechseln, Strg+Alt+L und Edit-Modus | `docs/spike-overlay-*.json` |
| Sichtbarkeit über Borderless-Spielen, auch bei Topmost-Reassert | sichtbar | **erfüllt**: `game-aggressive` 15/15, Overlay in 120 von 120 Proben oben, 12-mal nachgesetzt in 6 s. Echte Spiele (Borderless, exklusives Vollbild) und Vollbild-Video hat der Nutzer in Phase 0/1 geprüft | `docs/spike-overlay-game-aggressive.json`, `docs/spike-overlay.md` |
| CPU pausiert | < 0,5 % | **erfüllt**: 0,03 % aller Kerne (Release, 30 s) | `measure-cpu.ps1` |
| CPU spielend | < 2 % | **erfüllt**: 0,48 % aller Kerne (Release, 60 s) | `measure-cpu.ps1` |
| RAM | < 200 MB privat | **erfüllt**: 179 MB für LyricHub und alle WebView2-Prozesse, Release, Wiedergabe | siehe unten |
| FPS im Spiel | keine spürbare Einbuße | vom Nutzer in Phase 4 erlassen | `docs/phase4.md` |
| Robustheit: Spotify-Neustart | erholt sich | **erfüllt**: Sitzung weg, nach dem Neustart wieder gebunden, Titel und Lyrics da | Log 14:39 UTC |
| Robustheit: Gerätewechsel | erholt sich | Lautsprecher → Kopfhörer erkannt (Phase 3); kein Bluetooth-Gerät vorhanden | `docs/phase3.md` |
| Robustheit: Monitorwechsel, Skalierung | erholt sich | **erfüllt**: Der Nutzer hat 125 % und 150 % geprüft; jeder Wechsel kam als `WM_DISPLAYCHANGE` an, das Overlay wurde sofort neu platziert | Log 14:51 UTC |
| Robustheit: Schlafmodus | erholt sich | **nicht live geprüft**: Neuverbindung ist gebaut; in den Logs gibt es keinen Schlafmodus (der PC war über Nacht aus) | – |
| Kein Fokusklau beim Start | Autostart minimiert, kein Fenster | **erfüllt**: Vordergrund blieb 6 s beim Browser | `start-focus-test.ps1` |

### Messungen

- **Speicher, Release-Build, Wiedergabe:** ohne Schalter 188 MB im eingeschwungenen Zustand, 202–204 MB nach einer Edit-Modus-Sitzung. Mit `--disable-gpu-rasterization` 172–184 MB, im Mittel 179 MB. Größter Posten ist der GPU-Prozess (80 statt 97 MB).
- **Lyrics online:** 135 Suchen aus den Logs vom 25. und 26.10. Direkttreffer: 87 Suchen, Median 120 ms. Ohne Album: 23, Median 1,5 s. Suche: 21, Median 1,9 s. Freitext: 4, Median 2,3 s. LRCLIB antwortet auf Fehlschläge und Suchen unter Last langsam (Phase 2). Würden alle Stufen gleich parallel starten statt gestaffelt nach 350 ms, ginge es schneller, aber mit bis zu vierfacher Last auf einen kostenlosen Dienst. Das ist bewusst nicht umgesetzt.
- **Spike-Szenarien:** plain 13/13, game-once 15/15, game-aggressive 15/15. Die zwei Fehlschläge der Gruppe „tauri“ sind seit Phase 0 dokumentierte Tao-Befunde, kein Produktionspfad.

## Definition of Done

| Punkt | Stand |
|---|---|
| Alle Akzeptanzkriterien aus Abschnitt 2 erfüllt und belegt | Bis auf zwei Punkte erfüllt: p95 vom Nutzer akzeptiert; Online-Lyrics zu 76 % unter 1,5 s, abhängig von LRCLIB. Schlafmodus nicht live geprüft. |
| Installer auf frischem Profil getestet | **offen**: Der Nutzer gibt den Installer an einen Freund weiter. Prüfliste in der Übergabe an den Nutzer. |

## Abweichungen vom Plan

- **Sync-Protokoll 11.3 nicht wiederholt:** Der Nutzer hat das p95 mit Verweis auf die menschliche Reaktion beim Tippen akzeptiert; es gelten die Messungen aus Phase 3 (Lauf 1–3).
- **Installer-Test auf einem anderen PC** statt eines frischen Profils hier: Der Nutzer legt kein Testkonto an, ein Freund testet.
- **Kein Hinweis-Ballon:** Der Tray-Hinweis steht im Tooltip und als erster Menüeintrag; Tauri hat keine Tray-Benachrichtigungen.
- **Exklusives Vollbild:** nur ein vorsichtiger Hinweis. Ob Windows das Overlay über den Vollbild-Optimierungen zeichnet, lässt sich nicht verlässlich erkennen.
- **GPU-Rasterung aus:** Abweichung von der Chromium-Voreinstellung, um unter 200 MB zu bleiben. Der Text des Overlays wird nur bei Zeilenwechseln gerastert.
- **Glass Light mit mindestens 70 % Deckkraft:** Bei 20 % war dunkle Schrift über dunklen Spielen nicht mehr lesbar (beim Screenshot für die README aufgefallen).
- **Selbsttest-Bericht bleibt Deutsch**, wie in Phase 4.
- **Kein Code-Signing:** Windows SmartScreen zeigt beim ersten Start des Installers „Unbekannter Herausgeber“.

## Gefunden und behoben

- Der Topmost-Check des Selbsttests lief ohne festes Intervall über (`u64::MAX + 50`) und brach im Debug-Build ab.
- Die Werkzeugleiste des Edit-Modus erbte bei Neon und Minimal Kontur und Schatten der Lyrics; „Speichern“ war schlecht lesbar (Hinweis des Nutzers).
- Glass Light bei niedriger Deckkraft (siehe oben).
- Nach dem Tag: `taskkill` ohne /F schloss nur das Overlay-Fenster. LyricHub lief ohne Overlay weiter und schrieb zweimal pro Sekunde eine Warnung ins Log. Jetzt beendet eine Schließen-Anfrage an das Overlay die App wie „Beenden“ im Tray.

## Offene Punkte

- Installer-Test durch den Freund des Nutzers.
- Schlafmodus einmal live prüfen (Energie sparen, aufwachen, Song abspielen; im Log `resumed from sleep`).
- Einstellungsseiten „Lyrics“ (Cache leeren, LRC importieren) und „Verbindung“ (9.7, Punkte 1 und 5) gibt es nicht; die Web API ist gestrichen.
- `npm audit`: drei Lücken mit niedrigem Schweregrad in `cookie` über `@sveltejs/kit` (Fix bräuchte Kit 3).
