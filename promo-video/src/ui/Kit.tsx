// Promo graphics: scene shell, title chip, callouts, key caps, mouse cursor, panels.
import React from "react";
import { AbsoluteFill, Img, staticFile } from "remotion";
import { SCENES } from "../data/scenes";
import { FontGate } from "../lib/fonts";
import { PROMO } from "../lib/theme";
import { Captions } from "./Captions";

export const sceneById = (id: string) => {
  const s = SCENES.find((x) => x.id === id);
  if (!s) throw new Error(`unknown scene ${id}`);
  return s;
};

/** Every scene: fonts loaded, background, the scene content, captions on top. */
export const SceneShell: React.FC<{ id: string; children: React.ReactNode; bg?: React.ReactNode; chip?: boolean }> = ({
  id,
  children,
  bg,
  chip = true,
}) => {
  const scene = sceneById(id);
  return (
    <FontGate>
      <AbsoluteFill style={{ background: PROMO.bg, fontFamily: PROMO.font, color: PROMO.text, overflow: "hidden" }}>
        {bg}
        {children}
        {chip ? <TitleChip text={scene.title} /> : null}
        <Captions text={scene.vo} />
      </AbsoluteFill>
    </FontGate>
  );
};

export const TitleChip: React.FC<{ text: string }> = ({ text }) => (
  <div
    style={{
      position: "absolute",
      left: 56,
      top: 44,
      display: "flex",
      alignItems: "center",
      gap: 12,
      padding: "8px 18px 8px 10px",
      borderRadius: 999,
      background: "rgba(10,12,20,.72)",
      border: `1px solid ${PROMO.line}`,
      fontSize: 24,
      fontWeight: 600,
      color: PROMO.text,
    }}
  >
    <Img src={staticFile("lyrichub-icon.svg")} style={{ width: 30, height: 30 }} />
    {text}
  </div>
);

export const Callout: React.FC<{ text: string; o?: number; x?: number; y?: number; size?: number; style?: React.CSSProperties }> = ({
  text,
  o = 1,
  x,
  y,
  size = 30,
  style,
}) => (
  <div
    style={{
      position: x !== undefined || y !== undefined ? "absolute" : "relative",
      left: x,
      top: y,
      display: "inline-flex",
      alignItems: "center",
      gap: 12,
      padding: "10px 20px",
      borderRadius: 14,
      background: "rgba(12,14,22,.82)",
      border: "1px solid rgba(255,211,110,.35)",
      color: PROMO.text,
      fontSize: size,
      fontWeight: 600,
      whiteSpace: "nowrap",
      opacity: o,
      transform: `translateY(${(1 - o) * 10}px)`,
      ...style,
    }}
  >
    <span style={{ width: 10, height: 10, borderRadius: "50%", background: PROMO.accent, flexShrink: 0 }} />
    {text}
  </div>
);

/** One key cap; `pressed` 0..1 pushes it down and lights it amber. */
export const KeyCap: React.FC<{ label: string; pressed?: number; size?: number }> = ({ label, pressed = 0, size = 1 }) => {
  const h = 64 * size;
  const wide = label.length > 1;
  return (
    <div
      style={{
        minWidth: wide ? 0 : h,
        height: h,
        padding: wide ? `0 ${20 * size}px` : 0,
        boxSizing: "border-box",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: 12 * size,
        background: pressed > 0.5 ? "#3a3220" : "#1d2130",
        border: `2px solid ${pressed > 0.5 ? PROMO.accent : "#39405a"}`,
        boxShadow: `0 ${(1 - pressed) * 6 * size}px 0 ${pressed > 0.5 ? "#7a6224" : "#0d0f17"}`,
        transform: `translateY(${pressed * 5 * size}px)`,
        color: pressed > 0.5 ? PROMO.accent : PROMO.text,
        fontSize: (wide ? 26 : 32) * size,
        fontWeight: 700,
        fontFamily: PROMO.font,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </div>
  );
};

/** A key combination like "Ctrl+Alt+Shift+," shown as caps joined by plus signs. */
export const KeyCombo: React.FC<{ combo: string; pressed?: number; size?: number }> = ({ combo, pressed = 0, size = 1 }) => {
  const keys = combo.split("+").map((k) => k.trim());
  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: 10 * size }}>
      {keys.map((k, i) => (
        <React.Fragment key={i}>
          {i > 0 ? <span style={{ color: PROMO.muted, fontSize: 26 * size, fontWeight: 600 }}>+</span> : null}
          <KeyCap label={k === "," ? "," : k === "." ? "." : k} pressed={pressed} size={size} />
        </React.Fragment>
      ))}
    </div>
  );
};

export const Cursor: React.FC<{ x: number; y: number; down?: boolean; kind?: "arrow" | "move" | "resize" }> = ({ x, y, down = false, kind = "arrow" }) => (
  <svg width={36} height={36} viewBox="0 0 36 36" style={{ position: "absolute", left: x - 4, top: y - 2, transform: `scale(${down ? 0.9 : 1})` }}>
    {kind === "move" ? (
      <g fill="#fff" stroke="#000" strokeWidth={1.5}>
        <path d="M18 2 L23 8 H20 V16 H28 V13 L34 18 L28 23 V20 H20 V28 H23 L18 34 L13 28 H16 V20 H8 V23 L2 18 L8 13 V16 H16 V8 H13 Z" />
      </g>
    ) : kind === "resize" ? (
      <g fill="#fff" stroke="#000" strokeWidth={1.5}>
        <path d="M4 4 H16 L12 8 L28 24 L32 20 V32 H20 L24 28 L8 12 L4 16 Z" />
      </g>
    ) : (
      <path d="M4 2 L4 28 L11 21 L16 32 L21 30 L16 19 L26 19 Z" fill="#fff" stroke="#000" strokeWidth={1.8} strokeLinejoin="round" />
    )}
  </svg>
);

export const Panel: React.FC<{ children: React.ReactNode; style?: React.CSSProperties }> = ({ children, style }) => (
  <div
    style={{
      background: "rgba(16,19,30,.92)",
      border: `1px solid ${PROMO.line}`,
      borderRadius: 20,
      padding: "28px 32px",
      boxShadow: "0 20px 60px rgba(0,0,0,.45)",
      ...style,
    }}
  >
    {children}
  </div>
);

export const BigTitle: React.FC<{ text: string; o?: number; size?: number; style?: React.CSSProperties }> = ({ text, o = 1, size = 76, style }) => (
  <div style={{ fontSize: size, fontWeight: 800, letterSpacing: -1, color: PROMO.text, opacity: o, transform: `translateY(${(1 - o) * 16}px)`, ...style }}>
    {text}
  </div>
);
