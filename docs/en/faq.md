# FAQ

German: [docs/faq.md](../faq.md)

## Is LyricHub safe to use with anti-cheat?

LyricHub does not inject anything into games, does not read their memory and does not hook into DirectX. It is a separate, click-through window that stays on top. To know which window is in front it uses standard Windows window-order events, without injection. Anti-cheat systems have no reason to object to that, but there is no guarantee: no game publisher has checked or approved LyricHub.

## Does it work in exclusive fullscreen?

In a tested game, yes. If a game hides the overlay anyway, the tray shows a hint; switching the game to borderless window mode helps. Borderless and windowed games always work.

## Windows says "Windows protected your PC". Is that a virus?

No. The installer is not signed, so Windows SmartScreen reports an unknown publisher. Choose "More info" → "Run anyway". The release notes list the installer's SHA-256 checksum, so you can check that your download is the published file.

## Does LyricHub collect data?

No. There is no telemetry and no account. The only online service it talks to is [LRCLIB](https://lrclib.net), over HTTPS: to find lyrics it sends the song's title, artist, album and length. Lyrics, settings and logs stay on your PC in `%APPDATA%\LyricHub`. The diagnostics export hides song details, so it can go into a public bug report.

## Do I need Spotify Premium or a Spotify login?

No. LyricHub reads what the Spotify desktop app reports to Windows and needs no login. Ads and podcasts are skipped.

## Which music apps work?

The Spotify desktop app, from the Microsoft Store or from spotify.com. The Spotify web player and other music apps are not supported yet.

## The lyrics are early or late. What can I do?

Press Ctrl+Alt+, or Ctrl+Alt+. to move the lyrics of the current song 100 ms earlier or later (add Shift for 25 ms steps). LyricHub remembers the correction for that song. If every song is off by about the same amount, run the calibration under Settings → Sync: it sets an offset for your audio device.

## A song shows no lyrics.

LyricHub gets lyrics from LRCLIB, a community-maintained database. If LRCLIB has no synced lyrics for a song, LyricHub says so briefly and then shows just the title.

## Does it work on Windows 10?

It is built for Windows 10 from version 2004 and Windows 11, but it has only been tested on Windows 11.

## Why "source available" and not "open source"?

LyricHub uses the MIT License with the Commons Clause. The code is public, and anyone may use, change and pass it on, but nobody may sell it. Because of that one restriction, it is not an open-source licence in the strict sense, so LyricHub calls itself source available.

## Can I use it while streaming?

The licence allows it. Whether showing song lyrics on a stream is allowed is up to the streaming platform's rules and the rights holders.

## How do I remove it completely?

Uninstall LyricHub under Windows Settings → Apps. To also remove settings, the lyrics cache and logs, delete the folder `%APPDATA%\LyricHub`.
