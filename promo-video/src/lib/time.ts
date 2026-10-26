// Timing helpers. Scene beats are expressed as fractions of the scene length so a scene can be
// stretched to a longer voice-over without re-editing.
import { Easing, interpolate, useCurrentFrame, useVideoConfig } from "remotion";

export const EASE = Easing.bezier(0.2, 0.8, 0.2, 1); // LyricHub's line-change curve
export const EASE_IO = Easing.inOut(Easing.cubic);

export function useScene() {
  const frame = useCurrentFrame();
  const { fps, durationInFrames } = useVideoConfig();
  const sec = frame / fps;
  const durSec = durationInFrames / fps;
  /** 0→1 progress between two fractions of the scene, eased. */
  const p = (from: number, to: number, ease = EASE): number =>
    interpolate(frame, [from * durationInFrames, to * durationInFrames], [0, 1], {
      extrapolateLeft: "clamp",
      extrapolateRight: "clamp",
      easing: ease,
    });
  /** Fade in at `from`, fade out at `to` (fractions), with a fixed fade length in seconds. */
  const window = (from: number, to: number, fadeSec = 0.35): number => {
    const a = from * durationInFrames;
    const b = to * durationInFrames;
    const f = fadeSec * fps;
    if (b - a < 2 * f) return frame >= a && frame <= b ? 1 : 0;
    return interpolate(frame, [a, a + f, b - f, b], [0, 1, 1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  };
  /** Seconds since a fraction of the scene (negative before it). */
  const since = (from: number): number => sec - from * durSec;
  return { frame, fps, durationInFrames, sec, durSec, p, window, since };
}

export const clamp01 = (v: number): number => Math.max(0, Math.min(1, v));
