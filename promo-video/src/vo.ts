// Audio slots. Looks for voice-over files in public/vo/ and music in public/music/, and works out
// the timeline: scenes stretch to fit their voice-over. See README.md "Adding the voice-over".
import { getAudioDurationInSeconds } from "@remotion/media-utils";
import { getStaticFiles, staticFile } from "remotion";
import { FPS, MUSIC_DIR, SCENES, VO_EXTENSIONS, VO_SINGLE_FILE_BASENAME, VO_TAIL_SEC } from "./data/scenes";

const staticNames = (): string[] => getStaticFiles().map((f) => f.name.replace(/\\/g, "/"));
const isAudio = (name: string): boolean => VO_EXTENSIONS.some((ext) => name.toLowerCase().endsWith(`.${ext}`));

const voFile = (basename: string): string | null => {
  const files = staticNames();
  for (const ext of VO_EXTENSIONS) {
    const name = `vo/${basename}.${ext}`;
    if (files.includes(name)) return name;
  }
  return null;
};

/** public/vo/<scene id>.<ext> if present. */
export const sceneVoFile = (id: string): string | null => voFile(id);

/** public/vo/voiceover.<ext> if present. */
export const singleVoFile = (): string | null => voFile(VO_SINGLE_FILE_BASENAME);

/** The audio files in public/music/, in file-name order. */
export const musicFiles = (): string[] =>
  staticNames()
    .filter((n) => n.startsWith(`${MUSIC_DIR}/`) && isAudio(n))
    .sort();

/** Seconds of voice-over per scene, 0 where a scene has no file. */
async function sceneVoSeconds(): Promise<number[]> {
  return Promise.all(
    SCENES.map(async (s) => {
      const f = sceneVoFile(s.id);
      return f ? getAudioDurationInSeconds(staticFile(f)) : 0;
    }),
  );
}

const framesFor = (voSec: number[]): number[] =>
  SCENES.map((s, i) => Math.round(Math.max(s.durationSec, voSec[i] ? voSec[i] + VO_TAIL_SEC : 0) * FPS));

/** Frames per scene: the configured length, longer if a per-scene VO file needs it. */
export async function sceneDurations(): Promise<number[]> {
  return framesFor(await sceneVoSeconds());
}

export type Timeline = {
  /** Frames per scene. */
  durations: number[];
  /** Seconds of voice-over per scene (0 without a file). */
  voSec: number[];
  /** Length of public/vo/voiceover.<ext>, 0 without it. */
  singleVoSec: number;
  /** Music files with their lengths in seconds. */
  music: { file: string; sec: number }[];
};

export const EMPTY_TIMELINE: Timeline = {
  durations: SCENES.map((s) => Math.round(s.durationSec * FPS)),
  voSec: SCENES.map(() => 0),
  singleVoSec: 0,
  music: [],
};

/** Everything the master needs to place scenes, voice-over and music. */
export async function timeline(): Promise<Timeline> {
  const voSec = await sceneVoSeconds();
  const single = singleVoFile();
  return {
    durations: framesFor(voSec),
    voSec,
    singleVoSec: single ? await getAudioDurationInSeconds(staticFile(single)) : 0,
    music: await Promise.all(musicFiles().map(async (file) => ({ file, sec: await getAudioDurationInSeconds(staticFile(file)) }))),
  };
}
