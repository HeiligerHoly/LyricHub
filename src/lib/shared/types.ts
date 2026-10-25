// Mirrors of the Rust types (PLAN.md 4.3, 4.4), maintained by hand.
// Sources: src-tauri/src/sources/types.rs, settings/schema.rs, overlay/mod.rs,
// overlay/window.rs, overlay/selftest.rs, app_state.rs, ipc/commands.rs.

export type PlaybackStatus = "closed" | "opened" | "changing" | "stopped" | "playing" | "paused";

export interface ArtRef {
  /** data: URL from the SMTC thumbnail */
  url: string;
}

export interface TrackInfo {
  key: string;
  title: string;
  artist: string;
  album: string | null;
  duration_ms: number | null;
  art: ArtRef | null;
  kind: "music" | "unknown";
  source: "smtc";
}

export interface NowPlaying {
  track: TrackInfo | null;
  status: PlaybackStatus;
}

export interface PlaybackEvent {
  status: PlaybackStatus;
}

export type Anchor = "tl" | "tc" | "tr" | "ml" | "mc" | "mr" | "bl" | "bc" | "br";
export const ANCHORS: Anchor[][] = [
  ["tl", "tc", "tr"],
  ["ml", "mc", "mr"],
  ["bl", "bc", "br"],
];

export interface OverlaySettings {
  monitor_id: string | null;
  anchor: Anchor;
  offset_x_pct: number;
  offset_y_pct: number;
  width_pct: number;
  scale: number;
  hide_after_pause_s: number;
}

export interface BehaviorSettings {
  autostart: boolean;
  start_minimized: boolean;
}

export interface Settings {
  schema_version: number;
  overlay: OverlaySettings;
  behavior: BehaviorSettings;
}

/** JSON Merge Patch for `set_settings`: `null` resets a field to its default. */
export type SettingsPatch = {
  overlay?: { [K in keyof OverlaySettings]?: OverlaySettings[K] | null };
  behavior?: { [K in keyof BehaviorSettings]?: BehaviorSettings[K] | null };
};

export const SCALE_MIN = 0.5;
export const SCALE_MAX = 3;
export const WIDTH_PCT_MIN = 10;
export const WIDTH_PCT_MAX = 100;
/** Narrowest overlay in CSS px (overlay/window.rs). */
export const MIN_WIDTH_CSS = 200;

export interface Monitor {
  id: string;
  number: number;
  primary: boolean;
  x: number;
  y: number;
  width: number;
  height: number;
  scale_factor: number;
}

export type Align = "start" | "center" | "end";

export interface EditLayout {
  seq: number;
  monitor_width: number;
  monitor_height: number;
  monitor_label: string;
  monitor_count: number;
  x: number;
  y: number;
  scale: number;
  res_factor: number;
  snap_margin: number;
}

/** Payload of `overlay-layout`. */
export interface OverlayLayout {
  seq: number;
  width_css: number;
  ui_scale: number;
  align: Align;
  preview: boolean;
  show_seq: number;
  edit: EditLayout | null;
}

/** Content box in the edit mode, CSS px relative to the monitor. */
export interface EditDraft {
  x: number;
  y: number;
  width: number;
  height: number;
  width_css: number;
  scale: number;
}

export interface LatencySample {
  title: string;
  artist: string;
  first_sign_to_render_ms: number;
  emit_to_render_ms: number;
  unix_ms: number;
}

export interface AppInfo {
  version: string;
  settings_path: string;
  log_dir: string;
  autostart_registered: boolean | null;
}

export interface OverlayDiagnostics {
  visible: boolean;
  click_through: boolean;
  edit_mode: boolean;
  bounds: string;
  ex_style: number;
  ex_style_text: string;
  topmost: boolean;
  no_activate: boolean;
  tool_window: boolean;
  transparent: boolean;
  layered: boolean;
  app_window: boolean;
  foreground_is_overlay: boolean;
  foreground_title: string;
}

export interface SelfTestStep {
  group: string;
  name: string;
  status: "pass" | "fail" | "info";
  detail: string;
}

export interface SelfTestReport {
  unix_time: number;
  foreground_at_start: string;
  /** False if a LyricHub window held the foreground at start; focus checks are then only info. */
  foreground_conclusive: boolean;
  fake_game_present: boolean;
  /** Production path (groups win32 and topmost). */
  passed: number;
  failed: number;
  /** Expected findings in the tauri comparison group. */
  tauri_failed: number;
  steps: SelfTestStep[];
}
