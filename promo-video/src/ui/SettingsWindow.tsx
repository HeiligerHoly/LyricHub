// Recreation of LyricHub's settings window (src/lib/settings/App.svelte and pages/*.svelte):
// 960x640, 232 px sidebar, palette from App.svelte, English labels from locales/en.ts.
import React from "react";
import { Img, staticFile } from "remotion";
import { SONG_A, type DemoSong } from "../data/demo";
import { SETTINGS as C } from "../lib/theme";
import { KeyCombo } from "./Kit";

export type Page = "overlay" | "appearance" | "sync" | "hotkeys" | "behavior" | "diagnostics";

const NAV: { id: Page; label: string }[] = [
  { id: "overlay", label: "Overlay" },
  { id: "appearance", label: "Appearance" },
  { id: "sync", label: "Sync" },
  { id: "hotkeys", label: "Hotkeys" },
  { id: "behavior", label: "Behavior" },
  { id: "diagnostics", label: "Diagnostics" },
];

const Card: React.FC<{ title?: string; children: React.ReactNode; style?: React.CSSProperties }> = ({ title, children, style }) => (
  <div style={{ background: C.card, border: `1px solid ${C.line}`, borderRadius: 12, padding: "14px 18px", marginBottom: 12, ...style }}>
    {title ? <div style={{ fontSize: 15, fontWeight: 600, marginBottom: 10 }}>{title}</div> : null}
    {children}
  </div>
);

const Row: React.FC<{ label: string; children: React.ReactNode }> = ({ label, children }) => (
  <div style={{ display: "grid", gridTemplateColumns: "190px 1fr", alignItems: "center", gap: 12, minHeight: 34, fontSize: 14 }}>
    <span style={{ color: C.muted }}>{label}</span>
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>{children}</div>
  </div>
);

const Select: React.FC<{ value: string; w?: number }> = ({ value, w = 220 }) => (
  <div style={{ width: w, height: 30, borderRadius: 7, background: C.input, border: `1px solid ${C.line}`, display: "flex", alignItems: "center", justifyContent: "space-between", padding: "0 10px", fontSize: 14 }}>
    {value}
    <span style={{ color: C.muted }}>▾</span>
  </div>
);

const Slider: React.FC<{ v: number; w?: number }> = ({ v, w = 240 }) => (
  <div style={{ position: "relative", width: w, height: 18 }}>
    <div style={{ position: "absolute", top: 7, left: 0, right: 0, height: 4, borderRadius: 2, background: C.line }} />
    <div style={{ position: "absolute", top: 7, left: 0, width: `${v * 100}%`, height: 4, borderRadius: 2, background: C.accent }} />
    <div style={{ position: "absolute", top: 1, left: `calc(${v * 100}% - 8px)`, width: 16, height: 16, borderRadius: "50%", background: C.accent, border: `2px solid ${C.accentInk}` }} />
  </div>
);

const Check: React.FC<{ on: boolean; label: string }> = ({ on, label }) => (
  <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
    <span style={{ width: 16, height: 16, borderRadius: 4, background: on ? C.accent : C.input, border: `1px solid ${on ? C.accent : C.line}`, color: C.accentInk, fontSize: 12, display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
      {on ? "✓" : ""}
    </span>
    {label}
  </span>
);

const Button: React.FC<{ label: string; primary?: boolean }> = ({ label, primary }) => (
  <span style={{ padding: "6px 12px", borderRadius: 8, background: primary ? C.accent : C.button, color: primary ? C.accentInk : C.text, fontWeight: primary ? 600 : 400, fontSize: 14, border: `1px solid ${primary ? C.accent : C.line}`, whiteSpace: "nowrap" }}>
    {label}
  </span>
);

const THEMES: { id: string; name: string; note: string; plate: string; text: string; accent: string; outline?: boolean }[] = [
  { id: "glass_dark", name: "Glass Dark", note: "Default, dark plate", plate: "rgba(12,12,16,.75)", text: "#fff", accent: "#ffd36e" },
  { id: "glass_light", name: "Glass Light", note: "Light plate, dark text", plate: "rgba(255,255,255,.85)", text: "#111", accent: "#9a6700" },
  { id: "neon", name: "Neon", note: "No plate, outline and glow", plate: "transparent", text: "#fff", accent: "#ffd36e", outline: true },
  { id: "mono", name: "Mono", note: "Black and white, high contrast", plate: "rgba(0,0,0,.9)", text: "#fff", accent: "#fff" },
  { id: "custom", name: "Custom", note: "Your own colors, outline and radius", plate: "rgba(16,32,54,.85)", text: "#e6f7ff", accent: "#6ee7ff" },
];

export type SettingsState = {
  theme?: string;
  accentFromCover?: boolean;
  opacity?: number;
  width?: number;
  scale?: number;
  syncOffsetSong?: number;
  calibration?: string | null;
  font?: string;
  reducedMotion?: boolean;
  onlyFullscreen?: boolean;
  /** 0..1 highlight on a control (draws an amber ring), keyed by control name. */
  focus?: string | null;
};

function OverlayPage(st: SettingsState) {
  return (
    <>
      <Card title="Position and size on screen">
        <div style={{ fontSize: 13, color: C.muted, marginBottom: 10 }}>In edit mode you can move and scale the overlay directly. Enter saves, Esc cancels.</div>
        <Button label="Start edit mode" primary />
      </Card>
      <Card>
        <Row label="Layout"><Select value="Karaoke" /></Row>
        <Row label="Monitor"><Select value="Primary display (automatic)" w={260} /></Row>
        <Row label="Anchor">
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 18px)", gap: 4 }}>
            {Array.from({ length: 9 }, (_, i) => (
              <div key={i} style={{ width: 18, height: 12, borderRadius: 3, background: i === 1 ? C.accent : C.input, border: `1px solid ${C.line}` }} />
            ))}
          </div>
        </Row>
        <Row label="Width"><Slider v={((st.width ?? 35) - 10) / 90} /><span>{st.width ?? 35}% of the monitor width</span></Row>
        <Row label="Scale"><Slider v={((st.scale ?? 1) - 0.5) / 2.5} /><span>{(st.scale ?? 1).toFixed(2)}</span></Row>
        <Row label="Japanese lines"><Check on={false} label="Show in romaji (Hepburn)" /></Row>
        <Row label="Hide when paused after"><Select value="10" w={70} /><span style={{ color: C.muted }}>seconds · 0 = never hide</span></Row>
        <Row label="Only with a fullscreen app"><Check on={!!st.onlyFullscreen} label="Show only while a game or video runs fullscreen" /></Row>
      </Card>
    </>
  );
}

function AppearancePage(st: SettingsState) {
  return (
    <>
      <Card title="Theme">
        <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 8 }}>
          {THEMES.map((t) => (
            <div key={t.id} style={{ borderRadius: 10, padding: 8, border: `2px solid ${st.theme === t.id ? C.accent : C.line}`, background: "#0f1218" }}>
              <div style={{ height: 40, borderRadius: 7, background: `linear-gradient(135deg, #2a3350, #4b2b4f)`, display: "flex", alignItems: "center", justifyContent: "center" }}>
                <div style={{ padding: "3px 8px", borderRadius: 6, background: t.plate, fontSize: 11, fontWeight: 700, color: t.text, WebkitTextStroke: t.outline ? "0.6px #000" : undefined }}>
                  <span style={{ color: t.accent }}>Lyr</span>ics
                </div>
              </div>
              <div style={{ fontSize: 13, fontWeight: 600, marginTop: 6 }}>{t.name}</div>
              <div style={{ fontSize: 11, color: C.muted, lineHeight: 1.25 }}>{t.note}</div>
            </div>
          ))}
        </div>
        <div style={{ fontSize: 12, color: C.muted, marginTop: 8 }}>While this page is open, the overlay shows a sample line in the chosen look.</div>
      </Card>
      <Card>
        <Row label="Accent color">
          <Check on={st.accentFromCover !== false} label="From the cover" />
          <Check on={st.accentFromCover === false} label="Fixed" />
          <span style={{ width: 22, height: 22, borderRadius: 5, background: "#ffd36e", border: `1px solid ${C.line}` }} />
        </Row>
        <Row label="Plate opacity"><Slider v={st.opacity ?? 0.55} /><span>{Math.round((st.opacity ?? 0.55) * 100)}%</span></Row>
        <Row label="Font"><Select value={st.font ?? "Inter (bundled)"} /></Row>
        <Row label="Reduce motion"><Check on={!!st.reducedMotion} label="Transitions at most 80 ms" /></Row>
      </Card>
    </>
  );
}

function SyncPage(st: SettingsState) {
  const song = st.syncOffsetSong ?? 0;
  return (
    <>
      <Card title="Offset for this song">
        <Row label={`${SONG_A.title} – ${SONG_A.artist}`}>
          <span style={{ fontWeight: 600, color: song ? C.accent : C.text }}>{song === 0 ? "No shift" : `Lyrics ${Math.abs(song)} ms ${song > 0 ? "later" : "earlier"}`}</span>
          <Button label="Reset to 0" />
        </Row>
      </Card>
      <Card title="Audio devices">
        <Row label="Headphones (wired) · active"><Select value="+100 ms" w={110} /></Row>
        <Row label="Bluetooth headset"><Select value="+150 ms" w={110} /></Row>
      </Card>
      <Card title="Calibrate">
        <div style={{ display: "flex", gap: 10, alignItems: "center", fontSize: 14 }}>
          <Button label="Start calibration" primary />
          <span style={{ color: C.muted }}>{st.calibration ?? "Tap Space at 5 line starts you hear"}</span>
        </div>
      </Card>
      <Card title="Global offset">
        <Row label="Earlier ↔ Later"><Slider v={0.5} /><span>0 ms</span><Button label="Reset to 0" /></Row>
      </Card>
    </>
  );
}

const HOTKEY_ROWS: [string, string][] = [
  ["Overlay on/off", "Ctrl+Alt+L"],
  ["Edit mode (again: save)", "Ctrl+Alt+E"],
  ["Switch layout", "Ctrl+Alt+K"],
  ["Romaji on/off", "Ctrl+Alt+R"],
  ["Song: lyrics 100 ms earlier", "Ctrl+Alt+,"],
  ["Song: lyrics 100 ms later", "Ctrl+Alt+."],
  ["Song: lyrics 25 ms earlier", "Ctrl+Alt+Shift+,"],
  ["Song: lyrics 25 ms later", "Ctrl+Alt+Shift+."],
];

function HotkeysPage() {
  return (
    <Card>
      {HOTKEY_ROWS.map(([label, combo]) => (
        <div key={label} style={{ display: "grid", gridTemplateColumns: "230px 1fr auto", alignItems: "center", gap: 10, height: 44, borderBottom: `1px solid ${C.line}`, fontSize: 14 }}>
          <span>{label}</span>
          <KeyCombo combo={combo} size={0.42} />
          <span style={{ display: "flex", gap: 6 }}>
            <Button label="Change" />
            <Button label="Off" />
            <Button label="Default" />
          </span>
        </div>
      ))}
      <div style={{ marginTop: 10 }}><Button label="Reset all to default" /></div>
    </Card>
  );
}

function BehaviorPage() {
  return (
    <>
      <Card>
        <Row label="Start with Windows"><Check on={true} label="Starts quietly in the tray" /></Row>
        <Row label="Start in the tray"><Check on={false} label="even when started manually" /></Row>
        <Row label="Language"><Select value="System (Windows display language)" w={290} /></Row>
      </Card>
      <Card title="Files">
        <Row label="Settings"><span style={{ color: C.muted }}>%APPDATA%\LyricHub\settings.json</span></Row>
        <Row label="Logs"><span style={{ color: C.muted }}>%APPDATA%\LyricHub\logs</span></Row>
        <Row label="Version"><span>1.0.0</span></Row>
      </Card>
    </>
  );
}

function DiagnosticsPage() {
  const pts = Array.from({ length: 60 }, (_, i) => {
    const y = 100 + Math.sin(i * 0.7) * 6 + (i % 13 === 0 ? 18 : 0);
    return `${20 + i * 11},${y}`;
  }).join(" ");
  return (
    <>
      <Card title="Sync Lab">
        <svg width={690} height={200} style={{ display: "block" }}>
          <rect x={0} y={0} width={690} height={200} rx={8} fill="#12151b" />
          {[40, 100, 160].map((y) => (
            <line key={y} x1={10} x2={680} y1={y} y2={y} stroke={C.line} strokeDasharray={y === 100 ? "0" : "4 4"} />
          ))}
          <text x={14} y={34} fill={C.muted} fontSize={12}>+300 ms</text>
          <text x={14} y={186} fill={C.muted} fontSize={12}>−300 ms</text>
          <polyline points={pts} fill="none" stroke={C.accent} strokeWidth={2} />
          {Array.from({ length: 14 }, (_, i) => (
            <circle key={i} cx={40 + i * 46} cy={100 + Math.cos(i) * 10} r={4} fill={C.ok} />
          ))}
        </svg>
        <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
          <Button label="Show as table" />
          <Button label="Open logs" />
          <Button label="Export diagnostics" primary />
        </div>
      </Card>
      <Card title="Overlay window">
        <Row label="Mode"><span>Click-through</span></Row>
        <Row label="Always on top"><span>re-asserted 12 times</span></Row>
        <Row label=""><Button label="Run self-test" /></Row>
      </Card>
    </>
  );
}

export const SettingsWindow: React.FC<{ page: Page; state?: SettingsState; song?: DemoSong; scale?: number }> = ({ page, state = {}, song = SONG_A, scale = 1 }) => (
  <div
    style={{
      width: 960,
      height: 678,
      transform: `scale(${scale})`,
      transformOrigin: "top left",
      borderRadius: 10,
      overflow: "hidden",
      border: "1px solid rgba(255,255,255,.12)",
      boxShadow: "0 24px 70px rgba(0,0,0,.6)",
      fontFamily: C.font,
      color: C.text,
      background: C.bg,
      display: "flex",
      flexDirection: "column",
    }}
  >
    <div style={{ height: 38, display: "flex", alignItems: "center", gap: 10, padding: "0 14px", background: "#0b0d11", fontSize: 14, color: "#dfe3ec" }}>
      <Img src={staticFile("lyrichub-icon.svg")} style={{ width: 18, height: 18 }} />
      <span style={{ flex: 1 }}>LyricHub</span>
      <span style={{ opacity: 0.6, letterSpacing: 18 }}>– ▢ ✕</span>
    </div>
    <div style={{ flex: 1, display: "grid", gridTemplateColumns: "232px 1fr" }}>
      <div style={{ background: C.sidebar, padding: "18px 14px", borderRight: `1px solid ${C.line}`, display: "flex", flexDirection: "column" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 16 }}>
          <Img src={staticFile("lyrichub-icon.svg")} style={{ width: 28, height: 28 }} />
          <span style={{ fontSize: 17, fontWeight: 650 }}>LyricHub</span>
        </div>
        <div style={{ background: C.card, border: `1px solid ${C.line}`, borderRadius: 10, padding: 10, display: "flex", gap: 10, marginBottom: 16 }}>
          <div style={{ width: 44, height: 44, borderRadius: 7, background: `linear-gradient(135deg, ${song.cover[0]}, ${song.cover[1]})`, flexShrink: 0 }} />
          <div style={{ minWidth: 0 }}>
            <div style={{ fontSize: 11, color: C.muted }}>Now playing</div>
            <div style={{ fontSize: 13, fontWeight: 600, whiteSpace: "nowrap" }}>{song.title}</div>
            <div style={{ fontSize: 12, color: C.muted, whiteSpace: "nowrap" }}>{song.artist}</div>
          </div>
        </div>
        {NAV.map((n) => (
          <div key={n.id} style={{ padding: "8px 12px", borderRadius: 8, fontSize: 14, marginBottom: 2, background: n.id === page ? "rgba(255,211,110,.12)" : "transparent", color: n.id === page ? C.accent : C.text, fontWeight: n.id === page ? 600 : 400 }}>
            {n.label}
          </div>
        ))}
        <div style={{ marginTop: "auto", fontSize: 12, color: C.muted }}>v1.0.0</div>
      </div>
      <div style={{ padding: "22px 28px", overflow: "hidden" }}>
        <div style={{ fontSize: 22, fontWeight: 600, marginBottom: 14 }}>{NAV.find((n) => n.id === page)?.label}</div>
        {page === "overlay" ? OverlayPage(state) : null}
        {page === "appearance" ? AppearancePage(state) : null}
        {page === "sync" ? SyncPage(state) : null}
        {page === "hotkeys" ? <HotkeysPage /> : null}
        {page === "behavior" ? <BehaviorPage /> : null}
        {page === "diagnostics" ? <DiagnosticsPage /> : null}
      </div>
    </div>
  </div>
);
