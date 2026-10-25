<script lang="ts">
  // One lyric line with a line-level karaoke fill (PLAN.md 9.3). Two text layers: the base in a
  // dimmed colour, and an accent copy in a clipping layer. The clip slides in from the left while
  // its content slides the other way, so the text stands still and only the visible part grows.
  // Both moves are transforms driven by Web Animations with a steps() easing at 30 updates per
  // second of song time: the compositor runs them without JavaScript per frame, and on a 164 Hz
  // monitor no more frames are composed than needed. JavaScript only re-aligns the animations
  // when the clock changes speed or position. (An animated gradient with background-clip: text
  // repainted every frame and cost 55 % of a core in Spike B.)
  import { onMount } from "svelte";
  import type { LocalClock } from "./clock";
  import { langOf } from "./lyricsTimeline";

  interface Props {
    text: string;
    startMs: number;
    fillEndMs: number;
    clock: LocalClock;
    /** Increments with every clock anchor, so the fill re-aligns. */
    clockVersion: number;
    /** Fixed fill share for sample lines (edit mode, self-test); no animation. */
    staticFill?: number | null;
  }
  let { text, startMs, fillEndMs, clock, clockVersion, staticFill = null }: Props = $props();

  const UPDATES_PER_S = 30;
  /** Long lines shrink to fit the width down to this share of the font size, then ellipsis. */
  const MIN_FIT = 0.72;

  let line: HTMLElement;
  let base: HTMLElement;
  let outer: HTMLElement;
  let inner: HTMLElement;
  let fit = $state(1);
  let anims: Animation[] = [];
  let resyncTimer: ReturnType<typeof setTimeout> | undefined;

  const duration = $derived(Math.max(1, fillEndMs - startMs));

  function build() {
    for (const a of anims) a.cancel();
    const steps = Math.max(2, Math.round((duration / 1000) * UPDATES_PER_S));
    const options: KeyframeAnimationOptions = { duration, easing: `steps(${steps}, jump-none)`, fill: "both" };
    anims = [
      outer.animate([{ transform: "translateX(-100%)" }, { transform: "translateX(0)" }], options),
      inner.animate([{ transform: "translateX(100%)" }, { transform: "translateX(0)" }], options),
    ];
    sync();
  }

  function sync() {
    clearTimeout(resyncTimer);
    if (!anims.length) return;
    const now = performance.now();
    const t = staticFill !== null ? staticFill * duration : clock.display(now) - startMs;
    const speed = staticFill !== null ? 0 : clock.speed(now);
    const done = t >= duration;
    for (const a of anims) {
      a.currentTime = Math.min(t, duration);
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
</script>

<p class="line" bind:this={line} lang={langOf(text)} style="--fit: {fit}">
  <span class="base" bind:this={base}>{text}</span>
  <span class="reveal" bind:this={outer} aria-hidden="true"><span class="fill" bind:this={inner}>{text}</span></span>
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
  .fill {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .base {
    color: rgba(255, 255, 255, 0.55);
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
    will-change: transform;
  }
</style>
