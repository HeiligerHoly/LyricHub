// Edit-mode geometry (PLAN.md 9.6): dragging with snap points, proportional scaling from the
// corners and width from the sides. All values in CSS px relative to the monitor. The backend
// turns the final box into anchor and offsets (overlay/window.rs, geometry_from_rect).
import { ANCHORS, type Anchor } from "$lib/shared/types";

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Bounds {
  /** Monitor size in CSS px */
  width: number;
  height: number;
  /** Distance of the snap points from the edges */
  margin: number;
}

/** Which snap column/row (0 left/top, 1 centre, 2 right/bottom) the box sits on, if any. */
export interface SnapState {
  col: number | null;
  row: number | null;
}

/** Draft state of the edit mode as the page keeps it. */
export interface EditChange {
  x: number;
  y: number;
  widthCss: number;
  scale: number;
}

export type Corner = "tl" | "tr" | "bl" | "br";
export type Side = "l" | "r";

export const SNAP_DISTANCE = 14;

/** Moves the box, snapping each axis to the edge margins or the centre line. */
export function moveWithSnap(start: Box, dx: number, dy: number, b: Bounds): { box: Box; snap: SnapState } {
  const [x, col] = snapAxis(start.x + dx, start.w, b.width, b.margin);
  const [y, row] = snapAxis(start.y + dy, start.h, b.height, b.margin);
  return { box: { ...start, x, y }, snap: { col, row } };
}

function snapAxis(pos: number, size: number, total: number, margin: number): [number, number | null] {
  const targets = [margin, (total - size) / 2, total - margin - size];
  let best: number | null = null;
  for (let i = 0; i < targets.length; i++) {
    const d = Math.abs(pos - targets[i]);
    if (d <= SNAP_DISTANCE && (best === null || d < Math.abs(pos - targets[best]))) best = i;
  }
  const snapped = best === null ? pos : targets[best];
  return [clamp(snapped, 0, Math.max(0, total - size)), best];
}

/** Snap state of a box that was placed by the backend (no drag yet). */
export function snapStateOf(box: Box, b: Bounds): SnapState {
  const near = (pos: number, size: number, total: number) => {
    const targets = [b.margin, (total - size) / 2, total - b.margin - size];
    const i = targets.findIndex((t) => Math.abs(pos - t) < 0.75);
    return i < 0 ? null : i;
  };
  return { col: near(box.x, box.w, b.width), row: near(box.y, box.h, b.height) };
}

export interface ScaleLimits {
  scale: number;
  widthCss: number;
  minScale: number;
  maxScale: number;
  minWidth: number;
  maxWidth: number;
}

/**
 * Scales the box proportionally from a corner; the opposite corner stays where it is. Text
 * and spacing follow the scale, so the height grows by the same factor as the width.
 */
export function scaleFromCorner(
  start: Box,
  corner: Corner,
  dx: number,
  dy: number,
  l: ScaleLimits,
): { box: Box; scale: number; widthCss: number } {
  const sx = corner === "tr" || corner === "br" ? 1 : -1;
  const sy = corner === "bl" || corner === "br" ? 1 : -1;
  const kx = (start.w + sx * dx) / start.w;
  const ky = (start.h + sy * dy) / start.h;
  let k = (kx + ky) / 2;
  k = clamp(k, Math.max(l.minScale / l.scale, l.minWidth / l.widthCss), Math.min(l.maxScale / l.scale, l.maxWidth / l.widthCss));
  const w = start.w * k;
  const h = start.h * k;
  const x = sx > 0 ? start.x : start.x + start.w - w;
  const y = sy > 0 ? start.y : start.y + start.h - h;
  return { box: { x, y, w, h }, scale: round(l.scale * k, 3), widthCss: l.widthCss * k };
}

/** Changes only the width from the left or right side; the other side stays. */
export function resizeWidth(start: Box, side: Side, dx: number, minWidth: number, maxWidth: number): Box {
  const w = clamp(start.w + (side === "r" ? dx : -dx), minWidth, maxWidth);
  const x = side === "r" ? start.x : start.x + start.w - w;
  return { ...start, x, w };
}

/** Keeps the box on the monitor. */
export function clampBox(box: Box, b: Bounds): Box {
  return {
    ...box,
    x: clamp(box.x, 0, Math.max(0, b.width - box.w)),
    y: clamp(box.y, 0, Math.max(0, b.height - box.h)),
  };
}

/** Anchor of the third that holds the box centre, for the toolbar readout. */
export function anchorAt(box: Box, b: Bounds): Anchor {
  const third = (c: number, total: number) => (c < total / 3 ? 0 : c > (total * 2) / 3 ? 2 : 1);
  return ANCHORS[third(box.y + box.h / 2, b.height)][third(box.x + box.w / 2, b.width)];
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

function round(v: number, digits: number): number {
  const f = 10 ** digits;
  return Math.round(v * f) / f;
}
