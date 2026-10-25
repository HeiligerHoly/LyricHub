// Themes of the overlay (PLAN.md 9.5) as CSS custom properties on the stage. Layouts and lines
// only read the variables: plate, text shades, accent, outline and glow.
//
//  Glass Dark (default)  translucent dark plate, white text, accent from the cover
//  Glass Light           translucent light plate, dark text
//  Neon                  no plate, outlined text, strong accent glow; for dark games
//  Mono                  high contrast black and white, no accent
//  Custom                plate, colours, outline and radius from the settings
//
// The layout "Minimal" always drops the plate and outlines the text (PLAN.md 9.2).
import { DEFAULT_ACCENT, type AppearanceSettings, type Layout } from "$lib/shared/types";
import { parseHex, rgba, toHsl, type Rgb } from "./color";

export interface ThemeLook {
  /** Plate colour without alpha; null draws no plate. */
  plate: Rgb | null;
  plateAlpha: number;
  radius: number;
  text: Rgb;
  outline: boolean;
  /** Accent used when the cover gives none (or accent is "fixed" without a colour). */
  fallbackAccent: Rgb;
  /** Accent follows the cover / fixed colour; false keeps the fallback (Mono). */
  accentAllowed: boolean;
  /** Extra glow around the filled text. */
  neon: boolean;
}

const WHITE: Rgb = [255, 255, 255];
const INK: Rgb = [17, 17, 17];
const YELLOW = parseHex(DEFAULT_ACCENT)!;
/** Amber that keeps 4.5:1 on a light plate. */
const AMBER: Rgb = [154, 103, 0];

export function look(a: AppearanceSettings, layout: Layout | undefined): ThemeLook {
  const custom = a.custom;
  let t: ThemeLook;
  switch (a.theme) {
    case "glass_light":
      t = { plate: WHITE, plateAlpha: a.plate_opacity, radius: 14, text: INK, outline: false, fallbackAccent: AMBER, accentAllowed: true, neon: false };
      break;
    case "neon":
      t = { plate: null, plateAlpha: 0, radius: 14, text: WHITE, outline: true, fallbackAccent: YELLOW, accentAllowed: true, neon: true };
      break;
    case "mono":
      // Black plate, at least 85 % opaque, white fill: contrast comes from brightness alone.
      t = { plate: [0, 0, 0], plateAlpha: Math.max(0.85, a.plate_opacity), radius: 10, text: WHITE, outline: false, fallbackAccent: WHITE, accentAllowed: false, neon: false };
      break;
    case "custom":
      t = {
        plate: custom.plate ? (parseHex(custom.plate_color) ?? [12, 12, 16]) : null,
        plateAlpha: a.plate_opacity,
        radius: custom.radius_px,
        text: parseHex(custom.text_color) ?? WHITE,
        outline: custom.outline,
        fallbackAccent: parseHex(a.accent_color) ?? YELLOW,
        accentAllowed: true,
        neon: false,
      };
      break;
    default:
      t = { plate: [12, 12, 16], plateAlpha: a.plate_opacity, radius: 14, text: WHITE, outline: false, fallbackAccent: YELLOW, accentAllowed: true, neon: false };
  }
  if (layout === "minimal") t = { ...t, plate: null, plateAlpha: 0, outline: true };
  return t;
}

/** What the accent is checked against: the plate, or a dark game behind outlined text. */
export function contrastGround(t: ThemeLook): Rgb {
  return t.plate && t.plateAlpha >= 0.5 ? t.plate : [0, 0, 0];
}

/** The accent to use before (or without) a cover colour. */
export function baseAccent(a: AppearanceSettings, t: ThemeLook): Rgb {
  if (!t.accentAllowed) return t.fallbackAccent;
  if (a.accent === "fixed") return parseHex(a.accent_color) ?? t.fallbackAccent;
  return t.fallbackAccent;
}

/** CSS custom properties for the stage. */
export function cssVars(t: ThemeLook, accent: Rgb, font: string): string {
  const light = toHsl(t.text)[2] < 0.5;
  // Outline: the stroke sits behind the fill (paint-order), so about half of it shows; the
  // tight shadow closes gaps on thin strokes, the wide one lifts the text off busy scenes.
  const halo = light ? "rgba(255, 255, 255, 0.75)" : "rgba(0, 0, 0, 0.8)";
  const textShadow = t.outline
    ? `0 0 calc(2px * var(--s)) ${halo}, 0 calc(2px * var(--s)) calc(8px * var(--s)) ${light ? "rgba(255, 255, 255, 0.6)" : "rgba(0, 0, 0, 0.7)"}`
    : "none";
  const neonGlow = `0 0 calc(10px * var(--s)) ${rgba(accent, 0.75)}`;
  const vars: Record<string, string> = {
    "--plate-bg": t.plate ? rgba(t.plate, t.plateAlpha) : "transparent",
    "--plate-radius": `${t.radius}px`,
    "--text": rgba(t.text, 0.92),
    "--text-meta": rgba(t.text, light ? 0.8 : 0.75),
    "--text-base": rgba(t.text, t.plate ? 0.55 : 0.72),
    "--text-next": rgba(t.text, t.plate ? 0.5 : 0.7),
    "--text-dim": rgba(t.text, 0.45),
    "--accent": rgba(accent, 1),
    "--accent-soft": rgba(accent, light ? 0.16 : 0.18),
    "--glow": rgba(accent, t.neon ? 0.85 : 0.6),
    "--text-shadow": textShadow,
    "--fill-shadow": t.neon ? (t.outline ? `${neonGlow}, ${textShadow}` : neonGlow) : textShadow,
    "--stroke": t.outline ? "calc(3.5px * var(--s))" : "0px",
    "--stroke-color": light ? "rgba(255, 255, 255, 0.9)" : "rgba(0, 0, 0, 0.9)",
  };
  if (font) vars["--font"] = `"${font}"`;
  return Object.entries(vars)
    .map(([k, v]) => `${k}: ${v}`)
    .join("; ");
}
