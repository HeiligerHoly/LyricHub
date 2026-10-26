// Single source of truth for the promo: canvas, scene order, durations, voice-over text and
// on-screen text. Change durations here to fit the final voice-over; everything else (master
// timeline, captions, script.md) follows. Plain data only, so Node can import it directly
// (scripts/build-script-md.ts) and Remotion bundles it.

export const FPS = 60;
export const WIDTH = 1920;
export const HEIGHT = 1080;

/** Words per minute used to check that each scene leaves enough time for its voice-over. */
export const WPM = 150;

/**
 * Voice-over. Two ways to add it, no re-editing needed:
 * 1. Per scene: put `public/vo/<scene id>.mp3` (or .wav), e.g. `public/vo/s06-hotkeys.mp3`.
 *    Each file starts with its scene; a scene grows automatically if its file is longer than
 *    `durationSec - VO_TAIL_SEC`.
 * 2. One file: put `public/vo/voiceover.mp3` (or .wav). It starts at 0:00; adjust the
 *    `durationSec` values below so the scenes match it.
 */
export const VO_SINGLE_FILE_BASENAME = "voiceover";
export const VO_EXTENSIONS = ["mp3", "wav", "m4a"] as const;
/** Seconds of picture kept after a per-scene voice-over file ends (matches the last caption's end). */
export const VO_TAIL_SEC = 0.6;
/** Seconds before the first caption of a scene appears. */
export const CAPTION_LEAD_SEC = 0.4;

/**
 * Background music: every audio file in `public/music/` (mp3, wav, m4a) plays in file-name
 * order, each overlapping the next by the crossfade, and the list repeats until the video ends.
 */
export const MUSIC_DIR = "music";
/** Music level (0–1) with no voice-over speaking. */
export const MUSIC_VOLUME = 0.6;
/** Music level while the voice-over speaks. */
export const MUSIC_UNDER_VO_VOLUME = 0.14;
/** Voice-over pauses shorter than this keep the music down, so it does not pump between sentences. */
export const MUSIC_GAP_SEC = 2.5;
export const MUSIC_FADE_IN_SEC = 1.5;
/** Fade at the very end, unless the music reaches its own ending there (see MUSIC_TRACKS). */
export const MUSIC_FADE_OUT_SEC = 4;
export const MUSIC_CROSSFADE_SEC = 3;

export type MusicTrackInfo = {
  /** Length of one bar in seconds and the time of one bar start, measured from the file. */
  barSec: number;
  firstBarSec: number;
  /** Seconds between which the track may jump back: steady groove, no intro, break or ending. */
  loopWindow: [number, number];
};

/**
 * Measured tracks, by file name in public/music/. When the only music file is listed here, it is
 * fitted to the video instead of looped: it starts at the beginning, jumps back once by whole
 * bars inside its loop window (crossfaded), and plays its own ending into the last frame.
 */
// The current track, "Under the Streetlights" (6:12), is longer than the video and fades out on
// its own from about 5:35, so it plays from the start without an entry here. Example entry:
//   "my-track.mp3": { barSec: 2.38616, firstBarSec: 0.012, loopWindow: [9.6, 195.6] },
export const MUSIC_TRACKS: Record<string, MusicTrackInfo> = {};

export type Scene = {
  id: string;
  title: string;
  /** Scene length in seconds (before any automatic extension by a per-scene VO file). */
  durationSec: number;
  /** Spoken text, read as-is by ElevenLabs or a human. Also drives the on-screen captions. */
  vo: string;
  /** Text that appears on screen as graphics (not the captions). */
  onScreen: string[];
};

export const SCENES: Scene[] = [
  {
    id: "s01-hook",
    title: "Hook",
    durationSec: 9,
    vo: "You're deep in a game. The music is playing. And for once, you can actually sing along.",
    onScreen: ["LyricHub"],
  },
  {
    id: "s02-what",
    title: "What it is",
    durationSec: 19,
    vo:
      "This is LyricHub, a free overlay for Windows. It shows the song that's playing in Spotify, with synced, karaoke-style lyrics, right on top of your game. No Spotify login, no account, no API key. LyricHub reads the song straight from Windows.",
    onScreen: ["Synced lyrics over your games", "No login", "No account", "No API key", "Reads the song from Windows"],
  },
  {
    id: "s03-setup",
    title: "Setup",
    durationSec: 25,
    vo:
      "Setup takes a minute. The installer is about ten megabytes and installs just for your user, so no admin rights are needed. Windows may show a SmartScreen warning once, because the installer isn't signed. After that, LyricHub lives in your system tray. Start Spotify, press play, and the overlay appears at the top of your screen.",
    onScreen: [
      "About 10 MB",
      "Installs for your user only",
      "No admin rights",
      "Not signed: SmartScreen may warn once",
      "Lives in the system tray",
      "Start Spotify, press play",
    ],
  },
  {
    id: "s04-overlay",
    title: "Overlay in action",
    durationSec: 23,
    vo:
      "Here's the overlay in action. The current line fills from left to right, word by word, in time with the vocals. The next line waits just below. When a new song starts, the old one fades out, the new title slides in, and the accent color is picked from the album cover.",
    onScreen: ["Word-by-word fill", "Next line preview", "New song: new title", "Accent color from the cover"],
  },
  {
    id: "s05-out-of-the-way",
    title: "Never in the way",
    durationSec: 22,
    vo:
      "It never gets in your way. Clicks go straight through to the game, it never steals keyboard focus, and it stays out of Alt+Tab and the taskbar. It even holds its place on top when a game keeps pushing itself to the front, in borderless, windowed and fullscreen games.",
    onScreen: [
      "Clicks go through",
      "Never steals focus",
      "Not in Alt+Tab or the taskbar",
      "Stays on top",
      "Borderless · Windowed · Fullscreen",
    ],
  },
  {
    id: "s06-hotkeys",
    title: "Hotkeys",
    durationSec: 35,
    vo:
      "Everything runs on hotkeys, so you never have to leave the game. Control, Alt, L shows or hides the overlay. Control, Alt, E opens the edit mode. Control, Alt, K switches the layout. Control, Alt, R turns romaji on for Japanese songs. Control, Alt, comma and period move the lyrics of the current song a hundred milliseconds earlier or later. Add Shift for fine steps of twenty-five. All eight shortcuts can be changed or switched off.",
    onScreen: [
      "Show / hide overlay",
      "Edit mode (again: save)",
      "Switch layout",
      "Romaji on/off",
      "Song: lyrics 100 ms earlier",
      "Song: lyrics 100 ms later",
      "Song: lyrics 25 ms earlier",
      "Song: lyrics 25 ms later",
      "All 8 can be changed or turned off",
    ],
  },
  {
    id: "s07-edit-mode",
    title: "Edit mode",
    durationSec: 23,
    vo:
      "Press Control, Alt, E, and the overlay becomes editable while the game keeps focus. Drag it anywhere, scale it from the corners, change the width from the sides, and let it snap to nine anchor points. Enter saves, Escape cancels, and on multiple monitors you can send it to the next screen.",
    onScreen: ["Drag to move", "Corners scale", "Sides change the width", "9 snap points", "Enter saves · Esc cancels", "Next monitor"],
  },
  {
    id: "s08-behavior",
    title: "Smart behavior",
    durationSec: 34,
    vo:
      "LyricHub also knows when to step back. When you pause, the overlay dims, and after ten seconds it fades away. If a song has no synced lyrics, or is instrumental, it says so briefly and then shows just the title. Ads and podcasts are skipped. If the lyrics service can't be reached, a small dot appears and LyricHub retries on its own. And if you like, it only shows up while a fullscreen game is in front.",
    onScreen: [
      "Paused: dims, then fades out",
      "No synced lyrics",
      "Instrumental",
      "Ads and podcasts skipped",
      "Retries on its own",
      "Only with a fullscreen app",
    ],
  },
  {
    id: "s09-layouts",
    title: "Layouts",
    durationSec: 18,
    vo:
      "Pick the layout that fits your game. Karaoke shows the current and next line. Bar puts everything in a single row. Stack shows the lines before and after. Minimal is just the line itself, outlined, with no background.",
    onScreen: ["Karaoke", "Bar", "Stack", "Minimal"],
  },
  {
    id: "s10-appearance",
    title: "Appearance",
    durationSec: 24,
    vo:
      "Make it yours. Choose from five themes: Glass Dark, Glass Light, Neon, Mono, or your own custom colors. The accent can follow the album cover or stay fixed. Set the background opacity, pick any installed font, or reduce motion. While the Appearance page is open, the overlay previews every change live.",
    onScreen: [
      "Glass Dark",
      "Glass Light",
      "Neon",
      "Mono",
      "Custom",
      "Accent: from the cover or fixed",
      "Opacity · Font · Reduce motion",
      "Live preview",
    ],
  },
  {
    id: "s11-sync",
    title: "Sync tools",
    durationSec: 36,
    vo:
      "Sync is what LyricHub is built around. If a song's lyrics run early or late, nudge them with a hotkey. The correction is saved for that song only, so the next song starts fresh. Every audio device gets its own offset, and new Bluetooth headphones start with a sensible default. The calibration assistant measures your setup: just tap Space when you hear a line begin. And on the diagnostics page, the Sync Lab shows exactly how the clock follows Spotify.",
    onScreen: [
      "Nudge per song",
      "Saved for this song only",
      "Offset per audio device",
      "Calibration: tap Space at each line start",
      "Sync Lab",
    ],
  },
  {
    id: "s12-romaji",
    title: "Japanese and romaji",
    durationSec: 11,
    vo: "Listening to Japanese songs? Press Control, Alt, R, and every Japanese line switches to romaji, converted offline on your PC.",
    onScreen: ["Romaji, converted offline"],
  },
  {
    id: "s13-settings",
    title: "Settings",
    durationSec: 15,
    vo:
      "Everything else lives in the settings window: overlay position and behavior, appearance, sync, hotkeys, startup with Windows, and diagnostics. The interface speaks English and German.",
    onScreen: ["Overlay", "Appearance", "Sync", "Hotkeys", "Behavior", "Diagnostics", "English · Deutsch"],
  },
  {
    id: "s14-tests",
    title: "Tested",
    durationSec: 39,
    vo:
      "And it's tested. Ninety-three automated unit tests cover lyrics, sync, settings and the overlay window. A built-in self-test checks focus, click-through and staying on top. Against a simulated game that forces itself to the front every half second, the overlay stayed on top in every one of a hundred and twenty samples. Measured on a real setup, the median sync error is ninety-six milliseconds, and after a seek the lyrics are back in sync within about thirty. While playing, LyricHub uses less than half a percent of the CPU.",
    onScreen: [
      "93 unit tests · all passing",
      "Built-in self-test",
      "120 / 120 on top",
      "96 ms median sync error",
      "≈30 ms after a seek",
      "0.48 % CPU while playing",
    ],
  },
  {
    id: "s15-outro",
    title: "Outro",
    durationSec: 14,
    vo:
      "LyricHub is free to use and share, but not to sell. Download it from GitHub, start a song, and sing along. LyricHub is not affiliated with Spotify.",
    onScreen: ["LyricHub", "github.com/HeiligerHoly/LyricHub", "Free to use and share · Not for sale", "Not affiliated with Spotify"],
  },
];

export const wordCount = (s: string): number => s.trim().split(/\s+/).filter(Boolean).length;

/** Seconds the voice-over of a scene needs at WPM. */
export const voSeconds = (s: Scene): number => (wordCount(s.vo) / WPM) * 60;
