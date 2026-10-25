import { invoke } from "@tauri-apps/api/core";

// Mirrors the Rust structs in src-tauri/src/overlay/mod.rs, smtc.rs and overlay/selftest.rs.

/** Snapshot of the overlay window's Win32 state, used by Spike B checks. */
export interface OverlayDiagnostics {
  visible: boolean;
  click_through: boolean;
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

/** One Spotify media-session snapshot read via the Rust WinRT path. */
export interface NowPlaying {
  app_id: string;
  title: string;
  artist: string;
  album: string;
  status: string;
  position_ms: number;
  position_now_ms: number;
  end_ms: number;
  last_update_age_ms: number;
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
  fake_game_present: boolean;
  passed: number;
  failed: number;
  steps: SelfTestStep[];
}

export const ipc = {
  overlayDiagnostics: () => invoke<OverlayDiagnostics>("overlay_diagnostics"),
  toggleOverlay: () => invoke<boolean>("toggle_overlay"),
  setClickThrough: (enabled: boolean) => invoke<OverlayDiagnostics>("set_click_through", { enabled }),
  nowPlaying: () => invoke<NowPlaying | null>("now_playing_snapshot"),
  runOverlaySelfTest: () => invoke<SelfTestReport>("run_overlay_selftest"),
};

export function formatMs(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}
