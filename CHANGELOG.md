# Changelog

The notable changes of each version. The release notes in [docs/release-notes/](docs/release-notes/) have the details.

## Unreleased

### Added

- Setting "Fill word by word" on the Appearance page. Off, the current line shows whole in the accent colour at once instead of filling with mostly estimated word times.

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
