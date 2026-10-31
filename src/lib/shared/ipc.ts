// Typed wrappers around Tauri invoke/listen (PLAN.md 4.4). Command arguments are camelCase on
// this side; Tauri maps them to the snake_case Rust parameters. In a plain browser (the OBS
// browser source, src-tauri/src/stream.rs) the overlay reads the same data over HTTP instead.
import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";
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
  LastCrash,
  LatencySample,
  LyricsCandidates,
  LyricsEvent,
  Monitor,
  NudgeEvent,
  NowPlaying,
  OverlayDiagnostics,
  OverlayLayout,
  PlaybackEvent,
  PlayerSession,
  PluginInfo,
  SelfTestReport,
  Settings,
  SettingsPatch,
  SyncLab,
  TrackInfo,
  ZipInfo,
} from "./types";

/** No Tauri: the page runs as the OBS browser source, served by LyricHub itself. */
const web = typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window);
/** What the browser source can read; everything else concerns the desktop window and is a no-op. */
const WEB_READS = new Set(["get_now_playing", "get_lyrics", "get_clock", "get_settings", "get_overlay_layout"]);

function call<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  if (!web) return tauriInvoke<T>(cmd, args);
  if (!WEB_READS.has(cmd)) return Promise.resolve(null as T);
  // The URL carries the token, and optionally `width` and `scale` for the layout.
  const params = new URLSearchParams(location.search);
  if (cmd === "get_overlay_layout" && !params.has("width")) params.set("width", String(window.innerWidth));
  return fetch(`/api/${cmd}?${params}`).then((r) => r.json() as Promise<T>);
}

export const ipc = {
  getSettings: () => call<Settings>("get_settings"),
  setSettings: (patch: SettingsPatch) => call<Settings>("set_settings", { patch }),
  setOverlayAnchor: (anchor: Anchor) => call<Settings>("set_overlay_anchor", { anchor }),
  resetOverlayGeometry: () => call<Settings>("reset_overlay_geometry"),
  listMonitors: () => call<Monitor[]>("list_monitors"),
  listPlayers: () => call<PlayerSession[]>("list_players"),
  listPlugins: () => call<PluginInfo[]>("list_plugins"),
  /** The ZIP goes as the raw request body, not as JSON. */
  checkPluginZip: (zip: Uint8Array) => call<ZipInfo>("check_plugin_zip", zip),
  installStagedPlugin: () => call<PluginInfo[]>("install_staged_plugin"),
  discardStagedPlugin: () => call<void>("discard_staged_plugin"),
  setPluginEnabled: (id: string, enabled: boolean) => call<PluginInfo[]>("set_plugin_enabled", { id, enabled }),
  setPluginSetting: (id: string, key: string, value: boolean) => call<PluginInfo[]>("set_plugin_setting", { id, key, value }),
  removePlugin: (id: string) => call<PluginInfo[]>("remove_plugin", { id }),
  getNowPlaying: () => call<NowPlaying>("get_now_playing"),
  trackLatencies: () => call<LatencySample[]>("track_latencies"),
  getLyrics: () => call<LyricsEvent | null>("get_lyrics"),
  getClock: () => call<ClockEvent>("get_clock"),
  nudgeOffset: (deltaMs: number) => call<number | null>("nudge_offset", { deltaMs }),
  setSongOffset: (offsetMs: number) => call<number | null>("set_song_offset", { offsetMs }),
  listAudioDevices: () => call<AudioDevice[]>("list_audio_devices"),
  setDeviceOffset: (id: string, offsetMs: number) => call<AudioDevice[]>("set_device_offset", { id, offsetMs }),
  startCalibration: (mode: CalibrationMode) => call<CalibrationView>("start_calibration", { mode }),
  stopCalibration: () => call<CalibrationView | null>("stop_calibration"),
  applyCalibration: () => call<number>("apply_calibration"),
  getCalibration: () => call<CalibrationView | null>("get_calibration"),
  clockApplied: (seq: number) => call<void>("clock_applied", { seq }),
  syncLab: () => call<SyncLab>("sync_lab"),
  exportDiagnostics: () => call<string>("export_diagnostics"),
  reloadLyrics: () => call<void>("reload_lyrics"),
  discardPluginLyrics: () => call<void>("discard_plugin_lyrics"),
  lyricsCandidates: () => call<LyricsCandidates>("lyrics_candidates"),
  /** "lrclib" with an entry's id, "none" for no lyrics, "auto" back to the automatic search. */
  chooseLyrics: (mode: "lrclib" | "none" | "auto", lrclibId: number | null = null) =>
    call<void>("choose_lyrics", { mode, lrclibId }),
  openLrcFolder: () => call<void>("open_lrc_folder"),
  ownLyrics: () => call<string | null>("own_lyrics"),
  saveOwnLyrics: (text: string) => call<void>("save_own_lyrics", { text }),
  deleteOwnLyrics: () => call<void>("delete_own_lyrics"),
  getOverlayLayout: () => call<OverlayLayout | null>("get_overlay_layout"),
  reportOverlaySize: (width: number, height: number, uiScale: number) =>
    call<void>("report_overlay_size", { width, height, uiScale }),
  overlayLayoutReady: (seq: number) => call<void>("overlay_layout_ready", { seq }),
  overlayRendered: (key: string) => call<void>("overlay_rendered", { key }),
  enterEditMode: () => call<void>("enter_edit_mode"),
  overlayEditUpdate: (draft: EditDraft) => call<void>("overlay_edit_update", { draft }),
  overlayEditMonitor: (step: number) => call<void>("overlay_edit_monitor", { step }),
  overlayEditForProgram: (on: boolean) => call<void>("overlay_edit_for_program", { on }),
  saveOverlayGeometry: (draft: EditDraft | null) => call<void>("save_overlay_geometry", { draft }),
  cancelEditMode: () => call<void>("cancel_edit_mode"),
  toggleOverlay: () => call<boolean>("toggle_overlay"),
  setOverlayPreview: (on: boolean) => call<void>("set_overlay_preview", { on }),
  listFonts: () => call<FontList>("list_fonts"),
  hotkeyStatus: () => call<HotkeyStatus[]>("hotkey_status"),
  suspendHotkeys: (suspended: boolean) => call<HotkeyStatus[]>("suspend_hotkeys", { suspended }),
  overlayDiagnostics: () => call<OverlayDiagnostics>("overlay_diagnostics"),
  runOverlaySelfTest: () => call<SelfTestReport>("run_overlay_selftest"),
  appInfo: () => call<AppInfo>("app_info"),
  lastCrash: () => call<LastCrash | null>("last_crash"),
  openLogsFolder: () => call<void>("open_logs_folder"),
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
  plugins: PluginInfo[];
}

export function on<E extends keyof EventPayloads>(
  event: E,
  handler: (payload: EventPayloads[E]) => void,
): Promise<UnlistenFn> {
  if (web) return webOn(event, handler);
  return listen<EventPayloads[E]>(event, (e) => handler(e.payload));
}

let source: EventSource | null = null;

/** Events of the browser source, from the server's event stream. */
function webOn<E extends keyof EventPayloads>(event: E, handler: (payload: EventPayloads[E]) => void): Promise<UnlistenFn> {
  const token = new URLSearchParams(location.search).get("token") ?? "";
  source ??= new EventSource(`/api/events?token=${encodeURIComponent(token)}`);
  const es = source;
  if (event === "overlay-layout") {
    // The server has no window to lay out: the layout follows the settings and the source's size.
    const relayout = () => call<EventPayloads[E] | null>("get_overlay_layout").then((l) => l && handler(l));
    es.addEventListener("settings", relayout);
    window.addEventListener("resize", relayout);
    return Promise.resolve(() => {
      es.removeEventListener("settings", relayout);
      window.removeEventListener("resize", relayout);
    });
  }
  const listener = (e: MessageEvent) => handler(JSON.parse(e.data) as EventPayloads[E]);
  es.addEventListener(event, listener as EventListener);
  return Promise.resolve(() => es.removeEventListener(event, listener as EventListener));
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
