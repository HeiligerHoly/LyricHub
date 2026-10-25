// Helpers to find lines in time and to pick the script of a line.
import type { Line } from "$lib/shared/types";

/** Index of the line shown at `ms` (last line that has started), -1 before the first one. */
export function lineIndexAt(lines: Line[], ms: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].start_ms <= ms) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return found;
}

/** The next line with text after `index`, for the preview row. */
export function nextTextLine(lines: Line[], index: number): Line | null {
  for (let i = index + 1; i < lines.length; i++) {
    if (lines[i].text) return lines[i];
  }
  return null;
}

// Hiragana, katakana, CJK ideographs, half-width katakana.
const JAPANESE = /[぀-ヿ㐀-䶿一-鿿ｦ-ﾟ]/;

/**
 * "ja" for lines with kana or kanji, so WebView2 picks Japanese glyph forms for Han characters
 * (PLAN.md 9.5). Japanese is the user's third lyrics language; Chinese-only lines are rare.
 */
export function langOf(text: string): string | undefined {
  return JAPANESE.test(text) ? "ja" : undefined;
}
