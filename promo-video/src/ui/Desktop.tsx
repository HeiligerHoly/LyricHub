// Windows-style taskbar with LyricHub's tray icon and its right-click menu (labels from
// src-tauri/src/tray.rs, English, default shortcuts). Generic shapes only; no OS logos.
import React from "react";
import { Img, staticFile } from "remotion";
import { PROMO } from "../lib/theme";

export const TASKBAR_H = 56;

export const Taskbar: React.FC<{ grey?: boolean; highlight?: number; y?: number }> = ({ grey = false, highlight = 0, y }) => (
  <div
    style={{
      position: "absolute",
      left: 0,
      right: 0,
      top: y,
      bottom: y === undefined ? 0 : undefined,
      height: TASKBAR_H,
      background: "rgba(28,30,38,.92)",
      borderTop: "1px solid rgba(255,255,255,.08)",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
    }}
  >
    <div style={{ display: "flex", gap: 14 }}>
      {["#4f8cff", "#2a2f3d", "#2a2f3d", "#2a2f3d", "#2a2f3d"].map((c, i) => (
        <div key={i} style={{ width: 34, height: 34, borderRadius: 8, background: c, opacity: i === 0 ? 0.8 : 1 }} />
      ))}
    </div>
    <div style={{ position: "absolute", right: 24, display: "flex", alignItems: "center", gap: 16, color: "#dfe3ec", fontSize: 18, fontFamily: PROMO.font }}>
      <span style={{ fontSize: 16, opacity: 0.8 }}>⌃</span>
      <div
        style={{
          width: 40,
          height: 40,
          borderRadius: 8,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: highlight ? `rgba(255,211,110,${0.25 * highlight})` : "transparent",
          outline: highlight ? `2px solid rgba(255,211,110,${0.8 * highlight})` : "none",
        }}
      >
        <Img src={staticFile("lyrichub-icon.svg")} style={{ width: 24, height: 24, filter: grey ? "grayscale(1) brightness(.8)" : "none" }} />
      </div>
      <div style={{ width: 20, height: 14, borderRadius: 3, border: "2px solid #dfe3ec", opacity: 0.8 }} />
      <div style={{ textAlign: "right", lineHeight: 1.15, fontSize: 15 }}>
        <div>21:40</div>
        <div style={{ opacity: 0.75 }}>26.10.2026</div>
      </div>
    </div>
  </div>
);

type Item = { label: string; check?: boolean; sub?: boolean; sep?: boolean; hot?: boolean };

export const TRAY_ITEMS: Item[] = [
  { label: "Show overlay (Ctrl+Alt+L)", check: true },
  { label: "Change position and size (Ctrl+Alt+E)" },
  { label: "", sep: true },
  { label: "Song: lyrics earlier, −100 ms (Ctrl+Alt+,)" },
  { label: "Song: lyrics later, +100 ms (Ctrl+Alt+.)" },
  { label: "Layout (Ctrl+Alt+K)", sub: true },
  { label: "", sep: true },
  { label: "Settings …" },
  { label: "Run overlay self-test" },
  { label: "", sep: true },
  { label: "Quit" },
];

export const TrayMenu: React.FC<{ hot?: number; o?: number }> = ({ hot = -1, o = 1 }) => (
  <div
    style={{
      width: 470,
      padding: "6px 0",
      borderRadius: 10,
      background: "#2b2d35",
      border: "1px solid rgba(255,255,255,.1)",
      boxShadow: "0 16px 40px rgba(0,0,0,.5)",
      fontFamily: '"Segoe UI Variable", "Segoe UI", sans-serif',
      fontSize: 19,
      color: "#eceef4",
      opacity: o,
    }}
  >
    {TRAY_ITEMS.map((it, i) =>
      it.sep ? (
        <div key={i} style={{ height: 1, background: "rgba(255,255,255,.1)", margin: "6px 12px" }} />
      ) : (
        <div
          key={i}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "8px 16px",
            margin: "0 6px",
            borderRadius: 6,
            background: i === hot ? "rgba(255,255,255,.1)" : "transparent",
          }}
        >
          <span style={{ width: 18, color: PROMO.accent }}>{it.check ? "✓" : ""}</span>
          <span style={{ flex: 1 }}>{it.label}</span>
          {it.sub ? <span style={{ opacity: 0.7 }}>›</span> : null}
        </div>
      ),
    )}
  </div>
);

/** A plain desktop-app window frame used for the installer and windowed-game shots. */
export const WindowFrame: React.FC<{ title: string; width: number; height: number; children: React.ReactNode; style?: React.CSSProperties }> = ({
  title,
  width,
  height,
  children,
  style,
}) => (
  <div
    style={{
      width,
      height,
      borderRadius: 10,
      overflow: "hidden",
      background: "#1c1f27",
      border: "1px solid rgba(255,255,255,.12)",
      boxShadow: "0 24px 70px rgba(0,0,0,.55)",
      display: "flex",
      flexDirection: "column",
      ...style,
    }}
  >
    <div
      style={{
        height: 38,
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "0 14px",
        background: "#15171d",
        color: "#dfe3ec",
        fontSize: 15,
        fontFamily: '"Segoe UI Variable", "Segoe UI", sans-serif',
      }}
    >
      <Img src={staticFile("lyrichub-icon.svg")} style={{ width: 18, height: 18 }} />
      <span style={{ flex: 1 }}>{title}</span>
      <span style={{ opacity: 0.6, letterSpacing: 18 }}>– ▢ ✕</span>
    </div>
    <div style={{ flex: 1, position: "relative" }}>{children}</div>
  </div>
);
