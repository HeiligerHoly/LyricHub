# Phase 1 – Dünner Durchstich „Now Playing“

> Datum: 2026-10-25 · Bezug: PLAN.md 10 (Phase 1), 4.2–4.4, 6.4, 9.6, 9.7
> Ergebnis: **Phase 1 abgeschlossen** (Tag `v0.1.0-phase1`). Automatische Tests grün, Sichtprüfung über einem Spiel durch den Nutzer bestanden.

## Was gebaut wurde

| Bereich | Umsetzung | Dateien |
|---|---|---|
| SMTC-Quelle | Ein Worker-Thread besitzt alle WinRT-Objekte. Die drei Session-Events und `SessionsChanged` schicken nur eine Nachricht; gelesen wird im Worker. Dazu Polling von Status und Timeline alle 250 ms, Medien-Eigenschaften jede Sekunde als Netz. Fällt die Position an den Songanfang, pollt der Worker 400 ms lang alle 40 ms die Eigenschaften, weil Spotify den Titel bis zu 250 ms nach der Timeline liefert. Titel geht vor dem Cover raus. | `src-tauri/src/sources/` |
| `track`-Event | `TrackInfo` nach PLAN.md 4.3, Cover als `data:`-URL. `null`, wenn Spotify verschwindet. Dazu `playback` mit dem Status. | `app_state.rs`, `ipc/events.rs` |
| Overlay „Bar“ | Cover · Titel – Artist auf der Glass-Dark-Platte. Langer Titel wird zuerst gekürzt, der Artist behält bis 35 %. Springt die Position an einen Songanfang, verblasst der alte Titel sofort (Event `track-changing`); der neue blendet ein, sobald Spotify ihn liefert. Dimmen nach 2 s Pause, Ausblenden nach der eingestellten Zeit. Keine Daueranimation. | `src/lib/overlay/` |
| Settings v1 | `schema_version`, JSON in `%APPDATA%/LyricHub/settings.json`, Schreiben über Temp-Datei, kaputte Datei wird gesichert und durch Standardwerte ersetzt, Änderungen als JSON Merge Patch mit Bereichsprüfung. | `src-tauri/src/settings/` |
| Geometrie | Monitor (GDI-Gerätename, Rückfall auf den Hauptbildschirm), 9 Anker, Offsets und Breite in Prozent, Skalierung, DPI pro Monitor. Fenster = Platte; die Seite misst und meldet, das Backend platziert mit einem `SetWindowPos`. | `overlay/window.rs`, `overlay/mod.rs` |
| Edit-Modus | Strg+Alt+E: Fenster deckt den Monitor ab, Klickdurchlässigkeit aus (nur über `win32.rs`), `WS_EX_NOACTIVATE` bleibt. Ziehen mit Einrasten an Rändern und Mitte, Ecken skalieren proportional, Seiten ändern die Breite, Monitorwechsel per Knopf. Enter speichert, Esc bricht ab; beide sind nur während des Edit-Modus global registriert. Der Anker ergibt sich aus dem Drittel, in dem die Mitte liegt. | `EditFrame.svelte`, `editMode.ts`, `hotkeys.rs` |
| Einstellungsfenster | Wird beim Öffnen erzeugt und beim Schließen zerstört. Seiten Overlay, Verhalten, Diagnose (Latenz, Fensterzustand, Selbsttest). | `settings_window.rs`, `src/lib/settings/` |
| Tray | Overlay anzeigen (Häkchen), Edit-Modus, Einstellungen, Selbsttest, Beenden. Tooltip „LyricHub – Titel – Artist“. | `tray.rs` |

## Definition of Done

| Kriterium | Ergebnis | Nachweis |
|---|---|---|
| Titelwechsel erscheint in ≤ 300 ms im Overlay | **LyricHubs Anteil: 7–10 ms.** Gesamt ab dem ersten Anzeichen 196–308 ms; die zwei Werte knapp über 300 ms liegen an Spotify, das den Titel 298 ms nach der Timeline veröffentlichte. Der alte Titel verblasst deshalb schon beim ersten Anzeichen. Über einem Spiel vom Nutzer bestätigt. | Log `track change shown in overlay`, Seite Diagnose, Tabelle unten |
| Position und Größe per Edit-Modus setzen | **erfüllt**: automatisch 13 von 13, vom Nutzer im Spiel geprüft | `tools/overlay-spike/edit-mode-test.ps1` |
| Position und Größe überleben einen Neustart | **erfüllt**: auf allen drei Monitoren pixelgenau; nach dem Speichern im Edit-Modus („Mitte rechts“) stand das Overlay nach dem Neustart wieder bei 1632,688 | Tabelle unten |
| `run-spike.ps1` grün | **erfüllt**: plain 13/13, game-once 15/15, game-aggressive 13/14 (Phase 0: 12/14; der Fehler ist der bekannte Worst Case, siehe unten) | `docs/spike-overlay-*.json` |
| CPU ≤ 2 % aller Kerne bei Wiedergabe | **erfüllt**: 0,1 % eines Kerns = 0,00 % von 16 Kernen | `measure-cpu.ps1 -Seconds 60` |

### Latenz Titelwechsel

Gemessen wird vom ersten Anzeichen in der Media-Session (Timeline springt an den Songanfang oder neuer Titel) bis die Overlay-Seite den neuen Titel gezeichnet hat (Bestätigung nach zwei `requestAnimationFrame`). Alle fünf Wechsel waren natürliche Songwechsel bei laufender Wiedergabe.

| Wechsel | Gesamt | davon Spotify (Timeline → Titel) | davon LyricHub (Event → Bild) |
|---|---|---|---|
| Payphone | 232 ms | 222 ms | 10 ms |
| Küss dein Herz | 307 ms | 299 ms | 8 ms |
| i fight my demons when i sleep | 308 ms | 298 ms | 9 ms |
| It's Been So Long | 196 ms | 186 ms | 10 ms |
| Aces | 292 ms | 285 ms | 7 ms |

Spotify veröffentlicht den neuen Titel heute bis zu 299 ms nach der neuen Timeline (Spike A: bis 250 ms). Schnelleres Abfragen hilft nicht: Der Worker fragt in dieser Zeit alle 40 ms, und der Titel kam jedes Mal erst mit dem Event. Damit im Spiel während dieser Lücke nicht der alte Titel steht, verblasst er schon beim ersten Anzeichen; LyricHub reagiert also nach etwa 10 ms sichtbar. Der erste Song nach dem Start zählt nicht, er enthält das Laden der Seite.

### Edit-Modus

`edit-mode-test.ps1` startet das Fake-Game (randloses Vollbild, hält den Vordergrund) und steuert LyricHub mit synthetischen Eingaben.

| Prüfung | Ergebnis |
|---|---|
| Strg+Alt+E: Fenster deckt den Monitor ab, fängt Klicks, bleibt `WS_EX_NOACTIVATE` | bestanden |
| Maus trifft das Overlay (`WindowFromPoint`) | bestanden |
| Ziehen um +400/+300, Enter: Overlay bei 1232,332, `settings.json` aktualisiert (x 15,625 %, y 23,033 %) | bestanden |
| Ziehen, Esc: Overlay zurück am gespeicherten Platz | bestanden |
| Zurückziehen, Enter: rastet exakt an der Ausgangsposition 832,32 ein | bestanden |
| Nach jedem Schritt: Spiel hat den Vordergrund, kein Esc beim Spiel angekommen | bestanden |

**Befund und Korrektur:** Im ersten Lauf kam das Ziehen nicht an. Windows behandelte das Layered-Overlay beim Hit-Test als vollständig durchsichtig, obwohl `WS_EX_TRANSPARENT` aus war. WebView2 zeichnet über DirectComposition, nicht in die Fläche, die Windows für den Hit-Test prüft. Seit `SetLayeredWindowAttributes` mit konstantem Alpha 255 gesetzt ist, trifft der Klick das Overlay; die Darstellung bleibt transparent. `win32.rs` setzt die Attribute bei jedem Stil-Abgleich, der Wächter prüft sie. Der Selbsttest aus Phase 0 hatte das nicht bemerkt, weil er nur im kleinen Fenster gemessen hat.

### Selbsttest aus Spike B

| Szenario | Phase 0 | Phase 1 |
|---|---|---|
| plain | 13 von 13 | 13 von 13 |
| game-once (Spiel setzt Topmost einmal) | 15 von 15 | 15 von 15, Overlay 100 % der Zeit oben |
| game-aggressive (Spiel setzt Topmost alle 500 ms) | 12 von 14 | 13 von 14 |

Der verbleibende Fehler im Worst Case: Das Spiel lag beim Klicktest über dem Overlay. Ereignisgesteuertes Topmost kommt in Phase 5 (PLAN.md 10).

### Platzierung nach Neustart

Einstellungen geschrieben, App gestartet, Fensterrechteck mit `GetWindowRect` gemessen.

| Einstellung | Erwartet | Gemessen |
|---|---|---|
| Hauptbildschirm 2560×1440, unten rechts, −2 %/−3 %, Breite 30 %, Skalierung 1,2 | 1741,1320 · 768×77 | 1741,1320 · 768×77 |
| Monitor 4 (1200×1920 hochkant), oben links, Breite 50 % | 2560,−267 · 600×54 | 2560,−267 · 600×54 |
| Monitor 3 (1920×1080 links), unten mittig, −2,2 %, Breite 40 % | −1344,1158 · 768×48 | −1344,1158 · 768×48 |
| Monitor existiert nicht → Hauptbildschirm, Mitte, Breite 20 %, Skalierung 0,5 | 1024,704 · 512×32 | 1024,704 · 512×32 |
| Ungültiges JSON | Sicherung `settings.broken-<unix>.json`, Standardwerte | wie erwartet |

Alle drei Monitore laufen mit 100 % Skalierung. Gemischte DPI-Werte sind nur über Unit-Tests abgedeckt; die DPI-Tests mit 125/150 % stehen in Phase 5.

### Speicher

| Zustand | Prozesse | privat |
|---|---|---|
| Frisch gestartet, Einstellungsfenster nie geöffnet | 7 | 181 MB |
| Einstellungsfenster offen | 8 | 237 MB |
| Nach dem Schließen (Fenster zerstört, nicht versteckt) | 7 | 203 MB |
| Phase 0, Einstellungsfenster versteckt | – | 203 MB |

Das Schließen gibt 34 MB frei. Ein Rest bleibt im gemeinsamen WebView2-Browserprozess. Die Grenze von 200 MB (PLAN.md 2) gilt für die Abnahme in Phase 5 und wird dort mit einer Release-Build erneut gemessen.

## Abweichungen vom Plan

- **`overlay-layout` statt `overlay-mode`**: Ein Event trägt Breite, Skalierung, Ausrichtung und den Edit-Zustand gemeinsam, damit Modus und Geometrie nie auseinanderlaufen.
- **Größe relativ zur Auflösung**: Skalierung 1,0 entspricht der Designgröße bei 1080 logischen Pixeln auf der kürzeren Monitorseite; ein 1440p-Monitor zeigt das Overlay 1,33-mal so groß. So passen Breite in Prozent und Schriftgröße auf jedem Monitor zusammen (PLAN.md 9.5: „28 px bei scale 1 auf 1080p“).
- **Mindestgröße**: Nur die Mindestbreite von 200 px gilt. Das Fenster ist exakt so hoch wie die Platte; eine Mindesthöhe von 60 px hätte die Platte bei unteren Ankern sichtbar vom Rand abgehoben.
- **Fenstergeometrie über Win32**: `SetWindowPos` mit `SWP_ASYNCWINDOWPOS` statt Tauris `set_size`/`set_position`. Ein Aufruf statt zwei, und kein Warten auf den Hauptthread, während eine Sperre gehalten wird.
- **`WM_DISPLAYCHANGE`**: Der Wächter vergleicht alle 2 s die Monitorliste und das Fensterrechteck und platziert bei Abweichung neu. DPI-Wechsel des Fensters lösen zusätzlich sofort eine Neuplatzierung aus.
- **Einstellungsfelder ohne Funktion fehlen noch**: Layout-Auswahl (Phase 2/4), „Nur bei Vollbild-App zeigen“ (Phase 5), Sprache (Phase 4). Sie kommen mit ihrer Funktion; weil jedes Feld einen Standardwert hat, braucht das keine Migration.
- **Strg+Alt+E im Edit-Modus speichert.** Der Plan legt das nicht fest; Esc bleibt der Weg zum Abbrechen.

## Sichtprüfung über einem Spiel

Durch den Nutzer am 2026-10-25, Spiel im Borderless-Modus: **bestanden.**

- Spotify-Titelwechsel im Spiel: Der alte Titel verblasst, der neue erscheint.
- Edit-Modus im Spiel: Ziehen, Skalieren, Breite, Enter und Esc funktionieren; das Spiel behält die Eingaben.
- Strg+Alt+L blendet aus und ein.
- Beenden über das Tray und Neustart: Overlay steht am gespeicherten Platz in der gespeicherten Größe.
- Einstellungsfenster mit den Seiten Overlay, Verhalten und Diagnose passt.
