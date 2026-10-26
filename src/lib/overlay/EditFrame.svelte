<script lang="ts">
  // Edit-mode chrome (PLAN.md 9.6): dashed frame with drag area, corner handles (scale) and
  // side handles (width), nine snap markers on the monitor and a toolbar. The overlay window
  // covers the whole monitor while this is shown, so clientX/Y are monitor coordinates.
  import { num, t } from "$lib/shared/i18n.svelte";
  import type { EditLayout } from "$lib/shared/types";
  import { MIN_WIDTH_CSS, SCALE_MAX, SCALE_MIN } from "$lib/shared/types";
  import {
    anchorAt,
    clampBox,
    moveWithSnap,
    resizeWidth,
    scaleFromCorner,
    snapStateOf,
    type Bounds,
    type Box,
    type Corner,
    type EditChange,
    type Side,
    type SnapState,
  } from "./editMode";

  interface Props {
    edit: EditLayout;
    box: Box;
    scale: number;
    onchange: (next: EditChange) => void;
    oncommit: () => void;
    onsave: () => void;
    oncancel: () => void;
    onmonitor: () => void;
  }
  let { edit, box, scale, onchange, oncommit, onsave, oncancel, onmonitor }: Props = $props();

  type Op =
    | { kind: "move" }
    | { kind: "corner"; corner: Corner }
    | { kind: "side"; side: Side };
  let active = $state<{ op: Op; px: number; py: number; start: Box; scale: number } | null>(null);
  let dragSnap = $state<SnapState | null>(null);

  const bounds = $derived<Bounds>({ width: edit.monitor_width, height: edit.monitor_height, margin: edit.snap_margin });
  const snap = $derived(dragSnap ?? snapStateOf(box, bounds));
  const markerX = $derived([bounds.margin, bounds.width / 2, bounds.width - bounds.margin]);
  const markerY = $derived([bounds.margin, bounds.height / 2, bounds.height - bounds.margin]);
  const toolbarTop = $derived(box.y + box.h / 2 < bounds.height / 2 ? bounds.height * 0.62 : bounds.height * 0.24);
  const uiFactor = $derived(Math.min(1.5, Math.max(1, edit.res_factor)));
  const corners: Corner[] = ["tl", "tr", "bl", "br"];
  const anchorNow = $derived(anchorAt(box, bounds));

  function begin(e: PointerEvent, op: Op) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    active = { op, px: e.clientX, py: e.clientY, start: { ...box }, scale };
  }

  function move(e: PointerEvent) {
    if (!active) return;
    const dx = e.clientX - active.px;
    const dy = e.clientY - active.py;
    const { op, start } = active;
    if (op.kind === "move") {
      const r = moveWithSnap(start, dx, dy, bounds);
      dragSnap = r.snap;
      onchange({ x: r.box.x, y: r.box.y, widthCss: start.w, scale: active.scale });
    } else if (op.kind === "corner") {
      const r = scaleFromCorner(start, op.corner, dx, dy, {
        scale: active.scale,
        widthCss: start.w,
        minScale: SCALE_MIN,
        maxScale: SCALE_MAX,
        minWidth: MIN_WIDTH_CSS,
        maxWidth: bounds.width,
      });
      const b = clampBox(r.box, bounds);
      onchange({ x: b.x, y: b.y, widthCss: r.widthCss, scale: r.scale });
    } else {
      const b = clampBox(resizeWidth(start, op.side, dx, MIN_WIDTH_CSS, bounds.width), bounds);
      onchange({ x: b.x, y: b.y, widthCss: b.w, scale: active.scale });
    }
  }

  function end(e: PointerEvent) {
    if (!active) return;
    // Chromium may fold the last move into the release: take the release position too, or the
    // overlay lands a few pixels short of the mouse. A cancelled pointer keeps the last move.
    if (e.type === "pointerup") move(e);
    (e.currentTarget as Element).releasePointerCapture?.(e.pointerId);
    active = null;
    dragSnap = null;
    oncommit();
  }
</script>

<div class="markers" aria-hidden="true">
  {#each [0, 1, 2] as row (row)}
    {#each [0, 1, 2] as col (col)}
      <span
        class="marker"
        class:near={snap.col === col || snap.row === row}
        class:hit={snap.col === col && snap.row === row}
        style:left="{markerX[col]}px"
        style:top="{markerY[row]}px"
      ></span>
    {/each}
  {/each}
</div>

<div
  class="frame"
  class:dragging={active?.op.kind === "move"}
  style:left="{box.x}px"
  style:top="{box.y}px"
  style:width="{box.w}px"
  style:height="{box.h}px"
  role="presentation"
  onpointerdown={(e) => begin(e, { kind: "move" })}
  onpointermove={move}
  onpointerup={end}
  onpointercancel={end}
>
  {#each corners as corner (corner)}
    <span class="handle corner {corner}" role="presentation" onpointerdown={(e) => begin(e, { kind: "corner", corner })}></span>
  {/each}
  <span class="handle side l" role="presentation" onpointerdown={(e) => begin(e, { kind: "side", side: "l" })}></span>
  <span class="handle side r" role="presentation" onpointerdown={(e) => begin(e, { kind: "side", side: "r" })}></span>
</div>

<div class="toolbar" style:top="{toolbarTop}px" style="--r: {uiFactor}">
  <div class="row">
    <strong>{t("edit.title")}</strong>
    <span class="readout">
      {t("edit.readout", { anchor: t(`anchor.${anchorNow}`), width: Math.round((box.w / bounds.width) * 100), scale: num(scale, 2) })}
    </span>
  </div>
  <p class="help">{t("edit.help")}</p>
  <div class="row buttons">
    <button class="primary" onclick={onsave}>{t("edit.save")} <kbd>Enter</kbd></button>
    <button onclick={oncancel}>{t("common.cancel")} <kbd>Esc</kbd></button>
    {#if edit.monitor_count > 1}
      <button onclick={onmonitor}>{t("edit.nextMonitor")}</button>
      <span class="monitor">{edit.monitor_label}</span>
    {/if}
  </div>
</div>

<style>
  .markers {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .marker {
    position: absolute;
    width: 14px;
    height: 14px;
    margin: -7px 0 0 -7px;
    background:
      linear-gradient(rgba(255, 255, 255, 0.55), rgba(255, 255, 255, 0.55)) center / 14px 2px no-repeat,
      linear-gradient(rgba(255, 255, 255, 0.55), rgba(255, 255, 255, 0.55)) center / 2px 14px no-repeat;
    transition: transform 120ms ease, filter 120ms ease;
  }
  .marker.near {
    filter: drop-shadow(0 0 3px rgba(255, 211, 110, 0.8));
  }
  .marker.hit {
    transform: scale(1.5);
    background:
      linear-gradient(#ffd36e, #ffd36e) center / 14px 2px no-repeat,
      linear-gradient(#ffd36e, #ffd36e) center / 2px 14px no-repeat;
  }

  .frame {
    position: absolute;
    box-sizing: border-box;
    outline: 2px dashed rgba(255, 211, 110, 0.95);
    outline-offset: 4px;
    cursor: grab;
    touch-action: none;
  }
  .frame.dragging {
    cursor: grabbing;
  }
  .handle {
    position: absolute;
    box-sizing: border-box;
    background: #ffd36e;
    border: 2px solid #1a1400;
    border-radius: 3px;
  }
  .corner {
    width: 14px;
    height: 14px;
  }
  .corner.tl {
    left: -12px;
    top: -12px;
    cursor: nwse-resize;
  }
  .corner.br {
    right: -12px;
    bottom: -12px;
    cursor: nwse-resize;
  }
  .corner.tr {
    right: -12px;
    top: -12px;
    cursor: nesw-resize;
  }
  .corner.bl {
    left: -12px;
    bottom: -12px;
    cursor: nesw-resize;
  }
  .side {
    width: 10px;
    height: 26px;
    top: 50%;
    margin-top: -13px;
    cursor: ew-resize;
  }
  .side.l {
    left: -10px;
  }
  .side.r {
    right: -10px;
  }

  .toolbar {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    box-sizing: border-box;
    max-width: calc(100% - 32px);
    padding: calc(12px * var(--r)) calc(16px * var(--r));
    border-radius: 12px;
    background: rgba(18, 20, 26, 0.94);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.45);
    color: #e8eaf0;
    font-size: calc(13px * var(--r));
    display: grid;
    gap: calc(8px * var(--r));
    /* The stage outlines the lyrics for plate-less themes (Neon, Minimal); the toolbar has its
       own plate and must not inherit that. */
    -webkit-text-stroke: 0;
    paint-order: normal;
    text-shadow: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: calc(12px * var(--r));
    flex-wrap: wrap;
  }
  .readout,
  .monitor {
    color: #9aa1b2;
  }
  .help {
    margin: 0;
    color: #9aa1b2;
  }
  button {
    background: #262b36;
    color: #e8eaf0;
    border: 1px solid #3a4150;
    border-radius: 8px;
    padding: calc(6px * var(--r)) calc(12px * var(--r));
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    border-color: #9aa1b2;
  }
  button.primary {
    background: #ffd36e;
    color: #1a1400;
    border-color: transparent;
    font-weight: 600;
  }
  kbd {
    margin-left: 6px;
    padding: 0 5px;
    border-radius: 4px;
    font: inherit;
    font-size: 0.85em;
    background: rgba(0, 0, 0, 0.18);
  }
</style>
