// Scenes 4–8: overlay in action, never in the way, hotkeys, edit mode, smart behaviour.
import React from "react";
import { AbsoluteFill, interpolate } from "remotion";
import { JP_LINE, SAMPLE, SONG_A, SONG_B, SONG_C } from "../data/demo";
import { PROMO } from "../lib/theme";
import { EASE, useScene } from "../lib/time";
import { WindowFrame } from "../ui/Desktop";
import { EditDim, EditFrame, EditToolbar, SnapMarkers } from "../ui/EditMode";
import { GameScene } from "../ui/GameScene";
import { Callout, Cursor, KeyCap, KeyCombo, Panel, SceneShell, sceneById } from "../ui/Kit";
import type { Layout } from "../ui/Overlay";
import { lyricAt, schedule } from "../ui/lyrics";
import { OverlayStage } from "../ui/Stage";

/* ---------------------------------------------------------------- s04 overlay in action */

export const S04Overlay: React.FC = () => {
  const { sec, durSec, window, p } = useScene();
  const on = sceneById("s04-overlay").onScreen;
  const changeAt = 0.52 * durSec; // first sign of the new song
  const a = lyricAt(schedule(SONG_A.lines, 0.6, [3.6, 3.2, 3.4, 3.4]), sec);
  const b = lyricAt(schedule(SONG_B.lines, changeAt + 0.9, [3.4, 3.2, 3.2]), sec);
  const beforeChange = sec < changeAt;
  // content fades to 0.15 over 150 ms at the first sign, the new header fades in over 300 ms
  const fadeOut = interpolate(sec, [changeAt, changeAt + 0.15], [1, 0.15], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const headerIn = interpolate(sec, [changeAt + 0.55, changeAt + 0.85], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const props = beforeChange
    ? { song: SONG_A, accent: SONG_A.accent, ...a, contentOpacity: fadeOut }
    : sec < changeAt + 0.55
      ? { song: SONG_A, accent: SONG_A.accent, ...a, contentOpacity: 0.15 }
      : { song: SONG_B, accent: SONG_B.accent, ...b, headerIn, contentOpacity: 1 };
  const swatch = window(0.79, 1.01);
  return (
    <SceneShell id="s04-overlay" bg={<GameScene />}>
      <OverlayStage zoom={2} top={110} {...props} />
      <div style={{ position: "absolute", left: 0, right: 0, top: 470, display: "flex", justifyContent: "center", gap: 28 }}>
        <Callout text={on[0]} o={window(0.1, 0.52)} size={36} />
        <Callout text={on[1]} o={window(0.405, 0.52)} size={36} />
      </div>
      <div style={{ position: "absolute", left: 0, right: 0, top: 470, display: "flex", justifyContent: "center" }}>
        <Callout text={on[2]} o={window(0.53, 0.79)} size={36} />
      </div>
      <div style={{ position: "absolute", left: 0, right: 0, top: 470, display: "flex", justifyContent: "center", alignItems: "center", gap: 30, opacity: swatch }}>
        <div style={{ width: 120, height: 120, borderRadius: 18, background: `linear-gradient(135deg, ${SONG_B.cover[0]}, ${SONG_B.cover[1]})`, position: "relative", overflow: "hidden" }}>
          <div style={{ position: "absolute", left: 36, top: 10, width: 74, height: 74, borderRadius: "50%", background: SONG_B.cover[2], opacity: 0.85 }} />
        </div>
        <div style={{ fontSize: 50, color: PROMO.accent }}>→</div>
        <div style={{ width: 120, height: 120, borderRadius: 60, background: `rgb(${SONG_B.accent.join(",")})`, transform: `scale(${0.8 + 0.2 * p(0.79, 0.86)})` }} />
        <Callout text={on[3]} size={36} />
      </div>
    </SceneShell>
  );
};

/* ------------------------------------------------------------ s05 never in the way */

export const S05OutOfTheWay: React.FC = () => {
  const { sec, durSec, window, p } = useScene();
  const on = sceneById("s05-out-of-the-way").onScreen;
  const ly = lyricAt(schedule(SONG_A.lines, 0.3, 3.3), sec % 13);
  // 1. clicks go through
  const click = window(0.1, 0.27);
  const cx = interpolate(sec, [0.11 * durSec, 0.17 * durSec], [1300, 960], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: EASE });
  const cy = interpolate(sec, [0.11 * durSec, 0.17 * durSec], [600, 80], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: EASE });
  const tClick = sec - 0.185 * durSec;
  const ripple = tClick > 0 && tClick < 0.8 ? tClick / 0.8 : 0;
  // 2. focus stays with the game
  const focus = window(0.27, 0.37);
  const keys = ["W", "A", "S", "D"];
  const kIndex = Math.floor((sec * 3) % 4);
  // 3. not in Alt+Tab or taskbar
  const alt = window(0.37, 0.545);
  // 4. stays on top
  const top = window(0.545, 1.01);
  const push = (Math.floor(sec * 2) % 2 === 0 ? 1 : 0) * top;
  const modes = window(0.855, 1.01);
  return (
    <SceneShell id="s05-out-of-the-way" bg={<GameScene />}>
      <OverlayStage zoom={1.25} top={40} song={SONG_A} accent={SONG_A.accent} {...ly} />
      {/* 1 */}
      <AbsoluteFill style={{ opacity: click }}>
        {ripple ? (
          <div style={{ position: "absolute", left: 960 - 60 * ripple - 10, top: 80 - 60 * ripple - 10, width: 20 + 120 * ripple, height: 20 + 120 * ripple, borderRadius: "50%", border: `3px solid rgba(255,211,110,${1 - ripple})` }} />
        ) : null}
        <Cursor x={cx} y={cy} down={tClick > 0 && tClick < 0.2} />
        <div style={{ position: "absolute", left: 0, right: 0, top: 560, display: "flex", justifyContent: "center" }}>
          <Callout text={on[0]} size={40} />
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 640, display: "flex", justifyContent: "center", color: PROMO.muted, fontSize: 28, opacity: ripple ? 1 : tClick > 0.8 ? 1 : 0 }}>
          The click reached the game, not the overlay
        </div>
      </AbsoluteFill>
      {/* 2 */}
      <AbsoluteFill style={{ opacity: focus }}>
        <div style={{ position: "absolute", left: 0, right: 0, top: 470, display: "flex", justifyContent: "center", gap: 16 }}>
          {keys.map((k, i) => (
            <KeyCap key={k} label={k} pressed={i === kIndex ? 1 : 0} size={1.2} />
          ))}
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 600, display: "flex", justifyContent: "center" }}>
          <Callout text={on[1]} size={40} />
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 680, display: "flex", justifyContent: "center", color: PROMO.muted, fontSize: 28 }}>
          Every key press still goes to the game
        </div>
      </AbsoluteFill>
      {/* 3 */}
      <AbsoluteFill style={{ opacity: alt }}>
        <div style={{ position: "absolute", left: 0, right: 0, top: 360, display: "flex", justifyContent: "center" }}>
          <Panel style={{ display: "flex", gap: 26, padding: 26 }}>
            {["Game", "Browser", "Spotify"].map((n, i) => (
              <div key={n} style={{ width: 220, textAlign: "center" }}>
                <div style={{ height: 130, borderRadius: 12, background: i === 0 ? "linear-gradient(160deg,#2a1846,#0e1126)" : "#1d2130", border: `2px solid ${i === 0 ? PROMO.accent : PROMO.line}` }} />
                <div style={{ fontSize: 24, marginTop: 10 }}>{n}</div>
              </div>
            ))}
          </Panel>
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 640, display: "flex", justifyContent: "center", gap: 20, alignItems: "center" }}>
          <KeyCombo combo="Alt+Tab" size={0.8} />
          <Callout text={on[2]} size={36} />
        </div>
      </AbsoluteFill>
      {/* 4 */}
      <AbsoluteFill style={{ opacity: top }}>
        <AbsoluteFill style={{ background: `rgba(255,120,80,${0.18 * push})` }} />
        <div style={{ position: "absolute", left: 0, right: 0, top: 400, display: "flex", justifyContent: "center", opacity: 1 - modes }}>
          <Panel style={{ textAlign: "center" }}>
            <div style={{ fontSize: 30, color: PROMO.muted }}>Game forces itself to the front every 500 ms</div>
            <div style={{ fontSize: 44, fontWeight: 800, marginTop: 10, color: push ? "#ff9b7a" : PROMO.text }}>{push ? "Game: bring to front!" : "LyricHub: back on top"}</div>
          </Panel>
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 600, display: "flex", justifyContent: "center", opacity: 1 - modes }}>
          <Callout text={on[3]} size={40} />
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 430, display: "flex", justifyContent: "center", gap: 40, opacity: modes }}>
          {(["Borderless", "Windowed", "Fullscreen"] as const).map((m, i) => (
            <div key={m} style={{ textAlign: "center", opacity: p(0.86 + i * 0.025, 0.89 + i * 0.025) }}>
              <div style={{ width: 400, height: 225, borderRadius: 12, overflow: "hidden", position: "relative", background: "linear-gradient(160deg,#2a1846,#0e1126)", border: `2px solid ${PROMO.line}` }}>
                {m === "Windowed" ? <div style={{ position: "absolute", inset: 20, borderRadius: 8, border: "2px solid rgba(255,255,255,.35)" }} /> : null}
                <div style={{ position: "absolute", left: "50%", top: m === "Windowed" ? 30 : 8, transform: "translateX(-50%)", width: 150, height: 26, borderRadius: 6, background: "rgba(12,12,16,.7)" }}>
                  <div style={{ width: "55%", height: 6, borderRadius: 3, background: PROMO.accent, margin: "10px 12px" }} />
                </div>
              </div>
              <div style={{ fontSize: 30, fontWeight: 700, marginTop: 12 }}>{m}</div>
            </div>
          ))}
        </div>
      </AbsoluteFill>
    </SceneShell>
  );
};

/* -------------------------------------------------------------------- s06 hotkeys */

const HOTKEYS: [string, number][] = [
  ["Ctrl+Alt+L", 0],
  ["Ctrl+Alt+E", 1],
  ["Ctrl+Alt+K", 2],
  ["Ctrl+Alt+R", 3],
  ["Ctrl+Alt+,", 4],
  ["Ctrl+Alt+.", 5],
  ["Ctrl+Alt+Shift+,", 6],
  ["Ctrl+Alt+Shift+.", 7],
];
// beats as fractions of the scene, following the voice-over order
const HK_BEATS: [number, number][] = [
  [0.16, 0.27],
  [0.27, 0.36],
  [0.36, 0.43],
  [0.43, 0.55],
  [0.55, 0.66],
  [0.66, 0.78],
  [0.78, 0.83],
  [0.83, 0.87],
];
const LAYOUTS: Layout[] = ["karaoke", "bar", "stack", "minimal"];

export const S06Hotkeys: React.FC = () => {
  const { sec, durSec, p, window } = useScene();
  const on = sceneById("s06-hotkeys").onScreen;
  const at = (f: number) => f * durSec;
  const ly = lyricAt(schedule(SONG_A.lines, 0.2, 3.3), sec % 13.2);
  const active = HK_BEATS.findIndex(([a, b]) => sec >= at(a) && sec < at(b));
  const pressedFor = (i: number) => {
    const t = sec - at(HK_BEATS[i][0]) - 0.3;
    return t > 0 && t < 0.35 ? 1 : 0;
  };
  // Effect on the overlay
  let visible = 1;
  let layout: Layout = "karaoke";
  let edit = false;
  let toast: string | null = null;
  let romaji = false;
  if (active === 0) {
    const t = sec - at(HK_BEATS[0][0]);
    visible = t < 0.6 ? 1 : t < 2.0 ? 0 : 1;
  } else if (active === 1) edit = sec - at(HK_BEATS[1][0]) > 0.6;
  else if (active === 2) {
    const t = sec - at(HK_BEATS[2][0]) - 0.6;
    layout = t < 0 ? "karaoke" : LAYOUTS[Math.min(3, Math.floor(t / 0.55) + 1)] ?? "minimal";
  } else if (active === 3) {
    const t = sec - at(HK_BEATS[3][0]);
    romaji = t > 0.6;
    if (t > 0.6 && t < 2.1) toast = "Romaji on";
  } else if (active >= 4 && active <= 7) {
    const t = sec - at(HK_BEATS[active][0]);
    const msg = ["Song offset −100 ms · Lyrics earlier", "Song offset +100 ms · Lyrics later", "Song offset +75 ms · Lyrics later", "Song offset +100 ms · Lyrics later"][active - 4];
    if (t > 0.6 && t < 2.1) toast = msg;
  }
  const jp = romaji ? JP_LINE.romaji : JP_LINE.text;
  const overlayProps =
    active === 3
      ? { song: SONG_A, accent: SONG_A.accent, active: jp, fill: interpolate(sec - at(HK_BEATS[3][0]), [0.2, 3.5], [0.05, 0.9], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }), next: "", lineIn: 1 }
      : { song: SONG_A, accent: SONG_A.accent, ...ly };
  const listO = p(0.04, 0.12);
  const allO = window(0.87, 1.01);
  return (
    <SceneShell id="s06-hotkeys" bg={<GameScene dim={0.5} />}>
      {/* list of hotkeys */}
      <div style={{ position: "absolute", left: 50, top: 130, width: 960, opacity: listO * (1 - allO) }}>
        {HOTKEYS.map(([combo, i]) => {
          const isActive = active === i;
          return (
            <div
              key={combo}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 22,
                height: 86,
                padding: "0 18px",
                borderRadius: 14,
                background: isActive ? "rgba(255,211,110,.12)" : "transparent",
                border: `1px solid ${isActive ? "rgba(255,211,110,.45)" : "transparent"}`,
                opacity: active < 0 || isActive ? 1 : 0.55,
              }}
            >
              <div style={{ width: 420, flexShrink: 0 }}>
                <KeyCombo combo={combo} pressed={isActive ? pressedFor(i) : 0} size={0.74} />
              </div>
              <div style={{ fontSize: 28, fontWeight: 600, whiteSpace: "nowrap" }}>{on[i]}</div>
            </div>
          );
        })}
      </div>
      {/* effect on the overlay (right) */}
      <div style={{ opacity: 1 - allO }}>
        <OverlayStage zoom={1.25} top={300} centerX={1475} o={visible} layout={layout} toast={toast} {...overlayProps} />
        {edit ? (
          <div style={{ position: "absolute", left: 1475 - (672 * 1.25) / 2 - 6, top: 294, width: 672 * 1.25 + 12, height: 106 * 1.25 + 12, outline: "2px dashed rgba(255,211,110,.95)", borderRadius: 4 }} />
        ) : null}
      </div>
      {/* all eight can be changed */}
      <div style={{ position: "absolute", left: 0, right: 0, top: 120, display: "flex", flexDirection: "column", alignItems: "center", gap: 26, opacity: allO }}>
        <Panel style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: "10px 40px", alignItems: "center" }}>
          {HOTKEYS.map(([combo, i]) => (
            <React.Fragment key={combo}>
              <span style={{ fontSize: 26 }}>{on[i]}</span>
              <span style={{ display: "flex", gap: 10 }}>
                {["Change", "Off", "Default"].map((b) => (
                  <span key={b} style={{ padding: "6px 14px", borderRadius: 8, background: "#262b36", border: "1px solid #2a2f3a", fontSize: 22 }}>{b}</span>
                ))}
              </span>
            </React.Fragment>
          ))}
        </Panel>
        <Callout text={on[8]} size={40} />
      </div>
    </SceneShell>
  );
};

/* ------------------------------------------------------------------ s07 edit mode */

export const S07EditMode: React.FC = () => {
  const { sec, durSec, window, p } = useScene();
  const on = sceneById("s07-edit-mode").onScreen;
  const at = (f: number) => f * durSec;
  const enter = p(0.085, 0.11);
  const pressE = sec > at(0.06) && sec < at(0.085) ? 1 : 0;
  // drag top-centre → top-left (snap)
  const drag = interpolate(sec, [at(0.275), at(0.33)], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: EASE });
  const scaleK = interpolate(sec, [at(0.335), at(0.415)], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: EASE });
  const widthK = interpolate(sec, [at(0.425), at(0.525)], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: EASE });
  const snapShow = sec > at(0.535) && sec < at(0.68);
  const saved = sec > at(0.75);
  const s = 1 + 0.25 * scaleK;
  const widthPct = 35 + 10 * widthK;
  const w = (1920 * widthPct) / 100;
  const h = 106 * s;
  const x = interpolate(drag, [0, 1], [(1920 - 672) / 2, 24]);
  const y = 24;
  const near = drag > 0.75 && drag < 0.98 ? 0 : -1;
  const hit = drag >= 0.98 ? 0 : -1;
  const anchor = drag >= 0.98 ? "Top left" : "Top center";
  const readout = `${anchor} · Width ${Math.round(widthPct)}% · Size ${s.toFixed(2)}`;
  // cursor path
  let cx = 960;
  let cy = 500;
  let kind: "arrow" | "move" | "resize" = "arrow";
  let down = false;
  if (sec < at(0.275)) {
    const k = interpolate(sec, [at(0.2), at(0.265)], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: EASE });
    cx = 960;
    cy = 500 + (70 - 500) * k;
    kind = k > 0.9 ? "move" : "arrow";
  } else if (sec < at(0.335)) {
    cx = x + 336;
    cy = 70;
    kind = "move";
    down = sec < at(0.33);
  } else if (sec < at(0.42)) {
    cx = x + w + 6;
    cy = y + h + 6;
    kind = "resize";
    down = sec > at(0.335) && sec < at(0.415);
  } else if (sec < at(0.53)) {
    cx = x + w + 6;
    cy = y + h / 2;
    kind = "resize";
    down = sec > at(0.425) && sec < at(0.525);
  } else {
    cx = x + w + 120;
    cy = y + h + 120;
  }
  const editing = !saved && enter > 0;
  // edit mode shows LyricHub's sample texts; outside it the song plays on
  const content = editing
    ? { song: { ...SONG_A, title: SAMPLE.title, artist: SAMPLE.artist }, active: SAMPLE.line, fill: 0.45, next: SAMPLE.later }
    : { song: SONG_A, ...lyricAt(schedule(SONG_A.lines, 0.2, 3.3), sec % 16.5) };
  return (
    <SceneShell id="s07-edit-mode" chip={false} bg={<GameScene />}>
      {editing ? <EditDim /> : null}
      {editing ? <SnapMarkers w={1920} h={1080} near={near} hit={snapShow ? 0 : hit} allNear={snapShow} /> : null}
      <div style={{ position: "absolute", left: x, top: y }}>
        <OverlayStage centerX={w / 2} top={0} width={w} s={s} accent={SONG_A.accent} align="start" {...content} />
      </div>
      {editing ? <EditFrame x={x} y={y} w={w} h={h} /> : null}
      {editing ? <EditToolbar x={960} y={1080 * 0.62} zoom={1.5} readout={readout} savePressed={sec > at(0.72)} /> : null}
      {editing ? <Cursor x={cx} y={cy} kind={kind} down={down} /> : null}
      {/* Ctrl+Alt+E opens the edit mode */}
      <div style={{ position: "absolute", left: 0, right: 0, top: 380, display: "flex", justifyContent: "center", opacity: window(0.01, 0.2) }}>
        <KeyCombo combo="Ctrl+Alt+E" pressed={pressE} size={1} />
      </div>
      {/* callouts */}
      <div style={{ position: "absolute", left: 1080, top: 260, display: "grid", gap: 18 }}>
        <Callout text={on[0]} o={window(0.27, 0.335, 0.2)} size={34} />
        <Callout text={on[1]} o={window(0.335, 0.425, 0.2)} size={34} />
        <Callout text={on[2]} o={window(0.425, 0.535, 0.2)} size={34} />
        <Callout text={on[3]} o={window(0.535, 0.68, 0.2)} size={34} />
      </div>
      <div style={{ position: "absolute", left: 0, right: 0, top: 380, display: "flex", justifyContent: "center", alignItems: "center", gap: 26, opacity: window(0.68, 0.76, 0.2) }}>
        <KeyCap label="Enter" pressed={sec > at(0.72) && sec < at(0.75) ? 1 : 0} size={1.1} />
        <KeyCap label="Esc" size={1.1} />
        <Callout text={on[4]} size={36} />
      </div>
      <div style={{ position: "absolute", left: 0, right: 0, top: 380, display: "flex", justifyContent: "center", opacity: window(0.76, 1.01) }}>
        <Callout text={on[5]} size={36} />
      </div>
      <div style={{ position: "absolute", left: 0, right: 0, top: 460, display: "flex", justifyContent: "center", opacity: window(0.76, 1.01), color: PROMO.muted, fontSize: 28 }}>
        Toolbar button with more than one monitor
      </div>
    </SceneShell>
  );
};

/* -------------------------------------------------------------- s08 smart behaviour */

export const S08Behavior: React.FC = () => {
  const { sec, durSec, window } = useScene();
  const on = sceneById("s08-behavior").onScreen;
  const at = (f: number) => f * durSec;
  // segments follow the voice-over sentences; before B[0] the overlay simply plays
  const B = [0.1, 0.265, 0.39, 0.515, 0.58, 0.795, 1.0];
  const seg = B.findIndex((b, i) => i < 6 && sec >= at(b) && sec < at(B[i + 1]));
  const tIn = (i: number) => sec - at(B[i]);
  const ly = lyricAt(schedule(SONG_A.lines, 0, 3.3), sec % 13.2);

  let props: React.ComponentProps<typeof OverlayStage> = { song: SONG_A, accent: SONG_A.accent, ...ly };
  let chip = "";
  if (seg === 0) {
    // paused: dims after 2 s (shown compressed), fades out at the configured time
    const t = tIn(0);
    const box = t < 1.4 ? 1 : t < 1.8 ? 1 - 0.4 * ((t - 1.4) / 0.4) : t < 4.0 ? 0.6 : t < 4.4 ? 0.6 * (1 - (t - 4.0) / 0.4) : 0;
    props = { song: SONG_A, accent: SONG_A.accent, ...lyricAt(schedule(SONG_A.lines, -1, 3.3), 0.4), boxOpacity: box };
    chip = t < 1.4 ? "⏸ Paused" : t < 4.0 ? "⏸ Paused · after 2 s: dims" : "⏸ Paused · after 10 s: fades out";
  } else if (seg === 1) {
    const t = tIn(1);
    props = t < 1.0 ? { song: SONG_C, accent: SONG_C.accent, state: "loading", time: t } : t < 4.0 ? { song: SONG_C, accent: SONG_C.accent, state: "hint", hint: "No synced lyrics found" } : { song: SONG_C, accent: SONG_C.accent, state: "empty" };
  } else if (seg === 2) {
    const t = tIn(2);
    props = t < 3.2 ? { song: SONG_B, accent: SONG_B.accent, state: "hint", hint: "Instrumental" } : { song: SONG_B, accent: SONG_B.accent, state: "empty" };
  } else if (seg === 3) {
    props = { song: SONG_A, accent: SONG_A.accent, ...ly, o: 0 };
    chip = "Ad or podcast playing · overlay hidden, no lyrics lookup";
  } else if (seg === 4) {
    props = { song: SONG_C, accent: SONG_C.accent, offlineDot: true, state: "loading", time: tIn(4) };
    chip = "Lyrics service unreachable · retry after 5 s, 30 s, 120 s";
  }
  const fsSeg = seg === 5;
  const tFs = tIn(5);
  const fullscreenFront = tFs > 2.4;
  return (
    <SceneShell id="s08-behavior" bg={<GameScene dim={0.15} />}>
      {!fsSeg ? <OverlayStage zoom={1.9} top={110} {...props} /> : null}
      {chip ? (
        <div style={{ position: "absolute", left: 0, right: 0, top: 410, display: "flex", justifyContent: "center" }}>
          <Panel style={{ padding: "14px 26px", fontSize: 30, fontWeight: 600 }}>{chip}</Panel>
        </div>
      ) : null}
      <div style={{ position: "absolute", left: 0, right: 0, top: 560, display: "flex", justifyContent: "center" }}>
        {on.map((t, i) => (
          <div key={t} style={{ position: "absolute", opacity: window(B[i] + 0.01, B[i + 1] + 0.005) }}>
            <Callout text={t} size={40} />
          </div>
        ))}
      </div>
      {fsSeg ? (
        <AbsoluteFill>
          <AbsoluteFill style={{ background: "linear-gradient(160deg, #1d2a52, #2b1b44)", opacity: fullscreenFront ? 0 : 1 }} />
          {!fullscreenFront ? (
            <div style={{ position: "absolute", left: 360, top: 110 }}>
              <WindowFrame title="Browser" width={1200} height={260}>
                <div style={{ padding: 30, color: "#9aa1b2", fontSize: 24, fontFamily: '"Segoe UI", sans-serif' }}>A normal window in front · the overlay stays hidden</div>
              </WindowFrame>
            </div>
          ) : null}
          {fullscreenFront ? <OverlayStage zoom={1.6} top={90} song={SONG_A} accent={SONG_A.accent} {...ly} /> : null}
          <div style={{ position: "absolute", left: 0, right: 0, top: 410, display: "flex", justifyContent: "center" }}>
            <Panel style={{ padding: "14px 26px", fontSize: 30, fontWeight: 600 }}>{fullscreenFront ? "Fullscreen game in front · overlay shows" : "Option on: only with a fullscreen app"}</Panel>
          </div>
        </AbsoluteFill>
      ) : null}
    </SceneShell>
  );
};
