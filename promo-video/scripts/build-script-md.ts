// Writes promo-video/script.md from src/data/scenes.ts and checks the pacing.
// Run: npm run script:md   (Node 22+ strips the TypeScript types itself)
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { SCENES, WPM, voSeconds, wordCount } from "../src/data/scenes.ts";

const here = dirname(fileURLToPath(import.meta.url));
const fmt = (sec: number): string => {
  const m = Math.floor(sec / 60);
  const s = sec - m * 60;
  return `${m}:${s.toFixed(1).padStart(4, "0")}`;
};

let t = 0;
let words = 0;
const blocks: string[] = [];
const warnings: string[] = [];
for (const s of SCENES) {
  const start = t;
  const end = t + s.durationSec;
  const need = voSeconds(s);
  const slack = s.durationSec - need;
  if (slack < 1.5) warnings.push(`${s.id}: only ${slack.toFixed(1)} s slack for ${need.toFixed(1)} s of voice-over`);
  words += wordCount(s.vo);
  blocks.push(
    [
      `## ${s.id} · ${s.title}`,
      "",
      `**Time:** ${fmt(start)} – ${fmt(end)} (${s.durationSec} s) · **Words:** ${wordCount(s.vo)} (≈ ${need.toFixed(1)} s at ${WPM} wpm)`,
      "",
      "**Voice-over:**",
      "",
      `> ${s.vo}`,
      "",
      "**On-screen text:**",
      "",
      ...s.onScreen.map((o) => `- ${o}`),
      "",
    ].join("\n"),
  );
  t = end;
}

const md = [
  "# LyricHub promo · voice-over script",
  "",
  "Generated from `src/data/scenes.ts` by `npm run script:md`. Do not edit by hand; change the scene file and regenerate.",
  "",
  `Total runtime: **${fmt(t)}** (${t} s) · ${words} words · pacing target ${WPM} words per minute.`,
  "",
  "Reading notes:",
  "",
  '- Read the hotkeys as written: "Control, Alt, L" and so on, with short pauses between the keys.',
  '- "romaji" is pronounced ROH-mah-jee. "LRCLIB" is not spoken.',
  "- One file per scene works best: name each file after the scene id (for example `s06-hotkeys.mp3`) and drop it into `public/vo/`. See README.md.",
  "",
  ...blocks,
].join("\n");

writeFileSync(join(here, "..", "script.md"), md, "utf8");
console.log(`script.md written: ${SCENES.length} scenes, ${fmt(t)}, ${words} words`);
for (const w of warnings) console.log(`WARNING ${w}`);
