// Accent colour from the album cover (PLAN.md 9.5). The cover is drawn into a 32×32 canvas and
// reduced to four colours with k-means; the most saturated colour that covers a fair part of
// the cover wins. Its lightness is then moved until it has a contrast of at least 4.5:1 against
// the plate (lighter on dark plates, darker on light ones). Covers without a usable colour
// (black and white, or almost grey) give null, and the theme's fallback accent applies.
// Runs once per cover, never per frame.
import { contrast, fromHsl, toHsl, type Rgb } from "./color";

const SIZE = 32;
const K = 4;
const ROUNDS = 8;
/** A colour must cover this share of the cover to count. */
const MIN_SHARE = 0.06;
const MIN_SATURATION = 0.22;
const MIN_CONTRAST = 4.5;
/** Spotify's brand green is not copied exactly (branding rules): hues this close move away. */
const SPOTIFY_HUE = 141;
const SPOTIFY_HUE_GAP = 8;

const cache = new Map<string, Rgb | null>();

/** Accent for `url` (a data: URL) against a plate colour, or null for the fallback. */
export async function coverAccent(url: string, plate: Rgb): Promise<Rgb | null> {
  const key = `${url.length}:${url.slice(-64)}:${plate.join(",")}`;
  if (cache.has(key)) return cache.get(key) ?? null;
  const pixels = await samplePixels(url);
  const picked = pixels ? pick(pixels) : null;
  const accent = picked ? readable(picked, plate) : null;
  if (cache.size > 32) cache.clear();
  cache.set(key, accent);
  return accent;
}

async function samplePixels(url: string): Promise<Rgb[] | null> {
  try {
    const img = new Image();
    img.decoding = "async";
    img.src = url;
    await img.decode();
    const canvas = new OffscreenCanvas(SIZE, SIZE);
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(img, 0, 0, SIZE, SIZE);
    const data = ctx.getImageData(0, 0, SIZE, SIZE).data;
    const out: Rgb[] = [];
    for (let i = 0; i < data.length; i += 4) {
      if (data[i + 3] > 128) out.push([data[i], data[i + 1], data[i + 2]]);
    }
    return out.length ? out : null;
  } catch {
    return null;
  }
}

/** k-means with K centres, seeded along the lightness range; the most saturated big cluster. */
export function pick(pixels: Rgb[]): Rgb | null {
  const sorted = [...pixels].sort((a, b) => a[0] + a[1] + a[2] - (b[0] + b[1] + b[2]));
  let centres: Rgb[] = Array.from({ length: K }, (_, i) => sorted[Math.floor(((i + 0.5) / K) * sorted.length)]);
  let assign = new Array<number>(pixels.length).fill(0);
  for (let round = 0; round < ROUNDS; round++) {
    assign = pixels.map((p) => nearest(p, centres));
    centres = centres.map((c, k) => {
      let n = 0;
      const sum: Rgb = [0, 0, 0];
      pixels.forEach((p, i) => {
        if (assign[i] !== k) return;
        n++;
        sum[0] += p[0];
        sum[1] += p[1];
        sum[2] += p[2];
      });
      return n ? [sum[0] / n, sum[1] / n, sum[2] / n] : c;
    });
  }
  let best: Rgb | null = null;
  let bestScore = 0;
  centres.forEach((c, k) => {
    const share = assign.filter((a) => a === k).length / pixels.length;
    const [, s, l] = toHsl(c);
    // Very dark or very light clusters carry little colour even with a high saturation value.
    const vivid = s * (1 - Math.abs(2 * l - 1));
    if (share < MIN_SHARE || s < MIN_SATURATION || vivid <= bestScore) return;
    best = c;
    bestScore = vivid;
  });
  return best;
}

function nearest(p: Rgb, centres: Rgb[]): number {
  let best = 0;
  let bestD = Infinity;
  centres.forEach((c, k) => {
    const d = (p[0] - c[0]) ** 2 + (p[1] - c[1]) ** 2 + (p[2] - c[2]) ** 2;
    if (d < bestD) {
      bestD = d;
      best = k;
    }
  });
  return best;
}

/** The colour with its lightness moved until it reads on the plate. */
export function readable(c: Rgb, plate: Rgb): Rgb | null {
  let [h, s, l] = toHsl(c);
  if (Math.abs(h - SPOTIFY_HUE) < SPOTIFY_HUE_GAP) h = h < SPOTIFY_HUE ? SPOTIFY_HUE - SPOTIFY_HUE_GAP : SPOTIFY_HUE + SPOTIFY_HUE_GAP;
  s = Math.max(s, 0.45);
  const darkPlate = toHsl(plate)[2] < 0.5;
  if (darkPlate) l = Math.max(l, 0.6);
  for (let i = 0; i < 40; i++) {
    const rgb = fromHsl(h, s, l);
    if (contrast(rgb, plate) >= MIN_CONTRAST) return rgb;
    l = darkPlate ? l + 0.01 : l - 0.01;
    if (l <= 0.05 || l >= 0.95) break;
  }
  return null;
}
