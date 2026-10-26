// Renders check stills for every scene (or the scene ids given as arguments) into out/stills/.
// Run: npm run stills            all scenes, 3 stills each
//      npm run stills s06-hotkeys s07-edit-mode
//      node scripts/stills.ts --at=0.1,0.3 s05-out-of-the-way   custom positions (fractions)
//      node scripts/stills.ts s05-out-of-the-way@0.1,0.3 s07-edit-mode@0.6   positions per scene
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { bundle } from "@remotion/bundler";
import { renderStill, selectComposition } from "@remotion/renderer";
import { SCENES } from "../src/data/scenes.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "out", "stills");
mkdirSync(outDir, { recursive: true });

const args = process.argv.slice(2);
const atArg = args.find((a) => a.startsWith("--at="));
const fractions = atArg ? atArg.slice(5).split(",").map(Number) : [0.15, 0.5, 0.85];
// "id" or "id@0.1,0.3"
const picks = new Map<string, number[]>();
for (const a of args.filter((x) => !x.startsWith("--"))) {
  const [id, at] = a.split("@");
  picks.set(id, at ? at.split(",").map(Number) : fractions);
}
const scenes = picks.size ? SCENES.filter((s) => picks.has(s.id)) : SCENES;

const serveUrl = await bundle({ entryPoint: join(root, "src", "index.ts") });
for (const s of scenes) {
  const composition = await selectComposition({ serveUrl, id: s.id });
  for (const f of picks.get(s.id) ?? fractions) {
    const frame = Math.min(composition.durationInFrames - 1, Math.round(f * composition.durationInFrames));
    const output = join(outDir, `${s.id}-${String(Math.round(f * 100)).padStart(2, "0")}.png`);
    await renderStill({ composition, serveUrl, output, frame });
    console.log(output);
  }
}
