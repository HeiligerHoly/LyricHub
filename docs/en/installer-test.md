# Installer test on another PC

*English translation of [installer-test.md](../installer-test.md).*

For Phase 5 ([PLAN.md](../../PLAN.en.md) 10, DoD "Installer tested on a fresh Windows profile"). The test takes about 10 minutes and needs Spotify (Store or desktop version) with a song that has synced lyrics.

**File:** `LyricHub_0.5.0_x64-setup.exe` (10 MB)

## Steps

1. **Start the installer.** SmartScreen reports an unknown publisher because the installer is not signed: "More info" → "Run anyway".
   - [ ] There is **no** prompt for administrator rights (UAC).
   - [ ] The installation runs through without errors, in German or English.
2. **Start LyricHub** (Start menu or the end of the installer).
   - [ ] The settings window opens, and the LyricHub icon (a double note on an amber and dark tile) appears in the tray.
   - [ ] Without Spotify running, the tray icon is gray.
3. **Start Spotify and play a song.**
   - [ ] The overlay appears at the top center with title, artist and lyrics that follow along.
   - [ ] The lines change roughly when they are sung.
4. **Try the shortcuts:** Ctrl+Alt+L (overlay off/on), Ctrl+Alt+K (switch layout), Ctrl+Alt+E (edit mode; drag the overlay, Enter saves).
   - [ ] All three work, even while another window (e.g. a browser) has the focus.
5. **Click through the settings:** Overlay, Appearance (themes), Hotkeys, Behavior → set Language to English and back.
   - [ ] No error message, no red or yellow notice bar (unless LRCLIB is unreachable at the moment).
6. **Optional, with a game in "Borderless window" („Randloses Fenster“) mode:**
   - [ ] The overlay stays visible above the game, and the game keeps mouse and keyboard.
7. **Uninstall:** Windows Settings → Apps → LyricHub → Uninstall.
   - [ ] Runs through without administrator rights; LyricHub is gone from the Start menu and the tray.

## If something does not work

Settings → Diagnostics → "Export diagnostics" („Diagnose exportieren“). The file lands in the log folder (`%APPDATA%\LyricHub\logs\diagnostics-<number>.json`); it contains no lyrics. Send back the file and a short description.
