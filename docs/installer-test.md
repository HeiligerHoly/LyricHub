# Installer-Test auf einem fremden PC

Für Phase 5 (PLAN.md 10, DoD „Installer auf frischem Windows-Profil getestet“). Der Test braucht etwa 10 Minuten und Spotify (Store- oder Desktop-Version) mit einem Song, für den es synchrone Lyrics gibt.

**Datei:** `LyricHub_0.5.0_x64-setup.exe` (10 MB)

## Ablauf

1. **Installer starten.** SmartScreen meldet einen unbekannten Herausgeber, weil der Installer nicht signiert ist: „Weitere Informationen“ → „Trotzdem ausführen“.
   - [ ] Es kommt **keine** Abfrage nach Administratorrechten (UAC).
   - [ ] Die Installation läuft ohne Fehler durch, Sprache Deutsch oder Englisch.
2. **LyricHub starten** (Startmenü oder Ende des Installers).
   - [ ] Das Einstellungsfenster öffnet sich, im Tray erscheint das LyricHub-Icon (Sprechblase).
   - [ ] Ohne laufendes Spotify ist das Tray-Icon grau.
3. **Spotify starten und einen Song abspielen.**
   - [ ] Oben mittig erscheint das Overlay mit Titel, Artist und mitlaufenden Lyrics.
   - [ ] Die Zeilen wechseln ungefähr dann, wenn sie gesungen werden.
4. **Kürzel ausprobieren:** Strg+Alt+L (Overlay aus/an), Strg+Alt+K (Layout wechseln), Strg+Alt+E (Edit-Modus; Overlay ziehen, Enter speichert).
   - [ ] Alle drei funktionieren, auch während ein anderes Fenster (z. B. ein Browser) den Fokus hat.
5. **Einstellungen durchklicken:** Overlay, Darstellung (Themes), Hotkeys, Verhalten → Sprache auf Englisch und zurück.
   - [ ] Keine Fehlermeldung, kein roter oder gelber Hinweisbalken (außer, LRCLIB ist gerade nicht erreichbar).
6. **Optional, mit einem Spiel im Modus „Randloses Fenster“:**
   - [ ] Das Overlay bleibt über dem Spiel sichtbar, das Spiel behält Maus und Tastatur.
7. **Deinstallieren:** Windows-Einstellungen → Apps → LyricHub → Deinstallieren.
   - [ ] Läuft ohne Administratorrechte durch, LyricHub ist aus Startmenü und Tray verschwunden.

## Wenn etwas nicht klappt

Einstellungen → Diagnose → „Diagnose exportieren“. Die Datei landet im Log-Ordner (`%APPDATA%\LyricHub\logs\diagnostics-<zahl>.json`); sie enthält keine Liedtexte. Datei und eine kurze Beschreibung zurückschicken.
