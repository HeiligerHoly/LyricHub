// Turns a list of demo lines plus a song clock into what the overlay shows at time t.
import { Easing } from "remotion";
import { OVERLAY_FONT } from "../lib/theme";
import { buildFillTrack, fillAt, type FillTrack } from "../lib/words";

export type Timed = { text: string; start: number; end: number };

/** Lines one after another, each `dur` seconds long (number or per-line array). */
export function schedule(lines: string[], start: number, dur: number | number[]): Timed[] {
  let t = start;
  return lines.map((text, i) => {
    const d = Array.isArray(dur) ? dur[i] ?? dur[dur.length - 1] : dur;
    const item = { text, start: t, end: t + d };
    t += d;
    return item;
  });
}

const cache = new Map<string, FillTrack>();
const ACTIVE_FONT = `650 28px ${OVERLAY_FONT}`;
const ease = Easing.bezier(0.2, 0.8, 0.2, 1);

export type LyricState = {
  index: number;
  active: string;
  prev: string;
  next: string;
  next2: string;
  fill: number;
  /** 0..1 line-change animation (220 ms). */
  lineIn: number;
};

export function lyricAt(timed: Timed[], t: number): LyricState {
  let index = -1;
  for (let i = 0; i < timed.length; i++) if (t >= timed[i].start) index = i;
  if (index < 0) {
    return { index, active: "", prev: "", next: timed[0]?.text ?? "", next2: timed[1]?.text ?? "", fill: 0, lineIn: 1 };
  }
  const line = timed[index];
  const key = `${line.text}|${line.start}|${line.end}`;
  let track = cache.get(key);
  if (!track) {
    track = buildFillTrack(line.text, ACTIVE_FONT, line.start, line.end);
    cache.set(key, track);
  }
  const lineIn = ease(Math.max(0, Math.min(1, (t - line.start) / 0.22)));
  return {
    index,
    active: line.text,
    prev: timed[index - 1]?.text ?? "",
    next: timed[index + 1]?.text ?? "",
    next2: timed[index + 2]?.text ?? "",
    fill: fillAt(track, t),
    lineIn,
  };
}
