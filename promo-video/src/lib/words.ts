// Word-by-word karaoke fill, modelled on LyricHub:
// - word times estimated like src-tauri/src/lyrics/words.rs (60 ms lead-in, 120 ms tail,
//   weight 1 per letter or kana, 1.5 per kanji, 0 for punctuation),
// - keyframes at each word's left edge (its start) and right edge (its end) like
//   src/lib/overlay/wordFill.ts,
// - quantised to 30 updates per second of song time like KaraokeLine.svelte (steps()).

const LEAD = 0.06;
const TAIL = 0.12;
export const FILL_UPDATES_PER_SEC = 30;

const isKanji = (ch: string) => /[㐀-䶿一-鿿]/.test(ch);
const isWordChar = (ch: string) => /[\p{L}\p{N}]/u.test(ch);

function weight(word: string): number {
  let w = 0;
  for (const ch of word) w += isKanji(ch) ? 1.5 : isWordChar(ch) ? 1 : 0;
  return w;
}

type Token = { text: string; start: number; end: number };

/** Words with their character ranges; CJK text without spaces is split per character. */
function tokenize(text: string): Token[] {
  const tokens: Token[] = [];
  const cjk = /[぀-ヿ㐀-鿿]/.test(text) && !/\s/.test(text.trim());
  if (cjk) {
    let i = 0;
    for (const ch of text) {
      tokens.push({ text: ch, start: i, end: i + ch.length });
      i += ch.length;
    }
    return tokens;
  }
  const re = /\S+/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) tokens.push({ text: m[0], start: m.index, end: m.index + m[0].length });
  return tokens;
}

let ctx: CanvasRenderingContext2D | null = null;
function measure(text: string, font: string): number {
  if (typeof document === "undefined") return text.length;
  if (!ctx) ctx = document.createElement("canvas").getContext("2d");
  if (!ctx) return text.length;
  ctx.font = font;
  return ctx.measureText(text).width;
}

export type FillTrack = { times: number[]; fracs: number[] };

/** Keyframes (song time → fill fraction of the text width) for one line. */
export function buildFillTrack(text: string, font: string, lineStart: number, lineEnd: number): FillTrack {
  const tokens = tokenize(text);
  const total = measure(text, font) || 1;
  const weights = tokens.map((t) => Math.max(weight(t.text), 0.0001));
  const sum = weights.reduce((a, b) => a + b, 0);
  const a = lineStart + LEAD;
  const b = Math.max(a + 0.05, lineEnd - TAIL);
  const times: number[] = [lineStart];
  const fracs: number[] = [0];
  let t = a;
  tokens.forEach((tok, i) => {
    const left = measure(text.slice(0, tok.start), font) / total;
    const right = measure(text.slice(0, tok.end), font) / total;
    const dur = ((b - a) * weights[i]) / sum;
    times.push(t, t + dur);
    fracs.push(left, right);
    t += dur;
  });
  times.push(lineEnd);
  fracs.push(1);
  return { times, fracs };
}

/** Fill fraction at song time `t`, stepped at 30 updates per second like the app. */
export function fillAt(track: FillTrack, t: number): number {
  const start = track.times[0];
  const tq = start + Math.floor((t - start) * FILL_UPDATES_PER_SEC) / FILL_UPDATES_PER_SEC;
  const { times, fracs } = track;
  if (tq <= times[0]) return 0;
  for (let i = 1; i < times.length; i++) {
    if (tq <= times[i]) {
      const span = times[i] - times[i - 1];
      const k = span <= 0 ? 1 : (tq - times[i - 1]) / span;
      return fracs[i - 1] + (fracs[i] - fracs[i - 1]) * k;
    }
  }
  return 1;
}
