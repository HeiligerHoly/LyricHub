<script lang="ts">
  // Layout "Stack" (PLAN.md 9.2): small header, then the previous line dimmed, the current line
  // with the fill and the next two lines dimmed. On a line change the list scrolls up softly
  // (220 ms); every row has a fixed height, so the window keeps its size.
  import type { Align, Line, TrackInfo } from "#lib/shared/types.js";
  import type { LocalClock } from "../clock";
  import { langOf, shownText } from "../lyricsTimeline";
  import ActiveRow from "./ActiveRow.svelte";
  import TrackHeader from "./TrackHeader.svelte";

  interface Props {
    track: TrackInfo | null;
    placeholder: { title: string; artist: string };
    align: Align;
    previous: Line | null;
    active: Line | null;
    activeKey: string;
    /** Up to two lines after the active one. */
    upcoming: Line[];
    romaji: boolean;
    glow?: boolean;
    /** Word-by-word fill; off shows the whole line in the fill colour. */
    fill?: boolean;
    status: "loading" | "instrumental" | null;
    toast: string | null;
    offline?: boolean;
    recognized?: boolean;
    clock: LocalClock;
    clockVersion: number;
    staticFill?: number | null;
  }
  let {
    track,
    placeholder,
    align,
    previous,
    active,
    activeKey,
    upcoming,
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
  }: Props = $props();

  const justify = $derived(align === "center" ? "center" : align === "end" ? "flex-end" : "flex-start");
  const rest = $derived([0, 1].map((i) => upcoming[i] ?? null));
</script>

<div class="stack" style="--justify: {justify}">
  <TrackHeader {track} {placeholder} {toast} {offline} {recognized} />
  <div class="window">
    {#key activeKey}
      <div class="list scroll">
        <div class="row side">
          {#if previous}<p class="dim" lang={langOf(shownText(previous, romaji))}>{shownText(previous, romaji)}</p>{/if}
        </div>
        <ActiveRow {active} activeKey="static" {romaji} {glow} {fill} {status} {clock} {clockVersion} {staticFill} />
        {#each rest as line, i (i)}
          <div class="row side">
            {#if line}<p class="dim" lang={langOf(shownText(line, romaji))}>{shownText(line, romaji)}</p>{/if}
          </div>
        {/each}
      </div>
    {/key}
  </div>
</div>

<style>
  .stack {
    display: grid;
    gap: calc(4px * var(--s));
    --line-size: calc(28px * var(--s));
    --side: calc(18px * 1.25 * var(--s));
  }
  .window {
    /* Clip with room for the outline and shadows of the themes (theme.ts), which reach past
       the glyphs; too long text still ends in an ellipsis. A fixed length: WebView2 ignores
       calc() with var() here. */
    overflow: clip;
    overflow-clip-margin: 16px;
  }
  .list {
    display: grid;
    gap: calc(2px * var(--s));
  }
  .row {
    display: flex;
    justify-content: var(--justify);
    align-items: center;
    min-width: 0;
    overflow: clip;
    overflow-clip-margin: 16px;
  }
  .side {
    height: var(--side);
  }
  .dim {
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
  .side:first-child .dim {
    opacity: 0.7;
  }
  /* The list moves up by one small row, the way a lyrics sheet scrolls. */
  .scroll {
    animation: scroll-in 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes scroll-in {
    from {
      transform: translateY(var(--side));
    }
    to {
      transform: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .scroll {
      animation-duration: 80ms;
    }
  }
</style>
