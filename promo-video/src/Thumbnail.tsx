// YouTube thumbnail (still "Thumbnail"): the overlay over the abstract game scene, a large
// headline and the logo. Laid out at 1920x1080; render it at 1280x720 (see README.md).
import React from "react";
import { AbsoluteFill, Img, staticFile } from "remotion";
import { SONG_A } from "./data/demo";
import { FontGate } from "./lib/fonts";
import { PROMO } from "./lib/theme";
import { GameScene } from "./ui/GameScene";
import { OverlayStage } from "./ui/Stage";

export const Thumbnail: React.FC = () => (
  <FontGate>
    <AbsoluteFill style={{ background: PROMO.bg, fontFamily: PROMO.font, color: PROMO.text, overflow: "hidden" }}>
      <GameScene hud={false} />
      {/* darker towards the bottom so the headline reads at any size */}
      <AbsoluteFill style={{ background: "linear-gradient(180deg, rgba(5,6,12,0) 30%, rgba(5,6,12,.82) 100%)" }} />
      {/* neon glow behind the headline, like the sunset in the video */}
      <AbsoluteFill
        style={{
          background:
            "radial-gradient(900px 380px at 50% 58%, rgba(214,72,160,.42), rgba(214,72,160,0) 70%), radial-gradient(1300px 520px at 50% 64%, rgba(255,160,90,.16), rgba(255,160,90,0) 70%)",
        }}
      />
      <OverlayStage zoom={2.35} top={64} song={SONG_A} accent={SONG_A.accent} active={SONG_A.lines[0]} fill={0.56} next={SONG_A.lines[1]} />
      <div style={{ position: "absolute", left: 0, right: 0, top: 455, display: "flex", flexDirection: "column", alignItems: "center", gap: 36 }}>
        <div style={{ fontSize: 172, fontWeight: 800, lineHeight: 1, letterSpacing: -4, textAlign: "center", textShadow: "0 8px 36px rgba(0,0,0,.65)" }}>
          Lyrics over
          <br />
          <span style={{ color: PROMO.accent }}>your games</span>
        </div>
        <div
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 18,
            padding: "14px 30px 14px 18px",
            borderRadius: 999,
            background: "rgba(10,12,20,.82)",
            border: `2px solid ${PROMO.line}`,
            fontSize: 46,
            fontWeight: 700,
          }}
        >
          <Img src={staticFile("lyrichub-icon.svg")} style={{ width: 62, height: 62 }} />
          LyricHub
          <span style={{ color: PROMO.muted, fontWeight: 600 }}>· free for Windows</span>
        </div>
      </div>
    </AbsoluteFill>
  </FontGate>
);
