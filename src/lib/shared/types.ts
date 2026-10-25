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
  kind: "music" | "ad" | "podcast" | "unknown";
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

export type Layout = "karaoke" | "bar" | "stack" | "minimal";
export const LAYOUTS: { id: Layout; label: string }[] = [
  { id: "karaoke", label: "Karaoke" },
  { id: "bar", label: "Leiste" },
  { id: "stack", label: "Stapel" },
  { id: "minimal", label: "Minimal" },
];

export interface OverlaySettings {
  layout: Layout;
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

export type Theme = "glass_dark" | "glass_light" | "neon" | "mono" | "custom";
export type AccentMode = "cover" | "fixed";

export interface CustomTheme {
  plate: boolean;
  /** `#rrggbb`; the opacity is `plate_opacity`. */
  plate_color: string;
  text_color: string;
  outline: boolean;
  radius_px: number;
}
export const RADIUS_MAX_PX = 32;
export const DEFAULT_ACCENT = "#ffd36e";

export interface AppearanceSettings {
  theme: Theme;
  /** Opacity of the plate behind the text, 0..1. */
  plate_opacity: number;
  accent: AccentMode;
  /** `#rrggbb`, used with accent "fixed" and as the custom theme's accent. */
  accent_color: string;
  /** Font family for the lyrics; empty means the bundled Inter. */
  font: string;
  /** All transitions at most 80 ms and no word glow. */
  reduced_motion: boolean;
  custom: CustomTheme;
  /** Show lines with kana or kanji in romaji, modified Hepburn (docs/romaji.md). */
  romaji: boolean;
}

/** Installed font families (fonts.rs). */
export interface FontList {
  families: string[];
  /** Yu Gothic UI or Meiryo UI is installed. */
  japanese_ui_font: boolean;
}

export interface SyncSettings {
  /** Positive shows the lyrics later ("lyrics too early → increase"). */
  global_offset_ms: number;
}
export const OFFSET_LIMIT_MS = 5000;

export interface Settings {
  schema_version: number;
  overlay: OverlaySettings;
  appearance: AppearanceSettings;
  sync: SyncSettings;
  behavior: BehaviorSettings;
}

/** JSON Merge Patch for `set_settings`: `null` resets a field to its default. */
export type SettingsPatch = {
  overlay?: { [K in keyof OverlaySettings]?: OverlaySettings[K] | null };
  appearance?: { [K in keyof AppearanceSettings]?: (K extends "custom" ? Partial<CustomTheme> : AppearanceSettings[K]) | null };
  sync?: { [K in keyof SyncSettings]?: SyncSettings[K] | null };
  behavior?: { [K in keyof BehaviorSettings]?: BehaviorSettings[K] | null };
};

/** Offset parts (sync/offsets.rs); positive shows the lyrics later. */
export interface Offsets {
  global_ms: number;
  device_ms: number;
  song_ms: number;
}

/** Payload of `clock` (app_state.rs). Display position = pos + elapsed × rate − offset. */
export interface ClockEvent {
  pos_ms: number;
  playing: boolean;
  rate: number;
  /** How long `rate` still applies before the speed returns to 1. */
  slew_ms: number;
  seq: number;
  /** Sum of the offsets; positive shows the lyrics later. */
  offset_ms: number;
  offsets: Offsets;
  /** Why the anchor went out: status, seek, jump, slew, adopt, offset, heartbeat, query. */
  kind: string;
}

export interface AudioDevice {
  id: string;
  name: string;
  offset_ms: number;
  current: boolean;
  bluetooth: boolean;
}

export interface TapStats {
  count: number;
  median_abs_ms: number;
  p95_abs_ms: number;
  bias_ms: number;
  within_100: number;
}

export type CalibrationMode = "calibrate" | "protocol";

export interface Tap {
  key: string;
  title: string;
  line_index: number;
  line_start_ms: number;
  /** Overlay position at the tap minus the line start; positive = lyrics too early. */
  error_ms: number;
  unix_ms: number;
}

/** Payload of `calibration` (calibration.rs). */
export interface CalibrationView {
  active: boolean;
  mode: CalibrationMode;
  taps: Tap[];
  missed: number;
  needed: number;
  total: TapStats | null;
  per_song: { key: string; title: string; stats: TapStats }[];
  suggestion_ms: number | null;
  device_offset_ms: number;
  report_path: string | null;
  message: string | null;
}

export interface LabEntry {
  t_ms: number;
  kind: "obs" | "status" | "track" | "offset" | "tap";
  source: string;
  value_ms: number;
  residual_ms: number | null;
  age_ms: number | null;
  update: string;
}

/** Sync Lab snapshot (sync_lab.rs). */
export interface SyncLab {
  now_ms: number;
  entries: LabEntry[];
  clock: ClockEvent;
  cadence_ms: number | null;
  deviating: string[];
  track_key: string | null;
}

export interface Word {
  start_ms: number;
  end_ms: number;
  text: string;
  estimated: boolean;
}

export interface Line {
  start_ms: number;
  end_ms: number | null;
  /** When the karaoke fill is complete; before a long pause earlier than end_ms. */
  fill_end_ms: number;
  /** Empty for an instrumental marker. */
  text: string;
  words: Word[] | null;
  /** Romaji of a line with kana or kanji, filled once per song while the setting is on. */
  romaji: string | null;
  /** Estimated word times of `romaji`. */
  romaji_words: Word[] | null;
}

export type LyricsState = "loading" | "synced" | "unsynced" | "instrumental" | "missing" | "error";

/** Payload of `lyrics` (lyrics/types.rs). */
export interface LyricsEvent {
  key: string;
  state: LyricsState;
  source: string;
  lines: Line[];
  lrc_offset_ms: number;
}

export interface NudgeEvent {
  offset_ms: number;
  delta_ms: number;
}

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
  layout: Layout;
  width_css: number;
  /** The page may widen the plate up to this for long lyric lines. */
  max_width_css: number;
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
