// Recreation of LyricHub's edit mode (src/lib/overlay/EditFrame.svelte, editMode.ts):
// dimmed screen, dashed amber frame, corner/side handles, nine snap markers, toolbar.
import React from "react";
import { AbsoluteFill } from "remotion";

const AMBER = "#ffd36e";
const MARGIN = 24;

export const SnapMarkers: React.FC<{ w: number; h: number; near?: number; hit?: number; allNear?: boolean }> = ({ w, h, near = -1, hit = -1, allNear = false }) => {
  const xs = [MARGIN, w / 2, w - MARGIN];
  const ys = [MARGIN, h / 2, h - MARGIN];
  return (
    <>
      {ys.flatMap((y, r) =>
        xs.map((x, c) => {
          const i = r * 3 + c;
          const isHit = i === hit;
          const isNear = i === near || allNear;
          const col = isHit ? AMBER : "rgba(255,255,255,.55)";
          return (
            <div
              key={i}
              style={{
                position: "absolute",
                left: x - 7,
                top: y - 7,
                width: 14,
                height: 14,
                transform: `scale(${isHit ? 1.5 : 1})`,
                filter: isNear || isHit ? "drop-shadow(0 0 3px rgba(255,211,110,.8))" : "none",
              }}
            >
              <div style={{ position: "absolute", left: 6, top: 0, width: 2, height: 14, background: col }} />
              <div style={{ position: "absolute", left: 0, top: 6, width: 14, height: 2, background: col }} />
            </div>
          );
        }),
      )}
    </>
  );
};

/** Frame and handles drawn around a box at (x, y, w, h). */
export const EditFrame: React.FC<{ x: number; y: number; w: number; h: number }> = ({ x, y, w, h }) => {
  const corner = (cx: number, cy: number, k: string) => (
    <div key={k} style={{ position: "absolute", left: cx - 7, top: cy - 7, width: 14, height: 14, borderRadius: 3, background: AMBER, border: "2px solid #1a1400", boxSizing: "border-box" }} />
  );
  const side = (cx: number, cy: number, k: string) => (
    <div key={k} style={{ position: "absolute", left: cx - 5, top: cy - 13, width: 10, height: 26, borderRadius: 3, background: AMBER, border: "2px solid #1a1400", boxSizing: "border-box" }} />
  );
  return (
    <>
      <div style={{ position: "absolute", left: x - 4, top: y - 4, width: w + 8, height: h + 8, outline: "2px dashed rgba(255,211,110,.95)", borderRadius: 4 }} />
      {corner(x - 12 + 7, y - 12 + 7, "tl")}
      {corner(x + w + 12 - 7, y - 12 + 7, "tr")}
      {corner(x - 12 + 7, y + h + 12 - 7, "bl")}
      {corner(x + w + 12 - 7, y + h + 12 - 7, "br")}
      {side(x - 12 + 5, y + h / 2, "l")}
      {side(x + w + 12 - 5, y + h / 2, "r")}
    </>
  );
};

export const EditToolbar: React.FC<{ readout: string; y: number; x: number; multiMonitor?: boolean; savePressed?: boolean; zoom?: number }> = ({
  readout,
  y,
  x,
  multiMonitor = true,
  savePressed = false,
  zoom = 1,
}) => (
  <div
    style={{
      position: "absolute",
      left: x,
      top: y,
      transform: `translateX(-50%) scale(${zoom})`,
      transformOrigin: "top center",
      background: "rgba(18,20,26,.94)",
      border: "1px solid rgba(255,255,255,.12)",
      borderRadius: 12,
      boxShadow: "0 8px 30px rgba(0,0,0,.45)",
      color: "#e8eaf0",
      fontFamily: '"Segoe UI Variable", "Segoe UI", sans-serif',
      fontSize: 13,
      padding: "12px 16px",
      display: "grid",
      gap: 8,
      minWidth: 560,
    }}
  >
    <div style={{ display: "flex", justifyContent: "space-between", gap: 20 }}>
      <span style={{ fontWeight: 600 }}>Position and size</span>
      <span style={{ color: "#9aa1b2" }}>{readout}</span>
    </div>
    <div style={{ color: "#9aa1b2" }}>Drag to move · Corners scale · Sides change the width · Markers snap</div>
    <div style={{ display: "flex", gap: 8 }}>
      <span style={{ padding: "6px 12px", borderRadius: 8, background: AMBER, color: "#1a1400", fontWeight: 600, outline: savePressed ? "3px solid rgba(255,211,110,.5)" : "none" }}>
        Save <span style={{ marginLeft: 6, padding: "1px 6px", borderRadius: 4, border: "1px solid #1a1400", fontSize: 11 }}>Enter</span>
      </span>
      <span style={{ padding: "6px 12px", borderRadius: 8, background: "#262b36", border: "1px solid #2a2f3a" }}>
        Cancel <span style={{ marginLeft: 6, padding: "1px 6px", borderRadius: 4, border: "1px solid #9aa1b2", fontSize: 11 }}>Esc</span>
      </span>
      {multiMonitor ? <span style={{ padding: "6px 12px", borderRadius: 8, background: "#262b36", border: "1px solid #2a2f3a" }}>Next monitor · Monitor 2</span> : null}
    </div>
  </div>
);

export const EditDim: React.FC = () => <AbsoluteFill style={{ background: "rgba(6,8,12,.35)" }} />;
