<script lang="ts">
  // Layout "Karaoke" (PLAN.md 9.2): small header with cover and title – artist, the current line
  // large with the fill, the next line small and dimmed below. Every row has a fixed height, so
  // the window keeps its size while lines change. With romaji on (docs/romaji.md) both rows show
  // the romaji of Japanese lines.
  import type { Align, Line, TrackInfo } from "#lib/shared/types.js";
  import type { LocalClock } from "../clock";
  import { langOf, shownText } from "../lyricsTimeline";
  import ActiveRow from "./ActiveRow.svelte";
  import TrackHeader from "./TrackHeader.svelte";

  interface Props {
    track: TrackInfo | null;
    placeholder: { title: string; artist: string };
    align: Align;
    active: Line | null;
    /** Changes when the active line changes, for the enter transition. */
    activeKey: string;
    next: Line | null;
    /** Japanese lines in romaji. */
    romaji: boolean;
    /** Word glow for real word times (off with reduced motion). */
    glow?: boolean;
    /** Word-by-word fill; off shows the whole line in the fill colour. */
    fill?: boolean;
    /** What the active row shows when no line is active. */
    status: "loading" | "instrumental" | null;
    toast: string | null;
    offline?: boolean;
    recognized?: boolean;
    clock: LocalClock;
    clockVersion: number;
    staticFill?: number | null;
    /** Calibration: time left until the next line starts, drawn as a shrinking bar. */
    countdown?: { key: number; ms: number } | null;
  }
  let {
    track,
    placeholder,
    align,
    active,
    activeKey,
    next,
    romaji,
    glow = true,
    fill = true,
    status,
    toast,
    offline = false,
    recognized = false,
    clock,
    clockVersion,
    staticFill = null,
    countdown = null,
  }: Props = $props();

  const nextText = $derived(next ? shownText(next, romaji) : "");
  const justify = $derived(align === "center" ? "center" : align === "end" ? "flex-end" : "flex-start");
</script>

<div class="karaoke" style="--justify: {justify}">
  <TrackHeader {track} {placeholder} {toast} {offline} {recognized} />

  {#if countdown}
    {#key countdown.key}
      <div class="countdown" aria-hidden="true"><span style="animation-duration: {Math.max(1, countdown.ms)}ms"></span></div>
    {/key}
  {/if}

  <ActiveRow {active} {activeKey} {romaji} {glow} {fill} {status} {clock} {clockVersion} {staticFill} />

  <div class="row upcoming">
    {#key next?.start_ms ?? -1}
      {#if next}
        <p class="next enter-next" lang={langOf(nextText)}>{nextText}</p>
      {/if}
    {/key}
  </div>
</div>

<style>
  .karaoke {
    position: relative;
    display: grid;
    gap: calc(4px * var(--s));
    --line-size: calc(28px * var(--s));
  }
  /* Sits on the gap below the header, so the layout height does not change. */
  .countdown {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(25px * var(--s));
    height: calc(3px * var(--s));
    border-radius: 2px;
    background: color-mix(in srgb, var(--text) 12%, transparent);
    overflow: hidden;
  }
  .countdown span {
    display: block;
    height: 100%;
    background: var(--accent);
    transform-origin: left center;
    animation-name: shrink;
    animation-timing-function: linear;
    animation-fill-mode: both;
  }
  @keyframes shrink {
    from {
      transform: scaleX(1);
    }
    to {
      transform: scaleX(0);
    }
  }
  .row {
    display: flex;
    justify-content: var(--justify);
    align-items: center;
    min-width: 0;
    /* Clip with room for the outline and shadows of the themes (theme.ts), which reach past
       the glyphs; too long text still ends in an ellipsis. A fixed length: WebView2 ignores
       calc() with var() here. */
    overflow: clip;
    overflow-clip-margin: 16px;
  }
  .upcoming {
    height: calc(18px * 1.25 * var(--s));
  }
  .next {
    margin: 0;
    min-width: 0;
    overflow: clip;
    overflow-clip-margin: 16px;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: calc(18px * var(--s));
    line-height: 1.25;
    font-weight: 500;
    color: var(--text-next);
  }
  .enter-next {
    animation: next-in 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes next-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .enter-next {
      animation-duration: 80ms;
    }
  }
</style>
