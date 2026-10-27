// Word-by-word karaoke fill (PLAN.md 5.6, 9.3). The backend sends word times per line: real
// ones from Enhanced LRC, else an estimate by letter count (lyrics/words.rs). Here they become
// keyframe stops for the fill: at each word's start the fill stands at the word's left edge, at
// its end at its right edge. The fill still runs as one compositor animation per layer.
import type { Word } from "#lib/shared/types.js";

/** Share of the line's fill time (`offset`) and share of the text width (`ratio`). */
export interface FillStop {
  offset: number;
  ratio: number;
}

/** Character ranges of the words in `text`, in order; null if a word is not found. */
export function charRanges(text: string, words: Word[]): Array<[number, number]> | null {
  const ranges: Array<[number, number]> = [];
  let cursor = 0;
  for (const w of words) {
    const at = text.indexOf(w.text, cursor);
    if (at < 0 || !w.text) return null;
    ranges.push([at, at + w.text.length]);
    cursor = at + w.text.length;
  }
  return ranges;
}

/**
 * Stops from line start (nothing filled) to fill end (all filled). `edges[i]` holds the left and
 * right edge of word i as a share of the text width. Offsets and ratios never go back, so
 * overlapping or out-of-order word times cannot make the fill run backwards.
 */
export function fillStops(words: Word[], edges: Array<[number, number]>, startMs: number, fillEndMs: number): FillStop[] {
  const duration = Math.max(1, fillEndMs - startMs);
  const stops: FillStop[] = [{ offset: 0, ratio: 0 }];
  const add = (ms: number, ratio: number) => {
    const last = stops[stops.length - 1];
    const offset = Math.min(1, Math.max(last.offset, (ms - startMs) / duration));
    stops.push({ offset, ratio: Math.min(1, Math.max(last.ratio, ratio)) });
  };
  words.forEach((w, i) => {
    add(w.start_ms, edges[i][0]);
    add(w.end_ms, edges[i][1]);
  });
  add(fillEndMs, 1);
  return stops;
}

/** Text split into word and gap pieces, for the glow layer. */
export function pieces(text: string, ranges: Array<[number, number]>): Array<{ text: string; word: number | null }> {
  const out: Array<{ text: string; word: number | null }> = [];
  let cursor = 0;
  ranges.forEach(([from, to], i) => {
    if (from > cursor) out.push({ text: text.slice(cursor, from), word: null });
    out.push({ text: text.slice(from, to), word: i });
    cursor = to;
  });
  if (cursor < text.length) out.push({ text: text.slice(cursor), word: null });
  return out;
}
