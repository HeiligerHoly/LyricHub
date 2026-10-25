# Spike B – Overlay-Fenster (Tauri 2 unter Windows)

> Datum: 2026-10-25 · Phase 0 · Bezug: PLAN.md 6 und 10
> Ergebnis: **Tauri bleibt.** Das Overlay funktioniert über Borderless-Spielen, im exklusiven Vollbild, über Vollbild-Videos, klickdurchlässig und ohne Fokusverlust. Voraussetzung ist, dass es direkt über Win32 gesteuert wird und nicht über Tauri-Setter.

## Testsystem

| Punkt | Wert |
|---|---|
| Betriebssystem | Windows 11 Pro 10.0.26300 |
| Grafik | NVIDIA GeForce GTX 1080 Ti, 2560×1440 bei 164 Hz, Skalierung 100 % |
| CPU | 16 logische Kerne |
| Stack | Tauri 2.12.5, tao 0.37.2, windows-rs 0.62.2, WebView2 155 |

## Befunde aus dem tao-Quelltext

Geprüft in `tao/src/platform_impl/windows/window_state.rs`, im Selbsttest bestätigt.

1. **Jede Flag-Änderung überschreibt den kompletten erweiterten Fensterstil.** Nach Tauris `set_ignore_cursor_events(true)` fehlte `WS_EX_TOOLWINDOW`, dafür war `WS_EX_APPWINDOW` gesetzt. Das Overlay erscheint dann in Alt+Tab und in der Taskleiste. Gemessen: Stil `0x080C0138` statt `0x080801B8`.
2. **Nach jeder Flag-Änderung an einem sichtbaren Fenster ruft tao `ShowWindow(SW_SHOW)` auf.** Im ersten Testlauf hielt LyricHub selbst den Vordergrund. Nach Tauris `hide()` und `show()` lag der Fokus dann beim Overlay. Hält ein anderer Prozess den Vordergrund, verhindert Windows den Wechsel. LyricHub darf ihn aber direkt nach einem eigenen Tastenkürzel oder Tray-Klick übernehmen. Genau dann würde Tauris `show()` dem Spiel den Fokus nehmen.
3. **`focusable: false` setzt `WS_EX_NOACTIVATE` innerhalb von tao** und überlebt deshalb jede Neuberechnung.
4. **Ein Fenster mit Tauris Standard `focus: true` übernimmt beim Start den Vordergrund, auch wenn es unsichtbar ist.** Das versteckte Einstellungsfenster hat so beim Programmstart den Fokus gestohlen. Behoben mit `focus: false`; `open_settings` fokussiert das Fenster ausdrücklich.

## Umgesetzte Strategie

- Overlay unsichtbar erzeugt, mit `focus: false`, `focusable: false`, transparent, ohne Rahmen, `alwaysOnTop`, `skipTaskbar` und `backgroundThrottling: disabled`.
- **Nach dem Erzeugen keine Tauri-Setter mit Flag-Änderung mehr.** Einblenden, Ausblenden, Klickdurchlässigkeit und Topmost laufen über Win32 in `src-tauri/src/overlay/win32.rs`.
- Ein Wächter prüft alle 500 ms Stil-Bits und Sichtbarkeit und setzt alle 2 s Topmost neu. Bei der Sichtprüfung musste er kein einziges Mal eingreifen.

## Selbsttest (automatisch)

`tools/overlay-spike/run-spike.ps1` baut die App und startet den Selbsttest in drei Szenarien. Die Berichte liegen in `docs/spike-overlay-*.json`.

| Szenario | Produktionsweg | Fokustest aussagekräftig | Overlay über Spiel |
|---|---|---|---|
| Ohne Spiel, Claude im Vordergrund | 13 von 13 bestanden | ja | – |
| Spiel setzt Topmost einmal (typisch) | 15 von 15 bestanden | ja, Spiel im Vordergrund | 100 % der Zeit |
| Spiel setzt Topmost alle 500 ms (Worst Case) | 12 von 14 bestanden | ja | 5 % der Zeit |

Geprüft wird: kein Fokusverlust beim Einblenden, Umschalten und Ausblenden; vollständige Stil-Bits; das Overlay wird gezeichnet (5,8 bis 6,4 % der Pixel im Overlay-Bereich ändern sich); Klicks gehen durch bzw. werden im Edit-Modus abgefangen; Ausschluss aus Alt+Tab. Die zwei Fehler im Worst Case sind Folgen davon, dass das Spiel das Overlay überdeckt.

**Worst Case, aggressives Topmost:** Häufigeres Nachsetzen hilft nur begrenzt.

| Nachsetzen alle | Overlay oben |
|---|---|
| 2000 ms | 5 % |
| 250 ms | 58 % |
| 100 ms | 82 % |

Abfragen im Takt flackert also auch bei 100 ms noch. Spiele, die sich laufend selbst nach oben setzen, sind selten. Für Phase 5 empfohlen: auf Änderungen der Fensterreihenfolge reagieren (`SetWinEventHook` mit `EVENT_OBJECT_REORDER` und `EVENT_SYSTEM_FOREGROUND`, `WINEVENT_OUTOFCONTEXT`). Das läuft außerhalb des Spielprozesses, ist also keine Injection.

## Sichtprüfung durch den Nutzer

| Prüfung | Ergebnis |
|---|---|
| Overlay oben mittig, Spotify-Song im Einstellungsfenster | bestanden |
| Spiel im Borderless-Modus, Strg+Alt+L und Strg+Alt+E im Spiel | bestanden, Spiel behält Fokus und Eingaben |
| Spiel im exklusiven Vollbild | **Overlay sichtbar** (wahrscheinlich über die Vollbild-Optimierungen von Windows) |
| Vollbild-Video im Browser | bestanden |
| Alt+Tab | Overlay nicht in der Liste |

## Leistung

Gemessen mit `tools/overlay-spike/measure-cpu.ps1`: LyricHub samt aller WebView2-Prozesse, Overlay sichtbar mit laufender Füllung, Einstellungsfenster versteckt.

| Variante der Karaoke-Füllung | CPU (ein Kern = 100 %) | Anteil an 16 Kernen |
|---|---|---|
| Animierter Farbverlauf mit `background-clip: text` | 55 % | 3,45 % |
| Zwei Textebenen, Aufdecken per `transform` | 31 % | 1,91 % |
| Wie oben, gedrosselt auf 30 Bilder/s (`steps(150)` über 5 s) | **13 %** | **0,79 %** |

Der größte Posten bleibt der GPU-Prozess von WebView2 mit 9 % eines Kerns. Ungedrosselt setzt er bei 164 Hz jedes Bild neu zusammen.

**Speicher:** 203 MB privat, davon etwa 30 MB für das versteckte Einstellungsfenster. Das Working Set von 432 MB zählt geteilten Speicher mehrfach.

**Folgen für Phase 2 und 4:**
- Die Karaoke-Füllung nur über `transform` und `opacity` umsetzen, niemals über animierte Farbverläufe oder Layout-Eigenschaften.
- Die Bildrate der Füllung begrenzen. In Phase 2 treibt der Playback-Clock die Füllung per `requestAnimationFrame`; dort auf 30 bis 60 Updates pro Sekunde drosseln, statt jedem Monitor-Frame zu folgen.
- Bei Pause und ausgeblendetem Overlay alle Animationen stoppen.
- Das Einstellungsfenster erst beim Öffnen erzeugen und beim Schließen zerstören. Das spart etwa 30 MB.

## Entscheidung

**Tauri bleibt.** Alle Kernanforderungen an das Overlay sind belegt. Die gefundenen Risiken sind durch direkte Win32-Steuerung und eine sparsame Füllung gelöst. WPF ist nicht nötig.

## Offene Punkte für spätere Phasen

- Ereignisgesteuertes Topmost gegen aggressive Spiele (Phase 5).
- Einstellungsfenster bei Bedarf erzeugen (Phase 1 oder 5).
- Die Leistung mit echten Lyrics und Playback-Clock erneut messen (Phase 2), dazu ein FPS-Test im Spiel (Phase 4).
- Den echten Edit-Modus mit Ziehen und Skalieren bauen (Phase 1). Aktuell schaltet Strg+Alt+E nur die Klickdurchlässigkeit um.
