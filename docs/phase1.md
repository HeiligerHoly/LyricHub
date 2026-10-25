# Phase 1 – Dünner Durchstich „Now Playing“

> Datum: 2026-10-25 · Bezug: PLAN.md 10 (Phase 1), 4.2–4.4, 6.4, 9.6, 9.7
> Stand: umgesetzt und automatisch geprüft. Die Sichtprüfung über einem echten Spiel steht noch aus (siehe unten).

## Was gebaut wurde

| Bereich | Umsetzung | Dateien |
|---|---|---|
| SMTC-Quelle | Ein Worker-Thread besitzt alle WinRT-Objekte. Die drei Session-Events und `SessionsChanged` schicken nur eine Nachricht; gelesen wird im Worker. Dazu Polling von Status und Timeline alle 250 ms, Medien-Eigenschaften jede Sekunde als Netz. Fällt die Position an den Songanfang, pollt der Worker 400 ms lang alle 40 ms die Eigenschaften, weil Spotify den Titel bis zu 250 ms nach der Timeline liefert. Titel geht vor dem Cover raus. | `src-tauri/src/sources/` |
| `track`-Event | `TrackInfo` nach PLAN.md 4.3, Cover als `data:`-URL. `null`, wenn Spotify verschwindet. Dazu `playback` mit dem Status. | `app_state.rs`, `ipc/events.rs` |
| Overlay „Bar“ | Cover · Titel – Artist auf der Glass-Dark-Platte. Langer Titel wird zuerst gekürzt, der Artist behält bis 35 %. Dimmen nach 2 s Pause, Ausblenden nach der eingestellten Zeit. Keine Daueranimation. | `src/lib/overlay/` |
| Settings v1 | `schema_version`, JSON in `%APPDATA%/LyricHub/settings.json`, Schreiben über Temp-Datei, kaputte Datei wird gesichert und durch Standardwerte ersetzt, Änderungen als JSON Merge Patch mit Bereichsprüfung. | `src-tauri/src/settings/` |
| Geometrie | Monitor (GDI-Gerätename, Rückfall auf den Hauptbildschirm), 9 Anker, Offsets und Breite in Prozent, Skalierung, DPI pro Monitor. Fenster = Platte; die Seite misst und meldet, das Backend platziert mit einem `SetWindowPos`. | `overlay/window.rs`, `overlay/mod.rs` |
| Edit-Modus | Strg+Alt+E: Fenster deckt den Monitor ab, Klickdurchlässigkeit aus (nur über `win32.rs`), `WS_EX_NOACTIVATE` bleibt. Ziehen mit Einrasten an Rändern und Mitte, Ecken skalieren proportional, Seiten ändern die Breite, Monitorwechsel per Knopf. Enter speichert, Esc bricht ab; beide sind nur während des Edit-Modus global registriert. Der Anker ergibt sich aus dem Drittel, in dem die Mitte liegt. | `EditFrame.svelte`, `editMode.ts`, `hotkeys.rs` |
| Einstellungsfenster | Wird beim Öffnen erzeugt und beim Schließen zerstört. Seiten Overlay, Verhalten, Diagnose (Latenz, Fensterzustand, Selbsttest). | `settings_window.rs`, `src/lib/settings/` |
| Tray | Overlay anzeigen (Häkchen), Edit-Modus, Einstellungen, Selbsttest, Beenden. Tooltip „LyricHub – Titel – Artist“. | `tray.rs` |

## Definition of Done

| Kriterium | Ergebnis | Nachweis |
|---|---|---|
| Titelwechsel erscheint in ≤ 300 ms im Overlay | **erfüllt** im Normalbetrieb, Messwert unten. Über einem Spiel: Sichtprüfung offen | Log `track change shown in overlay`, Seite Diagnose |
| Position und Größe per Edit-Modus setzen | automatisch geprüft: offen (braucht Maus und Vollbildfenster) | `tools/overlay-spike/edit-mode-test.ps1` |
| Position und Größe überleben einen Neustart | **erfüllt**: gespeicherte Geometrie wird beim Start auf den Pixel genau angewendet, auf allen drei Monitoren | Tabelle unten |
| `run-spike.ps1` grün | Szenario „plain“: **13 von 13**. Spiel-Szenarien: offen (Vollbildfenster) | `docs/spike-overlay-*.json` |
| CPU ≤ 2 % aller Kerne bei Wiedergabe | **erfüllt**: 0,1 % eines Kerns = 0,00 % von 16 Kernen | `measure-cpu.ps1 -Seconds 60` |

### Latenz Titelwechsel

Gemessen wird vom ersten Anzeichen in der Media-Session (Timeline springt an den Songanfang oder neuer Titel) bis die Overlay-Seite den neuen Titel gezeichnet hat (Bestätigung nach zwei `requestAnimationFrame`).

| Wechsel | Gesamt | davon Spotify (Timeline → Titel) | davon LyricHub (Event → Bild) |
|---|---|---|---|
| Payphone (natürlicher Songwechsel) | 232 ms | 222 ms | 10 ms |

Der größte Teil ist Spotifys eigener Abstand zwischen neuer Timeline und neuem Titel; die Spike-Messung sah bis zu 250 ms. LyricHub selbst braucht etwa 10 ms. Der erste Song nach dem Start zählt nicht, er enthält das Laden der Seite.

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

181 MB privat für LyricHub samt WebView2 bei geschlossenem Einstellungsfenster (Phase 0 mit verstecktem Fenster: 203 MB). Grenze laut PLAN.md 2: 200 MB.

## Abweichungen vom Plan

- **`overlay-layout` statt `overlay-mode`**: Ein Event trägt Breite, Skalierung, Ausrichtung und den Edit-Zustand gemeinsam, damit Modus und Geometrie nie auseinanderlaufen.
- **Größe relativ zur Auflösung**: Skalierung 1,0 entspricht der Designgröße bei 1080 logischen Pixeln auf der kürzeren Monitorseite; ein 1440p-Monitor zeigt das Overlay 1,33-mal so groß. So passen Breite in Prozent und Schriftgröße auf jedem Monitor zusammen (PLAN.md 9.5: „28 px bei scale 1 auf 1080p“).
- **Mindestgröße**: Nur die Mindestbreite von 200 px gilt. Das Fenster ist exakt so hoch wie die Platte; eine Mindesthöhe von 60 px hätte die Platte bei unteren Ankern sichtbar vom Rand abgehoben.
- **Fenstergeometrie über Win32**: `SetWindowPos` mit `SWP_ASYNCWINDOWPOS` statt Tauris `set_size`/`set_position`. Ein Aufruf statt zwei, und kein Warten auf den Hauptthread, während eine Sperre gehalten wird.
- **`WM_DISPLAYCHANGE`**: Der Wächter vergleicht alle 2 s die Monitorliste und das Fensterrechteck und platziert bei Abweichung neu. DPI-Wechsel des Fensters lösen zusätzlich sofort eine Neuplatzierung aus.
- **Einstellungsfelder ohne Funktion fehlen noch**: Layout-Auswahl (Phase 2/4), „Nur bei Vollbild-App zeigen“ (Phase 5), Sprache (Phase 4). Sie kommen mit ihrer Funktion; weil jedes Feld einen Standardwert hat, braucht das keine Migration.
- **Strg+Alt+E im Edit-Modus speichert.** Der Plan legt das nicht fest; Esc bleibt der Weg zum Abbrechen.

## Sichtprüfung über einem Spiel

Offen. Ablauf: Spiel im Borderless-Modus, Spotify-Titel wechseln, Edit-Modus im Spiel (Ziehen, Skalieren, Enter, Esc), Spiel behält die Eingaben, LyricHub neu starten.
