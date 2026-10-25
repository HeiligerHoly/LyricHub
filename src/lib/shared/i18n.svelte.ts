// Interface language for the settings window and the overlay (PLAN.md 9.7, "Sprache"). Texts live
// in locales/de.ts (the reference: every key exists there) and locales/en.ts. `t()` reads the
// current language from reactive state, so templates update when the setting changes. The
// backend translates what it draws itself (tray, notices, errors; src-tauri/src/i18n.rs).
import type { Language } from "./types";
import { de, type Key } from "./locales/de";
import { en } from "./locales/en";

export type Lang = "de" | "en";
export type { Key };

const dicts: Record<Lang, Record<Key, string>> = { de, en };

let current = $state<Lang>("de");

/** Applies the setting; "system" follows the language WebView2 reports for Windows. */
export function setLanguage(setting: Language | undefined): void {
  current = setting === "de" || setting === "en" ? setting : navigator.language.toLowerCase().startsWith("de") ? "de" : "en";
  document.documentElement.lang = current;
}

export function lang(): Lang {
  return current;
}

/** Text for `key`; `{name}` placeholders are filled from `params`. */
export function t(key: Key, params?: Record<string, string | number>): string {
  const text = dicts[current][key] ?? de[key];
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (all, name: string) => (name in params ? String(params[name]) : all));
}

/** Number with the decimal separator of the current language. */
export function num(value: number, digits = 0): string {
  return value.toLocaleString(current === "de" ? "de-DE" : "en-US", { minimumFractionDigits: digits, maximumFractionDigits: digits });
}
