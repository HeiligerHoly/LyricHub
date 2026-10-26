// Invented demo content. No real songs, artists, lyrics or album art anywhere in the video.
import type { Rgb } from "../lib/theme";

export type DemoSong = {
  title: string;
  artist: string;
  /** Abstract cover: two gradient colours and a shape colour. */
  cover: [string, string, string];
  /** The accent LyricHub would pick from this cover (vivid, readable on the dark plate). */
  accent: Rgb;
  lines: string[];
};

export const SONG_A: DemoSong = {
  title: "Neon Tide",
  artist: "Paper Satellites",
  cover: ["#3b1d6e", "#c2418f", "#ffb3e6"],
  accent: [255, 138, 216],
  lines: [
    "We ride the static through the midnight glow",
    "Every signal sings a song we know",
    "Turn the city lights to slow motion",
    "Hold the chorus like an open ocean",
    "Light the night until the morning shows",
  ],
};

export const SONG_B: DemoSong = {
  title: "Gold Run",
  artist: "The Lantern Kids",
  cover: ["#5a2a05", "#f08a24", "#ffe1a8"],
  accent: [255, 179, 92],
  lines: [
    "Running on a highway made of gold",
    "Every heartbeat louder than the cold",
    "Sparks across the asphalt, never slow",
  ],
};

export const SONG_C: DemoSong = {
  title: "Lowlight",
  artist: "Mira Vale",
  cover: ["#06343a", "#1fa39a", "#b8fff4"],
  accent: [111, 227, 214],
  lines: ["Quiet rooms and paper planes", "Counting stars between the rain", "Every window holds a flame"],
};

export const SONG_JP: DemoSong = {
  title: "Yozora Signal",
  artist: "Kaze no Lantern",
  cover: ["#0b1e4a", "#3d6fd6", "#cfe0ff"],
  accent: [150, 196, 255],
  lines: [],
};

/** Invented Japanese placeholder lyric and its romaji (written for this video). */
export const JP_LINE = { text: "夜空にひかる小さな声", romaji: "yozora ni hikaru chiisana koe" };

/** LyricHub's own Japanese sample line (src/lib/overlay/Overlay.svelte:42-44). */
export const JP_SAMPLE = {
  text: "次の歌詞はここに少し小さく表示されます",
  romaji: "tsugi no kashi wa koko ni sukoshi chiisaku hyōji saremasu",
};

/** LyricHub's sample texts used by the edit mode and the Appearance preview (locales/en.ts). */
export const SAMPLE = {
  line: "This is a sample line for alignment",
  previous: "The line before is dimmed",
  later: "and one more line after that",
  title: "Song title",
  artist: "Artist",
};
