# Spike B – Overlay-Fenster (Tauri 2 unter Windows)

> Datum: 2026-10-25 · Phase 0 · Bezug: PLAN.md 6 und 10
> Stand: **Code und Selbsttest fertig, Messung ausstehend.** Die Visual Studio Build Tools fehlen noch, deshalb ist nichts kompiliert. Die Installation braucht einmal eine UAC-Bestätigung am PC.
> Vorläufige Entscheidung: **Tauri bleibt**, mit direkter Win32-Steuerung des Overlays. Sie wird nach der Messung bestätigt oder revidiert.

## Ergebnis der Quelltext-Analyse (tao 0.37.2, Tauri 2.12.5)

Vor dem Bauen wurde geprüft, wie Tauri das Overlay-Fenster unter Windows tatsächlich steuert. Das Fensterbackend tao liegt in `tao/src/platform_impl/windows/window_state.rs`.

1. **Jede Flag-Änderung überschreibt den kompletten erweiterten Fensterstil.** `WindowFlags::apply_diff` berechnet `GWL_EXSTYLE` aus tao-eigenen Flags neu und schreibt ihn per `SetWindowLongW`. Eigene Bits wie `WS_EX_TOOLWINDOW` gehen dabei verloren. Das bestätigt den Fallstrick aus PLAN.md 6.1.
2. **Nach jeder Flag-Änderung an einem sichtbaren Fenster ruft tao `ShowWindow` auf.** Mit `SW_SHOWNOACTIVATE` geschieht das nur, solange die Markierung „nicht fokussieren“ gesetzt ist. Die entfernt tao aber direkt nach dem Erzeugen des Fensters. Danach folgt `SW_SHOW`. Ein `set_ignore_cursor_events` auf dem sichtbaren Overlay kann das Overlay deshalb aktivieren und dem Spiel den Fokus nehmen. Dieses Risiko stand bisher nicht im Plan.
3. **`focusable: false` setzt `WS_EX_NOACTIVATE` innerhalb von tao.** Das Bit überlebt deshalb jede Neuberechnung. Die Option gibt es seit Tauri 2.x, sie stand nicht im Plan.
4. **Ist ein Fenster für tao unsichtbar, schreibt tao `GWL_STYLE` ohne `WS_VISIBLE`.** Ein per Win32 eingeblendetes Overlay verschwindet also, wenn danach irgendein Tauri-Setter mit Flag-Änderung läuft.
5. tao setzt für Klickdurchlässigkeit `WS_EX_TRANSPARENT | WS_EX_LAYERED`, ruft aber nie `SetLayeredWindowAttributes` auf. Ob das WebView2-Overlay danach weiter gezeichnet wird, prüft der Selbsttest per Bildschirmpixel.

## Umgesetzte Strategie

- Das Overlay wird unsichtbar erzeugt (`visible: false`, `focus: false`, `focusable: false`, `transparent`, ohne Rahmen, `alwaysOnTop`, `skipTaskbar`, `backgroundThrottling: disabled`).
- **Nach dem Erzeugen ruft der Code keine Tauri-Setter mehr auf, die Flags ändern.** Einblenden, Ausblenden, Klickdurchlässigkeit und Topmost laufen direkt über Win32 (`src-tauri/src/overlay/win32.rs`): `ShowWindow(SW_SHOWNOACTIVATE)`, `SetWindowLongPtrW`, `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`.
- `set_size` und `set_position` von Tauri bleiben erlaubt, sie ändern keine Flags.
- Ein Wächter-Thread prüft alle 500 ms Stil-Bits und Sichtbarkeit und stellt sie ohne Aktivierung wieder her. Alle 2 s setzt er Topmost neu. Jede Korrektur wird geloggt.

## Selbsttest

`src-tauri/src/overlay/selftest.rs` prüft ohne Mensch, was Windows meldet. Er lässt sich über das Tray-Menü, das Einstellungsfenster oder `LYRICHUB_SELFTEST=1` starten.

| Gruppe | Prüfungen |
|---|---|
| win32 (Produktionsweg) | Fokus bleibt beim Einblenden, Umschalten und Ausblenden; Stil-Bits vollständig; Overlay wird gezeichnet (Pixelvergleich); Klicks gehen durch bzw. werden im Edit-Modus abgefangen (`WindowFromPoint`); Alt+Tab-Ausschluss |
| tauri (Vergleich) | dieselben Prüfungen nach `set_ignore_cursor_events`, `hide()` und `show()` von Tauri, um die Befunde 1 und 2 zu belegen |
| topmost | Anteil der Zeit, in der das Overlay über einem simulierten Spiel liegt, das sich selbst topmost hält |

`tools/overlay-spike/run-spike.ps1` baut die App und führt den Test in drei Szenarien aus: ohne Spiel, mit einem Spiel, das einmal topmost setzt, und mit einem, das alle 500 ms nachsetzt. Die Ergebnisse landen in `docs/spike-overlay-*.json`.

## Noch offen

1. Build Tools installieren (UAC am PC bestätigen), dann `run-spike.ps1` ausführen und die Ergebnisse hier eintragen.
2. **Sichtprüfung durch den Nutzer**, die der Selbsttest nicht ersetzen kann:
   - Overlay über einem echten Spiel im Borderless-Modus.
   - Overlay über einem Spiel im exklusiven Vollbild. Gezeigt oder nicht? Spielname notieren.
   - Overlay über einem Vollbild-Video im Browser.
   - Alt+Tab öffnen: Das Overlay darf nicht in der Liste stehen.
   - Im Spiel tippen, während Strg+Alt+L und Strg+Alt+E gedrückt werden: Kein Tastendruck darf verloren gehen.
3. Endgültige Entscheidung „Tauri bleibt / WPF“ hier festhalten.
