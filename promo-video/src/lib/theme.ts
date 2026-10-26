// Overlay look, copied from LyricHub's real styles (see FEATURES.md section 8 and 14):
// src/lib/overlay/theme.ts, src/lib/overlay/Overlay.svelte, layouts/*.svelte.

export type Rgb = [number, number, number];
export type ThemeId = "glass_dark" | "glass_light" | "neon" | "mono" | "custom";

export type ThemeLook = {
  plate: Rgb | null;
  plateAlpha: number;
  radius: number;
  text: Rgb;
  outline: boolean;
  fallbackAccent: Rgb;
  accentAllowed: boolean;
  neon: boolean;
  /** true when the text is dark (Glass Light): halos turn white. */
  darkText: boolean;
};

export const WHITE: Rgb = [255, 255, 255];
export const INK: Rgb = [17, 17, 17];
export const YELLOW: Rgb = [255, 211, 110]; // #ffd36e, DEFAULT_ACCENT
export const AMBER: Rgb = [154, 103, 0]; // #9a6700, Glass Light accent

export const DEFAULT_PLATE_OPACITY = 0.55;

export function themeLook(id: ThemeId, plateOpacity = DEFAULT_PLATE_OPACITY): ThemeLook {
  switch (id) {
    case "glass_light":
      return { plate: WHITE, plateAlpha: Math.max(0.7, plateOpacity), radius: 14, text: INK, outline: false, fallbackAccent: AMBER, accentAllowed: true, neon: false, darkText: true };
    case "neon":
      return { plate: null, plateAlpha: 0, radius: 14, text: WHITE, outline: true, fallbackAccent: YELLOW, accentAllowed: true, neon: true, darkText: false };
    case "mono":
      return { plate: [0, 0, 0], plateAlpha: Math.max(0.85, plateOpacity), radius: 10, text: WHITE, outline: false, fallbackAccent: WHITE, accentAllowed: false, neon: false, darkText: false };
    case "custom":
      // Invented custom colours for the demo: deep blue plate, ice text, fixed cyan accent.
      return { plate: [16, 32, 54], plateAlpha: plateOpacity, radius: 22, text: [230, 247, 255], outline: false, fallbackAccent: [110, 231, 255], accentAllowed: true, neon: false, darkText: false };
    case "glass_dark":
    default:
      return { plate: [12, 12, 16], plateAlpha: plateOpacity, radius: 14, text: WHITE, outline: false, fallbackAccent: YELLOW, accentAllowed: true, neon: false, darkText: false };
  }
}

export const rgba = (c: Rgb, a: number): string => `rgba(${c[0]},${c[1]},${c[2]},${a})`;

export type OverlayVars = {
  plateBg: string;
  radius: number;
  text: string;
  meta: string;
  base: string;
  next: string;
  accent: string;
  accentSoft: string;
  glow: string;
  textShadow: string;
  fillShadow: string;
  stroke: number;
  strokeColor: string;
};

/** CSS values per theme, following theme.ts cssVars() at scale s. */
export function overlayVars(t: ThemeLook, accent: Rgb, s: number, forceNoPlate = false): OverlayVars {
  const hasPlate = !!t.plate && !forceNoPlate;
  const outline = t.outline || forceNoPlate;
  const halo = t.darkText
    ? `0 0 ${2 * s}px rgba(255,255,255,.75), 0 ${2 * s}px ${8 * s}px rgba(255,255,255,.6)`
    : `0 0 ${2 * s}px rgba(0,0,0,.8), 0 ${2 * s}px ${8 * s}px rgba(0,0,0,.7)`;
  return {
    plateBg: hasPlate && t.plate ? rgba(t.plate, t.plateAlpha) : "transparent",
    radius: t.radius * s,
    text: rgba(t.text, 0.92),
    meta: rgba(t.text, t.darkText ? 0.8 : 0.75),
    base: rgba(t.text, hasPlate ? 0.55 : 0.72),
    next: rgba(t.text, hasPlate ? 0.5 : 0.7),
    accent: rgba(accent, 1),
    accentSoft: rgba(accent, t.darkText ? 0.16 : 0.18),
    glow: rgba(accent, t.neon ? 0.85 : 0.6),
    textShadow: halo,
    fillShadow: t.neon ? `0 0 ${10 * s}px ${rgba(accent, 0.75)}, ${halo}` : halo,
    stroke: outline ? 3.5 * s : 0,
    strokeColor: t.darkText ? "rgba(255,255,255,.9)" : "rgba(0,0,0,.9)",
  };
}

/** Overlay font stack from Overlay.svelte. */
export const OVERLAY_FONT =
  '"Inter Variable", "Segoe UI Variable", "Segoe UI", "Yu Gothic UI", "Meiryo UI", "Noto Sans JP", sans-serif';

/** Settings window palette (src/lib/settings/App.svelte). */
export const SETTINGS = {
  bg: "#111318",
  sidebar: "#0d0f13",
  card: "#1a1d24",
  line: "#2a2f3a",
  text: "#e8eaf0",
  muted: "#9aa1b2",
  accent: "#ffd36e",
  accentInk: "#1a1400",
  ok: "#4ade80",
  fail: "#f87171",
  button: "#262b36",
  input: "#12151b",
  font: '"Segoe UI Variable", "Segoe UI", sans-serif',
};

/** Promo graphics palette (not part of the app UI). */
export const PROMO = {
  bg: "#0b0d14",
  panel: "#141824",
  line: "#252b3b",
  text: "#eef1f7",
  muted: "#a3abbe",
  accent: "#ffd36e",
  accentInk: "#1a1400",
  ok: "#4ade80",
  font: '"Inter Variable", "Segoe UI Variable", "Segoe UI", sans-serif',
};
