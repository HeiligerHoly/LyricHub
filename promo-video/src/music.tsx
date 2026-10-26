// Background music from public/music/. A single measured track (MUSIC_TRACKS) is fitted to the
// video: one bar-aligned jump back, then its own ending into the last frame. Anything else plays
// as a playlist in file-name order with crossfades, repeating until the video ends. The music
// ducks while the voice-over speaks. Levels and fade lengths are in src/data/scenes.ts.
import React from "react";
import { Audio, interpolate, Sequence, staticFile, useVideoConfig } from "remotion";
import {
  MUSIC_CROSSFADE_SEC,
  MUSIC_FADE_IN_SEC,
  MUSIC_FADE_OUT_SEC,
  MUSIC_GAP_SEC,
  MUSIC_TRACKS,
  MUSIC_UNDER_VO_VOLUME,
  MUSIC_VOLUME,
} from "./data/scenes";
import type { Timeline } from "./vo";

const clamp = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;

/** One stretch of a music file on the video timeline (all values in seconds). */
type Piece = { file: string; at: number; from: number; to: number };

/** Frame ranges in which the voice-over speaks, with short pauses merged. */
function speechRanges(t: Timeline, fps: number): [number, number][] {
  const ranges: [number, number][] = [];
  if (t.singleVoSec > 0) ranges.push([0, t.singleVoSec * fps]);
  let start = 0;
  t.durations.forEach((d, i) => {
    if (t.voSec[i] > 0) ranges.push([start, start + t.voSec[i] * fps]);
    start += d;
  });
  ranges.sort((a, b) => a[0] - b[0]);
  const merged: [number, number][] = [];
  for (const r of ranges) {
    const last = merged[merged.length - 1];
    if (last && r[0] - last[1] < MUSIC_GAP_SEC * fps) last[1] = Math.max(last[1], r[1]);
    else merged.push([r[0], r[1]]);
  }
  return merged;
}

/**
 * A measured track that is longer than half the video: start it so that, after one jump back
 * by whole bars, it ends exactly with the video. Null when the jump does not fit its window.
 */
function fitted(file: string, sec: number, videoSec: number): Piece[] | null {
  const info = MUSIC_TRACKS[file.split("/").pop() ?? ""];
  if (!info || videoSec <= sec) return null;
  const extra = videoSec - sec;
  const bars = Math.floor(extra / info.barSec);
  const delay = extra - bars * info.barSec; // under one bar of silence at the start
  const jump = bars * info.barSec;
  // latest bar start for the jump whose crossfade still ends inside the window
  const [lo, hi] = info.loopWindow;
  const n = Math.floor((hi - MUSIC_CROSSFADE_SEC - info.firstBarSec) / info.barSec);
  const leave = info.firstBarSec + n * info.barSec;
  const land = leave - jump;
  if (land < lo) return null;
  return [
    { file, at: delay, from: 0, to: leave + MUSIC_CROSSFADE_SEC },
    { file, at: delay + leave, from: land, to: sec },
  ];
}

/** The playlist end to end, each piece overlapping the previous one by the crossfade. */
function looped(tracks: Timeline["music"], videoSec: number): Piece[] {
  const pieces: Piece[] = [];
  for (let at = 0, i = 0; at < videoSec; i++) {
    const t = tracks[i % tracks.length];
    pieces.push({ file: t.file, at, from: 0, to: t.sec });
    at += t.sec - MUSIC_CROSSFADE_SEC;
  }
  return pieces;
}

export const BackgroundMusic: React.FC<{ timeline: Timeline }> = ({ timeline }) => {
  const { fps, durationInFrames } = useVideoConfig();
  const videoSec = durationInFrames / fps;
  const tracks = timeline.music.filter((m) => m.sec > 2 * MUSIC_CROSSFADE_SEC);
  if (!tracks.length) return null;
  const fit = tracks.length === 1 ? fitted(tracks[0].file, tracks[0].sec, videoSec) : null;
  const pieces = fit ?? looped(tracks, videoSec);
  const xf = MUSIC_CROSSFADE_SEC * fps;
  const speech = speechRanges(timeline, fps);
  // ducks down in 0.5 s before the voice starts, comes back up over 1 s after it stops
  const level = (f: number): number => {
    const duck = Math.max(0, ...speech.map(([a, b]) => interpolate(f, [a - 0.5 * fps, a, b, b + fps], [0, 1, 1, 0], clamp)));
    const fadeIn = interpolate(f, [0, MUSIC_FADE_IN_SEC * fps], [0, 1], clamp);
    const fadeOut = fit ? 1 : interpolate(f, [durationInFrames - MUSIC_FADE_OUT_SEC * fps, durationInFrames], [1, 0], clamp);
    return (MUSIC_VOLUME + (MUSIC_UNDER_VO_VOLUME - MUSIC_VOLUME) * duck) * fadeIn * fadeOut;
  };
  // equal-power crossfade, so the overlap does not dip in loudness
  const fadeUp = (f: number) => Math.sin((Math.PI / 2) * interpolate(f, [0, xf], [0, 1], clamp));
  return (
    <>
      {pieces.map((p, k) => {
        // a sequence starts on a whole frame; the audio trim takes up the remainder, so pieces
        // stay sample-accurate against each other (a bar is not a whole number of frames)
        const at = Math.ceil(p.at * fps);
        const trim = p.from * fps + (at - p.at * fps);
        const len = Math.floor((p.to - p.from) * fps - (at - p.at * fps));
        const last = k === pieces.length - 1;
        return (
          <Sequence key={k} from={at} durationInFrames={len} name={`music: ${p.file}`}>
            <Audio
              src={staticFile(p.file)}
              trimBefore={trim}
              volume={(f) => {
                const xin = k === 0 ? 1 : fadeUp(f);
                const xout = last && fit ? 1 : fadeUp(len - f);
                return level(at + f) * xin * xout;
              }}
            />
          </Sequence>
        );
      })}
    </>
  );
};
