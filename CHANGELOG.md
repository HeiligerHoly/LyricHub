# Changelog

The notable changes of each version. The release notes in [docs/release-notes/](docs/release-notes/) have the details.

## Unreleased

### Added

- Plugin host (PLAN-plugins.md, P1): Settings → Plugins installs a plugin from a ZIP after showing its name, version, permissions and SHA-256, switches it on or off, and shows its state. Plugins run as separate programs at below-normal priority inside a job object that ends them with LyricHub; a crashing plugin is restarted after 2, 10 and 60 s, then left off. There are no plugins to install yet; the first, real word times from the audio, follows.
- Word times from plugins (P2): a plugin with the capability `word_timing` hears the track, the playback position and the synced lines, and answers with word times. LyricHub checks them line by line, stores them per lyrics fingerprint, and they replace the estimate, with the word glow. Diagnostics shows where a song's word times come from. Removing a plugin also removes its word times.

## 1.1.0 – 2026-10-28

See [docs/release-notes/v1.1.0.md](docs/release-notes/v1.1.0.md).

### Added

- Experimental browser support: Settings → Behavior → Player follows a browser instead of Spotify, picked from the media sessions Windows knows. YouTube Music works in Edge and Firefox-based browsers, regular YouTube and the Spotify web player in Edge; Firefox-based browsers often report no time on regular YouTube ([browser spike](docs/en/spike-browser.md)). YouTube titles such as "Artist - Song (Official Video)" by a VEVO or "Topic" channel are cleaned before the lookup; playback without a song length or artist counts as a video, gets no lyrics lookup and shows the compact card. Each player has its own offset (Settings → Sync), and the Sync Lab notes that browsers report rarely.
- Setting "Fill word by word" on the Appearance page. Off, the current line shows whole in the accent colour at once instead of filling with mostly estimated word times.

### Fixed

- The outline and shadows of themes with an outline were cut off at the edges of each text line in the overlay, most visibly on the first and last letter and above and below the title. Text boxes now leave them room; lines that are too long still end in an ellipsis.

### Changed

- The word timing estimate for lyrics without word times is closer to the singing: words share a line by syllables instead of letters, the last word is held longer, and a line is sung for nine tenths of the interval to the next one but rarely more than 400 ms per syllable. Measured against 79 word-aligned songs, the median error of a word start fell from 184 to 136 ms and the slowest tenth from 1.28 s to 0.73 s ([docs/en/word-timing.md](docs/en/word-timing.md)).

## 1.0.1 – 2026-10-27

See [docs/release-notes/v1.0.1.md](docs/release-notes/v1.0.1.md).

### Fixed

- AltGr no longer triggers the hotkeys: AltGr reaches Windows as Ctrl+Alt, so AltGr+E (€ on a German keyboard) opened the edit mode and the € never arrived. When right Alt is held and the combination types a character in the active keyboard layout, LyricHub now hands the key back to the window in front. Ctrl+Alt with the left Alt key works as before.
- `THIRD_PARTY_NOTICES.md` lists SvelteKit 3.1.0 and devalue, which the SvelteKit 3 client runtime brings into the app's pages.

### Changed

- A song without synced lyrics, and a podcast episode, shrinks the overlay to a compact card with cover, title and artist, at the anchor side of its position. The 4-second note "No synced lyrics found" is gone. The Bar layout keeps its single row and only gets narrower.
- Logs, the Diagnostics page and self-test reports name the program behind a window (for example `Discord.exe`) instead of its title, which could show private things such as chat servers or documents.
- The diagnostics export hides song titles, artists and albums, so it can go into a public bug report.
- LRCLIB requests identify LyricHub with a link to this repository.
- The settings window and the overlay run on SvelteKit 3. The configuration moved from `svelte.config.js` into `vite.config.js`, and imports use `#lib` instead of `$lib`. This fixes two low-severity SvelteKit advisories and makes the old `cookie` override unnecessary. The hourly version check that SvelteKit 3 would start is turned off.
- promo-video: zod 4.6.6 instead of 4.5.4 (security fix).

### Added

- Bug and idea forms, a pull request checklist, [SECURITY.md](SECURITY.md), [CONTRIBUTING.md](CONTRIBUTING.md), a [FAQ](docs/en/faq.md) and this changelog.
- [promo-video/](promo-video/): the promo video as a Remotion project, with the YouTube thumbnail.

## 1.0.0 – 2026-10-26

First public release. See [docs/release-notes/v1.0.0.md](docs/release-notes/v1.0.0.md).
