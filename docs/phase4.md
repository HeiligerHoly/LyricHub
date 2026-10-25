# Phase 4 – Karaoke-Feinschliff und Design

> Datum: 2026-10-26 · Bezug: PLAN.md 10 (Phase 4), 5.6, 9.1–9.7
> Ergebnis: **Phase 4 abgeschlossen** (Tag `v0.4.0-phase4`). Die Füllung läuft Wort für Wort, es gibt fünf Themes mit Akzentfarbe aus dem Cover, vier Layouts, die Seite „Darstellung“ mit Live-Vorschau sowie Deutsch und Englisch. Die Füllung löst kein Layout pro Frame aus, und die CPU-Last liegt bei 0,48 % aller Kerne. Den FPS-Test im Spiel hat der Nutzer für nicht nötig erklärt.

Vor Phase 4 kam die Romaji-Anzeige aus `claude/unruffled-dirac-66618b` nach main (Merge `9a3d938`, siehe `docs/romaji.md`). Mit ihr ist die japanische Sync-Messung nachgeholt (`docs/sync-report.md`, Lauf 3).

## Was gebaut wurde

| Bereich | Umsetzung | Dateien |
|---|---|---|
| Wortzeiten (5.6) | Zeilen ohne Enhanced-LRC-Marken bekommen geschätzte Wortzeiten. Latein, Kyrillisch und Hangul werden an Leerzeichen getrennt, Kana und Kanji pro Zeichen. Gewichte: Buchstabe und Kana 1, Kanji 1,5, Satzzeichen 0 (hängen am Wort davor). 60 ms Vorlauf, 120 ms Nachlauf. Romaji-Zeilen bekommen eigene Wortzeiten. | `lyrics/words.rs`, `parser.rs`, `romaji.rs` |
| Wortweise Füllung (9.3) | Die Keyframes der Füllung haben Stopps an den Wortkanten, einmal pro Zeile gemessen als Anteil der Textbreite. Damit gelten sie bei jeder Schriftgröße. Weiterhin eine Compositor-Animation pro Ebene, 30 Schritte pro Sekunde, kein JavaScript pro Frame. | `overlay/wordFill.ts`, `KaraokeLine.svelte` |
| Wort-Glow (9.4) | Nur bei echten Wortzeiten: Eine Kopie mit Textschatten blendet pro Wort in 120 ms ein und 300 ms nach dem Wortende aus, über die Deckkraft. | `KaraokeLine.svelte` |
| Themes (9.5) | Glass Dark (Standard), Glass Light, Neon, Mono, Eigenes. Die Themes sind CSS-Variablen an der Bühne; die Layouts lesen daraus Platte, Textstufen, Akzent, Kontur und Glow. Das Layout „Minimal“ hat immer Kontur und keine Platte. | `overlay/theme.ts`, `color.ts`, Layouts |
| Akzent aus dem Cover (9.5) | Das Cover wird auf 32×32 verkleinert, k-means findet vier Farben. Gewählt wird die kräftigste, die mindestens 6 % des Covers einnimmt. Ihre Helligkeit wird angepasst, bis sie gegen die Platte mindestens 4,5:1 Kontrast hat. Spotifys Grün wird um 8° verschoben. Graue Cover bekommen den Ersatzakzent. | `overlay/accent.ts` |
| Layouts (9.2) | Neu: „Stapel“ (vorige Zeile, aktuelle, zwei kommende; weiches Scrollen, 220 ms) und „Minimal“ (nur die aktive Zeile, ohne Platte). Kopf und aktive Zeile sind gemeinsame Bausteine; der Tray und Strg+Alt+K schalten durch alle vier. | `layouts/Stack.svelte`, `Minimal.svelte`, `TrackHeader.svelte`, `ActiveRow.svelte` |
| Schrift (9.5) | Inter Variable (OFL-1.1) wird mitgeliefert (`@fontsource-variable/inter`). Eigene Schrift aus der Systemliste über DirectWrite. Ist Yu Gothic UI oder Meiryo UI installiert? Fehlen beide, nennt die Darstellungsseite das passende Windows-Feature. | `fonts.rs`, `routes/overlay/+page.svelte` |
| Bewegung (9.4) | Zeilenwechsel 220 ms, Songwechsel 300 ms mit Cover von 0,96 auf 1, Pause dimmt nach 2 s in 400 ms. „Bewegung reduzieren“ in den Einstellungen oder in Windows: alle Übergänge höchstens 80 ms, kein Wort-Glow. | `Overlay.svelte`, Layouts |
| Zustände (9.6) | Ist der Lyrics-Dienst nicht erreichbar, zeigt der Kopf einen kleinen hohlen Punkt, Details stehen in den Einstellungen. Laden, Instrumental, keine synchronen Lyrics und Werbung gibt es seit Phase 2 und 3. | `TrackHeader.svelte` |
| Seite „Darstellung“ (9.7) | Theme-Karten mit Mini-Vorschau, Akzent aus dem Cover oder fest, Deckkraft der Platte (von der Overlay-Seite hierher verschoben), Schrift, „Bewegung reduzieren“, eigenes Theme (Platte, Farben, Kontur, Radius). Solange die Seite offen ist, zeigt das Overlay Beispielzeilen im aktuellen Look; das Schließen des Fensters beendet die Vorschau. | `AppearancePage.svelte`, `set_overlay_preview` |
| Sprache (i18n) | `behavior.language`: System, Deutsch, Englisch. „System“ folgt der Windows-Anzeigesprache. 247 Texte pro Sprache in `locales/de.ts` und `en.ts`, die deutschen unverändert. Beide Fenster wechseln sofort. Tray-Menü, Hinweise und Fehler übersetzt das Backend selbst. | `shared/i18n.svelte.ts`, `locales/`, `src-tauri/src/i18n.rs` |
| Prüfwerkzeug | `layout-count.mjs` zählt über das DevTools-Protokoll von WebView2 die Layouts und Style-Berechnungen der Overlay-Seite. | `tools/overlay-spike/` |

## Definition of Done

| Kriterium | Ergebnis | Nachweis |
|---|---|---|
| Design-Review gegen 9.1–9.6 | **erfüllt**, siehe Tabelle unten. Alle Layouts und Themes per Screenshot über einem echten Desktop (Steam, Explorer) geprüft. Dabei gefunden und behoben: Kontur und Schatten fehlten bei Minimal und Neon, weil die Skalierung `--s` nur an der Platte hing. | Screenshots (nicht im Repo, zeigen fremde Fenster) |
| Kein Layout pro Frame | **erfüllt**: 6 Layouts und 38 Style-Berechnungen in 30 s Wiedergabe, 0,2 Layouts pro Sekunde (eines pro Frame wären 30). | `node tools/overlay-spike/layout-count.mjs 30` |
| FPS-Test im Spiel ohne messbare Einbuße | **entfällt** (Nutzerentscheidung: „FPS-Test brauchen wir nicht, das passt“). Hinweise: CPU 0,48 % aller Kerne, Füllung nur auf dem Compositor. | – |
| `run-spike.ps1` grün | plain 13/13, game-once 15/15, game-aggressive 13/14 (bekannter Worst Case aus Phase 0: „Edit-Modus fängt Klicks ab“ bei aggressivem Topmost) | `docs/spike-overlay-*.json` |
| Edit-Modus weiter intakt | 13 von 13 | `edit-mode-test.ps1` |
| CPU ≤ 2 % bei laufender Füllung | **erfüllt**: 7,7 % eines Kerns = 0,48 % von 16 Kernen über 60 s, japanischer Song mit Romaji | `measure-cpu.ps1 -Seconds 60` |
| Unit-Tests | 90 grün (neu: Wortschätzung 5, Settings-Prüfung erweitert) | `cargo test` |

Speicher: 196 MB privat für LyricHub und alle WebView2-Prozesse. Das liegt knapp unter der Grenze von 200 MB aus Abschnitt 2, gemessen mit eingeschaltetem Debug-Zugang. Das Romaji-Wörterbuch liegt im gemappten Image und zählt nicht als privater Speicher. Der Nachweis gehört zu Phase 5.

### Design-Review

| Abschnitt | Punkt | Stand |
|---|---|---|
| 9.1 | Lesbar über jedem Spiel | Platte bei Glass und Mono; Kontur mit Halo und Schatten bei Neon, Minimal und auf Wunsch bei Eigenes. Geprüft über hellem Steam-Kopf und dunklem Explorer. |
| 9.1 | Ruhe | Nur die Füllung bewegt sich dauerhaft; alles andere ≤ 300 ms, dann still. |
| 9.1 | Zeit ist Wahrheit | Füllung linear zwischen den Wortstopps, ohne Easing; Easing nur beim Zeilenwechsel. |
| 9.1 | Gute Defaults | Glass Dark mit Akzent aus dem Cover, Karaoke, Inter. |
| 9.2 | Vier Layouts | Karaoke, Leiste, Stapel (4 Zeilen), Minimal. |
| 9.3 | Technik der Füllung | Zwei Ebenen, Clip per `translateX`, weiche Kante per `mask-image`, kein `background-clip: text`, 30 Schritte pro Sekunde. Inter Variable, Gewicht 650/500 ohne Layoutsprung. |
| 9.3 | Wort-Glow | Bei echten Wortzeiten, über Deckkraft statt animiertem Schatten (siehe Abweichungen). |
| 9.4 | Motion | Zeilenwechsel 220 ms `cubic-bezier(.2,.8,.2,1)`, Songwechsel 300 ms mit Cover-Scale, Pause 2 s → 0,6 in 400 ms, Toast 1,5 s, reduzierte Bewegung ≤ 80 ms ohne Glow. |
| 9.5 | Typografie | 28 px bei Skalierung 1 auf 1080p, Zeilenhöhe 1,25, `line-break: strict`, Fallback-Kette mit Yu Gothic UI und Meiryo UI. |
| 9.5 | Themes und Akzent | Fünf Themes, Akzent nach dem beschriebenen Verfahren mit Kontrastprüfung, Spotify-Grün gemieden. |
| 9.6 | Zustände | Laden ≤ 1,5 s, keine synchronen Lyrics 4 s, Instrumental 4 s, Werbung aus, Netzfehler als dezente Marke, Edit-Modus wie in Phase 1. |

## Abweichungen vom Plan

- **Wortschätzung nicht kalibriert:** Phase 2 schlug vor, die Schätzung an LRCLIB-Songs mit Wortzeiten abzugleichen. LRCLIB liefert die Felder `hasWordSync` und `lyricsfile`, aber bei acht bekannten Songs mit je 20 Treffern hatte kein einziger Treffer Wortzeiten (0 von 160). Die Parameter folgen deshalb PLAN.md 5.6.
- **Schätzung im Backend:** Die Schätzung läuft in Rust, mit Unit-Tests. Die Füllung im Overlay misst nur noch die Wortkanten. Das Feld `estimated` am Wort unterscheidet echte von geschätzten Zeiten.
- **Wort-Glow über Deckkraft:** 9.3/9.4 beschreiben einen animierten `text-shadow`. Der würde während jedes Glows neu zeichnen. Stattdessen liegt pro Wort eine Kopie mit festem Schatten darüber, deren Deckkraft der Compositor animiert.
- **Kontur 3,5 px statt 2 px** (bei Skalierung 1): Der Strich liegt hinter der Füllung (`paint-order`), es ist also nur etwa die Hälfte zu sehen. Dazu kommt ein enger Halo-Schatten. 2 px waren über hellen Szenen zu dünn.
- **Nächste Zeile beim Zeilenwechsel:** Sie blendet ein, statt 12 px hochzurücken. Die aktive Zeile steigt um 12 px und blendet von 0,55 ein, wie in 9.4.
- **Kein gebündeltes Noto-Sans-JP-Subset:** Yu Gothic UI ist auf dem PC des Nutzers vorhanden. Fehlen Yu Gothic UI und Meiryo UI, zeigt die Darstellungsseite das Windows-Feature „Japanische ergänzende Schriftarten“. Ein Bündel würde die App um mehrere MB vergrößern.
- **Deckkraft gilt für alle Themes mit Platte:** Glass Light nutzt denselben Regler statt fest 0,7; Mono hat mindestens 85 %.
- **Option „Werbung“ anzeigen** fehlt: Der Nutzer hat Premium (14.1), Werbung blendet das Overlay aus.
- **Netzfehler-Marke** nur in Karaoke und Stapel, die einen Kopf haben. Leiste und Minimal bleiben ohne.
- **Selbsttest-Bericht bleibt Deutsch:** Er ist eine Entwickler-Diagnose, die Spike-Skripte lesen ihn.
- **Toast im Kopf statt Pille unter dem Overlay:** wie seit Phase 2, damit das Fenster seine Größe behält.

## Offene Punkte

- Die wortweise Füllung ist eine Schätzung nach Buchstabenzahl. Wie gut sie zum Gesang passt, hängt vom Song ab (Tempo innerhalb der Zeile, Melismen). Echte Wortzeiten bringen nur Enhanced-LRC-Dateien oder Forced Alignment (PLAN.md 13.3).
- Eine Seite „Hotkeys“ (9.7, Punkt 6) und „Nur bei Vollbild-App zeigen“ folgen in Phase 5.
- `npm audit` meldet drei Lücken mit niedrigem Schweregrad in `cookie` über `@sveltejs/kit`. Die Behebung bräuchte Kit 3 (Breaking Change); die App liefert keine Cookies aus.
