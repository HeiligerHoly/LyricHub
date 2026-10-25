<script lang="ts">
  // One lyric line with a karaoke fill (PLAN.md 5.6, 9.3). Two text layers: the base in a
  // dimmed colour, and an accent copy in a clipping layer. The clip slides in from the left while
  // its content slides the other way, so the text stands still and only the visible part grows.
  // The fill moves word by word: keyframe stops at each word's edges (wordFill.ts), measured once
  // per line as shares of the text width, so they hold at any font size. Both moves are
  // transforms driven by Web Animations with a steps() easing at 30 updates per second of song
  // time: the compositor runs them without JavaScript per frame, and on a 164 Hz monitor no more
  // frames are composed than needed. JavaScript only re-aligns the animations when the clock
  // changes speed or position. (An animated gradient with background-clip: text repainted every
  // frame and cost 55 % of a core in Spike B.)
  //
  // With real word times (Enhanced LRC) a third layer lets each word glow: a copy with a text
  // shadow whose opacity fades in over 120 ms at the word start and out over 300 ms after its
  // end (PLAN.md 9.4). Opacity runs on the compositor; an animated text-shadow would repaint.
  import { onMount, tick, untrack } from "svelte";
  import type { Word } from "$lib/shared/types";
  import type { LocalClock } from "./clock";
  import { langOf } from "./lyricsTimeline";
  import { charRanges, fillStops, pieces } from "./wordFill";

  interface Props {
    text: string;
    startMs: number;
    fillEndMs: number;
    /** Word times of `text`; null fills the line evenly. */
    words?: Word[] | null;
    /** Word glow for real word times; off with reduced motion. */
    glow?: boolean;
    clock: LocalClock;
    /** Increments with every clock anchor, so the fill re-aligns. */
    clockVersion: number;
    /** Fixed fill share for sample lines (edit mode, self-test); no animation. */
    staticFill?: number | null;
  }
  let { text, startMs, fillEndMs, words = null, glow = true, clock, clockVersion, staticFill = null }: Props = $props();

  const UPDATES_PER_S = 30;
  /** Long lines shrink to fit the width down to this share of the font size, then ellipsis. */
  const MIN_FIT = 0.72;
  const GLOW_IN_MS = 120;
  const GLOW_OUT_MS = 300;

  let line: HTMLElement;
  let base: HTMLElement;
  let outer: HTMLElement;
  let inner: HTMLElement;
  let glowLayer: HTMLElement | undefined = $state();
  let fit = $state(1);
  let anims: Animation[] = [];
  let resyncTimer: ReturnType<typeof setTimeout> | undefined;

  const duration = $derived(Math.max(1, fillEndMs - startMs));
  const ranges = $derived(words?.length ? charRanges(text, words) : null);
  const glowPieces = $derived(glow && words && ranges && words.some((w) => !w.estimated) ? pieces(text, ranges) : null);

  /** Left and right edge of each word as a share of the visible text width. */
  function measureEdges(): Array<[number, number]> | null {
    const node = base.firstChild;
    const box = base.getBoundingClientRect();
    if (!ranges || !(node instanceof Text) || box.width <= 0) return null;
    const range = document.createRange();
    const share = (x: number) => Math.min(1, Math.max(0, (x - box.left) / box.width));
    const edges: Array<[number, number]> = [];
    for (const [from, to] of ranges) {
      range.setStart(node, Math.min(from, node.length));
      range.setEnd(node, Math.min(to, node.length));
      const r = range.getBoundingClientRect();
      edges.push([share(r.left), share(r.right)]);
    }
    return edges;
  }

  function build() {
    for (const a of anims) a.cancel();
    const steps = Math.max(2, Math.round((duration / 1000) * UPDATES_PER_S));
    const options: KeyframeAnimationOptions = { duration, easing: `steps(${steps}, jump-none)`, fill: "both" };
    const edges = words && ranges ? measureEdges() : null;
    const stops = edges && words ? fillStops(words, edges, startMs, fillEndMs) : fillStops([], [], startMs, fillEndMs);
    anims = [
      outer.animate(stops.map((s) => ({ offset: s.offset, transform: `translateX(${(s.ratio - 1) * 100}%)` })), options),
      inner.animate(stops.map((s) => ({ offset: s.offset, transform: `translateX(${(1 - s.ratio) * 100}%)` })), options),
    ];
    if (glowLayer && words && staticFill === null) {
      for (const el of glowLayer.querySelectorAll<HTMLElement>("[data-word]")) {
        const w = words[Number(el.dataset.word)];
        if (!w || w.estimated) continue;
        const sung = Math.max(1, w.end_ms - w.start_ms);
        const total = sung + GLOW_OUT_MS;
        const lit = sung / total;
        anims.push(
          el.animate(
            [
              { offset: 0, opacity: 0, easing: "ease-out" },
              { offset: Math.min(GLOW_IN_MS / total, lit), opacity: 1 },
              { offset: lit, opacity: 1, easing: "ease-out" },
              { offset: 1, opacity: 0 },
            ],
            { duration: total, delay: w.start_ms - startMs, fill: "both" },
          ),
        );
      }
    }
    sync();
  }

  function sync() {
    clearTimeout(resyncTimer);
    if (!anims.length) return;
    const now = performance.now();
    const t = staticFill !== null ? staticFill * duration : clock.display(now) - startMs;
    const speed = staticFill !== null ? 0 : clock.speed(now);
    const done = t >= duration + GLOW_OUT_MS;
    for (const a of anims) {
      a.currentTime = Math.max(0, t);
      if (speed > 0 && !done) {
        a.playbackRate = speed;
        if (a.playState !== "running") a.play();
      } else {
        a.pause();
      }
    }
    // Slew or blend ends: the speed changes, so the animations need the new rate.
    const next = clock.nextChange(now);
    if (!done && speed > 0 && Number.isFinite(next)) resyncTimer = setTimeout(sync, next - now + 1);
  }

  function measureFit() {
    fit = 1;
    requestAnimationFrame(() => {
      const available = line.parentElement?.clientWidth ?? 0;
      const natural = base.scrollWidth;
      if (available > 0 && natural > available) fit = Math.max(MIN_FIT, available / natural);
    });
  }

  onMount(() => {
    build();
    // The plate can widen after the line appeared (lyrics of a long song just arrived).
    const observer = new ResizeObserver(measureFit);
    if (line.parentElement) observer.observe(line.parentElement);
    return () => {
      observer.disconnect();
      clearTimeout(resyncTimer);
      for (const a of anims) a.cancel();
    };
  });

  $effect(() => {
    void clockVersion;
    sync();
  });

  // The text switches script while the line runs (romaji on or off): fit and measure again.
  let shownText = untrack(() => text);
  $effect(() => {
    if (text === shownText) return;
    shownText = text;
    measureFit();
    tick().then(build);
  });
</script>

<p class="line" bind:this={line} lang={langOf(text)} style="--fit: {fit}">
  <span class="base" bind:this={base}>{text}</span>
  <span class="reveal" bind:this={outer} aria-hidden="true"><span class="fill" bind:this={inner}>{text}</span></span>
  {#if glowPieces}
    <span class="glow" bind:this={glowLayer} aria-hidden="true"
      >{#each glowPieces as p, i (i)}{#if p.word === null}{p.text}{:else}<span class="lit" data-word={p.word}>{p.text}</span>{/if}{/each}</span
    >
  {/if}
</p>

<style>
  .line {
    position: relative;
    margin: 0;
    width: fit-content;
    max-width: 100%;
    font-size: calc(var(--line-size) * var(--fit));
    font-weight: 650;
    line-height: 1.25;
    white-space: nowrap;
    line-break: strict;
  }
  .base,
  .fill,
  .glow {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .base {
    color: var(--text-base, rgba(255, 255, 255, 0.55));
  }
  /* The clip reaches 18 px past the text, so its soft leading edge lies outside the text once
     the line is full; the fill's padding keeps its text box (and ellipsis) equal to the base. */
  .reveal {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    right: -18px;
    overflow: hidden;
    mask-image: linear-gradient(90deg, #000 calc(100% - 18px), transparent);
    will-change: transform;
  }
  .fill {
    box-sizing: border-box;
    padding-right: 18px;
    color: var(--accent, #ffd36e);
    text-shadow: var(--fill-shadow, none);
    will-change: transform;
  }
  .glow {
    position: absolute;
    inset: 0;
    color: transparent;
    pointer-events: none;
  }
  .lit {
    color: var(--accent, #ffd36e);
    text-shadow:
      0 0 calc(6px * var(--s, 1)) var(--accent, #ffd36e),
      0 0 calc(14px * var(--s, 1)) var(--glow, rgba(255, 211, 110, 0.6));
    opacity: 0;
    will-change: opacity;
  }
</style>
