# LyricHub promo video

An animated promo and explainer for LyricHub (about 5:47), built in code with
[Remotion](https://www.remotion.dev). Every frame is drawn from React components: the overlay,
the settings window and the edit mode are recreated from LyricHub's own styles. All songs,
artists, lyrics and covers in it are invented, and the game backgrounds are abstract.

The finished video is on YouTube: [youtu.be/lT9tKeel5nE](https://youtu.be/lT9tKeel5nE).

- [FEATURES.md](FEATURES.md): what LyricHub does, with source references. Everything the video
  claims comes from here.
- [script.md](script.md): the voice-over script, one block per scene with start, end, spoken text
  and on-screen text. Generated from `src/data/scenes.ts`, so do not edit it by hand.

## Setup

Needs Node.js 22.18 or newer (the helper scripts are TypeScript that Node runs directly).
Everything installs into this folder:

```bash
npm install
```

## Commands

| Command | What it does |
|---|---|
| `npm run studio` | Opens Remotion Studio in the browser. Pick `Master` or any scene under `Scenes`. |
| `npm run render` | Renders the full video to `out/lyrichub-promo.mp4`. |
| `npm run thumbnail` | Renders the YouTube thumbnail (1280x720 JPG) to `out/thumbnail.jpg`. |
| `npm run stills` | Renders check stills (15 %, 50 %, 85 % of each scene) to `out/stills/`. |
| `npm run script:md` | Rewrites `script.md` from `src/data/scenes.ts` and warns if a scene is too short for its text. |
| `npm run typecheck` | Checks the TypeScript. |

The stills script also takes scene ids and custom positions, for all listed scenes or per scene:

```bash
node scripts/stills.ts --at=0.1,0.3 s05-out-of-the-way s06-hotkeys
```

```bash
node scripts/stills.ts s05-out-of-the-way@0.1,0.3 s07-edit-mode@0.6
```

The render output is 1920x1080, 60 fps, H.264 (CRF 18, yuv420p, BT.709) with an AAC audio track.
Without a voice-over that track is silent, so the file already has an audio slot that video
editors and YouTube accept. Settings are in `remotion.config.ts`.

## Adding the voice-over

Record from [script.md](script.md). Captions are burned into the video from the same text, so
read it as written. Then pick one of two ways:

### Option A: one file per scene (recommended)

Put one audio file per scene into `public/vo/`, named after the scene id:

```
public/vo/s01-hook.mp3
public/vo/s02-what.mp3
...
public/vo/s15-outro.mp3
```

`.mp3`, `.wav` and `.m4a` work. Each file starts with its scene. If a file is longer than its
scene allows, the scene stretches to the file's length plus 1.2 s, and every animation and
caption inside it stretches with it. Scenes without a file keep their length, so you can add
files one at a time.

### Option B: one file for the whole video

Put the whole recording at `public/vo/voiceover.mp3` (or `.wav` / `.m4a`). It starts at 0:00
and the scenes are not stretched for it. Set each scene's `durationSec` in `src/data/scenes.ts`
so the scene changes land on your recording's pauses, then run `npm run script:md` to check the
new start times.

Do not combine both options, or the two voices play on top of each other.

### Re-render

```bash
npm run render
```

To listen to a single scene with its voice-over first, open `npm run studio` and pick the scene
under `Scenes`.

## Adding background music

Put one or more instrumental tracks into `public/music/` (`.mp3`, `.wav` or `.m4a`). They play in
file-name order, each overlapping the next by a 3 s crossfade, and the list repeats until the
video ends. Name them `01-….mp3`, `02-….mp3` to set the order. A single 3-minute track works too;
it loops with a crossfade.

The music fades in over 1.5 s and out over the last 4 s. While a voice-over speaks it drops to
about a quarter of its level and comes back up only in pauses longer than 2.5 s, so it does not
pump between sentences. Levels and fade lengths are the `MUSIC_*` values in
`src/data/scenes.ts`. Then run `npm run render`.

A track listed in `MUSIC_TRACKS` (in the same file) is fitted to the video instead of looped,
as long as it is the only file in `public/music/`. Its entry holds the bar length, one bar
start and a loop window, all measured from the file. The track starts with the video, jumps
back once by whole bars inside the window with an equal-power crossfade, and plays its own
ending into the last frame, with no fade-out. When scenes get longer because of a voice-over,
the jump length adapts. If the jump no longer fits the window, the track loops instead.

### Music in this video

"Under the Streetlights - outrun racing 1980s synth soundtrack" by melodyayresgriffiths, from
Pixabay (pixabay.com/music/video-games-under-the-streetlights-outrun-racing-1980s-synth-soundtrack-184776/),
under the Pixabay Content License. When it was downloaded on 2026-10-26, its page showed neither
a "Content ID Registered" badge nor an "AI generated" label. The track (6:12) is longer than the
video and fades out on its own from about 5:35, which lands on the outro, so it plays from the
start with no cut. No credit is required; to credit it in the video description:
`Music: "Under the Streetlights" by melodyayresgriffiths (Pixabay)`.

The audio files are not in the repository: `public/music/` and `public/vo/` are in `.gitignore`,
because the Pixabay licence does not allow passing the file on by itself. To render with music,
download the track from the page above into `public/music/` (any file name ending in `.mp3`).
Without it, the video renders with a silent audio track.

Pick a source whose licence covers a promo video on YouTube:

- **YouTube Audio Library** (studio.youtube.com, Audio library): safest for YouTube. Its tracks are
  not claimed by Content ID and the video can be monetised. Filter by genre, mood, duration and
  "Attribution not required"; for Creative Commons tracks put the credit line in the video
  description.
- **Pixabay Music**: free, no credit needed. Anyone can upload there, and many tracks carry a
  "Content ID Registered" badge on their page, which means YouTube will claim the video. Pick a
  track without that badge, and note where and when you downloaded it in case of a claim.
- **NoCopyrightSounds** is free for creators, but its policy asks brands promoting a product to
  get a licence first, and most of its tracks have vocals. Not a good fit here.

The video itself must keep to the rules above: no real song lyrics, titles or artists on screen.
Instrumental music in the background is fine.

## Where the timing lives

- **Scene lengths:** `durationSec` per scene in `src/data/scenes.ts`. The master timeline,
  captions and `script.md` follow from it.
- **Animations inside a scene:** each scene component in `src/scenes/` places its beats as
  fractions of the scene length (for example `window(0.27, 0.335)`). They are aligned to the
  caption cues, which assume the text is read at an even pace, so a longer or shorter scene
  keeps them in sync. If one sentence in your recording runs much faster or slower than the
  rest, shift that scene's fractions.
- **Captions:** each sentence is one cue, or several when it has more than 13 words, broken
  after commas or before a new clause. Cues are timed by word count across the scene
  (`src/lib/cues.ts`). When you change a scene's `vo`, check that its beats still land on the
  right cues.

## Changing text

- Spoken text and captions: the scene's `vo` in `src/data/scenes.ts`.
- On-screen labels and callouts: the scene's `onScreen` list in the same file.
- Demo songs and lyrics: `src/data/demo.ts`. Keep them invented: no real songs, artists,
  lyrics, album art, Spotify logo or game footage.

After a text change, run `npm run script:md` and `npm run stills` and look at the stills.

## Project layout

```
src/data/       scenes.ts (single source of truth), demo.ts (invented songs)
src/scenes/     Intro.tsx (s01–s03), Core.tsx (s04–s08), More.tsx (s09–s15)
src/ui/         recreated LyricHub UI: Overlay, SettingsWindow, EditMode, Desktop,
                plus GameScene, Captions and shared parts in Kit.tsx
src/lib/        theme colours, timing helpers, karaoke word timing, font loading
src/vo.ts       finds voice-over and music files, stretches scenes to fit the voice-over
src/music.tsx   background music: fitted track or playlist loop, ducking under the voice-over
src/Root.tsx    compositions: Master and one per scene
scripts/        build-script-md.ts, stills.ts
public/         lyrichub-icon.svg, vo/ (voice-over files), music/ (background music)
```

## Fonts and licences

- Inter (SIL Open Font License) is bundled through `@fontsource-variable/inter`.
- The settings window, edit mode and Japanese lines use the Windows fonts Segoe UI and Yu Gothic
  UI, as LyricHub does. Render on Windows; elsewhere those parts fall back to other fonts.
- Remotion is free for individuals and for companies with up to three people. Larger companies
  need a company licence; see [remotion.dev/license](https://www.remotion.dev/license).
