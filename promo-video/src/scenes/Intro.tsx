// Scenes 1–3: hook, what it is, setup.
import React from "react";
import { AbsoluteFill, Img, interpolate, staticFile } from "remotion";
import { SONG_A } from "../data/demo";
import { PROMO } from "../lib/theme";
import { useScene } from "../lib/time";
import { Taskbar, WindowFrame } from "../ui/Desktop";
import { GameScene } from "../ui/GameScene";
import { BigTitle, Callout, Panel, SceneShell, sceneById } from "../ui/Kit";
import { lyricAt, schedule } from "../ui/lyrics";
import { OverlayStage } from "../ui/Stage";

const A_LINES = schedule(SONG_A.lines, 0.8, [3.4, 3.0, 3.2, 3.4]);

export const S01Hook: React.FC = () => {
  const { sec, p } = useScene();
  const on = sceneById("s01-hook").onScreen;
  const ly = lyricAt(A_LINES, sec - 1.2);
  const title = p(0.5, 0.68);
  return (
    <SceneShell id="s01-hook" chip={false} bg={<GameScene />}>
      <OverlayStage o={p(0.1, 0.22)} song={SONG_A} accent={SONG_A.accent} {...ly} />
      <AbsoluteFill style={{ background: `rgba(0,0,0,${0.45 * title})` }} />
      <div style={{ position: "absolute", left: 0, right: 0, top: 380, display: "flex", flexDirection: "column", alignItems: "center", gap: 18, opacity: title, transform: `scale(${0.94 + 0.06 * title})` }}>
        <Img src={staticFile("lyrichub-icon.svg")} style={{ width: 150, height: 150 }} />
        <BigTitle text={on[0]} size={104} />
      </div>
    </SceneShell>
  );
};

export const S02What: React.FC = () => {
  const { sec, p, window } = useScene();
  const on = sceneById("s02-what").onScreen;
  const ly = lyricAt(A_LINES, sec);
  const box = (label: string, sub: string, o: number) => (
    <div style={{ opacity: o, transform: `translateY(${(1 - o) * 12}px)`, padding: "22px 28px", borderRadius: 16, background: PROMO.panel, border: `1px solid ${PROMO.line}`, minWidth: 300, textAlign: "center" }}>
      <div style={{ fontSize: 32, fontWeight: 700 }}>{label}</div>
      <div style={{ fontSize: 22, color: PROMO.muted, marginTop: 6 }}>{sub}</div>
    </div>
  );
  const arrow = (o: number) => <div style={{ fontSize: 46, color: PROMO.accent, opacity: o }}>→</div>;
  return (
    <SceneShell id="s02-what" bg={<GameScene dim={0.35} />}>
      <OverlayStage zoom={1.7} top={130} song={SONG_A} accent={SONG_A.accent} {...ly} />
      <div style={{ position: "absolute", top: 470, left: 0, right: 0, display: "flex", justifyContent: "center" }}>
        <Callout text={on[0]} o={window(0.21, 0.62)} size={40} />
      </div>
      <div style={{ position: "absolute", top: 470, left: 0, right: 0, display: "flex", justifyContent: "center", gap: 24 }}>
        <Callout text={on[1]} o={window(0.625, 0.8)} size={36} />
        <Callout text={on[2]} o={window(0.65, 0.8)} size={36} />
        <Callout text={on[3]} o={window(0.675, 0.8)} size={36} />
      </div>
      <div style={{ position: "absolute", top: 450, left: 0, right: 0, display: "flex", justifyContent: "center", alignItems: "center", gap: 28 }}>
        {box("Spotify", "desktop app", window(0.8, 1.02))}
        {arrow(window(0.82, 1.02))}
        {box("Windows", "media session", window(0.83, 1.02))}
        {arrow(window(0.85, 1.02))}
        {box("LyricHub", "overlay", window(0.86, 1.02))}
      </div>
      <div style={{ position: "absolute", top: 650, left: 0, right: 0, display: "flex", justifyContent: "center" }}>
        <Callout text={on[4]} o={window(0.88, 1.02)} size={34} />
      </div>
    </SceneShell>
  );
};

export const S03Setup: React.FC = () => {
  const { sec, p, window, since } = useScene();
  const on = sceneById("s03-setup").onScreen;
  const install = window(0.02, 0.4);
  const prog = p(0.08, 0.34);
  const warn = window(0.39, 0.6);
  const desk = window(0.6, 0.76);
  const game = p(0.725, 0.755);
  const tGame = since(0.77);
  const ly = lyricAt(schedule(SONG_A.lines, 0.8, 3.2), tGame);
  const idle = tGame < 0.9;
  return (
    <SceneShell id="s03-setup" bg={<AbsoluteFill style={{ background: "radial-gradient(1200px 700px at 50% 30%, #1a2140, #0b0d14)" }} />}>
      {/* installer */}
      <div style={{ position: "absolute", left: 160, top: 210, opacity: install }}>
        <WindowFrame title="LyricHub Setup" width={760} height={420}>
          <div style={{ padding: 36, color: "#e8eaf0", fontFamily: '"Segoe UI Variable", "Segoe UI", sans-serif' }}>
            <div style={{ fontSize: 30, fontWeight: 600 }}>Installing LyricHub 1.0.0</div>
            <div style={{ fontSize: 19, color: "#9aa1b2", marginTop: 10 }}>Installing for the current user</div>
            <div style={{ marginTop: 70, height: 16, borderRadius: 8, background: "#2a2f3a" }}>
              <div style={{ width: `${prog * 100}%`, height: "100%", borderRadius: 8, background: PROMO.accent }} />
            </div>
            <div style={{ fontSize: 17, color: "#9aa1b2", marginTop: 14 }}>{prog < 1 ? "Extracting files…" : "Completed"}</div>
          </div>
        </WindowFrame>
      </div>
      <div style={{ position: "absolute", left: 1000, top: 250, display: "grid", gap: 22, opacity: install }}>
        <Callout text={on[0]} o={window(0.12, 0.4)} size={36} />
        <Callout text={on[1]} o={window(0.2, 0.4)} size={36} />
        <Callout text={on[2]} o={window(0.29, 0.4)} size={36} />
      </div>
      {/* SmartScreen note */}
      <div style={{ position: "absolute", left: 0, right: 0, top: 300, display: "flex", justifyContent: "center", opacity: warn }}>
        <Panel style={{ maxWidth: 1100, textAlign: "center" }}>
          <div style={{ fontSize: 40, fontWeight: 700 }}>{on[3]}</div>
          <div style={{ fontSize: 30, color: PROMO.muted, marginTop: 16 }}>
            Click <b style={{ color: PROMO.text }}>More info</b> → <b style={{ color: PROMO.text }}>Run anyway</b>
          </div>
        </Panel>
      </div>
      {/* tray */}
      <AbsoluteFill style={{ opacity: desk }}>
        <AbsoluteFill style={{ background: "linear-gradient(160deg, #1d2a52, #2b1b44)" }} />
        <Taskbar highlight={window(0.62, 0.76)} y={800} />
        <div style={{ position: "absolute", right: 120, top: 640 }}>
          <Callout text={on[4]} o={window(0.62, 0.76)} size={38} />
        </div>
      </AbsoluteFill>
      {/* game with overlay */}
      <AbsoluteFill style={{ opacity: game }}>
        <GameScene />
        <OverlayStage
          zoom={1.3}
          top={40}
          idle={idle}
          song={idle ? null : SONG_A}
          accent={SONG_A.accent}
          {...(idle ? { active: "", state: "empty" as const, next: "" } : ly)}
        />
        <div style={{ position: "absolute", left: 0, right: 0, top: 560, display: "flex", justifyContent: "center" }}>
          <Callout text={on[5]} o={p(0.74, 0.77)} size={40} />
        </div>
      </AbsoluteFill>
    </SceneShell>
  );
};

export const fadeBetween = (frame: number, a: number, b: number) => interpolate(frame, [a, b], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
