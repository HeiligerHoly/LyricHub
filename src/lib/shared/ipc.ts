// Typed wrappers around Tauri invoke/listen (PLAN.md 4.4). Command arguments are camelCase on
// this side; Tauri maps them to the snake_case Rust parameters.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Anchor,
  AppInfo,
  AudioDevice,
  CalibrationMode,
  CalibrationView,
  ClockEvent,
  EditDraft,
  FontList,
  HotkeyStatus,
  LatencySample,
  LyricsEvent,
  Monitor,
  NudgeEvent,
  NowPlaying,
  OverlayDiagnostics,
  OverlayLayout,
  PlaybackEvent,
  PlayerSession,
  SelfTestReport,
  Settings,
  SettingsPatch,
  SyncLab,
  TrackInfo,
} from "./types";

export const ipc = {
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (patch: SettingsPatch) => invoke<Settings>("set_settings", { patch }),
  setOverlayAnchor: (anchor: Anchor) => invoke<Settings>("set_overlay_anchor", { anchor }),
  resetOverlayGeometry: () => invoke<Settings>("reset_overlay_geometry"),
  listMonitors: () => invoke<Monitor[]>("list_monitors"),
  listPlayers: () => invoke<PlayerSession[]>("list_players"),
  getNowPlaying: () => invoke<NowPlaying>("get_now_playing"),
  trackLatencies: () => invoke<LatencySample[]>("track_latencies"),
  getLyrics: () => invoke<LyricsEvent | null>("get_lyrics"),
  getClock: () => invoke<ClockEvent>("get_clock"),
  nudgeOffset: (deltaMs: number) => invoke<number | null>("nudge_offset", { deltaMs }),
  setSongOffset: (offsetMs: number) => invoke<number | null>("set_song_offset", { offsetMs }),
  listAudioDevices: () => invoke<AudioDevice[]>("list_audio_devices"),
  setDeviceOffset: (id: string, offsetMs: number) => invoke<AudioDevice[]>("set_device_offset", { id, offsetMs }),
  startCalibration: (mode: CalibrationMode) => invoke<CalibrationView>("start_calibration", { mode }),
  stopCalibration: () => invoke<CalibrationView | null>("stop_calibration"),
  applyCalibration: () => invoke<number>("apply_calibration"),
  getCalibration: () => invoke<CalibrationView | null>("get_calibration"),
  clockApplied: (seq: number) => invoke<void>("clock_applied", { seq }),
  syncLab: () => invoke<SyncLab>("sync_lab"),
  exportDiagnostics: () => invoke<string>("export_diagnostics"),
  reloadLyrics: () => invoke<void>("reload_lyrics"),
  getOverlayLayout: () => invoke<OverlayLayout | null>("get_overlay_layout"),
  reportOverlaySize: (width: number, height: number, uiScale: number) =>
    invoke<void>("report_overlay_size", { width, height, uiScale }),
  overlayLayoutReady: (seq: number) => invoke<void>("overlay_layout_ready", { seq }),
  overlayRendered: (key: string) => invoke<void>("overlay_rendered", { key }),
  enterEditMode: () => invoke<void>("enter_edit_mode"),
  overlayEditUpdate: (draft: EditDraft) => invoke<void>("overlay_edit_update", { draft }),
  overlayEditMonitor: (step: number) => invoke<void>("overlay_edit_monitor", { step }),
  saveOverlayGeometry: (draft: EditDraft | null) => invoke<void>("save_overlay_geometry", { draft }),
  cancelEditMode: () => invoke<void>("cancel_edit_mode"),
  toggleOverlay: () => invoke<boolean>("toggle_overlay"),
  setOverlayPreview: (on: boolean) => invoke<void>("set_overlay_preview", { on }),
  listFonts: () => invoke<FontList>("list_fonts"),
  hotkeyStatus: () => invoke<HotkeyStatus[]>("hotkey_status"),
  suspendHotkeys: (suspended: boolean) => invoke<HotkeyStatus[]>("suspend_hotkeys", { suspended }),
  overlayDiagnostics: () => invoke<OverlayDiagnostics>("overlay_diagnostics"),
  runOverlaySelfTest: () => invoke<SelfTestReport>("run_overlay_selftest"),
  appInfo: () => invoke<AppInfo>("app_info"),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
};

interface EventPayloads {
  track: TrackInfo | null;
  "track-changing": null;
  playback: PlaybackEvent;
  clock: ClockEvent;
  lyrics: LyricsEvent;
  nudge: NudgeEvent;
  notice: { text: string };
  calibration: CalibrationView;
  settings: Settings;
  "overlay-layout": OverlayLayout;
}

export function on<E extends keyof EventPayloads>(
  event: E,
  handler: (payload: EventPayloads[E]) => void,
): Promise<UnlistenFn> {
  return listen<EventPayloads[E]>(event, (e) => handler(e.payload));
}

/** Subscribes to several events and returns one cleanup function for `onMount`. */
export function subscribe(...subs: Promise<UnlistenFn>[]): () => void {
  return () => {
    for (const s of subs) s.then((unlisten) => unlisten());
  };
}

/** Identity of a song independent of duration and cover updates. */
export function songId(t: TrackInfo | null): string {
  return t ? `${t.title}\u0000${t.artist}\u0000${t.album ?? ""}` : "";
}

export function formatMs(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}
