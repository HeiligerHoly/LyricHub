// Caption cues from a scene's voice-over text. Plain TypeScript so Node scripts can import it
// (scripts/build-script-md.ts lists the cue times) and Remotion bundles it (src/ui/Captions.tsx).
// Each sentence becomes one cue, or several when it is longer than MAX_WORDS; the breaks are
// chosen to land after commas or before a new clause and to keep the pieces about equal.
// Cues are timed by word count across the scene, so they follow a VO read at an even pace.
import { CAPTION_LEAD_SEC } from "../data/scenes.ts";

const MAX_WORDS = 13;
const CLAUSE_START = new Set(["when", "and", "but", "or", "so", "because", "while", "which", "where", "if", "then", "until", "even"]);
const WEAK_END = new Set(["a", "an", "the", "of", "to", "in", "on", "for", "with", "at", "by", "from", "and", "or", "your", "its", "is", "are", "every"]);

/** Cost of a break after word i-1 (before word i). */
function breakCost(words: string[], i: number): number {
  const last = words[i - 1];
  const next = words[i].toLowerCase();
  let cost = /[,;:]$/.test(last) ? 0 : CLAUSE_START.has(next) ? 50 : 100;
  if (WEAK_END.has(last.toLowerCase().replace(/[^a-z]/g, ""))) cost += 60;
  if (next === "of") cost += 60;
  return cost;
}

function splitSentence(sentence: string): string[] {
  const words = sentence.split(/\s+/);
  const n = words.length;
  if (n <= MAX_WORDS) return [sentence];
  // dp[i]: cheapest way to cue words[0..i); a cue costs 100 plus its length squared
  const dp = new Array<number>(n + 1).fill(Infinity);
  const from = new Array<number>(n + 1).fill(0);
  dp[0] = 0;
  for (let i = 1; i <= n; i++) {
    for (let j = Math.max(0, i - MAX_WORDS); j < i; j++) {
      const len = i - j;
      const cost = dp[j] + 100 + len * len + (len < 4 ? 300 : 0) + (i < n ? breakCost(words, i) : 0);
      if (cost < dp[i]) {
        dp[i] = cost;
        from[i] = j;
      }
    }
  }
  const cues: string[] = [];
  for (let i = n; i > 0; i = from[i]) cues.unshift(words.slice(from[i], i).join(" "));
  return cues;
}

export function splitCues(text: string): string[] {
  const sentences = text.match(/[^.!?]+[.!?]+["”]?|[^.!?]+$/g) ?? [text];
  return sentences.map((s) => s.trim()).filter(Boolean).flatMap(splitSentence);
}

export type Cue = { text: string; startSec: number; endSec: number };

/** Cue texts with their start and end in seconds for a scene of the given length. */
export function cueTimes(text: string, durSec: number): Cue[] {
  const cues = splitCues(text);
  const counts = cues.map((c) => c.split(/\s+/).length);
  const total = counts.reduce((a, b) => a + b, 0) || 1;
  const usable = durSec - CAPTION_LEAD_SEC - 0.6;
  let start = CAPTION_LEAD_SEC;
  return cues.map((c, i) => {
    const len = (usable * counts[i]) / total;
    const cue = { text: c, startSec: start, endSec: start + len };
    start += len;
    return cue;
  });
}
