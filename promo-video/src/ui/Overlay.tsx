// Faithful recreation of LyricHub's overlay plate (src/lib/overlay/Overlay.svelte, layouts/*,
// TrackHeader.svelte, KaraokeLine.svelte, ActiveRow.svelte). Values are design px at scale
// s = 1 on a 1080p screen. Zoom for close-ups with a CSS transform around it, so every value
// (including the fixed 18 px fill edge) keeps its real proportion.
import React from "react";
import type { DemoSong } from "../data/demo";
import { OVERLAY_FONT, type OverlayVars, type Rgb, type ThemeId, overlayVars, themeLook } from "../lib/theme";

export type Layout = "karaoke" | "bar" | "stack" | "minimal";

export type OverlayProps = {
  layout?: Layout;
  theme?: ThemeId;
  plateOpacity?: number;
  /** Accent in use (from the cover or fixed). */
  accent?: Rgb;
  song?: DemoSong | null;
  active?: string;
  /** Karaoke fill of the active line, 0..1. */
  fill?: number;
  next?: string;
  next2?: string;
  prev?: string;
  /** What the active row shows. */
  state?: "lines" | "loading" | "hint" | "empty";
  hint?: string;
  /** Pill toast; replaces the title row for 1.5 s in the app. */
  toast?: string | null;
  offlineDot?: boolean;
  /** Settings "Scale" (0.5–3). */
  s?: number;
  /** Plate width in design px (default: 35 % of 1920). */
  width?: number;
  /** Line-change animation progress 0..1 (220 ms, slide up 12 px, opacity 0.55 → 1). */
  lineIn?: number;
  /** Next-line fade-in progress 0..1. */
  nextIn?: number;
  /** Track-change: content opacity (fades to 0.15 at the first sign of a new song). */
  contentOpacity?: number;
  /** New header fade-in progress 0..1 (300 ms; cover scales 0.96 → 1). */
  headerIn?: number;
  /** Pause dimming / fade-out (box opacity). */
  boxOpacity?: number;
  /** User font override (Appearance → Font). */
  fontFamily?: string;
  /** Loading-dot animation time in seconds. */
  time?: number;
  /** Text alignment within the plate. */
  align?: "start" | "center" | "end";
  /** Idle hint "LyricHub – waiting for Spotify" (3 s after showing with nothing playing). */
  idle?: boolean;
};

export const DEFAULT_WIDTH = 672; // 35 % of 1920

const Cover: React.FC<{ song: DemoSong; size: number; radius: number; scale?: number }> = ({ song, size, radius, scale = 1 }) => (
  <div
    style={{
      width: size,
      height: size,
      borderRadius: radius,
      flexShrink: 0,
      background: `linear-gradient(135deg, ${song.cover[0]}, ${song.cover[1]})`,
      position: "relative",
      overflow: "hidden",
      transform: `scale(${scale})`,
    }}
  >
    <div
      style={{
        position: "absolute",
        width: size * 0.62,
        height: size * 0.62,
        left: size * 0.3,
        top: size * 0.08,
        borderRadius: "50%",
        background: song.cover[2],
        opacity: 0.85,
      }}
    />
  </div>
);

export const KaraokeLine: React.FC<{
  text: string;
  fill: number;
  v: OverlayVars;
  size: number;
  weight: number;
  font: string;
}> = ({ text, fill, v, size, weight, font }) => {
  const r = Math.max(0, Math.min(1, fill));
  const stroke: React.CSSProperties = v.stroke
    ? { WebkitTextStroke: `${v.stroke}px ${v.strokeColor}`, paintOrder: "stroke fill" }
    : {};
  return (
    <div
      style={{
        position: "relative",
        width: "fit-content",
        maxWidth: "100%",
        fontSize: size,
        fontWeight: weight,
        lineHeight: 1.25,
        whiteSpace: "nowrap",
        fontFamily: font,
      }}
    >
      <span style={{ color: v.base, textShadow: v.textShadow, ...stroke }}>{text}</span>
      <span
        aria-hidden
        style={{
          position: "absolute",
          top: 0,
          bottom: 0,
          left: 0,
          right: -18,
          overflow: "hidden",
          WebkitMaskImage: "linear-gradient(90deg, #000 calc(100% - 18px), transparent)",
          maskImage: "linear-gradient(90deg, #000 calc(100% - 18px), transparent)",
          transform: `translateX(${(r - 1) * 100}%)`,
        }}
      >
        <span
          style={{
            display: "block",
            paddingRight: 18,
            color: v.accent,
            textShadow: v.fillShadow,
            transform: `translateX(${(1 - r) * 100}%)`,
            ...stroke,
          }}
        >
          {text}
        </span>
      </span>
    </div>
  );
};

const Dots: React.FC<{ v: OverlayVars; s: number; time: number }> = ({ v, s, time }) => (
  <div style={{ display: "flex", gap: 6 * s, alignItems: "center", height: 35 * s }}>
    {[0, 0.15, 0.3].map((d) => {
      const ph = ((time - d) % 0.9 + 0.9) % 0.9 / 0.9;
      const o = 0.25 + 0.75 * (0.5 - 0.5 * Math.cos(ph * Math.PI * 2));
      return <div key={d} style={{ width: 7 * s, height: 7 * s, borderRadius: "50%", background: v.meta, opacity: o }} />;
    })}
  </div>
);

export const Toast: React.FC<{ text: string; v: OverlayVars; s: number; size?: number; lh?: number }> = ({ text, v, s, size = 13, lh = 22 }) => (
  <div
    style={{
      display: "inline-flex",
      alignItems: "center",
      background: v.accentSoft,
      color: v.accent,
      padding: `0 ${10 * s}px`,
      borderRadius: 999,
      fontSize: size * s,
      fontWeight: 600,
      lineHeight: `${lh * s}px`,
      height: lh * s,
      whiteSpace: "nowrap",
    }}
  >
    {text}
  </div>
);

export const Overlay: React.FC<OverlayProps> = ({
  layout = "karaoke",
  theme = "glass_dark",
  plateOpacity = 0.55,
  accent,
  song = null,
  active = "",
  fill = 0,
  next = "",
  next2 = "",
  prev = "",
  state = "lines",
  hint = "",
  toast = null,
  offlineDot = false,
  s = 1,
  width = DEFAULT_WIDTH,
  lineIn = 1,
  nextIn = 1,
  contentOpacity = 1,
  headerIn = 1,
  boxOpacity = 1,
  fontFamily,
  time = 0,
  align = "center",
  idle = false,
}) => {
  const look = themeLook(theme, plateOpacity);
  const minimal = layout === "minimal";
  const acc: Rgb = look.accentAllowed ? accent ?? look.fallbackAccent : look.fallbackAccent;
  const v = overlayVars(look, acc, s, minimal);
  const font = fontFamily ? `"${fontFamily}", ${OVERLAY_FONT}` : OVERLAY_FONT;
  const justify = align === "start" ? "flex-start" : align === "end" ? "flex-end" : "center";
  const stroke: React.CSSProperties = v.stroke ? { WebkitTextStroke: `${v.stroke}px ${v.strokeColor}`, paintOrder: "stroke fill" } : {};
  const lineInStyle: React.CSSProperties = {
    opacity: 0.55 + 0.45 * lineIn,
    transform: `translateY(${(1 - lineIn) * 12 * s}px)`,
  };

  const header = idle ? (
    <div style={{ display: "flex", alignItems: "center", gap: 8 * s, height: 24 * s, justifyContent: justify, opacity: 0.75 }}>
      <span style={{ fontSize: 13 * s, fontWeight: 600, color: v.text, textShadow: v.textShadow }}>LyricHub</span>
      <span style={{ fontSize: 13 * s, color: v.meta }}>–</span>
      <span style={{ fontSize: 13 * s, color: v.meta, textShadow: v.textShadow }}>waiting for Spotify</span>
    </div>
  ) : song ? (
    <div style={{ display: "flex", alignItems: "center", gap: 8 * s, height: 24 * s, justifyContent: justify, opacity: headerIn }}>
      {toast ? (
        <Toast text={toast} v={v} s={s} />
      ) : (
        <>
          <Cover song={song} size={24 * s} radius={5 * s} scale={0.96 + 0.04 * headerIn} />
          <span style={{ fontSize: 13 * s, fontWeight: 600, color: v.text, textShadow: v.textShadow, whiteSpace: "nowrap", ...stroke }}>{song.title}</span>
          <span style={{ fontSize: 13 * s, color: v.meta, textShadow: v.textShadow, ...stroke }}>–</span>
          <span style={{ fontSize: 13 * s, color: v.meta, textShadow: v.textShadow, whiteSpace: "nowrap", ...stroke }}>{song.artist}</span>
          {offlineDot ? (
            <span style={{ width: 7 * s, height: 7 * s, borderRadius: "50%", border: `1.5px solid ${v.meta}`, marginLeft: 2 * s }} />
          ) : null}
        </>
      )}
    </div>
  ) : toast ? (
    <div style={{ display: "flex", justifyContent: justify, height: 24 * s, alignItems: "center" }}>
      <Toast text={toast} v={v} s={s} />
    </div>
  ) : null;

  const activeRow = (
    <div style={{ height: 35 * s, display: "flex", alignItems: "center", justifyContent: justify }}>
      {state === "loading" ? (
        <Dots v={v} s={s} time={time} />
      ) : state === "hint" ? (
        <span style={{ fontSize: 16 * s, color: v.meta, textShadow: v.textShadow, ...stroke }}>{hint}</span>
      ) : state === "empty" ? null : active === "♪" ? (
        <span style={{ fontSize: 16 * s, color: v.meta }}>♪</span>
      ) : (
        <div style={lineInStyle}>
          <KaraokeLine text={active} fill={fill} v={v} size={28 * s} weight={650} font={font} />
        </div>
      )}
    </div>
  );

  const sideRow = (text: string, extraOpacity = 1, fade = 1) => (
    <div
      style={{
        height: 22.5 * s,
        display: "flex",
        alignItems: "center",
        justifyContent: justify,
        fontSize: 18 * s,
        fontWeight: 500,
        color: v.next,
        textShadow: v.textShadow,
        whiteSpace: "nowrap",
        opacity: extraOpacity * fade,
        ...stroke,
      }}
    >
      {text}
    </div>
  );

  let body: React.ReactNode;
  if (layout === "bar") {
    body = (
      <div style={{ display: "flex", alignItems: "center", gap: 10 * s, height: 32 * s, justifyContent: justify, whiteSpace: "nowrap" }}>
        {song ? <Cover song={song} size={32 * s} radius={6 * s} /> : null}
        {song ? (
          <span style={{ fontSize: 18 * s, fontWeight: 650, color: v.text, textShadow: v.textShadow, maxWidth: active ? "28%" : "45%", overflow: "hidden", textOverflow: "ellipsis", flexShrink: 0, ...stroke }}>{song.title}</span>
        ) : null}
        {song ? <span style={{ fontSize: 18 * s, color: v.meta, flexShrink: 0 }}>–</span> : null}
        {song ? (
          <span style={{ fontSize: 18 * s, fontWeight: 450, color: v.meta, textShadow: v.textShadow, maxWidth: active ? "18%" : "35%", overflow: "hidden", textOverflow: "ellipsis", flexShrink: 0, ...stroke }}>{song.artist}</span>
        ) : null}
        {active ? <span style={{ width: 1.5, height: 18 * s, background: v.meta, opacity: 0.6, flexShrink: 0 }} /> : null}
        {toast ? (
          <Toast text={toast} v={v} s={s} size={15} lh={26} />
        ) : active ? (
          <span style={{ fontSize: 18 * s, fontWeight: 600, color: v.text, textShadow: v.textShadow, overflow: "hidden", textOverflow: "ellipsis", minWidth: 0, flex: "1 1 auto", ...stroke }}>{active}</span>
        ) : null}
      </div>
    );
  } else if (layout === "stack") {
    body = (
      <div style={{ display: "grid", gap: 4 * s, opacity: contentOpacity }}>
        {header}
        <div style={{ display: "grid", gap: 2 * s }}>
          {sideRow(prev, 0.7)}
          {activeRow}
          {sideRow(next, 1, nextIn)}
          {sideRow(next2, 1, nextIn)}
        </div>
      </div>
    );
  } else if (minimal) {
    const showNote = !active && !toast && song;
    body = (
      <div style={{ display: "grid", gap: 4 * s }}>
        {toast ? (
          <div style={{ display: "flex", justifyContent: justify, height: 20 * s, alignItems: "center" }}>
            <Toast text={toast} v={v} s={s} lh={20} />
          </div>
        ) : showNote ? (
          <div style={{ display: "flex", justifyContent: justify, height: 20 * s, alignItems: "center", fontSize: 14 * s, fontWeight: 600, color: v.meta, textShadow: v.textShadow, ...stroke }}>
            {song!.title} – {song!.artist}
          </div>
        ) : null}
        {active || state !== "lines" ? activeRow : null}
      </div>
    );
  } else {
    body = (
      <div style={{ display: "grid", gap: 4 * s, opacity: contentOpacity }}>
        {header}
        {activeRow}
        {sideRow(next, 1, nextIn)}
      </div>
    );
  }

  return (
    <div
      style={{
        width,
        boxSizing: "border-box",
        padding: `${8 * s}px ${14 * s}px`,
        borderRadius: v.radius,
        background: v.plateBg,
        fontFamily: font,
        color: v.text,
        textRendering: "optimizeLegibility",
        opacity: boxOpacity,
        textAlign: align === "center" ? "center" : align === "end" ? "right" : "left",
      }}
    >
      {body}
    </div>
  );
};
