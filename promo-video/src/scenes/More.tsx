// Scenes 9–15: layouts, appearance, sync tools, romaji, settings tour, tests, outro.
import React from "react";
import { AbsoluteFill, Img, interpolate, staticFile } from "remotion";
import { JP_LINE, JP_SAMPLE, SAMPLE, SONG_A, SONG_B, SONG_JP, type DemoSong } from "../data/demo";
import { PROMO, type ThemeId } from "../lib/theme";
import { EASE, useScene } from "../lib/time";
import { GameScene } from "../ui/GameScene";
import { BigTitle, Callout, KeyCap, KeyCombo, Panel, SceneShell, sceneById } from "../ui/Kit";
import type { Layout } from "../ui/Overlay";
import { lyricAt, schedule } from "../ui/lyrics";
import { SettingsWindow, type Page } from "../ui/SettingsWindow";
import { OverlayStage } from "../ui/Stage";

const SAMPLE_SONG: DemoSong = { title: SAMPLE.title, artist: SAMPLE.artist, cover: ["#2d3a63", "#6b4a8f", "#c9d4ff"], accent: [255, 211, 110], lines: [] };

/* ------------------------------------------------------------------- s09 layouts */

export const S09Layouts: React.FC = () => {
  const { sec, window } = useScene();
  const on = sceneById("s09-layouts").onScreen;
  const ly = lyricAt(schedule(SONG_A.lines, 0.2, 3.0), sec % 12);
  const order: Layout[] = ["karaoke", "bar", "stack", "minimal"];
  const B = [0.03, 0.37, 0.54, 0.72, 1.0];
  return (
    <SceneShell id="s09-layouts" bg={<GameScene dim={0.2} />}>
      {order.map((l, i) => {
        const o = window(B[i], B[i + 1] + (i === 3 ? 0.01 : 0));
        return o > 0 ? (
          <React.Fragment key={l}>
            <OverlayStage zoom={1.7} top={120} o={o} layout={l} song={SONG_A} accent={SONG_A.accent} {...ly} />
            <div style={{ position: "absolute", left: 0, right: 0, top: 560, display: "flex", justifyContent: "center", opacity: o }}>
              <Callout text={on[i]} size={46} />
            </div>
          </React.Fragment>
        ) : null;
      })}
    </SceneShell>
  );
};

/* ---------------------------------------------------------------- s10 appearance */

export const S10Appearance: React.FC = () => {
  const { sec, durSec, window } = useScene();
  const on = sceneById("s10-appearance").onScreen;
  const at = (f: number) => f * durSec;
  const themes: ThemeId[] = ["glass_dark", "glass_light", "neon", "mono", "custom"];
  // beats follow the caption cues: themes 0.06–0.36, accent 0.36–0.54, opacity/font/motion 0.54–0.75, preview after
  const TB = [0.06, 0.17, 0.22, 0.25, 0.28];
  let theme: ThemeId = "glass_dark";
  themes.forEach((t, i) => {
    if (sec >= at(TB[i])) theme = t;
  });
  if (sec >= at(0.36)) theme = "glass_dark";
  const fromCover = !(sec >= at(0.48) && sec < at(0.545));
  const opacity = sec < at(0.55) ? 0.55 : interpolate(sec, [at(0.55), at(0.585), at(0.62)], [0.55, 0.9, 0.55], { extrapolateRight: "clamp" });
  const font = sec >= at(0.62) && sec < at(0.69) ? "Georgia" : undefined;
  const reduced = sec >= at(0.69);
  const fill = ((sec * 0.28) % 1.15);
  const accent = fromCover ? SONG_A.accent : undefined;
  return (
    <SceneShell id="s10-appearance" bg={<GameScene dim={0.25} />}>
      <div style={{ position: "absolute", left: 40, top: 110 }}>
        <SettingsWindow
          page="appearance"
          scale={1.08}
          state={{ theme, accentFromCover: fromCover, opacity, font: font ?? "Inter (bundled)", reducedMotion: reduced }}
        />
      </div>
      <OverlayStage
        zoom={1.12}
        top={250}
        centerX={1480}
        theme={theme}
        plateOpacity={opacity}
        accent={accent}
        fontFamily={font}
        song={SAMPLE_SONG}
        active={SAMPLE.line}
        fill={Math.min(1, fill)}
        next={SAMPLE.later}
      />
      <div style={{ position: "absolute", left: 1100, width: 760, top: 470, display: "flex", justifyContent: "center" }}>
        {[
          [on[0], 0.06, 0.17],
          [on[1], 0.17, 0.22],
          [on[2], 0.22, 0.25],
          [on[3], 0.25, 0.28],
          [on[4], 0.28, 0.36],
          [on[5], 0.36, 0.54],
          [on[6], 0.54, 0.75],
          [on[7], 0.75, 1.01],
        ].map(([t, a, b]) => (
          <div key={t as string} style={{ position: "absolute", opacity: window(a as number, b as number, 0.12) }}>
            <Callout text={t as string} size={36} />
          </div>
        ))}
      </div>
    </SceneShell>
  );
};

/* ---------------------------------------------------------------- s11 sync tools */

export const S11Sync: React.FC = () => {
  const { sec, durSec, window, p } = useScene();
  const on = sceneById("s11-sync").onScreen;
  const at = (f: number) => f * durSec;
  const ly = lyricAt(schedule(SONG_A.lines, 0.4, 3.2), sec % 12.8);
  const nudgeAt = at(0.19);
  const toast = sec > nudgeAt && sec < nudgeAt + 1.5 ? "Song offset +100 ms · Lyrics later" : null;
  const s1 = window(0.02, 0.26);
  const s2 = window(0.26, 0.42);
  const s3 = window(0.42, 0.62);
  const s4 = window(0.62, 0.8);
  const s5 = window(0.8, 1.01);
  // calibration taps
  const calStart = at(0.675);
  const taps = Math.max(0, Math.min(5, Math.floor((sec - calStart) / 0.9)));
  const tapPressed = sec > calStart && (sec - calStart) % 0.9 < 0.18 && taps < 5 ? 1 : 0;
  const calToast = taps < 5 ? `Calibration ${taps}/5 · Space when you hear the next line start · Esc ends` : null;
  const row = (label: string, value: string, note: string, hl: boolean) => (
    <div style={{ display: "grid", gridTemplateColumns: "520px 220px 1fr", alignItems: "center", height: 78, padding: "0 20px", borderRadius: 12, background: hl ? "rgba(255,211,110,.1)" : "transparent", fontSize: 30 }}>
      <span>{label}</span>
      <span style={{ fontWeight: 700, color: hl ? PROMO.accent : PROMO.text }}>{value}</span>
      <span style={{ color: PROMO.muted, fontSize: 26 }}>{note}</span>
    </div>
  );
  return (
    <SceneShell id="s11-sync" bg={<GameScene dim={0.45} />}>
      {/* 1 nudge */}
      <AbsoluteFill style={{ opacity: s1 }}>
        <OverlayStage zoom={1.8} top={120} song={SONG_A} accent={SONG_A.accent} toast={toast} {...ly} />
        <div style={{ position: "absolute", left: 0, right: 0, top: 440, display: "flex", justifyContent: "center", gap: 30, alignItems: "center" }}>
          <KeyCombo combo="Ctrl+Alt+." pressed={sec > nudgeAt - 0.2 && sec < nudgeAt + 0.2 ? 1 : 0} size={0.9} />
          <span style={{ fontSize: 30, color: PROMO.muted }}>{sec < nudgeAt ? "Lyrics run 100 ms early" : "Now in sync"}</span>
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 580, display: "flex", justifyContent: "center" }}>
          <Callout text={on[0]} size={40} />
        </div>
      </AbsoluteFill>
      {/* 2 saved per song */}
      <AbsoluteFill style={{ opacity: s2 }}>
        <div style={{ position: "absolute", left: 0, right: 0, top: 230, display: "flex", justifyContent: "center" }}>
          <Panel style={{ width: 1300 }}>
            {row(`${SONG_A.title} – ${SONG_A.artist}`, "+100 ms", "saved for this song", true)}
            {row(`${SONG_B.title} – ${SONG_B.artist}`, "0 ms", "next song starts fresh", false)}
          </Panel>
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 520, display: "flex", justifyContent: "center" }}>
          <Callout text={on[1]} size={40} />
        </div>
      </AbsoluteFill>
      {/* 3 devices */}
      <AbsoluteFill style={{ opacity: s3 }}>
        <div style={{ position: "absolute", left: 0, right: 0, top: 230, display: "flex", justifyContent: "center" }}>
          <Panel style={{ width: 1300 }}>
            {row("Headphones (wired) · active", "+100 ms", "switches automatically", true)}
            {row("Bluetooth headset · new", "+150 ms", "default for new Bluetooth devices", false)}
          </Panel>
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 520, display: "flex", justifyContent: "center" }}>
          <Callout text={on[2]} size={40} />
        </div>
      </AbsoluteFill>
      {/* 4 calibration */}
      <AbsoluteFill style={{ opacity: s4 }}>
        <OverlayStage zoom={1.6} top={110} song={SONG_A} accent={SONG_A.accent} toast={calToast} {...ly} width={900} />
        <div style={{ position: "absolute", left: 0, right: 0, top: 420, display: "flex", justifyContent: "center", alignItems: "center", gap: 30 }}>
          <KeyCap label="Space" pressed={tapPressed} size={1.2} />
          <span style={{ fontSize: 30, color: PROMO.muted }}>{taps < 5 ? `${taps} / 5 taps` : "Suggestion: device offset 110 ms (currently 100 ms) · Apply"}</span>
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 560, display: "flex", justifyContent: "center" }}>
          <Callout text={on[3]} size={40} />
        </div>
      </AbsoluteFill>
      {/* 5 sync lab */}
      <AbsoluteFill style={{ opacity: s5 }}>
        <div style={{ position: "absolute", left: (1920 - 960 * 0.98) / 2, top: 100, opacity: p(0.8, 0.84) }}>
          <SettingsWindow page="diagnostics" scale={0.98} />
        </div>
        <div style={{ position: "absolute", right: 110, top: 330, opacity: p(0.86, 0.89) }}>
          <Callout text={on[4]} size={40} />
        </div>
      </AbsoluteFill>
    </SceneShell>
  );
};

/* ------------------------------------------------------------------- s12 romaji */

export const S12Romaji: React.FC = () => {
  const { sec, durSec, window } = useScene();
  const on = sceneById("s12-romaji").onScreen;
  const pressAt = 0.37 * durSec;
  const romaji = sec > pressAt;
  const toast = romaji && sec < pressAt + 1.5 ? "Romaji on" : null;
  const fill = interpolate(sec, [0.6, durSec - 0.8], [0.05, 0.95], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <SceneShell id="s12-romaji" bg={<GameScene dim={0.2} hue={-30} />}>
      <OverlayStage
        zoom={2}
        top={140}
        song={SONG_JP}
        accent={SONG_JP.accent}
        toast={toast}
        active={romaji ? JP_LINE.romaji : JP_LINE.text}
        next={romaji ? JP_SAMPLE.romaji : JP_SAMPLE.text}
        fill={fill}
      />
      <div style={{ position: "absolute", left: 0, right: 0, top: 480, display: "flex", justifyContent: "center", opacity: window(0.2, 0.6) }}>
        <KeyCombo combo="Ctrl+Alt+R" pressed={sec > pressAt - 0.2 && sec < pressAt + 0.25 ? 1 : 0} size={0.95} />
      </div>
      <div style={{ position: "absolute", left: 0, right: 0, top: 600, display: "flex", justifyContent: "center", opacity: window(0.6, 1.01) }}>
        <Callout text={on[0]} size={42} />
      </div>
    </SceneShell>
  );
};

/* --------------------------------------------------------------- s13 settings tour */

export const S13Settings: React.FC = () => {
  const { sec, durSec, window } = useScene();
  const on = sceneById("s13-settings").onScreen;
  const pages: Page[] = ["overlay", "appearance", "sync", "hotkeys", "behavior", "diagnostics"];
  // the caption names overlay and behavior first, then appearance, sync, hotkeys, startup, diagnostics
  const B = [0.03, 0.425, 0.475, 0.525, 0.575, 0.665];
  let idx = 0;
  pages.forEach((_, i) => {
    if (sec >= B[i] * durSec) idx = i;
  });
  if (sec >= 0.74 * durSec) idx = 4; // language lives on the Behavior page
  const lang = window(0.74, 1.01);
  return (
    <SceneShell id="s13-settings" bg={<AbsoluteFill style={{ background: "radial-gradient(1200px 700px at 40% 30%, #1a2140, #0b0d14)" }} />}>
      <div style={{ position: "absolute", left: 70, top: 105 }}>
        <SettingsWindow page={pages[idx]} scale={1.12} />
      </div>
      <div style={{ position: "absolute", left: 1250, top: 170, display: "grid", gap: 14 }}>
        {pages.map((pg, i) => (
          <div key={pg} style={{ fontSize: 34, fontWeight: i === idx ? 800 : 500, color: i === idx ? PROMO.accent : PROMO.muted, transform: `translateX(${i === idx ? 10 : 0}px)` }}>
            {on[i]}
          </div>
        ))}
        <div style={{ marginTop: 26, opacity: lang }}>
          <Callout text={on[6]} size={36} />
        </div>
      </div>
    </SceneShell>
  );
};

/* -------------------------------------------------------------------- s14 tests */

const TEST_GROUPS: [string, number][] = [
  ["lyrics", 46],
  ["sync", 18],
  ["settings", 11],
  ["overlay window", 8],
  ["sources", 7],
  ["hotkeys", 2],
  ["Sync Lab", 1],
];
const SELF_CHECKS = [
  "Focus stays with the game",
  "Window styles complete",
  "Overlay is drawn",
  "Clicks go through",
  "Edit mode catches clicks",
  "Not in Alt+Tab",
  "Hide and show without focus loss",
];

export const S14Tests: React.FC = () => {
  const { sec, durSec, window, p } = useScene();
  const on = sceneById("s14-tests").onScreen;
  const at = (f: number) => f * durSec;
  // beats follow the caption cues: unit tests 0.04–0.17, self-test 0.17–0.28, simulated game 0.28–0.58, numbers after
  const b1 = window(0.02, 0.175);
  const b2 = window(0.175, 0.285);
  const b3 = window(0.285, 0.585);
  const b4 = window(0.58, 1.01);
  const lines = Math.floor(interpolate(sec, [at(0.04), at(0.11)], [0, TEST_GROUPS.length + 2], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }));
  const checks = Math.floor(interpolate(sec, [at(0.18), at(0.265)], [0, SELF_CHECKS.length], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }));
  const samples = Math.round(interpolate(sec, [at(0.3), at(0.55)], [0, 120], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }));
  const push = Math.floor(sec * 2) % 2 === 0;
  const tile = (big: string, small: string, k: number) => (
    <div style={{ width: 500, padding: "30px 32px", borderRadius: 20, background: PROMO.panel, border: `1px solid ${PROMO.line}`, opacity: k, transform: `translateY(${(1 - k) * 14}px)` }}>
      <div style={{ fontSize: 52, fontWeight: 800, color: PROMO.accent, letterSpacing: -0.5 }}>{big}</div>
      <div style={{ fontSize: 26, color: PROMO.muted, marginTop: 8 }}>{small}</div>
    </div>
  );
  return (
    <SceneShell id="s14-tests" bg={<AbsoluteFill style={{ background: "radial-gradient(1200px 700px at 50% 25%, #16203a, #0b0d14)" }} />}>
      {/* 1 unit tests */}
      <AbsoluteFill style={{ opacity: b1 }}>
        <div style={{ position: "absolute", left: 110, top: 150, width: 1020 }}>
          <Panel style={{ fontFamily: '"Cascadia Mono", Consolas, monospace', fontSize: 26, lineHeight: 1.55, minHeight: 470 }}>
            <div style={{ color: PROMO.muted }}>$ cargo test</div>
            {lines > 0 ? <div>running 93 tests</div> : null}
            {TEST_GROUPS.slice(0, Math.max(0, lines - 1)).map(([g, n]) => (
              <div key={g}>
                {g.padEnd(16, " ")} <span style={{ color: PROMO.ok }}>{n} ok</span>
              </div>
            ))}
            {lines > TEST_GROUPS.length + 1 ? (
              <div style={{ color: PROMO.ok, fontWeight: 700, fontSize: 22 }}>test result: ok. 93 passed; 0 failed; 0 ignored; finished in 0.37s</div>
            ) : null}
          </Panel>
        </div>
        <div style={{ position: "absolute", left: 1200, top: 300 }}>
          <Callout text={on[0]} size={40} o={p(0.1, 0.12)} />
          <div style={{ fontSize: 28, color: PROMO.muted, marginTop: 20, width: 600, opacity: p(0.115, 0.135) }}>Lyrics, sync, settings and the overlay window. LRCLIB answers are recorded, so no test needs the network.</div>
        </div>
      </AbsoluteFill>
      {/* 2 self-test */}
      <AbsoluteFill style={{ opacity: b2 }}>
        <div style={{ position: "absolute", left: 0, right: 0, top: 140, display: "flex", justifyContent: "center", gap: 60, alignItems: "flex-start" }}>
          <Panel style={{ width: 760 }}>
            {SELF_CHECKS.map((c, i) => (
              <div key={c} style={{ display: "flex", alignItems: "center", gap: 18, height: 70, fontSize: 32, opacity: i < checks ? 1 : 0.35 }}>
                <span style={{ width: 40, height: 40, borderRadius: 10, display: "inline-flex", alignItems: "center", justifyContent: "center", background: i < checks ? "rgba(74,222,128,.18)" : "#1d2130", color: PROMO.ok, fontWeight: 800 }}>{i < checks ? "✓" : ""}</span>
                {c}
              </div>
            ))}
          </Panel>
          <div style={{ paddingTop: 140 }}>
            <Callout text={on[1]} size={40} />
            <div style={{ fontSize: 28, color: PROMO.muted, marginTop: 20, width: 660 }}>Run it from the tray or the Diagnostics page.</div>
          </div>
        </div>
      </AbsoluteFill>
      {/* 3 aggressive game */}
      <AbsoluteFill style={{ opacity: b3 }}>
        <div style={{ position: "absolute", left: 260, top: 140, width: 1400, height: 560, borderRadius: 16, overflow: "hidden", background: push ? "#1f5a3a" : "#183f2c", border: `2px solid ${PROMO.line}` }}>
          <div style={{ position: "absolute", left: 30, top: 24, fontSize: 30, color: "rgba(255,255,255,.75)" }}>Simulated game · forces itself to the front every 500 ms</div>
          <div style={{ position: "absolute", left: "50%", top: 90, transform: "translateX(-50%) scale(1.2)", transformOrigin: "top center" }}>
            <OverlayStage centerX={0} top={0} song={SONG_A} accent={SONG_A.accent} {...lyricAt(schedule(SONG_A.lines, 0, 3.2), sec % 12.8)} />
          </div>
          <div style={{ position: "absolute", left: 0, right: 0, bottom: 60, textAlign: "center" }}>
            <div style={{ fontSize: 96, fontWeight: 800, color: PROMO.text }}>{samples} / 120</div>
            <div style={{ fontSize: 30, color: "rgba(255,255,255,.8)" }}>samples with LyricHub on top</div>
          </div>
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 730, display: "flex", justifyContent: "center", opacity: samples >= 120 ? 1 : 0 }}>
          <Callout text={on[2]} size={40} />
        </div>
      </AbsoluteFill>
      {/* 4 numbers */}
      <AbsoluteFill style={{ opacity: b4 }}>
        <div style={{ position: "absolute", left: 0, right: 0, top: 200, display: "flex", justifyContent: "center", gap: 40 }}>
          {tile("96 ms", "median sync error · 13 songs, 115 taps, wired", p(0.585, 0.62, EASE))}
          {tile("≈30 ms", "back in sync after a seek", p(0.715, 0.75, EASE))}
          {tile("0.48 %", "of all CPU cores while playing", p(0.855, 0.89, EASE))}
        </div>
        <div style={{ position: "absolute", left: 0, right: 0, top: 520, textAlign: "center", fontSize: 26, color: PROMO.muted, opacity: p(0.89, 0.93) }}>
          Measured with LyricHub's own test tools · details in the project docs
        </div>
      </AbsoluteFill>
    </SceneShell>
  );
};

/* -------------------------------------------------------------------- s15 outro */

export const S15Outro: React.FC = () => {
  const { p } = useScene();
  const on = sceneById("s15-outro").onScreen;
  const k = p(0.02, 0.18);
  return (
    <SceneShell id="s15-outro" chip={false} bg={<GameScene dim={0.62} speed={0.5} />}>
      <div style={{ position: "absolute", left: 0, right: 0, top: 160, display: "flex", flexDirection: "column", alignItems: "center", gap: 22 }}>
        <Img src={staticFile("lyrichub-icon.svg")} style={{ width: 170, height: 170, opacity: k, transform: `scale(${0.92 + 0.08 * k})` }} />
        <BigTitle text={on[0]} size={110} o={k} />
        <div style={{ fontSize: 40, fontWeight: 600, color: PROMO.accent, opacity: p(0.2, 0.32) }}>{on[1]}</div>
        <div style={{ fontSize: 32, color: PROMO.text, opacity: p(0.3, 0.42) }}>{on[2]}</div>
        <div style={{ fontSize: 24, color: PROMO.muted, opacity: p(0.6, 0.72), marginTop: 20 }}>{on[3]}</div>
      </div>
    </SceneShell>
  );
};
