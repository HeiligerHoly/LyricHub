// On-screen captions generated from the scene's voice-over text (cue split and timing in
// src/lib/cues.ts). They stay inside the bottom band (y > 900), which scenes keep free.
import React from "react";
import { interpolate, useCurrentFrame, useVideoConfig } from "remotion";
import { cueTimes } from "../lib/cues";
import { PROMO } from "../lib/theme";

export const Captions: React.FC<{ text: string }> = ({ text }) => {
  const frame = useCurrentFrame();
  const { fps, durationInFrames } = useVideoConfig();
  const sec = frame / fps;
  const cue = cueTimes(text, durationInFrames / fps).find((c) => sec >= c.startSec && sec < c.endSec);
  if (!cue) return null;
  const cueStart = cue.startSec * fps;
  const cueEnd = cue.endSec * fps;
  const fade = 5;
  const o = interpolate(frame, [cueStart, cueStart + fade, cueEnd - fade, cueEnd], [0, 1, 1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  return (
    <div style={{ position: "absolute", left: 0, right: 0, bottom: 52, display: "flex", justifyContent: "center", pointerEvents: "none" }}>
      <div
        style={{
          maxWidth: 1480,
          padding: "12px 28px",
          borderRadius: 14,
          background: "rgba(5,6,10,.72)",
          color: PROMO.text,
          fontFamily: PROMO.font,
          fontSize: 40,
          fontWeight: 500,
          lineHeight: 1.3,
          textAlign: "center",
          opacity: o,
        }}
      >
        {cue.text}
      </div>
    </div>
  );
};
